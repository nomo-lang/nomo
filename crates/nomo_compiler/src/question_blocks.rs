use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn lower_expr_as_assignment_block(
    path: &Path,
    target: &str,
    expr: &AstExpr,
    scope: &mut HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    return_type: &ValueType,
    expected: Option<&ValueType>,
    span: &Span,
) -> Result<(ValueType, Vec<Statement>), Diagnostic> {
    let mut out = Vec::new();
    let (expr, _) = extract_question_exprs(
        path,
        expr,
        scope,
        imports,
        signatures,
        structs,
        enums,
        return_type,
        span,
        &mut out,
    )?;
    let (value_type, value) = lower_value_expr_with_expected(
        path, &expr, scope, imports, signatures, structs, enums, expected, span,
    )?;
    if value_type != ValueType::Never {
        out.push(Statement::Assign {
            name: target.to_string(),
            value,
        });
    }
    Ok((value_type, out))
}

#[allow(clippy::too_many_arguments)]
pub(super) fn lower_expr_as_target_assignment_block(
    path: &Path,
    target: &[String],
    op: AssignOp,
    expr: &AstExpr,
    scope: &mut HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    return_type: &ValueType,
    span: &Span,
) -> Result<Vec<Statement>, Diagnostic> {
    let mut out = Vec::new();
    let (expr, _) = extract_question_exprs(
        path,
        expr,
        scope,
        imports,
        signatures,
        structs,
        enums,
        return_type,
        span,
        &mut out,
    )?;
    out.push(lower_assign_stmt(
        path, target, op, &expr, scope, imports, signatures, structs, enums, span,
    )?);
    Ok(out)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn lower_tail_expr_as_return_block(
    path: &Path,
    expr: &AstExpr,
    scope: &mut HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    return_type: &ValueType,
    span: &Span,
) -> Result<Vec<Statement>, Diagnostic> {
    let mut out = Vec::new();
    let stmt = Stmt::Return {
        value: Some(expr.clone()),
        span: span.clone(),
    };
    lower_stmt_into(
        path,
        &stmt,
        scope,
        imports,
        signatures,
        structs,
        enums,
        return_type,
        false,
        0,
        &mut out,
    )?;
    Ok(out)
}
