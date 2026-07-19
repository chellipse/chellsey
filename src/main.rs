use std::path::{Path, PathBuf};

use clap::Parser;

pub mod ast;
pub mod diagnostic;
pub mod lexer;

use diagnostic::SourceManager;

#[derive(Debug, Parser)]
struct Cli {
    input: Vec<PathBuf>,
}

fn main() {
    let cli = Cli::parse();
    let sources = SourceManager::new();
    for path in cli.input.iter() {
        if let Err(e) = compile(&sources, path) {
            // A span-carrying error resolves against the source store;
            // anything else (e.g. a failed file open) has no source location.
            match e.downcast_ref::<diagnostic::Error>() {
                Some(diag) => sources.report(diag),
                None => eprintln!("error: {e:?}"),
            }
        }
    }
}

fn compile(sources: &SourceManager, path: &Path) -> anyhow::Result<()> {
    let tokens = lexer::Lexer::new(sources).lex(path)?;
    println!("Tokens: {:?}", &tokens);

    let ast = ast::Parser::new(tokens).parse()?;
    println!("Result: {:?}", &ast);

    Ok(())
}
