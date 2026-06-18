use std::{fs::File, io::Read, path::PathBuf};

use clap::Parser;

pub mod lexer;

#[derive(Debug, Parser)]
struct Cli {
    input: Vec<PathBuf>,
}

fn main() {
    let cli = Cli::parse();
    for path in cli.input.iter() {
        let mut s = String::new();
        let mut f = File::open(path).unwrap();
        f.read_to_string(&mut s).unwrap();
        lexer::lex(&s);
    }
}
