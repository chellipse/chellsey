use std::{
    collections::HashSet,
    hash::{Hash, Hasher},
};

use anyhow::anyhow;

use super::types::*;
use crate::diagnostic::Error;
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

    pub fn parse(mut self) -> Result<TranslationUnit> {
        let mut decls = Vec::new();
        while self.peek().is_ok() {
            decls.push(self.parse_ext_decl()?);
        }
        Ok(TranslationUnit { decls })
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
        // shared prefix: both a function-definition and a declaration start here
        let type_spec = self.parse_decl_specifiers()?;
        let ident = self.parse_declarator()?;

        // the one token that decides the branch: '{' (a body) => FuncDef, else Decl
        if let Ok(tok) = self.peek()
            && tok.kind == TokenKind::Punct(Punct::LBrace)
        {
            let body = self.parse_comp_stmt()?;
            Ok(ExtDecl::FuncDef { type_spec, ident, body })
        } else {
            // minimal declaration: no initializers or multi-declarator lists yet
            self.consume_expect(TokenKind::Punct(Punct::SemiColon))?;
            Ok(ExtDecl::Decl { type_spec, ident })
        }
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
        let tok = self.peek()?;
        let result = match &tok.kind {
            TokenKind::Kw(Kw::_return) => {
                self.consume(1);
                let expr = self.parse_expr().ok();
                self.consume_expect(TokenKind::Punct(Punct::SemiColon))?;
                Some(Stmt::Return(expr))
            }
            TokenKind::Punct(Punct::LBrace) => Some(self.parse_comp_stmt()?),
            _ => None,
        };

        result.ok_or_else(|| self.error("expected a statement"))
    }

    fn parse_comp_stmt(&mut self) -> Result<Stmt> {
        self.consume_expect(TokenKind::Punct(Punct::LBrace))?;
        let mut stmts = Vec::new();
        while let Ok(tok) = self.peek()
            && tok.kind != TokenKind::Punct(Punct::RBrace)
        {
            stmts.push(self.parse_stmt()?);
        }
        self.consume_expect(TokenKind::Punct(Punct::RBrace))?;
        Ok(Stmt::Compound(stmts))
    }

    fn parse_expr(&mut self) -> Result<Expr> {
        let tok = self.peek()?;
        let result = match &tok.kind {
            TokenKind::IntConst { value, .. } => Some(Expr::IntLit(*value)),
            _ => None,
        };

        result
            .ok_or_else(|| self.error("expected an expression"))
            .inspect(|_| self.cursor += 1)
    }
}
