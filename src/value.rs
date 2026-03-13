use std::fmt::{self, Display};

#[derive(Debug, Copy, Clone)]
pub enum Value {
    Number(f64)    
}

impl Display for Value {
   fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Value::Number(v) => write!(f, "{}", v)
        }
    } 
}

