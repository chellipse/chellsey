use std::path::PathBuf;

use clap::Parser;

pub mod ast;
pub mod lexer;

#[derive(Debug, Parser)]
struct Cli {
    input: Vec<PathBuf>,
}

fn main() {
    let cli = Cli::parse();
    for path in cli.input.iter() {
        let tokens = lexer::Lexer::new().lex(path).unwrap();

        println!("Tokens: {:?}", &tokens);

        let result = ast::Parser::new(tokens).parse();

        println!("Result: {:?}", &result);
    }
}
