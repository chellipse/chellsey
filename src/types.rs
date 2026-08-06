//! Cross-stage shared types: the machine-type lattice and the operator
//! vocabulary. HIR, the BB-form IR, and (later) the value graph all speak the
//! same `i32` / `add` / `icmp slt` / UB-flag language, so those definitions
//! live here once. Everything *stage-shaped* (regions, blocks, instructions,
//! functions) stays in its stage's module; a type belongs here exactly when a
//! second stage would otherwise re-declare it.

use std::fmt;

/// The machine-type lattice. Unlike the sema `CType`, this is meant to be an
/// *open* set (ME-1 design update, MIR-TY-1): additions like `f80`, `_BitInt`
/// widths, or aggregate blobs belong here and to their introducing lowering,
/// and no pass elsewhere should assume it is closed.
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

#[allow(dead_code)]
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

/// Integer binary operators (ME-5). The full set is defined; the unsigned
/// variants are produced once unsigned types land.
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
