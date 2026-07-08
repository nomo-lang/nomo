use super::*;

pub(super) fn ast_functions(ast: &SourceFile) -> impl Iterator<Item = &AstFunction> {
    ast.functions
        .iter()
        .chain(ast.impls.iter().flat_map(|item| item.methods.iter()))
}

pub(super) fn collect_generic_function_instances(
    path: &Path,
    ast: &SourceFile,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
) -> Result<Vec<FunctionInstance>, Diagnostic> {
    let mut out = Vec::new();
    for function in ast_functions(ast) {
        for stmt in &function.body {
            collect_stmt_generic_function_instances(
                path, stmt, imports, signatures, structs, enums, &mut out,
            )?;
        }
    }
    Ok(out)
}

fn collect_stmt_generic_function_instances(
    path: &Path,
    stmt: &Stmt,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    out: &mut Vec<FunctionInstance>,
) -> Result<(), Diagnostic> {
    match stmt {
        Stmt::Let { value, .. } | Stmt::Assign { value, .. } => {
            collect_expr_generic_function_instances(
                path, value, imports, signatures, structs, enums, out,
            )
        }
        Stmt::LetElse {
            value, else_body, ..
        } => {
            collect_expr_generic_function_instances(
                path, value, imports, signatures, structs, enums, out,
            )?;
            for stmt in else_body {
                collect_stmt_generic_function_instances(
                    path, stmt, imports, signatures, structs, enums, out,
                )?;
            }
            Ok(())
        }
        Stmt::IfLet {
            value,
            body,
            else_body,
            ..
        } => {
            collect_expr_generic_function_instances(
                path, value, imports, signatures, structs, enums, out,
            )?;
            for stmt in body {
                collect_stmt_generic_function_instances(
                    path, stmt, imports, signatures, structs, enums, out,
                )?;
            }
            if let Some(else_body) = else_body {
                for stmt in else_body {
                    collect_stmt_generic_function_instances(
                        path, stmt, imports, signatures, structs, enums, out,
                    )?;
                }
            }
            Ok(())
        }
        Stmt::Return { value, .. } => {
            if let Some(value) = value {
                collect_expr_generic_function_instances(
                    path, value, imports, signatures, structs, enums, out,
                )?;
            }
            Ok(())
        }
        Stmt::Expr { expr, .. } => collect_expr_generic_function_instances(
            path, expr, imports, signatures, structs, enums, out,
        ),
        Stmt::Match { value, arms, .. } => {
            collect_expr_generic_function_instances(
                path, value, imports, signatures, structs, enums, out,
            )?;
            for arm in arms {
                for stmt in &arm.body {
                    collect_stmt_generic_function_instances(
                        path, stmt, imports, signatures, structs, enums, out,
                    )?;
                }
            }
            Ok(())
        }
        Stmt::For { variant, .. } => match variant {
            ForVariant::Infinite { body } => {
                for stmt in body {
                    collect_stmt_generic_function_instances(
                        path, stmt, imports, signatures, structs, enums, out,
                    )?;
                }
                Ok(())
            }
            ForVariant::While { condition, body } => {
                collect_expr_generic_function_instances(
                    path, condition, imports, signatures, structs, enums, out,
                )?;
                for stmt in body {
                    collect_stmt_generic_function_instances(
                        path, stmt, imports, signatures, structs, enums, out,
                    )?;
                }
                Ok(())
            }
            ForVariant::Iterate { iterable, body, .. } => {
                collect_expr_generic_function_instances(
                    path, iterable, imports, signatures, structs, enums, out,
                )?;
                for stmt in body {
                    collect_stmt_generic_function_instances(
                        path, stmt, imports, signatures, structs, enums, out,
                    )?;
                }
                Ok(())
            }
        },
        Stmt::Defer { stmt, .. } => collect_stmt_generic_function_instances(
            path, stmt, imports, signatures, structs, enums, out,
        ),
        Stmt::Unsafe { body, .. } => {
            for stmt in body {
                collect_stmt_generic_function_instances(
                    path, stmt, imports, signatures, structs, enums, out,
                )?;
            }
            Ok(())
        }
        Stmt::Postfix { .. } => Ok(()),
        Stmt::Break { .. } | Stmt::Continue { .. } => Ok(()),
    }
}

fn collect_expr_generic_function_instances(
    path: &Path,
    expr: &AstExpr,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    out: &mut Vec<FunctionInstance>,
) -> Result<(), Diagnostic> {
    match expr {
        AstExpr::Call {
            callee,
            type_args,
            args,
        } => {
            if callee.len() == 1 && !type_args.is_empty() {
                let name = &callee[0];
                if resolve_specific_value_builtin(name, imports).is_some() {
                    for arg in args {
                        collect_expr_generic_function_instances(
                            path, arg, imports, signatures, structs, enums, out,
                        )?;
                    }
                    return Ok(());
                }
                if core_prelude_variant(name).is_some() {
                    for arg in args {
                        collect_expr_generic_function_instances(
                            path, arg, imports, signatures, structs, enums, out,
                        )?;
                    }
                    return Ok(());
                }
                let signature = signatures.get(name).ok_or_else(|| {
                    Diagnostic::new(
                        "E0305",
                        format!("unknown function `{name}`"),
                        path,
                        1,
                        1,
                        1,
                        "",
                    )
                })?;
                if signature.type_params.is_empty() {
                    return Err(Diagnostic::new(
                        "E0404",
                        format!("function `{name}` does not accept type arguments"),
                        path,
                        1,
                        1,
                        1,
                        "",
                    ));
                }
                if type_args.len() != signature.type_params.len() {
                    return Err(Diagnostic::new(
                        "E0407",
                        format!(
                            "function `{name}` expects {} type argument(s), got {}",
                            signature.type_params.len(),
                            type_args.len()
                        ),
                        path,
                        1,
                        1,
                        1,
                        "",
                    ));
                }
                let args = type_args
                    .iter()
                    .map(|arg| parse_non_void_type(arg, structs, enums))
                    .collect::<Option<Vec<_>>>()
                    .ok_or_else(|| {
                        let type_arg = type_args
                            .iter()
                            .find(|arg| parse_non_void_type(arg, structs, enums).is_none())
                            .expect("at least one type argument failed to lower");
                        unsupported_type_diagnostic_from_maps(
                            path,
                            &synthetic_span(),
                            type_arg,
                            format!("unsupported type argument for `{name}`"),
                            structs,
                            enums,
                        )
                    })?;
                let instance = FunctionInstance {
                    name: name.clone(),
                    args,
                };
                if !out.contains(&instance) {
                    out.push(instance);
                }
            }
            for arg in args {
                collect_expr_generic_function_instances(
                    path, arg, imports, signatures, structs, enums, out,
                )?;
            }
            Ok(())
        }
        AstExpr::StructLiteral { fields, .. } => {
            for (_, value) in fields {
                collect_expr_generic_function_instances(
                    path, value, imports, signatures, structs, enums, out,
                )?;
            }
            Ok(())
        }
        AstExpr::Match { value, arms } => {
            collect_expr_generic_function_instances(
                path, value, imports, signatures, structs, enums, out,
            )?;
            for arm in arms {
                collect_expr_generic_function_instances(
                    path, &arm.value, imports, signatures, structs, enums, out,
                )?;
            }
            Ok(())
        }
        AstExpr::If {
            condition,
            then_branch,
            else_branch,
        } => {
            collect_expr_generic_function_instances(
                path, condition, imports, signatures, structs, enums, out,
            )?;
            collect_expr_generic_function_instances(
                path,
                then_branch,
                imports,
                signatures,
                structs,
                enums,
                out,
            )?;
            collect_expr_generic_function_instances(
                path,
                else_branch,
                imports,
                signatures,
                structs,
                enums,
                out,
            )
        }
        AstExpr::Panic { message }
        | AstExpr::Question { expr: message }
        | AstExpr::Unary { expr: message, .. } => collect_expr_generic_function_instances(
            path, message, imports, signatures, structs, enums, out,
        ),
        AstExpr::Cast { expr, .. } => collect_expr_generic_function_instances(
            path, expr, imports, signatures, structs, enums, out,
        ),
        AstExpr::Binary { left, right, .. } => {
            collect_expr_generic_function_instances(
                path, left, imports, signatures, structs, enums, out,
            )?;
            collect_expr_generic_function_instances(
                path, right, imports, signatures, structs, enums, out,
            )
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
