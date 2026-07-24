use super::*;

pub(super) fn is_http_builtin_call(callee: &[String]) -> bool {
    matches!(
        callee,
        [module, name]
            if module == "http"
                && matches!(
                    name.as_str(),
                    "get"
                        | "post"
                        | "send"
                        | "open_stream"
                        | "read_text"
                        | "next_sse"
                        | "cancel_stream"
                        | "close_stream"
                        | "listen"
                        | "accept"
                        | "respond_string"
                        | "close_server"
                        | "close_exchange"
                )
    )
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
    let http_stream = ValueType::Struct("HttpStream".to_string(), Vec::new());
    let http_stream_chunk = ValueType::Struct("HttpStreamChunk".to_string(), Vec::new());
    let sse_event = ValueType::Struct("SseEvent".to_string(), Vec::new());
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
        "send" => {
            let [request_arg] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`http.send` expects exactly one HttpRequest",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (request_type, request) = lower_value_expr(
                path,
                request_arg,
                scope,
                imports,
                signatures,
                structs,
                enums,
                span,
            )?;
            let expected = ValueType::Struct("HttpRequest".to_string(), Vec::new());
            if request_type != expected {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    "`http.send` expects an HttpRequest value",
                    &expected,
                    &request_type,
                ));
            }
            Ok((
                response_result_type,
                ValueExpr::Call {
                    name: BUILTIN_HTTP_SEND_EXPR.to_string(),
                    args: vec![request],
                },
            ))
        }
        "open_stream" => {
            let [request_arg, idle_timeout_arg] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`http.open_stream` expects an HttpRequest and idle timeout",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (request_type, request) = lower_value_expr(
                path,
                request_arg,
                scope,
                imports,
                signatures,
                structs,
                enums,
                span,
            )?;
            let expected_request = ValueType::Struct("HttpRequest".to_string(), Vec::new());
            if request_type != expected_request {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    "`http.open_stream` expects an HttpRequest value",
                    &expected_request,
                    &request_type,
                ));
            }
            let (idle_timeout_type, idle_timeout) = lower_value_expr_with_expected(
                path,
                idle_timeout_arg,
                scope,
                imports,
                signatures,
                structs,
                enums,
                Some(&ValueType::U64),
                span,
            )?;
            if idle_timeout_type != ValueType::U64 {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    "`http.open_stream` expects a u64 idle timeout",
                    &ValueType::U64,
                    &idle_timeout_type,
                ));
            }
            Ok((
                ValueType::Enum(
                    "Result".to_string(),
                    vec![http_stream.clone(), http_error.clone()],
                ),
                ValueExpr::Call {
                    name: BUILTIN_HTTP_OPEN_STREAM_EXPR.to_string(),
                    args: vec![request, idle_timeout],
                },
            ))
        }
        "read_text" => {
            let [stream_arg, max_chunk_arg] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`http.read_text` expects an HttpStream and chunk limit",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (stream_type, stream) = lower_value_expr(
                path, stream_arg, scope, imports, signatures, structs, enums, span,
            )?;
            if stream_type != http_stream {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    "`http.read_text` expects an HttpStream value",
                    &http_stream,
                    &stream_type,
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
                    "`http.read_text` expects a u64 chunk limit",
                    &ValueType::U64,
                    &max_chunk_type,
                ));
            }
            Ok((
                ValueType::Enum(
                    "Result".to_string(),
                    vec![http_stream_chunk, http_error.clone()],
                ),
                ValueExpr::Call {
                    name: BUILTIN_HTTP_READ_TEXT_EXPR.to_string(),
                    args: vec![stream, max_chunk],
                },
            ))
        }
        "next_sse" => {
            let [stream_arg, max_event_arg] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`http.next_sse` expects an HttpStream and event limit",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (stream_type, stream) = lower_value_expr(
                path, stream_arg, scope, imports, signatures, structs, enums, span,
            )?;
            if stream_type != http_stream {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    "`http.next_sse` expects an HttpStream value",
                    &http_stream,
                    &stream_type,
                ));
            }
            let (max_event_type, max_event) = lower_value_expr_with_expected(
                path,
                max_event_arg,
                scope,
                imports,
                signatures,
                structs,
                enums,
                Some(&ValueType::U64),
                span,
            )?;
            if max_event_type != ValueType::U64 {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    "`http.next_sse` expects a u64 event limit",
                    &ValueType::U64,
                    &max_event_type,
                ));
            }
            let event_option = ValueType::Enum("Option".to_string(), vec![sse_event]);
            Ok((
                ValueType::Enum("Result".to_string(), vec![event_option, http_error.clone()]),
                ValueExpr::Call {
                    name: BUILTIN_HTTP_NEXT_SSE_EXPR.to_string(),
                    args: vec![stream, max_event],
                },
            ))
        }
        "cancel_stream" | "close_stream" => {
            let [stream_arg] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    format!("`http.{name}` expects exactly one HttpStream"),
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (stream_type, stream) = lower_value_expr(
                path, stream_arg, scope, imports, signatures, structs, enums, span,
            )?;
            if stream_type != http_stream {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    format!("`http.{name}` expects an HttpStream value"),
                    &http_stream,
                    &stream_type,
                ));
            }
            let intrinsic = if name == "cancel_stream" {
                BUILTIN_HTTP_CANCEL_STREAM_EXPR
            } else {
                BUILTIN_HTTP_CLOSE_STREAM_EXPR
            };
            Ok((
                ValueType::Void,
                ValueExpr::Call {
                    name: intrinsic.to_string(),
                    args: vec![stream],
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
