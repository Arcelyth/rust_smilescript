#![allow(dead_code)]
use std::fmt;

use crate::value::*;

#[derive(Debug, Copy, Clone)]
pub enum OpCode {
    OpReturn,
    OpConstant(u8),
}

pub struct Chunk {
    pub code: Vec<OpCode>,
    pub lines: Vec<usize>, 
    pub constants: Vec<Value>,
}

impl Chunk {
    pub fn new() -> Self {
        Self {
            code: Vec::new(),
            lines: Vec::new(),
            constants: Vec::new()
        }
    }
    
    pub fn write(&mut self, op_code: OpCode, line: usize) {
        self.code.push(op_code);
        self.lines.push(line);
    }

    pub fn add_constant(&mut self, value: Value) -> usize {
        self.constants.push(value);
        self.constants.len() - 1
    }
}
