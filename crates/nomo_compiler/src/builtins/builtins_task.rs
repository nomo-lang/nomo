use super::*;

pub(super) fn is_task_builtin_call(callee: &[String]) -> bool {
    matches!(
        callee,
        [module, name]
            if module == "task"
                && (matches!(
                    name.as_str(),
                    "spawn"
                        | "is_cancelled"
                        | "check_cancelled"
                        | "join"
                        | "cancel"
                        | "close"
                        | "yield_now"
                        | "sleep"
                ) || name == TASK_STRUCTURED_SPAWN_AST_NAME)
    )
}

pub(super) fn lower_task_builtin(
    path: &Path,
    callee: &[String],
    args: &[AstExpr],
    scope: &HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    span: &Span,
) -> Result<(ValueType, ValueExpr), Diagnostic> {
    let [module, name] = callee else {
        unreachable!("task builtin dispatcher only passes qualified calls")
    };
    debug_assert_eq!(module, "task");

    let task_type = ValueType::Struct("Task".to_string(), Vec::new());
    let context_type = ValueType::Struct("TaskContext".to_string(), Vec::new());
    let error_type = ValueType::Struct("TaskError".to_string(), Vec::new());
    let join_type = ValueType::Enum("TaskJoin".to_string(), Vec::new());

    match name.as_str() {
        TASK_STRUCTURED_SPAWN_AST_NAME => {
            if !current_function_has_task_scope(scope) {
                return Err(Diagnostic::new(
                    "E0871",
                    "structured task.spawn is only allowed inside task.scope or task.deadline",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            }
            let [target] = args else {
                return Err(task_arity_diagnostic(
                    path,
                    span,
                    "task.spawn",
                    1,
                    args.len(),
                ));
            };
            let AstExpr::Call {
                callee,
                type_args,
                args: target_args,
            } = target
            else {
                return Err(structured_task_target_diagnostic(
                    path,
                    span,
                    "task.spawn expects one direct call to a named suspend function",
                ));
            };
            let [target_name] = callee.as_slice() else {
                return Err(structured_task_target_diagnostic(
                    path,
                    span,
                    "task.spawn target must be an unqualified top-level suspend function",
                ));
            };
            let Some(signature) = signatures.get(target_name) else {
                return Err(structured_task_target_diagnostic(
                    path,
                    span,
                    &format!("unknown structured task function `{target_name}`"),
                ));
            };
            if !signature.is_suspend {
                return Err(structured_task_target_diagnostic(
                    path,
                    span,
                    &format!("task.spawn target `{target_name}` must be declared `suspend fn`"),
                ));
            }
            if !type_args.is_empty()
                || !signature.type_params.is_empty()
                || signature.extern_symbol.is_some()
                || signature.params.iter().any(|parameter| parameter.mutable)
            {
                return Err(Diagnostic::new(
                    "E0876",
                    "the first structured task slice requires a non-generic suspend target with immutable parameters",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            }
            if target_args.len() != signature.params.len() {
                return Err(task_arity_diagnostic(
                    path,
                    span,
                    target_name,
                    signature.params.len(),
                    target_args.len(),
                ));
            }
            let mut lowered_args = Vec::with_capacity(target_args.len());
            let mut mutable_borrows = Vec::new();
            let mut moved_bindings = HashSet::new();
            for (index, (argument, parameter)) in
                target_args.iter().zip(&signature.params).enumerate()
            {
                let mut lowered = lower_call_arg_for_param(
                    path,
                    argument,
                    parameter,
                    scope,
                    imports,
                    signatures,
                    structs,
                    enums,
                    span,
                    target_name,
                    index + 1,
                    &mut mutable_borrows,
                )?;
                let transfer = validate_publication_type(
                    path,
                    span,
                    &format!("argument {} to `{target_name}`", index + 1),
                    &parameter.value_type,
                    structs,
                    enums,
                )?;
                if transfer == PublicationTransfer::Move
                    && let AstExpr::Name(argument_path) = argument
                    && let Some(root) = argument_path.first()
                    && let Some(binding) = scope.get(root)
                {
                    if argument_path.len() != 1 {
                        return Err(Diagnostic::new(
                            "E0883",
                            format!(
                                "structured task.spawn cannot move only `{}` from non-Copy binding `{root}`; publish the whole binding or construct a temporary value",
                                argument_path.join(".")
                            ),
                            path,
                            span.line,
                            span.column,
                            span.length,
                            &span.text,
                        ));
                    }
                    match binding.source {
                        BindingSource::Local | BindingSource::Param => {
                            ensure_publication_binding_available(path, span, scope, root)?;
                            if !moved_bindings.insert(root.clone()) {
                                return Err(Diagnostic::new(
                                    "E0881",
                                    format!(
                                        "binding `{root}` is consumed more than once by the same structured task.spawn publication"
                                    ),
                                    path,
                                    span.line,
                                    span.column,
                                    span.length,
                                    &span.text,
                                ));
                            }
                            lowered = ValueExpr::Call {
                                name: BUILTIN_TASK_PUBLICATION_MOVE_EXPR.to_string(),
                                args: vec![lowered],
                            };
                        }
                        BindingSource::EnumPayload { .. } => {
                            return Err(Diagnostic::new(
                                "E0883",
                                format!(
                                    "structured task.spawn cannot move enum payload binding `{root}` in the current P3-A slice; construct an owned temporary first"
                                ),
                                path,
                                span.line,
                                span.column,
                                span.length,
                                &span.text,
                            ));
                        }
                        BindingSource::Const => {}
                        BindingSource::FunctionEffect { .. }
                        | BindingSource::TaskScope
                        | BindingSource::PublicationMove { .. } => {
                            unreachable!("internal bindings cannot be publication operands")
                        }
                    }
                }
                lowered_args.push(lowered);
            }
            Ok((
                ValueType::Struct("Task".to_string(), vec![signature.return_type.clone()]),
                ValueExpr::Call {
                    name: format!("{BUILTIN_TASK_STRUCTURED_SPAWN_PREFIX}{target_name}"),
                    args: lowered_args,
                },
            ))
        }
        "yield_now" => {
            if !args.is_empty() {
                return Err(task_arity_diagnostic(
                    path,
                    span,
                    "task.yield_now",
                    0,
                    args.len(),
                ));
            }
            if !current_function_is_suspend(scope) {
                return Err(Diagnostic::new(
                    "E0870",
                    "synchronous function cannot call suspend function `task.yield_now`; mark the caller `suspend`",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            }
            Ok((
                ValueType::Void,
                ValueExpr::Call {
                    name: BUILTIN_TASK_YIELD_EXPR.to_string(),
                    args: Vec::new(),
                },
            ))
        }
        "check_cancelled" => {
            if !args.is_empty() {
                return Err(task_arity_diagnostic(
                    path,
                    span,
                    "task.check_cancelled",
                    0,
                    args.len(),
                ));
            }
            if !current_function_is_suspend(scope) {
                return Err(Diagnostic::new(
                    "E0870",
                    "synchronous function cannot call `task.check_cancelled`; mark the caller `suspend`",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            }
            Ok((
                ValueType::Void,
                ValueExpr::Call {
                    name: BUILTIN_TASK_CHECK_CANCELLED_EXPR.to_string(),
                    args: Vec::new(),
                },
            ))
        }
        "sleep" => {
            let [duration_arg] = args else {
                return Err(task_arity_diagnostic(
                    path,
                    span,
                    "task.sleep",
                    1,
                    args.len(),
                ));
            };
            if !current_function_is_suspend(scope) {
                return Err(Diagnostic::new(
                    "E0870",
                    "synchronous function cannot call suspend function `task.sleep`; mark the caller `suspend`",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            }
            let duration_type = ValueType::Struct("Duration".to_string(), Vec::new());
            let (actual, duration) = lower_value_expr_with_expected(
                path,
                duration_arg,
                scope,
                imports,
                signatures,
                structs,
                enums,
                Some(&duration_type),
                span,
            )?;
            require_task_type(path, span, "task.sleep duration", &duration_type, &actual)?;
            Ok((
                ValueType::Enum(
                    "Result".to_string(),
                    vec![ValueType::Void, error_type.clone()],
                ),
                ValueExpr::Call {
                    name: BUILTIN_TASK_SLEEP_EXPR.to_string(),
                    args: vec![duration],
                },
            ))
        }
        "spawn" => {
            let [worker_arg, input_arg] = args else {
                return Err(task_arity_diagnostic(
                    path,
                    span,
                    "task.spawn",
                    2,
                    args.len(),
                ));
            };
            let worker = lower_task_worker(path, worker_arg, scope, signatures, span)?;
            let (input_type, input) = lower_value_expr(
                path, input_arg, scope, imports, signatures, structs, enums, span,
            )?;
            require_task_type(
                path,
                span,
                "task.spawn input",
                &ValueType::String,
                &input_type,
            )?;
            Ok((
                ValueType::Enum(
                    "Result".to_string(),
                    vec![task_type.clone(), error_type.clone()],
                ),
                ValueExpr::Call {
                    name: BUILTIN_TASK_SPAWN_EXPR.to_string(),
                    args: vec![worker, input],
                },
            ))
        }
        "is_cancelled" => {
            let [context_arg] = args else {
                return Err(task_arity_diagnostic(
                    path,
                    span,
                    "task.is_cancelled",
                    1,
                    args.len(),
                ));
            };
            let (actual, context) = lower_value_expr(
                path,
                context_arg,
                scope,
                imports,
                signatures,
                structs,
                enums,
                span,
            )?;
            require_task_type(
                path,
                span,
                "task.is_cancelled context",
                &context_type,
                &actual,
            )?;
            Ok((
                ValueType::Bool,
                ValueExpr::Call {
                    name: BUILTIN_TASK_IS_CANCELLED_EXPR.to_string(),
                    args: vec![context],
                },
            ))
        }
        "join" => {
            if args.len() == 1 {
                if !current_function_has_task_scope(scope) {
                    return Err(Diagnostic::new(
                        "E0871",
                        "structured task.join is only allowed inside task.scope or task.deadline",
                        path,
                        span.line,
                        span.column,
                        span.length,
                        &span.text,
                    ));
                }
                let (actual_task, task_value) = lower_value_expr(
                    path, &args[0], scope, imports, signatures, structs, enums, span,
                )?;
                let ValueType::Struct(task_name, task_args) = actual_task else {
                    return Err(type_mismatch(
                        path,
                        span,
                        "task.join expects a scope-owned Task<T> handle",
                    ));
                };
                let [result_type] = task_args.as_slice() else {
                    return Err(type_mismatch(
                        path,
                        span,
                        "task.join expects a scope-owned Task<T> handle",
                    ));
                };
                if task_name != "Task" {
                    return Err(type_mismatch(
                        path,
                        span,
                        "task.join expects a scope-owned Task<T> handle",
                    ));
                }
                return Ok((
                    ValueType::Enum(
                        "Result".to_string(),
                        vec![result_type.clone(), error_type.clone()],
                    ),
                    ValueExpr::Call {
                        name: BUILTIN_TASK_STRUCTURED_JOIN_EXPR.to_string(),
                        args: vec![task_value],
                    },
                ));
            }
            let [task_arg, timeout_arg] = args else {
                return Err(task_arity_diagnostic(
                    path,
                    span,
                    "task.join",
                    2,
                    args.len(),
                ));
            };
            let (actual_task, task_value) = lower_value_expr(
                path, task_arg, scope, imports, signatures, structs, enums, span,
            )?;
            require_task_type(path, span, "task.join task", &task_type, &actual_task)?;
            let (actual_timeout, timeout) = lower_value_expr_with_expected(
                path,
                timeout_arg,
                scope,
                imports,
                signatures,
                structs,
                enums,
                Some(&ValueType::U64),
                span,
            )?;
            require_task_type(
                path,
                span,
                "task.join timeout",
                &ValueType::U64,
                &actual_timeout,
            )?;
            Ok((
                ValueType::Enum("Result".to_string(), vec![join_type, error_type.clone()]),
                ValueExpr::Call {
                    name: BUILTIN_TASK_JOIN_EXPR.to_string(),
                    args: vec![task_value, timeout],
                },
            ))
        }
        "cancel" => {
            let [task_arg] = args else {
                return Err(task_arity_diagnostic(
                    path,
                    span,
                    "task.cancel",
                    1,
                    args.len(),
                ));
            };
            let (actual, task_value) = lower_value_expr(
                path, task_arg, scope, imports, signatures, structs, enums, span,
            )?;
            if matches!(
                &actual,
                ValueType::Struct(task_name, task_args)
                    if task_name == "Task" && task_args.len() == 1
            ) {
                if !current_function_has_task_scope(scope) {
                    return Err(Diagnostic::new(
                        "E0871",
                        "structured task.cancel is only allowed inside task.scope or task.deadline",
                        path,
                        span.line,
                        span.column,
                        span.length,
                        &span.text,
                    ));
                }
                return Ok((
                    ValueType::Enum("Result".to_string(), vec![ValueType::Void, error_type]),
                    ValueExpr::Call {
                        name: BUILTIN_TASK_STRUCTURED_CANCEL_JOIN_EXPR.to_string(),
                        args: vec![task_value],
                    },
                ));
            }
            require_task_type(path, span, "task.cancel task", &task_type, &actual)?;
            Ok((
                ValueType::Enum("Result".to_string(), vec![ValueType::Void, error_type]),
                ValueExpr::Call {
                    name: BUILTIN_TASK_CANCEL_EXPR.to_string(),
                    args: vec![task_value],
                },
            ))
        }
        "close" => {
            let [task_arg] = args else {
                return Err(task_arity_diagnostic(
                    path,
                    span,
                    "task.close",
                    1,
                    args.len(),
                ));
            };
            let (actual, task_value) = lower_value_expr(
                path, task_arg, scope, imports, signatures, structs, enums, span,
            )?;
            require_task_type(path, span, "task.close task", &task_type, &actual)?;
            Ok((
                ValueType::Enum("Result".to_string(), vec![ValueType::Void, error_type]),
                ValueExpr::Call {
                    name: BUILTIN_TASK_CLOSE_EXPR.to_string(),
                    args: vec![task_value],
                },
            ))
        }
        _ => unreachable!("task builtin matcher and lowering must stay aligned"),
    }
}

fn lower_task_worker(
    path: &Path,
    worker: &AstExpr,
    scope: &HashMap<String, Binding>,
    signatures: &HashMap<String, FunctionSignature>,
    span: &Span,
) -> Result<ValueExpr, Diagnostic> {
    let AstExpr::Name(worker_path) = worker else {
        return Err(task_worker_diagnostic(
            path,
            span,
            "workers must be non-capturing top-level function names",
        ));
    };
    let [name] = worker_path.as_slice() else {
        return Err(task_worker_diagnostic(
            path,
            span,
            "workers must be unqualified top-level function names",
        ));
    };

    if let Some(binding) = scope.get(name) {
        if matches!(binding.value_type, ValueType::TaskCallback { .. }) {
            return Ok(binding_value_expr(name, binding));
        }
        return Err(task_worker_diagnostic(
            path,
            span,
            &format!("local `{name}` is not a task worker"),
        ));
    }

    let Some(signature) = signatures.get(name) else {
        return Err(task_worker_diagnostic(
            path,
            span,
            &format!("unknown worker function `{name}`"),
        ));
    };
    let expected_params = [
        ValueType::Struct("TaskContext".to_string(), Vec::new()),
        ValueType::String,
    ];
    let matches = signature.extern_symbol.is_none()
        && !signature.is_suspend
        && signature.type_params.is_empty()
        && signature.params.iter().all(|param| !param.mutable)
        && signature
            .params
            .iter()
            .map(|param| &param.value_type)
            .eq(expected_params.iter())
        && signature.return_type == ValueType::String;
    if !matches {
        return Err(task_worker_diagnostic(
            path,
            span,
            &format!("function `{name}` must have signature `fn(TaskContext, string) -> string`"),
        ));
    }
    Ok(ValueExpr::FunctionRef(name.clone()))
}

fn require_task_type(
    path: &Path,
    span: &Span,
    label: &str,
    expected: &ValueType,
    actual: &ValueType,
) -> Result<(), Diagnostic> {
    if actual == expected {
        Ok(())
    } else {
        Err(type_mismatch_expected_found(
            path,
            span,
            format!("invalid {label}"),
            expected,
            actual,
        ))
    }
}

fn task_arity_diagnostic(
    path: &Path,
    span: &Span,
    callable: &str,
    expected: usize,
    actual: usize,
) -> Diagnostic {
    Diagnostic::new(
        "E0407",
        format!("{callable} expects {expected} argument(s), got {actual}"),
        path,
        span.line,
        span.column,
        span.length,
        &span.text,
    )
}

fn task_worker_diagnostic(path: &Path, span: &Span, message: &str) -> Diagnostic {
    Diagnostic::new(
        "E0820",
        message,
        path,
        span.line,
        span.column,
        span.length,
        &span.text,
    )
}

fn structured_task_target_diagnostic(path: &Path, span: &Span, message: &str) -> Diagnostic {
    Diagnostic::new(
        "E0875",
        message,
        path,
        span.line,
        span.column,
        span.length,
        &span.text,
    )
}
