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

    let supported_signature = function.name == "main"
        && function.params.is_empty()
        && function.return_type.path.as_slice() == ["void"]
        && function.return_type.args.is_empty();
    let supported_body = function.body.iter().all(|statement| match statement {
        Stmt::Expr { expr, .. } => {
            (ast_expr_is_direct_yield(expr, imports) || !ast_expr_contains_yield(expr, imports))
                && !ast_expr_contains_frame_exit(expr)
        }
        Stmt::Let { mutable, value, .. } => {
            !mutable
                && !ast_expr_contains_yield(value, imports)
                && !ast_expr_contains_frame_exit(value)
        }
        _ => false,
    });

    if supported_signature && supported_body {
        return Ok(());
    }

    Err(Diagnostic::new(
        "E0876",
        "the current frame-liveness slice supports immutable top-level locals and standalone `task.yield_now()` in parameterless `suspend fn main() -> void`; mutable locals, nested control flow, `?`, explicit panic, handles, and non-root suspension require a later slice",
        path,
        function.span.line,
        function.span.column,
        function.span.length,
        &function.span.text,
    ))
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
    match statement {
        Stmt::Let { value, .. }
        | Stmt::Assign { value, .. }
        | Stmt::Return {
            value: Some(value), ..
        }
        | Stmt::Expr { expr: value, .. } => ast_expr_contains_yield(value, imports),
        Stmt::IndexAssign { indices, value, .. } => {
            indices
                .iter()
                .any(|index| ast_expr_contains_yield(index, imports))
                || ast_expr_contains_yield(value, imports)
        }
        Stmt::LetElse {
            value, else_body, ..
        } => {
            ast_expr_contains_yield(value, imports)
                || else_body
                    .iter()
                    .any(|statement| ast_statement_contains_yield(statement, imports))
        }
        Stmt::IfLet {
            value,
            body,
            else_body,
            ..
        } => {
            ast_expr_contains_yield(value, imports)
                || body
                    .iter()
                    .any(|statement| ast_statement_contains_yield(statement, imports))
                || else_body.as_ref().is_some_and(|body| {
                    body.iter()
                        .any(|statement| ast_statement_contains_yield(statement, imports))
                })
        }
        Stmt::Match { value, arms, .. } => {
            ast_expr_contains_yield(value, imports)
                || arms.iter().any(|arm| {
                    arm.body
                        .iter()
                        .any(|statement| ast_statement_contains_yield(statement, imports))
                })
        }
        Stmt::For { variant, .. } => match variant {
            ForVariant::Infinite { body } => body
                .iter()
                .any(|statement| ast_statement_contains_yield(statement, imports)),
            ForVariant::While { condition, body } => {
                ast_expr_contains_yield(condition, imports)
                    || body
                        .iter()
                        .any(|statement| ast_statement_contains_yield(statement, imports))
            }
            ForVariant::CStyle {
                initializer,
                condition,
                update,
                body,
                ..
            } => {
                ast_expr_contains_yield(initializer, imports)
                    || ast_expr_contains_yield(condition, imports)
                    || ast_statement_contains_yield(update, imports)
                    || body
                        .iter()
                        .any(|statement| ast_statement_contains_yield(statement, imports))
            }
            ForVariant::Iterate { iterable, body, .. } => {
                ast_expr_contains_yield(iterable, imports)
                    || body
                        .iter()
                        .any(|statement| ast_statement_contains_yield(statement, imports))
            }
        },
        Stmt::Defer { stmt, .. } => ast_statement_contains_yield(stmt, imports),
        Stmt::Unsafe { body, .. } => body
            .iter()
            .any(|statement| ast_statement_contains_yield(statement, imports)),
        Stmt::Postfix { .. }
        | Stmt::Return { value: None, .. }
        | Stmt::Break { .. }
        | Stmt::Continue { .. } => false,
    }
}

fn ast_expr_contains_yield(expr: &AstExpr, imports: &[String]) -> bool {
    ast_expr_any(expr, |candidate| {
        ast_expr_is_direct_yield(candidate, imports)
    })
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
