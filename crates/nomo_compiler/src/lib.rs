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

mod ffi_layout;
pub use ffi_layout::{CFieldLayout, CStructLayout, compute_repr_c_layout};

use crate::ast::{
    AssignOp, BinaryOp as AstBinaryOp, EnumDef as AstEnumDef, Expr as AstExpr,
    ExternOpaqueType as AstExternOpaqueType, ForVariant, Function as AstFunction,
    FunctionSignature as AstFunctionSignature, InterfaceDef as AstInterfaceDef,
    MatchArm as AstMatchArm, PostfixOp, SourceFile, Span, Stmt, StructDef as AstStructDef,
    TypeRef as AstTypeRef, UnaryOp as AstUnaryOp,
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
#[path = "builtins/builtins_cron.rs"]
mod builtins_cron;
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
#[path = "builtins/builtins_fmt.rs"]
mod builtins_fmt;
#[path = "builtins/builtins_fs.rs"]
mod builtins_fs;
#[path = "builtins/builtins_hash.rs"]
mod builtins_hash;
#[path = "builtins/builtins_http.rs"]
mod builtins_http;
#[path = "builtins/builtins_io.rs"]
mod builtins_io;
#[path = "builtins/builtins_jsonrpc.rs"]
mod builtins_jsonrpc;
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
#[path = "builtins/builtins_sqlite.rs"]
mod builtins_sqlite;
#[path = "builtins/builtins_string.rs"]
mod builtins_string;
#[path = "builtins/builtins_task.rs"]
mod builtins_task;
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
#[path = "validation/validation_concurrency.rs"]
mod validation_concurrency;
#[path = "validation/validation_imports.rs"]
mod validation_imports;
#[path = "validation/validation_suspend.rs"]
mod validation_suspend;
#[path = "validation/validation_tasks.rs"]
mod validation_tasks;
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
use builtins_cron::*;
use builtins_diagnostics::*;
use builtins_env::*;
use builtins_extensions::*;
use builtins_ffi::*;
use builtins_file_methods::*;
use builtins_fmt::*;
use builtins_fs::*;
use builtins_hash::*;
use builtins_http::*;
use builtins_io::*;
use builtins_jsonrpc::*;
use builtins_math::*;
use builtins_net_methods::*;
use builtins_num::*;
use builtins_option::*;
use builtins_os::*;
use builtins_path::*;
use builtins_process::*;
use builtins_result::*;
use builtins_sqlite::*;
use builtins_string::*;
use builtins_task::*;
use builtins_time::*;
use declarations::*;
pub use driver::{
    build_module_graph, build_module_graph_with_overrides,
    check_module_source_text_with_project_modules_and_overrides, check_script_source_text,
    check_source, check_source_text, check_source_text_with_external_imports,
    check_source_text_with_project_modules, check_source_text_with_project_modules_and_overrides,
    check_source_with_external_imports, check_source_with_external_modules,
    compile_script_source_to_c, compile_script_source_to_c_for_target,
    compile_source_text_to_c_with_project_modules, compile_source_to_c,
    compile_source_to_c_for_target, compile_source_to_c_with_external_imports,
    compile_source_to_c_with_external_modules, compile_source_to_c_with_project_modules,
    compile_source_to_c_with_project_modules_for_target,
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
use validation_concurrency::*;

const BUILTIN_PRINTLN_EXPR: &str = "__nomo_builtin_println";
const BUILTIN_PRINT_EXPR: &str = "__nomo_builtin_print";
const BUILTIN_EPRINTLN_EXPR: &str = "__nomo_builtin_eprintln";
const BUILTIN_EPRINT_EXPR: &str = "__nomo_builtin_eprint";
const BUILTIN_CSTRING_FROM_STRING_EXPR: &str = "__nomo_cstring_from_string";
const BUILTIN_CSTRING_DATA_EXPR: &str = "__nomo_cstring_data";
const BUILTIN_NULLABLE_NONE_EXPR: &str = "__nomo_nullable_none";
const BUILTIN_NULLABLE_SOME_EXPR: &str = "__nomo_nullable_some";
const BUILTIN_NULLABLE_IS_NULL_EXPR: &str = "__nomo_nullable_is_null";
const BUILTIN_NULLABLE_UNWRAP_EXPR: &str = "__nomo_nullable_unwrap";
const BUILTIN_OWNED_BORROW_EXPR: &str = "__nomo_owned_borrow";
const EXTERN_CALL_PREFIX: &str = "__nomo_extern::";
const BUILTIN_HTTP_GET_EXPR: &str = "__nomo_http_get";
const BUILTIN_HTTP_POST_EXPR: &str = "__nomo_http_post";
const BUILTIN_HTTP_SEND_EXPR: &str = "__nomo_http_send";
const BUILTIN_HTTP_OPEN_STREAM_EXPR: &str = "__nomo_http_open_stream";
const BUILTIN_HTTP_READ_TEXT_EXPR: &str = "__nomo_http_read_text";
const BUILTIN_HTTP_NEXT_SSE_EXPR: &str = "__nomo_http_next_sse";
const BUILTIN_HTTP_CANCEL_STREAM_EXPR: &str = "__nomo_http_cancel_stream";
const BUILTIN_HTTP_CLOSE_STREAM_EXPR: &str = "__nomo_http_close_stream";
const BUILTIN_HTTP_LISTEN_EXPR: &str = "__nomo_http_listen";
const BUILTIN_HTTP_ACCEPT_EXPR: &str = "__nomo_http_accept";
const BUILTIN_HTTP_RESPOND_STRING_EXPR: &str = "__nomo_http_respond_string";
const BUILTIN_HTTP_CLOSE_SERVER_EXPR: &str = "__nomo_http_close_server";
const BUILTIN_HTTP_CLOSE_EXCHANGE_EXPR: &str = "__nomo_http_close_exchange";
const BUILTIN_NET_CONNECT_EXPR: &str = "__nomo_net_connect_async";
const BUILTIN_PROCESS_START_EXPR: &str = "__nomo_process_start";
const BUILTIN_PROCESS_WRITE_STDIN_EXPR: &str = "__nomo_process_write_stdin";
const BUILTIN_PROCESS_CLOSE_STDIN_EXPR: &str = "__nomo_process_close_stdin";
const BUILTIN_PROCESS_NEXT_EVENT_EXPR: &str = "__nomo_process_next_event";
const BUILTIN_PROCESS_TRY_WAIT_EXPR: &str = "__nomo_process_try_wait";
const BUILTIN_PROCESS_TERMINATE_EXPR: &str = "__nomo_process_terminate";
const BUILTIN_PROCESS_CLOSE_CHILD_EXPR: &str = "__nomo_process_close_child";
const BUILTIN_TASK_SPAWN_EXPR: &str = "__nomo_task_spawn";
const BUILTIN_TASK_IS_CANCELLED_EXPR: &str = "__nomo_task_is_cancelled";
const BUILTIN_TASK_JOIN_EXPR: &str = "__nomo_task_join";
const BUILTIN_TASK_CANCEL_EXPR: &str = "__nomo_task_cancel";
const BUILTIN_TASK_CLOSE_EXPR: &str = "__nomo_task_close";
const BUILTIN_TASK_YIELD_EXPR: &str = "__nomo_task_yield";
const BUILTIN_TASK_SLEEP_EXPR: &str = "__nomo_task_sleep";
const BUILTIN_TASK_CHECK_CANCELLED_EXPR: &str = "__nomo_task_check_cancelled";
const BUILTIN_TASK_DEADLINE_ENTER_EXPR: &str = "__nomo_task_deadline_enter";
const BUILTIN_TASK_DEADLINE_EXIT_EXPR: &str = "__nomo_task_deadline_exit";
const BUILTIN_TASK_STRUCTURED_SPAWN_PREFIX: &str = "__nomo_structured_task_spawn::";
const BUILTIN_TASK_PUBLICATION_MOVE_EXPR: &str = "__nomo_task_publication_move";
const BUILTIN_TASK_STRUCTURED_JOIN_EXPR: &str = "__nomo_structured_task_join";
const BUILTIN_TASK_STRUCTURED_CANCEL_EXPR: &str = "__nomo_structured_task_cancel";
const BUILTIN_TASK_STRUCTURED_CANCEL_JOIN_EXPR: &str = "__nomo_structured_task_cancel_join";
const BUILTIN_TASK_CHANNEL_PREFIX: &str = "__nomo_task_channel::";
const BUILTIN_TASK_SEND_PREFIX: &str = "__nomo_task_send::";
const BUILTIN_TASK_RECEIVE_PREFIX: &str = "__nomo_task_receive::";
const BUILTIN_TASK_TRY_SEND_PREFIX: &str = "__nomo_task_try_send::";
const BUILTIN_TASK_TRY_RECEIVE_PREFIX: &str = "__nomo_task_try_receive::";
const BUILTIN_TASK_CLOSE_CHANNEL_PREFIX: &str = "__nomo_task_close_channel::";
const TASK_STRUCTURED_SPAWN_AST_NAME: &str = "\0nomo_structured_spawn";
const BUILTIN_SQLITE_OPEN_EXPR: &str = "__nomo_sqlite_open";
const BUILTIN_SQLITE_OPEN_MEMORY_EXPR: &str = "__nomo_sqlite_open_memory";
const BUILTIN_SQLITE_EXECUTE_EXPR: &str = "__nomo_sqlite_execute";
const BUILTIN_SQLITE_QUERY_EXPR: &str = "__nomo_sqlite_query";
const BUILTIN_SQLITE_NEXT_EXPR: &str = "__nomo_sqlite_next";
const BUILTIN_SQLITE_RESET_EXPR: &str = "__nomo_sqlite_reset";
const BUILTIN_SQLITE_CLOSE_QUERY_EXPR: &str = "__nomo_sqlite_close_query";
const BUILTIN_SQLITE_CLOSE_EXPR: &str = "__nomo_sqlite_close";

pub use nomo_ir::{
    BinaryOp, Const, CronOperation, DeferredCall, EnumType, EnumVariantType, ExternFunction,
    Function, JsonOperation, JsonRpcOperation, LoopKind, MatchStatementArm, MatchValueArm,
    MathBinaryFunction, MathUnaryFunction, NumBinaryFunction, Parameter, Program, QuestionCarrier,
    Statement, StructField, StructType, TaskSelectArm, TaskSelectOperation, UnaryOp, ValueExpr,
    ValueType,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalModule {
    /// The name used by the consuming project's imports.
    pub import_root: String,
    /// The dependency's own, manifest-derived module root.
    pub source_import_root: String,
    pub source_root: PathBuf,
}

#[derive(Debug, Clone)]
struct FunctionSignature {
    is_suspend: bool,
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
    Const,
    Param,
    EnumPayload { value: ValueExpr, variant: String },
    FunctionEffect { is_suspend: bool },
    TaskScope,
    PublicationMove { line: usize, boundary: &'static str },
}

fn binding_value_expr(name: &str, binding: &Binding) -> ValueExpr {
    match &binding.source {
        BindingSource::Local | BindingSource::Const | BindingSource::Param => {
            ValueExpr::Variable(name.to_string())
        }
        BindingSource::EnumPayload { value, variant } => ValueExpr::EnumPayload {
            value: Box::new(value.clone()),
            variant: variant.clone(),
        },
        BindingSource::FunctionEffect { .. } => {
            unreachable!("the internal function-effect binding is never a source expression")
        }
        BindingSource::TaskScope => {
            unreachable!("the internal task-scope binding is never a source expression")
        }
        BindingSource::PublicationMove { .. } => {
            unreachable!("publication-move markers are stored under internal scope keys")
        }
    }
}

const PUBLICATION_MOVE_BINDING_PREFIX: &str = "\0nomo_publication_move::";

fn publication_move_binding_key(name: &str) -> String {
    format!("{PUBLICATION_MOVE_BINDING_PREFIX}{name}")
}

fn publication_move_site(
    scope: &HashMap<String, Binding>,
    name: &str,
) -> Option<(usize, &'static str)> {
    match scope
        .get(&publication_move_binding_key(name))
        .map(|binding| &binding.source)
    {
        Some(BindingSource::PublicationMove { line, boundary }) => Some((*line, *boundary)),
        _ => None,
    }
}

fn mark_publication_move(
    scope: &mut HashMap<String, Binding>,
    name: &str,
    line: usize,
    boundary: &'static str,
) {
    scope.insert(
        publication_move_binding_key(name),
        Binding {
            value_type: ValueType::Void,
            mutable: false,
            source: BindingSource::PublicationMove { line, boundary },
        },
    );
}

fn propagate_publication_moves(
    destination: &mut HashMap<String, Binding>,
    source: &HashMap<String, Binding>,
) {
    for (name, binding) in source {
        if name.starts_with(PUBLICATION_MOVE_BINDING_PREFIX) {
            destination.insert(name.clone(), binding.clone());
        }
    }
}

fn binding_source_noun(binding: &Binding) -> &'static str {
    match binding.source {
        BindingSource::Param => "parameter",
        _ => "variable",
    }
}

const FUNCTION_EFFECT_BINDING: &str = "\0nomo_function_effect";
const TASK_SCOPE_BINDING: &str = "\0nomo_task_scope";

fn current_function_is_suspend(scope: &HashMap<String, Binding>) -> bool {
    matches!(
        scope
            .get(FUNCTION_EFFECT_BINDING)
            .map(|binding| &binding.source),
        Some(BindingSource::FunctionEffect { is_suspend: true })
    )
}

fn current_function_has_task_scope(scope: &HashMap<String, Binding>) -> bool {
    matches!(
        scope.get(TASK_SCOPE_BINDING).map(|binding| &binding.source),
        Some(BindingSource::TaskScope)
    )
}

fn ensure_suspend_call_allowed(
    path: &Path,
    span: &Span,
    callable: &str,
    signature: &FunctionSignature,
    scope: &HashMap<String, Binding>,
) -> Result<(), Diagnostic> {
    if signature.is_suspend && !current_function_is_suspend(scope) {
        return Err(Diagnostic::new(
            "E0870",
            format!(
                "synchronous function cannot call suspend function `{callable}`; mark the caller `suspend`"
            ),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FunctionInstance {
    name: String,
    args: Vec<ValueType>,
}

#[cfg(test)]
mod tests;
