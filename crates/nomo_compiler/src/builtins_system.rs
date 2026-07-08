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

pub(super) fn lower_env_builtin(
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
    match callee {
        [module, name] if module == "env" && name == "get" => {
            let [name_arg] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`env.get` expects exactly one environment variable name",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (name_type, lowered_name) = lower_value_expr(
                path, name_arg, scope, imports, signatures, structs, enums, span,
            )?;
            if name_type != ValueType::String {
                return Err(type_mismatch(
                    path,
                    span,
                    "`env.get` expects a string variable name",
                ));
            }
            Ok((
                ValueType::Enum("Option".to_string(), vec![ValueType::String]),
                ValueExpr::EnvGet {
                    name: Box::new(lowered_name),
                },
            ))
        }
        [module, name] if module == "env" && name == "set" => {
            let [name_arg, value_arg] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`env.set` expects exactly a name and value string",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (name_type, lowered_name) = lower_value_expr(
                path, name_arg, scope, imports, signatures, structs, enums, span,
            )?;
            let (value_type, lowered_value) = lower_value_expr(
                path, value_arg, scope, imports, signatures, structs, enums, span,
            )?;
            if name_type != ValueType::String || value_type != ValueType::String {
                return Err(type_mismatch(
                    path,
                    span,
                    "`env.set` expects two string arguments",
                ));
            }
            Ok((
                ValueType::Void,
                ValueExpr::EnvSet {
                    name: Box::new(lowered_name),
                    value: Box::new(lowered_value),
                },
            ))
        }
        [module, name] if module == "env" && name == "cwd" => {
            if !args.is_empty() {
                return Err(Diagnostic::new(
                    "E0407",
                    "`env.cwd` does not accept arguments",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            }
            Ok((ValueType::String, ValueExpr::EnvCwd))
        }
        [module, name] if module == "env" && name == "home_dir" => {
            if !args.is_empty() {
                return Err(Diagnostic::new(
                    "E0407",
                    "`env.home_dir` does not accept arguments",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            }
            Ok((
                ValueType::Enum("Option".to_string(), vec![ValueType::String]),
                ValueExpr::EnvHomeDir,
            ))
        }
        [module, name] if module == "env" && name == "temp_dir" => {
            if !args.is_empty() {
                return Err(Diagnostic::new(
                    "E0407",
                    "`env.temp_dir` does not accept arguments",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            }
            Ok((ValueType::String, ValueExpr::EnvTempDir))
        }
        [module, name] if module == "env" && name == "args" => {
            if !args.is_empty() {
                return Err(Diagnostic::new(
                    "E0407",
                    "`env.args` does not accept arguments",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            }
            Ok((
                ValueType::Array(Box::new(ValueType::String)),
                ValueExpr::EnvArgs,
            ))
        }
        _ => unreachable!("env builtin dispatcher only passes known calls"),
    }
}

pub(super) fn lower_process_builtin(
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
    let process_error = ValueType::Struct("ProcessError".to_string(), Vec::new());
    match callee {
        [module, name] if module == "process" && name == "exit" => {
            let [code_arg] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`process.exit` expects exactly one i64 exit code",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (code_type, lowered_code) = lower_value_expr(
                path, code_arg, scope, imports, signatures, structs, enums, span,
            )?;
            if code_type != ValueType::Int {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    "`process.exit` expects an i64 exit code",
                    &ValueType::Int,
                    &code_type,
                ));
            }
            Ok((
                ValueType::Void,
                ValueExpr::ProcessExit {
                    code: Box::new(lowered_code),
                },
            ))
        }
        [module, name]
            if module == "process"
                && (name == "spawn" || name == "status" || name == "exec" || name == "output") =>
        {
            let [command_arg] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    format!("`process.{name}` expects exactly one command string"),
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (command_type, lowered_command) = lower_value_expr(
                path,
                command_arg,
                scope,
                imports,
                signatures,
                structs,
                enums,
                span,
            )?;
            if command_type != ValueType::String {
                return Err(type_mismatch(
                    path,
                    span,
                    format!("`process.{name}` expects a string command"),
                ));
            }
            if name == "spawn" {
                Ok((
                    ValueType::Enum("Result".to_string(), vec![ValueType::I32, process_error]),
                    ValueExpr::ProcessSpawn {
                        command: Box::new(lowered_command),
                    },
                ))
            } else if name == "status" {
                Ok((
                    ValueType::Enum("Result".to_string(), vec![ValueType::I32, process_error]),
                    ValueExpr::ProcessStatus {
                        command: Box::new(lowered_command),
                    },
                ))
            } else if name == "exec" {
                Ok((
                    ValueType::Enum("Result".to_string(), vec![ValueType::String, process_error]),
                    ValueExpr::ProcessExec {
                        command: Box::new(lowered_command),
                    },
                ))
            } else {
                Ok((
                    ValueType::Enum(
                        "Result".to_string(),
                        vec![
                            ValueType::Struct("ProcessOutput".to_string(), Vec::new()),
                            process_error,
                        ],
                    ),
                    ValueExpr::ProcessOutput {
                        command: Box::new(lowered_command),
                    },
                ))
            }
        }
        _ => unreachable!("process builtin dispatcher only passes known calls"),
    }
}

pub(super) fn is_env_builtin_call(callee: &[String]) -> bool {
    matches!(
        callee,
        [module, name]
            if module == "env"
                && matches!(
                    name.as_str(),
                    "args" | "get" | "set" | "cwd" | "home_dir" | "temp_dir"
                )
    )
}

pub(super) fn is_io_value_builtin_call(callee: &[String]) -> bool {
    matches!(
        callee,
        [module, name] if module == "io" && matches!(name.as_str(), "read_line")
    )
}

pub(super) fn is_process_builtin_call(callee: &[String]) -> bool {
    matches!(
        callee,
        [module, name]
            if module == "process"
                && matches!(name.as_str(), "exit" | "spawn" | "status" | "exec" | "output")
    )
}
