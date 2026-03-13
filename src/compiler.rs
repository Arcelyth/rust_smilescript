use crate::scanner::*;

pub fn compile(src: &str) {
    let mut scanner = Scanner::new(src);
    let mut line = 0;
    loop {
        let token = scanner.scan();
        if token.line != line {
            print!("{:4} ", token.line);
            line = token.line;
        } else {
            print!("   | ");
        }
        println!("{:2?} '{}'", token.kind, token.lexeme);

        if let TokenType::Eof = token.kind {
            break;
        };
    }
}
