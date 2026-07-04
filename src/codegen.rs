use crate::compiler::{
    BinaryOp, DeferredCall, EnumType, Function, LoopKind, MatchStatementArm, MathBinaryFunction,
    MathUnaryFunction, NumBinaryFunction, Program, QuestionCarrier, Statement, StructType, UnaryOp,
    ValueExpr, ValueType,
};
use std::collections::BTreeSet;

const BUILTIN_PRINTLN_EXPR: &str = "__nomo_builtin_println";
const BUILTIN_PRINT_EXPR: &str = "__nomo_builtin_print";
const BUILTIN_EPRINTLN_EXPR: &str = "__nomo_builtin_eprintln";
const BUILTIN_EPRINT_EXPR: &str = "__nomo_builtin_eprint";

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
        "#define _POSIX_C_SOURCE 200809L\n#include <ctype.h>\n#include <errno.h>\n#include <inttypes.h>\n#include <limits.h>\n#include <math.h>\n#include <stdint.h>\n#include <stdio.h>\n#include <stdlib.h>\n#include <string.h>\n#include <sys/stat.h>\n#include <time.h>\n#ifdef _WIN32\n#include <direct.h>\n#include <windows.h>\n#define NOMO_GETCWD _getcwd\n#define NOMO_POPEN _popen\n#define NOMO_PCLOSE _pclose\n#else\n#include <dirent.h>\n#include <sys/time.h>\n#include <sys/wait.h>\n#include <unistd.h>\n#define NOMO_GETCWD getcwd\n#define NOMO_POPEN popen\n#define NOMO_PCLOSE pclose\n#endif\n#ifndef PATH_MAX\n#define PATH_MAX 4096\n#endif\n\n",
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
    if uses_process_status(program) || uses_process_exec(program) || uses_process_output(program) {
        emit_process_common_helpers(&mut out);
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

fn emit_operator_runtime(out: &mut String) {
    out.push_str("static long long nomo_add_i64(long long left, long long right) {\n");
    out.push_str("    if ((right > 0 && left > LLONG_MAX - right) || (right < 0 && left < LLONG_MIN - right)) { nomo_panic(\"signed integer overflow\"); }\n");
    out.push_str("    return left + right;\n");
    out.push_str("}\n\n");
    out.push_str("static long long nomo_sub_i64(long long left, long long right) {\n");
    out.push_str("    if ((right < 0 && left > LLONG_MAX + right) || (right > 0 && left < LLONG_MIN + right)) { nomo_panic(\"signed integer overflow\"); }\n");
    out.push_str("    return left - right;\n");
    out.push_str("}\n\n");
    out.push_str("static long long nomo_mul_i64(long long left, long long right) {\n");
    out.push_str("    if (left == 0 || right == 0) { return 0; }\n");
    out.push_str("    if ((left == -1 && right == LLONG_MIN) || (right == -1 && left == LLONG_MIN)) { nomo_panic(\"signed integer overflow\"); }\n");
    out.push_str("    if (left > 0) {\n");
    out.push_str("        if (right > 0) { if (left > LLONG_MAX / right) { nomo_panic(\"signed integer overflow\"); } }\n");
    out.push_str("        else { if (right < LLONG_MIN / left) { nomo_panic(\"signed integer overflow\"); } }\n");
    out.push_str("    } else {\n");
    out.push_str("        if (right > 0) { if (left < LLONG_MIN / right) { nomo_panic(\"signed integer overflow\"); } }\n");
    out.push_str("        else { if (left < LLONG_MAX / right) { nomo_panic(\"signed integer overflow\"); } }\n");
    out.push_str("    }\n");
    out.push_str("    return left * right;\n");
    out.push_str("}\n\n");
    out.push_str("static long long nomo_div_i64(long long left, long long right) {\n");
    out.push_str("    if (right == 0) { nomo_panic(\"division by zero\"); }\n");
    out.push_str(
        "    if (left == LLONG_MIN && right == -1) { nomo_panic(\"signed integer overflow\"); }\n",
    );
    out.push_str("    return left / right;\n");
    out.push_str("}\n\n");
    out.push_str("static long long nomo_rem_i64(long long left, long long right) {\n");
    out.push_str("    if (right == 0) { nomo_panic(\"division by zero\"); }\n");
    out.push_str(
        "    if (left == LLONG_MIN && right == -1) { nomo_panic(\"signed integer overflow\"); }\n",
    );
    out.push_str("    return left % right;\n");
    out.push_str("}\n\n");
    out.push_str("static int32_t nomo_add_i32(int32_t left, int32_t right) {\n");
    out.push_str("    if ((right > 0 && left > INT32_MAX - right) || (right < 0 && left < INT32_MIN - right)) { nomo_panic(\"signed integer overflow\"); }\n");
    out.push_str("    return left + right;\n");
    out.push_str("}\n\n");
    out.push_str("static int32_t nomo_sub_i32(int32_t left, int32_t right) {\n");
    out.push_str("    if ((right < 0 && left > INT32_MAX + right) || (right > 0 && left < INT32_MIN + right)) { nomo_panic(\"signed integer overflow\"); }\n");
    out.push_str("    return left - right;\n");
    out.push_str("}\n\n");
    out.push_str("static int32_t nomo_mul_i32(int32_t left, int32_t right) {\n");
    out.push_str("    if (left == 0 || right == 0) { return 0; }\n");
    out.push_str("    if ((left == -1 && right == INT32_MIN) || (right == -1 && left == INT32_MIN)) { nomo_panic(\"signed integer overflow\"); }\n");
    out.push_str("    if (left > 0) {\n");
    out.push_str("        if (right > 0) { if (left > INT32_MAX / right) { nomo_panic(\"signed integer overflow\"); } }\n");
    out.push_str("        else { if (right < INT32_MIN / left) { nomo_panic(\"signed integer overflow\"); } }\n");
    out.push_str("    } else {\n");
    out.push_str("        if (right > 0) { if (left < INT32_MIN / right) { nomo_panic(\"signed integer overflow\"); } }\n");
    out.push_str("        else { if (left < INT32_MAX / right) { nomo_panic(\"signed integer overflow\"); } }\n");
    out.push_str("    }\n");
    out.push_str("    return left * right;\n");
    out.push_str("}\n\n");
    out.push_str("static int32_t nomo_div_i32(int32_t left, int32_t right) {\n");
    out.push_str("    if (right == 0) { nomo_panic(\"division by zero\"); }\n");
    out.push_str(
        "    if (left == INT32_MIN && right == -1) { nomo_panic(\"signed integer overflow\"); }\n",
    );
    out.push_str("    return left / right;\n");
    out.push_str("}\n\n");
    out.push_str("static int32_t nomo_rem_i32(int32_t left, int32_t right) {\n");
    out.push_str("    if (right == 0) { nomo_panic(\"division by zero\"); }\n");
    out.push_str(
        "    if (left == INT32_MIN && right == -1) { nomo_panic(\"signed integer overflow\"); }\n",
    );
    out.push_str("    return left % right;\n");
    out.push_str("}\n\n");
    out.push_str("static uint32_t nomo_div_u32(uint32_t left, uint32_t right) {\n");
    out.push_str("    if (right == 0) { nomo_panic(\"division by zero\"); }\n");
    out.push_str("    return left / right;\n");
    out.push_str("}\n\n");
    out.push_str("static uint32_t nomo_rem_u32(uint32_t left, uint32_t right) {\n");
    out.push_str("    if (right == 0) { nomo_panic(\"division by zero\"); }\n");
    out.push_str("    return left % right;\n");
    out.push_str("}\n\n");
    out.push_str("static uint64_t nomo_div_u64(uint64_t left, uint64_t right) {\n");
    out.push_str("    if (right == 0) { nomo_panic(\"division by zero\"); }\n");
    out.push_str("    return left / right;\n");
    out.push_str("}\n\n");
    out.push_str("static uint64_t nomo_rem_u64(uint64_t left, uint64_t right) {\n");
    out.push_str("    if (right == 0) { nomo_panic(\"division by zero\"); }\n");
    out.push_str("    return left % right;\n");
    out.push_str("}\n\n");
    out.push_str("static double nomo_div_f64(double left, double right) {\n");
    out.push_str("    if (right == 0.0) { nomo_panic(\"division by zero\"); }\n");
    out.push_str("    return left / right;\n");
    out.push_str("}\n\n");
    out.push_str("static long long nomo_shl_i64(long long left, uint64_t right) {\n");
    out.push_str(
        "    if (right >= sizeof(left) * CHAR_BIT) { nomo_panic(\"invalid shift amount\"); }\n",
    );
    out.push_str("    return left << right;\n");
    out.push_str("}\n\n");
    out.push_str("static long long nomo_shr_i64(long long left, uint64_t right) {\n");
    out.push_str(
        "    if (right >= sizeof(left) * CHAR_BIT) { nomo_panic(\"invalid shift amount\"); }\n",
    );
    out.push_str("    return left >> right;\n");
    out.push_str("}\n\n");
    out.push_str("static int32_t nomo_shl_i32(int32_t left, uint64_t right) {\n");
    out.push_str(
        "    if (right >= sizeof(left) * CHAR_BIT) { nomo_panic(\"invalid shift amount\"); }\n",
    );
    out.push_str("    return left << right;\n");
    out.push_str("}\n\n");
    out.push_str("static int32_t nomo_shr_i32(int32_t left, uint64_t right) {\n");
    out.push_str(
        "    if (right >= sizeof(left) * CHAR_BIT) { nomo_panic(\"invalid shift amount\"); }\n",
    );
    out.push_str("    return left >> right;\n");
    out.push_str("}\n\n");
    out.push_str("static uint32_t nomo_shl_u32(uint32_t left, uint64_t right) {\n");
    out.push_str(
        "    if (right >= sizeof(left) * CHAR_BIT) { nomo_panic(\"invalid shift amount\"); }\n",
    );
    out.push_str("    return left << right;\n");
    out.push_str("}\n\n");
    out.push_str("static uint32_t nomo_shr_u32(uint32_t left, uint64_t right) {\n");
    out.push_str(
        "    if (right >= sizeof(left) * CHAR_BIT) { nomo_panic(\"invalid shift amount\"); }\n",
    );
    out.push_str("    return left >> right;\n");
    out.push_str("}\n\n");
    out.push_str("static uint64_t nomo_shl_u64(uint64_t left, uint64_t right) {\n");
    out.push_str(
        "    if (right >= sizeof(left) * CHAR_BIT) { nomo_panic(\"invalid shift amount\"); }\n",
    );
    out.push_str("    return left << right;\n");
    out.push_str("}\n\n");
    out.push_str("static uint64_t nomo_shr_u64(uint64_t left, uint64_t right) {\n");
    out.push_str(
        "    if (right >= sizeof(left) * CHAR_BIT) { nomo_panic(\"invalid shift amount\"); }\n",
    );
    out.push_str("    return left >> right;\n");
    out.push_str("}\n\n");
    out.push_str("static long long nomo_wrap_i64(uint64_t bits) {\n");
    out.push_str("    if (bits <= (uint64_t)LLONG_MAX) { return (long long)bits; }\n");
    out.push_str("    return -1 - (long long)(UINT64_MAX - bits);\n");
    out.push_str("}\n\n");
    out.push_str("static int32_t nomo_wrap_i32(uint32_t bits) {\n");
    out.push_str("    if (bits <= (uint32_t)INT32_MAX) { return (int32_t)bits; }\n");
    out.push_str("    return (int32_t)(-1 - (int32_t)(UINT32_MAX - bits));\n");
    out.push_str("}\n\n");
    out.push_str("static long long nomo_num_wrapping_add_i64(long long left, long long right) {\n");
    out.push_str("    return nomo_wrap_i64((uint64_t)left + (uint64_t)right);\n");
    out.push_str("}\n\n");
    out.push_str("static long long nomo_num_wrapping_sub_i64(long long left, long long right) {\n");
    out.push_str("    return nomo_wrap_i64((uint64_t)left - (uint64_t)right);\n");
    out.push_str("}\n\n");
    out.push_str("static long long nomo_num_wrapping_mul_i64(long long left, long long right) {\n");
    out.push_str("    return nomo_wrap_i64((uint64_t)left * (uint64_t)right);\n");
    out.push_str("}\n\n");
    out.push_str("static int32_t nomo_num_wrapping_add_i32(int32_t left, int32_t right) {\n");
    out.push_str("    return nomo_wrap_i32((uint32_t)left + (uint32_t)right);\n");
    out.push_str("}\n\n");
    out.push_str("static int32_t nomo_num_wrapping_sub_i32(int32_t left, int32_t right) {\n");
    out.push_str("    return nomo_wrap_i32((uint32_t)left - (uint32_t)right);\n");
    out.push_str("}\n\n");
    out.push_str("static int32_t nomo_num_wrapping_mul_i32(int32_t left, int32_t right) {\n");
    out.push_str("    return nomo_wrap_i32((uint32_t)left * (uint32_t)right);\n");
    out.push_str("}\n\n");
    out.push_str("static uint32_t nomo_num_wrapping_add_u32(uint32_t left, uint32_t right) {\n");
    out.push_str("    return left + right;\n");
    out.push_str("}\n\n");
    out.push_str("static uint32_t nomo_num_wrapping_sub_u32(uint32_t left, uint32_t right) {\n");
    out.push_str("    return left - right;\n");
    out.push_str("}\n\n");
    out.push_str("static uint32_t nomo_num_wrapping_mul_u32(uint32_t left, uint32_t right) {\n");
    out.push_str("    return left * right;\n");
    out.push_str("}\n\n");
    out.push_str("static uint64_t nomo_num_wrapping_add_u64(uint64_t left, uint64_t right) {\n");
    out.push_str("    return left + right;\n");
    out.push_str("}\n\n");
    out.push_str("static uint64_t nomo_num_wrapping_sub_u64(uint64_t left, uint64_t right) {\n");
    out.push_str("    return left - right;\n");
    out.push_str("}\n\n");
    out.push_str("static uint64_t nomo_num_wrapping_mul_u64(uint64_t left, uint64_t right) {\n");
    out.push_str("    return left * right;\n");
    out.push_str("}\n");
}

fn emit_math_runtime(out: &mut String) {
    out.push_str("static int64_t nomo_math_abs_i64(int64_t value) {\n");
    out.push_str("    if (value == INT64_MIN) { nomo_panic(\"integer overflow\"); }\n");
    out.push_str("    return value < 0 ? -value : value;\n");
    out.push_str("}\n\n");
    out.push_str("static int32_t nomo_math_abs_i32(int32_t value) {\n");
    out.push_str("    if (value == INT32_MIN) { nomo_panic(\"integer overflow\"); }\n");
    out.push_str("    return value < 0 ? -value : value;\n");
    out.push_str("}\n\n");
    out.push_str("static uint32_t nomo_math_abs_u32(uint32_t value) {\n");
    out.push_str("    return value;\n");
    out.push_str("}\n\n");
    out.push_str("static uint64_t nomo_math_abs_u64(uint64_t value) {\n");
    out.push_str("    return value;\n");
    out.push_str("}\n\n");
    out.push_str("static double nomo_math_abs_f64(double value) {\n");
    out.push_str("    return fabs(value);\n");
    out.push_str("}\n\n");
    out.push_str("static int64_t nomo_math_min_i64(int64_t left, int64_t right) {\n");
    out.push_str("    return left < right ? left : right;\n");
    out.push_str("}\n\n");
    out.push_str("static int32_t nomo_math_min_i32(int32_t left, int32_t right) {\n");
    out.push_str("    return left < right ? left : right;\n");
    out.push_str("}\n\n");
    out.push_str("static uint32_t nomo_math_min_u32(uint32_t left, uint32_t right) {\n");
    out.push_str("    return left < right ? left : right;\n");
    out.push_str("}\n\n");
    out.push_str("static uint64_t nomo_math_min_u64(uint64_t left, uint64_t right) {\n");
    out.push_str("    return left < right ? left : right;\n");
    out.push_str("}\n\n");
    out.push_str("static double nomo_math_min_f64(double left, double right) {\n");
    out.push_str("    return left < right ? left : right;\n");
    out.push_str("}\n\n");
    out.push_str("static int64_t nomo_math_max_i64(int64_t left, int64_t right) {\n");
    out.push_str("    return left > right ? left : right;\n");
    out.push_str("}\n\n");
    out.push_str("static int32_t nomo_math_max_i32(int32_t left, int32_t right) {\n");
    out.push_str("    return left > right ? left : right;\n");
    out.push_str("}\n\n");
    out.push_str("static uint32_t nomo_math_max_u32(uint32_t left, uint32_t right) {\n");
    out.push_str("    return left > right ? left : right;\n");
    out.push_str("}\n\n");
    out.push_str("static uint64_t nomo_math_max_u64(uint64_t left, uint64_t right) {\n");
    out.push_str("    return left > right ? left : right;\n");
    out.push_str("}\n\n");
    out.push_str("static double nomo_math_max_f64(double left, double right) {\n");
    out.push_str("    return left > right ? left : right;\n");
    out.push_str("}\n");
}

fn emit_string_runtime(out: &mut String) {
    out.push_str("typedef struct nomo_string {\n");
    out.push_str("    const char *data;\n");
    out.push_str("    size_t *refcount;\n");
    out.push_str("} nomo_string;\n\n");
    out.push_str("static nomo_string nomo_string_literal(const char *data) {\n");
    out.push_str("    return (nomo_string){.data = data, .refcount = NULL};\n");
    out.push_str("}\n\n");
    out.push_str("static nomo_string nomo_string_owned(char *data) {\n");
    out.push_str("    size_t *refcount = (size_t *)malloc(sizeof(size_t));\n");
    out.push_str("    if (refcount == NULL) {\n");
    out.push_str("        free(data);\n");
    out.push_str("        nomo_panic(\"out of memory\");\n");
    out.push_str("    }\n");
    out.push_str("    *refcount = 1;\n");
    out.push_str("    return (nomo_string){.data = data, .refcount = refcount};\n");
    out.push_str("}\n\n");
    out.push_str("static nomo_string nomo_string_from_cstr(const char *value) {\n");
    out.push_str("    size_t len = strlen(value);\n");
    out.push_str("    char *data = (char *)malloc(len + 1);\n");
    out.push_str("    if (data == NULL) { nomo_panic(\"out of memory\"); }\n");
    out.push_str("    memcpy(data, value, len + 1);\n");
    out.push_str("    return nomo_string_owned(data);\n");
    out.push_str("}\n\n");
    out.push_str(
        "static nomo_string nomo_string_from_slice(const char *data, size_t start, size_t len) {\n",
    );
    out.push_str("    char *out = (char *)malloc(len + 1);\n");
    out.push_str("    if (out == NULL) { nomo_panic(\"out of memory\"); }\n");
    out.push_str("    memcpy(out, data + start, len);\n");
    out.push_str("    out[len] = '\\0';\n");
    out.push_str("    return nomo_string_owned(out);\n");
    out.push_str("}\n\n");
    out.push_str("static nomo_string nomo_string_retain(nomo_string value) {\n");
    out.push_str("    if (value.refcount != NULL) { *value.refcount += 1; }\n");
    out.push_str("    return value;\n");
    out.push_str("}\n\n");
    out.push_str("static void nomo_string_release(nomo_string value) {\n");
    out.push_str("    if (value.refcount == NULL) { return; }\n");
    out.push_str("    *value.refcount -= 1;\n");
    out.push_str("    if (*value.refcount != 0) { return; }\n");
    out.push_str("    free((char *)value.data);\n");
    out.push_str("    free(value.refcount);\n");
    out.push_str("}\n\n");
    out.push_str("static nomo_string nomo_string_concat(nomo_string left, nomo_string right) {\n");
    out.push_str("    size_t left_len = strlen(left.data);\n");
    out.push_str("    size_t right_len = strlen(right.data);\n");
    out.push_str("    char *out = (char *)malloc(left_len + right_len + 1);\n");
    out.push_str("    if (out == NULL) {\n");
    out.push_str("        nomo_panic(\"out of memory\");\n");
    out.push_str("    }\n");
    out.push_str("    memcpy(out, left.data, left_len);\n");
    out.push_str("    memcpy(out + left_len, right.data, right_len + 1);\n");
    out.push_str("    return nomo_string_owned(out);\n");
    out.push_str("}\n");
    out.push_str("\nstatic int nomo_string_equal(nomo_string left, nomo_string right) {\n");
    out.push_str("    return strcmp(left.data, right.data) == 0;\n");
    out.push_str("}\n");
    out.push_str("\nstatic int nomo_string_is_empty(nomo_string value) {\n");
    out.push_str("    return value.data[0] == '\\0';\n");
    out.push_str("}\n");
    out.push_str("\nstatic int nomo_string_contains(nomo_string value, nomo_string needle) {\n");
    out.push_str("    return strstr(value.data, needle.data) != NULL;\n");
    out.push_str("}\n");
    out.push_str("\nstatic int nomo_string_starts_with(nomo_string value, nomo_string prefix) {\n");
    out.push_str("    size_t prefix_len = strlen(prefix.data);\n");
    out.push_str("    return strncmp(value.data, prefix.data, prefix_len) == 0;\n");
    out.push_str("}\n");
    out.push_str("\nstatic int nomo_string_ends_with(nomo_string value, nomo_string suffix) {\n");
    out.push_str("    size_t value_len = strlen(value.data);\n");
    out.push_str("    size_t suffix_len = strlen(suffix.data);\n");
    out.push_str("    if (suffix_len > value_len) { return 0; }\n");
    out.push_str(
        "    return memcmp(value.data + value_len - suffix_len, suffix.data, suffix_len) == 0;\n",
    );
    out.push_str("}\n");
    out.push_str("\nstatic nomo_string nomo_string_trim(nomo_string value) {\n");
    out.push_str("    size_t start = 0;\n");
    out.push_str("    size_t end = strlen(value.data);\n");
    out.push_str(
        "    while (start < end && isspace((unsigned char)value.data[start])) { start += 1; }\n",
    );
    out.push_str(
        "    while (end > start && isspace((unsigned char)value.data[end - 1])) { end -= 1; }\n",
    );
    out.push_str("    return nomo_string_from_slice(value.data, start, end - start);\n");
    out.push_str("}\n");
    out.push_str("\nstatic nomo_string nomo_string_to_lower(nomo_string value) {\n");
    out.push_str("    size_t len = strlen(value.data);\n");
    out.push_str("    char *out = (char *)malloc(len + 1);\n");
    out.push_str("    if (out == NULL) { nomo_panic(\"out of memory\"); }\n");
    out.push_str("    for (size_t i = 0; i < len; i += 1) { out[i] = (char)tolower((unsigned char)value.data[i]); }\n");
    out.push_str("    out[len] = '\\0';\n");
    out.push_str("    return nomo_string_owned(out);\n");
    out.push_str("}\n");
    out.push_str("\nstatic nomo_string nomo_string_to_upper(nomo_string value) {\n");
    out.push_str("    size_t len = strlen(value.data);\n");
    out.push_str("    char *out = (char *)malloc(len + 1);\n");
    out.push_str("    if (out == NULL) { nomo_panic(\"out of memory\"); }\n");
    out.push_str("    for (size_t i = 0; i < len; i += 1) { out[i] = (char)toupper((unsigned char)value.data[i]); }\n");
    out.push_str("    out[len] = '\\0';\n");
    out.push_str("    return nomo_string_owned(out);\n");
    out.push_str("}\n");
    out.push_str("\nstatic int nomo_char_is_digit(uint32_t value) {\n");
    out.push_str("    return value <= 127 && isdigit((unsigned char)value) != 0;\n");
    out.push_str("}\n");
    out.push_str("\nstatic int nomo_char_is_alpha(uint32_t value) {\n");
    out.push_str("    return value <= 127 && isalpha((unsigned char)value) != 0;\n");
    out.push_str("}\n");
    out.push_str("\nstatic int nomo_char_is_whitespace(uint32_t value) {\n");
    out.push_str("    return value <= 127 && isspace((unsigned char)value) != 0;\n");
    out.push_str("}\n");
    out.push_str("\nstatic nomo_string nomo_char_to_string(uint32_t value) {\n");
    out.push_str("    char *out = (char *)malloc(5);\n");
    out.push_str("    if (out == NULL) { nomo_panic(\"out of memory\"); }\n");
    out.push_str("    if (value <= 0x7F) {\n");
    out.push_str("        out[0] = (char)value;\n");
    out.push_str("        out[1] = '\\0';\n");
    out.push_str("    } else if (value <= 0x7FF) {\n");
    out.push_str("        out[0] = (char)(0xC0 | (value >> 6));\n");
    out.push_str("        out[1] = (char)(0x80 | (value & 0x3F));\n");
    out.push_str("        out[2] = '\\0';\n");
    out.push_str("    } else if (value <= 0xFFFF) {\n");
    out.push_str("        out[0] = (char)(0xE0 | (value >> 12));\n");
    out.push_str("        out[1] = (char)(0x80 | ((value >> 6) & 0x3F));\n");
    out.push_str("        out[2] = (char)(0x80 | (value & 0x3F));\n");
    out.push_str("        out[3] = '\\0';\n");
    out.push_str("    } else if (value <= 0x10FFFF) {\n");
    out.push_str("        out[0] = (char)(0xF0 | (value >> 18));\n");
    out.push_str("        out[1] = (char)(0x80 | ((value >> 12) & 0x3F));\n");
    out.push_str("        out[2] = (char)(0x80 | ((value >> 6) & 0x3F));\n");
    out.push_str("        out[3] = (char)(0x80 | (value & 0x3F));\n");
    out.push_str("        out[4] = '\\0';\n");
    out.push_str("    } else {\n");
    out.push_str("        out[0] = '?';\n");
    out.push_str("        out[1] = '\\0';\n");
    out.push_str("    }\n");
    out.push_str("    return nomo_string_owned(out);\n");
    out.push_str("}\n");
    out.push_str("\nstatic nomo_string nomo_os_platform(void) {\n");
    out.push_str("#if defined(_WIN32)\n");
    out.push_str("    return nomo_string_literal(\"windows\");\n");
    out.push_str("#elif defined(__APPLE__)\n");
    out.push_str("    return nomo_string_literal(\"macos\");\n");
    out.push_str("#elif defined(__linux__)\n");
    out.push_str("    return nomo_string_literal(\"linux\");\n");
    out.push_str("#elif defined(__FreeBSD__)\n");
    out.push_str("    return nomo_string_literal(\"freebsd\");\n");
    out.push_str("#else\n");
    out.push_str("    return nomo_string_literal(\"unknown\");\n");
    out.push_str("#endif\n");
    out.push_str("}\n");
    out.push_str("\nstatic nomo_string nomo_os_arch(void) {\n");
    out.push_str("#if defined(__aarch64__) || defined(_M_ARM64)\n");
    out.push_str("    return nomo_string_literal(\"aarch64\");\n");
    out.push_str("#elif defined(__x86_64__) || defined(_M_X64)\n");
    out.push_str("    return nomo_string_literal(\"x86_64\");\n");
    out.push_str("#elif defined(__i386__) || defined(_M_IX86)\n");
    out.push_str("    return nomo_string_literal(\"x86\");\n");
    out.push_str("#elif defined(__arm__) || defined(_M_ARM)\n");
    out.push_str("    return nomo_string_literal(\"arm\");\n");
    out.push_str("#else\n");
    out.push_str("    return nomo_string_literal(\"unknown\");\n");
    out.push_str("#endif\n");
    out.push_str("}\n");
    out.push_str("\nstatic nomo_string nomo_os_path_separator(void) {\n");
    out.push_str("#if defined(_WIN32)\n");
    out.push_str("    return nomo_string_literal(\"\\\\\");\n");
    out.push_str("#else\n");
    out.push_str("    return nomo_string_literal(\"/\");\n");
    out.push_str("#endif\n");
    out.push_str("}\n");
    out.push_str("\nstatic nomo_string nomo_os_line_ending(void) {\n");
    out.push_str("#if defined(_WIN32)\n");
    out.push_str("    return nomo_string_literal(\"\\r\\n\");\n");
    out.push_str("#else\n");
    out.push_str("    return nomo_string_literal(\"\\n\");\n");
    out.push_str("#endif\n");
    out.push_str("}\n");
    out.push_str("\nstatic int64_t nomo_time_now_millis(void) {\n");
    out.push_str("#if defined(_WIN32)\n");
    out.push_str("    FILETIME ft;\n");
    out.push_str("    ULARGE_INTEGER value;\n");
    out.push_str("    GetSystemTimeAsFileTime(&ft);\n");
    out.push_str("    value.LowPart = ft.dwLowDateTime;\n");
    out.push_str("    value.HighPart = ft.dwHighDateTime;\n");
    out.push_str("    return (int64_t)((value.QuadPart - 116444736000000000ULL) / 10000ULL);\n");
    out.push_str("#else\n");
    out.push_str("    struct timeval tv;\n");
    out.push_str(
        "    if (gettimeofday(&tv, NULL) != 0) { nomo_panic(\"time.now_millis failed\"); }\n",
    );
    out.push_str("    return ((int64_t)tv.tv_sec * 1000) + ((int64_t)tv.tv_usec / 1000);\n");
    out.push_str("#endif\n");
    out.push_str("}\n");
    out.push_str("\nstatic int64_t nomo_time_monotonic_millis(void) {\n");
    out.push_str("#if defined(_WIN32)\n");
    out.push_str("    return (int64_t)GetTickCount64();\n");
    out.push_str("#else\n");
    out.push_str("    struct timespec ts;\n");
    out.push_str("    if (clock_gettime(CLOCK_MONOTONIC, &ts) != 0) { nomo_panic(\"time.monotonic_millis failed\"); }\n");
    out.push_str("    return ((int64_t)ts.tv_sec * 1000) + ((int64_t)ts.tv_nsec / 1000000);\n");
    out.push_str("#endif\n");
    out.push_str("}\n");
    out.push_str("\nstatic void nomo_time_sleep_millis(int64_t duration) {\n");
    out.push_str("    if (duration < 0) { nomo_panic(\"time.sleep_millis duration must be non-negative\"); }\n");
    out.push_str("#if defined(_WIN32)\n");
    out.push_str("    Sleep((DWORD)duration);\n");
    out.push_str("#else\n");
    out.push_str("    struct timespec request;\n");
    out.push_str("    request.tv_sec = (time_t)(duration / 1000);\n");
    out.push_str("    request.tv_nsec = (long)((duration % 1000) * 1000000);\n");
    out.push_str("    while (nanosleep(&request, &request) != 0) {\n");
    out.push_str("        if (errno != EINTR) { nomo_panic(\"time.sleep_millis failed\"); }\n");
    out.push_str("    }\n");
    out.push_str("#endif\n");
    out.push_str("}\n");
    out.push_str("\nstatic nomo_string nomo_num_i64_to_string(int64_t value) {\n");
    out.push_str("    char buffer[64];\n");
    out.push_str("    snprintf(buffer, sizeof(buffer), \"%\" PRId64, value);\n");
    out.push_str("    return nomo_string_from_cstr(buffer);\n");
    out.push_str("}\n");
    out.push_str("\nstatic nomo_string nomo_num_i32_to_string(int32_t value) {\n");
    out.push_str("    char buffer[32];\n");
    out.push_str("    snprintf(buffer, sizeof(buffer), \"%\" PRId32, value);\n");
    out.push_str("    return nomo_string_from_cstr(buffer);\n");
    out.push_str("}\n");
    out.push_str("\nstatic nomo_string nomo_num_u32_to_string(uint32_t value) {\n");
    out.push_str("    char buffer[32];\n");
    out.push_str("    snprintf(buffer, sizeof(buffer), \"%\" PRIu32, value);\n");
    out.push_str("    return nomo_string_from_cstr(buffer);\n");
    out.push_str("}\n");
    out.push_str("\nstatic nomo_string nomo_num_u64_to_string(uint64_t value) {\n");
    out.push_str("    char buffer[64];\n");
    out.push_str("    snprintf(buffer, sizeof(buffer), \"%\" PRIu64, value);\n");
    out.push_str("    return nomo_string_from_cstr(buffer);\n");
    out.push_str("}\n");
    out.push_str("\nstatic nomo_string nomo_num_f64_to_string(double value) {\n");
    out.push_str("    char buffer[128];\n");
    out.push_str("    snprintf(buffer, sizeof(buffer), \"%.17g\", value);\n");
    out.push_str("    return nomo_string_from_cstr(buffer);\n");
    out.push_str("}\n");
    out.push_str("\nstatic nomo_string nomo_path_string_from_slice(const char *data, size_t start, size_t len) {\n");
    out.push_str("    return nomo_string_from_slice(data, start, len);\n");
    out.push_str("}\n\n");
    out.push_str("static size_t nomo_path_trim_trailing_slashes(const char *data) {\n");
    out.push_str("    size_t len = strlen(data);\n");
    out.push_str("    while (len > 1 && data[len - 1] == '/') { len -= 1; }\n");
    out.push_str("    return len;\n");
    out.push_str("}\n\n");
    out.push_str("static nomo_string nomo_path_join(nomo_string left, nomo_string right) {\n");
    out.push_str("    if (right.data[0] == '/' || left.data[0] == '\\0') {\n");
    out.push_str("        return nomo_string_from_cstr(right.data);\n");
    out.push_str("    }\n");
    out.push_str("    if (right.data[0] == '\\0') {\n");
    out.push_str("        return nomo_string_from_cstr(left.data);\n");
    out.push_str("    }\n");
    out.push_str("    size_t left_len = strlen(left.data);\n");
    out.push_str("    size_t right_len = strlen(right.data);\n");
    out.push_str("    int needs_sep = left.data[left_len - 1] != '/';\n");
    out.push_str("    char *out = (char *)malloc(left_len + (size_t)needs_sep + right_len + 1);\n");
    out.push_str("    if (out == NULL) { nomo_panic(\"out of memory\"); }\n");
    out.push_str("    memcpy(out, left.data, left_len);\n");
    out.push_str("    size_t offset = left_len;\n");
    out.push_str("    if (needs_sep) { out[offset] = '/'; offset += 1; }\n");
    out.push_str("    memcpy(out + offset, right.data, right_len + 1);\n");
    out.push_str("    return nomo_string_owned(out);\n");
    out.push_str("}\n\n");
    out.push_str("static nomo_string nomo_path_basename(nomo_string path) {\n");
    out.push_str("    size_t len = nomo_path_trim_trailing_slashes(path.data);\n");
    out.push_str("    if (len == 0) { return nomo_string_literal(\"\"); }\n");
    out.push_str(
        "    if (len == 1 && path.data[0] == '/') { return nomo_string_literal(\"/\"); }\n",
    );
    out.push_str("    size_t start = len;\n");
    out.push_str("    while (start > 0 && path.data[start - 1] != '/') { start -= 1; }\n");
    out.push_str("    return nomo_path_string_from_slice(path.data, start, len - start);\n");
    out.push_str("}\n\n");
    out.push_str("static nomo_string nomo_path_dirname(nomo_string path) {\n");
    out.push_str("    size_t len = nomo_path_trim_trailing_slashes(path.data);\n");
    out.push_str("    if (len == 0) { return nomo_string_literal(\".\"); }\n");
    out.push_str(
        "    if (len == 1 && path.data[0] == '/') { return nomo_string_literal(\"/\"); }\n",
    );
    out.push_str("    size_t slash = len;\n");
    out.push_str("    while (slash > 0 && path.data[slash - 1] != '/') { slash -= 1; }\n");
    out.push_str("    if (slash == 0) { return nomo_string_literal(\".\"); }\n");
    out.push_str("    while (slash > 1 && path.data[slash - 1] == '/') { slash -= 1; }\n");
    out.push_str("    return nomo_path_string_from_slice(path.data, 0, slash);\n");
    out.push_str("}\n\n");
    out.push_str("static nomo_string nomo_path_extension(nomo_string path) {\n");
    out.push_str("    size_t len = nomo_path_trim_trailing_slashes(path.data);\n");
    out.push_str("    size_t start = len;\n");
    out.push_str("    while (start > 0 && path.data[start - 1] != '/') { start -= 1; }\n");
    out.push_str("    size_t dot = len;\n");
    out.push_str("    while (dot > start && path.data[dot - 1] != '.') { dot -= 1; }\n");
    out.push_str("    if (dot == start || dot == len) { return nomo_string_literal(\"\"); }\n");
    out.push_str("    return nomo_path_string_from_slice(path.data, dot, len - dot);\n");
    out.push_str("}\n\n");
    out.push_str("static int nomo_path_is_absolute(nomo_string path) {\n");
    out.push_str("    return path.data[0] == '/';\n");
    out.push_str("}\n\n");
    out.push_str("static int nomo_path_prev_segment_is_dotdot(char *out, size_t *starts, size_t *lens, size_t count) {\n");
    out.push_str("    if (count == 0) { return 0; }\n");
    out.push_str("    size_t start = starts[count - 1];\n");
    out.push_str("    if (out[start] == '/') { start += 1; }\n");
    out.push_str(
        "    return lens[count - 1] == 2 && out[start] == '.' && out[start + 1] == '.';\n",
    );
    out.push_str("}\n\n");
    out.push_str("static void nomo_path_append_segment(char *out, size_t *out_len, size_t *starts, size_t *lens, size_t *count, const char *segment, size_t segment_len) {\n");
    out.push_str("    size_t restore = *out_len;\n");
    out.push_str("    if (*out_len > 0 && out[*out_len - 1] != '/') { out[*out_len] = '/'; *out_len += 1; }\n");
    out.push_str("    starts[*count] = restore;\n");
    out.push_str("    lens[*count] = segment_len;\n");
    out.push_str("    *count += 1;\n");
    out.push_str("    memcpy(out + *out_len, segment, segment_len);\n");
    out.push_str("    *out_len += segment_len;\n");
    out.push_str("    (void)restore;\n");
    out.push_str("}\n\n");
    out.push_str("static nomo_string nomo_path_normalize(nomo_string path) {\n");
    out.push_str("    const char *data = path.data;\n");
    out.push_str("    size_t len = strlen(data);\n");
    out.push_str("    if (len == 0) { return nomo_string_literal(\".\"); }\n");
    out.push_str("    int absolute = data[0] == '/';\n");
    out.push_str("    char *out = (char *)malloc(len + 2);\n");
    out.push_str("    size_t *starts = (size_t *)malloc((len + 1) * sizeof(size_t));\n");
    out.push_str("    size_t *lens = (size_t *)malloc((len + 1) * sizeof(size_t));\n");
    out.push_str("    if (out == NULL || starts == NULL || lens == NULL) {\n");
    out.push_str("        free(out); free(starts); free(lens);\n");
    out.push_str("        nomo_panic(\"out of memory\");\n");
    out.push_str("    }\n");
    out.push_str("    size_t out_len = 0;\n");
    out.push_str("    size_t count = 0;\n");
    out.push_str("    if (absolute) { out[out_len] = '/'; out_len += 1; }\n");
    out.push_str("    size_t index = 0;\n");
    out.push_str("    while (index < len) {\n");
    out.push_str("        while (index < len && data[index] == '/') { index += 1; }\n");
    out.push_str("        size_t start = index;\n");
    out.push_str("        while (index < len && data[index] != '/') { index += 1; }\n");
    out.push_str("        size_t segment_len = index - start;\n");
    out.push_str(
        "        if (segment_len == 0 || (segment_len == 1 && data[start] == '.')) { continue; }\n",
    );
    out.push_str(
        "        if (segment_len == 2 && data[start] == '.' && data[start + 1] == '.') {\n",
    );
    out.push_str("            if (count > 0 && !nomo_path_prev_segment_is_dotdot(out, starts, lens, count)) {\n");
    out.push_str("                count -= 1;\n");
    out.push_str("                out_len = starts[count];\n");
    out.push_str(
        "                if (absolute && out_len == 0) { out[out_len] = '/'; out_len += 1; }\n",
    );
    out.push_str("            } else if (!absolute) {\n");
    out.push_str("                nomo_path_append_segment(out, &out_len, starts, lens, &count, data + start, segment_len);\n");
    out.push_str("            }\n");
    out.push_str("        } else {\n");
    out.push_str("            nomo_path_append_segment(out, &out_len, starts, lens, &count, data + start, segment_len);\n");
    out.push_str("        }\n");
    out.push_str("    }\n");
    out.push_str("    if (out_len == 0) { out[out_len] = '.'; out_len += 1; }\n");
    out.push_str("    out[out_len] = '\\0';\n");
    out.push_str("    free(starts); free(lens);\n");
    out.push_str("    return nomo_string_owned(out);\n");
    out.push_str("}\n");
}

fn emit_log_enabled_helper(out: &mut String) {
    out.push_str("static int32_t nomo_log_level_value(const char *level) {\n");
    out.push_str("    if (strcmp(level, \"debug\") == 0) { return 0; }\n");
    out.push_str("    if (strcmp(level, \"info\") == 0) { return 1; }\n");
    out.push_str(
        "    if (strcmp(level, \"warn\") == 0 || strcmp(level, \"warning\") == 0) { return 2; }\n",
    );
    out.push_str("    if (strcmp(level, \"error\") == 0) { return 3; }\n");
    out.push_str("    if (strcmp(level, \"off\") == 0) { return 4; }\n");
    out.push_str("    return 1;\n");
    out.push_str("}\n\n");
    out.push_str("static int32_t nomo_log_enabled(nomo_string level) {\n");
    out.push_str("    const char *filter = getenv(\"NOMO_LOG\");\n");
    out.push_str("    int32_t threshold = filter == NULL ? 1 : nomo_log_level_value(filter);\n");
    out.push_str("    int32_t current = nomo_log_level_value(level.data);\n");
    out.push_str("    return threshold < 4 && current >= threshold;\n");
    out.push_str("}\n");
}

fn emit_hash_helpers(out: &mut String) {
    let hash_state = c_type(&ValueType::Struct("HashState".to_string(), Vec::new()));
    let value_field = c_member_ident("value");
    out.push_str("static const uint64_t NOMO_HASH_OFFSET = UINT64_C(14695981039346656037);\n");
    out.push_str("static const uint64_t NOMO_HASH_PRIME = UINT64_C(1099511628211);\n\n");
    out.push_str("static uint64_t nomo_hash_write_bytes(uint64_t state, const char *data) {\n");
    out.push_str("    const unsigned char *bytes = (const unsigned char *)data;\n");
    out.push_str("    while (*bytes != '\\0') {\n");
    out.push_str("        state ^= (uint64_t)(*bytes);\n");
    out.push_str("        state *= NOMO_HASH_PRIME;\n");
    out.push_str("        bytes += 1;\n");
    out.push_str("    }\n");
    out.push_str("    return state;\n");
    out.push_str("}\n\n");
    out.push_str("static ");
    out.push_str(&hash_state);
    out.push_str(" nomo_hash_new(void) {\n");
    out.push_str("    return (");
    out.push_str(&hash_state);
    out.push_str("){.");
    out.push_str(&value_field);
    out.push_str(" = NOMO_HASH_OFFSET};\n");
    out.push_str("}\n\n");
    out.push_str("static uint64_t nomo_hash_string(nomo_string value) {\n");
    out.push_str("    return nomo_hash_write_bytes(NOMO_HASH_OFFSET, value.data);\n");
    out.push_str("}\n\n");
    out.push_str("static ");
    out.push_str(&hash_state);
    out.push_str(" nomo_hash_write_string(");
    out.push_str(&hash_state);
    out.push_str(" state, nomo_string value) {\n");
    out.push_str("    return (");
    out.push_str(&hash_state);
    out.push_str("){.");
    out.push_str(&value_field);
    out.push_str(" = nomo_hash_write_bytes(state.");
    out.push_str(&value_field);
    out.push_str(", value.data)};\n");
    out.push_str("}\n\n");
    out.push_str("static uint64_t nomo_hash_finish(");
    out.push_str(&hash_state);
    out.push_str(" state) {\n");
    out.push_str("    return state.");
    out.push_str(&value_field);
    out.push_str(";\n");
    out.push_str("}\n");
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

fn emit_process_status_helper(out: &mut String) {
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
    out.push_str(" nomo_process_status(nomo_string command) {\n");
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
        Statement::Return(_) | Statement::QuestionReturnOk { .. } | Statement::Panic(_) => true,
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
        | Statement::QuestionReturnOk { .. }
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
        Statement::QuestionReturnOk {
            ok_type,
            result_type,
            return_type,
            result_expr,
        } => emit_question_return_ok(
            out,
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
        | ValueExpr::FsExists { path: expr }
        | ValueExpr::FsMetadata { path: expr }
        | ValueExpr::FsCreateDir { path: expr }
        | ValueExpr::FsRemoveDir { path: expr }
        | ValueExpr::FsReadDir { path: expr }
        | ValueExpr::FsOpen { path: expr }
        | ValueExpr::FileClose { file: expr }
        | ValueExpr::FileReadToString { file: expr }
        | ValueExpr::EnvGet { name: expr }
        | ValueExpr::TimeSleepMillis { duration: expr }
        | ValueExpr::LogEnabled { level: expr }
        | ValueExpr::HashString { value: expr }
        | ValueExpr::HashFinish { state: expr }
        | ValueExpr::ProcessExit { code: expr }
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
        | ValueExpr::HashWriteString {
            state: left,
            value: right,
        }
        | ValueExpr::FileWriteString {
            file: left,
            content: right,
        } => expr_may_share_array_storage(left) || expr_may_share_array_storage(right),
        ValueExpr::StringConcat { .. }
        | ValueExpr::StringIsEmpty { .. }
        | ValueExpr::StringContains { .. }
        | ValueExpr::StringStartsWith { .. }
        | ValueExpr::StringEndsWith { .. }
        | ValueExpr::StringSplit { .. }
        | ValueExpr::StringTrim { .. }
        | ValueExpr::StringToLower { .. }
        | ValueExpr::StringToUpper { .. }
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

fn emit_question_return_ok(
    out: &mut String,
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
    write_indent(out, indent + 1);
    out.push_str("if (nomo__question_result.tag == ");
    out.push_str(&c_enum_variant_ident(result_name, result_args, "Err"));
    out.push_str(") {\n");
    write_indent(out, indent + 2);
    out.push_str(&c_enum_ident(return_name, return_args));
    out.push_str(" nomo__question_return = (");
    out.push_str(&c_enum_ident(return_name, return_args));
    out.push_str("){.tag = ");
    out.push_str(&c_enum_variant_ident(return_name, return_args, "Err"));
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = nomo__question_result.payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str("};\n");
    if expr_may_share_array_storage(result_expr) && value_type_needs_release(return_type) {
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
    out.push_str(&c_payload_ident("Ok"));
    out.push_str(";\n");
    write_indent(out, indent + 1);
    out.push_str(&c_enum_ident(return_name, return_args));
    out.push_str(" nomo__return = (");
    out.push_str(&c_enum_ident(return_name, return_args));
    out.push_str("){.tag = ");
    out.push_str(&c_enum_variant_ident(return_name, return_args, "Ok"));
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Ok"));
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
        ValueExpr::HashWriteString { state, value } => {
            out.push_str("nomo_hash_write_string(");
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
        ValueExpr::ProcessExit { code } => {
            out.push_str("exit((int)");
            emit_expr(out, code);
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

fn emit_match_expr(out: &mut String, value: &ValueExpr, arms: &[crate::compiler::MatchValueArm]) {
    emit_match_arm(out, value, arms, 0);
}

fn emit_match_arm(
    out: &mut String,
    value: &ValueExpr,
    arms: &[crate::compiler::MatchValueArm],
    index: usize,
) {
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

fn collect_result_map_err_instances(program: &Program) -> Vec<ResultMapErrInstance> {
    let mut out = Vec::new();
    for function in &program.functions {
        for statement in &function.body {
            collect_stmt_result_map_err(statement, &mut out);
        }
    }
    out
}

fn collect_result_unwrap_or_instances(program: &Program) -> Vec<ResultUnwrapOrInstance> {
    let mut out = Vec::new();
    walk_program_exprs(program, &mut |expr| {
        if let ValueExpr::ResultUnwrapOr {
            ok_type, err_type, ..
        } = expr
        {
            let instance = ResultUnwrapOrInstance {
                ok_type: ok_type.clone(),
                err_type: err_type.clone(),
            };
            if !out.contains(&instance) {
                out.push(instance);
            }
        }
    });
    out
}

fn collect_result_map_instances(program: &Program) -> Vec<ResultMapInstance> {
    let mut out = Vec::new();
    walk_program_exprs(program, &mut |expr| {
        if let ValueExpr::ResultMap {
            source_ok_type,
            target_ok_type,
            err_type,
            converter,
            ..
        } = expr
        {
            let instance = ResultMapInstance {
                source_ok_type: source_ok_type.clone(),
                target_ok_type: target_ok_type.clone(),
                err_type: err_type.clone(),
                converter: converter.clone(),
            };
            if !out.contains(&instance) {
                out.push(instance);
            }
        }
    });
    out
}

fn collect_result_and_then_instances(program: &Program) -> Vec<ResultAndThenInstance> {
    let mut out = Vec::new();
    walk_program_exprs(program, &mut |expr| {
        if let ValueExpr::ResultAndThen {
            source_ok_type,
            target_ok_type,
            err_type,
            converter,
            ..
        } = expr
        {
            let instance = ResultAndThenInstance {
                source_ok_type: source_ok_type.clone(),
                target_ok_type: target_ok_type.clone(),
                err_type: err_type.clone(),
                converter: converter.clone(),
            };
            if !out.contains(&instance) {
                out.push(instance);
            }
        }
    });
    out
}

fn collect_option_unwrap_or_instances(program: &Program) -> Vec<OptionUnwrapOrInstance> {
    let mut out = Vec::new();
    walk_program_exprs(program, &mut |expr| {
        if let ValueExpr::OptionUnwrapOr { payload_type, .. } = expr {
            let instance = OptionUnwrapOrInstance {
                payload_type: payload_type.clone(),
            };
            if !out.contains(&instance) {
                out.push(instance);
            }
        }
    });
    out
}

fn collect_option_map_instances(program: &Program) -> Vec<OptionMapInstance> {
    let mut out = Vec::new();
    walk_program_exprs(program, &mut |expr| {
        if let ValueExpr::OptionMap {
            source_type,
            target_type,
            converter,
            ..
        } = expr
        {
            let instance = OptionMapInstance {
                source_type: source_type.clone(),
                target_type: target_type.clone(),
                converter: converter.clone(),
            };
            if !out.contains(&instance) {
                out.push(instance);
            }
        }
    });
    out
}

fn collect_option_and_then_instances(program: &Program) -> Vec<OptionAndThenInstance> {
    let mut out = Vec::new();
    walk_program_exprs(program, &mut |expr| {
        if let ValueExpr::OptionAndThen {
            source_type,
            target_type,
            converter,
            ..
        } = expr
        {
            let instance = OptionAndThenInstance {
                source_type: source_type.clone(),
                target_type: target_type.clone(),
                converter: converter.clone(),
            };
            if !out.contains(&instance) {
                out.push(instance);
            }
        }
    });
    out
}

fn collect_num_checked_binary_instances(program: &Program) -> Vec<NumCheckedBinaryInstance> {
    let mut out = Vec::new();
    walk_program_exprs(program, &mut |expr| {
        if let ValueExpr::NumBinary {
            function: NumBinaryFunction::Checked,
            op,
            value_type,
            ..
        } = expr
        {
            let instance = NumCheckedBinaryInstance {
                op: *op,
                value_type: value_type.clone(),
            };
            if !out.contains(&instance) {
                out.push(instance);
            }
        }
    });
    out
}

fn collect_stmt_result_map_err(statement: &Statement, out: &mut Vec<ResultMapErrInstance>) {
    match statement {
        Statement::Let { initializer, .. }
        | Statement::QuestionLet {
            result_expr: initializer,
            ..
        }
        | Statement::QuestionReturnOk {
            result_expr: initializer,
            ..
        }
        | Statement::Assign {
            value: initializer, ..
        }
        | Statement::AssignField {
            value: initializer, ..
        }
        | Statement::Println(initializer)
        | Statement::Print(initializer)
        | Statement::Eprintln(initializer)
        | Statement::Eprint(initializer)
        | Statement::Panic(initializer)
        | Statement::Return(Some(initializer))
        | Statement::Expr(initializer) => collect_expr_result_map_err(initializer, out),
        Statement::LetElse {
            value, else_body, ..
        } => {
            collect_expr_result_map_err(value, out);
            for statement in else_body {
                collect_stmt_result_map_err(statement, out);
            }
        }
        Statement::IfLet {
            value,
            body,
            else_body,
            ..
        } => {
            collect_expr_result_map_err(value, out);
            for statement in body {
                collect_stmt_result_map_err(statement, out);
            }
            if let Some(else_body) = else_body {
                for statement in else_body {
                    collect_stmt_result_map_err(statement, out);
                }
            }
        }
        Statement::If {
            condition,
            body,
            else_body,
        } => {
            collect_expr_result_map_err(condition, out);
            for statement in body {
                collect_stmt_result_map_err(statement, out);
            }
            for statement in else_body {
                collect_stmt_result_map_err(statement, out);
            }
        }
        Statement::LetIf {
            condition,
            body,
            else_body,
            ..
        } => {
            collect_expr_result_map_err(condition, out);
            for statement in body {
                collect_stmt_result_map_err(statement, out);
            }
            for statement in else_body {
                collect_stmt_result_map_err(statement, out);
            }
        }
        Statement::LetMatch { value, arms, .. } => {
            collect_expr_result_map_err(value, out);
            for arm in arms {
                for statement in &arm.body {
                    collect_stmt_result_map_err(statement, out);
                }
            }
        }
        Statement::Loop { kind, body } => {
            match kind {
                LoopKind::Infinite => {}
                LoopKind::While(condition) => collect_expr_result_map_err(condition, out),
                LoopKind::Iterate { iterable, .. } => collect_expr_result_map_err(iterable, out),
            }
            for statement in body {
                collect_stmt_result_map_err(statement, out);
            }
        }
        Statement::Match { value, arms, .. } => {
            collect_expr_result_map_err(value, out);
            for arm in arms {
                for statement in &arm.body {
                    collect_stmt_result_map_err(statement, out);
                }
            }
        }
        Statement::Defer { call } => collect_deferred_result_map_err(call, out),
        Statement::Return(None) | Statement::Break | Statement::Continue => {}
    }
}

fn collect_deferred_result_map_err(call: &DeferredCall, out: &mut Vec<ResultMapErrInstance>) {
    match call {
        DeferredCall::Expr(expr)
        | DeferredCall::Println(expr)
        | DeferredCall::Print(expr)
        | DeferredCall::Eprintln(expr)
        | DeferredCall::Eprint(expr) => {
            collect_expr_result_map_err(expr, out);
        }
    }
}

fn collect_expr_result_map_err(expr: &ValueExpr, out: &mut Vec<ResultMapErrInstance>) {
    match expr {
        ValueExpr::ResultMapErr {
            result,
            ok_type,
            source_err_type,
            target_err_type,
            converter,
        } => {
            collect_expr_result_map_err(result, out);
            let instance = ResultMapErrInstance {
                ok_type: ok_type.clone(),
                source_err_type: source_err_type.clone(),
                target_err_type: target_err_type.clone(),
                converter: converter.clone(),
            };
            if !out.contains(&instance) {
                out.push(instance);
            }
        }
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
        | ValueExpr::MathBinary { left, right, .. } => {
            collect_expr_result_map_err(left, out);
            collect_expr_result_map_err(right, out);
        }
        ValueExpr::FsReadToString { path }
        | ValueExpr::FsExists { path }
        | ValueExpr::FsMetadata { path }
        | ValueExpr::FsCreateDir { path }
        | ValueExpr::FsRemoveDir { path }
        | ValueExpr::FsReadDir { path }
        | ValueExpr::FsOpen { path }
        | ValueExpr::FileClose { file: path }
        | ValueExpr::FileReadToString { file: path }
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
        | ValueExpr::TimeSleepMillis { duration: path }
        | ValueExpr::LogEnabled { level: path }
        | ValueExpr::HashString { value: path }
        | ValueExpr::HashFinish { state: path }
        | ValueExpr::ProcessExit { code: path }
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
        | ValueExpr::ArrayLen { array: path } => collect_expr_result_map_err(path, out),
        ValueExpr::ResultUnwrapOr {
            result, default, ..
        } => {
            collect_expr_result_map_err(result, out);
            collect_expr_result_map_err(default, out);
        }
        ValueExpr::OptionUnwrapOr {
            option, default, ..
        } => {
            collect_expr_result_map_err(option, out);
            collect_expr_result_map_err(default, out);
        }
        ValueExpr::FsWriteString { path, content } => {
            collect_expr_result_map_err(path, out);
            collect_expr_result_map_err(content, out);
        }
        ValueExpr::FileWriteString { file, content } => {
            collect_expr_result_map_err(file, out);
            collect_expr_result_map_err(content, out);
        }
        ValueExpr::EnvSet { name, value } => {
            collect_expr_result_map_err(name, out);
            collect_expr_result_map_err(value, out);
        }
        ValueExpr::HashWriteString { state, value } => {
            collect_expr_result_map_err(state, out);
            collect_expr_result_map_err(value, out);
        }
        ValueExpr::Call { args, .. } => {
            for arg in args {
                collect_expr_result_map_err(arg, out);
            }
        }
        ValueExpr::ArrayGet { array, index, .. } => {
            collect_expr_result_map_err(array, out);
            collect_expr_result_map_err(index, out);
        }
        ValueExpr::ArrayPop { .. } | ValueExpr::ArrayClear { .. } => {}
        ValueExpr::ArrayRemove { index, .. } => {
            collect_expr_result_map_err(index, out);
        }
        ValueExpr::ArrayPush { value, .. } => collect_expr_result_map_err(value, out),
        ValueExpr::ArraySet { index, value, .. } => {
            collect_expr_result_map_err(index, out);
            collect_expr_result_map_err(value, out);
        }
        ValueExpr::ArrayInsert { index, value, .. } => {
            collect_expr_result_map_err(index, out);
            collect_expr_result_map_err(value, out);
        }
        ValueExpr::StructLiteral { fields, .. } => {
            for (_, value) in fields {
                collect_expr_result_map_err(value, out);
            }
        }
        ValueExpr::EnumVariant { payload, .. } => {
            if let Some(payload) = payload {
                collect_expr_result_map_err(payload, out);
            }
        }
        ValueExpr::If {
            condition,
            then_branch,
            else_branch,
        } => {
            collect_expr_result_map_err(condition, out);
            collect_expr_result_map_err(then_branch, out);
            collect_expr_result_map_err(else_branch, out);
        }
        ValueExpr::Panic { message, .. } => collect_expr_result_map_err(message, out),
        ValueExpr::Match { value, arms } => {
            collect_expr_result_map_err(value, out);
            for arm in arms {
                collect_expr_result_map_err(&arm.value, out);
            }
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
        | ValueExpr::EnvArgs
        | ValueExpr::IoReadLine
        | ValueExpr::ArrayNew { .. }
        | ValueExpr::FieldAccess { .. } => {}
    }
}

fn walk_program_exprs<F>(program: &Program, visit: &mut F)
where
    F: FnMut(&ValueExpr),
{
    for function in &program.functions {
        for statement in &function.body {
            walk_stmt_exprs(statement, visit);
        }
    }
}

fn walk_stmt_exprs<F>(statement: &Statement, visit: &mut F)
where
    F: FnMut(&ValueExpr),
{
    match statement {
        Statement::Let { initializer, .. }
        | Statement::QuestionLet {
            result_expr: initializer,
            ..
        }
        | Statement::QuestionReturnOk {
            result_expr: initializer,
            ..
        }
        | Statement::Assign {
            value: initializer, ..
        }
        | Statement::AssignField {
            value: initializer, ..
        }
        | Statement::Println(initializer)
        | Statement::Print(initializer)
        | Statement::Eprintln(initializer)
        | Statement::Eprint(initializer)
        | Statement::Panic(initializer)
        | Statement::Return(Some(initializer))
        | Statement::Expr(initializer) => walk_expr(initializer, visit),
        Statement::LetElse {
            value, else_body, ..
        } => {
            walk_expr(value, visit);
            for statement in else_body {
                walk_stmt_exprs(statement, visit);
            }
        }
        Statement::IfLet {
            value,
            body,
            else_body,
            ..
        } => {
            walk_expr(value, visit);
            for statement in body {
                walk_stmt_exprs(statement, visit);
            }
            if let Some(else_body) = else_body {
                for statement in else_body {
                    walk_stmt_exprs(statement, visit);
                }
            }
        }
        Statement::If {
            condition,
            body,
            else_body,
        } => {
            walk_expr(condition, visit);
            for statement in body {
                walk_stmt_exprs(statement, visit);
            }
            for statement in else_body {
                walk_stmt_exprs(statement, visit);
            }
        }
        Statement::LetIf {
            condition,
            body,
            else_body,
            ..
        } => {
            walk_expr(condition, visit);
            for statement in body {
                walk_stmt_exprs(statement, visit);
            }
            for statement in else_body {
                walk_stmt_exprs(statement, visit);
            }
        }
        Statement::LetMatch { value, arms, .. } | Statement::Match { value, arms, .. } => {
            walk_expr(value, visit);
            for arm in arms {
                for statement in &arm.body {
                    walk_stmt_exprs(statement, visit);
                }
            }
        }
        Statement::Loop { kind, body } => {
            match kind {
                LoopKind::Infinite => {}
                LoopKind::While(condition) => walk_expr(condition, visit),
                LoopKind::Iterate { iterable, .. } => walk_expr(iterable, visit),
            }
            for statement in body {
                walk_stmt_exprs(statement, visit);
            }
        }
        Statement::Defer { call } => walk_deferred_exprs(call, visit),
        Statement::Return(None) | Statement::Break | Statement::Continue => {}
    }
}

fn walk_deferred_exprs<F>(call: &DeferredCall, visit: &mut F)
where
    F: FnMut(&ValueExpr),
{
    match call {
        DeferredCall::Expr(expr)
        | DeferredCall::Println(expr)
        | DeferredCall::Print(expr)
        | DeferredCall::Eprintln(expr)
        | DeferredCall::Eprint(expr) => {
            walk_expr(expr, visit);
        }
    }
}

fn walk_expr<F>(expr: &ValueExpr, visit: &mut F)
where
    F: FnMut(&ValueExpr),
{
    visit(expr);
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
        | ValueExpr::MathBinary { left, right, .. } => {
            walk_expr(left, visit);
            walk_expr(right, visit);
        }
        ValueExpr::FsReadToString { path }
        | ValueExpr::FsExists { path }
        | ValueExpr::FsMetadata { path }
        | ValueExpr::FsCreateDir { path }
        | ValueExpr::FsRemoveDir { path }
        | ValueExpr::FsReadDir { path }
        | ValueExpr::FsOpen { path }
        | ValueExpr::FileClose { file: path }
        | ValueExpr::FileReadToString { file: path }
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
        | ValueExpr::TimeSleepMillis { duration: path }
        | ValueExpr::LogEnabled { level: path }
        | ValueExpr::HashString { value: path }
        | ValueExpr::HashFinish { state: path }
        | ValueExpr::ProcessExit { code: path }
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
        | ValueExpr::ArrayLen { array: path } => walk_expr(path, visit),
        ValueExpr::ResultMapErr { result, .. } => walk_expr(result, visit),
        ValueExpr::ResultUnwrapOr {
            result, default, ..
        } => {
            walk_expr(result, visit);
            walk_expr(default, visit);
        }
        ValueExpr::OptionUnwrapOr {
            option, default, ..
        } => {
            walk_expr(option, visit);
            walk_expr(default, visit);
        }
        ValueExpr::FsWriteString { path, content } => {
            walk_expr(path, visit);
            walk_expr(content, visit);
        }
        ValueExpr::FileWriteString { file, content } => {
            walk_expr(file, visit);
            walk_expr(content, visit);
        }
        ValueExpr::EnvSet { name, value } => {
            walk_expr(name, visit);
            walk_expr(value, visit);
        }
        ValueExpr::HashWriteString { state, value } => {
            walk_expr(state, visit);
            walk_expr(value, visit);
        }
        ValueExpr::Call { args, .. } => {
            for arg in args {
                walk_expr(arg, visit);
            }
        }
        ValueExpr::ArrayGet { array, index, .. } => {
            walk_expr(array, visit);
            walk_expr(index, visit);
        }
        ValueExpr::ArrayPop { .. } | ValueExpr::ArrayClear { .. } => {}
        ValueExpr::ArrayRemove { index, .. } => walk_expr(index, visit),
        ValueExpr::ArrayPush { value, .. } => walk_expr(value, visit),
        ValueExpr::ArraySet { index, value, .. } => {
            walk_expr(index, visit);
            walk_expr(value, visit);
        }
        ValueExpr::ArrayInsert { index, value, .. } => {
            walk_expr(index, visit);
            walk_expr(value, visit);
        }
        ValueExpr::StructLiteral { fields, .. } => {
            for (_, value) in fields {
                walk_expr(value, visit);
            }
        }
        ValueExpr::EnumVariant { payload, .. } => {
            if let Some(payload) = payload {
                walk_expr(payload, visit);
            }
        }
        ValueExpr::If {
            condition,
            then_branch,
            else_branch,
        } => {
            walk_expr(condition, visit);
            walk_expr(then_branch, visit);
            walk_expr(else_branch, visit);
        }
        ValueExpr::Panic { message, .. } => walk_expr(message, visit),
        ValueExpr::Match { value, arms } => {
            walk_expr(value, visit);
            for arm in arms {
                walk_expr(&arm.value, visit);
            }
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
        | ValueExpr::EnvArgs
        | ValueExpr::IoReadLine
        | ValueExpr::ArrayNew { .. }
        | ValueExpr::FieldAccess { .. } => {}
    }
}

fn collect_struct_instances(program: &Program) -> Vec<(String, Vec<ValueType>)> {
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for struct_type in &program.structs {
        if struct_type.type_params.is_empty() {
            push_struct_instance(&mut seen, &mut out, &struct_type.name, &[]);
        }
    }
    for function in &program.functions {
        collect_type_struct(&function.return_type, &mut seen, &mut out);
        for param in &function.params {
            collect_type_struct(&param.value_type, &mut seen, &mut out);
        }
        for statement in &function.body {
            collect_stmt_struct(statement, &mut seen, &mut out);
        }
    }
    out
}

fn collect_stmt_struct(
    statement: &Statement,
    seen: &mut BTreeSet<String>,
    out: &mut Vec<(String, Vec<ValueType>)>,
) {
    match statement {
        Statement::Let {
            value_type,
            initializer,
            ..
        } => {
            collect_type_struct(value_type, seen, out);
            collect_expr_struct(initializer, seen, out);
        }
        Statement::LetIf {
            value_type,
            condition,
            body,
            else_body,
            ..
        } => {
            collect_type_struct(value_type, seen, out);
            collect_expr_struct(condition, seen, out);
            for stmt in body {
                collect_stmt_struct(stmt, seen, out);
            }
            for stmt in else_body {
                collect_stmt_struct(stmt, seen, out);
            }
        }
        Statement::LetMatch {
            value_type,
            value,
            enum_args,
            arms,
            ..
        } => {
            collect_type_struct(value_type, seen, out);
            for arg in enum_args {
                collect_type_struct(arg, seen, out);
            }
            collect_expr_struct(value, seen, out);
            for arm in arms {
                for stmt in &arm.body {
                    collect_stmt_struct(stmt, seen, out);
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
            collect_type_struct(value_type, seen, out);
            collect_type_struct(result_type, seen, out);
            collect_type_struct(return_type, seen, out);
            collect_expr_struct(result_expr, seen, out);
        }
        Statement::QuestionReturnOk {
            ok_type,
            result_type,
            return_type,
            result_expr,
        } => {
            collect_type_struct(ok_type, seen, out);
            collect_type_struct(result_type, seen, out);
            collect_type_struct(return_type, seen, out);
            collect_expr_struct(result_expr, seen, out);
        }
        Statement::LetElse {
            value_type,
            value,
            enum_args,
            else_body,
            ..
        } => {
            collect_type_struct(value_type, seen, out);
            for arg in enum_args {
                collect_type_struct(arg, seen, out);
            }
            collect_expr_struct(value, seen, out);
            for stmt in else_body {
                collect_stmt_struct(stmt, seen, out);
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
                collect_type_struct(value_type, seen, out);
            }
            for arg in enum_args {
                collect_type_struct(arg, seen, out);
            }
            collect_expr_struct(value, seen, out);
            for stmt in body {
                collect_stmt_struct(stmt, seen, out);
            }
            if let Some(else_body) = else_body {
                for stmt in else_body {
                    collect_stmt_struct(stmt, seen, out);
                }
            }
        }
        Statement::If {
            condition,
            body,
            else_body,
        } => {
            collect_expr_struct(condition, seen, out);
            for stmt in body {
                collect_stmt_struct(stmt, seen, out);
            }
            for stmt in else_body {
                collect_stmt_struct(stmt, seen, out);
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
        | Statement::Return(Some(value)) => {
            collect_expr_struct(value, seen, out);
        }
        Statement::Loop { kind, body } => {
            match kind {
                LoopKind::Infinite => {}
                LoopKind::While(condition) => collect_expr_struct(condition, seen, out),
                LoopKind::Iterate {
                    element_type,
                    iterable,
                    ..
                } => {
                    collect_type_struct(element_type, seen, out);
                    collect_expr_struct(iterable, seen, out);
                }
            }
            for stmt in body {
                collect_stmt_struct(stmt, seen, out);
            }
        }
        Statement::Match { value, arms, .. } => {
            collect_expr_struct(value, seen, out);
            for arm in arms {
                for stmt in &arm.body {
                    collect_stmt_struct(stmt, seen, out);
                }
            }
        }
        Statement::Defer { call } => collect_deferred_struct(call, seen, out),
        Statement::Break | Statement::Continue => {}
        Statement::Return(None) => {}
    }
}

fn collect_deferred_struct(
    call: &DeferredCall,
    seen: &mut BTreeSet<String>,
    out: &mut Vec<(String, Vec<ValueType>)>,
) {
    match call {
        DeferredCall::Expr(expr)
        | DeferredCall::Println(expr)
        | DeferredCall::Print(expr)
        | DeferredCall::Eprintln(expr)
        | DeferredCall::Eprint(expr) => {
            collect_expr_struct(expr, seen, out);
        }
    }
}

fn collect_type_struct(
    value_type: &ValueType,
    seen: &mut BTreeSet<String>,
    out: &mut Vec<(String, Vec<ValueType>)>,
) {
    match value_type {
        ValueType::Struct(name, args) => {
            push_struct_instance(seen, out, name, args);
            for arg in args {
                collect_type_struct(arg, seen, out);
            }
        }
        ValueType::Enum(_, args) => {
            for arg in args {
                collect_type_struct(arg, seen, out);
            }
        }
        ValueType::Array(element) => collect_type_struct(element, seen, out),
        _ => {}
    }
}

fn collect_expr_struct(
    expr: &ValueExpr,
    seen: &mut BTreeSet<String>,
    out: &mut Vec<(String, Vec<ValueType>)>,
) {
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
        | ValueExpr::MathBinary { left, right, .. } => {
            collect_expr_struct(left, seen, out);
            collect_expr_struct(right, seen, out);
        }
        ValueExpr::Call { args, .. } => {
            for arg in args {
                collect_expr_struct(arg, seen, out);
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
        | ValueExpr::PathBasename { path: value }
        | ValueExpr::PathDirname { path: value }
        | ValueExpr::PathExtension { path: value }
        | ValueExpr::PathNormalize { path: value }
        | ValueExpr::PathIsAbsolute { path: value }
        | ValueExpr::MathUnary { value, .. }
        | ValueExpr::TimeSleepMillis { duration: value }
        | ValueExpr::LogEnabled { level: value }
        | ValueExpr::HashString { value }
        | ValueExpr::HashFinish { state: value }
        | ValueExpr::ProcessExit { code: value }
        | ValueExpr::Unary { expr: value, .. } => collect_expr_struct(value, seen, out),
        ValueExpr::HashNew => {
            push_struct_instance(seen, out, "HashState", &[]);
        }
        ValueExpr::HashWriteString { state, value } => {
            push_struct_instance(seen, out, "HashState", &[]);
            collect_expr_struct(state, seen, out);
            collect_expr_struct(value, seen, out);
        }
        ValueExpr::ProcessStatus { command } | ValueExpr::ProcessExec { command } => {
            push_struct_instance(seen, out, "ProcessError", &[]);
            collect_expr_struct(command, seen, out);
        }
        ValueExpr::ProcessOutput { command } => {
            push_struct_instance(seen, out, "ProcessError", &[]);
            push_struct_instance(seen, out, "ProcessOutput", &[]);
            collect_expr_struct(command, seen, out);
        }
        ValueExpr::FsReadToString { path } => {
            push_struct_instance(seen, out, "FsError", &[]);
            collect_expr_struct(path, seen, out);
        }
        ValueExpr::FsWriteString { path, content } => {
            push_struct_instance(seen, out, "FsError", &[]);
            collect_expr_struct(path, seen, out);
            collect_expr_struct(content, seen, out);
        }
        ValueExpr::FsExists { path } => collect_expr_struct(path, seen, out),
        ValueExpr::FsMetadata { path } => {
            push_struct_instance(seen, out, "FsError", &[]);
            push_struct_instance(seen, out, "FileMetadata", &[]);
            collect_expr_struct(path, seen, out);
        }
        ValueExpr::FsCreateDir { path }
        | ValueExpr::FsRemoveDir { path }
        | ValueExpr::FsReadDir { path } => {
            push_struct_instance(seen, out, "FsError", &[]);
            collect_expr_struct(path, seen, out);
        }
        ValueExpr::FsOpen { path } => {
            push_struct_instance(seen, out, "FsError", &[]);
            push_struct_instance(seen, out, "File", &[]);
            collect_expr_struct(path, seen, out);
        }
        ValueExpr::FileReadToString { file } => {
            push_struct_instance(seen, out, "FsError", &[]);
            collect_expr_struct(file, seen, out);
        }
        ValueExpr::FileWriteString { file, content } => {
            push_struct_instance(seen, out, "FsError", &[]);
            collect_expr_struct(file, seen, out);
            collect_expr_struct(content, seen, out);
        }
        ValueExpr::IoReadLine => {
            push_struct_instance(seen, out, "IoError", &[]);
        }
        ValueExpr::NumParseI64 { value }
        | ValueExpr::NumParseU64 { value }
        | ValueExpr::NumParseF64 { value } => {
            push_struct_instance(seen, out, "NumError", &[]);
            collect_expr_struct(value, seen, out);
        }
        ValueExpr::NumToString { value, .. } => collect_expr_struct(value, seen, out),
        ValueExpr::FileClose { file } => collect_expr_struct(file, seen, out),
        ValueExpr::ResultMapErr {
            result,
            ok_type,
            source_err_type,
            target_err_type,
            ..
        } => {
            collect_type_struct(ok_type, seen, out);
            collect_type_struct(source_err_type, seen, out);
            collect_type_struct(target_err_type, seen, out);
            collect_expr_struct(result, seen, out);
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
            collect_type_struct(ok_type, seen, out);
            collect_type_struct(err_type, seen, out);
            collect_expr_struct(result, seen, out);
        }
        ValueExpr::ResultUnwrapOr {
            result,
            default,
            ok_type,
            err_type,
        } => {
            collect_type_struct(ok_type, seen, out);
            collect_type_struct(err_type, seen, out);
            collect_expr_struct(result, seen, out);
            collect_expr_struct(default, seen, out);
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
            collect_type_struct(source_ok_type, seen, out);
            collect_type_struct(target_ok_type, seen, out);
            collect_type_struct(err_type, seen, out);
            collect_expr_struct(result, seen, out);
        }
        ValueExpr::OptionIsSome {
            option,
            payload_type,
        }
        | ValueExpr::OptionIsNone {
            option,
            payload_type,
        } => {
            collect_type_struct(payload_type, seen, out);
            collect_expr_struct(option, seen, out);
        }
        ValueExpr::OptionUnwrapOr {
            option,
            default,
            payload_type,
        } => {
            collect_type_struct(payload_type, seen, out);
            collect_expr_struct(option, seen, out);
            collect_expr_struct(default, seen, out);
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
            collect_type_struct(source_type, seen, out);
            collect_type_struct(target_type, seen, out);
            collect_expr_struct(option, seen, out);
        }
        ValueExpr::EnvGet { name } => collect_expr_struct(name, seen, out),
        ValueExpr::EnvSet { name, value } => {
            collect_expr_struct(name, seen, out);
            collect_expr_struct(value, seen, out);
        }
        ValueExpr::EnvCwd | ValueExpr::EnvHomeDir | ValueExpr::EnvTempDir => {}
        ValueExpr::EnvArgs => {}
        ValueExpr::ArrayNew { element_type } => collect_type_struct(element_type, seen, out),
        ValueExpr::ArrayLen { array } => collect_expr_struct(array, seen, out),
        ValueExpr::ArrayIter {
            array,
            element_type,
        } => {
            collect_type_struct(element_type, seen, out);
            collect_expr_struct(array, seen, out);
        }
        ValueExpr::ArrayGet {
            array,
            index,
            element_type,
        } => {
            collect_type_struct(element_type, seen, out);
            collect_expr_struct(array, seen, out);
            collect_expr_struct(index, seen, out);
        }
        ValueExpr::ArrayPop { element_type, .. } | ValueExpr::ArrayClear { element_type, .. } => {
            collect_type_struct(element_type, seen, out)
        }
        ValueExpr::ArrayRemove {
            index,
            element_type,
            ..
        } => {
            collect_type_struct(element_type, seen, out);
            collect_expr_struct(index, seen, out);
        }
        ValueExpr::ArrayPush {
            value,
            element_type,
            ..
        } => {
            collect_type_struct(element_type, seen, out);
            collect_expr_struct(value, seen, out);
        }
        ValueExpr::ArraySet {
            index,
            value,
            element_type,
            ..
        } => {
            collect_type_struct(element_type, seen, out);
            collect_expr_struct(index, seen, out);
            collect_expr_struct(value, seen, out);
        }
        ValueExpr::ArrayInsert {
            index,
            value,
            element_type,
            ..
        } => {
            collect_type_struct(element_type, seen, out);
            collect_expr_struct(index, seen, out);
            collect_expr_struct(value, seen, out);
        }
        ValueExpr::Cast { expr, target_type } => {
            collect_type_struct(target_type, seen, out);
            collect_expr_struct(expr, seen, out);
        }
        ValueExpr::StructLiteral {
            type_name,
            struct_args,
            fields,
        } => {
            push_struct_instance(seen, out, type_name, struct_args);
            for arg in struct_args {
                collect_type_struct(arg, seen, out);
            }
            for (_, value) in fields {
                collect_expr_struct(value, seen, out);
            }
        }
        ValueExpr::EnumVariant {
            enum_args, payload, ..
        } => {
            for arg in enum_args {
                collect_type_struct(arg, seen, out);
            }
            if let Some(payload) = payload {
                collect_expr_struct(payload, seen, out);
            }
        }
        ValueExpr::EnumPayload { value, .. } => collect_expr_struct(value, seen, out),
        ValueExpr::EnumPayloadFieldAccess { value, .. } => collect_expr_struct(value, seen, out),
        ValueExpr::If {
            condition,
            then_branch,
            else_branch,
        } => {
            collect_expr_struct(condition, seen, out);
            collect_expr_struct(then_branch, seen, out);
            collect_expr_struct(else_branch, seen, out);
        }
        ValueExpr::Panic {
            message,
            fallback_type,
        } => {
            collect_type_struct(fallback_type, seen, out);
            collect_expr_struct(message, seen, out);
        }
        ValueExpr::Match { value, arms } => {
            collect_expr_struct(value, seen, out);
            for arm in arms {
                for arg in &arm.enum_args {
                    collect_type_struct(arg, seen, out);
                }
                collect_expr_struct(&arm.value, seen, out);
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
        | ValueExpr::OsPlatform
        | ValueExpr::OsArch
        | ValueExpr::OsPathSeparator
        | ValueExpr::OsLineEnding
        | ValueExpr::TimeNowMillis
        | ValueExpr::TimeMonotonicMillis
        | ValueExpr::FieldAccess { .. } => {}
    }
}

fn collect_enum_instances(program: &Program) -> Vec<(String, Vec<ValueType>)> {
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for enum_type in &program.enums {
        if enum_type.type_params.is_empty() {
            push_enum_instance(&mut seen, &mut out, &enum_type.name, &[]);
        }
    }
    for function in &program.functions {
        collect_type_enum(&function.return_type, &mut seen, &mut out);
        for param in &function.params {
            collect_type_enum(&param.value_type, &mut seen, &mut out);
        }
        for statement in &function.body {
            collect_stmt_enum(statement, &mut seen, &mut out);
        }
    }
    for element_type in collect_array_element_types(program) {
        push_enum_instance(&mut seen, &mut out, "Option", &[element_type]);
    }
    out
}

fn collect_stmt_enum(
    statement: &Statement,
    seen: &mut BTreeSet<String>,
    out: &mut Vec<(String, Vec<ValueType>)>,
) {
    match statement {
        Statement::Let {
            value_type,
            initializer,
            ..
        } => {
            collect_type_enum(value_type, seen, out);
            collect_expr_enum(initializer, seen, out);
        }
        Statement::LetIf {
            value_type,
            condition,
            body,
            else_body,
            ..
        } => {
            collect_type_enum(value_type, seen, out);
            collect_expr_enum(condition, seen, out);
            for stmt in body {
                collect_stmt_enum(stmt, seen, out);
            }
            for stmt in else_body {
                collect_stmt_enum(stmt, seen, out);
            }
        }
        Statement::LetMatch {
            value_type,
            value,
            enum_name,
            enum_args,
            arms,
            ..
        } => {
            push_enum_instance(seen, out, enum_name, enum_args);
            collect_type_enum(value_type, seen, out);
            for arg in enum_args {
                collect_type_enum(arg, seen, out);
            }
            collect_expr_enum(value, seen, out);
            for arm in arms {
                for stmt in &arm.body {
                    collect_stmt_enum(stmt, seen, out);
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
            collect_type_enum(value_type, seen, out);
            collect_type_enum(result_type, seen, out);
            collect_type_enum(return_type, seen, out);
            collect_expr_enum(result_expr, seen, out);
        }
        Statement::QuestionReturnOk {
            ok_type,
            result_type,
            return_type,
            result_expr,
        } => {
            collect_type_enum(ok_type, seen, out);
            collect_type_enum(result_type, seen, out);
            collect_type_enum(return_type, seen, out);
            collect_expr_enum(result_expr, seen, out);
        }
        Statement::LetElse {
            value_type,
            value,
            enum_name,
            enum_args,
            else_body,
            ..
        } => {
            push_enum_instance(seen, out, enum_name, enum_args);
            collect_type_enum(value_type, seen, out);
            for arg in enum_args {
                collect_type_enum(arg, seen, out);
            }
            collect_expr_enum(value, seen, out);
            for stmt in else_body {
                collect_stmt_enum(stmt, seen, out);
            }
        }
        Statement::IfLet {
            value_type,
            value,
            enum_name,
            enum_args,
            body,
            else_body,
            ..
        } => {
            push_enum_instance(seen, out, enum_name, enum_args);
            if let Some(value_type) = value_type {
                collect_type_enum(value_type, seen, out);
            }
            for arg in enum_args {
                collect_type_enum(arg, seen, out);
            }
            collect_expr_enum(value, seen, out);
            for stmt in body {
                collect_stmt_enum(stmt, seen, out);
            }
            if let Some(else_body) = else_body {
                for stmt in else_body {
                    collect_stmt_enum(stmt, seen, out);
                }
            }
        }
        Statement::If {
            condition,
            body,
            else_body,
        } => {
            collect_expr_enum(condition, seen, out);
            for stmt in body {
                collect_stmt_enum(stmt, seen, out);
            }
            for stmt in else_body {
                collect_stmt_enum(stmt, seen, out);
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
        | Statement::Return(Some(value)) => {
            collect_expr_enum(value, seen, out);
        }
        Statement::Loop { kind, body } => {
            match kind {
                LoopKind::Infinite => {}
                LoopKind::While(condition) => collect_expr_enum(condition, seen, out),
                LoopKind::Iterate {
                    element_type,
                    iterable,
                    ..
                } => {
                    collect_type_enum(element_type, seen, out);
                    collect_expr_enum(iterable, seen, out);
                }
            }
            for stmt in body {
                collect_stmt_enum(stmt, seen, out);
            }
        }
        Statement::Match {
            value,
            enum_name,
            enum_args,
            arms,
        } => {
            push_enum_instance(seen, out, enum_name, enum_args);
            collect_expr_enum(value, seen, out);
            for arm in arms {
                for stmt in &arm.body {
                    collect_stmt_enum(stmt, seen, out);
                }
            }
        }
        Statement::Defer { call } => collect_deferred_enum(call, seen, out),
        Statement::Break | Statement::Continue => {}
        Statement::Return(None) => {}
    }
}

fn collect_deferred_enum(
    call: &DeferredCall,
    seen: &mut BTreeSet<String>,
    out: &mut Vec<(String, Vec<ValueType>)>,
) {
    match call {
        DeferredCall::Expr(expr)
        | DeferredCall::Println(expr)
        | DeferredCall::Print(expr)
        | DeferredCall::Eprintln(expr)
        | DeferredCall::Eprint(expr) => {
            collect_expr_enum(expr, seen, out);
        }
    }
}

fn collect_type_enum(
    value_type: &ValueType,
    seen: &mut BTreeSet<String>,
    out: &mut Vec<(String, Vec<ValueType>)>,
) {
    match value_type {
        ValueType::Enum(name, args) => {
            push_enum_instance(seen, out, name, args);
            for arg in args {
                collect_type_enum(arg, seen, out);
            }
        }
        ValueType::Array(element) => collect_type_enum(element, seen, out),
        ValueType::Never => {}
        _ => {}
    }
}

fn collect_expr_enum(
    expr: &ValueExpr,
    seen: &mut BTreeSet<String>,
    out: &mut Vec<(String, Vec<ValueType>)>,
) {
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
        | ValueExpr::MathBinary { left, right, .. } => {
            collect_expr_enum(left, seen, out);
            collect_expr_enum(right, seen, out);
        }
        ValueExpr::NumBinary {
            function,
            left,
            right,
            value_type,
            ..
        } => {
            if function == &NumBinaryFunction::Checked {
                push_enum_instance(seen, out, "Option", std::slice::from_ref(value_type));
                collect_type_enum(value_type, seen, out);
            }
            collect_expr_enum(left, seen, out);
            collect_expr_enum(right, seen, out);
        }
        ValueExpr::Call { args, .. } => {
            for arg in args {
                collect_expr_enum(arg, seen, out);
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
        | ValueExpr::PathBasename { path: value }
        | ValueExpr::PathDirname { path: value }
        | ValueExpr::PathExtension { path: value }
        | ValueExpr::PathNormalize { path: value }
        | ValueExpr::PathIsAbsolute { path: value }
        | ValueExpr::MathUnary { value, .. }
        | ValueExpr::TimeSleepMillis { duration: value }
        | ValueExpr::LogEnabled { level: value }
        | ValueExpr::HashString { value }
        | ValueExpr::HashFinish { state: value }
        | ValueExpr::ProcessExit { code: value }
        | ValueExpr::Unary { expr: value, .. } => collect_expr_enum(value, seen, out),
        ValueExpr::HashNew => {}
        ValueExpr::HashWriteString { state, value } => {
            collect_expr_enum(state, seen, out);
            collect_expr_enum(value, seen, out);
        }
        ValueExpr::ProcessStatus { command } => {
            push_enum_instance(
                seen,
                out,
                "Result",
                &[
                    ValueType::I32,
                    ValueType::Struct("ProcessError".to_string(), Vec::new()),
                ],
            );
            collect_expr_enum(command, seen, out);
        }
        ValueExpr::ProcessExec { command } => {
            push_enum_instance(
                seen,
                out,
                "Result",
                &[
                    ValueType::String,
                    ValueType::Struct("ProcessError".to_string(), Vec::new()),
                ],
            );
            collect_expr_enum(command, seen, out);
        }
        ValueExpr::ProcessOutput { command } => {
            push_enum_instance(
                seen,
                out,
                "Result",
                &[
                    ValueType::Struct("ProcessOutput".to_string(), Vec::new()),
                    ValueType::Struct("ProcessError".to_string(), Vec::new()),
                ],
            );
            collect_expr_enum(command, seen, out);
        }
        ValueExpr::FsReadToString { path } => {
            push_enum_instance(
                seen,
                out,
                "Result",
                &[
                    ValueType::String,
                    ValueType::Struct("FsError".to_string(), Vec::new()),
                ],
            );
            collect_expr_enum(path, seen, out);
        }
        ValueExpr::FsWriteString { path, content } => {
            push_enum_instance(
                seen,
                out,
                "Result",
                &[
                    ValueType::Void,
                    ValueType::Struct("FsError".to_string(), Vec::new()),
                ],
            );
            collect_expr_enum(path, seen, out);
            collect_expr_enum(content, seen, out);
        }
        ValueExpr::FsExists { path } => collect_expr_enum(path, seen, out),
        ValueExpr::FsMetadata { path } => {
            push_enum_instance(
                seen,
                out,
                "Result",
                &[
                    ValueType::Struct("FileMetadata".to_string(), Vec::new()),
                    ValueType::Struct("FsError".to_string(), Vec::new()),
                ],
            );
            collect_expr_enum(path, seen, out);
        }
        ValueExpr::FsCreateDir { path } | ValueExpr::FsRemoveDir { path } => {
            push_enum_instance(
                seen,
                out,
                "Result",
                &[
                    ValueType::Void,
                    ValueType::Struct("FsError".to_string(), Vec::new()),
                ],
            );
            collect_expr_enum(path, seen, out);
        }
        ValueExpr::FsReadDir { path } => {
            push_enum_instance(
                seen,
                out,
                "Result",
                &[
                    ValueType::Array(Box::new(ValueType::String)),
                    ValueType::Struct("FsError".to_string(), Vec::new()),
                ],
            );
            collect_expr_enum(path, seen, out);
        }
        ValueExpr::FsOpen { path } => {
            push_enum_instance(
                seen,
                out,
                "Result",
                &[
                    ValueType::Struct("File".to_string(), Vec::new()),
                    ValueType::Struct("FsError".to_string(), Vec::new()),
                ],
            );
            collect_expr_enum(path, seen, out);
        }
        ValueExpr::FileReadToString { file } => {
            push_enum_instance(
                seen,
                out,
                "Result",
                &[
                    ValueType::String,
                    ValueType::Struct("FsError".to_string(), Vec::new()),
                ],
            );
            collect_expr_enum(file, seen, out);
        }
        ValueExpr::FileWriteString { file, content } => {
            push_enum_instance(
                seen,
                out,
                "Result",
                &[
                    ValueType::Void,
                    ValueType::Struct("FsError".to_string(), Vec::new()),
                ],
            );
            collect_expr_enum(file, seen, out);
            collect_expr_enum(content, seen, out);
        }
        ValueExpr::FileClose { file } => collect_expr_enum(file, seen, out),
        ValueExpr::ResultMapErr {
            result,
            ok_type,
            source_err_type,
            target_err_type,
            ..
        } => {
            push_enum_instance(
                seen,
                out,
                "Result",
                &[ok_type.clone(), source_err_type.clone()],
            );
            push_enum_instance(
                seen,
                out,
                "Result",
                &[ok_type.clone(), target_err_type.clone()],
            );
            collect_type_enum(ok_type, seen, out);
            collect_type_enum(source_err_type, seen, out);
            collect_type_enum(target_err_type, seen, out);
            collect_expr_enum(result, seen, out);
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
            push_enum_instance(seen, out, "Result", &[ok_type.clone(), err_type.clone()]);
            collect_type_enum(ok_type, seen, out);
            collect_type_enum(err_type, seen, out);
            collect_expr_enum(result, seen, out);
        }
        ValueExpr::ResultUnwrapOr {
            result,
            default,
            ok_type,
            err_type,
        } => {
            push_enum_instance(seen, out, "Result", &[ok_type.clone(), err_type.clone()]);
            collect_type_enum(ok_type, seen, out);
            collect_type_enum(err_type, seen, out);
            collect_expr_enum(result, seen, out);
            collect_expr_enum(default, seen, out);
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
            push_enum_instance(
                seen,
                out,
                "Result",
                &[source_ok_type.clone(), err_type.clone()],
            );
            push_enum_instance(
                seen,
                out,
                "Result",
                &[target_ok_type.clone(), err_type.clone()],
            );
            collect_type_enum(source_ok_type, seen, out);
            collect_type_enum(target_ok_type, seen, out);
            collect_type_enum(err_type, seen, out);
            collect_expr_enum(result, seen, out);
        }
        ValueExpr::OptionIsSome {
            option,
            payload_type,
        }
        | ValueExpr::OptionIsNone {
            option,
            payload_type,
        } => {
            push_enum_instance(seen, out, "Option", &[payload_type.clone()]);
            collect_type_enum(payload_type, seen, out);
            collect_expr_enum(option, seen, out);
        }
        ValueExpr::OptionUnwrapOr {
            option,
            default,
            payload_type,
        } => {
            push_enum_instance(seen, out, "Option", &[payload_type.clone()]);
            collect_type_enum(payload_type, seen, out);
            collect_expr_enum(option, seen, out);
            collect_expr_enum(default, seen, out);
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
            push_enum_instance(seen, out, "Option", &[source_type.clone()]);
            push_enum_instance(seen, out, "Option", &[target_type.clone()]);
            collect_type_enum(source_type, seen, out);
            collect_type_enum(target_type, seen, out);
            collect_expr_enum(option, seen, out);
        }
        ValueExpr::EnvGet { name } => {
            push_enum_instance(seen, out, "Option", &[ValueType::String]);
            collect_expr_enum(name, seen, out);
        }
        ValueExpr::EnvSet { name, value } => {
            collect_expr_enum(name, seen, out);
            collect_expr_enum(value, seen, out);
        }
        ValueExpr::EnvHomeDir => {
            push_enum_instance(seen, out, "Option", &[ValueType::String]);
        }
        ValueExpr::EnvCwd | ValueExpr::EnvTempDir => {}
        ValueExpr::EnvArgs => {}
        ValueExpr::IoReadLine => {
            push_enum_instance(
                seen,
                out,
                "Result",
                &[
                    ValueType::String,
                    ValueType::Struct("IoError".to_string(), Vec::new()),
                ],
            );
        }
        ValueExpr::NumParseI64 { value } => {
            push_enum_instance(
                seen,
                out,
                "Result",
                &[
                    ValueType::Int,
                    ValueType::Struct("NumError".to_string(), Vec::new()),
                ],
            );
            collect_expr_enum(value, seen, out);
        }
        ValueExpr::NumParseU64 { value } => {
            push_enum_instance(
                seen,
                out,
                "Result",
                &[
                    ValueType::U64,
                    ValueType::Struct("NumError".to_string(), Vec::new()),
                ],
            );
            collect_expr_enum(value, seen, out);
        }
        ValueExpr::NumParseF64 { value } => {
            push_enum_instance(
                seen,
                out,
                "Result",
                &[
                    ValueType::Float,
                    ValueType::Struct("NumError".to_string(), Vec::new()),
                ],
            );
            collect_expr_enum(value, seen, out);
        }
        ValueExpr::NumToString { value, .. } => collect_expr_enum(value, seen, out),
        ValueExpr::ArrayNew { .. } => {}
        ValueExpr::ArrayLen { array } => collect_expr_enum(array, seen, out),
        ValueExpr::ArrayIter {
            array,
            element_type,
        } => {
            collect_type_enum(element_type, seen, out);
            collect_expr_enum(array, seen, out);
        }
        ValueExpr::ArrayGet {
            array,
            index,
            element_type,
        } => {
            push_enum_instance(seen, out, "Option", &[element_type.clone()]);
            collect_expr_enum(array, seen, out);
            collect_expr_enum(index, seen, out);
        }
        ValueExpr::ArrayPop { element_type, .. } => {
            push_enum_instance(seen, out, "Option", &[element_type.clone()]);
        }
        ValueExpr::ArrayRemove {
            index,
            element_type,
            ..
        } => {
            push_enum_instance(seen, out, "Option", &[element_type.clone()]);
            collect_expr_enum(index, seen, out);
        }
        ValueExpr::ArrayClear { .. } => {}
        ValueExpr::ArrayPush { value, .. } => collect_expr_enum(value, seen, out),
        ValueExpr::ArraySet { index, value, .. } => {
            collect_expr_enum(index, seen, out);
            collect_expr_enum(value, seen, out);
        }
        ValueExpr::ArrayInsert { index, value, .. } => {
            collect_expr_enum(index, seen, out);
            collect_expr_enum(value, seen, out);
        }
        ValueExpr::Cast { expr, .. } => collect_expr_enum(expr, seen, out),
        ValueExpr::StructLiteral { fields, .. } => {
            for (_, value) in fields {
                collect_expr_enum(value, seen, out);
            }
        }
        ValueExpr::EnumVariant {
            enum_name,
            enum_args,
            payload,
            ..
        } => {
            push_enum_instance(seen, out, enum_name, enum_args);
            if let Some(payload) = payload {
                collect_expr_enum(payload, seen, out);
            }
        }
        ValueExpr::EnumPayload { value, .. } => collect_expr_enum(value, seen, out),
        ValueExpr::EnumPayloadFieldAccess { value, .. } => collect_expr_enum(value, seen, out),
        ValueExpr::If {
            condition,
            then_branch,
            else_branch,
        } => {
            collect_expr_enum(condition, seen, out);
            collect_expr_enum(then_branch, seen, out);
            collect_expr_enum(else_branch, seen, out);
        }
        ValueExpr::Panic { message, .. } => collect_expr_enum(message, seen, out),
        ValueExpr::Match { value, arms } => {
            collect_expr_enum(value, seen, out);
            for arm in arms {
                push_enum_instance(seen, out, &arm.enum_name, &arm.enum_args);
                collect_expr_enum(&arm.value, seen, out);
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
        | ValueExpr::OsPlatform
        | ValueExpr::OsArch
        | ValueExpr::OsPathSeparator
        | ValueExpr::OsLineEnding
        | ValueExpr::TimeNowMillis
        | ValueExpr::TimeMonotonicMillis
        | ValueExpr::FieldAccess { .. } => {}
    }
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
        Statement::QuestionReturnOk { result_expr, .. } => expr_uses_fs_read_to_string(result_expr),
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
        Statement::QuestionReturnOk { result_expr, .. } => expr_uses_fs_write_string(result_expr),
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
        Statement::QuestionReturnOk { result_expr, .. } => expr_uses_fs_open(result_expr),
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
        | Statement::QuestionReturnOk { result_expr, .. } => expr_contains(result_expr, predicate),
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
        | ValueExpr::MathBinary { left, right, .. } => {
            expr_contains(left, predicate) || expr_contains(right, predicate)
        }
        ValueExpr::FsWriteString { path, content } => {
            expr_contains(path, predicate) || expr_contains(content, predicate)
        }
        ValueExpr::EnvSet { name, value } => {
            expr_contains(name, predicate) || expr_contains(value, predicate)
        }
        ValueExpr::HashWriteString { state, value } => {
            expr_contains(state, predicate) || expr_contains(value, predicate)
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
        | ValueExpr::FsOpen { path }
        | ValueExpr::FsExists { path }
        | ValueExpr::FsMetadata { path }
        | ValueExpr::FsCreateDir { path }
        | ValueExpr::FsRemoveDir { path }
        | ValueExpr::FsReadDir { path }
        | ValueExpr::FileClose { file: path }
        | ValueExpr::FileReadToString { file: path }
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
        | ValueExpr::TimeSleepMillis { duration: path }
        | ValueExpr::LogEnabled { level: path }
        | ValueExpr::HashString { value: path }
        | ValueExpr::HashFinish { state: path }
        | ValueExpr::ProcessExit { code: path }
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

fn expr_is_process_exec(expr: &ValueExpr) -> bool {
    matches!(expr, ValueExpr::ProcessExec { .. })
}

fn expr_is_process_output(expr: &ValueExpr) -> bool {
    matches!(expr, ValueExpr::ProcessOutput { .. })
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
            | ValueExpr::HashWriteString { .. }
            | ValueExpr::HashFinish { .. }
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
        Statement::QuestionReturnOk { result_expr, .. } => expr_uses_env_get(result_expr),
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
        Statement::QuestionReturnOk { result_expr, .. } => expr_uses_env_args(result_expr),
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
        Statement::QuestionReturnOk {
            ok_type,
            result_type,
            return_type,
            result_expr,
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
        | ValueExpr::MathBinary { left, right, .. } => {
            expr_uses_fs_read_to_string(left) || expr_uses_fs_read_to_string(right)
        }
        ValueExpr::FsWriteString { path, content } => {
            expr_uses_fs_read_to_string(path) || expr_uses_fs_read_to_string(content)
        }
        ValueExpr::EnvSet { name, value } => {
            expr_uses_fs_read_to_string(name) || expr_uses_fs_read_to_string(value)
        }
        ValueExpr::FsExists { path }
        | ValueExpr::FsMetadata { path }
        | ValueExpr::FsCreateDir { path }
        | ValueExpr::FsRemoveDir { path }
        | ValueExpr::FsReadDir { path }
        | ValueExpr::FsOpen { path }
        | ValueExpr::FileClose { file: path }
        | ValueExpr::FileReadToString { file: path } => expr_uses_fs_read_to_string(path),
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
        | ValueExpr::TimeSleepMillis { duration: name }
        | ValueExpr::LogEnabled { level: name }
        | ValueExpr::HashString { value: name }
        | ValueExpr::HashFinish { state: name }
        | ValueExpr::ProcessExit { code: name }
        | ValueExpr::ProcessStatus { command: name }
        | ValueExpr::ProcessExec { command: name }
        | ValueExpr::ProcessOutput { command: name }
        | ValueExpr::NumParseI64 { value: name }
        | ValueExpr::NumParseU64 { value: name }
        | ValueExpr::NumParseF64 { value: name }
        | ValueExpr::NumToString { value: name, .. } => expr_uses_fs_read_to_string(name),
        ValueExpr::EnvArgs => false,
        ValueExpr::ArrayNew { .. } | ValueExpr::HashNew => false,
        ValueExpr::HashWriteString { state, value } => {
            expr_uses_fs_read_to_string(state) || expr_uses_fs_read_to_string(value)
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
        | ValueExpr::MathBinary { left, right, .. } => {
            expr_uses_fs_write_string(left) || expr_uses_fs_write_string(right)
        }
        ValueExpr::FsReadToString { path }
        | ValueExpr::FsExists { path }
        | ValueExpr::FsMetadata { path }
        | ValueExpr::FsCreateDir { path }
        | ValueExpr::FsRemoveDir { path }
        | ValueExpr::FsReadDir { path } => expr_uses_fs_write_string(path),
        ValueExpr::FileReadToString { file } => expr_uses_fs_write_string(file),
        ValueExpr::FileWriteString { file, content } => {
            expr_uses_fs_write_string(file) || expr_uses_fs_write_string(content)
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
        | ValueExpr::TimeSleepMillis { duration: name }
        | ValueExpr::LogEnabled { level: name }
        | ValueExpr::HashString { value: name }
        | ValueExpr::HashFinish { state: name }
        | ValueExpr::ProcessExit { code: name }
        | ValueExpr::ProcessStatus { command: name }
        | ValueExpr::ProcessExec { command: name }
        | ValueExpr::ProcessOutput { command: name }
        | ValueExpr::NumParseI64 { value: name }
        | ValueExpr::NumParseU64 { value: name }
        | ValueExpr::NumParseF64 { value: name }
        | ValueExpr::NumToString { value: name, .. } => expr_uses_fs_write_string(name),
        ValueExpr::EnvArgs => false,
        ValueExpr::ArrayNew { .. } | ValueExpr::HashNew => false,
        ValueExpr::HashWriteString { state, value } => {
            expr_uses_fs_write_string(state) || expr_uses_fs_write_string(value)
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
        | ValueExpr::MathBinary { left, right, .. } => {
            expr_uses_fs_open(left) || expr_uses_fs_open(right)
        }
        ValueExpr::FsReadToString { path }
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
        | ValueExpr::TimeSleepMillis { duration: path }
        | ValueExpr::LogEnabled { level: path }
        | ValueExpr::HashString { value: path }
        | ValueExpr::HashFinish { state: path }
        | ValueExpr::ProcessExit { code: path }
        | ValueExpr::ProcessStatus { command: path }
        | ValueExpr::ProcessExec { command: path }
        | ValueExpr::ProcessOutput { command: path }
        | ValueExpr::NumParseI64 { value: path }
        | ValueExpr::NumParseU64 { value: path }
        | ValueExpr::NumParseF64 { value: path }
        | ValueExpr::NumToString { value: path, .. }
        | ValueExpr::ArrayLen { array: path }
        | ValueExpr::FileReadToString { file: path } => expr_uses_fs_open(path),
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
        ValueExpr::FileWriteString { file, content } => {
            expr_uses_fs_open(file) || expr_uses_fs_open(content)
        }
        ValueExpr::EnvSet { name, value } => expr_uses_fs_open(name) || expr_uses_fs_open(value),
        ValueExpr::EnvArgs => false,
        ValueExpr::EnvCwd | ValueExpr::EnvHomeDir | ValueExpr::EnvTempDir => false,
        ValueExpr::ArrayNew { .. } | ValueExpr::HashNew => false,
        ValueExpr::HashWriteString { state, value } => {
            expr_uses_fs_open(state) || expr_uses_fs_open(value)
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
        | ValueExpr::MathBinary { left, right, .. } => {
            expr_uses_env_get(left) || expr_uses_env_get(right)
        }
        ValueExpr::FsReadToString { path }
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
        | ValueExpr::TimeSleepMillis { duration: path }
        | ValueExpr::LogEnabled { level: path }
        | ValueExpr::HashString { value: path }
        | ValueExpr::HashFinish { state: path }
        | ValueExpr::ProcessExit { code: path }
        | ValueExpr::ProcessStatus { command: path }
        | ValueExpr::ProcessExec { command: path }
        | ValueExpr::ProcessOutput { command: path }
        | ValueExpr::NumParseI64 { value: path }
        | ValueExpr::NumParseU64 { value: path }
        | ValueExpr::NumParseF64 { value: path }
        | ValueExpr::NumToString { value: path, .. }
        | ValueExpr::FileReadToString { file: path } => expr_uses_env_get(path),
        ValueExpr::FsWriteString { path, content } => {
            expr_uses_env_get(path) || expr_uses_env_get(content)
        }
        ValueExpr::FileWriteString { file, content } => {
            expr_uses_env_get(file) || expr_uses_env_get(content)
        }
        ValueExpr::EnvSet { name, value } => expr_uses_env_get(name) || expr_uses_env_get(value),
        ValueExpr::HashWriteString { state, value } => {
            expr_uses_env_get(state) || expr_uses_env_get(value)
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
        ValueExpr::ArrayNew { .. } => false,
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
        | ValueExpr::MathBinary { left, right, .. } => {
            expr_uses_env_args(left) || expr_uses_env_args(right)
        }
        ValueExpr::FsReadToString { path }
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
        | ValueExpr::TimeSleepMillis { duration: path }
        | ValueExpr::LogEnabled { level: path }
        | ValueExpr::HashString { value: path }
        | ValueExpr::HashFinish { state: path }
        | ValueExpr::ProcessExit { code: path }
        | ValueExpr::ProcessStatus { command: path }
        | ValueExpr::ProcessExec { command: path }
        | ValueExpr::ProcessOutput { command: path }
        | ValueExpr::NumParseI64 { value: path }
        | ValueExpr::NumParseU64 { value: path }
        | ValueExpr::NumParseF64 { value: path }
        | ValueExpr::NumToString { value: path, .. }
        | ValueExpr::ArrayLen { array: path }
        | ValueExpr::FileReadToString { file: path } => expr_uses_env_args(path),
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
        ValueExpr::FileWriteString { file, content } => {
            expr_uses_env_args(file) || expr_uses_env_args(content)
        }
        ValueExpr::EnvSet { name, value } => expr_uses_env_args(name) || expr_uses_env_args(value),
        ValueExpr::HashWriteString { state, value } => {
            expr_uses_env_args(state) || expr_uses_env_args(value)
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
        | ValueExpr::MathBinary { left, right, .. } => {
            collect_expr_array_elements(left, seen, out);
            collect_expr_array_elements(right, seen, out);
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
        | ValueExpr::TimeSleepMillis { duration: path }
        | ValueExpr::LogEnabled { level: path }
        | ValueExpr::HashString { value: path }
        | ValueExpr::HashFinish { state: path }
        | ValueExpr::ProcessExit { code: path }
        | ValueExpr::ProcessStatus { command: path }
        | ValueExpr::ProcessExec { command: path }
        | ValueExpr::ProcessOutput { command: path }
        | ValueExpr::NumParseI64 { value: path }
        | ValueExpr::NumParseU64 { value: path }
        | ValueExpr::NumParseF64 { value: path }
        | ValueExpr::NumToString { value: path, .. } => {
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

fn subst_type(value_type: &ValueType, type_params: &[String], args: &[ValueType]) -> ValueType {
    match value_type {
        ValueType::TypeParam(name) => type_params
            .iter()
            .position(|param| param == name)
            .and_then(|index| args.get(index).cloned())
            .unwrap_or_else(|| value_type.clone()),
        ValueType::Enum(name, nested_args) => ValueType::Enum(
            name.clone(),
            nested_args
                .iter()
                .map(|arg| subst_type(arg, type_params, args))
                .collect(),
        ),
        ValueType::Struct(name, nested_args) => ValueType::Struct(
            name.clone(),
            nested_args
                .iter()
                .map(|arg| subst_type(arg, type_params, args))
                .collect(),
        ),
        ValueType::Array(element) => {
            ValueType::Array(Box::new(subst_type(element, type_params, args)))
        }
        _ => value_type.clone(),
    }
}

fn emit_string_data_expr(out: &mut String, expr: &ValueExpr) {
    out.push('(');
    emit_expr(out, expr);
    out.push_str(").data");
}

fn c_binary_op(op: &BinaryOp) -> &'static str {
    match op {
        BinaryOp::LogicalOr => "||",
        BinaryOp::LogicalAnd => "&&",
        BinaryOp::Add => "+",
        BinaryOp::Subtract => "-",
        BinaryOp::BitOr => "|",
        BinaryOp::BitXor => "^",
        BinaryOp::Multiply => "*",
        BinaryOp::Divide => "/",
        BinaryOp::Remainder => "%",
        BinaryOp::ShiftLeft => "<<",
        BinaryOp::ShiftRight => ">>",
        BinaryOp::BitAnd => "&",
        BinaryOp::BitAndNot => "&^",
        BinaryOp::Equal => "==",
        BinaryOp::NotEqual => "!=",
        BinaryOp::Less => "<",
        BinaryOp::LessEqual => "<=",
        BinaryOp::Greater => ">",
        BinaryOp::GreaterEqual => ">=",
    }
}

fn math_unary_function_name(function: MathUnaryFunction, value_type: &ValueType) -> &'static str {
    match (function, value_type) {
        (MathUnaryFunction::Abs, ValueType::Int) => "nomo_math_abs_i64",
        (MathUnaryFunction::Abs, ValueType::I32) => "nomo_math_abs_i32",
        (MathUnaryFunction::Abs, ValueType::U32) => "nomo_math_abs_u32",
        (MathUnaryFunction::Abs, ValueType::U64) => "nomo_math_abs_u64",
        (MathUnaryFunction::Abs, ValueType::Float) => "nomo_math_abs_f64",
        (MathUnaryFunction::Floor, ValueType::Float) => "floor",
        (MathUnaryFunction::Ceil, ValueType::Float) => "ceil",
        (MathUnaryFunction::Round, ValueType::Float) => "round",
        (MathUnaryFunction::Sqrt, ValueType::Float) => "sqrt",
        (MathUnaryFunction::Sin, ValueType::Float) => "sin",
        (MathUnaryFunction::Cos, ValueType::Float) => "cos",
        _ => unreachable!("compiler only emits well-typed math unary calls"),
    }
}

fn math_binary_function_name(function: MathBinaryFunction, value_type: &ValueType) -> &'static str {
    match (function, value_type) {
        (MathBinaryFunction::Min, ValueType::Int) => "nomo_math_min_i64",
        (MathBinaryFunction::Min, ValueType::I32) => "nomo_math_min_i32",
        (MathBinaryFunction::Min, ValueType::U32) => "nomo_math_min_u32",
        (MathBinaryFunction::Min, ValueType::U64) => "nomo_math_min_u64",
        (MathBinaryFunction::Min, ValueType::Float) => "nomo_math_min_f64",
        (MathBinaryFunction::Max, ValueType::Int) => "nomo_math_max_i64",
        (MathBinaryFunction::Max, ValueType::I32) => "nomo_math_max_i32",
        (MathBinaryFunction::Max, ValueType::U32) => "nomo_math_max_u32",
        (MathBinaryFunction::Max, ValueType::U64) => "nomo_math_max_u64",
        (MathBinaryFunction::Max, ValueType::Float) => "nomo_math_max_f64",
        (MathBinaryFunction::Pow, ValueType::Float) => "pow",
        _ => unreachable!("compiler only emits well-typed math binary calls"),
    }
}

fn checked_binary_helper(op: &BinaryOp, value_type: &ValueType) -> Option<&'static str> {
    match (op, value_type) {
        (BinaryOp::Add, ValueType::Int) => Some("nomo_add_i64"),
        (BinaryOp::Subtract, ValueType::Int) => Some("nomo_sub_i64"),
        (BinaryOp::Multiply, ValueType::Int) => Some("nomo_mul_i64"),
        (BinaryOp::Divide, ValueType::Int) => Some("nomo_div_i64"),
        (BinaryOp::Remainder, ValueType::Int) => Some("nomo_rem_i64"),
        (BinaryOp::Add, ValueType::I32) => Some("nomo_add_i32"),
        (BinaryOp::Subtract, ValueType::I32) => Some("nomo_sub_i32"),
        (BinaryOp::Multiply, ValueType::I32) => Some("nomo_mul_i32"),
        (BinaryOp::Divide, ValueType::I32) => Some("nomo_div_i32"),
        (BinaryOp::Remainder, ValueType::I32) => Some("nomo_rem_i32"),
        (BinaryOp::Divide, ValueType::U32) => Some("nomo_div_u32"),
        (BinaryOp::Remainder, ValueType::U32) => Some("nomo_rem_u32"),
        (BinaryOp::Divide, ValueType::U64) => Some("nomo_div_u64"),
        (BinaryOp::Remainder, ValueType::U64) => Some("nomo_rem_u64"),
        (BinaryOp::Divide, ValueType::Float) => Some("nomo_div_f64"),
        (BinaryOp::ShiftLeft, ValueType::Int) => Some("nomo_shl_i64"),
        (BinaryOp::ShiftRight, ValueType::Int) => Some("nomo_shr_i64"),
        (BinaryOp::ShiftLeft, ValueType::I32) => Some("nomo_shl_i32"),
        (BinaryOp::ShiftRight, ValueType::I32) => Some("nomo_shr_i32"),
        (BinaryOp::ShiftLeft, ValueType::U32) => Some("nomo_shl_u32"),
        (BinaryOp::ShiftRight, ValueType::U32) => Some("nomo_shr_u32"),
        (BinaryOp::ShiftLeft, ValueType::U64) => Some("nomo_shl_u64"),
        (BinaryOp::ShiftRight, ValueType::U64) => Some("nomo_shr_u64"),
        _ => None,
    }
}

fn c_unary_op(op: &UnaryOp) -> &'static str {
    match op {
        UnaryOp::Not => "!",
    }
}

fn num_to_string_helper_name(value_type: &ValueType) -> &'static str {
    match value_type {
        ValueType::Int => "nomo_num_i64_to_string",
        ValueType::I32 => "nomo_num_i32_to_string",
        ValueType::U32 => "nomo_num_u32_to_string",
        ValueType::U64 => "nomo_num_u64_to_string",
        ValueType::Float => "nomo_num_f64_to_string",
        _ => unreachable!("num.to_string only lowers supported numeric types"),
    }
}

fn num_checked_binary_helper_name(op: &BinaryOp, value_type: &ValueType) -> &'static str {
    match (op, value_type) {
        (BinaryOp::Add, ValueType::Int) => "nomo_num_checked_add_i64",
        (BinaryOp::Subtract, ValueType::Int) => "nomo_num_checked_sub_i64",
        (BinaryOp::Multiply, ValueType::Int) => "nomo_num_checked_mul_i64",
        (BinaryOp::Add, ValueType::I32) => "nomo_num_checked_add_i32",
        (BinaryOp::Subtract, ValueType::I32) => "nomo_num_checked_sub_i32",
        (BinaryOp::Multiply, ValueType::I32) => "nomo_num_checked_mul_i32",
        (BinaryOp::Add, ValueType::U32) => "nomo_num_checked_add_u32",
        (BinaryOp::Subtract, ValueType::U32) => "nomo_num_checked_sub_u32",
        (BinaryOp::Multiply, ValueType::U32) => "nomo_num_checked_mul_u32",
        (BinaryOp::Add, ValueType::U64) => "nomo_num_checked_add_u64",
        (BinaryOp::Subtract, ValueType::U64) => "nomo_num_checked_sub_u64",
        (BinaryOp::Multiply, ValueType::U64) => "nomo_num_checked_mul_u64",
        _ => unreachable!("num checked helpers only lower integer add/sub/mul"),
    }
}

fn num_wrapping_binary_helper_name(op: &BinaryOp, value_type: &ValueType) -> &'static str {
    match (op, value_type) {
        (BinaryOp::Add, ValueType::Int) => "nomo_num_wrapping_add_i64",
        (BinaryOp::Subtract, ValueType::Int) => "nomo_num_wrapping_sub_i64",
        (BinaryOp::Multiply, ValueType::Int) => "nomo_num_wrapping_mul_i64",
        (BinaryOp::Add, ValueType::I32) => "nomo_num_wrapping_add_i32",
        (BinaryOp::Subtract, ValueType::I32) => "nomo_num_wrapping_sub_i32",
        (BinaryOp::Multiply, ValueType::I32) => "nomo_num_wrapping_mul_i32",
        (BinaryOp::Add, ValueType::U32) => "nomo_num_wrapping_add_u32",
        (BinaryOp::Subtract, ValueType::U32) => "nomo_num_wrapping_sub_u32",
        (BinaryOp::Multiply, ValueType::U32) => "nomo_num_wrapping_mul_u32",
        (BinaryOp::Add, ValueType::U64) => "nomo_num_wrapping_add_u64",
        (BinaryOp::Subtract, ValueType::U64) => "nomo_num_wrapping_sub_u64",
        (BinaryOp::Multiply, ValueType::U64) => "nomo_num_wrapping_mul_u64",
        _ => unreachable!("num wrapping helpers only lower integer add/sub/mul"),
    }
}

fn c_type(value_type: &ValueType) -> String {
    match value_type {
        ValueType::String => "nomo_string".to_string(),
        ValueType::Int => "long long".to_string(),
        ValueType::I32 => "int32_t".to_string(),
        ValueType::U32 => "uint32_t".to_string(),
        ValueType::U64 => "uint64_t".to_string(),
        ValueType::Float => "double".to_string(),
        ValueType::Char => "uint32_t".to_string(),
        ValueType::Bool => "int".to_string(),
        ValueType::Array(element) if is_supported_array_element(element) => c_array_ident(element),
        ValueType::Array(element) => panic!(
            "unsupported Array element type reached C type lowering: {}",
            element.name()
        ),
        ValueType::Struct(name, args) => c_struct_ident(name, args),
        ValueType::Enum(name, args) => c_enum_ident(name, args),
        ValueType::TypeParam(name) => {
            panic!("unsubstituted type parameter reached C codegen: {name}")
        }
        ValueType::Void => "void".to_string(),
        ValueType::Never => "void".to_string(),
    }
}

fn result_void_error(value_type: &ValueType) -> Option<Vec<ValueType>> {
    let ValueType::Enum(name, args) = value_type else {
        return None;
    };
    if name == "Result" && args.len() == 2 && args[0] == ValueType::Void {
        Some(args.clone())
    } else {
        None
    }
}

fn c_payload_type(value_type: &ValueType) -> String {
    if value_type == &ValueType::Void {
        "char".to_string()
    } else {
        c_type(value_type)
    }
}

fn c_zero_value(value_type: &ValueType) -> String {
    match value_type {
        ValueType::String => "nomo_string_literal(\"\")".to_string(),
        ValueType::Int => "0".to_string(),
        ValueType::I32 | ValueType::U32 | ValueType::U64 => "0".to_string(),
        ValueType::Float => "0.0".to_string(),
        ValueType::Char => "0".to_string(),
        ValueType::Bool => "0".to_string(),
        ValueType::Array(element) if is_supported_array_element(element) => {
            format!("{}_new()", c_array_ident(element))
        }
        ValueType::Array(_) => "0".to_string(),
        ValueType::Struct(name, args) => format!("({}){{0}}", c_struct_ident(name, args)),
        ValueType::Enum(name, args) => format!("({}){{0}}", c_enum_ident(name, args)),
        ValueType::TypeParam(_) => "0".to_string(),
        ValueType::Void | ValueType::Never => "(void)0".to_string(),
    }
}

fn c_type_suffix(args: &[ValueType]) -> String {
    if args.is_empty() {
        return String::new();
    }
    let parts = args.iter().map(c_type_name_part).collect::<Vec<_>>();
    format!("_{}", parts.join("_"))
}

fn c_type_name_part(value_type: &ValueType) -> String {
    match value_type {
        ValueType::String => "string".to_string(),
        ValueType::Int => "i64".to_string(),
        ValueType::I32 => "i32".to_string(),
        ValueType::U32 => "u32".to_string(),
        ValueType::U64 => "u64".to_string(),
        ValueType::Float => "f64".to_string(),
        ValueType::Char => "char".to_string(),
        ValueType::Bool => "bool".to_string(),
        ValueType::Array(element) => format!("array_{}", c_type_name_part(element)),
        ValueType::Struct(name, args) => format!("struct_{}{}", name, c_type_suffix(args)),
        ValueType::Enum(name, args) => format!("enum_{}{}", name, c_type_suffix(args)),
        ValueType::TypeParam(name) => format!("param_{name}"),
        ValueType::Void => "void".to_string(),
        ValueType::Never => "never".to_string(),
    }
}

fn c_var_ident(name: &str) -> String {
    format!("nomo_{name}")
}

fn c_member_ident(name: &str) -> String {
    format!("nomo_member_{name}")
}

fn c_payload_ident(variant: &str) -> String {
    format!("nomo_payload_{variant}")
}

fn c_fn_ident(name: &str) -> String {
    format!("nomo_fn_{name}")
}

fn c_package_ident(package: &str) -> String {
    package
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
        .collect()
}

fn c_struct_ident(name: &str, args: &[ValueType]) -> String {
    format!("nomo_struct_{}{}", name, c_type_suffix(args))
}

fn c_array_ident(element_type: &ValueType) -> String {
    format!("nomo_array_{}", c_type_name_part(element_type))
}

fn c_enum_ident(name: &str, args: &[ValueType]) -> String {
    format!("nomo_enum_{}{}", name, c_type_suffix(args))
}

fn c_enum_tag_ident(name: &str, args: &[ValueType]) -> String {
    format!("nomo_enum_{}{}_tag", name, c_type_suffix(args))
}

fn c_enum_variant_ident(enum_name: &str, args: &[ValueType], variant: &str) -> String {
    format!("nomo_enum_{}{}_{}", enum_name, c_type_suffix(args), variant)
}

fn c_result_map_err_helper_ident(instance: &ResultMapErrInstance) -> String {
    format!(
        "nomo_result_map_err_{}_{}_{}_{}",
        c_type_name_part(&instance.ok_type),
        c_type_name_part(&instance.source_err_type),
        c_type_name_part(&instance.target_err_type),
        instance.converter
    )
}

fn c_result_unwrap_or_helper_ident(instance: &ResultUnwrapOrInstance) -> String {
    format!(
        "nomo_result_unwrap_or_{}_{}",
        c_type_name_part(&instance.ok_type),
        c_type_name_part(&instance.err_type)
    )
}

fn c_result_map_helper_ident(instance: &ResultMapInstance) -> String {
    format!(
        "nomo_result_map_{}_{}_{}_{}",
        c_type_name_part(&instance.source_ok_type),
        c_type_name_part(&instance.target_ok_type),
        c_type_name_part(&instance.err_type),
        instance.converter
    )
}

fn c_result_and_then_helper_ident(instance: &ResultAndThenInstance) -> String {
    format!(
        "nomo_result_and_then_{}_{}_{}_{}",
        c_type_name_part(&instance.source_ok_type),
        c_type_name_part(&instance.target_ok_type),
        c_type_name_part(&instance.err_type),
        instance.converter
    )
}

fn c_option_unwrap_or_helper_ident(instance: &OptionUnwrapOrInstance) -> String {
    format!(
        "nomo_option_unwrap_or_{}",
        c_type_name_part(&instance.payload_type)
    )
}

fn c_option_map_helper_ident(instance: &OptionMapInstance) -> String {
    format!(
        "nomo_option_map_{}_{}_{}",
        c_type_name_part(&instance.source_type),
        c_type_name_part(&instance.target_type),
        instance.converter
    )
}

fn c_option_and_then_helper_ident(instance: &OptionAndThenInstance) -> String {
    format!(
        "nomo_option_and_then_{}_{}_{}",
        c_type_name_part(&instance.source_type),
        c_type_name_part(&instance.target_type),
        instance.converter
    )
}

fn c_retain_ident(value_type: &ValueType) -> String {
    match value_type {
        ValueType::String => "nomo_string_retain".to_string(),
        ValueType::Array(element_type) => format!("{}_retain", c_array_ident(element_type)),
        ValueType::Struct(name, args) => format!("{}_retain", c_struct_ident(name, args)),
        ValueType::Enum(name, args) => format!("{}_retain", c_enum_ident(name, args)),
        _ => panic!(
            "unsupported retain helper requested for C type: {}",
            value_type.name()
        ),
    }
}

fn c_release_ident(value_type: &ValueType) -> String {
    match value_type {
        ValueType::String => "nomo_string_release".to_string(),
        ValueType::Array(element_type) => format!("{}_release", c_array_ident(element_type)),
        ValueType::Struct(name, args) => format!("{}_release", c_struct_ident(name, args)),
        ValueType::Enum(name, args) => format!("{}_release", c_enum_ident(name, args)),
        _ => panic!(
            "unsupported release helper requested for C type: {}",
            value_type.name()
        ),
    }
}

fn escape_c_string(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            c => escaped.push(c),
        }
    }
    escaped
}

fn is_supported_array_element(value_type: &ValueType) -> bool {
    !matches!(
        value_type,
        ValueType::Void | ValueType::Never | ValueType::TypeParam(_)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler::{EnumVariantType, MatchValueArm, Parameter, StructField, ValueExpr};

    fn string_literal(value: &str) -> String {
        format!("nomo_string_literal(\"{value}\")")
    }

    fn puts_literal(value: &str) -> String {
        format!("puts(({}).data);", string_literal(value))
    }

    fn fputs_literal(value: &str) -> String {
        format!("fputs(({}).data, stderr);", string_literal(value))
    }

    fn fputs_stdout_literal(value: &str) -> String {
        format!("fputs(({}).data, stdout);", string_literal(value))
    }

    fn panic_literal(value: &str) -> String {
        format!("nomo_panic(({}).data);", string_literal(value))
    }

    #[test]
    fn emits_puts_for_println() {
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: vec!["std.io".to_string()],
            structs: Vec::new(),
            enums: Vec::new(),
            functions: vec![Function {
                package: "app.main".to_string(),
                name: "main".to_string(),
                params: Vec::new(),
                return_type: ValueType::Void,
                body: vec![Statement::Println(ValueExpr::StringLiteral(
                    "Hello".to_string(),
                ))],
            }],
        };

        let c = emit_c(&program);
        assert!(c.contains("#include <stdio.h>"));
        assert!(c.contains(&puts_literal("Hello")));
    }

    #[test]
    fn emits_package_prefixed_function_symbol_macros() {
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: Vec::new(),
            structs: Vec::new(),
            enums: Vec::new(),
            functions: vec![
                Function {
                    package: "app.main".to_string(),
                    name: "add".to_string(),
                    params: vec![
                        Parameter {
                            name: "a".to_string(),
                            value_type: ValueType::I32,
                            mutable: false,
                        },
                        Parameter {
                            name: "b".to_string(),
                            value_type: ValueType::I32,
                            mutable: false,
                        },
                    ],
                    return_type: ValueType::I32,
                    body: vec![Statement::Return(Some(ValueExpr::Binary {
                        op: BinaryOp::Add,
                        left: Box::new(ValueExpr::Variable("a".to_string())),
                        right: Box::new(ValueExpr::Variable("b".to_string())),
                        value_type: ValueType::I32,
                    }))],
                },
                Function {
                    package: "app.main".to_string(),
                    name: "main".to_string(),
                    params: Vec::new(),
                    return_type: ValueType::Void,
                    body: vec![Statement::Expr(ValueExpr::Call {
                        name: "add".to_string(),
                        args: vec![ValueExpr::IntLiteral(1), ValueExpr::IntLiteral(2)],
                    })],
                },
            ],
        };

        let c = emit_c(&program);

        assert!(c.contains("#define nomo_fn_add nomo_pkg_app_main_fn_add"));
        assert!(c.contains("#define nomo_fn_main nomo_pkg_app_main_fn_main"));
        assert!(c.contains("int32_t nomo_fn_add(int32_t nomo_a, int32_t nomo_b);"));
        assert!(c.contains("nomo_fn_add(1, 2);"));
    }

    #[test]
    fn emits_package_prefixed_type_symbol_macros() {
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: Vec::new(),
            structs: vec![StructType {
                package: "app.main".to_string(),
                name: "Point".to_string(),
                type_params: Vec::new(),
                fields: vec![StructField {
                    name: "x".to_string(),
                    value_type: ValueType::I32,
                }],
            }],
            enums: vec![EnumType {
                package: "app.main".to_string(),
                name: "Color".to_string(),
                type_params: Vec::new(),
                variants: vec![
                    EnumVariantType {
                        name: "Red".to_string(),
                        payload: None,
                    },
                    EnumVariantType {
                        name: "Blue".to_string(),
                        payload: None,
                    },
                ],
            }],
            functions: vec![Function {
                package: "app.main".to_string(),
                name: "main".to_string(),
                params: Vec::new(),
                return_type: ValueType::Void,
                body: vec![
                    Statement::Let {
                        name: "point".to_string(),
                        value_type: ValueType::Struct("Point".to_string(), Vec::new()),
                        initializer: ValueExpr::StructLiteral {
                            type_name: "Point".to_string(),
                            struct_args: Vec::new(),
                            fields: vec![("x".to_string(), ValueExpr::IntLiteral(1))],
                        },
                    },
                    Statement::Let {
                        name: "color".to_string(),
                        value_type: ValueType::Enum("Color".to_string(), Vec::new()),
                        initializer: ValueExpr::EnumVariant {
                            enum_name: "Color".to_string(),
                            enum_args: Vec::new(),
                            variant: "Red".to_string(),
                            payload: None,
                        },
                    },
                ],
            }],
        };

        let c = emit_c(&program);

        assert!(c.contains("#define nomo_struct_Point nomo_pkg_app_main_struct_Point"));
        assert!(c.contains("#define nomo_enum_Color_tag nomo_pkg_app_main_enum_Color_tag"));
        assert!(c.contains("#define nomo_enum_Color nomo_pkg_app_main_enum_Color"));
        assert!(c.contains("#define nomo_enum_Color_Red nomo_pkg_app_main_enum_Color_Red"));
        assert!(c.contains("#define nomo_enum_Color_Blue nomo_pkg_app_main_enum_Color_Blue"));
    }

    #[test]
    fn emits_fputs_for_eprintln() {
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: vec!["std.io".to_string()],
            structs: Vec::new(),
            enums: Vec::new(),
            functions: vec![Function {
                package: "app.main".to_string(),
                name: "main".to_string(),
                params: Vec::new(),
                return_type: ValueType::Void,
                body: vec![Statement::Eprintln(ValueExpr::StringLiteral(
                    "error".to_string(),
                ))],
            }],
        };

        let c = emit_c(&program);
        assert!(c.contains(&fputs_literal("error")));
        assert!(c.contains("fputc('\\n', stderr);"));
    }

    #[test]
    fn emits_fputs_for_print_without_newline() {
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: vec!["std.io".to_string()],
            structs: Vec::new(),
            enums: Vec::new(),
            functions: vec![Function {
                package: "app.main".to_string(),
                name: "main".to_string(),
                params: Vec::new(),
                return_type: ValueType::Void,
                body: vec![Statement::Print(ValueExpr::StringLiteral(
                    "partial".to_string(),
                ))],
            }],
        };

        let c = emit_c(&program);
        assert!(c.contains(&fputs_stdout_literal("partial")));
        assert!(!c.contains(&puts_literal("partial")));
    }

    #[test]
    fn emits_fputs_for_eprint_without_newline() {
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: vec!["std.io".to_string()],
            structs: Vec::new(),
            enums: Vec::new(),
            functions: vec![Function {
                package: "app.main".to_string(),
                name: "main".to_string(),
                params: Vec::new(),
                return_type: ValueType::Void,
                body: vec![Statement::Eprint(ValueExpr::StringLiteral(
                    "partial error".to_string(),
                ))],
            }],
        };

        let c = emit_c(&program);
        assert!(c.contains(&fputs_literal("partial error")));
        assert!(!c.contains(&format!(
            "{}\n    fputc('\\n', stderr);",
            fputs_literal("partial error")
        )));
    }

    #[test]
    fn emits_function_and_call() {
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: vec!["std.io".to_string()],
            structs: Vec::new(),
            enums: Vec::new(),
            functions: vec![
                Function {
                    package: "app.main".to_string(),
                    name: "add".to_string(),
                    params: vec![
                        Parameter {
                            name: "a".to_string(),
                            mutable: false,
                            value_type: ValueType::Int,
                        },
                        Parameter {
                            name: "b".to_string(),
                            mutable: false,
                            value_type: ValueType::Int,
                        },
                    ],
                    return_type: ValueType::Int,
                    body: vec![Statement::Return(Some(ValueExpr::Binary {
                        left: Box::new(ValueExpr::Variable("a".to_string())),
                        op: BinaryOp::Add,
                        right: Box::new(ValueExpr::Variable("b".to_string())),
                        value_type: ValueType::Int,
                    }))],
                },
                Function {
                    package: "app.main".to_string(),
                    name: "main".to_string(),
                    params: Vec::new(),
                    return_type: ValueType::Void,
                    body: vec![
                        Statement::Let {
                            name: "answer".to_string(),
                            value_type: ValueType::Int,
                            initializer: ValueExpr::Call {
                                name: "add".to_string(),
                                args: vec![ValueExpr::IntLiteral(40), ValueExpr::IntLiteral(2)],
                            },
                        },
                        Statement::Println(ValueExpr::StringLiteral("done".to_string())),
                    ],
                },
            ],
        };

        let c = emit_c(&program);
        assert!(c.contains("long long nomo_fn_add(long long nomo_a, long long nomo_b);"));
        assert!(c.contains("long long nomo_fn_add(long long nomo_a, long long nomo_b)"));
        assert!(c.contains("return nomo_add_i64(nomo_a, nomo_b);"));
        assert!(c.contains("long long nomo_answer = nomo_fn_add(40, 2);"));
    }

    #[test]
    fn emits_mut_parameter_as_pointer_borrow() {
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: Vec::new(),
            structs: Vec::new(),
            enums: Vec::new(),
            functions: vec![
                Function {
                    package: "app.main".to_string(),
                    name: "bump".to_string(),
                    params: vec![Parameter {
                        name: "value".to_string(),
                        mutable: true,
                        value_type: ValueType::Int,
                    }],
                    return_type: ValueType::Void,
                    body: vec![Statement::Assign {
                        name: "value".to_string(),
                        value: ValueExpr::Binary {
                            left: Box::new(ValueExpr::Variable("value".to_string())),
                            op: BinaryOp::Add,
                            right: Box::new(ValueExpr::IntLiteral(1)),
                            value_type: ValueType::Int,
                        },
                    }],
                },
                Function {
                    package: "app.main".to_string(),
                    name: "main".to_string(),
                    params: Vec::new(),
                    return_type: ValueType::Void,
                    body: vec![
                        Statement::Let {
                            name: "count".to_string(),
                            value_type: ValueType::Int,
                            initializer: ValueExpr::IntLiteral(1),
                        },
                        Statement::Expr(ValueExpr::Call {
                            name: "bump".to_string(),
                            args: vec![ValueExpr::MutBorrow(vec!["count".to_string()])],
                        }),
                    ],
                },
            ],
        };

        let c = emit_c(&program);
        assert!(c.contains("void nomo_fn_bump(long long * nomo_value);"));
        assert!(c.contains("#define nomo_value (*nomo_value)"));
        assert!(c.contains("nomo_value = nomo_add_i64(nomo_value, 1);"));
        assert!(c.contains("#undef nomo_value"));
        assert!(c.contains("nomo_fn_bump(&nomo_count);"));
    }

    #[test]
    fn emits_mut_field_path_as_pointer_borrow() {
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: Vec::new(),
            structs: vec![StructType {
                package: "app.main".to_string(),
                name: "Point".to_string(),
                type_params: Vec::new(),
                fields: vec![
                    StructField {
                        name: "x".to_string(),
                        value_type: ValueType::I32,
                    },
                    StructField {
                        name: "y".to_string(),
                        value_type: ValueType::I32,
                    },
                ],
            }],
            enums: Vec::new(),
            functions: vec![
                Function {
                    package: "app.main".to_string(),
                    name: "bump".to_string(),
                    params: vec![Parameter {
                        name: "value".to_string(),
                        mutable: true,
                        value_type: ValueType::I32,
                    }],
                    return_type: ValueType::Void,
                    body: Vec::new(),
                },
                Function {
                    package: "app.main".to_string(),
                    name: "main".to_string(),
                    params: Vec::new(),
                    return_type: ValueType::Void,
                    body: vec![
                        Statement::Let {
                            name: "point".to_string(),
                            value_type: ValueType::Struct("Point".to_string(), Vec::new()),
                            initializer: ValueExpr::StructLiteral {
                                type_name: "Point".to_string(),
                                struct_args: Vec::new(),
                                fields: vec![
                                    ("x".to_string(), ValueExpr::IntLiteral(1)),
                                    ("y".to_string(), ValueExpr::IntLiteral(2)),
                                ],
                            },
                        },
                        Statement::Expr(ValueExpr::Call {
                            name: "bump".to_string(),
                            args: vec![ValueExpr::MutBorrow(vec![
                                "point".to_string(),
                                "x".to_string(),
                            ])],
                        }),
                    ],
                },
            ],
        };

        let c = emit_c(&program);
        assert!(c.contains("nomo_fn_bump(&nomo_point.nomo_member_x);"));
    }

    #[test]
    fn emits_float_literal_and_cast() {
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: vec!["std.io".to_string()],
            structs: Vec::new(),
            enums: Vec::new(),
            functions: vec![
                Function {
                    package: "app.main".to_string(),
                    name: "ratio".to_string(),
                    params: vec![Parameter {
                        name: "age".to_string(),
                        mutable: false,
                        value_type: ValueType::Int,
                    }],
                    return_type: ValueType::Float,
                    body: vec![Statement::Return(Some(ValueExpr::Cast {
                        expr: Box::new(ValueExpr::Variable("age".to_string())),
                        target_type: ValueType::Float,
                    }))],
                },
                Function {
                    package: "app.main".to_string(),
                    name: "main".to_string(),
                    params: Vec::new(),
                    return_type: ValueType::Void,
                    body: vec![
                        Statement::Let {
                            name: "pi".to_string(),
                            value_type: ValueType::Float,
                            initializer: ValueExpr::FloatLiteral("3.14".to_string()),
                        },
                        Statement::Println(ValueExpr::StringLiteral("done".to_string())),
                    ],
                },
            ],
        };

        let c = emit_c(&program);
        assert!(c.contains("double nomo_fn_ratio(long long nomo_age);"));
        assert!(c.contains("return ((double)nomo_age);"));
        assert!(c.contains("double nomo_pi = 3.14;"));
    }

    #[test]
    fn emits_char_literal() {
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: vec!["std.io".to_string()],
            structs: Vec::new(),
            enums: Vec::new(),
            functions: vec![
                Function {
                    package: "app.main".to_string(),
                    name: "initial".to_string(),
                    params: Vec::new(),
                    return_type: ValueType::Char,
                    body: vec![Statement::Return(Some(ValueExpr::CharLiteral('語')))],
                },
                Function {
                    package: "app.main".to_string(),
                    name: "main".to_string(),
                    params: Vec::new(),
                    return_type: ValueType::Void,
                    body: vec![
                        Statement::Let {
                            name: "letter".to_string(),
                            value_type: ValueType::Char,
                            initializer: ValueExpr::Call {
                                name: "initial".to_string(),
                                args: Vec::new(),
                            },
                        },
                        Statement::Println(ValueExpr::StringLiteral("done".to_string())),
                    ],
                },
            ],
        };

        let c = emit_c(&program);
        assert!(c.contains("uint32_t nomo_fn_initial(void);"));
        assert!(c.contains("return 35486;"));
        assert!(c.contains("uint32_t nomo_letter = nomo_fn_initial();"));
    }

    #[test]
    fn emits_char_helpers() {
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: vec!["std.char".to_string()],
            structs: Vec::new(),
            enums: Vec::new(),
            functions: vec![Function {
                package: "app.main".to_string(),
                name: "main".to_string(),
                params: Vec::new(),
                return_type: ValueType::Void,
                body: vec![
                    Statement::Let {
                        name: "digit".to_string(),
                        value_type: ValueType::Bool,
                        initializer: ValueExpr::CharIsDigit {
                            value: Box::new(ValueExpr::CharLiteral('7')),
                        },
                    },
                    Statement::Let {
                        name: "alpha".to_string(),
                        value_type: ValueType::Bool,
                        initializer: ValueExpr::CharIsAlpha {
                            value: Box::new(ValueExpr::CharLiteral('N')),
                        },
                    },
                    Statement::Let {
                        name: "space".to_string(),
                        value_type: ValueType::Bool,
                        initializer: ValueExpr::CharIsWhitespace {
                            value: Box::new(ValueExpr::CharLiteral(' ')),
                        },
                    },
                    Statement::Let {
                        name: "text".to_string(),
                        value_type: ValueType::String,
                        initializer: ValueExpr::CharToString {
                            value: Box::new(ValueExpr::CharLiteral('語')),
                        },
                    },
                ],
            }],
        };

        let c = emit_c(&program);
        assert!(c.contains("static int nomo_char_is_digit(uint32_t value)"));
        assert!(c.contains("static nomo_string nomo_char_to_string(uint32_t value)"));
        assert!(c.contains("int nomo_digit = nomo_char_is_digit(55);"));
        assert!(c.contains("nomo_string nomo_text = nomo_char_to_string(35486);"));
    }

    #[test]
    fn emits_os_helpers() {
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: vec!["std.os".to_string()],
            structs: Vec::new(),
            enums: Vec::new(),
            functions: vec![Function {
                package: "app.main".to_string(),
                name: "main".to_string(),
                params: Vec::new(),
                return_type: ValueType::Void,
                body: vec![
                    Statement::Let {
                        name: "platform".to_string(),
                        value_type: ValueType::String,
                        initializer: ValueExpr::OsPlatform,
                    },
                    Statement::Let {
                        name: "arch".to_string(),
                        value_type: ValueType::String,
                        initializer: ValueExpr::OsArch,
                    },
                    Statement::Let {
                        name: "separator".to_string(),
                        value_type: ValueType::String,
                        initializer: ValueExpr::OsPathSeparator,
                    },
                    Statement::Let {
                        name: "ending".to_string(),
                        value_type: ValueType::String,
                        initializer: ValueExpr::OsLineEnding,
                    },
                ],
            }],
        };

        let c = emit_c(&program);
        assert!(c.contains("static nomo_string nomo_os_platform(void)"));
        assert!(c.contains("static nomo_string nomo_os_arch(void)"));
        assert!(c.contains("nomo_string nomo_platform = nomo_os_platform();"));
        assert!(c.contains("nomo_string nomo_separator = nomo_os_path_separator();"));
    }

    #[test]
    fn emits_time_helpers() {
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: vec!["std.time".to_string()],
            structs: Vec::new(),
            enums: Vec::new(),
            functions: vec![Function {
                package: "app.main".to_string(),
                name: "main".to_string(),
                params: Vec::new(),
                return_type: ValueType::Void,
                body: vec![
                    Statement::Let {
                        name: "now".to_string(),
                        value_type: ValueType::Int,
                        initializer: ValueExpr::TimeNowMillis,
                    },
                    Statement::Let {
                        name: "tick".to_string(),
                        value_type: ValueType::Int,
                        initializer: ValueExpr::TimeMonotonicMillis,
                    },
                    Statement::Expr(ValueExpr::TimeSleepMillis {
                        duration: Box::new(ValueExpr::IntLiteral(0)),
                    }),
                ],
            }],
        };

        let c = emit_c(&program);
        assert!(c.contains("static int64_t nomo_time_now_millis(void)"));
        assert!(c.contains("static int64_t nomo_time_monotonic_millis(void)"));
        assert!(c.contains("static void nomo_time_sleep_millis(int64_t duration)"));
        assert!(c.contains("nomo_now = nomo_time_now_millis();"));
        assert!(c.contains("nomo_time_sleep_millis(0);"));
    }

    #[test]
    fn emits_process_helpers() {
        let process_error = ValueType::Struct("ProcessError".to_string(), Vec::new());
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: vec!["std.process".to_string()],
            structs: vec![
                StructType {
                    package: "std.process".to_string(),
                    name: "ProcessError".to_string(),
                    type_params: Vec::new(),
                    fields: vec![StructField {
                        name: "message".to_string(),
                        value_type: ValueType::String,
                    }],
                },
                StructType {
                    package: "std.process".to_string(),
                    name: "ProcessOutput".to_string(),
                    type_params: Vec::new(),
                    fields: vec![
                        StructField {
                            name: "status".to_string(),
                            value_type: ValueType::I32,
                        },
                        StructField {
                            name: "stdout".to_string(),
                            value_type: ValueType::String,
                        },
                        StructField {
                            name: "stderr".to_string(),
                            value_type: ValueType::String,
                        },
                    ],
                },
            ],
            enums: vec![EnumType {
                package: "std.result".to_string(),
                name: "Result".to_string(),
                type_params: vec!["T".to_string(), "E".to_string()],
                variants: vec![
                    EnumVariantType {
                        name: "Ok".to_string(),
                        payload: Some(ValueType::TypeParam("T".to_string())),
                    },
                    EnumVariantType {
                        name: "Err".to_string(),
                        payload: Some(ValueType::TypeParam("E".to_string())),
                    },
                ],
            }],
            functions: vec![Function {
                package: "app.main".to_string(),
                name: "main".to_string(),
                params: Vec::new(),
                return_type: ValueType::Void,
                body: vec![
                    Statement::Let {
                        name: "status".to_string(),
                        value_type: ValueType::Enum(
                            "Result".to_string(),
                            vec![ValueType::I32, process_error.clone()],
                        ),
                        initializer: ValueExpr::ProcessStatus {
                            command: Box::new(ValueExpr::StringLiteral("printf ok".to_string())),
                        },
                    },
                    Statement::Let {
                        name: "output".to_string(),
                        value_type: ValueType::Enum(
                            "Result".to_string(),
                            vec![ValueType::String, process_error.clone()],
                        ),
                        initializer: ValueExpr::ProcessExec {
                            command: Box::new(ValueExpr::StringLiteral("printf ok".to_string())),
                        },
                    },
                    Statement::Let {
                        name: "captured".to_string(),
                        value_type: ValueType::Enum(
                            "Result".to_string(),
                            vec![
                                ValueType::Struct("ProcessOutput".to_string(), Vec::new()),
                                process_error,
                            ],
                        ),
                        initializer: ValueExpr::ProcessOutput {
                            command: Box::new(ValueExpr::StringLiteral(
                                "printf ok; printf err 1>&2".to_string(),
                            )),
                        },
                    },
                    Statement::Expr(ValueExpr::ProcessExit {
                        code: Box::new(ValueExpr::IntLiteral(0)),
                    }),
                ],
            }],
        };

        let c = emit_c(&program);
        assert!(c.contains("static int32_t nomo_process_exit_code(int status)"));
        assert!(c.contains("nomo_process_status(nomo_string command)"));
        assert!(c.contains("nomo_process_exec(nomo_string command)"));
        assert!(c.contains("nomo_process_output(nomo_string command)"));
        assert!(
            c.contains("nomo_status = nomo_process_status(nomo_string_literal(\"printf ok\"));")
        );
        assert!(c.contains("nomo_output = nomo_process_exec(nomo_string_literal(\"printf ok\"));"));
        assert!(
            c.contains("nomo_captured = nomo_process_output(nomo_string_literal(\"printf ok; printf err 1>&2\"));")
        );
        assert!(c.contains("exit((int)0);"));
    }

    #[test]
    fn emits_fixed_width_integer_types() {
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: vec!["std.io".to_string()],
            structs: Vec::new(),
            enums: Vec::new(),
            functions: vec![
                Function {
                    package: "app.main".to_string(),
                    name: "add32".to_string(),
                    params: vec![
                        Parameter {
                            name: "a".to_string(),
                            mutable: false,
                            value_type: ValueType::I32,
                        },
                        Parameter {
                            name: "b".to_string(),
                            mutable: false,
                            value_type: ValueType::I32,
                        },
                    ],
                    return_type: ValueType::I32,
                    body: vec![Statement::Return(Some(ValueExpr::Binary {
                        left: Box::new(ValueExpr::Variable("a".to_string())),
                        op: BinaryOp::Add,
                        right: Box::new(ValueExpr::Variable("b".to_string())),
                        value_type: ValueType::I32,
                    }))],
                },
                Function {
                    package: "app.main".to_string(),
                    name: "main".to_string(),
                    params: Vec::new(),
                    return_type: ValueType::Void,
                    body: vec![
                        Statement::Let {
                            name: "signed".to_string(),
                            value_type: ValueType::I32,
                            initializer: ValueExpr::IntLiteral(1),
                        },
                        Statement::Let {
                            name: "word".to_string(),
                            value_type: ValueType::U32,
                            initializer: ValueExpr::IntLiteral(2),
                        },
                        Statement::Let {
                            name: "wide".to_string(),
                            value_type: ValueType::U64,
                            initializer: ValueExpr::IntLiteral(3),
                        },
                        Statement::Println(ValueExpr::StringLiteral("done".to_string())),
                    ],
                },
            ],
        };

        let c = emit_c(&program);
        assert!(c.contains("#include <stdint.h>"));
        assert!(c.contains("int32_t nomo_fn_add32(int32_t nomo_a, int32_t nomo_b);"));
        assert!(c.contains("int32_t nomo_signed = 1;"));
        assert!(c.contains("uint32_t nomo_word = 2;"));
        assert!(c.contains("uint64_t nomo_wide = 3;"));
    }

    #[test]
    fn emits_string_len_and_concat() {
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: vec!["std.io".to_string(), "std.string".to_string()],
            structs: Vec::new(),
            enums: Vec::new(),
            functions: vec![Function {
                package: "app.main".to_string(),
                name: "main".to_string(),
                params: Vec::new(),
                return_type: ValueType::Void,
                body: vec![
                    Statement::Let {
                        name: "message".to_string(),
                        value_type: ValueType::String,
                        initializer: ValueExpr::StringConcat {
                            left: Box::new(ValueExpr::StringLiteral("No".to_string())),
                            right: Box::new(ValueExpr::StringLiteral("mo".to_string())),
                        },
                    },
                    Statement::Let {
                        name: "count".to_string(),
                        value_type: ValueType::U64,
                        initializer: ValueExpr::StringLen {
                            value: Box::new(ValueExpr::Variable("message".to_string())),
                        },
                    },
                ],
            }],
        };

        let c = emit_c(&program);
        assert!(c.contains("#include <string.h>"));
        assert!(c.contains("static nomo_string nomo_string_concat"));
        assert!(c.contains(
            "nomo_string nomo_message = nomo_string_concat(nomo_string_literal(\"No\"), nomo_string_literal(\"mo\"));"
        ));
        assert!(c.contains("uint64_t nomo_count = ((uint64_t)strlen((nomo_message).data));"));
        assert!(c.contains("nomo_string_release(nomo_message);"));
    }

    #[test]
    fn emits_string_retain_and_release_for_shared_bindings() {
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: vec!["std.io".to_string(), "std.string".to_string()],
            structs: Vec::new(),
            enums: Vec::new(),
            functions: vec![Function {
                package: "app.main".to_string(),
                name: "main".to_string(),
                params: Vec::new(),
                return_type: ValueType::Void,
                body: vec![
                    Statement::Let {
                        name: "first".to_string(),
                        value_type: ValueType::String,
                        initializer: ValueExpr::StringConcat {
                            left: Box::new(ValueExpr::StringLiteral("No".to_string())),
                            right: Box::new(ValueExpr::StringLiteral("mo".to_string())),
                        },
                    },
                    Statement::Let {
                        name: "second".to_string(),
                        value_type: ValueType::String,
                        initializer: ValueExpr::Variable("first".to_string()),
                    },
                ],
            }],
        };

        let c = emit_c(&program);
        assert!(c.contains("typedef struct nomo_string"));
        assert!(c.contains("static nomo_string nomo_string_retain(nomo_string value)"));
        assert!(c.contains("static void nomo_string_release(nomo_string value)"));
        assert!(c.contains("nomo_second = nomo_string_retain(nomo_second);"));
        let retain = c
            .find("nomo_second = nomo_string_retain(nomo_second);")
            .unwrap();
        let release_second = c[retain..]
            .find("nomo_string_release(nomo_second);")
            .unwrap()
            + retain;
        let release_first = c[release_second..]
            .find("nomo_string_release(nomo_first);")
            .unwrap()
            + release_second;
        assert!(retain < release_second);
        assert!(release_second < release_first);
    }

    #[test]
    fn emits_string_parameter_retain_before_return_release() {
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: Vec::new(),
            structs: Vec::new(),
            enums: Vec::new(),
            functions: vec![
                Function {
                    package: "app.main".to_string(),
                    name: "echo".to_string(),
                    params: vec![Parameter {
                        name: "value".to_string(),
                        mutable: false,
                        value_type: ValueType::String,
                    }],
                    return_type: ValueType::String,
                    body: vec![Statement::Return(Some(ValueExpr::Variable(
                        "value".to_string(),
                    )))],
                },
                Function {
                    package: "app.main".to_string(),
                    name: "main".to_string(),
                    params: Vec::new(),
                    return_type: ValueType::Void,
                    body: Vec::new(),
                },
            ],
        };

        let c = emit_c(&program);
        let fn_start = c
            .find("nomo_string nomo_fn_echo(nomo_string nomo_value)")
            .unwrap();
        let param_retain = c[fn_start..]
            .find("nomo_value = nomo_string_retain(nomo_value);")
            .unwrap()
            + fn_start;
        let return_retain = c[param_retain..]
            .find("nomo__return = nomo_string_retain(nomo__return);")
            .unwrap()
            + param_retain;
        let param_release = c[return_retain..]
            .find("nomo_string_release(nomo_value);")
            .unwrap()
            + return_retain;
        let return_stmt = c[param_release..].find("return nomo__return;").unwrap() + param_release;
        assert!(fn_start < param_retain);
        assert!(param_retain < return_retain);
        assert!(return_retain < param_release);
        assert!(param_release < return_stmt);
    }

    #[test]
    fn emits_fs_read_and_write_helpers() {
        let fs_error = ValueType::Struct("FsError".to_string(), Vec::new());
        let result_string_error = ValueType::Enum(
            "Result".to_string(),
            vec![ValueType::String, fs_error.clone()],
        );
        let result_void_error = ValueType::Enum(
            "Result".to_string(),
            vec![ValueType::Void, fs_error.clone()],
        );
        let result_metadata_error = ValueType::Enum(
            "Result".to_string(),
            vec![
                ValueType::Struct("FileMetadata".to_string(), Vec::new()),
                fs_error.clone(),
            ],
        );
        let result_array_string_error = ValueType::Enum(
            "Result".to_string(),
            vec![
                ValueType::Array(Box::new(ValueType::String)),
                fs_error.clone(),
            ],
        );
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: vec!["std.fs".to_string()],
            structs: vec![
                StructType {
                    package: "app.main".to_string(),
                    name: "FsError".to_string(),
                    type_params: Vec::new(),
                    fields: vec![StructField {
                        name: "message".to_string(),
                        value_type: ValueType::String,
                    }],
                },
                StructType {
                    package: "app.main".to_string(),
                    name: "FileMetadata".to_string(),
                    type_params: Vec::new(),
                    fields: vec![
                        StructField {
                            name: "is_file".to_string(),
                            value_type: ValueType::Bool,
                        },
                        StructField {
                            name: "is_dir".to_string(),
                            value_type: ValueType::Bool,
                        },
                        StructField {
                            name: "size".to_string(),
                            value_type: ValueType::U64,
                        },
                    ],
                },
            ],
            enums: vec![
                EnumType {
                    package: "app.main".to_string(),
                    name: "Option".to_string(),
                    type_params: vec!["T".to_string()],
                    variants: vec![
                        EnumVariantType {
                            name: "Some".to_string(),
                            payload: Some(ValueType::TypeParam("T".to_string())),
                        },
                        EnumVariantType {
                            name: "None".to_string(),
                            payload: None,
                        },
                    ],
                },
                EnumType {
                    package: "app.main".to_string(),
                    name: "Result".to_string(),
                    type_params: vec!["T".to_string(), "E".to_string()],
                    variants: vec![
                        EnumVariantType {
                            name: "Ok".to_string(),
                            payload: Some(ValueType::TypeParam("T".to_string())),
                        },
                        EnumVariantType {
                            name: "Err".to_string(),
                            payload: Some(ValueType::TypeParam("E".to_string())),
                        },
                    ],
                },
            ],
            functions: vec![Function {
                package: "app.main".to_string(),
                name: "main".to_string(),
                params: Vec::new(),
                return_type: ValueType::Void,
                body: vec![
                    Statement::Let {
                        name: "read_result".to_string(),
                        value_type: result_string_error,
                        initializer: ValueExpr::FsReadToString {
                            path: Box::new(ValueExpr::StringLiteral("input.txt".to_string())),
                        },
                    },
                    Statement::Let {
                        name: "write_result".to_string(),
                        value_type: result_void_error.clone(),
                        initializer: ValueExpr::FsWriteString {
                            path: Box::new(ValueExpr::StringLiteral("output.txt".to_string())),
                            content: Box::new(ValueExpr::StringLiteral("hello".to_string())),
                        },
                    },
                    Statement::Let {
                        name: "exists".to_string(),
                        value_type: ValueType::Bool,
                        initializer: ValueExpr::FsExists {
                            path: Box::new(ValueExpr::StringLiteral("tmp".to_string())),
                        },
                    },
                    Statement::Let {
                        name: "metadata_result".to_string(),
                        value_type: result_metadata_error,
                        initializer: ValueExpr::FsMetadata {
                            path: Box::new(ValueExpr::StringLiteral("tmp".to_string())),
                        },
                    },
                    Statement::Let {
                        name: "create_result".to_string(),
                        value_type: result_void_error.clone(),
                        initializer: ValueExpr::FsCreateDir {
                            path: Box::new(ValueExpr::StringLiteral("tmp".to_string())),
                        },
                    },
                    Statement::Let {
                        name: "entries_result".to_string(),
                        value_type: result_array_string_error,
                        initializer: ValueExpr::FsReadDir {
                            path: Box::new(ValueExpr::StringLiteral("tmp".to_string())),
                        },
                    },
                    Statement::Let {
                        name: "remove_result".to_string(),
                        value_type: result_void_error,
                        initializer: ValueExpr::FsRemoveDir {
                            path: Box::new(ValueExpr::StringLiteral("tmp".to_string())),
                        },
                    },
                ],
            }],
        };

        let c = emit_c(&program);
        assert!(c.contains("#include <errno.h>"));
        assert!(c.contains("typedef struct nomo_struct_FsError"));
        assert!(c.contains("typedef struct nomo_struct_FileMetadata"));
        assert!(c.contains("static nomo_enum_Result_string_struct_FsError nomo_fs_read_to_string"));
        assert!(c.contains("static nomo_enum_Result_void_struct_FsError nomo_fs_write_string"));
        assert!(c.contains("static int nomo_fs_exists"));
        assert!(c.contains(
            "static nomo_enum_Result_struct_FileMetadata_struct_FsError nomo_fs_metadata"
        ));
        assert!(c.contains("static nomo_enum_Result_void_struct_FsError nomo_fs_create_dir"));
        assert!(c.contains("static nomo_enum_Result_void_struct_FsError nomo_fs_remove_dir"));
        assert!(c.contains("static nomo_enum_Result_array_string_struct_FsError nomo_fs_read_dir"));
        assert!(c.contains("typedef struct nomo_array_string"));
        assert!(c.contains("nomo_fs_read_to_string(nomo_string_literal(\"input.txt\"))"));
        assert!(c.contains(
            "nomo_fs_write_string(nomo_string_literal(\"output.txt\"), nomo_string_literal(\"hello\"))"
        ));
        assert!(c.contains("nomo_fs_exists(nomo_string_literal(\"tmp\"))"));
        assert!(c.contains("nomo_fs_metadata(nomo_string_literal(\"tmp\"))"));
        assert!(c.contains("nomo_fs_create_dir(nomo_string_literal(\"tmp\"))"));
        assert!(c.contains("nomo_fs_read_dir(nomo_string_literal(\"tmp\"))"));
        assert!(c.contains("nomo_fs_remove_dir(nomo_string_literal(\"tmp\"))"));
    }

    #[test]
    fn emits_file_read_write_close_helpers() {
        let fs_error = ValueType::Struct("FsError".to_string(), Vec::new());
        let file_type = ValueType::Struct("File".to_string(), Vec::new());
        let result_string_error = ValueType::Enum(
            "Result".to_string(),
            vec![ValueType::String, fs_error.clone()],
        );
        let result_void_error = ValueType::Enum(
            "Result".to_string(),
            vec![ValueType::Void, fs_error.clone()],
        );
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: vec!["std.fs".to_string()],
            structs: vec![
                StructType {
                    package: "app.main".to_string(),
                    name: "FsError".to_string(),
                    type_params: Vec::new(),
                    fields: vec![StructField {
                        name: "message".to_string(),
                        value_type: ValueType::String,
                    }],
                },
                StructType {
                    package: "app.main".to_string(),
                    name: "File".to_string(),
                    type_params: Vec::new(),
                    fields: Vec::new(),
                },
            ],
            enums: vec![EnumType {
                package: "app.main".to_string(),
                name: "Result".to_string(),
                type_params: vec!["T".to_string(), "E".to_string()],
                variants: vec![
                    EnumVariantType {
                        name: "Ok".to_string(),
                        payload: Some(ValueType::TypeParam("T".to_string())),
                    },
                    EnumVariantType {
                        name: "Err".to_string(),
                        payload: Some(ValueType::TypeParam("E".to_string())),
                    },
                ],
            }],
            functions: vec![
                Function {
                    package: "app.main".to_string(),
                    name: "process_file".to_string(),
                    params: vec![Parameter {
                        name: "file".to_string(),
                        mutable: false,
                        value_type: file_type,
                    }],
                    return_type: ValueType::Void,
                    body: vec![
                        Statement::Let {
                            name: "write_result".to_string(),
                            value_type: result_void_error,
                            initializer: ValueExpr::FileWriteString {
                                file: Box::new(ValueExpr::Variable("file".to_string())),
                                content: Box::new(ValueExpr::StringLiteral("hello".to_string())),
                            },
                        },
                        Statement::Let {
                            name: "read_result".to_string(),
                            value_type: result_string_error,
                            initializer: ValueExpr::FileReadToString {
                                file: Box::new(ValueExpr::Variable("file".to_string())),
                            },
                        },
                        Statement::Expr(ValueExpr::FileClose {
                            file: Box::new(ValueExpr::Variable("file".to_string())),
                        }),
                    ],
                },
                Function {
                    package: "app.main".to_string(),
                    name: "main".to_string(),
                    params: Vec::new(),
                    return_type: ValueType::Void,
                    body: Vec::new(),
                },
            ],
        };

        let c = emit_c(&program);
        assert!(c.contains("typedef struct nomo_struct_File"));
        assert!(
            c.contains("static nomo_enum_Result_string_struct_FsError nomo_file_read_to_string")
        );
        assert!(c.contains("static nomo_enum_Result_void_struct_FsError nomo_file_write_string"));
        assert!(c.contains("static void nomo_file_close"));
        assert!(c.contains("nomo_file_write_string(nomo_file, nomo_string_literal(\"hello\"))"));
        assert!(c.contains("nomo_file_read_to_string(nomo_file)"));
        assert!(c.contains("nomo_file_close(nomo_file)"));
    }

    #[test]
    fn emits_io_read_line_helper() {
        let io_error = ValueType::Struct("IoError".to_string(), Vec::new());
        let result_string_error =
            ValueType::Enum("Result".to_string(), vec![ValueType::String, io_error]);
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: vec!["std.io".to_string()],
            structs: vec![StructType {
                package: "std.io".to_string(),
                name: "IoError".to_string(),
                type_params: Vec::new(),
                fields: vec![StructField {
                    name: "message".to_string(),
                    value_type: ValueType::String,
                }],
            }],
            enums: vec![EnumType {
                package: "std.result".to_string(),
                name: "Result".to_string(),
                type_params: vec!["T".to_string(), "E".to_string()],
                variants: vec![
                    EnumVariantType {
                        name: "Ok".to_string(),
                        payload: Some(ValueType::TypeParam("T".to_string())),
                    },
                    EnumVariantType {
                        name: "Err".to_string(),
                        payload: Some(ValueType::TypeParam("E".to_string())),
                    },
                ],
            }],
            functions: vec![Function {
                package: "app.main".to_string(),
                name: "main".to_string(),
                params: Vec::new(),
                return_type: ValueType::Void,
                body: vec![Statement::Let {
                    name: "read_result".to_string(),
                    value_type: result_string_error,
                    initializer: ValueExpr::IoReadLine,
                }],
            }],
        };

        let c = emit_c(&program);
        assert!(c.contains("typedef struct nomo_struct_IoError"));
        assert!(c.contains("static nomo_enum_Result_string_struct_IoError nomo_io_read_line"));
        assert!(c.contains("nomo_io_read_line()"));
    }

    #[test]
    fn emits_num_helpers() {
        let num_error = ValueType::Struct("NumError".to_string(), Vec::new());
        let result_i64_error = ValueType::Enum(
            "Result".to_string(),
            vec![ValueType::Int, num_error.clone()],
        );
        let result_u64_error = ValueType::Enum(
            "Result".to_string(),
            vec![ValueType::U64, num_error.clone()],
        );
        let result_f64_error =
            ValueType::Enum("Result".to_string(), vec![ValueType::Float, num_error]);
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: vec!["std.num".to_string()],
            structs: vec![StructType {
                package: "std.num".to_string(),
                name: "NumError".to_string(),
                type_params: Vec::new(),
                fields: vec![StructField {
                    name: "message".to_string(),
                    value_type: ValueType::String,
                }],
            }],
            enums: vec![EnumType {
                package: "std.result".to_string(),
                name: "Result".to_string(),
                type_params: vec!["T".to_string(), "E".to_string()],
                variants: vec![
                    EnumVariantType {
                        name: "Ok".to_string(),
                        payload: Some(ValueType::TypeParam("T".to_string())),
                    },
                    EnumVariantType {
                        name: "Err".to_string(),
                        payload: Some(ValueType::TypeParam("E".to_string())),
                    },
                ],
            }],
            functions: vec![Function {
                package: "app.main".to_string(),
                name: "main".to_string(),
                params: Vec::new(),
                return_type: ValueType::Void,
                body: vec![
                    Statement::Let {
                        name: "integer".to_string(),
                        value_type: result_i64_error,
                        initializer: ValueExpr::NumParseI64 {
                            value: Box::new(ValueExpr::StringLiteral("42".to_string())),
                        },
                    },
                    Statement::Let {
                        name: "unsigned".to_string(),
                        value_type: result_u64_error,
                        initializer: ValueExpr::NumParseU64 {
                            value: Box::new(ValueExpr::StringLiteral("7".to_string())),
                        },
                    },
                    Statement::Let {
                        name: "decimal".to_string(),
                        value_type: result_f64_error,
                        initializer: ValueExpr::NumParseF64 {
                            value: Box::new(ValueExpr::StringLiteral("3.5".to_string())),
                        },
                    },
                    Statement::Let {
                        name: "text".to_string(),
                        value_type: ValueType::String,
                        initializer: ValueExpr::NumToString {
                            value: Box::new(ValueExpr::IntLiteral(42)),
                            value_type: ValueType::Int,
                        },
                    },
                ],
            }],
        };

        let c = emit_c(&program);
        assert!(c.contains("typedef struct nomo_struct_NumError"));
        assert!(c.contains("static nomo_enum_Result_i64_struct_NumError nomo_num_parse_i64"));
        assert!(c.contains("static nomo_enum_Result_u64_struct_NumError nomo_num_parse_u64"));
        assert!(c.contains("static nomo_enum_Result_f64_struct_NumError nomo_num_parse_f64"));
        assert!(c.contains("nomo_num_parse_i64(nomo_string_literal(\"42\"))"));
        assert!(c.contains("nomo_num_i64_to_string(42)"));
    }

    #[test]
    fn emits_num_checked_and_wrapping_helpers() {
        let option_i64 = ValueType::Enum("Option".to_string(), vec![ValueType::Int]);
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: vec!["std.num".to_string()],
            structs: Vec::new(),
            enums: vec![EnumType {
                package: "std.option".to_string(),
                name: "Option".to_string(),
                type_params: vec!["T".to_string()],
                variants: vec![
                    EnumVariantType {
                        name: "Some".to_string(),
                        payload: Some(ValueType::TypeParam("T".to_string())),
                    },
                    EnumVariantType {
                        name: "None".to_string(),
                        payload: None,
                    },
                ],
            }],
            functions: vec![Function {
                package: "app.main".to_string(),
                name: "main".to_string(),
                params: Vec::new(),
                return_type: ValueType::Void,
                body: vec![
                    Statement::Let {
                        name: "checked".to_string(),
                        value_type: option_i64,
                        initializer: ValueExpr::NumBinary {
                            function: NumBinaryFunction::Checked,
                            op: BinaryOp::Add,
                            left: Box::new(ValueExpr::IntLiteral(i64::MAX)),
                            right: Box::new(ValueExpr::IntLiteral(1)),
                            value_type: ValueType::Int,
                        },
                    },
                    Statement::Let {
                        name: "wrapped".to_string(),
                        value_type: ValueType::Int,
                        initializer: ValueExpr::NumBinary {
                            function: NumBinaryFunction::Wrapping,
                            op: BinaryOp::Subtract,
                            left: Box::new(ValueExpr::IntLiteral(i64::MIN)),
                            right: Box::new(ValueExpr::IntLiteral(1)),
                            value_type: ValueType::Int,
                        },
                    },
                ],
            }],
        };

        let c = emit_c(&program);
        assert!(c.contains("typedef enum nomo_enum_Option_i64_tag"));
        assert!(c.contains("static nomo_enum_Option_i64 nomo_num_checked_add_i64"));
        assert!(c.contains("nomo_num_checked_add_i64(9223372036854775807, 1)"));
        assert!(c.contains("static long long nomo_num_wrapping_sub_i64"));
        assert!(c.contains("nomo_wrapped = nomo_num_wrapping_sub_i64("));
    }

    #[test]
    fn emits_env_get_helper() {
        let option_string = ValueType::Enum("Option".to_string(), vec![ValueType::String]);
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: vec!["std.env".to_string()],
            structs: Vec::new(),
            enums: vec![EnumType {
                package: "app.main".to_string(),
                name: "Option".to_string(),
                type_params: vec!["T".to_string()],
                variants: vec![
                    EnumVariantType {
                        name: "Some".to_string(),
                        payload: Some(ValueType::TypeParam("T".to_string())),
                    },
                    EnumVariantType {
                        name: "None".to_string(),
                        payload: None,
                    },
                ],
            }],
            functions: vec![Function {
                package: "app.main".to_string(),
                name: "main".to_string(),
                params: Vec::new(),
                return_type: ValueType::Void,
                body: vec![Statement::Let {
                    name: "value".to_string(),
                    value_type: option_string,
                    initializer: ValueExpr::EnvGet {
                        name: Box::new(ValueExpr::StringLiteral("NOMO_TEST_ENV".to_string())),
                    },
                }],
            }],
        };

        let c = emit_c(&program);
        assert!(c.contains("static nomo_enum_Option_string nomo_env_get"));
        assert!(c.contains("getenv(name.data)"));
        assert!(c.contains("nomo_env_get(nomo_string_literal(\"NOMO_TEST_ENV\"))"));
    }

    #[test]
    #[should_panic(expected = "unsupported Array element type reached C type lowering")]
    fn panics_instead_of_emitting_unsupported_array_placeholders() {
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: Vec::new(),
            structs: Vec::new(),
            enums: Vec::new(),
            functions: vec![Function {
                package: "app.main".to_string(),
                name: "main".to_string(),
                params: Vec::new(),
                return_type: ValueType::Void,
                body: vec![Statement::Let {
                    name: "bad".to_string(),
                    value_type: ValueType::Array(Box::new(ValueType::Void)),
                    initializer: ValueExpr::ArrayNew {
                        element_type: ValueType::Void,
                    },
                }],
            }],
        };

        let _ = emit_c(&program);
    }

    #[test]
    fn emits_env_args_helper_and_main_arguments() {
        let array_string = ValueType::Array(Box::new(ValueType::String));
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: vec!["std.env".to_string()],
            structs: Vec::new(),
            enums: vec![EnumType {
                package: "app.main".to_string(),
                name: "Option".to_string(),
                type_params: vec!["T".to_string()],
                variants: vec![
                    EnumVariantType {
                        name: "Some".to_string(),
                        payload: Some(ValueType::TypeParam("T".to_string())),
                    },
                    EnumVariantType {
                        name: "None".to_string(),
                        payload: None,
                    },
                ],
            }],
            functions: vec![Function {
                package: "app.main".to_string(),
                name: "main".to_string(),
                params: Vec::new(),
                return_type: ValueType::Void,
                body: vec![Statement::Let {
                    name: "args".to_string(),
                    value_type: array_string,
                    initializer: ValueExpr::EnvArgs,
                }],
            }],
        };

        let c = emit_c(&program);
        assert!(c.contains("int main(int argc, char **argv)"));
        assert!(c.contains("static int nomo_argc = 0;"));
        assert!(c.contains("static char **nomo_argv = NULL;"));
        assert!(c.contains("static nomo_array_string nomo_env_args(int argc, char **argv)"));
        assert!(c.contains("nomo_argc = argc;"));
        assert!(c.contains("nomo_argv = argv;"));
        assert!(c.contains("nomo_array_string nomo_args = nomo_env_args(nomo_argc, nomo_argv);"));
    }

    #[test]
    fn emits_string_array_helpers() {
        let array_string = ValueType::Array(Box::new(ValueType::String));
        let option_string = ValueType::Enum("Option".to_string(), vec![ValueType::String]);
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: vec!["std.array".to_string()],
            structs: Vec::new(),
            enums: vec![EnumType {
                package: "app.main".to_string(),
                name: "Option".to_string(),
                type_params: vec!["T".to_string()],
                variants: vec![
                    EnumVariantType {
                        name: "Some".to_string(),
                        payload: Some(ValueType::TypeParam("T".to_string())),
                    },
                    EnumVariantType {
                        name: "None".to_string(),
                        payload: None,
                    },
                ],
            }],
            functions: vec![Function {
                package: "app.main".to_string(),
                name: "main".to_string(),
                params: Vec::new(),
                return_type: ValueType::Void,
                body: vec![
                    Statement::Let {
                        name: "items".to_string(),
                        value_type: array_string.clone(),
                        initializer: ValueExpr::ArrayNew {
                            element_type: ValueType::String,
                        },
                    },
                    Statement::Assign {
                        name: "items".to_string(),
                        value: ValueExpr::ArrayPush {
                            array: "items".to_string(),
                            value: Box::new(ValueExpr::StringLiteral("first".to_string())),
                            element_type: ValueType::String,
                        },
                    },
                    Statement::Let {
                        name: "size".to_string(),
                        value_type: ValueType::U64,
                        initializer: ValueExpr::ArrayLen {
                            array: Box::new(ValueExpr::Variable("items".to_string())),
                        },
                    },
                    Statement::Let {
                        name: "first".to_string(),
                        value_type: option_string,
                        initializer: ValueExpr::ArrayGet {
                            array: Box::new(ValueExpr::Variable("items".to_string())),
                            index: Box::new(ValueExpr::IntLiteral(0)),
                            element_type: ValueType::String,
                        },
                    },
                ],
            }],
        };

        let c = emit_c(&program);
        assert!(c.contains("typedef struct nomo_array_string"));
        assert!(c.contains("nomo_array_string nomo_items = nomo_array_string_new();"));
        assert!(c.contains(
            "nomo_items = nomo_array_string_push(nomo_items, nomo_string_literal(\"first\"));"
        ));
        assert!(c.contains("uint64_t nomo_size = ((uint64_t)nomo_items.len);"));
        assert!(c.contains("nomo_array_string_get(nomo_items, 0)"));
    }

    #[test]
    fn emits_i32_array_helpers() {
        let array_i32 = ValueType::Array(Box::new(ValueType::I32));
        let option_i32 = ValueType::Enum("Option".to_string(), vec![ValueType::I32]);
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: vec!["std.array".to_string()],
            structs: Vec::new(),
            enums: vec![EnumType {
                package: "app.main".to_string(),
                name: "Option".to_string(),
                type_params: vec!["T".to_string()],
                variants: vec![
                    EnumVariantType {
                        name: "Some".to_string(),
                        payload: Some(ValueType::TypeParam("T".to_string())),
                    },
                    EnumVariantType {
                        name: "None".to_string(),
                        payload: None,
                    },
                ],
            }],
            functions: vec![Function {
                package: "app.main".to_string(),
                name: "main".to_string(),
                params: Vec::new(),
                return_type: ValueType::Void,
                body: vec![
                    Statement::Let {
                        name: "items".to_string(),
                        value_type: array_i32.clone(),
                        initializer: ValueExpr::ArrayNew {
                            element_type: ValueType::I32,
                        },
                    },
                    Statement::Assign {
                        name: "items".to_string(),
                        value: ValueExpr::ArrayPush {
                            array: "items".to_string(),
                            value: Box::new(ValueExpr::IntLiteral(7)),
                            element_type: ValueType::I32,
                        },
                    },
                    Statement::Assign {
                        name: "items".to_string(),
                        value: ValueExpr::ArrayInsert {
                            array: "items".to_string(),
                            index: Box::new(ValueExpr::IntLiteral(0)),
                            value: Box::new(ValueExpr::IntLiteral(5)),
                            element_type: ValueType::I32,
                        },
                    },
                    Statement::Let {
                        name: "removed".to_string(),
                        value_type: option_i32.clone(),
                        initializer: ValueExpr::ArrayRemove {
                            array: "items".to_string(),
                            index: Box::new(ValueExpr::IntLiteral(0)),
                            element_type: ValueType::I32,
                        },
                    },
                    Statement::Let {
                        name: "popped".to_string(),
                        value_type: option_i32.clone(),
                        initializer: ValueExpr::ArrayPop {
                            array: "items".to_string(),
                            element_type: ValueType::I32,
                        },
                    },
                    Statement::Let {
                        name: "snapshot".to_string(),
                        value_type: array_i32.clone(),
                        initializer: ValueExpr::ArrayIter {
                            array: Box::new(ValueExpr::Variable("items".to_string())),
                            element_type: ValueType::I32,
                        },
                    },
                    Statement::Assign {
                        name: "items".to_string(),
                        value: ValueExpr::ArrayClear {
                            array: "items".to_string(),
                            element_type: ValueType::I32,
                        },
                    },
                    Statement::Let {
                        name: "first".to_string(),
                        value_type: option_i32,
                        initializer: ValueExpr::ArrayGet {
                            array: Box::new(ValueExpr::Variable("items".to_string())),
                            index: Box::new(ValueExpr::IntLiteral(0)),
                            element_type: ValueType::I32,
                        },
                    },
                ],
            }],
        };

        let c = emit_c(&program);
        assert!(c.contains("typedef struct nomo_array_i32"));
        assert!(c.contains("int32_t *data;"));
        assert!(c.contains("size_t *refcount;"));
        assert!(c.contains("static nomo_array_i32 nomo_array_i32_retain(nomo_array_i32 array)"));
        assert!(c.contains("static void nomo_array_i32_release(nomo_array_i32 array)"));
        assert!(c.contains("if (*array.refcount != 0) { return; }"));
        assert!(c.contains("free(array.data);"));
        assert!(c.contains("free(array.refcount);"));
        assert!(c.contains(
            "static nomo_array_i32 nomo_array_i32_make_unique(nomo_array_i32 array, size_t needed)"
        ));
        assert!(c.contains("array = nomo_array_i32_make_unique(array, array.len + 1);"));
        assert!(c.contains("array = nomo_array_i32_make_unique(array, array.len);"));
        assert!(c.contains("static nomo_array_i32 nomo_array_i32_insert("));
        assert!(c.contains("static nomo_array_i32 nomo_array_i32_clear("));
        assert!(c.contains("static nomo_enum_Option_i32 nomo_array_i32_pop("));
        assert!(c.contains("static nomo_enum_Option_i32 nomo_array_i32_remove("));
        assert!(c.contains("nomo_array_i32 nomo_items = nomo_array_i32_new();"));
        assert!(c.contains("nomo_items = nomo_array_i32_push(nomo_items, 7);"));
        assert!(c.contains("nomo_items = nomo_array_i32_insert(nomo_items, 0, 5);"));
        assert!(c.contains("nomo_array_i32_remove(&nomo_items, 0)"));
        assert!(c.contains("nomo_array_i32_pop(&nomo_items)"));
        assert!(c.contains("nomo_array_i32 nomo_snapshot = nomo_array_i32_retain(nomo_items);"));
        assert!(c.contains("nomo_items = nomo_array_i32_clear(nomo_items);"));
        assert!(c.contains("nomo_array_i32_get(nomo_items, 0)"));
    }

    #[test]
    fn emits_array_retain_for_shared_array_bindings_and_nested_elements() {
        let array_i32 = ValueType::Array(Box::new(ValueType::I32));
        let array_array_i32 = ValueType::Array(Box::new(array_i32.clone()));
        let option_array_i32 = ValueType::Enum("Option".to_string(), vec![array_i32.clone()]);
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: vec!["std.array".to_string()],
            structs: Vec::new(),
            enums: vec![EnumType {
                package: "app.main".to_string(),
                name: "Option".to_string(),
                type_params: vec!["T".to_string()],
                variants: vec![
                    EnumVariantType {
                        name: "Some".to_string(),
                        payload: Some(ValueType::TypeParam("T".to_string())),
                    },
                    EnumVariantType {
                        name: "None".to_string(),
                        payload: None,
                    },
                ],
            }],
            functions: vec![Function {
                package: "app.main".to_string(),
                name: "main".to_string(),
                params: Vec::new(),
                return_type: ValueType::Void,
                body: vec![
                    Statement::Let {
                        name: "inner".to_string(),
                        value_type: array_i32.clone(),
                        initializer: ValueExpr::ArrayNew {
                            element_type: ValueType::I32,
                        },
                    },
                    Statement::Let {
                        name: "snapshot".to_string(),
                        value_type: array_i32.clone(),
                        initializer: ValueExpr::Variable("inner".to_string()),
                    },
                    Statement::Let {
                        name: "outer".to_string(),
                        value_type: array_array_i32,
                        initializer: ValueExpr::ArrayNew {
                            element_type: array_i32.clone(),
                        },
                    },
                    Statement::Assign {
                        name: "outer".to_string(),
                        value: ValueExpr::ArrayPush {
                            array: "outer".to_string(),
                            value: Box::new(ValueExpr::Variable("inner".to_string())),
                            element_type: array_i32.clone(),
                        },
                    },
                    Statement::Let {
                        name: "first".to_string(),
                        value_type: option_array_i32,
                        initializer: ValueExpr::ArrayGet {
                            array: Box::new(ValueExpr::Variable("outer".to_string())),
                            index: Box::new(ValueExpr::IntLiteral(0)),
                            element_type: array_i32,
                        },
                    },
                ],
            }],
        };

        let c = emit_c(&program);
        assert!(c.contains("nomo_snapshot = nomo_array_i32_retain(nomo_snapshot);"));
        assert!(c.contains("array.data[array.len] = nomo_array_i32_retain(value);"));
        assert!(c.contains("nomo_array_i32_retain(array.data[index])"));
        assert!(c.contains("nomo_array_i32_release(nomo_snapshot);"));
        assert!(c.contains("nomo_array_i32_release(nomo_inner);"));
        assert!(c.contains("nomo_array_array_i32_release(nomo_outer);"));
        assert!(c.contains("nomo_array_i32_release(array.data[i]);"));
        assert!(c.contains("nomo_enum_Option_array_i32_release(nomo_first);"));
        assert!(c.contains("if (value.tag == nomo_enum_Option_array_i32_Some) {"));
        assert!(c.contains("nomo_array_i32_release(value.payload.nomo_payload_Some);"));
    }

    #[test]
    fn emits_array_releases_before_return_and_question_error_exit() {
        let array_i32 = ValueType::Array(Box::new(ValueType::I32));
        let result_i32_string = ValueType::Enum(
            "Result".to_string(),
            vec![ValueType::I32, ValueType::String],
        );
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: vec!["std.array".to_string()],
            structs: Vec::new(),
            enums: vec![
                EnumType {
                    package: "app.main".to_string(),
                    name: "Option".to_string(),
                    type_params: vec!["T".to_string()],
                    variants: vec![
                        EnumVariantType {
                            name: "Some".to_string(),
                            payload: Some(ValueType::TypeParam("T".to_string())),
                        },
                        EnumVariantType {
                            name: "None".to_string(),
                            payload: None,
                        },
                    ],
                },
                EnumType {
                    package: "app.main".to_string(),
                    name: "Result".to_string(),
                    type_params: vec!["T".to_string(), "E".to_string()],
                    variants: vec![
                        EnumVariantType {
                            name: "Ok".to_string(),
                            payload: Some(ValueType::TypeParam("T".to_string())),
                        },
                        EnumVariantType {
                            name: "Err".to_string(),
                            payload: Some(ValueType::TypeParam("E".to_string())),
                        },
                    ],
                },
            ],
            functions: vec![
                Function {
                    package: "app.main".to_string(),
                    name: "parse".to_string(),
                    params: Vec::new(),
                    return_type: result_i32_string.clone(),
                    body: vec![Statement::Return(Some(ValueExpr::EnumVariant {
                        enum_name: "Result".to_string(),
                        enum_args: vec![ValueType::I32, ValueType::String],
                        variant: "Ok".to_string(),
                        payload: Some(Box::new(ValueExpr::IntLiteral(7))),
                    }))],
                },
                Function {
                    package: "app.main".to_string(),
                    name: "compute".to_string(),
                    params: Vec::new(),
                    return_type: result_i32_string,
                    body: vec![
                        Statement::Let {
                            name: "items".to_string(),
                            value_type: array_i32.clone(),
                            initializer: ValueExpr::ArrayNew {
                                element_type: ValueType::I32,
                            },
                        },
                        Statement::QuestionLet {
                            carrier: QuestionCarrier::Result,
                            name: "value".to_string(),
                            value_type: ValueType::I32,
                            result_type: ValueType::Enum(
                                "Result".to_string(),
                                vec![ValueType::I32, ValueType::String],
                            ),
                            return_type: ValueType::Enum(
                                "Result".to_string(),
                                vec![ValueType::I32, ValueType::String],
                            ),
                            result_expr: ValueExpr::Call {
                                name: "parse".to_string(),
                                args: Vec::new(),
                            },
                        },
                        Statement::Return(Some(ValueExpr::EnumVariant {
                            enum_name: "Result".to_string(),
                            enum_args: vec![ValueType::I32, ValueType::String],
                            variant: "Ok".to_string(),
                            payload: Some(Box::new(ValueExpr::Variable("value".to_string()))),
                        })),
                    ],
                },
                Function {
                    package: "app.main".to_string(),
                    name: "main".to_string(),
                    params: Vec::new(),
                    return_type: ValueType::Void,
                    body: Vec::new(),
                },
            ],
        };

        let c = emit_c(&program);
        let question_error = c.find("if (nomo_value_result.tag").unwrap();
        let question_temp = c[question_error..]
            .find("nomo_enum_Result_i32_string nomo__question_return =")
            .unwrap();
        let release_in_error = c[question_error..]
            .find("nomo_array_i32_release(nomo_items);")
            .unwrap();
        let question_return = c[question_error..]
            .find("return nomo__question_return;")
            .unwrap();
        assert!(question_temp < release_in_error);
        assert!(release_in_error < question_return);
        let ok_return = c.rfind("return nomo__return;").unwrap();
        let release_before_ok = c[..ok_return]
            .rfind("nomo_array_i32_release(nomo_items);")
            .unwrap();
        assert!(release_before_ok < ok_return);
    }

    #[test]
    fn emits_question_return_ok_with_cleanup_on_error_and_success() {
        let array_i32 = ValueType::Array(Box::new(ValueType::I32));
        let result_i32_string = ValueType::Enum(
            "Result".to_string(),
            vec![ValueType::I32, ValueType::String],
        );
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: vec!["std.array".to_string()],
            structs: Vec::new(),
            enums: vec![
                EnumType {
                    package: "app.main".to_string(),
                    name: "Option".to_string(),
                    type_params: vec!["T".to_string()],
                    variants: vec![
                        EnumVariantType {
                            name: "Some".to_string(),
                            payload: Some(ValueType::TypeParam("T".to_string())),
                        },
                        EnumVariantType {
                            name: "None".to_string(),
                            payload: None,
                        },
                    ],
                },
                EnumType {
                    package: "app.main".to_string(),
                    name: "Result".to_string(),
                    type_params: vec!["T".to_string(), "E".to_string()],
                    variants: vec![
                        EnumVariantType {
                            name: "Ok".to_string(),
                            payload: Some(ValueType::TypeParam("T".to_string())),
                        },
                        EnumVariantType {
                            name: "Err".to_string(),
                            payload: Some(ValueType::TypeParam("E".to_string())),
                        },
                    ],
                },
            ],
            functions: vec![
                Function {
                    package: "app.main".to_string(),
                    name: "parse".to_string(),
                    params: Vec::new(),
                    return_type: result_i32_string.clone(),
                    body: vec![Statement::Return(Some(ValueExpr::EnumVariant {
                        enum_name: "Result".to_string(),
                        enum_args: vec![ValueType::I32, ValueType::String],
                        variant: "Ok".to_string(),
                        payload: Some(Box::new(ValueExpr::IntLiteral(7))),
                    }))],
                },
                Function {
                    package: "app.main".to_string(),
                    name: "compute".to_string(),
                    params: Vec::new(),
                    return_type: result_i32_string.clone(),
                    body: vec![
                        Statement::Let {
                            name: "items".to_string(),
                            value_type: array_i32.clone(),
                            initializer: ValueExpr::ArrayNew {
                                element_type: ValueType::I32,
                            },
                        },
                        Statement::QuestionReturnOk {
                            ok_type: ValueType::I32,
                            result_type: result_i32_string.clone(),
                            return_type: result_i32_string,
                            result_expr: ValueExpr::Call {
                                name: "parse".to_string(),
                                args: Vec::new(),
                            },
                        },
                    ],
                },
                Function {
                    package: "app.main".to_string(),
                    name: "main".to_string(),
                    params: Vec::new(),
                    return_type: ValueType::Void,
                    body: Vec::new(),
                },
            ],
        };

        let c = emit_c(&program);
        let question_result = c.find("nomo__question_result = nomo_fn_parse();").unwrap();
        let error_branch = c[question_result..]
            .find("if (nomo__question_result.tag == nomo_enum_Result_i32_string_Err)")
            .unwrap();
        let question_return = c[question_result..]
            .find("return nomo__question_return;")
            .unwrap();
        let error_release = c[question_result..question_result + question_return]
            .find("nomo_array_i32_release(nomo_items);")
            .unwrap();
        assert!(error_branch < error_release);

        let ok_temp = c[question_result..]
            .find("int32_t nomo__question_ok = nomo__question_result.payload.nomo_payload_Ok;")
            .unwrap();
        let ok_return = c[question_result..].find("return nomo__return;").unwrap();
        let success_release = c[question_result + ok_temp..question_result + ok_return]
            .find("nomo_array_i32_release(nomo_items);")
            .unwrap();
        assert!(success_release < ok_return - ok_temp);
    }

    #[test]
    fn question_let_retains_managed_payloads_when_result_expr_is_shared() {
        let array_i32 = ValueType::Array(Box::new(ValueType::I32));
        let result_array_array = ValueType::Enum(
            "Result".to_string(),
            vec![array_i32.clone(), array_i32.clone()],
        );
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: vec!["std.array".to_string()],
            structs: Vec::new(),
            enums: vec![
                EnumType {
                    package: "app.main".to_string(),
                    name: "Option".to_string(),
                    type_params: vec!["T".to_string()],
                    variants: vec![
                        EnumVariantType {
                            name: "Some".to_string(),
                            payload: Some(ValueType::TypeParam("T".to_string())),
                        },
                        EnumVariantType {
                            name: "None".to_string(),
                            payload: None,
                        },
                    ],
                },
                EnumType {
                    package: "app.main".to_string(),
                    name: "Result".to_string(),
                    type_params: vec!["T".to_string(), "E".to_string()],
                    variants: vec![
                        EnumVariantType {
                            name: "Ok".to_string(),
                            payload: Some(ValueType::TypeParam("T".to_string())),
                        },
                        EnumVariantType {
                            name: "Err".to_string(),
                            payload: Some(ValueType::TypeParam("E".to_string())),
                        },
                    ],
                },
            ],
            functions: vec![
                Function {
                    package: "app.main".to_string(),
                    name: "compute".to_string(),
                    params: Vec::new(),
                    return_type: result_array_array.clone(),
                    body: vec![
                        Statement::Let {
                            name: "items".to_string(),
                            value_type: array_i32.clone(),
                            initializer: ValueExpr::ArrayNew {
                                element_type: ValueType::I32,
                            },
                        },
                        Statement::Let {
                            name: "raw".to_string(),
                            value_type: result_array_array.clone(),
                            initializer: ValueExpr::EnumVariant {
                                enum_name: "Result".to_string(),
                                enum_args: vec![array_i32.clone(), array_i32.clone()],
                                variant: "Ok".to_string(),
                                payload: Some(Box::new(ValueExpr::Variable("items".to_string()))),
                            },
                        },
                        Statement::QuestionLet {
                            carrier: QuestionCarrier::Result,
                            name: "value".to_string(),
                            value_type: array_i32.clone(),
                            result_type: result_array_array.clone(),
                            return_type: result_array_array.clone(),
                            result_expr: ValueExpr::Variable("raw".to_string()),
                        },
                        Statement::Return(Some(ValueExpr::EnumVariant {
                            enum_name: "Result".to_string(),
                            enum_args: vec![array_i32.clone(), array_i32],
                            variant: "Ok".to_string(),
                            payload: Some(Box::new(ValueExpr::Variable("value".to_string()))),
                        })),
                    ],
                },
                Function {
                    package: "app.main".to_string(),
                    name: "main".to_string(),
                    params: Vec::new(),
                    return_type: ValueType::Void,
                    body: Vec::new(),
                },
            ],
        };

        let c = emit_c(&program);
        assert!(c.contains("nomo_enum_Result_array_i32_array_i32 nomo_value_result = nomo_raw;"));
        let question_error = c.find("if (nomo_value_result.tag").unwrap();
        let question_return_retain = c[question_error..]
            .find(
                "nomo__question_return = nomo_enum_Result_array_i32_array_i32_retain(nomo__question_return);",
            )
            .unwrap();
        let raw_release = c[question_error..]
            .find("nomo_enum_Result_array_i32_array_i32_release(nomo_raw);")
            .unwrap();
        let question_return = c[question_error..]
            .find("return nomo__question_return;")
            .unwrap();
        assert!(question_return_retain < raw_release);
        assert!(raw_release < question_return);
        assert!(c.contains("nomo_value = nomo_array_i32_retain(nomo_value);"));
    }

    #[test]
    fn break_releases_only_loop_body_array_locals() {
        let array_i32 = ValueType::Array(Box::new(ValueType::I32));
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: vec!["std.array".to_string()],
            structs: Vec::new(),
            enums: vec![EnumType {
                package: "app.main".to_string(),
                name: "Option".to_string(),
                type_params: vec!["T".to_string()],
                variants: vec![
                    EnumVariantType {
                        name: "Some".to_string(),
                        payload: Some(ValueType::TypeParam("T".to_string())),
                    },
                    EnumVariantType {
                        name: "None".to_string(),
                        payload: None,
                    },
                ],
            }],
            functions: vec![Function {
                package: "app.main".to_string(),
                name: "main".to_string(),
                params: Vec::new(),
                return_type: ValueType::Void,
                body: vec![
                    Statement::Let {
                        name: "items".to_string(),
                        value_type: array_i32.clone(),
                        initializer: ValueExpr::ArrayNew {
                            element_type: ValueType::I32,
                        },
                    },
                    Statement::Loop {
                        kind: LoopKind::Infinite,
                        body: vec![
                            Statement::Let {
                                name: "temp".to_string(),
                                value_type: array_i32,
                                initializer: ValueExpr::ArrayNew {
                                    element_type: ValueType::I32,
                                },
                            },
                            Statement::Break,
                        ],
                    },
                    Statement::Let {
                        name: "size".to_string(),
                        value_type: ValueType::U64,
                        initializer: ValueExpr::ArrayLen {
                            array: Box::new(ValueExpr::Variable("items".to_string())),
                        },
                    },
                ],
            }],
        };

        let c = emit_c(&program);
        let break_index = c.find("break;").unwrap();
        let temp_release = c.find("nomo_array_i32_release(nomo_temp);").unwrap();
        assert!(temp_release < break_index);
        assert!(!c[..break_index].contains("nomo_array_i32_release(nomo_items);"));
        let size_index = c
            .find("uint64_t nomo_size = ((uint64_t)nomo_items.len);")
            .unwrap();
        let items_release = c.rfind("nomo_array_i32_release(nomo_items);").unwrap();
        assert!(break_index < size_index);
        assert!(size_index < items_release);
    }

    #[test]
    fn for_in_releases_owned_iterable_temp_but_not_shared_iterable() {
        let array_string = ValueType::Array(Box::new(ValueType::String));
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: vec!["std.env".to_string(), "std.array".to_string()],
            structs: Vec::new(),
            enums: vec![EnumType {
                package: "app.main".to_string(),
                name: "Option".to_string(),
                type_params: vec!["T".to_string()],
                variants: vec![
                    EnumVariantType {
                        name: "Some".to_string(),
                        payload: Some(ValueType::TypeParam("T".to_string())),
                    },
                    EnumVariantType {
                        name: "None".to_string(),
                        payload: None,
                    },
                ],
            }],
            functions: vec![Function {
                package: "app.main".to_string(),
                name: "main".to_string(),
                params: Vec::new(),
                return_type: ValueType::Void,
                body: vec![
                    Statement::Loop {
                        kind: LoopKind::Iterate {
                            binding: "arg".to_string(),
                            element_type: ValueType::String,
                            iterable: ValueExpr::EnvArgs,
                        },
                        body: vec![Statement::Println(ValueExpr::Variable("arg".to_string()))],
                    },
                    Statement::Let {
                        name: "words".to_string(),
                        value_type: array_string,
                        initializer: ValueExpr::EnvArgs,
                    },
                    Statement::Loop {
                        kind: LoopKind::Iterate {
                            binding: "word".to_string(),
                            element_type: ValueType::String,
                            iterable: ValueExpr::Variable("words".to_string()),
                        },
                        body: vec![Statement::Println(ValueExpr::Variable("word".to_string()))],
                    },
                ],
            }],
        };

        let c = emit_c(&program);
        let owned_seq = "nomo_array_string nomo__seq = nomo_env_args(nomo_argc, nomo_argv);";
        let owned_release = "nomo_array_string_release(nomo__seq);";
        let shared_seq = "nomo_array_string nomo__seq = nomo_words;";
        let owned_seq_index = c.find(owned_seq).unwrap();
        let owned_release_index =
            c[owned_seq_index..].find(owned_release).unwrap() + owned_seq_index;
        let shared_seq_index = c.find(shared_seq).unwrap();
        assert!(owned_seq_index < owned_release_index);
        assert!(owned_release_index < shared_seq_index);
        assert!(!c[shared_seq_index..].contains(owned_release));
    }

    #[test]
    fn for_in_releases_managed_binding_after_each_iteration() {
        let array_i32 = ValueType::Array(Box::new(ValueType::I32));
        let array_array_i32 = ValueType::Array(Box::new(array_i32.clone()));
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: vec!["std.array".to_string()],
            structs: Vec::new(),
            enums: vec![EnumType {
                package: "app.main".to_string(),
                name: "Option".to_string(),
                type_params: vec!["T".to_string()],
                variants: vec![
                    EnumVariantType {
                        name: "Some".to_string(),
                        payload: Some(ValueType::TypeParam("T".to_string())),
                    },
                    EnumVariantType {
                        name: "None".to_string(),
                        payload: None,
                    },
                ],
            }],
            functions: vec![Function {
                package: "app.main".to_string(),
                name: "main".to_string(),
                params: Vec::new(),
                return_type: ValueType::Void,
                body: vec![
                    Statement::Let {
                        name: "items".to_string(),
                        value_type: array_array_i32,
                        initializer: ValueExpr::ArrayNew {
                            element_type: array_i32.clone(),
                        },
                    },
                    Statement::Loop {
                        kind: LoopKind::Iterate {
                            binding: "item".to_string(),
                            element_type: array_i32,
                            iterable: ValueExpr::Variable("items".to_string()),
                        },
                        body: vec![Statement::Println(ValueExpr::StringLiteral(
                            "tick".to_string(),
                        ))],
                    },
                ],
            }],
        };

        let c = emit_c(&program);
        let binding = "nomo_array_i32 nomo_item = nomo__seq.data[nomo_i];";
        let retain = "nomo_item = nomo_array_i32_retain(nomo_item);";
        let body = puts_literal("tick");
        let release = "nomo_array_i32_release(nomo_item);";
        let binding_index = c.find(binding).unwrap();
        let retain_index = c[binding_index..].find(retain).unwrap() + binding_index;
        let body_index = c[retain_index..].find(&body).unwrap() + retain_index;
        let release_index = c[body_index..].find(release).unwrap() + body_index;
        assert!(binding_index < retain_index);
        assert!(retain_index < body_index);
        assert!(body_index < release_index);
    }

    #[test]
    fn for_in_return_releases_owned_iterable_temp_and_managed_binding() {
        let array_i32 = ValueType::Array(Box::new(ValueType::I32));
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: vec!["std.array".to_string()],
            structs: Vec::new(),
            enums: vec![EnumType {
                package: "app.main".to_string(),
                name: "Option".to_string(),
                type_params: vec!["T".to_string()],
                variants: vec![
                    EnumVariantType {
                        name: "Some".to_string(),
                        payload: Some(ValueType::TypeParam("T".to_string())),
                    },
                    EnumVariantType {
                        name: "None".to_string(),
                        payload: None,
                    },
                ],
            }],
            functions: vec![
                Function {
                    package: "app.main".to_string(),
                    name: "take".to_string(),
                    params: Vec::new(),
                    return_type: array_i32.clone(),
                    body: vec![Statement::Loop {
                        kind: LoopKind::Iterate {
                            binding: "item".to_string(),
                            element_type: array_i32.clone(),
                            iterable: ValueExpr::ArrayNew {
                                element_type: array_i32.clone(),
                            },
                        },
                        body: vec![Statement::Return(Some(ValueExpr::Variable(
                            "item".to_string(),
                        )))],
                    }],
                },
                Function {
                    package: "app.main".to_string(),
                    name: "main".to_string(),
                    params: Vec::new(),
                    return_type: ValueType::Void,
                    body: Vec::new(),
                },
            ],
        };

        let c = emit_c(&program);
        let return_temp = "nomo_array_i32 nomo__return = nomo_item;";
        let retain_return = "nomo__return = nomo_array_i32_retain(nomo__return);";
        let release_binding = "nomo_array_i32_release(nomo_item);";
        let release_seq = "nomo_array_array_i32_release(nomo__seq);";
        let return_stmt = "return nomo__return;";
        let return_temp_index = c.find(return_temp).unwrap();
        let retain_index = c[return_temp_index..].find(retain_return).unwrap() + return_temp_index;
        let binding_release_index = c[retain_index..].find(release_binding).unwrap() + retain_index;
        let seq_release_index =
            c[binding_release_index..].find(release_seq).unwrap() + binding_release_index;
        let return_index = c[seq_release_index..].find(return_stmt).unwrap() + seq_release_index;
        assert!(return_temp_index < retain_index);
        assert!(retain_index < binding_release_index);
        assert!(binding_release_index < seq_release_index);
        assert!(seq_release_index < return_index);
    }

    #[test]
    fn array_reassignment_releases_old_storage_and_retains_shared_rhs() {
        let array_i32 = ValueType::Array(Box::new(ValueType::I32));
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: vec!["std.array".to_string()],
            structs: Vec::new(),
            enums: vec![EnumType {
                package: "app.main".to_string(),
                name: "Option".to_string(),
                type_params: vec!["T".to_string()],
                variants: vec![
                    EnumVariantType {
                        name: "Some".to_string(),
                        payload: Some(ValueType::TypeParam("T".to_string())),
                    },
                    EnumVariantType {
                        name: "None".to_string(),
                        payload: None,
                    },
                ],
            }],
            functions: vec![Function {
                package: "app.main".to_string(),
                name: "main".to_string(),
                params: Vec::new(),
                return_type: ValueType::Void,
                body: vec![
                    Statement::Let {
                        name: "left".to_string(),
                        value_type: array_i32.clone(),
                        initializer: ValueExpr::ArrayNew {
                            element_type: ValueType::I32,
                        },
                    },
                    Statement::Let {
                        name: "right".to_string(),
                        value_type: array_i32,
                        initializer: ValueExpr::ArrayNew {
                            element_type: ValueType::I32,
                        },
                    },
                    Statement::Assign {
                        name: "left".to_string(),
                        value: ValueExpr::Variable("right".to_string()),
                    },
                    Statement::Assign {
                        name: "left".to_string(),
                        value: ValueExpr::Variable("left".to_string()),
                    },
                ],
            }],
        };

        let c = emit_c(&program);
        let temp = "nomo_array_i32 nomo__assign_nomo_left = nomo_right;";
        let retain = "nomo__assign_nomo_left = nomo_array_i32_retain(nomo__assign_nomo_left);";
        let release = "nomo_array_i32_release(nomo_left);";
        let assign = "nomo_left = nomo__assign_nomo_left;";
        let temp_index = c.find(temp).unwrap();
        let retain_index = c[temp_index..].find(retain).unwrap() + temp_index;
        let release_index = c[retain_index..].find(release).unwrap() + retain_index;
        let assign_index = c[release_index..].find(assign).unwrap() + release_index;
        assert!(temp_index < retain_index);
        assert!(retain_index < release_index);
        assert!(release_index < assign_index);
        assert!(c.contains("nomo_array_i32 nomo__assign_nomo_left = nomo_left;"));
    }

    #[test]
    fn option_array_reassignment_retains_and_releases_payload() {
        let array_i32 = ValueType::Array(Box::new(ValueType::I32));
        let option_array_i32 = ValueType::Enum("Option".to_string(), vec![array_i32.clone()]);
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: vec!["std.array".to_string()],
            structs: Vec::new(),
            enums: vec![EnumType {
                package: "app.main".to_string(),
                name: "Option".to_string(),
                type_params: vec!["T".to_string()],
                variants: vec![
                    EnumVariantType {
                        name: "Some".to_string(),
                        payload: Some(ValueType::TypeParam("T".to_string())),
                    },
                    EnumVariantType {
                        name: "None".to_string(),
                        payload: None,
                    },
                ],
            }],
            functions: vec![Function {
                package: "app.main".to_string(),
                name: "main".to_string(),
                params: Vec::new(),
                return_type: ValueType::Void,
                body: vec![
                    Statement::Let {
                        name: "values".to_string(),
                        value_type: array_i32,
                        initializer: ValueExpr::ArrayNew {
                            element_type: ValueType::I32,
                        },
                    },
                    Statement::Let {
                        name: "maybe".to_string(),
                        value_type: option_array_i32.clone(),
                        initializer: ValueExpr::EnumVariant {
                            enum_name: "Option".to_string(),
                            enum_args: vec![ValueType::Array(Box::new(ValueType::I32))],
                            variant: "Some".to_string(),
                            payload: Some(Box::new(ValueExpr::Variable("values".to_string()))),
                        },
                    },
                    Statement::Let {
                        name: "snapshot".to_string(),
                        value_type: option_array_i32,
                        initializer: ValueExpr::Variable("maybe".to_string()),
                    },
                    Statement::Assign {
                        name: "maybe".to_string(),
                        value: ValueExpr::Variable("maybe".to_string()),
                    },
                ],
            }],
        };

        let c = emit_c(&program);
        assert!(c.contains("nomo_maybe = nomo_enum_Option_array_i32_retain(nomo_maybe);"));
        assert!(c.contains("nomo_snapshot = nomo_enum_Option_array_i32_retain(nomo_snapshot);"));
        assert!(c.contains(
            "nomo__assign_nomo_maybe = nomo_enum_Option_array_i32_retain(nomo__assign_nomo_maybe);"
        ));
        assert!(c.contains("nomo_enum_Option_array_i32_release(nomo_maybe);"));
        assert!(c.contains("if (value.tag == nomo_enum_Option_array_i32_Some) {"));
        assert!(c.contains("value.payload.nomo_payload_Some = nomo_array_i32_retain(value.payload.nomo_payload_Some);"));
        assert!(c.contains("nomo_array_i32_release(value.payload.nomo_payload_Some);"));
    }

    #[test]
    fn array_get_returns_owned_option_payload_without_extra_binding_retain() {
        let array_i32 = ValueType::Array(Box::new(ValueType::I32));
        let array_array_i32 = ValueType::Array(Box::new(array_i32.clone()));
        let option_array_i32 = ValueType::Enum("Option".to_string(), vec![array_i32.clone()]);
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: vec!["std.array".to_string()],
            structs: Vec::new(),
            enums: vec![EnumType {
                package: "app.main".to_string(),
                name: "Option".to_string(),
                type_params: vec!["T".to_string()],
                variants: vec![
                    EnumVariantType {
                        name: "Some".to_string(),
                        payload: Some(ValueType::TypeParam("T".to_string())),
                    },
                    EnumVariantType {
                        name: "None".to_string(),
                        payload: None,
                    },
                ],
            }],
            functions: vec![Function {
                package: "app.main".to_string(),
                name: "main".to_string(),
                params: Vec::new(),
                return_type: ValueType::Void,
                body: vec![
                    Statement::Let {
                        name: "inner".to_string(),
                        value_type: array_i32.clone(),
                        initializer: ValueExpr::ArrayNew {
                            element_type: ValueType::I32,
                        },
                    },
                    Statement::Let {
                        name: "outer".to_string(),
                        value_type: array_array_i32,
                        initializer: ValueExpr::ArrayNew {
                            element_type: array_i32.clone(),
                        },
                    },
                    Statement::Assign {
                        name: "outer".to_string(),
                        value: ValueExpr::ArrayPush {
                            array: "outer".to_string(),
                            value: Box::new(ValueExpr::Variable("inner".to_string())),
                            element_type: array_i32.clone(),
                        },
                    },
                    Statement::Let {
                        name: "maybe".to_string(),
                        value_type: option_array_i32.clone(),
                        initializer: ValueExpr::ArrayGet {
                            array: Box::new(ValueExpr::Variable("outer".to_string())),
                            index: Box::new(ValueExpr::IntLiteral(0)),
                            element_type: array_i32,
                        },
                    },
                    Statement::Let {
                        name: "snapshot".to_string(),
                        value_type: option_array_i32,
                        initializer: ValueExpr::Variable("maybe".to_string()),
                    },
                ],
            }],
        };

        let c = emit_c(&program);
        assert!(c.contains(
            "nomo_enum_Option_array_i32 nomo_maybe = nomo_array_array_i32_get(nomo_outer, 0);"
        ));
        assert!(!c.contains("nomo_maybe = nomo_enum_Option_array_i32_retain(nomo_maybe);"));
        assert!(c.contains("nomo_snapshot = nomo_enum_Option_array_i32_retain(nomo_snapshot);"));
    }

    #[test]
    fn if_let_releases_owned_enum_temp_after_retaining_payload_binding() {
        let array_i32 = ValueType::Array(Box::new(ValueType::I32));
        let array_array_i32 = ValueType::Array(Box::new(array_i32.clone()));
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: vec!["std.array".to_string()],
            structs: Vec::new(),
            enums: vec![EnumType {
                package: "app.main".to_string(),
                name: "Option".to_string(),
                type_params: vec!["T".to_string()],
                variants: vec![
                    EnumVariantType {
                        name: "Some".to_string(),
                        payload: Some(ValueType::TypeParam("T".to_string())),
                    },
                    EnumVariantType {
                        name: "None".to_string(),
                        payload: None,
                    },
                ],
            }],
            functions: vec![Function {
                package: "app.main".to_string(),
                name: "main".to_string(),
                params: Vec::new(),
                return_type: ValueType::Void,
                body: vec![
                    Statement::Let {
                        name: "outer".to_string(),
                        value_type: array_array_i32,
                        initializer: ValueExpr::ArrayNew {
                            element_type: array_i32.clone(),
                        },
                    },
                    Statement::IfLet {
                        binding: Some("values".to_string()),
                        value_type: Some(array_i32.clone()),
                        value: ValueExpr::ArrayGet {
                            array: Box::new(ValueExpr::Variable("outer".to_string())),
                            index: Box::new(ValueExpr::IntLiteral(0)),
                            element_type: array_i32,
                        },
                        enum_name: "Option".to_string(),
                        enum_args: vec![ValueType::Array(Box::new(ValueType::I32))],
                        variant: "Some".to_string(),
                        body: vec![Statement::Println(ValueExpr::StringLiteral(
                            "some".to_string(),
                        ))],
                        else_body: Some(vec![Statement::Println(ValueExpr::StringLiteral(
                            "none".to_string(),
                        ))]),
                    },
                ],
            }],
        };

        let c = emit_c(&program);
        let retain = "nomo_values = nomo_array_i32_retain(nomo_values);";
        let temp_release =
            "nomo_enum_Option_array_i32_release(nomo__if_let_nomo_enum_Option_array_i32_Some);";
        let body = puts_literal("some");
        let binding_release = "nomo_array_i32_release(nomo_values);";
        let retain_index = c.find(retain).unwrap();
        let release_index = c[retain_index..].find(temp_release).unwrap() + retain_index;
        let body_index = c[release_index..].find(&body).unwrap() + release_index;
        let binding_release_index = c[body_index..].find(binding_release).unwrap() + body_index;
        assert!(retain_index < release_index);
        assert!(release_index < body_index);
        assert!(body_index < binding_release_index);
        let else_index = c.find(" else {").unwrap();
        let else_release = c[else_index..].find(temp_release).unwrap() + else_index;
        let else_body = c[else_release..].find(&puts_literal("none")).unwrap() + else_release;
        assert!(else_index < else_release);
        assert!(else_release < else_body);
    }

    #[test]
    fn let_else_releases_owned_enum_temp_after_retaining_payload_binding() {
        let array_i32 = ValueType::Array(Box::new(ValueType::I32));
        let array_array_i32 = ValueType::Array(Box::new(array_i32.clone()));
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: vec!["std.array".to_string()],
            structs: Vec::new(),
            enums: vec![EnumType {
                package: "app.main".to_string(),
                name: "Option".to_string(),
                type_params: vec!["T".to_string()],
                variants: vec![
                    EnumVariantType {
                        name: "Some".to_string(),
                        payload: Some(ValueType::TypeParam("T".to_string())),
                    },
                    EnumVariantType {
                        name: "None".to_string(),
                        payload: None,
                    },
                ],
            }],
            functions: vec![Function {
                package: "app.main".to_string(),
                name: "main".to_string(),
                params: Vec::new(),
                return_type: ValueType::Void,
                body: vec![
                    Statement::Let {
                        name: "outer".to_string(),
                        value_type: array_array_i32,
                        initializer: ValueExpr::ArrayNew {
                            element_type: array_i32.clone(),
                        },
                    },
                    Statement::LetElse {
                        binding: "values".to_string(),
                        value_type: array_i32.clone(),
                        value: ValueExpr::ArrayGet {
                            array: Box::new(ValueExpr::Variable("outer".to_string())),
                            index: Box::new(ValueExpr::IntLiteral(0)),
                            element_type: array_i32,
                        },
                        enum_name: "Option".to_string(),
                        enum_args: vec![ValueType::Array(Box::new(ValueType::I32))],
                        variant: "Some".to_string(),
                        else_body: vec![Statement::Panic(ValueExpr::StringLiteral(
                            "missing".to_string(),
                        ))],
                    },
                    Statement::Println(ValueExpr::StringLiteral("ok".to_string())),
                ],
            }],
        };

        let c = emit_c(&program);
        let else_release = "nomo_enum_Option_array_i32_release(nomo__let_else_nomo_values);";
        let else_panic = panic_literal("missing");
        let binding_retain = "nomo_values = nomo_array_i32_retain(nomo_values);";
        let binding_release = "nomo_enum_Option_array_i32_release(nomo__let_else_nomo_values);";
        let else_index = c.find(else_release).unwrap();
        let panic_index = c[else_index..].find(&else_panic).unwrap() + else_index;
        assert!(else_index < panic_index);
        let retain_index = c.rfind(binding_retain).unwrap();
        let release_index = c[retain_index..].find(binding_release).unwrap() + retain_index;
        assert!(retain_index < release_index);
    }

    #[test]
    fn struct_and_custom_enum_lifecycle_helpers_manage_array_payloads() {
        let array_i32 = ValueType::Array(Box::new(ValueType::I32));
        let bag = ValueType::Struct("Bag".to_string(), Vec::new());
        let slot = ValueType::Enum("Slot".to_string(), Vec::new());
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: vec!["std.array".to_string()],
            structs: vec![StructType {
                package: "app.main".to_string(),
                name: "Bag".to_string(),
                type_params: Vec::new(),
                fields: vec![StructField {
                    name: "items".to_string(),
                    value_type: array_i32.clone(),
                }],
            }],
            enums: vec![
                EnumType {
                    package: "app.main".to_string(),
                    name: "Option".to_string(),
                    type_params: vec!["T".to_string()],
                    variants: vec![
                        EnumVariantType {
                            name: "Some".to_string(),
                            payload: Some(ValueType::TypeParam("T".to_string())),
                        },
                        EnumVariantType {
                            name: "None".to_string(),
                            payload: None,
                        },
                    ],
                },
                EnumType {
                    package: "app.main".to_string(),
                    name: "Slot".to_string(),
                    type_params: Vec::new(),
                    variants: vec![
                        EnumVariantType {
                            name: "Full".to_string(),
                            payload: Some(bag.clone()),
                        },
                        EnumVariantType {
                            name: "Empty".to_string(),
                            payload: None,
                        },
                    ],
                },
            ],
            functions: vec![
                Function {
                    package: "app.main".to_string(),
                    name: "label".to_string(),
                    params: vec![Parameter {
                        name: "bag".to_string(),
                        mutable: false,
                        value_type: bag.clone(),
                    }],
                    return_type: ValueType::Void,
                    body: Vec::new(),
                },
                Function {
                    package: "app.main".to_string(),
                    name: "main".to_string(),
                    params: Vec::new(),
                    return_type: ValueType::Void,
                    body: vec![
                        Statement::Let {
                            name: "items".to_string(),
                            value_type: array_i32.clone(),
                            initializer: ValueExpr::ArrayNew {
                                element_type: ValueType::I32,
                            },
                        },
                        Statement::Let {
                            name: "bag".to_string(),
                            value_type: bag.clone(),
                            initializer: ValueExpr::StructLiteral {
                                type_name: "Bag".to_string(),
                                struct_args: Vec::new(),
                                fields: vec![(
                                    "items".to_string(),
                                    ValueExpr::Variable("items".to_string()),
                                )],
                            },
                        },
                        Statement::Let {
                            name: "replacement".to_string(),
                            value_type: array_i32.clone(),
                            initializer: ValueExpr::ArrayNew {
                                element_type: ValueType::I32,
                            },
                        },
                        Statement::AssignField {
                            base: "bag".to_string(),
                            field: "items".to_string(),
                            value_type: array_i32,
                            value: ValueExpr::Variable("replacement".to_string()),
                        },
                        Statement::Let {
                            name: "slot".to_string(),
                            value_type: slot,
                            initializer: ValueExpr::EnumVariant {
                                enum_name: "Slot".to_string(),
                                enum_args: Vec::new(),
                                variant: "Full".to_string(),
                                payload: Some(Box::new(ValueExpr::Variable("bag".to_string()))),
                            },
                        },
                    ],
                },
            ],
        };

        let c = emit_c(&program);
        assert!(c.contains("static nomo_struct_Bag nomo_struct_Bag_retain(nomo_struct_Bag value)"));
        assert!(
            c.contains("value.nomo_member_items = nomo_array_i32_retain(value.nomo_member_items);")
        );
        assert!(c.contains("static void nomo_struct_Bag_release(nomo_struct_Bag value)"));
        assert!(c.contains("nomo_array_i32_release(value.nomo_member_items);"));
        assert!(c.contains("static nomo_enum_Slot nomo_enum_Slot_retain(nomo_enum_Slot value)"));
        assert!(c.contains("value.payload.nomo_payload_Full = nomo_struct_Bag_retain(value.payload.nomo_payload_Full);"));
        assert!(c.contains("nomo_struct_Bag_release(value.payload.nomo_payload_Full);"));
        assert!(c.contains("nomo_bag = nomo_struct_Bag_retain(nomo_bag);"));
        assert!(c.contains("nomo_slot = nomo_enum_Slot_retain(nomo_slot);"));
        assert!(c.contains("nomo_enum_Slot_release(nomo_slot);"));
        let field_temp =
            "nomo_array_i32 nomo__assign_nomo_bag_nomo_member_items = nomo_replacement;";
        let field_retain = "nomo__assign_nomo_bag_nomo_member_items = nomo_array_i32_retain(nomo__assign_nomo_bag_nomo_member_items);";
        let field_release = "nomo_array_i32_release(nomo_bag.nomo_member_items);";
        let field_assign = "nomo_bag.nomo_member_items = nomo__assign_nomo_bag_nomo_member_items;";
        let temp_index = c.find(field_temp).unwrap();
        let retain_index = c[temp_index..].find(field_retain).unwrap() + temp_index;
        let release_index = c[retain_index..].find(field_release).unwrap() + retain_index;
        let assign_index = c[release_index..].find(field_assign).unwrap() + release_index;
        assert!(temp_index < retain_index);
        assert!(retain_index < release_index);
        assert!(release_index < assign_index);
    }

    #[test]
    fn array_parameters_are_retained_and_released_by_value_but_not_mut_borrows() {
        let array_i32 = ValueType::Array(Box::new(ValueType::I32));
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: vec!["std.array".to_string()],
            structs: Vec::new(),
            enums: vec![EnumType {
                package: "app.main".to_string(),
                name: "Option".to_string(),
                type_params: vec!["T".to_string()],
                variants: vec![
                    EnumVariantType {
                        name: "Some".to_string(),
                        payload: Some(ValueType::TypeParam("T".to_string())),
                    },
                    EnumVariantType {
                        name: "None".to_string(),
                        payload: None,
                    },
                ],
            }],
            functions: vec![
                Function {
                    package: "app.main".to_string(),
                    name: "id".to_string(),
                    params: vec![Parameter {
                        name: "values".to_string(),
                        mutable: false,
                        value_type: array_i32.clone(),
                    }],
                    return_type: array_i32.clone(),
                    body: vec![Statement::Return(Some(ValueExpr::Variable(
                        "values".to_string(),
                    )))],
                },
                Function {
                    package: "app.main".to_string(),
                    name: "borrow".to_string(),
                    params: vec![Parameter {
                        name: "values".to_string(),
                        mutable: true,
                        value_type: array_i32,
                    }],
                    return_type: ValueType::Void,
                    body: Vec::new(),
                },
                Function {
                    package: "app.main".to_string(),
                    name: "main".to_string(),
                    params: Vec::new(),
                    return_type: ValueType::Void,
                    body: Vec::new(),
                },
            ],
        };

        let c = emit_c(&program);
        let id_start = c
            .find("nomo_array_i32 nomo_fn_id(nomo_array_i32 nomo_values)")
            .unwrap();
        let id_body = &c[id_start
            ..c[id_start..]
                .find("#undef")
                .map_or(c.len(), |end| id_start + end)];
        assert!(id_body.contains("nomo_values = nomo_array_i32_retain(nomo_values);"));
        assert!(id_body.contains("nomo__return = nomo_array_i32_retain(nomo__return);"));
        assert!(id_body.contains("nomo_array_i32_release(nomo_values);"));

        let borrow_start = c
            .rfind("void nomo_fn_borrow(nomo_array_i32 * nomo_values)")
            .unwrap();
        let main_start = c[borrow_start..]
            .find("int main")
            .map(|offset| borrow_start + offset)
            .unwrap_or(c.len());
        let borrow_body = &c[borrow_start..main_start];
        assert!(!borrow_body.contains("nomo_values = nomo_array_i32_retain(nomo_values);"));
        assert!(!borrow_body.contains("nomo_array_i32_release(nomo_values);"));
    }

    #[test]
    fn emits_array_helpers_for_all_v0_1_primitive_elements() {
        let elements = vec![
            ValueType::String,
            ValueType::Int,
            ValueType::I32,
            ValueType::U32,
            ValueType::U64,
            ValueType::Float,
            ValueType::Char,
            ValueType::Bool,
        ];
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: vec!["std.array".to_string()],
            structs: Vec::new(),
            enums: vec![EnumType {
                package: "app.main".to_string(),
                name: "Option".to_string(),
                type_params: vec!["T".to_string()],
                variants: vec![
                    EnumVariantType {
                        name: "Some".to_string(),
                        payload: Some(ValueType::TypeParam("T".to_string())),
                    },
                    EnumVariantType {
                        name: "None".to_string(),
                        payload: None,
                    },
                ],
            }],
            functions: vec![Function {
                package: "app.main".to_string(),
                name: "main".to_string(),
                params: Vec::new(),
                return_type: ValueType::Void,
                body: elements
                    .iter()
                    .map(|element_type| Statement::Let {
                        name: format!("items_{}", c_type_name_part(element_type)),
                        value_type: ValueType::Array(Box::new(element_type.clone())),
                        initializer: ValueExpr::ArrayNew {
                            element_type: element_type.clone(),
                        },
                    })
                    .collect(),
            }],
        };

        let c = emit_c(&program);
        for (element_type, c_data_type) in [
            (ValueType::String, "nomo_string"),
            (ValueType::Int, "long long"),
            (ValueType::I32, "int32_t"),
            (ValueType::U32, "uint32_t"),
            (ValueType::U64, "uint64_t"),
            (ValueType::Float, "double"),
            (ValueType::Char, "uint32_t"),
            (ValueType::Bool, "int"),
        ] {
            let array = c_array_ident(&element_type);
            assert!(c.contains(&format!("typedef struct {array}")));
            assert!(c.contains(&format!("{c_data_type} *data;")));
            assert!(c.contains(&format!("static {array} {array}_new(void)")));
        }
    }

    #[test]
    fn emits_if_expression_and_comparison() {
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: vec!["std.io".to_string()],
            structs: Vec::new(),
            enums: Vec::new(),
            functions: vec![
                Function {
                    package: "app.main".to_string(),
                    name: "label".to_string(),
                    params: vec![Parameter {
                        name: "score".to_string(),
                        mutable: false,
                        value_type: ValueType::Int,
                    }],
                    return_type: ValueType::String,
                    body: vec![Statement::Return(Some(ValueExpr::If {
                        condition: Box::new(ValueExpr::Binary {
                            left: Box::new(ValueExpr::Variable("score".to_string())),
                            op: BinaryOp::GreaterEqual,
                            right: Box::new(ValueExpr::IntLiteral(60)),
                            value_type: ValueType::Bool,
                        }),
                        then_branch: Box::new(ValueExpr::StringLiteral("pass".to_string())),
                        else_branch: Box::new(ValueExpr::StringLiteral("fail".to_string())),
                    }))],
                },
                Function {
                    package: "app.main".to_string(),
                    name: "main".to_string(),
                    params: Vec::new(),
                    return_type: ValueType::Void,
                    body: vec![Statement::Println(ValueExpr::StringLiteral(
                        "done".to_string(),
                    ))],
                },
            ],
        };

        let c = emit_c(&program);
        assert!(c.contains(
            "return ((nomo_score >= 60) ? nomo_string_literal(\"pass\") : nomo_string_literal(\"fail\"));"
        ));
    }

    #[test]
    fn emits_string_equality_with_runtime_compare() {
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: vec!["std.io".to_string()],
            structs: Vec::new(),
            enums: Vec::new(),
            functions: vec![Function {
                package: "app.main".to_string(),
                name: "main".to_string(),
                params: Vec::new(),
                return_type: ValueType::Void,
                body: vec![Statement::Let {
                    name: "same".to_string(),
                    value_type: ValueType::Bool,
                    initializer: ValueExpr::StringCompare {
                        left: Box::new(ValueExpr::StringLiteral("nomo".to_string())),
                        op: BinaryOp::Equal,
                        right: Box::new(ValueExpr::StringLiteral("nomo".to_string())),
                    },
                }],
            }],
        };

        let c = emit_c(&program);
        assert!(c.contains("static int nomo_string_equal(nomo_string left, nomo_string right)"));
        assert!(c.contains(
            "int nomo_same = (nomo_string_equal(nomo_string_literal(\"nomo\"), nomo_string_literal(\"nomo\")));"
        ));
    }

    #[test]
    fn emits_panic_statement_and_expression() {
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: vec!["std.io".to_string()],
            structs: Vec::new(),
            enums: Vec::new(),
            functions: vec![
                Function {
                    package: "app.main".to_string(),
                    name: "label".to_string(),
                    params: vec![Parameter {
                        name: "ok".to_string(),
                        mutable: false,
                        value_type: ValueType::Bool,
                    }],
                    return_type: ValueType::String,
                    body: vec![Statement::Return(Some(ValueExpr::If {
                        condition: Box::new(ValueExpr::Variable("ok".to_string())),
                        then_branch: Box::new(ValueExpr::StringLiteral("yes".to_string())),
                        else_branch: Box::new(ValueExpr::Panic {
                            message: Box::new(ValueExpr::StringLiteral("no".to_string())),
                            fallback_type: ValueType::String,
                        }),
                    }))],
                },
                Function {
                    package: "app.main".to_string(),
                    name: "main".to_string(),
                    params: Vec::new(),
                    return_type: ValueType::Void,
                    body: vec![Statement::Panic(ValueExpr::StringLiteral(
                        "boom".to_string(),
                    ))],
                },
            ],
        };

        let c = emit_c(&program);
        assert!(c.contains("static void nomo_panic"));
        assert!(c.contains(&panic_literal("boom")));
        assert!(c.contains(
            "(nomo_panic((nomo_string_literal(\"no\")).data), nomo_string_literal(\"\"))"
        ));
    }

    #[test]
    fn emits_binary_arithmetic_operators() {
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: Vec::new(),
            structs: Vec::new(),
            enums: Vec::new(),
            functions: vec![
                Function {
                    package: "app.main".to_string(),
                    name: "calc".to_string(),
                    params: vec![
                        Parameter {
                            name: "a".to_string(),
                            mutable: false,
                            value_type: ValueType::Int,
                        },
                        Parameter {
                            name: "b".to_string(),
                            mutable: false,
                            value_type: ValueType::Int,
                        },
                        Parameter {
                            name: "c".to_string(),
                            mutable: false,
                            value_type: ValueType::Int,
                        },
                    ],
                    return_type: ValueType::Int,
                    body: vec![Statement::Return(Some(ValueExpr::Binary {
                        left: Box::new(ValueExpr::Binary {
                            left: Box::new(ValueExpr::Variable("a".to_string())),
                            op: BinaryOp::Subtract,
                            right: Box::new(ValueExpr::Variable("b".to_string())),
                            value_type: ValueType::Int,
                        }),
                        op: BinaryOp::Remainder,
                        right: Box::new(ValueExpr::Binary {
                            left: Box::new(ValueExpr::Binary {
                                left: Box::new(ValueExpr::Variable("c".to_string())),
                                op: BinaryOp::Multiply,
                                right: Box::new(ValueExpr::IntLiteral(4)),
                                value_type: ValueType::Int,
                            }),
                            op: BinaryOp::Divide,
                            right: Box::new(ValueExpr::IntLiteral(2)),
                            value_type: ValueType::Int,
                        }),
                        value_type: ValueType::Int,
                    }))],
                },
                Function {
                    package: "app.main".to_string(),
                    name: "main".to_string(),
                    params: Vec::new(),
                    return_type: ValueType::Void,
                    body: Vec::new(),
                },
            ],
        };

        let c = emit_c(&program);

        assert!(c.contains(" - "));
        assert!(c.contains(" * "));
        assert!(c.contains("nomo_div_i64("));
        assert!(c.contains("nomo_rem_i64("));
    }

    #[test]
    fn emits_logical_operators() {
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: Vec::new(),
            structs: Vec::new(),
            enums: Vec::new(),
            functions: vec![
                Function {
                    package: "app.main".to_string(),
                    name: "check".to_string(),
                    params: vec![
                        Parameter {
                            name: "a".to_string(),
                            mutable: false,
                            value_type: ValueType::Bool,
                        },
                        Parameter {
                            name: "b".to_string(),
                            mutable: false,
                            value_type: ValueType::Bool,
                        },
                    ],
                    return_type: ValueType::Bool,
                    body: vec![Statement::Return(Some(ValueExpr::Binary {
                        left: Box::new(ValueExpr::Unary {
                            op: UnaryOp::Not,
                            expr: Box::new(ValueExpr::Variable("a".to_string())),
                        }),
                        op: BinaryOp::LogicalOr,
                        right: Box::new(ValueExpr::Binary {
                            left: Box::new(ValueExpr::Variable("a".to_string())),
                            op: BinaryOp::LogicalAnd,
                            right: Box::new(ValueExpr::Variable("b".to_string())),
                            value_type: ValueType::Bool,
                        }),
                        value_type: ValueType::Bool,
                    }))],
                },
                Function {
                    package: "app.main".to_string(),
                    name: "main".to_string(),
                    params: Vec::new(),
                    return_type: ValueType::Void,
                    body: Vec::new(),
                },
            ],
        };

        let c = emit_c(&program);

        assert!(c.contains("!"));
        assert!(c.contains(" || "));
        assert!(c.contains(" && "));
    }

    #[test]
    fn emits_bitwise_operators() {
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: Vec::new(),
            structs: Vec::new(),
            enums: Vec::new(),
            functions: vec![
                Function {
                    package: "app.main".to_string(),
                    name: "mask".to_string(),
                    params: vec![
                        Parameter {
                            name: "a".to_string(),
                            mutable: false,
                            value_type: ValueType::Int,
                        },
                        Parameter {
                            name: "b".to_string(),
                            mutable: false,
                            value_type: ValueType::Int,
                        },
                    ],
                    return_type: ValueType::Int,
                    body: vec![Statement::Return(Some(ValueExpr::Binary {
                        left: Box::new(ValueExpr::Binary {
                            left: Box::new(ValueExpr::Binary {
                                left: Box::new(ValueExpr::Variable("a".to_string())),
                                op: BinaryOp::BitAnd,
                                right: Box::new(ValueExpr::Variable("b".to_string())),
                                value_type: ValueType::Int,
                            }),
                            op: BinaryOp::BitOr,
                            right: Box::new(ValueExpr::Binary {
                                left: Box::new(ValueExpr::Variable("a".to_string())),
                                op: BinaryOp::BitXor,
                                right: Box::new(ValueExpr::Variable("b".to_string())),
                                value_type: ValueType::Int,
                            }),
                            value_type: ValueType::Int,
                        }),
                        op: BinaryOp::BitAndNot,
                        right: Box::new(ValueExpr::Binary {
                            left: Box::new(ValueExpr::Binary {
                                left: Box::new(ValueExpr::Variable("a".to_string())),
                                op: BinaryOp::ShiftLeft,
                                right: Box::new(ValueExpr::IntLiteral(1)),
                                value_type: ValueType::Int,
                            }),
                            op: BinaryOp::ShiftRight,
                            right: Box::new(ValueExpr::IntLiteral(1)),
                            value_type: ValueType::Int,
                        }),
                        value_type: ValueType::Int,
                    }))],
                },
                Function {
                    package: "app.main".to_string(),
                    name: "main".to_string(),
                    params: Vec::new(),
                    return_type: ValueType::Void,
                    body: Vec::new(),
                },
            ],
        };

        let c = emit_c(&program);

        assert!(c.contains(" & "));
        assert!(c.contains(" | "));
        assert!(c.contains(" ^ "));
        assert!(c.contains("nomo_shl_i64("));
        assert!(c.contains("nomo_shr_i64("));
        assert!(c.contains(" & ~("));
    }

    #[test]
    fn emits_defer_before_panic_statement() {
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: vec!["std.io".to_string()],
            structs: Vec::new(),
            enums: Vec::new(),
            functions: vec![
                Function {
                    package: "app.main".to_string(),
                    name: "cleanup".to_string(),
                    params: Vec::new(),
                    return_type: ValueType::Void,
                    body: vec![Statement::Println(ValueExpr::StringLiteral(
                        "cleanup".to_string(),
                    ))],
                },
                Function {
                    package: "app.main".to_string(),
                    name: "main".to_string(),
                    params: Vec::new(),
                    return_type: ValueType::Void,
                    body: vec![
                        Statement::Defer {
                            call: DeferredCall::Expr(ValueExpr::Call {
                                name: "cleanup".to_string(),
                                args: Vec::new(),
                            }),
                        },
                        Statement::Panic(ValueExpr::StringLiteral("boom".to_string())),
                    ],
                },
            ],
        };

        let c = emit_c(&program);
        let cleanup = c.find("nomo_fn_cleanup();").unwrap();
        let panic = c.find(&panic_literal("boom")).unwrap();
        assert!(cleanup < panic);
        assert_eq!(c.matches("nomo_fn_cleanup();").count(), 1);
    }

    #[test]
    fn emits_defer_at_fallthrough_function_exit() {
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: vec!["std.io".to_string()],
            structs: Vec::new(),
            enums: Vec::new(),
            functions: vec![
                Function {
                    package: "app.main".to_string(),
                    name: "cleanup".to_string(),
                    params: Vec::new(),
                    return_type: ValueType::Void,
                    body: vec![Statement::Println(ValueExpr::StringLiteral(
                        "cleanup".to_string(),
                    ))],
                },
                Function {
                    package: "app.main".to_string(),
                    name: "main".to_string(),
                    params: Vec::new(),
                    return_type: ValueType::Void,
                    body: vec![
                        Statement::Defer {
                            call: DeferredCall::Expr(ValueExpr::Call {
                                name: "cleanup".to_string(),
                                args: Vec::new(),
                            }),
                        },
                        Statement::Println(ValueExpr::StringLiteral("working".to_string())),
                    ],
                },
            ],
        };

        let c = emit_c(&program);
        let working = c.find(&puts_literal("working")).unwrap();
        let cleanup = c.find("nomo_fn_cleanup();").unwrap();
        assert!(working < cleanup);
        assert_eq!(c.matches("nomo_fn_cleanup();").count(), 1);
    }

    #[test]
    fn emits_deferred_println_at_fallthrough_exit() {
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: vec!["std.io".to_string()],
            structs: Vec::new(),
            enums: Vec::new(),
            functions: vec![Function {
                package: "app.main".to_string(),
                name: "main".to_string(),
                params: Vec::new(),
                return_type: ValueType::Void,
                body: vec![
                    Statement::Defer {
                        call: DeferredCall::Println(ValueExpr::StringLiteral(
                            "cleanup".to_string(),
                        )),
                    },
                    Statement::Println(ValueExpr::StringLiteral("working".to_string())),
                ],
            }],
        };

        let c = emit_c(&program);
        let working = c.find(&puts_literal("working")).unwrap();
        let cleanup = c.find(&puts_literal("cleanup")).unwrap();
        assert!(working < cleanup);
    }

    #[test]
    fn emits_nested_block_defer_at_block_fallthrough_exit() {
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: vec!["std.io".to_string()],
            structs: Vec::new(),
            enums: vec![EnumType {
                package: "app.main".to_string(),
                name: "Color".to_string(),
                type_params: Vec::new(),
                variants: vec![
                    EnumVariantType {
                        name: "Red".to_string(),
                        payload: None,
                    },
                    EnumVariantType {
                        name: "Blue".to_string(),
                        payload: None,
                    },
                ],
            }],
            functions: vec![Function {
                package: "app.main".to_string(),
                name: "main".to_string(),
                params: Vec::new(),
                return_type: ValueType::Void,
                body: vec![
                    Statement::Defer {
                        call: DeferredCall::Println(ValueExpr::StringLiteral("outer".to_string())),
                    },
                    Statement::Match {
                        value: ValueExpr::EnumVariant {
                            enum_name: "Color".to_string(),
                            enum_args: Vec::new(),
                            variant: "Red".to_string(),
                            payload: None,
                        },
                        enum_name: "Color".to_string(),
                        enum_args: Vec::new(),
                        arms: vec![
                            MatchStatementArm {
                                variant: "Red".to_string(),
                                binding: None,
                                body: vec![
                                    Statement::Defer {
                                        call: DeferredCall::Println(ValueExpr::StringLiteral(
                                            "inner".to_string(),
                                        )),
                                    },
                                    Statement::Println(ValueExpr::StringLiteral("red".to_string())),
                                ],
                            },
                            MatchStatementArm {
                                variant: "Blue".to_string(),
                                binding: None,
                                body: vec![Statement::Println(ValueExpr::StringLiteral(
                                    "blue".to_string(),
                                ))],
                            },
                        ],
                    },
                    Statement::Println(ValueExpr::StringLiteral("after".to_string())),
                ],
            }],
        };

        let c = emit_c(&program);
        let red = c.find(&puts_literal("red")).unwrap();
        let inner = c[red..].find(&puts_literal("inner")).unwrap() + red;
        let after = c[inner..].find(&puts_literal("after")).unwrap() + inner;
        let outer = c[after..].find(&puts_literal("outer")).unwrap() + after;
        assert!(red < inner);
        assert!(inner < after);
        assert!(after < outer);
        assert_eq!(c.matches(&puts_literal("inner")).count(), 1);
        assert_eq!(c.matches(&puts_literal("outer")).count(), 1);
    }

    #[test]
    fn emits_nested_block_defer_before_return_and_outer_defer() {
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: vec!["std.io".to_string()],
            structs: Vec::new(),
            enums: vec![EnumType {
                package: "app.main".to_string(),
                name: "Color".to_string(),
                type_params: Vec::new(),
                variants: vec![
                    EnumVariantType {
                        name: "Red".to_string(),
                        payload: None,
                    },
                    EnumVariantType {
                        name: "Blue".to_string(),
                        payload: None,
                    },
                ],
            }],
            functions: vec![Function {
                package: "app.main".to_string(),
                name: "main".to_string(),
                params: Vec::new(),
                return_type: ValueType::Void,
                body: vec![
                    Statement::Defer {
                        call: DeferredCall::Println(ValueExpr::StringLiteral("outer".to_string())),
                    },
                    Statement::Match {
                        value: ValueExpr::EnumVariant {
                            enum_name: "Color".to_string(),
                            enum_args: Vec::new(),
                            variant: "Red".to_string(),
                            payload: None,
                        },
                        enum_name: "Color".to_string(),
                        enum_args: Vec::new(),
                        arms: vec![
                            MatchStatementArm {
                                variant: "Red".to_string(),
                                binding: None,
                                body: vec![
                                    Statement::Defer {
                                        call: DeferredCall::Println(ValueExpr::StringLiteral(
                                            "inner".to_string(),
                                        )),
                                    },
                                    Statement::Return(None),
                                ],
                            },
                            MatchStatementArm {
                                variant: "Blue".to_string(),
                                binding: None,
                                body: Vec::new(),
                            },
                        ],
                    },
                ],
            }],
        };

        let c = emit_c(&program);
        let inner = c.find(&puts_literal("inner")).unwrap();
        let outer = c[inner..].find(&puts_literal("outer")).unwrap() + inner;
        let return_stmt = c[outer..].find("return;").unwrap() + outer;
        assert!(inner < outer);
        assert!(outer < return_stmt);
        assert_eq!(c.matches(&puts_literal("inner")).count(), 1);
        assert_eq!(c.matches(&puts_literal("outer")).count(), 2);
    }

    #[test]
    fn emits_loop_defer_before_break_without_function_defer() {
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: vec!["std.io".to_string()],
            structs: Vec::new(),
            enums: Vec::new(),
            functions: vec![Function {
                package: "app.main".to_string(),
                name: "main".to_string(),
                params: Vec::new(),
                return_type: ValueType::Void,
                body: vec![
                    Statement::Defer {
                        call: DeferredCall::Println(ValueExpr::StringLiteral("outer".to_string())),
                    },
                    Statement::Loop {
                        kind: LoopKind::Infinite,
                        body: vec![
                            Statement::Defer {
                                call: DeferredCall::Println(ValueExpr::StringLiteral(
                                    "inner".to_string(),
                                )),
                            },
                            Statement::Break,
                        ],
                    },
                ],
            }],
        };

        let c = emit_c(&program);
        let inner = c.find(&puts_literal("inner")).unwrap();
        let break_stmt = c[inner..].find("break;").unwrap() + inner;
        let outer = c[break_stmt..].find(&puts_literal("outer")).unwrap() + break_stmt;
        assert!(inner < break_stmt);
        assert!(break_stmt < outer);
        assert_eq!(c.matches(&puts_literal("inner")).count(), 1);
        assert_eq!(c.matches(&puts_literal("outer")).count(), 1);
    }

    #[test]
    fn emits_loop_defer_before_continue_without_function_defer() {
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: vec!["std.io".to_string()],
            structs: Vec::new(),
            enums: Vec::new(),
            functions: vec![Function {
                package: "app.main".to_string(),
                name: "main".to_string(),
                params: Vec::new(),
                return_type: ValueType::Void,
                body: vec![
                    Statement::Defer {
                        call: DeferredCall::Println(ValueExpr::StringLiteral("outer".to_string())),
                    },
                    Statement::Loop {
                        kind: LoopKind::Infinite,
                        body: vec![
                            Statement::Defer {
                                call: DeferredCall::Println(ValueExpr::StringLiteral(
                                    "inner".to_string(),
                                )),
                            },
                            Statement::Continue,
                        ],
                    },
                ],
            }],
        };

        let c = emit_c(&program);
        let inner = c.find(&puts_literal("inner")).unwrap();
        let continue_stmt = c[inner..].find("continue;").unwrap() + inner;
        let outer = c[continue_stmt..].find(&puts_literal("outer")).unwrap() + continue_stmt;
        assert!(inner < continue_stmt);
        assert!(continue_stmt < outer);
        assert_eq!(c.matches(&puts_literal("inner")).count(), 1);
        assert_eq!(c.matches(&puts_literal("outer")).count(), 1);
    }

    #[test]
    fn inner_loop_break_only_runs_inner_loop_defer() {
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: vec!["std.io".to_string()],
            structs: Vec::new(),
            enums: Vec::new(),
            functions: vec![Function {
                package: "app.main".to_string(),
                name: "main".to_string(),
                params: Vec::new(),
                return_type: ValueType::Void,
                body: vec![Statement::Loop {
                    kind: LoopKind::Infinite,
                    body: vec![
                        Statement::Defer {
                            call: DeferredCall::Println(ValueExpr::StringLiteral(
                                "outer loop".to_string(),
                            )),
                        },
                        Statement::Loop {
                            kind: LoopKind::Infinite,
                            body: vec![
                                Statement::Defer {
                                    call: DeferredCall::Println(ValueExpr::StringLiteral(
                                        "inner loop".to_string(),
                                    )),
                                },
                                Statement::Break,
                            ],
                        },
                        Statement::Break,
                    ],
                }],
            }],
        };

        let c = emit_c(&program);
        let inner = c.find(&puts_literal("inner loop")).unwrap();
        let inner_break = c[inner..].find("break;").unwrap() + inner;
        let outer = c[inner_break..].find(&puts_literal("outer loop")).unwrap() + inner_break;
        let outer_break = c[outer..].find("break;").unwrap() + outer;
        assert!(inner < inner_break);
        assert!(inner_break < outer);
        assert!(outer < outer_break);
        assert_eq!(c.matches(&puts_literal("inner loop")).count(), 1);
        assert_eq!(c.matches(&puts_literal("outer loop")).count(), 1);
    }

    #[test]
    fn emits_return_value_before_deferred_calls() {
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: vec!["std.io".to_string()],
            structs: Vec::new(),
            enums: Vec::new(),
            functions: vec![
                Function {
                    package: "app.main".to_string(),
                    name: "cleanup".to_string(),
                    params: Vec::new(),
                    return_type: ValueType::Void,
                    body: vec![Statement::Println(ValueExpr::StringLiteral(
                        "cleanup".to_string(),
                    ))],
                },
                Function {
                    package: "app.main".to_string(),
                    name: "value".to_string(),
                    params: Vec::new(),
                    return_type: ValueType::Int,
                    body: vec![Statement::Return(Some(ValueExpr::IntLiteral(7)))],
                },
                Function {
                    package: "app.main".to_string(),
                    name: "compute".to_string(),
                    params: Vec::new(),
                    return_type: ValueType::Int,
                    body: vec![
                        Statement::Defer {
                            call: DeferredCall::Expr(ValueExpr::Call {
                                name: "cleanup".to_string(),
                                args: Vec::new(),
                            }),
                        },
                        Statement::Return(Some(ValueExpr::Call {
                            name: "value".to_string(),
                            args: Vec::new(),
                        })),
                    ],
                },
                Function {
                    package: "app.main".to_string(),
                    name: "main".to_string(),
                    params: Vec::new(),
                    return_type: ValueType::Void,
                    body: vec![Statement::Println(ValueExpr::StringLiteral(
                        "done".to_string(),
                    ))],
                },
            ],
        };

        let c = emit_c(&program);
        let value = c.find("long long nomo__return = nomo_fn_value();").unwrap();
        let cleanup = c.find("nomo_fn_cleanup();").unwrap();
        let return_value = c.find("return nomo__return;").unwrap();
        assert!(value < cleanup);
        assert!(cleanup < return_value);
    }

    #[test]
    fn emits_assignment() {
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: vec!["std.io".to_string()],
            structs: Vec::new(),
            enums: Vec::new(),
            functions: vec![Function {
                package: "app.main".to_string(),
                name: "main".to_string(),
                params: Vec::new(),
                return_type: ValueType::Void,
                body: vec![
                    Statement::Let {
                        name: "count".to_string(),
                        value_type: ValueType::Int,
                        initializer: ValueExpr::IntLiteral(1),
                    },
                    Statement::Assign {
                        name: "count".to_string(),
                        value: ValueExpr::Binary {
                            left: Box::new(ValueExpr::Variable("count".to_string())),
                            op: BinaryOp::Add,
                            right: Box::new(ValueExpr::IntLiteral(1)),
                            value_type: ValueType::Int,
                        },
                    },
                ],
            }],
        };

        let c = emit_c(&program);
        assert!(c.contains("long long nomo_count = 1;"));
        assert!(c.contains("nomo_count = nomo_add_i64(nomo_count, 1);"));
    }

    #[test]
    fn emits_field_assignment() {
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: vec!["std.io".to_string()],
            structs: vec![StructType {
                package: "app.main".to_string(),
                name: "Counter".to_string(),
                type_params: Vec::new(),
                fields: vec![StructField {
                    name: "value".to_string(),
                    value_type: ValueType::Int,
                }],
            }],
            enums: Vec::new(),
            functions: vec![Function {
                package: "app.main".to_string(),
                name: "main".to_string(),
                params: Vec::new(),
                return_type: ValueType::Void,
                body: vec![
                    Statement::Let {
                        name: "counter".to_string(),
                        value_type: ValueType::Struct("Counter".to_string(), Vec::new()),
                        initializer: ValueExpr::StructLiteral {
                            type_name: "Counter".to_string(),
                            struct_args: Vec::new(),
                            fields: vec![("value".to_string(), ValueExpr::IntLiteral(1))],
                        },
                    },
                    Statement::AssignField {
                        base: "counter".to_string(),
                        field: "value".to_string(),
                        value_type: ValueType::Int,
                        value: ValueExpr::Binary {
                            left: Box::new(ValueExpr::FieldAccess {
                                base: "counter".to_string(),
                                field: "value".to_string(),
                            }),
                            op: BinaryOp::Add,
                            right: Box::new(ValueExpr::IntLiteral(1)),
                            value_type: ValueType::Int,
                        },
                    },
                ],
            }],
        };

        let c = emit_c(&program);
        assert!(c.contains(
            "nomo_counter.nomo_member_value = nomo_add_i64(nomo_counter.nomo_member_value, 1);"
        ));
    }

    #[test]
    fn emits_struct_type_literal_and_field_access() {
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: vec!["std.io".to_string()],
            structs: vec![StructType {
                package: "app.main".to_string(),
                name: "Point".to_string(),
                type_params: Vec::new(),
                fields: vec![
                    StructField {
                        name: "x".to_string(),
                        value_type: ValueType::Int,
                    },
                    StructField {
                        name: "y".to_string(),
                        value_type: ValueType::Int,
                    },
                ],
            }],
            enums: Vec::new(),
            functions: vec![Function {
                package: "app.main".to_string(),
                name: "main".to_string(),
                params: Vec::new(),
                return_type: ValueType::Void,
                body: vec![
                    Statement::Let {
                        name: "point".to_string(),
                        value_type: ValueType::Struct("Point".to_string(), Vec::new()),
                        initializer: ValueExpr::StructLiteral {
                            type_name: "Point".to_string(),
                            struct_args: Vec::new(),
                            fields: vec![
                                ("x".to_string(), ValueExpr::IntLiteral(1)),
                                ("y".to_string(), ValueExpr::IntLiteral(2)),
                            ],
                        },
                    },
                    Statement::Let {
                        name: "x".to_string(),
                        value_type: ValueType::Int,
                        initializer: ValueExpr::FieldAccess {
                            base: "point".to_string(),
                            field: "x".to_string(),
                        },
                    },
                ],
            }],
        };

        let c = emit_c(&program);
        assert!(c.contains("typedef struct nomo_struct_Point"));
        assert!(c.contains(
            "nomo_struct_Point nomo_point = (nomo_struct_Point){.nomo_member_x = 1, .nomo_member_y = 2};"
        ));
        assert!(c.contains("long long nomo_x = nomo_point.nomo_member_x;"));
    }

    #[test]
    fn emits_generic_struct_instance() {
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: vec!["std.io".to_string()],
            structs: vec![StructType {
                package: "app.main".to_string(),
                name: "Box".to_string(),
                type_params: vec!["T".to_string()],
                fields: vec![StructField {
                    name: "value".to_string(),
                    value_type: ValueType::TypeParam("T".to_string()),
                }],
            }],
            enums: Vec::new(),
            functions: vec![Function {
                package: "app.main".to_string(),
                name: "main".to_string(),
                params: Vec::new(),
                return_type: ValueType::Void,
                body: vec![Statement::Let {
                    name: "item".to_string(),
                    value_type: ValueType::Struct("Box".to_string(), vec![ValueType::I32]),
                    initializer: ValueExpr::StructLiteral {
                        type_name: "Box".to_string(),
                        struct_args: vec![ValueType::I32],
                        fields: vec![("value".to_string(), ValueExpr::IntLiteral(7))],
                    },
                }],
            }],
        };

        let c = emit_c(&program);
        assert!(c.contains("typedef struct nomo_struct_Box_i32"));
        assert!(c.contains("int32_t nomo_member_value;"));
        assert!(c.contains(
            "nomo_struct_Box_i32 nomo_item = (nomo_struct_Box_i32){.nomo_member_value = 7};"
        ));
    }

    #[test]
    fn emits_enum_variant_and_match_expression() {
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: vec!["std.io".to_string()],
            structs: Vec::new(),
            enums: vec![EnumType {
                package: "app.main".to_string(),
                name: "Color".to_string(),
                type_params: Vec::new(),
                variants: vec![
                    EnumVariantType {
                        name: "Red".to_string(),
                        payload: None,
                    },
                    EnumVariantType {
                        name: "Blue".to_string(),
                        payload: None,
                    },
                ],
            }],
            functions: vec![Function {
                package: "app.main".to_string(),
                name: "main".to_string(),
                params: Vec::new(),
                return_type: ValueType::Void,
                body: vec![
                    Statement::Let {
                        name: "color".to_string(),
                        value_type: ValueType::Enum("Color".to_string(), Vec::new()),
                        initializer: ValueExpr::EnumVariant {
                            enum_name: "Color".to_string(),
                            enum_args: Vec::new(),
                            variant: "Red".to_string(),
                            payload: None,
                        },
                    },
                    Statement::Let {
                        name: "label".to_string(),
                        value_type: ValueType::String,
                        initializer: ValueExpr::Match {
                            value: Box::new(ValueExpr::Variable("color".to_string())),
                            arms: vec![
                                MatchValueArm {
                                    enum_name: "Color".to_string(),
                                    enum_args: Vec::new(),
                                    variant: "Red".to_string(),
                                    binding: None,
                                    value: ValueExpr::StringLiteral("red".to_string()),
                                },
                                MatchValueArm {
                                    enum_name: "Color".to_string(),
                                    enum_args: Vec::new(),
                                    variant: "Blue".to_string(),
                                    binding: None,
                                    value: ValueExpr::StringLiteral("blue".to_string()),
                                },
                            ],
                        },
                    },
                ],
            }],
        };

        let c = emit_c(&program);
        assert!(c.contains("typedef enum nomo_enum_Color_tag"));
        assert!(c.contains(
            "nomo_enum_Color nomo_color = (nomo_enum_Color){.tag = nomo_enum_Color_Red};"
        ));
        assert!(c.contains(
            "nomo_color.tag == nomo_enum_Color_Red ? nomo_string_literal(\"red\") : nomo_string_literal(\"blue\")"
        ));
    }

    #[test]
    fn emits_payload_enum_and_match_binding_access() {
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: vec!["std.io".to_string()],
            structs: Vec::new(),
            enums: vec![EnumType {
                package: "app.main".to_string(),
                name: "MaybeInt".to_string(),
                type_params: Vec::new(),
                variants: vec![
                    EnumVariantType {
                        name: "Some".to_string(),
                        payload: Some(ValueType::Int),
                    },
                    EnumVariantType {
                        name: "None".to_string(),
                        payload: None,
                    },
                ],
            }],
            functions: vec![Function {
                package: "app.main".to_string(),
                name: "main".to_string(),
                params: Vec::new(),
                return_type: ValueType::Void,
                body: vec![
                    Statement::Let {
                        name: "value".to_string(),
                        value_type: ValueType::Enum("MaybeInt".to_string(), Vec::new()),
                        initializer: ValueExpr::EnumVariant {
                            enum_name: "MaybeInt".to_string(),
                            enum_args: Vec::new(),
                            variant: "Some".to_string(),
                            payload: Some(Box::new(ValueExpr::IntLiteral(41))),
                        },
                    },
                    Statement::Let {
                        name: "answer".to_string(),
                        value_type: ValueType::Int,
                        initializer: ValueExpr::Match {
                            value: Box::new(ValueExpr::Variable("value".to_string())),
                            arms: vec![
                                MatchValueArm {
                                    enum_name: "MaybeInt".to_string(),
                                    enum_args: Vec::new(),
                                    variant: "Some".to_string(),
                                    binding: Some("n".to_string()),
                                    value: ValueExpr::EnumPayload {
                                        value: Box::new(ValueExpr::Variable("value".to_string())),
                                        variant: "Some".to_string(),
                                    },
                                },
                                MatchValueArm {
                                    enum_name: "MaybeInt".to_string(),
                                    enum_args: Vec::new(),
                                    variant: "None".to_string(),
                                    binding: None,
                                    value: ValueExpr::IntLiteral(0),
                                },
                            ],
                        },
                    },
                ],
            }],
        };

        let c = emit_c(&program);
        assert!(c.contains("union"));
        assert!(c.contains("long long nomo_payload_Some;"));
        assert!(c.contains(".payload.nomo_payload_Some = 41"));
        assert!(c.contains("nomo_value.payload.nomo_payload_Some"));
    }

    #[test]
    fn emits_void_enum_payload_as_unit_storage() {
        let result_void_string = ValueType::Enum(
            "Result".to_string(),
            vec![ValueType::Void, ValueType::String],
        );
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: vec!["std.io".to_string()],
            structs: Vec::new(),
            enums: vec![EnumType {
                package: "app.main".to_string(),
                name: "Result".to_string(),
                type_params: vec!["T".to_string(), "E".to_string()],
                variants: vec![
                    EnumVariantType {
                        name: "Ok".to_string(),
                        payload: Some(ValueType::TypeParam("T".to_string())),
                    },
                    EnumVariantType {
                        name: "Err".to_string(),
                        payload: Some(ValueType::TypeParam("E".to_string())),
                    },
                ],
            }],
            functions: vec![
                Function {
                    package: "app.main".to_string(),
                    name: "write".to_string(),
                    params: Vec::new(),
                    return_type: result_void_string.clone(),
                    body: vec![Statement::Return(Some(ValueExpr::EnumVariant {
                        enum_name: "Result".to_string(),
                        enum_args: vec![ValueType::Void, ValueType::String],
                        variant: "Ok".to_string(),
                        payload: Some(Box::new(ValueExpr::VoidLiteral)),
                    }))],
                },
                Function {
                    package: "app.main".to_string(),
                    name: "main".to_string(),
                    params: Vec::new(),
                    return_type: ValueType::Void,
                    body: vec![Statement::Println(ValueExpr::StringLiteral(
                        "done".to_string(),
                    ))],
                },
            ],
        };

        let c = emit_c(&program);
        assert!(c.contains("char nomo_payload_Ok;"));
        assert!(!c.contains("void nomo_payload_Ok;"));
        assert!(c.contains(".payload.nomo_payload_Ok = 0"));
    }

    #[test]
    fn emits_result_question_let_early_return() {
        let result_i64_string = ValueType::Enum(
            "Result".to_string(),
            vec![ValueType::Int, ValueType::String],
        );
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: vec!["std.io".to_string()],
            structs: Vec::new(),
            enums: vec![EnumType {
                package: "app.main".to_string(),
                name: "Result".to_string(),
                type_params: vec!["T".to_string(), "E".to_string()],
                variants: vec![
                    EnumVariantType {
                        name: "Ok".to_string(),
                        payload: Some(ValueType::TypeParam("T".to_string())),
                    },
                    EnumVariantType {
                        name: "Err".to_string(),
                        payload: Some(ValueType::TypeParam("E".to_string())),
                    },
                ],
            }],
            functions: vec![
                Function {
                    package: "app.main".to_string(),
                    name: "parse".to_string(),
                    params: Vec::new(),
                    return_type: result_i64_string.clone(),
                    body: vec![Statement::Return(Some(ValueExpr::EnumVariant {
                        enum_name: "Result".to_string(),
                        enum_args: vec![ValueType::Int, ValueType::String],
                        variant: "Ok".to_string(),
                        payload: Some(Box::new(ValueExpr::IntLiteral(41))),
                    }))],
                },
                Function {
                    package: "app.main".to_string(),
                    name: "compute".to_string(),
                    params: Vec::new(),
                    return_type: result_i64_string.clone(),
                    body: vec![
                        Statement::QuestionLet {
                            carrier: QuestionCarrier::Result,
                            name: "value".to_string(),
                            value_type: ValueType::Int,
                            result_type: result_i64_string.clone(),
                            return_type: result_i64_string,
                            result_expr: ValueExpr::Call {
                                name: "parse".to_string(),
                                args: Vec::new(),
                            },
                        },
                        Statement::Return(Some(ValueExpr::EnumVariant {
                            enum_name: "Result".to_string(),
                            enum_args: vec![ValueType::Int, ValueType::String],
                            variant: "Ok".to_string(),
                            payload: Some(Box::new(ValueExpr::Binary {
                                left: Box::new(ValueExpr::Variable("value".to_string())),
                                op: BinaryOp::Add,
                                right: Box::new(ValueExpr::IntLiteral(1)),
                                value_type: ValueType::Int,
                            })),
                        })),
                    ],
                },
                Function {
                    package: "app.main".to_string(),
                    name: "main".to_string(),
                    params: Vec::new(),
                    return_type: ValueType::Void,
                    body: vec![Statement::Println(ValueExpr::StringLiteral(
                        "done".to_string(),
                    ))],
                },
            ],
        };

        let c = emit_c(&program);
        assert!(c.contains("nomo_enum_Result_i64_string nomo_value_result = nomo_fn_parse();"));
        assert!(c.contains("if (nomo_value_result.tag == nomo_enum_Result_i64_string_Err) {"));
        assert!(c.contains(
            "nomo_enum_Result_i64_string nomo__question_return = (nomo_enum_Result_i64_string){.tag = nomo_enum_Result_i64_string_Err, .payload.nomo_payload_Err = nomo_value_result.payload.nomo_payload_Err};"
        ));
        assert!(c.contains("return nomo__question_return;"));
        assert!(c.contains("long long nomo_value = nomo_value_result.payload.nomo_payload_Ok;"));
    }

    #[test]
    fn emits_result_void_question_let_without_void_temp() {
        let result_void_string = ValueType::Enum(
            "Result".to_string(),
            vec![ValueType::Void, ValueType::String],
        );
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: vec!["std.io".to_string()],
            structs: Vec::new(),
            enums: vec![EnumType {
                package: "app.main".to_string(),
                name: "Result".to_string(),
                type_params: vec!["T".to_string(), "E".to_string()],
                variants: vec![
                    EnumVariantType {
                        name: "Ok".to_string(),
                        payload: Some(ValueType::TypeParam("T".to_string())),
                    },
                    EnumVariantType {
                        name: "Err".to_string(),
                        payload: Some(ValueType::TypeParam("E".to_string())),
                    },
                ],
            }],
            functions: vec![
                Function {
                    package: "app.main".to_string(),
                    name: "write".to_string(),
                    params: Vec::new(),
                    return_type: result_void_string.clone(),
                    body: vec![Statement::Return(Some(ValueExpr::EnumVariant {
                        enum_name: "Result".to_string(),
                        enum_args: vec![ValueType::Void, ValueType::String],
                        variant: "Ok".to_string(),
                        payload: Some(Box::new(ValueExpr::VoidLiteral)),
                    }))],
                },
                Function {
                    package: "app.main".to_string(),
                    name: "compute".to_string(),
                    params: Vec::new(),
                    return_type: result_void_string.clone(),
                    body: vec![
                        Statement::QuestionLet {
                            carrier: QuestionCarrier::Result,
                            name: "ignored".to_string(),
                            value_type: ValueType::Void,
                            result_type: result_void_string.clone(),
                            return_type: result_void_string.clone(),
                            result_expr: ValueExpr::Call {
                                name: "write".to_string(),
                                args: Vec::new(),
                            },
                        },
                        Statement::Return(Some(ValueExpr::EnumVariant {
                            enum_name: "Result".to_string(),
                            enum_args: vec![ValueType::Void, ValueType::String],
                            variant: "Ok".to_string(),
                            payload: Some(Box::new(ValueExpr::VoidLiteral)),
                        })),
                    ],
                },
                Function {
                    package: "app.main".to_string(),
                    name: "main".to_string(),
                    params: Vec::new(),
                    return_type: ValueType::Void,
                    body: Vec::new(),
                },
            ],
        };

        let c = emit_c(&program);
        assert!(c.contains("char nomo_ignored = nomo_ignored_result.payload.nomo_payload_Ok;"));
        assert!(!c.contains("void nomo_ignored ="));
    }

    #[test]
    fn emits_result_void_question_return_ok_without_void_temp() {
        let result_void_string = ValueType::Enum(
            "Result".to_string(),
            vec![ValueType::Void, ValueType::String],
        );
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: vec!["std.io".to_string()],
            structs: Vec::new(),
            enums: vec![EnumType {
                package: "app.main".to_string(),
                name: "Result".to_string(),
                type_params: vec!["T".to_string(), "E".to_string()],
                variants: vec![
                    EnumVariantType {
                        name: "Ok".to_string(),
                        payload: Some(ValueType::TypeParam("T".to_string())),
                    },
                    EnumVariantType {
                        name: "Err".to_string(),
                        payload: Some(ValueType::TypeParam("E".to_string())),
                    },
                ],
            }],
            functions: vec![
                Function {
                    package: "app.main".to_string(),
                    name: "write".to_string(),
                    params: Vec::new(),
                    return_type: result_void_string.clone(),
                    body: vec![Statement::Return(Some(ValueExpr::EnumVariant {
                        enum_name: "Result".to_string(),
                        enum_args: vec![ValueType::Void, ValueType::String],
                        variant: "Ok".to_string(),
                        payload: Some(Box::new(ValueExpr::VoidLiteral)),
                    }))],
                },
                Function {
                    package: "app.main".to_string(),
                    name: "compute".to_string(),
                    params: Vec::new(),
                    return_type: result_void_string.clone(),
                    body: vec![Statement::QuestionReturnOk {
                        ok_type: ValueType::Void,
                        result_type: result_void_string.clone(),
                        return_type: result_void_string,
                        result_expr: ValueExpr::Call {
                            name: "write".to_string(),
                            args: Vec::new(),
                        },
                    }],
                },
                Function {
                    package: "app.main".to_string(),
                    name: "main".to_string(),
                    params: Vec::new(),
                    return_type: ValueType::Void,
                    body: Vec::new(),
                },
            ],
        };

        let c = emit_c(&program);
        assert!(
            c.contains("char nomo__question_ok = nomo__question_result.payload.nomo_payload_Ok;")
        );
        assert!(c.contains(".payload.nomo_payload_Ok = nomo__question_ok"));
        assert!(!c.contains("void nomo__question_ok ="));
    }
}
