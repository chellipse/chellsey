use std::fmt;

use crate::diagnostic::Span;
use crate::lexer::{IntLen, IntSuf};

/// The sema-level C type model (x86-64 System V layout). Kept *language-level*
/// and closed/small for v1: qualifiers, `typeof`, etc. are resolved before a
/// `CType` is formed. This is deliberately *not* the IR type — the extensible
/// machine-type lattice lives in `ir::types` (FE-1 design update).
///
/// The full base-type set is defined up front; the parser only builds the
/// variants it can lower today (`int`/`void`, plus the integer variants the
/// literal-suffix machinery produces), and the rest are wired incrementally.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub enum CType {
    Void,
    Bool,
    Char { signed: bool },
    Short { signed: bool },
    // `long` == `long long` == 8 bytes in this model.
    Int { signed: bool },
    Long { signed: bool },
    Float,
    Double,
    Ptr(Box<CType>),
    Array(Box<CType>, usize),
}

#[allow(dead_code)]
impl CType {
    pub const INT: CType = CType::Int { signed: true };
    pub const CHAR: CType = CType::Char { signed: true };
    pub const ULONG: CType = CType::Long { signed: false };

    /// Size in bytes (System V x86-64).
    pub fn size(&self) -> usize {
        match self {
            CType::Void => 0,
            CType::Bool | CType::Char { .. } => 1,
            CType::Short { .. } => 2,
            CType::Int { .. } | CType::Float => 4,
            CType::Long { .. } | CType::Double | CType::Ptr(_) => 8,
            CType::Array(elem, n) => elem.size() * n,
        }
    }

    /// Alignment in bytes: a scalar aligns to its size; an array aligns to its
    /// element's alignment.
    pub fn align(&self) -> usize {
        match self {
            CType::Array(elem, _) => elem.align(),
            other => other.size().max(1),
        }
    }

    pub fn is_integer(&self) -> bool {
        matches!(
            self,
            CType::Bool
                | CType::Char { .. }
                | CType::Short { .. }
                | CType::Int { .. }
                | CType::Long { .. }
        )
    }

    pub fn is_float(&self) -> bool {
        matches!(self, CType::Float | CType::Double)
    }

    pub fn is_arith(&self) -> bool {
        self.is_integer() || self.is_float()
    }

    /// Signedness of an integer type; non-integers (and `bool`) report `false`.
    pub fn is_signed(&self) -> bool {
        match self {
            CType::Char { signed }
            | CType::Short { signed }
            | CType::Int { signed }
            | CType::Long { signed } => *signed,
            _ => false,
        }
    }

    /// The referenced element type of a pointer or array, if any.
    pub fn pointee(&self) -> Option<&CType> {
        match self {
            CType::Ptr(inner) | CType::Array(inner, _) => Some(inner),
            _ => None,
        }
    }

    /// The integer promotions (6.3.1.1p2): types of rank below `int` promote
    /// to `int` (every value of `bool`/`char`/`short` fits). Applied to each
    /// operand of arithmetic, and alone for `~`/unary `-` and shift operands.
    pub fn promote(&self) -> CType {
        match self {
            CType::Bool | CType::Char { .. } | CType::Short { .. } => CType::INT,
            other => other.clone(),
        }
    }

    /// The usual arithmetic conversions (6.3.1.8) for two integer operands:
    /// promote both, then the common type is the higher rank; at equal rank
    /// with mixed signedness the unsigned type wins. (`long` is 8 bytes and
    /// `int` 4 in this model, so a signed `long` represents every `unsigned
    /// int` — the mixed-rank case never needs the unsigned-of-higher fallback.)
    pub fn usual_arith(a: &CType, b: &CType) -> CType {
        let (a, b) = (a.promote(), b.promote());
        if a == b {
            return a;
        }
        let long = |t: &CType| matches!(t, CType::Long { .. });
        match (long(&a), long(&b)) {
            (true, false) => a,
            (false, true) => b,
            // equal rank, signedness differs (equal types returned above)
            (false, false) => CType::Int { signed: false },
            (true, true) => CType::Long { signed: false },
        }
    }
}

impl fmt::Display for CType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CType::Void => f.write_str("void"),
            CType::Bool => f.write_str("bool"),
            CType::Char { signed: true } => f.write_str("char"),
            CType::Char { signed: false } => f.write_str("unsigned char"),
            CType::Short { signed: true } => f.write_str("short"),
            CType::Short { signed: false } => f.write_str("unsigned short"),
            CType::Int { signed: true } => f.write_str("int"),
            CType::Int { signed: false } => f.write_str("unsigned int"),
            CType::Long { signed: true } => f.write_str("long"),
            CType::Long { signed: false } => f.write_str("unsigned long"),
            CType::Float => f.write_str("float"),
            CType::Double => f.write_str("double"),
            CType::Ptr(inner) => write!(f, "{inner} *"),
            CType::Array(inner, n) => write!(f, "{inner}[{n}]"),
        }
    }
}

#[derive(Debug)]
pub struct TranslationUnit {
    pub decls: Vec<ExtDecl>,
    pub span: Span,
}

#[derive(Debug)]
pub struct ExtDecl {
    pub kind: ExtDeclKind,
    pub span: Span,
}

/// One formal parameter of a function declarator. The name is optional in a
/// prototype (`int f(int);`); sema requires it in a definition.
#[derive(Debug)]
pub struct Param {
    pub ty: CType,
    pub name: Option<String>,
    pub span: Span,
}

#[derive(Debug)]
pub enum ExtDeclKind {
    /// `ret ident(params) { body }`
    FuncDef {
        ret: CType,
        ident: String,
        params: Vec<Param>,
        varargs: bool,
        body: Stmt,
    },
    /// `ret ident(params);`
    FuncDecl { ret: CType, ident: String, params: Vec<Param>, varargs: bool },
}

#[derive(Debug)]
pub struct Stmt {
    pub kind: StmtKind,
    pub span: Span,
}

#[derive(Debug)]
pub enum StmtKind {
    Compound(Vec<Stmt>),
    Return(Option<Expr>),
    If {
        cond: Expr,
        then: Box<Stmt>,
        els: Option<Box<Stmt>>,
    },
    // structured on purpose (FE-18 / HIR-CF-1): loops are not pre-lowered
    While {
        cond: Expr,
        body: Box<Stmt>,
    },
    // `do body while (cond);` — the body runs once before the first test;
    // `continue` targets the condition (6.8.6.2).
    DoWhile {
        body: Box<Stmt>,
        cond: Expr,
    },
    // `for (init; cond; step) body` — absent clauses are None; `init` is a
    // declaration or an expression statement; an absent cond is always true.
    For {
        init: Option<Box<Stmt>>,
        cond: Option<Expr>,
        step: Option<Expr>,
        body: Box<Stmt>,
    },
    // `switch (disc) body` — kept structured (HIR-CF-3); the case/default
    // labels live inside `body` and are matched against `disc` at lowering.
    Switch {
        disc: Expr,
        body: Box<Stmt>,
    },
    // `case value: body` — a labeled statement; `value` is an integer constant
    // expression. Multiple statements after it are siblings reached by fallthrough.
    Case {
        value: Expr,
        body: Box<Stmt>,
    },
    // `default: body` — the switch's fall-through target when no case matches.
    Default {
        body: Box<Stmt>,
    },
    // loop/switch jumps; sema checks `break` sits inside a loop or switch and
    // `continue` inside a loop.
    Break,
    Continue,
    // `goto label;` — an unconditional jump to a function-scoped label (6.8.6.1).
    Goto {
        label: String,
    },
    // `label: body` — a labeled statement; the label has function scope and may
    // be the target of a `goto` that appears before it (a forward jump).
    Label {
        name: String,
        body: Box<Stmt>,
    },
    // `ty name [= init];` — one declarator per declaration in this subset
    // (FE-16's declarator lists widen this to a `Vec` later).
    Decl {
        ty: CType,
        name: String,
        init: Option<Expr>,
    },
    // an expression evaluated for its side effects (`x = 5;`)
    Expr(Expr),
    Empty,
}

#[derive(Debug)]
pub struct Expr {
    pub kind: ExprKind,
    /// The expression's type, computed and filled in by sema's pass 2 (§2.7:
    /// sema annotates, the middle-end consumes). `None` until then.
    pub ty: Option<CType>,
    pub span: Span,
}

#[derive(Debug)]
pub enum ExprKind {
    // The literal's own type (from its suffix) rides along in `ty`; the outer
    // `Expr.ty` slot is sema's annotation (identical here, distinct in general).
    IntLit { value: u64, ty: CType },
    // a use of a declared name; sema checks that it resolves
    Ident { name: String },
    Unary { op: UnOp, expr: Box<Expr> },
    Binary { op: BinOp, lhs: Box<Expr>, rhs: Box<Expr> },
    // simple assignment `lhs = rhs`; compound assignment stays a surface node
    // of its own when it lands (HIR-EXP-3 desugars once, not the parser).
    Assign { lhs: Box<Expr>, rhs: Box<Expr> },
    // `lhs op= rhs` — modify-in-place carrying the underlying binary operator
    // `op` (6.5.16.2). Lowering desugars it to a load-op-store on the lvalue's
    // slot, with `lhs` evaluated once.
    CompoundAssign { op: BinOp, lhs: Box<Expr>, rhs: Box<Expr> },
    // `++expr` / `expr++` (6.5.3.1 / 6.5.2.4): `pre` is prefix vs postfix,
    // `inc` is `++` vs `--`. A load-op-store like compound assignment; prefix
    // yields the new value, postfix the old.
    IncDec { pre: bool, inc: bool, expr: Box<Expr> },
    // `lhs , rhs` (6.5.17): evaluate `lhs` for its side effects and discard
    // it (a sequence point follows), then the result is `rhs`.
    Comma { lhs: Box<Expr>, rhs: Box<Expr> },
    // `cond ? then : els` — only the taken arm evaluates (6.5.15)
    Cond { cond: Box<Expr>, then: Box<Expr>, els: Box<Expr> },
    // `callee(args)` — a direct call to a named function (6.5.2.2); calls
    // through a function-pointer expression are a later item.
    Call { callee: String, args: Vec<Expr> },
}

/// Binary operators (FE-19). The full set is defined; only `+ - * / %` are
/// wired through sema/lowering in v1, the rest are clean TBD diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Shl,
    Shr,
    Lt,
    Gt,
    Le,
    Ge,
    Eq,
    Ne,
    BitAnd,
    BitOr,
    BitXor,
    LogAnd,
    LogOr,
}

/// Prefix operators that fold into a `Unary` node (FE-19). `Neg` is wired;
/// `Not`/`BitNot` parse but are TBD in sema. `*`/`&`/`++`/`--` are distinct
/// node kinds (not yet in this subset), so they TBD in the parser.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnOp {
    Neg,
    Not,
    BitNot,
}

/// Pick the type of an integer literal from its suffix (6.4.4.1). Magnitude
/// escalation (a value too large for the suffixed type widening to the next
/// rank) is a later refinement; the suffix alone drives the type for now.
pub fn int_lit(value: u64, suf: IntSuf) -> ExprKind {
    let signed = !suf.unsigned;
    let ty = match suf.len {
        IntLen::Int => CType::Int { signed },
        // `long` and `long long` share one 8-byte model.
        IntLen::Long | IntLen::LongLong => CType::Long { signed },
    };
    ExprKind::IntLit { value, ty }
}
