use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn lower_question_if_return(
    path: &Path,
    condition: &AstExpr,
    then_branch: &AstExpr,
    else_branch: &AstExpr,
    scope: &mut HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    return_type: &ValueType,
    span: &Span,
    out: &mut Vec<Statement>,
) -> Result<(), Diagnostic> {
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
    let body = lower_tail_expr_as_return_block(
        path,
        then_branch,
        &mut scope.clone(),
        imports,
        signatures,
        structs,
        enums,
        return_type,
        span,
    )?;
    let else_body = lower_tail_expr_as_return_block(
        path,
        else_branch,
        &mut scope.clone(),
        imports,
        signatures,
        structs,
        enums,
        return_type,
        span,
    )?;
    out.push(Statement::If {
        condition,
        body,
        else_body,
    });
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(super) fn lower_question_result_ok_if_return(
    path: &Path,
    callee: &[String],
    condition: &AstExpr,
    then_branch: &AstExpr,
    else_branch: &AstExpr,
    scope: &mut HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    return_type: &ValueType,
    span: &Span,
    out: &mut Vec<Statement>,
) -> Result<(), Diagnostic> {
    let then_ok = AstExpr::Call {
        callee: callee.to_vec(),
        type_args: Vec::new(),
        args: vec![then_branch.clone()],
    };
    let else_ok = AstExpr::Call {
        callee: callee.to_vec(),
        type_args: Vec::new(),
        args: vec![else_branch.clone()],
    };
    lower_question_if_return(
        path,
        condition,
        &then_ok,
        &else_ok,
        scope,
        imports,
        signatures,
        structs,
        enums,
        return_type,
        span,
        out,
    )
}
