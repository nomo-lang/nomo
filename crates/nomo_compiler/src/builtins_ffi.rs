use super::*;

pub(super) fn is_ffi_builtin_call(callee: &[String]) -> bool {
    matches!(callee, [owner, method] if owner == "CString" && method == "from_string")
}

pub(super) fn lower_ffi_builtin(
    path: &Path,
    callee: &[String],
    args: &[AstExpr],
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
    debug_assert_eq!(owner, "CString");
    debug_assert_eq!(method, "from_string");
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
