//! Elaboration: HIR → the BB-form IR. Purely mechanical (hir_design.org §8.5):
//! regions become blocks, `Break` becomes a branch to the target region's exit
//! block, the canonical `loop` becomes a header block plus back edge, and a
//! `switch` region's `Case`/`Default` markers become the dispatch chain. All
//! desugaring already happened at hir-gen; elaborating verified HIR cannot
//! fail, so this stage is infallible — malformed input is a compiler bug the
//! HIR/IR verifiers catch on either side.

use std::collections::HashMap;

use super::types::*;
use crate::diagnostic::Span;
use crate::hir;

pub fn lower(prog: &hir::Program) -> Program {
    let funcs = prog.funcs.iter().map(function).collect();
    Program { funcs, data: Vec::new() }
}

fn function(func: &hir::Function) -> Function {
    // Entry block. Its terminator defaults to `ret void`, which an explicit
    // `Ret` item overwrites; `main`'s implicit `ret 0` is already explicit in
    // the HIR (hir-gen), so no special case survives to this stage.
    let entry = Block { id: BlockId(0), insts: Vec::new(), term: Terminator::Ret(None) };
    let mut e = Elab {
        blocks: vec![entry],
        current: 0,
        terminated: false,
        // fresh values (the switch dispatch compares) number after hir's temps
        next_reg: func.temps,
        exits: HashMap::new(),
        labels: HashMap::new(),
        switches: Vec::new(),
    };
    // A label may be a forward `goto` target, so give every one a block first.
    e.collect_labels(&func.body);
    e.region(&func.body);

    Function {
        name: func.name.clone(),
        params: func.locals[..func.params].iter().map(|l| l.ty).collect(),
        ret_ty: func.ret_ty,
        slots: func.locals.iter().map(|l| l.ty).collect(),
        blocks: e.blocks,
    }
}

// A switch region's dispatch table, gathered from its `Case`/`Default`
// markers (at any nesting depth) while its body is on the walk stack.
struct SwitchFrame {
    scrut: Value,
    ty: Type,
    span: Span,
    cases: Vec<(i64, BlockId)>,
    default: Option<BlockId>,
}

struct Elab {
    blocks: Vec<Block>,
    /// Index into `blocks` of the block instructions currently append to.
    current: usize,
    // Whether the current block already has a terminator, so unreachable
    // items are elaborated into dead blocks rather than dropped (they can
    // still contain jump targets).
    terminated: bool,
    next_reg: u32,
    // Region id → its exit block, created lazily by the first `Break` to it:
    // a region nothing breaks out of adds no block.
    exits: HashMap<hir::RegionId, BlockId>,
    labels: HashMap<hir::LabelId, BlockId>,
    // enclosing switch frames, innermost last
    switches: Vec<SwitchFrame>,
}

impl Elab {
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

    fn new_reg(&mut self) -> u32 {
        let r = self.next_reg;
        self.next_reg += 1;
        r
    }

    /// The exit block of a region, created on first demand.
    fn exit_for(&mut self, id: hir::RegionId) -> BlockId {
        if let Some(&bb) = self.exits.get(&id) {
            return bb;
        }
        let bb = self.new_block();
        self.exits.insert(id, bb);
        bb
    }

    /// Pre-create a block for every label in the region tree.
    fn collect_labels(&mut self, region: &hir::Region) {
        let (body, arms): (&[hir::Item], &[&[hir::Item]]) = match &region.kind {
            hir::RegionKind::Block { body }
            | hir::RegionKind::Loop { body }
            | hir::RegionKind::Switch { body, .. } => (body, &[]),
            hir::RegionKind::If { then, els, .. } => (&[], &[then, els]),
        };
        for item in body.iter().chain(arms.iter().flat_map(|a| a.iter())) {
            match item {
                hir::Item::Label(l) => {
                    let bb = self.new_block();
                    self.labels.insert(*l, bb);
                }
                hir::Item::Region(r) => self.collect_labels(r),
                _ => {}
            }
        }
    }

    fn region(&mut self, region: &hir::Region) {
        match &region.kind {
            hir::RegionKind::Block { body } => {
                self.items(body);
                self.end_region(region.id);
            }
            hir::RegionKind::Loop { body } => {
                // The canonical loop: one header the back edge re-enters. Its
                // exit test is an `If { els: [Break] }` inside the body.
                let header = self.new_block();
                if !self.terminated {
                    self.set_term(Terminator::Br(header));
                }
                self.switch_to(header);
                self.items(body);
                if !self.terminated {
                    self.set_term(Terminator::Br(header)); // the back edge
                }
                self.end_region(region.id);
            }
            hir::RegionKind::If { cond, then, els } => {
                let then_bb = self.new_block();
                let else_bb = self.new_block();
                let cont_bb = self.new_block();
                self.set_term(Terminator::CondBr { cond: value(cond), then_bb, else_bb });

                self.switch_to(then_bb);
                self.items(then);
                if !self.terminated {
                    self.set_term(Terminator::Br(cont_bb));
                }

                self.switch_to(else_bb);
                self.items(els);
                if !self.terminated {
                    self.set_term(Terminator::Br(cont_bb));
                }

                self.switch_to(cont_bb);
            }
            hir::RegionKind::Switch { ty, scrut, body } => {
                // The markers in the body register themselves in the frame as
                // they are reached; the dispatch chain is built afterwards.
                // Items before the first marker are unreachable but still
                // elaborated — into `body_entry`, which nothing branches to.
                let dispatch_bb = self.new_block();
                let exit_bb = self.exit_for(region.id); // `Break`'s target
                let body_entry = self.new_block();
                self.set_term(Terminator::Br(dispatch_bb));

                self.switch_to(body_entry);
                self.switches.push(SwitchFrame {
                    scrut: value(scrut),
                    ty: *ty,
                    span: region.span.clone(),
                    cases: Vec::new(),
                    default: None,
                });
                self.items(body);
                if !self.terminated {
                    self.set_term(Terminator::Br(exit_bb)); // fall off the end
                }
                let frame = self.switches.pop().unwrap();

                // The dispatch: compare the discriminant against each case
                // value in turn, falling to default (or the exit) if none match.
                self.switch_to(dispatch_bb);
                for (val, target) in &frame.cases {
                    let t = self.new_reg();
                    self.emit(
                        InstKind::ICmp {
                            dst: t,
                            pred: IPred::Eq,
                            lhs: frame.scrut,
                            rhs: Value::Const(*val),
                            ty: frame.ty,
                        },
                        &frame.span,
                    );
                    let next = self.new_block();
                    self.set_term(Terminator::CondBr {
                        cond: Value::Reg(t),
                        then_bb: *target,
                        else_bb: next,
                    });
                    self.switch_to(next);
                }
                self.set_term(Terminator::Br(frame.default.unwrap_or(exit_bb)));

                self.switch_to(exit_bb);
            }
        }
    }

    /// Close out a `Block`/`Loop` region: if anything broke out of it, route
    /// the fallthrough into its exit block and continue there; otherwise
    /// control just keeps flowing in the current block.
    fn end_region(&mut self, id: hir::RegionId) {
        if let Some(&exit) = self.exits.get(&id) {
            if !self.terminated {
                self.set_term(Terminator::Br(exit));
            }
            self.switch_to(exit);
        }
    }

    fn items(&mut self, items: &[hir::Item]) {
        for item in items {
            // Labels and case/default markers are entered by non-fallthrough
            // edges, so they reopen a terminated stream themselves; anything
            // else after a terminator is unreachable — but may still contain a
            // jump target, so it is elaborated into a fresh dead block.
            let is_target = matches!(
                item,
                hir::Item::Label(_) | hir::Item::Case { .. } | hir::Item::Default
            );
            if self.terminated && !is_target {
                let dead = self.new_block();
                self.switch_to(dead);
            }
            match item {
                hir::Item::Inst(inst) => self.inst(&inst.kind, &inst.span),
                hir::Item::Region(region) => self.region(region),
                hir::Item::Break(rid) => {
                    let exit = self.exit_for(*rid);
                    self.set_term(Terminator::Br(exit));
                }
                hir::Item::Goto(l) => {
                    let target = self.labels[l];
                    self.set_term(Terminator::Br(target));
                }
                hir::Item::Label(l) => {
                    let bb = self.labels[l];
                    // the preceding code falls into the label unless it jumped
                    if !self.terminated {
                        self.set_term(Terminator::Br(bb));
                    }
                    self.switch_to(bb);
                }
                hir::Item::Case { value } => {
                    let bb = self.new_block();
                    let frame = self
                        .switches
                        .last_mut()
                        .expect("the HIR verifier kept `case` inside a switch");
                    frame.cases.push((*value, bb));
                    // fall through from the preceding code into this case
                    if !self.terminated {
                        self.set_term(Terminator::Br(bb));
                    }
                    self.switch_to(bb);
                }
                hir::Item::Default => {
                    let bb = self.new_block();
                    let frame = self
                        .switches
                        .last_mut()
                        .expect("the HIR verifier kept `default` inside a switch");
                    frame.default = Some(bb);
                    if !self.terminated {
                        self.set_term(Terminator::Br(bb));
                    }
                    self.switch_to(bb);
                }
                hir::Item::Ret(v) => {
                    self.set_term(Terminator::Ret(v.as_ref().map(value)));
                }
            }
        }
    }

    // One HIR instruction is one IR instruction: temps are registers, locals
    // are stack slots. (The volatile flags have no BB-form home yet; they are
    // always false in the current subset.)
    fn inst(&mut self, kind: &hir::InstKind, span: &Span) {
        let kind = match kind {
            hir::InstKind::IBin { dst, op, lhs, rhs, ty, flags } => InstKind::IBin {
                dst: dst.0,
                op: *op,
                lhs: value(lhs),
                rhs: value(rhs),
                ty: *ty,
                flags: *flags,
            },
            hir::InstKind::ICmp { dst, pred, lhs, rhs, ty } => InstKind::ICmp {
                dst: dst.0,
                pred: *pred,
                lhs: value(lhs),
                rhs: value(rhs),
                ty: *ty,
            },
            hir::InstKind::Convert { dst, kind, val } => {
                InstKind::Convert { dst: dst.0, kind: *kind, val: value(val) }
            }
            hir::InstKind::LoadLocal { dst, local, ty, .. } => {
                InstKind::Load { dst: dst.0, slot: SlotId(local.0), ty: *ty }
            }
            hir::InstKind::StoreLocal { local, val, ty, .. } => {
                InstKind::Store { slot: SlotId(local.0), val: value(val), ty: *ty }
            }
            hir::InstKind::Call { dst, callee, args, ty } => InstKind::Call {
                dst: dst.0,
                callee: callee.clone(),
                args: args.iter().map(value).collect(),
                ty: *ty,
            },
        };
        self.emit(kind, span);
    }
}

fn value(v: &hir::Value) -> Value {
    match v {
        hir::Value::Const(c) => Value::Const(*c),
        hir::Value::Temp(t) => Value::Reg(t.0),
        hir::Value::FConst(x) => Value::FConst(*x),
    }
}
