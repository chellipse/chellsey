use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Type {
    Void,
    I32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Value {
    Const(i64),
    /// The result of the instruction that defined value `%n` (SSA).
    Reg(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BlockId(pub usize);

#[derive(Debug)]
pub struct Program {
    pub functions: Vec<Function>,
}

#[derive(Debug)]
pub struct Function {
    pub name: String,
    pub ret_ty: Type,
    pub blocks: Vec<BasicBlock>,
}

#[derive(Debug)]
pub struct BasicBlock {
    pub id: BlockId,
    pub insts: Vec<Inst>,
    pub term: Terminator,
}

#[derive(Debug)]
pub enum Inst {
    /// `%dst = add lhs, rhs`
    Add { dst: usize, lhs: Value, rhs: Value },
}

#[derive(Debug)]
pub enum Terminator {
    Ret(Option<Value>),
}

impl fmt::Display for Program {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for func in &self.functions {
            writeln!(f, "{func}")?;
        }
        Ok(())
    }
}

impl fmt::Display for Function {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "fn @{}() -> {} {{", self.name, self.ret_ty)?;
        for block in &self.blocks {
            writeln!(f, "{block}")?;
        }
        write!(f, "}}")
    }
}

impl fmt::Display for BasicBlock {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "{}:", self.id)?;
        for inst in &self.insts {
            writeln!(f, "    {inst}")?;
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
            Type::I32 => "i32",
        })
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Const(c) => write!(f, "{c}"),
            Value::Reg(id) => write!(f, "%{id}"),
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

impl fmt::Display for Inst {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Inst::Add { dst, lhs, rhs } => write!(f, "%{dst} = add {lhs}, {rhs}"),
        }
    }
}
