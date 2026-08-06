use std::path::{Path, PathBuf};

use clap::Parser;

pub mod ast;
pub mod codegen;
pub mod diagnostic;
pub mod hir;
pub mod ir;
pub mod lexer;
pub mod sema;
pub mod types;

use diagnostic::SourceManager;

#[derive(Debug, Parser)]
struct Cli {
    input: Vec<PathBuf>,
    // Object output path; defaults to `<input>.o`. Only valid with one input.
    #[arg(short = 'o')]
    output: Option<PathBuf>,
}

fn main() {
    let cli = Cli::parse();
    if cli.output.is_some() && cli.input.len() > 1 {
        eprintln!("error: -o cannot be given with multiple input files");
        std::process::exit(1);
    }
    let sources = SourceManager::new();
    let mut failed = false;
    for path in cli.input.iter() {
        let out = cli
            .output
            .clone()
            .unwrap_or_else(|| path.with_extension("o"));
        if let Err(e) = compile(&sources, path, &out) {
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

fn compile(sources: &SourceManager, path: &Path, out: &Path) -> anyhow::Result<()> {
    let tokens = lexer::Lexer::new(sources).lex(path)?;
    println!("Tokens:\n{tokens:?}\n");

    let mut ast = ast::Parser::new(tokens).parse()?;
    println!("AST:\n{ast:?}\n");

    // Sema annotates the AST (pass 2 fills each expression's type) and hands
    // hir-gen a `ProgramInfo`; the AST is frozen afterwards.
    let info = sema::Sema::new().analyze(&mut ast)?;

    // The frontend's terminal form: fully desugared and explicit (hir_design.org).
    let hir = hir::build(&ast, &info)?;
    println!("HIR:\n{hir}\n");

    // Malformed HIR/IR is a compiler bug, not a user error — surface it loudly.
    hir::verify(&hir).map_err(|e| anyhow::anyhow!("internal compiler error: invalid HIR: {e}"))?;

    let ir = ir::lower(&hir);
    println!("IR:\n{ir}\n");

    ir::verify(&ir).map_err(|e| anyhow::anyhow!("internal compiler error: invalid IR: {e}"))?;

    println!("ASM:\n{}\n", codegen::assembly(&ir)?);

    let obj = codegen::emit_object(&ir)?;
    std::fs::write(out, obj)?;
    println!("wrote {}", out.display());

    Ok(())
}
