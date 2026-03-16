use crate::chunk::*;
use crate::object::*;
use crate::parser::*;
use crate::scanner::*;

pub struct Compiler<'c> {
    pub enclosing: Option<Box<Compiler<'c>>>,
    pub function: Function,
    pub fn_ty: FunctionType,
    pub locals: Vec<Local<'c>>,
    pub scope_depth: i32,
}

impl<'c> Compiler<'c> {
    pub const LOCAL_COUNT: usize = std::u8::MAX as usize + 1;
    pub fn new(func_name: &str, fn_ty: FunctionType) -> Self {
        let mut n = Self {
            enclosing: None,
            function: Function::new(func_name),
            fn_ty,
            locals: Vec::with_capacity(Self::LOCAL_COUNT),
            scope_depth: 0,
        };

        let token = match fn_ty {
            _ => Token::new(TokenType::Error, "", 0),
        };
        n.locals.push(Local::new(token, 0));
        n
    }

    pub fn current_chunk(&mut self) -> &mut Chunk {
        &mut self.function.chunk
    }
}

pub struct Local<'src> {
    pub name: Token<'src>,
    pub depth: i32,
}

impl<'src> Local<'src> {
    pub fn new(name: Token<'src>, depth: i32) -> Self {
        Self { name, depth }
    }
}

pub enum FunctionType {
    Function,
    Script,
}
