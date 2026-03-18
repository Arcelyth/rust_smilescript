use std::rc::Rc;
use std::cell::RefCell;

use crate::chunk::*;
use crate::value::*;

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
    pub upvalues: Vec<Rc<RefCell<UpValue>>>,
}

impl Closure {
    pub fn new(function: Rc<Function>) -> Self {
        Self { 
            function,
            upvalues: Vec::new(),
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
        Self { index, is_local }
    }
}
#[derive(Debug, Clone)]
pub struct UpValue {
    pub location: usize,
    pub closed: Option<Value>,
}

impl UpValue {
    pub fn new(location: usize) -> Self {
        Self {
            location,
            closed: None
        }
    }
}
