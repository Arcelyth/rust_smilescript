use std::rc::Rc;
use crate::chunk::*;

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
