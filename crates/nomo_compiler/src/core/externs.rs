use super::*;

pub(super) fn collect_extern_signatures(
    path: &Path,
    ast: &SourceFile,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    repr_c_structs: &HashSet<String>,
    signatures: &mut HashMap<String, FunctionSignature>,
) -> Result<(HashSet<String>, Vec<ExternFunction>), Diagnostic> {
    let mut extern_names = HashSet::new();
    let mut extern_functions = Vec::new();
    for block in &ast.extern_blocks {
        if block.abi != "C" {
            return Err(Diagnostic::new(
                "E1511",
                "v0.1 only supports extern \"C\" blocks",
                path,
                block.span.line,
                block.span.column,
                block.span.length,
                &block.span.text,
            ));
        }
        for function in &block.functions {
            if !extern_names.insert(function.name.clone())
                || signatures.contains_key(&function.name)
            {
                return Err(Diagnostic::new(
                    "E0304",
                    format!("function `{}` is already defined", function.name),
                    path,
                    function.span.line,
                    function.span.column,
                    function.span.length,
                    &function.span.text,
                ));
            }
            let signature =
                extern_function_signature(path, function, structs, enums, repr_c_structs)?;
            extern_functions.push(ExternFunction {
                symbol: function.name.clone(),
                params: signature
                    .params
                    .iter()
                    .map(|param| param.value_type.clone())
                    .collect(),
                return_type: signature.return_type.clone(),
            });
            signatures.insert(function.name.clone(), signature);
        }
    }
    Ok((extern_names, extern_functions))
}

pub(super) fn validate_opaque_handle_release_functions(
    path: &Path,
    opaque_types: &[AstExternOpaqueType],
    signatures: &HashMap<String, FunctionSignature>,
) -> Result<(), Diagnostic> {
    for item in opaque_types {
        let Some(release_function) = &item.release_function else {
            continue;
        };
        let expected = ValueType::OwnedHandle(item.name.clone());
        let valid = signatures.get(release_function).is_some_and(|signature| {
            signature.extern_symbol.as_deref() == Some(release_function.as_str())
                && signature.return_type == ValueType::Void
                && signature.params.len() == 1
                && signature.params[0].value_type == expected
                && !signature.params[0].mutable
        });
        if !valid {
            return Err(Diagnostic::new(
                "E1523",
                format!(
                    "release function `{release_function}` for `{}` must be declared in an extern \"C\" block as `fn {release_function}(handle: Owned<{}>) -> void`",
                    item.name, item.name
                ),
                path,
                item.span.line,
                item.span.column,
                item.span.length,
                &item.span.text,
            ));
        }
    }
    Ok(())
}

fn extern_function_signature(
    path: &Path,
    function: &AstFunctionSignature,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    repr_c_structs: &HashSet<String>,
) -> Result<FunctionSignature, Diagnostic> {
    if !function.type_params.is_empty() {
        return Err(Diagnostic::new(
            "E1519",
            "extern \"C\" functions cannot be generic in v0.1",
            path,
            function.span.line,
            function.span.column,
            function.span.length,
            &function.span.text,
        ));
    }
    let struct_names = struct_type_names(structs);
    let enum_names = enums
        .values()
        .map(|item| (item.name.clone(), item.type_params.len()))
        .collect::<Vec<_>>();
    let params = function
        .params
        .iter()
        .map(|param| {
            if param.mutable {
                return Err(Diagnostic::new(
                    "E1519",
                    "extern \"C\" function parameters cannot be `mut` in v0.1",
                    path,
                    function.span.line,
                    function.span.column,
                    function.span.length,
                    &function.span.text,
                ));
            }
            let value_type = parse_value_type_with_names(
                &param.type_ref,
                &struct_names,
                &enum_names,
                &function.type_params,
            )
            .ok_or_else(|| {
                unsupported_type_diagnostic(
                    path,
                    &function.span,
                    &param.type_ref,
                    "unsupported extern parameter type in v0.1 current implementation",
                    &struct_names,
                    &enum_names,
                )
            })?;
            ensure_supported_extern_type(
                path,
                &function.span,
                &value_type,
                ExternTypePosition::Parameter,
                repr_c_structs,
            )?;
            Ok(ParamSignature {
                value_type,
                mutable: false,
            })
        })
        .collect::<Result<Vec<_>, Diagnostic>>()?;
    let return_type = parse_value_type_with_names(
        &function.return_type,
        &struct_names,
        &enum_names,
        &function.type_params,
    )
    .ok_or_else(|| {
        unsupported_type_diagnostic(
            path,
            &function.span,
            &function.return_type,
            "unsupported extern return type in v0.1 current implementation",
            &struct_names,
            &enum_names,
        )
    })?;
    ensure_supported_extern_type(
        path,
        &function.span,
        &return_type,
        ExternTypePosition::Return,
        repr_c_structs,
    )?;
    Ok(FunctionSignature {
        type_params: Vec::new(),
        params,
        return_type,
        extern_symbol: Some(function.name.clone()),
    })
}

fn ensure_supported_extern_type(
    path: &Path,
    span: &Span,
    value_type: &ValueType,
    position: ExternTypePosition,
    repr_c_structs: &HashSet<String>,
) -> Result<(), Diagnostic> {
    if let ValueType::ExternCallback {
        params,
        return_type,
    } = value_type
    {
        if position != ExternTypePosition::Parameter {
            return Err(Diagnostic::new(
                "E1525",
                "extern C callbacks may only be passed as extern function parameters",
                path,
                span.line,
                span.column,
                span.length,
                &span.text,
            ));
        }
        for param in params {
            ensure_supported_callback_component(path, span, param, false, repr_c_structs)?;
        }
        ensure_supported_callback_component(path, span, return_type, true, repr_c_structs)?;
        return Ok(());
    }
    let supported = matches!(
        value_type,
        ValueType::Int
            | ValueType::I32
            | ValueType::U32
            | ValueType::U64
            | ValueType::Float
            | ValueType::Bool
            | ValueType::Char
            | ValueType::Opaque
            | ValueType::OpaqueHandle(_)
            | ValueType::OwnedHandle(_)
            | ValueType::BorrowedHandle(_)
            | ValueType::Nullable(_)
    ) || matches!(value_type, ValueType::Struct(name, args) if args.is_empty() && repr_c_structs.contains(name))
        || (position == ExternTypePosition::Parameter && value_type == &ValueType::CString)
        || (position == ExternTypePosition::Return && value_type == &ValueType::Void);
    if supported {
        Ok(())
    } else {
        Err(Diagnostic::new(
            "E1519",
            format!(
                "extern \"C\" {} type `{}` is not supported; current FFI supports primitive integer, float, bool, char, Opaque{}",
                position.label(),
                value_type.name(),
                if position == ExternTypePosition::Parameter {
                    ", and CString parameter types"
                } else {
                    ", and void return types; CString cannot be returned by C"
                }
            ),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ))
    }
}

fn ensure_supported_callback_component(
    path: &Path,
    span: &Span,
    value_type: &ValueType,
    is_return: bool,
    repr_c_structs: &HashSet<String>,
) -> Result<(), Diagnostic> {
    let supported = matches!(
        value_type,
        ValueType::Int
            | ValueType::I32
            | ValueType::U32
            | ValueType::U64
            | ValueType::Float
            | ValueType::Bool
            | ValueType::Char
            | ValueType::Opaque
            | ValueType::OpaqueHandle(_)
            | ValueType::BorrowedHandle(_)
            | ValueType::Nullable(_)
    ) || matches!(value_type, ValueType::Struct(name, args) if args.is_empty() && repr_c_structs.contains(name))
        || (is_return && value_type == &ValueType::Void);
    if supported {
        Ok(())
    } else {
        Err(Diagnostic::new(
            "E1525",
            format!(
                "extern C callback {} type `{}` is not ABI-safe in the current implementation",
                if is_return { "return" } else { "parameter" },
                value_type.name()
            ),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExternTypePosition {
    Parameter,
    Return,
}

impl ExternTypePosition {
    fn label(self) -> &'static str {
        match self {
            Self::Parameter => "parameter",
            Self::Return => "return",
        }
    }
}

pub(super) fn validate_extern_calls_are_unsafe(
    path: &Path,
    ast: &SourceFile,
    extern_names: &HashSet<String>,
) -> Result<(), Diagnostic> {
    for const_def in &ast.consts {
        validate_extern_expr_is_unsafe(
            path,
            &const_def.value,
            false,
            extern_names,
            &const_def.span,
        )?;
    }
    for function in ast_functions(ast) {
        for stmt in &function.body {
            validate_extern_stmt_is_unsafe(path, stmt, false, extern_names)?;
        }
    }
    for stmt in &ast.script_body {
        validate_extern_stmt_is_unsafe(path, stmt, false, extern_names)?;
    }
    Ok(())
}

fn validate_extern_stmt_is_unsafe(
    path: &Path,
    stmt: &Stmt,
    in_unsafe: bool,
    extern_names: &HashSet<String>,
) -> Result<(), Diagnostic> {
    match stmt {
        Stmt::Let { value, span, .. }
        | Stmt::Assign { value, span, .. }
        | Stmt::Expr { expr: value, span } => {
            validate_extern_expr_is_unsafe(path, value, in_unsafe, extern_names, span)
        }
        Stmt::LetElse {
            value,
            else_body,
            span,
            ..
        } => {
            validate_extern_expr_is_unsafe(path, value, in_unsafe, extern_names, span)?;
            for stmt in else_body {
                validate_extern_stmt_is_unsafe(path, stmt, in_unsafe, extern_names)?;
            }
            Ok(())
        }
        Stmt::IfLet {
            value,
            body,
            else_body,
            span,
            ..
        } => {
            validate_extern_expr_is_unsafe(path, value, in_unsafe, extern_names, span)?;
            for stmt in body {
                validate_extern_stmt_is_unsafe(path, stmt, in_unsafe, extern_names)?;
            }
            if let Some(else_body) = else_body {
                for stmt in else_body {
                    validate_extern_stmt_is_unsafe(path, stmt, in_unsafe, extern_names)?;
                }
            }
            Ok(())
        }
        Stmt::Return { value, span } => {
            if let Some(value) = value {
                validate_extern_expr_is_unsafe(path, value, in_unsafe, extern_names, span)?;
            }
            Ok(())
        }
        Stmt::Match { value, arms, span } => {
            validate_extern_expr_is_unsafe(path, value, in_unsafe, extern_names, span)?;
            for arm in arms {
                for stmt in &arm.body {
                    validate_extern_stmt_is_unsafe(path, stmt, in_unsafe, extern_names)?;
                }
            }
            Ok(())
        }
        Stmt::For { variant, span } => {
            match variant {
                crate::ast::ForVariant::Infinite { body } => {
                    for stmt in body {
                        validate_extern_stmt_is_unsafe(path, stmt, in_unsafe, extern_names)?;
                    }
                }
                crate::ast::ForVariant::While { condition, body } => {
                    validate_extern_expr_is_unsafe(path, condition, in_unsafe, extern_names, span)?;
                    for stmt in body {
                        validate_extern_stmt_is_unsafe(path, stmt, in_unsafe, extern_names)?;
                    }
                }
                crate::ast::ForVariant::Iterate { iterable, body, .. } => {
                    validate_extern_expr_is_unsafe(path, iterable, in_unsafe, extern_names, span)?;
                    for stmt in body {
                        validate_extern_stmt_is_unsafe(path, stmt, in_unsafe, extern_names)?;
                    }
                }
            }
            Ok(())
        }
        Stmt::Defer { stmt, .. } => {
            validate_extern_stmt_is_unsafe(path, stmt, in_unsafe, extern_names)
        }
        Stmt::Unsafe { body, .. } => {
            for stmt in body {
                validate_extern_stmt_is_unsafe(path, stmt, true, extern_names)?;
            }
            Ok(())
        }
        Stmt::Postfix { .. } | Stmt::Break { .. } | Stmt::Continue { .. } => Ok(()),
    }
}

fn validate_extern_expr_is_unsafe(
    path: &Path,
    expr: &AstExpr,
    in_unsafe: bool,
    extern_names: &HashSet<String>,
    span: &Span,
) -> Result<(), Diagnostic> {
    match expr {
        AstExpr::Call { callee, args, .. } => {
            if callee.len() == 1 && extern_names.contains(&callee[0]) && !in_unsafe {
                return Err(Diagnostic::new(
                    "E1519",
                    format!(
                        "extern function `{}` must be called inside an `unsafe` block",
                        callee[0]
                    ),
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            }
            for arg in args {
                validate_extern_expr_is_unsafe(path, arg, in_unsafe, extern_names, span)?;
            }
            Ok(())
        }
        AstExpr::Question { expr: base }
        | AstExpr::Unary { expr: base, .. }
        | AstExpr::Cast { expr: base, .. } => {
            validate_extern_expr_is_unsafe(path, base, in_unsafe, extern_names, span)
        }
        AstExpr::Binary { left, right, .. } => {
            validate_extern_expr_is_unsafe(path, left, in_unsafe, extern_names, span)?;
            validate_extern_expr_is_unsafe(path, right, in_unsafe, extern_names, span)
        }
        AstExpr::If {
            condition,
            then_branch,
            else_branch,
        } => {
            validate_extern_expr_is_unsafe(path, condition, in_unsafe, extern_names, span)?;
            validate_extern_expr_is_unsafe(path, then_branch, in_unsafe, extern_names, span)?;
            validate_extern_expr_is_unsafe(path, else_branch, in_unsafe, extern_names, span)
        }
        AstExpr::Match { value, arms } => {
            validate_extern_expr_is_unsafe(path, value, in_unsafe, extern_names, span)?;
            for arm in arms {
                validate_extern_expr_is_unsafe(path, &arm.value, in_unsafe, extern_names, span)?;
            }
            Ok(())
        }
        AstExpr::StructLiteral { fields, .. } => {
            for (_, value) in fields {
                validate_extern_expr_is_unsafe(path, value, in_unsafe, extern_names, span)?;
            }
            Ok(())
        }
        AstExpr::Panic { message } => {
            validate_extern_expr_is_unsafe(path, message, in_unsafe, extern_names, span)
        }
        AstExpr::MutArg { .. }
        | AstExpr::Name(_)
        | AstExpr::String(_)
        | AstExpr::Int(_)
        | AstExpr::Float(_)
        | AstExpr::Char(_)
        | AstExpr::Bool(_)
        | AstExpr::Void => Ok(()),
    }
}
