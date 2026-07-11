use super::*;

pub(super) fn is_option_builtin_call(callee: &[String]) -> bool {
    matches!(
        callee,
        [module, name]
            if module == "option"
                && matches!(
                    name.as_str(),
                    "is_some" | "is_none" | "unwrap_or" | "map" | "and_then"
                )
    )
}

pub(super) fn lower_option_builtin(
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
        unreachable!("option builtin dispatcher only passes qualified calls");
    };
    debug_assert_eq!(module, "option");
    match method.as_str() {
        "is_some" | "is_none" => {
            let [option] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    format!("`option.{method}` expects exactly one Option argument"),
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (option_type, lowered_option) = lower_value_expr(
                path, option, scope, imports, signatures, structs, enums, span,
            )?;
            let payload_type = option_payload(&option_type).ok_or_else(|| {
                type_mismatch(
                    path,
                    span,
                    format!("`option.{method}` expects an Option value"),
                )
            })?;
            let value = if method == "is_some" {
                ValueExpr::OptionIsSome {
                    option: Box::new(lowered_option),
                    payload_type,
                }
            } else {
                ValueExpr::OptionIsNone {
                    option: Box::new(lowered_option),
                    payload_type,
                }
            };
            Ok((ValueType::Bool, value))
        }
        "unwrap_or" => {
            let [option, default] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`option.unwrap_or` expects an Option value and a default value",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (option_type, lowered_option) = lower_value_expr(
                path, option, scope, imports, signatures, structs, enums, span,
            )?;
            let payload_type = option_payload(&option_type).ok_or_else(|| {
                type_mismatch(path, span, "`option.unwrap_or` expects an Option value")
            })?;
            if payload_type == ValueType::Void {
                return Err(type_mismatch(
                    path,
                    span,
                    "`option.unwrap_or` does not support Option<void>",
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
                Some(&payload_type),
                span,
            )?;
            if default_type != payload_type {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    format!(
                        "`option.unwrap_or` default is `{}` but payload is `{}`",
                        default_type.name(),
                        payload_type.name()
                    ),
                    &payload_type,
                    &default_type,
                ));
            }
            Ok((
                payload_type.clone(),
                ValueExpr::OptionUnwrapOr {
                    option: Box::new(lowered_option),
                    default: Box::new(lowered_default),
                    payload_type,
                },
            ))
        }
        "map" | "and_then" => {
            let [option, converter] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    format!("`option.{method}` expects an Option value and a converter function"),
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (option_type, lowered_option) = lower_value_expr(
                path, option, scope, imports, signatures, structs, enums, span,
            )?;
            let source_type = option_payload(&option_type).ok_or_else(|| {
                type_mismatch(
                    path,
                    span,
                    format!("`option.{method}` expects an Option value"),
                )
            })?;
            lower_option_converter_call(
                path,
                span,
                method,
                lowered_option,
                source_type,
                converter,
                signatures,
            )
        }
        _ => unreachable!("option builtin dispatcher only passes known calls"),
    }
}

pub(super) fn lower_option_value_method(
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
        "is_some" | "is_none" | "unwrap_or" | "map" | "and_then"
    ) {
        return Ok(None);
    }
    let Some(binding) = scope.get(receiver_name) else {
        return Ok(None);
    };
    let Some(payload_type) = option_payload(&binding.value_type) else {
        if matches!(method.as_str(), "unwrap_or" | "map" | "and_then")
            && result_parts(&binding.value_type).is_some()
        {
            return Ok(None);
        }
        return Err(type_mismatch(
            path,
            span,
            format!("`{receiver_name}.{method}` expects an Option value"),
        ));
    };
    require_option_method_import(path, imports, span, method)?;
    let option = binding_value_expr(receiver_name, binding);
    match method.as_str() {
        "is_some" => Ok(Some((
            ValueType::Bool,
            ValueExpr::OptionIsSome {
                option: Box::new(option),
                payload_type,
            },
        ))),
        "is_none" => Ok(Some((
            ValueType::Bool,
            ValueExpr::OptionIsNone {
                option: Box::new(option),
                payload_type,
            },
        ))),
        "unwrap_or" => {
            let [default] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`Option.unwrap_or` expects exactly one default value",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            if payload_type == ValueType::Void {
                return Err(type_mismatch(
                    path,
                    span,
                    "`Option.unwrap_or` does not support Option<void>",
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
                Some(&payload_type),
                span,
            )?;
            if default_type != payload_type {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    format!(
                        "`Option.unwrap_or` default is `{}` but payload is `{}`",
                        default_type.name(),
                        payload_type.name()
                    ),
                    &payload_type,
                    &default_type,
                ));
            }
            Ok(Some((
                payload_type.clone(),
                ValueExpr::OptionUnwrapOr {
                    option: Box::new(option),
                    default: Box::new(lowered_default),
                    payload_type,
                },
            )))
        }
        "map" | "and_then" => {
            let [converter] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    format!("`Option.{method}` expects exactly one converter function"),
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            lower_option_converter_call(
                path,
                span,
                method,
                option,
                payload_type,
                converter,
                signatures,
            )
            .map(Some)
        }
        _ => unreachable!("option method dispatcher only passes known calls"),
    }
}

pub(super) fn lower_option_converter_call(
    path: &Path,
    span: &Span,
    method: &str,
    option: ValueExpr,
    source_type: ValueType,
    converter: &AstExpr,
    signatures: &HashMap<String, FunctionSignature>,
) -> Result<(ValueType, ValueExpr), Diagnostic> {
    let converter_name = option_converter_name(path, span, method, converter)?;
    let converter_signature =
        option_converter_signature(path, span, method, &converter_name, signatures)?;
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
    if converter_param.value_type != source_type {
        return Err(type_mismatch_expected_found(
            path,
            span,
            format!(
                "`Option.{method}` converter `{converter_name}` takes `{}` but payload is `{}`",
                converter_param.value_type.name(),
                source_type.name()
            ),
            &source_type,
            &converter_param.value_type,
        ));
    }
    match method {
        "map" => {
            if converter_signature.return_type == ValueType::Void {
                return Err(type_mismatch(
                    path,
                    span,
                    format!("converter function `{converter_name}` must return a mapped value"),
                ));
            }
            let target_type = converter_signature.return_type.clone();
            Ok((
                ValueType::Enum("Option".to_string(), vec![target_type.clone()]),
                ValueExpr::OptionMap {
                    option: Box::new(option),
                    source_type,
                    target_type,
                    converter: converter_name,
                },
            ))
        }
        "and_then" => {
            let Some(target_type) = option_payload(&converter_signature.return_type) else {
                return Err(type_mismatch(
                    path,
                    span,
                    format!("converter function `{converter_name}` must return an Option value"),
                ));
            };
            Ok((
                ValueType::Enum("Option".to_string(), vec![target_type.clone()]),
                ValueExpr::OptionAndThen {
                    option: Box::new(option),
                    source_type,
                    target_type,
                    converter: converter_name,
                },
            ))
        }
        _ => unreachable!("option converter helper only supports map/and_then"),
    }
}

pub(super) fn option_converter_name(
    path: &Path,
    span: &Span,
    method: &str,
    converter: &AstExpr,
) -> Result<String, Diagnostic> {
    let AstExpr::Name(converter_path) = converter else {
        return Err(Diagnostic::new(
            "E0407",
            format!("`Option.{method}` expects a converter function name"),
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
            format!("`Option.{method}` expects an unqualified converter function name"),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    };
    Ok(converter_name.clone())
}

pub(super) fn option_converter_signature<'a>(
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
            format!("`Option.{method}` converter `{converter_name}` must not be generic"),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    }
    Ok(converter_signature)
}
