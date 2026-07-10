#![allow(
    clippy::cloned_ref_to_slice_refs,
    clippy::collapsible_if,
    clippy::large_enum_variant,
    clippy::needless_borrow,
    clippy::needless_option_as_deref,
    clippy::redundant_closure,
    clippy::result_large_err,
    clippy::single_char_add_str,
    clippy::too_many_arguments
)]

pub mod compiler;
pub mod doc;
pub mod format;
pub mod project;
pub mod semantic;

pub use compiler::{
    Program, check_script_source_text, check_source, check_source_text,
    check_source_text_with_external_imports, check_source_text_with_project_modules,
    check_source_text_with_project_modules_and_overrides, check_source_with_external_imports,
    compile_script_source_to_c, compile_source_text_to_c_with_project_modules, compile_source_to_c,
    compile_source_to_c_with_external_imports,
};
pub use format::format_source;
pub use lexer::{Token, TokenKind, lex};
pub use nomo_codegen_c as codegen;
pub use nomo_diagnostics as diagnostic;
pub use nomo_diagnostics::{Diagnostic, Suggestion};
pub use nomo_syntax::{ast, lexer, parser};
