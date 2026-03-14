use crate::scanner::*;
use crate::parser::*;
use crate::chunk::*;

pub struct Compiler<'c> {
    compiling_chunk: &'c mut Chunk
}

impl<'c> Compiler<'c> {
    pub fn new(chunk: &'c mut Chunk) -> Self {
        Self {
            compiling_chunk: chunk,
        }
    }

    pub fn current_chunk(&mut self) -> &mut Chunk {
        &mut self.compiling_chunk
    }
}

pub fn compile(src: &str, chunk: &mut Chunk) -> bool {
    let compiler = Compiler::new(chunk);
    let mut parser = Parser::new(src, compiler);
    parser.compile()
}
