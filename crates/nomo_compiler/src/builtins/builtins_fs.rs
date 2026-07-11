use super::*;

pub(super) fn lower_fs_builtin(
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
    let fs_error = ValueType::Struct("FsError".to_string(), Vec::new());
    match callee {
        [module, name] if module == "fs" && name == "read_to_string" => {
            let [path_arg] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`fs.read_to_string` expects exactly one path string",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (path_type, lowered_path) = lower_value_expr(
                path, path_arg, scope, imports, signatures, structs, enums, span,
            )?;
            if path_type != ValueType::String {
                return Err(type_mismatch(
                    path,
                    span,
                    "`fs.read_to_string` expects a string path",
                ));
            }
            Ok((
                ValueType::Enum("Result".to_string(), vec![ValueType::String, fs_error]),
                ValueExpr::FsReadToString {
                    path: Box::new(lowered_path),
                },
            ))
        }
        [module, name] if module == "fs" && name == "write_string" => {
            let [path_arg, content_arg] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`fs.write_string` expects path and content strings",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (path_type, lowered_path) = lower_value_expr(
                path, path_arg, scope, imports, signatures, structs, enums, span,
            )?;
            let (content_type, lowered_content) = lower_value_expr(
                path,
                content_arg,
                scope,
                imports,
                signatures,
                structs,
                enums,
                span,
            )?;
            if path_type != ValueType::String || content_type != ValueType::String {
                return Err(type_mismatch(
                    path,
                    span,
                    "`fs.write_string` expects string path and content",
                ));
            }
            Ok((
                ValueType::Enum("Result".to_string(), vec![ValueType::Void, fs_error]),
                ValueExpr::FsWriteString {
                    path: Box::new(lowered_path),
                    content: Box::new(lowered_content),
                },
            ))
        }
        [module, name] if module == "fs" && name == "read_bytes" => {
            let [path_arg] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`fs.read_bytes` expects exactly one path string",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (path_type, lowered_path) = lower_value_expr(
                path, path_arg, scope, imports, signatures, structs, enums, span,
            )?;
            if path_type != ValueType::String {
                return Err(type_mismatch(
                    path,
                    span,
                    "`fs.read_bytes` expects a string path",
                ));
            }
            Ok((
                ValueType::Enum(
                    "Result".to_string(),
                    vec![ValueType::Array(Box::new(ValueType::U32)), fs_error],
                ),
                ValueExpr::FsReadBytes {
                    path: Box::new(lowered_path),
                },
            ))
        }
        [module, name] if module == "fs" && name == "write_bytes" => {
            let [path_arg, bytes_arg] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`fs.write_bytes` expects a path string and Array<u32> bytes",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (path_type, lowered_path) = lower_value_expr(
                path, path_arg, scope, imports, signatures, structs, enums, span,
            )?;
            let (bytes_type, lowered_bytes) = lower_value_expr(
                path, bytes_arg, scope, imports, signatures, structs, enums, span,
            )?;
            let expected_bytes = ValueType::Array(Box::new(ValueType::U32));
            if path_type != ValueType::String {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    "`fs.write_bytes` expects a string path",
                    &ValueType::String,
                    &path_type,
                ));
            }
            if bytes_type != expected_bytes {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    "`fs.write_bytes` expects Array<u32> bytes",
                    &expected_bytes,
                    &bytes_type,
                ));
            }
            Ok((
                ValueType::Enum("Result".to_string(), vec![ValueType::Void, fs_error]),
                ValueExpr::FsWriteBytes {
                    path: Box::new(lowered_path),
                    bytes: Box::new(lowered_bytes),
                },
            ))
        }
        [module, name] if module == "fs" && name == "exists" => {
            let [path_arg] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`fs.exists` expects exactly one path string",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (path_type, lowered_path) = lower_value_expr(
                path, path_arg, scope, imports, signatures, structs, enums, span,
            )?;
            if path_type != ValueType::String {
                return Err(type_mismatch(
                    path,
                    span,
                    "`fs.exists` expects a string path",
                ));
            }
            Ok((
                ValueType::Bool,
                ValueExpr::FsExists {
                    path: Box::new(lowered_path),
                },
            ))
        }
        [module, name] if module == "fs" && name == "metadata" => {
            let [path_arg] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`fs.metadata` expects exactly one path string",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (path_type, lowered_path) = lower_value_expr(
                path, path_arg, scope, imports, signatures, structs, enums, span,
            )?;
            if path_type != ValueType::String {
                return Err(type_mismatch(
                    path,
                    span,
                    "`fs.metadata` expects a string path",
                ));
            }
            Ok((
                ValueType::Enum(
                    "Result".to_string(),
                    vec![
                        ValueType::Struct("FileMetadata".to_string(), Vec::new()),
                        fs_error,
                    ],
                ),
                ValueExpr::FsMetadata {
                    path: Box::new(lowered_path),
                },
            ))
        }
        [module, name] if module == "fs" && (name == "create_dir" || name == "remove_dir") => {
            let [path_arg] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    format!("`fs.{name}` expects exactly one path string"),
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (path_type, lowered_path) = lower_value_expr(
                path, path_arg, scope, imports, signatures, structs, enums, span,
            )?;
            if path_type != ValueType::String {
                return Err(type_mismatch(
                    path,
                    span,
                    format!("`fs.{name}` expects a string path"),
                ));
            }
            let expr = if name == "create_dir" {
                ValueExpr::FsCreateDir {
                    path: Box::new(lowered_path),
                }
            } else {
                ValueExpr::FsRemoveDir {
                    path: Box::new(lowered_path),
                }
            };
            Ok((
                ValueType::Enum("Result".to_string(), vec![ValueType::Void, fs_error]),
                expr,
            ))
        }
        [module, name] if module == "fs" && name == "read_dir" => {
            let [path_arg] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`fs.read_dir` expects exactly one path string",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (path_type, lowered_path) = lower_value_expr(
                path, path_arg, scope, imports, signatures, structs, enums, span,
            )?;
            if path_type != ValueType::String {
                return Err(type_mismatch(
                    path,
                    span,
                    "`fs.read_dir` expects a string path",
                ));
            }
            Ok((
                ValueType::Enum(
                    "Result".to_string(),
                    vec![
                        ValueType::Array(Box::new(ValueType::String)),
                        ValueType::Struct("FsError".to_string(), Vec::new()),
                    ],
                ),
                ValueExpr::FsReadDir {
                    path: Box::new(lowered_path),
                },
            ))
        }
        [module, name] if module == "fs" && name == "open" => {
            let [path_arg] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`fs.open` expects exactly one path string",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (path_type, lowered_path) = lower_value_expr(
                path, path_arg, scope, imports, signatures, structs, enums, span,
            )?;
            if path_type != ValueType::String {
                return Err(type_mismatch(path, span, "`fs.open` expects a string path"));
            }
            Ok((
                ValueType::Enum(
                    "Result".to_string(),
                    vec![ValueType::Struct("File".to_string(), Vec::new()), fs_error],
                ),
                ValueExpr::FsOpen {
                    path: Box::new(lowered_path),
                },
            ))
        }
        _ => unreachable!("fs builtin dispatcher only passes known calls"),
    }
}
