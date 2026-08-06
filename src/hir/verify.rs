//! A structural verifier for HIR (XC-2 "verification culture"). It checks the
//! invariants hir-gen is supposed to uphold, so a desugaring bug is caught as
//! a clear internal error before elaboration. Failures are compiler bugs, not
//! user errors.
//!
//! Checked: region ids unique; labels unique and every `goto` resolved; every
//! temp defined exactly once, used only where its definition dominates in the
//! region tree (a def inside a region is invisible after that region exits —
//! the "temps never cross joins or back-edges" rule from hir_design.org §3);
//! `break` targets an enclosing region; `case`/`default` sit inside a switch,
//! with no duplicate values and at most one default per switch; every local
//! reference is in range.

use std::collections::HashSet;

use super::types::*;

pub fn verify(program: &Program) -> Result<(), String> {
    for func in &program.funcs {
        verify_fn(func).map_err(|e| format!("in @{}: {e}", func.name))?;
    }
    Ok(())
}

// Per-switch bookkeeping while its body is on the walk stack, innermost last.
struct SwitchFrame {
    default_seen: bool,
    cases: HashSet<i64>,
}

struct Checker<'a> {
    func: &'a Function,
    // whole-function single-assignment set
    defined: HashSet<TempId>,
    // visibility stack: temps defined per open region, innermost last
    scopes: Vec<HashSet<TempId>>,
    // enclosing region ids (`break` targets), innermost last
    enclosing: Vec<RegionId>,
    seen_regions: HashSet<RegionId>,
    labels: HashSet<LabelId>,
    switches: Vec<SwitchFrame>,
}

fn verify_fn(func: &Function) -> Result<(), String> {
    if func.params > func.locals.len() {
        return Err(format!(
            "{} parameters but only {} locals",
            func.params,
            func.locals.len()
        ));
    }
    let mut labels = HashSet::new();
    collect_labels(&func.body, &mut labels)?;
    let mut checker = Checker {
        func,
        defined: HashSet::new(),
        scopes: Vec::new(),
        enclosing: Vec::new(),
        seen_regions: HashSet::new(),
        labels,
        switches: Vec::new(),
    };
    checker.region(&func.body)
}

/// Collect every label in the tree (they have function scope, so a `goto` may
/// target one lowered later), diagnosing duplicates.
fn collect_labels(region: &Region, labels: &mut HashSet<LabelId>) -> Result<(), String> {
    let (items, arms): (&[Item], &[&[Item]]) = match &region.kind {
        RegionKind::Block { body } | RegionKind::Loop { body } => (body, &[]),
        RegionKind::If { then, els, .. } => (&[], &[then, els]),
        RegionKind::Switch { body, .. } => (body, &[]),
    };
    for item in items.iter().chain(arms.iter().flat_map(|a| a.iter())) {
        match item {
            Item::Label(l) => {
                if !labels.insert(*l) {
                    return Err(format!("label {l} defined more than once"));
                }
            }
            Item::Region(r) => collect_labels(r, labels)?,
            _ => {}
        }
    }
    Ok(())
}

impl Checker<'_> {
    fn region(&mut self, region: &Region) -> Result<(), String> {
        if !self.seen_regions.insert(region.id) {
            return Err(format!("region {} defined more than once", region.id));
        }
        self.enclosing.push(region.id);
        match &region.kind {
            RegionKind::Block { body } | RegionKind::Loop { body } => self.scoped(body)?,
            RegionKind::If { cond, then, els } => {
                self.uses(&[cond])?;
                // each arm is its own visibility scope: a def in one arm is
                // invisible in the other and after the join
                self.scoped(then)?;
                self.scoped(els)?;
            }
            RegionKind::Switch { scrut, body, .. } => {
                self.uses(&[scrut])?;
                self.switches
                    .push(SwitchFrame { default_seen: false, cases: HashSet::new() });
                self.scoped(body)?;
                self.switches.pop();
            }
        }
        self.enclosing.pop();
        Ok(())
    }

    /// Walk `items` in a fresh visibility scope; its temps die on exit.
    fn scoped(&mut self, items: &[Item]) -> Result<(), String> {
        self.scopes.push(HashSet::new());
        for item in items {
            self.item(item)?;
        }
        self.scopes.pop();
        Ok(())
    }

    fn item(&mut self, item: &Item) -> Result<(), String> {
        match item {
            Item::Inst(inst) => self.inst(&inst.kind),
            Item::Region(region) => self.region(region),
            Item::Break(r) => {
                if !self.enclosing.contains(r) {
                    return Err(format!("break {r} does not target an enclosing region"));
                }
                Ok(())
            }
            Item::Goto(l) => {
                if !self.labels.contains(l) {
                    return Err(format!("goto {l} targets no label"));
                }
                Ok(())
            }
            Item::Label(_) => Ok(()),
            Item::Case { value } => {
                let frame = self
                    .switches
                    .last_mut()
                    .ok_or_else(|| format!("case {value}: outside a switch"))?;
                if !frame.cases.insert(*value) {
                    return Err(format!("duplicate case {value}: in one switch"));
                }
                Ok(())
            }
            Item::Default => {
                let frame = self
                    .switches
                    .last_mut()
                    .ok_or_else(|| "default: outside a switch".to_string())?;
                if frame.default_seen {
                    return Err("duplicate default: in one switch".into());
                }
                frame.default_seen = true;
                Ok(())
            }
            Item::Ret(v) => self.uses(&v.iter().collect::<Vec<_>>()),
        }
    }

    fn inst(&mut self, kind: &InstKind) -> Result<(), String> {
        // uses are checked before the def lands: `%0 = add %0, 1` is invalid
        self.uses(&inst_uses(kind))?;
        if let Some(local) = inst_local(kind)
            && local.0 as usize >= self.func.locals.len()
        {
            return Err(format!("access to nonexistent local {local}"));
        }
        match kind {
            InstKind::IBin { dst, .. }
            | InstKind::ICmp { dst, .. }
            | InstKind::LoadLocal { dst, .. }
            | InstKind::Call { dst, .. } => self.define(*dst),
            InstKind::StoreLocal { .. } => Ok(()),
        }
    }

    fn define(&mut self, dst: TempId) -> Result<(), String> {
        if dst.0 >= self.func.temps {
            return Err(format!("temp {dst} outside the function's temp count"));
        }
        if !self.defined.insert(dst) {
            return Err(format!("temp {dst} defined more than once"));
        }
        self.scopes.last_mut().unwrap().insert(dst);
        Ok(())
    }

    fn uses(&self, values: &[&Value]) -> Result<(), String> {
        for v in values {
            if let Value::Temp(t) = v
                && !self.scopes.iter().any(|s| s.contains(t))
            {
                return Err(if self.defined.contains(t) {
                    format!("temp {t} used outside the region that defines it")
                } else {
                    format!("use of undefined temp {t}")
                });
            }
        }
        Ok(())
    }
}

// The temp operands an instruction reads. Exhaustive on purpose: a new
// `InstKind` must teach the verifier where its uses are.
fn inst_uses(kind: &InstKind) -> Vec<&Value> {
    match kind {
        InstKind::IBin { lhs, rhs, .. } | InstKind::ICmp { lhs, rhs, .. } => vec![lhs, rhs],
        InstKind::LoadLocal { .. } => vec![],
        InstKind::StoreLocal { val, .. } => vec![val],
        InstKind::Call { args, .. } => args.iter().collect(),
    }
}

// The local an instruction touches, if any. Exhaustive for the same reason.
fn inst_local(kind: &InstKind) -> Option<LocalId> {
    match kind {
        InstKind::LoadLocal { local, .. } | InstKind::StoreLocal { local, .. } => Some(*local),
        InstKind::IBin { .. } | InstKind::ICmp { .. } | InstKind::Call { .. } => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostic::Span;
    use crate::types::{IBinOp, UbFlags};

    fn span() -> Span {
        Span { start: 0, end: 0, src: 0 }
    }

    fn ibin(dst: u32, lhs: Value, rhs: Value) -> Item {
        let kind = InstKind::IBin {
            dst: TempId(dst),
            op: IBinOp::Add,
            lhs,
            rhs,
            ty: Type::I32,
            flags: UbFlags::default(),
        };
        Item::Inst(Inst { kind, span: span() })
    }

    fn block(id: u32, body: Vec<Item>) -> Region {
        Region {
            id: RegionId(id),
            kind: RegionKind::Block { body },
            span: span(),
        }
    }

    fn func(locals: usize, body: Region) -> Program {
        let locals = (0..locals)
            .map(|_| LocalDecl { ty: Type::I32, addr_taken: false, volatile: false })
            .collect();
        let f = Function {
            name: "f".into(),
            params: 0,
            ret_ty: Type::I32,
            locals,
            temps: 8,
            body,
        };
        Program { funcs: vec![f] }
    }

    #[test]
    fn accepts_straightline() {
        let store = InstKind::StoreLocal {
            local: LocalId(0),
            val: Value::Const(5),
            ty: Type::I32,
            volatile: false,
        };
        let load = InstKind::LoadLocal {
            dst: TempId(0),
            local: LocalId(0),
            ty: Type::I32,
            volatile: false,
        };
        let body = block(
            0,
            vec![
                Item::Inst(Inst { kind: store, span: span() }),
                Item::Inst(Inst { kind: load, span: span() }),
                ibin(1, Value::Temp(TempId(0)), Value::Const(1)),
                Item::Ret(Some(Value::Temp(TempId(1)))),
            ],
        );
        assert!(verify(&func(1, body)).is_ok());
    }

    #[test]
    fn rejects_temp_double_def() {
        let body = block(
            0,
            vec![
                ibin(0, Value::Const(1), Value::Const(2)),
                ibin(0, Value::Const(3), Value::Const(4)),
            ],
        );
        assert!(verify(&func(0, body)).is_err());
    }

    #[test]
    fn rejects_temp_escaping_region() {
        // %0 is defined inside a nested block and used after it exits: the
        // "temps never cross joins" rule.
        let inner = block(1, vec![ibin(0, Value::Const(1), Value::Const(2))]);
        let body = block(
            0,
            vec![Item::Region(inner), Item::Ret(Some(Value::Temp(TempId(0))))],
        );
        assert!(verify(&func(0, body)).is_err());
    }

    #[test]
    fn accepts_loop_with_break() {
        let looped = Region {
            id: RegionId(1),
            kind: RegionKind::Loop { body: vec![Item::Break(RegionId(1))] },
            span: span(),
        };
        let body = block(
            0,
            vec![Item::Region(looped), Item::Ret(Some(Value::Const(0)))],
        );
        assert!(verify(&func(0, body)).is_ok());
    }

    #[test]
    fn rejects_break_to_non_enclosing() {
        let body = block(0, vec![Item::Break(RegionId(7))]);
        assert!(verify(&func(0, body)).is_err());
    }

    #[test]
    fn rejects_goto_missing_label() {
        let body = block(0, vec![Item::Goto(LabelId(0))]);
        assert!(verify(&func(0, body)).is_err());
    }

    #[test]
    fn rejects_case_outside_switch() {
        let body = block(0, vec![Item::Case { value: 1 }]);
        assert!(verify(&func(0, body)).is_err());
    }

    #[test]
    fn accepts_switch_with_cases() {
        let switch = Region {
            id: RegionId(1),
            kind: RegionKind::Switch {
                ty: Type::I32,
                scrut: Value::Const(3),
                body: vec![
                    Item::Case { value: 1 },
                    Item::Break(RegionId(1)),
                    Item::Default,
                    Item::Break(RegionId(1)),
                ],
            },
            span: span(),
        };
        let body = block(
            0,
            vec![Item::Region(switch), Item::Ret(Some(Value::Const(0)))],
        );
        assert!(verify(&func(0, body)).is_ok());
    }
}
