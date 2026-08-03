use std::collections::HashMap;

use anyhow::anyhow;

use crate::ast::{
    BinOp, CType, Expr, ExprKind, ExtDecl, ExtDeclKind, Stmt, StmtKind, TranslationUnit, UnOp,
};
use crate::diagnostic::Error;

type Result<T> = std::result::Result<T, Error>;

/// A function's signature — the unit of the frontend→middle-end contract.
#[derive(Debug)]
pub struct FuncSig {
    pub ret: CType,
    pub params: Vec<CType>,
    pub varargs: bool,
    pub defined: bool,
}

/// The symbol tables sema hands to the middle-end (§1: the AST is frozen after
/// this, and lowering is a pure consumer of it).
#[derive(Debug, Default)]
pub struct ProgramInfo {
    pub funcs: HashMap<String, FuncSig>,
    pub globals: HashMap<String, CType>,
}

#[derive(Default)]
pub struct Sema {
    info: ProgramInfo,
    // The block-scope stack of the function being checked, innermost last.
    // Each scope maps a declared name to its type (6.2.1).
    scopes: Vec<HashMap<String, CType>>,
}

impl Sema {
    pub fn new() -> Self {
        Self::default()
    }

    /// Two passes over the (frozen-after-this) translation unit:
    /// * pass 1 collects file-scope signatures into `ProgramInfo`;
    /// * pass 2 walks each body, annotating every `Expr`'s `ty` (§2.7) and
    ///   diagnosing the out-of-subset constructs.
    ///
    /// Pass 2 mutates the AST's `ty` slots, so `unit` is taken by `&mut`.
    pub fn analyze(mut self, unit: &mut TranslationUnit) -> Result<ProgramInfo> {
        for decl in &unit.decls {
            self.collect(decl)?;
        }
        for decl in &mut unit.decls {
            if let ExtDeclKind::FuncDef { ret, body, .. } = &mut decl.kind {
                // `ret` is not touched by the walk, so this split borrow is fine.
                let ret = ret.clone();
                // A fresh function scope; parameters will populate it (FE-14).
                self.scopes.clear();
                self.scopes.push(HashMap::new());
                self.check_stmt(body, &ret)?;
            }
        }
        Ok(self.info)
    }

    // ----- pass 1: signature collection ------------------------------------

    fn collect(&mut self, decl: &ExtDecl) -> Result<()> {
        let (ret, ident, params, varargs, defined) = match &decl.kind {
            ExtDeclKind::FuncDef { ret, ident, params, varargs, .. } => {
                (ret, ident, params, *varargs, true)
            }
            ExtDeclKind::FuncDecl { ret, ident, params, varargs } => {
                (ret, ident, params, *varargs, false)
            }
        };

        // v1 subset: only `int` functions are modelled end-to-end.
        if *ret != CType::INT {
            return Err(decl
                .span
                .clone()
                .into_error(anyhow!("only `int` functions are supported (TBD)")));
        }

        let param_tys: Vec<CType> = params.iter().map(|p| p.ty.clone()).collect();
        let sig = FuncSig { ret: ret.clone(), params: param_tys, varargs, defined };

        match self.info.funcs.get_mut(ident) {
            None => {
                self.info.funcs.insert(ident.clone(), sig);
            }
            Some(prev) => {
                if prev.ret != sig.ret || prev.params != sig.params || prev.varargs != sig.varargs {
                    return Err(decl
                        .span
                        .clone()
                        .into_error(anyhow!("conflicting declaration of `{ident}`")));
                }
                if defined {
                    if prev.defined {
                        return Err(decl
                            .span
                            .clone()
                            .into_error(anyhow!("redefinition of `{ident}`")));
                    }
                    prev.defined = true;
                }
            }
        }
        Ok(())
    }

    // ----- pass 2: type annotation + subset legality -----------------------

    /// Resolve a name against the scope stack, innermost scope first.
    fn lookup(&self, name: &str) -> Option<&CType> {
        self.scopes.iter().rev().find_map(|s| s.get(name))
    }

    fn check_stmt(&mut self, stmt: &mut Stmt, ret: &CType) -> Result<()> {
        match &mut stmt.kind {
            StmtKind::Compound(stmts) => {
                // a block opens a scope; its declarations vanish at the `}`
                self.scopes.push(HashMap::new());
                for s in stmts {
                    self.check_stmt(s, ret)?;
                }
                self.scopes.pop();
                Ok(())
            }
            StmtKind::Decl { ty, name, init } => {
                // v1 subset: only `int` locals are modelled end-to-end.
                if *ty != CType::INT {
                    return Err(stmt
                        .span
                        .clone()
                        .into_error(anyhow!("only `int` locals are supported (TBD)")));
                }
                let scope = self.scopes.last_mut().unwrap();
                if scope.insert(name.clone(), ty.clone()).is_some() {
                    return Err(stmt
                        .span
                        .clone()
                        .into_error(anyhow!("redeclaration of `{name}`")));
                }
                // The name is in scope for its own initializer (6.2.1p7), so
                // `int x = x;` resolves — to the uninitialized `x` — as C says.
                if let Some(init) = init {
                    self.check_expr(init)?;
                }
                Ok(())
            }
            StmtKind::Expr(expr) => self.check_expr(expr),
            StmtKind::If { cond, then, els } => {
                self.check_expr(cond)?;
                self.check_stmt(then, ret)?;
                if let Some(els) = els {
                    self.check_stmt(els, ret)?;
                }
                Ok(())
            }
            StmtKind::While { cond, body } => {
                self.check_expr(cond)?;
                self.check_stmt(body, ret)
            }
            StmtKind::Return(Some(expr)) => self.check_expr(expr),
            StmtKind::Return(None) => {
                // Every modelled function returns `int`, so a bare `return;` has
                // no value to hand back — diagnose rather than miscompile.
                if *ret == CType::Void {
                    Ok(())
                } else {
                    Err(stmt.span.clone().into_error(anyhow!(
                        "`return;` with no value in a function returning `{ret}`"
                    )))
                }
            }
            StmtKind::Empty => Ok(()),
        }
    }

    /// Compute and store an expression's type. Everything in this subset is
    /// `int`; the walk exists so the slot/annotation design is in place (FE-21).
    fn check_expr(&self, expr: &mut Expr) -> Result<()> {
        let span = expr.span.clone();
        let ty = match &mut expr.kind {
            ExprKind::IntLit { ty, .. } => ty.clone(),
            // an identifier's type is its declaration's
            ExprKind::Ident { name } => match self.lookup(name) {
                Some(ty) => ty.clone(),
                None if self.info.funcs.contains_key(name.as_str()) => {
                    return Err(span
                        .into_error(anyhow!("function designators are not yet supported (TBD)")));
                }
                None => {
                    return Err(span.into_error(anyhow!("use of undeclared identifier `{name}`")));
                }
            },
            ExprKind::Assign { lhs, rhs } => {
                // The modifiable-lvalue check (6.5.16): a declared name is the
                // only lvalue in this subset.
                if !matches!(lhs.kind, ExprKind::Ident { .. }) {
                    return Err(span.into_error(anyhow!("left operand of `=` is not assignable")));
                }
                self.check_expr(lhs)?;
                self.check_expr(rhs)?;
                // the assignment's type is the lhs's (conversions are identity
                // in the all-`int` subset)
                lhs.ty.clone().expect("just annotated")
            }
            ExprKind::Unary { op, expr: inner } => {
                self.check_expr(inner)?;
                match op {
                    // `-int`/`~int` promote to `int`; `!` yields a 0/1 `int`
                    UnOp::Neg | UnOp::BitNot | UnOp::Not => CType::INT,
                }
            }
            ExprKind::Binary { op, lhs, rhs } => {
                self.check_expr(lhs)?;
                self.check_expr(rhs)?;
                match op {
                    BinOp::Add
                    | BinOp::Sub
                    | BinOp::Mul
                    | BinOp::Div
                    | BinOp::Rem
                    | BinOp::BitAnd
                    | BinOp::BitOr
                    | BinOp::BitXor
                    | BinOp::Shl
                    | BinOp::Shr => CType::INT,
                    // relational / equality operators yield a 0/1 `int`
                    BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge | BinOp::Eq | BinOp::Ne => {
                        CType::INT
                    }
                    _ => {
                        return Err(
                            span.into_error(anyhow!("this operator is not yet supported (TBD)"))
                        );
                    }
                }
            }
        };
        expr.ty = Some(ty);
        Ok(())
    }
}
