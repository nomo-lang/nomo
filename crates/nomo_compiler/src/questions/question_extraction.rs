use super::*;

pub(super) fn ast_expr_contains_question(expr: &AstExpr) -> bool {
    match expr {
        AstExpr::Question { .. } => true,
        AstExpr::Call { args, .. } => args.iter().any(ast_expr_contains_question),
        AstExpr::StructLiteral { fields, .. } => fields
            .iter()
            .any(|(_, value)| ast_expr_contains_question(value)),
        AstExpr::Match { value, arms } => {
            ast_expr_contains_question(value)
                || arms
                    .iter()
                    .any(|arm| ast_expr_contains_question(&arm.value))
        }
        AstExpr::If {
            condition,
            then_branch,
            else_branch,
        } => {
            ast_expr_contains_question(condition)
                || ast_expr_contains_question(then_branch)
                || ast_expr_contains_question(else_branch)
        }
        AstExpr::Panic { message } | AstExpr::Unary { expr: message, .. } => {
            ast_expr_contains_question(message)
        }
        AstExpr::Cast { expr, .. } => ast_expr_contains_question(expr),
        AstExpr::Binary { left, right, .. } => {
            ast_expr_contains_question(left) || ast_expr_contains_question(right)
        }
        AstExpr::MutArg { .. }
        | AstExpr::Name(_)
        | AstExpr::String(_)
        | AstExpr::Int(_)
        | AstExpr::Float(_)
        | AstExpr::Char(_)
        | AstExpr::Bool(_)
        | AstExpr::Void => false,
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn extract_question_exprs(
    path: &Path,
    expr: &AstExpr,
    scope: &mut HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    return_type: &ValueType,
    span: &Span,
    out: &mut Vec<Statement>,
) -> Result<(AstExpr, bool), Diagnostic> {
    match expr {
        AstExpr::Question { expr } => {
            let (rewritten_result, _) = extract_question_exprs(
                path,
                expr,
                scope,
                imports,
                signatures,
                structs,
                enums,
                return_type,
                span,
                out,
            )?;
            let (result_type, result_expr) = lower_value_expr(
                path,
                &rewritten_result,
                scope,
                imports,
                signatures,
                structs,
                enums,
                span,
            )?;
            let (carrier, ok_type) = question_payload(path, span, &result_type, return_type)?;
            let temp = fresh_internal_binding(scope, "question_value");
            scope.insert(
                temp.clone(),
                Binding {
                    value_type: ok_type.clone(),
                    mutable: false,
                    source: BindingSource::Local,
                },
            );
            out.push(Statement::QuestionLet {
                carrier,
                name: temp.clone(),
                value_type: ok_type,
                result_type,
                return_type: return_type.clone(),
                result_expr,
            });
            Ok((AstExpr::Name(vec![temp]), true))
        }
        AstExpr::Call {
            callee,
            type_args,
            args,
        } => {
            let (args, changed) = extract_question_exprs_from_vec(
                path,
                args,
                scope,
                imports,
                signatures,
                structs,
                enums,
                return_type,
                span,
                out,
            )?;
            Ok((
                AstExpr::Call {
                    callee: callee.clone(),
                    type_args: type_args.clone(),
                    args,
                },
                changed,
            ))
        }
        AstExpr::StructLiteral { type_name, fields } => {
            let mut changed = false;
            let mut rewritten = Vec::new();
            for (field, value) in fields {
                let (value, value_changed) = extract_question_exprs(
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
                changed |= value_changed;
                rewritten.push((field.clone(), value));
            }
            Ok((
                AstExpr::StructLiteral {
                    type_name: type_name.clone(),
                    fields: rewritten,
                },
                changed,
            ))
        }
        AstExpr::Binary { left, op, right } => {
            let (left, left_changed) = extract_question_exprs(
                path,
                left,
                scope,
                imports,
                signatures,
                structs,
                enums,
                return_type,
                span,
                out,
            )?;
            let (right, right_changed) = extract_question_exprs(
                path,
                right,
                scope,
                imports,
                signatures,
                structs,
                enums,
                return_type,
                span,
                out,
            )?;
            Ok((
                AstExpr::Binary {
                    left: Box::new(left),
                    op: op.clone(),
                    right: Box::new(right),
                },
                left_changed || right_changed,
            ))
        }
        AstExpr::Cast { expr, target } => {
            let (expr, changed) = extract_question_exprs(
                path,
                expr,
                scope,
                imports,
                signatures,
                structs,
                enums,
                return_type,
                span,
                out,
            )?;
            Ok((
                AstExpr::Cast {
                    expr: Box::new(expr),
                    target: target.clone(),
                },
                changed,
            ))
        }
        AstExpr::Unary { op, expr } => {
            let (expr, changed) = extract_question_exprs(
                path,
                expr,
                scope,
                imports,
                signatures,
                structs,
                enums,
                return_type,
                span,
                out,
            )?;
            Ok((
                AstExpr::Unary {
                    op: op.clone(),
                    expr: Box::new(expr),
                },
                changed,
            ))
        }
        AstExpr::Match { value, arms } => {
            let (value, changed) = extract_question_exprs(
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
            Ok((
                AstExpr::Match {
                    value: Box::new(value),
                    arms: arms.clone(),
                },
                changed,
            ))
        }
        AstExpr::If {
            condition,
            then_branch,
            else_branch,
        } => {
            let (condition, changed) = extract_question_exprs(
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
            Ok((
                AstExpr::If {
                    condition: Box::new(condition),
                    then_branch: then_branch.clone(),
                    else_branch: else_branch.clone(),
                },
                changed,
            ))
        }
        AstExpr::Panic { message } => {
            let (message, changed) = extract_question_exprs(
                path,
                message,
                scope,
                imports,
                signatures,
                structs,
                enums,
                return_type,
                span,
                out,
            )?;
            Ok((
                AstExpr::Panic {
                    message: Box::new(message),
                },
                changed,
            ))
        }
        AstExpr::MutArg { .. }
        | AstExpr::Name(_)
        | AstExpr::String(_)
        | AstExpr::Int(_)
        | AstExpr::Float(_)
        | AstExpr::Char(_)
        | AstExpr::Bool(_)
        | AstExpr::Void => Ok((expr.clone(), false)),
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn extract_question_exprs_from_vec(
    path: &Path,
    exprs: &[AstExpr],
    scope: &mut HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    return_type: &ValueType,
    span: &Span,
    out: &mut Vec<Statement>,
) -> Result<(Vec<AstExpr>, bool), Diagnostic> {
    let mut changed = false;
    let mut rewritten = Vec::new();
    for expr in exprs {
        let (expr, expr_changed) = extract_question_exprs(
            path,
            expr,
            scope,
            imports,
            signatures,
            structs,
            enums,
            return_type,
            span,
            out,
        )?;
        changed |= expr_changed;
        rewritten.push(expr);
    }
    Ok((rewritten, changed))
}

pub(super) fn fresh_internal_binding(scope: &HashMap<String, Binding>, prefix: &str) -> String {
    let mut index = 0;
    loop {
        let candidate = format!("__{prefix}_{index}");
        if !scope.contains_key(&candidate) {
            return candidate;
        }
        index += 1;
    }
}
