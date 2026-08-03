use anyhow::anyhow;

use super::types::*;
use crate::diagnostic::{Error, Span};
use crate::lexer::{Kw, Punct, Token, TokenKind};

type Result<T> = std::result::Result<T, Error>;

pub struct Parser {
    tokens: Vec<Token>,
    cursor: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, cursor: 0 }
    }

    /// Panics:
    /// * if self.tokens.len() == 0
    pub fn parse(mut self) -> Result<TranslationUnit> {
        let mut decls = Vec::new();
        while self.peek().is_ok() {
            decls.push(self.parse_ext_decl()?);
        }
        let span = self.tokens[0].span.union(&self.tokens.last().unwrap().span);
        Ok(TranslationUnit { decls, span })
    }
}

impl Parser {
    fn peek(&self) -> Result<&Token> {
        self.tokens
            .get(self.cursor)
            .ok_or_else(|| self.error("unexpected end of input"))
    }

    /// One-token lookahead past the cursor, for disambiguation (e.g. a cast
    /// `(type)` vs a parenthesised expression). `None` past end of input.
    fn peek2(&self) -> Option<&Token> {
        self.tokens.get(self.cursor + 1)
    }

    /// Is the current token exactly `kind`? (False at end of input.)
    fn at(&self, kind: TokenKind) -> bool {
        matches!(self.peek(), Ok(tok) if tok.kind == kind)
    }

    /// Consume the current token iff it is `kind`, reporting whether it was.
    fn eat(&mut self, kind: TokenKind) -> bool {
        if self.at(kind) {
            self.consume(1);
            true
        } else {
            false
        }
    }

    /// Panics:
    /// * if self.tokens.len() == 0
    fn peek_or_last(&self) -> &Token {
        self.tokens
            .get(self.cursor)
            .or_else(|| self.tokens.last())
            .unwrap()
    }

    /// A span-carrying diagnostic error anchored at the current token (or the
    /// last one, at EOF). Handed to `main` via the `Result` chain.
    fn error(&self, msg: &str) -> Error {
        self.peek_or_last()
            .span
            .clone()
            .into_error(anyhow!("{msg}"))
    }

    fn consume(&mut self, n: usize) {
        self.cursor += n;
    }

    fn spanned(&self, lo: usize) -> Span {
        self.tokens[lo]
            .span
            .union(&self.tokens[self.cursor - 1].span)
    }

    fn consume_expect(&mut self, kind: TokenKind) -> Result<()> {
        if let Ok(tok) = self.peek()
            && tok.kind == kind
        {
            self.consume(1);
            Ok(())
        } else {
            Err(self.error(&format!("expected {kind:?}")))
        }
    }
}

impl Parser {
    // ----- external declarations (FE-15) -----------------------------------

    /// A function definition (`ret f(params) { ... }`) or a function prototype
    /// (`ret f(params);`). File-scope variables, pointers, arrays, and real
    /// parameters are later items and diagnose as TBD here.
    fn parse_ext_decl(&mut self) -> Result<ExtDecl> {
        let lo = self.cursor;

        let ret = self.parse_type_specifiers()?;
        let ident = self.parse_ident()?;

        // The v1 declarator is just `ident (params)`. Anything not immediately
        // opening a parameter list is a file-scope object declaration.
        if !self.at(TokenKind::Punct(Punct::LParen)) {
            return Err(self.error("file-scope variable declarations are not yet supported (TBD)"));
        }
        let (params, varargs) = self.parse_params()?;

        // '{' opens a body (definition); ';' ends a prototype.
        let kind = if self.at(TokenKind::Punct(Punct::LBrace)) {
            let body = self.parse_comp_stmt()?;
            ExtDeclKind::FuncDef { ret, ident, params, varargs, body }
        } else {
            self.consume_expect(TokenKind::Punct(Punct::SemiColon))?;
            ExtDeclKind::FuncDecl { ret, ident, params, varargs }
        };

        Ok(ExtDecl { kind, span: self.spanned(lo) })
    }

    /// The v1 subset accepts a single `int` or `void` specifier. The full
    /// base-type fold (`long int`, `unsigned`, `struct`, ...) is FE-13, so any
    /// other specifier — or a specifier *run* — is a clean TBD.
    fn parse_type_specifiers(&mut self) -> Result<CType> {
        let ty = match &self.peek()?.kind {
            TokenKind::Kw(Kw::int) => CType::INT,
            TokenKind::Kw(Kw::void) => CType::Void,
            TokenKind::Kw(kw) if is_type_kw(kw) => {
                return Err(self.error("this type specifier is not yet supported (TBD)"));
            }
            _ => return Err(self.error("expected a type specifier")),
        };
        self.consume(1);

        if let Ok(tok) = self.peek()
            && let TokenKind::Kw(kw) = &tok.kind
            && is_type_kw(kw)
        {
            return Err(self.error("compound type specifiers are not yet supported (TBD)"));
        }
        Ok(ty)
    }

    fn parse_ident(&mut self) -> Result<String> {
        let tok = self.peek()?;
        let TokenKind::Ident { value } = &tok.kind else {
            return Err(self.error("expected an identifier"));
        };
        let value = value.clone();
        self.consume(1);
        Ok(value)
    }

    /// A parameter list: `()`, `(void)`, or comma-separated `type [ident]`
    /// declarations (the name is optional in a prototype; sema requires it in
    /// a definition). Varargs `...` and pointer/array declarators are TBD.
    /// Returns `(params, varargs)`.
    fn parse_params(&mut self) -> Result<(Vec<Param>, bool)> {
        self.consume_expect(TokenKind::Punct(Punct::LParen))?;

        // `()` — an unprototyped declarator; treated as no parameters here.
        if self.eat(TokenKind::Punct(Punct::RParen)) {
            return Ok((Vec::new(), false));
        }
        // `(void)` — an explicit empty parameter list.
        if self.at(TokenKind::Kw(Kw::void))
            && matches!(
                self.peek2().map(|t| &t.kind),
                Some(TokenKind::Punct(Punct::RParen))
            )
        {
            self.consume(2); // `void` `)`
            return Ok((Vec::new(), false));
        }

        let mut params = Vec::new();
        loop {
            if self.at(TokenKind::Punct(Punct::Ellipsis)) {
                return Err(self.error("variadic functions are not yet supported (TBD)"));
            }
            let lo = self.cursor;
            let ty = self.parse_type_specifiers()?;
            let name = if matches!(self.peek()?.kind, TokenKind::Ident { .. }) {
                Some(self.parse_ident()?)
            } else {
                None
            };
            params.push(Param { ty, name, span: self.spanned(lo) });
            if !self.eat(TokenKind::Punct(Punct::Comma)) {
                break;
            }
        }
        self.consume_expect(TokenKind::Punct(Punct::RParen))?;
        Ok((params, false))
    }

    // ----- statements (FE-16 / FE-17) --------------------------------------

    /// A block item (6.8.2): a declaration or a statement. Only compound-
    /// statement bodies contain declarations; a branch/loop body is a plain
    /// statement, which is why `parse_stmt` itself rejects them.
    fn parse_block_item(&mut self) -> Result<Stmt> {
        if is_type_start(&self.peek()?.kind) {
            return self.parse_decl();
        }
        self.parse_stmt()
    }

    /// `ty ident [= init] ;` — a single declarator with an optional
    /// initializer. Declarator lists (`int a, b;`), pointer/array declarators,
    /// and local function declarations are FE-16's widening and TBD here.
    fn parse_decl(&mut self) -> Result<Stmt> {
        let lo = self.cursor;
        let ty = self.parse_type_specifiers()?;
        let name = self.parse_ident()?;
        // an initializer is an assignment-expression (6.7.10) — `,` stays out
        let init = if self.eat(TokenKind::Punct(Punct::Eq)) {
            Some(self.parse_assign()?)
        } else {
            None
        };
        if self.at(TokenKind::Punct(Punct::Comma)) {
            return Err(self.error("declarator lists are not yet supported (TBD)"));
        }
        self.consume_expect(TokenKind::Punct(Punct::SemiColon))?;
        Ok(Stmt {
            kind: StmtKind::Decl { ty, name, init },
            span: self.spanned(lo),
        })
    }

    fn parse_stmt(&mut self) -> Result<Stmt> {
        let lo = self.cursor;
        let kind = match &self.peek()?.kind {
            TokenKind::Kw(Kw::_return) => {
                self.consume(1);
                if self.eat(TokenKind::Punct(Punct::SemiColon)) {
                    StmtKind::Return(None)
                } else {
                    let expr = self.parse_expr()?;
                    self.consume_expect(TokenKind::Punct(Punct::SemiColon))?;
                    StmtKind::Return(Some(expr))
                }
            }
            TokenKind::Punct(Punct::SemiColon) => {
                self.consume(1);
                StmtKind::Empty
            }
            // a nested block builds its own spanned `Stmt`
            TokenKind::Punct(Punct::LBrace) => return self.parse_comp_stmt(),
            TokenKind::Kw(Kw::_if) => return self.parse_if(),
            TokenKind::Kw(Kw::_while) => return self.parse_while(),
            TokenKind::Kw(Kw::_for) => return self.parse_for(),
            TokenKind::Kw(Kw::_do) => {
                return Err(self.error("`do`/`while` loops are not yet supported (TBD)"));
            }
            TokenKind::Kw(Kw::_break) => {
                self.consume(1);
                self.consume_expect(TokenKind::Punct(Punct::SemiColon))?;
                StmtKind::Break
            }
            TokenKind::Kw(Kw::_continue) => {
                self.consume(1);
                self.consume_expect(TokenKind::Punct(Punct::SemiColon))?;
                StmtKind::Continue
            }
            // a declaration is a block item, not a statement (6.8.2), so it
            // cannot be the branch of an `if` — C requires the braces.
            k if is_type_start(k) => {
                return Err(self.error("a declaration is not a statement; wrap it in `{ }`"));
            }
            // control flow, labels, ... are later items; everything else is an
            // expression statement: `expr ;`
            _ => {
                let expr = self.parse_expr()?;
                self.consume_expect(TokenKind::Punct(Punct::SemiColon))?;
                StmtKind::Expr(expr)
            }
        };

        Ok(Stmt { kind, span: self.spanned(lo) })
    }

    fn parse_comp_stmt(&mut self) -> Result<Stmt> {
        let lo = self.cursor;
        self.consume_expect(TokenKind::Punct(Punct::LBrace))?;
        let mut stmts = Vec::new();
        while let Ok(tok) = self.peek()
            && tok.kind != TokenKind::Punct(Punct::RBrace)
        {
            stmts.push(self.parse_block_item()?);
        }
        self.consume_expect(TokenKind::Punct(Punct::RBrace))?;
        Ok(Stmt { kind: StmtKind::Compound(stmts), span: self.spanned(lo) })
    }

    /// `if ( expr ) stmt [ else stmt ]`. The `else` binds to the nearest `if`,
    /// which falls out of parsing the branch as a single statement.
    fn parse_if(&mut self) -> Result<Stmt> {
        let lo = self.cursor;
        self.consume(1); // `if`
        self.consume_expect(TokenKind::Punct(Punct::LParen))?;
        let cond = self.parse_expr()?;
        self.consume_expect(TokenKind::Punct(Punct::RParen))?;
        let then = Box::new(self.parse_stmt()?);
        let els = if self.eat(TokenKind::Kw(Kw::_else)) {
            Some(Box::new(self.parse_stmt()?))
        } else {
            None
        };
        Ok(Stmt { kind: StmtKind::If { cond, then, els }, span: self.spanned(lo) })
    }

    /// `for ( init ; cond ; step ) stmt` — each clause may be empty; `init`
    /// may also be a declaration (6.8.5.3), whose scope is the whole loop.
    fn parse_for(&mut self) -> Result<Stmt> {
        let lo = self.cursor;
        self.consume(1); // `for`
        self.consume_expect(TokenKind::Punct(Punct::LParen))?;

        let init = if self.eat(TokenKind::Punct(Punct::SemiColon)) {
            None
        } else if is_type_start(&self.peek()?.kind) {
            // a declaration consumes its own `;`
            Some(Box::new(self.parse_decl()?))
        } else {
            let expr = self.parse_expr()?;
            self.consume_expect(TokenKind::Punct(Punct::SemiColon))?;
            let span = expr.span.clone();
            Some(Box::new(Stmt { kind: StmtKind::Expr(expr), span }))
        };

        let cond = if self.at(TokenKind::Punct(Punct::SemiColon)) {
            None
        } else {
            Some(self.parse_expr()?)
        };
        self.consume_expect(TokenKind::Punct(Punct::SemiColon))?;

        let step = if self.at(TokenKind::Punct(Punct::RParen)) {
            None
        } else {
            Some(self.parse_expr()?)
        };
        self.consume_expect(TokenKind::Punct(Punct::RParen))?;

        let body = Box::new(self.parse_stmt()?);
        Ok(Stmt {
            kind: StmtKind::For { init, cond, step, body },
            span: self.spanned(lo),
        })
    }

    /// `while ( expr ) stmt`
    fn parse_while(&mut self) -> Result<Stmt> {
        let lo = self.cursor;
        self.consume(1); // `while`
        self.consume_expect(TokenKind::Punct(Punct::LParen))?;
        let cond = self.parse_expr()?;
        self.consume_expect(TokenKind::Punct(Punct::RParen))?;
        let body = Box::new(self.parse_stmt()?);
        Ok(Stmt { kind: StmtKind::While { cond, body }, span: self.spanned(lo) })
    }

    // ----- expression tower (FE-19) ----------------------------------------
    //
    // Each level calls the next-tighter one; the wired subset is integer
    // literals, parentheses, unary minus, and `+ - * / %`. Every other level is
    // a passthrough or a TBD arm, so later batches widen without reshaping.

    /// `parse_expr = comma` — the comma operator is TBD.
    fn parse_expr(&mut self) -> Result<Expr> {
        let expr = self.parse_assign()?;
        if self.at(TokenKind::Punct(Punct::Comma)) {
            return Err(self.error("the comma operator is not yet supported (TBD)"));
        }
        Ok(expr)
    }

    /// `parse_assign` — simple assignment, right-associative (`x = y = 5`).
    /// The lvalue check on the left operand is sema's (6.5.16); compound
    /// assignment (`+=` ...) is a distinct surface node later and TBD here.
    fn parse_assign(&mut self) -> Result<Expr> {
        let lhs = self.parse_cond()?;
        if self.eat(TokenKind::Punct(Punct::Eq)) {
            let rhs = self.parse_assign()?;
            let span = lhs.span.union(&rhs.span);
            let kind = ExprKind::Assign { lhs: Box::new(lhs), rhs: Box::new(rhs) };
            return Ok(Expr { kind, ty: None, span });
        }
        if let Ok(tok) = self.peek()
            && is_assign_op(&tok.kind)
        {
            return Err(self.error("compound assignment is not yet supported (TBD)"));
        }
        Ok(lhs)
    }

    /// `parse_cond` — the conditional operator (6.5.15): the middle is a full
    /// expression, the third arm recurses right-associatively (`a?b:c?d:e` is
    /// `a?b:(c?d:e)`).
    fn parse_cond(&mut self) -> Result<Expr> {
        let cond = self.parse_binary(0)?;
        if self.eat(TokenKind::Punct(Punct::Question)) {
            let then = self.parse_expr()?;
            self.consume_expect(TokenKind::Punct(Punct::Colon))?;
            let els = self.parse_cond()?;
            let span = cond.span.union(&els.span);
            let kind =
                ExprKind::Cond { cond: Box::new(cond), then: Box::new(then), els: Box::new(els) };
            return Ok(Expr { kind, ty: None, span });
        }
        Ok(cond)
    }

    /// `parse_binary` — precedence climbing. All C binary operators parse into
    /// `Binary` nodes with their proper precedence; the unwired ones
    /// (comparisons, shifts, bitwise, logical) are rejected as TBD in sema.
    fn parse_binary(&mut self, min_bp: u8) -> Result<Expr> {
        let mut lhs = self.parse_cast_expr()?;
        while let Some((op, bp)) = self.peek().ok().and_then(|tok| bin_op(&tok.kind)) {
            if bp < min_bp {
                break;
            }
            self.consume(1);
            // left-associative: the right operand binds strictly tighter.
            let rhs = self.parse_binary(bp + 1)?;
            let span = lhs.span.union(&rhs.span);
            let kind = ExprKind::Binary { op, lhs: Box::new(lhs), rhs: Box::new(rhs) };
            lhs = Expr { kind, ty: None, span };
        }
        Ok(lhs)
    }

    /// `parse_cast_expr` — `(type) expr` casts are TBD; disambiguated from a
    /// parenthesised expression by peeking past the `(` for a type specifier.
    fn parse_cast_expr(&mut self) -> Result<Expr> {
        if self.at(TokenKind::Punct(Punct::LParen))
            && self.peek2().is_some_and(|tok| is_type_start(&tok.kind))
        {
            return Err(self.error("cast expressions are not yet supported (TBD)"));
        }
        self.parse_unary()
    }

    /// `parse_unary` — prefix `- ! ~` fold into `Unary`; unary `+` is identity;
    /// `* & ++ --` and `sizeof` are later items and TBD here.
    fn parse_unary(&mut self) -> Result<Expr> {
        let lo = self.cursor;
        let op = match self.peek()?.kind.clone() {
            TokenKind::Punct(Punct::Minus) => UnOp::Neg,
            TokenKind::Punct(Punct::Bang) => UnOp::Not,
            TokenKind::Punct(Punct::Tilde) => UnOp::BitNot,
            TokenKind::Punct(Punct::Plus) => {
                // unary plus is the identity on an arithmetic operand
                self.consume(1);
                return self.parse_cast_expr();
            }
            TokenKind::Punct(Punct::Star)
            | TokenKind::Punct(Punct::Amp)
            | TokenKind::Punct(Punct::PlusPlus)
            | TokenKind::Punct(Punct::MinusMinus) => {
                return Err(self.error("this prefix operator is not yet supported (TBD)"));
            }
            TokenKind::Kw(Kw::sizeof) => {
                return Err(self.error("`sizeof` is not yet supported (TBD)"));
            }
            _ => return self.parse_postfix(),
        };
        self.consume(1);
        let expr = self.parse_cast_expr()?;
        let kind = ExprKind::Unary { op, expr: Box::new(expr) };
        Ok(Expr { kind, ty: None, span: self.spanned(lo) })
    }

    /// `parse_postfix` — a direct function call `f(args)`; subscripts and
    /// postfix `++ --` are later items and TBD here.
    fn parse_postfix(&mut self) -> Result<Expr> {
        let lo = self.cursor;
        let expr = self.parse_primary()?;
        match self.peek().ok().map(|t| t.kind.clone()) {
            Some(TokenKind::Punct(Punct::LParen)) => {
                // The callee is a bare name in this subset — calling through an
                // arbitrary expression (a function pointer) is a later item.
                let ExprKind::Ident { name } = expr.kind else {
                    return Err(self.error("only named functions can be called (TBD)"));
                };
                let args = self.parse_args()?;
                let kind = ExprKind::Call { callee: name, args };
                Ok(Expr { kind, ty: None, span: self.spanned(lo) })
            }
            Some(TokenKind::Punct(Punct::LBracket)) => {
                Err(self.error("array subscripting is not yet supported (TBD)"))
            }
            Some(TokenKind::Punct(Punct::PlusPlus | Punct::MinusMinus)) => {
                Err(self.error("postfix `++`/`--` are not yet supported (TBD)"))
            }
            _ => Ok(expr),
        }
    }

    /// A call's parenthesised argument list. Each argument is an
    /// assignment-expression, so the commas separate arguments rather than
    /// invoking the comma operator (6.5.2.2).
    fn parse_args(&mut self) -> Result<Vec<Expr>> {
        self.consume_expect(TokenKind::Punct(Punct::LParen))?;
        let mut args = Vec::new();
        if self.eat(TokenKind::Punct(Punct::RParen)) {
            return Ok(args);
        }
        loop {
            args.push(self.parse_assign()?);
            if !self.eat(TokenKind::Punct(Punct::Comma)) {
                break;
            }
        }
        self.consume_expect(TokenKind::Punct(Punct::RParen))?;
        Ok(args)
    }

    /// `parse_primary` — integer literals, identifiers, and parenthesised
    /// expressions. Every other literal kind is a later item and TBD here.
    fn parse_primary(&mut self) -> Result<Expr> {
        let lo = self.cursor;
        match self.peek()?.kind.clone() {
            TokenKind::IntConst { value, suf } => {
                self.consume(1);
                Ok(Expr { kind: int_lit(value, suf), ty: None, span: self.spanned(lo) })
            }
            // parentheses don't create a node — the inner expression's own span
            // stands in for the group.
            TokenKind::Punct(Punct::LParen) => {
                self.consume(1);
                let inner = self.parse_expr()?;
                self.consume_expect(TokenKind::Punct(Punct::RParen))?;
                Ok(inner)
            }
            TokenKind::Ident { value } => {
                self.consume(1);
                Ok(Expr {
                    kind: ExprKind::Ident { name: value },
                    ty: None,
                    span: self.spanned(lo),
                })
            }
            TokenKind::FloatConst { .. } => {
                Err(self.error("floating-point literals are not yet supported (TBD)"))
            }
            TokenKind::CharConst { .. } => {
                Err(self.error("character constants are not yet supported (TBD)"))
            }
            TokenKind::StrLit { .. } => {
                Err(self.error("string literals are not yet supported (TBD)"))
            }
            TokenKind::Kw(Kw::_true | Kw::_false) => {
                Err(self.error("`true`/`false` are not yet supported (TBD)"))
            }
            TokenKind::Kw(Kw::nullptr) => Err(self.error("`nullptr` is not yet supported (TBD)")),
            _ => Err(self.error("expected an expression")),
        }
    }
}

/// The binary operator and its left-binding power (FE-19, lowest→highest all
/// left-associative), or `None` if the token is not a binary operator.
fn bin_op(kind: &TokenKind) -> Option<(BinOp, u8)> {
    let TokenKind::Punct(p) = kind else {
        return None;
    };
    let pair = match p {
        Punct::PipePipe => (BinOp::LogOr, 1),
        Punct::AmpAmp => (BinOp::LogAnd, 2),
        Punct::Pipe => (BinOp::BitOr, 3),
        Punct::Caret => (BinOp::BitXor, 4),
        Punct::Amp => (BinOp::BitAnd, 5),
        Punct::EqEq => (BinOp::Eq, 6),
        Punct::BangEq => (BinOp::Ne, 6),
        Punct::Lt => (BinOp::Lt, 7),
        Punct::Gt => (BinOp::Gt, 7),
        Punct::LtEq => (BinOp::Le, 7),
        Punct::GtEq => (BinOp::Ge, 7),
        Punct::LtLt => (BinOp::Shl, 8),
        Punct::GtGt => (BinOp::Shr, 8),
        Punct::Plus => (BinOp::Add, 9),
        Punct::Minus => (BinOp::Sub, 9),
        Punct::Star => (BinOp::Mul, 10),
        Punct::Slash => (BinOp::Div, 10),
        Punct::Percent => (BinOp::Rem, 10),
        _ => return None,
    };
    Some(pair)
}

/// Whether a punctuator is a (compound-)assignment operator.
fn is_assign_op(kind: &TokenKind) -> bool {
    matches!(
        kind,
        TokenKind::Punct(
            Punct::Eq
                | Punct::PlusEq
                | Punct::MinusEq
                | Punct::StarEq
                | Punct::SlashEq
                | Punct::PercentEq
                | Punct::LtLtEq
                | Punct::GtGtEq
                | Punct::AmpEq
                | Punct::CaretEq
                | Punct::PipeEq
        )
    )
}

/// A keyword that begins a type specifier (used to detect declarations and to
/// disambiguate casts). The FE-13 fold will consume these; here they only gate
/// diagnostics.
fn is_type_kw(kw: &Kw) -> bool {
    matches!(
        kw,
        Kw::void
            | Kw::bool
            | Kw::char
            | Kw::short
            | Kw::int
            | Kw::long
            | Kw::signed
            | Kw::unsigned
            | Kw::float
            | Kw::double
            | Kw::_struct
            | Kw::union
            | Kw::_enum
            | Kw::_BitInt
            | Kw::_Complex
    )
}

fn is_type_start(kind: &TokenKind) -> bool {
    matches!(kind, TokenKind::Kw(kw) if is_type_kw(kw))
}
