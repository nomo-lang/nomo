use super::*;
use crate::validation_tasks::{statement_span, visit_statement_expressions};

pub(super) fn validate_suspend_blocking_calls(
    path: &Path,
    ast: &SourceFile,
    imports: &[String],
) -> Result<(), Diagnostic> {
    let functions = ast
        .functions
        .iter()
        .map(|function| (function.name.as_str(), function))
        .collect::<HashMap<_, _>>();

    for function in ast.functions.iter().filter(|function| function.is_suspend) {
        let mut visiting = Vec::new();
        validate_suspend_blocking_function(path, function, imports, &functions, &mut visiting)?;
    }
    for method in ast
        .impls
        .iter()
        .flat_map(|impl_block| impl_block.methods.iter())
        .filter(|method| method.is_suspend)
    {
        let mut visiting = Vec::new();
        validate_suspend_blocking_function(path, method, imports, &functions, &mut visiting)?;
    }

    Ok(())
}

fn validate_suspend_blocking_function<'a>(
    path: &Path,
    function: &'a AstFunction,
    imports: &[String],
    functions: &HashMap<&str, &'a AstFunction>,
    visiting: &mut Vec<&'a str>,
) -> Result<(), Diagnostic> {
    if visiting.contains(&function.name.as_str()) {
        return Ok(());
    }
    visiting.push(function.name.as_str());
    for statement in &function.body {
        let span = statement_span(statement);
        visit_statement_expressions(statement, &mut |expression| {
            let AstExpr::Call { callee, .. } = expression else {
                return Ok(());
            };
            if let Some(operation) = blocking_sleep_operation(callee.as_slice(), imports) {
                let mut call_path = visiting.join(" -> ");
                call_path.push_str(" -> ");
                call_path.push_str(operation);
                return Err(Diagnostic::new(
                    "E0891",
                    format!(
                        "suspend function reaches a blocking operation via {call_path}; use nonblocking `task.sleep` and handle its result, or move the whole operation to the bounded blocking pool"
                    ),
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            }
            let Some(name) = callee.last() else {
                return Ok(());
            };
            let Some(callee_function) = functions.get(name.as_str()).copied() else {
                return Ok(());
            };
            validate_suspend_blocking_function(path, callee_function, imports, functions, visiting)
        })?;
    }
    visiting.pop();
    Ok(())
}

fn blocking_sleep_operation(callee: &[String], imports: &[String]) -> Option<&'static str> {
    let resolved = if let [name] = callee
        && let Some(qualified) = resolve_specific_value_builtin(name, imports)
    {
        qualified
    } else {
        callee.to_vec()
    };
    match resolved.as_slice() {
        [module, operation]
            if module == "time"
                && matches!(operation.as_str(), "sleep" | "sleep_millis")
                && imports
                    .iter()
                    .any(|item| item == "std.time" || item == &format!("std.time.{operation}")) =>
        {
            Some(if operation == "sleep" {
                "time.sleep"
            } else {
                "time.sleep_millis"
            })
        }
        [root, module, operation]
            if root == "std"
                && module == "time"
                && matches!(operation.as_str(), "sleep" | "sleep_millis") =>
        {
            Some(if operation == "sleep" {
                "time.sleep"
            } else {
                "time.sleep_millis"
            })
        }
        _ => None,
    }
}

pub(super) fn validate_p1_yield_function(
    path: &Path,
    function: &AstFunction,
    imports: &[String],
) -> Result<(), Diagnostic> {
    let has_runtime_suspend = function
        .body
        .iter()
        .any(|statement| ast_statement_contains_runtime_suspend(statement, imports));
    if !has_runtime_suspend || !function.is_suspend {
        return Ok(());
    }

    if function.package.as_slice() == ["std", "task"]
        && matches!(function.name.as_str(), "yield_now" | "sleep")
    {
        return Ok(());
    }

    let suspending_functions = HashSet::from([function.name.clone()]);
    validate_p1_suspend_function_shape(path, function, imports, &suspending_functions)
}

fn validate_p1_suspend_function_shape(
    path: &Path,
    function: &AstFunction,
    imports: &[String],
    suspending_functions: &HashSet<String>,
) -> Result<(), Diagnostic> {
    let returns_void =
        function.return_type.path.as_slice() == ["void"] && function.return_type.args.is_empty();
    let supported_signature = function.type_params.is_empty()
        && function.params.iter().all(|parameter| !parameter.mutable)
        && (function.name != "main" || returns_void);
    let supported_body =
        function
            .body
            .iter()
            .enumerate()
            .all(|(index, statement)| match statement {
                Stmt::Expr {
                    expr: AstExpr::Panic { message },
                    ..
                } => {
                    !ast_expr_contains_suspension(message, imports, suspending_functions)
                        && !ast_expr_contains_frame_exit(message)
                }
                Stmt::Expr { expr, .. } => {
                    ((ast_expr_is_direct_suspension(expr, imports, suspending_functions)
                        && !ast_expr_is_direct_sleep(expr, imports, suspending_functions))
                        || !ast_expr_contains_suspension(expr, imports, suspending_functions))
                        && !ast_expr_contains_frame_exit(expr)
                }
                Stmt::Let { mutable, value, .. } => {
                    !mutable
                        && (ast_expr_is_direct_suspension(value, imports, suspending_functions)
                            || !ast_expr_contains_suspension(value, imports, suspending_functions))
                        && !ast_expr_contains_frame_exit(value)
                }
                Stmt::Return { value, .. } => {
                    index + 1 == function.body.len()
                        && value.as_ref().is_none_or(|value| {
                            !ast_expr_contains_suspension(value, imports, suspending_functions)
                                && !ast_expr_contains_frame_exit(value)
                        })
                }
                Stmt::TaskScope { body, .. } => {
                    let has_return = body
                        .iter()
                        .any(|statement| matches!(statement, Stmt::Return { .. }));
                    (!has_return || index + 1 == function.body.len())
                        && body
                            .iter()
                            .enumerate()
                            .all(|(scope_index, statement)| match statement {
                                Stmt::Let { mutable, value, .. } => {
                                    if let AstExpr::Question { expr } = value {
                                        !mutable
                                            && (ast_expr_is_structured_join(expr)
                                                || !ast_expr_contains_suspension(
                                                    expr,
                                                    imports,
                                                    suspending_functions,
                                                ))
                                            && !ast_expr_contains_frame_exit(expr)
                                    } else {
                                        !mutable
                                            && (ast_expr_is_structured_spawn(value)
                                                || ast_expr_is_structured_join(value)
                                                || ast_expr_is_structured_cancel(value)
                                                || !ast_expr_contains_suspension(
                                                    value,
                                                    imports,
                                                    suspending_functions,
                                                ))
                                            && !ast_expr_contains_frame_exit(value)
                                    }
                                }
                                Stmt::Expr {
                                    expr: AstExpr::Panic { message },
                                    ..
                                } => {
                                    !ast_expr_contains_suspension(
                                        message,
                                        imports,
                                        suspending_functions,
                                    ) && !ast_expr_contains_frame_exit(message)
                                }
                                Stmt::Expr { expr, .. } => {
                                    !ast_expr_contains_suspension(
                                        expr,
                                        imports,
                                        suspending_functions,
                                    ) && !ast_expr_contains_frame_exit(expr)
                                }
                                Stmt::Return { value, .. } => {
                                    scope_index + 1 == body.len()
                                        && value.as_ref().is_none_or(|value| {
                                            !ast_expr_contains_suspension(
                                                value,
                                                imports,
                                                suspending_functions,
                                            ) && !ast_expr_contains_frame_exit(value)
                                        })
                                }
                                _ => false,
                            })
                }
                _ => false,
            });

    if supported_signature && supported_body {
        return Ok(());
    }

    Err(Diagnostic::new(
        "E0876",
        "the current nested-frame slice supports immutable top-level locals, frame-safe immutable parameters/results, standalone void suspend calls, `let`-bound value suspend calls, `let`-bound `task.sleep(Duration)` results, normal task.scope cancellation cleanup, direct immutable `?` bindings inside task.scope, direct explicit panic statements, and a final task.scope return that cancels unjoined children in non-generic `suspend fn` functions; async `main` still returns `void`, while mutable parameters/locals, recursive suspension, nested control flow, `?` or panic in other expression positions, and non-final early control transfers require a later slice",
        path,
        function.span.line,
        function.span.column,
        function.span.length,
        &function.span.text,
    ))
}

pub(super) fn validate_p1_suspending_functions(
    path: &Path,
    functions: &[AstFunction],
    imports: &[String],
) -> Result<(), Diagnostic> {
    let mut suspending = functions
        .iter()
        .filter(|function| {
            function.is_suspend
                && function
                    .body
                    .iter()
                    .any(|statement| ast_statement_contains_runtime_suspend(statement, imports))
        })
        .map(|function| function.name.clone())
        .collect::<HashSet<_>>();
    for function in functions {
        for statement in &function.body {
            let Stmt::TaskScope { body, .. } = statement else {
                continue;
            };
            for statement in body {
                let Stmt::Let { value, .. } = statement else {
                    continue;
                };
                if let Some(target) = ast_expr_structured_spawn_target(value) {
                    suspending.insert(target.to_string());
                }
            }
        }
    }

    loop {
        let discovered = functions
            .iter()
            .filter(|function| function.is_suspend && !suspending.contains(&function.name))
            .filter(|function| {
                function
                    .body
                    .iter()
                    .any(|statement| ast_statement_contains_call_to(statement, &suspending))
            })
            .map(|function| function.name.clone())
            .collect::<Vec<_>>();
        if discovered.is_empty() {
            break;
        }
        suspending.extend(discovered);
    }

    let function_map = functions
        .iter()
        .map(|function| (function.name.as_str(), function))
        .collect::<HashMap<_, _>>();
    let mut visited = HashSet::new();
    let mut visiting = Vec::new();
    for function in functions
        .iter()
        .filter(|function| suspending.contains(&function.name))
    {
        validate_p1_suspend_acyclic(
            path,
            function,
            &function_map,
            &suspending,
            &mut visited,
            &mut visiting,
        )?;
        validate_p1_suspend_call_bindings(path, function, &function_map, &suspending)?;
        validate_p1_suspend_function_shape(path, function, imports, &suspending)?;
    }

    Ok(())
}

fn validate_p1_suspend_call_bindings(
    path: &Path,
    function: &AstFunction,
    functions: &HashMap<&str, &AstFunction>,
    suspending: &HashSet<String>,
) -> Result<(), Diagnostic> {
    for statement in &function.body {
        let Stmt::Expr {
            expr: AstExpr::Call { callee, .. },
            span,
        } = statement
        else {
            continue;
        };
        let Some(name) = callee.last() else {
            continue;
        };
        if !suspending.contains(name) {
            continue;
        }
        let Some(callee_function) = functions.get(name.as_str()) else {
            continue;
        };
        let returns_void = callee_function.return_type.path.as_slice() == ["void"]
            && callee_function.return_type.args.is_empty();
        if returns_void {
            continue;
        }
        return Err(Diagnostic::new(
            "E0876",
            format!(
                "suspend call to `{name}` returns a value; bind the result with an immutable `let`"
            ),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    }
    Ok(())
}

fn validate_p1_suspend_acyclic(
    path: &Path,
    function: &AstFunction,
    functions: &HashMap<&str, &AstFunction>,
    suspending: &HashSet<String>,
    visited: &mut HashSet<String>,
    visiting: &mut Vec<String>,
) -> Result<(), Diagnostic> {
    if visited.contains(&function.name) {
        return Ok(());
    }
    if let Some(start) = visiting.iter().position(|name| name == &function.name) {
        let mut cycle = visiting[start..].to_vec();
        cycle.push(function.name.clone());
        return Err(Diagnostic::new(
            "E0876",
            format!(
                "recursive suspend call graph `{}` needs dynamically sized frames; break the cycle for the current nested-frame slice",
                cycle.join(" -> ")
            ),
            path,
            function.span.line,
            function.span.column,
            function.span.length,
            &function.span.text,
        ));
    }

    visiting.push(function.name.clone());
    let mut callees = functions
        .values()
        .copied()
        .filter(|callee| suspending.contains(&callee.name))
        .filter(|callee| {
            function
                .body
                .iter()
                .any(|statement| ast_statement_contains_named_call(statement, &callee.name))
        })
        .collect::<Vec<_>>();
    callees.sort_by(|left, right| left.name.cmp(&right.name));
    for callee_function in callees {
        validate_p1_suspend_acyclic(
            path,
            callee_function,
            functions,
            suspending,
            visited,
            visiting,
        )?;
    }
    visiting.pop();
    visited.insert(function.name.clone());
    Ok(())
}

pub(super) fn validate_p1_yield_ir_function(
    path: &Path,
    source: &AstFunction,
    body: &[Statement],
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
) -> Result<(), Diagnostic> {
    let has_runtime_suspend = body.iter().any(ir_statement_contains_runtime_suspend);
    if !has_runtime_suspend {
        return Ok(());
    }

    let unsupported = body.iter().find_map(|statement| match statement {
        Statement::Let {
            name,
            value_type,
            initializer,
        } if !p1_frame_value_type_supported(value_type, structs, enums, &mut Vec::new()) => {
            (!ir_expr_is_structured_spawn(initializer)).then_some((name, value_type))
        }
        _ => None,
    });
    if let Some((name, value_type)) = unsupported {
        return Err(Diagnostic::new(
            "E0876",
            format!(
                "local `{name}` has type `{}` without a P1 frame move/drop implementation; keep it within a non-suspending function",
                value_type.name()
            ),
            path,
            source.span.line,
            source.span.column,
            source.span.length,
            &source.span.text,
        ));
    }
    Ok(())
}

pub(super) fn validate_p1_suspend_ir_program(
    path: &Path,
    functions: &[Function],
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
) -> Result<(), Diagnostic> {
    let mut suspending = functions
        .iter()
        .filter(|function| {
            function.is_suspend
                && function
                    .body
                    .iter()
                    .any(ir_statement_contains_runtime_suspend)
        })
        .map(|function| function.name.clone())
        .collect::<HashSet<_>>();
    for function in functions {
        for statement in &function.body {
            if let Some(target) = ir_statement_structured_spawn_target(statement) {
                suspending.insert(target.to_string());
            }
        }
    }

    loop {
        let discovered = functions
            .iter()
            .filter(|function| function.is_suspend && !suspending.contains(&function.name))
            .filter(|function| {
                function
                    .body
                    .iter()
                    .any(|statement| ir_statement_calls_any(statement, &suspending))
            })
            .map(|function| function.name.clone())
            .collect::<Vec<_>>();
        if discovered.is_empty() {
            break;
        }
        suspending.extend(discovered);
    }

    for function in functions
        .iter()
        .filter(|function| suspending.contains(&function.name))
    {
        if let Some(parameter) = function.params.iter().find(|parameter| {
            parameter.mutable
                || !p1_frame_value_type_supported(
                    &parameter.value_type,
                    structs,
                    enums,
                    &mut Vec::new(),
                )
        }) {
            return Err(Diagnostic::new(
                "E0876",
                format!(
                    "parameter `{}` in suspend function `{}` must be immutable and frame-safe for the current call ABI slice",
                    parameter.name, function.name
                ),
                path,
                1,
                1,
                1,
                "",
            ));
        }
        if function.name == "main" && function.return_type != ValueType::Void {
            return Err(Diagnostic::new(
                "E0876",
                "async `main` must return `void` until the root result-to-exit-status ABI lands",
                path,
                1,
                1,
                1,
                "",
            ));
        }
        if function.return_type != ValueType::Void
            && !p1_frame_value_type_supported(
                &function.return_type,
                structs,
                enums,
                &mut Vec::new(),
            )
        {
            return Err(Diagnostic::new(
                "E0876",
                format!(
                    "result of suspend function `{}` has type `{}` without a frame-safe result-slot implementation",
                    function.name,
                    function.return_type.name()
                ),
                path,
                1,
                1,
                1,
                "",
            ));
        }
        if let Some((name, value_type)) = function.body.iter().find_map(|statement| match statement
        {
            Statement::Let {
                name,
                value_type,
                initializer,
            } if !p1_frame_value_type_supported(value_type, structs, enums, &mut Vec::new()) => {
                (!ir_expr_is_structured_spawn(initializer)).then_some((name, value_type))
            }
            Statement::QuestionLet {
                name, value_type, ..
            } if !p1_frame_value_type_supported(value_type, structs, enums, &mut Vec::new()) => {
                Some((name, value_type))
            }
            _ => None,
        }) {
            return Err(Diagnostic::new(
                "E0876",
                format!(
                    "local `{name}` in suspend function `{}` has type `{}` without a P1 nested-frame move/drop implementation; keep it within a non-suspending function",
                    function.name,
                    value_type.name()
                ),
                path,
                1,
                1,
                1,
                "",
            ));
        }
        for statement in &function.body {
            let Some((callee_name, binding)) =
                ir_statement_direct_suspend_call(statement, &suspending)
            else {
                continue;
            };
            let Some(callee) = functions
                .iter()
                .find(|candidate| candidate.name == callee_name)
            else {
                continue;
            };
            if binding.is_none() && callee.return_type != ValueType::Void {
                return Err(Diagnostic::new(
                    "E0876",
                    format!(
                        "suspend call to `{callee_name}` returns `{}`; bind the result with an immutable `let`",
                        callee.return_type.name()
                    ),
                    path,
                    1,
                    1,
                    1,
                    "",
                ));
            }
        }
    }
    Ok(())
}

fn ir_statement_calls_any(statement: &Statement, names: &HashSet<String>) -> bool {
    matches!(
        statement,
        Statement::Expr(ValueExpr::Call { name, .. })
            | Statement::Let {
                initializer: ValueExpr::Call { name, .. },
                ..
            } if names.contains(name)
    )
}

fn ir_statement_direct_suspend_call<'a>(
    statement: &'a Statement,
    names: &HashSet<String>,
) -> Option<(&'a str, Option<(&'a str, &'a ValueType)>)> {
    match statement {
        Statement::Expr(ValueExpr::Call { name, .. }) if names.contains(name) => Some((name, None)),
        Statement::Let {
            name: binding,
            value_type,
            initializer: ValueExpr::Call { name, .. },
        } if names.contains(name) => Some((name, Some((binding, value_type)))),
        _ => None,
    }
}

fn p1_frame_value_type_supported(
    value_type: &ValueType,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    visiting: &mut Vec<ValueType>,
) -> bool {
    match value_type {
        ValueType::String
        | ValueType::CString
        | ValueType::Int
        | ValueType::I32
        | ValueType::U32
        | ValueType::U64
        | ValueType::Float
        | ValueType::Char
        | ValueType::Bool => true,
        ValueType::Array(element) => {
            p1_frame_value_type_supported(element, structs, enums, visiting)
        }
        ValueType::Struct(name, args) => {
            if visiting.contains(value_type) {
                return true;
            }
            let Some(struct_type) = structs.get(name) else {
                return false;
            };
            if p1_frame_resource_struct(struct_type)
                || struct_type.type_params.len() != args.len()
                || !args
                    .iter()
                    .all(|arg| p1_frame_value_type_supported(arg, structs, enums, visiting))
            {
                return false;
            }
            visiting.push(value_type.clone());
            let supported = struct_type.fields.iter().all(|field| {
                let field_type =
                    substitute_type_params(&field.value_type, &struct_type.type_params, args);
                p1_frame_value_type_supported(&field_type, structs, enums, visiting)
            });
            visiting.pop();
            supported
        }
        ValueType::Enum(name, args) => {
            if visiting.contains(value_type) {
                return true;
            }
            let Some(enum_type) = enums.get(name) else {
                return false;
            };
            if enum_type.type_params.len() != args.len()
                || !args.iter().all(|arg| {
                    *arg == ValueType::Void
                        || p1_frame_value_type_supported(arg, structs, enums, visiting)
                })
            {
                return false;
            }
            visiting.push(value_type.clone());
            let supported = enum_type.variants.iter().all(|variant| {
                variant.payload.as_ref().is_none_or(|payload| {
                    let payload_type =
                        substitute_type_params(payload, &enum_type.type_params, args);
                    payload_type == ValueType::Void
                        || p1_frame_value_type_supported(&payload_type, structs, enums, visiting)
                })
            });
            visiting.pop();
            supported
        }
        ValueType::Opaque
        | ValueType::OpaqueHandle(_)
        | ValueType::OwnedHandle(_)
        | ValueType::BorrowedHandle(_)
        | ValueType::Nullable(_)
        | ValueType::ExternCallback { .. }
        | ValueType::TaskCallback { .. }
        | ValueType::TypeParam(_)
        | ValueType::Void
        | ValueType::Never => false,
    }
}

fn p1_frame_resource_struct(struct_type: &StructType) -> bool {
    is_opaque_handle_struct(struct_type)
        || matches!(
            (struct_type.package.as_str(), struct_type.name.as_str()),
            ("std.fs", "File")
                | ("std.net", "TcpStream" | "TcpListener" | "UdpSocket")
                | ("std.http", "HttpServer" | "HttpExchange" | "HttpStream")
                | ("std.process", "ProcessChild")
                | ("std.task", "Task" | "TaskContext")
                | ("std.sqlite", "SqliteDatabase" | "SqliteQuery")
        )
}

fn ast_statement_contains_runtime_suspend(statement: &Stmt, imports: &[String]) -> bool {
    ast_statement_any_expr(statement, |candidate| {
        ast_expr_is_direct_yield(candidate, imports)
            || ast_expr_is_direct_sleep(candidate, imports, &HashSet::new())
            || ast_expr_is_structured_join(candidate)
            || ast_expr_is_structured_cancel(candidate)
    })
}

fn ast_statement_contains_call_to(statement: &Stmt, names: &HashSet<String>) -> bool {
    ast_statement_any_expr(statement, |candidate| {
        matches!(
            candidate,
            AstExpr::Call { callee, .. }
                if callee.last().is_some_and(|name| names.contains(name))
        )
    })
}

fn ast_statement_contains_named_call(statement: &Stmt, name: &str) -> bool {
    ast_statement_any_expr(statement, |candidate| {
        matches!(
            candidate,
            AstExpr::Call { callee, .. }
                if callee.last().is_some_and(|callee| callee == name)
        )
    })
}

fn ast_statement_any_expr<P>(statement: &Stmt, predicate: P) -> bool
where
    P: Fn(&AstExpr) -> bool + Copy,
{
    match statement {
        Stmt::Let { value, .. }
        | Stmt::Assign { value, .. }
        | Stmt::Return {
            value: Some(value), ..
        }
        | Stmt::Expr { expr: value, .. } => ast_expr_any(value, predicate),
        Stmt::IndexAssign { indices, value, .. } => {
            indices.iter().any(|index| ast_expr_any(index, predicate))
                || ast_expr_any(value, predicate)
        }
        Stmt::LetElse {
            value, else_body, ..
        } => {
            ast_expr_any(value, predicate)
                || else_body
                    .iter()
                    .any(|statement| ast_statement_any_expr(statement, predicate))
        }
        Stmt::IfLet {
            value,
            body,
            else_body,
            ..
        } => {
            ast_expr_any(value, predicate)
                || body
                    .iter()
                    .any(|statement| ast_statement_any_expr(statement, predicate))
                || else_body.as_ref().is_some_and(|body| {
                    body.iter()
                        .any(|statement| ast_statement_any_expr(statement, predicate))
                })
        }
        Stmt::Match { value, arms, .. } => {
            ast_expr_any(value, predicate)
                || arms.iter().any(|arm| {
                    arm.body
                        .iter()
                        .any(|statement| ast_statement_any_expr(statement, predicate))
                })
        }
        Stmt::For { variant, .. } => match variant {
            ForVariant::Infinite { body } => body
                .iter()
                .any(|statement| ast_statement_any_expr(statement, predicate)),
            ForVariant::While { condition, body } => {
                ast_expr_any(condition, predicate)
                    || body
                        .iter()
                        .any(|statement| ast_statement_any_expr(statement, predicate))
            }
            ForVariant::CStyle {
                initializer,
                condition,
                update,
                body,
                ..
            } => {
                ast_expr_any(initializer, predicate)
                    || ast_expr_any(condition, predicate)
                    || ast_statement_any_expr(update, predicate)
                    || body
                        .iter()
                        .any(|statement| ast_statement_any_expr(statement, predicate))
            }
            ForVariant::Iterate { iterable, body, .. } => {
                ast_expr_any(iterable, predicate)
                    || body
                        .iter()
                        .any(|statement| ast_statement_any_expr(statement, predicate))
            }
        },
        Stmt::Defer { stmt, .. } => ast_statement_any_expr(stmt, predicate),
        Stmt::TaskScope { body, .. } | Stmt::Unsafe { body, .. } => body
            .iter()
            .any(|statement| ast_statement_any_expr(statement, predicate)),
        Stmt::Postfix { .. }
        | Stmt::Return { value: None, .. }
        | Stmt::Break { .. }
        | Stmt::Continue { .. } => false,
    }
}

fn ast_expr_contains_suspension(
    expr: &AstExpr,
    imports: &[String],
    suspending_functions: &HashSet<String>,
) -> bool {
    ast_expr_any(expr, |candidate| {
        ast_expr_is_direct_suspension(candidate, imports, suspending_functions)
    })
}

fn ast_expr_is_direct_suspension(
    expr: &AstExpr,
    imports: &[String],
    suspending_functions: &HashSet<String>,
) -> bool {
    if ast_expr_is_direct_yield(expr, imports) {
        return true;
    }
    if ast_expr_is_direct_sleep(expr, imports, suspending_functions) {
        return true;
    }
    let AstExpr::Call {
        callee,
        type_args,
        args,
    } = expr
    else {
        return false;
    };
    type_args.is_empty()
        && args.iter().all(|argument| {
            !ast_expr_contains_suspension(argument, imports, suspending_functions)
                && !ast_expr_contains_frame_exit(argument)
        })
        && callee
            .last()
            .is_some_and(|name| suspending_functions.contains(name))
}

fn ast_expr_is_structured_spawn(expr: &AstExpr) -> bool {
    matches!(
        expr,
        AstExpr::Call {
            callee,
            type_args,
            args,
        } if callee.as_slice() == ["task", TASK_STRUCTURED_SPAWN_AST_NAME]
            && type_args.is_empty()
            && args.len() == 1
    )
}

fn ast_expr_structured_spawn_target(expr: &AstExpr) -> Option<&str> {
    let AstExpr::Call {
        callee,
        type_args,
        args,
    } = expr
    else {
        return None;
    };
    if callee.as_slice() != ["task", TASK_STRUCTURED_SPAWN_AST_NAME] || !type_args.is_empty() {
        return None;
    }
    let [AstExpr::Call { callee, .. }] = args.as_slice() else {
        return None;
    };
    let [target] = callee.as_slice() else {
        return None;
    };
    Some(target)
}

fn ast_expr_is_structured_join(expr: &AstExpr) -> bool {
    matches!(
        expr,
        AstExpr::Call {
            callee,
            type_args,
            args,
        } if callee.as_slice() == ["task", "join"]
            && type_args.is_empty()
            && args.len() == 1
    )
}

fn ast_expr_is_structured_cancel(expr: &AstExpr) -> bool {
    matches!(
        expr,
        AstExpr::Call {
            callee,
            type_args,
            args,
        } if callee.as_slice() == ["task", "cancel"]
            && type_args.is_empty()
            && args.len() == 1
    )
}

fn ast_expr_is_direct_sleep(
    expr: &AstExpr,
    imports: &[String],
    suspending_functions: &HashSet<String>,
) -> bool {
    let AstExpr::Call {
        callee,
        type_args,
        args,
    } = expr
    else {
        return false;
    };
    let direct = callee.as_slice() == ["task", "sleep"]
        || (callee.as_slice() == ["sleep"] && imports.iter().any(|item| item == "std.task.sleep"));
    direct
        && type_args.is_empty()
        && matches!(args.as_slice(), [duration]
            if !ast_expr_contains_suspension(duration, imports, suspending_functions)
                && !ast_expr_contains_frame_exit(duration))
}

fn ast_expr_contains_frame_exit(expr: &AstExpr) -> bool {
    ast_expr_any(expr, |candidate| {
        matches!(candidate, AstExpr::Panic { .. } | AstExpr::Question { .. })
    })
}

fn ast_expr_any<P>(expr: &AstExpr, predicate: P) -> bool
where
    P: Fn(&AstExpr) -> bool + Copy,
{
    if predicate(expr) {
        return true;
    }
    match expr {
        AstExpr::ArrayLiteral { elements } => elements
            .iter()
            .any(|element| ast_expr_any(element, predicate)),
        AstExpr::Index { base, index } => {
            ast_expr_any(base, predicate) || ast_expr_any(index, predicate)
        }
        AstExpr::Call { args, .. } => args
            .iter()
            .any(|argument| ast_expr_any(argument, predicate)),
        AstExpr::StructLiteral { fields, .. } => fields
            .iter()
            .any(|(_, value)| ast_expr_any(value, predicate)),
        AstExpr::Match { value, arms } => {
            ast_expr_any(value, predicate)
                || arms.iter().any(|arm| ast_expr_any(&arm.value, predicate))
        }
        AstExpr::If {
            condition,
            then_branch,
            else_branch,
        } => {
            ast_expr_any(condition, predicate)
                || ast_expr_any(then_branch, predicate)
                || ast_expr_any(else_branch, predicate)
        }
        AstExpr::Panic { message }
        | AstExpr::Question { expr: message }
        | AstExpr::Cast { expr: message, .. }
        | AstExpr::Unary { expr: message, .. } => ast_expr_any(message, predicate),
        AstExpr::Binary { left, right, .. } => {
            ast_expr_any(left, predicate) || ast_expr_any(right, predicate)
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

fn ast_expr_is_direct_yield(expr: &AstExpr, imports: &[String]) -> bool {
    let AstExpr::Call {
        callee,
        type_args,
        args,
    } = expr
    else {
        return false;
    };
    type_args.is_empty()
        && args.is_empty()
        && (callee.as_slice() == ["task", "yield_now"]
            || (callee.as_slice() == ["yield_now"]
                && imports.iter().any(|item| item == "std.task.yield_now")))
}

fn ir_statement_contains_runtime_suspend(statement: &Statement) -> bool {
    matches!(
        statement,
        Statement::Expr(ValueExpr::Call { name, args })
            | Statement::Let {
                initializer: ValueExpr::Call { name, args },
                ..
            }
            if (name == BUILTIN_TASK_YIELD_EXPR && args.is_empty())
                || (name == BUILTIN_TASK_SLEEP_EXPR && args.len() == 1)
                || (name == BUILTIN_TASK_STRUCTURED_JOIN_EXPR && args.len() == 1)
                || (name == BUILTIN_TASK_STRUCTURED_CANCEL_JOIN_EXPR && args.len() == 1)
    )
}

fn ir_expr_is_structured_spawn(expr: &ValueExpr) -> bool {
    matches!(
        expr,
        ValueExpr::Call { name, .. }
            if name.starts_with(BUILTIN_TASK_STRUCTURED_SPAWN_PREFIX)
    )
}

fn ir_statement_structured_spawn_target(statement: &Statement) -> Option<&str> {
    let Statement::Let {
        initializer: ValueExpr::Call { name, .. },
        ..
    } = statement
    else {
        return None;
    };
    name.strip_prefix(BUILTIN_TASK_STRUCTURED_SPAWN_PREFIX)
}
