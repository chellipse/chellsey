use std::collections::{HashMap, HashSet};

use anyhow::anyhow;

use crate::ast::{
    BinOp, CType, Expr, ExprKind, ExtDecl, ExtDeclKind, Param, Stmt, StmtKind, TranslationUnit,
    UnOp,
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
    // How many loop bodies enclose the statement being checked; gates
    // `continue` and (with `switches`) `break` (6.8.6.2/3).
    loop_depth: u32,
    // The enclosing `switch` statements, innermost last; gates `case`/`default`
    // and detects duplicate cases and multiple defaults (6.8.4.2).
    switches: Vec<SwitchAcc>,
    // Every label defined in the current function (function scope, 6.2.1p3),
    // gathered before the body walk so a forward `goto` resolves.
    labels: HashSet<String>,
}

// Per-switch accumulator: the case values seen so far (for duplicate
// detection) and whether a `default` label has appeared.
#[derive(Default)]
struct SwitchAcc {
    values: HashSet<i64>,
    has_default: bool,
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
            if let ExtDeclKind::FuncDef { ret, params, body, .. } = &mut decl.kind {
                // `ret` is not touched by the walk, so this split borrow is fine.
                let ret = ret.clone();
                // A fresh function scope, populated by the parameters (6.2.1:
                // they share the body's outermost scope).
                self.scopes.clear();
                self.scopes.push(HashMap::new());
                self.declare_params(params)?;
                self.loop_depth = 0;
                self.labels.clear();
                self.collect_labels(body)?;
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
        for p in params {
            if p.ty != CType::INT {
                return Err(p
                    .span
                    .clone()
                    .into_error(anyhow!("only `int` parameters are supported (TBD)")));
            }
        }
        // v1 passes every argument in a register (SysV: rdi..r9).
        if params.len() > 6 {
            return Err(decl.span.clone().into_error(anyhow!(
                "more than 6 parameters are not yet supported (TBD)"
            )));
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

    /// Enter a definition's parameters into the function scope. A definition
    /// (unlike a prototype) must name every parameter (6.9.1p5).
    fn declare_params(&mut self, params: &[Param]) -> Result<()> {
        for p in params {
            let Some(name) = &p.name else {
                return Err(p
                    .span
                    .clone()
                    .into_error(anyhow!("parameter name omitted in a function definition")));
            };
            let scope = self.scopes.last_mut().unwrap();
            if scope.insert(name.clone(), p.ty.clone()).is_some() {
                return Err(p
                    .span
                    .clone()
                    .into_error(anyhow!("redeclaration of parameter `{name}`")));
            }
        }
        Ok(())
    }

    /// Resolve a name against the scope stack, innermost scope first.
    fn lookup(&self, name: &str) -> Option<&CType> {
        self.scopes.iter().rev().find_map(|s| s.get(name))
    }

    /// Gather every label in a function body (labels have function scope, not
    /// block scope — 6.2.1p3), erroring on a redefinition. Run before the body
    /// walk so a `goto` to a not-yet-seen label resolves.
    fn collect_labels(&mut self, stmt: &Stmt) -> Result<()> {
        match &stmt.kind {
            StmtKind::Label { name, body } => {
                if !self.labels.insert(name.clone()) {
                    return Err(stmt
                        .span
                        .clone()
                        .into_error(anyhow!("redefinition of label `{name}`")));
                }
                self.collect_labels(body)
            }
            StmtKind::Compound(stmts) => {
                for s in stmts {
                    self.collect_labels(s)?;
                }
                Ok(())
            }
            StmtKind::If { then, els, .. } => {
                self.collect_labels(then)?;
                if let Some(els) = els {
                    self.collect_labels(els)?;
                }
                Ok(())
            }
            StmtKind::While { body, .. }
            | StmtKind::DoWhile { body, .. }
            | StmtKind::Switch { body, .. }
            | StmtKind::Case { body, .. }
            | StmtKind::Default { body } => self.collect_labels(body),
            StmtKind::For { init, body, .. } => {
                if let Some(init) = init {
                    self.collect_labels(init)?;
                }
                self.collect_labels(body)
            }
            _ => Ok(()),
        }
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
                self.loop_depth += 1;
                self.check_stmt(body, ret)?;
                self.loop_depth -= 1;
                Ok(())
            }
            StmtKind::DoWhile { body, cond } => {
                // Body first (it always runs once), then the controlling
                // expression, which lives in the enclosing scope.
                self.loop_depth += 1;
                self.check_stmt(body, ret)?;
                self.loop_depth -= 1;
                self.check_expr(cond)?;
                Ok(())
            }
            StmtKind::For { init, cond, step, body } => {
                // The for clause opens a scope enclosing cond, step, and body
                // (6.8.5.3); the body's own block still nests inside it.
                self.scopes.push(HashMap::new());
                if let Some(init) = init {
                    self.check_stmt(init, ret)?;
                }
                if let Some(cond) = cond {
                    self.check_expr(cond)?;
                }
                if let Some(step) = step {
                    self.check_expr(step)?;
                }
                self.loop_depth += 1;
                self.check_stmt(body, ret)?;
                self.loop_depth -= 1;
                self.scopes.pop();
                Ok(())
            }
            StmtKind::Switch { disc, body } => {
                // The controlling expression is an integer (int in this subset);
                // `case`/`default` inside register against this switch.
                self.check_expr(disc)?;
                self.switches.push(SwitchAcc::default());
                self.check_stmt(body, ret)?;
                self.switches.pop();
                Ok(())
            }
            StmtKind::Case { value, body } => {
                if self.switches.is_empty() {
                    return Err(stmt
                        .span
                        .clone()
                        .into_error(anyhow!("`case` label not within a switch")));
                }
                // The label must be an integer constant expression (6.8.4.2).
                let Some(v) = const_eval_int(value) else {
                    return Err(value.span.clone().into_error(anyhow!(
                        "case label is not an integer constant expression"
                    )));
                };
                self.check_expr(value)?;
                if !self.switches.last_mut().unwrap().values.insert(v) {
                    return Err(value
                        .span
                        .clone()
                        .into_error(anyhow!("duplicate case value `{v}`")));
                }
                self.check_stmt(body, ret)
            }
            StmtKind::Default { body } => {
                let Some(acc) = self.switches.last_mut() else {
                    return Err(stmt
                        .span
                        .clone()
                        .into_error(anyhow!("`default` label not within a switch")));
                };
                if acc.has_default {
                    return Err(stmt
                        .span
                        .clone()
                        .into_error(anyhow!("multiple `default` labels in one switch")));
                }
                acc.has_default = true;
                self.check_stmt(body, ret)
            }
            StmtKind::Break => {
                if self.loop_depth == 0 && self.switches.is_empty() {
                    return Err(stmt
                        .span
                        .clone()
                        .into_error(anyhow!("`break` is not inside a loop or switch")));
                }
                Ok(())
            }
            StmtKind::Continue => {
                if self.loop_depth == 0 {
                    return Err(stmt
                        .span
                        .clone()
                        .into_error(anyhow!("`continue` is not inside a loop")));
                }
                Ok(())
            }
            StmtKind::Goto { label } => {
                if !self.labels.contains(label) {
                    return Err(stmt
                        .span
                        .clone()
                        .into_error(anyhow!("use of undeclared label `{label}`")));
                }
                Ok(())
            }
            StmtKind::Label { body, .. } => {
                // The label name was gathered in the pre-pass; check its statement.
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
            ExprKind::Cond { cond, then, els } => {
                self.check_expr(cond)?;
                self.check_expr(then)?;
                self.check_expr(els)?;
                // Both arms are `int` in this subset, so the result is too;
                // the full arithmetic-conversion merge of the arms is TBD
                // alongside the wider types.
                CType::INT
            }
            ExprKind::Call { callee, args } => {
                // A name in scope is an object, not a function — it can't be
                // called (function pointers are a later item).
                if self.lookup(callee).is_some() {
                    return Err(span.into_error(anyhow!("called object `{callee}` is not a function")));
                }
                let Some(sig) = self.info.funcs.get(callee.as_str()) else {
                    return Err(span.into_error(anyhow!("call to undeclared function `{callee}`")));
                };
                // Arity: exact for a prototyped function; a variadic tail (once
                // it exists) only relaxes the upper bound.
                let (nparams, varargs, ret) = (sig.params.len(), sig.varargs, sig.ret.clone());
                if args.len() < nparams || (!varargs && args.len() > nparams) {
                    return Err(span.into_error(anyhow!(
                        "`{callee}` takes {nparams} argument(s), but {} given",
                        args.len()
                    )));
                }
                for arg in args {
                    self.check_expr(arg)?;
                }
                ret
            }
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
            ExprKind::CompoundAssign { lhs, rhs, .. } => {
                // The same modifiable-lvalue rule as `=` (6.5.16.2); every
                // compound operator is valid on the all-`int` operands here, so
                // there is nothing further to constrain per operator.
                if !matches!(lhs.kind, ExprKind::Ident { .. }) {
                    return Err(span
                        .into_error(anyhow!("left operand of compound assignment is not assignable")));
                }
                self.check_expr(lhs)?;
                self.check_expr(rhs)?;
                lhs.ty.clone().expect("just annotated")
            }
            ExprKind::IncDec { expr: inner, .. } => {
                // `++`/`--` need a modifiable lvalue (6.5.2.4/6.5.3.1); an ident
                // is the only one in this subset. The result is the operand's
                // type, whether prefix (new value) or postfix (old).
                if !matches!(inner.kind, ExprKind::Ident { .. }) {
                    return Err(span
                        .into_error(anyhow!("operand of `++`/`--` is not a modifiable lvalue")));
                }
                self.check_expr(inner)?;
                inner.ty.clone().expect("just annotated")
            }
            ExprKind::Comma { lhs, rhs } => {
                // `lhs` is evaluated and discarded; the result is `rhs` (6.5.17).
                self.check_expr(lhs)?;
                self.check_expr(rhs)?;
                rhs.ty.clone().expect("just annotated")
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
                    // short-circuit operators also yield a 0/1 `int` (6.5.13/14)
                    BinOp::LogAnd | BinOp::LogOr => CType::INT,
                }
            }
        };
        expr.ty = Some(ty);
        Ok(())
    }
}

/// Fold an integer constant expression (6.6) to its value, or `None` if it is
/// not a constant this subset evaluates. Used for `case` labels; the seed of
/// the HIR const-eval pass (HIR-CONST-1). Division/shift/etc. use wrapping i64
/// arithmetic, which agrees with `int` for the small values case labels hold.
pub(crate) fn const_eval_int(e: &Expr) -> Option<i64> {
    match &e.kind {
        ExprKind::IntLit { value, .. } => Some(*value as i64),
        ExprKind::Unary { op, expr } => {
            let v = const_eval_int(expr)?;
            Some(match op {
                UnOp::Neg => v.wrapping_neg(),
                UnOp::BitNot => !v,
                UnOp::Not => (v == 0) as i64,
            })
        }
        ExprKind::Binary { op, lhs, rhs } => {
            let a = const_eval_int(lhs)?;
            let b = const_eval_int(rhs)?;
            Some(match op {
                BinOp::Add => a.wrapping_add(b),
                BinOp::Sub => a.wrapping_sub(b),
                BinOp::Mul => a.wrapping_mul(b),
                BinOp::Div => (b != 0).then(|| a.wrapping_div(b))?,
                BinOp::Rem => (b != 0).then(|| a.wrapping_rem(b))?,
                BinOp::BitAnd => a & b,
                BinOp::BitOr => a | b,
                BinOp::BitXor => a ^ b,
                BinOp::Shl => a.wrapping_shl(b as u32),
                BinOp::Shr => a.wrapping_shr(b as u32),
                BinOp::Lt => (a < b) as i64,
                BinOp::Gt => (a > b) as i64,
                BinOp::Le => (a <= b) as i64,
                BinOp::Ge => (a >= b) as i64,
                BinOp::Eq => (a == b) as i64,
                BinOp::Ne => (a != b) as i64,
                BinOp::LogAnd => ((a != 0) && (b != 0)) as i64,
                BinOp::LogOr => ((a != 0) || (b != 0)) as i64,
            })
        }
        // `?:` is permitted in a constant expression; only the taken arm counts.
        ExprKind::Cond { cond, then, els } => {
            if const_eval_int(cond)? != 0 {
                const_eval_int(then)
            } else {
                const_eval_int(els)
            }
        }
        _ => None,
    }
}
