use anyhow::anyhow;

use super::types::*;
use crate::ast::{
    BinOp, CType, Expr, ExprKind, ExtDeclKind, Stmt, StmtKind, TranslationUnit, UnOp,
};
use crate::diagnostic::{Error, Span};
use crate::sema::ProgramInfo;

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
            ExtDeclKind::FuncDef { ident, body, .. } => {
                funcs.push(low.function(ident, body)?);
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
    fn function(&self, name: &str, body: &Stmt) -> Result<Function> {
        let sig = self
            .info
            .funcs
            .get(name)
            .expect("sema collected every defined function's signature");
        let params = sig.params.iter().map(ir_ty).collect();
        let ret_ty = ir_ty(&sig.ret);

        let mut b = FnBuilder::new(name.to_string(), params, ret_ty);
        b.stmt(body)?;
        // ME-12: `main` falling off the end returns 0.
        if name == "main" {
            b.implicit_return_zero();
        }
        Ok(b.finish())
    }
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
        }
    }

    fn new_reg(&mut self) -> u32 {
        let r = self.next_reg;
        self.next_reg += 1;
        r
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
            blocks: self.blocks,
        }
    }

    fn stmt(&mut self, stmt: &Stmt) -> Result<()> {
        match &stmt.kind {
            StmtKind::Compound(stmts) => {
                for s in stmts {
                    self.stmt(s)?;
                }
                Ok(())
            }
            StmtKind::Return(expr) => {
                // ME-12: the value is converted to the declared return type. In
                // this all-`int` subset the conversion is the identity.
                let val = match expr {
                    Some(e) => Some(self.expr(e)?.val),
                    None => None,
                };
                self.blocks[self.current].term = Terminator::Ret(val);
                Ok(())
            }
            StmtKind::Empty => Ok(()),
        }
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
                    UnOp::Not => Err(err(
                        &e.span,
                        "this unary operator is not yet supported (TBD)",
                    )),
                }
            }
            ExprKind::Binary { op, lhs, rhs } => {
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
                let ir_op = match op {
                    BinOp::Add => IBinOp::Add,
                    BinOp::Sub => IBinOp::Sub,
                    BinOp::Mul => IBinOp::Mul,
                    BinOp::Div => IBinOp::SDiv,
                    BinOp::Rem => IBinOp::SRem,
                    BinOp::BitAnd => IBinOp::And,
                    BinOp::BitOr => IBinOp::Or,
                    BinOp::BitXor => IBinOp::Xor,
                    BinOp::Shl => IBinOp::Shl,
                    // `int` is signed, so `>>` is an arithmetic shift.
                    BinOp::Shr => IBinOp::AShr,
                    _ => return Err(err(&e.span, "this operator is not yet supported (TBD)")),
                };
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
