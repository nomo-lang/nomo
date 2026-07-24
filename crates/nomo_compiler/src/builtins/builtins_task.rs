use super::*;

pub(super) fn is_task_builtin_call(callee: &[String]) -> bool {
    matches!(
        callee,
        [module, name]
            if module == "task"
                && matches!(
                    name.as_str(),
                    "spawn" | "is_cancelled" | "join" | "cancel" | "close"
                )
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
        "cancel" | "close" => {
            let [task_arg] = args else {
                return Err(task_arity_diagnostic(
                    path,
                    span,
                    &format!("task.{name}"),
                    1,
                    args.len(),
                ));
            };
            let (actual, task_value) = lower_value_expr(
                path, task_arg, scope, imports, signatures, structs, enums, span,
            )?;
            require_task_type(
                path,
                span,
                &format!("task.{name} task"),
                &task_type,
                &actual,
            )?;
            Ok((
                ValueType::Enum("Result".to_string(), vec![ValueType::Void, error_type]),
                ValueExpr::Call {
                    name: if name == "cancel" {
                        BUILTIN_TASK_CANCEL_EXPR
                    } else {
                        BUILTIN_TASK_CLOSE_EXPR
                    }
                    .to_string(),
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
