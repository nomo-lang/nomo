use super::*;

pub(super) fn is_ffi_builtin_call(callee: &[String]) -> bool {
    matches!(callee, [owner, method]
        if (owner == "CString" && method == "from_string")
            || (owner == "Nullable" && matches!(method.as_str(), "none" | "some")))
}

pub(super) fn lower_ffi_builtin(
    path: &Path,
    callee: &[String],
    args: &[AstExpr],
    type_args: &[AstTypeRef],
    scope: &HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    span: &Span,
) -> Result<(ValueType, ValueExpr), Diagnostic> {
    let [owner, method] = callee else {
        unreachable!("FFI builtin dispatcher only passes qualified calls")
    };
    if owner == "Nullable" {
        return lower_nullable_constructor(
            path, method, args, type_args, scope, imports, signatures, structs, enums, span,
        );
    }
    debug_assert_eq!(owner, "CString");
    debug_assert_eq!(method, "from_string");
    if !type_args.is_empty() {
        return Err(type_mismatch(
            path,
            span,
            "`CString.from_string` does not accept type arguments",
        ));
    }
    if !imports
        .iter()
        .any(|import| import == "std.ffi" || import == "std.ffi.CString")
    {
        return Err(Diagnostic::new(
            "E0301",
            "`CString.from_string` requires `import std.ffi`",
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    }
    let [value] = args else {
        return Err(Diagnostic::new(
            "E0407",
            "`CString.from_string` expects exactly one string argument",
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    };
    let (value_type, lowered) = lower_value_expr(
        path, value, scope, imports, signatures, structs, enums, span,
    )?;
    if value_type != ValueType::String {
        return Err(type_mismatch_expected_found(
            path,
            span,
            "`CString.from_string` expects a string value",
            &ValueType::String,
            &value_type,
        ));
    }
    Ok((
        ValueType::CString,
        ValueExpr::Call {
            name: BUILTIN_CSTRING_FROM_STRING_EXPR.to_string(),
            args: vec![lowered],
        },
    ))
}

#[allow(clippy::too_many_arguments)]
fn lower_nullable_constructor(
    path: &Path,
    method: &str,
    args: &[AstExpr],
    type_args: &[AstTypeRef],
    scope: &HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    span: &Span,
) -> Result<(ValueType, ValueExpr), Diagnostic> {
    match method {
        "none" => {
            if !args.is_empty() || type_args.len() != 1 {
                return Err(Diagnostic::new(
                    "E0407",
                    "`Nullable.none<Handle>()` expects one opaque handle type argument and no values",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            }
            let handle_type =
                parse_non_void_type(&type_args[0], structs, enums).ok_or_else(|| {
                    type_mismatch(path, span, "`Nullable.none` requires an opaque handle type")
                })?;
            if !is_ffi_handle_type(&handle_type) {
                return Err(type_mismatch(
                    path,
                    span,
                    "`Nullable.none` requires an opaque handle type",
                ));
            }
            Ok((
                ValueType::Nullable(Box::new(handle_type)),
                ValueExpr::Call {
                    name: BUILTIN_NULLABLE_NONE_EXPR.to_string(),
                    args: Vec::new(),
                },
            ))
        }
        "some" => {
            if !type_args.is_empty() {
                return Err(type_mismatch(
                    path,
                    span,
                    "`Nullable.some` infers its handle type and does not accept type arguments",
                ));
            }
            let [value] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`Nullable.some` expects exactly one opaque handle value",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (handle_type, value) = lower_value_expr(
                path, value, scope, imports, signatures, structs, enums, span,
            )?;
            if !is_ffi_handle_type(&handle_type) {
                return Err(type_mismatch(
                    path,
                    span,
                    "`Nullable.some` expects an opaque handle value",
                ));
            }
            Ok((
                ValueType::Nullable(Box::new(handle_type)),
                ValueExpr::Call {
                    name: BUILTIN_NULLABLE_SOME_EXPR.to_string(),
                    args: vec![value],
                },
            ))
        }
        _ => unreachable!("FFI dispatcher passes known Nullable constructors"),
    }
}

pub(super) fn is_owned_handle_value_method(
    callee: &[String],
    scope: &HashMap<String, Binding>,
) -> bool {
    callee.len() == 2
        && scope
            .get(&callee[0])
            .is_some_and(|binding| matches!(binding.value_type, ValueType::OwnedHandle(_)))
}

pub(super) fn lower_owned_handle_value_method(
    path: &Path,
    callee: &[String],
    args: &[AstExpr],
    scope: &HashMap<String, Binding>,
    span: &Span,
) -> Result<(ValueType, ValueExpr), Diagnostic> {
    let binding = scope
        .get(&callee[0])
        .expect("owned handle method receiver must be in scope");
    let ValueType::OwnedHandle(handle) = &binding.value_type else {
        unreachable!("owned handle method dispatcher validates receiver type")
    };
    if callee[1] != "borrow" {
        return Err(Diagnostic::new(
            "E0407",
            format!(
                "Owned<{}> has no method `{}`; use `borrow`",
                handle, callee[1]
            ),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    }
    if !args.is_empty() {
        return Err(Diagnostic::new(
            "E0407",
            "`Owned.borrow` does not accept arguments",
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    }
    Ok((
        ValueType::BorrowedHandle(handle.clone()),
        ValueExpr::Call {
            name: BUILTIN_OWNED_BORROW_EXPR.to_string(),
            args: vec![binding_value_expr(&callee[0], binding)],
        },
    ))
}

pub(super) fn is_nullable_value_method(
    callee: &[String],
    scope: &HashMap<String, Binding>,
) -> bool {
    callee.len() == 2
        && scope
            .get(&callee[0])
            .is_some_and(|binding| matches!(binding.value_type, ValueType::Nullable(_)))
}

pub(super) fn lower_nullable_value_method(
    path: &Path,
    callee: &[String],
    args: &[AstExpr],
    scope: &HashMap<String, Binding>,
    span: &Span,
) -> Result<(ValueType, ValueExpr), Diagnostic> {
    let binding = scope
        .get(&callee[0])
        .expect("nullable method receiver must be in scope");
    let ValueType::Nullable(handle_type) = &binding.value_type else {
        unreachable!("nullable method dispatcher validates receiver type")
    };
    if !args.is_empty() {
        return Err(Diagnostic::new(
            "E0407",
            format!("`Nullable.{}` does not accept arguments", callee[1]),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    }
    let receiver = binding_value_expr(&callee[0], binding);
    match callee[1].as_str() {
        "is_null" => Ok((
            ValueType::Bool,
            ValueExpr::Call {
                name: BUILTIN_NULLABLE_IS_NULL_EXPR.to_string(),
                args: vec![receiver],
            },
        )),
        "unwrap" => Ok((
            handle_type.as_ref().clone(),
            ValueExpr::Call {
                name: BUILTIN_NULLABLE_UNWRAP_EXPR.to_string(),
                args: vec![receiver],
            },
        )),
        method => Err(Diagnostic::new(
            "E0407",
            format!("Nullable has no method `{method}`; use `is_null` or `unwrap`"),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        )),
    }
}
