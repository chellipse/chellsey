use crate::diagnostic::Span;

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

#[derive(Debug)]
pub enum ExtDeclKind {
    FuncDef {
        type_spec: Vec<String>,
        ident: String,
        body: Stmt,
    },
    Decl {
        type_spec: Vec<String>,
        ident: String,
    },
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
}

#[derive(Debug)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}

#[derive(Debug)]
pub enum ExprKind {
    IntLit(u64),
    Add(Box<Expr>, Box<Expr>),
}
