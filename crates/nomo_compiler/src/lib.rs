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

// Semantic analysis and usage collection.
#[path = "analysis/analysis.rs"]
mod analysis;
#[path = "analysis/analysis_generic.rs"]
mod analysis_generic;
#[path = "analysis/analysis_usage.rs"]
mod analysis_usage;
#[path = "analysis/analysis_usage_builtins.rs"]
mod analysis_usage_builtins;
#[path = "analysis/analysis_usage_prelude.rs"]
mod analysis_usage_prelude;
// Standard-library and builtin lowering.
#[path = "builtins/builtins_array_methods.rs"]
mod builtins_array_methods;
#[path = "builtins/builtins_char.rs"]
mod builtins_char;
#[path = "builtins/builtins_collections.rs"]
mod builtins_collections;
#[path = "builtins/builtins_diagnostics.rs"]
mod builtins_diagnostics;
#[path = "builtins/builtins_env.rs"]
mod builtins_env;
#[path = "builtins/builtins_extensions.rs"]
mod builtins_extensions;
#[path = "builtins/builtins_ffi.rs"]
mod builtins_ffi;
#[path = "builtins/builtins_file_methods.rs"]
mod builtins_file_methods;
#[path = "builtins/builtins_fs.rs"]
mod builtins_fs;
#[path = "builtins/builtins_hash.rs"]
mod builtins_hash;
#[path = "builtins/builtins_http.rs"]
mod builtins_http;
#[path = "builtins/builtins_io.rs"]
mod builtins_io;
#[path = "builtins/builtins_math.rs"]
mod builtins_math;
#[path = "builtins/builtins_net_methods.rs"]
mod builtins_net_methods;
#[path = "builtins/builtins_num.rs"]
mod builtins_num;
#[path = "builtins/builtins_option.rs"]
mod builtins_option;
#[path = "builtins/builtins_os.rs"]
mod builtins_os;
#[path = "builtins/builtins_path.rs"]
mod builtins_path;
#[path = "builtins/builtins_process.rs"]
mod builtins_process;
#[path = "builtins/builtins_result.rs"]
mod builtins_result;
#[path = "builtins/builtins_string.rs"]
mod builtins_string;
#[path = "builtins/builtins_time.rs"]
mod builtins_time;
// Compiler driver and shared type/declaration machinery.
#[path = "core/declarations.rs"]
mod declarations;
#[path = "core/driver.rs"]
mod driver;
// Expression lowering.
#[path = "expressions/expression_calls.rs"]
mod expression_calls;
#[path = "expressions/expression_enums.rs"]
mod expression_enums;
#[path = "expressions/expression_helpers.rs"]
mod expression_helpers;
#[path = "expressions/expression_if.rs"]
mod expression_if;
#[path = "expressions/expression_match.rs"]
mod expression_match;
#[path = "expressions/expression_ops.rs"]
mod expression_ops;
#[path = "expressions/expression_single_calls.rs"]
mod expression_single_calls;
#[path = "expressions/expression_structs.rs"]
mod expression_structs;
#[path = "expressions/expressions.rs"]
mod expressions;
#[path = "core/externs.rs"]
mod externs;
// Import and project module graph handling.
#[path = "imports/import_diagnostics.rs"]
mod import_diagnostics;
#[path = "imports/import_resolution.rs"]
mod import_resolution;
#[path = "imports/imports.rs"]
mod imports;
#[path = "core/interfaces.rs"]
mod interfaces;
#[path = "imports/module_graph.rs"]
mod module_graph;
#[path = "imports/modules.rs"]
mod modules;
#[path = "core/program_lowering.rs"]
mod program_lowering;
// Question operator lowering.
#[path = "questions/question_assignments.rs"]
mod question_assignments;
#[path = "questions/question_blocks.rs"]
mod question_blocks;
#[path = "questions/question_extraction.rs"]
mod question_extraction;
#[path = "questions/question_initializers.rs"]
mod question_initializers;
#[path = "questions/question_lowering.rs"]
mod question_lowering;
#[path = "questions/question_match.rs"]
mod question_match;
#[path = "questions/question_returns.rs"]
mod question_returns;
// Statement lowering.
#[path = "statements/statement_assignments.rs"]
mod statement_assignments;
#[path = "statements/statement_blocks.rs"]
mod statement_blocks;
#[path = "statements/statement_patterns.rs"]
mod statement_patterns;
#[path = "statements/statement_returns.rs"]
mod statement_returns;
#[path = "statements/statements.rs"]
mod statements;
// Type parsing and validation.
#[path = "core/type_parsing.rs"]
mod type_parsing;
#[path = "core/typing.rs"]
mod typing;
#[path = "validation/validation.rs"]
mod validation;
#[path = "validation/validation_imports.rs"]
mod validation_imports;
#[path = "validation/validation_type_diagnostics.rs"]
mod validation_type_diagnostics;
#[path = "validation/validation_types.rs"]
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
use builtins_ffi::*;
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
    build_module_graph, build_module_graph_with_overrides,
    check_module_source_text_with_project_modules_and_overrides, check_script_source_text,
    check_source, check_source_text, check_source_text_with_external_imports,
    check_source_text_with_project_modules, check_source_text_with_project_modules_and_overrides,
    check_source_with_external_imports, check_source_with_external_modules,
    compile_script_source_to_c, compile_source_text_to_c_with_project_modules, compile_source_to_c,
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
pub use module_graph::{ModuleGraph, ModuleId, ModuleNode};
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
const BUILTIN_CSTRING_FROM_STRING_EXPR: &str = "__nomo_cstring_from_string";
const BUILTIN_CSTRING_DATA_EXPR: &str = "__nomo_cstring_data";
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct GenericInterfaceBound {
    type_param_index: usize,
    type_param: String,
    interface: String,
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
