use super::types::*;
use crate::ast::{Expr, ExprKind, ExtDeclKind, Stmt, StmtKind, TranslationUnit};

pub fn lower(unit: &TranslationUnit) -> Program {
    let mut functions = Vec::new();
    for decl in &unit.decls {
        match &decl.kind {
            ExtDeclKind::FuncDef { type_spec, ident, body } => {
                functions.push(lower_function(ident, type_spec, body));
            }
            // TODO: lower a file-scope Decl into a global.
            ExtDeclKind::Decl { .. } => {}
        }
    }
    Program { functions }
}

fn lower_function(name: &str, type_spec: &[String], body: &Stmt) -> Function {
    let mut b = FnBuilder::new(name.to_string(), lower_type(type_spec));
    b.stmt(body);
    b.finish()
}

struct FnBuilder {
    name: String,
    ret_ty: Type,
    blocks: Vec<BasicBlock>,
    current: usize,
}

impl FnBuilder {
    fn new(name: String, ret_ty: Type) -> Self {
        // Entry block. Its terminator defaults to an implicit `ret void`, which
        // an explicit `return` overwrites (and which covers an empty body).
        let entry = BasicBlock { id: BlockId(0), insts: Vec::new(), term: Terminator::Ret(None) };
        Self { name, ret_ty, blocks: vec![entry], current: 0 }
    }

    fn stmt(&mut self, stmt: &Stmt) {
        match &stmt.kind {
            StmtKind::Compound(stmts) => {
                for s in stmts {
                    self.stmt(s);
                }
            }
            StmtKind::Return(expr) => {
                let val = expr.as_ref().map(lower_expr);
                self.blocks[self.current].term = Terminator::Ret(val);
            }
        }
    }

    fn finish(self) -> Function {
        Function { name: self.name, ret_ty: self.ret_ty, blocks: self.blocks }
    }
}

fn lower_expr(expr: &Expr) -> Value {
    match &expr.kind {
        // widening the u64 literal into the operand's i64 store is fine for the
        // constants we currently accept.
        ExprKind::IntLit(v) => Value::Const(*v as i64),
    }
}

fn lower_type(spec: &[String]) -> Type {
    match spec
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        ["void"] => Type::Void,
        _ => Type::I32,
    }
}
