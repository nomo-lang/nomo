use super::*;
pub(super) fn lower_io_builtin(
    path: &Path,
    callee: &[String],
    args: &[AstExpr],
    span: &Span,
) -> Result<(ValueType, ValueExpr), Diagnostic> {
    match callee {
        [module, name] if module == "io" && name == "read_line" => {
            if !args.is_empty() {
                return Err(Diagnostic::new(
                    "E0407",
                    "`io.read_line` does not accept arguments",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            }
            Ok((
                ValueType::Enum(
                    "Result".to_string(),
                    vec![
                        ValueType::String,
                        ValueType::Struct("IoError".to_string(), Vec::new()),
                    ],
                ),
                ValueExpr::IoReadLine,
            ))
        }
        _ => unreachable!("io builtin dispatcher only passes known calls"),
    }
}
pub(super) fn is_io_value_builtin_call(callee: &[String]) -> bool {
    matches!(
        callee,
        [module, name] if module == "io" && matches!(name.as_str(), "read_line")
    )
}
