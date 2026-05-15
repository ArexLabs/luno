use crate::error::{Span, Diag, Diagnostics};

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    // Literals
    Int(i64),
    Float(f64),
    Bool(bool),
    Char(u8),
    Str(String),
    Null,

    // Identifiers & keywords
    Ident(String),
    Let, Const, Fn, Return,
    If, Elif, Else,
    For, In, While,
    Break, Continue,
    Match, Case,
    Type, Enum, Trait, Impl,
    Import, From,
    Async, Await, Spawn,
    Self_, True, False,
    And, Or, Not,
    As, Move, Make,
    Underscore,

    // Operators
    Plus, Minus, Star, Slash, Percent,
    Assign, PlusEq, MinusEq, StarEq, SlashEq,
    Eq, NotEq, Lt, Gt, LtEq, GtEq,
    AndAnd, OrOr,
    Shl, Shr,
    Pipe,       // |
    Colon, ColonEq,   // : :=
    Arrow,      // ->
    FatArrow,   // =>
    Dot, DotDot, // . ..
    Hash,       // #
    At,         // @
    Tilde,      // ~
    Caret,      // ^
    Ampersand, AmpersandMut, // & &mut
    StarStar,   // **

    // Delimiters
    LParen, RParen,
    LBrack, RBrack,
    LBrace, RBrace,
    Comma, Semicolon,

    // Special
    Newline,
    Eof,
    Error(String),
}

#[derive(Debug, Clone)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

impl Token {
    pub fn new(kind: TokenKind, start: usize, end: usize, line: usize, col: usize) -> Self {
        Token { kind, span: Span::new(start, end, line, col) }
    }
}

pub struct Lexer {
    chars: Vec<char>,
    pos: usize,
    line: usize,
    col: usize,
    diags: Diagnostics,
}

impl Lexer {
    pub fn new(source: &str) -> Self {
        Lexer {
            chars: source.chars().collect(),
            pos: 0,
            line: 1,
            col: 1,
            diags: Diagnostics::new(),
        }
    }

    pub fn tokenize(&mut self) -> std::result::Result<Vec<Token>, Diagnostics> {
        let mut tokens = Vec::new();

        loop {
            self.skip_whitespace_and_comments();
            if self.pos >= self.chars.len() {
                break;
            }

            let ch = self.chars[self.pos];
            let start = self.pos;
            let line = self.line;
            let col = self.col;

            match ch {
                // Single-line comment: # to end of line
                '#' => {
                    self.advance();
                    while self.pos < self.chars.len() && self.chars[self.pos] != '\n' {
                        self.advance();
                    }
                    continue;
                }

                // Newlines
                '\n' => {
                    self.advance();
                    tokens.push(Token::new(TokenKind::Newline, start, self.pos, line, col));
                    continue;
                }
                '\r' => { self.advance(); continue; }

                // Strings
                '"' => {
                    tokens.push(self.read_string(start, line, col)?);
                    continue;
                }
                '\'' => {
                    tokens.push(self.read_char(start, line, col)?);
                    continue;
                }

                // Digits
                '0'..='9' => {
                    tokens.push(self.read_number(start, line, col)?);
                    continue;
                }

                // Identifiers and keywords
                'a'..='z' | 'A'..='Z' | '_' => {
                    tokens.push(self.read_ident_or_keyword(start, line, col));
                    continue;
                }

                _ => {}
            }

            // Operators and delimiters
            match ch {
                '+' => {
                    self.advance();
                    if self.matches('=') { tokens.push(self.tok(TokenKind::PlusEq, start, line, col)); }
                    else { tokens.push(self.tok(TokenKind::Plus, start, line, col)); }
                }
                '-' => {
                    self.advance();
                    if self.matches('=') { tokens.push(self.tok(TokenKind::MinusEq, start, line, col)); }
                    else if self.matches('>') { tokens.push(self.tok(TokenKind::Arrow, start, line, col)); }
                    else { tokens.push(self.tok(TokenKind::Minus, start, line, col)); }
                }
                '*' => {
                    self.advance();
                    if self.matches('=') { tokens.push(self.tok(TokenKind::StarEq, start, line, col)); }
                    else if self.matches('*') { tokens.push(self.tok(TokenKind::StarStar, start, line, col)); }
                    else { tokens.push(self.tok(TokenKind::Star, start, line, col)); }
                }
                '/' => {
                    self.advance();
                    if self.matches('=') { tokens.push(self.tok(TokenKind::SlashEq, start, line, col)); }
                    else { tokens.push(self.tok(TokenKind::Slash, start, line, col)); }
                }
                '%' => { self.advance(); tokens.push(self.tok(TokenKind::Percent, start, line, col)); }
                '=' => {
                    self.advance();
                    if self.matches('=') { tokens.push(self.tok(TokenKind::Eq, start, line, col)); }
                    else if self.matches('>') { tokens.push(self.tok(TokenKind::FatArrow, start, line, col)); }
                    else { tokens.push(self.tok(TokenKind::Assign, start, line, col)); }
                }
                '!' => {
                    self.advance();
                    if self.matches('=') { tokens.push(self.tok(TokenKind::NotEq, start, line, col)); }
                    else { tokens.push(self.tok(TokenKind::Not, start, line, col)); }
                }
                '<' => {
                    self.advance();
                    if self.matches('=') { tokens.push(self.tok(TokenKind::LtEq, start, line, col)); }
                    else if self.matches('<') {
                        self.advance();
                        if self.matches('=') { panic!("<<="); }
                        else { tokens.push(self.tok(TokenKind::Shl, start, line, col)); }
                    }
                    else { tokens.push(self.tok(TokenKind::Lt, start, line, col)); }
                }
                '>' => {
                    self.advance();
                    if self.matches('=') { tokens.push(self.tok(TokenKind::GtEq, start, line, col)); }
                    else if self.matches('>') {
                        self.advance();
                        if self.matches('=') { panic!(">>="); }
                        else { tokens.push(self.tok(TokenKind::Shr, start, line, col)); }
                    }
                    else { tokens.push(self.tok(TokenKind::Gt, start, line, col)); }
                }
                '&' => {
                    self.advance();
                    if self.matches('&') { tokens.push(self.tok(TokenKind::AndAnd, start, line, col)); }
                    else if self.check_str("mut") {
                        self.advance(); self.advance(); self.advance(); // m u t
                        tokens.push(self.tok(TokenKind::AmpersandMut, start, line, col));
                    }
                    else { tokens.push(self.tok(TokenKind::Ampersand, start, line, col)); }
                }
                '|' => {
                    self.advance();
                    if self.matches('|') { tokens.push(self.tok(TokenKind::OrOr, start, line, col)); }
                    else { tokens.push(self.tok(TokenKind::Pipe, start, line, col)); }
                }
                '^' => { self.advance(); tokens.push(self.tok(TokenKind::Caret, start, line, col)); }
                '~' => { self.advance(); tokens.push(self.tok(TokenKind::Tilde, start, line, col)); }
                '.' => {
                    self.advance();
                    if self.matches('.') { tokens.push(self.tok(TokenKind::DotDot, start, line, col)); }
                    else { tokens.push(self.tok(TokenKind::Dot, start, line, col)); }
                }
                ':' => {
                    self.advance();
                    if self.matches('=') { tokens.push(self.tok(TokenKind::ColonEq, start, line, col)); }
                    else { tokens.push(self.tok(TokenKind::Colon, start, line, col)); }
                }
                '@' => { self.advance(); tokens.push(self.tok(TokenKind::At, start, line, col)); }
                '(' => { self.advance(); tokens.push(self.tok(TokenKind::LParen, start, line, col)); }
                ')' => { self.advance(); tokens.push(self.tok(TokenKind::RParen, start, line, col)); }
                '[' => { self.advance(); tokens.push(self.tok(TokenKind::LBrack, start, line, col)); }
                ']' => { self.advance(); tokens.push(self.tok(TokenKind::RBrack, start, line, col)); }
                '{' => { self.advance(); tokens.push(self.tok(TokenKind::LBrace, start, line, col)); }
                '}' => { self.advance(); tokens.push(self.tok(TokenKind::RBrace, start, line, col)); }
                ',' => { self.advance(); tokens.push(self.tok(TokenKind::Comma, start, line, col)); }
                ';' => { self.advance(); tokens.push(self.tok(TokenKind::Semicolon, start, line, col)); }

                _ => {
                    let msg = format!("unexpected character '{}'", ch);
                    self.diags.push(Diag::error(msg, Span::new(start, self.pos + 1, line, col)));
                    self.advance();
                }
            }
        }

        tokens.push(Token::new(TokenKind::Eof, self.chars.len(), self.chars.len(), self.line, self.col));

        if self.diags.has_errors() {
            Err(std::mem::take(&mut self.diags))
        } else {
            Ok(tokens)
        }
    }

    fn tok(&self, kind: TokenKind, start: usize, line: usize, col: usize) -> Token {
        Token::new(kind, start, self.pos, line, col)
    }

    fn advance(&mut self) {
        if self.pos < self.chars.len() {
            if self.chars[self.pos] == '\n' {
                self.line += 1;
                self.col = 1;
            } else {
                self.col += 1;
            }
            self.pos += 1;
        }
    }

    fn peek(&self) -> Option<char> {
        if self.pos < self.chars.len() { Some(self.chars[self.pos]) } else { None }
    }

    fn matches(&mut self, expected: char) -> bool {
        if self.pos < self.chars.len() && self.chars[self.pos] == expected {
            self.advance();
            true
        } else {
            false
        }
    }

    fn check_str(&self, s: &str) -> bool {
        for (i, c) in s.chars().enumerate() {
            if self.pos + i >= self.chars.len() || self.chars[self.pos + i] != c {
                return false;
            }
        }
        true
    }

    fn skip_whitespace_and_comments(&mut self) {
        loop {
            match self.peek() {
                Some(c) if c == ' ' || c == '\t' => { self.advance(); }
                _ => break,
            }
        }
    }

    fn read_string(&mut self, start: usize, line: usize, col: usize) -> std::result::Result<Token, Diagnostics> {
        self.advance(); // skip opening "
        let mut s = String::new();
        loop {
            match self.peek() {
                None => {
                    self.diags.push(Diag::error("unterminated string literal",
                        Span::new(start, self.pos, line, col)));
                    return Err(std::mem::take(&mut self.diags));
                }
                Some('"') => {
                    self.advance();
                    return Ok(Token::new(TokenKind::Str(s), start, self.pos, line, col));
                }
                Some('\\') => {
                    self.advance();
                    match self.peek() {
                        Some('n') => { s.push('\n'); self.advance(); }
                        Some('t') => { s.push('\t'); self.advance(); }
                        Some('r') => { s.push('\r'); self.advance(); }
                        Some('\\') => { s.push('\\'); self.advance(); }
                        Some('"') => { s.push('"'); self.advance(); }
                        Some('0') => { s.push('\0'); self.advance(); }
                        Some(c) => { s.push('\\'); s.push(c); self.advance(); }
                        None => {
                            self.diags.push(Diag::error("unterminated escape in string",
                                Span::new(start, self.pos, line, col)));
                            return Err(std::mem::take(&mut self.diags));
                        }
                    }
                }
                Some(c) => { s.push(c); self.advance(); }
            }
        }
    }

    fn read_char(&mut self, start: usize, line: usize, col: usize) -> std::result::Result<Token, Diagnostics> {
        self.advance(); // skip '
        let val = match self.peek() {
            None => {
                self.diags.push(Diag::error("unterminated char literal",
                    Span::new(start, self.pos, line, col)));
                return Err(std::mem::take(&mut self.diags));
            }
            Some(c) => { self.advance(); c as u8 }
        };
        if self.peek() != Some('\'') {
            self.diags.push(Diag::error("unterminated char literal",
                Span::new(start, self.pos, line, col)));
            return Err(std::mem::take(&mut self.diags));
        }
        self.advance(); // skip closing '
        Ok(Token::new(TokenKind::Char(val), start, self.pos, line, col))
    }

    fn read_number(&mut self, start: usize, line: usize, col: usize) -> std::result::Result<Token, Diagnostics> {
        let mut is_float = false;
        let mut num_str = String::new();

        while self.pos < self.chars.len() {
            let c = self.chars[self.pos];
            if c.is_ascii_digit() {
                num_str.push(c);
                self.advance();
            } else if c == '.' {
                if self.pos + 1 < self.chars.len() && self.chars[self.pos + 1].is_ascii_digit() {
                    is_float = true;
                    num_str.push(c);
                    self.advance();
                } else {
                    break;
                }
            } else if c == '_' {
                self.advance(); // skip numeric separator
            } else {
                break;
            }
        }

        if is_float {
            match num_str.parse::<f64>() {
                Ok(n) => Ok(Token::new(TokenKind::Float(n), start, self.pos, line, col)),
                Err(_) => {
                    self.diags.push(Diag::error("invalid float literal",
                        Span::new(start, self.pos, line, col)));
                    Err(std::mem::take(&mut self.diags))
                }
            }
        } else {
            match num_str.parse::<i64>() {
                Ok(n) => Ok(Token::new(TokenKind::Int(n), start, self.pos, line, col)),
                Err(_) => {
                    self.diags.push(Diag::error("invalid integer literal",
                        Span::new(start, self.pos, line, col)));
                    Err(std::mem::take(&mut self.diags))
                }
            }
        }
    }

    fn read_ident_or_keyword(&mut self, start: usize, line: usize, col: usize) -> Token {
        let mut s = String::new();
        while self.pos < self.chars.len() {
            let c = self.chars[self.pos];
            if c.is_alphanumeric() || c == '_' {
                s.push(c);
                self.advance();
            } else {
                break;
            }
        }

        let kind = match s.as_str() {
            "let" => TokenKind::Let,
            "const" => TokenKind::Const,
            "fn" => TokenKind::Fn,
            "return" => TokenKind::Return,
            "if" => TokenKind::If,
            "elif" => TokenKind::Elif,
            "else" => TokenKind::Else,
            "for" => TokenKind::For,
            "in" => TokenKind::In,
            "while" => TokenKind::While,
            "break" => TokenKind::Break,
            "continue" => TokenKind::Continue,
            "match" => TokenKind::Match,
            "case" => TokenKind::Case,
            "type" => TokenKind::Type,
            "enum" => TokenKind::Enum,
            "trait" => TokenKind::Trait,
            "impl" => TokenKind::Impl,
            "import" => TokenKind::Import,
            "from" => TokenKind::From,
            "async" => TokenKind::Async,
            "await" => TokenKind::Await,
            "spawn" => TokenKind::Spawn,
            "self" => TokenKind::Self_,
            "true" => TokenKind::True,
            "false" => TokenKind::False,
            "and" => TokenKind::And,
            "or" => TokenKind::Or,
            "not" => TokenKind::Not,
            "as" => TokenKind::As,
            "move" => TokenKind::Move,
            "make" => TokenKind::Make,
            "_" => TokenKind::Underscore,
            _ => TokenKind::Ident(s),
        };

        Token::new(kind, start, self.pos, line, col)
    }
}
