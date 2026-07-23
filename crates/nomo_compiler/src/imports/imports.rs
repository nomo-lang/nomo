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

#[allow(clippy::too_many_arguments)]
pub(super) fn lower_io_print_args(
    path: &Path,
    args: &[AstExpr],
    scope: &HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    span: &Span,
    function_name: &str,
) -> Result<ValueExpr, Diagnostic> {
    let mut rendered = Vec::with_capacity(args.len());
    for arg in args {
        let (value_type, value) =
            lower_value_expr(path, arg, scope, imports, signatures, structs, enums, span)?;
        let value = match value_type {
            ValueType::String => value,
            value_type if value_type.is_numeric() => ValueExpr::NumToString {
                value: Box::new(value),
                value_type,
            },
            ValueType::Char => ValueExpr::CharToString {
                value: Box::new(value),
            },
            ValueType::Bool => ValueExpr::If {
                condition: Box::new(value),
                then_branch: Box::new(ValueExpr::StringLiteral("true".to_string())),
                else_branch: Box::new(ValueExpr::StringLiteral("false".to_string())),
            },
            _ => return Err(println_type_error(path, span, function_name)),
        };
        rendered.push(value);
    }

    let mut rendered = rendered.into_iter();
    let Some(first) = rendered.next() else {
        return Ok(ValueExpr::StringLiteral(String::new()));
    };
    Ok(rendered.fold(first, |left, right| {
        let with_separator = ValueExpr::StringConcat {
            left: Box::new(left),
            right: Box::new(ValueExpr::StringLiteral(" ".to_string())),
        };
        ValueExpr::StringConcat {
            left: Box::new(with_separator),
            right: Box::new(right),
        }
    }))
}
