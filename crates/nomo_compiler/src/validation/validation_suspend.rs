use super::*;

pub(super) fn validate_p1_yield_function(
    path: &Path,
    function: &AstFunction,
    imports: &[String],
) -> Result<(), Diagnostic> {
    let has_yield = function
        .body
        .iter()
        .any(|statement| ast_statement_contains_yield(statement, imports));
    if !has_yield || !function.is_suspend {
        return Ok(());
    }

    if function.package.as_slice() == ["std", "task"] && function.name == "yield_now" {
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
    let supported_signature = function.type_params.is_empty()
        && function.params.is_empty()
        && function.return_type.path.as_slice() == ["void"]
        && function.return_type.args.is_empty();
    let supported_body = function.body.iter().all(|statement| match statement {
        Stmt::Expr { expr, .. } => {
            (ast_expr_is_direct_suspension(expr, imports, suspending_functions)
                || !ast_expr_contains_suspension(expr, imports, suspending_functions))
                && !ast_expr_contains_frame_exit(expr)
        }
        Stmt::Let { mutable, value, .. } => {
            !mutable
                && !ast_expr_contains_suspension(value, imports, suspending_functions)
                && !ast_expr_contains_frame_exit(value)
        }
        _ => false,
    });

    if supported_signature && supported_body {
        return Ok(());
    }

    Err(Diagnostic::new(
        "E0876",
        "the current nested-frame slice supports immutable top-level locals and standalone suspend calls in non-generic parameterless `suspend fn` functions returning `void`; mutable locals, generics, arguments/results, recursive suspension, nested control flow, `?`, explicit panic, and handles require a later slice",
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
                    .any(|statement| ast_statement_contains_yield(statement, imports))
        })
        .map(|function| function.name.clone())
        .collect::<HashSet<_>>();

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
        validate_p1_suspend_function_shape(path, function, imports, &suspending)?;
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
    let has_yield = body.iter().any(|statement| {
        matches!(
            statement,
            Statement::Expr(ValueExpr::Call { name, args })
                if name == BUILTIN_TASK_YIELD_EXPR && args.is_empty()
        )
    });
    if !has_yield {
        return Ok(());
    }

    let unsupported = body.iter().find_map(|statement| match statement {
        Statement::Let {
            name, value_type, ..
        } if !p1_frame_value_type_supported(value_type, structs, enums, &mut Vec::new()) => {
            Some((name, value_type))
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
                && function.body.iter().any(|statement| {
                    matches!(
                        statement,
                        Statement::Expr(ValueExpr::Call { name, args })
                            if name == BUILTIN_TASK_YIELD_EXPR && args.is_empty()
                    )
                })
        })
        .map(|function| function.name.clone())
        .collect::<HashSet<_>>();

    loop {
        let discovered = functions
            .iter()
            .filter(|function| function.is_suspend && !suspending.contains(&function.name))
            .filter(|function| {
                function.body.iter().any(|statement| {
                    matches!(
                        statement,
                        Statement::Expr(ValueExpr::Call { name, args })
                            if args.is_empty() && suspending.contains(name)
                    )
                })
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
        if let Some((name, value_type)) = function.body.iter().find_map(|statement| match statement
        {
            Statement::Let {
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
    }
    Ok(())
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
                || !args
                    .iter()
                    .all(|arg| p1_frame_value_type_supported(arg, structs, enums, visiting))
            {
                return false;
            }
            visiting.push(value_type.clone());
            let supported = enum_type.variants.iter().all(|variant| {
                variant.payload.as_ref().is_none_or(|payload| {
                    let payload_type =
                        substitute_type_params(payload, &enum_type.type_params, args);
                    p1_frame_value_type_supported(&payload_type, structs, enums, visiting)
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

fn ast_statement_contains_yield(statement: &Stmt, imports: &[String]) -> bool {
    ast_statement_any_expr(statement, |candidate| {
        ast_expr_is_direct_yield(candidate, imports)
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
        Stmt::Unsafe { body, .. } => body
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
    let AstExpr::Call { callee, args, .. } = expr else {
        return false;
    };
    args.is_empty()
        && callee
            .last()
            .is_some_and(|name| suspending_functions.contains(name))
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
