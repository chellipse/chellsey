use std::collections::HashMap;

use anyhow::anyhow;

use super::types::*;
use crate::ast::{
    BinOp, CType, Expr, ExprKind, ExtDeclKind, Param, Stmt, StmtKind, TranslationUnit, UnOp,
};
use crate::diagnostic::{Error, Span};
use crate::sema::{const_eval_int, ProgramInfo};

type Result<T> = std::result::Result<T, Error>;

/// A typed value — an IR operand paired with the sema `CType` that produced it
/// (ME-2). The type comes from sema's annotation, never a fresh decision here.
struct TV {
    val: Value,
    ty: CType,
}

pub fn lower(unit: &TranslationUnit, info: &ProgramInfo) -> Result<Program> {
    let low = Lowerer { info };
    let mut funcs = Vec::new();
    for decl in &unit.decls {
        match &decl.kind {
            ExtDeclKind::FuncDef { ident, params, body, .. } => {
                funcs.push(low.function(ident, params, body)?);
            }
            // A prototype contributes a signature (already in `info`) but no code.
            ExtDeclKind::FuncDecl { .. } => {}
        }
    }
    Ok(Program { funcs, data: Vec::new() })
}

/// Cross-function lowering state. Near-empty in v1 (string interning and the
/// global-data table arrive with ME-11); it owns the `ProgramInfo` borrow that
/// per-function lowering consults for signatures.
struct Lowerer<'a> {
    info: &'a ProgramInfo,
}

impl Lowerer<'_> {
    fn function(&self, name: &str, params: &[Param], body: &Stmt) -> Result<Function> {
        let sig = self
            .info
            .funcs
            .get(name)
            .expect("sema collected every defined function's signature");
        let param_tys = sig.params.iter().map(ir_ty).collect();
        let ret_ty = ir_ty(&sig.ret);

        let mut b = FnBuilder::new(name.to_string(), param_tys, ret_ty);
        // Parameters take the first `params.len()` slots (the backend's
        // prologue relies on this) and are named in the function scope.
        for p in params {
            let name = p
                .name
                .as_ref()
                .expect("sema required names in a definition");
            let slot = b.new_slot(ir_ty(&p.ty));
            b.scopes[0].insert(name.clone(), slot);
        }
        b.stmt(body)?;
        // ME-12: `main` falling off the end returns 0.
        if name == "main" {
            b.implicit_return_zero();
        }
        Ok(b.finish())
    }
}

// A break/continue region on the lowering stack, innermost last. `break`
// leaves the nearest region (loop or switch); `continue` re-enters the nearest
// *loop*, passing through switches. A switch frame also gathers its case/default
// target blocks as they are lowered, for building the dispatch chain afterward.
enum CfFrame {
    Loop {
        continue_bb: BlockId,
        break_bb: BlockId,
    },
    Switch {
        break_bb: BlockId,
        disc: Value,
        disc_ty: Type,
        cases: Vec<(i64, BlockId)>,
        default: Option<BlockId>,
    },
}

struct FnBuilder {
    name: String,
    params: Vec<Type>,
    ret_ty: Type,
    blocks: Vec<Block>,
    /// Index into `blocks` of the block instructions currently append to.
    current: usize,
    /// Next SSA register to hand out.
    next_reg: u32,
    // Whether the current block already has a terminator, so trailing dead
    // statements and fall-through edges are suppressed.
    terminated: bool,
    /// The function's stack-slot table (MIR-STR-4): one slot per local.
    slots: Vec<Type>,
    // Name → slot, one map per open block scope (innermost last). This mirrors
    // sema's scope discipline exactly, so a name resolves to the same
    // declaration in both walks; sema already diagnosed the failures.
    scopes: Vec<HashMap<String, SlotId>>,
    // Enclosing break/continue regions (loops and switches), innermost last.
    // Sema already rejected jumps outside their region.
    cf: Vec<CfFrame>,
}

impl FnBuilder {
    fn new(name: String, params: Vec<Type>, ret_ty: Type) -> Self {
        // Entry block. Its terminator defaults to `ret void`, which an explicit
        // `return` overwrites; for `main` an unterminated fall-through becomes
        // `ret 0` (ME-12). A multi-block CFG arrives with control flow (ME-7).
        let entry = Block { id: BlockId(0), insts: Vec::new(), term: Terminator::Ret(None) };
        Self {
            name,
            params,
            ret_ty,
            blocks: vec![entry],
            current: 0,
            next_reg: 0,
            terminated: false,
            slots: Vec::new(),
            // the function scope; parameters will land here (FE-14)
            scopes: vec![HashMap::new()],
            cf: Vec::new(),
        }
    }

    fn new_reg(&mut self) -> u32 {
        let r = self.next_reg;
        self.next_reg += 1;
        r
    }

    /// Reserve a stack slot for a local and return its id.
    fn new_slot(&mut self, ty: Type) -> SlotId {
        let id = SlotId(self.slots.len() as u32);
        self.slots.push(ty);
        id
    }

    /// Resolve a name to its slot, innermost scope first. Sema diagnosed every
    /// unresolved name, so failure here is a compiler bug.
    fn lookup(&self, name: &str) -> SlotId {
        *self
            .scopes
            .iter()
            .rev()
            .find_map(|s| s.get(name))
            .expect("sema resolved every identifier")
    }

    /// Append a fresh, open block (default `ret void` terminator) and return its id.
    fn new_block(&mut self) -> BlockId {
        let id = BlockId(self.blocks.len() as u32);
        self.blocks
            .push(Block { id, insts: Vec::new(), term: Terminator::Ret(None) });
        id
    }

    /// Make `bb` the block that subsequent instructions append to.
    fn switch_to(&mut self, bb: BlockId) {
        self.current = bb.0 as usize;
        self.terminated = false;
    }

    /// Set the current block's terminator and mark it closed.
    fn set_term(&mut self, term: Terminator) {
        self.blocks[self.current].term = term;
        self.terminated = true;
    }

    fn emit(&mut self, kind: InstKind, span: &Span) {
        self.blocks[self.current]
            .insts
            .push(Inst { kind, span: span.clone() });
    }

    fn implicit_return_zero(&mut self) {
        let term = &mut self.blocks[self.current].term;
        if matches!(term, Terminator::Ret(None)) {
            *term = Terminator::Ret(Some(Value::Const(0)));
        }
    }

    fn finish(self) -> Function {
        Function {
            name: self.name,
            params: self.params,
            ret_ty: self.ret_ty,
            slots: self.slots,
            blocks: self.blocks,
        }
    }

    fn stmt(&mut self, stmt: &Stmt) -> Result<()> {
        // Statements following a terminator (e.g. after `return`) are dead —
        // except `case`/`default` labels, which are reached from the switch
        // dispatch rather than by fallthrough, so a preceding `break` does not
        // make them unreachable.
        if self.terminated && !matches!(stmt.kind, StmtKind::Case { .. } | StmtKind::Default { .. })
        {
            return Ok(());
        }
        match &stmt.kind {
            StmtKind::Compound(stmts) => {
                self.scopes.push(HashMap::new());
                for s in stmts {
                    self.stmt(s)?;
                }
                self.scopes.pop();
                Ok(())
            }
            StmtKind::Decl { ty, name, init } => {
                // The name enters scope before its initializer (6.2.1p7,
                // matching sema); the initializer is just a store to the slot.
                let slot = self.new_slot(ir_ty(ty));
                self.scopes.last_mut().unwrap().insert(name.clone(), slot);
                if let Some(init) = init {
                    let v = self.expr(init)?;
                    self.emit(
                        InstKind::Store { slot, val: v.val, ty: ir_ty(ty) },
                        &stmt.span,
                    );
                }
                Ok(())
            }
            StmtKind::Expr(expr) => {
                // evaluated for its side effects; the value is discarded
                self.expr(expr)?;
                Ok(())
            }
            StmtKind::Return(expr) => {
                // ME-12: the value is converted to the declared return type. In
                // this all-`int` subset the conversion is the identity.
                let val = match expr {
                    Some(e) => Some(self.expr(e)?.val),
                    None => None,
                };
                self.set_term(Terminator::Ret(val));
                Ok(())
            }
            StmtKind::If { cond, then, els } => {
                let c = self.expr(cond)?;
                let then_bb = self.new_block();
                let else_bb = self.new_block();
                let cont_bb = self.new_block();
                self.set_term(Terminator::CondBr { cond: c.val, then_bb, else_bb });

                self.switch_to(then_bb);
                self.stmt(then)?;
                if !self.terminated {
                    self.set_term(Terminator::Br(cont_bb));
                }

                self.switch_to(else_bb);
                if let Some(els) = els {
                    self.stmt(els)?;
                }
                if !self.terminated {
                    self.set_term(Terminator::Br(cont_bb));
                }

                self.switch_to(cont_bb);
                Ok(())
            }
            StmtKind::While { cond, body } => {
                // The condition lives in its own block: the back edge re-enters
                // it, so it re-evaluates on every iteration.
                let cond_bb = self.new_block();
                let body_bb = self.new_block();
                let exit_bb = self.new_block();
                self.set_term(Terminator::Br(cond_bb));

                self.switch_to(cond_bb);
                let c = self.expr(cond)?;
                self.set_term(Terminator::CondBr {
                    cond: c.val,
                    then_bb: body_bb,
                    else_bb: exit_bb,
                });

                self.switch_to(body_bb);
                // `continue` re-tests the condition; `break` leaves the loop.
                self.cf.push(CfFrame::Loop { continue_bb: cond_bb, break_bb: exit_bb });
                self.stmt(body)?;
                self.cf.pop();
                if !self.terminated {
                    self.set_term(Terminator::Br(cond_bb)); // the back edge
                }

                self.switch_to(exit_bb);
                Ok(())
            }
            StmtKind::DoWhile { body, cond } => {
                // The body runs before the first test; the back edge re-enters
                // it. `continue` jumps to the condition, `break` past the loop.
                let body_bb = self.new_block();
                let cond_bb = self.new_block();
                let exit_bb = self.new_block();
                self.set_term(Terminator::Br(body_bb));

                self.switch_to(body_bb);
                self.cf.push(CfFrame::Loop { continue_bb: cond_bb, break_bb: exit_bb });
                self.stmt(body)?;
                self.cf.pop();
                if !self.terminated {
                    self.set_term(Terminator::Br(cond_bb));
                }

                self.switch_to(cond_bb);
                let c = self.expr(cond)?;
                self.set_term(Terminator::CondBr {
                    cond: c.val,
                    then_bb: body_bb, // the back edge
                    else_bb: exit_bb,
                });

                self.switch_to(exit_bb);
                Ok(())
            }
            StmtKind::For { init, cond, step, body } => {
                // The for-clause scope (6.8.5.3): an init declaration is
                // visible in cond, step, and body, and dies at the loop's end.
                self.scopes.push(HashMap::new());
                if let Some(init) = init {
                    self.stmt(init)?;
                }
                let cond_bb = self.new_block();
                let body_bb = self.new_block();
                let step_bb = self.new_block();
                let exit_bb = self.new_block();
                self.set_term(Terminator::Br(cond_bb));

                self.switch_to(cond_bb);
                let c = match cond {
                    Some(cond) => self.expr(cond)?.val,
                    // an absent controlling expression is always true
                    None => Value::Const(1),
                };
                self.set_term(Terminator::CondBr { cond: c, then_bb: body_bb, else_bb: exit_bb });

                self.switch_to(body_bb);
                // `continue` runs the step before re-testing (6.8.6.2).
                self.cf.push(CfFrame::Loop { continue_bb: step_bb, break_bb: exit_bb });
                self.stmt(body)?;
                self.cf.pop();
                if !self.terminated {
                    self.set_term(Terminator::Br(step_bb));
                }

                // A separate step block so `continue` has its target (FE-18).
                self.switch_to(step_bb);
                if let Some(step) = step {
                    self.expr(step)?;
                }
                self.set_term(Terminator::Br(cond_bb)); // the back edge

                self.switch_to(exit_bb);
                self.scopes.pop();
                Ok(())
            }
            StmtKind::Switch { disc, body } => {
                // Evaluate the controlling expression once; the dispatch chain
                // reuses it across blocks (a value may be used wherever its def
                // dominates it). Statements before the first `case` are
                // unreachable but still lowered — into `body_entry`, which
                // nothing branches to.
                let d = self.expr(disc)?;
                let disc_ty = ir_ty(&d.ty);
                let dispatch_bb = self.new_block();
                let exit_bb = self.new_block();
                let body_entry = self.new_block();
                self.set_term(Terminator::Br(dispatch_bb));

                self.switch_to(body_entry);
                self.cf.push(CfFrame::Switch {
                    break_bb: exit_bb,
                    disc: d.val,
                    disc_ty,
                    cases: Vec::new(),
                    default: None,
                });
                self.stmt(body)?;
                if !self.terminated {
                    self.set_term(Terminator::Br(exit_bb)); // fall off the end
                }
                let CfFrame::Switch { disc, disc_ty, cases, default, .. } = self.cf.pop().unwrap()
                else {
                    unreachable!("just pushed a switch frame");
                };

                // The dispatch: compare the discriminant against each case value
                // in turn, falling to `default` (or the exit) when none match.
                self.switch_to(dispatch_bb);
                for (val, target) in cases {
                    let t = self.new_reg();
                    self.emit(
                        InstKind::ICmp {
                            dst: t,
                            pred: IPred::Eq,
                            lhs: disc,
                            rhs: Value::Const(val),
                            ty: disc_ty,
                        },
                        &stmt.span,
                    );
                    let next = self.new_block();
                    self.set_term(Terminator::CondBr {
                        cond: Value::Reg(t),
                        then_bb: target,
                        else_bb: next,
                    });
                    self.switch_to(next);
                }
                self.set_term(Terminator::Br(default.unwrap_or(exit_bb)));

                self.switch_to(exit_bb);
                Ok(())
            }
            StmtKind::Case { value, body } => {
                let val = const_eval_int(value).expect("sema validated the case label is constant");
                let case_bb = self.new_block();
                {
                    let cases = self
                        .cf
                        .iter_mut()
                        .rev()
                        .find_map(|f| match f {
                            CfFrame::Switch { cases, .. } => Some(cases),
                            CfFrame::Loop { .. } => None,
                        })
                        .expect("sema kept `case` inside a switch");
                    cases.push((val, case_bb));
                }
                // fall through from the preceding code into this case's block
                if !self.terminated {
                    self.set_term(Terminator::Br(case_bb));
                }
                self.switch_to(case_bb);
                self.stmt(body)
            }
            StmtKind::Default { body } => {
                let def_bb = self.new_block();
                {
                    let default = self
                        .cf
                        .iter_mut()
                        .rev()
                        .find_map(|f| match f {
                            CfFrame::Switch { default, .. } => Some(default),
                            CfFrame::Loop { .. } => None,
                        })
                        .expect("sema kept `default` inside a switch");
                    *default = Some(def_bb);
                }
                if !self.terminated {
                    self.set_term(Terminator::Br(def_bb));
                }
                self.switch_to(def_bb);
                self.stmt(body)
            }
            StmtKind::Break => {
                let break_bb = self
                    .cf
                    .last()
                    .map(|f| match f {
                        CfFrame::Loop { break_bb, .. } | CfFrame::Switch { break_bb, .. } => {
                            *break_bb
                        }
                    })
                    .expect("sema kept `break` inside a loop or switch");
                self.set_term(Terminator::Br(break_bb));
                Ok(())
            }
            StmtKind::Continue => {
                let continue_bb = self
                    .cf
                    .iter()
                    .rev()
                    .find_map(|f| match f {
                        CfFrame::Loop { continue_bb, .. } => Some(*continue_bb),
                        CfFrame::Switch { .. } => None,
                    })
                    .expect("sema kept `continue` inside a loop");
                self.set_term(Terminator::Br(continue_bb));
                Ok(())
            }
            StmtKind::Empty => Ok(()),
        }
    }

    /// `a && b` / `a || b` (6.5.13/14) through a result slot — the memory-form
    /// answer to a value crossing a join (no phis, MIR-STR-4): the deciding
    /// branch stores the known constant, the other evaluates the rhs and
    /// stores its normalized truth value; the join reloads the slot.
    fn short_circuit(
        &mut self,
        op: BinOp,
        lhs: &Expr,
        rhs: &Expr,
        span: &Span,
        ty: CType,
    ) -> Result<TV> {
        let slot = self.new_slot(ir_ty(&ty));
        let l = self.expr(lhs)?;

        let rhs_bb = self.new_block();
        let short_bb = self.new_block();
        let join_bb = self.new_block();
        // `false && _` is 0 without looking at the rhs; `true || _` is 1.
        let (then_bb, else_bb, short_val) = match op {
            BinOp::LogAnd => (rhs_bb, short_bb, 0),
            BinOp::LogOr => (short_bb, rhs_bb, 1),
            _ => unreachable!("only the short-circuit operators come here"),
        };
        self.set_term(Terminator::CondBr { cond: l.val, then_bb, else_bb });

        self.switch_to(rhs_bb);
        let r = self.expr(rhs)?;
        // the result is the rhs's truth value, not the rhs itself
        let norm = self.new_reg();
        self.emit(
            InstKind::ICmp {
                dst: norm,
                pred: IPred::Ne,
                lhs: r.val,
                rhs: Value::Const(0),
                ty: ir_ty(&r.ty),
            },
            span,
        );
        self.emit(
            InstKind::Store { slot, val: Value::Reg(norm), ty: ir_ty(&ty) },
            span,
        );
        self.set_term(Terminator::Br(join_bb));

        self.switch_to(short_bb);
        self.emit(
            InstKind::Store { slot, val: Value::Const(short_val), ty: ir_ty(&ty) },
            span,
        );
        self.set_term(Terminator::Br(join_bb));

        self.switch_to(join_bb);
        let dst = self.new_reg();
        self.emit(InstKind::Load { dst, slot, ty: ir_ty(&ty) }, span);
        Ok(TV { val: Value::Reg(dst), ty })
    }

    /// Lower an expression to a typed value, emitting the instructions it needs
    /// into the current block. `TV.ty` is read from sema's annotation.
    fn expr(&mut self, e: &Expr) -> Result<TV> {
        let ty =
            e.ty.clone()
                .ok_or_else(|| err(&e.span, "internal: expression left untyped by sema"))?;

        match &e.kind {
            ExprKind::IntLit { value, ty: lit_ty } => {
                Ok(TV { val: Value::Const(*value as i64), ty: lit_ty.clone() })
            }
            // a variable read: load its stack slot into a fresh register
            ExprKind::Ident { name } => {
                let slot = self.lookup(name);
                let dst = self.new_reg();
                self.emit(InstKind::Load { dst, slot, ty: ir_ty(&ty) }, &e.span);
                Ok(TV { val: Value::Reg(dst), ty })
            }
            // `c ? a : b` through a result slot, like `&&`/`||` but storing
            // the taken arm's value unchanged — no truth normalization.
            ExprKind::Cond { cond, then, els } => {
                let slot = self.new_slot(ir_ty(&ty));
                let c = self.expr(cond)?;
                let then_bb = self.new_block();
                let else_bb = self.new_block();
                let join_bb = self.new_block();
                self.set_term(Terminator::CondBr { cond: c.val, then_bb, else_bb });

                self.switch_to(then_bb);
                let t = self.expr(then)?;
                self.emit(
                    InstKind::Store { slot, val: t.val, ty: ir_ty(&ty) },
                    &e.span,
                );
                self.set_term(Terminator::Br(join_bb));

                self.switch_to(else_bb);
                let v = self.expr(els)?;
                self.emit(
                    InstKind::Store { slot, val: v.val, ty: ir_ty(&ty) },
                    &e.span,
                );
                self.set_term(Terminator::Br(join_bb));

                self.switch_to(join_bb);
                let dst = self.new_reg();
                self.emit(InstKind::Load { dst, slot, ty: ir_ty(&ty) }, &e.span);
                Ok(TV { val: Value::Reg(dst), ty })
            }
            // `lhs = rhs`: evaluate the rhs, store it to the lvalue's slot; the
            // assignment's own value is the value stored (6.5.16.1).
            ExprKind::Assign { lhs, rhs } => {
                let v = self.expr(rhs)?;
                let ExprKind::Ident { name } = &lhs.kind else {
                    return Err(err(
                        &lhs.span,
                        "internal: non-lvalue assignment target past sema",
                    ));
                };
                let slot = self.lookup(name);
                self.emit(
                    InstKind::Store { slot, val: v.val, ty: ir_ty(&ty) },
                    &e.span,
                );
                Ok(TV { val: v.val, ty })
            }
            // `lhs op= rhs` (6.5.16.2): load the lvalue once, combine it with
            // the rhs under `op`, store the result back, and yield it. `lhs` is
            // an ident here, so "evaluate the lvalue once" is one slot lookup.
            ExprKind::CompoundAssign { op, lhs, rhs } => {
                let ExprKind::Ident { name } = &lhs.kind else {
                    return Err(err(
                        &lhs.span,
                        "internal: non-lvalue compound-assignment target past sema",
                    ));
                };
                let slot = self.lookup(name);
                let cur = self.new_reg();
                self.emit(InstKind::Load { dst: cur, slot, ty: ir_ty(&ty) }, &e.span);
                let r = self.expr(rhs)?;
                let dst = self.new_reg();
                self.emit(
                    InstKind::IBin {
                        dst,
                        op: ibin_op(*op).expect("a compound-assignment operator is arithmetic"),
                        lhs: Value::Reg(cur),
                        rhs: r.val,
                        ty: ir_ty(&ty),
                        flags: UbFlags::default(),
                    },
                    &e.span,
                );
                self.emit(
                    InstKind::Store { slot, val: Value::Reg(dst), ty: ir_ty(&ty) },
                    &e.span,
                );
                Ok(TV { val: Value::Reg(dst), ty })
            }
            // `++x`/`x++` (6.5.3.1/6.5.2.4): load the lvalue once, add or
            // subtract 1, store it back. Prefix's value is the updated register;
            // postfix's is the value loaded before the update.
            ExprKind::IncDec { pre, inc, expr } => {
                let ExprKind::Ident { name } = &expr.kind else {
                    return Err(err(
                        &expr.span,
                        "internal: non-lvalue `++`/`--` target past sema",
                    ));
                };
                let slot = self.lookup(name);
                let cur = self.new_reg();
                self.emit(InstKind::Load { dst: cur, slot, ty: ir_ty(&ty) }, &e.span);
                let next = self.new_reg();
                self.emit(
                    InstKind::IBin {
                        dst: next,
                        op: if *inc { IBinOp::Add } else { IBinOp::Sub },
                        lhs: Value::Reg(cur),
                        rhs: Value::Const(1),
                        ty: ir_ty(&ty),
                        flags: UbFlags::default(),
                    },
                    &e.span,
                );
                self.emit(
                    InstKind::Store { slot, val: Value::Reg(next), ty: ir_ty(&ty) },
                    &e.span,
                );
                let val = if *pre { Value::Reg(next) } else { Value::Reg(cur) };
                Ok(TV { val, ty })
            }
            // `lhs , rhs` (6.5.17): evaluate the left for its side effects and
            // discard the value, then the whole expression is the right.
            ExprKind::Comma { lhs, rhs } => {
                self.expr(lhs)?;
                self.expr(rhs)
            }
            // `f(args)`: evaluate every argument (left to right), then call.
            // All arguments are computed before the call, so a nested call in
            // one argument can't clobber another — each has its own slot.
            ExprKind::Call { callee, args } => {
                let mut arg_vals = Vec::with_capacity(args.len());
                for a in args {
                    arg_vals.push(self.expr(a)?.val);
                }
                let dst = self.new_reg();
                self.emit(
                    InstKind::Call { dst, callee: callee.clone(), args: arg_vals, ty: ir_ty(&ty) },
                    &e.span,
                );
                Ok(TV { val: Value::Reg(dst), ty })
            }
            ExprKind::Unary { op, expr } => {
                let operand = self.expr(expr)?;
                match op {
                    // ME-5: unary minus lowers as `0 - x`.
                    UnOp::Neg => {
                        let dst = self.new_reg();
                        self.emit(
                            InstKind::IBin {
                                dst,
                                op: IBinOp::Sub,
                                lhs: Value::Const(0),
                                rhs: operand.val,
                                ty: ir_ty(&ty),
                                flags: UbFlags::default(),
                            },
                            &e.span,
                        );
                        Ok(TV { val: Value::Reg(dst), ty })
                    }
                    // `~x` is `x ^ -1`: flipping all 64 bits of the sign-extended
                    // operand is the correct `int` complement, canonical form and
                    // all (bit 63 tracks the flipped sign bit).
                    UnOp::BitNot => {
                        let dst = self.new_reg();
                        self.emit(
                            InstKind::IBin {
                                dst,
                                op: IBinOp::Xor,
                                lhs: operand.val,
                                rhs: Value::Const(-1),
                                ty: ir_ty(&ty),
                                flags: UbFlags::default(),
                            },
                            &e.span,
                        );
                        Ok(TV { val: Value::Reg(dst), ty })
                    }
                    // `!x` is `x == 0` — an icmp yielding a 0/1 int.
                    UnOp::Not => {
                        let dst = self.new_reg();
                        self.emit(
                            InstKind::ICmp {
                                dst,
                                pred: IPred::Eq,
                                lhs: operand.val,
                                rhs: Value::Const(0),
                                ty: ir_ty(&operand.ty),
                            },
                            &e.span,
                        );
                        Ok(TV { val: Value::Reg(dst), ty })
                    }
                }
            }
            ExprKind::Binary { op, lhs, rhs } => {
                // `&&`/`||` evaluate the rhs conditionally, so they build CFG
                // instead of a single instruction — before touching operands.
                if matches!(op, BinOp::LogAnd | BinOp::LogOr) {
                    return self.short_circuit(*op, lhs, rhs, &e.span, ty);
                }
                let l = self.expr(lhs)?;
                let r = self.expr(rhs)?;
                // Relational / equality operators compare the operands and yield
                // a 0/1 `int` — an `icmp`, not an `IBin`. `ty` on the compare is
                // the operand type; the result type is `ty` (the outer int).
                if let Some(pred) = int_pred(op) {
                    let dst = self.new_reg();
                    self.emit(
                        InstKind::ICmp { dst, pred, lhs: l.val, rhs: r.val, ty: ir_ty(&l.ty) },
                        &e.span,
                    );
                    return Ok(TV { val: Value::Reg(dst), ty });
                }
                // Operands are signed `int` in this subset, so the signed ops.
                let ir_op = ibin_op(*op).expect("comparisons and short-circuits are handled above");
                let dst = self.new_reg();
                self.emit(
                    InstKind::IBin {
                        dst,
                        op: ir_op,
                        lhs: l.val,
                        rhs: r.val,
                        ty: ir_ty(&ty),
                        flags: UbFlags::default(),
                    },
                    &e.span,
                );
                Ok(TV { val: Value::Reg(dst), ty })
            }
        }
    }
}

/// Map a sema `CType` to its IR machine type.
fn ir_ty(ct: &CType) -> Type {
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

/// The IR integer-arithmetic opcode for an arithmetic/bitwise/shift operator,
/// or `None` for the relational/equality/short-circuit operators (those lower
/// to an `icmp` or to control flow, never an `IBin`). Operands are signed `int`
/// in this subset, so division and `>>` map to the signed opcodes.
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

/// The signed integer comparison predicate for a relational/equality operator,
/// or `None` for any other operator. `int` is signed, so orderings map to the
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
