use std::fs;
use std::process;

use smsc::vm::Vm;

pub fn should_ok(path: &str) {
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(error) => {
            eprint!("Failed to read file {}: {}", path, error);
            process::exit(74);
        }
    };
    let mut vm = Vm::new();
    let res = Vm::interpret(&mut vm, &content);
    assert!(res.is_ok());
}
