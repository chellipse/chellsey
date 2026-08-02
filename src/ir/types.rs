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
            InstKind::IBin { dst, .. } => Some(*dst),
        }
    }
}

#[derive(Debug)]
pub enum InstKind {
    /// `%dst = <op> <ty> lhs, rhs`
    IBin {
        dst: u32,
        op: IBinOp,
        lhs: Value,
        rhs: Value,
        ty: Type,
        flags: UbFlags,
    },
}

/// A basic block's exit. Defined as an enum so `Br`/`CondBr`/`Switch` slot in
/// later (ME-7) without reshaping.
#[derive(Debug)]
pub enum Terminator {
    Ret(Option<Value>),
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

impl fmt::Display for InstKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InstKind::IBin { dst, op, lhs, rhs, ty, .. } => {
                write!(f, "%{dst} = {op} {ty} {lhs}, {rhs}")
            }
        }
    }
}

impl fmt::Display for Terminator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Terminator::Ret(None) => f.write_str("ret void"),
            Terminator::Ret(Some(v)) => write!(f, "ret {v}"),
        }
    }
}
