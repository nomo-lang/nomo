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

mod expressions;
mod host_helpers;
mod instances;
mod names;
mod result_option_helpers;
mod runtime;
mod statements;
mod types;
mod usage;
use expressions::*;
use host_helpers::*;
use instances::*;
use names::*;
use result_option_helpers::*;
use runtime::*;
use statements::*;
use types::*;
use usage::*;

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

pub fn emit_c(program: &Program) -> String {
    let mut out = String::new();
    out.push_str(
        "#define _POSIX_C_SOURCE 200809L\n#ifdef _WIN32\n#define _CRT_RAND_S\n#endif\n#include <ctype.h>\n#include <errno.h>\n#include <inttypes.h>\n#include <limits.h>\n#include <math.h>\n#include <stdint.h>\n#include <stdio.h>\n#include <stdlib.h>\n#include <string.h>\n#include <sys/stat.h>\n#include <time.h>\n#ifdef _WIN32\n#include <direct.h>\n#include <winsock2.h>\n#include <ws2tcpip.h>\n#include <windows.h>\ntypedef SOCKET nomo_socket;\n#define NOMO_INVALID_SOCKET INVALID_SOCKET\n#define NOMO_SOCKET_CLOSE closesocket\n#define NOMO_GETCWD _getcwd\n#define NOMO_POPEN _popen\n#define NOMO_PCLOSE _pclose\n#else\n#include <dirent.h>\n#include <netdb.h>\n#include <regex.h>\n#include <sys/socket.h>\n#include <sys/time.h>\n#include <sys/types.h>\n#include <sys/wait.h>\n#include <unistd.h>\ntypedef int nomo_socket;\n#define NOMO_INVALID_SOCKET (-1)\n#define NOMO_SOCKET_CLOSE close\n#define NOMO_GETCWD getcwd\n#define NOMO_POPEN popen\n#define NOMO_PCLOSE pclose\n#endif\n#ifndef PATH_MAX\n#define PATH_MAX 4096\n#endif\n\n",
    );
    out.push_str("static void nomo_panic(const char *message) {\n");
    out.push_str("    fputs(\"panic: \", stderr);\n");
    out.push_str("    fputs(message, stderr);\n");
    out.push_str("    fputc('\\n', stderr);\n");
    out.push_str("    exit(1);\n");
    out.push_str("}\n\n");
    emit_operator_runtime(&mut out);
    out.push('\n');
    emit_math_runtime(&mut out);
    out.push('\n');
    emit_string_runtime(&mut out);
    out.push('\n');
    if uses_log_enabled(program) {
        emit_log_enabled_helper(&mut out);
        out.push('\n');
    }

    for const_def in &program.consts {
        out.push_str("#define ");
        out.push_str(&c_var_ident(&const_def.name));
        out.push(' ');
        emit_expr(&mut out, &const_def.initializer);
        out.push('\n');
    }
    if !program.consts.is_empty() {
        out.push('\n');
    }

    emit_function_name_macros(&mut out, program);
    emit_type_name_macros(&mut out, program);

    let array_element_types = collect_array_element_types(program);
    emit_type_forward_declarations(&mut out, program, &array_element_types);
    emit_lifecycle_helper_prototypes(&mut out, program, &array_element_types);
    emit_extern_function_prototypes(&mut out, program);

    for element_type in &array_element_types {
        emit_array_type(&mut out, element_type);
        out.push('\n');
    }
    emit_struct_and_enum_types(&mut out, program);
    emit_nominal_lifecycle_helpers(&mut out, program);
    if uses_hash_builtin(program) {
        emit_hash_helpers(&mut out);
        out.push('\n');
    }
    for element_type in &array_element_types {
        emit_array_helpers(&mut out, element_type);
        out.push('\n');
    }
    if uses_crypto_builtin(program) {
        emit_crypto_helpers(&mut out);
        out.push('\n');
    }
    if uses_collections_builtin(program) {
        emit_collections_helpers(&mut out);
        out.push('\n');
    }
    if array_element_types
        .iter()
        .any(|item| item == &ValueType::String)
    {
        emit_string_split_helper(&mut out);
        out.push('\n');
    }
    if uses_io_read_line(program) {
        emit_io_read_line_helper(&mut out);
        out.push('\n');
    }
    if uses_env_args(program) {
        out.push_str("static int nomo_argc = 0;\n");
        out.push_str("static char **nomo_argv = NULL;\n\n");
        emit_env_args_helper(&mut out);
        out.push('\n');
    }
    if uses_fs_read_to_string(program) {
        emit_fs_read_to_string_helper(&mut out);
        out.push('\n');
    }
    if uses_fs_write_string(program) {
        emit_fs_write_string_helper(&mut out);
        out.push('\n');
    }
    if uses_fs_read_bytes(program) {
        emit_fs_read_bytes_helper(&mut out);
        out.push('\n');
    }
    if uses_fs_write_bytes(program) {
        emit_fs_write_bytes_helper(&mut out);
        out.push('\n');
    }
    if uses_fs_exists(program) {
        emit_fs_exists_helper(&mut out);
        out.push('\n');
    }
    if uses_fs_metadata(program) {
        emit_fs_metadata_helper(&mut out);
        out.push('\n');
    }
    if uses_fs_create_dir(program) {
        emit_fs_create_dir_helper(&mut out);
        out.push('\n');
    }
    if uses_fs_remove_dir(program) {
        emit_fs_remove_dir_helper(&mut out);
        out.push('\n');
    }
    if uses_fs_read_dir(program) {
        emit_fs_read_dir_helper(&mut out);
        out.push('\n');
    }
    if uses_fs_open(program) {
        emit_fs_open_helper(&mut out);
        out.push('\n');
    }
    if uses_file_read_to_string(program) {
        emit_file_read_to_string_helper(&mut out);
        out.push('\n');
    }
    if uses_file_write_string(program) {
        emit_file_write_string_helper(&mut out);
        out.push('\n');
    }
    if uses_file_close(program) {
        emit_file_close_helper(&mut out);
        out.push('\n');
    }
    if uses_net_connect(program)
        || uses_net_listen(program)
        || uses_net_udp_bind(program)
        || uses_http_client(program)
        || uses_http_server(program)
        || uses_tcp_listener_accept(program)
        || uses_tcp_stream_read_to_string(program)
        || uses_tcp_stream_write_string(program)
        || uses_udp_socket_recv_from_string(program)
        || uses_udp_socket_send_to_string(program)
    {
        emit_net_common_helpers(&mut out);
        out.push('\n');
    }
    if uses_net_connect(program) {
        emit_net_connect_helper(&mut out);
        out.push('\n');
    }
    if uses_net_listen(program) {
        emit_net_listen_helper(&mut out);
        out.push('\n');
    }
    if uses_net_udp_bind(program) {
        emit_net_udp_bind_helper(&mut out);
        out.push('\n');
    }
    if uses_tcp_listener_accept(program) {
        emit_tcp_listener_accept_helper(&mut out);
        out.push('\n');
    }
    if uses_tcp_listener_close(program) {
        emit_tcp_listener_close_helper(&mut out);
        out.push('\n');
    }
    if uses_tcp_stream_read_to_string(program) {
        emit_tcp_stream_read_to_string_helper(&mut out);
        out.push('\n');
    }
    if uses_tcp_stream_write_string(program) {
        emit_tcp_stream_write_string_helper(&mut out);
        out.push('\n');
    }
    if uses_tcp_stream_close(program) {
        emit_tcp_stream_close_helper(&mut out);
        out.push('\n');
    }
    if uses_udp_socket_recv_from_string(program) {
        emit_udp_socket_recv_from_string_helper(&mut out);
        out.push('\n');
    }
    if uses_udp_socket_send_to_string(program) {
        emit_udp_socket_send_to_string_helper(&mut out);
        out.push('\n');
    }
    if uses_udp_socket_close(program) {
        emit_udp_socket_close_helper(&mut out);
        out.push('\n');
    }
    if uses_http_client(program) {
        emit_http_client_helpers(&mut out);
        out.push('\n');
    }
    if uses_http_server(program) {
        emit_http_server_helpers(&mut out);
        out.push('\n');
    }
    if uses_env_get(program) {
        emit_env_get_helper(&mut out);
        out.push('\n');
    }
    if uses_env_set(program) {
        emit_env_set_helper(&mut out);
        out.push('\n');
    }
    if uses_env_cwd(program) {
        emit_env_cwd_helper(&mut out);
        out.push('\n');
    }
    if uses_env_home_dir(program) {
        emit_env_home_dir_helper(&mut out);
        out.push('\n');
    }
    if uses_env_temp_dir(program) {
        emit_env_temp_dir_helper(&mut out);
        out.push('\n');
    }
    if uses_process_spawn(program)
        || uses_process_status(program)
        || uses_process_exec(program)
        || uses_process_output(program)
    {
        emit_process_common_helpers(&mut out);
        out.push('\n');
    }
    if uses_process_spawn(program) || uses_process_status(program) {
        emit_process_spawn_helper(&mut out);
        out.push('\n');
    }
    if uses_process_status(program) {
        emit_process_status_helper(&mut out);
        out.push('\n');
    }
    if uses_process_exec(program) {
        emit_process_exec_helper(&mut out);
        out.push('\n');
    }
    if uses_process_output(program) {
        emit_process_output_helper(&mut out);
        out.push('\n');
    }
    if uses_json_builtin(program) {
        emit_json_helpers(&mut out);
        out.push('\n');
    }
    if uses_regex_builtin(program) {
        emit_regex_helpers(&mut out);
        out.push('\n');
    }
    if uses_num_parse_i64(program) {
        emit_num_parse_i64_helper(&mut out);
        out.push('\n');
    }
    if uses_num_parse_u64(program) {
        emit_num_parse_u64_helper(&mut out);
        out.push('\n');
    }
    if uses_num_parse_f64(program) {
        emit_num_parse_f64_helper(&mut out);
        out.push('\n');
    }
    let num_checked_binary_instances = collect_num_checked_binary_instances(program);
    for instance in &num_checked_binary_instances {
        emit_num_checked_binary_helper(&mut out, instance);
        out.push('\n');
    }

    let main = program
        .functions
        .iter()
        .find(|function| function.name == "main")
        .expect("checked programs always contain main");
    let main_returns_result = result_void_error(&main.return_type).is_some();

    for function in program
        .functions
        .iter()
        .filter(|function| function.name != "main" || main_returns_result)
    {
        emit_prototype(&mut out, function);
    }
    if program
        .functions
        .iter()
        .any(|function| function.name != "main" || main_returns_result)
    {
        out.push('\n');
    }

    let result_map_err_instances = collect_result_map_err_instances(program);
    for instance in &result_map_err_instances {
        emit_result_map_err_helper(&mut out, instance);
        out.push('\n');
    }
    let result_unwrap_or_instances = collect_result_unwrap_or_instances(program);
    for instance in &result_unwrap_or_instances {
        emit_result_unwrap_or_helper(&mut out, instance);
        out.push('\n');
    }
    let result_map_instances = collect_result_map_instances(program);
    for instance in &result_map_instances {
        emit_result_map_helper(&mut out, instance);
        out.push('\n');
    }
    let result_and_then_instances = collect_result_and_then_instances(program);
    for instance in &result_and_then_instances {
        emit_result_and_then_helper(&mut out, instance);
        out.push('\n');
    }
    let option_unwrap_or_instances = collect_option_unwrap_or_instances(program);
    for instance in &option_unwrap_or_instances {
        emit_option_unwrap_or_helper(&mut out, instance);
        out.push('\n');
    }
    let option_map_instances = collect_option_map_instances(program);
    for instance in &option_map_instances {
        emit_option_map_helper(&mut out, instance);
        out.push('\n');
    }
    let option_and_then_instances = collect_option_and_then_instances(program);
    for instance in &option_and_then_instances {
        emit_option_and_then_helper(&mut out, instance);
        out.push('\n');
    }

    for function in program
        .functions
        .iter()
        .filter(|function| function.name != "main" || main_returns_result)
    {
        emit_function(&mut out, function);
        out.push('\n');
    }

    if uses_env_args(program) {
        out.push_str("int main(int argc, char **argv) {\n");
    } else {
        out.push_str("int main(void) {\n");
    }
    if uses_env_args(program) {
        out.push_str("    nomo_argc = argc;\n");
        out.push_str("    nomo_argv = argv;\n");
    }
    if let Some(result_args) = result_void_error(&main.return_type) {
        let result_type = c_enum_ident("Result", &result_args);
        out.push_str("    ");
        out.push_str(&result_type);
        out.push_str(" nomo__result = ");
        out.push_str(&c_fn_ident("main"));
        out.push_str("();\n");
        out.push_str("    return nomo__result.tag == ");
        out.push_str(&c_enum_variant_ident("Result", &result_args, "Ok"));
        out.push_str(" ? 0 : 1;\n");
    } else {
        emit_body(&mut out, main);
        out.push_str("    return 0;\n");
    }
    out.push_str("}\n");
    out
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

fn emit_array_helpers(out: &mut String, element_type: &ValueType) {
    let array = c_array_ident(element_type);
    let option = c_enum_ident("Option", &[element_type.clone()]);
    let some = c_enum_variant_ident("Option", &[element_type.clone()], "Some");
    let none = c_enum_variant_ident("Option", &[element_type.clone()], "None");
    out.push_str("static ");
    out.push_str(&array);
    out.push(' ');
    out.push_str(&array);
    out.push_str("_new(void) {\n");
    out.push_str("    return (");
    out.push_str(&array);
    out.push_str("){.len = 0, .cap = 0, .data = NULL, .refcount = NULL};\n");
    out.push_str("}\n\n");
    out.push_str("static ");
    out.push_str(&array);
    out.push(' ');
    out.push_str(&array);
    out.push_str("_retain(");
    out.push_str(&array);
    out.push_str(" array) {\n");
    out.push_str("    if (array.refcount != NULL) { *array.refcount += 1; }\n");
    out.push_str("    return array;\n");
    out.push_str("}\n\n");
    out.push_str("static ");
    out.push_str("void ");
    out.push_str(&array);
    out.push_str("_release(");
    out.push_str(&array);
    out.push_str(" array) {\n");
    out.push_str("    if (array.refcount == NULL) { return; }\n");
    out.push_str("    *array.refcount -= 1;\n");
    out.push_str("    if (*array.refcount != 0) { return; }\n");
    emit_array_element_release_loop(out, element_type);
    out.push_str("    free(array.data);\n");
    out.push_str("    free(array.refcount);\n");
    out.push_str("}\n\n");
    out.push_str("static ");
    out.push_str(&array);
    out.push(' ');
    out.push_str(&array);
    out.push_str("_make_unique(");
    out.push_str(&array);
    out.push_str(" array, size_t needed) {\n");
    out.push_str("    size_t cap = array.cap;\n");
    out.push_str("    if (cap < needed) { cap = cap == 0 ? 4 : cap; }\n");
    out.push_str("    while (cap < needed) { cap *= 2; }\n");
    out.push_str("    if (cap == 0) { return array; }\n");
    out.push_str("    if (array.refcount != NULL && *array.refcount == 1 && array.cap >= needed) { return array; }\n");
    out.push_str("    ");
    out.push_str(&c_type(element_type));
    out.push_str(" *data = (");
    out.push_str(&c_type(element_type));
    out.push_str(" *)malloc(cap * sizeof(");
    out.push_str(&c_type(element_type));
    out.push_str("));\n");
    out.push_str("    if (data == NULL) { nomo_panic(\"out of memory\"); }\n");
    out.push_str("    for (size_t i = 0; i < array.len; i += 1) { data[i] = ");
    emit_array_element_retain_expr(out, element_type, "array.data[i]");
    out.push_str("; }\n");
    out.push_str("    size_t *refcount = (size_t *)malloc(sizeof(size_t));\n");
    out.push_str("    if (refcount == NULL) { nomo_panic(\"out of memory\"); }\n");
    out.push_str("    *refcount = 1;\n");
    out.push_str("    ");
    out.push_str(&array);
    out.push_str("_release(array);\n");
    out.push_str("    array.data = data;\n");
    out.push_str("    array.cap = cap;\n");
    out.push_str("    array.refcount = refcount;\n");
    out.push_str("    return array;\n");
    out.push_str("}\n\n");
    out.push_str("static ");
    out.push_str(&array);
    out.push(' ');
    out.push_str(&array);
    out.push_str("_push(");
    out.push_str(&array);
    out.push_str(" array, ");
    out.push_str(&c_type(element_type));
    out.push_str(" value) {\n");
    out.push_str("    array = ");
    out.push_str(&array);
    out.push_str("_make_unique(array, array.len + 1);\n");
    out.push_str("    array.data[array.len] = ");
    emit_array_element_retain_expr(out, element_type, "value");
    out.push_str(";\n");
    out.push_str("    array.len += 1;\n");
    out.push_str("    return array;\n");
    out.push_str("}\n\n");
    out.push_str("static ");
    out.push_str(&array);
    out.push(' ');
    out.push_str(&array);
    out.push_str("_insert(");
    out.push_str(&array);
    out.push_str(" array, uint64_t index, ");
    out.push_str(&c_type(element_type));
    out.push_str(" value) {\n");
    out.push_str(
        "    if (index > array.len) { nomo_panic(\"Array.insert index out of bounds\"); }\n",
    );
    out.push_str("    array = ");
    out.push_str(&array);
    out.push_str("_make_unique(array, array.len + 1);\n");
    out.push_str("    size_t insert_index = (size_t)index;\n");
    out.push_str("    for (size_t i = array.len; i > insert_index; i -= 1) { array.data[i] = array.data[i - 1]; }\n");
    out.push_str("    array.data[insert_index] = ");
    emit_array_element_retain_expr(out, element_type, "value");
    out.push_str(";\n");
    out.push_str("    array.len += 1;\n");
    out.push_str("    return array;\n");
    out.push_str("}\n\n");
    out.push_str("static ");
    out.push_str(&array);
    out.push(' ');
    out.push_str(&array);
    out.push_str("_clear(");
    out.push_str(&array);
    out.push_str(" array) {\n");
    out.push_str("    array = ");
    out.push_str(&array);
    out.push_str("_make_unique(array, array.len);\n");
    emit_array_element_release_loop(out, element_type);
    out.push_str("    array.len = 0;\n");
    out.push_str("    return array;\n");
    out.push_str("}\n\n");
    out.push_str("static ");
    out.push_str(&option);
    out.push(' ');
    out.push_str(&array);
    out.push_str("_pop(");
    out.push_str(&array);
    out.push_str(" *array) {\n");
    out.push_str("    if (array->len == 0) {\n");
    out.push_str("        return (");
    out.push_str(&option);
    out.push_str("){.tag = ");
    out.push_str(&none);
    out.push_str("};\n");
    out.push_str("    }\n");
    out.push_str("    *array = ");
    out.push_str(&array);
    out.push_str("_make_unique(*array, array->len);\n");
    out.push_str("    size_t index = array->len - 1;\n");
    out.push_str("    ");
    out.push_str(&c_type(element_type));
    out.push_str(" value = ");
    emit_array_element_retain_expr(out, element_type, "array->data[index]");
    out.push_str(";\n");
    emit_array_element_release_stmt(out, element_type, "array->data[index]");
    out.push_str("    array->len -= 1;\n");
    out.push_str("    return (");
    out.push_str(&option);
    out.push_str("){.tag = ");
    out.push_str(&some);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Some"));
    out.push_str(" = value};\n");
    out.push_str("}\n\n");
    out.push_str("static ");
    out.push_str(&option);
    out.push(' ');
    out.push_str(&array);
    out.push_str("_remove(");
    out.push_str(&array);
    out.push_str(" *array, uint64_t index) {\n");
    out.push_str("    if (index >= array->len) {\n");
    out.push_str("        return (");
    out.push_str(&option);
    out.push_str("){.tag = ");
    out.push_str(&none);
    out.push_str("};\n");
    out.push_str("    }\n");
    out.push_str("    *array = ");
    out.push_str(&array);
    out.push_str("_make_unique(*array, array->len);\n");
    out.push_str("    size_t remove_index = (size_t)index;\n");
    out.push_str("    ");
    out.push_str(&c_type(element_type));
    out.push_str(" value = ");
    emit_array_element_retain_expr(out, element_type, "array->data[remove_index]");
    out.push_str(";\n");
    emit_array_element_release_stmt(out, element_type, "array->data[remove_index]");
    out.push_str("    for (size_t i = remove_index; i + 1 < array->len; i += 1) { array->data[i] = array->data[i + 1]; }\n");
    out.push_str("    array->len -= 1;\n");
    out.push_str("    return (");
    out.push_str(&option);
    out.push_str("){.tag = ");
    out.push_str(&some);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Some"));
    out.push_str(" = value};\n");
    out.push_str("}\n\n");
    out.push_str("static ");
    out.push_str(&option);
    out.push(' ');
    out.push_str(&array);
    out.push_str("_get(");
    out.push_str(&array);
    out.push_str(" array, uint64_t index) {\n");
    out.push_str("    if (index >= array.len) {\n");
    out.push_str("        return (");
    out.push_str(&option);
    out.push_str("){.tag = ");
    out.push_str(&none);
    out.push_str("};\n");
    out.push_str("    }\n");
    out.push_str("    return (");
    out.push_str(&option);
    out.push_str("){.tag = ");
    out.push_str(&some);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Some"));
    out.push_str(" = ");
    emit_array_element_retain_expr(out, element_type, "array.data[index]");
    out.push_str("};\n");
    out.push_str("}\n\n");
    out.push_str("static ");
    out.push_str(&array);
    out.push(' ');
    out.push_str(&array);
    out.push_str("_set(");
    out.push_str(&array);
    out.push_str(" array, uint64_t index, ");
    out.push_str(&c_type(element_type));
    out.push_str(" value) {\n");
    out.push_str(
        "    if (index >= array.len) { nomo_panic(\"Array.set index out of bounds\"); }\n",
    );
    out.push_str("    array = ");
    out.push_str(&array);
    out.push_str("_make_unique(array, array.len);\n");
    emit_array_element_release_stmt(out, element_type, "array.data[index]");
    out.push_str("    array.data[index] = ");
    emit_array_element_retain_expr(out, element_type, "value");
    out.push_str(";\n");
    out.push_str("    return array;\n");
    out.push_str("}\n");
}

fn emit_string_split_helper(out: &mut String) {
    let array = c_array_ident(&ValueType::String);
    out.push_str("static ");
    out.push_str(&array);
    out.push_str(" nomo_string_split(nomo_string value, nomo_string separator) {\n");
    out.push_str("    size_t separator_len = strlen(separator.data);\n");
    out.push_str("    if (separator_len == 0) { nomo_panic(\"string.split separator must not be empty\"); }\n");
    out.push_str("    const char *data = value.data;\n");
    out.push_str("    size_t data_len = strlen(data);\n");
    out.push_str("    size_t start = 0;\n");
    out.push_str("    ");
    out.push_str(&array);
    out.push_str(" parts = ");
    out.push_str(&array);
    out.push_str("_new();\n");
    out.push_str("    while (start <= data_len) {\n");
    out.push_str("        const char *found = strstr(data + start, separator.data);\n");
    out.push_str("        size_t segment_len = found == NULL ? data_len - start : (size_t)(found - (data + start));\n");
    out.push_str(
        "        nomo_string segment = nomo_string_from_slice(data, start, segment_len);\n",
    );
    out.push_str("        parts = ");
    out.push_str(&array);
    out.push_str("_push(parts, segment);\n");
    out.push_str("        nomo_string_release(segment);\n");
    out.push_str("        if (found == NULL) { break; }\n");
    out.push_str("        start = (size_t)(found - data) + separator_len;\n");
    out.push_str("    }\n");
    out.push_str("    return parts;\n");
    out.push_str("}\n");
}

fn emit_collections_helpers(out: &mut String) {
    let string_array = c_array_ident(&ValueType::String);
    let map_type = ValueType::Struct("StringMap".to_string(), Vec::new());
    let set_type = ValueType::Struct("StringSet".to_string(), Vec::new());
    let map = c_type(&map_type);
    let set = c_type(&set_type);
    let option_string = c_enum_ident("Option", &[ValueType::String]);
    let option_some = c_enum_variant_ident("Option", &[ValueType::String], "Some");
    let option_none = c_enum_variant_ident("Option", &[ValueType::String], "None");
    let keys = c_member_ident("keys");
    let values = c_member_ident("values");
    let payload_some = c_payload_ident("Some");

    out.push_str("static ");
    out.push_str(&map);
    out.push_str(" nomo_collections_map_new(void) {\n");
    out.push_str("    return (");
    out.push_str(&map);
    out.push_str("){.");
    out.push_str(&keys);
    out.push_str(" = ");
    out.push_str(&string_array);
    out.push_str("_new(), .");
    out.push_str(&values);
    out.push_str(" = ");
    out.push_str(&string_array);
    out.push_str("_new()};\n");
    out.push_str("}\n\n");

    out.push_str("static uint64_t nomo_collections_map_len(");
    out.push_str(&map);
    out.push_str(" map) {\n");
    out.push_str("    return (uint64_t)map.");
    out.push_str(&keys);
    out.push_str(".len;\n");
    out.push_str("}\n\n");

    out.push_str("static ");
    out.push_str(&option_string);
    out.push_str(" nomo_collections_map_get(");
    out.push_str(&map);
    out.push_str(" map, nomo_string key) {\n");
    out.push_str("    for (size_t i = 0; i < map.");
    out.push_str(&keys);
    out.push_str(".len; i += 1) {\n");
    out.push_str("        if (nomo_string_equal(map.");
    out.push_str(&keys);
    out.push_str(".data[i], key)) {\n");
    out.push_str("            return (");
    out.push_str(&option_string);
    out.push_str("){.tag = ");
    out.push_str(&option_some);
    out.push_str(", .payload.");
    out.push_str(&payload_some);
    out.push_str(" = nomo_string_retain(map.");
    out.push_str(&values);
    out.push_str(".data[i])};\n");
    out.push_str("        }\n");
    out.push_str("    }\n");
    out.push_str("    return (");
    out.push_str(&option_string);
    out.push_str("){.tag = ");
    out.push_str(&option_none);
    out.push_str("};\n");
    out.push_str("}\n\n");

    out.push_str("static int nomo_collections_map_contains(");
    out.push_str(&map);
    out.push_str(" map, nomo_string key) {\n");
    out.push_str("    for (size_t i = 0; i < map.");
    out.push_str(&keys);
    out.push_str(".len; i += 1) {\n");
    out.push_str("        if (nomo_string_equal(map.");
    out.push_str(&keys);
    out.push_str(".data[i], key)) { return 1; }\n");
    out.push_str("    }\n");
    out.push_str("    return 0;\n");
    out.push_str("}\n\n");

    out.push_str("static ");
    out.push_str(&map);
    out.push_str(" nomo_collections_map_set(");
    out.push_str(&map);
    out.push_str(" map, nomo_string key, nomo_string value) {\n");
    out.push_str("    map = ");
    out.push_str(&c_retain_ident(&map_type));
    out.push_str("(map);\n");
    out.push_str("    for (size_t i = 0; i < map.");
    out.push_str(&keys);
    out.push_str(".len; i += 1) {\n");
    out.push_str("        if (nomo_string_equal(map.");
    out.push_str(&keys);
    out.push_str(".data[i], key)) {\n");
    out.push_str("            map.");
    out.push_str(&values);
    out.push_str(" = ");
    out.push_str(&string_array);
    out.push_str("_set(map.");
    out.push_str(&values);
    out.push_str(", (uint64_t)i, value);\n");
    out.push_str("            return map;\n");
    out.push_str("        }\n");
    out.push_str("    }\n");
    out.push_str("    map.");
    out.push_str(&keys);
    out.push_str(" = ");
    out.push_str(&string_array);
    out.push_str("_push(map.");
    out.push_str(&keys);
    out.push_str(", key);\n");
    out.push_str("    map.");
    out.push_str(&values);
    out.push_str(" = ");
    out.push_str(&string_array);
    out.push_str("_push(map.");
    out.push_str(&values);
    out.push_str(", value);\n");
    out.push_str("    return map;\n");
    out.push_str("}\n\n");

    out.push_str("static ");
    out.push_str(&map);
    out.push_str(" nomo_collections_map_remove(");
    out.push_str(&map);
    out.push_str(" map, nomo_string key) {\n");
    out.push_str("    map = ");
    out.push_str(&c_retain_ident(&map_type));
    out.push_str("(map);\n");
    out.push_str("    for (size_t i = 0; i < map.");
    out.push_str(&keys);
    out.push_str(".len; i += 1) {\n");
    out.push_str("        if (nomo_string_equal(map.");
    out.push_str(&keys);
    out.push_str(".data[i], key)) {\n");
    out.push_str("            ");
    out.push_str(&option_string);
    out.push_str(" removed_key = ");
    out.push_str(&string_array);
    out.push_str("_remove(&map.");
    out.push_str(&keys);
    out.push_str(", (uint64_t)i);\n");
    out.push_str("            ");
    out.push_str(&option_string);
    out.push_str(" removed_value = ");
    out.push_str(&string_array);
    out.push_str("_remove(&map.");
    out.push_str(&values);
    out.push_str(", (uint64_t)i);\n");
    out.push_str("            ");
    out.push_str(&option_string);
    out.push_str("_release(removed_key);\n");
    out.push_str("            ");
    out.push_str(&option_string);
    out.push_str("_release(removed_value);\n");
    out.push_str("            return map;\n");
    out.push_str("        }\n");
    out.push_str("    }\n");
    out.push_str("    return map;\n");
    out.push_str("}\n\n");

    out.push_str("static ");
    out.push_str(&set);
    out.push_str(" nomo_collections_set_new(void) {\n");
    out.push_str("    return (");
    out.push_str(&set);
    out.push_str("){.");
    out.push_str(&values);
    out.push_str(" = ");
    out.push_str(&string_array);
    out.push_str("_new()};\n");
    out.push_str("}\n\n");

    out.push_str("static uint64_t nomo_collections_set_len(");
    out.push_str(&set);
    out.push_str(" set) {\n");
    out.push_str("    return (uint64_t)set.");
    out.push_str(&values);
    out.push_str(".len;\n");
    out.push_str("}\n\n");

    out.push_str("static int nomo_collections_set_contains(");
    out.push_str(&set);
    out.push_str(" set, nomo_string value) {\n");
    out.push_str("    for (size_t i = 0; i < set.");
    out.push_str(&values);
    out.push_str(".len; i += 1) {\n");
    out.push_str("        if (nomo_string_equal(set.");
    out.push_str(&values);
    out.push_str(".data[i], value)) { return 1; }\n");
    out.push_str("    }\n");
    out.push_str("    return 0;\n");
    out.push_str("}\n\n");

    out.push_str("static ");
    out.push_str(&set);
    out.push_str(" nomo_collections_set_insert(");
    out.push_str(&set);
    out.push_str(" set, nomo_string value) {\n");
    out.push_str("    set = ");
    out.push_str(&c_retain_ident(&set_type));
    out.push_str("(set);\n");
    out.push_str("    if (!nomo_collections_set_contains(set, value)) {\n");
    out.push_str("        set.");
    out.push_str(&values);
    out.push_str(" = ");
    out.push_str(&string_array);
    out.push_str("_push(set.");
    out.push_str(&values);
    out.push_str(", value);\n");
    out.push_str("    }\n");
    out.push_str("    return set;\n");
    out.push_str("}\n\n");

    out.push_str("static ");
    out.push_str(&set);
    out.push_str(" nomo_collections_set_remove(");
    out.push_str(&set);
    out.push_str(" set, nomo_string value) {\n");
    out.push_str("    set = ");
    out.push_str(&c_retain_ident(&set_type));
    out.push_str("(set);\n");
    out.push_str("    for (size_t i = 0; i < set.");
    out.push_str(&values);
    out.push_str(".len; i += 1) {\n");
    out.push_str("        if (nomo_string_equal(set.");
    out.push_str(&values);
    out.push_str(".data[i], value)) {\n");
    out.push_str("            ");
    out.push_str(&option_string);
    out.push_str(" removed = ");
    out.push_str(&string_array);
    out.push_str("_remove(&set.");
    out.push_str(&values);
    out.push_str(", (uint64_t)i);\n");
    out.push_str("            ");
    out.push_str(&option_string);
    out.push_str("_release(removed);\n");
    out.push_str("            return set;\n");
    out.push_str("        }\n");
    out.push_str("    }\n");
    out.push_str("    return set;\n");
    out.push_str("}\n");
}

fn emit_array_element_release_loop(out: &mut String, element_type: &ValueType) {
    if value_type_needs_release(element_type) {
        out.push_str("    for (size_t i = 0; i < array.len; i += 1) { ");
        emit_array_element_release_expr(out, element_type, "array.data[i]");
        out.push_str("; }\n");
    }
}

fn emit_array_element_release_stmt(out: &mut String, element_type: &ValueType, value: &str) {
    if value_type_needs_release(element_type) {
        out.push_str("    ");
        emit_array_element_release_expr(out, element_type, value);
        out.push_str(";\n");
    }
}

fn emit_array_element_release_expr(out: &mut String, element_type: &ValueType, value: &str) {
    if value_type_needs_release(element_type) {
        out.push_str(&c_release_ident(element_type));
        out.push('(');
        out.push_str(value);
        out.push(')');
    }
}

fn emit_array_element_retain_expr(out: &mut String, element_type: &ValueType, value: &str) {
    if value_type_needs_release(element_type) {
        out.push_str(&c_retain_ident(element_type));
        out.push('(');
        out.push_str(value);
        out.push(')');
    } else {
        out.push_str(value);
    }
}

#[cfg(test)]
mod tests;
