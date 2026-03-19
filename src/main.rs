use std::env;
use std::io::{self, Write};
use std::process;
use std::fs;

mod chunk;
mod debug;
mod value;
mod compiler;
mod scanner;
mod parser;
mod object;
mod gc;

mod vm;
use vm::*;

mod error;
use error::*;

fn main() {
    let mut vm = Vm::new();
    
    let args: Vec<String> = env::args().collect();
    if args.len() == 1 {
        repl(&mut vm);
    } else if args.len() == 2 {
        run_file(&mut vm, &args[1])
    } else {
        println!("Usage: sms [path]");
        return;
    }

}

fn repl(vm: &mut Vm) {
    loop {
        print!("> ");
        io::stdout().flush().unwrap();
        let mut line = String::new();
        io::stdin().read_line(&mut line).expect("Failed to read line");
        vm.interpret(&line).ok();
        line.clear();
    }
}

fn run_file(vm: &mut Vm, path: &str) {
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(error) => {
            eprint!("Failed to read file {}: {}", path, error);
            process::exit(74);
        }
    };
    match vm.interpret(&content) {
        Err(e) => match e {
            SmsError::CompileError => process::exit(65),
            SmsError::RuntimeError => process::exit(70),
        }
        _ => ()
    }
}


