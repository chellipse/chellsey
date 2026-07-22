use anyhow::anyhow;

use crate::ast::{Expr, ExprKind, ExtDecl, ExtDeclKind, Stmt, StmtKind, TranslationUnit};
use crate::diagnostic::Error;

type Result<T> = std::result::Result<T, Error>;

/// A minimal type lattice — just enough to confirm a function's `return`s match
/// its declared return type. Everything past this (pointers, integer ranks,
/// `unsigned`, conversions, ...) is TBD until the type system firms up
/// alongside the MLIR work.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Ty {
    Int,
    Void,
}

#[derive(Default)]
pub struct Sema {
    // symbol tables / type environment land here as passes are added
}

impl Sema {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn analyze(&mut self, unit: &TranslationUnit) -> Result<()> {
        for decl in &unit.decls {
            self.ext_decl(decl)?;
        }
        Ok(())
    }

    fn ext_decl(&mut self, decl: &ExtDecl) -> Result<()> {
        match &decl.kind {
            ExtDeclKind::FuncDef { type_spec, body, .. } => {
                // The declaration specifiers give the return type; only the
                // simple `int` / `void` cases are modelled so far.
                let ret = ty_of_spec(type_spec).ok_or_else(|| {
                    decl.span.clone().into_error(anyhow!(
                        "unsupported return type `{}` (TBD)",
                        type_spec.join(" ")
                    ))
                })?;
                self.check_returns(body, ret)
            }
            // TODO: record the symbol in scope, validate its type
            ExtDeclKind::Decl { .. } => Ok(()),
        }
    }

    /// Walk a function body and check every `return` against `expected`.
    fn check_returns(&self, stmt: &Stmt, expected: Ty) -> Result<()> {
        match &stmt.kind {
            StmtKind::Compound(stmts) => {
                for s in stmts {
                    self.check_returns(s, expected)?;
                }
                Ok(())
            }
            StmtKind::Return(expr) => {
                // `return;` yields `void`; otherwise the value's type. Anchor the
                // error at the value when there is one, else at the statement.
                let (actual, span) = match expr {
                    Some(e) => (expr_ty(e), &e.span),
                    None => (Ty::Void, &stmt.span),
                };

                if actual == expected {
                    Ok(())
                } else {
                    Err(span.clone().into_error(anyhow!(
                        "returns `{actual:?}`, but the function returns `{expected:?}`"
                    )))
                }
            }
        }
    }
}

/// The type of an expression. Trivial for now — the only expression is an
/// integer literal.
fn expr_ty(expr: &Expr) -> Ty {
    match &expr.kind {
        ExprKind::IntLit(_) => Ty::Int,
    }
}

/// Map declaration specifiers to a return type. `None` means "not yet modelled".
fn ty_of_spec(spec: &[String]) -> Option<Ty> {
    match spec.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
        ["int"] => Some(Ty::Int),
        ["void"] => Some(Ty::Void),
        _ => None,
    }
}
