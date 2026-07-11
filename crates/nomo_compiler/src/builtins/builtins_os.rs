use super::*;
pub(super) fn is_os_builtin_call(callee: &[String]) -> bool {
    matches!(
        callee,
        [module, name]
            if module == "os"
                && matches!(
                    name.as_str(),
                    "platform" | "arch" | "path_separator" | "line_ending"
                )
    )
}
pub(super) fn lower_os_builtin(
    path: &Path,
    callee: &[String],
    args: &[AstExpr],
    span: &Span,
) -> Result<(ValueType, ValueExpr), Diagnostic> {
    let [module, name] = callee else {
        unreachable!("os builtin dispatcher only passes qualified calls")
    };
    debug_assert_eq!(module, "os");
    if !args.is_empty() {
        return Err(Diagnostic::new(
            "E0407",
            format!("`os.{name}` does not accept arguments"),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    }
    let expr = match name.as_str() {
        "platform" => ValueExpr::OsPlatform,
        "arch" => ValueExpr::OsArch,
        "path_separator" => ValueExpr::OsPathSeparator,
        "line_ending" => ValueExpr::OsLineEnding,
        _ => unreachable!("os builtin dispatcher only passes known calls"),
    };
    Ok((ValueType::String, expr))
}
