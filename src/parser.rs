use crate::chunk::*;
use crate::debug::Disassembler;
use crate::compiler::*;
use crate::scanner::*;
use crate::value::*;

#[derive(Copy, Clone, PartialOrd, PartialEq)]
enum Precedence {
    None,
    Assignment,
    Or,
    And,
    Equality,
    Comparison,
    Term,
    Factor,
    Unary,
    Call,
    Primary,
}

impl Precedence {
    fn next(&self) -> Precedence {
        match self {
            Precedence::None => Precedence::Assignment,
            Precedence::Assignment => Precedence::Or,
            Precedence::Or => Precedence::And,
            Precedence::And => Precedence::Equality,
            Precedence::Equality => Precedence::Comparison,
            Precedence::Comparison => Precedence::Term,
            Precedence::Term => Precedence::Factor,
            Precedence::Factor => Precedence::Unary,
            Precedence::Unary => Precedence::Call,
            Precedence::Call => Precedence::Primary,
            Precedence::Primary => Precedence::None,
        }
    }
}

type ParseFn<'c> = fn(&mut Parser<'c>);

struct ParseRule<'c> {
    prefix: Option<ParseFn<'c>>,
    infix: Option<ParseFn<'c>>,
    prec: Precedence,
}

impl<'c> ParseRule<'c> {
    pub fn new(prefix: Option<ParseFn<'c>>, infix: Option<ParseFn<'c>>, prec: Precedence) -> Self {
        Self {
            prefix,
            infix,
            prec,
        }
    }
}

pub struct Parser<'c> {
    scanner: Scanner,
    compiler: Compiler<'c>,
    previous: Token,
    current: Token,
    had_error: bool,
    panic_mode: bool,
}

impl<'c> Parser<'c> {
    pub fn new(src: &str, compiler: Compiler<'c>) -> Self {
        Self {
            scanner: Scanner::new(src),
            compiler,
            previous: Token::new(TokenType::Error, "", 0),
            current: Token::new(TokenType::Error, "", 0),
            had_error: false,
            panic_mode: false,
        }
    }

    // return false if an error occurred
    pub fn compile(&mut self) -> bool {
        self.advance();
        self.expression();
        self.consume(TokenType::Eof, "Expect end of expression.");
        self.end_compiler();
        !self.had_error
    }

    fn advance(&mut self) {
        self.previous = self.current.clone();
        loop {
            self.current = self.scanner.scan();
            if self.current.kind != TokenType::Error {
                break;
            }
            self.error_at_current(&self.current.clone().lexeme);
        }
    }

    fn error_at_current(&mut self, msg: &str) {
        self.error_at(self.current.clone(), msg);
    }

    fn error(&mut self, msg: &str) {
        self.error_at(self.previous.clone(), msg);
    }

    fn error_at(&mut self, token: Token, msg: &str) {
        if self.panic_mode {
            return;
        }
        self.panic_mode = true;
        eprint!("[line {}] Error", token.line);
        match token.kind {
            TokenType::Eof => {
                eprint!(" at end");
            }
            TokenType::Error => {}
            _ => {
                eprint!(" at '{}'", token.lexeme);
            }
        }
        eprintln!(": {}", msg);
        self.had_error = true;
    }

    fn consume(&mut self, kind: TokenType, msg: &str) {
        if self.current.kind == kind {
            self.advance();
        } else {
            self.error_at_current(msg);
        }
    }

    fn current_chunk(&mut self) -> &mut Chunk {
        self.compiler.current_chunk()
    }

    fn end_compiler(&mut self) {
        self.emit_return();

        #[cfg(feature = "debug_print_code")]
        {
            if !self.had_error {
                let disassembler = Disassembler::new(self.current_chunk());
                disassembler.dasm_chunk("code");
            }
        }
    }

    fn emit_code(&mut self, code: OpCode) {
        let line = self.previous.line;
        self.current_chunk().write(code, line);
    }

    fn emit_return(&mut self) {
        self.emit_code(OpCode::Return);
    }

    fn emit_constant(&mut self, v: Value) {
        let idx = self.make_constant(v);
        self.emit_code(OpCode::Constant(idx));
    }

    fn make_constant(&mut self, v: Value) -> u8 {
        let c = self.current_chunk().add_constant(v);
        match u8::try_from(c) {
            Ok(index) => index,
            Err(_) => {
                self.error("Too many constants in one chunk.");
                0
            }
        }
    }

    fn number(&mut self) {
        let v = self.previous.lexeme.parse::<f64>().unwrap();
        self.emit_constant(Value::Number(v));
    }

    fn grouping(&mut self) {
        self.expression();
        self.consume(TokenType::RightParen, "Expect ')' after expression.");
    }

    fn unary(&mut self) {
        self.parse_precedence(Precedence::Unary);

        match self.previous.kind {
            TokenType::Minus => self.emit_code(OpCode::Negate),
            _ => (),
        }
    }

    fn binary(&mut self) {
        let kind = self.previous.kind;
        let rule = Parser::get_rule(kind);
        self.parse_precedence(rule.prec.next());

        match kind {
            TokenType::Plus => self.emit_code(OpCode::Add),
            TokenType::Minus => self.emit_code(OpCode::Subtract),
            TokenType::Star => self.emit_code(OpCode::Multiply),
            TokenType::Slash => self.emit_code(OpCode::Divide),
            _ => (),
        }
    }

    fn variable(&mut self) {}

    fn string(&mut self) {}

    fn literal(&mut self) {}

    fn and(&mut self) {}

    fn or(&mut self) {}

    fn parse_precedence(&mut self, prec: Precedence) {
        self.advance();
        let pre_rule = Parser::get_rule(self.previous.kind).prefix;
        if let Some(f) = pre_rule {
            f(self);
        } else {
            self.error("Expect expression.");
        }

        while prec <= Parser::get_rule(self.current.kind).prec {
            self.advance();
            let inf_rule = Parser::get_rule(self.previous.kind).infix;
            if let Some(f) = inf_rule {
                f(self);
            }
        }
    }

    fn expression(&mut self) {
        self.parse_precedence(Precedence::Assignment);
    }

    fn get_rule(kind: TokenType) -> ParseRule<'c> {
        match kind {
            TokenType::LeftParen => ParseRule::new(Some(Parser::grouping), None, Precedence::None),
            TokenType::RightParen => ParseRule::new(None, None, Precedence::None),
            TokenType::LeftBrace => ParseRule::new(None, None, Precedence::None),
            TokenType::RightBrace => ParseRule::new(None, None, Precedence::None),
            TokenType::Comma => ParseRule::new(None, None, Precedence::None),
            TokenType::Dot => ParseRule::new(None, None, Precedence::None),
            TokenType::Minus => {
                ParseRule::new(Some(Parser::unary), Some(Parser::binary), Precedence::Term)
            }
            TokenType::Plus => ParseRule::new(None, Some(Parser::binary), Precedence::Term),
            TokenType::Semicolon => ParseRule::new(None, None, Precedence::None),
            TokenType::Slash => ParseRule::new(None, Some(Parser::binary), Precedence::Factor),
            TokenType::Star => ParseRule::new(None, Some(Parser::binary), Precedence::Factor),
            TokenType::Bang => ParseRule::new(Some(Parser::unary), None, Precedence::None),
            TokenType::BangEqual => {
                ParseRule::new(None, Some(Parser::binary), Precedence::Equality)
            }
            TokenType::Equal => ParseRule::new(None, None, Precedence::None),
            TokenType::EqualEqual => {
                ParseRule::new(None, Some(Parser::binary), Precedence::Equality)
            }
            TokenType::Greater => {
                ParseRule::new(None, Some(Parser::binary), Precedence::Comparison)
            }
            TokenType::GreaterEqual => {
                ParseRule::new(None, Some(Parser::binary), Precedence::Comparison)
            }
            TokenType::Less => ParseRule::new(None, Some(Parser::binary), Precedence::Comparison),
            TokenType::LessEqual => {
                ParseRule::new(None, Some(Parser::binary), Precedence::Comparison)
            }
            TokenType::Identifier => ParseRule::new(Some(Parser::variable), None, Precedence::None),
            TokenType::String => ParseRule::new(Some(Parser::string), None, Precedence::None),
            TokenType::Number => ParseRule::new(Some(Parser::number), None, Precedence::None),
            TokenType::And => ParseRule::new(None, Some(Parser::and), Precedence::And),
            TokenType::Class => ParseRule::new(None, None, Precedence::None),
            TokenType::Else => ParseRule::new(None, None, Precedence::None),
            TokenType::False => ParseRule::new(Some(Parser::literal), None, Precedence::None),
            TokenType::For => ParseRule::new(None, None, Precedence::None),
            TokenType::Fun => ParseRule::new(None, None, Precedence::None),
            TokenType::If => ParseRule::new(None, None, Precedence::None),
            TokenType::Nil => ParseRule::new(Some(Parser::literal), None, Precedence::None),
            TokenType::Or => ParseRule::new(None, Some(Parser::or), Precedence::Or),
            TokenType::Print => ParseRule::new(None, None, Precedence::None),
            TokenType::Return => ParseRule::new(None, None, Precedence::None),
            TokenType::Super => ParseRule::new(None, None, Precedence::None),
            TokenType::This => ParseRule::new(None, None, Precedence::None),
            TokenType::True => ParseRule::new(Some(Parser::literal), None, Precedence::None),
            TokenType::Var => ParseRule::new(None, None, Precedence::None),
            TokenType::While => ParseRule::new(None, None, Precedence::None),
            TokenType::Error => ParseRule::new(None, None, Precedence::None),
            TokenType::Eof => ParseRule::new(None, None, Precedence::None),
            _ => ParseRule::new(None, None, Precedence::None),
        }
    }
}
