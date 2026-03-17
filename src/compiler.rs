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

    pub fn resolve_local(&mut self, name: Token) -> Result<Option<u8>, String> {
        for i in (0..self.locals.len()).rev() {
            let local = &self.locals[i];
            if local.name.lexeme == name.lexeme {
                if local.depth == -1 {
                    return Err("Can't read local variable in its own initializer.".to_string());
                }
                return Ok(Some(i as u8));
            }
        }
        Ok(None)
    }

    pub fn resolve_upvalue(&mut self, name: Token) -> Result<Option<u8>, String> {
        if let Some(enclosing) = self.enclosing.as_mut() {
            if let Some(index) = enclosing.resolve_local(name)? {
                return Ok(Some(self.add_upvalue(index, true)?));
            }
            if let Some(index) = enclosing.resolve_upvalue(name)? {
                return Ok(Some(self.add_upvalue(index, true)?));
            }
        }
        Ok(None)
    }

    fn add_upvalue(&mut self, idx: u8, is_local: bool) -> Result<u8, &str> {
        for (i, upv) in self.function.upvalues.iter().enumerate() {
            if upv.index == idx && upv.is_local == is_local {
                return Ok(i as u8);
            }
        }

        match u8::try_from(self.function.upvalues.len()) {
            Ok(_index) => (),
            Err(_) => {
                return Err("Too many constants in one chunk.");
            }
        }

        self.function.upvalues.push(FnUpValue::new(idx, is_local));
        Ok(self.function.upvalues.len() as u8)
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
