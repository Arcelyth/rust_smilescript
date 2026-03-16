use std::rc::Rc;

use crate::value::*;
use crate::chunk::*;
use crate::vm::*;

#[derive(Debug, Clone)]
pub struct Function {
    pub arity: usize,
    pub chunk: Chunk,
    pub name: Rc<str>,
}

impl Function {
    pub fn new(name: &str) -> Self {
        Self {
            arity: 0,
            name: name.into(),
            chunk: Chunk::new(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct NativeFunction(pub fn(&[Value]) -> Value);


