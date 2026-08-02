use std::path::{Path, PathBuf};

use clap::Parser;

pub mod ast;
pub mod codegen;
pub mod diagnostic;
pub mod ir;
pub mod lexer;
pub mod sema;

use diagnostic::SourceManager;

#[derive(Debug, Parser)]
struct Cli {
    input: Vec<PathBuf>,
}

fn main() {
    let cli = Cli::parse();
    let sources = SourceManager::new();
    let mut failed = false;
    for path in cli.input.iter() {
        if let Err(e) = compile(&sources, path) {
            // A span-carrying error resolves against the source store;
            // anything else (e.g. a failed file open) has no source location.
            match e.downcast_ref::<diagnostic::Error>() {
                Some(diag) => sources.report(diag),
                None => eprintln!("error: {e:?}"),
            }
            failed = true;
        }
    }
    if failed {
        std::process::exit(1);
    }
}

fn compile(sources: &SourceManager, path: &Path) -> anyhow::Result<()> {
    let tokens = lexer::Lexer::new(sources).lex(path)?;
    println!("Tokens: {:?}", &tokens);

    let mut ast = ast::Parser::new(tokens).parse()?;
    println!("AST: {:?}", &ast);

    // Sema annotates the AST (pass 2 fills each expression's type) and hands the
    // middle-end a `ProgramInfo`; the AST is frozen afterwards.
    let info = sema::Sema::new().analyze(&mut ast)?;

    let ir = ir::lower(&ast, &info)?;
    println!("IR:\n{ir}");

    // Malformed IR is a compiler bug, not a user error — surface it loudly.
    ir::verify(&ir).map_err(|e| anyhow::anyhow!("internal compiler error: invalid IR: {e}"))?;

    println!("ASM:\n{}", codegen::assembly(&ir)?);

    let obj = codegen::emit_object(&ir)?;
    let out = path.with_extension("o");
    std::fs::write(&out, obj)?;
    println!("wrote {}", out.display());

    Ok(())
}
