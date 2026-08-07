//! The HIR data model (hir_design.org §3–§6): the frontend's terminal form.
//! A function body is a tree of *regions* whose leaves are flat, totally
//! ordered instruction sequences. Two value namespaces split the work:
//!
//! * *Temps* (`%n`) — single-assignment, and forbidden from crossing a
//!   control-flow join or loop back-edge (the verifier enforces both). They
//!   map 1:1 onto value-graph nodes later.
//! * *Locals* (`$n`) — explicit storage: every C object and every
//!   compiler-materialized slot (the `?:`/`&&` join slots). Anything that must
//!   cross control flow lives in a local, so HIR is phi-free by construction.

use std::fmt;

use crate::diagnostic::Span;
// The machine-type lattice and operator vocabulary are cross-stage
// (`src/types.rs`); re-exported so this module stays the one-stop import for
// HIR consumers.
pub use crate::types::{CastKind, IBinOp, IPred, Type, UbFlags};

/// A single-assignment temporary, `%n`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TempId(pub u32);

/// A storage slot, `$n`: an index into its function's `locals` table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LocalId(pub u32);

/// A region's identity, `r{n}` — the target namespace of `Break`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RegionId(pub u32);

/// A `goto` label, `L{n}`. Function-scoped, like C labels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LabelId(pub u32);

/// An HIR operand: an immediate, a temp, or a float immediate.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Value {
    Const(i64),
    Temp(TempId),
    /// A floating immediate (unused until FP lands — ME-6).
    #[allow(dead_code)]
    FConst(f64),
}

/// How an instruction interacts with the world — the *one* place effects are
/// classified (hir_design.org §8): the graph builder floats `Pure` ops and
/// chains the rest in program order. No pass may re-derive this by matching.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum Effect {
    Pure,
    Read,
    Write,
    // reads + writes + may not move: calls (and later asm)
    Opaque,
}

/// One instruction, carrying its source location (HIR-SRC-1) so the
/// diagnostic path and the codegen path share one data structure.
#[derive(Debug)]
pub struct Inst {
    pub kind: InstKind,
    pub span: Span,
}

impl Inst {
    /// The temp this instruction defines, if any.
    #[allow(dead_code)]
    pub fn dst(&self) -> Option<TempId> {
        match &self.kind {
            InstKind::IBin { dst, .. }
            | InstKind::ICmp { dst, .. }
            | InstKind::Convert { dst, .. }
            | InstKind::LoadLocal { dst, .. }
            | InstKind::AddrLocal { dst, .. }
            | InstKind::LoadPtr { dst, .. }
            | InstKind::Call { dst, .. } => Some(*dst),
            InstKind::StoreLocal { .. } | InstKind::StorePtr { .. } => None,
        }
    }
}

impl InstKind {
    #[allow(dead_code)]
    pub fn effect(&self) -> Effect {
        match self {
            InstKind::IBin { .. }
            | InstKind::ICmp { .. }
            | InstKind::Convert { .. }
            | InstKind::AddrLocal { .. } => Effect::Pure,
            InstKind::LoadLocal { .. } | InstKind::LoadPtr { .. } => Effect::Read,
            InstKind::StoreLocal { .. } | InstKind::StorePtr { .. } => Effect::Write,
            InstKind::Call { .. } => Effect::Opaque,
        }
    }
}

#[derive(Debug)]
pub enum InstKind {
    // `%dst = <op> <ty> lhs, rhs`
    IBin {
        dst: TempId,
        op: IBinOp,
        lhs: Value,
        rhs: Value,
        ty: Type,
        flags: UbFlags,
    },
    // `%dst = icmp <pred> <ty> lhs, rhs` — `ty` is the operand type; the
    // result is a 0/1 `int`.
    ICmp {
        dst: TempId,
        pred: IPred,
        lhs: Value,
        rhs: Value,
        ty: Type,
    },
    // `%dst = <kind> val` — the explicit value conversion (HIR-EXP-1); the
    // one instruction that changes a value's type/width (ME-4).
    Convert {
        dst: TempId,
        kind: CastKind,
        val: Value,
    },
    // `%dst = load <ty> $local` — an explicit lvalue-to-rvalue read
    // (HIR-EXP-2). Volatility is a property of the access (HIR-TY-2).
    LoadLocal {
        dst: TempId,
        local: LocalId,
        ty: Type,
        volatile: bool,
    },
    // `store <ty> val, $local` — every store is explicit; defines no temp.
    StoreLocal {
        local: LocalId,
        val: Value,
        ty: Type,
        volatile: bool,
    },
    // `%dst = addr $local` — the address of a local's slot: the pure value
    // behind `&x` (its emission marks the local address-taken).
    AddrLocal {
        dst: TempId,
        local: LocalId,
    },
    // `%dst = load <ty> [addr]` — a read through a pointer value.
    LoadPtr {
        dst: TempId,
        addr: Value,
        ty: Type,
        volatile: bool,
    },
    // `store <ty> val, [addr]` — a write through a pointer value.
    StorePtr {
        addr: Value,
        val: Value,
        ty: Type,
        volatile: bool,
    },
    // `%dst = call <ty> @callee(args...)` — language-level (hir_design.org
    // §9): args already explicitly converted; ABI slots come at the cliff.
    Call {
        dst: TempId,
        callee: String,
        args: Vec<Value>,
        ty: Type,
    },
}

/// One entry in a region's body. `Break` is the only structured-jump
/// primitive: it exits the targeted *enclosing* region (`continue` desugars to
/// a `Break` of the block wrapping the loop body). `goto`/labels stay direct —
/// C's goto is irreducible-capable, don't structure it (HIR-CF-1).
#[derive(Debug)]
pub enum Item {
    Inst(Inst),
    Region(Region),
    Break(RegionId),
    Goto(LabelId),
    Label(LabelId),
    // `case value:` — a dispatch target of the innermost enclosing switch.
    // Kept as a marker item, not a structured arm: C permits case labels at
    // any statement depth inside the body (Duff's device), and execution
    // falls through markers exactly like labels.
    Case { value: i64 },
    Default,
    Ret(Option<Value>),
}

#[derive(Debug)]
pub struct Region {
    pub id: RegionId,
    pub kind: RegionKind,
    pub span: Span,
}

#[derive(Debug)]
pub enum RegionKind {
    // a lexical scope; also `continue`'s desugar target inside loops
    Block { body: Vec<Item> },
    // the one canonical loop (design.org): repeats forever, exits only by
    // `Break` of its id; `while`/`do`/`for` are desugarings (§6.1)
    Loop { body: Vec<Item> },
    // both arms always exist (an absent `else` is an empty arm), so every
    // `if` has exactly two successors for elaboration and the graph
    If { cond: Value, then: Vec<Item>, els: Vec<Item> },
    // dispatches over the `Case`/`Default` markers in its body (at any
    // depth); `ty` is the discriminant's operand type; `Break(id)` exits
    Switch { ty: Type, scrut: Value, body: Vec<Item> },
}

/// One storage slot. The flags are per-declaration facts the middle-end needs
/// to *block promotion* (MIR-STR-4): both always false in the current subset.
#[derive(Debug)]
pub struct LocalDecl {
    pub ty: Type,
    #[allow(dead_code)]
    pub addr_taken: bool,
    #[allow(dead_code)]
    pub volatile: bool,
}

#[derive(Debug)]
pub struct Function {
    pub name: String,
    // the first `params` locals are the parameters, in order
    pub params: usize,
    pub ret_ty: Type,
    pub locals: Vec<LocalDecl>,
    // how many temps hir-gen handed out; elaboration numbers its own fresh
    // values from here
    pub temps: u32,
    pub body: Region,
}

#[derive(Debug)]
pub struct Program {
    pub funcs: Vec<Function>,
}

// ----- textual form (--dump-hir) -------------------------------------------

const INDENT: usize = 4;

impl fmt::Display for Program {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for func in &self.funcs {
            writeln!(f, "{func}")?;
        }
        Ok(())
    }
}

impl fmt::Display for Function {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "fn @{}(", self.name)?;
        for i in 0..self.params {
            if i > 0 {
                write!(f, ", ")?;
            }
            write!(f, "${i}: {}", self.locals[i].ty)?;
        }
        writeln!(f, ") -> {} {{", self.ret_ty)?;
        for (i, local) in self.locals.iter().enumerate().skip(self.params) {
            writeln!(f, "{:INDENT$}${i}: {}", "", local.ty)?;
        }
        fmt_region(f, &self.body, 1)?;
        write!(f, "\n}}")
    }
}

/// Print a region at `depth` (no trailing newline; the caller writes it).
fn fmt_region(f: &mut fmt::Formatter<'_>, region: &Region, depth: usize) -> fmt::Result {
    let pad = depth * INDENT;
    write!(f, "{:pad$}{}: ", "", region.id)?;
    match &region.kind {
        RegionKind::Block { body } => {
            writeln!(f, "block {{")?;
            fmt_items(f, body, depth + 1)?;
        }
        RegionKind::Loop { body } => {
            writeln!(f, "loop {{")?;
            fmt_items(f, body, depth + 1)?;
        }
        RegionKind::If { cond, then, els } => {
            writeln!(f, "if {cond} {{")?;
            fmt_items(f, then, depth + 1)?;
            if !els.is_empty() {
                writeln!(f, "{:pad$}}} else {{", "")?;
                fmt_items(f, els, depth + 1)?;
            }
        }
        RegionKind::Switch { ty, scrut, body } => {
            writeln!(f, "switch {ty} {scrut} {{")?;
            fmt_items(f, body, depth + 1)?;
        }
    }
    write!(f, "{:pad$}}}", "")
}

fn fmt_items(f: &mut fmt::Formatter<'_>, items: &[Item], depth: usize) -> fmt::Result {
    let pad = depth * INDENT;
    for item in items {
        match item {
            Item::Inst(inst) => writeln!(f, "{:pad$}{}", "", inst.kind)?,
            Item::Region(region) => {
                fmt_region(f, region, depth)?;
                writeln!(f)?;
            }
            Item::Break(r) => writeln!(f, "{:pad$}break {r}", "")?,
            Item::Goto(l) => writeln!(f, "{:pad$}goto {l}", "")?,
            // labels and case markers out-dent, like C convention
            Item::Label(l) => writeln!(f, "{:pad$}{l}:", "", pad = pad - INDENT / 2)?,
            Item::Case { value } => {
                writeln!(f, "{:pad$}case {value}:", "", pad = pad - INDENT / 2)?
            }
            Item::Default => writeln!(f, "{:pad$}default:", "", pad = pad - INDENT / 2)?,
            Item::Ret(None) => writeln!(f, "{:pad$}ret void", "")?,
            Item::Ret(Some(v)) => writeln!(f, "{:pad$}ret {v}", "")?,
        }
    }
    Ok(())
}

impl fmt::Display for TempId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "%{}", self.0)
    }
}

impl fmt::Display for LocalId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "${}", self.0)
    }
}

impl fmt::Display for RegionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "r{}", self.0)
    }
}

impl fmt::Display for LabelId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "L{}", self.0)
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Const(c) => write!(f, "{c}"),
            Value::Temp(t) => write!(f, "{t}"),
            Value::FConst(x) => write!(f, "{x}"),
        }
    }
}

impl fmt::Display for InstKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InstKind::IBin { dst, op, lhs, rhs, ty, .. } => {
                write!(f, "{dst} = {op} {ty} {lhs}, {rhs}")
            }
            InstKind::ICmp { dst, pred, lhs, rhs, ty } => {
                write!(f, "{dst} = icmp {pred} {ty} {lhs}, {rhs}")
            }
            InstKind::Convert { dst, kind, val } => write!(f, "{dst} = {kind} {val}"),
            InstKind::LoadLocal { dst, local, ty, volatile } => {
                let v = if *volatile { "volatile " } else { "" };
                write!(f, "{dst} = load {v}{ty} {local}")
            }
            InstKind::StoreLocal { local, val, ty, volatile } => {
                let v = if *volatile { "volatile " } else { "" };
                write!(f, "store {v}{ty} {val}, {local}")
            }
            InstKind::AddrLocal { dst, local } => write!(f, "{dst} = addr {local}"),
            InstKind::LoadPtr { dst, addr, ty, volatile } => {
                let v = if *volatile { "volatile " } else { "" };
                write!(f, "{dst} = load {v}{ty} [{addr}]")
            }
            InstKind::StorePtr { addr, val, ty, volatile } => {
                let v = if *volatile { "volatile " } else { "" };
                write!(f, "store {v}{ty} {val}, [{addr}]")
            }
            InstKind::Call { dst, callee, args, ty } => {
                write!(f, "{dst} = call {ty} @{callee}(")?;
                for (i, a) in args.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{a}")?;
                }
                write!(f, ")")
            }
        }
    }
}
