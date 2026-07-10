#![allow(
    clippy::collapsible_if,
    clippy::large_enum_variant,
    clippy::needless_borrow,
    clippy::needless_option_as_deref,
    clippy::redundant_closure,
    clippy::result_large_err,
    clippy::too_many_arguments
)]

pub use nomo_diagnostics as diagnostic;
pub use nomo_syntax::{ast, lexer, parser};

use crate::ast::{
    AssignOp, BinaryOp as AstBinaryOp, EnumDef as AstEnumDef, Expr as AstExpr, ForVariant,
    Function as AstFunction, FunctionSignature as AstFunctionSignature,
    InterfaceDef as AstInterfaceDef, MatchArm as AstMatchArm, PostfixOp, SourceFile, Span, Stmt,
    StructDef as AstStructDef, TypeRef as AstTypeRef, UnaryOp as AstUnaryOp,
};
use crate::diagnostic::{Diagnostic, Suggestion};
use nomo_codegen_c as codegen;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

mod analysis;
mod analysis_generic;
mod analysis_usage;
mod analysis_usage_builtins;
mod analysis_usage_prelude;
mod builtins_array_methods;
mod builtins_char;
mod builtins_collections;
mod builtins_diagnostics;
mod builtins_env;
mod builtins_extensions;
mod builtins_file_methods;
mod builtins_fs;
mod builtins_hash;
mod builtins_http;
mod builtins_io;
mod builtins_math;
mod builtins_net_methods;
mod builtins_num;
mod builtins_option;
mod builtins_os;
mod builtins_path;
mod builtins_process;
mod builtins_result;
mod builtins_string;
mod builtins_time;
mod declarations;
mod driver;
mod expression_calls;
mod expression_enums;
mod expression_helpers;
mod expression_if;
mod expression_match;
mod expression_ops;
mod expression_single_calls;
mod expression_structs;
mod expressions;
mod externs;
mod import_diagnostics;
mod import_resolution;
mod imports;
mod interfaces;
mod modules;
mod program_lowering;
mod question_assignments;
mod question_blocks;
mod question_extraction;
mod question_initializers;
mod question_lowering;
mod question_match;
mod question_returns;
mod statement_assignments;
mod statement_blocks;
mod statement_patterns;
mod statement_returns;
mod statements;
mod type_parsing;
mod typing;
mod validation;
mod validation_imports;
mod validation_type_diagnostics;
mod validation_types;
use analysis::*;
use analysis_generic::*;
use analysis_usage::*;
use builtins_array_methods::*;
use builtins_char::*;
use builtins_collections::*;
use builtins_diagnostics::*;
use builtins_env::*;
use builtins_extensions::*;
use builtins_file_methods::*;
use builtins_fs::*;
use builtins_hash::*;
use builtins_http::*;
use builtins_io::*;
use builtins_math::*;
use builtins_net_methods::*;
use builtins_num::*;
use builtins_option::*;
use builtins_os::*;
use builtins_path::*;
use builtins_process::*;
use builtins_result::*;
use builtins_string::*;
use builtins_time::*;
use declarations::*;
pub use driver::{
    check_script_source_text, check_source, check_source_text,
    check_source_text_with_external_imports, check_source_text_with_project_modules,
    check_source_text_with_project_modules_and_overrides, check_source_with_external_imports,
    check_source_with_external_modules, compile_script_source_to_c,
    compile_source_text_to_c_with_project_modules, compile_source_to_c,
    compile_source_to_c_with_external_imports, compile_source_to_c_with_external_modules,
    compile_source_to_c_with_project_modules,
};
use expression_calls::*;
use expression_enums::*;
use expression_helpers::*;
use expression_if::*;
use expression_match::*;
use expression_ops::*;
use expression_single_calls::*;
use expression_structs::*;
use expressions::*;
use externs::*;
use import_diagnostics::*;
use import_resolution::*;
use imports::*;
use interfaces::*;
use modules::merge_imported_public_api;
use program_lowering::{EntryMode, lower_program, reject_script_body};
use question_assignments::*;
use question_blocks::*;
use question_extraction::*;
use question_initializers::*;
use question_lowering::*;
use question_match::*;
use question_returns::*;
use statement_assignments::*;
use statement_blocks::*;
use statement_patterns::*;
use statement_returns::*;
use statements::*;
use type_parsing::*;
use typing::*;
use validation::*;

const BUILTIN_PRINTLN_EXPR: &str = "__nomo_builtin_println";
const BUILTIN_PRINT_EXPR: &str = "__nomo_builtin_print";
const BUILTIN_EPRINTLN_EXPR: &str = "__nomo_builtin_eprintln";
const BUILTIN_EPRINT_EXPR: &str = "__nomo_builtin_eprint";
const BUILTIN_FFI_PUTS_EXPR: &str = "__nomo_ffi_puts";
const EXTERN_CALL_PREFIX: &str = "__nomo_extern::";
const BUILTIN_HTTP_GET_EXPR: &str = "__nomo_http_get";
const BUILTIN_HTTP_POST_EXPR: &str = "__nomo_http_post";
const BUILTIN_HTTP_LISTEN_EXPR: &str = "__nomo_http_listen";
const BUILTIN_HTTP_ACCEPT_EXPR: &str = "__nomo_http_accept";
const BUILTIN_HTTP_RESPOND_STRING_EXPR: &str = "__nomo_http_respond_string";
const BUILTIN_HTTP_CLOSE_SERVER_EXPR: &str = "__nomo_http_close_server";
const BUILTIN_HTTP_CLOSE_EXCHANGE_EXPR: &str = "__nomo_http_close_exchange";

pub use nomo_ir::{
    BinaryOp, Const, DeferredCall, EnumType, EnumVariantType, ExternFunction, Function, LoopKind,
    MatchStatementArm, MatchValueArm, MathBinaryFunction, MathUnaryFunction, NumBinaryFunction,
    Parameter, Program, QuestionCarrier, Statement, StructField, StructType, UnaryOp, ValueExpr,
    ValueType,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalModule {
    pub import_root: String,
    pub source_root: PathBuf,
}

#[derive(Debug, Clone)]
struct FunctionSignature {
    type_params: Vec<String>,
    params: Vec<ParamSignature>,
    return_type: ValueType,
    extern_symbol: Option<String>,
}

#[derive(Debug, Clone)]
struct ParamSignature {
    value_type: ValueType,
    mutable: bool,
}

#[derive(Debug, Clone)]
struct Binding {
    value_type: ValueType,
    mutable: bool,
    source: BindingSource,
}

#[derive(Debug, Clone)]
enum BindingSource {
    Local,
    Param,
    EnumPayload { value: ValueExpr, variant: String },
}

fn binding_value_expr(name: &str, binding: &Binding) -> ValueExpr {
    match &binding.source {
        BindingSource::Local | BindingSource::Param => ValueExpr::Variable(name.to_string()),
        BindingSource::EnumPayload { value, variant } => ValueExpr::EnumPayload {
            value: Box::new(value.clone()),
            variant: variant.clone(),
        },
    }
}

fn binding_source_noun(binding: &Binding) -> &'static str {
    match binding.source {
        BindingSource::Param => "parameter",
        _ => "variable",
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FunctionInstance {
    name: String,
    args: Vec<ValueType>,
}

#[cfg(test)]
mod tests;
