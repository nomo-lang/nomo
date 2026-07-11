use super::*;

pub(super) fn is_debug_builtin_call(callee: &[String]) -> bool {
    matches!(
        callee,
        [module, name]
            if module == "debug"
                && matches!(name.as_str(), "print" | "println" | "panic" | "backtrace")
    )
}

pub(super) fn is_log_builtin_call(callee: &[String]) -> bool {
    matches!(
        callee,
        [module, name]
            if module == "log"
                && matches!(name.as_str(), "debug" | "info" | "warn" | "error" | "enabled")
    )
}

pub(super) fn lower_log_builtin(
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
    let [module, name] = callee else {
        unreachable!("log builtin dispatcher only passes qualified calls")
    };
    debug_assert_eq!(module, "log");
    match name.as_str() {
        "enabled" => {
            let [level_arg] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`log.enabled` expects exactly one string level",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (level_type, level) = lower_value_expr(
                path, level_arg, scope, imports, signatures, structs, enums, span,
            )?;
            if level_type != ValueType::String {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    "`log.enabled` expects a string level",
                    &ValueType::String,
                    &level_type,
                ));
            }
            Ok((
                ValueType::Bool,
                ValueExpr::LogEnabled {
                    level: Box::new(level),
                },
            ))
        }
        "debug" | "info" | "warn" | "error" => {
            let [message_arg] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    format!("`log.{name}` expects exactly one string message"),
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (message_type, message) = lower_value_expr(
                path,
                message_arg,
                scope,
                imports,
                signatures,
                structs,
                enums,
                span,
            )?;
            if message_type != ValueType::String {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    format!("`log.{name}` expects a string message"),
                    &ValueType::String,
                    &message_type,
                ));
            }
            Ok((ValueType::Void, log_statement_expr(name, message)))
        }
        _ => unreachable!("log builtin dispatcher only passes known calls"),
    }
}

pub(super) fn log_statement_expr(level: &str, message: ValueExpr) -> ValueExpr {
    let prefix = ValueExpr::StringLiteral(format!("[{level}] "));
    ValueExpr::If {
        condition: Box::new(ValueExpr::LogEnabled {
            level: Box::new(ValueExpr::StringLiteral(level.to_string())),
        }),
        then_branch: Box::new(ValueExpr::Call {
            name: BUILTIN_EPRINTLN_EXPR.to_string(),
            args: vec![ValueExpr::StringConcat {
                left: Box::new(prefix),
                right: Box::new(message),
            }],
        }),
        else_branch: Box::new(ValueExpr::VoidLiteral),
    }
}

pub(super) fn lower_debug_builtin(
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
    let [module, name] = callee else {
        unreachable!("debug builtin dispatcher only passes qualified calls")
    };
    debug_assert_eq!(module, "debug");
    match name.as_str() {
        "print" | "println" | "panic" => {
            let [message_arg] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    format!("`debug.{name}` expects exactly one string message"),
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (message_type, message) = lower_value_expr(
                path,
                message_arg,
                scope,
                imports,
                signatures,
                structs,
                enums,
                span,
            )?;
            if message_type != ValueType::String {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    format!("`debug.{name}` expects a string message"),
                    &ValueType::String,
                    &message_type,
                ));
            }
            let value = match name.as_str() {
                "print" => ValueExpr::Call {
                    name: BUILTIN_EPRINT_EXPR.to_string(),
                    args: vec![message],
                },
                "println" => ValueExpr::Call {
                    name: BUILTIN_EPRINTLN_EXPR.to_string(),
                    args: vec![message],
                },
                "panic" => ValueExpr::Panic {
                    message: Box::new(message),
                    fallback_type: ValueType::Void,
                },
                _ => unreachable!("debug string helper matched above"),
            };
            Ok((ValueType::Void, value))
        }
        "backtrace" => {
            if !args.is_empty() {
                return Err(Diagnostic::new(
                    "E0407",
                    "`debug.backtrace` does not accept arguments",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            }
            Ok((
                ValueType::String,
                ValueExpr::StringLiteral("backtrace unavailable".to_string()),
            ))
        }
        _ => unreachable!("debug builtin dispatcher only passes known calls"),
    }
}

pub(super) fn is_testing_builtin_call(callee: &[String]) -> bool {
    matches!(
        callee,
        [module, name]
            if module == "testing"
                && matches!(name.as_str(), "assert" | "assert_equal" | "assert_error")
    )
}

pub(super) fn lower_testing_builtin(
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
    let [module, name] = callee else {
        unreachable!("testing builtin dispatcher only passes qualified calls")
    };
    debug_assert_eq!(module, "testing");
    match name.as_str() {
        "assert" => {
            let [condition_arg, message_arg] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`testing.assert` expects a bool condition and string message",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (condition_type, condition) = lower_value_expr(
                path,
                condition_arg,
                scope,
                imports,
                signatures,
                structs,
                enums,
                span,
            )?;
            if condition_type != ValueType::Bool {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    "`testing.assert` expects a bool condition",
                    &ValueType::Bool,
                    &condition_type,
                ));
            }
            let (message_type, message) = lower_value_expr(
                path,
                message_arg,
                scope,
                imports,
                signatures,
                structs,
                enums,
                span,
            )?;
            if message_type != ValueType::String {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    "`testing.assert` expects a string message",
                    &ValueType::String,
                    &message_type,
                ));
            }
            Ok((ValueType::Void, assert_expr(condition, message)))
        }
        "assert_equal" => {
            let [left_arg, right_arg] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`testing.assert_equal` expects two comparable values",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (left_type, left) = lower_value_expr(
                path, left_arg, scope, imports, signatures, structs, enums, span,
            )?;
            let (right_type, right) = lower_value_expr(
                path, right_arg, scope, imports, signatures, structs, enums, span,
            )?;
            if left_type != right_type {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    "`testing.assert_equal` expects both values to have the same type",
                    &left_type,
                    &right_type,
                ));
            }
            let condition = equality_expr(left, right, &left_type).ok_or_else(|| {
                type_mismatch(
                    path,
                    span,
                    format!(
                        "`testing.assert_equal` does not support values of type `{}`",
                        left_type.name()
                    ),
                )
            })?;
            Ok((
                ValueType::Void,
                assert_expr(
                    condition,
                    ValueExpr::StringLiteral("assert_equal failed".to_string()),
                ),
            ))
        }
        "assert_error" => {
            let [result_arg] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`testing.assert_error` expects one Result<T, E> value",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (result_type, result) = lower_value_expr(
                path, result_arg, scope, imports, signatures, structs, enums, span,
            )?;
            let ValueType::Enum(enum_name, enum_args) = result_type.clone() else {
                return Err(type_mismatch(
                    path,
                    span,
                    "`testing.assert_error` expects a Result<T, E> value",
                ));
            };
            if enum_name != "Result" || enum_args.len() != 2 {
                return Err(type_mismatch(
                    path,
                    span,
                    "`testing.assert_error` expects a Result<T, E> value",
                ));
            }
            let condition = ValueExpr::ResultIsErr {
                result: Box::new(result),
                ok_type: enum_args[0].clone(),
                err_type: enum_args[1].clone(),
            };
            Ok((
                ValueType::Void,
                assert_expr(
                    condition,
                    ValueExpr::StringLiteral("expected Result.Err".to_string()),
                ),
            ))
        }
        _ => unreachable!("testing builtin dispatcher only passes known calls"),
    }
}

pub(super) fn assert_expr(condition: ValueExpr, message: ValueExpr) -> ValueExpr {
    ValueExpr::If {
        condition: Box::new(condition),
        then_branch: Box::new(ValueExpr::VoidLiteral),
        else_branch: Box::new(ValueExpr::Panic {
            message: Box::new(message),
            fallback_type: ValueType::Void,
        }),
    }
}

pub(super) fn equality_expr(
    left: ValueExpr,
    right: ValueExpr,
    value_type: &ValueType,
) -> Option<ValueExpr> {
    match value_type {
        ValueType::String => Some(ValueExpr::StringCompare {
            left: Box::new(left),
            op: BinaryOp::Equal,
            right: Box::new(right),
        }),
        ValueType::Char
        | ValueType::Bool
        | ValueType::Int
        | ValueType::I32
        | ValueType::U64
        | ValueType::Float => Some(ValueExpr::Binary {
            left: Box::new(left),
            op: BinaryOp::Equal,
            right: Box::new(right),
            value_type: value_type.clone(),
        }),
        _ => None,
    }
}
