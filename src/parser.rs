use crate::ast::*;
use crate::error::{Span, Diag, Diagnostics};
use crate::lexer::{Token, TokenKind};

pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
    diags: Diagnostics,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Parser { tokens, pos: 0, diags: Diagnostics::new() }
    }

    fn current(&self) -> &Token {
        &self.tokens[self.pos]
    }

    fn kind(&self) -> &TokenKind {
        &self.current().kind
    }

    fn span(&self) -> Span {
        self.current().span.clone()
    }

    fn at_end(&self) -> bool {
        matches!(self.kind(), TokenKind::Eof)
    }

    fn advance(&mut self) -> Token {
        let tok = self.tokens[self.pos].clone();
        if self.pos < self.tokens.len() {
            self.pos += 1;
        }
        tok
    }

    fn expect(&mut self, kind: &TokenKind) -> std::result::Result<Token, Diagnostics> {
        if self.kind() == kind {
            Ok(self.advance())
        } else {
            let msg = format!("expected {:?}, found {:?}", kind, self.kind());
            self.diags.push(Diag::error(msg, self.span()));
            Err(std::mem::take(&mut self.diags))
        }
    }

    fn expect_ident(&mut self) -> std::result::Result<String, Diagnostics> {
        match self.kind() {
            TokenKind::Ident(s) => {
                let s = s.clone();
                self.advance();
                Ok(s)
            }
            _ => {
                let msg = format!("expected identifier, found {:?}", self.kind());
                self.diags.push(Diag::error(msg, self.span()));
                Err(std::mem::take(&mut self.diags))
            }
        }
    }

    fn skip_newlines(&mut self) {
        while matches!(self.kind(), TokenKind::Newline) {
            self.advance();
        }
    }

    // --- Entry point ---

    pub fn parse(&mut self) -> std::result::Result<Program, Diagnostics> {
        let mut stmts = Vec::new();
        self.skip_newlines();

        while !self.at_end() {
            match self.parse_stmt() {
                Ok(s) => stmts.push(s),
                Err(_) => {
                    if self.diags.has_errors() {
                        break;
                    }
                    if !self.at_end() {
                        self.advance();
                    }
                }
            }
            self.skip_newlines();
        }

        if self.diags.has_errors() {
            Err(std::mem::take(&mut self.diags))
        } else {
            Ok(Program { stmts })
        }
    }

    // --- Statement parsing ---

    fn parse_stmt(&mut self) -> std::result::Result<Stmt, Diagnostics> {
        match self.kind() {
            TokenKind::Let => self.parse_let(),
            TokenKind::Const => self.parse_const(),
            TokenKind::Fn => self.parse_fn_def(),
            TokenKind::Return => self.parse_return(),
            TokenKind::If => self.parse_if(),
            TokenKind::For => self.parse_for(),
            TokenKind::While => self.parse_while(),
            TokenKind::Break => {
                let s = self.span();
                self.advance();
                Ok(Stmt::Break(s))
            }
            TokenKind::Continue => {
                let s = self.span();
                self.advance();
                Ok(Stmt::Continue(s))
            }
            TokenKind::Match => self.parse_match(),
            TokenKind::Type => self.parse_type_def(),
            TokenKind::Enum => self.parse_enum_def(),
            TokenKind::Trait => self.parse_trait_def(),
            TokenKind::Impl => self.parse_impl(),
            TokenKind::Import => self.parse_import(),
            TokenKind::From => self.parse_from_import(),
            TokenKind::LBrace => {
                let s = self.span();
                let body = self.parse_block()?;
                Ok(Stmt::Expr(Expr::Block(body, s)))
            }
            _ => self.parse_expr_stmt(),
        }
    }

    // --- Let: name := value or name: Type = value ---

    fn parse_let(&mut self) -> std::result::Result<Stmt, Diagnostics> {
        let s = self.span();
        self.advance(); // skip 'let'
        let name = self.expect_ident()?;
        let _mutable = true;
        let mut type_hint = None;
        let value;

        match self.kind() {
            TokenKind::ColonEq => {
                self.advance();
                value = self.parse_expr()?;
            }
            TokenKind::Colon => {
                self.advance();
                type_hint = Some(self.parse_type_expr()?);
                self.expect(&TokenKind::Assign)?;
                value = self.parse_expr()?;
            }
            TokenKind::Assign => {
                self.advance();
                value = self.parse_expr()?;
            }
            _ => {
                self.diags.push(Diag::error("expected ':=' or ': Type ='", self.span()));
                return Err(std::mem::take(&mut self.diags));
            }
        }

        Ok(Stmt::Let { name, type_hint, value, mutable: _mutable, span: s })
    }

    // --- Const: const name = value ---

    fn parse_const(&mut self) -> std::result::Result<Stmt, Diagnostics> {
        let s = self.span();
        self.advance();
        let name = self.expect_ident()?;
        self.expect(&TokenKind::Assign)?;
        let value = self.parse_expr()?;
        Ok(Stmt::Const { name, value, span: s })
    }

    // --- Function: fn name(params) -> Type { body } ---

    fn parse_fn_def(&mut self) -> std::result::Result<Stmt, Diagnostics> {
        let s = self.span();
        self.advance(); // skip 'fn'
        let name = self.expect_ident()?;
        let _generics = self.parse_generic_params()?;
        self.expect(&TokenKind::LParen)?;
        let params = self.parse_params()?;
        self.expect(&TokenKind::RParen)?;

        let return_type = if matches!(self.kind(), TokenKind::Arrow) {
            self.advance();
            Some(self.parse_type_expr()?)
        } else {
            None
        };

        let body = self.parse_block()?;

        Ok(Stmt::FnDef { name, params, return_type, body, span: s })
    }

    fn parse_generic_params(&mut self) -> std::result::Result<Vec<String>, Diagnostics> {
        if matches!(self.kind(), TokenKind::LBrack) {
            self.advance(); // skip [
            let mut params = Vec::new();
            params.push(self.expect_ident()?);
            while matches!(self.kind(), TokenKind::Comma) {
                self.advance();
                params.push(self.expect_ident()?);
            }
            self.expect(&TokenKind::RBrack)?;
            Ok(params)
        } else {
            Ok(vec![])
        }
    }

    fn parse_params(&mut self) -> std::result::Result<Vec<Param>, Diagnostics> {
        let mut params = Vec::new();
        if matches!(self.kind(), TokenKind::RParen) {
            return Ok(params);
        }

        loop {
            let s = self.span();
            let name = self.expect_ident()?;
            let type_hint = if matches!(self.kind(), TokenKind::Colon) {
                self.advance();
                Some(self.parse_type_expr()?)
            } else {
                None
            };
            let default = if matches!(self.kind(), TokenKind::Assign) {
                self.advance();
                Some(self.parse_expr()?)
            } else {
                None
            };
            params.push(Param { name, type_hint, default, span: s });

            if matches!(self.kind(), TokenKind::Comma) {
                self.advance();
            } else {
                break;
            }
        }
        Ok(params)
    }

    fn parse_block(&mut self) -> std::result::Result<Vec<Stmt>, Diagnostics> {
        self.expect(&TokenKind::LBrace)?;
        self.skip_newlines();
        let mut stmts = Vec::new();
        while !matches!(self.kind(), TokenKind::RBrace) && !self.at_end() {
            stmts.push(self.parse_stmt()?);
            self.skip_newlines();
        }
        self.expect(&TokenKind::RBrace)?;
        Ok(stmts)
    }

    // --- Return ---

    fn parse_return(&mut self) -> std::result::Result<Stmt, Diagnostics> {
        let s = self.span();
        self.advance();
        if matches!(self.kind(), TokenKind::Newline | TokenKind::RBrace | TokenKind::Eof) {
            Ok(Stmt::Return(None, s))
        } else {
            let expr = self.parse_expr()?;
            Ok(Stmt::Return(Some(expr), s))
        }
    }

    // --- If / Elif / Else ---

    fn parse_if(&mut self) -> std::result::Result<Stmt, Diagnostics> {
        let s = self.span();
        self.advance();
        let cond = self.parse_expr()?;
        let body = self.parse_block()?;

        let mut elifs = Vec::new();
        let mut else_body = None;

        self.skip_newlines();
        while matches!(self.kind(), TokenKind::Elif) {
            self.advance();
            let elif_cond = self.parse_expr()?;
            let elif_body = self.parse_block()?;
            elifs.push((elif_cond, elif_body));
            self.skip_newlines();
        }

        if matches!(self.kind(), TokenKind::Else) {
            self.advance();
            else_body = Some(self.parse_block()?);
        }

        Ok(Stmt::Expr(Expr::If(Box::new(cond), body, elifs, else_body, s)))
    }

    // --- For loop ---

    fn parse_for(&mut self) -> std::result::Result<Stmt, Diagnostics> {
        let s = self.span();
        self.advance();
        let var = self.expect_ident()?;
        self.expect(&TokenKind::In)?;
        let iterable = self.parse_expr()?;
        let body = self.parse_block()?;
        Ok(Stmt::Expr(Expr::ForLoop(var, Box::new(iterable), body, s)))
    }

    // --- While loop ---

    fn parse_while(&mut self) -> std::result::Result<Stmt, Diagnostics> {
        let s = self.span();
        self.advance();
        let cond = self.parse_expr()?;
        let body = self.parse_block()?;
        Ok(Stmt::Expr(Expr::WhileLoop(Box::new(cond), body, s)))
    }

    // --- Match ---

    fn parse_match(&mut self) -> std::result::Result<Stmt, Diagnostics> {
        let s = self.span();
        self.advance();
        let value = self.parse_expr()?;
        self.expect(&TokenKind::LBrace)?;
        self.skip_newlines();

        let mut arms = Vec::new();
        while matches!(self.kind(), TokenKind::Case) {
            self.advance();
            let pattern = self.parse_pattern()?;
            self.expect(&TokenKind::FatArrow)?;
            let body = if matches!(self.kind(), TokenKind::LBrace) {
                self.parse_block()?
            } else {
                let expr = self.parse_expr()?;
                vec![Stmt::Expr(expr)]
            };
            arms.push((pattern, body));
            self.skip_newlines();
        }

        self.expect(&TokenKind::RBrace)?;
        Ok(Stmt::Expr(Expr::Match(Box::new(value), arms, s)))
    }

    // --- Pattern ---

    fn parse_pattern(&mut self) -> std::result::Result<Pattern, Diagnostics> {
        let s = self.span();
        match self.kind() {
            TokenKind::Underscore => {
                self.advance();
                Ok(Pattern::Wildcard(s))
            }
            TokenKind::Int(n) => {
                let n = *n;
                self.advance();
                Ok(Pattern::Literal(Literal::Int(n), s))
            }
            TokenKind::Str(sval) => {
                let sval = sval.clone();
                self.advance();
                Ok(Pattern::Literal(Literal::String(sval), s))
            }
            TokenKind::True => {
                self.advance();
                Ok(Pattern::Literal(Literal::Bool(true), s))
            }
            TokenKind::False => {
                self.advance();
                Ok(Pattern::Literal(Literal::Bool(false), s))
            }
            TokenKind::Ident(name) => {
                let name = name.clone();
                self.advance();
                // Check if it's a variant pattern (PascalCase)
                if name.chars().next().map_or(false, |c| c.is_uppercase()) {
                    if matches!(self.kind(), TokenKind::LParen) {
                        self.advance();
                        let mut patterns = Vec::new();
                        if !matches!(self.kind(), TokenKind::RParen) {
                            loop {
                                patterns.push(self.parse_pattern()?);
                                if matches!(self.kind(), TokenKind::Comma) {
                                    self.advance();
                                } else {
                                    break;
                                }
                            }
                        }
                        self.expect(&TokenKind::RParen)?;
                        Ok(Pattern::Variant(name, patterns, s))
                    } else {
                        Ok(Pattern::Variant(name, vec![], s))
                    }
                } else {
                    Ok(Pattern::Binding(name, s))
                }
            }
            _ => {
                let msg = format!("expected pattern, found {:?}", self.kind());
                self.diags.push(Diag::error(msg, s));
                Err(std::mem::take(&mut self.diags))
            }
        }
    }

    // --- Type definition: type Name { fields } ---

    fn parse_type_def(&mut self) -> std::result::Result<Stmt, Diagnostics> {
        let s = self.span();
        self.advance();
        let name = self.expect_ident()?;
        let generics = self.parse_generic_params()?;
        self.expect(&TokenKind::LBrace)?;
        self.skip_newlines();

        let mut fields = Vec::new();
        while !matches!(self.kind(), TokenKind::RBrace) && !self.at_end() {
            let fs = self.span();
            let fname = self.expect_ident()?;
            self.expect(&TokenKind::Colon)?;
            let ftype = self.parse_type_expr()?;
            fields.push(Field { name: fname, type_expr: ftype, span: fs });
            self.skip_newlines();
        }
        self.expect(&TokenKind::RBrace)?;

        Ok(Stmt::TypeDef { name, generics, fields, span: s })
    }

    // --- Enum: enum Name { Variant, Variant(Type), ... } ---

    fn parse_enum_def(&mut self) -> std::result::Result<Stmt, Diagnostics> {
        let s = self.span();
        self.advance();
        let name = self.expect_ident()?;
        let generics = self.parse_generic_params()?;
        self.expect(&TokenKind::LBrace)?;
        self.skip_newlines();

        let mut variants = Vec::new();
        while !matches!(self.kind(), TokenKind::RBrace) && !self.at_end() {
            let vs = self.span();
            let vname = self.expect_ident()?;
            let mut types = Vec::new();
            if matches!(self.kind(), TokenKind::LParen) {
                self.advance();
                if !matches!(self.kind(), TokenKind::RParen) {
                    loop {
                        types.push(self.parse_type_expr()?);
                        if matches!(self.kind(), TokenKind::Comma) {
                            self.advance();
                        } else {
                            break;
                        }
                    }
                }
                self.expect(&TokenKind::RParen)?;
            }
            variants.push(Variant { name: vname, types, span: vs });
            self.skip_newlines();
            if matches!(self.kind(), TokenKind::Comma) {
                self.advance();
                self.skip_newlines();
            }
        }
        self.expect(&TokenKind::RBrace)?;

        Ok(Stmt::EnumDef { name, generics, variants, span: s })
    }

    // --- Trait: trait Name { fn sig(); ... } ---

    fn parse_trait_def(&mut self) -> std::result::Result<Stmt, Diagnostics> {
        let s = self.span();
        self.advance();
        let name = self.expect_ident()?;
        self.expect(&TokenKind::LBrace)?;
        self.skip_newlines();

        let mut methods = Vec::new();
        while !matches!(self.kind(), TokenKind::RBrace) && !self.at_end() {
            let ms = self.span();
            self.expect(&TokenKind::Fn)?;
            let mname = self.expect_ident()?;
            self.expect(&TokenKind::LParen)?;
            let params = self.parse_params()?;
            self.expect(&TokenKind::RParen)?;
            let return_type = if matches!(self.kind(), TokenKind::Arrow) {
                self.advance();
                Some(self.parse_type_expr()?)
            } else {
                None
            };
            methods.push(TraitMethod { name: mname, params, return_type, span: ms });
            if matches!(self.kind(), TokenKind::Semicolon) {
                self.advance();
            }
            self.skip_newlines();
        }
        self.expect(&TokenKind::RBrace)?;

        Ok(Stmt::TraitDef { name, methods, span: s })
    }

    // --- Impl: impl Type { methods } or impl Trait for Type { methods } ---

    fn parse_impl(&mut self) -> std::result::Result<Stmt, Diagnostics> {
        let s = self.span();
        self.advance();

        let name = self.expect_ident()?;

        if matches!(self.kind(), TokenKind::For) {
            self.advance();
            let type_name = self.expect_ident()?;
            let body = self.parse_block()?;
            let methods = self.filter_methods(body);
            Ok(Stmt::ImplTrait { trait_name: name, type_name, methods, span: s })
        } else {
            let body = self.parse_block()?;
            let methods = self.filter_methods(body);
            Ok(Stmt::ImplBlock { type_name: name, methods, span: s })
        }
    }

    fn filter_methods(&self, stmts: Vec<Stmt>) -> Vec<Stmt> {
        stmts.into_iter().filter(|s| matches!(s, Stmt::FnDef { .. })).collect()
    }

    // --- Import ---

    fn parse_import(&mut self) -> std::result::Result<Stmt, Diagnostics> {
        let s = self.span();
        self.advance();
        let module = self.expect_ident()?;
        Ok(Stmt::Import { module, span: s })
    }

    fn parse_from_import(&mut self) -> std::result::Result<Stmt, Diagnostics> {
        let s = self.span();
        self.advance();
        let module = self.expect_ident()?;
        self.expect(&TokenKind::Import)?;
        let mut names = Vec::new();
        names.push(self.expect_ident()?);
        while matches!(self.kind(), TokenKind::Comma) {
            self.advance();
            names.push(self.expect_ident()?);
        }
        Ok(Stmt::FromImport { module, names, span: s })
    }

    // --- Expression statement ---

    fn parse_expr_stmt(&mut self) -> std::result::Result<Stmt, Diagnostics> {
        let expr = self.parse_expr()?;

        match self.kind() {
            TokenKind::Assign => {
                let s = self.span();
                self.advance();
                let value = self.parse_expr()?;
                Ok(Stmt::Assign { target: expr, value, span: s })
            }
            TokenKind::PlusEq | TokenKind::MinusEq | TokenKind::StarEq | TokenKind::SlashEq => {
                let _op = match self.kind() {
                    TokenKind::PlusEq => AssignOp::AddEq,
                    TokenKind::MinusEq => AssignOp::SubEq,
                    TokenKind::StarEq => AssignOp::MulEq,
                    TokenKind::SlashEq => AssignOp::DivEq,
                    _ => unreachable!(),
                };
                let s = self.span();
                self.advance();
                let value = self.parse_expr()?;
                Ok(Stmt::Assign {
                    target: expr.clone(),
                    value: Expr::BinOp(Box::new(expr), BinOp::Add, Box::new(value), s.clone()),
                    span: s,
                })
            }
            _ => Ok(Stmt::Expr(expr)),
        }
    }

    // --- Expression parsing ---

    fn parse_expr(&mut self) -> std::result::Result<Expr, Diagnostics> {
        self.parse_or()
    }

    fn parse_or(&mut self) -> std::result::Result<Expr, Diagnostics> {
        let mut left = self.parse_and()?;
        while matches!(self.kind(), TokenKind::OrOr) {
            let s = self.span();
            self.advance();
            let right = self.parse_and()?;
            left = Expr::Logical(Box::new(left), BinOp::Or, Box::new(right), s);
        }
        Ok(left)
    }

    fn parse_and(&mut self) -> std::result::Result<Expr, Diagnostics> {
        let mut left = self.parse_cmp()?;
        while matches!(self.kind(), TokenKind::AndAnd) {
            let s = self.span();
            self.advance();
            let right = self.parse_cmp()?;
            left = Expr::Logical(Box::new(left), BinOp::And, Box::new(right), s);
        }
        Ok(left)
    }

    fn parse_cmp(&mut self) -> std::result::Result<Expr, Diagnostics> {
        let left = self.parse_add()?;

        let op = match self.kind() {
            TokenKind::Eq => CmpOp::Eq,
            TokenKind::NotEq => CmpOp::NotEq,
            TokenKind::Lt => CmpOp::Lt,
            TokenKind::Gt => CmpOp::Gt,
            TokenKind::LtEq => CmpOp::LtEq,
            TokenKind::GtEq => CmpOp::GtEq,
            _ => return Ok(left),
        };
        let s = self.span();
        self.advance();
        let right = self.parse_add()?;
        Ok(Expr::Cmp(Box::new(left), op, Box::new(right), s))
    }

    fn parse_add(&mut self) -> std::result::Result<Expr, Diagnostics> {
        let mut left = self.parse_mul()?;
        loop {
            let (op, s) = match self.kind() {
                TokenKind::Plus => (BinOp::Add, self.span()),
                TokenKind::Minus => (BinOp::Sub, self.span()),
                _ => break,
            };
            self.advance();
            let right = self.parse_mul()?;
            left = Expr::BinOp(Box::new(left), op, Box::new(right), s);
        }
        Ok(left)
    }

    fn parse_mul(&mut self) -> std::result::Result<Expr, Diagnostics> {
        let mut left = self.parse_unary()?;
        loop {
            let (op, s) = match self.kind() {
                TokenKind::Star => (BinOp::Mul, self.span()),
                TokenKind::Slash => (BinOp::Div, self.span()),
                TokenKind::Percent => (BinOp::Mod, self.span()),
                _ => break,
            };
            self.advance();
            let right = self.parse_unary()?;
            left = Expr::BinOp(Box::new(left), op, Box::new(right), s);
        }
        Ok(left)
    }

    fn parse_unary(&mut self) -> std::result::Result<Expr, Diagnostics> {
        match self.kind() {
            TokenKind::Minus => {
                let s = self.span();
                self.advance();
                let operand = self.parse_unary()?;
                Ok(Expr::UnaryOp(UnaryOp::Neg, Box::new(operand), s))
            }
            TokenKind::Not => {
                let s = self.span();
                self.advance();
                let operand = self.parse_unary()?;
                Ok(Expr::UnaryOp(UnaryOp::Not, Box::new(operand), s))
            }
            TokenKind::Ampersand => {
                let s = self.span();
                self.advance();
                let operand = self.parse_unary()?;
                Ok(Expr::UnaryOp(UnaryOp::Ref, Box::new(operand), s))
            }
            TokenKind::AmpersandMut => {
                let s = self.span();
                self.advance();
                let operand = self.parse_unary()?;
                Ok(Expr::UnaryOp(UnaryOp::MutRef, Box::new(operand), s))
            }
            TokenKind::Star => {
                let s = self.span();
                self.advance();
                let operand = self.parse_unary()?;
                Ok(Expr::UnaryOp(UnaryOp::Deref, Box::new(operand), s))
            }
            TokenKind::Await => {
                let s = self.span();
                self.advance();
                let operand = self.parse_unary()?;
                Ok(Expr::Await(Box::new(operand), s))
            }
            TokenKind::Spawn => {
                let s = self.span();
                self.advance();
                let operand = self.parse_unary()?;
                Ok(Expr::Spawn(Box::new(operand), s))
            }
            _ => self.parse_postfix(),
        }
    }

    fn parse_postfix(&mut self) -> std::result::Result<Expr, Diagnostics> {
        let mut expr = self.parse_primary()?;

        loop {
            match self.kind() {
                TokenKind::LParen => {
                    let s = self.span();
                    self.advance();
                    let mut args = Vec::new();
                    if !matches!(self.kind(), TokenKind::RParen) {
                        loop {
                            args.push(self.parse_expr()?);
                            if matches!(self.kind(), TokenKind::Comma) {
                                self.advance();
                            } else {
                                break;
                            }
                        }
                    }
                    self.expect(&TokenKind::RParen)?;
                    expr = Expr::Call(Box::new(expr), args, s);
                }
                TokenKind::Dot => {
                    let s = self.span();
                    self.advance();

                    if matches!(self.kind(), TokenKind::Dot) {
                        self.advance();
                        let end = self.parse_expr()?;
                        expr = Expr::Range(Box::new(expr), Box::new(end), s);
                    } else {
                        let name = self.expect_ident()?;
                        if matches!(self.kind(), TokenKind::LParen) {
                            self.advance();
                            let mut args = Vec::new();
                            if !matches!(self.kind(), TokenKind::RParen) {
                                loop {
                                    args.push(self.parse_expr()?);
                                    if matches!(self.kind(), TokenKind::Comma) {
                                        self.advance();
                                    } else {
                                        break;
                                    }
                                }
                            }
                            self.expect(&TokenKind::RParen)?;
                            expr = Expr::MethodCall(Box::new(expr), name, args, s);
                        } else {
                            expr = Expr::Attribute(Box::new(expr), name, s);
                        }
                    }
                }
                TokenKind::LBrack => {
                    let s = self.span();
                    self.advance();
                    let index = self.parse_expr()?;
                    self.expect(&TokenKind::RBrack)?;
                    expr = Expr::Index(Box::new(expr), Box::new(index), s);
                }
                TokenKind::Not => {
                    let s = self.span();
                    self.advance();
                    expr = Expr::TryOp(Box::new(expr), s);
                }
                _ => break,
            }
        }

        Ok(expr)
    }

    fn parse_primary(&mut self) -> std::result::Result<Expr, Diagnostics> {
        match self.kind() {
            TokenKind::Int(n) => {
                let n = *n;
                let s = self.span();
                self.advance();
                Ok(Expr::Literal(Literal::Int(n), s))
            }
            TokenKind::Float(n) => {
                let n = *n;
                let s = self.span();
                self.advance();
                Ok(Expr::Literal(Literal::Float(n), s))
            }
            TokenKind::Str(sval) => {
                let sval = sval.clone();
                let s = self.span();
                self.advance();
                Ok(Expr::Literal(Literal::String(sval), s))
            }
            TokenKind::Char(c) => {
                let c = *c;
                let s = self.span();
                self.advance();
                Ok(Expr::Literal(Literal::Char(c), s))
            }
            TokenKind::True => {
                let s = self.span();
                self.advance();
                Ok(Expr::Literal(Literal::Bool(true), s))
            }
            TokenKind::False => {
                let s = self.span();
                self.advance();
                Ok(Expr::Literal(Literal::Bool(false), s))
            }
            TokenKind::Null => {
                let s = self.span();
                self.advance();
                Ok(Expr::Literal(Literal::Null, s))
            }
            TokenKind::Ident(name) => {
                let name = name.clone();
                let s = self.span();
                self.advance();

                // Generic type instantiation: Foo[T]
                if matches!(self.kind(), TokenKind::LBrack) && name.chars().next().map_or(false, |c| c.is_uppercase()) {
                    self.advance();
                    let mut type_args = Vec::new();
                    loop {
                        type_args.push(self.parse_type_expr()?);
                        if matches!(self.kind(), TokenKind::Comma) {
                            self.advance();
                        } else {
                            break;
                        }
                    }
                    self.expect(&TokenKind::RBrack)?;

                    // Check if it's a struct literal: Foo[T] { ... }
                    if matches!(self.kind(), TokenKind::LBrace) {
                        let mut fields = Vec::new();
                        self.advance();
                        self.skip_newlines();
                        while !matches!(self.kind(), TokenKind::RBrace) && !self.at_end() {
                            let fname = self.expect_ident()?;
                            self.expect(&TokenKind::Colon)?;
                            let fval = self.parse_expr()?;
                            fields.push((fname, fval));
                            self.skip_newlines();
                            if matches!(self.kind(), TokenKind::Comma) {
                                self.advance();
                                self.skip_newlines();
                            }
                        }
                        self.expect(&TokenKind::RBrace)?;
                        return Ok(Expr::StructLit(name, fields, s));
                    }

                    return Ok(Expr::Ident(name, s));
                }

                // Enum variant: Foo::Bar or Foo(Bar)
                if matches!(self.kind(), TokenKind::Colon) && matches!(self.tokens.get(self.pos + 1).map(|t| &t.kind), Some(TokenKind::Colon)) {
                    self.advance(); self.advance(); // skip ::
                    let variant = self.expect_ident()?;
                    let args = if matches!(self.kind(), TokenKind::LParen) {
                        self.advance();
                        let mut a = Vec::new();
                        if !matches!(self.kind(), TokenKind::RParen) {
                            loop {
                                a.push(self.parse_expr()?);
                                if matches!(self.kind(), TokenKind::Comma) {
                                    self.advance();
                                } else {
                                    break;
                                }
                            }
                        }
                        self.expect(&TokenKind::RParen)?;
                        a
                    } else {
                        vec![]
                    };
                    return Ok(Expr::EnumVariant(name, variant, args, s));
                }

                // Struct literal: Name { ... }
                if matches!(self.kind(), TokenKind::LBrace) {
                    self.advance();
                    self.skip_newlines();
                    let mut fields = Vec::new();
                    while !matches!(self.kind(), TokenKind::RBrace) && !self.at_end() {
                        let fname = self.expect_ident()?;
                        self.expect(&TokenKind::Colon)?;
                        let fval = self.parse_expr()?;
                        fields.push((fname, fval));
                        self.skip_newlines();
                        if matches!(self.kind(), TokenKind::Comma) {
                            self.advance();
                            self.skip_newlines();
                        }
                    }
                    self.expect(&TokenKind::RBrace)?;
                    return Ok(Expr::StructLit(name, fields, s));
                }

                Ok(Expr::Ident(name, s))
            }
            TokenKind::Self_ => {
                let s = self.span();
                self.advance();
                Ok(Expr::Ident("self".into(), s))
            }
            TokenKind::LParen => {
                let s = self.span();
                self.advance();
                let mut exprs = Vec::new();
                exprs.push(self.parse_expr()?);
                while matches!(self.kind(), TokenKind::Comma) {
                    self.advance();
                    exprs.push(self.parse_expr()?);
                }
                self.expect(&TokenKind::RParen)?;
                if exprs.len() == 1 {
                    Ok(exprs.into_iter().next().unwrap())
                } else {
                    Ok(Expr::Tuple(exprs, s))
                }
            }
            TokenKind::LBrack => {
                let s = self.span();
                self.advance();
                let mut items = Vec::new();
                if !matches!(self.kind(), TokenKind::RBrack) {
                    loop {
                        items.push(self.parse_expr()?);
                        if matches!(self.kind(), TokenKind::Comma) {
                            self.advance();
                        } else {
                            break;
                        }
                    }
                }
                self.expect(&TokenKind::RBrack)?;
                Ok(Expr::List(items, s))
            }
            TokenKind::LBrace => {
                let body = self.parse_block()?;
                let s = self.span();
                Ok(Expr::Block(body, s))
            }
            _ => {
                let msg = format!("unexpected token {:?}", self.kind());
                self.diags.push(Diag::error(msg, self.span()));
                Err(std::mem::take(&mut self.diags))
            }
        }
    }

    // --- Type expressions ---

    fn parse_type_expr(&mut self) -> std::result::Result<TypeExpr, Diagnostics> {
        let s = self.span();
        match self.kind() {
            TokenKind::Ampersand => {
                self.advance();
                let inner = self.parse_type_expr()?;
                Ok(TypeExpr::Ref(Box::new(inner), s))
            }
            TokenKind::AmpersandMut => {
                self.advance();
                let inner = self.parse_type_expr()?;
                Ok(TypeExpr::MutRef(Box::new(inner), s))
            }
            TokenKind::Underscore => {
                self.advance();
                Ok(TypeExpr::Infer(s))
            }
            TokenKind::Ident(name) => {
                let name = name.clone();
                self.advance();

                if matches!(self.kind(), TokenKind::LBrack) {
                    self.advance();
                    let mut args = Vec::new();
                    loop {
                        args.push(self.parse_type_expr()?);
                        if matches!(self.kind(), TokenKind::Comma) {
                            self.advance();
                        } else {
                            break;
                        }
                    }
                    self.expect(&TokenKind::RBrack)?;
                    Ok(TypeExpr::Generic(name, args, s))
                } else {
                    Ok(TypeExpr::Named(name, s))
                }
            }
            _ => {
                let msg = format!("expected type, found {:?}", self.kind());
                self.diags.push(Diag::error(msg, s));
                Err(std::mem::take(&mut self.diags))
            }
        }
    }
}
