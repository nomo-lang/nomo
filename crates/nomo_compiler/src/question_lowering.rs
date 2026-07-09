use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn lower_question_exprs_in_stmt_into(
    path: &Path,
    stmt: &Stmt,
    scope: &mut HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    return_type: &ValueType,
    is_tail: bool,
    loop_depth: usize,
    out: &mut Vec<Statement>,
) -> Result<bool, Diagnostic> {
    let rewritten = match stmt {
        Stmt::Let {
            name,
            mutable,
            type_annotation,
            value,
            span,
        } if matches!(value, AstExpr::If { .. }) && ast_expr_contains_question(value) => {
            lower_question_if_let_initializer(
                path,
                name,
                *mutable,
                type_annotation.as_ref(),
                value,
                span,
                scope,
                imports,
                signatures,
                structs,
                enums,
                return_type,
                out,
            )?;
            return Ok(true);
        }
        Stmt::Let {
            name,
            mutable,
            type_annotation,
            value,
            span,
        } if matches!(value, AstExpr::Match { .. }) && ast_expr_contains_question(value) => {
            lower_question_match_let_initializer(
                path,
                name,
                *mutable,
                type_annotation.as_ref(),
                value,
                span,
                scope,
                imports,
                signatures,
                structs,
                enums,
                return_type,
                out,
            )?;
            return Ok(true);
        }
        Stmt::Let {
            name,
            mutable,
            type_annotation,
            value,
            span,
        } if !matches!(value, AstExpr::Question { .. }) => {
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
            if !changed {
                return Ok(false);
            }
            Stmt::Let {
                name: name.clone(),
                mutable: *mutable,
                type_annotation: type_annotation.clone(),
                value,
                span: span.clone(),
            }
        }
        Stmt::LetElse {
            pattern,
            binding,
            value,
            else_body,
            span,
        } if ast_expr_contains_question(value) => {
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
            if !changed {
                return Ok(false);
            }
            Stmt::LetElse {
                pattern: pattern.clone(),
                binding: binding.clone(),
                value,
                else_body: else_body.clone(),
                span: span.clone(),
            }
        }
        Stmt::IfLet {
            pattern,
            binding,
            value,
            body,
            else_body,
            span,
        } if ast_expr_contains_question(value) => {
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
            if !changed {
                return Ok(false);
            }
            Stmt::IfLet {
                pattern: pattern.clone(),
                binding: binding.clone(),
                value,
                body: body.clone(),
                else_body: else_body.clone(),
                span: span.clone(),
            }
        }
        Stmt::Match { value, arms, span } if ast_expr_contains_question(value) => {
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
            if !changed {
                return Ok(false);
            }
            Stmt::Match {
                value,
                arms: arms.clone(),
                span: span.clone(),
            }
        }
        Stmt::For {
            variant:
                ForVariant::Iterate {
                    binding,
                    iterable,
                    body,
                },
            span,
        } if ast_expr_contains_question(iterable) => {
            let (iterable, changed) = extract_question_exprs(
                path,
                iterable,
                scope,
                imports,
                signatures,
                structs,
                enums,
                return_type,
                span,
                out,
            )?;
            if !changed {
                return Ok(false);
            }
            Stmt::For {
                variant: ForVariant::Iterate {
                    binding: binding.clone(),
                    iterable,
                    body: body.clone(),
                },
                span: span.clone(),
            }
        }
        Stmt::Assign {
            target,
            op,
            value,
            span,
        } if matches!(value, AstExpr::If { .. }) && ast_expr_contains_question(value) => {
            let AstExpr::If {
                condition,
                then_branch,
                else_branch,
            } = value
            else {
                unreachable!("guard matched if expression");
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
            let body = lower_expr_as_target_assignment_block(
                path,
                target,
                *op,
                then_branch,
                &mut scope.clone(),
                imports,
                signatures,
                structs,
                enums,
                return_type,
                span,
            )?;
            let else_body = lower_expr_as_target_assignment_block(
                path,
                target,
                *op,
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
            return Ok(true);
        }
        Stmt::Assign {
            target,
            op,
            value: AstExpr::Match { value, arms },
            span,
        } if ast_expr_contains_question(value)
            || arms
                .iter()
                .any(|arm| ast_expr_contains_question(&arm.value)) =>
        {
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
                    lower_expr_as_target_assignment_block(
                        path,
                        target,
                        *op,
                        &arm.value,
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
            return Ok(true);
        }
        Stmt::Assign {
            target,
            op,
            value,
            span,
        } if ast_expr_contains_question(value) => {
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
            if !changed {
                return Ok(false);
            }
            Stmt::Assign {
                target: target.clone(),
                op: *op,
                value,
                span: span.clone(),
            }
        }
        Stmt::Defer { stmt, span } if matches!(stmt.as_ref(), Stmt::Expr { .. }) => {
            let Stmt::Expr {
                expr,
                span: expr_span,
            } = stmt.as_ref()
            else {
                unreachable!("guard matched expression defer");
            };
            if !ast_expr_contains_question(expr) {
                return Ok(false);
            }
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
            if !changed {
                return Ok(false);
            }
            Stmt::Defer {
                stmt: Box::new(Stmt::Expr {
                    expr,
                    span: expr_span.clone(),
                }),
                span: span.clone(),
            }
        }
        Stmt::Expr { expr, span }
            if !(is_tail && return_type != &ValueType::Void)
                && ast_expr_contains_question(expr) =>
        {
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
            if !changed {
                return Ok(false);
            }
            Stmt::Expr {
                expr,
                span: span.clone(),
            }
        }
        Stmt::Return {
            value:
                Some(AstExpr::If {
                    condition,
                    then_branch,
                    else_branch,
                }),
            span,
        } if ast_expr_contains_question(condition)
            || ast_expr_contains_question(then_branch)
            || ast_expr_contains_question(else_branch) =>
        {
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
            return Ok(true);
        }
        Stmt::Return {
            value:
                Some(AstExpr::Call {
                    callee,
                    type_args,
                    args,
                }),
            span,
        } if type_args.is_empty()
            && is_result_ok_callee(callee, signatures)
            && matches!(args.as_slice(), [AstExpr::If { .. }])
            && args.iter().any(ast_expr_contains_question) =>
        {
            let [
                AstExpr::If {
                    condition,
                    then_branch,
                    else_branch,
                },
            ] = args.as_slice()
            else {
                unreachable!("guard matched single if argument");
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
            let then_ok = AstExpr::Call {
                callee: callee.clone(),
                type_args: Vec::new(),
                args: vec![then_branch.as_ref().clone()],
            };
            let else_ok = AstExpr::Call {
                callee: callee.clone(),
                type_args: Vec::new(),
                args: vec![else_branch.as_ref().clone()],
            };
            let body = lower_tail_expr_as_return_block(
                path,
                &then_ok,
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
                &else_ok,
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
            return Ok(true);
        }
        Stmt::Return {
            value:
                Some(AstExpr::Call {
                    callee,
                    type_args,
                    args,
                }),
            span,
        } if type_args.is_empty()
            && is_result_ok_callee(callee, signatures)
            && matches!(args.as_slice(), [AstExpr::Match { .. }])
            && args.iter().any(ast_expr_contains_question) =>
        {
            let [AstExpr::Match { value, arms }] = args.as_slice() else {
                unreachable!("guard matched single match argument");
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
                        callee: callee.clone(),
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
            return Ok(true);
        }
        Stmt::Return {
            value: Some(value),
            span,
        } if matches!(value, AstExpr::Match { .. }) && ast_expr_contains_question(value) => {
            let AstExpr::Match { value, arms } = value else {
                unreachable!("guard matched match expression");
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
            out.push(lower_tail_match_expr_as_statement(
                path,
                &value,
                arms,
                scope,
                imports,
                signatures,
                structs,
                enums,
                return_type,
                span,
            )?);
            return Ok(true);
        }
        Stmt::Return {
            value: Some(value),
            span,
        } if !matches!(value, AstExpr::Question { .. })
            && question_expr_from_success_return(value, signatures).is_none() =>
        {
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
            if !changed {
                return Ok(false);
            }
            Stmt::Return {
                value: Some(value),
                span: span.clone(),
            }
        }
        Stmt::Expr {
            expr:
                AstExpr::If {
                    condition,
                    then_branch,
                    else_branch,
                },
            span,
        } if is_tail
            && return_type != &ValueType::Void
            && (ast_expr_contains_question(condition)
                || ast_expr_contains_question(then_branch)
                || ast_expr_contains_question(else_branch)) =>
        {
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
            return Ok(true);
        }
        Stmt::Expr {
            expr: AstExpr::Match { value, arms },
            span,
        } if is_tail
            && return_type != &ValueType::Void
            && arms
                .iter()
                .any(|arm| ast_expr_contains_question(&arm.value)) =>
        {
            out.push(lower_tail_match_expr_as_statement(
                path,
                value,
                arms,
                scope,
                imports,
                signatures,
                structs,
                enums,
                return_type,
                span,
            )?);
            return Ok(true);
        }
        Stmt::Expr { expr, span } if is_tail && return_type != &ValueType::Void => {
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
            if !changed {
                return Ok(false);
            }
            Stmt::Expr {
                expr,
                span: span.clone(),
            }
        }
        _ => return Ok(false),
    };
    out.push(lower_stmt(
        path,
        &rewritten,
        scope,
        imports,
        signatures,
        structs,
        enums,
        return_type,
        false,
        loop_depth,
    )?);
    Ok(true)
}
