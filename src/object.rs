use crate::chunk::Chunk;
use crate::value::Value;
use crate::gc::GcRef;

#[derive(Debug, Clone)]
pub enum Obj {
    String(String),
    Function(Function),
    Closure(Closure),
    UpValue(UpValue), 
    Native(NativeFunction),
}

#[derive(Debug, Clone)]
pub struct GcObject {
    pub is_marked: bool,
    pub obj: Obj,
}

impl GcObject {
    pub fn new(obj: Obj) -> Self {
        Self {
            is_marked: false,
            obj,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Function {
    pub arity: usize,
    pub chunk: Chunk,
    pub name: GcRef,
    pub upvalues: Vec<FnUpValue>,
}

impl Function {
    pub fn new(name: GcRef) -> Self {
        Self {
            arity: 0,
            name,
            chunk: Chunk::new(),
            upvalues: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct NativeFunction(pub fn(&[Value]) -> Value);

#[derive(Debug, Clone)]
pub struct Closure {
    pub function: GcRef, 
    pub upvalues: Vec<GcRef>, 
}

impl Closure {
    pub fn new(function: GcRef) -> Self {
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
