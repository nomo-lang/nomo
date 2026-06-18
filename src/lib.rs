pub mod ast;
pub mod codegen;
pub mod compiler;
pub mod diagnostic;
pub mod lexer;
pub mod parser;
pub mod project;

pub use compiler::{Program, check_source, check_source_text, compile_source_to_c};
pub use diagnostic::{Diagnostic, Suggestion};
pub use lexer::{Token, TokenKind, lex};
