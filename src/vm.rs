use crate::chunk::*;
use crate::debug::Disassembler;
use crate::error::*;
use crate::value::*;
use crate::compiler::*;

pub struct Vm {
    pub chunk: Chunk,
    pub ip: usize,
    pub stack: Vec<Value>,
}

impl Vm {
    const STACK_MAX: usize = 256;

    pub fn new() -> Self {
        Self {
            chunk: Chunk::new(),
            ip: 0,
            stack: Vec::with_capacity(Self::STACK_MAX),
        }
    }

    pub fn interpret(&mut self, src: &str) -> Result<(), SmsError> {
        if !compile(src, &mut self.chunk) {
            return Err(SmsError::CompileError);
        } 
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
                    let v = self.chunk.constants[c as usize];
                    self.push(v);
                }
                OpCode::Return => {
                    println!("{}", self.pop());
                    return Ok(());
                }
                OpCode::Add => self.binary_op(|a, b| a + b)?,
                OpCode::Subtract => self.binary_op(|a, b| a - b)?,
                OpCode::Multiply => self.binary_op(|a, b| a * b)?,
                OpCode::Divide => self.binary_op(|a, b| a / b)?,
                OpCode::Negate => {
                    let n = match self.pop() {
                        Value::Number(n) => n,
                        _ => return self.runtime_error("Operand must be a number"),
                    };
                    self.push(Value::Number(-n));
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
    
    pub fn binary_op(&mut self, f: fn (f64, f64) -> f64) -> Result<(), SmsError>{
        match (self.pop(), self.pop()) {
            (Value::Number(lv), Value::Number(rv)) => {
                let result = f(lv, rv);
                Ok(self.push(Value::Number(result)))
            }
            _ => self.runtime_error("Operands must be numbers.")
        }
    }

    pub fn runtime_error(&self, msg: &str) -> Result<(), SmsError> {
        eprintln!("{}", msg);
        Err(SmsError::RuntimeError)
    }
}
