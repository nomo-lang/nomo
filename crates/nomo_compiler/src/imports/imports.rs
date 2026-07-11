use super::*;

pub(super) fn is_io_print_call(callee: &[String]) -> bool {
    matches!(
        callee,
        [module, name]
            if module == "io"
                && matches!(name.as_str(), "print" | "println" | "eprint" | "eprintln")
    ) || matches!(
        callee,
        [name] if matches!(name.as_str(), "print" | "println" | "eprint" | "eprintln")
    )
}

pub(super) fn io_print_statement(function_name: &str, arg: ValueExpr) -> Statement {
    match function_name {
        "print" => Statement::Print(arg),
        "eprint" => Statement::Eprint(arg),
        "eprintln" => Statement::Eprintln(arg),
        "println" => Statement::Println(arg),
        _ => unreachable!("io print dispatcher only passes known functions"),
    }
}

pub(super) fn io_print_deferred_call(function_name: &str, arg: ValueExpr) -> DeferredCall {
    match function_name {
        "print" => DeferredCall::Print(arg),
        "eprint" => DeferredCall::Eprint(arg),
        "eprintln" => DeferredCall::Eprintln(arg),
        "println" => DeferredCall::Println(arg),
        _ => unreachable!("io print dispatcher only passes known functions"),
    }
}

pub(super) fn io_print_builtin_expr_name(function_name: &str) -> String {
    match function_name {
        "print" => BUILTIN_PRINT_EXPR.to_string(),
        "eprint" => BUILTIN_EPRINT_EXPR.to_string(),
        "eprintln" => BUILTIN_EPRINTLN_EXPR.to_string(),
        "println" => BUILTIN_PRINTLN_EXPR.to_string(),
        _ => unreachable!("io print dispatcher only passes known functions"),
    }
}
