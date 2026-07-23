use std::{
    collections::HashSet,
    hash::{Hash, Hasher},
};

use anyhow::anyhow;

use super::types::*;
use crate::diagnostic::{Error, Span};
use crate::lexer::{Kw, Punct, Token, TokenKind};

type Result<T> = std::result::Result<T, Error>;

struct ScopeStack(Vec<HashSet<String>>);

impl ScopeStack {
    fn new() -> Self {
        Self(vec![Default::default()])
    }

    fn in_scope(&self, ident: &String) -> bool {
        self.0.iter().any(|set| set.contains(ident))
    }

    fn insert(&mut self, ident: String) {
        let i = self.0.len();
        self.0[i].insert(ident);
    }

    fn enter(&mut self) {
        self.0.push(Default::default());
    }

    fn exit(&mut self) {
        self.0.pop();
    }
}

pub struct Parser {
    tokens: Vec<Token>,
    cursor: usize,
    scopes: ScopeStack,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, cursor: 0, scopes: ScopeStack::new() }
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
    fn parse_ext_decl(&mut self) -> Result<ExtDecl> {
        let lo = self.cursor;

        // shared prefix: both a function-definition and a declaration start here
        let type_spec = self.parse_decl_specifiers()?;
        let ident = self.parse_declarator()?;

        // the one token that decides the branch: '{' (a body) => FuncDef, else Decl
        let kind = if let Ok(tok) = self.peek()
            && tok.kind == TokenKind::Punct(Punct::LBrace)
        {
            let body = self.parse_comp_stmt()?;
            ExtDeclKind::FuncDef { type_spec, ident, body }
        } else {
            // minimal declaration: no initializers or multi-declarator lists yet
            self.consume_expect(TokenKind::Punct(Punct::SemiColon))?;
            ExtDeclKind::Decl { type_spec, ident }
        };

        Ok(ExtDecl { kind, span: self.spanned(lo) })
    }

    fn parse_decl_specifiers(&mut self) -> Result<Vec<String>> {
        let mut specs = Vec::new();
        while let Ok(tok) = self.peek() {
            let name = match &tok.kind {
                TokenKind::Kw(Kw::void) => "void",
                TokenKind::Kw(Kw::char) => "char",
                TokenKind::Kw(Kw::short) => "short",
                TokenKind::Kw(Kw::int) => "int",
                TokenKind::Kw(Kw::long) => "long",
                TokenKind::Kw(Kw::signed) => "signed",
                TokenKind::Kw(Kw::unsigned) => "unsigned",
                TokenKind::Kw(Kw::float) => "float",
                TokenKind::Kw(Kw::double) => "double",
                TokenKind::Kw(Kw::bool) => "bool",
                _ => break,
            };
            specs.push(name.to_string());
            self.consume(1);
        }

        if specs.is_empty() {
            return Err(self.error("expected a type specifier"));
        }
        Ok(specs)
    }

    fn parse_declarator(&mut self) -> Result<String> {
        let tok = self.peek()?;
        let ident = match &tok.kind {
            TokenKind::Ident { value } => value.clone(),
            _ => return Err(self.error("expected an identifier")),
        };
        self.consume(1);

        // minimal function declarator: swallow an empty '(' ')' if present.
        // real declarators recurse and carry params / pointers / arrays.
        if let Ok(tok) = self.peek()
            && tok.kind == TokenKind::Punct(Punct::LParen)
        {
            self.consume(1);
            self.consume_expect(TokenKind::Punct(Punct::RParen))?;
        }
        Ok(ident)
    }

    fn parse_stmt(&mut self) -> Result<Stmt> {
        let lo = self.cursor;
        let tok = self.peek()?;
        let kind = match &tok.kind {
            TokenKind::Kw(Kw::_return) => {
                self.consume(1);
                let expr = self.parse_expr().ok();
                self.consume_expect(TokenKind::Punct(Punct::SemiColon))?;
                StmtKind::Return(expr)
            }
            // a nested block builds its own spanned `Stmt`
            TokenKind::Punct(Punct::LBrace) => return self.parse_comp_stmt(),
            _ => return Err(self.error("expected a statement")),
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
            stmts.push(self.parse_stmt()?);
        }
        self.consume_expect(TokenKind::Punct(Punct::RBrace))?;
        Ok(Stmt { kind: StmtKind::Compound(stmts), span: self.spanned(lo) })
    }

    /// Additive level: parse one primary, then fold `+` left-associatively.
    /// Lookahead is via `peek` (not `consume`), so a non-operator such as `;`
    /// is left in place for the caller — the previous version consumed it.
    fn parse_expr(&mut self) -> Result<Expr> {
        let mut lhs = self.parse_primary()?;

        while let Ok(tok) = self.peek()
            && tok.kind == TokenKind::Punct(Punct::Plus)
        {
            self.consume(1); // commit to the operator now that we've seen it
            let rhs = self.parse_primary()?;
            let span = lhs.span.union(&rhs.span);
            lhs = Expr { kind: ExprKind::Add(Box::new(lhs), Box::new(rhs)), span };
        }

        Ok(lhs)
    }

    /// A primary expression — the atoms operators combine. Integer literals for
    /// now; parens/calls/identifiers slot in here later. When a higher-
    /// precedence level (e.g. `*`) arrives, `parse_expr`'s operand becomes a
    /// call to *that* level, and that level's operand is `parse_primary`.
    fn parse_primary(&mut self) -> Result<Expr> {
        let lo = self.cursor;
        let tok = self.peek()?;
        let kind = match &tok.kind {
            TokenKind::IntConst { value, .. } => ExprKind::IntLit(*value),
            _ => return Err(self.error("expected an expression")),
        };
        self.consume(1);
        Ok(Expr { kind, span: self.spanned(lo) })
    }
}
