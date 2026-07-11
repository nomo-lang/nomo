use super::*;

pub(super) fn lower_single_segment_call_value_expr(
    path: &Path,
    callee: &[String],
    args: &[AstExpr],
    type_args: &[AstTypeRef],
    scope: &HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    expected: Option<&ValueType>,
    span: &Span,
) -> Result<(ValueType, ValueExpr), Diagnostic> {
    let name = &callee[0];
    if let Some(qualified) = resolve_specific_value_builtin(name, imports) {
        if qualified == ["Array", "new"] {
            return lower_array_new(path, type_args, args, structs, enums, span);
        }
        if !type_args.is_empty() {
            return Err(type_mismatch(
                path,
                span,
                format!("standard library function `{name}` does not accept type arguments"),
            ));
        }
        if qualified[0] == "string" {
            return lower_string_builtin(
                path, &qualified, args, scope, imports, signatures, structs, enums, span,
            );
        }
        if qualified[0] == "fs" {
            return lower_fs_builtin(
                path, &qualified, args, scope, imports, signatures, structs, enums, span,
            );
        }
        if qualified[0] == "io" {
            return lower_io_builtin(path, &qualified, args, span);
        }
        if qualified[0] == "debug" {
            return lower_debug_builtin(
                path, &qualified, args, scope, imports, signatures, structs, enums, span,
            );
        }
        if qualified[0] == "log" {
            return lower_log_builtin(
                path, &qualified, args, scope, imports, signatures, structs, enums, span,
            );
        }
        if qualified[0] == "hash" {
            return lower_hash_builtin(
                path, &qualified, args, scope, imports, signatures, structs, enums, span,
            );
        }
        if qualified[0] == "crypto" {
            return lower_crypto_builtin(
                path, &qualified, args, scope, imports, signatures, structs, enums, span,
            );
        }
        if qualified[0] == "json" {
            return lower_json_builtin(
                path, &qualified, args, scope, imports, signatures, structs, enums, span,
            );
        }
        if qualified[0] == "http" {
            return lower_http_builtin(
                path, &qualified, args, scope, imports, signatures, structs, enums, span,
            );
        }
        if qualified[0] == "net" {
            return lower_net_builtin(
                path, &qualified, args, scope, imports, signatures, structs, enums, span,
            );
        }
        if qualified[0] == "regex" {
            return lower_regex_builtin(
                path, &qualified, args, scope, imports, signatures, structs, enums, span,
            );
        }
        if qualified[0] == "collections" {
            return lower_collections_builtin(
                path, &qualified, args, scope, imports, signatures, structs, enums, span,
            );
        }
        if qualified[0] == "env" {
            return lower_env_builtin(
                path, &qualified, args, scope, imports, signatures, structs, enums, span,
            );
        }
        if qualified[0] == "process" {
            return lower_process_builtin(
                path, &qualified, args, scope, imports, signatures, structs, enums, span,
            );
        }
        if qualified[0] == "path" {
            return lower_path_builtin(
                path, &qualified, args, scope, imports, signatures, structs, enums, span,
            );
        }
        if qualified[0] == "math" {
            return lower_math_builtin(
                path, &qualified, args, scope, imports, signatures, structs, enums, span,
            );
        }
        if qualified[0] == "char" {
            return lower_char_builtin(
                path, &qualified, args, scope, imports, signatures, structs, enums, span,
            );
        }
        if qualified[0] == "os" {
            return lower_os_builtin(path, &qualified, args, span);
        }
        if qualified[0] == "time" {
            return lower_time_builtin(
                path, &qualified, args, scope, imports, signatures, structs, enums, span,
            );
        }
        if qualified[0] == "testing" {
            return lower_testing_builtin(
                path, &qualified, args, scope, imports, signatures, structs, enums, span,
            );
        }
        if qualified[0] == "num" {
            return lower_num_builtin(
                path, &qualified, args, scope, imports, signatures, structs, enums, span,
            );
        }
        if qualified[0] == "option" {
            return lower_option_builtin(
                path, &qualified, args, scope, imports, signatures, structs, enums, span,
            );
        }
        if qualified[0] == "result" {
            return lower_result_builtin(
                path, &qualified, args, scope, imports, signatures, structs, enums, span,
            );
        }
    }
    let Some(template_signature) = signatures.get(name) else {
        if scope.contains_key(name) {
            return Err(Diagnostic::new(
                "E0305",
                format!("local variable `{name}` is not callable"),
                path,
                span.line,
                span.column,
                span.length,
                &span.text,
            ));
        }
        if let Some((enum_name, variant)) = core_prelude_variant(name) {
            if !type_args.is_empty() {
                return Err(type_mismatch(
                    path,
                    span,
                    format!("enum variant `{enum_name}.{variant}` does not accept type arguments"),
                ));
            }
            return lower_enum_variant_with_payload(
                path, enum_name, variant, args, scope, imports, signatures, structs, enums,
                expected, span,
            );
        }
        return Err(Diagnostic::new(
            "E0305",
            format!("unknown function `{name}`"),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    };
    let (call_name, signature) = if type_args.is_empty() {
        if !template_signature.type_params.is_empty() {
            return Err(Diagnostic::new(
                "E0407",
                format!("generic function `{name}` requires explicit type arguments"),
                path,
                span.line,
                span.column,
                span.length,
                &span.text,
            ));
        }
        (name.clone(), template_signature.clone())
    } else {
        if template_signature.type_params.is_empty() {
            return Err(type_mismatch(
                path,
                span,
                format!("function `{name}` does not accept type arguments"),
            ));
        }
        if type_args.len() != template_signature.type_params.len() {
            return Err(Diagnostic::new(
                "E0407",
                format!(
                    "function `{name}` expects {} type argument(s), got {}",
                    template_signature.type_params.len(),
                    type_args.len()
                ),
                path,
                span.line,
                span.column,
                span.length,
                &span.text,
            ));
        }
        let instance_args = type_args
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
                    span,
                    type_arg,
                    format!("unsupported type argument for `{name}`"),
                    structs,
                    enums,
                )
            })?;
        (
            generic_function_instance_name(name, &instance_args),
            instantiate_function_signature(template_signature, &instance_args),
        )
    };
    if signature.return_type == ValueType::Void && !matches!(expected, Some(ValueType::Void)) {
        return Err(type_mismatch(
            path,
            span,
            format!("function `{call_name}` returns `void` and cannot be used as a value"),
        ));
    }
    if args.len() != signature.params.len() {
        return Err(Diagnostic::new(
            "E0407",
            format!(
                "function `{call_name}` expects {} argument(s), got {}",
                signature.params.len(),
                args.len()
            ),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    }

    let mut lowered_args = Vec::new();
    let mut mutable_borrows = Vec::new();
    for (index, (arg, expected)) in args.iter().zip(signature.params.iter()).enumerate() {
        let lowered = lower_call_arg_for_param(
            path,
            arg,
            expected,
            scope,
            imports,
            signatures,
            structs,
            enums,
            span,
            &call_name,
            index + 1,
            &mut mutable_borrows,
        )?;
        lowered_args.push(
            if signature.extern_symbol.is_some() && expected.value_type == ValueType::CString {
                ValueExpr::Call {
                    name: BUILTIN_CSTRING_DATA_EXPR.to_string(),
                    args: vec![lowered],
                }
            } else {
                lowered
            },
        );
    }

    let return_type =
        if signature.extern_symbol.is_some() && matches!(expected, Some(ValueType::Void)) {
            ValueType::Void
        } else {
            signature.return_type.clone()
        };
    Ok((
        return_type,
        ValueExpr::Call {
            name: signature
                .extern_symbol
                .as_ref()
                .map(|symbol| extern_call_name(symbol))
                .unwrap_or(call_name),
            args: lowered_args,
        },
    ))
}
