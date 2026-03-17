use std::rc::Rc;

use crate::value::*;
use crate::chunk::*;
use crate::vm::*;

#[derive(Debug, Clone)]
pub struct Function {
    pub arity: usize,
    pub chunk: Chunk,
    pub name: Rc<str>,
    pub upvalues: Vec<FnUpValue>,
}

impl Function {
    pub fn new(name: &str) -> Self {
        Self {
            arity: 0,
            name: name.into(),
            chunk: Chunk::new(),
            upvalues: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct NativeFunction(pub fn(&[Value]) -> Value);

#[derive(Debug, Clone)]
pub struct Closure {
    pub function: Rc<Function>,
}

impl Closure {
    pub fn new(function: Rc<Function>) -> Self {
        Self {
            function,
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub struct FnUpValue {
    pub index: u8, 
    pub is_local: bool, 
}

impl FnUpValue {
    pub fn new(index: u8, is_local: bool) -> Self {
        Self {
            index,
            is_local
        }
    }
}

