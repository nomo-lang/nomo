use super::*;

pub(super) fn is_string_builtin_call(callee: &[String]) -> bool {
    matches!(
        callee,
        [module, name]
            if module == "string"
                && matches!(
                    name.as_str(),
                    "len"
                        | "concat"
                        | "is_empty"
                        | "contains"
                        | "starts_with"
                        | "ends_with"
                        | "split"
                        | "trim"
                        | "to_lower"
                        | "to_upper"
                )
    )
}

pub(super) fn lower_string_builtin(
    path: &Path,
    callee: &[String],
    args: &[AstExpr],
    scope: &HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    span: &Span,
) -> Result<(ValueType, ValueExpr), Diagnostic> {
    match callee {
        [module, name] if module == "string" && name == "len" => {
            let lowered = lower_string_unary_builtin_arg(
                path, name, args, scope, imports, signatures, structs, enums, span,
            )?;
            Ok((
                ValueType::U64,
                ValueExpr::StringLen {
                    value: Box::new(lowered),
                },
            ))
        }
        [module, name] if module == "string" && name == "concat" => {
            let (lowered_left, lowered_right) = lower_string_binary_builtin_args(
                path, name, args, scope, imports, signatures, structs, enums, span,
            )?;
            Ok((
                ValueType::String,
                ValueExpr::StringConcat {
                    left: Box::new(lowered_left),
                    right: Box::new(lowered_right),
                },
            ))
        }
        [module, name] if module == "string" && name == "is_empty" => {
            let lowered = lower_string_unary_builtin_arg(
                path, name, args, scope, imports, signatures, structs, enums, span,
            )?;
            Ok((
                ValueType::Bool,
                ValueExpr::StringIsEmpty {
                    value: Box::new(lowered),
                },
            ))
        }
        [module, name] if module == "string" && name == "contains" => {
            let (lowered_value, lowered_needle) = lower_string_binary_builtin_args(
                path, name, args, scope, imports, signatures, structs, enums, span,
            )?;
            Ok((
                ValueType::Bool,
                ValueExpr::StringContains {
                    value: Box::new(lowered_value),
                    needle: Box::new(lowered_needle),
                },
            ))
        }
        [module, name] if module == "string" && name == "starts_with" => {
            let (lowered_value, lowered_prefix) = lower_string_binary_builtin_args(
                path, name, args, scope, imports, signatures, structs, enums, span,
            )?;
            Ok((
                ValueType::Bool,
                ValueExpr::StringStartsWith {
                    value: Box::new(lowered_value),
                    prefix: Box::new(lowered_prefix),
                },
            ))
        }
        [module, name] if module == "string" && name == "ends_with" => {
            let (lowered_value, lowered_suffix) = lower_string_binary_builtin_args(
                path, name, args, scope, imports, signatures, structs, enums, span,
            )?;
            Ok((
                ValueType::Bool,
                ValueExpr::StringEndsWith {
                    value: Box::new(lowered_value),
                    suffix: Box::new(lowered_suffix),
                },
            ))
        }
        [module, name] if module == "string" && name == "split" => {
            let (lowered_value, lowered_separator) = lower_string_binary_builtin_args(
                path, name, args, scope, imports, signatures, structs, enums, span,
            )?;
            Ok((
                ValueType::Array(Box::new(ValueType::String)),
                ValueExpr::StringSplit {
                    value: Box::new(lowered_value),
                    separator: Box::new(lowered_separator),
                },
            ))
        }
        [module, name] if module == "string" && name == "trim" => {
            let lowered = lower_string_unary_builtin_arg(
                path, name, args, scope, imports, signatures, structs, enums, span,
            )?;
            Ok((
                ValueType::String,
                ValueExpr::StringTrim {
                    value: Box::new(lowered),
                },
            ))
        }
        [module, name] if module == "string" && name == "to_lower" => {
            let lowered = lower_string_unary_builtin_arg(
                path, name, args, scope, imports, signatures, structs, enums, span,
            )?;
            Ok((
                ValueType::String,
                ValueExpr::StringToLower {
                    value: Box::new(lowered),
                },
            ))
        }
        [module, name] if module == "string" && name == "to_upper" => {
            let lowered = lower_string_unary_builtin_arg(
                path, name, args, scope, imports, signatures, structs, enums, span,
            )?;
            Ok((
                ValueType::String,
                ValueExpr::StringToUpper {
                    value: Box::new(lowered),
                },
            ))
        }
        _ => unreachable!("string builtin dispatcher only passes known calls"),
    }
}

pub(super) fn lower_string_unary_builtin_arg(
    path: &Path,
    name: &str,
    args: &[AstExpr],
    scope: &HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    span: &Span,
) -> Result<ValueExpr, Diagnostic> {
    let [arg] = args else {
        return Err(Diagnostic::new(
            "E0407",
            format!("`string.{name}` expects exactly one string argument"),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    };
    let (arg_type, lowered) =
        lower_value_expr(path, arg, scope, imports, signatures, structs, enums, span)?;
    if arg_type != ValueType::String {
        return Err(type_mismatch(
            path,
            span,
            format!("`string.{name}` expects a string"),
        ));
    }
    Ok(lowered)
}

pub(super) fn lower_string_binary_builtin_args(
    path: &Path,
    name: &str,
    args: &[AstExpr],
    scope: &HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    span: &Span,
) -> Result<(ValueExpr, ValueExpr), Diagnostic> {
    let [left, right] = args else {
        return Err(Diagnostic::new(
            "E0407",
            format!("`string.{name}` expects exactly two string arguments"),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    };
    let (left_type, lowered_left) =
        lower_value_expr(path, left, scope, imports, signatures, structs, enums, span)?;
    let (right_type, lowered_right) = lower_value_expr(
        path, right, scope, imports, signatures, structs, enums, span,
    )?;
    if left_type != ValueType::String || right_type != ValueType::String {
        return Err(type_mismatch(
            path,
            span,
            format!("`string.{name}` expects two strings"),
        ));
    }
    Ok((lowered_left, lowered_right))
}

pub(super) fn is_string_value_method(callee: &[String], scope: &HashMap<String, Binding>) -> bool {
    if callee.len() != 2 {
        return false;
    }
    scope
        .get(&callee[0])
        .is_some_and(|binding| binding.value_type == ValueType::String)
}

pub(super) fn lower_string_value_method(
    path: &Path,
    callee: &[String],
    args: &[AstExpr],
    scope: &HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    span: &Span,
) -> Result<(ValueType, ValueExpr), Diagnostic> {
    let receiver = &callee[0];
    let method = &callee[1];
    let binding = scope
        .get(receiver)
        .expect("string method receiver is in scope");
    let receiver_expr = binding_value_expr(receiver, binding);
    require_string_method_import(path, imports, span, method)?;
    match method.as_str() {
        "len" => {
            if !args.is_empty() {
                return Err(Diagnostic::new(
                    "E0407",
                    "`string.len` does not accept arguments when called as a method",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            }
            Ok((
                ValueType::U64,
                ValueExpr::StringLen {
                    value: Box::new(receiver_expr),
                },
            ))
        }
        "concat" => {
            let lowered_other = lower_string_method_arg(
                path, method, args, scope, imports, signatures, structs, enums, span,
            )?;
            Ok((
                ValueType::String,
                ValueExpr::StringConcat {
                    left: Box::new(receiver_expr),
                    right: Box::new(lowered_other),
                },
            ))
        }
        "is_empty" => {
            require_string_method_arity(path, span, method, args, 0)?;
            Ok((
                ValueType::Bool,
                ValueExpr::StringIsEmpty {
                    value: Box::new(receiver_expr),
                },
            ))
        }
        "contains" => {
            let lowered_needle = lower_string_method_arg(
                path, method, args, scope, imports, signatures, structs, enums, span,
            )?;
            Ok((
                ValueType::Bool,
                ValueExpr::StringContains {
                    value: Box::new(receiver_expr),
                    needle: Box::new(lowered_needle),
                },
            ))
        }
        "starts_with" => {
            let lowered_prefix = lower_string_method_arg(
                path, method, args, scope, imports, signatures, structs, enums, span,
            )?;
            Ok((
                ValueType::Bool,
                ValueExpr::StringStartsWith {
                    value: Box::new(receiver_expr),
                    prefix: Box::new(lowered_prefix),
                },
            ))
        }
        "ends_with" => {
            let lowered_suffix = lower_string_method_arg(
                path, method, args, scope, imports, signatures, structs, enums, span,
            )?;
            Ok((
                ValueType::Bool,
                ValueExpr::StringEndsWith {
                    value: Box::new(receiver_expr),
                    suffix: Box::new(lowered_suffix),
                },
            ))
        }
        "split" => {
            let lowered_separator = lower_string_method_arg(
                path, method, args, scope, imports, signatures, structs, enums, span,
            )?;
            Ok((
                ValueType::Array(Box::new(ValueType::String)),
                ValueExpr::StringSplit {
                    value: Box::new(receiver_expr),
                    separator: Box::new(lowered_separator),
                },
            ))
        }
        "trim" => {
            require_string_method_arity(path, span, method, args, 0)?;
            Ok((
                ValueType::String,
                ValueExpr::StringTrim {
                    value: Box::new(receiver_expr),
                },
            ))
        }
        "to_lower" => {
            require_string_method_arity(path, span, method, args, 0)?;
            Ok((
                ValueType::String,
                ValueExpr::StringToLower {
                    value: Box::new(receiver_expr),
                },
            ))
        }
        "to_upper" => {
            require_string_method_arity(path, span, method, args, 0)?;
            Ok((
                ValueType::String,
                ValueExpr::StringToUpper {
                    value: Box::new(receiver_expr),
                },
            ))
        }
        _ => Err(Diagnostic::new(
            "E0305",
            format!("unknown string method `{method}`"),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        )),
    }
}

pub(super) fn require_string_method_arity(
    path: &Path,
    span: &Span,
    method: &str,
    args: &[AstExpr],
    expected: usize,
) -> Result<(), Diagnostic> {
    if args.len() == expected {
        return Ok(());
    }
    let message = if expected == 0 {
        format!("`string.{method}` does not accept arguments when called as a method")
    } else {
        format!(
            "`string.{method}` expects exactly {expected} string argument(s) when called as a method"
        )
    };
    Err(Diagnostic::new(
        "E0407",
        message,
        path,
        span.line,
        span.column,
        span.length,
        &span.text,
    ))
}

pub(super) fn lower_string_method_arg(
    path: &Path,
    method: &str,
    args: &[AstExpr],
    scope: &HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    span: &Span,
) -> Result<ValueExpr, Diagnostic> {
    require_string_method_arity(path, span, method, args, 1)?;
    let (arg_type, lowered_arg) = lower_value_expr(
        path, &args[0], scope, imports, signatures, structs, enums, span,
    )?;
    if arg_type != ValueType::String {
        return Err(type_mismatch_expected_found(
            path,
            span,
            format!("`string.{method}` expects a string argument"),
            &ValueType::String,
            &arg_type,
        ));
    }
    Ok(lowered_arg)
}
