use super::*;

pub(super) fn lower_return_stmt(
    path: &Path,
    value: Option<&AstExpr>,
    scope: &HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    return_type: &ValueType,
    span: &Span,
) -> Result<Statement, Diagnostic> {
    match (return_type, value) {
        (ValueType::Void, None) => Ok(Statement::Return(None)),
        (ValueType::Void, Some(_)) => Err(type_mismatch(
            path,
            span,
            "`void` function cannot return a value",
        )),
        (_, None) => Err(type_mismatch(
            path,
            span,
            format!("function must return `{}`", return_type.name()),
        )),
        (expected, Some(value)) => {
            if let AstExpr::Question {
                expr: question_expr,
            } = value
            {
                let (result_type, result_expr) = lower_value_expr(
                    path,
                    question_expr,
                    scope,
                    imports,
                    signatures,
                    structs,
                    enums,
                    span,
                )?;
                let (carrier, ok_type) = question_payload(path, span, &result_type, expected)?;
                let return_payload_type = question_return_payload(expected, carrier);
                if ok_type != return_payload_type {
                    return Err(type_mismatch_expected_found(
                        path,
                        span,
                        format!(
                            "`?` unwraps `{}` but function returns `{}`",
                            ok_type.name(),
                            expected.name()
                        ),
                        &return_payload_type,
                        &ok_type,
                    ));
                }
                return Ok(Statement::QuestionReturn {
                    carrier,
                    ok_type,
                    result_type,
                    return_type: expected.clone(),
                    result_expr,
                });
            }
            if let Some((carrier, question_expr)) =
                question_expr_from_success_return(value, signatures)
            {
                let (result_type, result_expr) = lower_value_expr(
                    path,
                    question_expr,
                    scope,
                    imports,
                    signatures,
                    structs,
                    enums,
                    span,
                )?;
                let (actual_carrier, ok_type) =
                    question_payload(path, span, &result_type, expected)?;
                if actual_carrier != carrier {
                    return Err(Diagnostic::new(
                        "E0421",
                        "`?` carrier does not match the returned success variant",
                        path,
                        span.line,
                        span.column,
                        span.length,
                        &span.text,
                    ));
                }
                let return_payload_type = question_return_payload(expected, carrier);
                if ok_type != return_payload_type {
                    return Err(type_mismatch_expected_found(
                        path,
                        span,
                        format!(
                            "`?` unwraps `{}` but returned success variant expects `{}`",
                            ok_type.name(),
                            return_payload_type.name()
                        ),
                        &return_payload_type,
                        &ok_type,
                    ));
                }
                return Ok(Statement::QuestionReturn {
                    carrier,
                    ok_type,
                    result_type,
                    return_type: expected.clone(),
                    result_expr,
                });
            }
            let (actual, lowered) = lower_value_expr_with_expected(
                path,
                value,
                scope,
                imports,
                signatures,
                structs,
                enums,
                Some(expected),
                span,
            )?;
            if &actual != expected {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    format!(
                        "return value is `{}` but function expects `{}`",
                        actual.name(),
                        expected.name()
                    ),
                    expected,
                    &actual,
                ));
            }
            Ok(Statement::Return(Some(lowered)))
        }
    }
}

pub(super) fn question_expr_from_success_return<'a>(
    value: &'a AstExpr,
    signatures: &HashMap<String, FunctionSignature>,
) -> Option<(QuestionCarrier, &'a AstExpr)> {
    let AstExpr::Call { callee, args, .. } = value else {
        return None;
    };
    let [AstExpr::Question { expr }] = args.as_slice() else {
        return None;
    };
    if is_result_ok_callee(callee, signatures) {
        return Some((QuestionCarrier::Result, expr));
    }
    if is_option_some_callee(callee, signatures) {
        return Some((QuestionCarrier::Option, expr));
    }
    None
}

pub(super) fn is_result_ok_callee(
    callee: &[String],
    signatures: &HashMap<String, FunctionSignature>,
) -> bool {
    match callee {
        [name] => name == "Ok" && !signatures.contains_key("Ok"),
        [enum_name, variant] => enum_name == "Result" && variant == "Ok",
        _ => false,
    }
}

pub(super) fn is_option_some_callee(
    callee: &[String],
    signatures: &HashMap<String, FunctionSignature>,
) -> bool {
    match callee {
        [name] => name == "Some" && !signatures.contains_key("Some"),
        [enum_name, variant] => enum_name == "Option" && variant == "Some",
        _ => false,
    }
}
