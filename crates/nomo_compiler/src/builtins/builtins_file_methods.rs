use super::*;
pub(super) fn is_file_value_method(callee: &[String], scope: &HashMap<String, Binding>) -> bool {
    if callee.len() != 2 {
        return false;
    }
    scope.get(&callee[0]).is_some_and(|binding| {
        binding.value_type == ValueType::Struct("File".to_string(), Vec::new())
    })
}

pub(super) fn lower_file_value_method(
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
    let name = &callee[0];
    let method = &callee[1];
    let binding = scope.get(name).expect("file method receiver is in scope");
    let receiver_expr = binding_value_expr(name, binding);
    let fs_error = ValueType::Struct("FsError".to_string(), Vec::new());
    match method.as_str() {
        "close" => {
            if !args.is_empty() {
                return Err(Diagnostic::new(
                    "E0407",
                    "`File.close` does not accept arguments",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            }
            Ok((
                ValueType::Void,
                ValueExpr::FileClose {
                    file: Box::new(receiver_expr),
                },
            ))
        }
        "read_to_string" => {
            if !args.is_empty() {
                return Err(Diagnostic::new(
                    "E0407",
                    "`File.read_to_string` does not accept arguments",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            }
            Ok((
                ValueType::Enum("Result".to_string(), vec![ValueType::String, fs_error]),
                ValueExpr::FileReadToString {
                    file: Box::new(receiver_expr),
                },
            ))
        }
        "write_string" => {
            let [content_arg] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`File.write_string` expects exactly one content string",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (content_type, lowered_content) = lower_value_expr_with_expected(
                path,
                content_arg,
                scope,
                imports,
                signatures,
                structs,
                enums,
                Some(&ValueType::String),
                span,
            )?;
            if content_type != ValueType::String {
                return Err(type_mismatch(
                    path,
                    span,
                    "`File.write_string` expects string content",
                ));
            }
            Ok((
                ValueType::Enum("Result".to_string(), vec![ValueType::Void, fs_error]),
                ValueExpr::FileWriteString {
                    file: Box::new(receiver_expr),
                    content: Box::new(lowered_content),
                },
            ))
        }
        _ => Err(Diagnostic::new(
            "E0305",
            format!("unknown File method `{method}`"),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        )),
    }
}
