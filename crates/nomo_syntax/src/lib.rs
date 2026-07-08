#![allow(clippy::result_large_err)]

pub mod ast;
pub mod diagnostic;
pub mod lexer;
pub mod parser;

pub use diagnostic::{Diagnostic, Suggestion};
pub use lexer::{Token, TokenKind, lex};
