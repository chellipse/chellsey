#[derive(Debug)]
pub struct TranslationUnit {
    pub decls: Vec<ExtDecl>,
}

#[derive(Debug)]
pub enum ExtDecl {
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
pub enum Stmt {
    Compound(Vec<Stmt>),
    Return(Option<Expr>),
}

#[derive(Debug)]
pub enum Expr {
    IntLit(u64),
}
