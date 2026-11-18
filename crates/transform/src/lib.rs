//! Data transformations (Transform menu): Compute Variable with SPSS expression syntax.
mod compute;
mod eval;
mod functions;
mod lexer;
mod parser;

pub use compute::{ComputeReport, ComputeSpec, TransformError, compute};
pub use parser::{Expr, Op, ParseError, parse};
