//! A structural verifier for lowered IR (XC-2 "verification culture",
//! MIR-STR-1/2). It checks the invariants the lowerer is supposed to uphold, so
//! a bug in CFG construction or jump wiring is caught as a clear internal error
//! rather than as mysterious machine code. Failures are compiler bugs, not user
//! errors; the driver reports them as internal errors.
//!
//! Checked so far: block ids match their position, every branch target is a
//! real block, every register is assigned exactly once (SSA), no register is
//! used without a definition, and every load/store touches a slot the function
//! declared. Full dominance (a use is dominated by its def) needs a dominator
//! tree and is left for when value-carrying joins arrive.

use std::collections::HashSet;

use super::types::*;

pub fn verify(program: &Program) -> Result<(), String> {
    for func in &program.funcs {
        verify_fn(func).map_err(|e| format!("in @{}: {e}", func.name))?;
    }
    Ok(())
}

fn verify_fn(func: &Function) -> Result<(), String> {
    if func.blocks.is_empty() {
        return Err("function has no blocks".into());
    }
    // Parameters live in the first `params.len()` stack slots (the backend's
    // prologue spills the argument registers there).
    if func.slots.len() < func.params.len() {
        return Err(format!(
            "{} parameters but only {} stack slots",
            func.params.len(),
            func.slots.len()
        ));
    }

    // SSA: each register is defined by exactly one instruction.
    let mut defined = HashSet::new();
    for block in &func.blocks {
        for inst in &block.insts {
            if let Some(dst) = inst.dst()
                && !defined.insert(dst)
            {
                return Err(format!("register %{dst} defined more than once"));
            }
        }
    }

    let n = func.blocks.len();
    for (i, block) in func.blocks.iter().enumerate() {
        if block.id.0 as usize != i {
            return Err(format!("block {} is stored at index {i}", block.id));
        }
        for inst in &block.insts {
            for v in inst_uses(&inst.kind) {
                check_use(v, &defined)?;
            }
            if let Some(s) = inst_slot(&inst.kind)
                && s.0 as usize >= func.slots.len()
            {
                return Err(format!("access to nonexistent stack slot {s}"));
            }
        }
        for bb in term_targets(&block.term) {
            if bb.0 as usize >= n {
                return Err(format!("{} branches to nonexistent {bb}", block.id));
            }
        }
        for v in term_uses(&block.term) {
            check_use(v, &defined)?;
        }
    }
    Ok(())
}

fn check_use(v: &Value, defined: &HashSet<u32>) -> Result<(), String> {
    match v {
        Value::Reg(n) if !defined.contains(n) => Err(format!("use of undefined register %{n}")),
        _ => Ok(()),
    }
}

// The register operands an instruction reads. Exhaustive on purpose: a new
// `InstKind` must teach the verifier where its uses are.
fn inst_uses(kind: &InstKind) -> Vec<&Value> {
    match kind {
        InstKind::IBin { lhs, rhs, .. } | InstKind::ICmp { lhs, rhs, .. } => vec![lhs, rhs],
        InstKind::Load { .. } => vec![],
        InstKind::Store { val, .. } => vec![val],
    }
}

// The stack slot an instruction touches, if any. Exhaustive for the same reason.
fn inst_slot(kind: &InstKind) -> Option<SlotId> {
    match kind {
        InstKind::Load { slot, .. } | InstKind::Store { slot, .. } => Some(*slot),
        InstKind::IBin { .. } | InstKind::ICmp { .. } => None,
    }
}

fn term_uses(term: &Terminator) -> Vec<&Value> {
    match term {
        Terminator::Ret(v) => v.iter().collect(),
        Terminator::Br(_) => vec![],
        Terminator::CondBr { cond, .. } => vec![cond],
    }
}

fn term_targets(term: &Terminator) -> Vec<BlockId> {
    match term {
        Terminator::Ret(_) => vec![],
        Terminator::Br(bb) => vec![*bb],
        Terminator::CondBr { then_bb, else_bb, .. } => vec![*then_bb, *else_bb],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostic::Span;

    fn ibin(dst: u32, lhs: Value, rhs: Value) -> Inst {
        let kind = InstKind::IBin {
            dst,
            op: IBinOp::Add,
            lhs,
            rhs,
            ty: Type::I32,
            flags: UbFlags::default(),
        };
        Inst { kind, span: Span { start: 0, end: 0, src: 0 } }
    }

    fn func(slots: Vec<Type>, blocks: Vec<Block>) -> Program {
        let f = Function {
            name: "f".into(),
            params: vec![],
            ret_ty: Type::I32,
            slots,
            blocks,
        };
        Program { funcs: vec![f], data: vec![] }
    }

    #[test]
    fn accepts_well_formed() {
        let block = Block {
            id: BlockId(0),
            insts: vec![ibin(0, Value::Const(1), Value::Const(2))],
            term: Terminator::Ret(Some(Value::Reg(0))),
        };
        assert!(verify(&func(vec![], vec![block])).is_ok());
    }

    #[test]
    fn rejects_dangling_branch_target() {
        let block = Block {
            id: BlockId(0),
            insts: vec![],
            term: Terminator::Br(BlockId(7)),
        };
        assert!(verify(&func(vec![], vec![block])).is_err());
    }

    #[test]
    fn rejects_undefined_register_use() {
        let block = Block {
            id: BlockId(0),
            insts: vec![],
            term: Terminator::Ret(Some(Value::Reg(3))),
        };
        assert!(verify(&func(vec![], vec![block])).is_err());
    }

    #[test]
    fn rejects_double_definition() {
        let block = Block {
            id: BlockId(0),
            insts: vec![
                ibin(0, Value::Const(1), Value::Const(2)),
                ibin(0, Value::Const(3), Value::Const(4)),
            ],
            term: Terminator::Ret(None),
        };
        assert!(verify(&func(vec![], vec![block])).is_err());
    }

    #[test]
    fn accepts_slot_load_store() {
        let span = Span { start: 0, end: 0, src: 0 };
        let insts = vec![
            Inst {
                kind: InstKind::Store { slot: SlotId(0), val: Value::Const(7), ty: Type::I32 },
                span: span.clone(),
            },
            Inst {
                kind: InstKind::Load { dst: 0, slot: SlotId(0), ty: Type::I32 },
                span,
            },
        ];
        let block = Block {
            id: BlockId(0),
            insts,
            term: Terminator::Ret(Some(Value::Reg(0))),
        };
        assert!(verify(&func(vec![Type::I32], vec![block])).is_ok());
    }

    #[test]
    fn rejects_out_of_range_slot() {
        let inst = Inst {
            kind: InstKind::Store { slot: SlotId(1), val: Value::Const(7), ty: Type::I32 },
            span: Span { start: 0, end: 0, src: 0 },
        };
        let block = Block { id: BlockId(0), insts: vec![inst], term: Terminator::Ret(None) };
        assert!(verify(&func(vec![Type::I32], vec![block])).is_err());
    }
}
