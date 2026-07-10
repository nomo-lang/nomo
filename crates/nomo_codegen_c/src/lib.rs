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

mod array_helpers;
mod driver;
mod expression_arrays;
mod expression_collections;
mod expression_result_option;
mod expression_std_misc;
mod expression_string_char;
mod expressions;
mod host_env_process_helpers;
mod host_file_helpers;
mod host_fs_helpers;
mod host_helpers;
mod host_http_helpers;
mod host_http_server_helpers;
mod host_json_helpers;
mod host_net_helpers;
mod host_udp_helpers;
mod instances;
mod names;
mod nominal_enum_instances;
mod nominal_instances;
mod result_option_helpers;
mod runtime;
mod runtime_crypto;
mod runtime_path;
mod statement_bindings;
mod statement_control;
mod statement_function;
mod statement_lifecycle;
mod statement_question;
mod statements;
mod types;
mod usage;
mod usage_array_elements;
mod usage_env_scans;
mod usage_expr_walk;
mod usage_fs_open_scans;
mod usage_fs_read_scans;
mod usage_fs_write_scans;
use array_helpers::*;
pub use driver::emit_c;
use expression_arrays::*;
use expression_collections::*;
use expression_result_option::*;
use expression_std_misc::*;
use expression_string_char::*;
use expressions::*;
use host_env_process_helpers::*;
use host_file_helpers::*;
use host_fs_helpers::*;
use host_helpers::*;
use host_http_helpers::*;
use host_http_server_helpers::*;
use host_json_helpers::*;
use host_net_helpers::*;
use host_udp_helpers::*;
use instances::*;
use names::*;
use nominal_enum_instances::*;
use nominal_instances::*;
use result_option_helpers::*;
use runtime::*;
use runtime_crypto::*;
use runtime_path::*;
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
const BUILTIN_FFI_PUTS_EXPR: &str = "__nomo_ffi_puts";
const EXTERN_CALL_PREFIX: &str = "__nomo_extern::";
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
    out.push_str(&c_type(&function.return_type));
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
            out.push_str(&c_type(param));
        }
    }
    out.push_str(");\n");
}

#[cfg(test)]
mod tests;
