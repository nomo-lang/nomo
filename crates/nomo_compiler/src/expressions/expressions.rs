use super::*;

pub(super) fn lower_value_expr(
    path: &Path,
    expr: &AstExpr,
    scope: &HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    span: &Span,
) -> Result<(ValueType, ValueExpr), Diagnostic> {
    lower_value_expr_with_expected(
        path, expr, scope, imports, signatures, structs, enums, None, span,
    )
}

pub(super) fn lower_value_expr_with_expected(
    path: &Path,
    expr: &AstExpr,
    scope: &HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    expected: Option<&ValueType>,
    span: &Span,
) -> Result<(ValueType, ValueExpr), Diagnostic> {
    match expr {
        AstExpr::ArrayLiteral { elements } => {
            let expected_element = match expected {
                Some(ValueType::Array(element)) => Some(element.as_ref()),
                Some(other) => {
                    return Err(type_mismatch_expected_found(
                        path,
                        span,
                        "array literal requires an Array<T> context",
                        other,
                        &ValueType::Array(Box::new(ValueType::Never)),
                    ));
                }
                None => None,
            };
            if elements.is_empty() && expected_element.is_none() {
                return Err(Diagnostic::new(
                    "E0860",
                    "cannot infer the element type of empty array `[]`; add an `Array<T>` annotation",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            }
            let mut lowered = Vec::with_capacity(elements.len());
            let mut element_type = expected_element.cloned();
            if element_type.is_none() && matches!(elements.first(), Some(AstExpr::Int(_))) {
                // Collection literals use the fixed-width v0.1 element default
                // requested by RFC 0030 without changing the established scalar
                // integer default used by existing Nomo programs.
                element_type = Some(ValueType::I32);
            }
            for (position, element) in elements.iter().enumerate() {
                let (actual, value) = lower_value_expr_with_expected(
                    path,
                    element,
                    scope,
                    imports,
                    signatures,
                    structs,
                    enums,
                    element_type.as_ref(),
                    span,
                )?;
                if let Some(expected) = &element_type {
                    if &actual != expected {
                        return Err(Diagnostic::new(
                            "E0861",
                            format!(
                                "array element {position} has type `{}` but `{}` was expected; array literals do not perform implicit conversion",
                                actual.name(),
                                expected.name()
                            ),
                            path,
                            span.line,
                            span.column,
                            span.length,
                            &span.text,
                        ));
                    }
                } else {
                    element_type = Some(actual.clone());
                }
                lowered.push(value);
            }
            let element_type = element_type.expect("empty literal has expected element type");
            Ok((
                ValueType::Array(Box::new(element_type.clone())),
                ValueExpr::ArrayLiteral {
                    elements: lowered,
                    element_type,
                },
            ))
        }
        AstExpr::Index { base, index } => {
            let (array_type, array) =
                lower_value_expr(path, base, scope, imports, signatures, structs, enums, span)?;
            let ValueType::Array(element_type) = array_type else {
                return Err(Diagnostic::new(
                    "E0862",
                    format!(
                        "cannot index value of type `{}`; expected `Array<T>`",
                        array_type.name()
                    ),
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (index_type, index) = lower_value_expr_with_expected(
                path,
                index,
                scope,
                imports,
                signatures,
                structs,
                enums,
                Some(&ValueType::U64),
                span,
            )?;
            if index_type != ValueType::U64 {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    "array index must have type `u64`",
                    &ValueType::U64,
                    &index_type,
                ));
            }
            Ok((
                element_type.as_ref().clone(),
                ValueExpr::ArrayIndex {
                    array: Box::new(array),
                    index: Box::new(index),
                    element_type: element_type.as_ref().clone(),
                },
            ))
        }
        AstExpr::String(value) => Ok((ValueType::String, ValueExpr::StringLiteral(value.clone()))),
        AstExpr::Int(value) => lower_int_literal(path, *value, expected, span),
        AstExpr::Float(value) => Ok((ValueType::Float, ValueExpr::FloatLiteral(value.clone()))),
        AstExpr::Char(value) => Ok((ValueType::Char, ValueExpr::CharLiteral(*value))),
        AstExpr::Bool(value) => Ok((ValueType::Bool, ValueExpr::BoolLiteral(*value))),
        AstExpr::Void => Ok((ValueType::Void, ValueExpr::VoidLiteral)),
        AstExpr::MutArg { .. } => Err(Diagnostic::new(
            "E0505",
            "`mut` is only valid in function call arguments",
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        )),
        AstExpr::Name(name) if name.len() == 1 => {
            let name = &name[0];
            let Some(binding) = scope.get(name) else {
                if let Some((enum_name, variant)) = core_prelude_variant(name) {
                    return lower_enum_variant_without_payload(
                        path, enum_name, variant, enums, expected, span,
                    );
                }
                return Err(Diagnostic::new(
                    "E0303",
                    format!("unknown variable `{name}`"),
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let _ = binding.mutable;
            if let BindingSource::EnumPayload { value, variant } = &binding.source {
                return Ok((
                    binding.value_type.clone(),
                    ValueExpr::EnumPayload {
                        value: Box::new(value.clone()),
                        variant: variant.clone(),
                    },
                ));
            }
            Ok((
                binding.value_type.clone(),
                ValueExpr::Variable(name.clone()),
            ))
        }
        AstExpr::Name(name) if name.len() == 2 => {
            let base = &name[0];
            let field = &name[1];
            if let Some(enum_type) = enums.get(base) {
                let Some(variant_type) = enum_type
                    .variants
                    .iter()
                    .find(|variant| variant.name == *field)
                else {
                    return Err(Diagnostic::new(
                        "E0315",
                        format!("enum `{base}` has no variant `{field}`"),
                        path,
                        span.line,
                        span.column,
                        span.length,
                        &span.text,
                    ));
                };
                if variant_type.payload.is_some() {
                    return Err(Diagnostic::new(
                        "E0320",
                        format!("enum variant `{base}.{field}` requires a payload"),
                        path,
                        span.line,
                        span.column,
                        span.length,
                        &span.text,
                    ));
                }
                let enum_args = match expected {
                    Some(ValueType::Enum(expected_name, expected_args))
                        if expected_name == base =>
                    {
                        expected_args.clone()
                    }
                    _ if enum_type.type_params.is_empty() => Vec::new(),
                    _ => {
                        return Err(Diagnostic::new(
                            "E0324",
                            format!(
                                "generic enum constructor `{base}.{field}` needs a type annotation"
                            ),
                            path,
                            span.line,
                            span.column,
                            span.length,
                            &span.text,
                        ));
                    }
                };
                return Ok((
                    ValueType::Enum(base.clone(), enum_args.clone()),
                    ValueExpr::EnumVariant {
                        enum_name: base.clone(),
                        enum_args,
                        variant: field.clone(),
                        payload: None,
                    },
                ));
            }
            let Some(binding) = scope.get(base) else {
                return Err(Diagnostic::new(
                    "E0303",
                    format!("unknown variable `{base}`"),
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let ValueType::Struct(type_name, struct_args) = &binding.value_type else {
                return Err(type_mismatch(
                    path,
                    span,
                    format!("`{base}` is not a struct value"),
                ));
            };
            let struct_type = structs
                .get(type_name)
                .expect("struct binding must refer to a known struct");
            if is_task_runtime_opaque_struct(struct_type) {
                return Err(Diagnostic::new(
                    "E0820",
                    format!("runtime-owned task type `{type_name}` does not expose its fields"),
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            }
            if is_sqlite_runtime_opaque_struct(struct_type) {
                return Err(Diagnostic::new(
                    "E0830",
                    format!("runtime-owned SQLite type `{type_name}` does not expose its fields"),
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            }
            if is_jsonrpc_opaque_struct(struct_type) {
                return Err(Diagnostic::new(
                    "E0840",
                    format!("opaque JSON-RPC type `{type_name}` does not expose its fields"),
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            }
            if is_cron_opaque_struct(struct_type) {
                return Err(Diagnostic::new(
                    "E0850",
                    format!("opaque cron type `{type_name}` does not expose its fields"),
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            }
            let Some(field_type) = struct_type
                .fields
                .iter()
                .find(|item| item.name == *field)
                .map(|item| {
                    substitute_type_params(&item.value_type, &struct_type.type_params, struct_args)
                })
            else {
                return Err(Diagnostic::new(
                    "E0308",
                    format!("struct `{type_name}` has no field `{field}`"),
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let value = match &binding.source {
                BindingSource::EnumPayload { value, variant } => {
                    ValueExpr::EnumPayloadFieldAccess {
                        value: Box::new(value.clone()),
                        variant: variant.clone(),
                        field: field.clone(),
                    }
                }
                BindingSource::Local | BindingSource::Param => ValueExpr::FieldAccess {
                    base: base.clone(),
                    field: field.clone(),
                },
                BindingSource::FunctionEffect { .. } | BindingSource::TaskScope => {
                    unreachable!("internal scope bindings have no fields")
                }
            };
            Ok((field_type, value))
        }
        AstExpr::Match { value, arms } => lower_match_value_expr(
            path, value, arms, scope, imports, signatures, structs, enums, expected, span,
        ),
        AstExpr::If {
            condition,
            then_branch,
            else_branch,
        } => lower_if_value_expr(
            path,
            condition,
            then_branch,
            else_branch,
            scope,
            imports,
            signatures,
            structs,
            enums,
            expected,
            span,
        ),
        AstExpr::Panic { message } => {
            let message = lower_panic_message(
                path, message, scope, imports, signatures, structs, enums, span,
            )?;
            let fallback_type = expected.cloned().unwrap_or(ValueType::Never);
            Ok((
                fallback_type.clone(),
                ValueExpr::Panic {
                    message: Box::new(message),
                    fallback_type,
                },
            ))
        }
        AstExpr::Question { .. } => Err(Diagnostic::new(
            "E0422",
            "`?` is currently supported only in statement-level expressions with unconditional evaluation",
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        )),
        AstExpr::Cast { expr, target } => lower_cast_value_expr(
            path, expr, target, scope, imports, signatures, structs, enums, span,
        ),
        AstExpr::Unary { op, expr } => lower_unary_value_expr(
            path, op, expr, scope, imports, signatures, structs, enums, expected, span,
        ),
        AstExpr::Binary { left, op, right } => lower_binary_value_expr(
            path, left, op, right, scope, imports, signatures, structs, enums, span,
        ),
        AstExpr::Call {
            callee,
            args,
            type_args,
        } => lower_call_value_expr(
            path, callee, args, type_args, scope, imports, signatures, structs, enums, expected,
            span,
        ),
        AstExpr::StructLiteral { type_name, fields } if type_name.len() == 1 => {
            lower_struct_literal_value_expr(
                path, type_name, fields, scope, imports, signatures, structs, enums, expected, span,
            )
        }
        _ => Err(Diagnostic::new(
            "E0405",
            "expression is not supported as a value in v0.1 current implementation",
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        )),
    }
}
