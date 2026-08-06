//! HIR — the frontend's terminal form (hir_design.org): a region tree with
//! flat, totally ordered, fully explicit instruction sequences. `build` makes
//! it from the sema-annotated AST; `ir::lower` elaborates it into the BB-form
//! IR; this module holds the data model, printer, builder, and verifier.

mod build;
mod types;
mod verify;

pub use build::build;
pub use types::*;
pub use verify::verify;
