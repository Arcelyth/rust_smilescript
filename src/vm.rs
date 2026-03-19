use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::chunk::*;
use crate::compiler::*;
use crate::debug::Disassembler;
use crate::error::*;
use crate::gc::*;
use crate::object::*;
use crate::parser::*;
use crate::value::*;

macro_rules! binary_op {
    ($vm:ident, $value_type:ident, $op:tt) => {
        {
            let b = $vm.pop();
            let a = $vm.pop();
            match (b, a) {
                (Value::Number(b_val), Value::Number(a_val)) => {
                    $vm.push(Value::$value_type(a_val $op b_val));
                }
                _ => return $vm.runtime_error("Operands must be numbers."),
            }
        }
    };
}

pub struct CallFrame {
    pub closure: GcRef,
    ip: usize,
    slot: usize,
}

impl CallFrame {
    fn new(closure: GcRef, slot: usize) -> Self {
        Self {
            closure,
            ip: 0,
            slot,
        }
    }
}

pub struct Vm {
    pub frames: Vec<CallFrame>,
    pub stack: Vec<Value>,
    pub globals: HashMap<String, Value>,
    pub open_upvalues: Vec<GcRef>,
    pub gc: Gc,
}

impl Vm {
    const STACK_MAX: usize = 256;
    const FRAME_MAX: usize = 64;

    pub fn new() -> Self {
        let mut vm = Self {
            frames: Vec::with_capacity(Self::FRAME_MAX),
            stack: Vec::with_capacity(Self::STACK_MAX),
            globals: HashMap::new(),
            open_upvalues: Vec::with_capacity(Self::STACK_MAX),
            gc: Gc::new(),
        };
        vm.define_native("clock", NativeFunction(clock_native));
        vm
    }

    pub fn interpret(&mut self, src: &str) -> Result<(), SmsError> {
        let compiler = Compiler::new("", FunctionType::Script, &mut self.gc);
        let mut parser = Parser::new(src, compiler, &mut self.gc);

        let function = parser.compile();
        if let Some(f) = function {
            self.maybe_gc();
            let f_ref = self.gc.alloc(Obj::Function(f));
            let clos = Closure::new(f_ref);
            self.maybe_gc();
            let c_ref = self.gc.alloc(Obj::Closure(clos));
            self.push(Value::Obj(c_ref));
            self.call(c_ref, 0);
            self.run()
        } else {
            Err(SmsError::CompileError)
        }
    }

    pub fn read_byte(&mut self) -> OpCode {
        let frame = self.current_frame_mut();
        let ip = frame.ip;
        frame.ip += 1;

        let clos_ref = frame.closure;
        if let Obj::Closure(c) = self.gc.deref(clos_ref) {
            if let Obj::Function(f) = self.gc.deref(c.function) {
                return f.chunk.code[ip].clone();
            }
        }
        panic!("Invalid frame state");
    }

    pub fn run(&mut self) -> Result<(), SmsError> {
        loop {
            #[cfg(feature = "debug_trace_execution")]
            {
                print!("         ");
                for i in self.stack.iter() {
                    print!("[ {:?} ]", i);
                }
                println!();
            }
            match self.read_byte() {
                OpCode::Constant(c) => {
                    let v = self.read_constant(c);
                    self.push(v);
                }
                OpCode::Print => {
                    let val = self.pop();
                    self.print_value(&val);
                }
                OpCode::Return => {
                    let res = self.pop();
                    let frame = self.frames.pop().expect("No frame to pop.");
                    self.close_upvalues(frame.slot);
                    if self.frames.is_empty() {
                        return Ok(());
                    }
                    self.stack.truncate(frame.slot);
                    self.push(res);
                }
                OpCode::Nil => self.push(Value::Nil),
                OpCode::True => self.push(Value::Bool(true)),
                OpCode::False => self.push(Value::Bool(false)),
                OpCode::Pop => {
                    self.pop();
                }
                OpCode::GetLocal(slot) => {
                    let idx = self.current_frame().slot + slot as usize;
                    let v = self.stack[idx].clone();
                    self.push(v);
                }
                OpCode::SetLocal(slot) => {
                    let idx = self.current_frame().slot + slot as usize;
                    self.stack[idx] = self.peek(0);
                }
                OpCode::SetGlobal(idx) => {
                    let name = self.read_string(idx);
                    let v = self.peek(0);

                    if self.globals.insert(name.clone(), v).is_none() {
                        self.globals.remove(&name);
                        let msg = format!("Undefined variable '{}'.", name);
                        return self.runtime_error(&msg);
                    }
                }
                OpCode::GetGlobal(idx) => {
                    let name = self.read_string(idx);
                    if let Some(v) = self.globals.get(&name) {
                        let val = v.clone();
                        self.push(val)
                    } else {
                        let msg = format!("Undefined variable '{}'", name);
                        return self.runtime_error(&msg);
                    }
                }
                OpCode::DefineGlobal(idx) => {
                    let name = self.read_string(idx);
                    let v = self.pop();
                    self.globals.insert(name, v);
                }
                OpCode::Equal => {
                    let a = self.pop();
                    let b = self.pop();
                    self.push(Value::Bool(values_equal(&a, &b)));
                }
                OpCode::Greater => binary_op!(self, Bool, >),
                OpCode::Less => binary_op!(self, Bool, <),
                OpCode::Add => {
                    let b = self.pop();
                    let a = self.pop();
                    match (a.clone(), b.clone()) {
                        (Value::Number(n1), Value::Number(n2)) => {
                            self.push(Value::Number(n1 + n2));
                        }
                        (Value::Obj(r1), Value::Obj(r2)) => {
                            if self.is_string(r1) && self.is_string(r2) {
                                let s1 = self.get_string(r1);
                                let s2 = self.get_string(r2);
                                let new_str = format!("{}{}", s1, s2);
                                self.maybe_gc();
                                let s_ref = self.gc.alloc(Obj::String(new_str));
                                self.push(Value::Obj(s_ref));
                            } else {
                                self.push(a);
                                self.push(b);
                                return self
                                    .runtime_error("Operands must be two numbers or two strings.");
                            }
                        }
                        _ => {
                            self.push(a);
                            self.push(b);
                            return self
                                .runtime_error("Operands must be two numbers or two strings.");
                        }
                    }
                }
                OpCode::Subtract => binary_op!(self, Number, -),
                OpCode::Multiply => binary_op!(self, Number, *),
                OpCode::Divide => binary_op!(self, Number, /),
                OpCode::Negate => {
                    let n = match self.pop() {
                        Value::Number(n) => n,
                        _ => return self.runtime_error("Operand must be a number"),
                    };
                    self.push(Value::Number(-n));
                }
                OpCode::Not => {
                    let v = self.pop();
                    self.push(Value::Bool(self.is_falsey(&v)));
                }
                OpCode::JumpIfFalse(offset) => {
                    if self.is_falsey(&self.peek(0)) {
                        self.current_frame_mut().ip += offset as usize;
                    }
                }
                OpCode::Jump(offset) => {
                    self.current_frame_mut().ip += offset as usize;
                }
                OpCode::Loop(offset) => self.current_frame_mut().ip -= offset as usize,
                OpCode::Call(arg_count) => {
                    let arg_c = arg_count as usize;
                    let callee = self.peek(arg_c);
                    if !self.call_value(callee, arg_c) {
                        return Err(SmsError::RuntimeError);
                    }
                }
                OpCode::Closure(c) => {
                    let func_val = self.read_constant(c);
                    if let Value::Obj(func_ref) = func_val {
                        let f_upvalues = if let Obj::Function(f) = self.gc.deref(func_ref) {
                            f.upvalues.clone()
                        } else {
                            return self.runtime_error("Expected function for closure.");
                        };

                        let mut clos = Closure::new(func_ref);
                        for upvalue in f_upvalues {
                            let upv = if upvalue.is_local {
                                let loc = self.current_frame().slot + upvalue.index as usize;
                                self.capture_upvalue(loc)
                            } else {
                                let current_clos_ref = self.current_frame().closure;
                                if let Obj::Closure(c) = self.gc.deref(current_clos_ref) {
                                    c.upvalues[upvalue.index as usize]
                                } else {
                                    panic!("Not a closure");
                                }
                            };
                            clos.upvalues.push(upv);
                        }
                        self.maybe_gc();
                        let clos_ref = self.gc.alloc(Obj::Closure(clos));
                        self.push(Value::Obj(clos_ref));
                    } else {
                        return self.runtime_error("Closure without function value.");
                    }
                }
                OpCode::GetUpValue(slot) => {
                    let upv_ref = {
                        let current_clos_ref = self.current_frame().closure;
                        if let Obj::Closure(c) = self.gc.deref(current_clos_ref) {
                            c.upvalues[slot as usize]
                        } else {
                            panic!("Not a closure")
                        }
                    };

                    let value = if let Obj::UpValue(upv) = self.gc.deref(upv_ref) {
                        if let Some(val) = &upv.closed {
                            val.clone()
                        } else {
                            self.stack[upv.location].clone()
                        }
                    } else {
                        panic!("Not an upvalue")
                    };

                    self.push(value);
                }
                OpCode::SetUpValue(slot) => {
                    let upv_ref = {
                        let current_clos_ref = self.current_frame().closure;
                        if let Obj::Closure(c) = self.gc.deref(current_clos_ref) {
                            c.upvalues[slot as usize]
                        } else {
                            panic!("Not a closure")
                        }
                    };

                    let value = self.peek(0);

                    if let Obj::UpValue(upv) = self.gc.deref_mut(upv_ref) {
                        if upv.closed.is_none() {
                            self.stack[upv.location] = value;
                        } else {
                            upv.closed = Some(value);
                        }
                    }
                }
                OpCode::CloseUpValue => {
                    let pos = self.stack.len() - 1;
                    self.close_upvalues(pos);
                    self.pop();
                }

                _ => return Ok(()),
            }
        }
    }

    pub fn push(&mut self, value: Value) {
        self.stack.push(value);
    }

    pub fn pop(&mut self) -> Value {
        self.stack.pop().expect("Stack is empty")
    }

    fn peek(&self, n: usize) -> Value {
        let size = self.stack.len();
        self.stack[size - 1 - n].clone()
    }

    fn is_falsey(&self, val: &Value) -> bool {
        match *val {
            Value::Bool(b) => !b,
            Value::Nil => true,
            _ => false,
        }
    }

    fn read_constant(&self, idx: u8) -> Value {
        let clos_ref = self.current_frame().closure;
        if let Obj::Closure(c) = self.gc.deref(clos_ref) {
            if let Obj::Function(f) = self.gc.deref(c.function) {
                return f.chunk.constants[idx as usize].clone();
            }
        }
        panic!("Invalid state reading constant")
    }

    pub fn read_string(&self, idx: u8) -> String {
        let val = self.read_constant(idx);
        if let Value::Obj(s_ref) = val {
            if let Obj::String(s) = self.gc.deref(s_ref) {
                return s.clone();
            }
        }
        panic!("Constant is not String!")
    }

    fn is_string(&self, r: GcRef) -> bool {
        matches!(self.gc.deref(r), Obj::String(_))
    }

    fn get_string(&self, r: GcRef) -> String {
        if let Obj::String(s) = self.gc.deref(r) {
            s.clone()
        } else {
            unreachable!()
        }
    }

    fn capture_upvalue(&mut self, pos: usize) -> GcRef {
        for upv_ref in &self.open_upvalues {
            if let Obj::UpValue(upv) = self.gc.deref(*upv_ref) {
                if upv.location == pos {
                    return *upv_ref;
                }
            }
        }
        self.maybe_gc();
        let upv_ref = self.gc.alloc(Obj::UpValue(UpValue::new(pos)));
        self.open_upvalues.push(upv_ref);
        upv_ref
    }

    fn close_upvalues(&mut self, pos: usize) {
        let stack = &self.stack;
        self.open_upvalues.retain(|upv_ref| {
            if let Obj::UpValue(upv) = self.gc.deref_mut(*upv_ref) {
                if upv.location >= pos {
                    upv.closed = Some(stack[upv.location].clone());
                    return false;
                }
            }
            true
        });
    }

    fn current_frame(&self) -> &CallFrame {
        self.frames.last().unwrap()
    }

    fn current_frame_mut(&mut self) -> &mut CallFrame {
        self.frames.last_mut().unwrap()
    }

    fn call_value(&mut self, callee: Value, arg_count: usize) -> bool {
        match callee {
            Value::Obj(clos_ref) => {
                if let Obj::Closure(_) = self.gc.deref(clos_ref) {
                    return self.call(clos_ref, arg_count);
                }
                let _ = self.runtime_error("Can only call functions and classes");
                false
            }
            Value::Native(native) => {
                let offset = self.stack.len() - arg_count;
                let res = native.0(&self.stack[offset..]);
                self.stack.truncate(offset - 1);
                self.push(res);
                true
            }
            _ => {
                let _ = self.runtime_error("Can only call functions and classes");
                false
            }
        }
    }

    fn call(&mut self, clos_ref: GcRef, arg_count: usize) -> bool {
        let arity = if let Obj::Closure(c) = self.gc.deref(clos_ref) {
            if let Obj::Function(f) = self.gc.deref(c.function) {
                f.arity
            } else {
                0
            }
        } else {
            0
        };

        if arg_count != arity {
            let msg = format!("Expected {} arguments but got {}.", arity, arg_count);
            let _ = self.runtime_error(&msg);
            return false;
        }

        if self.frames.len() == Self::FRAME_MAX {
            let _ = self.runtime_error("Stack overflow.");
            return false;
        }

        let stack_len = self.stack.len();
        self.frames
            .push(CallFrame::new(clos_ref, stack_len - arg_count - 1));
        true
    }

    fn define_native(&mut self, name: &str, func: NativeFunction) {
        self.globals.insert(name.to_string(), Value::Native(func));
    }

    pub fn runtime_error(&mut self, msg: &str) -> Result<(), SmsError> {
        eprintln!("{}", msg);

        for frame in self.frames.iter().rev() {
            let inst = if frame.ip > 0 { frame.ip - 1 } else { 0 };

            let (line, name) = if let Obj::Closure(c) = self.gc.deref(frame.closure) {
                if let Obj::Function(f) = self.gc.deref(c.function) {
                    let line = f.chunk.lines[inst];
                    let name = if let Obj::String(f_name) = self.gc.deref(f.name) {
                        if f_name.is_empty() {
                            "script".to_string()
                        } else {
                            format!("{}()", f_name)
                        }
                    } else {
                        "script".to_string()
                    };
                    (line, name)
                } else {
                    (0, "unknown".to_string())
                }
            } else {
                (0, "unknown".to_string())
            };

            eprintln!("[line {}] in {}", line, name);
        }

        self.frames.clear();
        Err(SmsError::RuntimeError)
    }

    fn print_value(&self, v: &Value) {
        match v {
            Value::Number(n) => println!("{}", n),
            Value::Bool(b) => println!("{}", b),
            Value::Nil => println!("nil"),
            Value::Native(_) => println!("<native fn>"),
            Value::Obj(r) => match self.gc.deref(*r) {
                Obj::String(s) => println!("{}", s),
                Obj::Closure(c) => {
                    if let Obj::Function(f) = self.gc.deref(c.function) {
                        if let Obj::String(f_name) = self.gc.deref(f.name) {
                            if f_name.is_empty() {
                                println!("<script>");
                            } else {
                                println!("<fn {}>", f_name);
                            }
                        }
                    }
                }
                _ => println!("object"),
            },
        }
    }
}

pub fn clock_native(_args: &[Value]) -> Value {
    let start = SystemTime::now();
    let since_the_epoch = start
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards");

    Value::Number(since_the_epoch.as_secs_f64())
}
