use super::*;

#[test]
fn accepts_json_builtins() {
    let source = r#"package app.main

import std.json

fn main() -> Result<void, JsonError> {
    let parsed: Result<JsonValue, JsonError> = json.parse("{\"lang\":\"nomo\"}")
    let value: JsonValue = parsed?
    let text: string = json.stringify(value)
    return Ok(void)
}
"#;

    let program = parse_inline(source).unwrap();
    assert!(program.structs.iter().any(|item| item.name == "JsonValue"));
    assert!(program.structs.iter().any(|item| item.name == "JsonError"));
    let main = program.functions.iter().find(|f| f.name == "main").unwrap();
    assert!(matches!(
        main.body[0],
        Statement::Let {
            initializer: ValueExpr::JsonParse { .. },
            ..
        }
    ));
    assert!(matches!(
        main.body[2],
        Statement::Let {
            value_type: ValueType::String,
            initializer: ValueExpr::JsonStringify { .. },
            ..
        }
    ));
}

#[test]
fn accepts_specific_json_builtin_imports() {
    let source = r#"package app.main

import std.json.JsonError
import std.json.JsonValue
import std.json.parse
import std.json.stringify

fn main() -> Result<void, JsonError> {
    let parsed: Result<JsonValue, JsonError> = parse("true")
    let value: JsonValue = parsed?
    let text: string = stringify(value)
    return Ok(void)
}
"#;

    let program = parse_inline(source).unwrap();
    let main = program.functions.iter().find(|f| f.name == "main").unwrap();
    assert!(matches!(
        main.body[0],
        Statement::Let {
            initializer: ValueExpr::JsonParse { .. },
            ..
        }
    ));
    assert!(matches!(
        main.body[2],
        Statement::Let {
            initializer: ValueExpr::JsonStringify { .. },
            ..
        }
    ));
}

#[test]
fn accepts_structured_json_accessors_and_constructors() {
    let source = r#"package app.main

import std.array.Array
import std.json

fn main() -> Result<void, JsonError> {
    let root: JsonValue = json.parse("{\"items\":[1],\"ok\":true}")?
    let root_kind: JsonKind = json.kind(root)
    let ok: Option<JsonValue> = json.get(root, "ok")
    let truth: Option<bool> = json.as_bool(json.from_bool(true))
    let number: JsonValue = json.from_number_text("1e+2")?
    let exact: Option<string> = json.number_text(number)
    let text: JsonValue = json.from_string("nomo")?
    let decoded: Option<string> = json.as_string(text)
    let mut values: Array<JsonValue> = Array.new<JsonValue>()
    values.push(json.from_i64(-1))
    values.push(json.from_u64(2 as u64))
    values.push(json.from_null())
    let array: JsonValue = json.from_array(values)?
    let items: Option<Array<JsonValue>> = json.array_items(array)
    let mut members: Array<JsonMember> = Array.new<JsonMember>()
    members.push(JsonMember { key: "value", value: array })
    let object: JsonValue = json.from_object(members)?
    let all: Option<Array<JsonMember>> = json.object_members(object)
    return Ok(void)
}
"#;

    let program = parse_inline(source).unwrap();
    assert!(program.enums.iter().any(|item| item.name == "JsonKind"));
    assert!(program.structs.iter().any(|item| item.name == "JsonMember"));
    let main = program.functions.iter().find(|f| f.name == "main").unwrap();
    let debug = format!("{:?}", main.body);
    for operation in [
        "Kind",
        "Get",
        "AsBool",
        "FromBool",
        "FromNumberText",
        "NumberText",
        "FromString",
        "AsString",
        "FromI64",
        "FromU64",
        "FromNull",
        "FromArray",
        "ArrayItems",
        "FromObject",
        "ObjectMembers",
    ] {
        assert!(debug.contains(operation), "missing {operation} in {debug}");
    }
    let c = nomo_codegen_c::emit_c(&program);
    assert!(c.contains("#define NOMO_JSON_MAX_BYTES (8U * 1024U * 1024U)"));
    assert!(c.contains("#define NOMO_JSON_MAX_DEPTH 128U"));
    assert!(c.contains("#define NOMO_JSON_MAX_VALUES 262144U"));
    assert!(c.contains("nomo_json_object_members"));
    assert!(c.contains("nomo_json_from_object"));
    assert!(c.contains("return value.nomo_member_raw;"));
    assert!(c.contains("nomo_array_struct_JsonValue"));
    assert!(c.contains("nomo_array_struct_JsonMember"));
    assert!(
        c.contains("nomo_enum_JsonKind_release(nomo_enum_JsonKind value) {\n    (void)value;\n}")
    );
    assert!(!c.contains("@JSON_"));
}

#[test]
fn accepts_specific_structured_json_imports() {
    let source = r#"package app.main

import std.json.JsonValue
import std.json.from_null
import std.json.is_null

fn main() -> void {
    let value: JsonValue = from_null()
    let absent: bool = is_null(value)
}
"#;

    let program = parse_inline(source).unwrap();
    let main = program.functions.iter().find(|f| f.name == "main").unwrap();
    assert!(matches!(
        main.body[0],
        Statement::Let {
            initializer: ValueExpr::JsonStructured {
                operation: JsonOperation::FromNull,
                ..
            },
            ..
        }
    ));
    assert!(matches!(
        main.body[1],
        Statement::Let {
            initializer: ValueExpr::JsonStructured {
                operation: JsonOperation::IsNull,
                ..
            },
            ..
        }
    ));
}

#[test]
fn accepts_http_client_builtins() {
    let source = r#"package app.main

import std.http

fn main() -> Result<void, HttpError> {
    let first: HttpResponse = http.get_blocking("http://127.0.0.1/hello")?
    let second: HttpResponse = http.post_blocking("http://127.0.0.1/echo", "body")?
    return Ok(void)
}
"#;

    let program = parse_inline(source).unwrap();
    assert!(program.structs.iter().any(|item| item.name == "HttpError"));
    assert!(
        program
            .structs
            .iter()
            .any(|item| item.name == "HttpResponse")
    );
    let main = program.functions.iter().find(|f| f.name == "main").unwrap();
    assert!(matches!(
        main.body[0],
        Statement::QuestionLet {
            value_type: ValueType::Struct(ref value_name, ref args),
            result_expr: ValueExpr::Call { name: ref call_name, .. },
            ..
        } if value_name == "HttpResponse" && args.is_empty() && call_name == BUILTIN_HTTP_GET_BLOCKING_EXPR
    ));
    assert!(matches!(
        main.body[1],
        Statement::QuestionLet {
            value_type: ValueType::Struct(ref value_name, ref args),
            result_expr: ValueExpr::Call { name: ref call_name, .. },
            ..
        } if value_name == "HttpResponse" && args.is_empty() && call_name == BUILTIN_HTTP_POST_BLOCKING_EXPR
    ));
}

#[test]
fn accepts_structured_http_request_builtin() {
    let source = r#"package app.main

import std.array.Array
import std.http

fn main() -> Result<void, HttpError> {
    let mut headers: Array<HttpHeader> = Array.new<HttpHeader>()
    headers.push(HttpHeader { name: "Authorization", value: "Bearer test-token" })
    let request: HttpRequest = HttpRequest {
        method: "POST",
        url: "https://localhost/v1/chat/completions",
        headers: headers,
        body: "{\"stream\":false}",
        timeout_millis: 1000,
        max_response_bytes: 4096
    }
    let response: HttpResponse = http.send_blocking(request)?
    return Ok(void)
}
"#;

    let program = parse_inline(source).unwrap();
    for name in ["HttpError", "HttpHeader", "HttpRequest", "HttpResponse"] {
        assert!(program.structs.iter().any(|item| item.name == name));
    }
    let error = program
        .structs
        .iter()
        .find(|item| item.name == "HttpError")
        .unwrap();
    assert!(error.fields.iter().any(|field| field.name == "code"));
    let response = program
        .structs
        .iter()
        .find(|item| item.name == "HttpResponse")
        .unwrap();
    assert!(response.fields.iter().any(|field| field.name == "headers"));
    let main = program.functions.iter().find(|f| f.name == "main").unwrap();
    assert!(main.body.iter().any(|statement| matches!(
        statement,
        Statement::QuestionLet {
            result_expr: ValueExpr::Call { name, .. },
            ..
        } if name == BUILTIN_HTTP_SEND_BLOCKING_EXPR
    )));
    let c = nomo_codegen_c::emit_c(&program);
    assert!(
        c.contains("nomo_enum_Option_u64"),
        "unused standard structs must still collect enum-typed field dependencies"
    );
}

#[test]
fn accepts_http_streaming_and_sse_builtins() {
    let source = r#"package app.main

import std.array.Array
import std.http

fn stream(request: HttpRequest) -> Result<void, HttpError> {
    let response: BlockingHttpStream = http.open_stream_blocking(request, 1000)?
    defer http.close_stream_blocking(response)
    let chunk: HttpStreamChunk = http.read_text_blocking(response, 4096)?
    let event: Option<SseEvent> = http.next_sse_blocking(response, 65536)?
    http.cancel_stream_blocking(response)
    return Ok(void)
}

fn main() -> void {
}
"#;

    let program = parse_inline(source).unwrap();
    for name in [
        "HttpError",
        "HttpHeader",
        "HttpRequest",
        "BlockingHttpStream",
        "HttpStreamChunk",
        "SseEvent",
    ] {
        assert!(program.structs.iter().any(|item| item.name == name));
    }
    let stream = program
        .functions
        .iter()
        .find(|function| function.name == "stream")
        .unwrap();
    for intrinsic in [
        BUILTIN_HTTP_OPEN_STREAM_BLOCKING_EXPR,
        BUILTIN_HTTP_READ_TEXT_BLOCKING_EXPR,
        BUILTIN_HTTP_NEXT_SSE_BLOCKING_EXPR,
        BUILTIN_HTTP_CANCEL_STREAM_BLOCKING_EXPR,
        BUILTIN_HTTP_CLOSE_STREAM_BLOCKING_EXPR,
    ] {
        assert!(stream.body.iter().any(|statement| match statement {
            Statement::QuestionLet {
                result_expr: ValueExpr::Call { name, .. },
                ..
            }
            | Statement::Expr(ValueExpr::Call { name, .. }) => name == intrinsic,
            Statement::Defer {
                call: DeferredCall::Expr(ValueExpr::Call { name, .. }),
            } => name == intrinsic,
            _ => false,
        }));
    }
    let c = nomo_codegen_c::emit_c(&program);
    for symbol in [
        "__nomo_http_open_stream_blocking",
        "__nomo_http_read_text_blocking",
        "__nomo_http_next_sse_blocking",
        "__nomo_http_cancel_stream_blocking",
        "__nomo_http_close_stream_blocking",
        "nomo_enum_Option_u64",
        "curl_multi_perform",
    ] {
        assert!(c.contains(symbol), "missing generated C symbol `{symbol}`");
    }
    assert!(!c.contains("@HTTP_"));
    assert!(!c.contains("@OPEN_"));
    assert!(!c.contains("@READ_"));
    assert!(!c.contains("@SSE_"));
}

#[test]
fn accepts_specific_http_builtin_imports() {
    let source = r#"package app.main

import std.http.HttpError
import std.http.HttpResponse
import std.http.get_blocking
import std.http.post_blocking

fn main() -> Result<void, HttpError> {
    let first: HttpResponse = get_blocking("http://127.0.0.1/hello")?
    let second: HttpResponse = post_blocking("http://127.0.0.1/echo", "body")?
    return Ok(void)
}
"#;

    let program = parse_inline(source).unwrap();
    let main = program.functions.iter().find(|f| f.name == "main").unwrap();
    assert!(matches!(
        main.body[0],
        Statement::QuestionLet {
            result_expr: ValueExpr::Call { ref name, .. },
            ..
        } if name == BUILTIN_HTTP_GET_BLOCKING_EXPR
    ));
    assert!(matches!(
        main.body[1],
        Statement::QuestionLet {
            result_expr: ValueExpr::Call { ref name, .. },
            ..
        } if name == BUILTIN_HTTP_POST_BLOCKING_EXPR
    ));
}

#[test]
fn accepts_http_server_builtins() {
    let source = r#"package app.main

import std.http

fn serve(host: string, port: i64) -> Result<void, HttpError> {
    let server: HttpServer = http.listen(host, port)?
    defer http.close_server(server)
    let exchange: HttpExchange = http.accept(server)?
    defer http.close_exchange(exchange)
    let method: string = exchange.method
    let path: string = exchange.path
    let body: string = exchange.body
    http.respond_string(exchange, 200, body)?
    return Ok(void)
}

fn main() -> void {
}
"#;

    let program = parse_inline(source).unwrap();
    assert!(program.structs.iter().any(|item| item.name == "HttpServer"));
    assert!(
        program
            .structs
            .iter()
            .any(|item| item.name == "HttpExchange")
    );
    assert!(
        program.enums.iter().any(|item| item.name == "Option"),
        "server-only HTTP programs still need Option<HttpHeader> for injected array helpers"
    );
    let serve = program
        .functions
        .iter()
        .find(|f| f.name == "serve")
        .unwrap();
    assert!(matches!(
        serve.body[0],
        Statement::QuestionLet {
            value_type: ValueType::Struct(ref value_name, ref args),
            result_expr: ValueExpr::Call { name: ref call_name, .. },
            ..
        } if value_name == "HttpServer" && args.is_empty() && call_name == BUILTIN_HTTP_LISTEN_EXPR
    ));
    assert!(serve.body.iter().any(|stmt| matches!(
        stmt,
        Statement::QuestionLet {
            value_type: ValueType::Struct(value_name, args),
            result_expr: ValueExpr::Call { name: call_name, .. },
            ..
        } if value_name == "HttpExchange"
            && args.is_empty()
            && call_name == BUILTIN_HTTP_ACCEPT_EXPR
    )));
    assert!(serve.body.iter().any(|stmt| matches!(
        stmt,
        Statement::QuestionLet {
            value_type: ValueType::Void,
            result_expr: ValueExpr::Call { name: call_name, .. },
            ..
        } if call_name == BUILTIN_HTTP_RESPOND_STRING_EXPR
    )));
    assert!(serve.body.iter().any(|stmt| matches!(
        stmt,
        Statement::Defer {
            call: DeferredCall::Expr(ValueExpr::Call { name, .. })
        } if name == BUILTIN_HTTP_CLOSE_SERVER_EXPR
    )));
}

#[test]
fn accepts_specific_http_server_builtin_imports() {
    let source = r#"package app.main

import std.http.HttpError
import std.http.HttpExchange
import std.http.HttpServer
import std.http.accept
import std.http.close_exchange
import std.http.close_server
import std.http.listen
import std.http.respond_string

fn serve(host: string, port: i64) -> Result<void, HttpError> {
    let server: HttpServer = listen(host, port)?
    defer close_server(server)
    let exchange: HttpExchange = accept(server)?
    defer close_exchange(exchange)
    respond_string(exchange, 204, "")?
    return Ok(void)
}

fn main() -> void {
}
"#;

    let program = parse_inline(source).unwrap();
    let serve = program
        .functions
        .iter()
        .find(|f| f.name == "serve")
        .unwrap();
    assert!(matches!(
        serve.body[0],
        Statement::QuestionLet {
            result_expr: ValueExpr::Call { ref name, .. },
            ..
        } if name == BUILTIN_HTTP_LISTEN_EXPR
    ));
    assert!(serve.body.iter().any(|stmt| matches!(
        stmt,
        Statement::QuestionLet {
            result_expr: ValueExpr::Call { name, .. },
            ..
        } if name == BUILTIN_HTTP_ACCEPT_EXPR
    )));
    assert!(serve.body.iter().any(|stmt| matches!(
        stmt,
        Statement::QuestionLet {
            result_expr: ValueExpr::Call { name, .. },
            ..
        } if name == BUILTIN_HTTP_RESPOND_STRING_EXPR
    )));
    assert!(serve.body.iter().any(|stmt| matches!(
        stmt,
        Statement::Defer {
            call: DeferredCall::Expr(ValueExpr::Call { name, .. })
        } if name == BUILTIN_HTTP_CLOSE_EXCHANGE_EXPR
    )));
}

#[test]
fn accepts_regex_builtins_with_question() {
    let source = r#"package app.main

import std.regex
import std.array

fn main() -> Result<void, RegexError> {
    let compiled: Result<Regex, RegexError> = regex.compile("(nomo)-([0-9]+)")
    let rx: Regex = compiled?
    let matched: bool = regex.is_match(rx, "hello nomo-42")
    let groups: Option<Array<string>> = regex.captures(rx, "hello nomo-42")
    return Ok(void)
}
"#;

    let program = parse_inline(source).unwrap();
    assert!(program.structs.iter().any(|item| item.name == "Regex"));
    assert!(program.structs.iter().any(|item| item.name == "RegexError"));
    let main = program.functions.iter().find(|f| f.name == "main").unwrap();
    assert!(matches!(
        main.body[0],
        Statement::Let {
            initializer: ValueExpr::RegexCompile { .. },
            ..
        }
    ));
    assert!(main.body.iter().any(|stmt| matches!(
        stmt,
        Statement::QuestionLet {
            carrier: QuestionCarrier::Result,
            ..
        }
    )));
    assert!(main.body.iter().any(|stmt| {
        matches!(
            stmt,
            Statement::Let {
                value_type: ValueType::Bool,
                initializer: ValueExpr::RegexIsMatch { .. },
                ..
            }
        )
    }));
    assert!(main.body.iter().any(|stmt| {
        matches!(
            stmt,
            Statement::Let {
                value_type: ValueType::Enum(name, args),
                initializer: ValueExpr::RegexCaptures { .. },
                ..
            } if name == "Option" && args == &[ValueType::Array(Box::new(ValueType::String))]
        )
    }));
}

#[test]
fn accepts_specific_regex_builtin_imports() {
    let source = r#"package app.main

import std.regex.Regex
import std.regex.RegexError
import std.regex.captures
import std.regex.compile
import std.regex.is_match
import std.array.Array

fn main() -> Result<void, RegexError> {
    let rx: Regex = compile("nomo")?
    let matched: bool = is_match(rx, "nomo")
    let groups: Option<Array<string>> = captures(rx, "nomo")
    return Ok(void)
}
"#;

    let program = parse_inline(source).unwrap();
    let main = program.functions.iter().find(|f| f.name == "main").unwrap();
    assert!(main.body.iter().any(|stmt| matches!(
        stmt,
        Statement::QuestionLet {
            result_expr: ValueExpr::RegexCompile { .. },
            ..
        }
    )));
    assert!(main.body.iter().any(|stmt| {
        matches!(
            stmt,
            Statement::Let {
                initializer: ValueExpr::RegexIsMatch { .. },
                ..
            }
        )
    }));
    assert!(main.body.iter().any(|stmt| {
        matches!(
            stmt,
            Statement::Let {
                initializer: ValueExpr::RegexCaptures { .. },
                ..
            }
        )
    }));
}

#[test]
fn rejects_regex_compile_non_string_pattern() {
    let source = r#"package app.main

import std.regex

fn main() -> void {
    let parsed: Result<Regex, RegexError> = regex.compile(42)
}
"#;

    let err = parse_inline(source).unwrap_err();
    assert_eq!(err.code, "E0404");
    assert!(err.message.contains("regex.compile"));
    assert_eq!(err.expected.as_deref(), Some("string"));
    assert_eq!(err.found.as_deref(), Some("i64"));
}

#[test]
fn rejects_json_parse_non_string_argument() {
    let source = r#"package app.main

import std.json

fn main() -> void {
    let parsed: Result<JsonValue, JsonError> = json.parse(42)
}
"#;

    let err = parse_inline(source).unwrap_err();
    assert_eq!(err.code, "E0404");
    assert!(err.message.contains("json.parse"));
    assert_eq!(err.expected.as_deref(), Some("string"));
    assert_eq!(err.found.as_deref(), Some("i64"));
}
