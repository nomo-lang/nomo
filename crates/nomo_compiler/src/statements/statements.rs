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
                    early_exit_actions: Vec::new(),
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
        Stmt::IndexAssign {
            root,
            indices,
            value,
            span,
        } => lower_index_assign_stmt(
            path, root, indices, value, scope, imports, signatures, structs, enums, span,
        ),
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
            let lowered = lower_io_print_args(
                path,
                args,
                scope,
                imports,
                signatures,
                structs,
                enums,
                span,
                function_name,
            )?;
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
                let lowered = lower_io_print_args(
                    path,
                    args,
                    scope,
                    imports,
                    signatures,
                    structs,
                    enums,
                    span,
                    function_name,
                )?;
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
        Stmt::TaskScope { .. } => {
            unreachable!("task.scope is lowered by lower_stmt_into")
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
            match lowered {
                ValueExpr::Panic {
                    message,
                    fallback_type: ValueType::Void,
                } => Ok(Statement::Panic(*message)),
                other => Ok(Statement::Expr(other)),
            }
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
    if let Stmt::TaskScope { body, span } = stmt {
        require_import(path, imports, span, "std.task", "task.scope")?;
        if !current_function_is_suspend(scope) {
            return Err(Diagnostic::new(
                "E0870",
                "synchronous function cannot enter `task.scope`; mark the caller `suspend`",
                path,
                span.line,
                span.column,
                span.length,
                &span.text,
            ));
        }
        if current_function_has_task_scope(scope) {
            return Err(Diagnostic::new(
                "E0876",
                "nested task scopes require the later structured cancellation slice",
                path,
                span.line,
                span.column,
                span.length,
                &span.text,
            ));
        }
        let StructuredScopeValidation {
            unjoined,
            question_cancellations,
        } = validate_structured_scope(path, body)?;
        let exits_with_return = body
            .last()
            .is_some_and(|statement| matches!(statement, Stmt::Return { .. }));
        let mut task_scope = scope.clone();
        task_scope.insert(
            TASK_SCOPE_BINDING.to_string(),
            Binding {
                value_type: ValueType::Void,
                mutable: false,
                source: BindingSource::TaskScope,
            },
        );
        for (index, statement) in body.iter().enumerate() {
            if let Some(cancellations) = question_cancellations.get(&index) {
                lower_structured_question_let_into(
                    path,
                    statement,
                    &mut task_scope,
                    imports,
                    signatures,
                    structs,
                    enums,
                    return_type,
                    loop_depth,
                    cancellations,
                    out,
                )?;
                continue;
            }
            if exits_with_return && index + 1 == body.len() && !unjoined.is_empty() {
                let mut lowered_return = Vec::new();
                lower_stmt_into(
                    path,
                    statement,
                    &mut task_scope,
                    imports,
                    signatures,
                    structs,
                    enums,
                    return_type,
                    false,
                    loop_depth,
                    &mut lowered_return,
                )?;
                let [Statement::Return(value)] = lowered_return.as_slice() else {
                    unreachable!("validated final scope return lowers to one return statement");
                };
                if let Some(value) = value {
                    let temporary = structured_return_temporary(&task_scope);
                    out.push(Statement::Let {
                        name: temporary.clone(),
                        value_type: return_type.clone(),
                        initializer: value.clone(),
                    });
                    push_structured_cancellations(out, &unjoined);
                    out.push(Statement::Return(Some(ValueExpr::Variable(temporary))));
                } else {
                    push_structured_cancellations(out, &unjoined);
                    out.push(Statement::Return(None));
                }
                continue;
            }
            lower_stmt_into(
                path,
                statement,
                &mut task_scope,
                imports,
                signatures,
                structs,
                enums,
                return_type,
                false,
                loop_depth,
                out,
            )?;
        }
        if !exits_with_return {
            push_structured_cancellations(out, &unjoined);
        }
        return Ok(());
    }
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

fn structured_return_temporary(scope: &HashMap<String, Binding>) -> String {
    let mut name = "__nomo_structured_return_value".to_string();
    while scope.contains_key(&name) {
        name.push('_');
    }
    name
}

#[allow(clippy::too_many_arguments)]
fn lower_structured_question_let_into(
    path: &Path,
    statement: &Stmt,
    task_scope: &mut HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    return_type: &ValueType,
    loop_depth: usize,
    cancellations: &[String],
    out: &mut Vec<Statement>,
) -> Result<(), Diagnostic> {
    let mut lowered = lower_stmt(
        path,
        statement,
        task_scope,
        imports,
        signatures,
        structs,
        enums,
        return_type,
        false,
        loop_depth,
    )?;
    let Statement::QuestionLet {
        result_type,
        result_expr,
        early_exit_actions,
        ..
    } = &mut lowered
    else {
        unreachable!("validated structured question binding lowers to QuestionLet");
    };
    *early_exit_actions = structured_cancellation_actions(cancellations);
    if matches!(
        result_expr,
        ValueExpr::Call { name, args }
            if name == BUILTIN_TASK_STRUCTURED_JOIN_EXPR && args.len() == 1
    ) {
        let temporary = fresh_internal_binding(task_scope, "structured_question_result");
        let temporary_type = result_type.clone();
        let initializer = result_expr.clone();
        task_scope.insert(
            temporary.clone(),
            Binding {
                value_type: temporary_type.clone(),
                mutable: false,
                source: BindingSource::Local,
            },
        );
        out.push(Statement::Let {
            name: temporary.clone(),
            value_type: temporary_type,
            initializer,
        });
        *result_expr = ValueExpr::Variable(temporary);
    }
    out.push(lowered);
    Ok(())
}

fn structured_cancellation_actions(handles: &[String]) -> Vec<ValueExpr> {
    handles
        .iter()
        .map(|handle| ValueExpr::Call {
            name: BUILTIN_TASK_STRUCTURED_CANCEL_EXPR.to_string(),
            args: vec![ValueExpr::Variable(handle.clone())],
        })
        .collect()
}

fn push_structured_cancellations(out: &mut Vec<Statement>, handles: &[String]) {
    for action in structured_cancellation_actions(handles) {
        out.push(Statement::Expr(action));
    }
}

struct StructuredScopeValidation {
    unjoined: Vec<String>,
    question_cancellations: HashMap<usize, Vec<String>>,
}

fn validate_structured_scope(
    path: &Path,
    body: &[Stmt],
) -> Result<StructuredScopeValidation, Diagnostic> {
    let mut handles = HashMap::<String, bool>::new();
    let mut question_cancellations = HashMap::new();
    for (index, statement) in body.iter().enumerate() {
        let span = statement_span(statement);
        match statement {
            Stmt::Let {
                name,
                mutable,
                type_annotation,
                value:
                    AstExpr::Call {
                        callee,
                        args,
                        type_args,
                    },
                ..
            } if callee == &["task", TASK_STRUCTURED_SPAWN_AST_NAME] => {
                if *mutable || type_annotation.is_some() || !type_args.is_empty() {
                    return Err(Diagnostic::new(
                        "E0876",
                        "the first structured task slice requires an inferred immutable spawn handle",
                        path,
                        span.line,
                        span.column,
                        span.length,
                        &span.text,
                    ));
                }
                let [AstExpr::Call { .. }] = args.as_slice() else {
                    return Err(Diagnostic::new(
                        "E0875",
                        "task.spawn expects one direct call to a named suspend function",
                        path,
                        span.line,
                        span.column,
                        span.length,
                        &span.text,
                    ));
                };
                handles.insert(name.clone(), false);
            }
            Stmt::Let {
                mutable,
                value: AstExpr::Question { expr },
                ..
            } => {
                if *mutable {
                    return Err(Diagnostic::new(
                        "E0876",
                        "structured question results must use immutable bindings",
                        path,
                        span.line,
                        span.column,
                        span.length,
                        &span.text,
                    ));
                }
                if let AstExpr::Call {
                    callee,
                    args,
                    type_args,
                } = expr.as_ref()
                    && callee == &["task", "join"]
                    && type_args.is_empty()
                    && args.len() == 1
                {
                    consume_structured_join_handle(path, span, args, &mut handles)?;
                } else {
                    let mut escaped = None;
                    visit_statement_expressions(statement, &mut |expression| {
                        if let AstExpr::Name(name) = expression
                            && let [name] = name.as_slice()
                            && handles.contains_key(name)
                        {
                            escaped = Some(name.clone());
                        }
                        Ok(())
                    })?;
                    if let Some(handle) = escaped {
                        return Err(Diagnostic::new(
                            "E0872",
                            format!(
                                "task handle `{handle}` may only be consumed by task.join inside its scope"
                            ),
                            path,
                            span.line,
                            span.column,
                            span.length,
                            &span.text,
                        ));
                    }
                }
                question_cancellations.insert(index, unjoined_structured_handles(&handles));
            }
            Stmt::Let {
                mutable,
                value: AstExpr::Call { callee, args, .. },
                ..
            } if callee == &["task", "join"] && args.len() == 1 => {
                if *mutable {
                    return Err(Diagnostic::new(
                        "E0876",
                        "structured join results must use immutable bindings",
                        path,
                        span.line,
                        span.column,
                        span.length,
                        &span.text,
                    ));
                }
                consume_structured_join_handle(path, span, args, &mut handles)?;
            }
            Stmt::Return { .. } => {
                if index + 1 != body.len() {
                    return Err(Diagnostic::new(
                        "E0876",
                        "task.scope return must be its final statement in the current post-join slice",
                        path,
                        span.line,
                        span.column,
                        span.length,
                        &span.text,
                    ));
                }
                let mut escaped = None;
                visit_statement_expressions(statement, &mut |expression| {
                    if let AstExpr::Name(name) = expression
                        && let [name] = name.as_slice()
                        && handles.contains_key(name)
                    {
                        escaped = Some(name.clone());
                    }
                    Ok(())
                })?;
                if let Some(handle) = escaped {
                    return Err(Diagnostic::new(
                        "E0872",
                        format!("task handle `{handle}` cannot be returned from its scope"),
                        path,
                        span.line,
                        span.column,
                        span.length,
                        &span.text,
                    ));
                }
            }
            Stmt::TaskScope { .. }
            | Stmt::LetElse { .. }
            | Stmt::IfLet { .. }
            | Stmt::Match { .. }
            | Stmt::For { .. }
            | Stmt::Defer { .. }
            | Stmt::Unsafe { .. }
            | Stmt::Break { .. }
            | Stmt::Continue { .. } => {
                return Err(Diagnostic::new(
                    "E0876",
                    "the current structured task slice requires a top-level scope body without nested control flow, defer, or unsafe blocks; return is allowed only as the final statement, when unjoined children can be cancelled before completion",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            }
            _ => {
                let mut escaped = None;
                visit_statement_expressions(statement, &mut |expression| {
                    if let AstExpr::Name(name) = expression
                        && let [name] = name.as_slice()
                        && handles.contains_key(name)
                    {
                        escaped = Some(name.clone());
                    }
                    Ok(())
                })?;
                if let Some(handle) = escaped {
                    return Err(Diagnostic::new(
                        "E0872",
                        format!(
                            "task handle `{handle}` may only be consumed by task.join inside its scope"
                        ),
                        path,
                        span.line,
                        span.column,
                        span.length,
                        &span.text,
                    ));
                }
            }
        }
    }
    Ok(StructuredScopeValidation {
        unjoined: unjoined_structured_handles(&handles),
        question_cancellations,
    })
}

fn consume_structured_join_handle(
    path: &Path,
    span: &Span,
    args: &[AstExpr],
    handles: &mut HashMap<String, bool>,
) -> Result<(), Diagnostic> {
    let [AstExpr::Name(handle_path)] = args else {
        return Err(Diagnostic::new(
            "E0872",
            "task.join expects one scope-owned task handle",
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    };
    let [handle] = handle_path.as_slice() else {
        return Err(Diagnostic::new(
            "E0872",
            "task.join expects an unqualified scope-owned task handle",
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    };
    let Some(joined) = handles.get_mut(handle) else {
        return Err(Diagnostic::new(
            "E0872",
            format!("`{handle}` is not a task handle owned by this scope"),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    };
    if *joined {
        return Err(Diagnostic::new(
            "E0872",
            format!("task handle `{handle}` is joined more than once"),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    }
    *joined = true;
    Ok(())
}

fn unjoined_structured_handles(handles: &HashMap<String, bool>) -> Vec<String> {
    let mut unjoined = handles
        .iter()
        .filter_map(|(name, joined)| (!joined).then_some(name.as_str()))
        .collect::<Vec<_>>();
    unjoined.sort_unstable();
    unjoined.into_iter().map(str::to_string).collect()
}
