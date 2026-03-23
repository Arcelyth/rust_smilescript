use crate::object::{GcObject, Obj};
use crate::value::Value;
use crate::vm::*;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct GcRef(pub usize);

pub struct Gc {
    pub objects: Box<[Option<GcObject>]>,
    pub capacity: usize,
    pub object_count: usize,
    pub free_slots: Vec<usize>,
    pub gray_stack: Vec<usize>,
    pub bytes_allocated: usize,
    pub next_gc: usize,
}

const NONE: Option<GcObject> = None;

impl Gc {
    const GC_HEAP_GROW_FACTOR: usize = 2;
    pub fn new() -> Self {
        Self {
            objects: Box::new([NONE; 1024]),
            capacity: 1024,
            object_count: 0,
            free_slots: Vec::new(),
            gray_stack: Vec::new(),
            bytes_allocated: 0,
            next_gc: 1024,
        }
    }

    fn grow(&mut self) {
        let old_capacity = self.capacity;
        let new_capacity = old_capacity * Self::GC_HEAP_GROW_FACTOR;
        let old_objects = std::mem::replace(&mut self.objects, vec![None; 0].into_boxed_slice());
        let mut new_vec = old_objects.into_vec();
        new_vec.resize_with(new_capacity, || None);
        self.objects = new_vec.into_boxed_slice();
        self.capacity = new_capacity;

        #[cfg(feature = "debug_log_gc")]
        println!("Heap grown from {} to {}", old_capacity, new_capacity);
    }

    // allocate object
    pub fn alloc(&mut self, obj: Obj) -> GcRef {
        if self.free_slots.is_empty() && self.object_count >= self.capacity {
            self.grow();
        }

        self.bytes_allocated += 1;
        let gc_obj = GcObject::new(obj);

        // prioritize the reuse of the empty slots that have been swept out
        if let Some(index) = self.free_slots.pop() {
            self.objects[index] = Some(gc_obj);
            GcRef(index)
        } else {
            self.objects[self.object_count] = Some(gc_obj);
            self.object_count += 1;
            GcRef(self.object_count - 1)
        }
    }

    pub fn deref(&self, gc_ref: GcRef) -> &Obj {
        &self.objects[gc_ref.0].as_ref().unwrap().obj
    }

    pub fn deref_mut(&mut self, gc_ref: GcRef) -> &mut Obj {
        &mut self.objects[gc_ref.0].as_mut().unwrap().obj
    }

    pub fn collect_garbage(&mut self) {
        #[cfg(feature = "debug_log_gc")]
        {
            println!("== GC begin ==");
        }

        let _before = self.bytes_allocated;
        self.trace_references();
        self.sweep();
        self.next_gc *= Self::GC_HEAP_GROW_FACTOR;

        #[cfg(feature = "debug_log_gc")]
        println!(
            "== GC end == Collected {} objects",
            _before - self.bytes_allocated
        );
    }

    pub fn mark_value(&mut self, value: &Value) {
        if let Value::Obj(gc_ref) = value {
            self.mark_object(*gc_ref);
        }
    }

    pub fn mark_object(&mut self, gc_ref: GcRef) {
        let index = gc_ref.0;
        let object = self.objects[index]
            .as_mut()
            .expect("Attempting to mark an object that has been released");

        // prevent causing infinite recursion from circular references
        if object.is_marked {
            return;
        }

        #[cfg(feature = "debug_log_gc")]
        {
            println!("  mark object {}", index);
        }

        object.is_marked = true;
        self.gray_stack.push(index);
    }

    pub fn trace_references(&mut self) {
        while let Some(index) = self.gray_stack.pop() {
            self.blacken_object(index);
        }
    }

    pub fn blacken_object(&mut self, index: usize) {
        #[cfg(feature = "debug_log_gc")]
        {
            println!("blacken {:?}", self.objects[index]);
        }

        let gc_obj = self.objects[index].take().unwrap();
        match &gc_obj.obj {
            Obj::Function(f) => {
                self.mark_object(f.name);
                for constant in &f.chunk.constants {
                    self.mark_value(constant);
                }
            }
            Obj::Closure(c) => {
                self.mark_object(c.function);
                for upvalue in &c.upvalues {
                    self.mark_object(*upvalue);
                }
            }
            Obj::UpValue(u) => {
                if let Some(closed) = &u.closed {
                    self.mark_value(closed);
                }
            }
            Obj::Instance(instance) => {
                self.mark_object(instance.class);
                for (_name, value) in &instance.fields {
                    self.mark_value(value);
                }
            }
            Obj::Class(class) => {
                self.mark_object(class.name);
                for (_name, value) in &class.methods {
                    self.mark_value(value);
                }
            }
            Obj::BoundMethod(method) => {
                self.mark_value(&method.receiver);
                self.mark_object(method.method);
            }
            _ => {}
        }
        self.objects[index] = Some(gc_obj);
    }

    fn sweep(&mut self) {
        for i in 0..self.object_count {
            if let Some(gc_obj) = &mut self.objects[i] {
                if gc_obj.is_marked {
                    gc_obj.is_marked = false;
                } else {
                    #[cfg(feature = "debug_log_gc")]
                    println!("  sweep object {}", i);

                    self.objects[i] = None;
                    self.free_slots.push(i);
                    self.bytes_allocated -= 1;
                }
            }
        }
    }
}

impl Vm {
    fn collect_garbage(&mut self) {
        for i in 0..self.sp {
            self.gc.mark_value(&self.stack[i]);
        }
        for (_name, value) in &self.globals {
            //            self.gc.mark_object(*name);
            self.gc.mark_value(value);
        }
        for frame in &self.frames {
            self.gc.mark_object(frame.closure);
        }
        for upvalue in &self.open_upvalues {
            self.gc.mark_object(*upvalue);
        }
        self.gc.mark_object(self.init_string);
        self.gc.collect_garbage();
    }

    pub fn maybe_gc(&mut self) {
        if self.gc.bytes_allocated >= self.gc.next_gc {
            self.collect_garbage();
        }
    }
}
