use std::fmt::{self, Display};
use std::rc::Rc;

#[derive(Debug, Clone)]
pub enum Value {
    Nil, 
    Bool(bool),
    Number(f64),
    String(Rc<str>),
}

impl Display for Value {
   fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Value::Nil => write!(f, "nil"),
            Value::Bool(b) => write!(f, "{}", b),
            Value::Number(v) => write!(f, "{}", v),
            Value::String(s) => write!(f, "{}", s),
        }
    } 
}

pub fn values_equal(a: Value, b: Value) -> bool {
    match (a, b) {
        (Value::Bool(b1), Value::Bool(b2)) => b1 == b2,
        (Value::Nil, Value::Nil) => true,
        (Value::Number(n1), Value::Number(n2)) => n1 == n2,
        (Value::String(s1), Value::String(s2)) => s1 == s2,
        _ => false,
    }
}


