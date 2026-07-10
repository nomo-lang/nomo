use super::*;
use nomo_ir::{EnumVariantType, MatchValueArm, Parameter, StructField, ValueExpr};

#[path = "tests_array_lifecycle.rs"]
mod tests_array_lifecycle;
#[path = "tests_array_loop_lifecycle.rs"]
mod tests_array_loop_lifecycle;
#[path = "tests_array_question_lifecycle.rs"]
mod tests_array_question_lifecycle;
#[path = "tests_basic_io_symbols.rs"]
mod tests_basic_io_symbols;
#[path = "tests_defer_control.rs"]
mod tests_defer_control;
#[path = "tests_expressions.rs"]
mod tests_expressions;
#[path = "tests_ffi.rs"]
mod tests_ffi;
#[path = "tests_host_helpers.rs"]
mod tests_host_helpers;
#[path = "tests_host_net_helpers.rs"]
mod tests_host_net_helpers;
#[path = "tests_nominal.rs"]
mod tests_nominal;
#[path = "tests_result_question.rs"]
mod tests_result_question;
#[path = "tests_statements.rs"]
mod tests_statements;
#[path = "tests_std_primitives.rs"]
mod tests_std_primitives;

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
