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
    validate_statement_publication_uses(path, stmt, scope)?;
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
        Stmt::TaskDeadline { .. } => {
            unreachable!("task.deadline is lowered by lower_stmt_into")
        }
        Stmt::TaskSelect { .. } => {
            unreachable!("task.select is lowered by lower_stmt_into")
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
    select_exit_cancellations: &[String],
    out: &mut Vec<Statement>,
) -> Result<(), Diagnostic> {
    validate_statement_publication_uses(path, stmt, scope)?;
    if let Stmt::TaskSelect { arms, span } = stmt {
        require_import(path, imports, span, "std.task", "task.select")?;
        if !current_function_is_suspend(scope) {
            return Err(Diagnostic::new(
                "E0870",
                "synchronous function cannot enter `task.select`; mark the caller `suspend`",
                path,
                span.line,
                span.column,
                span.length,
                &span.text,
            ));
        }
        if !(2..=8).contains(&arms.len()) {
            return Err(Diagnostic::new(
                "E0886",
                format!(
                    "task.select expects 2 through 8 static arms, got {}",
                    arms.len()
                ),
                path,
                span.line,
                span.column,
                span.length,
                &span.text,
            ));
        }

        let mut lowered_arms = Vec::with_capacity(arms.len());
        for arm in arms {
            validate_static_select_arm_body(path, &arm.body, &arm.span)?;
            validate_static_select_operand_publication_uses(
                path,
                &arm.operation,
                scope,
                &arm.span,
            )?;
            if scope.contains_key(&arm.binding) {
                return Err(Diagnostic::new(
                    "E0886",
                    format!(
                        "task.select result binding `{}` is already defined in the surrounding scope",
                        arm.binding
                    ),
                    path,
                    arm.span.line,
                    arm.span.column,
                    arm.span.length,
                    &arm.span.text,
                ));
            }

            let AstExpr::Call {
                callee,
                type_args,
                args: _,
            } = &arm.operation
            else {
                return Err(static_select_operation_diagnostic(path, &arm.span));
            };
            if !type_args.is_empty()
                || !matches!(
                    callee.as_slice(),
                    [module, operation]
                        if module == "task"
                            && matches!(
                                operation.as_str(),
                                "receive" | "send" | "sleep" | "join"
                            )
                )
            {
                return Err(static_select_operation_diagnostic(path, &arm.span));
            }

            let (binding_type, lowered_operation) = lower_value_expr(
                path,
                &arm.operation,
                scope,
                imports,
                signatures,
                structs,
                enums,
                &arm.span,
            )?;
            let operation = match (callee[1].as_str(), lowered_operation) {
                ("receive", ValueExpr::Call { name, mut args })
                    if name.starts_with(BUILTIN_TASK_RECEIVE_PREFIX) && args.len() == 1 =>
                {
                    let ValueType::Enum(option, option_args) = &binding_type else {
                        unreachable!("task.receive always returns Option<T>")
                    };
                    debug_assert_eq!(option, "Option");
                    let [element_type] = option_args.as_slice() else {
                        unreachable!("task.receive Option has one type argument")
                    };
                    TaskSelectOperation::Receive {
                        channel: args.remove(0),
                        element_type: element_type.clone(),
                    }
                }
                ("send", ValueExpr::Call { name, mut args })
                    if name.starts_with(BUILTIN_TASK_SEND_PREFIX) && args.len() == 2 =>
                {
                    let ValueType::Enum(result, result_args) = &binding_type else {
                        unreachable!("task.send always returns Result<void, ChannelSendError<T>>")
                    };
                    debug_assert_eq!(result, "Result");
                    let [
                        ValueType::Void,
                        ValueType::Struct(send_error, send_error_args),
                    ] = result_args.as_slice()
                    else {
                        unreachable!("task.send result keeps its element type")
                    };
                    debug_assert_eq!(send_error, "ChannelSendError");
                    let [element_type] = send_error_args.as_slice() else {
                        unreachable!("ChannelSendError has one type argument")
                    };
                    let channel = args.remove(0);
                    let value = args.remove(0);
                    record_static_select_send_publication_move(
                        path, &arm.span, &value, scope, loop_depth,
                    )?;
                    TaskSelectOperation::Send {
                        channel,
                        value: Box::new(value),
                        element_type: element_type.clone(),
                    }
                }
                ("sleep", ValueExpr::Call { name, mut args })
                    if name == BUILTIN_TASK_SLEEP_EXPR && args.len() == 1 =>
                {
                    TaskSelectOperation::Sleep {
                        duration: args.remove(0),
                    }
                }
                ("join", ValueExpr::Call { name, mut args })
                    if name == BUILTIN_TASK_STRUCTURED_JOIN_EXPR && args.len() == 1 =>
                {
                    let ValueExpr::Variable(handle) = args.remove(0) else {
                        return Err(static_select_ownership_diagnostic(
                            path,
                            &arm.span,
                            "task.select join arms require one unqualified scope-owned handle",
                        ));
                    };
                    TaskSelectOperation::Join { handle }
                }
                _ => return Err(static_select_operation_diagnostic(path, &arm.span)),
            };

            let mut arm_scope = scope.clone();
            arm_scope.insert(
                arm.binding.clone(),
                Binding {
                    value_type: binding_type.clone(),
                    mutable: false,
                    source: BindingSource::Local,
                },
            );
            let mut body = Vec::new();
            for statement in &arm.body {
                lower_static_select_arm_statement_into(
                    path,
                    statement,
                    &mut arm_scope,
                    imports,
                    signatures,
                    structs,
                    enums,
                    return_type,
                    loop_depth,
                    select_exit_cancellations,
                    &mut body,
                )?;
            }
            if arm_scope.keys().any(|name| {
                name.starts_with(PUBLICATION_MOVE_BINDING_PREFIX) && !scope.contains_key(name)
            }) {
                return Err(Diagnostic::new(
                    "E0876",
                    "the first task.select slice does not allow arm-local publication moves of surrounding bindings",
                    path,
                    arm.span.line,
                    arm.span.column,
                    arm.span.length,
                    &arm.span.text,
                ));
            }
            lowered_arms.push(TaskSelectArm {
                operation,
                binding: arm.binding.clone(),
                binding_type,
                body,
            });
        }
        out.push(Statement::TaskSelect { arms: lowered_arms });
        return Ok(());
    }
    if let Stmt::TaskDeadline {
        duration,
        body,
        span,
    } = stmt
    {
        require_import(path, imports, span, "std.task", "task.deadline")?;
        if !current_function_is_suspend(scope) {
            return Err(Diagnostic::new(
                "E0870",
                "synchronous function cannot enter `task.deadline`; mark the caller `suspend`",
                path,
                span.line,
                span.column,
                span.length,
                &span.text,
            ));
        }
        if current_function_has_task_scope(scope)
            || out.iter().any(|statement| {
                matches!(
                    statement,
                    Statement::Expr(ValueExpr::Call { name, args })
                        if name == BUILTIN_TASK_DEADLINE_ENTER_EXPR && args.len() == 1
                )
            })
        {
            return Err(Diagnostic::new(
                "E0876",
                "the first deadline slice supports one non-nested task.deadline block per suspend function",
                path,
                span.line,
                span.column,
                span.length,
                &span.text,
            ));
        }
        validate_deadline_scope(path, body)?;
        let StructuredScopeValidation {
            unjoined,
            question_cancellations,
            select_cancellations,
        } = validate_structured_scope(path, body)?;
        debug_assert!(question_cancellations.is_empty());

        let duration_type = ValueType::Struct("Duration".to_string(), Vec::new());
        let (actual_duration_type, lowered_duration) = lower_value_expr_with_expected(
            path,
            duration,
            scope,
            imports,
            signatures,
            structs,
            enums,
            Some(&duration_type),
            span,
        )?;
        if actual_duration_type != duration_type {
            return Err(type_mismatch(
                path,
                span,
                "task.deadline expects one Duration value",
            ));
        }

        out.push(Statement::Expr(ValueExpr::Call {
            name: BUILTIN_TASK_DEADLINE_ENTER_EXPR.to_string(),
            args: vec![lowered_duration],
        }));
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
                select_cancellations
                    .get(&index)
                    .map(Vec::as_slice)
                    .unwrap_or(&[]),
                out,
            )?;
        }
        propagate_publication_moves(scope, &task_scope);
        push_structured_cancellations(out, &unjoined);
        out.push(Statement::Expr(ValueExpr::Call {
            name: BUILTIN_TASK_DEADLINE_EXIT_EXPR.to_string(),
            args: Vec::new(),
        }));
        return Ok(());
    }
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
            select_cancellations,
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
                    &[],
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
                select_cancellations
                    .get(&index)
                    .map(Vec::as_slice)
                    .unwrap_or(&[]),
                out,
            )?;
        }
        propagate_publication_moves(scope, &task_scope);
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
    let lowered = lower_stmt(
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
    )?;
    record_publication_moves(path, stmt, &lowered, scope, loop_depth)?;
    out.push(lowered);
    Ok(())
}

fn static_select_operation_diagnostic(path: &Path, span: &Span) -> Diagnostic {
    Diagnostic::new(
        "E0886",
        "task.select supports only direct `task.receive(channel)`, `task.send(channel, value)`, `task.sleep(duration)`, and `task.join(child)` arms",
        path,
        span.line,
        span.column,
        span.length,
        &span.text,
    )
}

fn static_select_ownership_diagnostic(
    path: &Path,
    span: &Span,
    message: impl Into<String>,
) -> Diagnostic {
    Diagnostic::new(
        "E0887",
        message,
        path,
        span.line,
        span.column,
        span.length,
        &span.text,
    )
}

fn validate_static_select_operand_publication_uses(
    path: &Path,
    operation: &AstExpr,
    scope: &HashMap<String, Binding>,
    span: &Span,
) -> Result<(), Diagnostic> {
    crate::validation_tasks::visit_expression(operation, &mut |candidate| {
        let referenced = match candidate {
            AstExpr::Name(parts) | AstExpr::MutArg { name: parts } => parts.first(),
            AstExpr::Call { callee, .. } => callee.first(),
            _ => None,
        };
        if let Some(name) = referenced
            && scope.contains_key(name)
        {
            ensure_publication_binding_available(path, span, scope, name)?;
        }
        Ok(())
    })
}

fn record_static_select_send_publication_move(
    path: &Path,
    span: &Span,
    value: &ValueExpr,
    scope: &mut HashMap<String, Binding>,
    loop_depth: usize,
) -> Result<(), Diagnostic> {
    let ValueExpr::Call { name, args } = value else {
        return Ok(());
    };
    if name != BUILTIN_TASK_PUBLICATION_MOVE_EXPR {
        return Ok(());
    }
    let [ValueExpr::Variable(binding)] = args.as_slice() else {
        return Ok(());
    };
    if loop_depth > 0 {
        return Err(static_select_ownership_diagnostic(
            path,
            span,
            format!(
                "binding `{binding}` cannot be moved into a repeatable task.select send arm; construct the value inside the loop"
            ),
        ));
    }
    mark_publication_move(scope, binding, span.line, "task.select send arm");
    Ok(())
}

fn validate_static_select_arm_body(
    path: &Path,
    body: &[Stmt],
    arm_span: &Span,
) -> Result<(), Diagnostic> {
    fn validate_statement(path: &Path, statement: &Stmt, nested: bool) -> Result<(), Diagnostic> {
        let span = statement_span(statement);
        match statement {
            Stmt::Return { .. } if nested => {
                return Err(Diagnostic::new(
                    "E0876",
                    "task.select supports return, panic, and `?` only as direct arm statements; nested frame exits remain unsupported",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            }
            Stmt::Break { .. }
            | Stmt::Continue { .. }
            | Stmt::Defer { .. }
            | Stmt::TaskScope { .. }
            | Stmt::TaskDeadline { .. }
            | Stmt::TaskSelect { .. }
            | Stmt::Unsafe { .. } => {
                return Err(Diagnostic::new(
                    "E0876",
                    "task.select arms reject break, continue, defer, unsafe, nested task scopes, deadlines, and nested select",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            }
            Stmt::Return { .. } => {}
            Stmt::LetElse { else_body, .. } => {
                for statement in else_body {
                    validate_statement(path, statement, true)?;
                }
            }
            Stmt::IfLet {
                body, else_body, ..
            } => {
                for statement in body {
                    validate_statement(path, statement, true)?;
                }
                if let Some(else_body) = else_body {
                    for statement in else_body {
                        validate_statement(path, statement, true)?;
                    }
                }
            }
            Stmt::Match { arms, .. } => {
                for arm in arms {
                    for statement in &arm.body {
                        validate_statement(path, statement, true)?;
                    }
                }
            }
            Stmt::For { variant, .. } => {
                let nested = match variant {
                    ForVariant::Infinite { body }
                    | ForVariant::While { body, .. }
                    | ForVariant::CStyle { body, .. }
                    | ForVariant::Iterate { body, .. } => body,
                };
                for statement in nested {
                    validate_statement(path, statement, true)?;
                }
            }
            Stmt::Let { .. }
            | Stmt::Assign { .. }
            | Stmt::IndexAssign { .. }
            | Stmt::Postfix { .. }
            | Stmt::Expr { .. } => {}
        }

        let mut contains_frame_exit = false;
        crate::validation_tasks::visit_statement_expressions(statement, &mut |expression| {
            if matches!(expression, AstExpr::Panic { .. } | AstExpr::Question { .. }) {
                contains_frame_exit = true;
            }
            Ok(())
        })?;
        let direct_frame_exit = matches!(
            statement,
            Stmt::Let {
                value: AstExpr::Question { .. },
                ..
            } | Stmt::Return { .. }
                | Stmt::Expr {
                    expr: AstExpr::Panic { .. },
                    ..
                }
        );
        if contains_frame_exit && (nested || !direct_frame_exit) {
            return Err(Diagnostic::new(
                "E0876",
                "task.select supports panic and `?` only as direct arm statements with a verified frame-drop plan",
                path,
                span.line,
                span.column,
                span.length,
                &span.text,
            ));
        }
        Ok(())
    }

    if body.is_empty() {
        return Err(Diagnostic::new(
            "E0886",
            "task.select arms require a non-empty lexical body",
            path,
            arm_span.line,
            arm_span.column,
            arm_span.length,
            &arm_span.text,
        ));
    }
    for statement in body {
        validate_statement(path, statement, false)?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn lower_static_select_arm_statement_into(
    path: &Path,
    statement: &Stmt,
    arm_scope: &mut HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    return_type: &ValueType,
    loop_depth: usize,
    cancellations: &[String],
    out: &mut Vec<Statement>,
) -> Result<(), Diagnostic> {
    if matches!(
        statement,
        Stmt::Let {
            value: AstExpr::Question { .. },
            ..
        }
    ) {
        return lower_structured_question_let_into(
            path,
            statement,
            arm_scope,
            imports,
            signatures,
            structs,
            enums,
            return_type,
            loop_depth,
            cancellations,
            out,
        );
    }

    let mut lowered = Vec::new();
    lower_stmt_into(
        path,
        statement,
        arm_scope,
        imports,
        signatures,
        structs,
        enums,
        return_type,
        false,
        loop_depth,
        &[],
        &mut lowered,
    )?;
    let [lowered_statement] = lowered.as_mut_slice() else {
        out.extend(lowered);
        return Ok(());
    };
    match lowered_statement {
        Statement::Return(Some(value)) if !cancellations.is_empty() => {
            let temporary = fresh_internal_binding(arm_scope, "select_return_value");
            arm_scope.insert(
                temporary.clone(),
                Binding {
                    value_type: return_type.clone(),
                    mutable: false,
                    source: BindingSource::Local,
                },
            );
            out.push(Statement::Let {
                name: temporary.clone(),
                value_type: return_type.clone(),
                initializer: value.clone(),
            });
            push_structured_cancellations(out, cancellations);
            out.push(Statement::Return(Some(ValueExpr::Variable(temporary))));
        }
        Statement::Return(None) if !cancellations.is_empty() => {
            push_structured_cancellations(out, cancellations);
            out.push(Statement::Return(None));
        }
        Statement::QuestionReturn {
            early_exit_actions, ..
        } => {
            *early_exit_actions = structured_cancellation_actions(cancellations);
            out.append(&mut lowered);
        }
        Statement::Panic(message) if !cancellations.is_empty() => {
            let temporary = fresh_internal_binding(arm_scope, "select_panic_message");
            arm_scope.insert(
                temporary.clone(),
                Binding {
                    value_type: ValueType::String,
                    mutable: false,
                    source: BindingSource::Local,
                },
            );
            out.push(Statement::Let {
                name: temporary.clone(),
                value_type: ValueType::String,
                initializer: message.clone(),
            });
            push_structured_cancellations(out, cancellations);
            out.push(Statement::Panic(ValueExpr::Variable(temporary)));
        }
        _ => out.append(&mut lowered),
    }
    Ok(())
}

fn record_publication_moves(
    path: &Path,
    source: &Stmt,
    lowered: &Statement,
    scope: &mut HashMap<String, Binding>,
    loop_depth: usize,
) -> Result<(), Diagnostic> {
    let call = match lowered {
        Statement::Let {
            initializer: ValueExpr::Call { name, args },
            ..
        }
        | Statement::Expr(ValueExpr::Call { name, args })
        | Statement::QuestionLet {
            result_expr: ValueExpr::Call { name, args },
            ..
        } => Some((name, args)),
        _ => None,
    };
    let Some((name, args)) = call else {
        return Ok(());
    };
    let boundary = if name.starts_with(BUILTIN_TASK_STRUCTURED_SPAWN_PREFIX) {
        "structured task.spawn"
    } else if name.starts_with(BUILTIN_TASK_SEND_PREFIX) {
        "task.send"
    } else if name.starts_with(BUILTIN_TASK_TRY_SEND_PREFIX) {
        "task.try_send"
    } else {
        return Ok(());
    };
    let span = statement_span(source);
    for argument in args {
        let ValueExpr::Call {
            name: move_name,
            args: move_args,
        } = argument
        else {
            continue;
        };
        if move_name != BUILTIN_TASK_PUBLICATION_MOVE_EXPR {
            continue;
        }
        let [ValueExpr::Variable(binding)] = move_args.as_slice() else {
            continue;
        };
        if loop_depth > 0 {
            return Err(Diagnostic::new(
                "E0881",
                format!(
                    "binding `{binding}` cannot be publication-moved from a repeatable loop in the current P3-A slice; construct the value inside a non-repeating task scope"
                ),
                path,
                span.line,
                span.column,
                span.length,
                &span.text,
            ));
        }
        mark_publication_move(scope, binding, span.line, boundary);
    }
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
    select_cancellations: HashMap<usize, Vec<String>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StructuredHandleState {
    Available,
    Consumed,
    SelectConsumed,
}

fn validate_structured_scope(
    path: &Path,
    body: &[Stmt],
) -> Result<StructuredScopeValidation, Diagnostic> {
    let mut handles = HashMap::<String, StructuredHandleState>::new();
    let mut question_cancellations = HashMap::new();
    let mut select_cancellations = HashMap::new();
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
                handles.insert(name.clone(), StructuredHandleState::Available);
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
                    consume_structured_handle(path, span, args, &mut handles, "task.join")?;
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
                        return Err(structured_handle_escape_diagnostic(
                            path,
                            span,
                            &handle,
                            &handles,
                            format!(
                                "task handle `{handle}` may only be consumed by task.join or task.cancel inside its scope"
                            ),
                        ));
                    }
                }
                question_cancellations.insert(index, cleanup_structured_handles(&handles));
            }
            Stmt::Let {
                mutable,
                value: AstExpr::Call { callee, args, .. },
                ..
            } if callee == &["task", "cancel"]
                && args.len() == 1
                && call_targets_structured_handle(args, &handles) =>
            {
                if *mutable {
                    return Err(Diagnostic::new(
                        "E0876",
                        "structured cancel results must use immutable bindings",
                        path,
                        span.line,
                        span.column,
                        span.length,
                        &span.text,
                    ));
                }
                consume_structured_handle(path, span, args, &mut handles, "task.cancel")?;
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
                consume_structured_handle(path, span, args, &mut handles, "task.join")?;
            }
            Stmt::TaskSelect { arms, .. } => {
                for arm in arms {
                    let AstExpr::Call {
                        callee,
                        args,
                        type_args,
                    } = &arm.operation
                    else {
                        continue;
                    };
                    if callee == &["task", "join"] && type_args.is_empty() && args.len() == 1 {
                        consume_structured_select_handle(path, &arm.span, args, &mut handles)?;
                        continue;
                    }
                    reject_structured_handle_escape(path, &arm.span, &arm.operation, &handles)?;
                }
                for arm in arms {
                    for arm_statement in &arm.body {
                        reject_structured_statement_handle_escape(path, arm_statement, &handles)?;
                    }
                }
                select_cancellations.insert(index, cleanup_structured_handles(&handles));
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
                    return Err(structured_handle_escape_diagnostic(
                        path,
                        span,
                        &handle,
                        &handles,
                        format!("task handle `{handle}` cannot be returned from its scope"),
                    ));
                }
            }
            Stmt::TaskScope { .. }
            | Stmt::TaskDeadline { .. }
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
                    return Err(structured_handle_escape_diagnostic(
                        path,
                        span,
                        &handle,
                        &handles,
                        format!(
                            "task handle `{handle}` may only be consumed by task.join or task.cancel inside its scope"
                        ),
                    ));
                }
            }
        }
    }
    Ok(StructuredScopeValidation {
        unjoined: cleanup_structured_handles(&handles),
        question_cancellations,
        select_cancellations,
    })
}

fn validate_deadline_scope(path: &Path, body: &[Stmt]) -> Result<(), Diagnostic> {
    for statement in body {
        let span = statement_span(statement);
        if matches!(statement, Stmt::Return { .. }) {
            return Err(Diagnostic::new(
                "E0876",
                "return inside task.deadline requires the later general structured-exit slice",
                path,
                span.line,
                span.column,
                span.length,
                &span.text,
            ));
        }
        let mut contains_frame_exit = false;
        visit_statement_expressions(statement, &mut |expression| {
            if matches!(expression, AstExpr::Panic { .. } | AstExpr::Question { .. }) {
                contains_frame_exit = true;
            }
            Ok(())
        })?;
        if contains_frame_exit {
            return Err(Diagnostic::new(
                "E0876",
                "panic and `?` inside task.deadline require the later general structured-exit slice",
                path,
                span.line,
                span.column,
                span.length,
                &span.text,
            ));
        }
    }
    Ok(())
}

fn call_targets_structured_handle(
    args: &[AstExpr],
    handles: &HashMap<String, StructuredHandleState>,
) -> bool {
    matches!(
        args,
        [AstExpr::Name(handle_path)]
            if matches!(handle_path.as_slice(), [handle] if handles.contains_key(handle))
    )
}

fn consume_structured_handle(
    path: &Path,
    span: &Span,
    args: &[AstExpr],
    handles: &mut HashMap<String, StructuredHandleState>,
    operation: &str,
) -> Result<(), Diagnostic> {
    let [AstExpr::Name(handle_path)] = args else {
        return Err(Diagnostic::new(
            "E0872",
            format!("{operation} expects one scope-owned task handle"),
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
            format!("{operation} expects an unqualified scope-owned task handle"),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    };
    let Some(state) = handles.get_mut(handle) else {
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
    if *state != StructuredHandleState::Available {
        if *state == StructuredHandleState::SelectConsumed {
            return Err(static_select_ownership_diagnostic(
                path,
                span,
                format!(
                    "task handle `{handle}` is unavailable after task.select; a winning join consumed it and a losing join returned it only to implicit scope cleanup"
                ),
            ));
        }
        let message = if operation == "task.join" {
            format!("task handle `{handle}` is joined more than once or after cancellation")
        } else {
            format!("task handle `{handle}` is cancelled more than once or after join")
        };
        return Err(Diagnostic::new(
            "E0872",
            message,
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    }
    *state = StructuredHandleState::Consumed;
    Ok(())
}

fn consume_structured_select_handle(
    path: &Path,
    span: &Span,
    args: &[AstExpr],
    handles: &mut HashMap<String, StructuredHandleState>,
) -> Result<(), Diagnostic> {
    let [AstExpr::Name(handle_path)] = args else {
        return Err(static_select_ownership_diagnostic(
            path,
            span,
            "task.select join arms require one scope-owned task handle",
        ));
    };
    let [handle] = handle_path.as_slice() else {
        return Err(static_select_ownership_diagnostic(
            path,
            span,
            "task.select join arms require one unqualified scope-owned task handle",
        ));
    };
    let Some(state) = handles.get_mut(handle) else {
        return Err(static_select_ownership_diagnostic(
            path,
            span,
            format!("`{handle}` is not a task handle owned by this scope"),
        ));
    };
    if *state != StructuredHandleState::Available {
        return Err(static_select_ownership_diagnostic(
            path,
            span,
            format!(
                "task handle `{handle}` cannot appear in more than one task.select join arm or after another consuming operation"
            ),
        ));
    }
    *state = StructuredHandleState::SelectConsumed;
    Ok(())
}

fn cleanup_structured_handles(handles: &HashMap<String, StructuredHandleState>) -> Vec<String> {
    let mut unjoined = handles
        .iter()
        .filter_map(|(name, state)| {
            (*state != StructuredHandleState::Consumed).then_some(name.as_str())
        })
        .collect::<Vec<_>>();
    unjoined.sort_unstable();
    unjoined.into_iter().map(str::to_string).collect()
}

fn structured_handle_escape_diagnostic(
    path: &Path,
    span: &Span,
    handle: &str,
    handles: &HashMap<String, StructuredHandleState>,
    fallback: String,
) -> Diagnostic {
    if handles.get(handle) == Some(&StructuredHandleState::SelectConsumed) {
        return static_select_ownership_diagnostic(
            path,
            span,
            format!(
                "task handle `{handle}` is unavailable after task.select; losing join ownership is reserved for mandatory implicit scope cleanup"
            ),
        );
    }
    Diagnostic::new(
        "E0872",
        fallback,
        path,
        span.line,
        span.column,
        span.length,
        &span.text,
    )
}

fn reject_structured_handle_escape(
    path: &Path,
    span: &Span,
    expression: &AstExpr,
    handles: &HashMap<String, StructuredHandleState>,
) -> Result<(), Diagnostic> {
    let mut escaped = None;
    crate::validation_tasks::visit_expression(expression, &mut |candidate| {
        if let AstExpr::Name(name) = candidate
            && let [name] = name.as_slice()
            && handles.contains_key(name)
        {
            escaped = Some(name.clone());
        }
        Ok(())
    })?;
    if let Some(handle) = escaped {
        return Err(structured_handle_escape_diagnostic(
            path,
            span,
            &handle,
            handles,
            format!(
                "task handle `{handle}` may only be consumed by task.join or task.cancel inside its scope"
            ),
        ));
    }
    Ok(())
}

fn reject_structured_statement_handle_escape(
    path: &Path,
    statement: &Stmt,
    handles: &HashMap<String, StructuredHandleState>,
) -> Result<(), Diagnostic> {
    let span = statement_span(statement);
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
        return Err(structured_handle_escape_diagnostic(
            path,
            span,
            &handle,
            handles,
            format!(
                "task handle `{handle}` may only be consumed by task.join or task.cancel inside its scope"
            ),
        ));
    }
    Ok(())
}
