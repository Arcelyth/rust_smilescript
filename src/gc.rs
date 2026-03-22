use crate::object::{GcObject, Obj};
use crate::value::Value;
use crate::vm::*;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct GcRef(pub usize);

pub struct Gc {
    pub objects: Vec<Option<GcObject>>,
    pub free_slots: Vec<usize>,
    pub gray_stack: Vec<usize>,
    pub bytes_allocated: usize,
    pub next_gc: usize,
}

impl Gc {
    const GC_HEAP_GROW_FACTOR: usize = 2;
    pub fn new() -> Self {
        Self {
            objects: Vec::new(),
            free_slots: Vec::new(),
            gray_stack: Vec::new(),
            bytes_allocated: 0,
            next_gc: 1024,
        }
    }

    // allocate object
    pub fn alloc(&mut self, obj: Obj) -> GcRef {
        self.bytes_allocated += 1;
        let gc_obj = GcObject::new(obj);

        // prioritize the reuse of the empty slots that have been swept out
        if let Some(index) = self.free_slots.pop() {
            self.objects[index] = Some(gc_obj);
            GcRef(index)
        } else {
            self.objects.push(Some(gc_obj));
            GcRef(self.objects.len() - 1)
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

        let before = self.bytes_allocated;
        self.trace_references();
        self.sweep();
        self.next_gc *= Self::GC_HEAP_GROW_FACTOR;

        #[cfg(feature = "debug_log_gc")]
        println!(
            "== GC end == Collected {} objects",
            before - self.bytes_allocated
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

        let mut refs_to_mark = Vec::new();
        let mut values_to_mark = Vec::new();

        if let Some(gc_obj) = &self.objects[index] {
            match &gc_obj.obj {
                Obj::Function(f) => {
                    refs_to_mark.push(f.name);
                    for constant in &f.chunk.constants {
                        values_to_mark.push(constant.clone());
                    }
                }
                Obj::Closure(c) => {
                    refs_to_mark.push(c.function);
                    for upvalue in &c.upvalues {
                        refs_to_mark.push(*upvalue);
                    }
                }
                Obj::UpValue(u) => {
                    if let Some(closed) = &u.closed {
                        values_to_mark.push(closed.clone());
                    }
                }
                Obj::Instance(instance) => {
                    refs_to_mark.push(instance.class);
                    for (_name, value) in &instance.fields {
                        values_to_mark.push(value.clone());
                    }
                }
                Obj::Class(class) => {
                    refs_to_mark.push(class.name);
                    for (_name, value) in &class.methods{
                        values_to_mark.push(value.clone());
                    }
                }
                Obj::BoundMethod(method) => {
                    values_to_mark.push(method.receiver.clone());
                    refs_to_mark.push(method.method);
                }
                _ => {}
            }
        }

        for r in refs_to_mark {
            self.mark_object(r);
        }

        for v in values_to_mark {
            self.mark_value(&v);
        }
    }

    fn sweep(&mut self) {
        for i in 0..self.objects.len() {
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
        for value in &self.stack {
            self.gc.mark_value(value);
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
