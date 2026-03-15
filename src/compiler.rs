use crate::scanner::*;
use crate::parser::*;
use crate::chunk::*;

pub struct Compiler<'c> {
    compiling_chunk: &'c mut Chunk,
    pub locals: Vec<Local<'c>>,
    pub scope_depth: i32,
}

impl<'c> Compiler<'c> {
    pub const LOCAL_COUNT: usize = std::u8::MAX as usize + 1;
    pub fn new(chunk: &'c mut Chunk) -> Self {
        Self {
            compiling_chunk: chunk,
            locals: Vec::with_capacity(Self::LOCAL_COUNT), 
            scope_depth: 0,
        }
    }

    pub fn current_chunk(&mut self) -> &mut Chunk {
        &mut self.compiling_chunk
    }
}

pub struct Local<'src> {
    pub name: Token<'src>,
    pub depth: i32,
}

impl<'src> Local<'src> {
    pub fn new(name: Token<'src>, depth: i32) -> Self {
        Self {
            name, 
            depth,
        }
    }
}
