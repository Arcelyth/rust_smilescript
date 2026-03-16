use std::collections::HashMap;
use std::rc::Rc;

use crate::chunk::*;
use crate::compiler::*;
use crate::debug::Disassembler;
use crate::error::*;
use crate::parser::*;
use crate::value::*;
use crate::object::*;

macro_rules! binary_op {
    ($vm:ident, $value_type:ident, $op:tt) => {
        match ($vm.pop(), $vm.pop()) {
            (Value::Number(b), Value::Number(a)) => {
                $vm.push(Value::$value_type(a $op b));
            }
            _ => return $vm.runtime_error("Operands must be numbers."),
        }
    };
}

pub struct CallFrame {
    function: Rc<Function>,
    ip: usize, 
    slot: usize,
}

impl CallFrame {
    fn new(func: Rc<Function>, slot: usize) -> Self {
        Self {
            function: func,
            ip: 0, 
            slot,
        }
    }
}

pub struct Vm {
    pub frames: Vec<CallFrame>,
    pub chunk: Chunk,
    pub stack: Vec<Value>,
    pub globals: HashMap<Rc<str>, Value>,
}

impl Vm {
    const STACK_MAX: usize = 256;
    const FRAME_MAX: usize = 64;

    pub fn new() -> Self {
        Self {
            frames: Vec::with_capacity(Self::FRAME_MAX),
            chunk: Chunk::new(),
            stack: Vec::with_capacity(Self::STACK_MAX),
            globals: HashMap::new(),
        }
    }

    pub fn interpret(&mut self, src: &str) -> Result<(), SmsError> {
        let compiler = Compiler::new("", FunctionType::Script);
        let mut parser = Parser::new(src, compiler);

        let function = parser.compile(); 
        if let Some(f) = function {
            self.push(Value::Function(Rc::from(f.clone())));
            self.frames.push(CallFrame::new(Rc::from(f.clone()), 0));
            self.run()
        } else {
            return Err(SmsError::CompileError);
        }
    }


    pub fn read_byte(&mut self) -> OpCode {
        let frame = self.current_frame_mut();
        let code = frame.function.chunk.code[frame.ip];
        frame.ip += 1; 
        code
    }

    pub fn run(&mut self) -> Result<(), SmsError> {

        loop {
            #[cfg(feature = "debug_trace_execution")]
            {
                print!("         ");
                for i in self.stack.iter() {
                    print!("[ {} ]", i);
                }
                println!();
                let disassembler = Disassembler::new(&self.current_frame().function.chunk);
                disassembler.dasm_instruction(self.current_frame().ip, &self.current_frame().function.chunk.code[self.current_frame().ip])
            }
            match self.read_byte() {
                OpCode::Constant(c) => {
                    let v = self.current_chunk().constants[c as usize].clone();
                    self.push(v);
                }
                OpCode::Print => println!("{}", self.pop()),
                OpCode::Return => {
                    return Ok(());
                }
                OpCode::Nil => self.push(Value::Nil),
                OpCode::True => self.push(Value::Bool(true)),
                OpCode::False => self.push(Value::Bool(false)),
                OpCode::Pop => {
                    self.pop();
                }
                OpCode::GetLocal(slot) => {
                    let idx = slot as usize + self.current_frame().slot;
                    let v = &self.stack[idx];
                    self.push(v.clone());
                }
                OpCode::SetLocal(slot) => {
                    self.stack[slot as usize] = self.peek(0);
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
                        self.push(v.clone())
                    } else {
                        self.runtime_error(&format!("Undefined variable '{}'", name))?;
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
                    self.push(Value::Bool(values_equal(a, b)));
                }
                OpCode::Greater => binary_op!(self, Bool, >),
                OpCode::Less => binary_op!(self, Bool, <),
                OpCode::Add => {
                    let (b, a) = (self.pop(), self.pop());
                    match (&a, &b) {
                        (Value::Number(n1), Value::Number(n2)) => {
                            self.push(Value::Number(n1 + n2));
                        }
                        (Value::String(s1), Value::String(s2)) => {
                            self.push(Value::String(format!("{}{}", s1, s2).into()));
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

    pub fn read_string(&self, idx: u8) -> Rc<str> {
        if let Value::String(s) = &self.current_frame().function.chunk.constants[idx as usize] {
            s.clone()
        } else {
            panic!("Constant is not String!")
        }
    }

    fn current_frame(&self) -> &CallFrame {
        self.frames.last().unwrap()
    }

    fn current_frame_mut(&mut self) -> &mut CallFrame {
        self.frames.last_mut().unwrap()
    }

    fn current_chunk(&self) -> &Chunk {
        &self.current_frame().function.chunk
    }

    pub fn runtime_error(&self, msg: &str) -> Result<(), SmsError> {
        eprintln!("{}", msg);
        let idx = self.current_frame().ip - 1;
        let line = self.current_chunk().lines[idx];
        eprintln!("[line {}] in script", line);
        Err(SmsError::RuntimeError)
    }
}

