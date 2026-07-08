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

mod instances;
mod names;
mod runtime;
use instances::*;
use names::*;
use runtime::*;

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

fn emit_io_read_line_helper(out: &mut String) {
    let io_error = c_struct_ident("IoError", &[]);
    let result = c_enum_ident(
        "Result",
        &[
            ValueType::String,
            ValueType::Struct("IoError".to_string(), Vec::new()),
        ],
    );
    let ok = c_enum_variant_ident(
        "Result",
        &[
            ValueType::String,
            ValueType::Struct("IoError".to_string(), Vec::new()),
        ],
        "Ok",
    );
    let err = c_enum_variant_ident(
        "Result",
        &[
            ValueType::String,
            ValueType::Struct("IoError".to_string(), Vec::new()),
        ],
        "Err",
    );
    out.push_str("static ");
    out.push_str(&result);
    out.push_str(" nomo_io_read_line(void) {\n");
    out.push_str("    size_t capacity = 128;\n");
    out.push_str("    size_t len = 0;\n");
    out.push_str("    char *buffer = (char *)malloc(capacity);\n");
    out.push_str("    if (buffer == NULL) { nomo_panic(\"out of memory\"); }\n");
    out.push_str("    int ch = 0;\n");
    out.push_str("    while ((ch = fgetc(stdin)) != EOF) {\n");
    out.push_str("        if (ch == '\\n') { break; }\n");
    out.push_str("        if (len + 1 >= capacity) {\n");
    out.push_str("            capacity *= 2;\n");
    out.push_str("            char *next = (char *)realloc(buffer, capacity);\n");
    out.push_str("            if (next == NULL) {\n");
    out.push_str("                free(buffer);\n");
    out.push_str("                nomo_panic(\"out of memory\");\n");
    out.push_str("            }\n");
    out.push_str("            buffer = next;\n");
    out.push_str("        }\n");
    out.push_str("        buffer[len] = (char)ch;\n");
    out.push_str("        len += 1;\n");
    out.push_str("    }\n");
    out.push_str("    if (ch == EOF && len == 0) {\n");
    out.push_str(
        "        const char *message = ferror(stdin) ? strerror(errno) : \"end of input\";\n",
    );
    out.push_str("        free(buffer);\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&io_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(message)}};\n");
    out.push_str("    }\n");
    out.push_str("    if (len > 0 && buffer[len - 1] == '\\r') { len -= 1; }\n");
    out.push_str("    buffer[len] = '\\0';\n");
    out.push_str("    return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&ok);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Ok"));
    out.push_str(" = nomo_string_owned(buffer)};\n");
    out.push_str("}\n");
}

fn emit_num_parse_i64_helper(out: &mut String) {
    emit_num_parse_helper(out, "i64", &ValueType::Int, "strtoll", "int64_t");
}

fn emit_num_parse_u64_helper(out: &mut String) {
    emit_num_parse_helper(out, "u64", &ValueType::U64, "strtoull", "uint64_t");
}

fn emit_num_parse_f64_helper(out: &mut String) {
    emit_num_parse_helper(out, "f64", &ValueType::Float, "strtod", "double");
}

fn emit_num_parse_helper(
    out: &mut String,
    suffix: &str,
    ok_type: &ValueType,
    c_parse_fn: &str,
    c_value_type: &str,
) {
    let num_error = ValueType::Struct("NumError".to_string(), Vec::new());
    let result = c_enum_ident("Result", &[ok_type.clone(), num_error.clone()]);
    let ok = c_enum_variant_ident("Result", &[ok_type.clone(), num_error.clone()], "Ok");
    let err = c_enum_variant_ident("Result", &[ok_type.clone(), num_error], "Err");
    let num_error_struct = c_struct_ident("NumError", &[]);
    out.push_str("static ");
    out.push_str(&result);
    out.push_str(" nomo_num_parse_");
    out.push_str(suffix);
    out.push_str("(nomo_string text) {\n");
    out.push_str("    errno = 0;\n");
    out.push_str("    char *end = NULL;\n");
    if suffix == "u64" {
        out.push_str("    if (text.data[0] == '-') {\n");
        emit_num_parse_error(out, &result, &err, &num_error_struct, suffix);
        out.push_str("    }\n");
    }
    out.push_str("    ");
    out.push_str(c_value_type);
    out.push_str(" parsed = (");
    out.push_str(c_value_type);
    out.push(')');
    out.push_str(c_parse_fn);
    if suffix == "f64" {
        out.push_str("(text.data, &end);\n");
    } else {
        out.push_str("(text.data, &end, 10);\n");
    }
    out.push_str("    if (text.data[0] == '\\0' || *end != '\\0' || errno == ERANGE) {\n");
    emit_num_parse_error(out, &result, &err, &num_error_struct, suffix);
    out.push_str("    }\n");
    out.push_str("    return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&ok);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Ok"));
    out.push_str(" = parsed};\n");
    out.push_str("}\n");
}

fn emit_num_parse_error(out: &mut String, result: &str, err: &str, num_error: &str, suffix: &str) {
    out.push_str("        return (");
    out.push_str(result);
    out.push_str("){.tag = ");
    out.push_str(err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(num_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_literal(\"invalid ");
    out.push_str(suffix);
    out.push_str("\")}};\n");
}

fn emit_json_helpers(out: &mut String) {
    let json_value = ValueType::Struct("JsonValue".to_string(), Vec::new());
    let json_error = ValueType::Struct("JsonError".to_string(), Vec::new());
    let result = c_enum_ident("Result", &[json_value.clone(), json_error.clone()]);
    let ok = c_enum_variant_ident("Result", &[json_value.clone(), json_error.clone()], "Ok");
    let err = c_enum_variant_ident("Result", &[json_value.clone(), json_error.clone()], "Err");
    let json_value_struct = c_struct_ident("JsonValue", &[]);
    let json_error_struct = c_struct_ident("JsonError", &[]);
    out.push_str(
        "static void nomo_json_skip_ws(const char *text, size_t *index) {\n\
    while (text[*index] == ' ' || text[*index] == '\\n' || text[*index] == '\\r' || text[*index] == '\\t') { *index += 1; }\n\
}\n\
\n\
static int nomo_json_parse_value(const char *text, size_t *index);\n\
\n\
static int nomo_json_parse_hex4(const char *text, size_t *index) {\n\
    for (int i = 0; i < 4; i += 1) {\n\
        unsigned char ch = (unsigned char)text[*index];\n\
        if (!((ch >= '0' && ch <= '9') || (ch >= 'a' && ch <= 'f') || (ch >= 'A' && ch <= 'F'))) { return 0; }\n\
        *index += 1;\n\
    }\n\
    return 1;\n\
}\n\
\n\
static int nomo_json_parse_string_token(const char *text, size_t *index) {\n\
    if (text[*index] != '\\\"') { return 0; }\n\
    *index += 1;\n\
    while (text[*index] != '\\0') {\n\
        unsigned char ch = (unsigned char)text[*index];\n\
        if (ch == '\\\"') { *index += 1; return 1; }\n\
        if (ch < 0x20) { return 0; }\n\
        if (ch == '\\\\') {\n\
            *index += 1;\n\
            char esc = text[*index];\n\
            if (esc == '\\\"' || esc == '\\\\' || esc == '/' || esc == 'b' || esc == 'f' || esc == 'n' || esc == 'r' || esc == 't') { *index += 1; continue; }\n\
            if (esc == 'u') { *index += 1; if (!nomo_json_parse_hex4(text, index)) { return 0; } continue; }\n\
            return 0;\n\
        }\n\
        *index += 1;\n\
    }\n\
    return 0;\n\
}\n\
\n\
static int nomo_json_parse_literal(const char *text, size_t *index, const char *literal) {\n\
    size_t len = strlen(literal);\n\
    if (strncmp(text + *index, literal, len) != 0) { return 0; }\n\
    *index += len;\n\
    return 1;\n\
}\n\
\n\
static int nomo_json_parse_number(const char *text, size_t *index) {\n\
    if (text[*index] == '-') { *index += 1; }\n\
    if (text[*index] == '0') {\n\
        *index += 1;\n\
    } else if (text[*index] >= '1' && text[*index] <= '9') {\n\
        while (text[*index] >= '0' && text[*index] <= '9') { *index += 1; }\n\
    } else {\n\
        return 0;\n\
    }\n\
    if (text[*index] == '.') {\n\
        *index += 1;\n\
        if (!(text[*index] >= '0' && text[*index] <= '9')) { return 0; }\n\
        while (text[*index] >= '0' && text[*index] <= '9') { *index += 1; }\n\
    }\n\
    if (text[*index] == 'e' || text[*index] == 'E') {\n\
        *index += 1;\n\
        if (text[*index] == '+' || text[*index] == '-') { *index += 1; }\n\
        if (!(text[*index] >= '0' && text[*index] <= '9')) { return 0; }\n\
        while (text[*index] >= '0' && text[*index] <= '9') { *index += 1; }\n\
    }\n\
    return 1;\n\
}\n\
\n\
static int nomo_json_parse_array(const char *text, size_t *index) {\n\
    if (text[*index] != '[') { return 0; }\n\
    *index += 1;\n\
    nomo_json_skip_ws(text, index);\n\
    if (text[*index] == ']') { *index += 1; return 1; }\n\
    while (1) {\n\
        if (!nomo_json_parse_value(text, index)) { return 0; }\n\
        nomo_json_skip_ws(text, index);\n\
        if (text[*index] == ']') { *index += 1; return 1; }\n\
        if (text[*index] != ',') { return 0; }\n\
        *index += 1;\n\
        nomo_json_skip_ws(text, index);\n\
    }\n\
}\n\
\n\
static int nomo_json_parse_object(const char *text, size_t *index) {\n\
    if (text[*index] != '{') { return 0; }\n\
    *index += 1;\n\
    nomo_json_skip_ws(text, index);\n\
    if (text[*index] == '}') { *index += 1; return 1; }\n\
    while (1) {\n\
        if (!nomo_json_parse_string_token(text, index)) { return 0; }\n\
        nomo_json_skip_ws(text, index);\n\
        if (text[*index] != ':') { return 0; }\n\
        *index += 1;\n\
        if (!nomo_json_parse_value(text, index)) { return 0; }\n\
        nomo_json_skip_ws(text, index);\n\
        if (text[*index] == '}') { *index += 1; return 1; }\n\
        if (text[*index] != ',') { return 0; }\n\
        *index += 1;\n\
        nomo_json_skip_ws(text, index);\n\
    }\n\
}\n\
\n\
static int nomo_json_parse_value(const char *text, size_t *index) {\n\
    nomo_json_skip_ws(text, index);\n\
    char ch = text[*index];\n\
    if (ch == '\\\"') { return nomo_json_parse_string_token(text, index); }\n\
    if (ch == '{') { return nomo_json_parse_object(text, index); }\n\
    if (ch == '[') { return nomo_json_parse_array(text, index); }\n\
    if (ch == '-' || (ch >= '0' && ch <= '9')) { return nomo_json_parse_number(text, index); }\n\
    if (ch == 't') { return nomo_json_parse_literal(text, index, \"true\"); }\n\
    if (ch == 'f') { return nomo_json_parse_literal(text, index, \"false\"); }\n\
    if (ch == 'n') { return nomo_json_parse_literal(text, index, \"null\"); }\n\
    return 0;\n\
}\n\
",
    );
    out.push_str("static ");
    out.push_str(&result);
    out.push_str(" nomo_json_parse(nomo_string text) {\n");
    out.push_str("    size_t index = 0;\n");
    out.push_str("    if (!nomo_json_parse_value(text.data, &index)) {\n");
    emit_json_parse_error(out, &result, &err, &json_error_struct);
    out.push_str("    }\n");
    out.push_str("    nomo_json_skip_ws(text.data, &index);\n");
    out.push_str("    if (text.data[index] != '\\0') {\n");
    emit_json_parse_error(out, &result, &err, &json_error_struct);
    out.push_str("    }\n");
    out.push_str("    return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&ok);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Ok"));
    out.push_str(" = (");
    out.push_str(&json_value_struct);
    out.push_str("){.");
    out.push_str(&c_member_ident("raw"));
    out.push_str(" = nomo_string_retain(text)}};\n");
    out.push_str("}\n\nstatic nomo_string nomo_json_stringify(");
    out.push_str(&json_value_struct);
    out.push_str(" value) {\n");
    out.push_str("    return nomo_string_retain(value.");
    out.push_str(&c_member_ident("raw"));
    out.push_str(");\n");
    out.push_str("}\n");
}

fn emit_json_parse_error(out: &mut String, result: &str, err: &str, json_error: &str) {
    out.push_str("        return (");
    out.push_str(result);
    out.push_str("){.tag = ");
    out.push_str(err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(json_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_literal(\"invalid json\")}};\n");
}

fn emit_regex_helpers(out: &mut String) {
    let regex_type = ValueType::Struct("Regex".to_string(), Vec::new());
    let regex_error = ValueType::Struct("RegexError".to_string(), Vec::new());
    let result = c_enum_ident("Result", &[regex_type.clone(), regex_error.clone()]);
    let ok = c_enum_variant_ident("Result", &[regex_type.clone(), regex_error.clone()], "Ok");
    let err = c_enum_variant_ident("Result", &[regex_type.clone(), regex_error.clone()], "Err");
    let regex_struct = c_struct_ident("Regex", &[]);
    let regex_error_struct = c_struct_ident("RegexError", &[]);
    let string_array_type = ValueType::Array(Box::new(ValueType::String));
    let option_array = c_enum_ident("Option", std::slice::from_ref(&string_array_type));
    let some_array =
        c_enum_variant_ident("Option", std::slice::from_ref(&string_array_type), "Some");
    let none_array =
        c_enum_variant_ident("Option", std::slice::from_ref(&string_array_type), "None");
    let string_array = c_array_ident(&ValueType::String);

    out.push_str("static ");
    out.push_str(&result);
    out.push_str(" nomo_regex_compile(nomo_string pattern) {\n");
    out.push_str("#if defined(_WIN32)\n");
    out.push_str("    (void)pattern;\n");
    out.push_str("#else\n");
    out.push_str("    regex_t compiled;\n");
    out.push_str("    int status = regcomp(&compiled, pattern.data, REG_EXTENDED);\n");
    out.push_str("    if (status != 0) {\n");
    out.push_str("        char message[256];\n");
    out.push_str("        regerror(status, &compiled, message, sizeof(message));\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&regex_error_struct);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(message)}};\n");
    out.push_str("    }\n");
    out.push_str("    regfree(&compiled);\n");
    out.push_str("#endif\n");
    out.push_str("    return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&ok);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Ok"));
    out.push_str(" = (");
    out.push_str(&regex_struct);
    out.push_str("){.");
    out.push_str(&c_member_ident("pattern"));
    out.push_str(" = nomo_string_retain(pattern)}};\n");
    out.push_str("}\n\nstatic int nomo_regex_is_match(");
    out.push_str(&regex_struct);
    out.push_str(" regex, nomo_string value) {\n");
    out.push_str("#if defined(_WIN32)\n");
    out.push_str("    return strstr(value.data, regex.");
    out.push_str(&c_member_ident("pattern"));
    out.push_str(".data) != NULL;\n");
    out.push_str("#else\n");
    out.push_str("    regex_t compiled;\n");
    out.push_str("    int status = regcomp(&compiled, regex.");
    out.push_str(&c_member_ident("pattern"));
    out.push_str(".data, REG_EXTENDED);\n");
    out.push_str("    if (status != 0) { return 0; }\n");
    out.push_str("    status = regexec(&compiled, value.data, 0, NULL, 0);\n");
    out.push_str("    regfree(&compiled);\n");
    out.push_str("    return status == 0;\n");
    out.push_str("#endif\n");
    out.push_str("}\n\nstatic ");
    out.push_str(&option_array);
    out.push_str(" nomo_regex_captures(");
    out.push_str(&regex_struct);
    out.push_str(" regex, nomo_string value) {\n");
    out.push_str("#if defined(_WIN32)\n");
    out.push_str("    char *found = strstr(value.data, regex.");
    out.push_str(&c_member_ident("pattern"));
    out.push_str(".data);\n");
    out.push_str("    if (found == NULL) { return (");
    out.push_str(&option_array);
    out.push_str("){.tag = ");
    out.push_str(&none_array);
    out.push_str("}; }\n");
    out.push_str("    ");
    out.push_str(&string_array);
    out.push_str(" captures = ");
    out.push_str(&string_array);
    out.push_str("_new();\n");
    out.push_str("    size_t start = (size_t)(found - value.data);\n");
    out.push_str(
        "    nomo_string capture = nomo_string_from_slice(value.data, start, strlen(regex.",
    );
    out.push_str(&c_member_ident("pattern"));
    out.push_str(".data));\n");
    out.push_str("    captures = ");
    out.push_str(&string_array);
    out.push_str("_push(captures, capture);\n");
    out.push_str("    nomo_string_release(capture);\n");
    out.push_str("#else\n");
    out.push_str("    regex_t compiled;\n");
    out.push_str("    int status = regcomp(&compiled, regex.");
    out.push_str(&c_member_ident("pattern"));
    out.push_str(".data, REG_EXTENDED);\n");
    out.push_str("    if (status != 0) { return (");
    out.push_str(&option_array);
    out.push_str("){.tag = ");
    out.push_str(&none_array);
    out.push_str("}; }\n");
    out.push_str("    size_t count = compiled.re_nsub + 1;\n");
    out.push_str("    regmatch_t *matches = (regmatch_t *)calloc(count, sizeof(regmatch_t));\n");
    out.push_str(
        "    if (matches == NULL) { regfree(&compiled); nomo_panic(\"out of memory\"); }\n",
    );
    out.push_str("    status = regexec(&compiled, value.data, count, matches, 0);\n");
    out.push_str("    if (status != 0) { free(matches); regfree(&compiled); return (");
    out.push_str(&option_array);
    out.push_str("){.tag = ");
    out.push_str(&none_array);
    out.push_str("}; }\n");
    out.push_str("    ");
    out.push_str(&string_array);
    out.push_str(" captures = ");
    out.push_str(&string_array);
    out.push_str("_new();\n");
    out.push_str("    for (size_t i = 0; i < count; i += 1) {\n");
    out.push_str("        nomo_string capture;\n");
    out.push_str("        if (matches[i].rm_so >= 0 && matches[i].rm_eo >= matches[i].rm_so) {\n");
    out.push_str("            capture = nomo_string_from_slice(value.data, (size_t)matches[i].rm_so, (size_t)(matches[i].rm_eo - matches[i].rm_so));\n");
    out.push_str("        } else {\n");
    out.push_str("            capture = nomo_string_literal(\"\");\n");
    out.push_str("        }\n");
    out.push_str("        captures = ");
    out.push_str(&string_array);
    out.push_str("_push(captures, capture);\n");
    out.push_str("        nomo_string_release(capture);\n");
    out.push_str("    }\n");
    out.push_str("    free(matches);\n");
    out.push_str("    regfree(&compiled);\n");
    out.push_str("#endif\n");
    out.push_str("    return (");
    out.push_str(&option_array);
    out.push_str("){.tag = ");
    out.push_str(&some_array);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Some"));
    out.push_str(" = captures};\n");
    out.push_str("}\n");
}

fn emit_num_checked_binary_helper(out: &mut String, instance: &NumCheckedBinaryInstance) {
    let option = c_enum_ident("Option", std::slice::from_ref(&instance.value_type));
    let some = c_enum_variant_ident("Option", std::slice::from_ref(&instance.value_type), "Some");
    let none = c_enum_variant_ident("Option", std::slice::from_ref(&instance.value_type), "None");
    let c_type = c_type(&instance.value_type);
    let helper = num_checked_binary_helper_name(&instance.op, &instance.value_type);
    out.push_str("static ");
    out.push_str(&option);
    out.push(' ');
    out.push_str(helper);
    out.push('(');
    out.push_str(&c_type);
    out.push_str(" left, ");
    out.push_str(&c_type);
    out.push_str(" right) {\n");
    emit_num_checked_overflow_guard(out, instance, &option, &none);
    out.push_str("    return (");
    out.push_str(&option);
    out.push_str("){.tag = ");
    out.push_str(&some);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Some"));
    out.push_str(" = ");
    out.push_str(num_wrapping_binary_helper_name(
        &instance.op,
        &instance.value_type,
    ));
    out.push_str("(left, right)};\n");
    out.push_str("}\n");
}

fn emit_num_checked_overflow_guard(
    out: &mut String,
    instance: &NumCheckedBinaryInstance,
    option: &str,
    none: &str,
) {
    let condition = num_checked_overflow_condition(&instance.op, &instance.value_type);
    out.push_str("    if (");
    out.push_str(condition);
    out.push_str(") { return (");
    out.push_str(option);
    out.push_str("){.tag = ");
    out.push_str(none);
    out.push_str("}; }\n");
}

fn num_checked_overflow_condition(op: &BinaryOp, value_type: &ValueType) -> &'static str {
    match (op, value_type) {
        (BinaryOp::Add, ValueType::Int) => {
            "(right > 0 && left > LLONG_MAX - right) || (right < 0 && left < LLONG_MIN - right)"
        }
        (BinaryOp::Subtract, ValueType::Int) => {
            "(right < 0 && left > LLONG_MAX + right) || (right > 0 && left < LLONG_MIN + right)"
        }
        (BinaryOp::Multiply, ValueType::Int) => {
            "left != 0 && right != 0 && ((left == -1 && right == LLONG_MIN) || (right == -1 && left == LLONG_MIN) || (left > 0 ? (right > 0 ? left > LLONG_MAX / right : right < LLONG_MIN / left) : (right > 0 ? left < LLONG_MIN / right : left < LLONG_MAX / right)))"
        }
        (BinaryOp::Add, ValueType::I32) => {
            "(right > 0 && left > INT32_MAX - right) || (right < 0 && left < INT32_MIN - right)"
        }
        (BinaryOp::Subtract, ValueType::I32) => {
            "(right < 0 && left > INT32_MAX + right) || (right > 0 && left < INT32_MIN + right)"
        }
        (BinaryOp::Multiply, ValueType::I32) => {
            "left != 0 && right != 0 && ((left == -1 && right == INT32_MIN) || (right == -1 && left == INT32_MIN) || (left > 0 ? (right > 0 ? left > INT32_MAX / right : right < INT32_MIN / left) : (right > 0 ? left < INT32_MIN / right : left < INT32_MAX / right)))"
        }
        (BinaryOp::Add, ValueType::U32) => "left > UINT32_MAX - right",
        (BinaryOp::Subtract, ValueType::U32) => "left < right",
        (BinaryOp::Multiply, ValueType::U32) => "right != 0 && left > UINT32_MAX / right",
        (BinaryOp::Add, ValueType::U64) => "left > UINT64_MAX - right",
        (BinaryOp::Subtract, ValueType::U64) => "left < right",
        (BinaryOp::Multiply, ValueType::U64) => "right != 0 && left > UINT64_MAX / right",
        _ => unreachable!("num checked helpers only support integer add/sub/mul"),
    }
}

fn emit_fs_read_to_string_helper(out: &mut String) {
    let fs_error = c_struct_ident("FsError", &[]);
    let result = c_enum_ident(
        "Result",
        &[
            ValueType::String,
            ValueType::Struct("FsError".to_string(), Vec::new()),
        ],
    );
    let ok = c_enum_variant_ident(
        "Result",
        &[
            ValueType::String,
            ValueType::Struct("FsError".to_string(), Vec::new()),
        ],
        "Ok",
    );
    let err = c_enum_variant_ident(
        "Result",
        &[
            ValueType::String,
            ValueType::Struct("FsError".to_string(), Vec::new()),
        ],
        "Err",
    );
    out.push_str("static ");
    out.push_str(&result);
    out.push_str(" nomo_fs_read_to_string(nomo_string path) {\n");
    out.push_str("    FILE *file = fopen(path.data, \"rb\");\n");
    out.push_str("    if (file == NULL) {\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&fs_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(strerror(errno))}};\n");
    out.push_str("    }\n");
    out.push_str("    if (fseek(file, 0, SEEK_END) != 0) {\n");
    out.push_str("        const char *message = strerror(errno);\n");
    out.push_str("        fclose(file);\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&fs_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(message)}};\n");
    out.push_str("    }\n");
    out.push_str("    long size = ftell(file);\n");
    out.push_str("    if (size < 0 || fseek(file, 0, SEEK_SET) != 0) {\n");
    out.push_str("        const char *message = strerror(errno);\n");
    out.push_str("        fclose(file);\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&fs_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(message)}};\n");
    out.push_str("    }\n");
    out.push_str("    char *buffer = (char *)malloc((size_t)size + 1);\n");
    out.push_str("    if (buffer == NULL) {\n");
    out.push_str("        fclose(file);\n");
    out.push_str("        nomo_panic(\"out of memory\");\n");
    out.push_str("    }\n");
    out.push_str("    size_t read = fread(buffer, 1, (size_t)size, file);\n");
    out.push_str("    if (read != (size_t)size) {\n");
    out.push_str(
        "        const char *message = ferror(file) ? strerror(errno) : \"short read\";\n",
    );
    out.push_str("        free(buffer);\n");
    out.push_str("        fclose(file);\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&fs_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(message)}};\n");
    out.push_str("    }\n");
    out.push_str("    buffer[size] = '\\0';\n");
    out.push_str("    fclose(file);\n");
    out.push_str("    return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&ok);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Ok"));
    out.push_str(" = nomo_string_owned(buffer)};\n");
    out.push_str("}\n");
}

fn emit_fs_write_string_helper(out: &mut String) {
    let fs_error = c_struct_ident("FsError", &[]);
    let result = c_enum_ident(
        "Result",
        &[
            ValueType::Void,
            ValueType::Struct("FsError".to_string(), Vec::new()),
        ],
    );
    let ok = c_enum_variant_ident(
        "Result",
        &[
            ValueType::Void,
            ValueType::Struct("FsError".to_string(), Vec::new()),
        ],
        "Ok",
    );
    let err = c_enum_variant_ident(
        "Result",
        &[
            ValueType::Void,
            ValueType::Struct("FsError".to_string(), Vec::new()),
        ],
        "Err",
    );
    out.push_str("static ");
    out.push_str(&result);
    out.push_str(" nomo_fs_write_string(nomo_string path, nomo_string content) {\n");
    out.push_str("    FILE *file = fopen(path.data, \"wb\");\n");
    out.push_str("    if (file == NULL) {\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&fs_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(strerror(errno))}};\n");
    out.push_str("    }\n");
    out.push_str("    size_t len = strlen(content.data);\n");
    out.push_str("    if (fwrite(content.data, 1, len, file) != len) {\n");
    out.push_str("        const char *message = strerror(errno);\n");
    out.push_str("        fclose(file);\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&fs_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(message)}};\n");
    out.push_str("    }\n");
    out.push_str("    if (fclose(file) != 0) {\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&fs_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(strerror(errno))}};\n");
    out.push_str("    }\n");
    out.push_str("    return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&ok);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Ok"));
    out.push_str(" = 0};\n");
    out.push_str("}\n");
}

fn emit_fs_read_bytes_helper(out: &mut String) {
    let fs_error = c_struct_ident("FsError", &[]);
    let array_u32 = ValueType::Array(Box::new(ValueType::U32));
    let result = c_enum_ident(
        "Result",
        &[
            array_u32.clone(),
            ValueType::Struct("FsError".to_string(), Vec::new()),
        ],
    );
    let ok = c_enum_variant_ident(
        "Result",
        &[
            array_u32.clone(),
            ValueType::Struct("FsError".to_string(), Vec::new()),
        ],
        "Ok",
    );
    let err = c_enum_variant_ident(
        "Result",
        &[
            array_u32,
            ValueType::Struct("FsError".to_string(), Vec::new()),
        ],
        "Err",
    );
    out.push_str("static ");
    out.push_str(&result);
    out.push_str(" nomo_fs_read_bytes(nomo_string path) {\n");
    out.push_str("    FILE *file = fopen(path.data, \"rb\");\n");
    out.push_str("    if (file == NULL) {\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&fs_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(strerror(errno))}};\n");
    out.push_str("    }\n");
    out.push_str("    nomo_array_u32 bytes = nomo_array_u32_new();\n");
    out.push_str("    int ch = 0;\n");
    out.push_str("    while ((ch = fgetc(file)) != EOF) {\n");
    out.push_str("        bytes = nomo_array_u32_push(bytes, (uint32_t)(unsigned char)ch);\n");
    out.push_str("    }\n");
    out.push_str("    if (ferror(file)) {\n");
    out.push_str("        const char *message = strerror(errno);\n");
    out.push_str("        fclose(file);\n");
    out.push_str("        nomo_array_u32_release(bytes);\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&fs_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(message)}};\n");
    out.push_str("    }\n");
    out.push_str("    if (fclose(file) != 0) {\n");
    out.push_str("        const char *message = strerror(errno);\n");
    out.push_str("        nomo_array_u32_release(bytes);\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&fs_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(message)}};\n");
    out.push_str("    }\n");
    out.push_str("    return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&ok);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Ok"));
    out.push_str(" = bytes};\n");
    out.push_str("}\n");
}

fn emit_fs_write_bytes_helper(out: &mut String) {
    let fs_error = c_struct_ident("FsError", &[]);
    let result = c_enum_ident(
        "Result",
        &[
            ValueType::Void,
            ValueType::Struct("FsError".to_string(), Vec::new()),
        ],
    );
    let ok = c_enum_variant_ident(
        "Result",
        &[
            ValueType::Void,
            ValueType::Struct("FsError".to_string(), Vec::new()),
        ],
        "Ok",
    );
    let err = c_enum_variant_ident(
        "Result",
        &[
            ValueType::Void,
            ValueType::Struct("FsError".to_string(), Vec::new()),
        ],
        "Err",
    );
    out.push_str("static ");
    out.push_str(&result);
    out.push_str(" nomo_fs_write_bytes(nomo_string path, nomo_array_u32 bytes) {\n");
    out.push_str("    FILE *file = fopen(path.data, \"wb\");\n");
    out.push_str("    if (file == NULL) {\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&fs_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(strerror(errno))}};\n");
    out.push_str("    }\n");
    out.push_str("    for (size_t i = 0; i < bytes.len; i += 1) {\n");
    out.push_str("        unsigned char value = (unsigned char)(bytes.data[i] & 0xffU);\n");
    out.push_str("        if (fwrite(&value, 1, 1, file) != 1) {\n");
    out.push_str("            const char *message = strerror(errno);\n");
    out.push_str("            fclose(file);\n");
    out.push_str("            return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&fs_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(message)}};\n");
    out.push_str("        }\n");
    out.push_str("    }\n");
    out.push_str("    if (fclose(file) != 0) {\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&fs_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(strerror(errno))}};\n");
    out.push_str("    }\n");
    out.push_str("    return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&ok);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Ok"));
    out.push_str(" = 0};\n");
    out.push_str("}\n");
}

fn emit_fs_exists_helper(out: &mut String) {
    out.push_str("static int nomo_fs_exists(nomo_string path) {\n");
    out.push_str("#ifdef _WIN32\n");
    out.push_str("    DWORD attrs = GetFileAttributesA(path.data);\n");
    out.push_str("    return attrs != INVALID_FILE_ATTRIBUTES;\n");
    out.push_str("#else\n");
    out.push_str("    struct stat info;\n");
    out.push_str("    return stat(path.data, &info) == 0;\n");
    out.push_str("#endif\n");
    out.push_str("}\n");
}

fn emit_fs_metadata_helper(out: &mut String) {
    let fs_error = c_struct_ident("FsError", &[]);
    let metadata = c_struct_ident("FileMetadata", &[]);
    let result = c_enum_ident(
        "Result",
        &[
            ValueType::Struct("FileMetadata".to_string(), Vec::new()),
            ValueType::Struct("FsError".to_string(), Vec::new()),
        ],
    );
    let ok = c_enum_variant_ident(
        "Result",
        &[
            ValueType::Struct("FileMetadata".to_string(), Vec::new()),
            ValueType::Struct("FsError".to_string(), Vec::new()),
        ],
        "Ok",
    );
    let err = c_enum_variant_ident(
        "Result",
        &[
            ValueType::Struct("FileMetadata".to_string(), Vec::new()),
            ValueType::Struct("FsError".to_string(), Vec::new()),
        ],
        "Err",
    );
    out.push_str("static ");
    out.push_str(&result);
    out.push_str(" nomo_fs_metadata(nomo_string path) {\n");
    out.push_str("#ifdef _WIN32\n");
    out.push_str("    WIN32_FILE_ATTRIBUTE_DATA data;\n");
    out.push_str("    if (!GetFileAttributesExA(path.data, GetFileExInfoStandard, &data)) {\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&fs_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(\"failed to read metadata\")}};\n");
    out.push_str("    }\n");
    out.push_str("    int is_dir = (data.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY) != 0;\n");
    out.push_str("    ");
    out.push_str(&metadata);
    out.push_str(" metadata = {.");
    out.push_str(&c_member_ident("is_file"));
    out.push_str(" = !is_dir, .");
    out.push_str(&c_member_ident("is_dir"));
    out.push_str(" = is_dir, .");
    out.push_str(&c_member_ident("size"));
    out.push_str(" = ((uint64_t)data.nFileSizeHigh << 32) | (uint64_t)data.nFileSizeLow};\n");
    out.push_str("#else\n");
    out.push_str("    struct stat info;\n");
    out.push_str("    if (stat(path.data, &info) != 0) {\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&fs_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(strerror(errno))}};\n");
    out.push_str("    }\n");
    out.push_str("    ");
    out.push_str(&metadata);
    out.push_str(" metadata = {.");
    out.push_str(&c_member_ident("is_file"));
    out.push_str(" = S_ISREG(info.st_mode), .");
    out.push_str(&c_member_ident("is_dir"));
    out.push_str(" = S_ISDIR(info.st_mode), .");
    out.push_str(&c_member_ident("size"));
    out.push_str(" = (uint64_t)info.st_size};\n");
    out.push_str("#endif\n");
    out.push_str("    return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&ok);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Ok"));
    out.push_str(" = metadata};\n");
    out.push_str("}\n");
}

fn emit_fs_create_dir_helper(out: &mut String) {
    emit_fs_dir_result_helper(out, "create_dir", "mkdir");
}

fn emit_fs_remove_dir_helper(out: &mut String) {
    emit_fs_dir_result_helper(out, "remove_dir", "rmdir");
}

fn emit_fs_dir_result_helper(out: &mut String, function: &str, c_call: &str) {
    let fs_error = c_struct_ident("FsError", &[]);
    let result = c_enum_ident(
        "Result",
        &[
            ValueType::Void,
            ValueType::Struct("FsError".to_string(), Vec::new()),
        ],
    );
    let ok = c_enum_variant_ident(
        "Result",
        &[
            ValueType::Void,
            ValueType::Struct("FsError".to_string(), Vec::new()),
        ],
        "Ok",
    );
    let err = c_enum_variant_ident(
        "Result",
        &[
            ValueType::Void,
            ValueType::Struct("FsError".to_string(), Vec::new()),
        ],
        "Err",
    );
    out.push_str("static ");
    out.push_str(&result);
    out.push_str(" nomo_fs_");
    out.push_str(function);
    out.push_str("(nomo_string path) {\n");
    out.push_str("#ifdef _WIN32\n");
    out.push_str("    int status = ");
    if c_call == "mkdir" {
        out.push_str("_mkdir(path.data);\n");
    } else {
        out.push_str("_rmdir(path.data);\n");
    }
    out.push_str("#else\n");
    out.push_str("    int status = ");
    if c_call == "mkdir" {
        out.push_str("mkdir(path.data, 0777);\n");
    } else {
        out.push_str("rmdir(path.data);\n");
    }
    out.push_str("#endif\n");
    out.push_str("    if (status != 0) {\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&fs_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(strerror(errno))}};\n");
    out.push_str("    }\n");
    out.push_str("    return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&ok);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Ok"));
    out.push_str(" = 0};\n");
    out.push_str("}\n");
}

fn emit_fs_read_dir_helper(out: &mut String) {
    let fs_error = c_struct_ident("FsError", &[]);
    let entries_type = ValueType::Array(Box::new(ValueType::String));
    let result = c_enum_ident(
        "Result",
        &[
            entries_type.clone(),
            ValueType::Struct("FsError".to_string(), Vec::new()),
        ],
    );
    let ok = c_enum_variant_ident(
        "Result",
        &[
            entries_type.clone(),
            ValueType::Struct("FsError".to_string(), Vec::new()),
        ],
        "Ok",
    );
    let err = c_enum_variant_ident(
        "Result",
        &[
            entries_type,
            ValueType::Struct("FsError".to_string(), Vec::new()),
        ],
        "Err",
    );
    out.push_str("static ");
    out.push_str(&result);
    out.push_str(" nomo_fs_read_dir(nomo_string path) {\n");
    out.push_str("    nomo_array_string entries = nomo_array_string_new();\n");
    out.push_str("#ifdef _WIN32\n");
    out.push_str("    size_t path_len = strlen(path.data);\n");
    out.push_str("    int needs_sep = path_len > 0 && path.data[path_len - 1] != '/' && path.data[path_len - 1] != '\\\\';\n");
    out.push_str("    char *pattern = (char *)malloc(path_len + (needs_sep ? 3 : 2));\n");
    out.push_str("    if (pattern == NULL) { nomo_panic(\"out of memory\"); }\n");
    out.push_str("    memcpy(pattern, path.data, path_len);\n");
    out.push_str("    if (needs_sep) {\n");
    out.push_str("        pattern[path_len] = '\\\\';\n");
    out.push_str("        pattern[path_len + 1] = '*';\n");
    out.push_str("        pattern[path_len + 2] = '\\0';\n");
    out.push_str("    } else {\n");
    out.push_str("        pattern[path_len] = '*';\n");
    out.push_str("        pattern[path_len + 1] = '\\0';\n");
    out.push_str("    }\n");
    out.push_str("    WIN32_FIND_DATAA data;\n");
    out.push_str("    HANDLE handle = FindFirstFileA(pattern, &data);\n");
    out.push_str("    free(pattern);\n");
    out.push_str("    if (handle == INVALID_HANDLE_VALUE) {\n");
    out.push_str("        nomo_array_string_release(entries);\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&fs_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(\"failed to read directory\")}};\n");
    out.push_str("    }\n");
    out.push_str("    do {\n");
    out.push_str("        if (strcmp(data.cFileName, \".\") != 0 && strcmp(data.cFileName, \"..\") != 0) {\n");
    out.push_str("            nomo_string entry = nomo_string_from_cstr(data.cFileName);\n");
    out.push_str("            entries = nomo_array_string_push(entries, entry);\n");
    out.push_str("            nomo_string_release(entry);\n");
    out.push_str("        }\n");
    out.push_str("    } while (FindNextFileA(handle, &data));\n");
    out.push_str("    FindClose(handle);\n");
    out.push_str("#else\n");
    out.push_str("    DIR *dir = opendir(path.data);\n");
    out.push_str("    if (dir == NULL) {\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&fs_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(strerror(errno))}};\n");
    out.push_str("    }\n");
    out.push_str("    errno = 0;\n");
    out.push_str("    struct dirent *entry;\n");
    out.push_str("    while ((entry = readdir(dir)) != NULL) {\n");
    out.push_str("        if (strcmp(entry->d_name, \".\") == 0 || strcmp(entry->d_name, \"..\") == 0) { continue; }\n");
    out.push_str("        nomo_string name = nomo_string_from_cstr(entry->d_name);\n");
    out.push_str("        entries = nomo_array_string_push(entries, name);\n");
    out.push_str("        nomo_string_release(name);\n");
    out.push_str("    }\n");
    out.push_str("    if (errno != 0) {\n");
    out.push_str("        const char *message = strerror(errno);\n");
    out.push_str("        closedir(dir);\n");
    out.push_str("        nomo_array_string_release(entries);\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&fs_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(message)}};\n");
    out.push_str("    }\n");
    out.push_str("    if (closedir(dir) != 0) {\n");
    out.push_str("        nomo_array_string_release(entries);\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&fs_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(strerror(errno))}};\n");
    out.push_str("    }\n");
    out.push_str("#endif\n");
    out.push_str("    return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&ok);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Ok"));
    out.push_str(" = entries};\n");
    out.push_str("}\n");
}

fn emit_fs_open_helper(out: &mut String) {
    let file_type = c_struct_ident("File", &[]);
    let fs_error = c_struct_ident("FsError", &[]);
    let result = c_enum_ident(
        "Result",
        &[
            ValueType::Struct("File".to_string(), Vec::new()),
            ValueType::Struct("FsError".to_string(), Vec::new()),
        ],
    );
    let ok = c_enum_variant_ident(
        "Result",
        &[
            ValueType::Struct("File".to_string(), Vec::new()),
            ValueType::Struct("FsError".to_string(), Vec::new()),
        ],
        "Ok",
    );
    let err = c_enum_variant_ident(
        "Result",
        &[
            ValueType::Struct("File".to_string(), Vec::new()),
            ValueType::Struct("FsError".to_string(), Vec::new()),
        ],
        "Err",
    );
    out.push_str("static ");
    out.push_str(&result);
    out.push_str(" nomo_fs_open(nomo_string path) {\n");
    out.push_str("    FILE *file = fopen(path.data, \"rb+\");\n");
    out.push_str("    if (file == NULL) {\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&fs_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(strerror(errno))}};\n");
    out.push_str("    }\n");
    out.push_str("    return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&ok);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Ok"));
    out.push_str(" = (");
    out.push_str(&file_type);
    out.push_str("){.");
    out.push_str(&c_member_ident("handle"));
    out.push_str(" = file}};\n");
    out.push_str("}\n");
}

fn emit_file_read_to_string_helper(out: &mut String) {
    let file_type = c_struct_ident("File", &[]);
    let fs_error = c_struct_ident("FsError", &[]);
    let result = c_enum_ident(
        "Result",
        &[
            ValueType::String,
            ValueType::Struct("FsError".to_string(), Vec::new()),
        ],
    );
    let ok = c_enum_variant_ident(
        "Result",
        &[
            ValueType::String,
            ValueType::Struct("FsError".to_string(), Vec::new()),
        ],
        "Ok",
    );
    let err = c_enum_variant_ident(
        "Result",
        &[
            ValueType::String,
            ValueType::Struct("FsError".to_string(), Vec::new()),
        ],
        "Err",
    );
    out.push_str("static ");
    out.push_str(&result);
    out.push_str(" nomo_file_read_to_string(");
    out.push_str(&file_type);
    out.push_str(" file) {\n");
    out.push_str("    if (file.");
    out.push_str(&c_member_ident("handle"));
    out.push_str(" == NULL) {\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&fs_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(\"file is closed\")}};\n");
    out.push_str("    }\n");
    out.push_str("    FILE *handle = file.");
    out.push_str(&c_member_ident("handle"));
    out.push_str(";\n");
    out.push_str("    if (fseek(handle, 0, SEEK_END) != 0) {\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&fs_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(strerror(errno))}};\n");
    out.push_str("    }\n");
    out.push_str("    long size = ftell(handle);\n");
    out.push_str("    if (size < 0 || fseek(handle, 0, SEEK_SET) != 0) {\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&fs_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(strerror(errno))}};\n");
    out.push_str("    }\n");
    out.push_str("    char *buffer = (char *)malloc((size_t)size + 1);\n");
    out.push_str("    if (buffer == NULL) { nomo_panic(\"out of memory\"); }\n");
    out.push_str("    size_t read = fread(buffer, 1, (size_t)size, handle);\n");
    out.push_str("    if (read != (size_t)size) {\n");
    out.push_str(
        "        const char *message = ferror(handle) ? strerror(errno) : \"short read\";\n",
    );
    out.push_str("        free(buffer);\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&fs_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(message)}};\n");
    out.push_str("    }\n");
    out.push_str("    buffer[size] = '\\0';\n");
    out.push_str("    return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&ok);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Ok"));
    out.push_str(" = nomo_string_owned(buffer)};\n");
    out.push_str("}\n");
}

fn emit_file_write_string_helper(out: &mut String) {
    let file_type = c_struct_ident("File", &[]);
    let fs_error = c_struct_ident("FsError", &[]);
    let result = c_enum_ident(
        "Result",
        &[
            ValueType::Void,
            ValueType::Struct("FsError".to_string(), Vec::new()),
        ],
    );
    let ok = c_enum_variant_ident(
        "Result",
        &[
            ValueType::Void,
            ValueType::Struct("FsError".to_string(), Vec::new()),
        ],
        "Ok",
    );
    let err = c_enum_variant_ident(
        "Result",
        &[
            ValueType::Void,
            ValueType::Struct("FsError".to_string(), Vec::new()),
        ],
        "Err",
    );
    out.push_str("static ");
    out.push_str(&result);
    out.push_str(" nomo_file_write_string(");
    out.push_str(&file_type);
    out.push_str(" file, nomo_string content) {\n");
    out.push_str("    if (file.");
    out.push_str(&c_member_ident("handle"));
    out.push_str(" == NULL) {\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&fs_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(\"file is closed\")}};\n");
    out.push_str("    }\n");
    out.push_str("    FILE *handle = file.");
    out.push_str(&c_member_ident("handle"));
    out.push_str(";\n");
    out.push_str("    if (fseek(handle, 0, SEEK_SET) != 0) {\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&fs_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(strerror(errno))}};\n");
    out.push_str("    }\n");
    out.push_str("    size_t len = strlen(content.data);\n");
    out.push_str("    if (fwrite(content.data, 1, len, handle) != len) {\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&fs_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(strerror(errno))}};\n");
    out.push_str("    }\n");
    out.push_str("    if (fflush(handle) != 0) {\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&fs_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(strerror(errno))}};\n");
    out.push_str("    }\n");
    out.push_str("    return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&ok);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Ok"));
    out.push_str(" = 0};\n");
    out.push_str("}\n");
}

fn emit_file_close_helper(out: &mut String) {
    let file_type = c_struct_ident("File", &[]);
    out.push_str("static void nomo_file_close(");
    out.push_str(&file_type);
    out.push_str(" file) {\n");
    out.push_str("    if (file.");
    out.push_str(&c_member_ident("handle"));
    out.push_str(" != NULL) {\n");
    out.push_str("        fclose(file.");
    out.push_str(&c_member_ident("handle"));
    out.push_str(");\n");
    out.push_str("    }\n");
    out.push_str("}\n");
}

fn emit_net_common_helpers(out: &mut String) {
    out.push_str("static nomo_string nomo_net_error_message(void) {\n");
    out.push_str("#ifdef _WIN32\n");
    out.push_str("    char buffer[64];\n");
    out.push_str(
        "    snprintf(buffer, sizeof(buffer), \"network error %d\", WSAGetLastError());\n",
    );
    out.push_str("    return nomo_string_from_cstr(buffer);\n");
    out.push_str("#else\n");
    out.push_str("    return nomo_string_from_cstr(strerror(errno));\n");
    out.push_str("#endif\n");
    out.push_str("}\n");
    out.push_str("\nstatic int nomo_net_init(void) {\n");
    out.push_str("#ifdef _WIN32\n");
    out.push_str("    static int initialized = 0;\n");
    out.push_str("    if (!initialized) {\n");
    out.push_str("        WSADATA data;\n");
    out.push_str("        if (WSAStartup(MAKEWORD(2, 2), &data) != 0) { return 0; }\n");
    out.push_str("        initialized = 1;\n");
    out.push_str("    }\n");
    out.push_str("#endif\n");
    out.push_str("    return 1;\n");
    out.push_str("}\n");
}

fn emit_net_connect_helper(out: &mut String) {
    let tcp_stream = c_struct_ident("TcpStream", &[]);
    let net_error = c_struct_ident("NetError", &[]);
    let result = c_enum_ident(
        "Result",
        &[
            ValueType::Struct("TcpStream".to_string(), Vec::new()),
            ValueType::Struct("NetError".to_string(), Vec::new()),
        ],
    );
    let ok = c_enum_variant_ident(
        "Result",
        &[
            ValueType::Struct("TcpStream".to_string(), Vec::new()),
            ValueType::Struct("NetError".to_string(), Vec::new()),
        ],
        "Ok",
    );
    let err = c_enum_variant_ident(
        "Result",
        &[
            ValueType::Struct("TcpStream".to_string(), Vec::new()),
            ValueType::Struct("NetError".to_string(), Vec::new()),
        ],
        "Err",
    );
    out.push_str("static ");
    out.push_str(&result);
    out.push_str(" nomo_net_connect(nomo_string host, int64_t port) {\n");
    out.push_str("    if (!nomo_net_init()) {\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&net_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(\"network initialization failed\")}};\n");
    out.push_str("    }\n");
    out.push_str("    if (port < 0 || port > 65535) {\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&net_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(\"invalid port\")}};\n");
    out.push_str("    }\n");
    out.push_str("    char port_text[16];\n");
    out.push_str("    snprintf(port_text, sizeof(port_text), \"%\" PRId64, port);\n");
    out.push_str("    struct addrinfo hints;\n");
    out.push_str("    memset(&hints, 0, sizeof(hints));\n");
    out.push_str("    hints.ai_family = AF_UNSPEC;\n");
    out.push_str("    hints.ai_socktype = SOCK_STREAM;\n");
    out.push_str("    struct addrinfo *addresses = NULL;\n");
    out.push_str("    int rc = getaddrinfo(host.data, port_text, &hints, &addresses);\n");
    out.push_str("    if (rc != 0) {\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&net_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(gai_strerror(rc))}};\n");
    out.push_str("    }\n");
    out.push_str("    nomo_socket handle = NOMO_INVALID_SOCKET;\n");
    out.push_str("    for (struct addrinfo *address = addresses; address != NULL; address = address->ai_next) {\n");
    out.push_str("        handle = socket(address->ai_family, address->ai_socktype, address->ai_protocol);\n");
    out.push_str("        if (handle == NOMO_INVALID_SOCKET) { continue; }\n");
    out.push_str(
        "        if (connect(handle, address->ai_addr, address->ai_addrlen) == 0) { break; }\n",
    );
    out.push_str("        NOMO_SOCKET_CLOSE(handle);\n");
    out.push_str("        handle = NOMO_INVALID_SOCKET;\n");
    out.push_str("    }\n");
    out.push_str("    freeaddrinfo(addresses);\n");
    out.push_str("    if (handle == NOMO_INVALID_SOCKET) {\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&net_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_net_error_message()}};\n");
    out.push_str("    }\n");
    out.push_str("    return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&ok);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Ok"));
    out.push_str(" = (");
    out.push_str(&tcp_stream);
    out.push_str("){.");
    out.push_str(&c_member_ident("handle"));
    out.push_str(" = handle}};\n");
    out.push_str("}\n");
}

fn emit_net_listen_helper(out: &mut String) {
    let tcp_listener = c_struct_ident("TcpListener", &[]);
    let net_error = c_struct_ident("NetError", &[]);
    let result = c_enum_ident(
        "Result",
        &[
            ValueType::Struct("TcpListener".to_string(), Vec::new()),
            ValueType::Struct("NetError".to_string(), Vec::new()),
        ],
    );
    let ok = c_enum_variant_ident(
        "Result",
        &[
            ValueType::Struct("TcpListener".to_string(), Vec::new()),
            ValueType::Struct("NetError".to_string(), Vec::new()),
        ],
        "Ok",
    );
    let err = c_enum_variant_ident(
        "Result",
        &[
            ValueType::Struct("TcpListener".to_string(), Vec::new()),
            ValueType::Struct("NetError".to_string(), Vec::new()),
        ],
        "Err",
    );
    out.push_str("static ");
    out.push_str(&result);
    out.push_str(" nomo_net_listen(nomo_string host, int64_t port) {\n");
    out.push_str("    if (!nomo_net_init()) {\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&net_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(\"network initialization failed\")}};\n");
    out.push_str("    }\n");
    out.push_str("    if (port < 0 || port > 65535) {\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&net_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(\"invalid port\")}};\n");
    out.push_str("    }\n");
    out.push_str("    char port_text[16];\n");
    out.push_str("    snprintf(port_text, sizeof(port_text), \"%\" PRId64, port);\n");
    out.push_str("    struct addrinfo hints;\n");
    out.push_str("    memset(&hints, 0, sizeof(hints));\n");
    out.push_str("    hints.ai_family = AF_UNSPEC;\n");
    out.push_str("    hints.ai_socktype = SOCK_STREAM;\n");
    out.push_str("    hints.ai_flags = AI_PASSIVE;\n");
    out.push_str("    struct addrinfo *addresses = NULL;\n");
    out.push_str("    int rc = getaddrinfo(host.data, port_text, &hints, &addresses);\n");
    out.push_str("    if (rc != 0) {\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&net_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(gai_strerror(rc))}};\n");
    out.push_str("    }\n");
    out.push_str("    nomo_socket handle = NOMO_INVALID_SOCKET;\n");
    out.push_str("    for (struct addrinfo *address = addresses; address != NULL; address = address->ai_next) {\n");
    out.push_str("        handle = socket(address->ai_family, address->ai_socktype, address->ai_protocol);\n");
    out.push_str("        if (handle == NOMO_INVALID_SOCKET) { continue; }\n");
    out.push_str("        int yes = 1;\n");
    out.push_str("#ifdef _WIN32\n");
    out.push_str(
        "        setsockopt(handle, SOL_SOCKET, SO_REUSEADDR, (const char *)&yes, sizeof(yes));\n",
    );
    out.push_str("#else\n");
    out.push_str("        setsockopt(handle, SOL_SOCKET, SO_REUSEADDR, &yes, sizeof(yes));\n");
    out.push_str("#endif\n");
    out.push_str("        if (bind(handle, address->ai_addr, address->ai_addrlen) == 0 && listen(handle, 128) == 0) { break; }\n");
    out.push_str("        NOMO_SOCKET_CLOSE(handle);\n");
    out.push_str("        handle = NOMO_INVALID_SOCKET;\n");
    out.push_str("    }\n");
    out.push_str("    freeaddrinfo(addresses);\n");
    out.push_str("    if (handle == NOMO_INVALID_SOCKET) {\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&net_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_net_error_message()}};\n");
    out.push_str("    }\n");
    out.push_str("    return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&ok);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Ok"));
    out.push_str(" = (");
    out.push_str(&tcp_listener);
    out.push_str("){.");
    out.push_str(&c_member_ident("handle"));
    out.push_str(" = handle}};\n");
    out.push_str("}\n");
}

fn emit_tcp_listener_accept_helper(out: &mut String) {
    let tcp_listener = c_struct_ident("TcpListener", &[]);
    let tcp_stream = c_struct_ident("TcpStream", &[]);
    let net_error = c_struct_ident("NetError", &[]);
    let result = c_enum_ident(
        "Result",
        &[
            ValueType::Struct("TcpStream".to_string(), Vec::new()),
            ValueType::Struct("NetError".to_string(), Vec::new()),
        ],
    );
    let ok = c_enum_variant_ident(
        "Result",
        &[
            ValueType::Struct("TcpStream".to_string(), Vec::new()),
            ValueType::Struct("NetError".to_string(), Vec::new()),
        ],
        "Ok",
    );
    let err = c_enum_variant_ident(
        "Result",
        &[
            ValueType::Struct("TcpStream".to_string(), Vec::new()),
            ValueType::Struct("NetError".to_string(), Vec::new()),
        ],
        "Err",
    );
    out.push_str("static ");
    out.push_str(&result);
    out.push_str(" nomo_tcp_listener_accept(");
    out.push_str(&tcp_listener);
    out.push_str(" listener) {\n");
    out.push_str("    if (listener.");
    out.push_str(&c_member_ident("handle"));
    out.push_str(" == NOMO_INVALID_SOCKET) {\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&net_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(\"listener is closed\")}};\n");
    out.push_str("    }\n");
    out.push_str("    nomo_socket handle = accept(listener.");
    out.push_str(&c_member_ident("handle"));
    out.push_str(", NULL, NULL);\n");
    out.push_str("    if (handle == NOMO_INVALID_SOCKET) {\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&net_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_net_error_message()}};\n");
    out.push_str("    }\n");
    out.push_str("    return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&ok);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Ok"));
    out.push_str(" = (");
    out.push_str(&tcp_stream);
    out.push_str("){.");
    out.push_str(&c_member_ident("handle"));
    out.push_str(" = handle}};\n");
    out.push_str("}\n");
}

fn emit_tcp_listener_close_helper(out: &mut String) {
    let tcp_listener = c_struct_ident("TcpListener", &[]);
    out.push_str("static void nomo_tcp_listener_close(");
    out.push_str(&tcp_listener);
    out.push_str(" listener) {\n");
    out.push_str("    if (listener.");
    out.push_str(&c_member_ident("handle"));
    out.push_str(" != NOMO_INVALID_SOCKET) {\n");
    out.push_str("        NOMO_SOCKET_CLOSE(listener.");
    out.push_str(&c_member_ident("handle"));
    out.push_str(");\n");
    out.push_str("    }\n");
    out.push_str("}\n");
}

fn emit_net_udp_bind_helper(out: &mut String) {
    let udp_socket = c_struct_ident("UdpSocket", &[]);
    let net_error = c_struct_ident("NetError", &[]);
    let result = c_enum_ident(
        "Result",
        &[
            ValueType::Struct("UdpSocket".to_string(), Vec::new()),
            ValueType::Struct("NetError".to_string(), Vec::new()),
        ],
    );
    let ok = c_enum_variant_ident(
        "Result",
        &[
            ValueType::Struct("UdpSocket".to_string(), Vec::new()),
            ValueType::Struct("NetError".to_string(), Vec::new()),
        ],
        "Ok",
    );
    let err = c_enum_variant_ident(
        "Result",
        &[
            ValueType::Struct("UdpSocket".to_string(), Vec::new()),
            ValueType::Struct("NetError".to_string(), Vec::new()),
        ],
        "Err",
    );
    out.push_str("static ");
    out.push_str(&result);
    out.push_str(" nomo_net_udp_bind(nomo_string host, int64_t port) {\n");
    out.push_str("    if (!nomo_net_init()) {\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&net_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(\"network initialization failed\")}};\n");
    out.push_str("    }\n");
    out.push_str("    if (port < 0 || port > 65535) {\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&net_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(\"invalid port\")}};\n");
    out.push_str("    }\n");
    out.push_str("    char port_text[16];\n");
    out.push_str("    snprintf(port_text, sizeof(port_text), \"%\" PRId64, port);\n");
    out.push_str("    struct addrinfo hints;\n");
    out.push_str("    memset(&hints, 0, sizeof(hints));\n");
    out.push_str("    hints.ai_family = AF_UNSPEC;\n");
    out.push_str("    hints.ai_socktype = SOCK_DGRAM;\n");
    out.push_str("    hints.ai_flags = AI_PASSIVE;\n");
    out.push_str("    struct addrinfo *addresses = NULL;\n");
    out.push_str("    int rc = getaddrinfo(host.data, port_text, &hints, &addresses);\n");
    out.push_str("    if (rc != 0) {\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&net_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(gai_strerror(rc))}};\n");
    out.push_str("    }\n");
    out.push_str("    nomo_socket handle = NOMO_INVALID_SOCKET;\n");
    out.push_str("    for (struct addrinfo *address = addresses; address != NULL; address = address->ai_next) {\n");
    out.push_str("        handle = socket(address->ai_family, address->ai_socktype, address->ai_protocol);\n");
    out.push_str("        if (handle == NOMO_INVALID_SOCKET) { continue; }\n");
    out.push_str("        int yes = 1;\n");
    out.push_str("#ifdef _WIN32\n");
    out.push_str(
        "        setsockopt(handle, SOL_SOCKET, SO_REUSEADDR, (const char *)&yes, sizeof(yes));\n",
    );
    out.push_str("#else\n");
    out.push_str("        setsockopt(handle, SOL_SOCKET, SO_REUSEADDR, &yes, sizeof(yes));\n");
    out.push_str("#endif\n");
    out.push_str(
        "        if (bind(handle, address->ai_addr, address->ai_addrlen) == 0) { break; }\n",
    );
    out.push_str("        NOMO_SOCKET_CLOSE(handle);\n");
    out.push_str("        handle = NOMO_INVALID_SOCKET;\n");
    out.push_str("    }\n");
    out.push_str("    freeaddrinfo(addresses);\n");
    out.push_str("    if (handle == NOMO_INVALID_SOCKET) {\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&net_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_net_error_message()}};\n");
    out.push_str("    }\n");
    out.push_str("    return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&ok);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Ok"));
    out.push_str(" = (");
    out.push_str(&udp_socket);
    out.push_str("){.");
    out.push_str(&c_member_ident("handle"));
    out.push_str(" = handle}};\n");
    out.push_str("}\n");
}

fn emit_udp_socket_recv_from_string_helper(out: &mut String) {
    let udp_socket = c_struct_ident("UdpSocket", &[]);
    let udp_datagram = c_struct_ident("UdpDatagram", &[]);
    let net_error = c_struct_ident("NetError", &[]);
    let result = c_enum_ident(
        "Result",
        &[
            ValueType::Struct("UdpDatagram".to_string(), Vec::new()),
            ValueType::Struct("NetError".to_string(), Vec::new()),
        ],
    );
    let ok = c_enum_variant_ident(
        "Result",
        &[
            ValueType::Struct("UdpDatagram".to_string(), Vec::new()),
            ValueType::Struct("NetError".to_string(), Vec::new()),
        ],
        "Ok",
    );
    let err = c_enum_variant_ident(
        "Result",
        &[
            ValueType::Struct("UdpDatagram".to_string(), Vec::new()),
            ValueType::Struct("NetError".to_string(), Vec::new()),
        ],
        "Err",
    );
    out.push_str("static ");
    out.push_str(&result);
    out.push_str(" nomo_udp_socket_recv_from_string(");
    out.push_str(&udp_socket);
    out.push_str(" socket, int64_t max_bytes) {\n");
    out.push_str("    if (socket.");
    out.push_str(&c_member_ident("handle"));
    out.push_str(" == NOMO_INVALID_SOCKET) {\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&net_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(\"socket is closed\")}};\n");
    out.push_str("    }\n");
    out.push_str("    if (max_bytes < 0 || max_bytes > INT32_MAX) {\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&net_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(\"invalid max byte count\")}};\n");
    out.push_str("    }\n");
    out.push_str("    char *buffer = (char *)malloc((size_t)max_bytes + 1);\n");
    out.push_str("    if (buffer == NULL) { nomo_panic(\"out of memory\"); }\n");
    out.push_str("    struct sockaddr_storage address;\n");
    out.push_str("#ifdef _WIN32\n");
    out.push_str("    int address_len = sizeof(address);\n");
    out.push_str("#else\n");
    out.push_str("    socklen_t address_len = sizeof(address);\n");
    out.push_str("#endif\n");
    out.push_str("    int received = recvfrom(socket.");
    out.push_str(&c_member_ident("handle"));
    out.push_str(", buffer, (int)max_bytes, 0, (struct sockaddr *)&address, &address_len);\n");
    out.push_str("    if (received < 0) {\n");
    out.push_str("        nomo_string message = nomo_net_error_message();\n");
    out.push_str("        free(buffer);\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&net_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = message}};\n");
    out.push_str("    }\n");
    out.push_str("    buffer[received] = '\\0';\n");
    out.push_str("    char host[1025];\n");
    out.push_str("    char service[32];\n");
    out.push_str("    int rc = getnameinfo((struct sockaddr *)&address, address_len, host, sizeof(host), service, sizeof(service), NI_NUMERICHOST | NI_NUMERICSERV);\n");
    out.push_str("    if (rc != 0) {\n");
    out.push_str("        free(buffer);\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&net_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(gai_strerror(rc))}};\n");
    out.push_str("    }\n");
    out.push_str("    return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&ok);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Ok"));
    out.push_str(" = (");
    out.push_str(&udp_datagram);
    out.push_str("){.");
    out.push_str(&c_member_ident("data"));
    out.push_str(" = nomo_string_owned(buffer), .");
    out.push_str(&c_member_ident("host"));
    out.push_str(" = nomo_string_from_cstr(host), .");
    out.push_str(&c_member_ident("port"));
    out.push_str(" = (int64_t)strtoll(service, NULL, 10)}};\n");
    out.push_str("}\n");
}

fn emit_udp_socket_send_to_string_helper(out: &mut String) {
    let udp_socket = c_struct_ident("UdpSocket", &[]);
    let net_error = c_struct_ident("NetError", &[]);
    let result = c_enum_ident(
        "Result",
        &[
            ValueType::Void,
            ValueType::Struct("NetError".to_string(), Vec::new()),
        ],
    );
    let ok = c_enum_variant_ident(
        "Result",
        &[
            ValueType::Void,
            ValueType::Struct("NetError".to_string(), Vec::new()),
        ],
        "Ok",
    );
    let err = c_enum_variant_ident(
        "Result",
        &[
            ValueType::Void,
            ValueType::Struct("NetError".to_string(), Vec::new()),
        ],
        "Err",
    );
    out.push_str("static ");
    out.push_str(&result);
    out.push_str(" nomo_udp_socket_send_to_string(");
    out.push_str(&udp_socket);
    out.push_str(" socket, nomo_string content, nomo_string host, int64_t port) {\n");
    out.push_str("    if (socket.");
    out.push_str(&c_member_ident("handle"));
    out.push_str(" == NOMO_INVALID_SOCKET) {\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&net_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(\"socket is closed\")}};\n");
    out.push_str("    }\n");
    out.push_str("    if (port < 0 || port > 65535) {\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&net_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(\"invalid port\")}};\n");
    out.push_str("    }\n");
    out.push_str("    char port_text[16];\n");
    out.push_str("    snprintf(port_text, sizeof(port_text), \"%\" PRId64, port);\n");
    out.push_str("    struct addrinfo hints;\n");
    out.push_str("    memset(&hints, 0, sizeof(hints));\n");
    out.push_str("    hints.ai_family = AF_UNSPEC;\n");
    out.push_str("    hints.ai_socktype = SOCK_DGRAM;\n");
    out.push_str("    struct addrinfo *addresses = NULL;\n");
    out.push_str("    int rc = getaddrinfo(host.data, port_text, &hints, &addresses);\n");
    out.push_str("    if (rc != 0) {\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&net_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(gai_strerror(rc))}};\n");
    out.push_str("    }\n");
    out.push_str("    size_t len = strlen(content.data);\n");
    out.push_str("    int sent = -1;\n");
    out.push_str("    for (struct addrinfo *address = addresses; address != NULL; address = address->ai_next) {\n");
    out.push_str("        sent = sendto(socket.");
    out.push_str(&c_member_ident("handle"));
    out.push_str(", content.data, (int)len, 0, address->ai_addr, address->ai_addrlen);\n");
    out.push_str("        if (sent == (int)len) { break; }\n");
    out.push_str("    }\n");
    out.push_str("    freeaddrinfo(addresses);\n");
    out.push_str("    if (sent != (int)len) {\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&net_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_net_error_message()}};\n");
    out.push_str("    }\n");
    out.push_str("    return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&ok);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Ok"));
    out.push_str(" = 0};\n");
    out.push_str("}\n");
}

fn emit_udp_socket_close_helper(out: &mut String) {
    let udp_socket = c_struct_ident("UdpSocket", &[]);
    out.push_str("static void nomo_udp_socket_close(");
    out.push_str(&udp_socket);
    out.push_str(" socket) {\n");
    out.push_str("    if (socket.");
    out.push_str(&c_member_ident("handle"));
    out.push_str(" != NOMO_INVALID_SOCKET) {\n");
    out.push_str("        NOMO_SOCKET_CLOSE(socket.");
    out.push_str(&c_member_ident("handle"));
    out.push_str(");\n");
    out.push_str("    }\n");
    out.push_str("}\n");
}

fn emit_http_client_helpers(out: &mut String) {
    let http_response = c_struct_ident("HttpResponse", &[]);
    let http_error = c_struct_ident("HttpError", &[]);
    let result = c_enum_ident(
        "Result",
        &[
            ValueType::Struct("HttpResponse".to_string(), Vec::new()),
            ValueType::Struct("HttpError".to_string(), Vec::new()),
        ],
    );
    let ok = c_enum_variant_ident(
        "Result",
        &[
            ValueType::Struct("HttpResponse".to_string(), Vec::new()),
            ValueType::Struct("HttpError".to_string(), Vec::new()),
        ],
        "Ok",
    );
    let err = c_enum_variant_ident(
        "Result",
        &[
            ValueType::Struct("HttpResponse".to_string(), Vec::new()),
            ValueType::Struct("HttpError".to_string(), Vec::new()),
        ],
        "Err",
    );
    let err_payload = c_payload_ident("Err");
    let ok_payload = c_payload_ident("Ok");
    let status_member = c_member_ident("status");
    let body_member = c_member_ident("body");
    let message_member = c_member_ident("message");
    let get_name = c_fn_ident(BUILTIN_HTTP_GET_EXPR);
    let post_name = c_fn_ident(BUILTIN_HTTP_POST_EXPR);
    out.push_str("typedef struct nomo_http_url {\n");
    out.push_str("    char *host;\n");
    out.push_str("    char *port;\n");
    out.push_str("    char *path;\n");
    out.push_str("} nomo_http_url;\n\n");
    out.push_str("static char *nomo_http_copy_slice(const char *data, size_t len) {\n");
    out.push_str("    char *out = (char *)malloc(len + 1);\n");
    out.push_str("    if (out == NULL) { nomo_panic(\"out of memory\"); }\n");
    out.push_str("    memcpy(out, data, len);\n");
    out.push_str("    out[len] = '\\0';\n");
    out.push_str("    return out;\n");
    out.push_str("}\n\n");
    out.push_str("static void nomo_http_url_free(nomo_http_url url) {\n");
    out.push_str("    free(url.host);\n");
    out.push_str("    free(url.port);\n");
    out.push_str("    free(url.path);\n");
    out.push_str("}\n\n");
    out.push_str("static ");
    out.push_str(&result);
    out.push_str(" nomo_http_error_from_string(nomo_string message) {\n");
    out.push_str("    return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&err_payload);
    out.push_str(" = (");
    out.push_str(&http_error);
    out.push_str("){.");
    out.push_str(&message_member);
    out.push_str(" = message}};\n");
    out.push_str("}\n\n");
    out.push_str("static ");
    out.push_str(&result);
    out.push_str(" nomo_http_error_from_cstr(const char *message) {\n");
    out.push_str("    return nomo_http_error_from_string(nomo_string_from_cstr(message));\n");
    out.push_str("}\n\n");
    out.push_str("static int nomo_http_parse_url(nomo_string value, nomo_http_url *out) {\n");
    out.push_str("    const char *text = value.data;\n");
    out.push_str("    const char *prefix = \"http://\";\n");
    out.push_str("    size_t prefix_len = strlen(prefix);\n");
    out.push_str("    if (strncmp(text, prefix, prefix_len) != 0) { return 0; }\n");
    out.push_str("    const char *host_start = text + prefix_len;\n");
    out.push_str("    const char *cursor = host_start;\n");
    out.push_str(
        "    while (*cursor != '\\0' && *cursor != ':' && *cursor != '/') { cursor += 1; }\n",
    );
    out.push_str("    if (cursor == host_start) { return 0; }\n");
    out.push_str(
        "    out->host = nomo_http_copy_slice(host_start, (size_t)(cursor - host_start));\n",
    );
    out.push_str("    if (*cursor == ':') {\n");
    out.push_str("        const char *port_start = cursor + 1;\n");
    out.push_str("        cursor = port_start;\n");
    out.push_str("        while (*cursor >= '0' && *cursor <= '9') { cursor += 1; }\n");
    out.push_str("        if (cursor == port_start || (*cursor != '\\0' && *cursor != '/')) { free(out->host); out->host = NULL; return 0; }\n");
    out.push_str(
        "        out->port = nomo_http_copy_slice(port_start, (size_t)(cursor - port_start));\n",
    );
    out.push_str("    } else {\n");
    out.push_str("        out->port = nomo_http_copy_slice(\"80\", 2);\n");
    out.push_str("    }\n");
    out.push_str("    if (*cursor == '/') {\n");
    out.push_str("        out->path = nomo_http_copy_slice(cursor, strlen(cursor));\n");
    out.push_str("    } else {\n");
    out.push_str("        out->path = nomo_http_copy_slice(\"/\", 1);\n");
    out.push_str("    }\n");
    out.push_str("    return 1;\n");
    out.push_str("}\n\n");
    out.push_str("static ");
    out.push_str(&result);
    out.push_str(" nomo_http_request(const char *method, nomo_string url_value, nomo_string body, int has_body) {\n");
    out.push_str("    if (!nomo_net_init()) { return nomo_http_error_from_cstr(\"network initialization failed\"); }\n");
    out.push_str("    nomo_http_url url = {0};\n");
    out.push_str("    if (!nomo_http_parse_url(url_value, &url)) { return nomo_http_error_from_cstr(\"unsupported or invalid HTTP URL\"); }\n");
    out.push_str("    struct addrinfo hints;\n");
    out.push_str("    memset(&hints, 0, sizeof(hints));\n");
    out.push_str("    hints.ai_family = AF_UNSPEC;\n");
    out.push_str("    hints.ai_socktype = SOCK_STREAM;\n");
    out.push_str("    struct addrinfo *addresses = NULL;\n");
    out.push_str("    int rc = getaddrinfo(url.host, url.port, &hints, &addresses);\n");
    out.push_str("    if (rc != 0) { nomo_http_url_free(url); return nomo_http_error_from_cstr(gai_strerror(rc)); }\n");
    out.push_str("    nomo_socket handle = NOMO_INVALID_SOCKET;\n");
    out.push_str("    for (struct addrinfo *address = addresses; address != NULL; address = address->ai_next) {\n");
    out.push_str("        handle = socket(address->ai_family, address->ai_socktype, address->ai_protocol);\n");
    out.push_str("        if (handle == NOMO_INVALID_SOCKET) { continue; }\n");
    out.push_str(
        "        if (connect(handle, address->ai_addr, address->ai_addrlen) == 0) { break; }\n",
    );
    out.push_str("        NOMO_SOCKET_CLOSE(handle);\n");
    out.push_str("        handle = NOMO_INVALID_SOCKET;\n");
    out.push_str("    }\n");
    out.push_str("    freeaddrinfo(addresses);\n");
    out.push_str("    if (handle == NOMO_INVALID_SOCKET) { nomo_http_url_free(url); return nomo_http_error_from_string(nomo_net_error_message()); }\n");
    out.push_str("    size_t body_len = has_body ? strlen(body.data) : 0;\n");
    out.push_str("    int header_len = snprintf(NULL, 0, \"%s %s HTTP/1.0\\r\\nHost: %s\\r\\nConnection: close\\r\\nContent-Length: %zu\\r\\n\\r\\n\", method, url.path, url.host, body_len);\n");
    out.push_str("    if (header_len < 0) { NOMO_SOCKET_CLOSE(handle); nomo_http_url_free(url); return nomo_http_error_from_cstr(\"failed to build HTTP request\"); }\n");
    out.push_str("    char *request = (char *)malloc((size_t)header_len + body_len + 1);\n");
    out.push_str("    if (request == NULL) { nomo_panic(\"out of memory\"); }\n");
    out.push_str("    snprintf(request, (size_t)header_len + 1, \"%s %s HTTP/1.0\\r\\nHost: %s\\r\\nConnection: close\\r\\nContent-Length: %zu\\r\\n\\r\\n\", method, url.path, url.host, body_len);\n");
    out.push_str("    if (body_len > 0) { memcpy(request + header_len, body.data, body_len); }\n");
    out.push_str("    size_t request_len = (size_t)header_len + body_len;\n");
    out.push_str("    request[request_len] = '\\0';\n");
    out.push_str("    size_t sent_total = 0;\n");
    out.push_str("    while (sent_total < request_len) {\n");
    out.push_str("        int sent = send(handle, request + sent_total, (int)(request_len - sent_total), 0);\n");
    out.push_str("        if (sent <= 0) { nomo_string message = nomo_net_error_message(); free(request); NOMO_SOCKET_CLOSE(handle); nomo_http_url_free(url); return nomo_http_error_from_string(message); }\n");
    out.push_str("        sent_total += (size_t)sent;\n");
    out.push_str("    }\n");
    out.push_str("    free(request);\n");
    out.push_str("    size_t cap = 4096;\n");
    out.push_str("    size_t len = 0;\n");
    out.push_str("    char *response = (char *)malloc(cap + 1);\n");
    out.push_str("    if (response == NULL) { nomo_panic(\"out of memory\"); }\n");
    out.push_str("    for (;;) {\n");
    out.push_str("        if (len + 4096 + 1 > cap) { while (len + 4096 + 1 > cap) { cap *= 2; } response = (char *)realloc(response, cap + 1); if (response == NULL) { nomo_panic(\"out of memory\"); } }\n");
    out.push_str("        int received = recv(handle, response + len, 4096, 0);\n");
    out.push_str("        if (received < 0) { nomo_string message = nomo_net_error_message(); free(response); NOMO_SOCKET_CLOSE(handle); nomo_http_url_free(url); return nomo_http_error_from_string(message); }\n");
    out.push_str("        if (received == 0) { break; }\n");
    out.push_str("        len += (size_t)received;\n");
    out.push_str("    }\n");
    out.push_str("    NOMO_SOCKET_CLOSE(handle);\n");
    out.push_str("    nomo_http_url_free(url);\n");
    out.push_str("    response[len] = '\\0';\n");
    out.push_str("    char *status_space = strchr(response, ' ');\n");
    out.push_str("    if (status_space == NULL) { free(response); return nomo_http_error_from_cstr(\"invalid HTTP response status line\"); }\n");
    out.push_str("    long status = strtol(status_space + 1, NULL, 10);\n");
    out.push_str("    char *body_start = strstr(response, \"\\r\\n\\r\\n\");\n");
    out.push_str("    if (body_start == NULL) { free(response); return nomo_http_error_from_cstr(\"invalid HTTP response headers\"); }\n");
    out.push_str("    body_start += 4;\n");
    out.push_str("    size_t body_size = len - (size_t)(body_start - response);\n");
    out.push_str("    char *body_copy = nomo_http_copy_slice(body_start, body_size);\n");
    out.push_str("    free(response);\n");
    out.push_str("    return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&ok);
    out.push_str(", .payload.");
    out.push_str(&ok_payload);
    out.push_str(" = (");
    out.push_str(&http_response);
    out.push_str("){.");
    out.push_str(&status_member);
    out.push_str(" = (int64_t)status, .");
    out.push_str(&body_member);
    out.push_str(" = nomo_string_owned(body_copy)}};\n");
    out.push_str("}\n\n");
    out.push_str("static ");
    out.push_str(&result);
    out.push(' ');
    out.push_str(&get_name);
    out.push_str("(nomo_string url) {\n");
    out.push_str("    return nomo_http_request(\"GET\", url, nomo_string_literal(\"\"), 0);\n");
    out.push_str("}\n\n");
    out.push_str("static ");
    out.push_str(&result);
    out.push(' ');
    out.push_str(&post_name);
    out.push_str("(nomo_string url, nomo_string body) {\n");
    out.push_str("    return nomo_http_request(\"POST\", url, body, 1);\n");
    out.push_str("}\n");
}

fn emit_http_server_helpers(out: &mut String) {
    let http_server = c_struct_ident("HttpServer", &[]);
    let http_exchange = c_struct_ident("HttpExchange", &[]);
    let http_error = c_struct_ident("HttpError", &[]);
    let result_server = c_enum_ident(
        "Result",
        &[
            ValueType::Struct("HttpServer".to_string(), Vec::new()),
            ValueType::Struct("HttpError".to_string(), Vec::new()),
        ],
    );
    let result_exchange = c_enum_ident(
        "Result",
        &[
            ValueType::Struct("HttpExchange".to_string(), Vec::new()),
            ValueType::Struct("HttpError".to_string(), Vec::new()),
        ],
    );
    let result_void = c_enum_ident(
        "Result",
        &[
            ValueType::Void,
            ValueType::Struct("HttpError".to_string(), Vec::new()),
        ],
    );
    let server_ok = c_enum_variant_ident(
        "Result",
        &[
            ValueType::Struct("HttpServer".to_string(), Vec::new()),
            ValueType::Struct("HttpError".to_string(), Vec::new()),
        ],
        "Ok",
    );
    let server_err = c_enum_variant_ident(
        "Result",
        &[
            ValueType::Struct("HttpServer".to_string(), Vec::new()),
            ValueType::Struct("HttpError".to_string(), Vec::new()),
        ],
        "Err",
    );
    let exchange_ok = c_enum_variant_ident(
        "Result",
        &[
            ValueType::Struct("HttpExchange".to_string(), Vec::new()),
            ValueType::Struct("HttpError".to_string(), Vec::new()),
        ],
        "Ok",
    );
    let exchange_err = c_enum_variant_ident(
        "Result",
        &[
            ValueType::Struct("HttpExchange".to_string(), Vec::new()),
            ValueType::Struct("HttpError".to_string(), Vec::new()),
        ],
        "Err",
    );
    let void_ok = c_enum_variant_ident(
        "Result",
        &[
            ValueType::Void,
            ValueType::Struct("HttpError".to_string(), Vec::new()),
        ],
        "Ok",
    );
    let void_err = c_enum_variant_ident(
        "Result",
        &[
            ValueType::Void,
            ValueType::Struct("HttpError".to_string(), Vec::new()),
        ],
        "Err",
    );
    let listen_name = c_fn_ident(BUILTIN_HTTP_LISTEN_EXPR);
    let accept_name = c_fn_ident(BUILTIN_HTTP_ACCEPT_EXPR);
    let respond_name = c_fn_ident(BUILTIN_HTTP_RESPOND_STRING_EXPR);
    let close_server_name = c_fn_ident(BUILTIN_HTTP_CLOSE_SERVER_EXPR);
    let close_exchange_name = c_fn_ident(BUILTIN_HTTP_CLOSE_EXCHANGE_EXPR);
    let handle_member = c_member_ident("handle");
    let method_member = c_member_ident("method");
    let path_member = c_member_ident("path");
    let body_member = c_member_ident("body");
    let message_member = c_member_ident("message");
    let ok_payload = c_payload_ident("Ok");
    let err_payload = c_payload_ident("Err");

    out.push_str("static char *nomo_http_server_copy_slice(const char *data, size_t len) {\n");
    out.push_str("    char *out = (char *)malloc(len + 1);\n");
    out.push_str("    if (out == NULL) { nomo_panic(\"out of memory\"); }\n");
    out.push_str("    memcpy(out, data, len);\n");
    out.push_str("    out[len] = '\\0';\n");
    out.push_str("    return out;\n");
    out.push_str("}\n\n");
    out.push_str("static ");
    out.push_str(&result_server);
    out.push_str(" nomo_http_server_listen_error(nomo_string message) {\n");
    out.push_str("    return (");
    out.push_str(&result_server);
    out.push_str("){.tag = ");
    out.push_str(&server_err);
    out.push_str(", .payload.");
    out.push_str(&err_payload);
    out.push_str(" = (");
    out.push_str(&http_error);
    out.push_str("){.");
    out.push_str(&message_member);
    out.push_str(" = message}};\n");
    out.push_str("}\n\n");
    out.push_str("static ");
    out.push_str(&result_exchange);
    out.push_str(" nomo_http_server_accept_error(nomo_string message) {\n");
    out.push_str("    return (");
    out.push_str(&result_exchange);
    out.push_str("){.tag = ");
    out.push_str(&exchange_err);
    out.push_str(", .payload.");
    out.push_str(&err_payload);
    out.push_str(" = (");
    out.push_str(&http_error);
    out.push_str("){.");
    out.push_str(&message_member);
    out.push_str(" = message}};\n");
    out.push_str("}\n\n");
    out.push_str("static ");
    out.push_str(&result_void);
    out.push_str(" nomo_http_server_void_error(nomo_string message) {\n");
    out.push_str("    return (");
    out.push_str(&result_void);
    out.push_str("){.tag = ");
    out.push_str(&void_err);
    out.push_str(", .payload.");
    out.push_str(&err_payload);
    out.push_str(" = (");
    out.push_str(&http_error);
    out.push_str("){.");
    out.push_str(&message_member);
    out.push_str(" = message}};\n");
    out.push_str("}\n\n");

    out.push_str("static ");
    out.push_str(&result_server);
    out.push(' ');
    out.push_str(&listen_name);
    out.push_str("(nomo_string host, int64_t port) {\n");
    out.push_str("    if (!nomo_net_init()) { return nomo_http_server_listen_error(nomo_string_from_cstr(\"network initialization failed\")); }\n");
    out.push_str("    if (port < 0 || port > 65535) { return nomo_http_server_listen_error(nomo_string_from_cstr(\"invalid port\")); }\n");
    out.push_str("    char port_text[16];\n");
    out.push_str("    snprintf(port_text, sizeof(port_text), \"%\" PRId64, port);\n");
    out.push_str("    struct addrinfo hints;\n");
    out.push_str("    memset(&hints, 0, sizeof(hints));\n");
    out.push_str("    hints.ai_family = AF_UNSPEC;\n");
    out.push_str("    hints.ai_socktype = SOCK_STREAM;\n");
    out.push_str("    hints.ai_flags = AI_PASSIVE;\n");
    out.push_str("    struct addrinfo *addresses = NULL;\n");
    out.push_str("    int rc = getaddrinfo(host.data, port_text, &hints, &addresses);\n");
    out.push_str("    if (rc != 0) { return nomo_http_server_listen_error(nomo_string_from_cstr(gai_strerror(rc))); }\n");
    out.push_str("    nomo_socket handle = NOMO_INVALID_SOCKET;\n");
    out.push_str("    for (struct addrinfo *address = addresses; address != NULL; address = address->ai_next) {\n");
    out.push_str("        handle = socket(address->ai_family, address->ai_socktype, address->ai_protocol);\n");
    out.push_str("        if (handle == NOMO_INVALID_SOCKET) { continue; }\n");
    out.push_str("        int yes = 1;\n");
    out.push_str("#ifdef _WIN32\n");
    out.push_str(
        "        setsockopt(handle, SOL_SOCKET, SO_REUSEADDR, (const char *)&yes, sizeof(yes));\n",
    );
    out.push_str("#else\n");
    out.push_str("        setsockopt(handle, SOL_SOCKET, SO_REUSEADDR, &yes, sizeof(yes));\n");
    out.push_str("#endif\n");
    out.push_str("        if (bind(handle, address->ai_addr, address->ai_addrlen) == 0 && listen(handle, 16) == 0) { break; }\n");
    out.push_str("        NOMO_SOCKET_CLOSE(handle);\n");
    out.push_str("        handle = NOMO_INVALID_SOCKET;\n");
    out.push_str("    }\n");
    out.push_str("    freeaddrinfo(addresses);\n");
    out.push_str("    if (handle == NOMO_INVALID_SOCKET) { return nomo_http_server_listen_error(nomo_net_error_message()); }\n");
    out.push_str("    return (");
    out.push_str(&result_server);
    out.push_str("){.tag = ");
    out.push_str(&server_ok);
    out.push_str(", .payload.");
    out.push_str(&ok_payload);
    out.push_str(" = (");
    out.push_str(&http_server);
    out.push_str("){.");
    out.push_str(&handle_member);
    out.push_str(" = handle}};\n");
    out.push_str("}\n\n");

    out.push_str("static ");
    out.push_str(&result_exchange);
    out.push(' ');
    out.push_str(&accept_name);
    out.push('(');
    out.push_str(&http_server);
    out.push_str(" server) {\n");
    out.push_str("    if (server.");
    out.push_str(&handle_member);
    out.push_str(" == NOMO_INVALID_SOCKET) { return nomo_http_server_accept_error(nomo_string_from_cstr(\"server is closed\")); }\n");
    out.push_str("    nomo_socket client = accept(server.");
    out.push_str(&handle_member);
    out.push_str(", NULL, NULL);\n");
    out.push_str("    if (client == NOMO_INVALID_SOCKET) { return nomo_http_server_accept_error(nomo_net_error_message()); }\n");
    out.push_str("    size_t cap = 4096;\n");
    out.push_str("    size_t len = 0;\n");
    out.push_str("    char *request = (char *)malloc(cap + 1);\n");
    out.push_str("    if (request == NULL) { nomo_panic(\"out of memory\"); }\n");
    out.push_str("    size_t expected_len = 0;\n");
    out.push_str("    for (;;) {\n");
    out.push_str("        if (len + 1024 + 1 > cap) { while (len + 1024 + 1 > cap) { cap *= 2; } request = (char *)realloc(request, cap + 1); if (request == NULL) { nomo_panic(\"out of memory\"); } }\n");
    out.push_str("        int received = recv(client, request + len, 1024, 0);\n");
    out.push_str("        if (received < 0) { nomo_string message = nomo_net_error_message(); free(request); NOMO_SOCKET_CLOSE(client); return nomo_http_server_accept_error(message); }\n");
    out.push_str("        if (received == 0) { break; }\n");
    out.push_str("        len += (size_t)received;\n");
    out.push_str("        request[len] = '\\0';\n");
    out.push_str("        char *headers_end = strstr(request, \"\\r\\n\\r\\n\");\n");
    out.push_str("        if (headers_end != NULL) {\n");
    out.push_str("            if (expected_len == 0) {\n");
    out.push_str("                expected_len = (size_t)(headers_end - request) + 4;\n");
    out.push_str("                char *content_length = strstr(request, \"Content-Length: \");\n");
    out.push_str("                if (content_length != NULL && content_length < headers_end) { expected_len += (size_t)strtoull(content_length + 16, NULL, 10); }\n");
    out.push_str("            }\n");
    out.push_str("            if (len >= expected_len) { break; }\n");
    out.push_str("        }\n");
    out.push_str("    }\n");
    out.push_str("    request[len] = '\\0';\n");
    out.push_str("    char *method_end = strchr(request, ' ');\n");
    out.push_str("    if (method_end == NULL) { free(request); NOMO_SOCKET_CLOSE(client); return nomo_http_server_accept_error(nomo_string_from_cstr(\"invalid HTTP request line\")); }\n");
    out.push_str("    char *path_start = method_end + 1;\n");
    out.push_str("    char *path_end = strchr(path_start, ' ');\n");
    out.push_str("    if (path_end == NULL) { free(request); NOMO_SOCKET_CLOSE(client); return nomo_http_server_accept_error(nomo_string_from_cstr(\"invalid HTTP request path\")); }\n");
    out.push_str("    char *headers_end = strstr(request, \"\\r\\n\\r\\n\");\n");
    out.push_str("    if (headers_end == NULL) { free(request); NOMO_SOCKET_CLOSE(client); return nomo_http_server_accept_error(nomo_string_from_cstr(\"invalid HTTP request headers\")); }\n");
    out.push_str("    char *body_start = headers_end + 4;\n");
    out.push_str("    size_t body_len = len - (size_t)(body_start - request);\n");
    out.push_str("    char *method_copy = nomo_http_server_copy_slice(request, (size_t)(method_end - request));\n");
    out.push_str("    char *path_copy = nomo_http_server_copy_slice(path_start, (size_t)(path_end - path_start));\n");
    out.push_str("    char *body_copy = nomo_http_server_copy_slice(body_start, body_len);\n");
    out.push_str("    free(request);\n");
    out.push_str("    return (");
    out.push_str(&result_exchange);
    out.push_str("){.tag = ");
    out.push_str(&exchange_ok);
    out.push_str(", .payload.");
    out.push_str(&ok_payload);
    out.push_str(" = (");
    out.push_str(&http_exchange);
    out.push_str("){.");
    out.push_str(&handle_member);
    out.push_str(" = client, .");
    out.push_str(&method_member);
    out.push_str(" = nomo_string_owned(method_copy), .");
    out.push_str(&path_member);
    out.push_str(" = nomo_string_owned(path_copy), .");
    out.push_str(&body_member);
    out.push_str(" = nomo_string_owned(body_copy)}};\n");
    out.push_str("}\n\n");

    out.push_str("static ");
    out.push_str(&result_void);
    out.push(' ');
    out.push_str(&respond_name);
    out.push('(');
    out.push_str(&http_exchange);
    out.push_str(" exchange, int64_t status, nomo_string body) {\n");
    out.push_str("    if (exchange.");
    out.push_str(&handle_member);
    out.push_str(" == NOMO_INVALID_SOCKET) { return nomo_http_server_void_error(nomo_string_from_cstr(\"exchange is closed\")); }\n");
    out.push_str("    if (status < 100 || status > 999) { return nomo_http_server_void_error(nomo_string_from_cstr(\"invalid HTTP status\")); }\n");
    out.push_str("    size_t body_len = strlen(body.data);\n");
    out.push_str("    int header_len = snprintf(NULL, 0, \"HTTP/1.0 %\" PRId64 \" OK\\r\\nContent-Length: %zu\\r\\nConnection: close\\r\\n\\r\\n\", status, body_len);\n");
    out.push_str("    if (header_len < 0) { return nomo_http_server_void_error(nomo_string_from_cstr(\"failed to build HTTP response\")); }\n");
    out.push_str("    char *response = (char *)malloc((size_t)header_len + body_len + 1);\n");
    out.push_str("    if (response == NULL) { nomo_panic(\"out of memory\"); }\n");
    out.push_str("    snprintf(response, (size_t)header_len + 1, \"HTTP/1.0 %\" PRId64 \" OK\\r\\nContent-Length: %zu\\r\\nConnection: close\\r\\n\\r\\n\", status, body_len);\n");
    out.push_str("    if (body_len > 0) { memcpy(response + header_len, body.data, body_len); }\n");
    out.push_str("    size_t response_len = (size_t)header_len + body_len;\n");
    out.push_str("    size_t sent_total = 0;\n");
    out.push_str("    while (sent_total < response_len) {\n");
    out.push_str("        int sent = send(exchange.");
    out.push_str(&handle_member);
    out.push_str(", response + sent_total, (int)(response_len - sent_total), 0);\n");
    out.push_str("        if (sent <= 0) { nomo_string message = nomo_net_error_message(); free(response); NOMO_SOCKET_CLOSE(exchange.");
    out.push_str(&handle_member);
    out.push_str("); return nomo_http_server_void_error(message); }\n");
    out.push_str("        sent_total += (size_t)sent;\n");
    out.push_str("    }\n");
    out.push_str("    free(response);\n");
    out.push_str("    return (");
    out.push_str(&result_void);
    out.push_str("){.tag = ");
    out.push_str(&void_ok);
    out.push_str(", .payload.");
    out.push_str(&ok_payload);
    out.push_str(" = 0};\n");
    out.push_str("}\n\n");

    out.push_str("static void ");
    out.push_str(&close_server_name);
    out.push('(');
    out.push_str(&http_server);
    out.push_str(" server) {\n");
    out.push_str("    if (server.");
    out.push_str(&handle_member);
    out.push_str(" != NOMO_INVALID_SOCKET) { NOMO_SOCKET_CLOSE(server.");
    out.push_str(&handle_member);
    out.push_str("); }\n");
    out.push_str("}\n\n");
    out.push_str("static void ");
    out.push_str(&close_exchange_name);
    out.push('(');
    out.push_str(&http_exchange);
    out.push_str(" exchange) {\n");
    out.push_str("    if (exchange.");
    out.push_str(&handle_member);
    out.push_str(" != NOMO_INVALID_SOCKET) { NOMO_SOCKET_CLOSE(exchange.");
    out.push_str(&handle_member);
    out.push_str("); }\n");
    out.push_str("}\n");
}

fn emit_tcp_stream_read_to_string_helper(out: &mut String) {
    let tcp_stream = c_struct_ident("TcpStream", &[]);
    let net_error = c_struct_ident("NetError", &[]);
    let result = c_enum_ident(
        "Result",
        &[
            ValueType::String,
            ValueType::Struct("NetError".to_string(), Vec::new()),
        ],
    );
    let ok = c_enum_variant_ident(
        "Result",
        &[
            ValueType::String,
            ValueType::Struct("NetError".to_string(), Vec::new()),
        ],
        "Ok",
    );
    let err = c_enum_variant_ident(
        "Result",
        &[
            ValueType::String,
            ValueType::Struct("NetError".to_string(), Vec::new()),
        ],
        "Err",
    );
    out.push_str("static ");
    out.push_str(&result);
    out.push_str(" nomo_tcp_stream_read_to_string(");
    out.push_str(&tcp_stream);
    out.push_str(" stream) {\n");
    out.push_str("    if (stream.");
    out.push_str(&c_member_ident("handle"));
    out.push_str(" == NOMO_INVALID_SOCKET) {\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&net_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(\"stream is closed\")}};\n");
    out.push_str("    }\n");
    out.push_str("    size_t len = 0;\n");
    out.push_str("    size_t cap = 1;\n");
    out.push_str("    char *buffer = (char *)malloc(cap);\n");
    out.push_str("    if (buffer == NULL) { nomo_panic(\"out of memory\"); }\n");
    out.push_str("    char chunk[512];\n");
    out.push_str("    for (;;) {\n");
    out.push_str("        int received = recv(stream.");
    out.push_str(&c_member_ident("handle"));
    out.push_str(", chunk, sizeof(chunk), 0);\n");
    out.push_str("        if (received == 0) { break; }\n");
    out.push_str("        if (received < 0) {\n");
    out.push_str("            nomo_string message = nomo_net_error_message();\n");
    out.push_str("            free(buffer);\n");
    out.push_str("            return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&net_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = message}};\n");
    out.push_str("        }\n");
    out.push_str("        if (len + (size_t)received + 1 > cap) {\n");
    out.push_str("            while (len + (size_t)received + 1 > cap) { cap *= 2; }\n");
    out.push_str("            char *next = (char *)realloc(buffer, cap);\n");
    out.push_str(
        "            if (next == NULL) { free(buffer); nomo_panic(\"out of memory\"); }\n",
    );
    out.push_str("            buffer = next;\n");
    out.push_str("        }\n");
    out.push_str("        memcpy(buffer + len, chunk, (size_t)received);\n");
    out.push_str("        len += (size_t)received;\n");
    out.push_str("    }\n");
    out.push_str("    buffer[len] = '\\0';\n");
    out.push_str("    return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&ok);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Ok"));
    out.push_str(" = nomo_string_owned(buffer)};\n");
    out.push_str("}\n");
}

fn emit_tcp_stream_write_string_helper(out: &mut String) {
    let tcp_stream = c_struct_ident("TcpStream", &[]);
    let net_error = c_struct_ident("NetError", &[]);
    let result = c_enum_ident(
        "Result",
        &[
            ValueType::Void,
            ValueType::Struct("NetError".to_string(), Vec::new()),
        ],
    );
    let ok = c_enum_variant_ident(
        "Result",
        &[
            ValueType::Void,
            ValueType::Struct("NetError".to_string(), Vec::new()),
        ],
        "Ok",
    );
    let err = c_enum_variant_ident(
        "Result",
        &[
            ValueType::Void,
            ValueType::Struct("NetError".to_string(), Vec::new()),
        ],
        "Err",
    );
    out.push_str("static ");
    out.push_str(&result);
    out.push_str(" nomo_tcp_stream_write_string(");
    out.push_str(&tcp_stream);
    out.push_str(" stream, nomo_string content) {\n");
    out.push_str("    if (stream.");
    out.push_str(&c_member_ident("handle"));
    out.push_str(" == NOMO_INVALID_SOCKET) {\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&net_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(\"stream is closed\")}};\n");
    out.push_str("    }\n");
    out.push_str("    size_t len = strlen(content.data);\n");
    out.push_str("    size_t written = 0;\n");
    out.push_str("    while (written < len) {\n");
    out.push_str("        int sent = send(stream.");
    out.push_str(&c_member_ident("handle"));
    out.push_str(", content.data + written, (int)(len - written), 0);\n");
    out.push_str("        if (sent <= 0) {\n");
    out.push_str("            return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&net_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_net_error_message()}};\n");
    out.push_str("        }\n");
    out.push_str("        written += (size_t)sent;\n");
    out.push_str("    }\n");
    out.push_str("    return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&ok);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Ok"));
    out.push_str(" = 0};\n");
    out.push_str("}\n");
}

fn emit_tcp_stream_close_helper(out: &mut String) {
    let tcp_stream = c_struct_ident("TcpStream", &[]);
    out.push_str("static void nomo_tcp_stream_close(");
    out.push_str(&tcp_stream);
    out.push_str(" stream) {\n");
    out.push_str("    if (stream.");
    out.push_str(&c_member_ident("handle"));
    out.push_str(" != NOMO_INVALID_SOCKET) {\n");
    out.push_str("        NOMO_SOCKET_CLOSE(stream.");
    out.push_str(&c_member_ident("handle"));
    out.push_str(");\n");
    out.push_str("    }\n");
    out.push_str("}\n");
}

fn emit_env_get_helper(out: &mut String) {
    let result = c_enum_ident("Option", &[ValueType::String]);
    let some = c_enum_variant_ident("Option", &[ValueType::String], "Some");
    let none = c_enum_variant_ident("Option", &[ValueType::String], "None");
    out.push_str("static ");
    out.push_str(&result);
    out.push_str(" nomo_env_get(nomo_string name) {\n");
    out.push_str("    const char *value = getenv(name.data);\n");
    out.push_str("    if (value == NULL) {\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&none);
    out.push_str("};\n");
    out.push_str("    }\n");
    out.push_str("    return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&some);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Some"));
    out.push_str(" = nomo_string_from_cstr(value)};\n");
    out.push_str("}\n");
}

fn emit_env_set_helper(out: &mut String) {
    out.push_str("static void nomo_env_set(nomo_string name, nomo_string value) {\n");
    out.push_str("#ifdef _WIN32\n");
    out.push_str(
        "    if (_putenv_s(name.data, value.data) != 0) { nomo_panic(\"env.set failed\"); }\n",
    );
    out.push_str("#else\n");
    out.push_str(
        "    if (setenv(name.data, value.data, 1) != 0) { nomo_panic(\"env.set failed\"); }\n",
    );
    out.push_str("#endif\n");
    out.push_str("}\n");
}

fn emit_env_cwd_helper(out: &mut String) {
    out.push_str("static nomo_string nomo_env_cwd(void) {\n");
    out.push_str("    char buffer[PATH_MAX];\n");
    out.push_str("    if (NOMO_GETCWD(buffer, sizeof(buffer)) == NULL) { nomo_panic(\"env.cwd failed\"); }\n");
    out.push_str("    return nomo_string_from_cstr(buffer);\n");
    out.push_str("}\n");
}

fn emit_env_home_dir_helper(out: &mut String) {
    let result = c_enum_ident("Option", &[ValueType::String]);
    let some = c_enum_variant_ident("Option", &[ValueType::String], "Some");
    let none = c_enum_variant_ident("Option", &[ValueType::String], "None");
    out.push_str("static ");
    out.push_str(&result);
    out.push_str(" nomo_env_home_dir(void) {\n");
    out.push_str("#ifdef _WIN32\n");
    out.push_str("    const char *value = getenv(\"USERPROFILE\");\n");
    out.push_str("    if (value == NULL) { value = getenv(\"HOME\"); }\n");
    out.push_str("#else\n");
    out.push_str("    const char *value = getenv(\"HOME\");\n");
    out.push_str("#endif\n");
    out.push_str("    if (value == NULL) {\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&none);
    out.push_str("};\n");
    out.push_str("    }\n");
    out.push_str("    return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&some);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Some"));
    out.push_str(" = nomo_string_from_cstr(value)};\n");
    out.push_str("}\n");
}

fn emit_env_temp_dir_helper(out: &mut String) {
    out.push_str("static nomo_string nomo_env_temp_dir(void) {\n");
    out.push_str("    const char *value = getenv(\"TMPDIR\");\n");
    out.push_str("    if (value == NULL) { value = getenv(\"TEMP\"); }\n");
    out.push_str("    if (value == NULL) { value = getenv(\"TMP\"); }\n");
    out.push_str("    if (value == NULL) { value = \"/tmp\"; }\n");
    out.push_str("    return nomo_string_from_cstr(value);\n");
    out.push_str("}\n");
}

fn emit_process_common_helpers(out: &mut String) {
    out.push_str("static int32_t nomo_process_exit_code(int status) {\n");
    out.push_str("#ifdef _WIN32\n");
    out.push_str("    return (int32_t)status;\n");
    out.push_str("#else\n");
    out.push_str("    if (WIFEXITED(status)) { return (int32_t)WEXITSTATUS(status); }\n");
    out.push_str("    return (int32_t)status;\n");
    out.push_str("#endif\n");
    out.push_str("}\n\n");
    out.push_str("static nomo_string nomo_process_status_message(int32_t status) {\n");
    out.push_str("    char buffer[64];\n");
    out.push_str(
        "    snprintf(buffer, sizeof(buffer), \"process exited with status %\" PRId32, status);\n",
    );
    out.push_str("    return nomo_string_from_cstr(buffer);\n");
    out.push_str("}\n");
    out.push_str("\nstatic char *nomo_process_temp_path(const char *label) {\n");
    out.push_str("    static unsigned long counter = 0;\n");
    out.push_str("    const char *dir = getenv(\"TMPDIR\");\n");
    out.push_str("    if (dir == NULL) { dir = getenv(\"TEMP\"); }\n");
    out.push_str("    if (dir == NULL) { dir = getenv(\"TMP\"); }\n");
    out.push_str("#ifdef _WIN32\n");
    out.push_str("    if (dir == NULL) { dir = \".\"; }\n");
    out.push_str("    unsigned long pid = (unsigned long)GetCurrentProcessId();\n");
    out.push_str("    const char *sep = \"\\\\\";\n");
    out.push_str("#else\n");
    out.push_str("    if (dir == NULL) { dir = \"/tmp\"; }\n");
    out.push_str("    unsigned long pid = (unsigned long)getpid();\n");
    out.push_str("    const char *sep = \"/\";\n");
    out.push_str("#endif\n");
    out.push_str("    unsigned long id = counter++;\n");
    out.push_str("    size_t cap = strlen(dir) + strlen(label) + 96;\n");
    out.push_str("    char *path = (char *)malloc(cap);\n");
    out.push_str("    if (path == NULL) { nomo_panic(\"out of memory\"); }\n");
    out.push_str("    snprintf(path, cap, \"%s%s%s-%lu-%lu.tmp\", dir, sep, label, pid, id);\n");
    out.push_str("    return path;\n");
    out.push_str("}\n");
    out.push_str("\nstatic int nomo_process_read_file(const char *path, nomo_string *text, nomo_string *error_message) {\n");
    out.push_str("    FILE *file = fopen(path, \"rb\");\n");
    out.push_str("    if (file == NULL) { *error_message = nomo_string_from_cstr(strerror(errno)); return 0; }\n");
    out.push_str("    if (fseek(file, 0, SEEK_END) != 0) {\n");
    out.push_str("        const char *message = strerror(errno);\n");
    out.push_str("        fclose(file);\n");
    out.push_str("        *error_message = nomo_string_from_cstr(message);\n");
    out.push_str("        return 0;\n");
    out.push_str("    }\n");
    out.push_str("    long size = ftell(file);\n");
    out.push_str("    if (size < 0 || fseek(file, 0, SEEK_SET) != 0) {\n");
    out.push_str("        const char *message = strerror(errno);\n");
    out.push_str("        fclose(file);\n");
    out.push_str("        *error_message = nomo_string_from_cstr(message);\n");
    out.push_str("        return 0;\n");
    out.push_str("    }\n");
    out.push_str("    char *buffer = (char *)malloc((size_t)size + 1);\n");
    out.push_str("    if (buffer == NULL) { fclose(file); nomo_panic(\"out of memory\"); }\n");
    out.push_str("    size_t read = fread(buffer, 1, (size_t)size, file);\n");
    out.push_str("    if (read != (size_t)size) {\n");
    out.push_str(
        "        const char *message = ferror(file) ? strerror(errno) : \"short read\";\n",
    );
    out.push_str("        free(buffer);\n");
    out.push_str("        fclose(file);\n");
    out.push_str("        *error_message = nomo_string_from_cstr(message);\n");
    out.push_str("        return 0;\n");
    out.push_str("    }\n");
    out.push_str("    buffer[size] = '\\0';\n");
    out.push_str("    fclose(file);\n");
    out.push_str("    *text = nomo_string_owned(buffer);\n");
    out.push_str("    return 1;\n");
    out.push_str("}\n");
}

fn emit_process_spawn_helper(out: &mut String) {
    let process_error = c_struct_ident("ProcessError", &[]);
    let result = c_enum_ident(
        "Result",
        &[
            ValueType::I32,
            ValueType::Struct("ProcessError".to_string(), Vec::new()),
        ],
    );
    let ok = c_enum_variant_ident(
        "Result",
        &[
            ValueType::I32,
            ValueType::Struct("ProcessError".to_string(), Vec::new()),
        ],
        "Ok",
    );
    let err = c_enum_variant_ident(
        "Result",
        &[
            ValueType::I32,
            ValueType::Struct("ProcessError".to_string(), Vec::new()),
        ],
        "Err",
    );
    out.push_str("static ");
    out.push_str(&result);
    out.push_str(" nomo_process_spawn(nomo_string command) {\n");
    out.push_str("    int status = system(command.data);\n");
    out.push_str("    if (status == -1) {\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&process_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(strerror(errno))}};\n");
    out.push_str("    }\n");
    out.push_str("    return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&ok);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Ok"));
    out.push_str(" = nomo_process_exit_code(status)};\n");
    out.push_str("}\n");
}

fn emit_process_status_helper(out: &mut String) {
    let result = c_enum_ident(
        "Result",
        &[
            ValueType::I32,
            ValueType::Struct("ProcessError".to_string(), Vec::new()),
        ],
    );
    out.push_str("static ");
    out.push_str(&result);
    out.push_str(" nomo_process_status(nomo_string command) {\n");
    out.push_str("    return nomo_process_spawn(command);\n");
    out.push_str("}\n");
}

fn emit_process_exec_helper(out: &mut String) {
    let process_error = c_struct_ident("ProcessError", &[]);
    let result = c_enum_ident(
        "Result",
        &[
            ValueType::String,
            ValueType::Struct("ProcessError".to_string(), Vec::new()),
        ],
    );
    let ok = c_enum_variant_ident(
        "Result",
        &[
            ValueType::String,
            ValueType::Struct("ProcessError".to_string(), Vec::new()),
        ],
        "Ok",
    );
    let err = c_enum_variant_ident(
        "Result",
        &[
            ValueType::String,
            ValueType::Struct("ProcessError".to_string(), Vec::new()),
        ],
        "Err",
    );
    out.push_str("static ");
    out.push_str(&result);
    out.push_str(" nomo_process_exec(nomo_string command) {\n");
    out.push_str("    FILE *pipe = NOMO_POPEN(command.data, \"r\");\n");
    out.push_str("    if (pipe == NULL) {\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&process_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(strerror(errno))}};\n");
    out.push_str("    }\n");
    out.push_str("    size_t len = 0;\n");
    out.push_str("    size_t cap = 1;\n");
    out.push_str("    char *buffer = (char *)malloc(cap);\n");
    out.push_str("    if (buffer == NULL) { NOMO_PCLOSE(pipe); nomo_panic(\"out of memory\"); }\n");
    out.push_str("    char chunk[256];\n");
    out.push_str("    size_t read = 0;\n");
    out.push_str("    while ((read = fread(chunk, 1, sizeof(chunk), pipe)) > 0) {\n");
    out.push_str("        if (len + read + 1 > cap) {\n");
    out.push_str("            while (len + read + 1 > cap) { cap *= 2; }\n");
    out.push_str("            char *next = (char *)realloc(buffer, cap);\n");
    out.push_str("            if (next == NULL) { free(buffer); NOMO_PCLOSE(pipe); nomo_panic(\"out of memory\"); }\n");
    out.push_str("            buffer = next;\n");
    out.push_str("        }\n");
    out.push_str("        memcpy(buffer + len, chunk, read);\n");
    out.push_str("        len += read;\n");
    out.push_str("    }\n");
    out.push_str("    if (ferror(pipe)) {\n");
    out.push_str("        const char *message = strerror(errno);\n");
    out.push_str("        free(buffer);\n");
    out.push_str("        NOMO_PCLOSE(pipe);\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&process_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(message)}};\n");
    out.push_str("    }\n");
    out.push_str("    int status = NOMO_PCLOSE(pipe);\n");
    out.push_str("    if (status == -1) {\n");
    out.push_str("        const char *message = strerror(errno);\n");
    out.push_str("        free(buffer);\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&process_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(message)}};\n");
    out.push_str("    }\n");
    out.push_str("    int32_t code = nomo_process_exit_code(status);\n");
    out.push_str("    if (code != 0) {\n");
    out.push_str("        nomo_string message = nomo_process_status_message(code);\n");
    out.push_str("        free(buffer);\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&process_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = message}};\n");
    out.push_str("    }\n");
    out.push_str("    buffer[len] = '\\0';\n");
    out.push_str("    return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&ok);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Ok"));
    out.push_str(" = nomo_string_owned(buffer)};\n");
    out.push_str("}\n");
}

fn emit_process_output_helper(out: &mut String) {
    let process_error = c_struct_ident("ProcessError", &[]);
    let process_output = c_struct_ident("ProcessOutput", &[]);
    let result = c_enum_ident(
        "Result",
        &[
            ValueType::Struct("ProcessOutput".to_string(), Vec::new()),
            ValueType::Struct("ProcessError".to_string(), Vec::new()),
        ],
    );
    let ok = c_enum_variant_ident(
        "Result",
        &[
            ValueType::Struct("ProcessOutput".to_string(), Vec::new()),
            ValueType::Struct("ProcessError".to_string(), Vec::new()),
        ],
        "Ok",
    );
    let err = c_enum_variant_ident(
        "Result",
        &[
            ValueType::Struct("ProcessOutput".to_string(), Vec::new()),
            ValueType::Struct("ProcessError".to_string(), Vec::new()),
        ],
        "Err",
    );
    out.push_str("static ");
    out.push_str(&result);
    out.push_str(" nomo_process_output(nomo_string command) {\n");
    out.push_str("    char *stdout_path = nomo_process_temp_path(\"nomo-stdout\");\n");
    out.push_str("    char *stderr_path = nomo_process_temp_path(\"nomo-stderr\");\n");
    out.push_str("    size_t command_len = strlen(command.data) + strlen(stdout_path) + strlen(stderr_path) + 40;\n");
    out.push_str("    char *wrapped = (char *)malloc(command_len);\n");
    out.push_str("    if (wrapped == NULL) { free(stdout_path); free(stderr_path); nomo_panic(\"out of memory\"); }\n");
    out.push_str("    snprintf(wrapped, command_len, \"(%s) > \\\"%s\\\" 2> \\\"%s\\\"\", command.data, stdout_path, stderr_path);\n");
    out.push_str("    int status = system(wrapped);\n");
    out.push_str("    free(wrapped);\n");
    out.push_str("    if (status == -1) {\n");
    out.push_str("        nomo_string message = nomo_string_from_cstr(strerror(errno));\n");
    out.push_str("        remove(stdout_path); remove(stderr_path);\n");
    out.push_str("        free(stdout_path); free(stderr_path);\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&process_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = message}};\n");
    out.push_str("    }\n");
    out.push_str("    nomo_string stdout_text;\n");
    out.push_str("    nomo_string stderr_text;\n");
    out.push_str("    nomo_string message;\n");
    out.push_str("    if (!nomo_process_read_file(stdout_path, &stdout_text, &message)) {\n");
    out.push_str("        remove(stdout_path); remove(stderr_path);\n");
    out.push_str("        free(stdout_path); free(stderr_path);\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&process_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = message}};\n");
    out.push_str("    }\n");
    out.push_str("    if (!nomo_process_read_file(stderr_path, &stderr_text, &message)) {\n");
    out.push_str("        nomo_string_release(stdout_text);\n");
    out.push_str("        remove(stdout_path); remove(stderr_path);\n");
    out.push_str("        free(stdout_path); free(stderr_path);\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&process_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = message}};\n");
    out.push_str("    }\n");
    out.push_str("    remove(stdout_path); remove(stderr_path);\n");
    out.push_str("    free(stdout_path); free(stderr_path);\n");
    out.push_str("    ");
    out.push_str(&process_output);
    out.push_str(" output = {.");
    out.push_str(&c_member_ident("status"));
    out.push_str(" = nomo_process_exit_code(status), .");
    out.push_str(&c_member_ident("stdout"));
    out.push_str(" = stdout_text, .");
    out.push_str(&c_member_ident("stderr"));
    out.push_str(" = stderr_text};\n");
    out.push_str("    return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&ok);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Ok"));
    out.push_str(" = output};\n");
    out.push_str("}\n");
}

fn emit_env_args_helper(out: &mut String) {
    out.push_str("static nomo_array_string nomo_env_args(int argc, char **argv) {\n");
    out.push_str("    nomo_array_string args = nomo_array_string_new();\n");
    out.push_str("    for (int i = 0; i < argc; i += 1) {\n");
    out.push_str("        nomo_string arg = nomo_string_from_cstr(argv[i]);\n");
    out.push_str("        args = nomo_array_string_push(args, arg);\n");
    out.push_str("        nomo_string_release(arg);\n");
    out.push_str("    }\n");
    out.push_str("    return args;\n");
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

fn uses_fs_read_to_string(program: &Program) -> bool {
    program.functions.iter().any(|function| {
        function
            .body
            .iter()
            .any(|statement| statement_uses_fs_read_to_string(statement))
    })
}

fn uses_fs_write_string(program: &Program) -> bool {
    program.functions.iter().any(|function| {
        function
            .body
            .iter()
            .any(|statement| statement_uses_fs_write_string(statement))
    })
}

fn uses_fs_read_bytes(program: &Program) -> bool {
    program.functions.iter().any(|function| {
        function
            .body
            .iter()
            .any(|statement| statement_contains_expr(statement, expr_is_fs_read_bytes))
    })
}

fn uses_fs_write_bytes(program: &Program) -> bool {
    program.functions.iter().any(|function| {
        function
            .body
            .iter()
            .any(|statement| statement_contains_expr(statement, expr_is_fs_write_bytes))
    })
}

fn uses_fs_exists(program: &Program) -> bool {
    program.functions.iter().any(|function| {
        function
            .body
            .iter()
            .any(|statement| statement_contains_expr(statement, expr_is_fs_exists))
    })
}

fn uses_fs_metadata(program: &Program) -> bool {
    program.functions.iter().any(|function| {
        function
            .body
            .iter()
            .any(|statement| statement_contains_expr(statement, expr_is_fs_metadata))
    })
}

fn uses_fs_create_dir(program: &Program) -> bool {
    program.functions.iter().any(|function| {
        function
            .body
            .iter()
            .any(|statement| statement_contains_expr(statement, expr_is_fs_create_dir))
    })
}

fn uses_fs_remove_dir(program: &Program) -> bool {
    program.functions.iter().any(|function| {
        function
            .body
            .iter()
            .any(|statement| statement_contains_expr(statement, expr_is_fs_remove_dir))
    })
}

fn uses_fs_read_dir(program: &Program) -> bool {
    program.functions.iter().any(|function| {
        function
            .body
            .iter()
            .any(|statement| statement_contains_expr(statement, expr_is_fs_read_dir))
    })
}

fn uses_fs_open(program: &Program) -> bool {
    program.functions.iter().any(|function| {
        function
            .body
            .iter()
            .any(|statement| statement_uses_fs_open(statement))
    })
}

fn uses_file_read_to_string(program: &Program) -> bool {
    program.functions.iter().any(|function| {
        function
            .body
            .iter()
            .any(|statement| statement_contains_expr(statement, expr_is_file_read_to_string))
    })
}

fn uses_file_write_string(program: &Program) -> bool {
    program.functions.iter().any(|function| {
        function
            .body
            .iter()
            .any(|statement| statement_contains_expr(statement, expr_is_file_write_string))
    })
}

fn uses_file_close(program: &Program) -> bool {
    program.functions.iter().any(|function| {
        function
            .body
            .iter()
            .any(|statement| statement_contains_expr(statement, expr_is_file_close))
    })
}

fn uses_net_connect(program: &Program) -> bool {
    program.functions.iter().any(|function| {
        function
            .body
            .iter()
            .any(|statement| statement_contains_expr(statement, expr_is_net_connect))
    })
}

fn uses_net_listen(program: &Program) -> bool {
    program.functions.iter().any(|function| {
        function
            .body
            .iter()
            .any(|statement| statement_contains_expr(statement, expr_is_net_listen))
    })
}

fn uses_net_udp_bind(program: &Program) -> bool {
    program.functions.iter().any(|function| {
        function
            .body
            .iter()
            .any(|statement| statement_contains_expr(statement, expr_is_net_udp_bind))
    })
}

fn uses_http_client(program: &Program) -> bool {
    program.functions.iter().any(|function| {
        function
            .body
            .iter()
            .any(|statement| statement_contains_expr(statement, expr_is_http_client_call))
    })
}

fn uses_http_server(program: &Program) -> bool {
    program.functions.iter().any(|function| {
        function
            .body
            .iter()
            .any(|statement| statement_contains_expr(statement, expr_is_http_server_call))
    })
}

fn uses_tcp_listener_accept(program: &Program) -> bool {
    program.functions.iter().any(|function| {
        function
            .body
            .iter()
            .any(|statement| statement_contains_expr(statement, expr_is_tcp_listener_accept))
    })
}

fn uses_tcp_listener_close(program: &Program) -> bool {
    program.functions.iter().any(|function| {
        function
            .body
            .iter()
            .any(|statement| statement_contains_expr(statement, expr_is_tcp_listener_close))
    })
}

fn uses_tcp_stream_read_to_string(program: &Program) -> bool {
    program.functions.iter().any(|function| {
        function
            .body
            .iter()
            .any(|statement| statement_contains_expr(statement, expr_is_tcp_stream_read_to_string))
    })
}

fn uses_tcp_stream_write_string(program: &Program) -> bool {
    program.functions.iter().any(|function| {
        function
            .body
            .iter()
            .any(|statement| statement_contains_expr(statement, expr_is_tcp_stream_write_string))
    })
}

fn uses_tcp_stream_close(program: &Program) -> bool {
    program.functions.iter().any(|function| {
        function
            .body
            .iter()
            .any(|statement| statement_contains_expr(statement, expr_is_tcp_stream_close))
    })
}

fn uses_udp_socket_recv_from_string(program: &Program) -> bool {
    program.functions.iter().any(|function| {
        function.body.iter().any(|statement| {
            statement_contains_expr(statement, expr_is_udp_socket_recv_from_string)
        })
    })
}

fn uses_udp_socket_send_to_string(program: &Program) -> bool {
    program.functions.iter().any(|function| {
        function
            .body
            .iter()
            .any(|statement| statement_contains_expr(statement, expr_is_udp_socket_send_to_string))
    })
}

fn uses_udp_socket_close(program: &Program) -> bool {
    program.functions.iter().any(|function| {
        function
            .body
            .iter()
            .any(|statement| statement_contains_expr(statement, expr_is_udp_socket_close))
    })
}

fn uses_io_read_line(program: &Program) -> bool {
    program
        .functions
        .iter()
        .any(|function| function.body.iter().any(statement_uses_io_read_line))
}

fn uses_log_enabled(program: &Program) -> bool {
    program.functions.iter().any(|function| {
        function
            .body
            .iter()
            .any(|statement| statement_contains_expr(statement, expr_is_log_enabled))
    })
}

fn uses_hash_builtin(program: &Program) -> bool {
    program.functions.iter().any(|function| {
        function
            .body
            .iter()
            .any(|statement| statement_contains_expr(statement, expr_is_hash_builtin))
    })
}

fn uses_crypto_builtin(program: &Program) -> bool {
    program.functions.iter().any(|function| {
        function
            .body
            .iter()
            .any(|statement| statement_contains_expr(statement, expr_is_crypto_builtin))
    })
}

fn uses_json_builtin(program: &Program) -> bool {
    program.functions.iter().any(|function| {
        function
            .body
            .iter()
            .any(|statement| statement_contains_expr(statement, expr_is_json_builtin))
    })
}

fn uses_regex_builtin(program: &Program) -> bool {
    program.functions.iter().any(|function| {
        function
            .body
            .iter()
            .any(|statement| statement_contains_expr(statement, expr_is_regex_builtin))
    })
}

fn uses_collections_builtin(program: &Program) -> bool {
    program.functions.iter().any(|function| {
        function
            .body
            .iter()
            .any(|statement| statement_contains_expr(statement, expr_is_collections_builtin))
    })
}

fn uses_num_parse_i64(program: &Program) -> bool {
    program.functions.iter().any(|function| {
        function
            .body
            .iter()
            .any(|statement| statement_contains_expr(statement, expr_is_num_parse_i64))
    })
}

fn uses_num_parse_u64(program: &Program) -> bool {
    program.functions.iter().any(|function| {
        function
            .body
            .iter()
            .any(|statement| statement_contains_expr(statement, expr_is_num_parse_u64))
    })
}

fn uses_num_parse_f64(program: &Program) -> bool {
    program.functions.iter().any(|function| {
        function
            .body
            .iter()
            .any(|statement| statement_contains_expr(statement, expr_is_num_parse_f64))
    })
}

fn uses_env_get(program: &Program) -> bool {
    program.functions.iter().any(|function| {
        function
            .body
            .iter()
            .any(|statement| statement_uses_env_get(statement))
    })
}

fn uses_env_args(program: &Program) -> bool {
    program.functions.iter().any(|function| {
        function
            .body
            .iter()
            .any(|statement| statement_uses_env_args(statement))
    })
}

fn uses_env_set(program: &Program) -> bool {
    program
        .functions
        .iter()
        .any(|function| function.body.iter().any(statement_uses_env_set))
}

fn uses_env_cwd(program: &Program) -> bool {
    program
        .functions
        .iter()
        .any(|function| function.body.iter().any(statement_uses_env_cwd))
}

fn uses_env_home_dir(program: &Program) -> bool {
    program
        .functions
        .iter()
        .any(|function| function.body.iter().any(statement_uses_env_home_dir))
}

fn uses_env_temp_dir(program: &Program) -> bool {
    program
        .functions
        .iter()
        .any(|function| function.body.iter().any(statement_uses_env_temp_dir))
}

fn uses_process_status(program: &Program) -> bool {
    program.functions.iter().any(|function| {
        function
            .body
            .iter()
            .any(|statement| statement_contains_expr(statement, expr_is_process_status))
    })
}

fn uses_process_spawn(program: &Program) -> bool {
    program.functions.iter().any(|function| {
        function
            .body
            .iter()
            .any(|statement| statement_contains_expr(statement, expr_is_process_spawn))
    })
}

fn uses_process_exec(program: &Program) -> bool {
    program.functions.iter().any(|function| {
        function
            .body
            .iter()
            .any(|statement| statement_contains_expr(statement, expr_is_process_exec))
    })
}

fn uses_process_output(program: &Program) -> bool {
    program.functions.iter().any(|function| {
        function
            .body
            .iter()
            .any(|statement| statement_contains_expr(statement, expr_is_process_output))
    })
}

fn collect_array_element_types(program: &Program) -> Vec<ValueType> {
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for function in &program.functions {
        collect_type_array_elements(&function.return_type, &mut seen, &mut out);
        for param in &function.params {
            collect_type_array_elements(&param.value_type, &mut seen, &mut out);
        }
        for statement in &function.body {
            collect_statement_array_elements(statement, &mut seen, &mut out);
        }
    }
    out
}

fn collect_type_array_elements(
    value_type: &ValueType,
    seen: &mut BTreeSet<String>,
    out: &mut Vec<ValueType>,
) {
    match value_type {
        ValueType::Array(element) => {
            push_array_element_type(seen, out, element);
            collect_type_array_elements(element, seen, out);
        }
        ValueType::Enum(_, args) => {
            for arg in args {
                collect_type_array_elements(arg, seen, out);
            }
        }
        _ => {}
    }
}

fn push_array_element_type(
    seen: &mut BTreeSet<String>,
    out: &mut Vec<ValueType>,
    element_type: &ValueType,
) {
    if is_supported_array_element(element_type) {
        let key = c_type_name_part(element_type);
        if seen.insert(key) {
            out.push(element_type.clone());
        }
    }
}

fn statement_uses_fs_read_to_string(statement: &Statement) -> bool {
    match statement {
        Statement::Let { initializer, .. } => expr_uses_fs_read_to_string(initializer),
        Statement::LetIf {
            condition,
            body,
            else_body,
            ..
        } => {
            expr_uses_fs_read_to_string(condition)
                || body.iter().any(statement_uses_fs_read_to_string)
                || else_body.iter().any(statement_uses_fs_read_to_string)
        }
        Statement::LetMatch { value, arms, .. } => {
            expr_uses_fs_read_to_string(value)
                || arms
                    .iter()
                    .any(|arm| arm.body.iter().any(statement_uses_fs_read_to_string))
        }
        Statement::QuestionLet { result_expr, .. } => expr_uses_fs_read_to_string(result_expr),
        Statement::QuestionReturn { result_expr, .. } => expr_uses_fs_read_to_string(result_expr),
        Statement::LetElse {
            value, else_body, ..
        } => {
            expr_uses_fs_read_to_string(value)
                || else_body.iter().any(statement_uses_fs_read_to_string)
        }
        Statement::IfLet {
            value,
            body,
            else_body,
            ..
        } => {
            expr_uses_fs_read_to_string(value)
                || body.iter().any(statement_uses_fs_read_to_string)
                || else_body
                    .as_ref()
                    .is_some_and(|else_body| else_body.iter().any(statement_uses_fs_read_to_string))
        }
        Statement::If {
            condition,
            body,
            else_body,
        } => {
            expr_uses_fs_read_to_string(condition)
                || body.iter().any(statement_uses_fs_read_to_string)
                || else_body.iter().any(statement_uses_fs_read_to_string)
        }
        Statement::Assign { value, .. }
        | Statement::AssignField { value, .. }
        | Statement::Println(value)
        | Statement::Print(value)
        | Statement::Eprintln(value)
        | Statement::Eprint(value)
        | Statement::Panic(value)
        | Statement::Expr(value)
        | Statement::Return(Some(value)) => expr_uses_fs_read_to_string(value),
        Statement::Loop { kind, body } => match kind {
            LoopKind::Infinite => body.iter().any(statement_uses_fs_read_to_string),
            LoopKind::While(condition) => {
                expr_uses_fs_read_to_string(condition)
                    || body.iter().any(statement_uses_fs_read_to_string)
            }
            LoopKind::Iterate { iterable, .. } => {
                expr_uses_fs_read_to_string(iterable)
                    || body.iter().any(statement_uses_fs_read_to_string)
            }
        },
        Statement::Match { value, arms, .. } => {
            expr_uses_fs_read_to_string(value)
                || arms
                    .iter()
                    .any(|arm| arm.body.iter().any(statement_uses_fs_read_to_string))
        }
        Statement::Defer { call } => deferred_uses_fs_read_to_string(call),
        Statement::Break | Statement::Continue => false,
        Statement::Return(None) => false,
    }
}

fn statement_uses_fs_write_string(statement: &Statement) -> bool {
    match statement {
        Statement::Let { initializer, .. } => expr_uses_fs_write_string(initializer),
        Statement::LetIf {
            condition,
            body,
            else_body,
            ..
        } => {
            expr_uses_fs_write_string(condition)
                || body.iter().any(statement_uses_fs_write_string)
                || else_body.iter().any(statement_uses_fs_write_string)
        }
        Statement::LetMatch { value, arms, .. } => {
            expr_uses_fs_write_string(value)
                || arms
                    .iter()
                    .any(|arm| arm.body.iter().any(statement_uses_fs_write_string))
        }
        Statement::QuestionLet { result_expr, .. } => expr_uses_fs_write_string(result_expr),
        Statement::QuestionReturn { result_expr, .. } => expr_uses_fs_write_string(result_expr),
        Statement::LetElse {
            value, else_body, ..
        } => {
            expr_uses_fs_write_string(value) || else_body.iter().any(statement_uses_fs_write_string)
        }
        Statement::IfLet {
            value,
            body,
            else_body,
            ..
        } => {
            expr_uses_fs_write_string(value)
                || body.iter().any(statement_uses_fs_write_string)
                || else_body
                    .as_ref()
                    .is_some_and(|else_body| else_body.iter().any(statement_uses_fs_write_string))
        }
        Statement::If {
            condition,
            body,
            else_body,
        } => {
            expr_uses_fs_write_string(condition)
                || body.iter().any(statement_uses_fs_write_string)
                || else_body.iter().any(statement_uses_fs_write_string)
        }
        Statement::Assign { value, .. }
        | Statement::AssignField { value, .. }
        | Statement::Println(value)
        | Statement::Print(value)
        | Statement::Eprintln(value)
        | Statement::Eprint(value)
        | Statement::Panic(value)
        | Statement::Expr(value)
        | Statement::Return(Some(value)) => expr_uses_fs_write_string(value),
        Statement::Loop { kind, body } => match kind {
            LoopKind::Infinite => body.iter().any(statement_uses_fs_write_string),
            LoopKind::While(condition) => {
                expr_uses_fs_write_string(condition)
                    || body.iter().any(statement_uses_fs_write_string)
            }
            LoopKind::Iterate { iterable, .. } => {
                expr_uses_fs_write_string(iterable)
                    || body.iter().any(statement_uses_fs_write_string)
            }
        },
        Statement::Match { value, arms, .. } => {
            expr_uses_fs_write_string(value)
                || arms
                    .iter()
                    .any(|arm| arm.body.iter().any(statement_uses_fs_write_string))
        }
        Statement::Defer { call } => deferred_uses_fs_write_string(call),
        Statement::Break | Statement::Continue => false,
        Statement::Return(None) => false,
    }
}

fn statement_uses_fs_open(statement: &Statement) -> bool {
    match statement {
        Statement::Let { initializer, .. } => expr_uses_fs_open(initializer),
        Statement::LetIf {
            condition,
            body,
            else_body,
            ..
        } => {
            expr_uses_fs_open(condition)
                || body.iter().any(statement_uses_fs_open)
                || else_body.iter().any(statement_uses_fs_open)
        }
        Statement::LetMatch { value, arms, .. } => {
            expr_uses_fs_open(value)
                || arms
                    .iter()
                    .any(|arm| arm.body.iter().any(statement_uses_fs_open))
        }
        Statement::QuestionLet { result_expr, .. } => expr_uses_fs_open(result_expr),
        Statement::QuestionReturn { result_expr, .. } => expr_uses_fs_open(result_expr),
        Statement::LetElse {
            value, else_body, ..
        } => expr_uses_fs_open(value) || else_body.iter().any(statement_uses_fs_open),
        Statement::IfLet {
            value,
            body,
            else_body,
            ..
        } => {
            expr_uses_fs_open(value)
                || body.iter().any(statement_uses_fs_open)
                || else_body
                    .as_ref()
                    .is_some_and(|else_body| else_body.iter().any(statement_uses_fs_open))
        }
        Statement::If {
            condition,
            body,
            else_body,
        } => {
            expr_uses_fs_open(condition)
                || body.iter().any(statement_uses_fs_open)
                || else_body.iter().any(statement_uses_fs_open)
        }
        Statement::Assign { value, .. }
        | Statement::AssignField { value, .. }
        | Statement::Println(value)
        | Statement::Print(value)
        | Statement::Eprintln(value)
        | Statement::Eprint(value)
        | Statement::Panic(value)
        | Statement::Expr(value)
        | Statement::Return(Some(value)) => expr_uses_fs_open(value),
        Statement::Loop { kind, body } => match kind {
            LoopKind::Infinite => body.iter().any(statement_uses_fs_open),
            LoopKind::While(condition) => {
                expr_uses_fs_open(condition) || body.iter().any(statement_uses_fs_open)
            }
            LoopKind::Iterate { iterable, .. } => {
                expr_uses_fs_open(iterable) || body.iter().any(statement_uses_fs_open)
            }
        },
        Statement::Match { value, arms, .. } => {
            expr_uses_fs_open(value)
                || arms
                    .iter()
                    .any(|arm| arm.body.iter().any(statement_uses_fs_open))
        }
        Statement::Defer { call } => deferred_uses_fs_open(call),
        Statement::Break | Statement::Continue => false,
        Statement::Return(None) => false,
    }
}

fn statement_uses_env_set(statement: &Statement) -> bool {
    statement_contains_expr(statement, expr_is_env_set)
}

fn statement_uses_env_cwd(statement: &Statement) -> bool {
    statement_contains_expr(statement, expr_is_env_cwd)
}

fn statement_uses_env_home_dir(statement: &Statement) -> bool {
    statement_contains_expr(statement, expr_is_env_home_dir)
}

fn statement_uses_env_temp_dir(statement: &Statement) -> bool {
    statement_contains_expr(statement, expr_is_env_temp_dir)
}

fn statement_uses_io_read_line(statement: &Statement) -> bool {
    statement_contains_expr(statement, expr_is_io_read_line)
}

fn statement_contains_expr(statement: &Statement, predicate: fn(&ValueExpr) -> bool) -> bool {
    match statement {
        Statement::Let { initializer, .. } => expr_contains(initializer, predicate),
        Statement::LetIf {
            condition,
            body,
            else_body,
            ..
        } => {
            expr_contains(condition, predicate)
                || body
                    .iter()
                    .any(|statement| statement_contains_expr(statement, predicate))
                || else_body
                    .iter()
                    .any(|statement| statement_contains_expr(statement, predicate))
        }
        Statement::LetMatch { value, arms, .. } => {
            expr_contains(value, predicate)
                || arms.iter().any(|arm| {
                    arm.body
                        .iter()
                        .any(|statement| statement_contains_expr(statement, predicate))
                })
        }
        Statement::QuestionLet { result_expr, .. }
        | Statement::QuestionReturn { result_expr, .. } => expr_contains(result_expr, predicate),
        Statement::LetElse {
            value, else_body, ..
        } => {
            expr_contains(value, predicate)
                || else_body
                    .iter()
                    .any(|statement| statement_contains_expr(statement, predicate))
        }
        Statement::IfLet {
            value,
            body,
            else_body,
            ..
        } => {
            expr_contains(value, predicate)
                || body
                    .iter()
                    .any(|statement| statement_contains_expr(statement, predicate))
                || else_body.as_ref().is_some_and(|else_body| {
                    else_body
                        .iter()
                        .any(|statement| statement_contains_expr(statement, predicate))
                })
        }
        Statement::If {
            condition,
            body,
            else_body,
        } => {
            expr_contains(condition, predicate)
                || body
                    .iter()
                    .any(|statement| statement_contains_expr(statement, predicate))
                || else_body
                    .iter()
                    .any(|statement| statement_contains_expr(statement, predicate))
        }
        Statement::Assign { value, .. }
        | Statement::AssignField { value, .. }
        | Statement::Println(value)
        | Statement::Print(value)
        | Statement::Eprintln(value)
        | Statement::Eprint(value)
        | Statement::Panic(value)
        | Statement::Expr(value)
        | Statement::Return(Some(value)) => expr_contains(value, predicate),
        Statement::Loop { kind, body } => match kind {
            LoopKind::Infinite => body
                .iter()
                .any(|statement| statement_contains_expr(statement, predicate)),
            LoopKind::While(condition) => {
                expr_contains(condition, predicate)
                    || body
                        .iter()
                        .any(|statement| statement_contains_expr(statement, predicate))
            }
            LoopKind::Iterate { iterable, .. } => {
                expr_contains(iterable, predicate)
                    || body
                        .iter()
                        .any(|statement| statement_contains_expr(statement, predicate))
            }
        },
        Statement::Match { value, arms, .. } => {
            expr_contains(value, predicate)
                || arms.iter().any(|arm| {
                    arm.body
                        .iter()
                        .any(|statement| statement_contains_expr(statement, predicate))
                })
        }
        Statement::Defer { call } => deferred_contains_expr(call, predicate),
        Statement::Break | Statement::Continue | Statement::Return(None) => false,
    }
}

fn deferred_contains_expr(call: &DeferredCall, predicate: fn(&ValueExpr) -> bool) -> bool {
    match call {
        DeferredCall::Expr(expr)
        | DeferredCall::Println(expr)
        | DeferredCall::Print(expr)
        | DeferredCall::Eprintln(expr)
        | DeferredCall::Eprint(expr) => expr_contains(expr, predicate),
    }
}

fn expr_contains(expr: &ValueExpr, predicate: fn(&ValueExpr) -> bool) -> bool {
    if predicate(expr) {
        return true;
    }
    match expr {
        ValueExpr::Binary { left, right, .. }
        | ValueExpr::StringCompare { left, right, .. }
        | ValueExpr::StringConcat { left, right }
        | ValueExpr::StringContains {
            value: left,
            needle: right,
        }
        | ValueExpr::StringStartsWith {
            value: left,
            prefix: right,
        }
        | ValueExpr::StringEndsWith {
            value: left,
            suffix: right,
        }
        | ValueExpr::StringSplit {
            value: left,
            separator: right,
        }
        | ValueExpr::PathJoin { left, right }
        | ValueExpr::NumBinary { left, right, .. }
        | ValueExpr::MathBinary { left, right, .. }
        | ValueExpr::CollectionsStringMapGet {
            map: left,
            key: right,
        }
        | ValueExpr::CollectionsStringMapContains {
            map: left,
            key: right,
        }
        | ValueExpr::CollectionsStringMapRemove {
            map: left,
            key: right,
        }
        | ValueExpr::CollectionsStringSetContains {
            set: left,
            value: right,
        }
        | ValueExpr::CollectionsStringSetInsert {
            set: left,
            value: right,
        }
        | ValueExpr::CollectionsStringSetRemove {
            set: left,
            value: right,
        }
        | ValueExpr::RegexIsMatch {
            regex: left,
            value: right,
        }
        | ValueExpr::RegexCaptures {
            regex: left,
            value: right,
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
        } => expr_contains(left, predicate) || expr_contains(right, predicate),
        ValueExpr::UdpSocketSendToString {
            socket,
            content,
            host,
            port,
        } => {
            expr_contains(socket, predicate)
                || expr_contains(content, predicate)
                || expr_contains(host, predicate)
                || expr_contains(port, predicate)
        }
        ValueExpr::FsWriteString { path, content } => {
            expr_contains(path, predicate) || expr_contains(content, predicate)
        }
        ValueExpr::FsWriteBytes { path, bytes } => {
            expr_contains(path, predicate) || expr_contains(bytes, predicate)
        }
        ValueExpr::EnvSet { name, value } => {
            expr_contains(name, predicate) || expr_contains(value, predicate)
        }
        ValueExpr::HashWriteString { state, value } => {
            expr_contains(state, predicate) || expr_contains(value, predicate)
        }
        ValueExpr::HashWriteBytes { state, value } => {
            expr_contains(state, predicate) || expr_contains(value, predicate)
        }
        ValueExpr::CollectionsStringMapSet { map, key, value } => {
            expr_contains(map, predicate)
                || expr_contains(key, predicate)
                || expr_contains(value, predicate)
        }
        ValueExpr::Call { args, .. } => args.iter().any(|arg| expr_contains(arg, predicate)),
        ValueExpr::ArrayGet { array, index, .. } => {
            expr_contains(array, predicate) || expr_contains(index, predicate)
        }
        ValueExpr::ArrayPop { .. } | ValueExpr::ArrayClear { .. } => false,
        ValueExpr::ArrayRemove { index, .. } => expr_contains(index, predicate),
        ValueExpr::ArraySet { index, value, .. } => {
            expr_contains(index, predicate) || expr_contains(value, predicate)
        }
        ValueExpr::ArrayPush { value, .. } => expr_contains(value, predicate),
        ValueExpr::ArrayInsert { index, value, .. } => {
            expr_contains(index, predicate) || expr_contains(value, predicate)
        }
        ValueExpr::FsReadToString { path }
        | ValueExpr::FsReadBytes { path }
        | ValueExpr::FsOpen { path }
        | ValueExpr::FsExists { path }
        | ValueExpr::FsMetadata { path }
        | ValueExpr::FsCreateDir { path }
        | ValueExpr::FsRemoveDir { path }
        | ValueExpr::FsReadDir { path }
        | ValueExpr::FileClose { file: path }
        | ValueExpr::FileReadToString { file: path }
        | ValueExpr::TcpListenerAccept { listener: path }
        | ValueExpr::TcpListenerClose { listener: path }
        | ValueExpr::TcpStreamClose { stream: path }
        | ValueExpr::TcpStreamReadToString { stream: path }
        | ValueExpr::UdpSocketClose { socket: path }
        | ValueExpr::EnvGet { name: path }
        | ValueExpr::StringLen { value: path }
        | ValueExpr::StringIsEmpty { value: path }
        | ValueExpr::StringTrim { value: path }
        | ValueExpr::StringToLower { value: path }
        | ValueExpr::StringToUpper { value: path }
        | ValueExpr::CharIsDigit { value: path }
        | ValueExpr::CharIsAlpha { value: path }
        | ValueExpr::CharIsWhitespace { value: path }
        | ValueExpr::CharToString { value: path }
        | ValueExpr::PathBasename { path }
        | ValueExpr::PathDirname { path }
        | ValueExpr::PathExtension { path }
        | ValueExpr::PathNormalize { path }
        | ValueExpr::PathIsAbsolute { path }
        | ValueExpr::MathUnary { value: path, .. }
        | ValueExpr::TimeDurationMillis { millis: path }
        | ValueExpr::TimeDurationSeconds { seconds: path }
        | ValueExpr::TimeDurationAsMillis { duration: path }
        | ValueExpr::TimeFormatDuration { duration: path }
        | ValueExpr::TimeSleep { duration: path }
        | ValueExpr::TimeSleepMillis { duration: path }
        | ValueExpr::LogEnabled { level: path }
        | ValueExpr::HashString { value: path }
        | ValueExpr::HashBytes { value: path }
        | ValueExpr::HashFinish { state: path }
        | ValueExpr::CryptoSha256 { value: path }
        | ValueExpr::CryptoSha512 { value: path }
        | ValueExpr::CryptoRandomBytes { count: path }
        | ValueExpr::JsonParse { value: path }
        | ValueExpr::JsonStringify { value: path }
        | ValueExpr::RegexCompile { pattern: path }
        | ValueExpr::CollectionsStringMapLen { map: path }
        | ValueExpr::CollectionsStringSetLen { set: path }
        | ValueExpr::ProcessExit { code: path }
        | ValueExpr::ProcessSpawn { command: path }
        | ValueExpr::ProcessStatus { command: path }
        | ValueExpr::ProcessExec { command: path }
        | ValueExpr::ProcessOutput { command: path }
        | ValueExpr::NumParseI64 { value: path }
        | ValueExpr::NumParseU64 { value: path }
        | ValueExpr::NumParseF64 { value: path }
        | ValueExpr::NumToString { value: path, .. }
        | ValueExpr::Unary { expr: path, .. }
        | ValueExpr::Cast { expr: path, .. }
        | ValueExpr::ResultIsOk { result: path, .. }
        | ValueExpr::ResultIsErr { result: path, .. }
        | ValueExpr::ResultMap { result: path, .. }
        | ValueExpr::ResultAndThen { result: path, .. }
        | ValueExpr::OptionIsSome { option: path, .. }
        | ValueExpr::OptionIsNone { option: path, .. }
        | ValueExpr::OptionMap { option: path, .. }
        | ValueExpr::OptionAndThen { option: path, .. }
        | ValueExpr::EnumPayload { value: path, .. }
        | ValueExpr::EnumPayloadFieldAccess { value: path, .. }
        | ValueExpr::ArrayIter { array: path, .. }
        | ValueExpr::ArrayLen { array: path } => expr_contains(path, predicate),
        ValueExpr::ResultUnwrapOr {
            result, default, ..
        } => expr_contains(result, predicate) || expr_contains(default, predicate),
        ValueExpr::OptionUnwrapOr {
            option, default, ..
        } => expr_contains(option, predicate) || expr_contains(default, predicate),
        ValueExpr::FileWriteString { file, content } => {
            expr_contains(file, predicate) || expr_contains(content, predicate)
        }
        ValueExpr::StructLiteral { fields, .. } => fields
            .iter()
            .any(|(_, value)| expr_contains(value, predicate)),
        ValueExpr::EnumVariant { payload, .. } => payload
            .as_ref()
            .is_some_and(|payload| expr_contains(payload, predicate)),
        ValueExpr::If {
            condition,
            then_branch,
            else_branch,
        } => {
            expr_contains(condition, predicate)
                || expr_contains(then_branch, predicate)
                || expr_contains(else_branch, predicate)
        }
        ValueExpr::Panic { message, .. } => expr_contains(message, predicate),
        ValueExpr::Match { value, arms } => {
            expr_contains(value, predicate)
                || arms.iter().any(|arm| expr_contains(&arm.value, predicate))
        }
        ValueExpr::ResultMapErr { result, .. } => expr_contains(result, predicate),
        ValueExpr::StringLiteral(_)
        | ValueExpr::IntLiteral(_)
        | ValueExpr::FloatLiteral(_)
        | ValueExpr::CharLiteral(_)
        | ValueExpr::BoolLiteral(_)
        | ValueExpr::VoidLiteral
        | ValueExpr::HashNew
        | ValueExpr::CollectionsStringMapNew
        | ValueExpr::CollectionsStringSetNew
        | ValueExpr::Variable(_)
        | ValueExpr::MutBorrow(_)
        | ValueExpr::EnvArgs
        | ValueExpr::IoReadLine
        | ValueExpr::EnvCwd
        | ValueExpr::EnvHomeDir
        | ValueExpr::EnvTempDir
        | ValueExpr::OsPlatform
        | ValueExpr::OsArch
        | ValueExpr::OsPathSeparator
        | ValueExpr::OsLineEnding
        | ValueExpr::TimeNowMillis
        | ValueExpr::TimeMonotonicMillis
        | ValueExpr::ArrayNew { .. }
        | ValueExpr::FieldAccess { .. } => false,
    }
}

fn expr_is_env_set(expr: &ValueExpr) -> bool {
    matches!(expr, ValueExpr::EnvSet { .. })
}

fn expr_is_process_status(expr: &ValueExpr) -> bool {
    matches!(expr, ValueExpr::ProcessStatus { .. })
}

fn expr_is_process_spawn(expr: &ValueExpr) -> bool {
    matches!(expr, ValueExpr::ProcessSpawn { .. })
}

fn expr_is_process_exec(expr: &ValueExpr) -> bool {
    matches!(expr, ValueExpr::ProcessExec { .. })
}

fn expr_is_process_output(expr: &ValueExpr) -> bool {
    matches!(expr, ValueExpr::ProcessOutput { .. })
}

fn expr_is_net_connect(expr: &ValueExpr) -> bool {
    matches!(expr, ValueExpr::NetConnect { .. })
}

fn expr_is_net_listen(expr: &ValueExpr) -> bool {
    matches!(expr, ValueExpr::NetListen { .. })
}

fn expr_is_net_udp_bind(expr: &ValueExpr) -> bool {
    matches!(expr, ValueExpr::NetUdpBind { .. })
}

fn expr_is_http_client_call(expr: &ValueExpr) -> bool {
    matches!(
        expr,
        ValueExpr::Call { name, .. }
            if name == BUILTIN_HTTP_GET_EXPR || name == BUILTIN_HTTP_POST_EXPR
    )
}

fn expr_is_http_server_call(expr: &ValueExpr) -> bool {
    matches!(
        expr,
        ValueExpr::Call { name, .. }
            if name == BUILTIN_HTTP_LISTEN_EXPR
                || name == BUILTIN_HTTP_ACCEPT_EXPR
                || name == BUILTIN_HTTP_RESPOND_STRING_EXPR
                || name == BUILTIN_HTTP_CLOSE_SERVER_EXPR
                || name == BUILTIN_HTTP_CLOSE_EXCHANGE_EXPR
    )
}

fn expr_is_tcp_listener_accept(expr: &ValueExpr) -> bool {
    matches!(expr, ValueExpr::TcpListenerAccept { .. })
}

fn expr_is_tcp_listener_close(expr: &ValueExpr) -> bool {
    matches!(expr, ValueExpr::TcpListenerClose { .. })
}

fn expr_is_tcp_stream_read_to_string(expr: &ValueExpr) -> bool {
    matches!(expr, ValueExpr::TcpStreamReadToString { .. })
}

fn expr_is_tcp_stream_write_string(expr: &ValueExpr) -> bool {
    matches!(expr, ValueExpr::TcpStreamWriteString { .. })
}

fn expr_is_tcp_stream_close(expr: &ValueExpr) -> bool {
    matches!(expr, ValueExpr::TcpStreamClose { .. })
}

fn expr_is_udp_socket_recv_from_string(expr: &ValueExpr) -> bool {
    matches!(expr, ValueExpr::UdpSocketRecvFromString { .. })
}

fn expr_is_udp_socket_send_to_string(expr: &ValueExpr) -> bool {
    matches!(expr, ValueExpr::UdpSocketSendToString { .. })
}

fn expr_is_udp_socket_close(expr: &ValueExpr) -> bool {
    matches!(expr, ValueExpr::UdpSocketClose { .. })
}

fn expr_is_fs_exists(expr: &ValueExpr) -> bool {
    matches!(expr, ValueExpr::FsExists { .. })
}

fn expr_is_fs_metadata(expr: &ValueExpr) -> bool {
    matches!(expr, ValueExpr::FsMetadata { .. })
}

fn expr_is_fs_create_dir(expr: &ValueExpr) -> bool {
    matches!(expr, ValueExpr::FsCreateDir { .. })
}

fn expr_is_fs_remove_dir(expr: &ValueExpr) -> bool {
    matches!(expr, ValueExpr::FsRemoveDir { .. })
}

fn expr_is_fs_read_dir(expr: &ValueExpr) -> bool {
    matches!(expr, ValueExpr::FsReadDir { .. })
}

fn expr_is_fs_read_bytes(expr: &ValueExpr) -> bool {
    matches!(expr, ValueExpr::FsReadBytes { .. })
}

fn expr_is_fs_write_bytes(expr: &ValueExpr) -> bool {
    matches!(expr, ValueExpr::FsWriteBytes { .. })
}

fn expr_is_file_read_to_string(expr: &ValueExpr) -> bool {
    matches!(expr, ValueExpr::FileReadToString { .. })
}

fn expr_is_file_write_string(expr: &ValueExpr) -> bool {
    matches!(expr, ValueExpr::FileWriteString { .. })
}

fn expr_is_file_close(expr: &ValueExpr) -> bool {
    matches!(expr, ValueExpr::FileClose { .. })
}

fn expr_is_io_read_line(expr: &ValueExpr) -> bool {
    matches!(expr, ValueExpr::IoReadLine)
}

fn expr_is_log_enabled(expr: &ValueExpr) -> bool {
    matches!(expr, ValueExpr::LogEnabled { .. })
}

fn expr_is_hash_builtin(expr: &ValueExpr) -> bool {
    matches!(
        expr,
        ValueExpr::HashNew
            | ValueExpr::HashString { .. }
            | ValueExpr::HashBytes { .. }
            | ValueExpr::HashWriteString { .. }
            | ValueExpr::HashWriteBytes { .. }
            | ValueExpr::HashFinish { .. }
    )
}

fn expr_is_crypto_builtin(expr: &ValueExpr) -> bool {
    matches!(
        expr,
        ValueExpr::CryptoSha256 { .. }
            | ValueExpr::CryptoSha512 { .. }
            | ValueExpr::CryptoRandomBytes { .. }
    )
}

fn expr_is_json_builtin(expr: &ValueExpr) -> bool {
    matches!(
        expr,
        ValueExpr::JsonParse { .. } | ValueExpr::JsonStringify { .. }
    )
}

fn expr_is_regex_builtin(expr: &ValueExpr) -> bool {
    matches!(
        expr,
        ValueExpr::RegexCompile { .. }
            | ValueExpr::RegexIsMatch { .. }
            | ValueExpr::RegexCaptures { .. }
    )
}

fn expr_is_collections_builtin(expr: &ValueExpr) -> bool {
    matches!(
        expr,
        ValueExpr::CollectionsStringMapNew
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
    )
}

fn expr_is_num_parse_i64(expr: &ValueExpr) -> bool {
    matches!(expr, ValueExpr::NumParseI64 { .. })
}

fn expr_is_num_parse_u64(expr: &ValueExpr) -> bool {
    matches!(expr, ValueExpr::NumParseU64 { .. })
}

fn expr_is_num_parse_f64(expr: &ValueExpr) -> bool {
    matches!(expr, ValueExpr::NumParseF64 { .. })
}

fn expr_is_env_cwd(expr: &ValueExpr) -> bool {
    matches!(expr, ValueExpr::EnvCwd)
}

fn expr_is_env_home_dir(expr: &ValueExpr) -> bool {
    matches!(expr, ValueExpr::EnvHomeDir)
}

fn expr_is_env_temp_dir(expr: &ValueExpr) -> bool {
    matches!(expr, ValueExpr::EnvTempDir)
}

fn statement_uses_env_get(statement: &Statement) -> bool {
    match statement {
        Statement::Let { initializer, .. } => expr_uses_env_get(initializer),
        Statement::LetIf {
            condition,
            body,
            else_body,
            ..
        } => {
            expr_uses_env_get(condition)
                || body.iter().any(statement_uses_env_get)
                || else_body.iter().any(statement_uses_env_get)
        }
        Statement::LetMatch { value, arms, .. } => {
            expr_uses_env_get(value)
                || arms
                    .iter()
                    .any(|arm| arm.body.iter().any(statement_uses_env_get))
        }
        Statement::QuestionLet { result_expr, .. } => expr_uses_env_get(result_expr),
        Statement::QuestionReturn { result_expr, .. } => expr_uses_env_get(result_expr),
        Statement::LetElse {
            value, else_body, ..
        } => expr_uses_env_get(value) || else_body.iter().any(statement_uses_env_get),
        Statement::IfLet {
            value,
            body,
            else_body,
            ..
        } => {
            expr_uses_env_get(value)
                || body.iter().any(statement_uses_env_get)
                || else_body
                    .as_ref()
                    .is_some_and(|else_body| else_body.iter().any(statement_uses_env_get))
        }
        Statement::If {
            condition,
            body,
            else_body,
        } => {
            expr_uses_env_get(condition)
                || body.iter().any(statement_uses_env_get)
                || else_body.iter().any(statement_uses_env_get)
        }
        Statement::Assign { value, .. }
        | Statement::AssignField { value, .. }
        | Statement::Println(value)
        | Statement::Print(value)
        | Statement::Eprintln(value)
        | Statement::Eprint(value)
        | Statement::Panic(value)
        | Statement::Expr(value)
        | Statement::Return(Some(value)) => expr_uses_env_get(value),
        Statement::Loop { kind, body } => match kind {
            LoopKind::Infinite => body.iter().any(statement_uses_env_get),
            LoopKind::While(condition) => {
                expr_uses_env_get(condition) || body.iter().any(statement_uses_env_get)
            }
            LoopKind::Iterate { iterable, .. } => {
                expr_uses_env_get(iterable) || body.iter().any(statement_uses_env_get)
            }
        },
        Statement::Match { value, arms, .. } => {
            expr_uses_env_get(value)
                || arms
                    .iter()
                    .any(|arm| arm.body.iter().any(statement_uses_env_get))
        }
        Statement::Defer { call } => deferred_uses_env_get(call),
        Statement::Break | Statement::Continue => false,
        Statement::Return(None) => false,
    }
}

fn statement_uses_env_args(statement: &Statement) -> bool {
    match statement {
        Statement::Let { initializer, .. } => expr_uses_env_args(initializer),
        Statement::LetIf {
            condition,
            body,
            else_body,
            ..
        } => {
            expr_uses_env_args(condition)
                || body.iter().any(statement_uses_env_args)
                || else_body.iter().any(statement_uses_env_args)
        }
        Statement::LetMatch { value, arms, .. } => {
            expr_uses_env_args(value)
                || arms
                    .iter()
                    .any(|arm| arm.body.iter().any(statement_uses_env_args))
        }
        Statement::QuestionLet { result_expr, .. } => expr_uses_env_args(result_expr),
        Statement::QuestionReturn { result_expr, .. } => expr_uses_env_args(result_expr),
        Statement::LetElse {
            value, else_body, ..
        } => expr_uses_env_args(value) || else_body.iter().any(statement_uses_env_args),
        Statement::IfLet {
            value,
            body,
            else_body,
            ..
        } => {
            expr_uses_env_args(value)
                || body.iter().any(statement_uses_env_args)
                || else_body
                    .as_ref()
                    .is_some_and(|else_body| else_body.iter().any(statement_uses_env_args))
        }
        Statement::If {
            condition,
            body,
            else_body,
        } => {
            expr_uses_env_args(condition)
                || body.iter().any(statement_uses_env_args)
                || else_body.iter().any(statement_uses_env_args)
        }
        Statement::Assign { value, .. }
        | Statement::AssignField { value, .. }
        | Statement::Println(value)
        | Statement::Print(value)
        | Statement::Eprintln(value)
        | Statement::Eprint(value)
        | Statement::Panic(value)
        | Statement::Expr(value)
        | Statement::Return(Some(value)) => expr_uses_env_args(value),
        Statement::Loop { kind, body } => match kind {
            LoopKind::Infinite => body.iter().any(statement_uses_env_args),
            LoopKind::While(condition) => {
                expr_uses_env_args(condition) || body.iter().any(statement_uses_env_args)
            }
            LoopKind::Iterate { iterable, .. } => {
                expr_uses_env_args(iterable) || body.iter().any(statement_uses_env_args)
            }
        },
        Statement::Match { value, arms, .. } => {
            expr_uses_env_args(value)
                || arms
                    .iter()
                    .any(|arm| arm.body.iter().any(statement_uses_env_args))
        }
        Statement::Defer { call } => deferred_uses_env_args(call),
        Statement::Break | Statement::Continue => false,
        Statement::Return(None) => false,
    }
}

fn collect_statement_array_elements(
    statement: &Statement,
    seen: &mut BTreeSet<String>,
    out: &mut Vec<ValueType>,
) {
    match statement {
        Statement::Let {
            value_type,
            initializer,
            ..
        } => {
            collect_type_array_elements(value_type, seen, out);
            collect_expr_array_elements(initializer, seen, out);
        }
        Statement::LetIf {
            value_type,
            condition,
            body,
            else_body,
            ..
        } => {
            collect_type_array_elements(value_type, seen, out);
            collect_expr_array_elements(condition, seen, out);
            for stmt in body {
                collect_statement_array_elements(stmt, seen, out);
            }
            for stmt in else_body {
                collect_statement_array_elements(stmt, seen, out);
            }
        }
        Statement::LetMatch {
            value_type,
            value,
            enum_args,
            arms,
            ..
        } => {
            collect_type_array_elements(value_type, seen, out);
            for arg in enum_args {
                collect_type_array_elements(arg, seen, out);
            }
            collect_expr_array_elements(value, seen, out);
            for arm in arms {
                for stmt in &arm.body {
                    collect_statement_array_elements(stmt, seen, out);
                }
            }
        }
        Statement::QuestionLet {
            value_type,
            result_type,
            return_type,
            result_expr,
            ..
        } => {
            collect_type_array_elements(value_type, seen, out);
            collect_type_array_elements(result_type, seen, out);
            collect_type_array_elements(return_type, seen, out);
            collect_expr_array_elements(result_expr, seen, out);
        }
        Statement::QuestionReturn {
            ok_type,
            result_type,
            return_type,
            result_expr,
            ..
        } => {
            collect_type_array_elements(ok_type, seen, out);
            collect_type_array_elements(result_type, seen, out);
            collect_type_array_elements(return_type, seen, out);
            collect_expr_array_elements(result_expr, seen, out);
        }
        Statement::LetElse {
            value_type,
            value,
            enum_args,
            else_body,
            ..
        } => {
            collect_type_array_elements(value_type, seen, out);
            for arg in enum_args {
                collect_type_array_elements(arg, seen, out);
            }
            collect_expr_array_elements(value, seen, out);
            for stmt in else_body {
                collect_statement_array_elements(stmt, seen, out);
            }
        }
        Statement::IfLet {
            value_type,
            value,
            enum_args,
            body,
            else_body,
            ..
        } => {
            if let Some(value_type) = value_type {
                collect_type_array_elements(value_type, seen, out);
            }
            for arg in enum_args {
                collect_type_array_elements(arg, seen, out);
            }
            collect_expr_array_elements(value, seen, out);
            for stmt in body {
                collect_statement_array_elements(stmt, seen, out);
            }
            if let Some(else_body) = else_body {
                for stmt in else_body {
                    collect_statement_array_elements(stmt, seen, out);
                }
            }
        }
        Statement::If {
            condition,
            body,
            else_body,
        } => {
            collect_expr_array_elements(condition, seen, out);
            for stmt in body {
                collect_statement_array_elements(stmt, seen, out);
            }
            for stmt in else_body {
                collect_statement_array_elements(stmt, seen, out);
            }
        }
        Statement::Assign { value, .. }
        | Statement::AssignField { value, .. }
        | Statement::Println(value)
        | Statement::Print(value)
        | Statement::Eprintln(value)
        | Statement::Eprint(value)
        | Statement::Panic(value)
        | Statement::Expr(value)
        | Statement::Return(Some(value)) => collect_expr_array_elements(value, seen, out),
        Statement::Loop { kind, body } => match kind {
            LoopKind::Infinite => {
                for stmt in body {
                    collect_statement_array_elements(stmt, seen, out);
                }
            }
            LoopKind::While(condition) => {
                collect_expr_array_elements(condition, seen, out);
                for stmt in body {
                    collect_statement_array_elements(stmt, seen, out);
                }
            }
            LoopKind::Iterate {
                element_type,
                iterable,
                ..
            } => {
                collect_type_array_elements(element_type, seen, out);
                collect_expr_array_elements(iterable, seen, out);
                for stmt in body {
                    collect_statement_array_elements(stmt, seen, out);
                }
            }
        },
        Statement::Match { value, arms, .. } => {
            collect_expr_array_elements(value, seen, out);
            for arm in arms {
                for stmt in &arm.body {
                    collect_statement_array_elements(stmt, seen, out);
                }
            }
        }
        Statement::Defer { call } => collect_deferred_array_elements(call, seen, out),
        Statement::Break | Statement::Continue => {}
        Statement::Return(None) => {}
    }
}

fn deferred_uses_fs_read_to_string(call: &DeferredCall) -> bool {
    match call {
        DeferredCall::Expr(expr)
        | DeferredCall::Println(expr)
        | DeferredCall::Print(expr)
        | DeferredCall::Eprintln(expr)
        | DeferredCall::Eprint(expr) => expr_uses_fs_read_to_string(expr),
    }
}

fn deferred_uses_fs_write_string(call: &DeferredCall) -> bool {
    match call {
        DeferredCall::Expr(expr)
        | DeferredCall::Println(expr)
        | DeferredCall::Print(expr)
        | DeferredCall::Eprintln(expr)
        | DeferredCall::Eprint(expr) => expr_uses_fs_write_string(expr),
    }
}

fn deferred_uses_fs_open(call: &DeferredCall) -> bool {
    match call {
        DeferredCall::Expr(expr)
        | DeferredCall::Println(expr)
        | DeferredCall::Print(expr)
        | DeferredCall::Eprintln(expr)
        | DeferredCall::Eprint(expr) => expr_uses_fs_open(expr),
    }
}

fn deferred_uses_env_get(call: &DeferredCall) -> bool {
    match call {
        DeferredCall::Expr(expr)
        | DeferredCall::Println(expr)
        | DeferredCall::Print(expr)
        | DeferredCall::Eprintln(expr)
        | DeferredCall::Eprint(expr) => expr_uses_env_get(expr),
    }
}

fn deferred_uses_env_args(call: &DeferredCall) -> bool {
    match call {
        DeferredCall::Expr(expr)
        | DeferredCall::Println(expr)
        | DeferredCall::Print(expr)
        | DeferredCall::Eprintln(expr)
        | DeferredCall::Eprint(expr) => expr_uses_env_args(expr),
    }
}

fn collect_deferred_array_elements(
    call: &DeferredCall,
    seen: &mut BTreeSet<String>,
    out: &mut Vec<ValueType>,
) {
    match call {
        DeferredCall::Expr(expr)
        | DeferredCall::Println(expr)
        | DeferredCall::Print(expr)
        | DeferredCall::Eprintln(expr)
        | DeferredCall::Eprint(expr) => {
            collect_expr_array_elements(expr, seen, out);
        }
    }
}

fn expr_uses_fs_read_to_string(expr: &ValueExpr) -> bool {
    match expr {
        ValueExpr::FsReadToString { .. } => true,
        ValueExpr::Binary { left, right, .. }
        | ValueExpr::StringCompare { left, right, .. }
        | ValueExpr::StringConcat { left, right }
        | ValueExpr::StringContains {
            value: left,
            needle: right,
        }
        | ValueExpr::StringStartsWith {
            value: left,
            prefix: right,
        }
        | ValueExpr::StringEndsWith {
            value: left,
            suffix: right,
        }
        | ValueExpr::StringSplit {
            value: left,
            separator: right,
        }
        | ValueExpr::PathJoin { left, right }
        | ValueExpr::NumBinary { left, right, .. }
        | ValueExpr::MathBinary { left, right, .. }
        | ValueExpr::CollectionsStringMapGet {
            map: left,
            key: right,
        }
        | ValueExpr::CollectionsStringMapContains {
            map: left,
            key: right,
        }
        | ValueExpr::CollectionsStringMapRemove {
            map: left,
            key: right,
        }
        | ValueExpr::CollectionsStringSetContains {
            set: left,
            value: right,
        }
        | ValueExpr::CollectionsStringSetInsert {
            set: left,
            value: right,
        }
        | ValueExpr::CollectionsStringSetRemove {
            set: left,
            value: right,
        }
        | ValueExpr::RegexIsMatch {
            regex: left,
            value: right,
        }
        | ValueExpr::RegexCaptures {
            regex: left,
            value: right,
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
        } => expr_uses_fs_read_to_string(left) || expr_uses_fs_read_to_string(right),
        ValueExpr::UdpSocketSendToString {
            socket,
            content,
            host,
            port,
        } => {
            expr_uses_fs_read_to_string(socket)
                || expr_uses_fs_read_to_string(content)
                || expr_uses_fs_read_to_string(host)
                || expr_uses_fs_read_to_string(port)
        }
        ValueExpr::FsWriteString { path, content } => {
            expr_uses_fs_read_to_string(path) || expr_uses_fs_read_to_string(content)
        }
        ValueExpr::FsWriteBytes { path, bytes } => {
            expr_uses_fs_read_to_string(path) || expr_uses_fs_read_to_string(bytes)
        }
        ValueExpr::EnvSet { name, value } => {
            expr_uses_fs_read_to_string(name) || expr_uses_fs_read_to_string(value)
        }
        ValueExpr::FsExists { path }
        | ValueExpr::FsReadBytes { path }
        | ValueExpr::FsMetadata { path }
        | ValueExpr::FsCreateDir { path }
        | ValueExpr::FsRemoveDir { path }
        | ValueExpr::FsReadDir { path }
        | ValueExpr::FsOpen { path }
        | ValueExpr::FileClose { file: path }
        | ValueExpr::FileReadToString { file: path }
        | ValueExpr::TcpListenerAccept { listener: path }
        | ValueExpr::TcpListenerClose { listener: path }
        | ValueExpr::TcpStreamClose { stream: path }
        | ValueExpr::TcpStreamReadToString { stream: path }
        | ValueExpr::UdpSocketClose { socket: path } => expr_uses_fs_read_to_string(path),
        ValueExpr::FileWriteString { file, content } => {
            expr_uses_fs_read_to_string(file) || expr_uses_fs_read_to_string(content)
        }
        ValueExpr::ResultMapErr { result, .. }
        | ValueExpr::ResultIsOk { result, .. }
        | ValueExpr::ResultIsErr { result, .. }
        | ValueExpr::ResultMap { result, .. }
        | ValueExpr::ResultAndThen { result, .. }
        | ValueExpr::OptionIsSome { option: result, .. }
        | ValueExpr::OptionIsNone { option: result, .. }
        | ValueExpr::OptionMap { option: result, .. }
        | ValueExpr::OptionAndThen { option: result, .. } => expr_uses_fs_read_to_string(result),
        ValueExpr::ResultUnwrapOr {
            result, default, ..
        } => expr_uses_fs_read_to_string(result) || expr_uses_fs_read_to_string(default),
        ValueExpr::OptionUnwrapOr {
            option, default, ..
        } => expr_uses_fs_read_to_string(option) || expr_uses_fs_read_to_string(default),
        ValueExpr::EnvGet { name }
        | ValueExpr::PathBasename { path: name }
        | ValueExpr::PathDirname { path: name }
        | ValueExpr::PathExtension { path: name }
        | ValueExpr::PathNormalize { path: name }
        | ValueExpr::PathIsAbsolute { path: name }
        | ValueExpr::MathUnary { value: name, .. }
        | ValueExpr::TimeDurationMillis { millis: name }
        | ValueExpr::TimeDurationSeconds { seconds: name }
        | ValueExpr::TimeDurationAsMillis { duration: name }
        | ValueExpr::TimeFormatDuration { duration: name }
        | ValueExpr::TimeSleep { duration: name }
        | ValueExpr::TimeSleepMillis { duration: name }
        | ValueExpr::LogEnabled { level: name }
        | ValueExpr::HashString { value: name }
        | ValueExpr::HashBytes { value: name }
        | ValueExpr::HashFinish { state: name }
        | ValueExpr::CryptoSha256 { value: name }
        | ValueExpr::CryptoSha512 { value: name }
        | ValueExpr::CryptoRandomBytes { count: name }
        | ValueExpr::JsonParse { value: name }
        | ValueExpr::JsonStringify { value: name }
        | ValueExpr::RegexCompile { pattern: name }
        | ValueExpr::CollectionsStringMapLen { map: name }
        | ValueExpr::CollectionsStringSetLen { set: name }
        | ValueExpr::ProcessExit { code: name }
        | ValueExpr::ProcessSpawn { command: name }
        | ValueExpr::ProcessStatus { command: name }
        | ValueExpr::ProcessExec { command: name }
        | ValueExpr::ProcessOutput { command: name }
        | ValueExpr::NumParseI64 { value: name }
        | ValueExpr::NumParseU64 { value: name }
        | ValueExpr::NumParseF64 { value: name }
        | ValueExpr::NumToString { value: name, .. } => expr_uses_fs_read_to_string(name),
        ValueExpr::EnvArgs => false,
        ValueExpr::ArrayNew { .. }
        | ValueExpr::HashNew
        | ValueExpr::CollectionsStringMapNew
        | ValueExpr::CollectionsStringSetNew => false,
        ValueExpr::HashWriteString { state, value } => {
            expr_uses_fs_read_to_string(state) || expr_uses_fs_read_to_string(value)
        }
        ValueExpr::HashWriteBytes { state, value } => {
            expr_uses_fs_read_to_string(state) || expr_uses_fs_read_to_string(value)
        }
        ValueExpr::CollectionsStringMapSet { map, key, value } => {
            expr_uses_fs_read_to_string(map)
                || expr_uses_fs_read_to_string(key)
                || expr_uses_fs_read_to_string(value)
        }
        ValueExpr::ArrayLen { array } => expr_uses_fs_read_to_string(array),
        ValueExpr::ArrayIter { array, .. } => expr_uses_fs_read_to_string(array),
        ValueExpr::ArrayGet { array, index, .. } => {
            expr_uses_fs_read_to_string(array) || expr_uses_fs_read_to_string(index)
        }
        ValueExpr::ArrayPop { .. } | ValueExpr::ArrayClear { .. } => false,
        ValueExpr::ArrayRemove { index, .. } => expr_uses_fs_read_to_string(index),
        ValueExpr::ArrayPush { value, .. } => expr_uses_fs_read_to_string(value),
        ValueExpr::ArraySet { index, value, .. } => {
            expr_uses_fs_read_to_string(index) || expr_uses_fs_read_to_string(value)
        }
        ValueExpr::ArrayInsert { index, value, .. } => {
            expr_uses_fs_read_to_string(index) || expr_uses_fs_read_to_string(value)
        }
        ValueExpr::Call { args, .. } => args.iter().any(expr_uses_fs_read_to_string),
        ValueExpr::StringLen { value }
        | ValueExpr::StringIsEmpty { value }
        | ValueExpr::StringTrim { value }
        | ValueExpr::StringToLower { value }
        | ValueExpr::StringToUpper { value }
        | ValueExpr::CharIsDigit { value }
        | ValueExpr::CharIsAlpha { value }
        | ValueExpr::CharIsWhitespace { value }
        | ValueExpr::CharToString { value }
        | ValueExpr::Unary { expr: value, .. }
        | ValueExpr::Cast { expr: value, .. }
        | ValueExpr::EnumPayload { value, .. } => expr_uses_fs_read_to_string(value),
        ValueExpr::StructLiteral { fields, .. } => fields
            .iter()
            .any(|(_, value)| expr_uses_fs_read_to_string(value)),
        ValueExpr::EnumVariant { payload, .. } => {
            payload.as_deref().is_some_and(expr_uses_fs_read_to_string)
        }
        ValueExpr::If {
            condition,
            then_branch,
            else_branch,
        } => {
            expr_uses_fs_read_to_string(condition)
                || expr_uses_fs_read_to_string(then_branch)
                || expr_uses_fs_read_to_string(else_branch)
        }
        ValueExpr::Panic { message, .. } => expr_uses_fs_read_to_string(message),
        ValueExpr::Match { value, arms } => {
            expr_uses_fs_read_to_string(value)
                || arms
                    .iter()
                    .any(|arm| expr_uses_fs_read_to_string(&arm.value))
        }
        ValueExpr::StringLiteral(_)
        | ValueExpr::IntLiteral(_)
        | ValueExpr::FloatLiteral(_)
        | ValueExpr::CharLiteral(_)
        | ValueExpr::BoolLiteral(_)
        | ValueExpr::VoidLiteral
        | ValueExpr::Variable(_)
        | ValueExpr::MutBorrow(_)
        | ValueExpr::EnvCwd
        | ValueExpr::EnvHomeDir
        | ValueExpr::EnvTempDir
        | ValueExpr::OsPlatform
        | ValueExpr::OsArch
        | ValueExpr::OsPathSeparator
        | ValueExpr::OsLineEnding
        | ValueExpr::TimeNowMillis
        | ValueExpr::TimeMonotonicMillis
        | ValueExpr::IoReadLine
        | ValueExpr::FieldAccess { .. } => false,
        ValueExpr::EnumPayloadFieldAccess { value, .. } => expr_uses_fs_read_to_string(value),
    }
}

fn expr_uses_fs_write_string(expr: &ValueExpr) -> bool {
    match expr {
        ValueExpr::FsWriteString { .. } => true,
        ValueExpr::Binary { left, right, .. }
        | ValueExpr::StringCompare { left, right, .. }
        | ValueExpr::StringConcat { left, right }
        | ValueExpr::StringContains {
            value: left,
            needle: right,
        }
        | ValueExpr::StringStartsWith {
            value: left,
            prefix: right,
        }
        | ValueExpr::StringEndsWith {
            value: left,
            suffix: right,
        }
        | ValueExpr::StringSplit {
            value: left,
            separator: right,
        }
        | ValueExpr::PathJoin { left, right }
        | ValueExpr::NumBinary { left, right, .. }
        | ValueExpr::MathBinary { left, right, .. }
        | ValueExpr::CollectionsStringMapGet {
            map: left,
            key: right,
        }
        | ValueExpr::CollectionsStringMapContains {
            map: left,
            key: right,
        }
        | ValueExpr::CollectionsStringMapRemove {
            map: left,
            key: right,
        }
        | ValueExpr::CollectionsStringSetContains {
            set: left,
            value: right,
        }
        | ValueExpr::CollectionsStringSetInsert {
            set: left,
            value: right,
        }
        | ValueExpr::CollectionsStringSetRemove {
            set: left,
            value: right,
        }
        | ValueExpr::RegexIsMatch {
            regex: left,
            value: right,
        }
        | ValueExpr::RegexCaptures {
            regex: left,
            value: right,
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
        } => expr_uses_fs_write_string(left) || expr_uses_fs_write_string(right),
        ValueExpr::UdpSocketSendToString {
            socket,
            content,
            host,
            port,
        } => {
            expr_uses_fs_write_string(socket)
                || expr_uses_fs_write_string(content)
                || expr_uses_fs_write_string(host)
                || expr_uses_fs_write_string(port)
        }
        ValueExpr::FsReadToString { path }
        | ValueExpr::FsReadBytes { path }
        | ValueExpr::FsExists { path }
        | ValueExpr::FsMetadata { path }
        | ValueExpr::FsCreateDir { path }
        | ValueExpr::FsRemoveDir { path }
        | ValueExpr::FsReadDir { path } => expr_uses_fs_write_string(path),
        ValueExpr::FileReadToString { file }
        | ValueExpr::TcpListenerAccept { listener: file }
        | ValueExpr::TcpListenerClose { listener: file }
        | ValueExpr::TcpStreamClose { stream: file }
        | ValueExpr::TcpStreamReadToString { stream: file }
        | ValueExpr::UdpSocketClose { socket: file } => expr_uses_fs_write_string(file),
        ValueExpr::FileWriteString { file, content } => {
            expr_uses_fs_write_string(file) || expr_uses_fs_write_string(content)
        }
        ValueExpr::FsWriteBytes { path, bytes } => {
            expr_uses_fs_write_string(path) || expr_uses_fs_write_string(bytes)
        }
        ValueExpr::EnvSet { name, value } => {
            expr_uses_fs_write_string(name) || expr_uses_fs_write_string(value)
        }
        ValueExpr::FsOpen { path } | ValueExpr::FileClose { file: path } => {
            expr_uses_fs_write_string(path)
        }
        ValueExpr::ResultMapErr { result, .. }
        | ValueExpr::ResultIsOk { result, .. }
        | ValueExpr::ResultIsErr { result, .. }
        | ValueExpr::ResultMap { result, .. }
        | ValueExpr::ResultAndThen { result, .. }
        | ValueExpr::OptionIsSome { option: result, .. }
        | ValueExpr::OptionIsNone { option: result, .. }
        | ValueExpr::OptionMap { option: result, .. }
        | ValueExpr::OptionAndThen { option: result, .. } => expr_uses_fs_write_string(result),
        ValueExpr::ResultUnwrapOr {
            result, default, ..
        } => expr_uses_fs_write_string(result) || expr_uses_fs_write_string(default),
        ValueExpr::OptionUnwrapOr {
            option, default, ..
        } => expr_uses_fs_write_string(option) || expr_uses_fs_write_string(default),
        ValueExpr::EnvGet { name }
        | ValueExpr::PathBasename { path: name }
        | ValueExpr::PathDirname { path: name }
        | ValueExpr::PathExtension { path: name }
        | ValueExpr::PathNormalize { path: name }
        | ValueExpr::PathIsAbsolute { path: name }
        | ValueExpr::MathUnary { value: name, .. }
        | ValueExpr::TimeDurationMillis { millis: name }
        | ValueExpr::TimeDurationSeconds { seconds: name }
        | ValueExpr::TimeDurationAsMillis { duration: name }
        | ValueExpr::TimeFormatDuration { duration: name }
        | ValueExpr::TimeSleep { duration: name }
        | ValueExpr::TimeSleepMillis { duration: name }
        | ValueExpr::LogEnabled { level: name }
        | ValueExpr::HashString { value: name }
        | ValueExpr::HashBytes { value: name }
        | ValueExpr::HashFinish { state: name }
        | ValueExpr::CryptoSha256 { value: name }
        | ValueExpr::CryptoSha512 { value: name }
        | ValueExpr::CryptoRandomBytes { count: name }
        | ValueExpr::JsonParse { value: name }
        | ValueExpr::JsonStringify { value: name }
        | ValueExpr::RegexCompile { pattern: name }
        | ValueExpr::CollectionsStringMapLen { map: name }
        | ValueExpr::CollectionsStringSetLen { set: name }
        | ValueExpr::ProcessExit { code: name }
        | ValueExpr::ProcessSpawn { command: name }
        | ValueExpr::ProcessStatus { command: name }
        | ValueExpr::ProcessExec { command: name }
        | ValueExpr::ProcessOutput { command: name }
        | ValueExpr::NumParseI64 { value: name }
        | ValueExpr::NumParseU64 { value: name }
        | ValueExpr::NumParseF64 { value: name }
        | ValueExpr::NumToString { value: name, .. } => expr_uses_fs_write_string(name),
        ValueExpr::EnvArgs => false,
        ValueExpr::ArrayNew { .. }
        | ValueExpr::HashNew
        | ValueExpr::CollectionsStringMapNew
        | ValueExpr::CollectionsStringSetNew => false,
        ValueExpr::HashWriteString { state, value } => {
            expr_uses_fs_write_string(state) || expr_uses_fs_write_string(value)
        }
        ValueExpr::HashWriteBytes { state, value } => {
            expr_uses_fs_write_string(state) || expr_uses_fs_write_string(value)
        }
        ValueExpr::CollectionsStringMapSet { map, key, value } => {
            expr_uses_fs_write_string(map)
                || expr_uses_fs_write_string(key)
                || expr_uses_fs_write_string(value)
        }
        ValueExpr::ArrayLen { array } => expr_uses_fs_write_string(array),
        ValueExpr::ArrayIter { array, .. } => expr_uses_fs_write_string(array),
        ValueExpr::ArrayGet { array, index, .. } => {
            expr_uses_fs_write_string(array) || expr_uses_fs_write_string(index)
        }
        ValueExpr::ArrayPop { .. } | ValueExpr::ArrayClear { .. } => false,
        ValueExpr::ArrayRemove { index, .. } => expr_uses_fs_write_string(index),
        ValueExpr::ArrayPush { value, .. } => expr_uses_fs_write_string(value),
        ValueExpr::ArraySet { index, value, .. } => {
            expr_uses_fs_write_string(index) || expr_uses_fs_write_string(value)
        }
        ValueExpr::ArrayInsert { index, value, .. } => {
            expr_uses_fs_write_string(index) || expr_uses_fs_write_string(value)
        }
        ValueExpr::Call { args, .. } => args.iter().any(expr_uses_fs_write_string),
        ValueExpr::StringLen { value }
        | ValueExpr::StringIsEmpty { value }
        | ValueExpr::StringTrim { value }
        | ValueExpr::StringToLower { value }
        | ValueExpr::StringToUpper { value }
        | ValueExpr::CharIsDigit { value }
        | ValueExpr::CharIsAlpha { value }
        | ValueExpr::CharIsWhitespace { value }
        | ValueExpr::CharToString { value }
        | ValueExpr::Unary { expr: value, .. }
        | ValueExpr::Cast { expr: value, .. }
        | ValueExpr::EnumPayload { value, .. } => expr_uses_fs_write_string(value),
        ValueExpr::StructLiteral { fields, .. } => fields
            .iter()
            .any(|(_, value)| expr_uses_fs_write_string(value)),
        ValueExpr::EnumVariant { payload, .. } => {
            payload.as_deref().is_some_and(expr_uses_fs_write_string)
        }
        ValueExpr::If {
            condition,
            then_branch,
            else_branch,
        } => {
            expr_uses_fs_write_string(condition)
                || expr_uses_fs_write_string(then_branch)
                || expr_uses_fs_write_string(else_branch)
        }
        ValueExpr::Panic { message, .. } => expr_uses_fs_write_string(message),
        ValueExpr::Match { value, arms } => {
            expr_uses_fs_write_string(value)
                || arms.iter().any(|arm| expr_uses_fs_write_string(&arm.value))
        }
        ValueExpr::StringLiteral(_)
        | ValueExpr::IntLiteral(_)
        | ValueExpr::FloatLiteral(_)
        | ValueExpr::CharLiteral(_)
        | ValueExpr::BoolLiteral(_)
        | ValueExpr::VoidLiteral
        | ValueExpr::Variable(_)
        | ValueExpr::MutBorrow(_)
        | ValueExpr::EnvCwd
        | ValueExpr::EnvHomeDir
        | ValueExpr::EnvTempDir
        | ValueExpr::OsPlatform
        | ValueExpr::OsArch
        | ValueExpr::OsPathSeparator
        | ValueExpr::OsLineEnding
        | ValueExpr::TimeNowMillis
        | ValueExpr::TimeMonotonicMillis
        | ValueExpr::IoReadLine
        | ValueExpr::FieldAccess { .. } => false,
        ValueExpr::EnumPayloadFieldAccess { value, .. } => expr_uses_fs_write_string(value),
    }
}

fn expr_uses_fs_open(expr: &ValueExpr) -> bool {
    match expr {
        ValueExpr::FsOpen { .. } | ValueExpr::FileClose { .. } => true,
        ValueExpr::Binary { left, right, .. }
        | ValueExpr::StringCompare { left, right, .. }
        | ValueExpr::StringConcat { left, right }
        | ValueExpr::StringContains {
            value: left,
            needle: right,
        }
        | ValueExpr::StringStartsWith {
            value: left,
            prefix: right,
        }
        | ValueExpr::StringEndsWith {
            value: left,
            suffix: right,
        }
        | ValueExpr::StringSplit {
            value: left,
            separator: right,
        }
        | ValueExpr::PathJoin { left, right }
        | ValueExpr::NumBinary { left, right, .. }
        | ValueExpr::MathBinary { left, right, .. }
        | ValueExpr::CollectionsStringMapGet {
            map: left,
            key: right,
        }
        | ValueExpr::CollectionsStringMapContains {
            map: left,
            key: right,
        }
        | ValueExpr::CollectionsStringMapRemove {
            map: left,
            key: right,
        }
        | ValueExpr::CollectionsStringSetContains {
            set: left,
            value: right,
        }
        | ValueExpr::CollectionsStringSetInsert {
            set: left,
            value: right,
        }
        | ValueExpr::CollectionsStringSetRemove {
            set: left,
            value: right,
        }
        | ValueExpr::RegexIsMatch {
            regex: left,
            value: right,
        }
        | ValueExpr::RegexCaptures {
            regex: left,
            value: right,
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
        } => expr_uses_fs_open(left) || expr_uses_fs_open(right),
        ValueExpr::UdpSocketSendToString {
            socket,
            content,
            host,
            port,
        } => {
            expr_uses_fs_open(socket)
                || expr_uses_fs_open(content)
                || expr_uses_fs_open(host)
                || expr_uses_fs_open(port)
        }
        ValueExpr::FsReadToString { path }
        | ValueExpr::FsReadBytes { path }
        | ValueExpr::FsExists { path }
        | ValueExpr::FsMetadata { path }
        | ValueExpr::FsCreateDir { path }
        | ValueExpr::FsRemoveDir { path }
        | ValueExpr::FsReadDir { path }
        | ValueExpr::EnvGet { name: path }
        | ValueExpr::PathBasename { path }
        | ValueExpr::PathDirname { path }
        | ValueExpr::PathExtension { path }
        | ValueExpr::PathNormalize { path }
        | ValueExpr::PathIsAbsolute { path }
        | ValueExpr::MathUnary { value: path, .. }
        | ValueExpr::TimeDurationMillis { millis: path }
        | ValueExpr::TimeDurationSeconds { seconds: path }
        | ValueExpr::TimeDurationAsMillis { duration: path }
        | ValueExpr::TimeFormatDuration { duration: path }
        | ValueExpr::TimeSleep { duration: path }
        | ValueExpr::TimeSleepMillis { duration: path }
        | ValueExpr::LogEnabled { level: path }
        | ValueExpr::HashString { value: path }
        | ValueExpr::HashBytes { value: path }
        | ValueExpr::HashFinish { state: path }
        | ValueExpr::CryptoSha256 { value: path }
        | ValueExpr::CryptoSha512 { value: path }
        | ValueExpr::CryptoRandomBytes { count: path }
        | ValueExpr::JsonParse { value: path }
        | ValueExpr::JsonStringify { value: path }
        | ValueExpr::RegexCompile { pattern: path }
        | ValueExpr::CollectionsStringMapLen { map: path }
        | ValueExpr::CollectionsStringSetLen { set: path }
        | ValueExpr::ProcessExit { code: path }
        | ValueExpr::ProcessSpawn { command: path }
        | ValueExpr::ProcessStatus { command: path }
        | ValueExpr::ProcessExec { command: path }
        | ValueExpr::ProcessOutput { command: path }
        | ValueExpr::NumParseI64 { value: path }
        | ValueExpr::NumParseU64 { value: path }
        | ValueExpr::NumParseF64 { value: path }
        | ValueExpr::NumToString { value: path, .. }
        | ValueExpr::ArrayLen { array: path }
        | ValueExpr::FileReadToString { file: path }
        | ValueExpr::TcpListenerAccept { listener: path }
        | ValueExpr::TcpListenerClose { listener: path }
        | ValueExpr::TcpStreamClose { stream: path }
        | ValueExpr::TcpStreamReadToString { stream: path }
        | ValueExpr::UdpSocketClose { socket: path } => expr_uses_fs_open(path),
        ValueExpr::ResultMapErr { result, .. }
        | ValueExpr::ResultIsOk { result, .. }
        | ValueExpr::ResultIsErr { result, .. }
        | ValueExpr::ResultMap { result, .. }
        | ValueExpr::ResultAndThen { result, .. }
        | ValueExpr::OptionIsSome { option: result, .. }
        | ValueExpr::OptionIsNone { option: result, .. }
        | ValueExpr::OptionMap { option: result, .. }
        | ValueExpr::OptionAndThen { option: result, .. } => expr_uses_fs_open(result),
        ValueExpr::ResultUnwrapOr {
            result, default, ..
        } => expr_uses_fs_open(result) || expr_uses_fs_open(default),
        ValueExpr::OptionUnwrapOr {
            option, default, ..
        } => expr_uses_fs_open(option) || expr_uses_fs_open(default),
        ValueExpr::FsWriteString { path, content } => {
            expr_uses_fs_open(path) || expr_uses_fs_open(content)
        }
        ValueExpr::FsWriteBytes { path, bytes } => {
            expr_uses_fs_open(path) || expr_uses_fs_open(bytes)
        }
        ValueExpr::FileWriteString { file, content } => {
            expr_uses_fs_open(file) || expr_uses_fs_open(content)
        }
        ValueExpr::EnvSet { name, value } => expr_uses_fs_open(name) || expr_uses_fs_open(value),
        ValueExpr::EnvArgs => false,
        ValueExpr::EnvCwd | ValueExpr::EnvHomeDir | ValueExpr::EnvTempDir => false,
        ValueExpr::ArrayNew { .. }
        | ValueExpr::HashNew
        | ValueExpr::CollectionsStringMapNew
        | ValueExpr::CollectionsStringSetNew => false,
        ValueExpr::HashWriteString { state, value } => {
            expr_uses_fs_open(state) || expr_uses_fs_open(value)
        }
        ValueExpr::HashWriteBytes { state, value } => {
            expr_uses_fs_open(state) || expr_uses_fs_open(value)
        }
        ValueExpr::CollectionsStringMapSet { map, key, value } => {
            expr_uses_fs_open(map) || expr_uses_fs_open(key) || expr_uses_fs_open(value)
        }
        ValueExpr::ArrayIter { array, .. } => expr_uses_fs_open(array),
        ValueExpr::ArrayGet { array, index, .. } => {
            expr_uses_fs_open(array) || expr_uses_fs_open(index)
        }
        ValueExpr::ArrayPop { .. } | ValueExpr::ArrayClear { .. } => false,
        ValueExpr::ArrayRemove { index, .. } => expr_uses_fs_open(index),
        ValueExpr::ArrayPush { value, .. } => expr_uses_fs_open(value),
        ValueExpr::ArraySet { index, value, .. } => {
            expr_uses_fs_open(index) || expr_uses_fs_open(value)
        }
        ValueExpr::ArrayInsert { index, value, .. } => {
            expr_uses_fs_open(index) || expr_uses_fs_open(value)
        }
        ValueExpr::Call { args, .. } => args.iter().any(expr_uses_fs_open),
        ValueExpr::StringLen { value }
        | ValueExpr::StringIsEmpty { value }
        | ValueExpr::StringTrim { value }
        | ValueExpr::StringToLower { value }
        | ValueExpr::StringToUpper { value }
        | ValueExpr::CharIsDigit { value }
        | ValueExpr::CharIsAlpha { value }
        | ValueExpr::CharIsWhitespace { value }
        | ValueExpr::CharToString { value }
        | ValueExpr::Unary { expr: value, .. }
        | ValueExpr::Cast { expr: value, .. }
        | ValueExpr::EnumPayload { value, .. }
        | ValueExpr::EnumPayloadFieldAccess { value, .. } => expr_uses_fs_open(value),
        ValueExpr::StructLiteral { fields, .. } => {
            fields.iter().any(|(_, value)| expr_uses_fs_open(value))
        }
        ValueExpr::EnumVariant { payload, .. } => payload.as_deref().is_some_and(expr_uses_fs_open),
        ValueExpr::If {
            condition,
            then_branch,
            else_branch,
        } => {
            expr_uses_fs_open(condition)
                || expr_uses_fs_open(then_branch)
                || expr_uses_fs_open(else_branch)
        }
        ValueExpr::Panic { message, .. } => expr_uses_fs_open(message),
        ValueExpr::Match { value, arms } => {
            expr_uses_fs_open(value) || arms.iter().any(|arm| expr_uses_fs_open(&arm.value))
        }
        ValueExpr::StringLiteral(_)
        | ValueExpr::IntLiteral(_)
        | ValueExpr::FloatLiteral(_)
        | ValueExpr::CharLiteral(_)
        | ValueExpr::BoolLiteral(_)
        | ValueExpr::VoidLiteral
        | ValueExpr::Variable(_)
        | ValueExpr::MutBorrow(_)
        | ValueExpr::OsPlatform
        | ValueExpr::OsArch
        | ValueExpr::OsPathSeparator
        | ValueExpr::OsLineEnding
        | ValueExpr::TimeNowMillis
        | ValueExpr::TimeMonotonicMillis
        | ValueExpr::IoReadLine
        | ValueExpr::FieldAccess { .. } => false,
    }
}

fn expr_uses_env_get(expr: &ValueExpr) -> bool {
    match expr {
        ValueExpr::EnvGet { .. } => true,
        ValueExpr::Binary { left, right, .. }
        | ValueExpr::StringCompare { left, right, .. }
        | ValueExpr::StringConcat { left, right }
        | ValueExpr::StringContains {
            value: left,
            needle: right,
        }
        | ValueExpr::StringStartsWith {
            value: left,
            prefix: right,
        }
        | ValueExpr::StringEndsWith {
            value: left,
            suffix: right,
        }
        | ValueExpr::StringSplit {
            value: left,
            separator: right,
        }
        | ValueExpr::PathJoin { left, right }
        | ValueExpr::NumBinary { left, right, .. }
        | ValueExpr::MathBinary { left, right, .. }
        | ValueExpr::CollectionsStringMapGet {
            map: left,
            key: right,
        }
        | ValueExpr::CollectionsStringMapContains {
            map: left,
            key: right,
        }
        | ValueExpr::CollectionsStringMapRemove {
            map: left,
            key: right,
        }
        | ValueExpr::CollectionsStringSetContains {
            set: left,
            value: right,
        }
        | ValueExpr::CollectionsStringSetInsert {
            set: left,
            value: right,
        }
        | ValueExpr::CollectionsStringSetRemove {
            set: left,
            value: right,
        }
        | ValueExpr::RegexIsMatch {
            regex: left,
            value: right,
        }
        | ValueExpr::RegexCaptures {
            regex: left,
            value: right,
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
        } => expr_uses_env_get(left) || expr_uses_env_get(right),
        ValueExpr::UdpSocketSendToString {
            socket,
            content,
            host,
            port,
        } => {
            expr_uses_env_get(socket)
                || expr_uses_env_get(content)
                || expr_uses_env_get(host)
                || expr_uses_env_get(port)
        }
        ValueExpr::FsReadToString { path }
        | ValueExpr::FsReadBytes { path }
        | ValueExpr::FsExists { path }
        | ValueExpr::FsMetadata { path }
        | ValueExpr::FsCreateDir { path }
        | ValueExpr::FsRemoveDir { path }
        | ValueExpr::FsReadDir { path }
        | ValueExpr::PathBasename { path }
        | ValueExpr::PathDirname { path }
        | ValueExpr::PathExtension { path }
        | ValueExpr::PathNormalize { path }
        | ValueExpr::PathIsAbsolute { path }
        | ValueExpr::MathUnary { value: path, .. }
        | ValueExpr::TimeDurationMillis { millis: path }
        | ValueExpr::TimeDurationSeconds { seconds: path }
        | ValueExpr::TimeDurationAsMillis { duration: path }
        | ValueExpr::TimeFormatDuration { duration: path }
        | ValueExpr::TimeSleep { duration: path }
        | ValueExpr::TimeSleepMillis { duration: path }
        | ValueExpr::LogEnabled { level: path }
        | ValueExpr::HashString { value: path }
        | ValueExpr::HashBytes { value: path }
        | ValueExpr::HashFinish { state: path }
        | ValueExpr::CryptoSha256 { value: path }
        | ValueExpr::CryptoSha512 { value: path }
        | ValueExpr::CryptoRandomBytes { count: path }
        | ValueExpr::JsonParse { value: path }
        | ValueExpr::JsonStringify { value: path }
        | ValueExpr::RegexCompile { pattern: path }
        | ValueExpr::CollectionsStringMapLen { map: path }
        | ValueExpr::CollectionsStringSetLen { set: path }
        | ValueExpr::ProcessExit { code: path }
        | ValueExpr::ProcessSpawn { command: path }
        | ValueExpr::ProcessStatus { command: path }
        | ValueExpr::ProcessExec { command: path }
        | ValueExpr::ProcessOutput { command: path }
        | ValueExpr::NumParseI64 { value: path }
        | ValueExpr::NumParseU64 { value: path }
        | ValueExpr::NumParseF64 { value: path }
        | ValueExpr::NumToString { value: path, .. }
        | ValueExpr::FileReadToString { file: path }
        | ValueExpr::TcpListenerAccept { listener: path }
        | ValueExpr::TcpListenerClose { listener: path }
        | ValueExpr::TcpStreamClose { stream: path }
        | ValueExpr::TcpStreamReadToString { stream: path }
        | ValueExpr::UdpSocketClose { socket: path } => expr_uses_env_get(path),
        ValueExpr::FsWriteString { path, content } => {
            expr_uses_env_get(path) || expr_uses_env_get(content)
        }
        ValueExpr::FsWriteBytes { path, bytes } => {
            expr_uses_env_get(path) || expr_uses_env_get(bytes)
        }
        ValueExpr::FileWriteString { file, content } => {
            expr_uses_env_get(file) || expr_uses_env_get(content)
        }
        ValueExpr::EnvSet { name, value } => expr_uses_env_get(name) || expr_uses_env_get(value),
        ValueExpr::HashWriteString { state, value } => {
            expr_uses_env_get(state) || expr_uses_env_get(value)
        }
        ValueExpr::HashWriteBytes { state, value } => {
            expr_uses_env_get(state) || expr_uses_env_get(value)
        }
        ValueExpr::CollectionsStringMapSet { map, key, value } => {
            expr_uses_env_get(map) || expr_uses_env_get(key) || expr_uses_env_get(value)
        }
        ValueExpr::FsOpen { path } | ValueExpr::FileClose { file: path } => expr_uses_env_get(path),
        ValueExpr::ResultMapErr { result, .. }
        | ValueExpr::ResultIsOk { result, .. }
        | ValueExpr::ResultIsErr { result, .. }
        | ValueExpr::ResultMap { result, .. }
        | ValueExpr::ResultAndThen { result, .. }
        | ValueExpr::OptionIsSome { option: result, .. }
        | ValueExpr::OptionIsNone { option: result, .. }
        | ValueExpr::OptionMap { option: result, .. }
        | ValueExpr::OptionAndThen { option: result, .. } => expr_uses_env_get(result),
        ValueExpr::ResultUnwrapOr {
            result, default, ..
        } => expr_uses_env_get(result) || expr_uses_env_get(default),
        ValueExpr::OptionUnwrapOr {
            option, default, ..
        } => expr_uses_env_get(option) || expr_uses_env_get(default),
        ValueExpr::EnvArgs => false,
        ValueExpr::ArrayNew { .. }
        | ValueExpr::CollectionsStringMapNew
        | ValueExpr::CollectionsStringSetNew => false,
        ValueExpr::ArrayLen { array } => expr_uses_env_get(array),
        ValueExpr::ArrayIter { array, .. } => expr_uses_env_get(array),
        ValueExpr::ArrayGet { array, index, .. } => {
            expr_uses_env_get(array) || expr_uses_env_get(index)
        }
        ValueExpr::ArrayPop { .. } | ValueExpr::ArrayClear { .. } => false,
        ValueExpr::ArrayRemove { index, .. } => expr_uses_env_get(index),
        ValueExpr::ArrayPush { value, .. } => expr_uses_env_get(value),
        ValueExpr::ArraySet { index, value, .. } => {
            expr_uses_env_get(index) || expr_uses_env_get(value)
        }
        ValueExpr::ArrayInsert { index, value, .. } => {
            expr_uses_env_get(index) || expr_uses_env_get(value)
        }
        ValueExpr::Call { args, .. } => args.iter().any(expr_uses_env_get),
        ValueExpr::StringLen { value }
        | ValueExpr::StringIsEmpty { value }
        | ValueExpr::StringTrim { value }
        | ValueExpr::StringToLower { value }
        | ValueExpr::StringToUpper { value }
        | ValueExpr::CharIsDigit { value }
        | ValueExpr::CharIsAlpha { value }
        | ValueExpr::CharIsWhitespace { value }
        | ValueExpr::CharToString { value }
        | ValueExpr::Unary { expr: value, .. }
        | ValueExpr::Cast { expr: value, .. }
        | ValueExpr::EnumPayload { value, .. }
        | ValueExpr::EnumPayloadFieldAccess { value, .. } => expr_uses_env_get(value),
        ValueExpr::StructLiteral { fields, .. } => {
            fields.iter().any(|(_, value)| expr_uses_env_get(value))
        }
        ValueExpr::EnumVariant { payload, .. } => payload.as_deref().is_some_and(expr_uses_env_get),
        ValueExpr::If {
            condition,
            then_branch,
            else_branch,
        } => {
            expr_uses_env_get(condition)
                || expr_uses_env_get(then_branch)
                || expr_uses_env_get(else_branch)
        }
        ValueExpr::Panic { message, .. } => expr_uses_env_get(message),
        ValueExpr::Match { value, arms } => {
            expr_uses_env_get(value) || arms.iter().any(|arm| expr_uses_env_get(&arm.value))
        }
        ValueExpr::StringLiteral(_)
        | ValueExpr::IntLiteral(_)
        | ValueExpr::FloatLiteral(_)
        | ValueExpr::CharLiteral(_)
        | ValueExpr::BoolLiteral(_)
        | ValueExpr::VoidLiteral
        | ValueExpr::HashNew
        | ValueExpr::Variable(_)
        | ValueExpr::MutBorrow(_)
        | ValueExpr::EnvCwd
        | ValueExpr::EnvHomeDir
        | ValueExpr::EnvTempDir
        | ValueExpr::OsPlatform
        | ValueExpr::OsArch
        | ValueExpr::OsPathSeparator
        | ValueExpr::OsLineEnding
        | ValueExpr::TimeNowMillis
        | ValueExpr::TimeMonotonicMillis
        | ValueExpr::IoReadLine
        | ValueExpr::FieldAccess { .. } => false,
    }
}

fn expr_uses_env_args(expr: &ValueExpr) -> bool {
    match expr {
        ValueExpr::EnvArgs => true,
        ValueExpr::Binary { left, right, .. }
        | ValueExpr::StringCompare { left, right, .. }
        | ValueExpr::StringConcat { left, right }
        | ValueExpr::StringContains {
            value: left,
            needle: right,
        }
        | ValueExpr::StringStartsWith {
            value: left,
            prefix: right,
        }
        | ValueExpr::StringEndsWith {
            value: left,
            suffix: right,
        }
        | ValueExpr::StringSplit {
            value: left,
            separator: right,
        }
        | ValueExpr::PathJoin { left, right }
        | ValueExpr::NumBinary { left, right, .. }
        | ValueExpr::MathBinary { left, right, .. }
        | ValueExpr::CollectionsStringMapGet {
            map: left,
            key: right,
        }
        | ValueExpr::CollectionsStringMapContains {
            map: left,
            key: right,
        }
        | ValueExpr::CollectionsStringMapRemove {
            map: left,
            key: right,
        }
        | ValueExpr::CollectionsStringSetContains {
            set: left,
            value: right,
        }
        | ValueExpr::CollectionsStringSetInsert {
            set: left,
            value: right,
        }
        | ValueExpr::CollectionsStringSetRemove {
            set: left,
            value: right,
        }
        | ValueExpr::RegexIsMatch {
            regex: left,
            value: right,
        }
        | ValueExpr::RegexCaptures {
            regex: left,
            value: right,
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
        } => expr_uses_env_args(left) || expr_uses_env_args(right),
        ValueExpr::UdpSocketSendToString {
            socket,
            content,
            host,
            port,
        } => {
            expr_uses_env_args(socket)
                || expr_uses_env_args(content)
                || expr_uses_env_args(host)
                || expr_uses_env_args(port)
        }
        ValueExpr::FsReadToString { path }
        | ValueExpr::FsReadBytes { path }
        | ValueExpr::FsOpen { path }
        | ValueExpr::FsExists { path }
        | ValueExpr::FsMetadata { path }
        | ValueExpr::FsCreateDir { path }
        | ValueExpr::FsRemoveDir { path }
        | ValueExpr::FsReadDir { path }
        | ValueExpr::FileClose { file: path }
        | ValueExpr::EnvGet { name: path }
        | ValueExpr::PathBasename { path }
        | ValueExpr::PathDirname { path }
        | ValueExpr::PathExtension { path }
        | ValueExpr::PathNormalize { path }
        | ValueExpr::PathIsAbsolute { path }
        | ValueExpr::MathUnary { value: path, .. }
        | ValueExpr::TimeDurationMillis { millis: path }
        | ValueExpr::TimeDurationSeconds { seconds: path }
        | ValueExpr::TimeDurationAsMillis { duration: path }
        | ValueExpr::TimeFormatDuration { duration: path }
        | ValueExpr::TimeSleep { duration: path }
        | ValueExpr::TimeSleepMillis { duration: path }
        | ValueExpr::LogEnabled { level: path }
        | ValueExpr::HashString { value: path }
        | ValueExpr::HashBytes { value: path }
        | ValueExpr::HashFinish { state: path }
        | ValueExpr::CryptoSha256 { value: path }
        | ValueExpr::CryptoSha512 { value: path }
        | ValueExpr::CryptoRandomBytes { count: path }
        | ValueExpr::JsonParse { value: path }
        | ValueExpr::JsonStringify { value: path }
        | ValueExpr::RegexCompile { pattern: path }
        | ValueExpr::CollectionsStringMapLen { map: path }
        | ValueExpr::CollectionsStringSetLen { set: path }
        | ValueExpr::ProcessExit { code: path }
        | ValueExpr::ProcessSpawn { command: path }
        | ValueExpr::ProcessStatus { command: path }
        | ValueExpr::ProcessExec { command: path }
        | ValueExpr::ProcessOutput { command: path }
        | ValueExpr::NumParseI64 { value: path }
        | ValueExpr::NumParseU64 { value: path }
        | ValueExpr::NumParseF64 { value: path }
        | ValueExpr::NumToString { value: path, .. }
        | ValueExpr::ArrayLen { array: path }
        | ValueExpr::FileReadToString { file: path }
        | ValueExpr::TcpListenerAccept { listener: path }
        | ValueExpr::TcpListenerClose { listener: path }
        | ValueExpr::TcpStreamClose { stream: path }
        | ValueExpr::TcpStreamReadToString { stream: path }
        | ValueExpr::UdpSocketClose { socket: path } => expr_uses_env_args(path),
        ValueExpr::ResultMapErr { result, .. }
        | ValueExpr::ResultIsOk { result, .. }
        | ValueExpr::ResultIsErr { result, .. }
        | ValueExpr::ResultMap { result, .. }
        | ValueExpr::ResultAndThen { result, .. }
        | ValueExpr::OptionIsSome { option: result, .. }
        | ValueExpr::OptionIsNone { option: result, .. }
        | ValueExpr::OptionMap { option: result, .. }
        | ValueExpr::OptionAndThen { option: result, .. } => expr_uses_env_args(result),
        ValueExpr::ResultUnwrapOr {
            result, default, ..
        } => expr_uses_env_args(result) || expr_uses_env_args(default),
        ValueExpr::OptionUnwrapOr {
            option, default, ..
        } => expr_uses_env_args(option) || expr_uses_env_args(default),
        ValueExpr::FsWriteString { path, content } => {
            expr_uses_env_args(path) || expr_uses_env_args(content)
        }
        ValueExpr::FsWriteBytes { path, bytes } => {
            expr_uses_env_args(path) || expr_uses_env_args(bytes)
        }
        ValueExpr::FileWriteString { file, content } => {
            expr_uses_env_args(file) || expr_uses_env_args(content)
        }
        ValueExpr::EnvSet { name, value } => expr_uses_env_args(name) || expr_uses_env_args(value),
        ValueExpr::HashWriteString { state, value } => {
            expr_uses_env_args(state) || expr_uses_env_args(value)
        }
        ValueExpr::HashWriteBytes { state, value } => {
            expr_uses_env_args(state) || expr_uses_env_args(value)
        }
        ValueExpr::CollectionsStringMapSet { map, key, value } => {
            expr_uses_env_args(map) || expr_uses_env_args(key) || expr_uses_env_args(value)
        }
        ValueExpr::ArrayIter { array, .. } => expr_uses_env_args(array),
        ValueExpr::ArrayGet { array, index, .. } => {
            expr_uses_env_args(array) || expr_uses_env_args(index)
        }
        ValueExpr::ArrayPop { .. } | ValueExpr::ArrayClear { .. } => false,
        ValueExpr::ArrayRemove { index, .. } => expr_uses_env_args(index),
        ValueExpr::ArrayPush { value, .. } => expr_uses_env_args(value),
        ValueExpr::ArraySet { index, value, .. } => {
            expr_uses_env_args(index) || expr_uses_env_args(value)
        }
        ValueExpr::ArrayInsert { index, value, .. } => {
            expr_uses_env_args(index) || expr_uses_env_args(value)
        }
        ValueExpr::Call { args, .. } => args.iter().any(expr_uses_env_args),
        ValueExpr::StringLen { value }
        | ValueExpr::StringIsEmpty { value }
        | ValueExpr::StringTrim { value }
        | ValueExpr::StringToLower { value }
        | ValueExpr::StringToUpper { value }
        | ValueExpr::CharIsDigit { value }
        | ValueExpr::CharIsAlpha { value }
        | ValueExpr::CharIsWhitespace { value }
        | ValueExpr::CharToString { value }
        | ValueExpr::Unary { expr: value, .. }
        | ValueExpr::Cast { expr: value, .. }
        | ValueExpr::EnumPayload { value, .. }
        | ValueExpr::EnumPayloadFieldAccess { value, .. } => expr_uses_env_args(value),
        ValueExpr::StructLiteral { fields, .. } => {
            fields.iter().any(|(_, value)| expr_uses_env_args(value))
        }
        ValueExpr::EnumVariant { payload, .. } => {
            payload.as_deref().is_some_and(expr_uses_env_args)
        }
        ValueExpr::If {
            condition,
            then_branch,
            else_branch,
        } => {
            expr_uses_env_args(condition)
                || expr_uses_env_args(then_branch)
                || expr_uses_env_args(else_branch)
        }
        ValueExpr::Panic { message, .. } => expr_uses_env_args(message),
        ValueExpr::Match { value, arms } => {
            expr_uses_env_args(value) || arms.iter().any(|arm| expr_uses_env_args(&arm.value))
        }
        ValueExpr::ArrayNew { .. }
        | ValueExpr::HashNew
        | ValueExpr::CollectionsStringMapNew
        | ValueExpr::CollectionsStringSetNew
        | ValueExpr::StringLiteral(_)
        | ValueExpr::IntLiteral(_)
        | ValueExpr::FloatLiteral(_)
        | ValueExpr::CharLiteral(_)
        | ValueExpr::BoolLiteral(_)
        | ValueExpr::VoidLiteral
        | ValueExpr::Variable(_)
        | ValueExpr::MutBorrow(_)
        | ValueExpr::EnvCwd
        | ValueExpr::EnvHomeDir
        | ValueExpr::EnvTempDir
        | ValueExpr::OsPlatform
        | ValueExpr::OsArch
        | ValueExpr::OsPathSeparator
        | ValueExpr::OsLineEnding
        | ValueExpr::TimeNowMillis
        | ValueExpr::TimeMonotonicMillis
        | ValueExpr::IoReadLine
        | ValueExpr::FieldAccess { .. } => false,
    }
}

fn collect_expr_array_elements(
    expr: &ValueExpr,
    seen: &mut BTreeSet<String>,
    out: &mut Vec<ValueType>,
) {
    match expr {
        ValueExpr::EnvArgs => push_array_element_type(seen, out, &ValueType::String),
        ValueExpr::ArrayIter {
            array,
            element_type,
        } => {
            push_array_element_type(seen, out, element_type);
            collect_expr_array_elements(array, seen, out);
        }
        ValueExpr::ArrayNew { element_type }
        | ValueExpr::ArrayGet { element_type, .. }
        | ValueExpr::ArrayPop { element_type, .. }
        | ValueExpr::ArrayRemove { element_type, .. }
        | ValueExpr::ArrayPush { element_type, .. }
        | ValueExpr::ArraySet { element_type, .. }
        | ValueExpr::ArrayInsert { element_type, .. }
        | ValueExpr::ArrayClear { element_type, .. } => {
            push_array_element_type(seen, out, element_type);
        }
        ValueExpr::ArrayLen { array } => collect_expr_array_elements(array, seen, out),
        ValueExpr::Binary { left, right, .. }
        | ValueExpr::StringCompare { left, right, .. }
        | ValueExpr::StringConcat { left, right }
        | ValueExpr::StringContains {
            value: left,
            needle: right,
        }
        | ValueExpr::StringStartsWith {
            value: left,
            prefix: right,
        }
        | ValueExpr::StringEndsWith {
            value: left,
            suffix: right,
        }
        | ValueExpr::PathJoin { left, right }
        | ValueExpr::NumBinary { left, right, .. }
        | ValueExpr::MathBinary { left, right, .. }
        | ValueExpr::CollectionsStringMapGet {
            map: left,
            key: right,
        }
        | ValueExpr::CollectionsStringMapContains {
            map: left,
            key: right,
        }
        | ValueExpr::CollectionsStringMapRemove {
            map: left,
            key: right,
        }
        | ValueExpr::CollectionsStringSetContains {
            set: left,
            value: right,
        }
        | ValueExpr::CollectionsStringSetInsert {
            set: left,
            value: right,
        }
        | ValueExpr::CollectionsStringSetRemove {
            set: left,
            value: right,
        }
        | ValueExpr::RegexIsMatch {
            regex: left,
            value: right,
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
        } => {
            collect_expr_array_elements(left, seen, out);
            collect_expr_array_elements(right, seen, out);
        }
        ValueExpr::UdpSocketSendToString {
            socket,
            content,
            host,
            port,
        } => {
            collect_expr_array_elements(socket, seen, out);
            collect_expr_array_elements(content, seen, out);
            collect_expr_array_elements(host, seen, out);
            collect_expr_array_elements(port, seen, out);
        }
        ValueExpr::RegexCaptures { regex, value } => {
            push_array_element_type(seen, out, &ValueType::String);
            collect_expr_array_elements(regex, seen, out);
            collect_expr_array_elements(value, seen, out);
        }
        ValueExpr::CollectionsStringMapNew | ValueExpr::CollectionsStringSetNew => {
            push_array_element_type(seen, out, &ValueType::String);
        }
        ValueExpr::CryptoRandomBytes { count } => {
            push_array_element_type(seen, out, &ValueType::U32);
            collect_expr_array_elements(count, seen, out);
        }
        ValueExpr::HashBytes { value } => {
            push_array_element_type(seen, out, &ValueType::U32);
            collect_expr_array_elements(value, seen, out);
        }
        ValueExpr::FsReadBytes { path } => {
            push_array_element_type(seen, out, &ValueType::U32);
            collect_expr_array_elements(path, seen, out);
        }
        ValueExpr::CollectionsStringMapSet { map, key, value } => {
            push_array_element_type(seen, out, &ValueType::String);
            collect_expr_array_elements(map, seen, out);
            collect_expr_array_elements(key, seen, out);
            collect_expr_array_elements(value, seen, out);
        }
        ValueExpr::StringSplit { value, separator } => {
            push_array_element_type(seen, out, &ValueType::String);
            collect_expr_array_elements(value, seen, out);
            collect_expr_array_elements(separator, seen, out);
        }
        ValueExpr::FsReadToString { path }
        | ValueExpr::FsExists { path }
        | ValueExpr::FsMetadata { path }
        | ValueExpr::FsCreateDir { path }
        | ValueExpr::FsRemoveDir { path }
        | ValueExpr::EnvGet { name: path }
        | ValueExpr::PathBasename { path }
        | ValueExpr::PathDirname { path }
        | ValueExpr::PathExtension { path }
        | ValueExpr::PathNormalize { path }
        | ValueExpr::PathIsAbsolute { path }
        | ValueExpr::MathUnary { value: path, .. }
        | ValueExpr::TimeDurationMillis { millis: path }
        | ValueExpr::TimeDurationSeconds { seconds: path }
        | ValueExpr::TimeDurationAsMillis { duration: path }
        | ValueExpr::TimeFormatDuration { duration: path }
        | ValueExpr::TimeSleep { duration: path }
        | ValueExpr::TimeSleepMillis { duration: path }
        | ValueExpr::LogEnabled { level: path }
        | ValueExpr::HashString { value: path }
        | ValueExpr::HashFinish { state: path }
        | ValueExpr::CryptoSha256 { value: path }
        | ValueExpr::CryptoSha512 { value: path }
        | ValueExpr::JsonParse { value: path }
        | ValueExpr::JsonStringify { value: path }
        | ValueExpr::RegexCompile { pattern: path }
        | ValueExpr::CollectionsStringMapLen { map: path }
        | ValueExpr::CollectionsStringSetLen { set: path }
        | ValueExpr::ProcessExit { code: path }
        | ValueExpr::ProcessSpawn { command: path }
        | ValueExpr::ProcessStatus { command: path }
        | ValueExpr::ProcessExec { command: path }
        | ValueExpr::ProcessOutput { command: path }
        | ValueExpr::NumParseI64 { value: path }
        | ValueExpr::NumParseU64 { value: path }
        | ValueExpr::NumParseF64 { value: path }
        | ValueExpr::NumToString { value: path, .. }
        | ValueExpr::TcpListenerAccept { listener: path }
        | ValueExpr::TcpListenerClose { listener: path }
        | ValueExpr::TcpStreamClose { stream: path }
        | ValueExpr::TcpStreamReadToString { stream: path }
        | ValueExpr::UdpSocketClose { socket: path } => {
            collect_expr_array_elements(path, seen, out);
        }
        ValueExpr::FileReadToString { file } => {
            collect_expr_array_elements(file, seen, out);
        }
        ValueExpr::FsReadDir { path } => {
            push_array_element_type(seen, out, &ValueType::String);
            collect_expr_array_elements(path, seen, out);
        }
        ValueExpr::FsOpen { path } | ValueExpr::FileClose { file: path } => {
            collect_expr_array_elements(path, seen, out);
        }
        ValueExpr::HashNew => {}
        ValueExpr::HashWriteString { state, value } => {
            collect_expr_array_elements(state, seen, out);
            collect_expr_array_elements(value, seen, out);
        }
        ValueExpr::HashWriteBytes { state, value } => {
            push_array_element_type(seen, out, &ValueType::U32);
            collect_expr_array_elements(state, seen, out);
            collect_expr_array_elements(value, seen, out);
        }
        ValueExpr::ResultMapErr {
            result,
            ok_type,
            source_err_type,
            target_err_type,
            ..
        } => {
            collect_type_array_elements(ok_type, seen, out);
            collect_type_array_elements(source_err_type, seen, out);
            collect_type_array_elements(target_err_type, seen, out);
            collect_expr_array_elements(result, seen, out);
        }
        ValueExpr::ResultIsOk {
            result,
            ok_type,
            err_type,
        }
        | ValueExpr::ResultIsErr {
            result,
            ok_type,
            err_type,
        } => {
            collect_type_array_elements(ok_type, seen, out);
            collect_type_array_elements(err_type, seen, out);
            collect_expr_array_elements(result, seen, out);
        }
        ValueExpr::ResultUnwrapOr {
            result,
            default,
            ok_type,
            err_type,
        } => {
            collect_type_array_elements(ok_type, seen, out);
            collect_type_array_elements(err_type, seen, out);
            collect_expr_array_elements(result, seen, out);
            collect_expr_array_elements(default, seen, out);
        }
        ValueExpr::ResultMap {
            result,
            source_ok_type,
            target_ok_type,
            err_type,
            ..
        }
        | ValueExpr::ResultAndThen {
            result,
            source_ok_type,
            target_ok_type,
            err_type,
            ..
        } => {
            collect_type_array_elements(source_ok_type, seen, out);
            collect_type_array_elements(target_ok_type, seen, out);
            collect_type_array_elements(err_type, seen, out);
            collect_expr_array_elements(result, seen, out);
        }
        ValueExpr::OptionIsSome {
            option,
            payload_type,
        }
        | ValueExpr::OptionIsNone {
            option,
            payload_type,
        } => {
            collect_type_array_elements(payload_type, seen, out);
            collect_expr_array_elements(option, seen, out);
        }
        ValueExpr::OptionUnwrapOr {
            option,
            default,
            payload_type,
        } => {
            collect_type_array_elements(payload_type, seen, out);
            collect_expr_array_elements(option, seen, out);
            collect_expr_array_elements(default, seen, out);
        }
        ValueExpr::OptionMap {
            option,
            source_type,
            target_type,
            ..
        }
        | ValueExpr::OptionAndThen {
            option,
            source_type,
            target_type,
            ..
        } => {
            collect_type_array_elements(source_type, seen, out);
            collect_type_array_elements(target_type, seen, out);
            collect_expr_array_elements(option, seen, out);
        }
        ValueExpr::FsWriteString { path, content } => {
            collect_expr_array_elements(path, seen, out);
            collect_expr_array_elements(content, seen, out);
        }
        ValueExpr::FsWriteBytes { path, bytes } => {
            collect_expr_array_elements(path, seen, out);
            collect_expr_array_elements(bytes, seen, out);
        }
        ValueExpr::FileWriteString { file, content } => {
            collect_expr_array_elements(file, seen, out);
            collect_expr_array_elements(content, seen, out);
        }
        ValueExpr::EnvSet { name, value } => {
            collect_expr_array_elements(name, seen, out);
            collect_expr_array_elements(value, seen, out);
        }
        ValueExpr::Call { args, .. } => {
            for arg in args {
                collect_expr_array_elements(arg, seen, out);
            }
        }
        ValueExpr::StringLen { value }
        | ValueExpr::StringIsEmpty { value }
        | ValueExpr::StringTrim { value }
        | ValueExpr::StringToLower { value }
        | ValueExpr::StringToUpper { value }
        | ValueExpr::CharIsDigit { value }
        | ValueExpr::CharIsAlpha { value }
        | ValueExpr::CharIsWhitespace { value }
        | ValueExpr::CharToString { value }
        | ValueExpr::Unary { expr: value, .. }
        | ValueExpr::Cast { expr: value, .. }
        | ValueExpr::EnumPayload { value, .. }
        | ValueExpr::EnumPayloadFieldAccess { value, .. } => {
            collect_expr_array_elements(value, seen, out);
        }
        ValueExpr::StructLiteral { fields, .. } => {
            for (_, value) in fields {
                collect_expr_array_elements(value, seen, out);
            }
        }
        ValueExpr::EnumVariant { payload, .. } => {
            if let Some(payload) = payload {
                collect_expr_array_elements(payload, seen, out);
            }
        }
        ValueExpr::If {
            condition,
            then_branch,
            else_branch,
        } => {
            collect_expr_array_elements(condition, seen, out);
            collect_expr_array_elements(then_branch, seen, out);
            collect_expr_array_elements(else_branch, seen, out);
        }
        ValueExpr::Panic { message, .. } => collect_expr_array_elements(message, seen, out),
        ValueExpr::Match { value, arms } => {
            collect_expr_array_elements(value, seen, out);
            for arm in arms {
                collect_expr_array_elements(&arm.value, seen, out);
            }
        }
        ValueExpr::StringLiteral(_)
        | ValueExpr::IntLiteral(_)
        | ValueExpr::FloatLiteral(_)
        | ValueExpr::CharLiteral(_)
        | ValueExpr::BoolLiteral(_)
        | ValueExpr::VoidLiteral
        | ValueExpr::Variable(_)
        | ValueExpr::MutBorrow(_)
        | ValueExpr::EnvCwd
        | ValueExpr::EnvHomeDir
        | ValueExpr::EnvTempDir
        | ValueExpr::OsPlatform
        | ValueExpr::OsArch
        | ValueExpr::OsPathSeparator
        | ValueExpr::OsLineEnding
        | ValueExpr::TimeNowMillis
        | ValueExpr::TimeMonotonicMillis
        | ValueExpr::IoReadLine
        | ValueExpr::FieldAccess { .. } => {}
    }
}

fn push_enum_instance(
    seen: &mut BTreeSet<String>,
    out: &mut Vec<(String, Vec<ValueType>)>,
    name: &str,
    args: &[ValueType],
) {
    let key = format!("{name}{}", c_type_suffix(args));
    if seen.insert(key) {
        out.push((name.to_string(), args.to_vec()));
    }
}

fn push_struct_instance(
    seen: &mut BTreeSet<String>,
    out: &mut Vec<(String, Vec<ValueType>)>,
    name: &str,
    args: &[ValueType],
) {
    let key = format!("{name}{}", c_type_suffix(args));
    if seen.insert(key) {
        out.push((name.to_string(), args.to_vec()));
    }
}

#[cfg(test)]
mod tests;
