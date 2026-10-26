//! tdesktop's `.style` language: [`lexer`] splits it, [`parser`] evaluates it as
//! `codegen/style` does, and [`emit`] writes the result as Rust.

pub(crate) mod emit;
pub(crate) mod lexer;
pub(crate) mod parser;
