use super::*;

pub(super) fn lower_stmt(
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
) -> Result<Statement, Diagnostic> {
    match stmt {
        Stmt::Let {
            name,
            mutable,
            type_annotation,
            value,
            span,
        } => {
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

            if let AstExpr::Question { expr } = value {
                let Some(annotation) = type_annotation.as_ref() else {
                    return Err(Diagnostic::new(
                        "E0403",
                        "`?` let bindings require an explicit non-void type annotation",
                        path,
                        span.line,
                        span.column,
                        span.length,
                        &span.text,
                    ));
                };
                let annotated_type =
                    parse_non_void_type(annotation, structs, enums).ok_or_else(|| {
                        if annotation.path == ["void"] {
                            Diagnostic::new(
                                "E0403",
                                "`?` let bindings require an explicit non-void type annotation",
                                path,
                                span.line,
                                span.column,
                                span.length,
                                &span.text,
                            )
                        } else {
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
                        }
                    })?;
                ensure_supported_value_type(path, &annotated_type, span)?;
                let (result_type, result_expr) =
                    lower_value_expr(path, expr, scope, imports, signatures, structs, enums, span)?;
                let (carrier, ok_type) = question_payload(path, span, &result_type, return_type)?;
                if ok_type != annotated_type {
                    return Err(type_mismatch_expected_found(
                        path,
                        span,
                        format!(
                            "`?` unwraps `{}` but binding `{name}` is annotated as `{}`",
                            ok_type.name(),
                            annotated_type.name()
                        ),
                        &annotated_type,
                        &ok_type,
                    ));
                }
                scope.insert(
                    name.clone(),
                    Binding {
                        value_type: annotated_type.clone(),
                        mutable: *mutable,
                        source: BindingSource::Local,
                    },
                );
                return Ok(Statement::QuestionLet {
                    carrier,
                    name: name.clone(),
                    value_type: annotated_type,
                    result_type,
                    return_type: return_type.clone(),
                    result_expr,
                });
            }

            let annotated_type = if let Some(annotation) = type_annotation {
                let annotated_type =
                    parse_non_void_type(annotation, structs, enums).ok_or_else(|| {
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
            let (inferred_type, initializer) = lower_value_expr_with_expected(
                path,
                value,
                scope,
                imports,
                signatures,
                structs,
                enums,
                annotated_type.as_ref(),
                span,
            )?;
            let value_type = if let Some(annotated_type) = annotated_type {
                if annotated_type != inferred_type {
                    return Err(type_mismatch_expected_found(
                        path,
                        span,
                        format!(
                            "cannot initialize `{name}` as `{}` from `{}`",
                            annotated_type.name(),
                            inferred_type.name()
                        ),
                        &annotated_type,
                        &inferred_type,
                    ));
                }
                annotated_type
            } else {
                inferred_type
            };

            scope.insert(
                name.clone(),
                Binding {
                    value_type: value_type.clone(),
                    mutable: *mutable,
                    source: BindingSource::Local,
                },
            );
            Ok(Statement::Let {
                name: name.clone(),
                value_type,
                initializer,
            })
        }
        Stmt::LetElse {
            pattern,
            binding,
            value,
            else_body,
            span,
        } => lower_let_else_stmt(
            path,
            pattern,
            binding,
            value,
            else_body,
            scope,
            imports,
            signatures,
            structs,
            enums,
            return_type,
            loop_depth,
            span,
        ),
        Stmt::IfLet {
            pattern,
            binding,
            value,
            body,
            else_body,
            span,
        } => lower_if_let_stmt(
            path,
            pattern,
            binding.as_deref(),
            value,
            body,
            else_body.as_deref(),
            scope,
            imports,
            signatures,
            structs,
            enums,
            return_type,
            loop_depth,
            span,
        ),
        Stmt::Assign {
            target,
            op,
            value,
            span,
        } => lower_assign_stmt(
            path, target, *op, value, scope, imports, signatures, structs, enums, span,
        ),
        Stmt::Postfix { target, op, span } => lower_postfix_stmt(
            path, target, *op, scope, imports, signatures, structs, enums, span,
        ),
        Stmt::Return { value, span } => lower_return_stmt(
            path,
            value.as_ref(),
            scope,
            imports,
            signatures,
            structs,
            enums,
            return_type,
            span,
        ),
        Stmt::Expr { expr, span } if is_tail && return_type != &ValueType::Void => {
            let (expr_type, lowered) = lower_value_expr_with_expected(
                path,
                expr,
                scope,
                imports,
                signatures,
                structs,
                enums,
                Some(return_type),
                span,
            )?;
            if &expr_type != return_type {
                return Err(type_mismatch(
                    path,
                    span,
                    format!(
                        "tail expression returns `{}` but function expects `{}`",
                        expr_type.name(),
                        return_type.name()
                    ),
                ));
            }
            Ok(Statement::Return(Some(lowered)))
        }
        Stmt::Expr {
            expr: AstExpr::Call { callee, args, .. },
            span,
        } if is_io_print_call(callee) => {
            let Some(function_name) = resolve_io_print_function(callee, imports) else {
                return Err(missing_io_import_diagnostic(path, span, callee));
            };
            let [arg] = args.as_slice() else {
                return Err(println_type_error(path, span, function_name));
            };
            let (arg_type, lowered) =
                lower_value_expr(path, arg, scope, imports, signatures, structs, enums, span)?;
            if arg_type != ValueType::String {
                return Err(println_type_error(path, span, function_name));
            }
            Ok(io_print_statement(function_name, lowered))
        }
        Stmt::Expr {
            expr: AstExpr::Panic { message },
            span,
        } => {
            let lowered = lower_panic_message(
                path, message, scope, imports, signatures, structs, enums, span,
            )?;
            Ok(Statement::Panic(lowered))
        }
        Stmt::Expr {
            expr:
                AstExpr::Call {
                    callee,
                    type_args,
                    args,
                },
            span,
        } if callee.len() == 2
            && matches!(callee[1].as_str(), "push" | "set" | "insert" | "clear")
            && !is_env_builtin_call(callee)
            && type_args.is_empty() =>
        {
            let lowered = lower_array_mutation(
                path, callee, args, scope, imports, signatures, structs, enums, span,
            )?;
            Ok(Statement::Assign {
                name: callee[0].clone(),
                value: lowered,
            })
        }
        Stmt::Match { value, arms, span } => lower_match_stmt(
            path,
            value,
            arms,
            scope,
            imports,
            signatures,
            structs,
            enums,
            return_type,
            loop_depth,
            span,
        ),
        Stmt::For { variant, span } => lower_for_stmt(
            path,
            variant,
            scope,
            imports,
            signatures,
            structs,
            enums,
            return_type,
            loop_depth,
            span,
        ),
        Stmt::Break { span } => {
            if loop_depth == 0 {
                return Err(Diagnostic::new(
                    "E0510",
                    "`break` is not allowed outside of a `for` loop",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            }
            Ok(Statement::Break)
        }
        Stmt::Continue { span } => {
            if loop_depth == 0 {
                return Err(Diagnostic::new(
                    "E0511",
                    "`continue` is not allowed outside of a `for` loop",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            }
            Ok(Statement::Continue)
        }
        Stmt::Defer { stmt, span } => {
            let Stmt::Expr { expr, .. } = stmt.as_ref() else {
                return Err(Diagnostic::new(
                    "E0265",
                    "`defer` expects a call expression",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            if let AstExpr::Call { callee, args, .. } = expr
                && is_io_print_call(callee)
            {
                let Some(function_name) = resolve_io_print_function(callee, imports) else {
                    return Err(missing_io_import_diagnostic(path, span, callee));
                };
                let [arg] = args.as_slice() else {
                    return Err(println_type_error(path, span, function_name));
                };
                let (arg_type, lowered) =
                    lower_value_expr(path, arg, scope, imports, signatures, structs, enums, span)?;
                if arg_type != ValueType::String {
                    return Err(println_type_error(path, span, function_name));
                }
                let call = io_print_deferred_call(function_name, lowered);
                return Ok(Statement::Defer { call });
            }
            let (_call_type, call) = lower_value_expr_with_expected(
                path,
                expr,
                scope,
                imports,
                signatures,
                structs,
                enums,
                Some(&ValueType::Void),
                span,
            )?;
            Ok(Statement::Defer {
                call: DeferredCall::Expr(call),
            })
        }
        Stmt::Unsafe { body, span } => {
            let [stmt] = body.as_slice() else {
                return Err(Diagnostic::new(
                    "E1519",
                    "v0.1 unsafe blocks must contain exactly one statement",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            lower_stmt(
                path,
                stmt,
                scope,
                imports,
                signatures,
                structs,
                enums,
                return_type,
                is_tail,
                loop_depth,
            )
        }
        Stmt::Expr { expr, span } => {
            let (expr_type, lowered) = lower_value_expr_with_expected(
                path,
                expr,
                scope,
                imports,
                signatures,
                structs,
                enums,
                Some(&ValueType::Void),
                span,
            )?;
            if expr_type != ValueType::Void {
                return Err(Diagnostic::new(
                    "E0203",
                    "unsupported non-void expression statement in v0.1 current implementation",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            }
            Ok(Statement::Expr(lowered))
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn lower_stmt_into(
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
) -> Result<(), Diagnostic> {
    if lower_question_exprs_in_stmt_into(
        path,
        stmt,
        scope,
        imports,
        signatures,
        structs,
        enums,
        return_type,
        is_tail,
        loop_depth,
        out,
    )? {
        return Ok(());
    }
    out.push(lower_stmt(
        path,
        stmt,
        scope,
        imports,
        signatures,
        structs,
        enums,
        return_type,
        is_tail,
        loop_depth,
    )?);
    Ok(())
}

pub(super) fn statements_diverge(statements: &[Statement]) -> bool {
    statements.last().is_some_and(statement_diverges)
}

pub(super) fn statement_diverges(statement: &Statement) -> bool {
    match statement {
        Statement::Return(_)
        | Statement::QuestionReturn { .. }
        | Statement::Panic(_)
        | Statement::Break
        | Statement::Continue => true,
        Statement::Match { arms, .. } => arms.iter().all(|arm| statements_diverge(&arm.body)),
        Statement::If {
            body, else_body, ..
        } => statements_diverge(body) && statements_diverge(else_body),
        Statement::IfLet {
            body,
            else_body: Some(else_body),
            ..
        } => statements_diverge(body) && statements_diverge(else_body),
        Statement::Loop { kind, .. } => matches!(kind, LoopKind::Infinite),
        _ => false,
    }
}

pub(super) fn statements_satisfy_function_return(statements: &[Statement]) -> bool {
    statements
        .last()
        .is_some_and(statement_satisfies_function_return)
}

pub(super) fn statement_satisfies_function_return(statement: &Statement) -> bool {
    match statement {
        Statement::Return(Some(_)) | Statement::QuestionReturn { .. } | Statement::Panic(_) => true,
        Statement::Match { arms, .. } => arms
            .iter()
            .all(|arm| statements_satisfy_function_return(&arm.body)),
        Statement::If {
            body, else_body, ..
        } => {
            statements_satisfy_function_return(body)
                && statements_satisfy_function_return(else_body)
        }
        Statement::IfLet {
            body,
            else_body: Some(else_body),
            ..
        } => {
            statements_satisfy_function_return(body)
                && statements_satisfy_function_return(else_body)
        }
        _ => false,
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn lower_for_stmt(
    path: &Path,
    variant: &ForVariant,
    scope: &mut HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    return_type: &ValueType,
    loop_depth: usize,
    span: &Span,
) -> Result<Statement, Diagnostic> {
    let (kind, body) = match variant {
        ForVariant::Infinite { body } => {
            let lowered = lower_block(
                path,
                body,
                scope,
                imports,
                signatures,
                structs,
                enums,
                return_type,
                loop_depth + 1,
            )?;
            (LoopKind::Infinite, lowered)
        }
        ForVariant::While { condition, body } => {
            if ast_expr_contains_question(condition) {
                let mut condition_scope = scope.clone();
                let mut lowered = Vec::new();
                let (condition, _) = extract_question_exprs(
                    path,
                    condition,
                    &mut condition_scope,
                    imports,
                    signatures,
                    structs,
                    enums,
                    return_type,
                    span,
                    &mut lowered,
                )?;
                let (cond_type, cond) = lower_value_expr(
                    path,
                    &condition,
                    &condition_scope,
                    imports,
                    signatures,
                    structs,
                    enums,
                    span,
                )?;
                if cond_type != ValueType::Bool {
                    return Err(type_mismatch(
                        path,
                        span,
                        format!(
                            "`for` condition must be `bool`, found `{}`",
                            cond_type.name()
                        ),
                    ));
                }
                let body = lower_block(
                    path,
                    body,
                    scope,
                    imports,
                    signatures,
                    structs,
                    enums,
                    return_type,
                    loop_depth + 1,
                )?;
                lowered.push(Statement::If {
                    condition: cond,
                    body,
                    else_body: vec![Statement::Break],
                });
                return Ok(Statement::Loop {
                    kind: LoopKind::Infinite,
                    body: lowered,
                });
            }
            let (cond_type, cond) = lower_value_expr(
                path, condition, scope, imports, signatures, structs, enums, span,
            )?;
            if cond_type != ValueType::Bool {
                return Err(type_mismatch(
                    path,
                    span,
                    format!(
                        "`for` condition must be `bool`, found `{}`",
                        cond_type.name()
                    ),
                ));
            }
            let lowered = lower_block(
                path,
                body,
                scope,
                imports,
                signatures,
                structs,
                enums,
                return_type,
                loop_depth + 1,
            )?;
            (LoopKind::While(cond), lowered)
        }
        ForVariant::Iterate {
            binding,
            iterable,
            body,
        } => {
            let (iter_type, iterable) = lower_value_expr(
                path, iterable, scope, imports, signatures, structs, enums, span,
            )?;
            let ValueType::Array(element_type) = &iter_type else {
                return Err(type_mismatch(
                    path,
                    span,
                    format!(
                        "`for ... in` requires `Array<T>`, found `{}`",
                        iter_type.name()
                    ),
                ));
            };
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
            let element_type = element_type.as_ref().clone();
            let mut loop_scope = scope.clone();
            loop_scope.insert(
                binding.clone(),
                Binding {
                    value_type: element_type.clone(),
                    mutable: false,
                    source: BindingSource::Local,
                },
            );
            let lowered = lower_block(
                path,
                body,
                &mut loop_scope,
                imports,
                signatures,
                structs,
                enums,
                return_type,
                loop_depth + 1,
            )?;
            (
                LoopKind::Iterate {
                    binding: binding.clone(),
                    element_type,
                    iterable,
                },
                lowered,
            )
        }
    };
    Ok(Statement::Loop { kind, body })
}

#[allow(clippy::too_many_arguments)]
pub(super) fn lower_block(
    path: &Path,
    statements: &[Stmt],
    scope: &mut HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    return_type: &ValueType,
    loop_depth: usize,
) -> Result<Vec<Statement>, Diagnostic> {
    let mut block_scope = scope.clone();
    let mut out = Vec::new();
    for stmt in statements {
        lower_stmt_into(
            path,
            stmt,
            &mut block_scope,
            imports,
            signatures,
            structs,
            enums,
            return_type,
            false,
            loop_depth,
            &mut out,
        )?;
    }
    Ok(out)
}
