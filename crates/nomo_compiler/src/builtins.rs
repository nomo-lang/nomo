use super::*;

pub(super) fn is_debug_builtin_call(callee: &[String]) -> bool {
    matches!(
        callee,
        [module, name]
            if module == "debug"
                && matches!(name.as_str(), "print" | "println" | "panic" | "backtrace")
    )
}

pub(super) fn is_log_builtin_call(callee: &[String]) -> bool {
    matches!(
        callee,
        [module, name]
            if module == "log"
                && matches!(name.as_str(), "debug" | "info" | "warn" | "error" | "enabled")
    )
}

pub(super) fn lower_log_builtin(
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
    let [module, name] = callee else {
        unreachable!("log builtin dispatcher only passes qualified calls")
    };
    debug_assert_eq!(module, "log");
    match name.as_str() {
        "enabled" => {
            let [level_arg] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`log.enabled` expects exactly one string level",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (level_type, level) = lower_value_expr(
                path, level_arg, scope, imports, signatures, structs, enums, span,
            )?;
            if level_type != ValueType::String {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    "`log.enabled` expects a string level",
                    &ValueType::String,
                    &level_type,
                ));
            }
            Ok((
                ValueType::Bool,
                ValueExpr::LogEnabled {
                    level: Box::new(level),
                },
            ))
        }
        "debug" | "info" | "warn" | "error" => {
            let [message_arg] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    format!("`log.{name}` expects exactly one string message"),
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (message_type, message) = lower_value_expr(
                path,
                message_arg,
                scope,
                imports,
                signatures,
                structs,
                enums,
                span,
            )?;
            if message_type != ValueType::String {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    format!("`log.{name}` expects a string message"),
                    &ValueType::String,
                    &message_type,
                ));
            }
            Ok((ValueType::Void, log_statement_expr(name, message)))
        }
        _ => unreachable!("log builtin dispatcher only passes known calls"),
    }
}

pub(super) fn log_statement_expr(level: &str, message: ValueExpr) -> ValueExpr {
    let prefix = ValueExpr::StringLiteral(format!("[{level}] "));
    ValueExpr::If {
        condition: Box::new(ValueExpr::LogEnabled {
            level: Box::new(ValueExpr::StringLiteral(level.to_string())),
        }),
        then_branch: Box::new(ValueExpr::Call {
            name: BUILTIN_EPRINTLN_EXPR.to_string(),
            args: vec![ValueExpr::StringConcat {
                left: Box::new(prefix),
                right: Box::new(message),
            }],
        }),
        else_branch: Box::new(ValueExpr::VoidLiteral),
    }
}

pub(super) fn is_hash_builtin_call(callee: &[String]) -> bool {
    matches!(
        callee,
        [module, name]
            if module == "hash"
                && matches!(
                    name.as_str(),
                    "new" | "string" | "bytes" | "write_string" | "write_bytes" | "finish"
                )
    )
}

pub(super) fn is_crypto_builtin_call(callee: &[String]) -> bool {
    matches!(
        callee,
        [module, name]
            if module == "crypto" && matches!(name.as_str(), "sha256" | "sha512" | "random_bytes")
    )
}

pub(super) fn is_json_builtin_call(callee: &[String]) -> bool {
    matches!(
        callee,
        [module, name] if module == "json" && matches!(name.as_str(), "parse" | "stringify")
    )
}

pub(super) fn is_http_builtin_call(callee: &[String]) -> bool {
    matches!(
        callee,
        [module, name]
            if module == "http"
                && matches!(
                    name.as_str(),
                    "get"
                        | "post"
                        | "listen"
                        | "accept"
                        | "respond_string"
                        | "close_server"
                        | "close_exchange"
                )
    )
}

pub(super) fn is_net_builtin_call(callee: &[String]) -> bool {
    matches!(
        callee,
        [module, name]
            if module == "net" && matches!(name.as_str(), "connect" | "listen" | "udp_bind")
    )
}

pub(super) fn is_regex_builtin_call(callee: &[String]) -> bool {
    matches!(
        callee,
        [module, name]
            if module == "regex" && matches!(name.as_str(), "compile" | "is_match" | "captures")
    )
}

pub(super) fn is_collections_builtin_call(callee: &[String]) -> bool {
    matches!(
        callee,
        [module, name]
            if module == "collections"
                && matches!(
                    name.as_str(),
                    "map_new"
                        | "map_len"
                        | "map_get"
                        | "map_contains"
                        | "map_set"
                        | "map_remove"
                        | "set_new"
                        | "set_len"
                        | "set_contains"
                        | "set_insert"
                        | "set_remove"
                )
    )
}

pub(super) fn lower_hash_builtin(
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
    let [module, name] = callee else {
        unreachable!("hash builtin dispatcher only passes qualified calls")
    };
    debug_assert_eq!(module, "hash");
    match name.as_str() {
        "new" => {
            if !args.is_empty() {
                return Err(Diagnostic::new(
                    "E0407",
                    "`hash.new` does not accept arguments",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            }
            Ok((
                ValueType::Struct("HashState".to_string(), Vec::new()),
                ValueExpr::HashNew,
            ))
        }
        "string" => {
            let [value_arg] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`hash.string` expects exactly one string value",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (value_type, value) = lower_value_expr(
                path, value_arg, scope, imports, signatures, structs, enums, span,
            )?;
            if value_type != ValueType::String {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    "`hash.string` expects a string value",
                    &ValueType::String,
                    &value_type,
                ));
            }
            Ok((
                ValueType::U64,
                ValueExpr::HashString {
                    value: Box::new(value),
                },
            ))
        }
        "bytes" => {
            let [value_arg] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`hash.bytes` expects exactly one Array<u32> value",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (value_type, value) = lower_value_expr(
                path, value_arg, scope, imports, signatures, structs, enums, span,
            )?;
            let expected_bytes = ValueType::Array(Box::new(ValueType::U32));
            if value_type != expected_bytes {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    "`hash.bytes` expects an Array<u32> value",
                    &expected_bytes,
                    &value_type,
                ));
            }
            Ok((
                ValueType::U64,
                ValueExpr::HashBytes {
                    value: Box::new(value),
                },
            ))
        }
        "write_string" => {
            let [state_arg, value_arg] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`hash.write_string` expects a HashState and string value",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (state_type, state) = lower_value_expr(
                path, state_arg, scope, imports, signatures, structs, enums, span,
            )?;
            let expected_state = ValueType::Struct("HashState".to_string(), Vec::new());
            if state_type != expected_state {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    "`hash.write_string` expects a HashState value",
                    &expected_state,
                    &state_type,
                ));
            }
            let (value_type, value) = lower_value_expr(
                path, value_arg, scope, imports, signatures, structs, enums, span,
            )?;
            if value_type != ValueType::String {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    "`hash.write_string` expects a string value",
                    &ValueType::String,
                    &value_type,
                ));
            }
            Ok((
                ValueType::Struct("HashState".to_string(), Vec::new()),
                ValueExpr::HashWriteString {
                    state: Box::new(state),
                    value: Box::new(value),
                },
            ))
        }
        "write_bytes" => {
            let [state_arg, value_arg] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`hash.write_bytes` expects a HashState and Array<u32> value",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (state_type, state) = lower_value_expr(
                path, state_arg, scope, imports, signatures, structs, enums, span,
            )?;
            let expected_state = ValueType::Struct("HashState".to_string(), Vec::new());
            if state_type != expected_state {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    "`hash.write_bytes` expects a HashState value",
                    &expected_state,
                    &state_type,
                ));
            }
            let (value_type, value) = lower_value_expr(
                path, value_arg, scope, imports, signatures, structs, enums, span,
            )?;
            let expected_bytes = ValueType::Array(Box::new(ValueType::U32));
            if value_type != expected_bytes {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    "`hash.write_bytes` expects an Array<u32> value",
                    &expected_bytes,
                    &value_type,
                ));
            }
            Ok((
                ValueType::Struct("HashState".to_string(), Vec::new()),
                ValueExpr::HashWriteBytes {
                    state: Box::new(state),
                    value: Box::new(value),
                },
            ))
        }
        "finish" => {
            let [state_arg] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`hash.finish` expects exactly one HashState value",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (state_type, state) = lower_value_expr(
                path, state_arg, scope, imports, signatures, structs, enums, span,
            )?;
            let expected_state = ValueType::Struct("HashState".to_string(), Vec::new());
            if state_type != expected_state {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    "`hash.finish` expects a HashState value",
                    &expected_state,
                    &state_type,
                ));
            }
            Ok((
                ValueType::U64,
                ValueExpr::HashFinish {
                    state: Box::new(state),
                },
            ))
        }
        _ => unreachable!("hash builtin dispatcher only passes known calls"),
    }
}

pub(super) fn lower_crypto_builtin(
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
    let [module, name] = callee else {
        unreachable!("crypto builtin dispatcher only passes qualified calls")
    };
    debug_assert_eq!(module, "crypto");
    if name == "random_bytes" {
        let [count_arg] = args else {
            return Err(Diagnostic::new(
                "E0407",
                "`crypto.random_bytes` expects exactly one u64 count",
                path,
                span.line,
                span.column,
                span.length,
                &span.text,
            ));
        };
        let (count_type, count) = lower_value_expr(
            path, count_arg, scope, imports, signatures, structs, enums, span,
        )?;
        if count_type != ValueType::U64 {
            return Err(type_mismatch_expected_found(
                path,
                span,
                "`crypto.random_bytes` expects a u64 count",
                &ValueType::U64,
                &count_type,
            ));
        }
        return Ok((
            ValueType::Array(Box::new(ValueType::U32)),
            ValueExpr::CryptoRandomBytes {
                count: Box::new(count),
            },
        ));
    }
    let [value_arg] = args else {
        return Err(Diagnostic::new(
            "E0407",
            format!("`crypto.{name}` expects exactly one string value"),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    };
    let (value_type, value) = lower_value_expr(
        path, value_arg, scope, imports, signatures, structs, enums, span,
    )?;
    if value_type != ValueType::String {
        return Err(type_mismatch_expected_found(
            path,
            span,
            format!("`crypto.{name}` expects a string value"),
            &ValueType::String,
            &value_type,
        ));
    }
    let expr = match name.as_str() {
        "sha256" => ValueExpr::CryptoSha256 {
            value: Box::new(value),
        },
        "sha512" => ValueExpr::CryptoSha512 {
            value: Box::new(value),
        },
        _ => unreachable!("crypto builtin dispatcher only passes known calls"),
    };
    Ok((ValueType::String, expr))
}

pub(super) fn lower_json_builtin(
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
    let [module, name] = callee else {
        unreachable!("json builtin dispatcher only passes qualified calls")
    };
    debug_assert_eq!(module, "json");
    let [value_arg] = args else {
        return Err(Diagnostic::new(
            "E0407",
            format!("`json.{name}` expects exactly one argument"),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    };
    let (value_type, value) = lower_value_expr(
        path, value_arg, scope, imports, signatures, structs, enums, span,
    )?;
    match name.as_str() {
        "parse" => {
            if value_type != ValueType::String {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    "`json.parse` expects a string value",
                    &ValueType::String,
                    &value_type,
                ));
            }
            Ok((
                ValueType::Enum(
                    "Result".to_string(),
                    vec![
                        ValueType::Struct("JsonValue".to_string(), Vec::new()),
                        ValueType::Struct("JsonError".to_string(), Vec::new()),
                    ],
                ),
                ValueExpr::JsonParse {
                    value: Box::new(value),
                },
            ))
        }
        "stringify" => {
            let json_value = ValueType::Struct("JsonValue".to_string(), Vec::new());
            if value_type != json_value {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    "`json.stringify` expects a JsonValue value",
                    &json_value,
                    &value_type,
                ));
            }
            Ok((
                ValueType::String,
                ValueExpr::JsonStringify {
                    value: Box::new(value),
                },
            ))
        }
        _ => unreachable!("json builtin dispatcher only passes known calls"),
    }
}

pub(super) fn lower_http_builtin(
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
    let [module, name] = callee else {
        unreachable!("http builtin dispatcher only passes qualified calls")
    };
    debug_assert_eq!(module, "http");
    let http_error = ValueType::Struct("HttpError".to_string(), Vec::new());
    let http_response = ValueType::Struct("HttpResponse".to_string(), Vec::new());
    let http_server = ValueType::Struct("HttpServer".to_string(), Vec::new());
    let http_exchange = ValueType::Struct("HttpExchange".to_string(), Vec::new());
    let response_result_type = ValueType::Enum(
        "Result".to_string(),
        vec![http_response, http_error.clone()],
    );
    match name.as_str() {
        "get" => {
            let [url_arg] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`http.get` expects exactly one URL string",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (url_type, url) = lower_value_expr(
                path, url_arg, scope, imports, signatures, structs, enums, span,
            )?;
            if url_type != ValueType::String {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    "`http.get` expects a string URL",
                    &ValueType::String,
                    &url_type,
                ));
            }
            Ok((
                response_result_type,
                ValueExpr::Call {
                    name: BUILTIN_HTTP_GET_EXPR.to_string(),
                    args: vec![url],
                },
            ))
        }
        "post" => {
            let [url_arg, body_arg] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`http.post` expects URL and body strings",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (url_type, url) = lower_value_expr(
                path, url_arg, scope, imports, signatures, structs, enums, span,
            )?;
            if url_type != ValueType::String {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    "`http.post` expects a string URL",
                    &ValueType::String,
                    &url_type,
                ));
            }
            let (body_type, body) = lower_value_expr(
                path, body_arg, scope, imports, signatures, structs, enums, span,
            )?;
            if body_type != ValueType::String {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    "`http.post` expects a string body",
                    &ValueType::String,
                    &body_type,
                ));
            }
            Ok((
                response_result_type,
                ValueExpr::Call {
                    name: BUILTIN_HTTP_POST_EXPR.to_string(),
                    args: vec![url, body],
                },
            ))
        }
        "listen" => {
            let [host_arg, port_arg] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`http.listen` expects host and port arguments",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (host_type, host) = lower_value_expr(
                path, host_arg, scope, imports, signatures, structs, enums, span,
            )?;
            if host_type != ValueType::String {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    "`http.listen` expects a string host",
                    &ValueType::String,
                    &host_type,
                ));
            }
            let (port_type, port) = lower_value_expr(
                path, port_arg, scope, imports, signatures, structs, enums, span,
            )?;
            if port_type != ValueType::Int {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    "`http.listen` expects an i64 port",
                    &ValueType::Int,
                    &port_type,
                ));
            }
            Ok((
                ValueType::Enum("Result".to_string(), vec![http_server, http_error.clone()]),
                ValueExpr::Call {
                    name: BUILTIN_HTTP_LISTEN_EXPR.to_string(),
                    args: vec![host, port],
                },
            ))
        }
        "accept" => {
            let [server_arg] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`http.accept` expects exactly one HttpServer",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (server_type, server) = lower_value_expr(
                path, server_arg, scope, imports, signatures, structs, enums, span,
            )?;
            if server_type != http_server {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    "`http.accept` expects an HttpServer value",
                    &http_server,
                    &server_type,
                ));
            }
            Ok((
                ValueType::Enum(
                    "Result".to_string(),
                    vec![http_exchange, http_error.clone()],
                ),
                ValueExpr::Call {
                    name: BUILTIN_HTTP_ACCEPT_EXPR.to_string(),
                    args: vec![server],
                },
            ))
        }
        "respond_string" => {
            let [exchange_arg, status_arg, body_arg] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`http.respond_string` expects exchange, status, and body arguments",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (exchange_type, exchange) = lower_value_expr(
                path,
                exchange_arg,
                scope,
                imports,
                signatures,
                structs,
                enums,
                span,
            )?;
            if exchange_type != http_exchange {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    "`http.respond_string` expects an HttpExchange value",
                    &http_exchange,
                    &exchange_type,
                ));
            }
            let (status_type, status) = lower_value_expr(
                path, status_arg, scope, imports, signatures, structs, enums, span,
            )?;
            if status_type != ValueType::Int {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    "`http.respond_string` expects an i64 status",
                    &ValueType::Int,
                    &status_type,
                ));
            }
            let (body_type, body) = lower_value_expr(
                path, body_arg, scope, imports, signatures, structs, enums, span,
            )?;
            if body_type != ValueType::String {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    "`http.respond_string` expects a string body",
                    &ValueType::String,
                    &body_type,
                ));
            }
            Ok((
                ValueType::Enum(
                    "Result".to_string(),
                    vec![ValueType::Void, http_error.clone()],
                ),
                ValueExpr::Call {
                    name: BUILTIN_HTTP_RESPOND_STRING_EXPR.to_string(),
                    args: vec![exchange, status, body],
                },
            ))
        }
        "close_server" => {
            let [server_arg] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`http.close_server` expects exactly one HttpServer",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (server_type, server) = lower_value_expr(
                path, server_arg, scope, imports, signatures, structs, enums, span,
            )?;
            if server_type != http_server {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    "`http.close_server` expects an HttpServer value",
                    &http_server,
                    &server_type,
                ));
            }
            Ok((
                ValueType::Void,
                ValueExpr::Call {
                    name: BUILTIN_HTTP_CLOSE_SERVER_EXPR.to_string(),
                    args: vec![server],
                },
            ))
        }
        "close_exchange" => {
            let [exchange_arg] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`http.close_exchange` expects exactly one HttpExchange",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (exchange_type, exchange) = lower_value_expr(
                path,
                exchange_arg,
                scope,
                imports,
                signatures,
                structs,
                enums,
                span,
            )?;
            if exchange_type != http_exchange {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    "`http.close_exchange` expects an HttpExchange value",
                    &http_exchange,
                    &exchange_type,
                ));
            }
            Ok((
                ValueType::Void,
                ValueExpr::Call {
                    name: BUILTIN_HTTP_CLOSE_EXCHANGE_EXPR.to_string(),
                    args: vec![exchange],
                },
            ))
        }
        _ => unreachable!("http builtin dispatcher only passes known calls"),
    }
}

pub(super) fn lower_net_builtin(
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
    let [module, name] = callee else {
        unreachable!("net builtin dispatcher only passes qualified calls")
    };
    debug_assert_eq!(module, "net");
    let net_error = ValueType::Struct("NetError".to_string(), Vec::new());
    match name.as_str() {
        "connect" | "listen" | "udp_bind" => {
            let [host_arg, port_arg] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    format!("`net.{name}` expects host and port arguments"),
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (host_type, host) = lower_value_expr_with_expected(
                path,
                host_arg,
                scope,
                imports,
                signatures,
                structs,
                enums,
                Some(&ValueType::String),
                span,
            )?;
            if host_type != ValueType::String {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    format!("`net.{name}` expects a string host"),
                    &ValueType::String,
                    &host_type,
                ));
            }
            let (port_type, port) = lower_value_expr_with_expected(
                path,
                port_arg,
                scope,
                imports,
                signatures,
                structs,
                enums,
                Some(&ValueType::Int),
                span,
            )?;
            if port_type != ValueType::Int {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    format!("`net.{name}` expects an i64 port"),
                    &ValueType::Int,
                    &port_type,
                ));
            }
            let ok_type = if name == "connect" {
                ValueType::Struct("TcpStream".to_string(), Vec::new())
            } else if name == "listen" {
                ValueType::Struct("TcpListener".to_string(), Vec::new())
            } else {
                ValueType::Struct("UdpSocket".to_string(), Vec::new())
            };
            let result_type = ValueType::Enum("Result".to_string(), vec![ok_type, net_error]);
            let expr = if name == "connect" {
                ValueExpr::NetConnect {
                    host: Box::new(host),
                    port: Box::new(port),
                }
            } else if name == "listen" {
                ValueExpr::NetListen {
                    host: Box::new(host),
                    port: Box::new(port),
                }
            } else {
                ValueExpr::NetUdpBind {
                    host: Box::new(host),
                    port: Box::new(port),
                }
            };
            Ok((result_type, expr))
        }
        _ => unreachable!("net builtin dispatcher only passes known calls"),
    }
}

pub(super) fn lower_regex_builtin(
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
    let [module, name] = callee else {
        unreachable!("regex builtin dispatcher only passes qualified calls")
    };
    debug_assert_eq!(module, "regex");
    let regex_type = ValueType::Struct("Regex".to_string(), Vec::new());
    let regex_error = ValueType::Struct("RegexError".to_string(), Vec::new());
    match name.as_str() {
        "compile" => {
            let [pattern_arg] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`regex.compile` expects exactly one string pattern",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (pattern_type, pattern) = lower_value_expr(
                path,
                pattern_arg,
                scope,
                imports,
                signatures,
                structs,
                enums,
                span,
            )?;
            if pattern_type != ValueType::String {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    "`regex.compile` expects a string pattern",
                    &ValueType::String,
                    &pattern_type,
                ));
            }
            Ok((
                ValueType::Enum("Result".to_string(), vec![regex_type.clone(), regex_error]),
                ValueExpr::RegexCompile {
                    pattern: Box::new(pattern),
                },
            ))
        }
        "is_match" | "captures" => {
            let [regex_arg, value_arg] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    format!("`regex.{name}` expects a Regex and string value"),
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (actual_regex_type, regex) = lower_value_expr(
                path, regex_arg, scope, imports, signatures, structs, enums, span,
            )?;
            if actual_regex_type != regex_type {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    format!("`regex.{name}` expects a Regex value"),
                    &regex_type,
                    &actual_regex_type,
                ));
            }
            let (value_type, value) = lower_value_expr(
                path, value_arg, scope, imports, signatures, structs, enums, span,
            )?;
            if value_type != ValueType::String {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    format!("`regex.{name}` expects a string value"),
                    &ValueType::String,
                    &value_type,
                ));
            }
            if name == "is_match" {
                Ok((
                    ValueType::Bool,
                    ValueExpr::RegexIsMatch {
                        regex: Box::new(regex),
                        value: Box::new(value),
                    },
                ))
            } else {
                Ok((
                    ValueType::Enum(
                        "Option".to_string(),
                        vec![ValueType::Array(Box::new(ValueType::String))],
                    ),
                    ValueExpr::RegexCaptures {
                        regex: Box::new(regex),
                        value: Box::new(value),
                    },
                ))
            }
        }
        _ => unreachable!("regex builtin dispatcher only passes known calls"),
    }
}

pub(super) fn lower_collections_builtin(
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
    let [module, name] = callee else {
        unreachable!("collections builtin dispatcher only passes qualified calls")
    };
    debug_assert_eq!(module, "collections");
    let map_type = ValueType::Struct("StringMap".to_string(), Vec::new());
    let set_type = ValueType::Struct("StringSet".to_string(), Vec::new());
    match name.as_str() {
        "map_new" => {
            expect_no_args(path, span, "collections.map_new", args)?;
            Ok((map_type, ValueExpr::CollectionsStringMapNew))
        }
        "map_len" => {
            let map = lower_collections_unary_arg(
                path,
                span,
                "collections.map_len",
                args,
                &map_type,
                scope,
                imports,
                signatures,
                structs,
                enums,
            )?;
            Ok((
                ValueType::U64,
                ValueExpr::CollectionsStringMapLen { map: Box::new(map) },
            ))
        }
        "map_get" => {
            let (map, key) = lower_collections_map_key_args(
                path,
                span,
                "collections.map_get",
                args,
                scope,
                imports,
                signatures,
                structs,
                enums,
            )?;
            Ok((
                ValueType::Enum("Option".to_string(), vec![ValueType::String]),
                ValueExpr::CollectionsStringMapGet {
                    map: Box::new(map),
                    key: Box::new(key),
                },
            ))
        }
        "map_contains" => {
            let (map, key) = lower_collections_map_key_args(
                path,
                span,
                "collections.map_contains",
                args,
                scope,
                imports,
                signatures,
                structs,
                enums,
            )?;
            Ok((
                ValueType::Bool,
                ValueExpr::CollectionsStringMapContains {
                    map: Box::new(map),
                    key: Box::new(key),
                },
            ))
        }
        "map_set" => {
            let [map_arg, key_arg, value_arg] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`collections.map_set` expects a StringMap, string key, and string value",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let map = lower_collections_arg(
                path,
                span,
                "collections.map_set",
                map_arg,
                &map_type,
                scope,
                imports,
                signatures,
                structs,
                enums,
            )?;
            let key = lower_collections_arg(
                path,
                span,
                "collections.map_set",
                key_arg,
                &ValueType::String,
                scope,
                imports,
                signatures,
                structs,
                enums,
            )?;
            let value = lower_collections_arg(
                path,
                span,
                "collections.map_set",
                value_arg,
                &ValueType::String,
                scope,
                imports,
                signatures,
                structs,
                enums,
            )?;
            Ok((
                map_type,
                ValueExpr::CollectionsStringMapSet {
                    map: Box::new(map),
                    key: Box::new(key),
                    value: Box::new(value),
                },
            ))
        }
        "map_remove" => {
            let (map, key) = lower_collections_map_key_args(
                path,
                span,
                "collections.map_remove",
                args,
                scope,
                imports,
                signatures,
                structs,
                enums,
            )?;
            Ok((
                map_type,
                ValueExpr::CollectionsStringMapRemove {
                    map: Box::new(map),
                    key: Box::new(key),
                },
            ))
        }
        "set_new" => {
            expect_no_args(path, span, "collections.set_new", args)?;
            Ok((set_type, ValueExpr::CollectionsStringSetNew))
        }
        "set_len" => {
            let set = lower_collections_unary_arg(
                path,
                span,
                "collections.set_len",
                args,
                &set_type,
                scope,
                imports,
                signatures,
                structs,
                enums,
            )?;
            Ok((
                ValueType::U64,
                ValueExpr::CollectionsStringSetLen { set: Box::new(set) },
            ))
        }
        "set_contains" => {
            let (set, value) = lower_collections_set_value_args(
                path,
                span,
                "collections.set_contains",
                args,
                scope,
                imports,
                signatures,
                structs,
                enums,
            )?;
            Ok((
                ValueType::Bool,
                ValueExpr::CollectionsStringSetContains {
                    set: Box::new(set),
                    value: Box::new(value),
                },
            ))
        }
        "set_insert" => {
            let (set, value) = lower_collections_set_value_args(
                path,
                span,
                "collections.set_insert",
                args,
                scope,
                imports,
                signatures,
                structs,
                enums,
            )?;
            Ok((
                set_type,
                ValueExpr::CollectionsStringSetInsert {
                    set: Box::new(set),
                    value: Box::new(value),
                },
            ))
        }
        "set_remove" => {
            let (set, value) = lower_collections_set_value_args(
                path,
                span,
                "collections.set_remove",
                args,
                scope,
                imports,
                signatures,
                structs,
                enums,
            )?;
            Ok((
                set_type,
                ValueExpr::CollectionsStringSetRemove {
                    set: Box::new(set),
                    value: Box::new(value),
                },
            ))
        }
        _ => unreachable!("collections builtin dispatcher only passes known calls"),
    }
}

pub(super) fn expect_no_args(
    path: &Path,
    span: &Span,
    callable: &str,
    args: &[AstExpr],
) -> Result<(), Diagnostic> {
    if args.is_empty() {
        return Ok(());
    }
    Err(Diagnostic::new(
        "E0407",
        format!("`{callable}` does not accept arguments"),
        path,
        span.line,
        span.column,
        span.length,
        &span.text,
    ))
}

#[allow(clippy::too_many_arguments)]
pub(super) fn lower_collections_unary_arg(
    path: &Path,
    span: &Span,
    callable: &str,
    args: &[AstExpr],
    expected: &ValueType,
    scope: &HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
) -> Result<ValueExpr, Diagnostic> {
    let [arg] = args else {
        return Err(Diagnostic::new(
            "E0407",
            format!("`{callable}` expects exactly one argument"),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    };
    lower_collections_arg(
        path, span, callable, arg, expected, scope, imports, signatures, structs, enums,
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn lower_collections_map_key_args(
    path: &Path,
    span: &Span,
    callable: &str,
    args: &[AstExpr],
    scope: &HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
) -> Result<(ValueExpr, ValueExpr), Diagnostic> {
    let [map_arg, key_arg] = args else {
        return Err(Diagnostic::new(
            "E0407",
            format!("`{callable}` expects a StringMap and string key"),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    };
    let map_type = ValueType::Struct("StringMap".to_string(), Vec::new());
    let map = lower_collections_arg(
        path, span, callable, map_arg, &map_type, scope, imports, signatures, structs, enums,
    )?;
    let key = lower_collections_arg(
        path,
        span,
        callable,
        key_arg,
        &ValueType::String,
        scope,
        imports,
        signatures,
        structs,
        enums,
    )?;
    Ok((map, key))
}

#[allow(clippy::too_many_arguments)]
pub(super) fn lower_collections_set_value_args(
    path: &Path,
    span: &Span,
    callable: &str,
    args: &[AstExpr],
    scope: &HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
) -> Result<(ValueExpr, ValueExpr), Diagnostic> {
    let [set_arg, value_arg] = args else {
        return Err(Diagnostic::new(
            "E0407",
            format!("`{callable}` expects a StringSet and string value"),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    };
    let set_type = ValueType::Struct("StringSet".to_string(), Vec::new());
    let set = lower_collections_arg(
        path, span, callable, set_arg, &set_type, scope, imports, signatures, structs, enums,
    )?;
    let value = lower_collections_arg(
        path,
        span,
        callable,
        value_arg,
        &ValueType::String,
        scope,
        imports,
        signatures,
        structs,
        enums,
    )?;
    Ok((set, value))
}

#[allow(clippy::too_many_arguments)]
pub(super) fn lower_collections_arg(
    path: &Path,
    span: &Span,
    callable: &str,
    arg: &AstExpr,
    expected: &ValueType,
    scope: &HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
) -> Result<ValueExpr, Diagnostic> {
    let (actual, lowered) = lower_value_expr_with_expected(
        path,
        arg,
        scope,
        imports,
        signatures,
        structs,
        enums,
        Some(expected),
        span,
    )?;
    if &actual != expected {
        return Err(type_mismatch_expected_found(
            path,
            span,
            format!(
                "`{callable}` argument is `{}` but expected `{}`",
                actual.name(),
                expected.name()
            ),
            expected,
            &actual,
        ));
    }
    Ok(lowered)
}

pub(super) fn lower_debug_builtin(
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
    let [module, name] = callee else {
        unreachable!("debug builtin dispatcher only passes qualified calls")
    };
    debug_assert_eq!(module, "debug");
    match name.as_str() {
        "print" | "println" | "panic" => {
            let [message_arg] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    format!("`debug.{name}` expects exactly one string message"),
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (message_type, message) = lower_value_expr(
                path,
                message_arg,
                scope,
                imports,
                signatures,
                structs,
                enums,
                span,
            )?;
            if message_type != ValueType::String {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    format!("`debug.{name}` expects a string message"),
                    &ValueType::String,
                    &message_type,
                ));
            }
            let value = match name.as_str() {
                "print" => ValueExpr::Call {
                    name: BUILTIN_EPRINT_EXPR.to_string(),
                    args: vec![message],
                },
                "println" => ValueExpr::Call {
                    name: BUILTIN_EPRINTLN_EXPR.to_string(),
                    args: vec![message],
                },
                "panic" => ValueExpr::Panic {
                    message: Box::new(message),
                    fallback_type: ValueType::Void,
                },
                _ => unreachable!("debug string helper matched above"),
            };
            Ok((ValueType::Void, value))
        }
        "backtrace" => {
            if !args.is_empty() {
                return Err(Diagnostic::new(
                    "E0407",
                    "`debug.backtrace` does not accept arguments",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            }
            Ok((
                ValueType::String,
                ValueExpr::StringLiteral("backtrace unavailable".to_string()),
            ))
        }
        _ => unreachable!("debug builtin dispatcher only passes known calls"),
    }
}

pub(super) fn is_testing_builtin_call(callee: &[String]) -> bool {
    matches!(
        callee,
        [module, name]
            if module == "testing"
                && matches!(name.as_str(), "assert" | "assert_equal" | "assert_error")
    )
}

pub(super) fn lower_testing_builtin(
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
    let [module, name] = callee else {
        unreachable!("testing builtin dispatcher only passes qualified calls")
    };
    debug_assert_eq!(module, "testing");
    match name.as_str() {
        "assert" => {
            let [condition_arg, message_arg] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`testing.assert` expects a bool condition and string message",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (condition_type, condition) = lower_value_expr(
                path,
                condition_arg,
                scope,
                imports,
                signatures,
                structs,
                enums,
                span,
            )?;
            if condition_type != ValueType::Bool {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    "`testing.assert` expects a bool condition",
                    &ValueType::Bool,
                    &condition_type,
                ));
            }
            let (message_type, message) = lower_value_expr(
                path,
                message_arg,
                scope,
                imports,
                signatures,
                structs,
                enums,
                span,
            )?;
            if message_type != ValueType::String {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    "`testing.assert` expects a string message",
                    &ValueType::String,
                    &message_type,
                ));
            }
            Ok((ValueType::Void, assert_expr(condition, message)))
        }
        "assert_equal" => {
            let [left_arg, right_arg] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`testing.assert_equal` expects two comparable values",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (left_type, left) = lower_value_expr(
                path, left_arg, scope, imports, signatures, structs, enums, span,
            )?;
            let (right_type, right) = lower_value_expr(
                path, right_arg, scope, imports, signatures, structs, enums, span,
            )?;
            if left_type != right_type {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    "`testing.assert_equal` expects both values to have the same type",
                    &left_type,
                    &right_type,
                ));
            }
            let condition = equality_expr(left, right, &left_type).ok_or_else(|| {
                type_mismatch(
                    path,
                    span,
                    format!(
                        "`testing.assert_equal` does not support values of type `{}`",
                        left_type.name()
                    ),
                )
            })?;
            Ok((
                ValueType::Void,
                assert_expr(
                    condition,
                    ValueExpr::StringLiteral("assert_equal failed".to_string()),
                ),
            ))
        }
        "assert_error" => {
            let [result_arg] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`testing.assert_error` expects one Result<T, E> value",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (result_type, result) = lower_value_expr(
                path, result_arg, scope, imports, signatures, structs, enums, span,
            )?;
            let ValueType::Enum(enum_name, enum_args) = result_type.clone() else {
                return Err(type_mismatch(
                    path,
                    span,
                    "`testing.assert_error` expects a Result<T, E> value",
                ));
            };
            if enum_name != "Result" || enum_args.len() != 2 {
                return Err(type_mismatch(
                    path,
                    span,
                    "`testing.assert_error` expects a Result<T, E> value",
                ));
            }
            let condition = ValueExpr::ResultIsErr {
                result: Box::new(result),
                ok_type: enum_args[0].clone(),
                err_type: enum_args[1].clone(),
            };
            Ok((
                ValueType::Void,
                assert_expr(
                    condition,
                    ValueExpr::StringLiteral("expected Result.Err".to_string()),
                ),
            ))
        }
        _ => unreachable!("testing builtin dispatcher only passes known calls"),
    }
}

pub(super) fn assert_expr(condition: ValueExpr, message: ValueExpr) -> ValueExpr {
    ValueExpr::If {
        condition: Box::new(condition),
        then_branch: Box::new(ValueExpr::VoidLiteral),
        else_branch: Box::new(ValueExpr::Panic {
            message: Box::new(message),
            fallback_type: ValueType::Void,
        }),
    }
}

pub(super) fn equality_expr(
    left: ValueExpr,
    right: ValueExpr,
    value_type: &ValueType,
) -> Option<ValueExpr> {
    match value_type {
        ValueType::String => Some(ValueExpr::StringCompare {
            left: Box::new(left),
            op: BinaryOp::Equal,
            right: Box::new(right),
        }),
        ValueType::Char
        | ValueType::Bool
        | ValueType::Int
        | ValueType::I32
        | ValueType::U64
        | ValueType::Float => Some(ValueExpr::Binary {
            left: Box::new(left),
            op: BinaryOp::Equal,
            right: Box::new(right),
            value_type: value_type.clone(),
        }),
        _ => None,
    }
}

pub(super) fn is_path_builtin_call(callee: &[String]) -> bool {
    matches!(
        callee,
        [module, name]
            if module == "path"
                && matches!(
                    name.as_str(),
                    "join" | "basename" | "dirname" | "extension" | "normalize" | "is_absolute"
                )
    )
}

pub(super) fn lower_path_builtin(
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
    let [module, name] = callee else {
        unreachable!("path builtin dispatcher only passes qualified calls")
    };
    debug_assert_eq!(module, "path");
    match name.as_str() {
        "join" => {
            let [left, right] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`path.join` expects exactly two string arguments",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (left_type, lowered_left) =
                lower_value_expr(path, left, scope, imports, signatures, structs, enums, span)?;
            let (right_type, lowered_right) = lower_value_expr(
                path, right, scope, imports, signatures, structs, enums, span,
            )?;
            if left_type != ValueType::String || right_type != ValueType::String {
                return Err(type_mismatch(path, span, "`path.join` expects two strings"));
            }
            Ok((
                ValueType::String,
                ValueExpr::PathJoin {
                    left: Box::new(lowered_left),
                    right: Box::new(lowered_right),
                },
            ))
        }
        "basename" | "dirname" | "extension" | "normalize" | "is_absolute" => {
            let [path_arg] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    format!("`path.{name}` expects exactly one string argument"),
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
                    format!("`path.{name}` expects a string"),
                ));
            }
            let return_type = if name == "is_absolute" {
                ValueType::Bool
            } else {
                ValueType::String
            };
            let lowered = match name.as_str() {
                "basename" => ValueExpr::PathBasename {
                    path: Box::new(lowered_path),
                },
                "dirname" => ValueExpr::PathDirname {
                    path: Box::new(lowered_path),
                },
                "extension" => ValueExpr::PathExtension {
                    path: Box::new(lowered_path),
                },
                "normalize" => ValueExpr::PathNormalize {
                    path: Box::new(lowered_path),
                },
                "is_absolute" => ValueExpr::PathIsAbsolute {
                    path: Box::new(lowered_path),
                },
                _ => unreachable!("path builtin dispatcher only passes known calls"),
            };
            Ok((return_type, lowered))
        }
        _ => unreachable!("path builtin dispatcher only passes known calls"),
    }
}

pub(super) fn is_math_builtin_call(callee: &[String]) -> bool {
    matches!(
        callee,
        [module, name]
            if module == "math"
                && matches!(
                    name.as_str(),
                    "abs"
                        | "min"
                        | "max"
                        | "floor"
                        | "ceil"
                        | "round"
                        | "sqrt"
                        | "pow"
                        | "sin"
                        | "cos"
                )
    )
}

pub(super) fn is_char_builtin_call(callee: &[String]) -> bool {
    matches!(
        callee,
        [module, name]
            if module == "char"
                && matches!(
                    name.as_str(),
                    "is_digit" | "is_alpha" | "is_whitespace" | "to_string"
                )
    )
}

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

pub(super) fn is_time_builtin_call(callee: &[String]) -> bool {
    matches!(
        callee,
        [module, name]
            if module == "time"
                && matches!(
                    name.as_str(),
                    "now_millis"
                        | "monotonic_millis"
                        | "duration_millis"
                        | "duration_seconds"
                        | "duration_as_millis"
                        | "format_duration"
                        | "sleep"
                        | "sleep_millis"
                )
    )
}

pub(super) fn is_num_builtin_call(callee: &[String]) -> bool {
    matches!(
        callee,
        [module, name]
            if module == "num"
                && matches!(
                    name.as_str(),
                    "parse_i64"
                        | "parse_u64"
                        | "parse_f64"
                        | "to_string"
                        | "checked_add"
                        | "checked_sub"
                        | "checked_mul"
                        | "wrapping_add"
                        | "wrapping_sub"
                        | "wrapping_mul"
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

pub(super) fn lower_time_builtin(
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
    let [module, name] = callee else {
        unreachable!("time builtin dispatcher only passes qualified calls")
    };
    debug_assert_eq!(module, "time");
    match name.as_str() {
        "now_millis" => {
            if !args.is_empty() {
                return Err(Diagnostic::new(
                    "E0407",
                    "`time.now_millis` does not accept arguments",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            }
            Ok((ValueType::Int, ValueExpr::TimeNowMillis))
        }
        "monotonic_millis" => {
            if !args.is_empty() {
                return Err(Diagnostic::new(
                    "E0407",
                    "`time.monotonic_millis` does not accept arguments",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            }
            Ok((ValueType::Int, ValueExpr::TimeMonotonicMillis))
        }
        "duration_millis" => {
            let [millis] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`time.duration_millis` expects exactly one i64 millisecond value",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (millis_type, lowered_millis) = lower_value_expr(
                path, millis, scope, imports, signatures, structs, enums, span,
            )?;
            if millis_type != ValueType::Int {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    "`time.duration_millis` expects an i64 millisecond value",
                    &ValueType::Int,
                    &millis_type,
                ));
            }
            Ok((
                ValueType::Struct("Duration".to_string(), Vec::new()),
                ValueExpr::TimeDurationMillis {
                    millis: Box::new(lowered_millis),
                },
            ))
        }
        "duration_seconds" => {
            let [seconds] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`time.duration_seconds` expects exactly one i64 second value",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (seconds_type, lowered_seconds) = lower_value_expr(
                path, seconds, scope, imports, signatures, structs, enums, span,
            )?;
            if seconds_type != ValueType::Int {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    "`time.duration_seconds` expects an i64 second value",
                    &ValueType::Int,
                    &seconds_type,
                ));
            }
            Ok((
                ValueType::Struct("Duration".to_string(), Vec::new()),
                ValueExpr::TimeDurationSeconds {
                    seconds: Box::new(lowered_seconds),
                },
            ))
        }
        "duration_as_millis" => {
            let [duration] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`time.duration_as_millis` expects exactly one Duration value",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (duration_type, lowered_duration) = lower_value_expr(
                path, duration, scope, imports, signatures, structs, enums, span,
            )?;
            let expected = ValueType::Struct("Duration".to_string(), Vec::new());
            if duration_type != expected {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    "`time.duration_as_millis` expects a Duration value",
                    &expected,
                    &duration_type,
                ));
            }
            Ok((
                ValueType::Int,
                ValueExpr::TimeDurationAsMillis {
                    duration: Box::new(lowered_duration),
                },
            ))
        }
        "format_duration" => {
            let [duration] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`time.format_duration` expects exactly one Duration value",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (duration_type, lowered_duration) = lower_value_expr(
                path, duration, scope, imports, signatures, structs, enums, span,
            )?;
            let expected = ValueType::Struct("Duration".to_string(), Vec::new());
            if duration_type != expected {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    "`time.format_duration` expects a Duration value",
                    &expected,
                    &duration_type,
                ));
            }
            Ok((
                ValueType::String,
                ValueExpr::TimeFormatDuration {
                    duration: Box::new(lowered_duration),
                },
            ))
        }
        "sleep" => {
            let [duration] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`time.sleep` expects exactly one Duration value",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (duration_type, lowered_duration) = lower_value_expr(
                path, duration, scope, imports, signatures, structs, enums, span,
            )?;
            let expected = ValueType::Struct("Duration".to_string(), Vec::new());
            if duration_type != expected {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    "`time.sleep` expects a Duration value",
                    &expected,
                    &duration_type,
                ));
            }
            Ok((
                ValueType::Void,
                ValueExpr::TimeSleep {
                    duration: Box::new(lowered_duration),
                },
            ))
        }
        "sleep_millis" => {
            let [duration] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`time.sleep_millis` expects exactly one i64 duration in milliseconds",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (duration_type, lowered_duration) = lower_value_expr(
                path, duration, scope, imports, signatures, structs, enums, span,
            )?;
            if duration_type != ValueType::Int {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    "`time.sleep_millis` expects an i64 duration in milliseconds",
                    &ValueType::Int,
                    &duration_type,
                ));
            }
            Ok((
                ValueType::Void,
                ValueExpr::TimeSleepMillis {
                    duration: Box::new(lowered_duration),
                },
            ))
        }
        _ => unreachable!("time builtin dispatcher only passes known calls"),
    }
}

pub(super) fn lower_num_builtin(
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
    let [module, name] = callee else {
        unreachable!("num builtin dispatcher only passes qualified calls")
    };
    debug_assert_eq!(module, "num");
    let num_error = ValueType::Struct("NumError".to_string(), Vec::new());
    match name.as_str() {
        "parse_i64" | "parse_u64" | "parse_f64" => {
            let [value] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    format!("`num.{name}` expects exactly one argument"),
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (value_type, lowered_value) = lower_value_expr(
                path, value, scope, imports, signatures, structs, enums, span,
            )?;
            if value_type != ValueType::String {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    format!("`num.{name}` expects a string argument"),
                    &ValueType::String,
                    &value_type,
                ));
            }
            let (ok_type, expr) = match name.as_str() {
                "parse_i64" => (
                    ValueType::Int,
                    ValueExpr::NumParseI64 {
                        value: Box::new(lowered_value),
                    },
                ),
                "parse_u64" => (
                    ValueType::U64,
                    ValueExpr::NumParseU64 {
                        value: Box::new(lowered_value),
                    },
                ),
                "parse_f64" => (
                    ValueType::Float,
                    ValueExpr::NumParseF64 {
                        value: Box::new(lowered_value),
                    },
                ),
                _ => unreachable!("num parse dispatcher only passes known calls"),
            };
            Ok((
                ValueType::Enum("Result".to_string(), vec![ok_type, num_error]),
                expr,
            ))
        }
        "to_string" => {
            let [value] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`num.to_string` expects exactly one argument",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (value_type, lowered_value) = lower_value_expr(
                path, value, scope, imports, signatures, structs, enums, span,
            )?;
            if !matches!(
                value_type,
                ValueType::Int
                    | ValueType::I32
                    | ValueType::U32
                    | ValueType::U64
                    | ValueType::Float
            ) {
                return Err(type_mismatch(
                    path,
                    span,
                    "`num.to_string` expects an i64, i32, u32, u64, or f64 value",
                ));
            }
            Ok((
                ValueType::String,
                ValueExpr::NumToString {
                    value: Box::new(lowered_value),
                    value_type,
                },
            ))
        }
        "checked_add" | "checked_sub" | "checked_mul" | "wrapping_add" | "wrapping_sub"
        | "wrapping_mul" => {
            let [left, right] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    format!("`num.{name}` expects exactly two integer arguments"),
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let ((left_type, lowered_left), (right_type, lowered_right)) = lower_binary_operands(
                path, left, right, scope, imports, signatures, structs, enums, span,
            )?;
            if left_type != right_type || !left_type.is_integer() {
                return Err(type_mismatch(
                    path,
                    span,
                    format!("`num.{name}` expects two matching integer operands"),
                ));
            }
            let op = match name.as_str() {
                "checked_add" | "wrapping_add" => BinaryOp::Add,
                "checked_sub" | "wrapping_sub" => BinaryOp::Subtract,
                "checked_mul" | "wrapping_mul" => BinaryOp::Multiply,
                _ => unreachable!("num binary dispatcher only passes known calls"),
            };
            let function = if name.starts_with("checked_") {
                NumBinaryFunction::Checked
            } else {
                NumBinaryFunction::Wrapping
            };
            let result_type = if function == NumBinaryFunction::Checked {
                ValueType::Enum("Option".to_string(), vec![left_type.clone()])
            } else {
                left_type.clone()
            };
            Ok((
                result_type,
                ValueExpr::NumBinary {
                    function,
                    op,
                    left: Box::new(lowered_left),
                    right: Box::new(lowered_right),
                    value_type: left_type,
                },
            ))
        }
        _ => unreachable!("num builtin dispatcher only passes known calls"),
    }
}

pub(super) fn lower_char_builtin(
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
    let [module, name] = callee else {
        unreachable!("char builtin dispatcher only passes qualified calls")
    };
    debug_assert_eq!(module, "char");
    let [value] = args else {
        return Err(Diagnostic::new(
            "E0407",
            format!("`char.{name}` expects exactly one char argument"),
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
    if value_type != ValueType::Char {
        return Err(type_mismatch_expected_found(
            path,
            span,
            format!("`char.{name}` expects a char value"),
            &ValueType::Char,
            &value_type,
        ));
    }
    let expr = match name.as_str() {
        "is_digit" => ValueExpr::CharIsDigit {
            value: Box::new(lowered),
        },
        "is_alpha" => ValueExpr::CharIsAlpha {
            value: Box::new(lowered),
        },
        "is_whitespace" => ValueExpr::CharIsWhitespace {
            value: Box::new(lowered),
        },
        "to_string" => ValueExpr::CharToString {
            value: Box::new(lowered),
        },
        _ => unreachable!("char builtin dispatcher only passes known calls"),
    };
    let return_type = if name == "to_string" {
        ValueType::String
    } else {
        ValueType::Bool
    };
    Ok((return_type, expr))
}

pub(super) fn lower_math_builtin(
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
    let [module, name] = callee else {
        unreachable!("math builtin dispatcher only passes qualified calls")
    };
    debug_assert_eq!(module, "math");
    match name.as_str() {
        "abs" => {
            let [value] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`math.abs` expects exactly one numeric argument",
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
            if !value_type.is_numeric() {
                return Err(type_mismatch(
                    path,
                    span,
                    "`math.abs` expects a numeric value",
                ));
            }
            Ok((
                value_type.clone(),
                ValueExpr::MathUnary {
                    function: MathUnaryFunction::Abs,
                    value: Box::new(lowered),
                    value_type,
                },
            ))
        }
        "floor" | "ceil" | "round" | "sqrt" | "sin" | "cos" => {
            let [value] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    format!("`math.{name}` expects exactly one f64 argument"),
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
            if value_type != ValueType::Float {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    format!("`math.{name}` expects an f64 value"),
                    &ValueType::Float,
                    &value_type,
                ));
            }
            let function = match name.as_str() {
                "floor" => MathUnaryFunction::Floor,
                "ceil" => MathUnaryFunction::Ceil,
                "round" => MathUnaryFunction::Round,
                "sqrt" => MathUnaryFunction::Sqrt,
                "sin" => MathUnaryFunction::Sin,
                "cos" => MathUnaryFunction::Cos,
                _ => unreachable!("math builtin dispatcher only passes known calls"),
            };
            Ok((
                ValueType::Float,
                ValueExpr::MathUnary {
                    function,
                    value: Box::new(lowered),
                    value_type: ValueType::Float,
                },
            ))
        }
        "min" | "max" => {
            let [left, right] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    format!("`math.{name}` expects exactly two matching numeric arguments"),
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (left_type, lowered_left) =
                lower_value_expr(path, left, scope, imports, signatures, structs, enums, span)?;
            let (right_type, lowered_right) = lower_value_expr(
                path, right, scope, imports, signatures, structs, enums, span,
            )?;
            if left_type != right_type || !left_type.is_numeric() {
                return Err(type_mismatch(
                    path,
                    span,
                    format!("`math.{name}` expects two matching numeric values"),
                ));
            }
            let function = if name == "min" {
                MathBinaryFunction::Min
            } else {
                MathBinaryFunction::Max
            };
            Ok((
                left_type.clone(),
                ValueExpr::MathBinary {
                    function,
                    left: Box::new(lowered_left),
                    right: Box::new(lowered_right),
                    value_type: left_type,
                },
            ))
        }
        "pow" => {
            let [left, right] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`math.pow` expects exactly two f64 arguments",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (left_type, lowered_left) =
                lower_value_expr(path, left, scope, imports, signatures, structs, enums, span)?;
            let (right_type, lowered_right) = lower_value_expr(
                path, right, scope, imports, signatures, structs, enums, span,
            )?;
            if left_type != ValueType::Float || right_type != ValueType::Float {
                return Err(type_mismatch(
                    path,
                    span,
                    "`math.pow` expects two f64 values",
                ));
            }
            Ok((
                ValueType::Float,
                ValueExpr::MathBinary {
                    function: MathBinaryFunction::Pow,
                    left: Box::new(lowered_left),
                    right: Box::new(lowered_right),
                    value_type: ValueType::Float,
                },
            ))
        }
        _ => unreachable!("math builtin dispatcher only passes known calls"),
    }
}

pub(super) fn lower_array_new(
    path: &Path,
    type_args: &[crate::ast::TypeRef],
    args: &[AstExpr],
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    span: &Span,
) -> Result<(ValueType, ValueExpr), Diagnostic> {
    let [type_arg] = type_args else {
        return Err(Diagnostic::new(
            "E0407",
            "`Array.new` expects exactly one type argument",
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    };
    if !args.is_empty() {
        return Err(Diagnostic::new(
            "E0407",
            "`Array.new<T>()` does not accept value arguments",
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    }
    let element_type = parse_value_type(type_arg, structs, enums).ok_or_else(|| {
        unsupported_type_diagnostic_from_maps(
            path,
            span,
            type_arg,
            "unsupported Array element type",
            structs,
            enums,
        )
    })?;
    ensure_supported_array_element(path, &element_type, span)?;
    Ok((
        ValueType::Array(Box::new(element_type.clone())),
        ValueExpr::ArrayNew { element_type },
    ))
}

pub(super) fn is_array_value_method(callee: &[String], scope: &HashMap<String, Binding>) -> bool {
    if callee.len() != 2 {
        return false;
    }
    scope
        .get(&callee[0])
        .is_some_and(|binding| matches!(binding.value_type, ValueType::Array(_)))
}

pub(super) fn lower_array_value_method(
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
    require_array_method_import(path, imports, span, method)?;
    let binding = scope.get(name).expect("array method receiver is in scope");
    let receiver_expr = binding_value_expr(name, binding);
    let ValueType::Array(element_type) = &binding.value_type else {
        unreachable!("array method dispatcher only passes arrays");
    };
    ensure_supported_array_element(path, element_type, span)?;
    match method.as_str() {
        "len" => {
            if !args.is_empty() {
                return Err(Diagnostic::new(
                    "E0407",
                    "`Array.len` does not accept arguments",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            }
            Ok((
                ValueType::U64,
                ValueExpr::ArrayLen {
                    array: Box::new(receiver_expr),
                },
            ))
        }
        "iter" => {
            if !args.is_empty() {
                return Err(Diagnostic::new(
                    "E0407",
                    "`Array.iter` does not accept arguments",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            }
            Ok((
                ValueType::Array(Box::new(element_type.as_ref().clone())),
                ValueExpr::ArrayIter {
                    array: Box::new(receiver_expr),
                    element_type: element_type.as_ref().clone(),
                },
            ))
        }
        "get" => {
            let [index] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`Array.get` expects exactly one index",
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
                return Err(type_mismatch(path, span, "`Array.get` index must be `u64`"));
            }
            Ok((
                ValueType::Enum("Option".to_string(), vec![element_type.as_ref().clone()]),
                ValueExpr::ArrayGet {
                    array: Box::new(receiver_expr),
                    index: Box::new(lowered_index),
                    element_type: element_type.as_ref().clone(),
                },
            ))
        }
        "pop" => {
            if !args.is_empty() {
                return Err(Diagnostic::new(
                    "E0407",
                    "`Array.pop` does not accept arguments",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            }
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
            Ok((
                ValueType::Enum("Option".to_string(), vec![element_type.as_ref().clone()]),
                ValueExpr::ArrayPop {
                    array: name.clone(),
                    element_type: element_type.as_ref().clone(),
                },
            ))
        }
        "remove" => {
            let [index] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`Array.remove` expects exactly one index",
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
                    "`Array.remove` index must be `u64`",
                ));
            }
            Ok((
                ValueType::Enum("Option".to_string(), vec![element_type.as_ref().clone()]),
                ValueExpr::ArrayRemove {
                    array: name.clone(),
                    index: Box::new(lowered_index),
                    element_type: element_type.as_ref().clone(),
                },
            ))
        }
        _ => Err(Diagnostic::new(
            "E0305",
            format!("unknown Array method `{method}`"),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        )),
    }
}

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

pub(super) fn is_tcp_stream_value_method(
    callee: &[String],
    scope: &HashMap<String, Binding>,
) -> bool {
    if callee.len() != 2 {
        return false;
    }
    scope.get(&callee[0]).is_some_and(|binding| {
        binding.value_type == ValueType::Struct("TcpStream".to_string(), Vec::new())
    })
}

pub(super) fn lower_tcp_stream_value_method(
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
    let binding = scope
        .get(name)
        .expect("tcp stream method receiver is in scope");
    let receiver_expr = binding_value_expr(name, binding);
    let net_error = ValueType::Struct("NetError".to_string(), Vec::new());
    match method.as_str() {
        "close" => {
            if !args.is_empty() {
                return Err(Diagnostic::new(
                    "E0407",
                    "`TcpStream.close` does not accept arguments",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            }
            Ok((
                ValueType::Void,
                ValueExpr::TcpStreamClose {
                    stream: Box::new(receiver_expr),
                },
            ))
        }
        "read_to_string" => {
            if !args.is_empty() {
                return Err(Diagnostic::new(
                    "E0407",
                    "`TcpStream.read_to_string` does not accept arguments",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            }
            Ok((
                ValueType::Enum("Result".to_string(), vec![ValueType::String, net_error]),
                ValueExpr::TcpStreamReadToString {
                    stream: Box::new(receiver_expr),
                },
            ))
        }
        "write_string" => {
            let [content_arg] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`TcpStream.write_string` expects exactly one content string",
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
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    "`TcpStream.write_string` expects string content",
                    &ValueType::String,
                    &content_type,
                ));
            }
            Ok((
                ValueType::Enum("Result".to_string(), vec![ValueType::Void, net_error]),
                ValueExpr::TcpStreamWriteString {
                    stream: Box::new(receiver_expr),
                    content: Box::new(lowered_content),
                },
            ))
        }
        _ => Err(Diagnostic::new(
            "E0305",
            format!("unknown TcpStream method `{method}`"),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        )),
    }
}

pub(super) fn is_tcp_listener_value_method(
    callee: &[String],
    scope: &HashMap<String, Binding>,
) -> bool {
    if callee.len() != 2 {
        return false;
    }
    scope.get(&callee[0]).is_some_and(|binding| {
        binding.value_type == ValueType::Struct("TcpListener".to_string(), Vec::new())
    })
}

pub(super) fn lower_tcp_listener_value_method(
    path: &Path,
    callee: &[String],
    args: &[AstExpr],
    scope: &HashMap<String, Binding>,
    span: &Span,
) -> Result<(ValueType, ValueExpr), Diagnostic> {
    let name = &callee[0];
    let method = &callee[1];
    let binding = scope
        .get(name)
        .expect("tcp listener method receiver is in scope");
    let receiver_expr = binding_value_expr(name, binding);
    let net_error = ValueType::Struct("NetError".to_string(), Vec::new());
    match method.as_str() {
        "accept" => {
            if !args.is_empty() {
                return Err(Diagnostic::new(
                    "E0407",
                    "`TcpListener.accept` does not accept arguments",
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
                        ValueType::Struct("TcpStream".to_string(), Vec::new()),
                        net_error,
                    ],
                ),
                ValueExpr::TcpListenerAccept {
                    listener: Box::new(receiver_expr),
                },
            ))
        }
        "close" => {
            if !args.is_empty() {
                return Err(Diagnostic::new(
                    "E0407",
                    "`TcpListener.close` does not accept arguments",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            }
            Ok((
                ValueType::Void,
                ValueExpr::TcpListenerClose {
                    listener: Box::new(receiver_expr),
                },
            ))
        }
        _ => Err(Diagnostic::new(
            "E0305",
            format!("unknown TcpListener method `{method}`"),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        )),
    }
}

pub(super) fn is_udp_socket_value_method(
    callee: &[String],
    scope: &HashMap<String, Binding>,
) -> bool {
    if callee.len() != 2 {
        return false;
    }
    scope.get(&callee[0]).is_some_and(|binding| {
        binding.value_type == ValueType::Struct("UdpSocket".to_string(), Vec::new())
    })
}

pub(super) fn lower_udp_socket_value_method(
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
    let binding = scope
        .get(name)
        .expect("udp socket method receiver is in scope");
    let receiver_expr = binding_value_expr(name, binding);
    let net_error = ValueType::Struct("NetError".to_string(), Vec::new());
    match method.as_str() {
        "close" => {
            if !args.is_empty() {
                return Err(Diagnostic::new(
                    "E0407",
                    "`UdpSocket.close` does not accept arguments",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            }
            Ok((
                ValueType::Void,
                ValueExpr::UdpSocketClose {
                    socket: Box::new(receiver_expr),
                },
            ))
        }
        "recv_from_string" => {
            let [max_bytes_arg] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`UdpSocket.recv_from_string` expects a max byte count",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (max_bytes_type, max_bytes) = lower_value_expr_with_expected(
                path,
                max_bytes_arg,
                scope,
                imports,
                signatures,
                structs,
                enums,
                Some(&ValueType::Int),
                span,
            )?;
            if max_bytes_type != ValueType::Int {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    "`UdpSocket.recv_from_string` expects an i64 max byte count",
                    &ValueType::Int,
                    &max_bytes_type,
                ));
            }
            Ok((
                ValueType::Enum(
                    "Result".to_string(),
                    vec![
                        ValueType::Struct("UdpDatagram".to_string(), Vec::new()),
                        net_error,
                    ],
                ),
                ValueExpr::UdpSocketRecvFromString {
                    socket: Box::new(receiver_expr),
                    max_bytes: Box::new(max_bytes),
                },
            ))
        }
        "send_to_string" => {
            let [content_arg, host_arg, port_arg] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`UdpSocket.send_to_string` expects content, host, and port arguments",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (content_type, content) = lower_value_expr_with_expected(
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
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    "`UdpSocket.send_to_string` expects string content",
                    &ValueType::String,
                    &content_type,
                ));
            }
            let (host_type, host) = lower_value_expr_with_expected(
                path,
                host_arg,
                scope,
                imports,
                signatures,
                structs,
                enums,
                Some(&ValueType::String),
                span,
            )?;
            if host_type != ValueType::String {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    "`UdpSocket.send_to_string` expects a string host",
                    &ValueType::String,
                    &host_type,
                ));
            }
            let (port_type, port) = lower_value_expr_with_expected(
                path,
                port_arg,
                scope,
                imports,
                signatures,
                structs,
                enums,
                Some(&ValueType::Int),
                span,
            )?;
            if port_type != ValueType::Int {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    "`UdpSocket.send_to_string` expects an i64 port",
                    &ValueType::Int,
                    &port_type,
                ));
            }
            Ok((
                ValueType::Enum("Result".to_string(), vec![ValueType::Void, net_error]),
                ValueExpr::UdpSocketSendToString {
                    socket: Box::new(receiver_expr),
                    content: Box::new(content),
                    host: Box::new(host),
                    port: Box::new(port),
                },
            ))
        }
        _ => Err(Diagnostic::new(
            "E0305",
            format!("unknown UdpSocket method `{method}`"),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        )),
    }
}

pub(super) fn is_option_builtin_call(callee: &[String]) -> bool {
    matches!(
        callee,
        [module, name]
            if module == "option"
                && matches!(
                    name.as_str(),
                    "is_some" | "is_none" | "unwrap_or" | "map" | "and_then"
                )
    )
}

pub(super) fn is_result_builtin_call(callee: &[String]) -> bool {
    matches!(
        callee,
        [module, name]
            if module == "result"
                && matches!(
                    name.as_str(),
                    "is_ok" | "is_err" | "unwrap_or" | "map" | "map_err" | "and_then"
                )
    )
}

pub(super) fn lower_option_builtin(
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
    let [module, method] = callee else {
        unreachable!("option builtin dispatcher only passes qualified calls");
    };
    debug_assert_eq!(module, "option");
    match method.as_str() {
        "is_some" | "is_none" => {
            let [option] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    format!("`option.{method}` expects exactly one Option argument"),
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (option_type, lowered_option) = lower_value_expr(
                path, option, scope, imports, signatures, structs, enums, span,
            )?;
            let payload_type = option_payload(&option_type).ok_or_else(|| {
                type_mismatch(
                    path,
                    span,
                    format!("`option.{method}` expects an Option value"),
                )
            })?;
            let value = if method == "is_some" {
                ValueExpr::OptionIsSome {
                    option: Box::new(lowered_option),
                    payload_type,
                }
            } else {
                ValueExpr::OptionIsNone {
                    option: Box::new(lowered_option),
                    payload_type,
                }
            };
            Ok((ValueType::Bool, value))
        }
        "unwrap_or" => {
            let [option, default] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`option.unwrap_or` expects an Option value and a default value",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (option_type, lowered_option) = lower_value_expr(
                path, option, scope, imports, signatures, structs, enums, span,
            )?;
            let payload_type = option_payload(&option_type).ok_or_else(|| {
                type_mismatch(path, span, "`option.unwrap_or` expects an Option value")
            })?;
            if payload_type == ValueType::Void {
                return Err(type_mismatch(
                    path,
                    span,
                    "`option.unwrap_or` does not support Option<void>",
                ));
            }
            let (default_type, lowered_default) = lower_value_expr_with_expected(
                path,
                default,
                scope,
                imports,
                signatures,
                structs,
                enums,
                Some(&payload_type),
                span,
            )?;
            if default_type != payload_type {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    format!(
                        "`option.unwrap_or` default is `{}` but payload is `{}`",
                        default_type.name(),
                        payload_type.name()
                    ),
                    &payload_type,
                    &default_type,
                ));
            }
            Ok((
                payload_type.clone(),
                ValueExpr::OptionUnwrapOr {
                    option: Box::new(lowered_option),
                    default: Box::new(lowered_default),
                    payload_type,
                },
            ))
        }
        "map" | "and_then" => {
            let [option, converter] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    format!("`option.{method}` expects an Option value and a converter function"),
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (option_type, lowered_option) = lower_value_expr(
                path, option, scope, imports, signatures, structs, enums, span,
            )?;
            let source_type = option_payload(&option_type).ok_or_else(|| {
                type_mismatch(
                    path,
                    span,
                    format!("`option.{method}` expects an Option value"),
                )
            })?;
            lower_option_converter_call(
                path,
                span,
                method,
                lowered_option,
                source_type,
                converter,
                signatures,
            )
        }
        _ => unreachable!("option builtin dispatcher only passes known calls"),
    }
}

pub(super) fn lower_option_value_method(
    path: &Path,
    callee: &[String],
    args: &[AstExpr],
    scope: &HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    span: &Span,
) -> Result<Option<(ValueType, ValueExpr)>, Diagnostic> {
    if callee.len() != 2 {
        return Ok(None);
    }
    let receiver_name = &callee[0];
    let method = &callee[1];
    if !matches!(
        method.as_str(),
        "is_some" | "is_none" | "unwrap_or" | "map" | "and_then"
    ) {
        return Ok(None);
    }
    let Some(binding) = scope.get(receiver_name) else {
        return Ok(None);
    };
    let Some(payload_type) = option_payload(&binding.value_type) else {
        if matches!(method.as_str(), "unwrap_or" | "map" | "and_then")
            && result_parts(&binding.value_type).is_some()
        {
            return Ok(None);
        }
        return Err(type_mismatch(
            path,
            span,
            format!("`{receiver_name}.{method}` expects an Option value"),
        ));
    };
    require_option_method_import(path, imports, span, method)?;
    let option = binding_value_expr(receiver_name, binding);
    match method.as_str() {
        "is_some" => Ok(Some((
            ValueType::Bool,
            ValueExpr::OptionIsSome {
                option: Box::new(option),
                payload_type,
            },
        ))),
        "is_none" => Ok(Some((
            ValueType::Bool,
            ValueExpr::OptionIsNone {
                option: Box::new(option),
                payload_type,
            },
        ))),
        "unwrap_or" => {
            let [default] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`Option.unwrap_or` expects exactly one default value",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            if payload_type == ValueType::Void {
                return Err(type_mismatch(
                    path,
                    span,
                    "`Option.unwrap_or` does not support Option<void>",
                ));
            }
            let (default_type, lowered_default) = lower_value_expr_with_expected(
                path,
                default,
                scope,
                imports,
                signatures,
                structs,
                enums,
                Some(&payload_type),
                span,
            )?;
            if default_type != payload_type {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    format!(
                        "`Option.unwrap_or` default is `{}` but payload is `{}`",
                        default_type.name(),
                        payload_type.name()
                    ),
                    &payload_type,
                    &default_type,
                ));
            }
            Ok(Some((
                payload_type.clone(),
                ValueExpr::OptionUnwrapOr {
                    option: Box::new(option),
                    default: Box::new(lowered_default),
                    payload_type,
                },
            )))
        }
        "map" | "and_then" => {
            let [converter] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    format!("`Option.{method}` expects exactly one converter function"),
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            lower_option_converter_call(
                path,
                span,
                method,
                option,
                payload_type,
                converter,
                signatures,
            )
            .map(Some)
        }
        _ => unreachable!("option method dispatcher only passes known calls"),
    }
}

pub(super) fn lower_option_converter_call(
    path: &Path,
    span: &Span,
    method: &str,
    option: ValueExpr,
    source_type: ValueType,
    converter: &AstExpr,
    signatures: &HashMap<String, FunctionSignature>,
) -> Result<(ValueType, ValueExpr), Diagnostic> {
    let converter_name = option_converter_name(path, span, method, converter)?;
    let converter_signature =
        option_converter_signature(path, span, method, &converter_name, signatures)?;
    let [converter_param] = converter_signature.params.as_slice() else {
        return Err(Diagnostic::new(
            "E0407",
            format!("converter function `{converter_name}` must take exactly one argument"),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    };
    if converter_param.value_type != source_type {
        return Err(type_mismatch_expected_found(
            path,
            span,
            format!(
                "`Option.{method}` converter `{converter_name}` takes `{}` but payload is `{}`",
                converter_param.value_type.name(),
                source_type.name()
            ),
            &source_type,
            &converter_param.value_type,
        ));
    }
    match method {
        "map" => {
            if converter_signature.return_type == ValueType::Void {
                return Err(type_mismatch(
                    path,
                    span,
                    format!("converter function `{converter_name}` must return a mapped value"),
                ));
            }
            let target_type = converter_signature.return_type.clone();
            Ok((
                ValueType::Enum("Option".to_string(), vec![target_type.clone()]),
                ValueExpr::OptionMap {
                    option: Box::new(option),
                    source_type,
                    target_type,
                    converter: converter_name,
                },
            ))
        }
        "and_then" => {
            let Some(target_type) = option_payload(&converter_signature.return_type) else {
                return Err(type_mismatch(
                    path,
                    span,
                    format!("converter function `{converter_name}` must return an Option value"),
                ));
            };
            Ok((
                ValueType::Enum("Option".to_string(), vec![target_type.clone()]),
                ValueExpr::OptionAndThen {
                    option: Box::new(option),
                    source_type,
                    target_type,
                    converter: converter_name,
                },
            ))
        }
        _ => unreachable!("option converter helper only supports map/and_then"),
    }
}

pub(super) fn option_converter_name(
    path: &Path,
    span: &Span,
    method: &str,
    converter: &AstExpr,
) -> Result<String, Diagnostic> {
    let AstExpr::Name(converter_path) = converter else {
        return Err(Diagnostic::new(
            "E0407",
            format!("`Option.{method}` expects a converter function name"),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    };
    let [converter_name] = converter_path.as_slice() else {
        return Err(Diagnostic::new(
            "E0407",
            format!("`Option.{method}` expects an unqualified converter function name"),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    };
    Ok(converter_name.clone())
}

pub(super) fn option_converter_signature<'a>(
    path: &Path,
    span: &Span,
    method: &str,
    converter_name: &str,
    signatures: &'a HashMap<String, FunctionSignature>,
) -> Result<&'a FunctionSignature, Diagnostic> {
    let Some(converter_signature) = signatures.get(converter_name) else {
        return Err(Diagnostic::new(
            "E0305",
            format!("unknown converter function `{converter_name}`"),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    };
    if !converter_signature.type_params.is_empty() {
        return Err(Diagnostic::new(
            "E0407",
            format!("`Option.{method}` converter `{converter_name}` must not be generic"),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    }
    Ok(converter_signature)
}

pub(super) fn lower_result_builtin(
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
    let [module, method] = callee else {
        unreachable!("result builtin dispatcher only passes qualified calls");
    };
    debug_assert_eq!(module, "result");
    match method.as_str() {
        "is_ok" | "is_err" => {
            let [result] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    format!("`result.{method}` expects exactly one Result argument"),
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (result_type, lowered_result) = lower_value_expr(
                path, result, scope, imports, signatures, structs, enums, span,
            )?;
            lower_result_predicate(path, span, method, lowered_result, &result_type)
        }
        "unwrap_or" => {
            let [result, default] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`result.unwrap_or` expects a Result value and a default value",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (result_type, lowered_result) = lower_value_expr(
                path, result, scope, imports, signatures, structs, enums, span,
            )?;
            lower_result_unwrap_or(
                path,
                span,
                "result.unwrap_or",
                lowered_result,
                &result_type,
                default,
                scope,
                imports,
                signatures,
                structs,
                enums,
            )
        }
        "map" | "map_err" | "and_then" => {
            let [result, converter] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    format!("`result.{method}` expects a Result value and a converter function"),
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (result_type, lowered_result) = lower_value_expr(
                path, result, scope, imports, signatures, structs, enums, span,
            )?;
            let (ok_type, err_type) = result_parts(&result_type).ok_or_else(|| {
                type_mismatch(
                    path,
                    span,
                    format!("`result.{method}` expects a Result value"),
                )
            })?;
            lower_result_converter_call(
                path,
                span,
                method,
                lowered_result,
                ok_type,
                err_type,
                converter,
                signatures,
            )
        }
        _ => unreachable!("result builtin dispatcher only passes known calls"),
    }
}

pub(super) fn lower_result_value_method(
    path: &Path,
    callee: &[String],
    args: &[AstExpr],
    scope: &HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    span: &Span,
) -> Result<Option<(ValueType, ValueExpr)>, Diagnostic> {
    if callee.len() != 2 {
        return Ok(None);
    }
    let receiver_name = &callee[0];
    let method = &callee[1];
    if !matches!(
        method.as_str(),
        "is_ok" | "is_err" | "unwrap_or" | "map" | "map_err" | "and_then"
    ) {
        return Ok(None);
    }
    let Some(binding) = scope.get(receiver_name) else {
        return Ok(None);
    };
    require_result_method_import(path, imports, span, method)?;
    let result = binding_value_expr(receiver_name, binding);
    let (ok_type, err_type) = result_parts(&binding.value_type).ok_or_else(|| {
        type_mismatch(
            path,
            span,
            format!("`{receiver_name}.{method}` expects a Result value"),
        )
    })?;
    match method.as_str() {
        "is_ok" | "is_err" => {
            if !args.is_empty() {
                return Err(Diagnostic::new(
                    "E0407",
                    format!("`Result.{method}` expects no arguments"),
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            }
            lower_result_predicate(path, span, method, result, &binding.value_type).map(Some)
        }
        "unwrap_or" => {
            let [default] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`Result.unwrap_or` expects exactly one default value",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            lower_result_unwrap_or(
                path,
                span,
                "Result.unwrap_or",
                result,
                &binding.value_type,
                default,
                scope,
                imports,
                signatures,
                structs,
                enums,
            )
            .map(Some)
        }
        "map" | "map_err" | "and_then" => {
            let [converter] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    format!("`Result.{method}` expects exactly one converter function"),
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            lower_result_converter_call(
                path, span, method, result, ok_type, err_type, converter, signatures,
            )
            .map(Some)
        }
        _ => unreachable!("result method dispatcher only passes known calls"),
    }
}

pub(super) fn lower_result_predicate(
    path: &Path,
    span: &Span,
    method: &str,
    result: ValueExpr,
    result_type: &ValueType,
) -> Result<(ValueType, ValueExpr), Diagnostic> {
    let (ok_type, err_type) = result_parts(result_type).ok_or_else(|| {
        type_mismatch(
            path,
            span,
            format!("`Result.{method}` expects a Result value"),
        )
    })?;
    let value = if method == "is_ok" {
        ValueExpr::ResultIsOk {
            result: Box::new(result),
            ok_type,
            err_type,
        }
    } else {
        ValueExpr::ResultIsErr {
            result: Box::new(result),
            ok_type,
            err_type,
        }
    };
    Ok((ValueType::Bool, value))
}

#[allow(clippy::too_many_arguments)]
pub(super) fn lower_result_unwrap_or(
    path: &Path,
    span: &Span,
    label: &str,
    result: ValueExpr,
    result_type: &ValueType,
    default: &AstExpr,
    scope: &HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
) -> Result<(ValueType, ValueExpr), Diagnostic> {
    let (ok_type, err_type) = result_parts(result_type)
        .ok_or_else(|| type_mismatch(path, span, format!("`{label}` expects a Result value")))?;
    if ok_type == ValueType::Void {
        return Err(type_mismatch(
            path,
            span,
            format!("`{label}` does not support Result<void, E>"),
        ));
    }
    let (default_type, lowered_default) = lower_value_expr_with_expected(
        path,
        default,
        scope,
        imports,
        signatures,
        structs,
        enums,
        Some(&ok_type),
        span,
    )?;
    if default_type != ok_type {
        return Err(type_mismatch_expected_found(
            path,
            span,
            format!(
                "`{label}` default is `{}` but ok type is `{}`",
                default_type.name(),
                ok_type.name()
            ),
            &ok_type,
            &default_type,
        ));
    }
    Ok((
        ok_type.clone(),
        ValueExpr::ResultUnwrapOr {
            result: Box::new(result),
            default: Box::new(lowered_default),
            ok_type,
            err_type,
        },
    ))
}

pub(super) fn lower_result_converter_call(
    path: &Path,
    span: &Span,
    method: &str,
    result: ValueExpr,
    ok_type: ValueType,
    err_type: ValueType,
    converter: &AstExpr,
    signatures: &HashMap<String, FunctionSignature>,
) -> Result<(ValueType, ValueExpr), Diagnostic> {
    let converter_name = result_converter_name(path, span, method, converter)?;
    let converter_signature =
        result_converter_signature(path, span, method, &converter_name, signatures)?;
    let [converter_param] = converter_signature.params.as_slice() else {
        return Err(Diagnostic::new(
            "E0407",
            format!("converter function `{converter_name}` must take exactly one argument"),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    };
    match method {
        "map" => {
            if ok_type == ValueType::Void {
                return Err(type_mismatch(
                    path,
                    span,
                    "`Result.map` does not support Result<void, E>",
                ));
            }
            if converter_param.value_type != ok_type {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    format!(
                        "`Result.map` converter `{converter_name}` takes `{}` but ok type is `{}`",
                        converter_param.value_type.name(),
                        ok_type.name()
                    ),
                    &ok_type,
                    &converter_param.value_type,
                ));
            }
            if converter_signature.return_type == ValueType::Void {
                return Err(type_mismatch(
                    path,
                    span,
                    format!("converter function `{converter_name}` must return a mapped value"),
                ));
            }
            let target_ok_type = converter_signature.return_type.clone();
            Ok((
                ValueType::Enum(
                    "Result".to_string(),
                    vec![target_ok_type.clone(), err_type.clone()],
                ),
                ValueExpr::ResultMap {
                    result: Box::new(result),
                    source_ok_type: ok_type,
                    target_ok_type,
                    err_type,
                    converter: converter_name,
                },
            ))
        }
        "map_err" => {
            if converter_param.value_type != err_type {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    format!(
                        "`Result.map_err` converter `{converter_name}` takes `{}` but error type is `{}`",
                        converter_param.value_type.name(),
                        err_type.name()
                    ),
                    &err_type,
                    &converter_param.value_type,
                ));
            }
            if converter_signature.return_type == ValueType::Void {
                return Err(type_mismatch(
                    path,
                    span,
                    format!("converter function `{converter_name}` must return an error value"),
                ));
            }
            let target_err_type = converter_signature.return_type.clone();
            Ok((
                ValueType::Enum(
                    "Result".to_string(),
                    vec![ok_type.clone(), target_err_type.clone()],
                ),
                ValueExpr::ResultMapErr {
                    result: Box::new(result),
                    ok_type,
                    source_err_type: err_type,
                    target_err_type,
                    converter: converter_name,
                },
            ))
        }
        "and_then" => {
            if ok_type == ValueType::Void {
                return Err(type_mismatch(
                    path,
                    span,
                    "`Result.and_then` does not support Result<void, E>",
                ));
            }
            if converter_param.value_type != ok_type {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    format!(
                        "`Result.and_then` converter `{converter_name}` takes `{}` but ok type is `{}`",
                        converter_param.value_type.name(),
                        ok_type.name()
                    ),
                    &ok_type,
                    &converter_param.value_type,
                ));
            }
            let Some((target_ok_type, target_err_type)) =
                result_parts(&converter_signature.return_type)
            else {
                return Err(type_mismatch(
                    path,
                    span,
                    format!("converter function `{converter_name}` must return a Result value"),
                ));
            };
            if target_err_type != err_type {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    format!(
                        "`Result.and_then` converter `{converter_name}` returns error `{}` but source error is `{}`",
                        target_err_type.name(),
                        err_type.name()
                    ),
                    &err_type,
                    &target_err_type,
                ));
            }
            Ok((
                ValueType::Enum(
                    "Result".to_string(),
                    vec![target_ok_type.clone(), err_type.clone()],
                ),
                ValueExpr::ResultAndThen {
                    result: Box::new(result),
                    source_ok_type: ok_type,
                    target_ok_type,
                    err_type,
                    converter: converter_name,
                },
            ))
        }
        _ => unreachable!("result converter helper only supports map/map_err/and_then"),
    }
}

pub(super) fn result_converter_name(
    path: &Path,
    span: &Span,
    method: &str,
    converter: &AstExpr,
) -> Result<String, Diagnostic> {
    let AstExpr::Name(converter_path) = converter else {
        return Err(Diagnostic::new(
            "E0407",
            format!("`Result.{method}` expects a converter function name"),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    };
    let [converter_name] = converter_path.as_slice() else {
        return Err(Diagnostic::new(
            "E0407",
            format!("`Result.{method}` expects an unqualified converter function name"),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    };
    Ok(converter_name.clone())
}

pub(super) fn result_converter_signature<'a>(
    path: &Path,
    span: &Span,
    method: &str,
    converter_name: &str,
    signatures: &'a HashMap<String, FunctionSignature>,
) -> Result<&'a FunctionSignature, Diagnostic> {
    let Some(converter_signature) = signatures.get(converter_name) else {
        return Err(Diagnostic::new(
            "E0305",
            format!("unknown converter function `{converter_name}`"),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    };
    if !converter_signature.type_params.is_empty() {
        return Err(Diagnostic::new(
            "E0407",
            format!("`Result.{method}` converter `{converter_name}` must not be generic"),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    }
    Ok(converter_signature)
}
