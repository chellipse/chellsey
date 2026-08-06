//! hir-gen: the sema-annotated, frozen AST → HIR (hir_design.org §6.1). This
//! is the one place surface constructs desugar — loops canonicalize to `loop`,
//! `continue` becomes a `Break` of the block wrapping the loop body, `&&`/
//! `||`/`?:` become `If` regions writing a join local, compound assignment and
//! `++`/`--` become load-op-store with the lvalue evaluated once. Types come
//! from sema's annotations; `CType` dies here, spent into machine types and
//! per-op facts.
//!
//! `main` falling off the end returns 0 (§2.5) — made *explicit* here, so the
//! language rule never leaks below the frontend.

use std::collections::HashMap;

use anyhow::anyhow;

use super::types::*;
use crate::ast::{
    BinOp, CType, Expr, ExprKind, ExtDeclKind, Stmt, StmtKind, TranslationUnit, UnOp,
};
use crate::diagnostic::{Error, Span};
use crate::sema::{ProgramInfo, const_eval_int};

type Result<T> = std::result::Result<T, Error>;

/// A typed value — an HIR operand paired with the sema `CType` that produced
/// it. The type comes from sema's annotation, never a fresh decision here.
struct TV {
    val: Value,
    ty: CType,
}

pub fn build(unit: &TranslationUnit, info: &ProgramInfo) -> Result<Program> {
    let mut funcs = Vec::new();
    for decl in &unit.decls {
        match &decl.kind {
            ExtDeclKind::FuncDef { ident, params, body, .. } => {
                funcs.push(function(info, ident, params, body)?);
            }
            // A prototype contributes a signature (already in `info`) but no code.
            ExtDeclKind::FuncDecl { .. } => {}
        }
    }
    Ok(Program { funcs })
}

fn function(
    info: &ProgramInfo,
    name: &str,
    params: &[crate::ast::Param],
    body: &Stmt,
) -> Result<Function> {
    let sig = info
        .funcs
        .get(name)
        .expect("sema collected every defined function's signature");

    let mut g = FnGen::new();
    // The function scope: parameters are the first locals, in order.
    for p in params {
        let pname = p
            .name
            .as_ref()
            .expect("sema required names in a definition");
        let local = g.new_local(machine_ty(&p.ty));
        g.scopes[0].insert(pname.clone(), local);
    }
    // A label may be a forward `goto` target, so number every one up front.
    g.collect_labels(body);

    // The body is a compound statement (parser-guaranteed); its items become
    // the root region directly rather than a block nested in a block.
    let mut items = match &body.kind {
        StmtKind::Compound(stmts) => g.block_items(stmts)?,
        _ => unreachable!("a function body is a compound statement"),
    };
    if name == "main" && !matches!(items.last(), Some(Item::Ret(_))) {
        items.push(Item::Ret(Some(Value::Const(0))));
    }
    let root = Region {
        id: g.new_region_id(),
        kind: RegionKind::Block { body: items },
        span: body.span.clone(),
    };

    Ok(Function {
        name: name.to_string(),
        params: params.len(),
        ret_ty: machine_ty(&sig.ret),
        locals: g.locals,
        temps: g.next_temp,
        body: root,
    })
}

// A break/continue region on the desugaring stack, innermost last. `break`
// leaves the nearest region (loop or switch); `continue` leaves the *block
// wrapping the nearest loop's body*, which re-enters the loop at its tail
// (the condition re-test / the `for` step).
enum CfTarget {
    Loop { brk: RegionId, cont: RegionId },
    Switch { brk: RegionId },
}

struct FnGen {
    locals: Vec<LocalDecl>,
    // Name → local, one map per open block scope (innermost last). This
    // mirrors sema's scope discipline exactly, so a name resolves to the same
    // declaration in both walks; sema already diagnosed the failures.
    scopes: Vec<HashMap<String, LocalId>>,
    next_temp: u32,
    next_region: u32,
    // Label name → id, pre-collected so a forward `goto` has a target.
    labels: HashMap<String, LabelId>,
    cf: Vec<CfTarget>,
}

impl FnGen {
    fn new() -> Self {
        Self {
            locals: Vec::new(),
            scopes: vec![HashMap::new()],
            next_temp: 0,
            next_region: 0,
            labels: HashMap::new(),
            cf: Vec::new(),
        }
    }

    fn new_temp(&mut self) -> TempId {
        let t = TempId(self.next_temp);
        self.next_temp += 1;
        t
    }

    fn new_region_id(&mut self) -> RegionId {
        let r = RegionId(self.next_region);
        self.next_region += 1;
        r
    }

    fn new_local(&mut self, ty: Type) -> LocalId {
        let id = LocalId(self.locals.len() as u32);
        self.locals
            .push(LocalDecl { ty, addr_taken: false, volatile: false });
        id
    }

    /// Resolve a name to its local, innermost scope first. Sema diagnosed
    /// every unresolved name, so failure here is a compiler bug.
    fn lookup(&self, name: &str) -> LocalId {
        *self
            .scopes
            .iter()
            .rev()
            .find_map(|s| s.get(name))
            .expect("sema resolved every identifier")
    }

    /// Number every label in the body (they have function scope). Mirrors
    /// sema's label collection.
    fn collect_labels(&mut self, stmt: &Stmt) {
        match &stmt.kind {
            StmtKind::Label { name, body } => {
                let id = LabelId(self.labels.len() as u32);
                self.labels.insert(name.clone(), id);
                self.collect_labels(body);
            }
            StmtKind::Compound(stmts) => {
                for s in stmts {
                    self.collect_labels(s);
                }
            }
            StmtKind::If { then, els, .. } => {
                self.collect_labels(then);
                if let Some(els) = els {
                    self.collect_labels(els);
                }
            }
            StmtKind::While { body, .. }
            | StmtKind::DoWhile { body, .. }
            | StmtKind::Switch { body, .. }
            | StmtKind::Case { body, .. }
            | StmtKind::Default { body } => self.collect_labels(body),
            StmtKind::For { init, body, .. } => {
                if let Some(init) = init {
                    self.collect_labels(init);
                }
                self.collect_labels(body);
            }
            _ => {}
        }
    }

    fn emit(&mut self, items: &mut Vec<Item>, kind: InstKind, span: &Span) {
        items.push(Item::Inst(Inst { kind, span: span.clone() }));
    }

    /// Load a local into a fresh temp.
    fn load(&mut self, items: &mut Vec<Item>, local: LocalId, ty: Type, span: &Span) -> TempId {
        let dst = self.new_temp();
        self.emit(
            items,
            InstKind::LoadLocal { dst, local, ty, volatile: false },
            span,
        );
        dst
    }

    fn store(&mut self, items: &mut Vec<Item>, local: LocalId, val: Value, ty: Type, span: &Span) {
        self.emit(
            items,
            InstKind::StoreLocal { local, val, ty, volatile: false },
            span,
        );
    }

    /// The items of a compound statement, in their own scope.
    fn block_items(&mut self, stmts: &[Stmt]) -> Result<Vec<Item>> {
        self.scopes.push(HashMap::new());
        let mut items = Vec::new();
        for s in stmts {
            self.stmt(&mut items, s)?;
        }
        self.scopes.pop();
        Ok(items)
    }

    fn stmt(&mut self, items: &mut Vec<Item>, stmt: &Stmt) -> Result<()> {
        match &stmt.kind {
            StmtKind::Compound(stmts) => {
                let body = self.block_items(stmts)?;
                let id = self.new_region_id();
                items.push(Item::Region(Region {
                    id,
                    kind: RegionKind::Block { body },
                    span: stmt.span.clone(),
                }));
                Ok(())
            }
            StmtKind::Decl { ty, name, init } => {
                // The name enters scope before its initializer (6.2.1p7,
                // matching sema); the initializer is just a store to the local.
                let local = self.new_local(machine_ty(ty));
                self.scopes.last_mut().unwrap().insert(name.clone(), local);
                if let Some(init) = init {
                    let v = self.expr(items, init)?;
                    self.store(items, local, v.val, machine_ty(ty), &stmt.span);
                }
                Ok(())
            }
            StmtKind::Expr(expr) => {
                // evaluated for its side effects; the value is discarded
                self.expr(items, expr)?;
                Ok(())
            }
            StmtKind::Return(expr) => {
                // ME-12: the value is converted to the declared return type. In
                // this all-`int` subset the conversion is the identity.
                let val = match expr {
                    Some(e) => Some(self.expr(items, e)?.val),
                    None => None,
                };
                items.push(Item::Ret(val));
                Ok(())
            }
            StmtKind::If { cond, then, els } => {
                let c = self.expr(items, cond)?;
                let mut then_items = Vec::new();
                self.stmt(&mut then_items, then)?;
                let mut els_items = Vec::new();
                if let Some(els) = els {
                    self.stmt(&mut els_items, els)?;
                }
                let id = self.new_region_id();
                items.push(Item::Region(Region {
                    id,
                    kind: RegionKind::If { cond: c.val, then: then_items, els: els_items },
                    span: stmt.span.clone(),
                }));
                Ok(())
            }
            // `while (c) S` ⇒ `loop L { if !c break L; block B { S } }` — the
            // canonical loop re-tests the condition at its head on every
            // iteration; `continue` (break B) falls to the back edge.
            StmtKind::While { cond, body } => {
                let loop_id = self.new_region_id();
                let body_id = self.new_region_id();
                let mut li = Vec::new();
                let c = self.expr(&mut li, cond)?;
                self.break_unless(&mut li, c.val, loop_id, &cond.span);

                self.cf.push(CfTarget::Loop { brk: loop_id, cont: body_id });
                let mut bi = Vec::new();
                self.stmt(&mut bi, body)?;
                self.cf.pop();
                li.push(Item::Region(Region {
                    id: body_id,
                    kind: RegionKind::Block { body: bi },
                    span: body.span.clone(),
                }));

                items.push(Item::Region(Region {
                    id: loop_id,
                    kind: RegionKind::Loop { body: li },
                    span: stmt.span.clone(),
                }));
                Ok(())
            }
            // `do S while (c)` ⇒ `loop L { block B { S }; if !c break L }` —
            // the body runs before the first test; `continue` (break B) lands
            // on the condition.
            StmtKind::DoWhile { body, cond } => {
                let loop_id = self.new_region_id();
                let body_id = self.new_region_id();
                let mut li = Vec::new();

                self.cf.push(CfTarget::Loop { brk: loop_id, cont: body_id });
                let mut bi = Vec::new();
                self.stmt(&mut bi, body)?;
                self.cf.pop();
                li.push(Item::Region(Region {
                    id: body_id,
                    kind: RegionKind::Block { body: bi },
                    span: body.span.clone(),
                }));

                let c = self.expr(&mut li, cond)?;
                self.break_unless(&mut li, c.val, loop_id, &cond.span);

                items.push(Item::Region(Region {
                    id: loop_id,
                    kind: RegionKind::Loop { body: li },
                    span: stmt.span.clone(),
                }));
                Ok(())
            }
            // `for (i; c; s) S` ⇒ `block { i; loop L { if !c break L;
            // block B { S }; s } }` — the outer block is the for-clause scope
            // (6.8.5.3); `continue` (break B) runs the step (6.8.6.2). An
            // absent condition is always true, so no test is emitted.
            StmtKind::For { init, cond, step, body } => {
                self.scopes.push(HashMap::new());
                let mut oi = Vec::new();
                if let Some(init) = init {
                    self.stmt(&mut oi, init)?;
                }

                let loop_id = self.new_region_id();
                let body_id = self.new_region_id();
                let mut li = Vec::new();
                if let Some(cond) = cond {
                    let c = self.expr(&mut li, cond)?;
                    self.break_unless(&mut li, c.val, loop_id, &cond.span);
                }

                self.cf.push(CfTarget::Loop { brk: loop_id, cont: body_id });
                let mut bi = Vec::new();
                self.stmt(&mut bi, body)?;
                self.cf.pop();
                li.push(Item::Region(Region {
                    id: body_id,
                    kind: RegionKind::Block { body: bi },
                    span: body.span.clone(),
                }));

                if let Some(step) = step {
                    self.expr(&mut li, step)?;
                }
                oi.push(Item::Region(Region {
                    id: loop_id,
                    kind: RegionKind::Loop { body: li },
                    span: stmt.span.clone(),
                }));

                self.scopes.pop();
                let outer_id = self.new_region_id();
                items.push(Item::Region(Region {
                    id: outer_id,
                    kind: RegionKind::Block { body: oi },
                    span: stmt.span.clone(),
                }));
                Ok(())
            }
            StmtKind::Switch { disc, body } => {
                // Evaluate the controlling expression once, before the region;
                // the case/default markers inside dispatch against it.
                let d = self.expr(items, disc)?;
                let switch_id = self.new_region_id();
                self.cf.push(CfTarget::Switch { brk: switch_id });
                let mut bi = Vec::new();
                self.stmt(&mut bi, body)?;
                self.cf.pop();
                items.push(Item::Region(Region {
                    id: switch_id,
                    kind: RegionKind::Switch { ty: machine_ty(&d.ty), scrut: d.val, body: bi },
                    span: stmt.span.clone(),
                }));
                Ok(())
            }
            StmtKind::Case { value, body } => {
                let value =
                    const_eval_int(value).expect("sema validated the case label is constant");
                items.push(Item::Case { value });
                self.stmt(items, body)
            }
            StmtKind::Default { body } => {
                items.push(Item::Default);
                self.stmt(items, body)
            }
            StmtKind::Break => {
                let target = self
                    .cf
                    .last()
                    .map(|f| match f {
                        CfTarget::Loop { brk, .. } | CfTarget::Switch { brk } => *brk,
                    })
                    .expect("sema kept `break` inside a loop or switch");
                items.push(Item::Break(target));
                Ok(())
            }
            StmtKind::Continue => {
                let target = self
                    .cf
                    .iter()
                    .rev()
                    .find_map(|f| match f {
                        CfTarget::Loop { cont, .. } => Some(*cont),
                        CfTarget::Switch { .. } => None,
                    })
                    .expect("sema kept `continue` inside a loop");
                items.push(Item::Break(target));
                Ok(())
            }
            StmtKind::Goto { label } => {
                let target = *self
                    .labels
                    .get(label)
                    .expect("sema resolved every goto target");
                items.push(Item::Goto(target));
                Ok(())
            }
            StmtKind::Label { name, body } => {
                let id = *self.labels.get(name).expect("label id pre-collected");
                items.push(Item::Label(id));
                self.stmt(items, body)
            }
            StmtKind::Empty => Ok(()),
        }
    }

    /// `if !cond break target` — the canonical loop's exit test, emitted as an
    /// `If` whose else-arm breaks (no negation instruction needed).
    fn break_unless(&mut self, items: &mut Vec<Item>, cond: Value, target: RegionId, span: &Span) {
        let id = self.new_region_id();
        items.push(Item::Region(Region {
            id,
            kind: RegionKind::If { cond, then: Vec::new(), els: vec![Item::Break(target)] },
            span: span.clone(),
        }));
    }

    /// `a && b` / `a || b` (6.5.13/14) through a join local — the phi-free
    /// answer to a value crossing a join: the deciding arm stores the known
    /// constant, the other evaluates the rhs and stores its normalized truth
    /// value; the code after the `If` reloads the local.
    fn short_circuit(
        &mut self,
        items: &mut Vec<Item>,
        op: BinOp,
        lhs: &Expr,
        rhs: &Expr,
        span: &Span,
        ty: CType,
    ) -> Result<TV> {
        let join = self.new_local(machine_ty(&ty));
        let l = self.expr(items, lhs)?;

        let mut rhs_items = Vec::new();
        let r = self.expr(&mut rhs_items, rhs)?;
        // the result is the rhs's truth value, not the rhs itself
        let norm = self.new_temp();
        self.emit(
            &mut rhs_items,
            InstKind::ICmp {
                dst: norm,
                pred: IPred::Ne,
                lhs: r.val,
                rhs: Value::Const(0),
                ty: machine_ty(&r.ty),
            },
            span,
        );
        self.store(
            &mut rhs_items,
            join,
            Value::Temp(norm),
            machine_ty(&ty),
            span,
        );

        // `false && _` is 0 without looking at the rhs; `true || _` is 1.
        let mut short_items = Vec::new();
        let short_val = match op {
            BinOp::LogAnd => 0,
            BinOp::LogOr => 1,
            _ => unreachable!("only the short-circuit operators come here"),
        };
        self.store(
            &mut short_items,
            join,
            Value::Const(short_val),
            machine_ty(&ty),
            span,
        );

        let (then, els) = match op {
            BinOp::LogAnd => (rhs_items, short_items),
            _ => (short_items, rhs_items),
        };
        let id = self.new_region_id();
        items.push(Item::Region(Region {
            id,
            kind: RegionKind::If { cond: l.val, then, els },
            span: span.clone(),
        }));

        let dst = self.load(items, join, machine_ty(&ty), span);
        Ok(TV { val: Value::Temp(dst), ty })
    }

    /// Desugar an expression to a typed value, emitting the instructions it
    /// needs into `items`. `TV.ty` is read from sema's annotation.
    fn expr(&mut self, items: &mut Vec<Item>, e: &Expr) -> Result<TV> {
        let ty =
            e.ty.clone()
                .ok_or_else(|| err(&e.span, "internal: expression left untyped by sema"))?;

        match &e.kind {
            ExprKind::IntLit { value, ty: lit_ty } => {
                Ok(TV { val: Value::Const(*value as i64), ty: lit_ty.clone() })
            }
            // a variable read: an explicit lvalue-to-rvalue load (HIR-EXP-2)
            ExprKind::Ident { name } => {
                let local = self.lookup(name);
                let dst = self.load(items, local, machine_ty(&ty), &e.span);
                Ok(TV { val: Value::Temp(dst), ty })
            }
            // `c ? a : b` through a join local, like `&&`/`||` but storing the
            // taken arm's value unchanged — no truth normalization.
            ExprKind::Cond { cond, then, els } => {
                let join = self.new_local(machine_ty(&ty));
                let c = self.expr(items, cond)?;

                let mut then_items = Vec::new();
                let t = self.expr(&mut then_items, then)?;
                self.store(&mut then_items, join, t.val, machine_ty(&ty), &e.span);

                let mut els_items = Vec::new();
                let v = self.expr(&mut els_items, els)?;
                self.store(&mut els_items, join, v.val, machine_ty(&ty), &e.span);

                let id = self.new_region_id();
                items.push(Item::Region(Region {
                    id,
                    kind: RegionKind::If { cond: c.val, then: then_items, els: els_items },
                    span: e.span.clone(),
                }));

                let dst = self.load(items, join, machine_ty(&ty), &e.span);
                Ok(TV { val: Value::Temp(dst), ty })
            }
            // `lhs = rhs`: evaluate the rhs, store it to the lvalue's local;
            // the assignment's own value is the value stored (6.5.16.1).
            ExprKind::Assign { lhs, rhs } => {
                let v = self.expr(items, rhs)?;
                let ExprKind::Ident { name } = &lhs.kind else {
                    return Err(err(
                        &lhs.span,
                        "internal: non-lvalue assignment target past sema",
                    ));
                };
                let local = self.lookup(name);
                self.store(items, local, v.val, machine_ty(&ty), &e.span);
                Ok(TV { val: v.val, ty })
            }
            // `lhs op= rhs` (6.5.16.2): load the lvalue once, combine it with
            // the rhs under `op`, store the result back, and yield it. `lhs`
            // is an ident here, so "evaluate the lvalue once" is one lookup.
            ExprKind::CompoundAssign { op, lhs, rhs } => {
                let ExprKind::Ident { name } = &lhs.kind else {
                    return Err(err(
                        &lhs.span,
                        "internal: non-lvalue compound-assignment target past sema",
                    ));
                };
                let local = self.lookup(name);
                let cur = self.load(items, local, machine_ty(&ty), &e.span);
                let r = self.expr(items, rhs)?;
                let dst = self.new_temp();
                self.emit(
                    items,
                    InstKind::IBin {
                        dst,
                        op: ibin_op(*op).expect("a compound-assignment operator is arithmetic"),
                        lhs: Value::Temp(cur),
                        rhs: r.val,
                        ty: machine_ty(&ty),
                        flags: UbFlags::default(),
                    },
                    &e.span,
                );
                self.store(items, local, Value::Temp(dst), machine_ty(&ty), &e.span);
                Ok(TV { val: Value::Temp(dst), ty })
            }
            // `++x`/`x++` (6.5.3.1/6.5.2.4): load the lvalue once, add or
            // subtract 1, store it back. Prefix's value is the updated temp;
            // postfix's is the value loaded before the update.
            ExprKind::IncDec { pre, inc, expr } => {
                let ExprKind::Ident { name } = &expr.kind else {
                    return Err(err(
                        &expr.span,
                        "internal: non-lvalue `++`/`--` target past sema",
                    ));
                };
                let local = self.lookup(name);
                let cur = self.load(items, local, machine_ty(&ty), &e.span);
                let next = self.new_temp();
                self.emit(
                    items,
                    InstKind::IBin {
                        dst: next,
                        op: if *inc { IBinOp::Add } else { IBinOp::Sub },
                        lhs: Value::Temp(cur),
                        rhs: Value::Const(1),
                        ty: machine_ty(&ty),
                        flags: UbFlags::default(),
                    },
                    &e.span,
                );
                self.store(items, local, Value::Temp(next), machine_ty(&ty), &e.span);
                let val = if *pre {
                    Value::Temp(next)
                } else {
                    Value::Temp(cur)
                };
                Ok(TV { val, ty })
            }
            // `lhs , rhs` (6.5.17): evaluate the left for its side effects and
            // discard the value, then the whole expression is the right.
            ExprKind::Comma { lhs, rhs } => {
                self.expr(items, lhs)?;
                self.expr(items, rhs)
            }
            // `f(args)`: evaluate every argument (left to right), then call.
            ExprKind::Call { callee, args } => {
                let mut arg_vals = Vec::with_capacity(args.len());
                for a in args {
                    arg_vals.push(self.expr(items, a)?.val);
                }
                let dst = self.new_temp();
                self.emit(
                    items,
                    InstKind::Call {
                        dst,
                        callee: callee.clone(),
                        args: arg_vals,
                        ty: machine_ty(&ty),
                    },
                    &e.span,
                );
                Ok(TV { val: Value::Temp(dst), ty })
            }
            ExprKind::Unary { op, expr } => {
                let operand = self.expr(items, expr)?;
                match op {
                    // ME-5: unary minus lowers as `0 - x`.
                    UnOp::Neg => {
                        let dst = self.new_temp();
                        self.emit(
                            items,
                            InstKind::IBin {
                                dst,
                                op: IBinOp::Sub,
                                lhs: Value::Const(0),
                                rhs: operand.val,
                                ty: machine_ty(&ty),
                                flags: UbFlags::default(),
                            },
                            &e.span,
                        );
                        Ok(TV { val: Value::Temp(dst), ty })
                    }
                    // `~x` is `x ^ -1`: flipping all 64 bits of the
                    // sign-extended operand is the correct `int` complement,
                    // canonical form and all (bit 63 tracks the flipped sign).
                    UnOp::BitNot => {
                        let dst = self.new_temp();
                        self.emit(
                            items,
                            InstKind::IBin {
                                dst,
                                op: IBinOp::Xor,
                                lhs: operand.val,
                                rhs: Value::Const(-1),
                                ty: machine_ty(&ty),
                                flags: UbFlags::default(),
                            },
                            &e.span,
                        );
                        Ok(TV { val: Value::Temp(dst), ty })
                    }
                    // `!x` is `x == 0` — an icmp yielding a 0/1 int.
                    UnOp::Not => {
                        let dst = self.new_temp();
                        self.emit(
                            items,
                            InstKind::ICmp {
                                dst,
                                pred: IPred::Eq,
                                lhs: operand.val,
                                rhs: Value::Const(0),
                                ty: machine_ty(&operand.ty),
                            },
                            &e.span,
                        );
                        Ok(TV { val: Value::Temp(dst), ty })
                    }
                }
            }
            ExprKind::Binary { op, lhs, rhs } => {
                // `&&`/`||` evaluate the rhs conditionally, so they build
                // control flow instead of a single instruction.
                if matches!(op, BinOp::LogAnd | BinOp::LogOr) {
                    return self.short_circuit(items, *op, lhs, rhs, &e.span, ty);
                }
                let l = self.expr(items, lhs)?;
                let r = self.expr(items, rhs)?;
                // Relational / equality operators compare the operands and
                // yield a 0/1 `int` — an `icmp`, not an `IBin`. `ty` on the
                // compare is the operand type; the result type is the outer
                // int.
                if let Some(pred) = int_pred(op) {
                    let dst = self.new_temp();
                    self.emit(
                        items,
                        InstKind::ICmp { dst, pred, lhs: l.val, rhs: r.val, ty: machine_ty(&l.ty) },
                        &e.span,
                    );
                    return Ok(TV { val: Value::Temp(dst), ty });
                }
                // Operands are signed `int` in this subset, so the signed ops.
                let op = ibin_op(*op).expect("comparisons and short-circuits are handled above");
                let dst = self.new_temp();
                self.emit(
                    items,
                    InstKind::IBin {
                        dst,
                        op,
                        lhs: l.val,
                        rhs: r.val,
                        ty: machine_ty(&ty),
                        flags: UbFlags::default(),
                    },
                    &e.span,
                );
                Ok(TV { val: Value::Temp(dst), ty })
            }
        }
    }
}

/// Map a sema `CType` to its machine type. `CType`'s last act (§4 of
/// hir_design.org): nothing below hir-gen sees a C type.
fn machine_ty(ct: &CType) -> Type {
    match ct {
        CType::Void => Type::Void,
        CType::Bool | CType::Char { .. } => Type::I8,
        CType::Short { .. } => Type::I16,
        CType::Int { .. } => Type::I32,
        CType::Long { .. } => Type::I64,
        CType::Float => Type::F32,
        CType::Double => Type::F64,
        // arrays decay to a pointer; neither is lowered further yet (ME-8).
        CType::Ptr(_) | CType::Array(..) => Type::Ptr,
    }
}

/// The integer-arithmetic opcode for an arithmetic/bitwise/shift operator, or
/// `None` for the relational/equality/short-circuit operators (those desugar
/// to an `icmp` or to control flow, never an `IBin`). Operands are signed
/// `int` in this subset, so division and `>>` map to the signed opcodes.
fn ibin_op(op: BinOp) -> Option<IBinOp> {
    Some(match op {
        BinOp::Add => IBinOp::Add,
        BinOp::Sub => IBinOp::Sub,
        BinOp::Mul => IBinOp::Mul,
        BinOp::Div => IBinOp::SDiv,
        BinOp::Rem => IBinOp::SRem,
        BinOp::BitAnd => IBinOp::And,
        BinOp::BitOr => IBinOp::Or,
        BinOp::BitXor => IBinOp::Xor,
        BinOp::Shl => IBinOp::Shl,
        BinOp::Shr => IBinOp::AShr,
        _ => return None,
    })
}

/// The signed comparison predicate for a relational/equality operator, or
/// `None` for any other operator. `int` is signed, so orderings map to the
/// signed predicates.
fn int_pred(op: &BinOp) -> Option<IPred> {
    Some(match op {
        BinOp::Lt => IPred::SLt,
        BinOp::Gt => IPred::SGt,
        BinOp::Le => IPred::SLe,
        BinOp::Ge => IPred::SGe,
        BinOp::Eq => IPred::Eq,
        BinOp::Ne => IPred::Ne,
        _ => return None,
    })
}

fn err(span: &Span, msg: &str) -> Error {
    span.clone().into_error(anyhow!("{msg}"))
}
