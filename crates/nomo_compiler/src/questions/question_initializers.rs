use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn lower_question_if_let_initializer(
    path: &Path,
    name: &str,
    mutable: bool,
    type_annotation: Option<&AstTypeRef>,
    value: &AstExpr,
    span: &Span,
    scope: &mut HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    return_type: &ValueType,
    out: &mut Vec<Statement>,
) -> Result<(), Diagnostic> {
    if scope.contains_key(name) {
        return Err(Diagnostic::new(
            "E0302",
            format!("variable `{name}` is already defined in this scope"),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    }
    let AstExpr::If {
        condition,
        then_branch,
        else_branch,
    } = value
    else {
        unreachable!("guard matched if expression");
    };
    let annotated_type = if let Some(annotation) = type_annotation {
        let annotated_type = parse_non_void_type(annotation, structs, enums).ok_or_else(|| {
            unsupported_type_diagnostic_from_maps(
                path,
                span,
                annotation,
                format!(
                    "unsupported variable type `{}` in v0.1 current implementation",
                    annotation.path.join(".")
                ),
                structs,
                enums,
            )
        })?;
        ensure_supported_value_type(path, &annotated_type, span)?;
        Some(annotated_type)
    } else {
        None
    };
    let (condition, _) = extract_question_exprs(
        path,
        condition,
        scope,
        imports,
        signatures,
        structs,
        enums,
        return_type,
        span,
        out,
    )?;
    let (condition_type, condition) = lower_value_expr(
        path, &condition, scope, imports, signatures, structs, enums, span,
    )?;
    if condition_type != ValueType::Bool {
        return Err(type_mismatch(path, span, "`if` condition must be `bool`"));
    }
    let (then_type, body) = lower_expr_as_assignment_block(
        path,
        name,
        then_branch,
        &mut scope.clone(),
        imports,
        signatures,
        structs,
        enums,
        return_type,
        annotated_type.as_ref(),
        span,
    )?;
    let else_expected = annotated_type
        .as_ref()
        .or(if then_type == ValueType::Never {
            None
        } else {
            Some(&then_type)
        });
    let (else_type, else_body) = lower_expr_as_assignment_block(
        path,
        name,
        else_branch,
        &mut scope.clone(),
        imports,
        signatures,
        structs,
        enums,
        return_type,
        else_expected,
        span,
    )?;
    let value_type = if let Some(annotated_type) = annotated_type {
        if then_type != ValueType::Never && then_type != annotated_type {
            return Err(type_mismatch_expected_found(
                path,
                span,
                format!(
                    "`if` branch returns `{}` but `{name}` is annotated as `{}`",
                    then_type.name(),
                    annotated_type.name()
                ),
                &annotated_type,
                &then_type,
            ));
        }
        if else_type != ValueType::Never && else_type != annotated_type {
            return Err(type_mismatch_expected_found(
                path,
                span,
                format!(
                    "`if` branch returns `{}` but `{name}` is annotated as `{}`",
                    else_type.name(),
                    annotated_type.name()
                ),
                &annotated_type,
                &else_type,
            ));
        }
        annotated_type
    } else if then_type == ValueType::Never && else_type == ValueType::Never {
        return Err(Diagnostic::new(
            "E0403",
            "`if` initializer must contain at least one value-producing branch",
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    } else if then_type == ValueType::Never {
        else_type
    } else if else_type == ValueType::Never || then_type == else_type {
        then_type
    } else {
        return Err(type_mismatch(
            path,
            span,
            format!(
                "`if` branches return `{}` and `{}`",
                then_type.name(),
                else_type.name()
            ),
        ));
    };
    scope.insert(
        name.to_string(),
        Binding {
            value_type: value_type.clone(),
            mutable,
            source: BindingSource::Local,
        },
    );
    out.push(Statement::LetIf {
        name: name.to_string(),
        value_type,
        condition,
        body,
        else_body,
    });
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(super) fn lower_question_match_let_initializer(
    path: &Path,
    name: &str,
    mutable: bool,
    type_annotation: Option<&AstTypeRef>,
    value: &AstExpr,
    span: &Span,
    scope: &mut HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    return_type: &ValueType,
    out: &mut Vec<Statement>,
) -> Result<(), Diagnostic> {
    if scope.contains_key(name) {
        return Err(Diagnostic::new(
            "E0302",
            format!("variable `{name}` is already defined in this scope"),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    }
    let AstExpr::Match { value, arms } = value else {
        unreachable!("guard matched match expression");
    };
    let annotated_type = if let Some(annotation) = type_annotation {
        let annotated_type = parse_non_void_type(annotation, structs, enums).ok_or_else(|| {
            unsupported_type_diagnostic_from_maps(
                path,
                span,
                annotation,
                format!(
                    "unsupported variable type `{}` in v0.1 current implementation",
                    annotation.path.join(".")
                ),
                structs,
                enums,
            )
        })?;
        ensure_supported_value_type(path, &annotated_type, span)?;
        Some(annotated_type)
    } else {
        None
    };
    let (value, _) = extract_question_exprs(
        path,
        value,
        scope,
        imports,
        signatures,
        structs,
        enums,
        return_type,
        span,
        out,
    )?;
    let (value_type, lowered_value) = lower_value_expr(
        path, &value, scope, imports, signatures, structs, enums, span,
    )?;
    let ValueType::Enum(enum_name, enum_args) = value_type else {
        return Err(type_mismatch(path, span, "`match` expects an enum value"));
    };
    let enum_type = enums
        .get(&enum_name)
        .expect("enum value must refer to a known enum");
    let mut result_type = annotated_type.clone();
    let lowered_arms = lower_question_match_arms(
        path,
        arms,
        &enum_name,
        &enum_args,
        enum_type,
        &lowered_value,
        scope,
        span,
        |arm, arm_scope| {
            let (arm_type, body) = lower_expr_as_assignment_block(
                path,
                name,
                &arm.value,
                arm_scope,
                imports,
                signatures,
                structs,
                enums,
                return_type,
                result_type.as_ref(),
                span,
            )?;
            if let Some(expected_type) = &result_type {
                if arm_type != ValueType::Never && expected_type != &arm_type {
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
                // A diverging arm does not determine the match initializer type.
            } else {
                result_type = Some(arm_type);
            }
            Ok(body)
        },
    )?;
    let Some(value_type) = result_type else {
        return Err(Diagnostic::new(
            "E0319",
            "`match` initializer must contain at least one value-producing arm",
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    };
    scope.insert(
        name.to_string(),
        Binding {
            value_type: value_type.clone(),
            mutable,
            source: BindingSource::Local,
        },
    );
    out.push(Statement::LetMatch {
        name: name.to_string(),
        value_type,
        value: lowered_value,
        enum_name,
        enum_args,
        arms: lowered_arms,
    });
    Ok(())
}
