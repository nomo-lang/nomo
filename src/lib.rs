pub mod ast;
pub mod codegen;
pub mod compiler;
pub mod diagnostic;
pub mod lexer;
pub mod parser;
pub mod project;

pub use compiler::{
    Program, check_source, check_source_text, check_source_text_with_external_imports,
    check_source_with_external_imports, compile_source_to_c,
    compile_source_to_c_with_external_imports,
};
pub use diagnostic::{Diagnostic, Suggestion};
pub use lexer::{Token, TokenKind, lex};
