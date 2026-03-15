use std::collections::HashMap;
use std::rc::Rc;

use crate::chunk::*;
use crate::compiler::*;
use crate::debug::Disassembler;
use crate::error::*;
use crate::parser::*;
use crate::value::*;

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

pub struct Vm {
    pub chunk: Chunk,
    pub ip: usize,
    pub stack: Vec<Value>,
    pub globals: HashMap<Rc<str>, Value>,
}

impl Vm {
    const STACK_MAX: usize = 256;

    pub fn new() -> Self {
        Self {
            chunk: Chunk::new(),
            ip: 0,
            stack: Vec::with_capacity(Self::STACK_MAX),
            globals: HashMap::new(),
        }
    }

    pub fn interpret(&mut self, src: &str) -> Result<(), SmsError> {
        let mut chunk = Chunk::new();
        let compiler = Compiler::new(&mut chunk);
        let mut parser = Parser::new(src, compiler);

        if !parser.compile() {
            return Err(SmsError::CompileError);
        }
        self.chunk = chunk;
        self.ip = 0;
        self.run()
    }

    pub fn read_byte(&mut self) -> OpCode {
        self.ip += 1;
        self.chunk.code[self.ip - 1]
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
                let disassembler = Disassembler::new(&self.chunk);
                disassembler.dasm_instruction(self.ip, &self.chunk.code[self.ip])
            }
            match self.read_byte() {
                OpCode::Constant(c) => {
                    let v = self.chunk.constants[c as usize].clone();
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
                    binary_op!(self, Number, +)
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
        if let Value::String(s) = &self.chunk.constants[idx as usize] {
            s.clone()
        } else {
            panic!("Constant is not String!")
        }
    }

    pub fn runtime_error(&self, msg: &str) -> Result<(), SmsError> {
        eprintln!("{}", msg);
        let idx = self.ip - 1;
        let line = self.chunk.lines[idx];
        eprintln!("[line {}] in script", line);
        Err(SmsError::RuntimeError)
    }
}
