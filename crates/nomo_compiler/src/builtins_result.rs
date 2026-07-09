use super::*;
pub(super) fn is_result_builtin_call(callee: &[String]) -> bool {
    matches!(
        callee,
        [module, name]
            if module == "result"
                && matches!(
                    name.as_str(),
                    "is_ok" | "is_err" | "unwrap_or" | "map" | "map_err" | "and_then"
                )
    )
}
pub(super) fn lower_result_builtin(
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
    let [module, method] = callee else {
        unreachable!("result builtin dispatcher only passes qualified calls");
    };
    debug_assert_eq!(module, "result");
    match method.as_str() {
        "is_ok" | "is_err" => {
            let [result] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    format!("`result.{method}` expects exactly one Result argument"),
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (result_type, lowered_result) = lower_value_expr(
                path, result, scope, imports, signatures, structs, enums, span,
            )?;
            lower_result_predicate(path, span, method, lowered_result, &result_type)
        }
        "unwrap_or" => {
            let [result, default] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`result.unwrap_or` expects a Result value and a default value",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (result_type, lowered_result) = lower_value_expr(
                path, result, scope, imports, signatures, structs, enums, span,
            )?;
            lower_result_unwrap_or(
                path,
                span,
                "result.unwrap_or",
                lowered_result,
                &result_type,
                default,
                scope,
                imports,
                signatures,
                structs,
                enums,
            )
        }
        "map" | "map_err" | "and_then" => {
            let [result, converter] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    format!("`result.{method}` expects a Result value and a converter function"),
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (result_type, lowered_result) = lower_value_expr(
                path, result, scope, imports, signatures, structs, enums, span,
            )?;
            let (ok_type, err_type) = result_parts(&result_type).ok_or_else(|| {
                type_mismatch(
                    path,
                    span,
                    format!("`result.{method}` expects a Result value"),
                )
            })?;
            lower_result_converter_call(
                path,
                span,
                method,
                lowered_result,
                ok_type,
                err_type,
                converter,
                signatures,
            )
        }
        _ => unreachable!("result builtin dispatcher only passes known calls"),
    }
}

pub(super) fn lower_result_value_method(
    path: &Path,
    callee: &[String],
    args: &[AstExpr],
    scope: &HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    span: &Span,
) -> Result<Option<(ValueType, ValueExpr)>, Diagnostic> {
    if callee.len() != 2 {
        return Ok(None);
    }
    let receiver_name = &callee[0];
    let method = &callee[1];
    if !matches!(
        method.as_str(),
        "is_ok" | "is_err" | "unwrap_or" | "map" | "map_err" | "and_then"
    ) {
        return Ok(None);
    }
    let Some(binding) = scope.get(receiver_name) else {
        return Ok(None);
    };
    require_result_method_import(path, imports, span, method)?;
    let result = binding_value_expr(receiver_name, binding);
    let (ok_type, err_type) = result_parts(&binding.value_type).ok_or_else(|| {
        type_mismatch(
            path,
            span,
            format!("`{receiver_name}.{method}` expects a Result value"),
        )
    })?;
    match method.as_str() {
        "is_ok" | "is_err" => {
            if !args.is_empty() {
                return Err(Diagnostic::new(
                    "E0407",
                    format!("`Result.{method}` expects no arguments"),
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            }
            lower_result_predicate(path, span, method, result, &binding.value_type).map(Some)
        }
        "unwrap_or" => {
            let [default] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`Result.unwrap_or` expects exactly one default value",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            lower_result_unwrap_or(
                path,
                span,
                "Result.unwrap_or",
                result,
                &binding.value_type,
                default,
                scope,
                imports,
                signatures,
                structs,
                enums,
            )
            .map(Some)
        }
        "map" | "map_err" | "and_then" => {
            let [converter] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    format!("`Result.{method}` expects exactly one converter function"),
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            lower_result_converter_call(
                path, span, method, result, ok_type, err_type, converter, signatures,
            )
            .map(Some)
        }
        _ => unreachable!("result method dispatcher only passes known calls"),
    }
}

pub(super) fn lower_result_predicate(
    path: &Path,
    span: &Span,
    method: &str,
    result: ValueExpr,
    result_type: &ValueType,
) -> Result<(ValueType, ValueExpr), Diagnostic> {
    let (ok_type, err_type) = result_parts(result_type).ok_or_else(|| {
        type_mismatch(
            path,
            span,
            format!("`Result.{method}` expects a Result value"),
        )
    })?;
    let value = if method == "is_ok" {
        ValueExpr::ResultIsOk {
            result: Box::new(result),
            ok_type,
            err_type,
        }
    } else {
        ValueExpr::ResultIsErr {
            result: Box::new(result),
            ok_type,
            err_type,
        }
    };
    Ok((ValueType::Bool, value))
}

#[allow(clippy::too_many_arguments)]
pub(super) fn lower_result_unwrap_or(
    path: &Path,
    span: &Span,
    label: &str,
    result: ValueExpr,
    result_type: &ValueType,
    default: &AstExpr,
    scope: &HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
) -> Result<(ValueType, ValueExpr), Diagnostic> {
    let (ok_type, err_type) = result_parts(result_type)
        .ok_or_else(|| type_mismatch(path, span, format!("`{label}` expects a Result value")))?;
    if ok_type == ValueType::Void {
        return Err(type_mismatch(
            path,
            span,
            format!("`{label}` does not support Result<void, E>"),
        ));
    }
    let (default_type, lowered_default) = lower_value_expr_with_expected(
        path,
        default,
        scope,
        imports,
        signatures,
        structs,
        enums,
        Some(&ok_type),
        span,
    )?;
    if default_type != ok_type {
        return Err(type_mismatch_expected_found(
            path,
            span,
            format!(
                "`{label}` default is `{}` but ok type is `{}`",
                default_type.name(),
                ok_type.name()
            ),
            &ok_type,
            &default_type,
        ));
    }
    Ok((
        ok_type.clone(),
        ValueExpr::ResultUnwrapOr {
            result: Box::new(result),
            default: Box::new(lowered_default),
            ok_type,
            err_type,
        },
    ))
}

pub(super) fn lower_result_converter_call(
    path: &Path,
    span: &Span,
    method: &str,
    result: ValueExpr,
    ok_type: ValueType,
    err_type: ValueType,
    converter: &AstExpr,
    signatures: &HashMap<String, FunctionSignature>,
) -> Result<(ValueType, ValueExpr), Diagnostic> {
    let converter_name = result_converter_name(path, span, method, converter)?;
    let converter_signature =
        result_converter_signature(path, span, method, &converter_name, signatures)?;
    let [converter_param] = converter_signature.params.as_slice() else {
        return Err(Diagnostic::new(
            "E0407",
            format!("converter function `{converter_name}` must take exactly one argument"),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    };
    match method {
        "map" => {
            if ok_type == ValueType::Void {
                return Err(type_mismatch(
                    path,
                    span,
                    "`Result.map` does not support Result<void, E>",
                ));
            }
            if converter_param.value_type != ok_type {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    format!(
                        "`Result.map` converter `{converter_name}` takes `{}` but ok type is `{}`",
                        converter_param.value_type.name(),
                        ok_type.name()
                    ),
                    &ok_type,
                    &converter_param.value_type,
                ));
            }
            if converter_signature.return_type == ValueType::Void {
                return Err(type_mismatch(
                    path,
                    span,
                    format!("converter function `{converter_name}` must return a mapped value"),
                ));
            }
            let target_ok_type = converter_signature.return_type.clone();
            Ok((
                ValueType::Enum(
                    "Result".to_string(),
                    vec![target_ok_type.clone(), err_type.clone()],
                ),
                ValueExpr::ResultMap {
                    result: Box::new(result),
                    source_ok_type: ok_type,
                    target_ok_type,
                    err_type,
                    converter: converter_name,
                },
            ))
        }
        "map_err" => {
            if converter_param.value_type != err_type {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    format!(
                        "`Result.map_err` converter `{converter_name}` takes `{}` but error type is `{}`",
                        converter_param.value_type.name(),
                        err_type.name()
                    ),
                    &err_type,
                    &converter_param.value_type,
                ));
            }
            if converter_signature.return_type == ValueType::Void {
                return Err(type_mismatch(
                    path,
                    span,
                    format!("converter function `{converter_name}` must return an error value"),
                ));
            }
            let target_err_type = converter_signature.return_type.clone();
            Ok((
                ValueType::Enum(
                    "Result".to_string(),
                    vec![ok_type.clone(), target_err_type.clone()],
                ),
                ValueExpr::ResultMapErr {
                    result: Box::new(result),
                    ok_type,
                    source_err_type: err_type,
                    target_err_type,
                    converter: converter_name,
                },
            ))
        }
        "and_then" => {
            if ok_type == ValueType::Void {
                return Err(type_mismatch(
                    path,
                    span,
                    "`Result.and_then` does not support Result<void, E>",
                ));
            }
            if converter_param.value_type != ok_type {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    format!(
                        "`Result.and_then` converter `{converter_name}` takes `{}` but ok type is `{}`",
                        converter_param.value_type.name(),
                        ok_type.name()
                    ),
                    &ok_type,
                    &converter_param.value_type,
                ));
            }
            let Some((target_ok_type, target_err_type)) =
                result_parts(&converter_signature.return_type)
            else {
                return Err(type_mismatch(
                    path,
                    span,
                    format!("converter function `{converter_name}` must return a Result value"),
                ));
            };
            if target_err_type != err_type {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    format!(
                        "`Result.and_then` converter `{converter_name}` returns error `{}` but source error is `{}`",
                        target_err_type.name(),
                        err_type.name()
                    ),
                    &err_type,
                    &target_err_type,
                ));
            }
            Ok((
                ValueType::Enum(
                    "Result".to_string(),
                    vec![target_ok_type.clone(), err_type.clone()],
                ),
                ValueExpr::ResultAndThen {
                    result: Box::new(result),
                    source_ok_type: ok_type,
                    target_ok_type,
                    err_type,
                    converter: converter_name,
                },
            ))
        }
        _ => unreachable!("result converter helper only supports map/map_err/and_then"),
    }
}

pub(super) fn result_converter_name(
    path: &Path,
    span: &Span,
    method: &str,
    converter: &AstExpr,
) -> Result<String, Diagnostic> {
    let AstExpr::Name(converter_path) = converter else {
        return Err(Diagnostic::new(
            "E0407",
            format!("`Result.{method}` expects a converter function name"),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    };
    let [converter_name] = converter_path.as_slice() else {
        return Err(Diagnostic::new(
            "E0407",
            format!("`Result.{method}` expects an unqualified converter function name"),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    };
    Ok(converter_name.clone())
}

pub(super) fn result_converter_signature<'a>(
    path: &Path,
    span: &Span,
    method: &str,
    converter_name: &str,
    signatures: &'a HashMap<String, FunctionSignature>,
) -> Result<&'a FunctionSignature, Diagnostic> {
    let Some(converter_signature) = signatures.get(converter_name) else {
        return Err(Diagnostic::new(
            "E0305",
            format!("unknown converter function `{converter_name}`"),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    };
    if !converter_signature.type_params.is_empty() {
        return Err(Diagnostic::new(
            "E0407",
            format!("`Result.{method}` converter `{converter_name}` must not be generic"),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    }
    Ok(converter_signature)
}
