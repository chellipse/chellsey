use std::fmt;

use crate::diagnostic::Span;

/// The IR machine-type lattice. Unlike the sema `CType`, this is meant to be an
/// *open* set (ME-1 design update, MIR-TY-1): additions like `f80` or `_BitInt`
/// widths belong here and to their introducing lowering, and no pass elsewhere
/// should assume it is closed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum Type {
    Void,
    I8,
    I16,
    I32,
    I64,
    F32,
    F64,
    Ptr,
}

impl Type {
    pub fn size(&self) -> usize {
        match self {
            Type::Void => 0,
            Type::I8 => 1,
            Type::I16 => 2,
            Type::I32 | Type::F32 => 4,
            Type::I64 | Type::F64 | Type::Ptr => 8,
        }
    }

    pub fn is_float(&self) -> bool {
        matches!(self, Type::F32 | Type::F64)
    }
}

/// An IR operand: an immediate, a defined SSA register, or a float immediate.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Value {
    Const(i64),
    /// The result of the instruction that defined register `%n`.
    Reg(u32),
    /// A floating immediate (unused until FP lands — ME-6).
    #[allow(dead_code)]
    FConst(f64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BlockId(pub u32);

/// A local's stack slot in the memory form (MIR-STR-4): an index into its
/// function's slot table. `mem2reg` later promotes unescaped slots to SSA.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SlotId(pub u32);

/// Integer binary operators (ME-5). The full set is defined; only
/// `Add`/`Sub`/`Mul`/`SDiv`/`SRem` are produced by lowering in v1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum IBinOp {
    Add,
    Sub,
    Mul,
    SDiv,
    UDiv,
    SRem,
    URem,
    And,
    Or,
    Xor,
    Shl,
    AShr,
    LShr,
}

/// Integer comparison predicates (ME-5). The signed and equality predicates are
/// produced by lowering in v1; the unsigned orderings arrive with unsigned
/// types. Result of an `icmp` is a 0/1 `int`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum IPred {
    Eq,
    Ne,
    SLt,
    SLe,
    SGt,
    SGe,
    ULt,
    ULe,
    UGt,
    UGe,
}

/// Undefined-behaviour assumptions carried by an integer op (ME-5 design
/// update, HIR-UB-1). Reserved from day one so `-fwrapv` etc. can suppress a
/// flag *at generation time*; unused (all `false`) in v1.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[allow(dead_code)]
pub struct UbFlags {
    pub no_signed_wrap: bool,
    pub no_unsigned_wrap: bool,
    pub exact: bool,
}

/// One instruction, carrying a source location (ME-1 design update, MIR-MD-1)
/// so the diagnostic path and the codegen path share one data structure.
#[derive(Debug)]
pub struct Inst {
    pub kind: InstKind,
    pub span: Span,
}

impl Inst {
    /// The SSA register this instruction defines, if any. Lets the backend size
    /// the frame without matching each kind.
    pub fn dst(&self) -> Option<u32> {
        match &self.kind {
            InstKind::IBin { dst, .. }
            | InstKind::ICmp { dst, .. }
            | InstKind::Load { dst, .. } => Some(*dst),
            InstKind::Store { .. } => None,
        }
    }
}

#[derive(Debug)]
pub enum InstKind {
    // `%dst = <op> <ty> lhs, rhs`
    IBin {
        dst: u32,
        op: IBinOp,
        lhs: Value,
        rhs: Value,
        ty: Type,
        flags: UbFlags,
    },
    // `%dst = icmp <pred> <ty> lhs, rhs` — `ty` is the operand type; the
    // result is a 0/1 `int`.
    ICmp { dst: u32, pred: IPred, lhs: Value, rhs: Value, ty: Type },
    // `%dst = load <ty> $slot` — read a stack slot (MIR-STR-4 memory form).
    Load { dst: u32, slot: SlotId, ty: Type },
    // `store <ty> val, $slot` — write a stack slot; defines no register.
    Store { slot: SlotId, val: Value, ty: Type },
}

/// A basic block's exit. Defined as an enum so `Br`/`CondBr`/`Switch` slot in
/// later (ME-7) without reshaping.
#[derive(Debug)]
pub enum Terminator {
    Ret(Option<Value>),
    Br(BlockId),
    CondBr { cond: Value, then_bb: BlockId, else_bb: BlockId },
}

#[derive(Debug)]
pub struct Block {
    pub id: BlockId,
    pub insts: Vec<Inst>,
    pub term: Terminator,
}

#[derive(Debug)]
pub struct Function {
    pub name: String,
    pub params: Vec<Type>,
    pub ret_ty: Type,
    // the stack-slot table: one entry (the slot's type) per local (MIR-STR-4)
    pub slots: Vec<Type>,
    pub blocks: Vec<Block>,
}

/// A file-scope datum (global / string literal). Populated by ME-11; defined
/// here so `Program`'s shape is final.
#[derive(Debug)]
#[allow(dead_code)]
pub struct DataItem {
    pub name: String,
    pub bytes: Vec<u8>,
    pub align: usize,
    pub readonly: bool,
}

#[derive(Debug)]
pub struct Program {
    pub funcs: Vec<Function>,
    pub data: Vec<DataItem>,
}

// ----- textual form (--dump-ir) --------------------------------------------

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
        for (i, p) in self.params.iter().enumerate() {
            if i > 0 {
                write!(f, ", ")?;
            }
            write!(f, "{p}")?;
        }
        writeln!(f, ") -> {} {{", self.ret_ty)?;
        for (i, ty) in self.slots.iter().enumerate() {
            writeln!(f, "    ${i}: {ty}")?;
        }
        for block in &self.blocks {
            writeln!(f, "{block}")?;
        }
        write!(f, "}}")
    }
}

impl fmt::Display for Block {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "{}:", self.id)?;
        for inst in &self.insts {
            writeln!(f, "    {}", inst.kind)?;
        }
        write!(f, "    {}", self.term)
    }
}

impl fmt::Display for BlockId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "bb{}", self.0)
    }
}

impl fmt::Display for SlotId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "${}", self.0)
    }
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Type::Void => "void",
            Type::I8 => "i8",
            Type::I16 => "i16",
            Type::I32 => "i32",
            Type::I64 => "i64",
            Type::F32 => "f32",
            Type::F64 => "f64",
            Type::Ptr => "ptr",
        })
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Const(c) => write!(f, "{c}"),
            Value::Reg(id) => write!(f, "%{id}"),
            Value::FConst(x) => write!(f, "{x}"),
        }
    }
}

impl fmt::Display for IBinOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            IBinOp::Add => "add",
            IBinOp::Sub => "sub",
            IBinOp::Mul => "mul",
            IBinOp::SDiv => "sdiv",
            IBinOp::UDiv => "udiv",
            IBinOp::SRem => "srem",
            IBinOp::URem => "urem",
            IBinOp::And => "and",
            IBinOp::Or => "or",
            IBinOp::Xor => "xor",
            IBinOp::Shl => "shl",
            IBinOp::AShr => "ashr",
            IBinOp::LShr => "lshr",
        })
    }
}

impl fmt::Display for IPred {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            IPred::Eq => "eq",
            IPred::Ne => "ne",
            IPred::SLt => "slt",
            IPred::SLe => "sle",
            IPred::SGt => "sgt",
            IPred::SGe => "sge",
            IPred::ULt => "ult",
            IPred::ULe => "ule",
            IPred::UGt => "ugt",
            IPred::UGe => "uge",
        })
    }
}

impl fmt::Display for InstKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InstKind::IBin { dst, op, lhs, rhs, ty, .. } => {
                write!(f, "%{dst} = {op} {ty} {lhs}, {rhs}")
            }
            InstKind::ICmp { dst, pred, lhs, rhs, ty } => {
                write!(f, "%{dst} = icmp {pred} {ty} {lhs}, {rhs}")
            }
            InstKind::Load { dst, slot, ty } => write!(f, "%{dst} = load {ty} {slot}"),
            InstKind::Store { slot, val, ty } => write!(f, "store {ty} {val}, {slot}"),
        }
    }
}

impl fmt::Display for Terminator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Terminator::Ret(None) => f.write_str("ret void"),
            Terminator::Ret(Some(v)) => write!(f, "ret {v}"),
            Terminator::Br(bb) => write!(f, "br {bb}"),
            Terminator::CondBr { cond, then_bb, else_bb } => {
                write!(f, "condbr {cond}, {then_bb}, {else_bb}")
            }
        }
    }
}
