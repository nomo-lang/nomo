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

#[allow(clippy::too_many_arguments)]
pub(super) fn lower_question_result_ok_match_return(
    path: &Path,
    callee: &[String],
    value: &AstExpr,
    arms: &[AstMatchArm],
    scope: &mut HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    return_type: &ValueType,
    span: &Span,
    out: &mut Vec<Statement>,
) -> Result<(), Diagnostic> {
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
            let ok_arm = AstExpr::Call {
                callee: callee.to_vec(),
                type_args: Vec::new(),
                args: vec![arm.value.clone()],
            };
            lower_tail_expr_as_return_block(
                path,
                &ok_arm,
                arm_scope,
                imports,
                signatures,
                structs,
                enums,
                return_type,
                span,
            )
        },
    )?;
    out.push(Statement::Match {
        value: lowered_value,
        enum_name,
        enum_args,
        arms: lowered_arms,
    });
    Ok(())
}
