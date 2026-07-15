use super::*;

pub(super) fn lower_panic_message(
    path: &Path,
    message: &AstExpr,
    scope: &HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    span: &Span,
) -> Result<ValueExpr, Diagnostic> {
    let (message_type, lowered) = lower_value_expr(
        path, message, scope, imports, signatures, structs, enums, span,
    )?;
    if message_type != ValueType::String {
        return Err(type_mismatch(
            path,
            span,
            "`panic` expects a string message",
        ));
    }
    Ok(lowered)
}

pub(super) fn lower_struct_value_method(
    path: &Path,
    callee: &[String],
    args: &[AstExpr],
    scope: &HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    span: &Span,
    allow_void: bool,
) -> Result<Option<(ValueType, ValueExpr)>, Diagnostic> {
    if callee.len() != 2 {
        return Ok(None);
    }
    let receiver_name = &callee[0];
    let method_name = &callee[1];
    let Some(binding) = scope.get(receiver_name) else {
        return Ok(None);
    };
    let ValueType::Struct(owner_name, owner_args) = &binding.value_type else {
        return Ok(None);
    };
    if !owner_args.is_empty() {
        return Ok(None);
    }
    let lowered_name = method_internal_name(owner_name, method_name);
    let Some(signature) = signatures.get(&lowered_name) else {
        return Err(Diagnostic::new(
            "E0314",
            format!("struct `{owner_name}` has no method `{method_name}`"),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    };
    if signature.return_type == ValueType::Void && !allow_void {
        return Err(type_mismatch(
            path,
            span,
            format!(
                "method `{owner_name}.{method_name}` returns `void` and cannot be used as a value"
            ),
        ));
    }
    if args.len() + 1 != signature.params.len() {
        return Err(Diagnostic::new(
            "E0407",
            format!(
                "method `{owner_name}.{method_name}` expects {} argument(s), got {}",
                signature.params.len() - 1,
                args.len()
            ),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    }
    let Some(receiver_param) = signature.params.first() else {
        return Ok(None);
    };
    if receiver_param.value_type != binding.value_type {
        return Err(Diagnostic::new(
            "E0257",
            format!("method `{owner_name}.{method_name}` has invalid receiver type"),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    }
    if receiver_param.mutable && !binding.mutable {
        return Err(Diagnostic::new(
            "E0501",
            format!(
                "cannot call mutating method `{owner_name}.{method_name}` on immutable {} `{receiver_name}`",
                binding_source_noun(binding)
            ),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    }

    let mut mutable_borrows = Vec::new();
    let mut lowered_args = if receiver_param.mutable {
        let receiver_path = vec![receiver_name.clone()];
        mutable_borrows.push(receiver_path.clone());
        vec![ValueExpr::MutBorrow(receiver_path)]
    } else {
        vec![binding_value_expr(receiver_name, binding)]
    };
    for (index, (arg, expected)) in args.iter().zip(signature.params.iter().skip(1)).enumerate() {
        lowered_args.push(lower_call_arg_for_param(
            path,
            arg,
            expected,
            scope,
            imports,
            signatures,
            structs,
            enums,
            span,
            &format!("{owner_name}.{method_name}"),
            index + 1,
            &mut mutable_borrows,
        )?);
    }

    Ok(Some((
        signature.return_type.clone(),
        ValueExpr::Call {
            name: lowered_name,
            args: lowered_args,
        },
    )))
}

pub(super) fn lower_call_arg_for_param(
    path: &Path,
    arg: &AstExpr,
    expected: &ParamSignature,
    scope: &HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    span: &Span,
    callable: &str,
    position: usize,
    mutable_borrows: &mut Vec<Vec<String>>,
) -> Result<ValueExpr, Diagnostic> {
    if !expected.mutable {
        if let ValueType::ExternCallback {
            params,
            return_type,
        } = &expected.value_type
        {
            let AstExpr::Name(callback_path) = arg else {
                return Err(callback_argument_diagnostic(
                    path,
                    span,
                    callable,
                    position,
                    "callbacks must be non-capturing top-level function names",
                ));
            };
            let [name] = callback_path.as_slice() else {
                return Err(callback_argument_diagnostic(
                    path,
                    span,
                    callable,
                    position,
                    "callbacks must be unqualified top-level function names",
                ));
            };
            let Some(signature) = signatures.get(name) else {
                return Err(callback_argument_diagnostic(
                    path,
                    span,
                    callable,
                    position,
                    &format!("unknown callback function `{name}`"),
                ));
            };
            let callback_matches = signature.extern_symbol.is_none()
                && signature.type_params.is_empty()
                && signature.params.iter().all(|param| !param.mutable)
                && signature
                    .params
                    .iter()
                    .map(|param| &param.value_type)
                    .eq(params.iter())
                && signature.return_type == **return_type;
            if !callback_matches {
                return Err(callback_argument_diagnostic(
                    path,
                    span,
                    callable,
                    position,
                    &format!("function `{name}` does not match the declared callback ABI"),
                ));
            }
            return Ok(ValueExpr::FunctionRef(name.clone()));
        }
    }
    match (expected.mutable, arg) {
        (true, AstExpr::MutArg { name }) => lower_mut_call_arg(
            path,
            name,
            expected,
            scope,
            structs,
            span,
            callable,
            position,
            mutable_borrows,
        ),
        (true, _) => Err(Diagnostic::new(
            "E0500",
            format!("argument {position} to `{callable}` must be passed as `mut`"),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        )),
        (false, AstExpr::MutArg { .. }) => Err(Diagnostic::new(
            "E0504",
            format!("argument {position} to `{callable}` is not declared `mut`"),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        )),
        (false, _) => {
            let (actual, lowered) = lower_value_expr_with_expected(
                path,
                arg,
                scope,
                imports,
                signatures,
                structs,
                enums,
                Some(&expected.value_type),
                span,
            )?;
            if actual != expected.value_type {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    format!(
                        "argument {position} to `{callable}` is `{}` but expected `{}`",
                        actual.name(),
                        expected.value_type.name()
                    ),
                    &expected.value_type,
                    &actual,
                ));
            }
            Ok(lowered)
        }
    }
}

fn callback_argument_diagnostic(
    path: &Path,
    span: &Span,
    callable: &str,
    position: usize,
    detail: &str,
) -> Diagnostic {
    Diagnostic::new(
        "E1525",
        format!("argument {position} to `{callable}` is not a valid extern C callback: {detail}"),
        path,
        span.line,
        span.column,
        span.length,
        &span.text,
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn lower_mut_call_arg(
    path: &Path,
    name: &[String],
    expected: &ParamSignature,
    scope: &HashMap<String, Binding>,
    structs: &HashMap<String, StructType>,
    span: &Span,
    callable: &str,
    position: usize,
    mutable_borrows: &mut Vec<Vec<String>>,
) -> Result<ValueExpr, Diagnostic> {
    if name.is_empty() {
        return Err(Diagnostic::new(
            "E0503",
            "`mut` call arguments must be local variables or fields",
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    }

    let root = &name[0];
    let Some(binding) = scope.get(root) else {
        return Err(Diagnostic::new(
            "E0303",
            format!("unknown variable `{root}`"),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    };
    if !matches!(binding.source, BindingSource::Local | BindingSource::Param) {
        return Err(Diagnostic::new(
            "E0503",
            "`mut` call arguments must be local variables or fields",
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    }
    if !binding.mutable {
        let message = match binding.source {
            BindingSource::Param => format!("cannot pass immutable parameter `{root}` as `mut`"),
            _ => format!("cannot pass immutable variable `{root}` as `mut`"),
        };
        return Err(Diagnostic::new(
            "E0501",
            message,
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    }

    let actual_type = mut_borrow_path_type(path, name, &binding.value_type, structs, span)?;
    if actual_type != expected.value_type {
        return Err(type_mismatch_expected_found(
            path,
            span,
            format!(
                "argument {position} to `{callable}` is `{}` but expected `{}`",
                actual_type.name(),
                expected.value_type.name()
            ),
            &expected.value_type,
            &actual_type,
        ));
    }

    if let Some(conflict) = mutable_borrows
        .iter()
        .find(|borrowed| mut_borrow_paths_conflict(borrowed, name))
    {
        return Err(Diagnostic::new(
            "E0502",
            format!(
                "mutable borrow `{}` conflicts with active mutable borrow `{}` in this call",
                mut_borrow_path_name(name),
                mut_borrow_path_name(conflict)
            ),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    }

    mutable_borrows.push(name.to_vec());
    Ok(ValueExpr::MutBorrow(name.to_vec()))
}

pub(super) fn mut_borrow_path_type(
    path: &Path,
    name: &[String],
    root_type: &ValueType,
    structs: &HashMap<String, StructType>,
    span: &Span,
) -> Result<ValueType, Diagnostic> {
    let mut current = root_type.clone();
    for field in name.iter().skip(1) {
        let ValueType::Struct(struct_name, struct_args) = &current else {
            return Err(type_mismatch(
                path,
                span,
                format!("`{}` is not a struct value", mut_borrow_path_name(name)),
            ));
        };
        let struct_type = structs
            .get(struct_name)
            .expect("struct binding must refer to a known struct");
        let Some(field_type) = struct_type
            .fields
            .iter()
            .find(|item| item.name == *field)
            .map(|item| {
                substitute_type_params(&item.value_type, &struct_type.type_params, struct_args)
            })
        else {
            return Err(Diagnostic::new(
                "E0316",
                format!("struct `{struct_name}` has no field `{field}`"),
                path,
                span.line,
                span.column,
                span.length,
                &span.text,
            ));
        };
        current = field_type;
    }
    Ok(current)
}

pub(super) fn mut_borrow_paths_conflict(left: &[String], right: &[String]) -> bool {
    left.first() == right.first() && (left.starts_with(right) || right.starts_with(left))
}

pub(super) fn mut_borrow_path_name(path: &[String]) -> String {
    path.join(".")
}

pub(super) fn lower_array_mutation(
    path: &Path,
    callee: &[String],
    args: &[AstExpr],
    scope: &HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    span: &Span,
) -> Result<ValueExpr, Diagnostic> {
    let name = &callee[0];
    let method = &callee[1];
    require_array_method_import(path, imports, span, method)?;
    let Some(binding) = scope.get(name) else {
        return Err(Diagnostic::new(
            "E0303",
            format!("unknown variable `{name}`"),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    };
    if !binding.mutable {
        return Err(Diagnostic::new(
            "E0501",
            format!(
                "cannot call mutating Array method on immutable {} `{name}`",
                binding_source_noun(binding)
            ),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    }
    let ValueType::Array(element_type) = &binding.value_type else {
        return Err(type_mismatch(
            path,
            span,
            format!("`{name}` is not an Array"),
        ));
    };
    ensure_supported_array_element(path, element_type, span)?;
    match method.as_str() {
        "push" => {
            let [value] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`Array.push` expects exactly one value",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (value_type, lowered_value) = lower_value_expr_with_expected(
                path,
                value,
                scope,
                imports,
                signatures,
                structs,
                enums,
                Some(element_type),
                span,
            )?;
            if &value_type != element_type.as_ref() {
                return Err(type_mismatch(
                    path,
                    span,
                    format!(
                        "`Array.push` value is `{}` but expected `{}`",
                        value_type.name(),
                        element_type.name()
                    ),
                ));
            }
            Ok(ValueExpr::ArrayPush {
                array: name.clone(),
                value: Box::new(lowered_value),
                element_type: element_type.as_ref().clone(),
            })
        }
        "set" => {
            let [index, value] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`Array.set` expects index and value",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (index_type, lowered_index) = lower_value_expr_with_expected(
                path,
                index,
                scope,
                imports,
                signatures,
                structs,
                enums,
                Some(&ValueType::U64),
                span,
            )?;
            if index_type != ValueType::U64 {
                return Err(type_mismatch(path, span, "`Array.set` index must be `u64`"));
            }
            let (value_type, lowered_value) = lower_value_expr_with_expected(
                path,
                value,
                scope,
                imports,
                signatures,
                structs,
                enums,
                Some(element_type),
                span,
            )?;
            if &value_type != element_type.as_ref() {
                return Err(type_mismatch(
                    path,
                    span,
                    format!(
                        "`Array.set` value is `{}` but expected `{}`",
                        value_type.name(),
                        element_type.name()
                    ),
                ));
            }
            Ok(ValueExpr::ArraySet {
                array: name.clone(),
                index: Box::new(lowered_index),
                value: Box::new(lowered_value),
                element_type: element_type.as_ref().clone(),
            })
        }
        "insert" => {
            let [index, value] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`Array.insert` expects index and value",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (index_type, lowered_index) = lower_value_expr_with_expected(
                path,
                index,
                scope,
                imports,
                signatures,
                structs,
                enums,
                Some(&ValueType::U64),
                span,
            )?;
            if index_type != ValueType::U64 {
                return Err(type_mismatch(
                    path,
                    span,
                    "`Array.insert` index must be `u64`",
                ));
            }
            let (value_type, lowered_value) = lower_value_expr_with_expected(
                path,
                value,
                scope,
                imports,
                signatures,
                structs,
                enums,
                Some(element_type),
                span,
            )?;
            if &value_type != element_type.as_ref() {
                return Err(type_mismatch(
                    path,
                    span,
                    format!(
                        "`Array.insert` value is `{}` but expected `{}`",
                        value_type.name(),
                        element_type.name()
                    ),
                ));
            }
            Ok(ValueExpr::ArrayInsert {
                array: name.clone(),
                index: Box::new(lowered_index),
                value: Box::new(lowered_value),
                element_type: element_type.as_ref().clone(),
            })
        }
        "clear" => {
            if !args.is_empty() {
                return Err(Diagnostic::new(
                    "E0407",
                    "`Array.clear` does not accept arguments",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            }
            Ok(ValueExpr::ArrayClear {
                array: name.clone(),
                element_type: element_type.as_ref().clone(),
            })
        }
        _ => Err(Diagnostic::new(
            "E0305",
            format!("unknown mutating Array method `{method}`"),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        )),
    }
}
