use std::mem;
use std::rc::Rc;

use crate::chunk::*;
use crate::compiler::*;
use crate::debug::Disassembler;
use crate::object::*;
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

type ParseFn<'c> = fn(&mut Parser<'c>, bool);

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

    pub fn compile(&mut self) -> Option<Function> {
        self.advance();
        while !self.match_token(TokenType::Eof) {
            self.declaration();
        }

        if self.had_error {
            None
        } else {
            Some(self.end_compiler()?)
        }
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

    fn check(&self, kind: TokenType) -> bool {
        self.current.kind == kind
    }

    pub fn current_chunk(&mut self) -> &mut Chunk {
        self.compiler.current_chunk()
    }

    fn end_compiler(&mut self) -> Option<Function> {
        self.emit_return();
        let f = self.pop_compiler();

        self.pop_compiler();
        #[cfg(feature = "debug_print_code")]
        {
            if !self.had_error {
                let name = if self.compiler.function.name != "".into() {
                    &self.compiler.function.name.clone()
                } else {
                    "<script>"
                };
                let disassembler = Disassembler::new(self.current_chunk());
                disassembler.dasm_chunk(name);
            }
        }
        if self.had_error { None } else { Some(f) }
    }

    fn emit_code(&mut self, code: OpCode) -> usize {
        let line = self.previous.line;
        self.current_chunk().write(code, line)
    }

    fn emit_return(&mut self) {
        self.emit_code(OpCode::Nil);
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

    fn number(&mut self, _can_assign: bool) {
        let v = self.previous.lexeme.parse::<f64>().unwrap();
        self.emit_constant(Value::Number(v));
    }

    fn grouping(&mut self, _can_assign: bool) {
        self.expression();
        self.consume(TokenType::RightParen, "Expect ')' after expression.");
    }

    fn unary(&mut self, _can_assign: bool) {
        let kind = self.previous.kind;
        self.parse_precedence(Precedence::Unary);

        match kind {
            TokenType::Minus => {
                self.emit_code(OpCode::Negate);
            }
            TokenType::Bang => {
                self.emit_code(OpCode::Not);
            }
            _ => (),
        }
    }

    fn binary(&mut self, _can_assign: bool) {
        let kind = self.previous.kind;
        let rule = Parser::get_rule(kind);
        self.parse_precedence(rule.prec.next());

        match kind {
            TokenType::Plus => {
                self.emit_code(OpCode::Add);
            }
            TokenType::Minus => {
                self.emit_code(OpCode::Subtract);
            }
            TokenType::Star => {
                self.emit_code(OpCode::Multiply);
            }
            TokenType::Slash => {
                self.emit_code(OpCode::Divide);
            }
            TokenType::BangEqual => {
                self.emit_code(OpCode::Equal);
                self.emit_code(OpCode::Not);
            }
            TokenType::Equal => {
                self.emit_code(OpCode::Equal);
            }
            TokenType::Greater => {
                self.emit_code(OpCode::Greater);
            }
            TokenType::GreaterEqual => {
                self.emit_code(OpCode::Less);
                self.emit_code(OpCode::Not);
            }
            TokenType::Less => {
                self.emit_code(OpCode::Less);
            }
            TokenType::LessEqual => {
                self.emit_code(OpCode::Greater);
                self.emit_code(OpCode::Not);
            }
            _ => (),
        }
    }

    fn call(&mut self, _can_assign: bool) {
        let arg_count = self.argument_list();
        self.emit_code(OpCode::Call(arg_count));
    }

    fn variable(&mut self, can_assign: bool) {
        self.named_variable(self.previous, can_assign);
    }

    fn named_variable(&mut self, name: Token, can_assign: bool) {
        let get_op;
        let set_op;

        if let Some(idx) = self.resolve_local(name) {
            get_op = OpCode::GetLocal(idx);
            set_op = OpCode::SetLocal(idx);
        } else if let Some(idx) = self.resolve_upvalue(name) {
            get_op = OpCode::GetUpValue(idx);
            set_op = OpCode::SetUpValue(idx);
        } else {
            let index = self.identifier_constant(name);
            get_op = OpCode::GetGlobal(index);
            set_op = OpCode::SetGlobal(index);
        }

        if can_assign && self.match_token(TokenType::Equal) {
            self.expression();
            self.emit_code(set_op);
        } else {
            self.emit_code(get_op);
        }
    }

    fn resolve_local(&mut self, name: Token) -> Option<u8> {
        match self.compiler.resolve_local(name) {
            Ok(r) => r,
            Err(s) => {
                self.error(&s);
                None
            }
        }
    }

    fn resolve_upvalue(&mut self, name: Token) -> Option<u8> {
        match self.compiler.resolve_upvalue(name) {
            Ok(r) => r,
            Err(s) => {
                self.error(&s);
                None
            }
        }
    }

    fn string(&mut self, _can_assign: bool) {
        let lexeme = self.previous.lexeme;
        self.emit_constant(Value::String(lexeme[1..lexeme.len() - 1].into()));
    }

    fn literal(&mut self, _can_assign: bool) {
        match self.previous.kind {
            TokenType::False => {
                self.emit_code(OpCode::False);
            }
            TokenType::Nil => {
                self.emit_code(OpCode::Nil);
            }
            TokenType::True => {
                self.emit_code(OpCode::True);
            }
            _ => (),
        }
    }

    fn and(&mut self, _can_assign: bool) {
        let end_jump = self.emit_code(OpCode::JumpIfFalse(0xffff));
        self.emit_code(OpCode::Pop);
        self.parse_precedence(Precedence::And);
        self.patch_jump(end_jump);
    }

    fn or(&mut self, _can_assign: bool) {
        let else_jump = self.emit_code(OpCode::JumpIfFalse(0xffff));
        let end_jump = self.emit_code(OpCode::Jump(0xffff));
        self.patch_jump(else_jump);
        self.emit_code(OpCode::Pop);
        self.parse_precedence(Precedence::And);
        self.patch_jump(end_jump);
    }

    fn expression(&mut self) {
        self.parse_precedence(Precedence::Assignment);
    }

    fn block(&mut self) {
        while !self.check(TokenType::RightBrace) && !self.check(TokenType::Eof) {
            self.declaration();
        }
        self.consume(TokenType::RightBrace, "Expect '}' after block.");
    }

    fn argument_list(&mut self) -> u8 {
        let mut arg_count = 0;
        if !self.check(TokenType::RightParen) {
            loop {
                self.expression();
                if arg_count == 255 {
                    self.error("Can't have more then 255 arguments");
                }
                arg_count += 1;
                if !self.match_token(TokenType::Comma) {
                    break;
                }
            }
        }
        self.consume(TokenType::RightParen, "Expect ')' after arguments.");
        arg_count
    }

    fn begin_scope(&mut self) {
        self.compiler.scope_depth += 1;
    }

    fn end_scope(&mut self) {
        self.compiler.scope_depth -= 1;

        loop {
            if let Some(l) = self.compiler.locals.last() {
                if l.depth > self.compiler.scope_depth {
                    self.emit_code(OpCode::Pop);
                    self.compiler.locals.pop();
                } else {
                    break;
                }
            } else {
                break;
            }
        }
    }

    fn statement(&mut self) {
        if self.match_token(TokenType::Print) {
            self.print_statement();
        } else if self.match_token(TokenType::If) {
            self.if_statement();
        } else if self.match_token(TokenType::While) {
            self.while_statement();
        } else if self.match_token(TokenType::For) {
            self.for_statement();
        } else if self.match_token(TokenType::Return) {
            self.return_statement();
        } else if self.match_token(TokenType::LeftBrace) {
            self.begin_scope();
            self.block();
            self.end_scope();
        } else {
            self.expression_statement();
        }
    }

    fn print_statement(&mut self) {
        self.expression();
        self.consume(TokenType::Semicolon, "Expect ';' after value.");
        self.emit_code(OpCode::Print);
    }

    fn if_statement(&mut self) {
        self.consume(TokenType::LeftParen, "Expect '(' fater if.");
        self.expression();
        self.consume(TokenType::RightParen, "Expect ')' after condition.");
        let then_jump = self.emit_code(OpCode::JumpIfFalse(0xffff));
        self.emit_code(OpCode::Pop);
        self.statement();
        let else_jump = self.emit_code(OpCode::Jump(0xffff));
        self.patch_jump(then_jump);
        self.emit_code(OpCode::Pop);

        if self.match_token(TokenType::Else) {
            self.statement();
        }
        self.patch_jump(else_jump);
    }

    fn while_statement(&mut self) {
        let loop_start = self.current_chunk().code.len();
        self.consume(TokenType::LeftParen, "Expect '(' after 'while'.");
        self.expression();
        self.consume(TokenType::RightParen, "Expect ')' after condition.");
        let exit_jump = self.emit_code(OpCode::JumpIfFalse(0xffff));
        self.emit_code(OpCode::Pop);
        self.statement();
        self.emit_loop(loop_start);
        self.patch_jump(exit_jump);
        self.emit_code(OpCode::Pop);
    }

    fn for_statement(&mut self) {
        self.begin_scope();
        self.consume(TokenType::LeftParen, "Expect '(' after 'for'.");
        if self.match_token(TokenType::Semicolon) {
        } else if self.match_token(TokenType::Var) {
            self.var_declaration();
        } else {
            self.expression_statement();
        }
        let mut loop_start = self.current_chunk().code.len();
        let mut exit_jump: i32 = -1;
        if !self.match_token(TokenType::Semicolon) {
            self.expression();
            self.consume(TokenType::Semicolon, "Expect ';' after loop condition.");
            exit_jump = self.emit_code(OpCode::JumpIfFalse(0xffff)) as i32;
            self.emit_code(OpCode::Pop);
        }

        if !self.match_token(TokenType::RightParen) {
            let body_jump = self.emit_code(OpCode::Jump(0xffff));
            let incr_start = self.current_chunk().code.len();
            self.expression();
            self.emit_code(OpCode::Pop);
            self.consume(TokenType::RightParen, "Expect ')' after for clauses.");
            self.emit_loop(loop_start);
            loop_start = incr_start;
            self.patch_jump(body_jump);
        }
        self.statement();
        self.emit_loop(loop_start);

        if exit_jump != -1 {
            self.patch_jump(exit_jump as usize);
            self.emit_code(OpCode::Pop);
        }
        self.end_scope();
    }

    fn return_statement(&mut self) {
        if self.match_token(TokenType::Semicolon) {
            self.emit_return();
        } else {
            self.expression();
            self.consume(TokenType::Semicolon, "Expect ';' return value.");
            self.emit_code(OpCode::Return);
        }
    }

    fn emit_loop(&mut self, start_pos: usize) {
        let offset = self.current_chunk().code.len() - start_pos + 1;
        if let Ok(o) = u16::try_from(offset) {
            self.emit_code(OpCode::Loop(o));
        } else {
            self.error("Loop body too large.");
        }
    }

    fn patch_jump(&mut self, offset: usize) {
        let jump = self.current_chunk().code.len() - 1 - offset;
        if let Ok(o) = u16::try_from(jump) {
            match self.current_chunk().code[offset] {
                OpCode::JumpIfFalse(ref mut p) => *p = o,
                OpCode::Jump(ref mut p) => *p = o,
                _ => self.error("Offset is not jump instruction."),
            }
        } else {
            self.error("Too much code to jump over.");
        }
    }

    fn expression_statement(&mut self) {
        self.expression();
        self.consume(TokenType::Semicolon, "Expect ';' after value.");
        self.emit_code(OpCode::Pop);
    }

    fn declaration(&mut self) {
        if self.match_token(TokenType::Var) {
            self.var_declaration();
        } else if self.match_token(TokenType::Fun) {
            self.fun_declaration();
        } else {
            self.statement();
        }
        if self.panic_mode {
            self.synchronize();
        }
    }

    fn fun_declaration(&mut self) {
        let global = self.parse_variable("Expect function name.");
        self.mark_init();
        self.function(FunctionType::Function);
        self.define_variable(global);
    }

    fn var_declaration(&mut self) {
        let global = self.parse_variable("Expect variable name.");
        if self.match_token(TokenType::Equal) {
            self.expression();
        } else {
            self.emit_code(OpCode::Nil);
        }
        self.consume(
            TokenType::Semicolon,
            "Expect ';' after variable declaration.",
        );
        self.define_variable(global);
    }

    fn parse_variable(&mut self, msg: &str) -> u8 {
        self.consume(TokenType::Identifier, msg);
        self.declare_variable();
        if self.compiler.scope_depth > 0 {
            return 0;
        }
        self.identifier_constant(self.previous)
    }

    fn identifier_constant(&mut self, name: Token) -> u8 {
        self.make_constant(Value::String(name.lexeme.into()))
    }

    fn define_variable(&mut self, idx: u8) {
        if self.compiler.scope_depth > 0 {
            self.mark_init();
            return;
        }
        self.emit_code(OpCode::DefineGlobal(idx));
    }

    fn mark_init(&mut self) {
        if self.compiler.scope_depth == 0 {
            return;
        }
        let len = self.compiler.locals.len();
        self.compiler.locals[len - 1].depth = self.compiler.scope_depth;
    }

    fn function(&mut self, fn_ty: FunctionType) {
        self.push_compiler(fn_ty);
        self.begin_scope();
        self.consume(TokenType::LeftParen, "Expect '(' after function name.");
        if !self.check(TokenType::RightParen) {
            loop {
                self.compiler.function.arity += 1;
                if self.compiler.function.arity > 255 {
                    self.error_at_current("Can't have more then 255 parameters.");
                }
                let c = self.parse_variable("Expect parameter name.");
                self.define_variable(c);
                if !self.match_token(TokenType::Comma) {
                    break;
                }
            }
        }
        self.consume(TokenType::RightParen, "Expect ')' after parameters.");
        self.consume(TokenType::LeftBrace, "Expect '{' after function body.");
        self.block();
        let f = self.pop_compiler();
        let v = self.make_constant(Value::Function(Rc::from(f)));
        self.emit_code(OpCode::Closure(v));
    }

    fn declare_variable(&mut self) {
        if self.compiler.scope_depth == 0 {
            return;
        }

        let name = self.previous;
        let mut has_err = false;
        for local in self.compiler.locals.iter().rev() {
            if local.depth != -1 && local.depth < self.compiler.scope_depth {
                break;
            }
            if name.lexeme == local.name.lexeme {
                has_err = true;
                break;
            }
        }
        if has_err {
            self.error("Already a variable with this name in this scope.");
        }
        self.add_local(name);
    }

    fn add_local(&mut self, name: Token<'c>) {
        if self.compiler.locals.len() == Compiler::LOCAL_COUNT {
            self.error("Too many local variables in function.");
            return;
        }
        let local = Local::new(name, -1);
        self.compiler.locals.push(local);
    }

    fn parse_precedence(&mut self, prec: Precedence) {
        self.advance();
        let pre_rule = Parser::get_rule(self.previous.kind).prefix;
        let pre_rule = if let Some(f) = pre_rule {
            f
        } else {
            self.error("Expect expression.");
            return;
        };

        let can_assign = prec <= Precedence::Assignment;
        pre_rule(self, can_assign);

        while prec <= Parser::get_rule(self.current.kind).prec {
            self.advance();
            let inf_rule = Parser::get_rule(self.previous.kind).infix;
            if let Some(f) = inf_rule {
                f(self, can_assign);
            }
        }

        if can_assign && self.match_token(TokenType::Equal) {
            self.error("Invalid assignment target.");
        }
    }

    pub fn push_compiler(&mut self, fn_ty: FunctionType) {
        let name = self.previous.lexeme;
        let newc = Compiler::new(name, fn_ty);
        let oldc = mem::replace(&mut self.compiler, newc);
        self.compiler.enclosing = Some(Box::new(oldc));
    }

    pub fn pop_compiler(&mut self) -> Function {
        if let Some(enclosing_box) = self.compiler.enclosing.take() {
            let current_compiler = mem::replace(&mut self.compiler, *enclosing_box);
            current_compiler.function
        } else {
            mem::replace(&mut self.compiler.function, Function::new("<script>"))
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
            TokenType::LeftParen => {
                ParseRule::new(Some(Parser::grouping), Some(Parser::call), Precedence::Call)
            }
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
