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

use nomo_ir::{
    BinaryOp, DeferredCall, EnumType, ExternFunction, Function, LoopKind, MatchStatementArm,
    MatchValueArm, MathBinaryFunction, MathUnaryFunction, NumBinaryFunction, Program,
    QuestionCarrier, Statement, StructType, UnaryOp, ValueExpr, ValueType,
};
use std::collections::BTreeSet;

// Core C emission, naming, and nominal type support.
#[path = "core/array_helpers.rs"]
mod array_helpers;
#[path = "core/driver.rs"]
mod driver;
// Expression emission.
#[path = "expressions/expression_arrays.rs"]
mod expression_arrays;
#[path = "expressions/expression_collections.rs"]
mod expression_collections;
#[path = "expressions/expression_result_option.rs"]
mod expression_result_option;
#[path = "expressions/expression_std_misc.rs"]
mod expression_std_misc;
#[path = "expressions/expression_string_char.rs"]
mod expression_string_char;
#[path = "expressions/expressions.rs"]
mod expressions;
// Runtime and host helper emission.
#[path = "runtime/host_env_process_helpers.rs"]
mod host_env_process_helpers;
#[path = "runtime/host_file_helpers.rs"]
mod host_file_helpers;
#[path = "runtime/host_fs_helpers.rs"]
mod host_fs_helpers;
#[path = "runtime/host_http_helpers.rs"]
mod host_http_helpers;
#[path = "runtime/host_http_server_helpers.rs"]
mod host_http_server_helpers;
#[path = "runtime/host_io_helpers.rs"]
mod host_io_helpers;
#[path = "runtime/host_json_helpers.rs"]
mod host_json_helpers;
#[path = "runtime/host_net_helpers.rs"]
mod host_net_helpers;
#[path = "runtime/host_num_checked_helpers.rs"]
mod host_num_checked_helpers;
#[path = "runtime/host_num_parse_helpers.rs"]
mod host_num_parse_helpers;
#[path = "runtime/host_regex_helpers.rs"]
mod host_regex_helpers;
#[path = "runtime/host_udp_helpers.rs"]
mod host_udp_helpers;
#[path = "core/instances.rs"]
mod instances;
#[path = "core/names.rs"]
mod names;
#[path = "core/nominal_enum_instances.rs"]
mod nominal_enum_instances;
#[path = "core/nominal_instances.rs"]
mod nominal_instances;
#[path = "runtime/result_option_helpers.rs"]
mod result_option_helpers;
#[path = "runtime/runtime_crypto.rs"]
mod runtime_crypto;
#[path = "runtime/runtime_hash.rs"]
mod runtime_hash;
// Statement emission.
#[path = "statements/statement_bindings.rs"]
mod statement_bindings;
#[path = "statements/statement_control.rs"]
mod statement_control;
#[path = "statements/statement_function.rs"]
mod statement_function;
#[path = "statements/statement_lifecycle.rs"]
mod statement_lifecycle;
#[path = "statements/statement_question.rs"]
mod statement_question;
#[path = "statements/statements.rs"]
mod statements;
#[path = "core/types.rs"]
mod types;
// Program usage scans used to select runtime support.
#[path = "usage/usage.rs"]
mod usage;
#[path = "usage/usage_array_elements.rs"]
mod usage_array_elements;
#[path = "usage/usage_env_scans.rs"]
mod usage_env_scans;
#[path = "usage/usage_expr_walk.rs"]
mod usage_expr_walk;
#[path = "usage/usage_fs_open_scans.rs"]
mod usage_fs_open_scans;
#[path = "usage/usage_fs_read_scans.rs"]
mod usage_fs_read_scans;
#[path = "usage/usage_fs_write_scans.rs"]
mod usage_fs_write_scans;
use array_helpers::*;
pub use driver::{emit_c, emit_c_for_target};
use expression_arrays::*;
use expression_collections::*;
use expression_result_option::*;
use expression_std_misc::*;
use expression_string_char::*;
use expressions::*;
use host_env_process_helpers::*;
use host_file_helpers::*;
use host_fs_helpers::*;
use host_http_helpers::*;
use host_http_server_helpers::*;
use host_io_helpers::*;
use host_json_helpers::*;
use host_net_helpers::*;
use host_num_checked_helpers::*;
use host_num_parse_helpers::*;
use host_regex_helpers::*;
use host_udp_helpers::*;
use instances::*;
use names::*;
use nominal_enum_instances::*;
use nominal_instances::*;
use nomo_runtime::{
    emit_c_prelude, emit_log_enabled_helper, emit_math_runtime, emit_operator_runtime,
    emit_string_runtime,
};
use result_option_helpers::*;
use runtime_crypto::*;
use runtime_hash::*;
use statement_bindings::*;
use statement_control::*;
use statement_function::*;
use statement_lifecycle::*;
use statement_question::*;
use statements::*;
use types::*;
use usage::*;
use usage_array_elements::*;
use usage_env_scans::*;
use usage_expr_walk::*;
use usage_fs_open_scans::*;
use usage_fs_read_scans::*;
use usage_fs_write_scans::*;

const BUILTIN_PRINTLN_EXPR: &str = "__nomo_builtin_println";
const BUILTIN_PRINT_EXPR: &str = "__nomo_builtin_print";
const BUILTIN_EPRINTLN_EXPR: &str = "__nomo_builtin_eprintln";
const BUILTIN_EPRINT_EXPR: &str = "__nomo_builtin_eprint";
const EXTERN_CALL_PREFIX: &str = "__nomo_extern::";
const BUILTIN_CSTRING_FROM_STRING_EXPR: &str = "__nomo_cstring_from_string";
const BUILTIN_CSTRING_DATA_EXPR: &str = "__nomo_cstring_data";
const BUILTIN_NULLABLE_NONE_EXPR: &str = "__nomo_nullable_none";
const BUILTIN_NULLABLE_SOME_EXPR: &str = "__nomo_nullable_some";
const BUILTIN_NULLABLE_IS_NULL_EXPR: &str = "__nomo_nullable_is_null";
const BUILTIN_NULLABLE_UNWRAP_EXPR: &str = "__nomo_nullable_unwrap";
const BUILTIN_OWNED_BORROW_EXPR: &str = "__nomo_owned_borrow";
const BUILTIN_HTTP_GET_EXPR: &str = "__nomo_http_get";
const BUILTIN_HTTP_POST_EXPR: &str = "__nomo_http_post";
const BUILTIN_HTTP_LISTEN_EXPR: &str = "__nomo_http_listen";
const BUILTIN_HTTP_ACCEPT_EXPR: &str = "__nomo_http_accept";
const BUILTIN_HTTP_RESPOND_STRING_EXPR: &str = "__nomo_http_respond_string";
const BUILTIN_HTTP_CLOSE_SERVER_EXPR: &str = "__nomo_http_close_server";
const BUILTIN_HTTP_CLOSE_EXCHANGE_EXPR: &str = "__nomo_http_close_exchange";

#[derive(Debug, Clone, PartialEq, Eq)]
struct NumCheckedBinaryInstance {
    op: BinaryOp,
    value_type: ValueType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LocalArray {
    name: String,
    value_type: ValueType,
    c_value: Option<String>,
}

fn emit_function_name_macros(out: &mut String, program: &Program) {
    for function in &program.functions {
        let package = c_package_ident(&function.package);
        out.push_str("#define ");
        out.push_str(&c_fn_ident(&function.name));
        out.push(' ');
        out.push_str("nomo_pkg_");
        out.push_str(&package);
        out.push_str("_fn_");
        out.push_str(&function.name);
        out.push('\n');
    }
    if !program.functions.is_empty() {
        out.push('\n');
    }
}

fn emit_prototype(out: &mut String, function: &Function) {
    emit_signature(out, function);
    out.push_str(";\n");
}

fn emit_extern_function_prototypes(out: &mut String, program: &Program) {
    for function in &program.extern_functions {
        emit_extern_function_prototype(out, function);
    }
    if !program.extern_functions.is_empty() {
        out.push('\n');
    }
}

fn emit_extern_function_prototype(out: &mut String, function: &ExternFunction) {
    out.push_str("extern ");
    out.push_str(&c_extern_type(&function.return_type));
    out.push(' ');
    out.push_str(&function.symbol);
    out.push('(');
    if function.params.is_empty() {
        out.push_str("void");
    } else {
        for (index, param) in function.params.iter().enumerate() {
            if index > 0 {
                out.push_str(", ");
            }
            out.push_str(&c_extern_type(param));
        }
    }
    out.push_str(");\n");
}

#[cfg(test)]
mod tests;
