use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PublicationTransfer {
    Copy,
    Move,
    Shared,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SendFailure {
    path: String,
    value_type: String,
    reason: &'static str,
    nested: bool,
}

pub(super) fn publication_transfer(
    value_type: &ValueType,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
) -> Result<PublicationTransfer, (String, String, &'static str, bool)> {
    let mut visiting = HashSet::new();
    check_send_type(value_type, structs, enums, value_type.name(), &mut visiting).map_err(
        |failure| {
            (
                failure.path,
                failure.value_type,
                failure.reason,
                failure.nested,
            )
        },
    )
}

pub(super) fn validate_structured_spawn_publications(
    path: &Path,
    ast: &SourceFile,
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
) -> Result<(), Diagnostic> {
    for function in ast
        .functions
        .iter()
        .chain(ast.impls.iter().flat_map(|item| item.methods.iter()))
    {
        for statement in &function.body {
            let span = statement_span(statement);
            visit_statement_expressions(statement, &mut |expression| {
                let AstExpr::Call {
                    callee,
                    args,
                    type_args,
                } = expression
                else {
                    return Ok(());
                };
                if callee.as_slice() != ["task", TASK_STRUCTURED_SPAWN_AST_NAME]
                    || !type_args.is_empty()
                {
                    return Ok(());
                }
                let [
                    AstExpr::Call {
                        callee: target,
                        args: target_args,
                        ..
                    },
                ] = args.as_slice()
                else {
                    return Ok(());
                };
                let [target_name] = target.as_slice() else {
                    return Ok(());
                };
                let Some(signature) = signatures.get(target_name) else {
                    return Ok(());
                };
                for (index, parameter) in signature.params.iter().enumerate() {
                    if target_args.get(index).is_none() {
                        break;
                    }
                    validate_publication_type(
                        path,
                        span,
                        &format!("argument {} to `{target_name}`", index + 1),
                        &parameter.value_type,
                        structs,
                        enums,
                    )?;
                }
                Ok(())
            })?;
        }
    }
    Ok(())
}

pub(super) fn validate_publication_type(
    path: &Path,
    span: &Span,
    label: &str,
    value_type: &ValueType,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
) -> Result<PublicationTransfer, Diagnostic> {
    publication_transfer(value_type, structs, enums).map_err(
        |(failure_path, local_type, reason, nested)| {
            let (code, message) = if nested {
                (
                    "E0883",
                    format!(
                        "{label} is not structurally Send; `{failure_path}` contains Local/!Send type `{local_type}`: {reason}"
                    ),
                )
            } else {
                (
                    "E0880",
                    format!(
                        "{label} has Local/!Send type `{local_type}` and cannot cross structured task.spawn publication: {reason}"
                    ),
                )
            };
            Diagnostic::new(
                code,
                message,
                path,
                span.line,
                span.column,
                span.length,
                &span.text,
            )
        },
    )
}

pub(super) fn validate_statement_publication_uses(
    path: &Path,
    statement: &Stmt,
    scope: &HashMap<String, Binding>,
) -> Result<(), Diagnostic> {
    let span = statement_span(statement);
    let validate_expr = |expression: &AstExpr| {
        crate::validation_tasks::visit_expression(expression, &mut |candidate| {
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
    };

    match statement {
        Stmt::Let { value, .. }
        | Stmt::LetElse { value, .. }
        | Stmt::IfLet { value, .. }
        | Stmt::Return {
            value: Some(value), ..
        }
        | Stmt::Match { value, .. }
        | Stmt::Expr { expr: value, .. } => validate_expr(value),
        Stmt::Assign { target, value, .. } => {
            validate_publication_target(path, span, scope, target)?;
            validate_expr(value)
        }
        Stmt::IndexAssign {
            root,
            indices,
            value,
            ..
        } => {
            ensure_publication_binding_available(path, span, scope, root)?;
            for index in indices {
                validate_expr(index)?;
            }
            validate_expr(value)
        }
        Stmt::Postfix { target, .. } => validate_publication_target(path, span, scope, target),
        Stmt::For { variant, .. } => match variant {
            ForVariant::Infinite { .. } => Ok(()),
            ForVariant::While { condition, .. } => validate_expr(condition),
            ForVariant::CStyle {
                initializer,
                condition,
                update,
                ..
            } => {
                validate_expr(initializer)?;
                validate_expr(condition)?;
                validate_statement_publication_uses(path, update, scope)
            }
            ForVariant::Iterate { iterable, .. } => validate_expr(iterable),
        },
        Stmt::Defer { stmt, .. } => validate_statement_publication_uses(path, stmt, scope),
        Stmt::TaskDeadline { duration, .. } => validate_expr(duration),
        Stmt::TaskSelect { arms, .. } => {
            for arm in arms {
                validate_expr(&arm.operation)?;
            }
            Ok(())
        }
        Stmt::Unsafe { .. }
        | Stmt::TaskScope { .. }
        | Stmt::Return { value: None, .. }
        | Stmt::Break { .. }
        | Stmt::Continue { .. } => Ok(()),
    }
}

pub(super) fn ensure_publication_binding_available(
    path: &Path,
    span: &Span,
    scope: &HashMap<String, Binding>,
    name: &str,
) -> Result<(), Diagnostic> {
    let Some((line, boundary)) = publication_move_site(scope, name) else {
        return Ok(());
    };
    Err(Diagnostic::new(
        "E0881",
        format!(
            "binding `{name}` is unavailable after publication move at line {line}; `{boundary}` consumed its ownership"
        ),
        path,
        span.line,
        span.column,
        span.length,
        &span.text,
    ))
}

fn validate_publication_target(
    path: &Path,
    span: &Span,
    scope: &HashMap<String, Binding>,
    target: &[String],
) -> Result<(), Diagnostic> {
    if let Some(name) = target.first() {
        ensure_publication_binding_available(path, span, scope, name)?;
    }
    Ok(())
}

fn check_send_type(
    value_type: &ValueType,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    path: &str,
    visiting: &mut HashSet<String>,
) -> Result<PublicationTransfer, SendFailure> {
    match value_type {
        ValueType::Int
        | ValueType::I32
        | ValueType::U32
        | ValueType::U64
        | ValueType::Float
        | ValueType::Char
        | ValueType::Bool
        | ValueType::Void
        | ValueType::Never => Ok(PublicationTransfer::Copy),
        ValueType::String | ValueType::CString => Ok(PublicationTransfer::Move),
        ValueType::Array(element) => {
            check_nested_send_type(element, structs, enums, &format!("{path}[]"), visiting)?;
            Ok(PublicationTransfer::Move)
        }
        ValueType::Nullable(payload) => {
            check_nested_send_type(payload, structs, enums, &format!("{path}.Some"), visiting)?;
            Ok(PublicationTransfer::Move)
        }
        ValueType::Struct(name, args) => {
            if name == "Channel" {
                let [element_type] = args.as_slice() else {
                    return Err(local_failure(
                        path,
                        value_type,
                        "Channel must have exactly one element type",
                    ));
                };
                check_nested_send_type(
                    element_type,
                    structs,
                    enums,
                    &format!("{path}.value"),
                    visiting,
                )?;
                return Ok(PublicationTransfer::Shared);
            }
            let Some(struct_type) = structs.get(name) else {
                return Err(local_failure(
                    path,
                    value_type,
                    "the nominal type is unavailable to structural Send derivation",
                ));
            };
            if is_publication_local_struct(struct_type) {
                return Err(local_failure(
                    path,
                    value_type,
                    "the value owns an executor-affine or native runtime resource",
                ));
            }
            let identity = format!("struct:{name}:{args:?}");
            if !visiting.insert(identity.clone()) {
                return Ok(PublicationTransfer::Move);
            }
            for field in &struct_type.fields {
                let field_type =
                    substitute_type_params(&field.value_type, &struct_type.type_params, args);
                check_nested_send_type(
                    &field_type,
                    structs,
                    enums,
                    &format!("{path}.{}", field.name),
                    visiting,
                )?;
            }
            visiting.remove(&identity);
            Ok(PublicationTransfer::Move)
        }
        ValueType::Enum(name, args) => {
            let Some(enum_type) = enums.get(name) else {
                return Err(local_failure(
                    path,
                    value_type,
                    "the nominal type is unavailable to structural Send derivation",
                ));
            };
            let identity = format!("enum:{name}:{args:?}");
            if !visiting.insert(identity.clone()) {
                return Ok(PublicationTransfer::Move);
            }
            for variant in &enum_type.variants {
                let Some(payload) = &variant.payload else {
                    continue;
                };
                let payload = substitute_type_params(payload, &enum_type.type_params, args);
                check_nested_send_type(
                    &payload,
                    structs,
                    enums,
                    &format!("{path}.{}", variant.name),
                    visiting,
                )?;
            }
            visiting.remove(&identity);
            Ok(PublicationTransfer::Move)
        }
        ValueType::Opaque
        | ValueType::OpaqueHandle(_)
        | ValueType::OwnedHandle(_)
        | ValueType::BorrowedHandle(_) => Err(local_failure(
            path,
            value_type,
            "opaque and native handles are Local until an explicit transfer contract exists",
        )),
        ValueType::ExternCallback { .. } | ValueType::TaskCallback { .. } => Err(local_failure(
            path,
            value_type,
            "callable runtime values do not have a v0.1 Send contract",
        )),
        ValueType::TypeParam(_) => Err(local_failure(
            path,
            value_type,
            "unresolved type parameters cannot satisfy compiler-known Send",
        )),
    }
}

fn check_nested_send_type(
    value_type: &ValueType,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    path: &str,
    visiting: &mut HashSet<String>,
) -> Result<(), SendFailure> {
    check_send_type(value_type, structs, enums, path, visiting)
        .map(|_| ())
        .map_err(|mut failure| {
            failure.nested = true;
            failure
        })
}

fn local_failure(path: &str, value_type: &ValueType, reason: &'static str) -> SendFailure {
    SendFailure {
        path: path.to_string(),
        value_type: value_type.name().to_string(),
        reason,
        nested: false,
    }
}

fn is_publication_local_struct(item: &StructType) -> bool {
    is_opaque_handle_struct(item)
        || matches!(
            (item.package.as_str(), item.name.as_str()),
            ("std.fs", "File")
                | ("std.net", "TcpStream" | "TcpListener" | "UdpSocket")
                | ("std.http", "HttpServer" | "HttpExchange" | "HttpStream")
                | ("std.process", "ProcessChild" | "BlockingProcessChild")
                | ("std.task", "Task" | "TaskContext")
                | ("std.sqlite", "SqliteDatabase" | "SqliteQuery")
        )
}
