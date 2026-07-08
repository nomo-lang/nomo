use super::*;

pub(super) fn lower_match_value_expr(
    path: &Path,
    value: &AstExpr,
    arms: &[AstMatchArm],
    scope: &HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    expected: Option<&ValueType>,
    span: &Span,
) -> Result<(ValueType, ValueExpr), Diagnostic> {
    let (value_type, lowered_value) = lower_value_expr(
        path, value, scope, imports, signatures, structs, enums, span,
    )?;
    let ValueType::Enum(enum_name, enum_args) = value_type else {
        return Err(type_mismatch(path, span, "`match` expects an enum value"));
    };
    let enum_type = enums
        .get(&enum_name)
        .expect("enum value must refer to a known enum");
    let mut seen = HashMap::new();
    let mut lowered_arms: Vec<MatchValueArm> = Vec::new();
    let mut result_type: Option<ValueType> = expected.cloned();
    for arm in arms {
        let Some(variant) = resolve_match_arm_variant(&arm.pattern, &enum_name, scope) else {
            return Err(Diagnostic::new(
                "E0316",
                format!("match arm must use `{enum_name}.Variant` or a supported prelude variant"),
                path,
                span.line,
                span.column,
                span.length,
                &span.text,
            ));
        };
        let Some(variant_type) = enum_type.variants.iter().find(|item| item.name == *variant)
        else {
            return Err(Diagnostic::new(
                "E0315",
                format!("enum `{enum_name}` has no variant `{variant}`"),
                path,
                span.line,
                span.column,
                span.length,
                &span.text,
            ));
        };
        let mut arm_scope = scope.clone();
        let payload_type = variant_type
            .payload
            .as_ref()
            .map(|payload| substitute_type_params(payload, &enum_type.type_params, &enum_args));
        match (&payload_type, &arm.binding) {
            (Some(payload_type), Some(binding)) => {
                if scope.contains_key(binding) {
                    return Err(Diagnostic::new(
                        "E0302",
                        format!("variable `{binding}` is already defined in this scope"),
                        path,
                        span.line,
                        span.column,
                        span.length,
                        &span.text,
                    ));
                }
                arm_scope.insert(
                    binding.clone(),
                    Binding {
                        value_type: payload_type.clone(),
                        mutable: false,
                        source: BindingSource::EnumPayload {
                            value: lowered_value.clone(),
                            variant: variant.clone(),
                        },
                    },
                );
            }
            (Some(_), None) => {
                return Err(Diagnostic::new(
                    "E0321",
                    format!("match arm `{enum_name}.{variant}` must bind its payload"),
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            }
            (None, Some(_)) => {
                return Err(Diagnostic::new(
                    "E0322",
                    format!("match arm `{enum_name}.{variant}` has no payload to bind"),
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            }
            (None, None) => {}
        }
        if seen.insert(variant.clone(), ()).is_some() {
            return Err(Diagnostic::new(
                "E0317",
                format!("duplicate match arm for `{enum_name}.{variant}`"),
                path,
                span.line,
                span.column,
                span.length,
                &span.text,
            ));
        }
        let (arm_type, mut arm_value) = lower_value_expr_with_expected(
            path,
            &arm.value,
            &arm_scope,
            imports,
            signatures,
            structs,
            enums,
            result_type.as_ref(),
            span,
        )?;
        if let Some(expected_type) = &result_type {
            if arm_type == ValueType::Never {
                arm_value = coerce_never_expr(arm_value, expected_type);
            } else if expected_type != &arm_type {
                return Err(type_mismatch(
                    path,
                    span,
                    format!(
                        "match arm returns `{}` but previous arms return `{}`",
                        arm_type.name(),
                        expected_type.name()
                    ),
                ));
            }
        } else if arm_type == ValueType::Never {
            // A diverging arm does not determine the match expression type.
        } else {
            result_type = Some(arm_type.clone());
            for previous in &mut lowered_arms {
                previous.value = coerce_never_expr(previous.value.clone(), &arm_type);
            }
        }
        lowered_arms.push(MatchValueArm {
            enum_name: enum_name.clone(),
            enum_args: enum_args.clone(),
            variant,
            binding: arm.binding.clone(),
            value: arm_value,
        });
    }
    for variant in &enum_type.variants {
        if !seen.contains_key(&variant.name) {
            return Err(Diagnostic::new(
                "E0318",
                format!("match is missing arm `{enum_name}.{}`", variant.name),
                path,
                span.line,
                span.column,
                span.length,
                &span.text,
            ));
        }
    }
    let Some(result_type) = result_type else {
        return Err(Diagnostic::new(
            "E0319",
            "`match` must contain at least one non-diverging arm",
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    };
    Ok((
        result_type,
        ValueExpr::Match {
            value: Box::new(lowered_value),
            arms: lowered_arms,
        },
    ))
}
