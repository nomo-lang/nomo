use super::*;

pub(super) fn lower_if_value_expr(
    path: &Path,
    condition: &AstExpr,
    then_branch: &AstExpr,
    else_branch: &AstExpr,
    scope: &HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    expected: Option<&ValueType>,
    span: &Span,
) -> Result<(ValueType, ValueExpr), Diagnostic> {
    let (condition_type, lowered_condition) = lower_value_expr(
        path, condition, scope, imports, signatures, structs, enums, span,
    )?;
    if condition_type != ValueType::Bool {
        return Err(type_mismatch(path, span, "`if` condition must be `bool`"));
    }

    let (then_type, mut lowered_then) = lower_value_expr_with_expected(
        path,
        then_branch,
        scope,
        imports,
        signatures,
        structs,
        enums,
        expected,
        span,
    )?;
    let else_expected = if then_type == ValueType::Never {
        expected
    } else {
        Some(&then_type)
    };
    let (else_type, mut lowered_else) = lower_value_expr_with_expected(
        path,
        else_branch,
        scope,
        imports,
        signatures,
        structs,
        enums,
        else_expected,
        span,
    )?;
    let result_type = if then_type == ValueType::Never && else_type == ValueType::Never {
        ValueType::Never
    } else if then_type == ValueType::Never {
        lowered_then = coerce_never_expr(lowered_then, &else_type);
        else_type
    } else if else_type == ValueType::Never {
        lowered_else = coerce_never_expr(lowered_else, &then_type);
        then_type
    } else if else_type == then_type {
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

    Ok((
        result_type,
        ValueExpr::If {
            condition: Box::new(lowered_condition),
            then_branch: Box::new(lowered_then),
            else_branch: Box::new(lowered_else),
        },
    ))
}
