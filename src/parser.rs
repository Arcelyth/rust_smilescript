use crate::chunk::*;
use crate::compiler::*;
use crate::debug::Disassembler;
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
    scanner: Scanner<'c>,
    compiler: Compiler<'c>,
    previous: Token<'c>,
    current: Token<'c>,
    had_error: bool,
    panic_mode: bool,
}

impl<'c> Parser<'c> {
    pub fn new(src: &'c str, compiler: Compiler<'c>) -> Self {
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
        while !self.match_token(TokenType::Eof) {
            self.declaration();
        }

        self.end_compiler();
        !self.had_error
    }

    fn match_token(&mut self, kind: TokenType) -> bool {
        if !(self.current.kind == kind) {
            return false;
        }
        self.advance();
        true
    }

    fn advance(&mut self) {
        self.previous = self.current;
        loop {
            self.current = self.scanner.scan();
            if self.current.kind != TokenType::Error {
                break;
            }
            self.error_at_current(self.current.lexeme);
        }
    }

    fn error_at_current(&mut self, msg: &str) {
        self.error_at(self.current, msg);
    }

    fn error(&mut self, msg: &str) {
        self.error_at(self.previous, msg);
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

    pub fn current_chunk(&mut self) -> &mut Chunk {
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
        let kind = self.previous.kind;
        self.parse_precedence(Precedence::Unary);

        match kind {
            TokenType::Minus => self.emit_code(OpCode::Negate),
            TokenType::Bang => self.emit_code(OpCode::Not),
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
            TokenType::BangEqual => {
                self.emit_code(OpCode::Equal);
                self.emit_code(OpCode::Not);
            }
            TokenType::Equal => self.emit_code(OpCode::Equal),
            TokenType::Greater => self.emit_code(OpCode::Greater),
            TokenType::GreaterEqual => {
                self.emit_code(OpCode::Less);
                self.emit_code(OpCode::Not);
            }
            TokenType::Less => self.emit_code(OpCode::Less),
            TokenType::LessEqual => {
                self.emit_code(OpCode::Greater);
                self.emit_code(OpCode::Not);
            }
            _ => (),
        }
    }

    fn variable(&mut self) {
        self.named_variable(self.previous);
    }

    fn named_variable(&mut self, name: Token) {
        let idx = self.identifier_constant(name);
        self.emit_code(OpCode::GetGlobal(idx));
    }

    fn string(&mut self) {
        let lexeme = self.previous.lexeme;
        self.emit_constant(Value::String(lexeme[1..lexeme.len() - 1].into()));
    }

    fn literal(&mut self) {
        match self.previous.kind {
            TokenType::False => self.emit_code(OpCode::False),
            TokenType::Nil => self.emit_code(OpCode::Nil),
            TokenType::True => self.emit_code(OpCode::True),
            _ => (),
        }
    }

    fn and(&mut self) {}

    fn or(&mut self) {}

    fn expression(&mut self) {
        self.parse_precedence(Precedence::Assignment);
    }

    fn statement(&mut self) {
        if self.match_token(TokenType::Print) {
            self.print_statement();
        } else {
            self.expression_statement();
        }
    }

    fn print_statement(&mut self) {
        self.expression();
        self.consume(TokenType::Semicolon, "Expect ';' after value.");
        self.emit_code(OpCode::Print);
    }

    fn expression_statement(&mut self) {
        self.expression();
        self.consume(TokenType::Semicolon, "Expect ';' after value.");
        self.emit_code(OpCode::Pop);
    }

    fn declaration(&mut self) {
        if self.match_token(TokenType::Var) {
            self.var_declaration();
        } else {
            self.statement();
        }
        if self.panic_mode {
            self.synchronize();
        }
    }

    fn var_declaration(&mut self) {
        let global = self.parse_variable("Expect variable name.");
        if self.match_token(TokenType::Equal) {
            self.expression();
        } else {
            self.emit_code(OpCode::Nil);
        }
        self.consume(TokenType::Semicolon, "Expect ';' after variable declaration.");
        self.define_variable(global);
    }

    fn parse_variable(&mut self, msg: &str) -> u8 {
        self.consume(TokenType::Identifier, msg);
        self.identifier_constant(self.previous)
    }

    fn identifier_constant(&mut self, name: Token) -> u8 {
        self.make_constant(Value::String(name.lexeme.into()))
    }

    fn define_variable(&mut self, idx: u8) {
        self.emit_code(OpCode::DefineGlobal(idx));
    }

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

    fn synchronize(&mut self) {
        self.panic_mode = false;
        while self.current.kind != TokenType::Eof {
            if self.previous.kind == TokenType::Semicolon {
                return;
            }
            match self.current.kind {
                TokenType::Class
                | TokenType::Fun
                | TokenType::Var
                | TokenType::For
                | TokenType::If
                | TokenType::While
                | TokenType::Print
                | TokenType::Return => {
                    return;
                }
                _ => (),
            }
            self.advance();
        }
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
