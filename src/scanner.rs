use std::rc::Rc;

pub struct Scanner<'src> {
    pub src: &'src str,
    pub start: usize,
    pub current: usize,
    pub line: usize,
}

impl<'src> Scanner<'src> {
    pub fn new(src: &'src str) -> Self {
        Self {
            src: src,
            start: 0,
            current: 0,
            line: 1,
        }
    }

    pub fn scan(&mut self) -> Token<'src> {
        self.skip_whitespace();
        self.start = self.current;
        if self.is_at_end() {
            return self.make_token(TokenType::Eof);
        }
        
        let c = self.advance();

        match c {
            b'(' => self.make_token(TokenType::LeftParen),
            b')' => self.make_token(TokenType::RightParen),
            b'{' => self.make_token(TokenType::LeftBrace),
            b'}' => self.make_token(TokenType::RightBrace),
            b';' => self.make_token(TokenType::Semicolon),
            b',' => self.make_token(TokenType::Comma),
            b'.' => self.make_token(TokenType::Dot),
            b'-' => self.make_token(TokenType::Minus),
            b'+' => self.make_token(TokenType::Plus),
            b'/' => self.make_token(TokenType::Slash),
            b'*' => self.make_token(TokenType::Star),
            b'!' if self.match_token(b'=') => self.make_token(TokenType::BangEqual),
            b'!' => self.make_token(TokenType::Bang),
            b'=' if self.match_token(b'=') => self.make_token(TokenType::EqualEqual),
            b'=' => self.make_token(TokenType::Equal),
            b'<' if self.match_token(b'=') => self.make_token(TokenType::LessEqual),
            b'<' => self.make_token(TokenType::Less),
            b'>' if self.match_token(b'=') => self.make_token(TokenType::GreaterEqual),
            b'>' => self.make_token(TokenType::Greater),
            b'"' => self.string(),
            c if is_alpha(c) => self.identifier(),
            c if is_digit(c) => self.number(),

            _ => self.error_token("Unexpected character."),
        }
    }

    fn match_token(&mut self, expected: u8) -> bool {
        if self.is_at_end() {
            return false;
        }
        if self.src.as_bytes()[self.current] != expected {
            return false;
        }
        self.current += 1;
        true
    }

    fn peek(&self) -> u8 {
        if self.is_at_end() {
            0
        } else {
            self.src.as_bytes()[self.current]
        }
    }

    fn peek_next(&self) -> u8 {
        if self.current + 1 >= self.src.len() {
            b'\0'
        } else {
            self.src.as_bytes()[self.current + 1]
        }
    }

    fn advance(&mut self) -> u8 {
        let char = self.peek();
        self.current += 1;
        char
    }

    fn is_at_end(&self) -> bool {
        self.current == self.src.len()
    }

    fn skip_whitespace(&mut self) {
        loop {
            match self.peek() {
                b' ' | b'\r' | b'\t' => {
                    self.advance();
                }
                b'\n' => {
                    self.line += 1;
                    self.advance();
                }
                b'/' if self.peek_next() == b'/' => {
                    while self.peek() != b'\n' && !self.is_at_end() {
                        self.advance();
                    }
                }
                _ => break,
            };
        }
    }

    fn string(&mut self) -> Token<'src> {
        while self.peek() != b'"' && !self.is_at_end() {
            if self.peek() == b'\n' {
                self.line += 1;
            }
            self.advance();
        }

        if self.is_at_end() {
            return self.error_token("Unterminated string.");
        }
        self.advance();
        self.make_token(TokenType::String)
    }

    fn number(&mut self) -> Token<'src> {
        while is_digit(self.peek()) {
            self.advance();
        }

        if self.peek() == b'.' && is_digit(self.peek_next()) {
            self.advance();
            while is_digit(self.peek()) {
                self.advance();
            }
        }

        self.make_token(TokenType::Number)
    }

    fn identifier(&mut self) -> Token<'src> {
        while is_alpha(self.peek()) || is_digit(self.peek()){
            self.advance();
        }

        self.make_token(self.ident_type())
    }

    fn ident_type(&self) -> TokenType {
        match self.lexeme() {
            "and" => TokenType::And,
            "class" => TokenType::Class,
            "else" => TokenType::Else,
            "false" => TokenType::False,
            "for" => TokenType::For,
            "fun" => TokenType::Fun,
            "if" => TokenType::If, 
            "nil" => TokenType::Nil,
            "or" => TokenType::Or,
            "print" => TokenType::Print,
            "return" => TokenType::Return,
            "super" => TokenType::Super,
            "this" => TokenType::This,
            "true" => TokenType::True,
            "var" => TokenType::Var,
            "while" => TokenType::While,
            _ => TokenType::Identifier,
        }
    }

    fn lexeme(&self) -> &'src str {
        &self.src[self.start..self.current]
    }

    fn make_token(&self, kind: TokenType) -> Token<'src> {
        Token {
            kind,
            lexeme: self.lexeme(),
            line: self.line,
        }
    }

    fn error_token(&self, msg: &'src str) -> Token<'src> {
        Token {
            kind: TokenType::Error,
            lexeme: msg,
            line: self.line,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum TokenType {
    // Single-character tokens.
    LeftParen,
    RightParen,
    LeftBrace,
    RightBrace,
    Comma,
    Dot,
    Minus,
    Plus,
    Semicolon,
    Slash,
    Star,

    // One or two character tokens.
    Bang,
    BangEqual,
    Equal,
    EqualEqual,
    Greater,
    GreaterEqual,
    Less,
    LessEqual,

    // Literals.
    Identifier,
    String,
    Number,

    // Keywords.
    And,
    Class,
    Else,
    False,
    For,
    Fun,
    If,
    Nil,
    Or,
    Print,
    Return,
    Super,
    This,
    True,
    Var,
    While,

    Error,
    Eof,
}

#[derive(Debug, Copy, Clone)]
pub struct Token<'src> {
    pub kind: TokenType,
    pub lexeme: &'src str,
    pub line: usize,
}

impl<'src> Token<'src> {
    pub fn new(kind: TokenType, lexeme: &'src str, line: usize) -> Self {
        Self {
            kind, 
            lexeme,
            line,
        }
    }
}



fn is_digit(ch: u8) -> bool {
    ch >= b'0' && ch <= b'9'
}

fn is_alpha(ch: u8) -> bool {
    ch >= b'a' && ch <= b'z' || ch >= b'A' && ch <= b'Z' || ch == b'_'
}
