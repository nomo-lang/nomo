use super::*;
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
    let process_child = ValueType::Struct("ProcessChild".to_string(), Vec::new());
    let blocking_process_child = ValueType::Struct("BlockingProcessChild".to_string(), Vec::new());
    let process_command = ValueType::Struct("ProcessCommand".to_string(), Vec::new());
    let process_control_error = ValueType::Struct("ProcessControlError".to_string(), Vec::new());
    let process_event = ValueType::Enum("ProcessEvent".to_string(), Vec::new());
    let process_exit = ValueType::Struct("ProcessExit".to_string(), Vec::new());
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
        [module, name]
            if module == "process" && matches!(name.as_str(), "start" | "start_blocking") =>
        {
            if name == "start" && !current_function_is_suspend(scope) {
                return Err(Diagnostic::new(
                    "E0870",
                    "synchronous function cannot call suspend function `process.start`; mark the caller `suspend` or use `process.start_blocking`",
                    path,
                    span.line,
                    span.column,
                    "process.start(...)".len(),
                    "process.start(...)",
                ));
            }
            let (command_arg, timeout_arg) = match (name.as_str(), args) {
                ("start", [command_arg, timeout_arg]) => (command_arg, Some(timeout_arg)),
                ("start_blocking", [command_arg]) => (command_arg, None),
                ("start", _) => {
                    return Err(Diagnostic::new(
                        "E0407",
                        "`process.start` expects a ProcessCommand and u64 timeout",
                        path,
                        span.line,
                        span.column,
                        span.length,
                        &span.text,
                    ));
                }
                _ => {
                    return Err(Diagnostic::new(
                        "E0407",
                        "`process.start_blocking` expects exactly one ProcessCommand",
                        path,
                        span.line,
                        span.column,
                        span.length,
                        &span.text,
                    ));
                }
            };
            let (command_type, command) = lower_value_expr(
                path,
                command_arg,
                scope,
                imports,
                signatures,
                structs,
                enums,
                span,
            )?;
            if command_type != process_command {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    format!("`process.{name}` expects a ProcessCommand value"),
                    &process_command,
                    &command_type,
                ));
            }
            let mut lowered_args = vec![command];
            if let Some(timeout_arg) = timeout_arg {
                let (timeout_type, timeout) = lower_value_expr_with_expected(
                    path,
                    timeout_arg,
                    scope,
                    imports,
                    signatures,
                    structs,
                    enums,
                    Some(&ValueType::U64),
                    span,
                )?;
                if timeout_type != ValueType::U64 {
                    return Err(type_mismatch_expected_found(
                        path,
                        span,
                        "`process.start` expects a u64 timeout",
                        &ValueType::U64,
                        &timeout_type,
                    ));
                }
                lowered_args.push(timeout);
            }
            let (child_type, builtin) = if name == "start" {
                (process_child.clone(), BUILTIN_PROCESS_START_EXPR)
            } else {
                (
                    blocking_process_child.clone(),
                    BUILTIN_PROCESS_START_BLOCKING_EXPR,
                )
            };
            Ok((
                ValueType::Enum(
                    "Result".to_string(),
                    vec![child_type, process_control_error],
                ),
                ValueExpr::Call {
                    name: builtin.to_string(),
                    args: lowered_args,
                },
            ))
        }
        [module, name]
            if module == "process"
                && matches!(name.as_str(), "write_stdin" | "write_stdin_blocking") =>
        {
            let [child_arg, data_arg] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    format!("`process.{name}` expects a child handle and string payload"),
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let expected_child = if name == "write_stdin" {
                &process_child
            } else {
                &blocking_process_child
            };
            let (child_type, child) = lower_value_expr(
                path, child_arg, scope, imports, signatures, structs, enums, span,
            )?;
            if &child_type != expected_child {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    format!("`process.{name}` expects a {} value", expected_child.name()),
                    expected_child,
                    &child_type,
                ));
            }
            let (data_type, data) = lower_value_expr(
                path, data_arg, scope, imports, signatures, structs, enums, span,
            )?;
            if data_type != ValueType::String {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    "`process.write_stdin` expects a string payload",
                    &ValueType::String,
                    &data_type,
                ));
            }
            Ok((
                ValueType::Enum(
                    "Result".to_string(),
                    vec![ValueType::Void, process_control_error],
                ),
                ValueExpr::Call {
                    name: if name == "write_stdin" {
                        BUILTIN_PROCESS_WRITE_STDIN_EXPR
                    } else {
                        BUILTIN_PROCESS_WRITE_STDIN_BLOCKING_EXPR
                    }
                    .to_string(),
                    args: vec![child, data],
                },
            ))
        }
        [module, name]
            if module == "process"
                && matches!(
                    name.as_str(),
                    "close_stdin"
                        | "try_wait"
                        | "terminate"
                        | "close_child"
                        | "close_stdin_blocking"
                        | "try_wait_blocking"
                        | "terminate_blocking"
                        | "close_child_blocking"
                ) =>
        {
            let [child_arg] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    format!("`process.{name}` expects exactly one child handle"),
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (child_type, child) = lower_value_expr(
                path, child_arg, scope, imports, signatures, structs, enums, span,
            )?;
            let is_blocking = name.ends_with("_blocking");
            let expected_child = if is_blocking {
                &blocking_process_child
            } else {
                &process_child
            };
            if &child_type != expected_child {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    format!("`process.{name}` expects a {} value", expected_child.name()),
                    expected_child,
                    &child_type,
                ));
            }
            match name.as_str() {
                "close_stdin" | "close_stdin_blocking" => Ok((
                    ValueType::Enum(
                        "Result".to_string(),
                        vec![ValueType::Void, process_control_error],
                    ),
                    ValueExpr::Call {
                        name: if is_blocking {
                            BUILTIN_PROCESS_CLOSE_STDIN_BLOCKING_EXPR
                        } else {
                            BUILTIN_PROCESS_CLOSE_STDIN_EXPR
                        }
                        .to_string(),
                        args: vec![child],
                    },
                )),
                "try_wait" | "try_wait_blocking" => Ok((
                    ValueType::Enum(
                        "Result".to_string(),
                        vec![
                            ValueType::Enum("Option".to_string(), vec![process_exit]),
                            process_control_error,
                        ],
                    ),
                    ValueExpr::Call {
                        name: if is_blocking {
                            BUILTIN_PROCESS_TRY_WAIT_BLOCKING_EXPR
                        } else {
                            BUILTIN_PROCESS_TRY_WAIT_EXPR
                        }
                        .to_string(),
                        args: vec![child],
                    },
                )),
                "terminate" | "terminate_blocking" => Ok((
                    ValueType::Enum(
                        "Result".to_string(),
                        vec![ValueType::Void, process_control_error],
                    ),
                    ValueExpr::Call {
                        name: if is_blocking {
                            BUILTIN_PROCESS_TERMINATE_BLOCKING_EXPR
                        } else {
                            BUILTIN_PROCESS_TERMINATE_EXPR
                        }
                        .to_string(),
                        args: vec![child],
                    },
                )),
                "close_child" | "close_child_blocking" => Ok((
                    ValueType::Void,
                    ValueExpr::Call {
                        name: if is_blocking {
                            BUILTIN_PROCESS_CLOSE_CHILD_BLOCKING_EXPR
                        } else {
                            BUILTIN_PROCESS_CLOSE_CHILD_EXPR
                        }
                        .to_string(),
                        args: vec![child],
                    },
                )),
                _ => unreachable!(),
            }
        }
        [module, name]
            if module == "process"
                && matches!(name.as_str(), "next_event" | "next_event_blocking") =>
        {
            if name == "next_event" && !current_function_is_suspend(scope) {
                return Err(Diagnostic::new(
                    "E0870",
                    "synchronous function cannot call suspend function `process.next_event`; mark the caller `suspend` or use `process.next_event_blocking`",
                    path,
                    span.line,
                    span.column,
                    "process.next_event(...)".len(),
                    "process.next_event(...)",
                ));
            }
            let [child_arg, max_chunk_arg, timeout_arg] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    format!("`process.{name}` expects a child handle, chunk limit, and timeout"),
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (child_type, child) = lower_value_expr(
                path, child_arg, scope, imports, signatures, structs, enums, span,
            )?;
            let expected_child = if name == "next_event" {
                &process_child
            } else {
                &blocking_process_child
            };
            if &child_type != expected_child {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    format!("`process.{name}` expects a {} value", expected_child.name()),
                    expected_child,
                    &child_type,
                ));
            }
            let (max_chunk_type, max_chunk) = lower_value_expr_with_expected(
                path,
                max_chunk_arg,
                scope,
                imports,
                signatures,
                structs,
                enums,
                Some(&ValueType::U64),
                span,
            )?;
            if max_chunk_type != ValueType::U64 {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    "`process.next_event` expects a u64 chunk limit",
                    &ValueType::U64,
                    &max_chunk_type,
                ));
            }
            let (timeout_type, timeout) = lower_value_expr_with_expected(
                path,
                timeout_arg,
                scope,
                imports,
                signatures,
                structs,
                enums,
                Some(&ValueType::U64),
                span,
            )?;
            if timeout_type != ValueType::U64 {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    "`process.next_event` expects a u64 timeout",
                    &ValueType::U64,
                    &timeout_type,
                ));
            }
            Ok((
                ValueType::Enum(
                    "Result".to_string(),
                    vec![process_event, process_control_error],
                ),
                ValueExpr::Call {
                    name: if name == "next_event" {
                        BUILTIN_PROCESS_NEXT_EVENT_EXPR
                    } else {
                        BUILTIN_PROCESS_NEXT_EVENT_BLOCKING_EXPR
                    }
                    .to_string(),
                    args: vec![child, max_chunk, timeout],
                },
            ))
        }
        _ => unreachable!("process builtin dispatcher only passes known calls"),
    }
}
pub(super) fn is_process_builtin_call(callee: &[String]) -> bool {
    matches!(
        callee,
        [module, name]
            if module == "process"
                && matches!(
                    name.as_str(),
                    "exit"
                        | "spawn"
                        | "status"
                        | "exec"
                        | "output"
                        | "start"
                        | "write_stdin"
                        | "close_stdin"
                        | "next_event"
                        | "try_wait"
                        | "terminate"
                        | "close_child"
                        | "start_blocking"
                        | "write_stdin_blocking"
                        | "close_stdin_blocking"
                        | "next_event_blocking"
                        | "try_wait_blocking"
                        | "terminate_blocking"
                        | "close_child_blocking"
                )
    )
}
