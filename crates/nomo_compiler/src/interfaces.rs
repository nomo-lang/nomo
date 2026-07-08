use super::*;

pub(super) fn collect_interfaces<'a>(
    path: &Path,
    interfaces: &'a [AstInterfaceDef],
) -> Result<HashMap<String, &'a AstInterfaceDef>, Diagnostic> {
    let mut out = HashMap::new();
    for interface in interfaces {
        if out.contains_key(&interface.name) {
            return Err(Diagnostic::new(
                "E0304",
                format!("interface `{}` is already defined", interface.name),
                path,
                interface.span.line,
                interface.span.column,
                interface.span.length,
                &interface.span.text,
            ));
        }
        out.insert(interface.name.clone(), interface);
    }
    Ok(out)
}

pub(super) fn validate_interface_impl(
    path: &Path,
    impl_block: &crate::ast::ImplBlock,
    interface_name: &AstTypeRef,
    owner_name: &str,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    interfaces: &HashMap<String, &AstInterfaceDef>,
) -> Result<(), Diagnostic> {
    let [interface_name] = interface_name.path.as_slice() else {
        return Err(interface_impl_error(
            path,
            None,
            "v0.1 interface impls must name a single interface",
        ));
    };
    if !impl_block
        .interface_name
        .as_ref()
        .is_some_and(|interface| interface.args.is_empty())
    {
        return Err(interface_impl_error(
            path,
            None,
            "v0.1 interface impls do not accept interface type arguments",
        ));
    }
    let Some(interface) = interfaces.get(interface_name) else {
        return Err(interface_impl_error(
            path,
            None,
            format!("unknown interface `{interface_name}`"),
        ));
    };

    for required in &interface.methods {
        let Some(method) = impl_block
            .methods
            .iter()
            .find(|method| method.name == required.name)
        else {
            return Err(interface_impl_error(
                path,
                Some(&required.span),
                format!(
                    "impl `{interface_name} for {owner_name}` is missing method `{}`",
                    required.name
                ),
            ));
        };
        let expected = interface_method_signature(path, required, owner_name, structs, enums)?;
        let actual = function_signature(path, method, structs, enums)?;
        validate_interface_method_signature(
            path,
            interface_name,
            owner_name,
            required,
            method,
            &expected,
            &actual,
        )?;
    }
    Ok(())
}

fn interface_method_signature(
    path: &Path,
    signature: &AstFunctionSignature,
    owner_name: &str,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
) -> Result<FunctionSignature, Diagnostic> {
    let struct_names = structs
        .values()
        .map(|item| (item.name.clone(), item.type_params.len()))
        .collect::<Vec<_>>();
    let enum_names = enums
        .values()
        .map(|item| (item.name.clone(), item.type_params.len()))
        .collect::<Vec<_>>();
    let params = signature
        .params
        .iter()
        .map(|param| {
            let type_ref =
                replace_self_type_ref(&param.type_ref, owner_name).map_err(|message| {
                    Diagnostic::new(
                        "E0258",
                        message,
                        path,
                        signature.span.line,
                        signature.span.column,
                        signature.span.length,
                        &signature.span.text,
                    )
                })?;
            let value_type = parse_value_type_with_names(
                &type_ref,
                &struct_names,
                &enum_names,
                &signature.type_params,
            )
            .ok_or_else(|| {
                unsupported_type_diagnostic(
                    path,
                    &signature.span,
                    &param.type_ref,
                    "unsupported interface method parameter type in v0.1 current implementation",
                    &struct_names,
                    &enum_names,
                )
            })?;
            ensure_supported_value_type(path, &value_type, &signature.span)?;
            Ok(ParamSignature {
                value_type,
                mutable: param.mutable,
            })
        })
        .collect::<Result<Vec<_>, Diagnostic>>()?;
    let return_type_ref =
        replace_self_type_ref(&signature.return_type, owner_name).map_err(|message| {
            Diagnostic::new(
                "E0258",
                message,
                path,
                signature.span.line,
                signature.span.column,
                signature.span.length,
                &signature.span.text,
            )
        })?;
    let return_type = parse_value_type_with_names(
        &return_type_ref,
        &struct_names,
        &enum_names,
        &signature.type_params,
    )
    .ok_or_else(|| {
        unsupported_type_diagnostic(
            path,
            &signature.span,
            &signature.return_type,
            "unsupported interface method return type in v0.1 current implementation",
            &struct_names,
            &enum_names,
        )
    })?;
    ensure_supported_value_type(path, &return_type, &signature.span)?;
    Ok(FunctionSignature {
        type_params: signature.type_params.clone(),
        params,
        return_type,
        extern_symbol: None,
    })
}

fn replace_self_type_ref(type_ref: &AstTypeRef, owner_name: &str) -> Result<AstTypeRef, String> {
    if type_ref.path == ["Self"] {
        if !type_ref.args.is_empty() {
            return Err(
                "`Self` in an interface method signature cannot take type arguments".to_string(),
            );
        }
        return Ok(AstTypeRef {
            path: vec![owner_name.to_string()],
            args: Vec::new(),
        });
    }
    Ok(AstTypeRef {
        path: type_ref.path.clone(),
        args: type_ref
            .args
            .iter()
            .map(|arg| replace_self_type_ref(arg, owner_name))
            .collect::<Result<Vec<_>, String>>()?,
    })
}

fn validate_interface_method_signature(
    path: &Path,
    interface_name: &str,
    owner_name: &str,
    _required: &AstFunctionSignature,
    method: &AstFunction,
    expected: &FunctionSignature,
    actual: &FunctionSignature,
) -> Result<(), Diagnostic> {
    let method_label = format!("{owner_name}.{}", method.name);
    if expected.type_params != actual.type_params {
        return Err(interface_impl_error(
            path,
            Some(&method.span),
            format!(
                "method `{method_label}` type parameters do not match interface `{interface_name}`"
            ),
        ));
    }
    if expected.params.len() != actual.params.len() {
        return Err(interface_impl_error(
            path,
            Some(&method.span),
            format!(
                "method `{method_label}` expects {} parameter(s) for interface `{interface_name}`, got {}",
                expected.params.len(),
                actual.params.len()
            ),
        ));
    }
    for (index, (expected_param, actual_param)) in
        expected.params.iter().zip(actual.params.iter()).enumerate()
    {
        if expected_param.mutable != actual_param.mutable {
            return Err(interface_impl_error(
                path,
                Some(&method.span),
                format!(
                    "method `{method_label}` parameter {} mutability does not match interface `{interface_name}`",
                    index + 1
                ),
            ));
        }
        if expected_param.value_type != actual_param.value_type {
            return Err(interface_impl_error(
                path,
                Some(&method.span),
                format!(
                    "method `{method_label}` parameter {} is `{}` but interface `{interface_name}` expects `{}`",
                    index + 1,
                    actual_param.value_type.name(),
                    expected_param.value_type.name()
                ),
            ));
        }
    }
    if expected.return_type != actual.return_type {
        return Err(interface_impl_error(
            path,
            Some(&method.span),
            format!(
                "method `{method_label}` returns `{}` but interface `{interface_name}` expects `{}`",
                actual.return_type.name(),
                expected.return_type.name()
            ),
        ));
    }
    Ok(())
}

fn interface_impl_error(
    path: &Path,
    span: Option<&Span>,
    message: impl Into<String>,
) -> Diagnostic {
    match span {
        Some(span) => Diagnostic::new(
            "E0258",
            message,
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ),
        None => Diagnostic::new("E0258", message, path, 1, 1, 1, ""),
    }
}
