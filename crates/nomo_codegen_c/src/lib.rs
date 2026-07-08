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

mod host_helpers;
mod instances;
mod names;
mod runtime;
mod usage;
use host_helpers::*;
use instances::*;
use names::*;
use runtime::*;
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
struct ResultMapErrInstance {
    ok_type: ValueType,
    source_err_type: ValueType,
    target_err_type: ValueType,
    converter: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ResultUnwrapOrInstance {
    ok_type: ValueType,
    err_type: ValueType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ResultMapInstance {
    source_ok_type: ValueType,
    target_ok_type: ValueType,
    err_type: ValueType,
    converter: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ResultAndThenInstance {
    source_ok_type: ValueType,
    target_ok_type: ValueType,
    err_type: ValueType,
    converter: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct OptionUnwrapOrInstance {
    payload_type: ValueType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct OptionMapInstance {
    source_type: ValueType,
    target_type: ValueType,
    converter: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct OptionAndThenInstance {
    source_type: ValueType,
    target_type: ValueType,
    converter: String,
}

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

fn emit_type_name_macros(out: &mut String, program: &Program) {
    for (struct_name, struct_args) in collect_struct_instances(program) {
        let struct_type = program
            .structs
            .iter()
            .find(|item| item.name == struct_name)
            .expect("checked programs only use known structs");
        let package = c_package_ident(&struct_type.package);
        let local = c_struct_ident(&struct_name, &struct_args);
        let suffix = c_type_suffix(&struct_args);
        out.push_str("#define ");
        out.push_str(&local);
        out.push_str(" nomo_pkg_");
        out.push_str(&package);
        out.push_str("_struct_");
        out.push_str(&struct_name);
        out.push_str(&suffix);
        out.push('\n');
    }
    for (enum_name, enum_args) in collect_enum_instances(program) {
        let enum_type = program
            .enums
            .iter()
            .find(|item| item.name == enum_name)
            .expect("checked programs only use known enums");
        let package = c_package_ident(&enum_type.package);
        let suffix = c_type_suffix(&enum_args);
        out.push_str("#define ");
        out.push_str(&c_enum_tag_ident(&enum_name, &enum_args));
        out.push_str(" nomo_pkg_");
        out.push_str(&package);
        out.push_str("_enum_");
        out.push_str(&enum_name);
        out.push_str(&suffix);
        out.push_str("_tag\n");
        out.push_str("#define ");
        out.push_str(&c_enum_ident(&enum_name, &enum_args));
        out.push_str(" nomo_pkg_");
        out.push_str(&package);
        out.push_str("_enum_");
        out.push_str(&enum_name);
        out.push_str(&suffix);
        out.push('\n');
        for variant in &enum_type.variants {
            out.push_str("#define ");
            out.push_str(&c_enum_variant_ident(&enum_name, &enum_args, &variant.name));
            out.push_str(" nomo_pkg_");
            out.push_str(&package);
            out.push_str("_enum_");
            out.push_str(&enum_name);
            out.push_str(&suffix);
            out.push('_');
            out.push_str(&variant.name);
            out.push('\n');
        }
    }
    if !program.structs.is_empty() || !program.enums.is_empty() {
        out.push('\n');
    }
}

fn emit_type_forward_declarations(
    out: &mut String,
    program: &Program,
    array_element_types: &[ValueType],
) {
    let mut emitted = false;
    for (struct_name, struct_args) in collect_struct_instances(program) {
        out.push_str("typedef struct ");
        out.push_str(&c_struct_ident(&struct_name, &struct_args));
        out.push(' ');
        out.push_str(&c_struct_ident(&struct_name, &struct_args));
        out.push_str(";\n");
        emitted = true;
    }
    for (enum_name, enum_args) in collect_enum_instances(program) {
        out.push_str("typedef struct ");
        out.push_str(&c_enum_ident(&enum_name, &enum_args));
        out.push(' ');
        out.push_str(&c_enum_ident(&enum_name, &enum_args));
        out.push_str(";\n");
        emitted = true;
    }
    for element_type in array_element_types {
        out.push_str("typedef struct ");
        out.push_str(&c_array_ident(element_type));
        out.push(' ');
        out.push_str(&c_array_ident(element_type));
        out.push_str(";\n");
        emitted = true;
    }
    if emitted {
        out.push('\n');
    }
}

fn emit_lifecycle_helper_prototypes(
    out: &mut String,
    program: &Program,
    array_element_types: &[ValueType],
) {
    let mut emitted = false;
    for element_type in array_element_types {
        let array_type = ValueType::Array(Box::new(element_type.clone()));
        emit_retain_prototype(out, &array_type);
        emit_release_prototype(out, &array_type);
        emitted = true;
    }
    for (name, args) in collect_struct_instances(program) {
        let value_type = ValueType::Struct(name, args);
        emit_retain_prototype(out, &value_type);
        emit_release_prototype(out, &value_type);
        emitted = true;
    }
    for (name, args) in collect_enum_instances(program) {
        let value_type = ValueType::Enum(name, args);
        emit_retain_prototype(out, &value_type);
        emit_release_prototype(out, &value_type);
        emitted = true;
    }
    if emitted {
        out.push('\n');
    }
}

fn emit_retain_prototype(out: &mut String, value_type: &ValueType) {
    out.push_str("static ");
    out.push_str(&c_type(value_type));
    out.push(' ');
    out.push_str(&c_retain_ident(value_type));
    out.push('(');
    out.push_str(&c_type(value_type));
    out.push_str(" value);\n");
}

fn emit_release_prototype(out: &mut String, value_type: &ValueType) {
    out.push_str("static void ");
    out.push_str(&c_release_ident(value_type));
    out.push('(');
    out.push_str(&c_type(value_type));
    out.push_str(" value);\n");
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

#[derive(Debug, Clone, PartialEq, Eq)]
enum TypeInstance {
    Struct(String, Vec<ValueType>),
    Enum(String, Vec<ValueType>),
}

fn emit_struct_and_enum_types(out: &mut String, program: &Program) {
    let mut remaining = collect_struct_instances(program)
        .into_iter()
        .map(|(name, args)| TypeInstance::Struct(name, args))
        .chain(
            collect_enum_instances(program)
                .into_iter()
                .map(|(name, args)| TypeInstance::Enum(name, args)),
        )
        .collect::<Vec<_>>();
    let mut defined = BTreeSet::new();

    while !remaining.is_empty() {
        let mut index = 0;
        let mut emitted_any = false;
        while index < remaining.len() {
            if type_instance_dependencies_satisfied(program, &remaining[index], &defined) {
                let item = remaining.remove(index);
                emit_type_instance(out, program, &item);
                out.push('\n');
                defined.insert(type_instance_key(&item));
                emitted_any = true;
            } else {
                index += 1;
            }
        }
        if !emitted_any {
            for item in remaining.drain(..) {
                emit_type_instance(out, program, &item);
                out.push('\n');
            }
        }
    }
}

fn emit_type_instance(out: &mut String, program: &Program, item: &TypeInstance) {
    match item {
        TypeInstance::Struct(name, args) => {
            let struct_type = program
                .structs
                .iter()
                .find(|item| item.name == *name)
                .expect("checked programs only use known structs");
            emit_struct_type(out, struct_type, args);
        }
        TypeInstance::Enum(name, args) => {
            let enum_type = program
                .enums
                .iter()
                .find(|item| item.name == *name)
                .expect("checked programs only use known enums");
            emit_enum_type(out, enum_type, args);
        }
    }
}

fn type_instance_dependencies_satisfied(
    program: &Program,
    item: &TypeInstance,
    defined: &BTreeSet<String>,
) -> bool {
    let deps = match item {
        TypeInstance::Struct(name, args) => {
            let struct_type = program
                .structs
                .iter()
                .find(|item| item.name == *name)
                .expect("checked programs only use known structs");
            let mut deps = BTreeSet::new();
            for field in &struct_type.fields {
                let field_type = subst_type(&field.value_type, &struct_type.type_params, args);
                collect_complete_type_dependencies(&field_type, &mut deps);
            }
            deps
        }
        TypeInstance::Enum(name, args) => {
            let enum_type = program
                .enums
                .iter()
                .find(|item| item.name == *name)
                .expect("checked programs only use known enums");
            let mut deps = BTreeSet::new();
            for variant in &enum_type.variants {
                if let Some(payload) = &variant.payload {
                    let payload_type = subst_type(payload, &enum_type.type_params, args);
                    collect_complete_type_dependencies(&payload_type, &mut deps);
                }
            }
            deps
        }
    };
    let self_key = type_instance_key(item);
    deps.iter()
        .filter(|dep| dep.as_str() != self_key)
        .all(|dep| defined.contains(dep))
}

fn collect_complete_type_dependencies(value_type: &ValueType, out: &mut BTreeSet<String>) {
    match value_type {
        ValueType::Struct(name, args) => {
            out.insert(type_instance_key(&TypeInstance::Struct(
                name.clone(),
                args.clone(),
            )));
            for arg in args {
                collect_complete_type_dependencies(arg, out);
            }
        }
        ValueType::Enum(name, args) => {
            out.insert(type_instance_key(&TypeInstance::Enum(
                name.clone(),
                args.clone(),
            )));
            for arg in args {
                collect_complete_type_dependencies(arg, out);
            }
        }
        ValueType::Array(_) => {}
        ValueType::String
        | ValueType::Int
        | ValueType::I32
        | ValueType::U32
        | ValueType::U64
        | ValueType::Float
        | ValueType::Char
        | ValueType::Bool
        | ValueType::TypeParam(_)
        | ValueType::Void
        | ValueType::Never => {}
    }
}

fn type_instance_key(item: &TypeInstance) -> String {
    match item {
        TypeInstance::Struct(name, args) => c_struct_ident(name, args),
        TypeInstance::Enum(name, args) => c_enum_ident(name, args),
    }
}

fn emit_struct_type(out: &mut String, struct_type: &StructType, struct_args: &[ValueType]) {
    if struct_type.name == "File" && struct_args.is_empty() {
        out.push_str("struct ");
        out.push_str(&c_struct_ident(&struct_type.name, struct_args));
        out.push_str(" {\n");
        out.push_str("    FILE *");
        out.push_str(&c_member_ident("handle"));
        out.push_str(";\n");
        out.push_str("};\n");
        return;
    }
    if struct_type.name == "TcpStream" && struct_args.is_empty() {
        out.push_str("struct ");
        out.push_str(&c_struct_ident(&struct_type.name, struct_args));
        out.push_str(" {\n");
        out.push_str("    nomo_socket ");
        out.push_str(&c_member_ident("handle"));
        out.push_str(";\n");
        out.push_str("};\n");
        return;
    }
    if struct_type.name == "TcpListener" && struct_args.is_empty() {
        out.push_str("struct ");
        out.push_str(&c_struct_ident(&struct_type.name, struct_args));
        out.push_str(" {\n");
        out.push_str("    nomo_socket ");
        out.push_str(&c_member_ident("handle"));
        out.push_str(";\n");
        out.push_str("};\n");
        return;
    }
    if struct_type.name == "UdpSocket" && struct_args.is_empty() {
        out.push_str("struct ");
        out.push_str(&c_struct_ident(&struct_type.name, struct_args));
        out.push_str(" {\n");
        out.push_str("    nomo_socket ");
        out.push_str(&c_member_ident("handle"));
        out.push_str(";\n");
        out.push_str("};\n");
        return;
    }
    if struct_type.name == "HttpServer" && struct_args.is_empty() {
        out.push_str("struct ");
        out.push_str(&c_struct_ident(&struct_type.name, struct_args));
        out.push_str(" {\n");
        out.push_str("    nomo_socket ");
        out.push_str(&c_member_ident("handle"));
        out.push_str(";\n");
        out.push_str("};\n");
        return;
    }
    if struct_type.name == "HttpExchange" && struct_args.is_empty() {
        out.push_str("struct ");
        out.push_str(&c_struct_ident(&struct_type.name, struct_args));
        out.push_str(" {\n");
        out.push_str("    nomo_socket ");
        out.push_str(&c_member_ident("handle"));
        out.push_str(";\n");
        for field in &struct_type.fields {
            out.push_str("    ");
            out.push_str(&c_type(&field.value_type));
            out.push(' ');
            out.push_str(&c_member_ident(&field.name));
            out.push_str(";\n");
        }
        out.push_str("};\n");
        return;
    }
    out.push_str("struct ");
    out.push_str(&c_struct_ident(&struct_type.name, struct_args));
    out.push_str(" {\n");
    for field in &struct_type.fields {
        out.push_str("    ");
        out.push_str(&c_type(&subst_type(
            &field.value_type,
            &struct_type.type_params,
            struct_args,
        )));
        out.push(' ');
        out.push_str(&c_member_ident(&field.name));
        out.push_str(";\n");
    }
    out.push_str("};\n");
}

fn emit_enum_type(out: &mut String, enum_type: &EnumType, enum_args: &[ValueType]) {
    out.push_str("typedef enum ");
    out.push_str(&c_enum_tag_ident(&enum_type.name, enum_args));
    out.push_str(" {\n");
    for (index, variant) in enum_type.variants.iter().enumerate() {
        out.push_str("    ");
        out.push_str(&c_enum_variant_ident(
            &enum_type.name,
            enum_args,
            &variant.name,
        ));
        if index + 1 != enum_type.variants.len() {
            out.push(',');
        }
        out.push('\n');
    }
    out.push_str("} ");
    out.push_str(&c_enum_tag_ident(&enum_type.name, enum_args));
    out.push_str(";\n\n");
    out.push_str("struct ");
    out.push_str(&c_enum_ident(&enum_type.name, enum_args));
    out.push_str(" {\n");
    out.push_str("    ");
    out.push_str(&c_enum_tag_ident(&enum_type.name, enum_args));
    out.push_str(" tag;\n");
    if enum_type
        .variants
        .iter()
        .any(|variant| variant.payload.is_some())
    {
        out.push_str("    union {\n");
        for variant in enum_type
            .variants
            .iter()
            .filter(|variant| variant.payload.is_some())
        {
            out.push_str("        ");
            out.push_str(&c_payload_type(&subst_type(
                variant.payload.as_ref().unwrap(),
                &enum_type.type_params,
                enum_args,
            )));
            out.push(' ');
            out.push_str(&c_payload_ident(&variant.name));
            out.push_str(";\n");
        }
        out.push_str("    } payload;\n");
    }
    out.push_str("};\n");
}

fn emit_nominal_lifecycle_helpers(out: &mut String, program: &Program) {
    for (name, args) in collect_struct_instances(program) {
        let struct_type = program
            .structs
            .iter()
            .find(|item| item.name == name)
            .expect("checked programs only use known structs");
        emit_struct_lifecycle_helpers(out, struct_type, &args);
        out.push('\n');
    }
    for (name, args) in collect_enum_instances(program) {
        let enum_type = program
            .enums
            .iter()
            .find(|item| item.name == name)
            .expect("checked programs only use known enums");
        emit_enum_lifecycle_helpers(out, enum_type, &args);
        out.push('\n');
    }
}

fn emit_struct_lifecycle_helpers(
    out: &mut String,
    struct_type: &StructType,
    struct_args: &[ValueType],
) {
    let value_type = ValueType::Struct(struct_type.name.clone(), struct_args.to_vec());
    let c_type_name = c_type(&value_type);
    out.push_str("static ");
    out.push_str(&c_type_name);
    out.push(' ');
    out.push_str(&c_retain_ident(&value_type));
    out.push('(');
    out.push_str(&c_type_name);
    out.push_str(" value) {\n");
    for field in &struct_type.fields {
        let field_type = subst_type(&field.value_type, &struct_type.type_params, struct_args);
        if value_type_needs_release(&field_type) {
            let field = format!("value.{}", c_member_ident(&field.name));
            emit_value_retain_in_place(out, &field_type, &field, 1);
        }
    }
    out.push_str("    return value;\n");
    out.push_str("}\n\n");

    out.push_str("static void ");
    out.push_str(&c_release_ident(&value_type));
    out.push('(');
    out.push_str(&c_type_name);
    out.push_str(" value) {\n");
    for field in &struct_type.fields {
        let field_type = subst_type(&field.value_type, &struct_type.type_params, struct_args);
        if value_type_needs_release(&field_type) {
            let field = format!("value.{}", c_member_ident(&field.name));
            emit_value_release_in_place(out, &field_type, &field, 1);
        }
    }
    out.push_str("}\n");
}

fn emit_enum_lifecycle_helpers(out: &mut String, enum_type: &EnumType, enum_args: &[ValueType]) {
    let value_type = ValueType::Enum(enum_type.name.clone(), enum_args.to_vec());
    let c_type_name = c_type(&value_type);
    out.push_str("static ");
    out.push_str(&c_type_name);
    out.push(' ');
    out.push_str(&c_retain_ident(&value_type));
    out.push('(');
    out.push_str(&c_type_name);
    out.push_str(" value) {\n");
    for variant in &enum_type.variants {
        let Some(payload_type) = &variant.payload else {
            continue;
        };
        let payload_type = subst_type(payload_type, &enum_type.type_params, enum_args);
        if value_type_needs_release(&payload_type) {
            write_indent(out, 1);
            out.push_str("if (value.tag == ");
            out.push_str(&c_enum_variant_ident(
                &enum_type.name,
                enum_args,
                &variant.name,
            ));
            out.push_str(") {\n");
            let payload = format!("value.payload.{}", c_payload_ident(&variant.name));
            emit_value_retain_in_place(out, &payload_type, &payload, 2);
            out.push_str("    }\n");
        }
    }
    out.push_str("    return value;\n");
    out.push_str("}\n\n");

    out.push_str("static void ");
    out.push_str(&c_release_ident(&value_type));
    out.push('(');
    out.push_str(&c_type_name);
    out.push_str(" value) {\n");
    for variant in &enum_type.variants {
        let Some(payload_type) = &variant.payload else {
            continue;
        };
        let payload_type = subst_type(payload_type, &enum_type.type_params, enum_args);
        if value_type_needs_release(&payload_type) {
            write_indent(out, 1);
            out.push_str("if (value.tag == ");
            out.push_str(&c_enum_variant_ident(
                &enum_type.name,
                enum_args,
                &variant.name,
            ));
            out.push_str(") {\n");
            let payload = format!("value.payload.{}", c_payload_ident(&variant.name));
            emit_value_release_in_place(out, &payload_type, &payload, 2);
            out.push_str("    }\n");
        }
    }
    out.push_str("}\n");
}

fn emit_array_type(out: &mut String, element_type: &ValueType) {
    let array = c_array_ident(element_type);
    out.push_str("struct ");
    out.push_str(&array);
    out.push_str(" {\n");
    out.push_str("    size_t len;\n");
    out.push_str("    size_t cap;\n");
    out.push_str("    ");
    out.push_str(&c_type(element_type));
    out.push_str(" *data;\n");
    out.push_str("    size_t *refcount;\n");
    out.push_str("};\n");
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

fn emit_function(out: &mut String, function: &Function) {
    emit_signature(out, function);
    out.push_str(" {\n");
    emit_mut_param_macros(out, function);
    emit_body(out, function);
    if function.return_type == ValueType::Void {
        out.push_str("    return;\n");
    }
    emit_mut_param_undefs(out, function);
    out.push_str("}\n");
}

fn emit_signature(out: &mut String, function: &Function) {
    out.push_str(&c_type(&function.return_type));
    out.push(' ');
    out.push_str(&c_fn_ident(&function.name));
    out.push('(');
    if function.params.is_empty() {
        out.push_str("void");
    } else {
        for (index, param) in function.params.iter().enumerate() {
            if index > 0 {
                out.push_str(", ");
            }
            out.push_str(&c_type(&param.value_type));
            if param.mutable {
                out.push_str(" *");
            }
            out.push(' ');
            out.push_str(&c_var_ident(&param.name));
        }
    }
    out.push(')');
}

fn emit_mut_param_macros(out: &mut String, function: &Function) {
    for param in &function.params {
        if param.mutable {
            let name = c_var_ident(&param.name);
            out.push_str("#define ");
            out.push_str(&name);
            out.push_str(" (*");
            out.push_str(&name);
            out.push_str(")\n");
        }
    }
}

fn emit_mut_param_undefs(out: &mut String, function: &Function) {
    for param in &function.params {
        if param.mutable {
            out.push_str("#undef ");
            out.push_str(&c_var_ident(&param.name));
            out.push('\n');
        }
    }
}

fn emit_body(out: &mut String, function: &Function) {
    let mut deferred: Vec<DeferredCall> = Vec::new();
    let mut active_arrays = array_params(function);
    for local in &active_arrays {
        emit_array_retain_binding(out, &local.name, &local.value_type, 1);
    }
    let mut last_statement_exits = false;
    for statement in &function.body {
        if let Statement::Defer { call } = statement {
            deferred.push(call.clone());
        } else {
            emit_stmt(
                out,
                statement,
                1,
                &deferred,
                &function.return_type,
                &active_arrays,
                0,
                0,
                0,
                0,
            );
            if let Some(local) = local_array_from_statement(statement) {
                active_arrays.push(local);
            }
            last_statement_exits = statement_exits_function(statement);
        }
    }
    if !last_statement_exits {
        emit_deferred(out, 1, &deferred);
        emit_array_releases(out, 1, &active_arrays);
    }
}

fn write_indent(out: &mut String, indent: usize) {
    for _ in 0..indent {
        out.push_str("    ");
    }
}

fn emit_deferred(out: &mut String, indent: usize, deferred: &[DeferredCall]) {
    for call in deferred.iter().rev() {
        emit_deferred_call(out, indent, call);
    }
}

fn emit_deferred_call(out: &mut String, indent: usize, call: &DeferredCall) {
    match call {
        DeferredCall::Expr(expr) => {
            write_indent(out, indent);
            emit_expr(out, expr);
            out.push_str(";\n");
        }
        DeferredCall::Println(arg) => {
            write_indent(out, indent);
            out.push_str("puts(");
            emit_string_data_expr(out, arg);
            out.push_str(");\n");
        }
        DeferredCall::Print(arg) => {
            write_indent(out, indent);
            out.push_str("fputs(");
            emit_string_data_expr(out, arg);
            out.push_str(", stdout);\n");
        }
        DeferredCall::Eprintln(arg) => {
            write_indent(out, indent);
            out.push_str("fputs(");
            emit_string_data_expr(out, arg);
            out.push_str(", stderr);\n");
            write_indent(out, indent);
            out.push_str("fputc('\\n', stderr);\n");
        }
        DeferredCall::Eprint(arg) => {
            write_indent(out, indent);
            out.push_str("fputs(");
            emit_string_data_expr(out, arg);
            out.push_str(", stderr);\n");
        }
    }
}

fn statement_exits_function(statement: &Statement) -> bool {
    match statement {
        Statement::Return(_) | Statement::QuestionReturn { .. } | Statement::Panic(_) => true,
        Statement::Match { arms, .. } => arms.iter().all(|arm| statements_exit_function(&arm.body)),
        Statement::If {
            body, else_body, ..
        } => statements_exit_function(body) && statements_exit_function(else_body),
        Statement::IfLet {
            body,
            else_body: Some(else_body),
            ..
        } => statements_exit_function(body) && statements_exit_function(else_body),
        _ => false,
    }
}

fn statements_exit_function(statements: &[Statement]) -> bool {
    statements.last().is_some_and(statement_exits_function)
}

fn statement_exits_block(statement: &Statement) -> bool {
    match statement {
        Statement::Return(_)
        | Statement::QuestionReturn { .. }
        | Statement::Panic(_)
        | Statement::Break
        | Statement::Continue => true,
        Statement::Match { arms, .. } => arms.iter().all(|arm| statements_exit_block(&arm.body)),
        Statement::If {
            body, else_body, ..
        } => statements_exit_block(body) && statements_exit_block(else_body),
        Statement::IfLet {
            body,
            else_body: Some(else_body),
            ..
        } => statements_exit_block(body) && statements_exit_block(else_body),
        _ => false,
    }
}

fn statements_exit_block(statements: &[Statement]) -> bool {
    statements.last().is_some_and(statement_exits_block)
}

fn emit_stmt(
    out: &mut String,
    statement: &Statement,
    indent: usize,
    deferred: &[DeferredCall],
    function_return_type: &ValueType,
    active_arrays: &[LocalArray],
    break_deferred_start: usize,
    continue_deferred_start: usize,
    break_cleanup_start: usize,
    continue_cleanup_start: usize,
) {
    match statement {
        Statement::Let {
            name,
            value_type,
            initializer,
        } => emit_let(out, name, value_type, initializer, indent),
        Statement::LetIf {
            name,
            value_type,
            condition,
            body,
            else_body,
        } => emit_let_if(
            out,
            name,
            value_type,
            condition,
            body,
            else_body,
            indent,
            deferred,
            function_return_type,
            active_arrays,
            break_deferred_start,
            continue_deferred_start,
            break_cleanup_start,
            continue_cleanup_start,
        ),
        Statement::LetMatch {
            name,
            value_type,
            value,
            enum_name,
            enum_args,
            arms,
        } => emit_let_match(
            out,
            name,
            value_type,
            value,
            enum_name,
            enum_args,
            arms,
            indent,
            deferred,
            function_return_type,
            active_arrays,
            break_deferred_start,
            continue_deferred_start,
            break_cleanup_start,
            continue_cleanup_start,
        ),
        Statement::LetElse {
            binding,
            value_type,
            value,
            enum_name,
            enum_args,
            variant,
            else_body,
        } => emit_let_else(
            out,
            binding,
            value_type,
            value,
            enum_name,
            enum_args,
            variant,
            else_body,
            indent,
            deferred,
            function_return_type,
            active_arrays,
            break_deferred_start,
            continue_deferred_start,
            break_cleanup_start,
            continue_cleanup_start,
        ),
        Statement::IfLet {
            binding,
            value_type,
            value,
            enum_name,
            enum_args,
            variant,
            body,
            else_body,
        } => emit_if_let(
            out,
            binding.as_deref(),
            value_type.as_ref(),
            value,
            enum_name,
            enum_args,
            variant,
            body,
            else_body.as_deref(),
            indent,
            deferred,
            function_return_type,
            active_arrays,
            break_deferred_start,
            continue_deferred_start,
            break_cleanup_start,
            continue_cleanup_start,
        ),
        Statement::If {
            condition,
            body,
            else_body,
        } => emit_if_statement(
            out,
            condition,
            body,
            else_body,
            indent,
            deferred,
            function_return_type,
            active_arrays,
            break_deferred_start,
            continue_deferred_start,
            break_cleanup_start,
            continue_cleanup_start,
        ),
        Statement::QuestionLet {
            carrier,
            name,
            value_type,
            result_type,
            return_type,
            result_expr,
        } => emit_question_let(
            out,
            *carrier,
            name,
            value_type,
            result_type,
            return_type,
            result_expr,
            indent,
            deferred,
            active_arrays,
        ),
        Statement::QuestionReturn {
            carrier,
            ok_type,
            result_type,
            return_type,
            result_expr,
        } => emit_question_return(
            out,
            *carrier,
            ok_type,
            result_type,
            return_type,
            result_expr,
            indent,
            deferred,
            active_arrays,
        ),
        Statement::Assign { name, value } => emit_assign(out, name, value, indent, active_arrays),
        Statement::AssignField {
            base,
            field,
            value_type,
            value,
        } => emit_assign_field(out, base, field, value_type, value, indent),
        Statement::Println(arg) => {
            write_indent(out, indent);
            out.push_str("puts(");
            emit_string_data_expr(out, arg);
            out.push_str(");\n");
        }
        Statement::Print(arg) => {
            write_indent(out, indent);
            out.push_str("fputs(");
            emit_string_data_expr(out, arg);
            out.push_str(", stdout);\n");
        }
        Statement::Eprintln(arg) => {
            write_indent(out, indent);
            out.push_str("fputs(");
            emit_string_data_expr(out, arg);
            out.push_str(", stderr);\n");
            write_indent(out, indent);
            out.push_str("fputc('\\n', stderr);\n");
        }
        Statement::Eprint(arg) => {
            write_indent(out, indent);
            out.push_str("fputs(");
            emit_string_data_expr(out, arg);
            out.push_str(", stderr);\n");
        }
        Statement::Panic(message) => {
            emit_deferred(out, indent, deferred);
            emit_array_releases(out, indent, active_arrays);
            write_indent(out, indent);
            out.push_str("nomo_panic(");
            emit_string_data_expr(out, message);
            out.push_str(");\n");
        }
        Statement::Return(Some(value)) => emit_return_value(
            out,
            value,
            indent,
            deferred,
            function_return_type,
            active_arrays,
        ),
        Statement::Return(None) => {
            emit_deferred(out, indent, deferred);
            emit_array_releases(out, indent, active_arrays);
            write_indent(out, indent);
            out.push_str("return;\n");
        }
        Statement::Expr(value) => {
            write_indent(out, indent);
            emit_expr(out, value);
            out.push_str(";\n");
        }
        Statement::Match {
            value,
            enum_name,
            enum_args,
            arms,
        } => emit_match_statement(
            out,
            value,
            enum_name,
            enum_args,
            arms,
            indent,
            deferred,
            function_return_type,
            active_arrays,
            break_deferred_start,
            continue_deferred_start,
            break_cleanup_start,
            continue_cleanup_start,
        ),
        Statement::Loop { kind, body } => emit_loop(
            out,
            kind,
            body,
            indent,
            deferred,
            function_return_type,
            active_arrays,
            break_deferred_start,
            continue_deferred_start,
            break_cleanup_start,
            continue_cleanup_start,
        ),
        Statement::Break => {
            emit_deferred(out, indent, &deferred[break_deferred_start..]);
            emit_array_releases(out, indent, &active_arrays[break_cleanup_start..]);
            write_indent(out, indent);
            out.push_str("break;\n");
        }
        Statement::Continue => {
            emit_deferred(out, indent, &deferred[continue_deferred_start..]);
            emit_array_releases(out, indent, &active_arrays[continue_cleanup_start..]);
            write_indent(out, indent);
            out.push_str("continue;\n");
        }
        Statement::Defer { .. } => {
            // Deferred calls are collected by emit_body and emitted at exit points.
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn emit_match_statement(
    out: &mut String,
    value: &ValueExpr,
    enum_name: &str,
    enum_args: &[ValueType],
    arms: &[MatchStatementArm],
    indent: usize,
    deferred: &[DeferredCall],
    function_return_type: &ValueType,
    active_arrays: &[LocalArray],
    break_deferred_start: usize,
    continue_deferred_start: usize,
    break_cleanup_start: usize,
    continue_cleanup_start: usize,
) {
    for (index, arm) in arms.iter().enumerate() {
        write_indent(out, indent);
        if index == 0 {
            out.push_str("if (");
        } else {
            out.push_str("else if (");
        }
        emit_expr(out, value);
        out.push_str(".tag == ");
        out.push_str(&c_enum_variant_ident(enum_name, enum_args, &arm.variant));
        out.push_str(") {\n");
        emit_block(
            out,
            &arm.body,
            indent + 1,
            deferred,
            function_return_type,
            active_arrays,
            break_deferred_start,
            continue_deferred_start,
            break_cleanup_start,
            continue_cleanup_start,
        );
        write_indent(out, indent);
        out.push_str("}\n");
    }
}

#[allow(clippy::too_many_arguments)]
fn emit_if_statement(
    out: &mut String,
    condition: &ValueExpr,
    body: &[Statement],
    else_body: &[Statement],
    indent: usize,
    deferred: &[DeferredCall],
    function_return_type: &ValueType,
    active_arrays: &[LocalArray],
    break_deferred_start: usize,
    continue_deferred_start: usize,
    break_cleanup_start: usize,
    continue_cleanup_start: usize,
) {
    write_indent(out, indent);
    out.push_str("if (");
    emit_expr(out, condition);
    out.push_str(") {\n");
    emit_block(
        out,
        body,
        indent + 1,
        deferred,
        function_return_type,
        active_arrays,
        break_deferred_start,
        continue_deferred_start,
        break_cleanup_start,
        continue_cleanup_start,
    );
    write_indent(out, indent);
    out.push_str("} else {\n");
    emit_block(
        out,
        else_body,
        indent + 1,
        deferred,
        function_return_type,
        active_arrays,
        break_deferred_start,
        continue_deferred_start,
        break_cleanup_start,
        continue_cleanup_start,
    );
    write_indent(out, indent);
    out.push_str("}\n");
}

#[allow(clippy::too_many_arguments)]
fn emit_if_let(
    out: &mut String,
    binding: Option<&str>,
    value_type: Option<&ValueType>,
    value: &ValueExpr,
    enum_name: &str,
    enum_args: &[ValueType],
    variant: &str,
    body: &[Statement],
    else_body: Option<&[Statement]>,
    indent: usize,
    deferred: &[DeferredCall],
    function_return_type: &ValueType,
    active_arrays: &[LocalArray],
    break_deferred_start: usize,
    continue_deferred_start: usize,
    break_cleanup_start: usize,
    continue_cleanup_start: usize,
) {
    let temp = format!(
        "nomo__if_let_{}",
        c_enum_variant_ident(enum_name, enum_args, variant)
    );
    write_indent(out, indent);
    out.push_str("{\n");
    write_indent(out, indent + 1);
    out.push_str(&c_enum_ident(enum_name, enum_args));
    out.push(' ');
    out.push_str(&temp);
    out.push_str(" = ");
    emit_expr(out, value);
    out.push_str(";\n");
    write_indent(out, indent + 1);
    out.push_str("if (");
    out.push_str(&temp);
    out.push_str(".tag == ");
    out.push_str(&c_enum_variant_ident(enum_name, enum_args, variant));
    out.push_str(") {\n");
    if let (Some(binding), Some(value_type)) = (binding, value_type) {
        write_indent(out, indent + 2);
        out.push_str(&c_type(value_type));
        out.push(' ');
        out.push_str(&c_var_ident(binding));
        out.push_str(" = ");
        out.push_str(&temp);
        out.push_str(".payload.");
        out.push_str(&c_payload_ident(variant));
        out.push_str(";\n");
        emit_array_retain_binding(out, binding, value_type, indent + 2);
    }
    emit_enum_temp_release_if_owned(out, &temp, enum_name, enum_args, value, indent + 2);
    let mut then_active_arrays = active_arrays.to_vec();
    if let (Some(binding), Some(value_type)) = (binding, value_type) {
        if let Some(local) = local_array(binding, value_type) {
            then_active_arrays.push(local);
        }
    }
    emit_block(
        out,
        body,
        indent + 2,
        deferred,
        function_return_type,
        &then_active_arrays,
        break_deferred_start,
        continue_deferred_start,
        break_cleanup_start,
        continue_cleanup_start,
    );
    if let (Some(binding), Some(value_type)) = (binding, value_type) {
        emit_value_release_binding(out, binding, value_type, indent + 2);
    }
    write_indent(out, indent + 1);
    out.push_str("}");
    if let Some(else_body) = else_body {
        out.push_str(" else {\n");
        emit_enum_temp_release_if_owned(out, &temp, enum_name, enum_args, value, indent + 2);
        emit_block(
            out,
            else_body,
            indent + 2,
            deferred,
            function_return_type,
            active_arrays,
            break_deferred_start,
            continue_deferred_start,
            break_cleanup_start,
            continue_cleanup_start,
        );
        write_indent(out, indent + 1);
        out.push('}');
    } else {
        out.push_str(" else {\n");
        emit_enum_temp_release_if_owned(out, &temp, enum_name, enum_args, value, indent + 2);
        write_indent(out, indent + 1);
        out.push('}');
    }
    out.push('\n');
    write_indent(out, indent);
    out.push_str("}\n");
}

fn emit_loop(
    out: &mut String,
    kind: &LoopKind,
    body: &[Statement],
    indent: usize,
    deferred: &[DeferredCall],
    function_return_type: &ValueType,
    active_arrays: &[LocalArray],
    _break_deferred_start: usize,
    _continue_deferred_start: usize,
    _break_cleanup_start: usize,
    _continue_cleanup_start: usize,
) {
    match kind {
        LoopKind::Infinite => {
            write_indent(out, indent);
            out.push_str("for (;;) {\n");
            emit_block(
                out,
                body,
                indent + 1,
                deferred,
                function_return_type,
                active_arrays,
                deferred.len(),
                deferred.len(),
                active_arrays.len(),
                active_arrays.len(),
            );
            write_indent(out, indent);
            out.push_str("}\n");
        }
        LoopKind::While(condition) => {
            write_indent(out, indent);
            out.push_str("while (");
            emit_expr(out, condition);
            out.push_str(") {\n");
            emit_block(
                out,
                body,
                indent + 1,
                deferred,
                function_return_type,
                active_arrays,
                deferred.len(),
                deferred.len(),
                active_arrays.len(),
                active_arrays.len(),
            );
            write_indent(out, indent);
            out.push_str("}\n");
        }
        LoopKind::Iterate {
            binding,
            element_type,
            iterable,
        } => {
            let array_type = ValueType::Array(Box::new(element_type.clone()));
            let owned_iterable = !expr_may_share_array_storage(iterable);
            write_indent(out, indent);
            out.push_str("{\n");
            write_indent(out, indent + 1);
            out.push_str(&c_type(&array_type));
            out.push_str(" nomo__seq = ");
            emit_expr(out, iterable);
            out.push_str(";\n");
            write_indent(out, indent + 1);
            out.push_str("for (uint64_t nomo_i = 0; nomo_i < nomo__seq.len; nomo_i++) {\n");
            write_indent(out, indent + 2);
            out.push_str(&c_type(element_type));
            out.push(' ');
            out.push_str(&c_var_ident(binding));
            out.push_str(" = nomo__seq.data[nomo_i];\n");
            emit_array_retain_binding(out, binding, element_type, indent + 2);
            let mut body_active_arrays = active_arrays.to_vec();
            if owned_iterable {
                if let Some(local) = local_c_value("nomo__seq", &array_type) {
                    body_active_arrays.push(local);
                }
            }
            let loop_binding_cleanup_start = body_active_arrays.len();
            if let Some(local) = local_array(binding, element_type) {
                body_active_arrays.push(local);
            }
            emit_block(
                out,
                body,
                indent + 2,
                deferred,
                function_return_type,
                &body_active_arrays,
                deferred.len(),
                deferred.len(),
                loop_binding_cleanup_start,
                loop_binding_cleanup_start,
            );
            emit_value_release_binding(out, binding, element_type, indent + 2);
            write_indent(out, indent + 1);
            out.push_str("}\n");
            if owned_iterable {
                emit_value_release_in_place(out, &array_type, "nomo__seq", indent + 1);
            }
            write_indent(out, indent);
            out.push_str("}\n");
        }
    }
}

fn emit_block(
    out: &mut String,
    body: &[Statement],
    indent: usize,
    deferred: &[DeferredCall],
    function_return_type: &ValueType,
    active_arrays: &[LocalArray],
    break_deferred_start: usize,
    continue_deferred_start: usize,
    break_cleanup_start: usize,
    continue_cleanup_start: usize,
) {
    let inherited_len = active_arrays.len();
    let mut scope_arrays = active_arrays.to_vec();
    let mut block_deferred: Vec<DeferredCall> = Vec::new();
    let mut last_statement_exits = false;
    for statement in body {
        if let Statement::Defer { call } = statement {
            block_deferred.push(call.clone());
            last_statement_exits = false;
            continue;
        }
        let mut active_deferred = deferred.to_vec();
        active_deferred.extend(block_deferred.iter().cloned());
        emit_stmt(
            out,
            statement,
            indent,
            &active_deferred,
            function_return_type,
            &scope_arrays,
            break_deferred_start,
            continue_deferred_start,
            break_cleanup_start,
            continue_cleanup_start,
        );
        if let Some(local) = local_array_from_statement(statement) {
            scope_arrays.push(local);
        }
        last_statement_exits = statement_exits_block(statement);
        if last_statement_exits {
            break;
        }
    }
    if !last_statement_exits {
        emit_deferred(out, indent, &block_deferred);
        if scope_arrays.len() > inherited_len {
            emit_array_releases(out, indent, &scope_arrays[inherited_len..]);
        }
    }
}

fn emit_return_value(
    out: &mut String,
    value: &ValueExpr,
    indent: usize,
    deferred: &[DeferredCall],
    return_type: &ValueType,
    active_arrays: &[LocalArray],
) {
    if deferred.is_empty() {
        if !active_arrays.is_empty() {
            write_indent(out, indent);
            out.push_str("{\n");
            write_indent(out, indent + 1);
            out.push_str(&c_type(return_type));
            out.push_str(" nomo__return = ");
            emit_expr(out, value);
            out.push_str(";\n");
            emit_array_retain_return_if_needed(out, value, return_type, indent + 1);
            emit_array_releases(out, indent + 1, active_arrays);
            write_indent(out, indent + 1);
            out.push_str("return nomo__return;\n");
            write_indent(out, indent);
            out.push_str("}\n");
            return;
        }
        write_indent(out, indent);
        out.push_str("return ");
        emit_expr(out, value);
        out.push_str(";\n");
        return;
    }

    write_indent(out, indent);
    out.push_str("{\n");
    write_indent(out, indent + 1);
    out.push_str(&c_type(return_type));
    out.push_str(" nomo__return = ");
    emit_expr(out, value);
    out.push_str(";\n");
    emit_array_retain_return_if_needed(out, value, return_type, indent + 1);
    emit_deferred(out, indent + 1, deferred);
    emit_array_releases(out, indent + 1, active_arrays);
    write_indent(out, indent + 1);
    out.push_str("return nomo__return;\n");
    write_indent(out, indent);
    out.push_str("}\n");
}

fn emit_let(
    out: &mut String,
    name: &str,
    value_type: &ValueType,
    initializer: &ValueExpr,
    indent: usize,
) {
    write_indent(out, indent);
    out.push_str(&c_type(value_type));
    out.push(' ');
    out.push_str(&c_var_ident(name));
    out.push_str(" = ");
    emit_expr(out, initializer);
    out.push_str(";\n");
    emit_array_retain_after_binding(out, name, value_type, initializer, indent);
}

#[allow(clippy::too_many_arguments)]
fn emit_let_if(
    out: &mut String,
    name: &str,
    value_type: &ValueType,
    condition: &ValueExpr,
    body: &[Statement],
    else_body: &[Statement],
    indent: usize,
    deferred: &[DeferredCall],
    function_return_type: &ValueType,
    active_arrays: &[LocalArray],
    break_deferred_start: usize,
    continue_deferred_start: usize,
    break_cleanup_start: usize,
    continue_cleanup_start: usize,
) {
    write_indent(out, indent);
    out.push_str(&c_type(value_type));
    out.push(' ');
    out.push_str(&c_var_ident(name));
    out.push_str(";\n");
    emit_if_statement(
        out,
        condition,
        body,
        else_body,
        indent,
        deferred,
        function_return_type,
        active_arrays,
        break_deferred_start,
        continue_deferred_start,
        break_cleanup_start,
        continue_cleanup_start,
    );
}

#[allow(clippy::too_many_arguments)]
fn emit_let_match(
    out: &mut String,
    name: &str,
    value_type: &ValueType,
    value: &ValueExpr,
    enum_name: &str,
    enum_args: &[ValueType],
    arms: &[MatchStatementArm],
    indent: usize,
    deferred: &[DeferredCall],
    function_return_type: &ValueType,
    active_arrays: &[LocalArray],
    break_deferred_start: usize,
    continue_deferred_start: usize,
    break_cleanup_start: usize,
    continue_cleanup_start: usize,
) {
    write_indent(out, indent);
    out.push_str(&c_type(value_type));
    out.push(' ');
    out.push_str(&c_var_ident(name));
    out.push_str(";\n");
    emit_match_statement(
        out,
        value,
        enum_name,
        enum_args,
        arms,
        indent,
        deferred,
        function_return_type,
        active_arrays,
        break_deferred_start,
        continue_deferred_start,
        break_cleanup_start,
        continue_cleanup_start,
    );
}

fn emit_assign(
    out: &mut String,
    name: &str,
    value: &ValueExpr,
    indent: usize,
    active_arrays: &[LocalArray],
) {
    let Some(value_type) = active_array_type(active_arrays, name) else {
        write_indent(out, indent);
        out.push_str(&c_var_ident(name));
        out.push_str(" = ");
        emit_expr(out, value);
        out.push_str(";\n");
        return;
    };
    if is_array_mutating_assignment(value) {
        write_indent(out, indent);
        out.push_str(&c_var_ident(name));
        out.push_str(" = ");
        emit_expr(out, value);
        out.push_str(";\n");
        return;
    }

    let temp = format!("nomo__assign_{}", c_var_ident(name));
    write_indent(out, indent);
    out.push_str("{\n");
    write_indent(out, indent + 1);
    out.push_str(&c_type(value_type));
    out.push(' ');
    out.push_str(&temp);
    out.push_str(" = ");
    emit_expr(out, value);
    out.push_str(";\n");
    emit_value_retain_value_if_needed(out, &temp, value_type, value, indent + 1);
    emit_value_release_binding(out, name, value_type, indent + 1);
    write_indent(out, indent + 1);
    out.push_str(&c_var_ident(name));
    out.push_str(" = ");
    out.push_str(&temp);
    out.push_str(";\n");
    write_indent(out, indent);
    out.push_str("}\n");
}

fn emit_assign_field(
    out: &mut String,
    base: &str,
    field: &str,
    value_type: &ValueType,
    value: &ValueExpr,
    indent: usize,
) {
    let field_access = format!("{}.{}", c_var_ident(base), c_member_ident(field));
    if !value_type_needs_release(value_type) {
        write_indent(out, indent);
        out.push_str(&field_access);
        out.push_str(" = ");
        emit_expr(out, value);
        out.push_str(";\n");
        return;
    }

    let temp = format!(
        "nomo__assign_{}_{}",
        c_var_ident(base),
        c_member_ident(field)
    );
    write_indent(out, indent);
    out.push_str("{\n");
    write_indent(out, indent + 1);
    out.push_str(&c_type(value_type));
    out.push(' ');
    out.push_str(&temp);
    out.push_str(" = ");
    emit_expr(out, value);
    out.push_str(";\n");
    emit_value_retain_value_if_needed(out, &temp, value_type, value, indent + 1);
    emit_value_release_in_place(out, value_type, &field_access, indent + 1);
    write_indent(out, indent + 1);
    out.push_str(&field_access);
    out.push_str(" = ");
    out.push_str(&temp);
    out.push_str(";\n");
    write_indent(out, indent);
    out.push_str("}\n");
}

fn is_array_mutating_assignment(value: &ValueExpr) -> bool {
    matches!(
        value,
        ValueExpr::ArrayPush { .. }
            | ValueExpr::ArraySet { .. }
            | ValueExpr::ArrayInsert { .. }
            | ValueExpr::ArrayClear { .. }
    )
}

#[allow(clippy::too_many_arguments)]
fn emit_let_else(
    out: &mut String,
    binding: &str,
    value_type: &ValueType,
    value: &ValueExpr,
    enum_name: &str,
    enum_args: &[ValueType],
    variant: &str,
    else_body: &[Statement],
    indent: usize,
    deferred: &[DeferredCall],
    function_return_type: &ValueType,
    active_arrays: &[LocalArray],
    break_deferred_start: usize,
    continue_deferred_start: usize,
    break_cleanup_start: usize,
    continue_cleanup_start: usize,
) {
    let temp = format!("nomo__let_else_{}", c_var_ident(binding));
    write_indent(out, indent);
    out.push_str(&c_enum_ident(enum_name, enum_args));
    out.push(' ');
    out.push_str(&temp);
    out.push_str(" = ");
    emit_expr(out, value);
    out.push_str(";\n");
    write_indent(out, indent);
    out.push_str("if (");
    out.push_str(&temp);
    out.push_str(".tag != ");
    out.push_str(&c_enum_variant_ident(enum_name, enum_args, variant));
    out.push_str(") {\n");
    emit_enum_temp_release_if_owned(out, &temp, enum_name, enum_args, value, indent + 1);
    emit_block(
        out,
        else_body,
        indent + 1,
        deferred,
        function_return_type,
        active_arrays,
        break_deferred_start,
        continue_deferred_start,
        break_cleanup_start,
        continue_cleanup_start,
    );
    write_indent(out, indent);
    out.push_str("}\n");
    write_indent(out, indent);
    out.push_str(&c_type(value_type));
    out.push(' ');
    out.push_str(&c_var_ident(binding));
    out.push_str(" = ");
    out.push_str(&temp);
    out.push_str(".payload.");
    out.push_str(&c_payload_ident(variant));
    out.push_str(";\n");
    emit_array_retain_binding(out, binding, value_type, indent);
    emit_enum_temp_release_if_owned(out, &temp, enum_name, enum_args, value, indent);
}

fn emit_enum_temp_release_if_owned(
    out: &mut String,
    temp: &str,
    enum_name: &str,
    enum_args: &[ValueType],
    value: &ValueExpr,
    indent: usize,
) {
    let enum_type = ValueType::Enum(enum_name.to_string(), enum_args.to_vec());
    if expr_may_share_array_storage(value) || !value_type_needs_release(&enum_type) {
        return;
    }
    emit_value_release_in_place(out, &enum_type, temp, indent);
}

fn emit_array_retain_after_binding(
    out: &mut String,
    name: &str,
    value_type: &ValueType,
    initializer: &ValueExpr,
    indent: usize,
) {
    if !value_type_needs_release(value_type) || !expr_may_share_array_storage(initializer) {
        return;
    }
    emit_value_retain_in_place(out, value_type, &c_var_ident(name), indent);
}

fn emit_array_retain_binding(out: &mut String, name: &str, value_type: &ValueType, indent: usize) {
    if !value_type_needs_release(value_type) {
        return;
    }
    emit_value_retain_in_place(out, value_type, &c_var_ident(name), indent);
}

fn emit_value_retain_value_if_needed(
    out: &mut String,
    c_value: &str,
    value_type: &ValueType,
    initializer: &ValueExpr,
    indent: usize,
) {
    if !expr_may_share_array_storage(initializer) || !value_type_needs_release(value_type) {
        return;
    }
    emit_value_retain_in_place(out, value_type, c_value, indent);
}

fn local_array_from_statement(statement: &Statement) -> Option<LocalArray> {
    match statement {
        Statement::Let {
            name, value_type, ..
        }
        | Statement::LetIf {
            name, value_type, ..
        }
        | Statement::LetMatch {
            name, value_type, ..
        }
        | Statement::QuestionLet {
            name, value_type, ..
        } => local_array(name, value_type),
        Statement::LetElse {
            binding,
            value_type,
            ..
        } => local_array(binding, value_type),
        _ => None,
    }
}

fn array_params(function: &Function) -> Vec<LocalArray> {
    function
        .params
        .iter()
        .filter(|param| !param.mutable)
        .filter_map(|param| local_array(&param.name, &param.value_type))
        .collect()
}

fn local_array(name: &str, value_type: &ValueType) -> Option<LocalArray> {
    if value_type_needs_release(value_type) {
        Some(LocalArray {
            name: name.to_string(),
            value_type: value_type.clone(),
            c_value: None,
        })
    } else {
        None
    }
}

fn local_c_value(c_value: &str, value_type: &ValueType) -> Option<LocalArray> {
    if value_type_needs_release(value_type) {
        Some(LocalArray {
            name: c_value.to_string(),
            value_type: value_type.clone(),
            c_value: Some(c_value.to_string()),
        })
    } else {
        None
    }
}

fn emit_array_releases(out: &mut String, indent: usize, active_arrays: &[LocalArray]) {
    for local in active_arrays.iter().rev() {
        if let Some(c_value) = &local.c_value {
            emit_value_release_in_place(out, &local.value_type, c_value, indent);
        } else {
            emit_value_release_binding(out, &local.name, &local.value_type, indent);
        }
    }
}

fn value_type_needs_release(value_type: &ValueType) -> bool {
    match value_type {
        ValueType::String => true,
        ValueType::Array(element_type) => is_supported_array_element(element_type),
        ValueType::Struct(_, _) | ValueType::Enum(_, _) => true,
        _ => false,
    }
}

fn emit_value_release_binding(out: &mut String, name: &str, value_type: &ValueType, indent: usize) {
    emit_value_release_in_place(out, value_type, &c_var_ident(name), indent);
}

fn emit_value_release_in_place(
    out: &mut String,
    value_type: &ValueType,
    c_value: &str,
    indent: usize,
) {
    match value_type {
        ValueType::Array(element_type) if is_supported_array_element(element_type) => {
            write_indent(out, indent);
            emit_array_release_expr(out, element_type, c_value);
            out.push_str(";\n");
        }
        ValueType::String => {
            write_indent(out, indent);
            out.push_str("nomo_string_release(");
            out.push_str(c_value);
            out.push_str(");\n");
        }
        ValueType::Struct(_, _) | ValueType::Enum(_, _) => {
            write_indent(out, indent);
            out.push_str(&c_release_ident(value_type));
            out.push('(');
            out.push_str(c_value);
            out.push_str(");\n");
        }
        _ => {}
    }
}

fn emit_value_retain_in_place(
    out: &mut String,
    value_type: &ValueType,
    c_value: &str,
    indent: usize,
) {
    match value_type {
        ValueType::Array(element_type) if is_supported_array_element(element_type) => {
            write_indent(out, indent);
            out.push_str(c_value);
            out.push_str(" = ");
            out.push_str(&c_array_ident(element_type));
            out.push_str("_retain(");
            out.push_str(c_value);
            out.push_str(");\n");
        }
        ValueType::String => {
            write_indent(out, indent);
            out.push_str(c_value);
            out.push_str(" = nomo_string_retain(");
            out.push_str(c_value);
            out.push_str(");\n");
        }
        ValueType::Struct(_, _) | ValueType::Enum(_, _) => {
            write_indent(out, indent);
            out.push_str(c_value);
            out.push_str(" = ");
            out.push_str(&c_retain_ident(value_type));
            out.push('(');
            out.push_str(c_value);
            out.push_str(");\n");
        }
        _ => {}
    }
}

fn emit_array_release_expr(out: &mut String, element_type: &ValueType, c_value: &str) {
    out.push_str(&c_array_ident(element_type));
    out.push_str("_release(");
    out.push_str(c_value);
    out.push(')');
}

fn active_array_type<'a>(active_arrays: &'a [LocalArray], name: &str) -> Option<&'a ValueType> {
    active_arrays
        .iter()
        .find(|local| local.name == name)
        .map(|local| &local.value_type)
}

fn emit_array_retain_return_if_needed(
    out: &mut String,
    value: &ValueExpr,
    return_type: &ValueType,
    indent: usize,
) {
    if !value_type_needs_release(return_type) || !expr_may_share_array_storage(value) {
        return;
    }
    emit_value_retain_in_place(out, return_type, "nomo__return", indent);
}

fn expr_may_share_array_storage(value: &ValueExpr) -> bool {
    match value {
        ValueExpr::Variable(_)
        | ValueExpr::FieldAccess { .. }
        | ValueExpr::EnumPayload { .. }
        | ValueExpr::EnumPayloadFieldAccess { .. } => true,
        ValueExpr::Cast { expr, .. }
        | ValueExpr::Unary { expr, .. }
        | ValueExpr::StringLen { value: expr }
        | ValueExpr::FsReadToString { path: expr }
        | ValueExpr::FsReadBytes { path: expr }
        | ValueExpr::FsExists { path: expr }
        | ValueExpr::FsMetadata { path: expr }
        | ValueExpr::FsCreateDir { path: expr }
        | ValueExpr::FsRemoveDir { path: expr }
        | ValueExpr::FsReadDir { path: expr }
        | ValueExpr::FsOpen { path: expr }
        | ValueExpr::FileClose { file: expr }
        | ValueExpr::FileReadToString { file: expr }
        | ValueExpr::TcpListenerAccept { listener: expr }
        | ValueExpr::TcpListenerClose { listener: expr }
        | ValueExpr::TcpStreamClose { stream: expr }
        | ValueExpr::TcpStreamReadToString { stream: expr }
        | ValueExpr::UdpSocketClose { socket: expr }
        | ValueExpr::EnvGet { name: expr }
        | ValueExpr::TimeDurationMillis { millis: expr }
        | ValueExpr::TimeDurationSeconds { seconds: expr }
        | ValueExpr::TimeDurationAsMillis { duration: expr }
        | ValueExpr::TimeFormatDuration { duration: expr }
        | ValueExpr::TimeSleep { duration: expr }
        | ValueExpr::TimeSleepMillis { duration: expr }
        | ValueExpr::LogEnabled { level: expr }
        | ValueExpr::HashString { value: expr }
        | ValueExpr::HashBytes { value: expr }
        | ValueExpr::HashFinish { state: expr }
        | ValueExpr::CryptoSha256 { value: expr }
        | ValueExpr::CryptoSha512 { value: expr }
        | ValueExpr::CryptoRandomBytes { count: expr }
        | ValueExpr::JsonParse { value: expr }
        | ValueExpr::JsonStringify { value: expr }
        | ValueExpr::ProcessExit { code: expr }
        | ValueExpr::ProcessSpawn { command: expr }
        | ValueExpr::ProcessStatus { command: expr }
        | ValueExpr::ProcessExec { command: expr }
        | ValueExpr::ProcessOutput { command: expr }
        | ValueExpr::NumParseI64 { value: expr }
        | ValueExpr::NumParseU64 { value: expr }
        | ValueExpr::NumParseF64 { value: expr }
        | ValueExpr::NumToString { value: expr, .. }
        | ValueExpr::ArrayLen { array: expr }
        | ValueExpr::EnumVariant {
            payload: Some(expr),
            ..
        } => expr_may_share_array_storage(expr),
        ValueExpr::Binary { left, right, .. }
        | ValueExpr::StringCompare { left, right, .. }
        | ValueExpr::NumBinary { left, right, .. }
        | ValueExpr::FsWriteString {
            path: left,
            content: right,
        }
        | ValueExpr::FsWriteBytes {
            path: left,
            bytes: right,
        }
        | ValueExpr::HashWriteString {
            state: left,
            value: right,
        }
        | ValueExpr::HashWriteBytes {
            state: left,
            value: right,
        }
        | ValueExpr::FileWriteString {
            file: left,
            content: right,
        }
        | ValueExpr::NetConnect {
            host: left,
            port: right,
        }
        | ValueExpr::NetListen {
            host: left,
            port: right,
        }
        | ValueExpr::NetUdpBind {
            host: left,
            port: right,
        }
        | ValueExpr::UdpSocketRecvFromString {
            socket: left,
            max_bytes: right,
        }
        | ValueExpr::TcpStreamWriteString {
            stream: left,
            content: right,
        } => expr_may_share_array_storage(left) || expr_may_share_array_storage(right),
        ValueExpr::UdpSocketSendToString {
            socket,
            content,
            host,
            port,
        } => {
            expr_may_share_array_storage(socket)
                || expr_may_share_array_storage(content)
                || expr_may_share_array_storage(host)
                || expr_may_share_array_storage(port)
        }
        ValueExpr::StringConcat { .. }
        | ValueExpr::StringIsEmpty { .. }
        | ValueExpr::StringContains { .. }
        | ValueExpr::StringStartsWith { .. }
        | ValueExpr::StringEndsWith { .. }
        | ValueExpr::StringSplit { .. }
        | ValueExpr::StringTrim { .. }
        | ValueExpr::StringToLower { .. }
        | ValueExpr::StringToUpper { .. }
        | ValueExpr::RegexCompile { .. }
        | ValueExpr::RegexIsMatch { .. }
        | ValueExpr::RegexCaptures { .. }
        | ValueExpr::CharIsDigit { .. }
        | ValueExpr::CharIsAlpha { .. }
        | ValueExpr::CharIsWhitespace { .. }
        | ValueExpr::CharToString { .. }
        | ValueExpr::PathJoin { .. }
        | ValueExpr::PathBasename { .. }
        | ValueExpr::PathDirname { .. }
        | ValueExpr::PathExtension { .. }
        | ValueExpr::PathNormalize { .. }
        | ValueExpr::PathIsAbsolute { .. }
        | ValueExpr::MathUnary { .. }
        | ValueExpr::MathBinary { .. } => false,
        ValueExpr::ArrayPush { value, .. }
        | ValueExpr::ArraySet { value, .. }
        | ValueExpr::ArrayInsert { value, .. } => expr_may_share_array_storage(value),
        ValueExpr::ArrayPop { .. }
        | ValueExpr::ArrayRemove { .. }
        | ValueExpr::ArrayClear { .. } => false,
        ValueExpr::EnvSet { .. } => false,
        ValueExpr::ResultMapErr { result, .. }
        | ValueExpr::ResultMap { result, .. }
        | ValueExpr::ResultAndThen { result, .. }
        | ValueExpr::OptionMap { option: result, .. }
        | ValueExpr::OptionAndThen { option: result, .. }
        | ValueExpr::ResultIsOk { result, .. }
        | ValueExpr::ResultIsErr { result, .. }
        | ValueExpr::OptionIsSome { option: result, .. }
        | ValueExpr::OptionIsNone { option: result, .. } => expr_may_share_array_storage(result),
        ValueExpr::ResultUnwrapOr {
            result, default, ..
        } => expr_may_share_array_storage(result) || expr_may_share_array_storage(default),
        ValueExpr::OptionUnwrapOr {
            option, default, ..
        } => expr_may_share_array_storage(option) || expr_may_share_array_storage(default),
        ValueExpr::StructLiteral { fields, .. } => fields
            .iter()
            .any(|(_, field)| expr_may_share_array_storage(field)),
        ValueExpr::Match { value, arms } => {
            expr_may_share_array_storage(value)
                || arms
                    .iter()
                    .any(|arm| expr_may_share_array_storage(&arm.value))
        }
        ValueExpr::If {
            condition,
            then_branch,
            else_branch,
        } => {
            expr_may_share_array_storage(condition)
                || expr_may_share_array_storage(then_branch)
                || expr_may_share_array_storage(else_branch)
        }
        ValueExpr::StringLiteral(_)
        | ValueExpr::IntLiteral(_)
        | ValueExpr::FloatLiteral(_)
        | ValueExpr::CharLiteral(_)
        | ValueExpr::BoolLiteral(_)
        | ValueExpr::VoidLiteral
        | ValueExpr::Panic { .. }
        | ValueExpr::HashNew
        | ValueExpr::CollectionsStringMapNew
        | ValueExpr::CollectionsStringMapLen { .. }
        | ValueExpr::CollectionsStringMapGet { .. }
        | ValueExpr::CollectionsStringMapContains { .. }
        | ValueExpr::CollectionsStringMapSet { .. }
        | ValueExpr::CollectionsStringMapRemove { .. }
        | ValueExpr::CollectionsStringSetNew
        | ValueExpr::CollectionsStringSetLen { .. }
        | ValueExpr::CollectionsStringSetContains { .. }
        | ValueExpr::CollectionsStringSetInsert { .. }
        | ValueExpr::CollectionsStringSetRemove { .. }
        | ValueExpr::MutBorrow(_)
        | ValueExpr::Call { .. }
        | ValueExpr::EnvCwd
        | ValueExpr::EnvHomeDir
        | ValueExpr::EnvTempDir
        | ValueExpr::OsPlatform
        | ValueExpr::OsArch
        | ValueExpr::OsPathSeparator
        | ValueExpr::OsLineEnding
        | ValueExpr::TimeNowMillis
        | ValueExpr::TimeMonotonicMillis
        | ValueExpr::EnvArgs
        | ValueExpr::IoReadLine
        | ValueExpr::ArrayNew { .. }
        | ValueExpr::ArrayIter { .. }
        | ValueExpr::ArrayGet { .. }
        | ValueExpr::EnumVariant { payload: None, .. } => false,
    }
}

fn emit_question_let(
    out: &mut String,
    carrier: QuestionCarrier,
    name: &str,
    value_type: &ValueType,
    result_type: &ValueType,
    return_type: &ValueType,
    result_expr: &ValueExpr,
    indent: usize,
    deferred: &[DeferredCall],
    active_arrays: &[LocalArray],
) {
    let temp = format!("{}_result", c_var_ident(name));
    write_indent(out, indent);
    out.push_str(&c_type(result_type));
    out.push(' ');
    out.push_str(&temp);
    out.push_str(" = ");
    emit_expr(out, result_expr);
    out.push_str(";\n");
    let ValueType::Enum(result_name, result_args) = result_type else {
        return;
    };
    let (early_variant, payload_variant) = match carrier {
        QuestionCarrier::Result => ("Err", "Ok"),
        QuestionCarrier::Option => ("None", "Some"),
    };
    write_indent(out, indent);
    out.push_str("if (");
    out.push_str(&temp);
    out.push_str(".tag == ");
    out.push_str(&c_enum_variant_ident(
        result_name,
        result_args,
        early_variant,
    ));
    out.push_str(") {\n");
    write_indent(out, indent + 1);
    let (return_name, return_args) = match return_type {
        ValueType::Enum(name, args) => (name, args),
        _ => (result_name, result_args),
    };
    out.push_str(&c_enum_ident(return_name, return_args));
    out.push_str(" nomo__question_return = (");
    out.push_str(&c_enum_ident(return_name, return_args));
    out.push_str("){.tag = ");
    out.push_str(&c_enum_variant_ident(
        return_name,
        return_args,
        early_variant,
    ));
    if carrier == QuestionCarrier::Result {
        out.push_str(", .payload.");
        out.push_str(&c_payload_ident("Err"));
        out.push_str(" = ");
        out.push_str(&temp);
        out.push_str(".payload.");
        out.push_str(&c_payload_ident("Err"));
    }
    out.push_str("};\n");
    if carrier == QuestionCarrier::Result
        && expr_may_share_array_storage(result_expr)
        && value_type_needs_release(return_type)
    {
        emit_value_retain_in_place(out, return_type, "nomo__question_return", indent + 1);
    }
    emit_deferred(out, indent + 1, deferred);
    emit_array_releases(out, indent + 1, active_arrays);
    write_indent(out, indent + 1);
    out.push_str("return nomo__question_return;\n");
    write_indent(out, indent);
    out.push_str("}\n");
    write_indent(out, indent);
    out.push_str(&c_payload_type(value_type));
    out.push(' ');
    out.push_str(&c_var_ident(name));
    out.push_str(" = ");
    out.push_str(&temp);
    out.push_str(".payload.");
    out.push_str(&c_payload_ident(payload_variant));
    out.push_str(";\n");
    if expr_may_share_array_storage(result_expr) && value_type_needs_release(value_type) {
        emit_value_retain_in_place(out, value_type, &c_var_ident(name), indent);
    }
}

fn emit_question_return(
    out: &mut String,
    carrier: QuestionCarrier,
    ok_type: &ValueType,
    result_type: &ValueType,
    return_type: &ValueType,
    result_expr: &ValueExpr,
    indent: usize,
    deferred: &[DeferredCall],
    active_arrays: &[LocalArray],
) {
    write_indent(out, indent);
    out.push_str("{\n");
    write_indent(out, indent + 1);
    out.push_str(&c_type(result_type));
    out.push_str(" nomo__question_result = ");
    emit_expr(out, result_expr);
    out.push_str(";\n");
    let ValueType::Enum(result_name, result_args) = result_type else {
        write_indent(out, indent);
        out.push_str("}\n");
        return;
    };
    let (return_name, return_args) = match return_type {
        ValueType::Enum(name, args) => (name, args),
        _ => (result_name, result_args),
    };
    let (early_variant, payload_variant) = match carrier {
        QuestionCarrier::Result => ("Err", "Ok"),
        QuestionCarrier::Option => ("None", "Some"),
    };
    write_indent(out, indent + 1);
    out.push_str("if (nomo__question_result.tag == ");
    out.push_str(&c_enum_variant_ident(
        result_name,
        result_args,
        early_variant,
    ));
    out.push_str(") {\n");
    write_indent(out, indent + 2);
    out.push_str(&c_enum_ident(return_name, return_args));
    out.push_str(" nomo__question_return = (");
    out.push_str(&c_enum_ident(return_name, return_args));
    out.push_str("){.tag = ");
    out.push_str(&c_enum_variant_ident(
        return_name,
        return_args,
        early_variant,
    ));
    if carrier == QuestionCarrier::Result {
        out.push_str(", .payload.");
        out.push_str(&c_payload_ident("Err"));
        out.push_str(" = nomo__question_result.payload.");
        out.push_str(&c_payload_ident("Err"));
    }
    out.push_str("};\n");
    if carrier == QuestionCarrier::Result
        && expr_may_share_array_storage(result_expr)
        && value_type_needs_release(return_type)
    {
        emit_value_retain_in_place(out, return_type, "nomo__question_return", indent + 2);
    }
    emit_deferred(out, indent + 2, deferred);
    emit_array_releases(out, indent + 2, active_arrays);
    write_indent(out, indent + 2);
    out.push_str("return nomo__question_return;\n");
    write_indent(out, indent + 1);
    out.push_str("}\n");
    write_indent(out, indent + 1);
    out.push_str(&c_payload_type(ok_type));
    out.push_str(" nomo__question_ok = nomo__question_result.payload.");
    out.push_str(&c_payload_ident(payload_variant));
    out.push_str(";\n");
    write_indent(out, indent + 1);
    out.push_str(&c_enum_ident(return_name, return_args));
    out.push_str(" nomo__return = (");
    out.push_str(&c_enum_ident(return_name, return_args));
    out.push_str("){.tag = ");
    out.push_str(&c_enum_variant_ident(
        return_name,
        return_args,
        payload_variant,
    ));
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident(payload_variant));
    out.push_str(" = nomo__question_ok};\n");
    if expr_may_share_array_storage(result_expr) && value_type_needs_release(return_type) {
        emit_value_retain_in_place(out, return_type, "nomo__return", indent + 1);
    }
    emit_deferred(out, indent + 1, deferred);
    emit_array_releases(out, indent + 1, active_arrays);
    write_indent(out, indent + 1);
    out.push_str("return nomo__return;\n");
    write_indent(out, indent);
    out.push_str("}\n");
}

fn emit_result_map_err_helper(out: &mut String, instance: &ResultMapErrInstance) {
    let source_args = vec![instance.ok_type.clone(), instance.source_err_type.clone()];
    let target_args = vec![instance.ok_type.clone(), instance.target_err_type.clone()];
    let helper_name = c_result_map_err_helper_ident(instance);
    out.push_str("static ");
    out.push_str(&c_enum_ident("Result", &target_args));
    out.push(' ');
    out.push_str(&helper_name);
    out.push('(');
    out.push_str(&c_enum_ident("Result", &source_args));
    out.push_str(" input) {\n");
    out.push_str("    if (input.tag == ");
    out.push_str(&c_enum_variant_ident("Result", &source_args, "Err"));
    out.push_str(") {\n");
    out.push_str("        return (");
    out.push_str(&c_enum_ident("Result", &target_args));
    out.push_str("){.tag = ");
    out.push_str(&c_enum_variant_ident("Result", &target_args, "Err"));
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = ");
    out.push_str(&c_fn_ident(&instance.converter));
    out.push_str("(input.payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(")};\n");
    out.push_str("    }\n");
    out.push_str("    return (");
    out.push_str(&c_enum_ident("Result", &target_args));
    out.push_str("){.tag = ");
    out.push_str(&c_enum_variant_ident("Result", &target_args, "Ok"));
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Ok"));
    out.push_str(" = input.payload.");
    out.push_str(&c_payload_ident("Ok"));
    out.push_str("};\n");
    out.push_str("}\n");
}

fn emit_result_unwrap_or_helper(out: &mut String, instance: &ResultUnwrapOrInstance) {
    let result_args = vec![instance.ok_type.clone(), instance.err_type.clone()];
    let helper_name = c_result_unwrap_or_helper_ident(instance);
    out.push_str("static ");
    out.push_str(&c_type(&instance.ok_type));
    out.push(' ');
    out.push_str(&helper_name);
    out.push('(');
    out.push_str(&c_enum_ident("Result", &result_args));
    out.push_str(" input, ");
    out.push_str(&c_type(&instance.ok_type));
    out.push_str(" default_value) {\n");
    out.push_str("    if (input.tag == ");
    out.push_str(&c_enum_variant_ident("Result", &result_args, "Ok"));
    out.push_str(") {\n");
    if value_type_needs_release(&instance.ok_type) {
        emit_value_release_in_place(out, &instance.ok_type, "default_value", 2);
    }
    out.push_str("        return input.payload.");
    out.push_str(&c_payload_ident("Ok"));
    out.push_str(";\n");
    out.push_str("    }\n");
    if value_type_needs_release(&instance.err_type) {
        let value = format!("input.payload.{}", c_payload_ident("Err"));
        emit_value_release_in_place(out, &instance.err_type, &value, 1);
    }
    out.push_str("    return default_value;\n");
    out.push_str("}\n");
}

fn emit_result_map_helper(out: &mut String, instance: &ResultMapInstance) {
    let source_args = vec![instance.source_ok_type.clone(), instance.err_type.clone()];
    let target_args = vec![instance.target_ok_type.clone(), instance.err_type.clone()];
    let helper_name = c_result_map_helper_ident(instance);
    out.push_str("static ");
    out.push_str(&c_enum_ident("Result", &target_args));
    out.push(' ');
    out.push_str(&helper_name);
    out.push('(');
    out.push_str(&c_enum_ident("Result", &source_args));
    out.push_str(" input) {\n");
    out.push_str("    if (input.tag == ");
    out.push_str(&c_enum_variant_ident("Result", &source_args, "Ok"));
    out.push_str(") {\n");
    out.push_str("        return (");
    out.push_str(&c_enum_ident("Result", &target_args));
    out.push_str("){.tag = ");
    out.push_str(&c_enum_variant_ident("Result", &target_args, "Ok"));
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Ok"));
    out.push_str(" = ");
    out.push_str(&c_fn_ident(&instance.converter));
    out.push_str("(input.payload.");
    out.push_str(&c_payload_ident("Ok"));
    out.push_str(")};\n");
    out.push_str("    }\n");
    out.push_str("    return (");
    out.push_str(&c_enum_ident("Result", &target_args));
    out.push_str("){.tag = ");
    out.push_str(&c_enum_variant_ident("Result", &target_args, "Err"));
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = input.payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str("};\n");
    out.push_str("}\n");
}

fn emit_result_and_then_helper(out: &mut String, instance: &ResultAndThenInstance) {
    let source_args = vec![instance.source_ok_type.clone(), instance.err_type.clone()];
    let target_args = vec![instance.target_ok_type.clone(), instance.err_type.clone()];
    let helper_name = c_result_and_then_helper_ident(instance);
    out.push_str("static ");
    out.push_str(&c_enum_ident("Result", &target_args));
    out.push(' ');
    out.push_str(&helper_name);
    out.push('(');
    out.push_str(&c_enum_ident("Result", &source_args));
    out.push_str(" input) {\n");
    out.push_str("    if (input.tag == ");
    out.push_str(&c_enum_variant_ident("Result", &source_args, "Ok"));
    out.push_str(") {\n");
    out.push_str("        return ");
    out.push_str(&c_fn_ident(&instance.converter));
    out.push_str("(input.payload.");
    out.push_str(&c_payload_ident("Ok"));
    out.push_str(");\n");
    out.push_str("    }\n");
    out.push_str("    return (");
    out.push_str(&c_enum_ident("Result", &target_args));
    out.push_str("){.tag = ");
    out.push_str(&c_enum_variant_ident("Result", &target_args, "Err"));
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = input.payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str("};\n");
    out.push_str("}\n");
}

fn emit_option_unwrap_or_helper(out: &mut String, instance: &OptionUnwrapOrInstance) {
    let option_args = vec![instance.payload_type.clone()];
    let helper_name = c_option_unwrap_or_helper_ident(instance);
    out.push_str("static ");
    out.push_str(&c_type(&instance.payload_type));
    out.push(' ');
    out.push_str(&helper_name);
    out.push('(');
    out.push_str(&c_enum_ident("Option", &option_args));
    out.push_str(" input, ");
    out.push_str(&c_type(&instance.payload_type));
    out.push_str(" default_value) {\n");
    out.push_str("    if (input.tag == ");
    out.push_str(&c_enum_variant_ident("Option", &option_args, "Some"));
    out.push_str(") {\n");
    if value_type_needs_release(&instance.payload_type) {
        emit_value_release_in_place(out, &instance.payload_type, "default_value", 2);
    }
    out.push_str("        return input.payload.");
    out.push_str(&c_payload_ident("Some"));
    out.push_str(";\n");
    out.push_str("    }\n");
    out.push_str("    return default_value;\n");
    out.push_str("}\n");
}

fn emit_option_map_helper(out: &mut String, instance: &OptionMapInstance) {
    let source_args = vec![instance.source_type.clone()];
    let target_args = vec![instance.target_type.clone()];
    let helper_name = c_option_map_helper_ident(instance);
    out.push_str("static ");
    out.push_str(&c_enum_ident("Option", &target_args));
    out.push(' ');
    out.push_str(&helper_name);
    out.push('(');
    out.push_str(&c_enum_ident("Option", &source_args));
    out.push_str(" input) {\n");
    out.push_str("    if (input.tag == ");
    out.push_str(&c_enum_variant_ident("Option", &source_args, "Some"));
    out.push_str(") {\n");
    out.push_str("        return (");
    out.push_str(&c_enum_ident("Option", &target_args));
    out.push_str("){.tag = ");
    out.push_str(&c_enum_variant_ident("Option", &target_args, "Some"));
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Some"));
    out.push_str(" = ");
    out.push_str(&c_fn_ident(&instance.converter));
    out.push_str("(input.payload.");
    out.push_str(&c_payload_ident("Some"));
    out.push_str(")};\n");
    out.push_str("    }\n");
    out.push_str("    return (");
    out.push_str(&c_enum_ident("Option", &target_args));
    out.push_str("){.tag = ");
    out.push_str(&c_enum_variant_ident("Option", &target_args, "None"));
    out.push_str("};\n");
    out.push_str("}\n");
}

fn emit_option_and_then_helper(out: &mut String, instance: &OptionAndThenInstance) {
    let source_args = vec![instance.source_type.clone()];
    let target_args = vec![instance.target_type.clone()];
    let helper_name = c_option_and_then_helper_ident(instance);
    out.push_str("static ");
    out.push_str(&c_enum_ident("Option", &target_args));
    out.push(' ');
    out.push_str(&helper_name);
    out.push('(');
    out.push_str(&c_enum_ident("Option", &source_args));
    out.push_str(" input) {\n");
    out.push_str("    if (input.tag == ");
    out.push_str(&c_enum_variant_ident("Option", &source_args, "Some"));
    out.push_str(") {\n");
    out.push_str("        return ");
    out.push_str(&c_fn_ident(&instance.converter));
    out.push_str("(input.payload.");
    out.push_str(&c_payload_ident("Some"));
    out.push_str(");\n");
    out.push_str("    }\n");
    out.push_str("    return (");
    out.push_str(&c_enum_ident("Option", &target_args));
    out.push_str("){.tag = ");
    out.push_str(&c_enum_variant_ident("Option", &target_args, "None"));
    out.push_str("};\n");
    out.push_str("}\n");
}

fn emit_expr(out: &mut String, expr: &ValueExpr) {
    match expr {
        ValueExpr::StringLiteral(value) => {
            out.push_str("nomo_string_literal(\"");
            out.push_str(&escape_c_string(value));
            out.push_str("\")");
        }
        ValueExpr::IntLiteral(value) => out.push_str(&value.to_string()),
        ValueExpr::FloatLiteral(value) => out.push_str(value),
        ValueExpr::CharLiteral(value) => out.push_str(&(*value as u32).to_string()),
        ValueExpr::BoolLiteral(value) => out.push_str(if *value { "1" } else { "0" }),
        ValueExpr::VoidLiteral => out.push('0'),
        ValueExpr::Variable(name) => out.push_str(&c_var_ident(name)),
        ValueExpr::MutBorrow(path) => {
            out.push('&');
            emit_lvalue_path(out, path);
        }
        ValueExpr::Cast { expr, target_type } => {
            out.push_str("((");
            out.push_str(&c_type(target_type));
            out.push(')');
            emit_expr(out, expr);
            out.push(')');
        }
        ValueExpr::StructLiteral {
            type_name,
            struct_args,
            fields,
        } => {
            out.push('(');
            out.push_str(&c_struct_ident(type_name, struct_args));
            out.push_str("){");
            for (index, (field_name, value)) in fields.iter().enumerate() {
                if index > 0 {
                    out.push_str(", ");
                }
                out.push('.');
                out.push_str(&c_member_ident(field_name));
                out.push_str(" = ");
                emit_expr(out, value);
            }
            out.push('}');
        }
        ValueExpr::FieldAccess { base, field } => {
            out.push_str(&c_var_ident(base));
            out.push('.');
            out.push_str(&c_member_ident(field));
        }
        ValueExpr::EnumPayloadFieldAccess {
            value,
            variant,
            field,
        } => {
            emit_expr(out, value);
            out.push_str(".payload.");
            out.push_str(&c_payload_ident(variant));
            out.push('.');
            out.push_str(&c_member_ident(field));
        }
        ValueExpr::EnumVariant {
            enum_name,
            enum_args,
            variant,
            payload,
        } => {
            out.push('(');
            out.push_str(&c_enum_ident(enum_name, enum_args));
            out.push_str("){.tag = ");
            out.push_str(&c_enum_variant_ident(enum_name, enum_args, variant));
            if let Some(payload) = payload {
                out.push_str(", .payload.");
                out.push_str(&c_payload_ident(variant));
                out.push_str(" = ");
                emit_expr(out, payload);
            }
            out.push('}');
        }
        ValueExpr::EnumPayload { value, variant } => {
            emit_expr(out, value);
            out.push_str(".payload.");
            out.push_str(&c_payload_ident(variant));
        }
        ValueExpr::If {
            condition,
            then_branch,
            else_branch,
        } => {
            out.push('(');
            emit_expr(out, condition);
            out.push_str(" ? ");
            emit_expr(out, then_branch);
            out.push_str(" : ");
            emit_expr(out, else_branch);
            out.push(')');
        }
        ValueExpr::Panic {
            message,
            fallback_type,
        } => {
            out.push_str("(nomo_panic(");
            emit_string_data_expr(out, message);
            out.push_str("), ");
            out.push_str(&c_zero_value(fallback_type));
            out.push(')');
        }
        ValueExpr::Match { value, arms } => emit_match_expr(out, value, arms),
        ValueExpr::Binary {
            left,
            op,
            right,
            value_type,
        } => {
            if let Some(helper) = checked_binary_helper(op, value_type) {
                out.push_str(helper);
                out.push('(');
                emit_expr(out, left);
                out.push_str(", ");
                emit_expr(out, right);
                out.push(')');
            } else {
                out.push('(');
                emit_expr(out, left);
                if matches!(op, BinaryOp::BitAndNot) {
                    out.push_str(" & ~(");
                    emit_expr(out, right);
                    out.push(')');
                } else {
                    out.push(' ');
                    out.push_str(c_binary_op(op));
                    out.push(' ');
                    emit_expr(out, right);
                }
                out.push(')');
            }
        }
        ValueExpr::Unary { op, expr } => {
            out.push('(');
            out.push_str(c_unary_op(op));
            emit_expr(out, expr);
            out.push(')');
        }
        ValueExpr::StringCompare { left, op, right } => {
            out.push('(');
            if matches!(op, BinaryOp::NotEqual) {
                out.push('!');
            }
            out.push_str("nomo_string_equal(");
            emit_expr(out, left);
            out.push_str(", ");
            emit_expr(out, right);
            out.push_str("))");
        }
        ValueExpr::Call { name, args } => {
            if name == BUILTIN_PRINTLN_EXPR {
                out.push_str("(puts(");
                emit_string_data_expr(out, &args[0]);
                out.push_str("), 0)");
            } else if name == BUILTIN_PRINT_EXPR {
                out.push_str("(fputs(");
                emit_string_data_expr(out, &args[0]);
                out.push_str(", stdout), 0)");
            } else if name == BUILTIN_EPRINTLN_EXPR {
                out.push_str("(fputs(");
                emit_string_data_expr(out, &args[0]);
                out.push_str(", stderr), fputc('\\n', stderr), 0)");
            } else if name == BUILTIN_EPRINT_EXPR {
                out.push_str("(fputs(");
                emit_string_data_expr(out, &args[0]);
                out.push_str(", stderr), 0)");
            } else if name == BUILTIN_FFI_PUTS_EXPR {
                out.push_str("puts(");
                emit_string_data_expr(out, &args[0]);
                out.push(')');
            } else if let Some(symbol) = name.strip_prefix(EXTERN_CALL_PREFIX) {
                out.push_str(symbol);
                out.push('(');
                for (index, arg) in args.iter().enumerate() {
                    if index > 0 {
                        out.push_str(", ");
                    }
                    emit_expr(out, arg);
                }
                out.push(')');
            } else {
                out.push_str(&c_fn_ident(name));
                out.push('(');
                for (index, arg) in args.iter().enumerate() {
                    if index > 0 {
                        out.push_str(", ");
                    }
                    emit_expr(out, arg);
                }
                out.push(')');
            }
        }
        ValueExpr::StringLen { value } => {
            out.push_str("((uint64_t)strlen(");
            emit_string_data_expr(out, value);
            out.push_str("))");
        }
        ValueExpr::StringConcat { left, right } => {
            out.push_str("nomo_string_concat(");
            emit_expr(out, left);
            out.push_str(", ");
            emit_expr(out, right);
            out.push(')');
        }
        ValueExpr::StringIsEmpty { value } => {
            out.push_str("nomo_string_is_empty(");
            emit_expr(out, value);
            out.push(')');
        }
        ValueExpr::StringContains { value, needle } => {
            out.push_str("nomo_string_contains(");
            emit_expr(out, value);
            out.push_str(", ");
            emit_expr(out, needle);
            out.push(')');
        }
        ValueExpr::StringStartsWith { value, prefix } => {
            out.push_str("nomo_string_starts_with(");
            emit_expr(out, value);
            out.push_str(", ");
            emit_expr(out, prefix);
            out.push(')');
        }
        ValueExpr::StringEndsWith { value, suffix } => {
            out.push_str("nomo_string_ends_with(");
            emit_expr(out, value);
            out.push_str(", ");
            emit_expr(out, suffix);
            out.push(')');
        }
        ValueExpr::StringSplit { value, separator } => {
            out.push_str("nomo_string_split(");
            emit_expr(out, value);
            out.push_str(", ");
            emit_expr(out, separator);
            out.push(')');
        }
        ValueExpr::StringTrim { value } => {
            out.push_str("nomo_string_trim(");
            emit_expr(out, value);
            out.push(')');
        }
        ValueExpr::StringToLower { value } => {
            out.push_str("nomo_string_to_lower(");
            emit_expr(out, value);
            out.push(')');
        }
        ValueExpr::StringToUpper { value } => {
            out.push_str("nomo_string_to_upper(");
            emit_expr(out, value);
            out.push(')');
        }
        ValueExpr::CharIsDigit { value } => {
            out.push_str("nomo_char_is_digit(");
            emit_expr(out, value);
            out.push(')');
        }
        ValueExpr::CharIsAlpha { value } => {
            out.push_str("nomo_char_is_alpha(");
            emit_expr(out, value);
            out.push(')');
        }
        ValueExpr::CharIsWhitespace { value } => {
            out.push_str("nomo_char_is_whitespace(");
            emit_expr(out, value);
            out.push(')');
        }
        ValueExpr::CharToString { value } => {
            out.push_str("nomo_char_to_string(");
            emit_expr(out, value);
            out.push(')');
        }
        ValueExpr::OsPlatform => {
            out.push_str("nomo_os_platform()");
        }
        ValueExpr::OsArch => {
            out.push_str("nomo_os_arch()");
        }
        ValueExpr::OsPathSeparator => {
            out.push_str("nomo_os_path_separator()");
        }
        ValueExpr::OsLineEnding => {
            out.push_str("nomo_os_line_ending()");
        }
        ValueExpr::TimeNowMillis => {
            out.push_str("nomo_time_now_millis()");
        }
        ValueExpr::TimeMonotonicMillis => {
            out.push_str("nomo_time_monotonic_millis()");
        }
        ValueExpr::TimeDurationMillis { millis } => {
            out.push_str("(nomo_struct_Duration){ .nomo_member_millis = ");
            emit_expr(out, millis);
            out.push_str(" }");
        }
        ValueExpr::TimeDurationSeconds { seconds } => {
            out.push_str("(nomo_struct_Duration){ .nomo_member_millis = nomo_time_duration_seconds_to_millis(");
            emit_expr(out, seconds);
            out.push_str(") }");
        }
        ValueExpr::TimeDurationAsMillis { duration } => {
            out.push('(');
            emit_expr(out, duration);
            out.push_str(").nomo_member_millis");
        }
        ValueExpr::TimeFormatDuration { duration } => {
            out.push_str("nomo_time_format_duration_millis((");
            emit_expr(out, duration);
            out.push_str(").nomo_member_millis)");
        }
        ValueExpr::TimeSleep { duration } => {
            out.push_str("nomo_time_sleep_millis((");
            emit_expr(out, duration);
            out.push_str(").nomo_member_millis)");
        }
        ValueExpr::TimeSleepMillis { duration } => {
            out.push_str("nomo_time_sleep_millis(");
            emit_expr(out, duration);
            out.push(')');
        }
        ValueExpr::LogEnabled { level } => {
            out.push_str("nomo_log_enabled(");
            emit_expr(out, level);
            out.push(')');
        }
        ValueExpr::HashNew => {
            out.push_str("nomo_hash_new()");
        }
        ValueExpr::HashString { value } => {
            out.push_str("nomo_hash_string(");
            emit_expr(out, value);
            out.push(')');
        }
        ValueExpr::HashBytes { value } => {
            out.push_str("nomo_hash_bytes(");
            emit_expr(out, value);
            out.push(')');
        }
        ValueExpr::HashWriteString { state, value } => {
            out.push_str("nomo_hash_write_string(");
            emit_expr(out, state);
            out.push_str(", ");
            emit_expr(out, value);
            out.push(')');
        }
        ValueExpr::HashWriteBytes { state, value } => {
            out.push_str("nomo_hash_write_bytes(");
            emit_expr(out, state);
            out.push_str(", ");
            emit_expr(out, value);
            out.push(')');
        }
        ValueExpr::HashFinish { state } => {
            out.push_str("nomo_hash_finish(");
            emit_expr(out, state);
            out.push(')');
        }
        ValueExpr::CryptoSha256 { value } => {
            out.push_str("nomo_crypto_sha256(");
            emit_expr(out, value);
            out.push(')');
        }
        ValueExpr::CryptoSha512 { value } => {
            out.push_str("nomo_crypto_sha512(");
            emit_expr(out, value);
            out.push(')');
        }
        ValueExpr::CryptoRandomBytes { count } => {
            out.push_str("nomo_crypto_random_bytes(");
            emit_expr(out, count);
            out.push(')');
        }
        ValueExpr::JsonParse { value } => {
            out.push_str("nomo_json_parse(");
            emit_expr(out, value);
            out.push(')');
        }
        ValueExpr::JsonStringify { value } => {
            out.push_str("nomo_json_stringify(");
            emit_expr(out, value);
            out.push(')');
        }
        ValueExpr::RegexCompile { pattern } => {
            out.push_str("nomo_regex_compile(");
            emit_expr(out, pattern);
            out.push(')');
        }
        ValueExpr::RegexIsMatch { regex, value } => {
            out.push_str("nomo_regex_is_match(");
            emit_expr(out, regex);
            out.push_str(", ");
            emit_expr(out, value);
            out.push(')');
        }
        ValueExpr::RegexCaptures { regex, value } => {
            out.push_str("nomo_regex_captures(");
            emit_expr(out, regex);
            out.push_str(", ");
            emit_expr(out, value);
            out.push(')');
        }
        ValueExpr::CollectionsStringMapNew => {
            out.push_str("nomo_collections_map_new()");
        }
        ValueExpr::CollectionsStringMapLen { map } => {
            out.push_str("nomo_collections_map_len(");
            emit_expr(out, map);
            out.push(')');
        }
        ValueExpr::CollectionsStringMapGet { map, key } => {
            out.push_str("nomo_collections_map_get(");
            emit_expr(out, map);
            out.push_str(", ");
            emit_expr(out, key);
            out.push(')');
        }
        ValueExpr::CollectionsStringMapContains { map, key } => {
            out.push_str("nomo_collections_map_contains(");
            emit_expr(out, map);
            out.push_str(", ");
            emit_expr(out, key);
            out.push(')');
        }
        ValueExpr::CollectionsStringMapSet { map, key, value } => {
            out.push_str("nomo_collections_map_set(");
            emit_expr(out, map);
            out.push_str(", ");
            emit_expr(out, key);
            out.push_str(", ");
            emit_expr(out, value);
            out.push(')');
        }
        ValueExpr::CollectionsStringMapRemove { map, key } => {
            out.push_str("nomo_collections_map_remove(");
            emit_expr(out, map);
            out.push_str(", ");
            emit_expr(out, key);
            out.push(')');
        }
        ValueExpr::CollectionsStringSetNew => {
            out.push_str("nomo_collections_set_new()");
        }
        ValueExpr::CollectionsStringSetLen { set } => {
            out.push_str("nomo_collections_set_len(");
            emit_expr(out, set);
            out.push(')');
        }
        ValueExpr::CollectionsStringSetContains { set, value } => {
            out.push_str("nomo_collections_set_contains(");
            emit_expr(out, set);
            out.push_str(", ");
            emit_expr(out, value);
            out.push(')');
        }
        ValueExpr::CollectionsStringSetInsert { set, value } => {
            out.push_str("nomo_collections_set_insert(");
            emit_expr(out, set);
            out.push_str(", ");
            emit_expr(out, value);
            out.push(')');
        }
        ValueExpr::CollectionsStringSetRemove { set, value } => {
            out.push_str("nomo_collections_set_remove(");
            emit_expr(out, set);
            out.push_str(", ");
            emit_expr(out, value);
            out.push(')');
        }
        ValueExpr::ProcessExit { code } => {
            out.push_str("exit((int)");
            emit_expr(out, code);
            out.push(')');
        }
        ValueExpr::ProcessSpawn { command } => {
            out.push_str("nomo_process_spawn(");
            emit_expr(out, command);
            out.push(')');
        }
        ValueExpr::ProcessStatus { command } => {
            out.push_str("nomo_process_status(");
            emit_expr(out, command);
            out.push(')');
        }
        ValueExpr::ProcessExec { command } => {
            out.push_str("nomo_process_exec(");
            emit_expr(out, command);
            out.push(')');
        }
        ValueExpr::ProcessOutput { command } => {
            out.push_str("nomo_process_output(");
            emit_expr(out, command);
            out.push(')');
        }
        ValueExpr::NumParseI64 { value } => {
            out.push_str("nomo_num_parse_i64(");
            emit_expr(out, value);
            out.push(')');
        }
        ValueExpr::NumParseU64 { value } => {
            out.push_str("nomo_num_parse_u64(");
            emit_expr(out, value);
            out.push(')');
        }
        ValueExpr::NumParseF64 { value } => {
            out.push_str("nomo_num_parse_f64(");
            emit_expr(out, value);
            out.push(')');
        }
        ValueExpr::NumToString { value, value_type } => {
            out.push_str(num_to_string_helper_name(value_type));
            out.push('(');
            emit_expr(out, value);
            out.push(')');
        }
        ValueExpr::NumBinary {
            function,
            op,
            left,
            right,
            value_type,
        } => {
            let helper = match function {
                NumBinaryFunction::Checked => num_checked_binary_helper_name(op, value_type),
                NumBinaryFunction::Wrapping => num_wrapping_binary_helper_name(op, value_type),
            };
            out.push_str(helper);
            out.push('(');
            emit_expr(out, left);
            out.push_str(", ");
            emit_expr(out, right);
            out.push(')');
        }
        ValueExpr::PathJoin { left, right } => {
            out.push_str("nomo_path_join(");
            emit_expr(out, left);
            out.push_str(", ");
            emit_expr(out, right);
            out.push(')');
        }
        ValueExpr::PathBasename { path } => {
            out.push_str("nomo_path_basename(");
            emit_expr(out, path);
            out.push(')');
        }
        ValueExpr::PathDirname { path } => {
            out.push_str("nomo_path_dirname(");
            emit_expr(out, path);
            out.push(')');
        }
        ValueExpr::PathExtension { path } => {
            out.push_str("nomo_path_extension(");
            emit_expr(out, path);
            out.push(')');
        }
        ValueExpr::PathNormalize { path } => {
            out.push_str("nomo_path_normalize(");
            emit_expr(out, path);
            out.push(')');
        }
        ValueExpr::PathIsAbsolute { path } => {
            out.push_str("nomo_path_is_absolute(");
            emit_expr(out, path);
            out.push(')');
        }
        ValueExpr::MathUnary {
            function,
            value,
            value_type,
        } => {
            out.push_str(math_unary_function_name(*function, value_type));
            out.push('(');
            emit_expr(out, value);
            out.push(')');
        }
        ValueExpr::MathBinary {
            function,
            left,
            right,
            value_type,
        } => {
            out.push_str(math_binary_function_name(*function, value_type));
            out.push('(');
            emit_expr(out, left);
            out.push_str(", ");
            emit_expr(out, right);
            out.push(')');
        }
        ValueExpr::FsReadToString { path } => {
            out.push_str("nomo_fs_read_to_string(");
            emit_expr(out, path);
            out.push(')');
        }
        ValueExpr::FsWriteString { path, content } => {
            out.push_str("nomo_fs_write_string(");
            emit_expr(out, path);
            out.push_str(", ");
            emit_expr(out, content);
            out.push(')');
        }
        ValueExpr::FsReadBytes { path } => {
            out.push_str("nomo_fs_read_bytes(");
            emit_expr(out, path);
            out.push(')');
        }
        ValueExpr::FsWriteBytes { path, bytes } => {
            out.push_str("nomo_fs_write_bytes(");
            emit_expr(out, path);
            out.push_str(", ");
            emit_expr(out, bytes);
            out.push(')');
        }
        ValueExpr::FsExists { path } => {
            out.push_str("nomo_fs_exists(");
            emit_expr(out, path);
            out.push(')');
        }
        ValueExpr::FsMetadata { path } => {
            out.push_str("nomo_fs_metadata(");
            emit_expr(out, path);
            out.push(')');
        }
        ValueExpr::FsCreateDir { path } => {
            out.push_str("nomo_fs_create_dir(");
            emit_expr(out, path);
            out.push(')');
        }
        ValueExpr::FsRemoveDir { path } => {
            out.push_str("nomo_fs_remove_dir(");
            emit_expr(out, path);
            out.push(')');
        }
        ValueExpr::FsReadDir { path } => {
            out.push_str("nomo_fs_read_dir(");
            emit_expr(out, path);
            out.push(')');
        }
        ValueExpr::FsOpen { path } => {
            out.push_str("nomo_fs_open(");
            emit_expr(out, path);
            out.push(')');
        }
        ValueExpr::IoReadLine => {
            out.push_str("nomo_io_read_line()");
        }
        ValueExpr::FileClose { file } => {
            out.push_str("nomo_file_close(");
            emit_expr(out, file);
            out.push(')');
        }
        ValueExpr::FileReadToString { file } => {
            out.push_str("nomo_file_read_to_string(");
            emit_expr(out, file);
            out.push(')');
        }
        ValueExpr::FileWriteString { file, content } => {
            out.push_str("nomo_file_write_string(");
            emit_expr(out, file);
            out.push_str(", ");
            emit_expr(out, content);
            out.push(')');
        }
        ValueExpr::NetConnect { host, port } => {
            out.push_str("nomo_net_connect(");
            emit_expr(out, host);
            out.push_str(", ");
            emit_expr(out, port);
            out.push(')');
        }
        ValueExpr::NetListen { host, port } => {
            out.push_str("nomo_net_listen(");
            emit_expr(out, host);
            out.push_str(", ");
            emit_expr(out, port);
            out.push(')');
        }
        ValueExpr::NetUdpBind { host, port } => {
            out.push_str("nomo_net_udp_bind(");
            emit_expr(out, host);
            out.push_str(", ");
            emit_expr(out, port);
            out.push(')');
        }
        ValueExpr::TcpListenerAccept { listener } => {
            out.push_str("nomo_tcp_listener_accept(");
            emit_expr(out, listener);
            out.push(')');
        }
        ValueExpr::TcpListenerClose { listener } => {
            out.push_str("nomo_tcp_listener_close(");
            emit_expr(out, listener);
            out.push(')');
        }
        ValueExpr::TcpStreamClose { stream } => {
            out.push_str("nomo_tcp_stream_close(");
            emit_expr(out, stream);
            out.push(')');
        }
        ValueExpr::TcpStreamReadToString { stream } => {
            out.push_str("nomo_tcp_stream_read_to_string(");
            emit_expr(out, stream);
            out.push(')');
        }
        ValueExpr::TcpStreamWriteString { stream, content } => {
            out.push_str("nomo_tcp_stream_write_string(");
            emit_expr(out, stream);
            out.push_str(", ");
            emit_expr(out, content);
            out.push(')');
        }
        ValueExpr::UdpSocketClose { socket } => {
            out.push_str("nomo_udp_socket_close(");
            emit_expr(out, socket);
            out.push(')');
        }
        ValueExpr::UdpSocketRecvFromString { socket, max_bytes } => {
            out.push_str("nomo_udp_socket_recv_from_string(");
            emit_expr(out, socket);
            out.push_str(", ");
            emit_expr(out, max_bytes);
            out.push(')');
        }
        ValueExpr::UdpSocketSendToString {
            socket,
            content,
            host,
            port,
        } => {
            out.push_str("nomo_udp_socket_send_to_string(");
            emit_expr(out, socket);
            out.push_str(", ");
            emit_expr(out, content);
            out.push_str(", ");
            emit_expr(out, host);
            out.push_str(", ");
            emit_expr(out, port);
            out.push(')');
        }
        ValueExpr::ResultMapErr {
            result,
            ok_type,
            source_err_type,
            target_err_type,
            converter,
        } => {
            out.push_str(&c_result_map_err_helper_ident(&ResultMapErrInstance {
                ok_type: ok_type.clone(),
                source_err_type: source_err_type.clone(),
                target_err_type: target_err_type.clone(),
                converter: converter.clone(),
            }));
            out.push('(');
            emit_expr(out, result);
            out.push(')');
        }
        ValueExpr::ResultIsOk {
            result,
            ok_type,
            err_type,
        } => {
            out.push('(');
            emit_expr(out, result);
            out.push_str(".tag == ");
            out.push_str(&c_enum_variant_ident(
                "Result",
                &[ok_type.clone(), err_type.clone()],
                "Ok",
            ));
            out.push(')');
        }
        ValueExpr::ResultIsErr {
            result,
            ok_type,
            err_type,
        } => {
            out.push('(');
            emit_expr(out, result);
            out.push_str(".tag == ");
            out.push_str(&c_enum_variant_ident(
                "Result",
                &[ok_type.clone(), err_type.clone()],
                "Err",
            ));
            out.push(')');
        }
        ValueExpr::ResultUnwrapOr {
            result,
            default,
            ok_type,
            err_type,
        } => {
            out.push_str(&c_result_unwrap_or_helper_ident(&ResultUnwrapOrInstance {
                ok_type: ok_type.clone(),
                err_type: err_type.clone(),
            }));
            out.push('(');
            emit_expr(out, result);
            out.push_str(", ");
            emit_expr(out, default);
            out.push(')');
        }
        ValueExpr::ResultMap {
            result,
            source_ok_type,
            target_ok_type,
            err_type,
            converter,
        } => {
            out.push_str(&c_result_map_helper_ident(&ResultMapInstance {
                source_ok_type: source_ok_type.clone(),
                target_ok_type: target_ok_type.clone(),
                err_type: err_type.clone(),
                converter: converter.clone(),
            }));
            out.push('(');
            emit_expr(out, result);
            out.push(')');
        }
        ValueExpr::ResultAndThen {
            result,
            source_ok_type,
            target_ok_type,
            err_type,
            converter,
        } => {
            out.push_str(&c_result_and_then_helper_ident(&ResultAndThenInstance {
                source_ok_type: source_ok_type.clone(),
                target_ok_type: target_ok_type.clone(),
                err_type: err_type.clone(),
                converter: converter.clone(),
            }));
            out.push('(');
            emit_expr(out, result);
            out.push(')');
        }
        ValueExpr::OptionIsSome {
            option,
            payload_type,
        } => {
            out.push('(');
            emit_expr(out, option);
            out.push_str(".tag == ");
            out.push_str(&c_enum_variant_ident(
                "Option",
                &[payload_type.clone()],
                "Some",
            ));
            out.push(')');
        }
        ValueExpr::OptionIsNone {
            option,
            payload_type,
        } => {
            out.push('(');
            emit_expr(out, option);
            out.push_str(".tag == ");
            out.push_str(&c_enum_variant_ident(
                "Option",
                &[payload_type.clone()],
                "None",
            ));
            out.push(')');
        }
        ValueExpr::OptionUnwrapOr {
            option,
            default,
            payload_type,
        } => {
            out.push_str(&c_option_unwrap_or_helper_ident(&OptionUnwrapOrInstance {
                payload_type: payload_type.clone(),
            }));
            out.push('(');
            emit_expr(out, option);
            out.push_str(", ");
            emit_expr(out, default);
            out.push(')');
        }
        ValueExpr::OptionMap {
            option,
            source_type,
            target_type,
            converter,
        } => {
            out.push_str(&c_option_map_helper_ident(&OptionMapInstance {
                source_type: source_type.clone(),
                target_type: target_type.clone(),
                converter: converter.clone(),
            }));
            out.push('(');
            emit_expr(out, option);
            out.push(')');
        }
        ValueExpr::OptionAndThen {
            option,
            source_type,
            target_type,
            converter,
        } => {
            out.push_str(&c_option_and_then_helper_ident(&OptionAndThenInstance {
                source_type: source_type.clone(),
                target_type: target_type.clone(),
                converter: converter.clone(),
            }));
            out.push('(');
            emit_expr(out, option);
            out.push(')');
        }
        ValueExpr::EnvGet { name } => {
            out.push_str("nomo_env_get(");
            emit_expr(out, name);
            out.push(')');
        }
        ValueExpr::EnvSet { name, value } => {
            out.push_str("nomo_env_set(");
            emit_expr(out, name);
            out.push_str(", ");
            emit_expr(out, value);
            out.push(')');
        }
        ValueExpr::EnvCwd => out.push_str("nomo_env_cwd()"),
        ValueExpr::EnvHomeDir => out.push_str("nomo_env_home_dir()"),
        ValueExpr::EnvTempDir => out.push_str("nomo_env_temp_dir()"),
        ValueExpr::EnvArgs => out.push_str("nomo_env_args(nomo_argc, nomo_argv)"),
        ValueExpr::ArrayNew { element_type } if is_supported_array_element(element_type) => {
            out.push_str(&c_array_ident(element_type));
            out.push_str("_new()");
        }
        ValueExpr::ArrayLen { array } => {
            out.push_str("((uint64_t)");
            emit_expr(out, array);
            out.push_str(".len)");
        }
        ValueExpr::ArrayIter {
            array,
            element_type,
        } if is_supported_array_element(element_type) => {
            if expr_may_share_array_storage(array) {
                out.push_str(&c_array_ident(element_type));
                out.push_str("_retain(");
                emit_expr(out, array);
                out.push(')');
            } else {
                emit_expr(out, array);
            }
        }
        ValueExpr::ArrayGet {
            array,
            index,
            element_type,
        } if is_supported_array_element(element_type) => {
            out.push_str(&c_array_ident(element_type));
            out.push_str("_get(");
            emit_expr(out, array);
            out.push_str(", ");
            emit_expr(out, index);
            out.push(')');
        }
        ValueExpr::ArrayPop {
            array,
            element_type,
        } if is_supported_array_element(element_type) => {
            out.push_str(&c_array_ident(element_type));
            out.push_str("_pop(&");
            out.push_str(&c_var_ident(array));
            out.push(')');
        }
        ValueExpr::ArrayRemove {
            array,
            index,
            element_type,
        } if is_supported_array_element(element_type) => {
            out.push_str(&c_array_ident(element_type));
            out.push_str("_remove(&");
            out.push_str(&c_var_ident(array));
            out.push_str(", ");
            emit_expr(out, index);
            out.push(')');
        }
        ValueExpr::ArrayPush {
            array,
            value,
            element_type,
        } if is_supported_array_element(element_type) => {
            out.push_str(&c_array_ident(element_type));
            out.push_str("_push(");
            out.push_str(&c_var_ident(array));
            out.push_str(", ");
            emit_expr(out, value);
            out.push(')');
        }
        ValueExpr::ArraySet {
            array,
            index,
            value,
            element_type,
        } if is_supported_array_element(element_type) => {
            out.push_str(&c_array_ident(element_type));
            out.push_str("_set(");
            out.push_str(&c_var_ident(array));
            out.push_str(", ");
            emit_expr(out, index);
            out.push_str(", ");
            emit_expr(out, value);
            out.push(')');
        }
        ValueExpr::ArrayInsert {
            array,
            index,
            value,
            element_type,
        } if is_supported_array_element(element_type) => {
            out.push_str(&c_array_ident(element_type));
            out.push_str("_insert(");
            out.push_str(&c_var_ident(array));
            out.push_str(", ");
            emit_expr(out, index);
            out.push_str(", ");
            emit_expr(out, value);
            out.push(')');
        }
        ValueExpr::ArrayClear {
            array,
            element_type,
        } if is_supported_array_element(element_type) => {
            out.push_str(&c_array_ident(element_type));
            out.push_str("_clear(");
            out.push_str(&c_var_ident(array));
            out.push(')');
        }
        ValueExpr::ArrayNew { element_type }
        | ValueExpr::ArrayIter { element_type, .. }
        | ValueExpr::ArrayGet { element_type, .. }
        | ValueExpr::ArrayPop { element_type, .. }
        | ValueExpr::ArrayRemove { element_type, .. }
        | ValueExpr::ArrayPush { element_type, .. }
        | ValueExpr::ArraySet { element_type, .. }
        | ValueExpr::ArrayInsert { element_type, .. }
        | ValueExpr::ArrayClear { element_type, .. } => {
            panic!(
                "unsupported Array element type reached C codegen: {}",
                element_type.name()
            );
        }
    }
}

fn emit_lvalue_path(out: &mut String, path: &[String]) {
    let Some((root, fields)) = path.split_first() else {
        return;
    };
    out.push_str(&c_var_ident(root));
    for field in fields {
        out.push('.');
        out.push_str(&c_member_ident(field));
    }
}

fn emit_match_expr(out: &mut String, value: &ValueExpr, arms: &[MatchValueArm]) {
    emit_match_arm(out, value, arms, 0);
}

fn emit_match_arm(out: &mut String, value: &ValueExpr, arms: &[MatchValueArm], index: usize) {
    let arm = &arms[index];
    if index + 1 == arms.len() {
        emit_expr(out, &arm.value);
        return;
    }
    out.push('(');
    emit_expr(out, value);
    out.push_str(".tag == ");
    out.push_str(&c_enum_variant_ident(
        &arm.enum_name,
        &arm.enum_args,
        &arm.variant,
    ));
    out.push_str(" ? ");
    emit_expr(out, &arm.value);
    out.push_str(" : ");
    emit_match_arm(out, value, arms, index + 1);
    out.push(')');
}

#[cfg(test)]
mod tests;
