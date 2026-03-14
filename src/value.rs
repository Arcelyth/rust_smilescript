use std::fmt::{self, Display};

#[derive(Debug, Copy, Clone)]
pub enum Value {
    Nil, 
    Bool(bool),
    Number(f64)    
}

impl Display for Value {
   fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Value::Nil => write!(f, "nil"),
            Value::Bool(b) => write!(f, "{}", b),
            Value::Number(v) => write!(f, "{}", v)
        }
    } 
}

