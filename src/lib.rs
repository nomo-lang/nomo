pub mod ast;
pub mod codegen;
pub mod compiler;
pub mod diagnostic;
pub mod doc;
pub mod format;
pub mod lexer;
pub mod parser;
pub mod project;
pub mod semantic;

pub use compiler::{
    Program, check_script_source_text, check_source, check_source_text,
    check_source_text_with_external_imports, check_source_text_with_project_modules,
    check_source_text_with_project_modules_and_overrides, check_source_with_external_imports,
    compile_script_source_to_c, compile_source_text_to_c_with_project_modules, compile_source_to_c,
    compile_source_to_c_with_external_imports,
};
pub use diagnostic::{Diagnostic, Suggestion};
pub use format::format_source;
pub use lexer::{Token, TokenKind, lex};
