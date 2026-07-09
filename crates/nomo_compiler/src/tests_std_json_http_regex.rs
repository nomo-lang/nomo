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
fn accepts_http_client_builtins() {
    let source = r#"package app.main

import std.http

fn main() -> Result<void, HttpError> {
    let first: HttpResponse = http.get("http://127.0.0.1/hello")?
    let second: HttpResponse = http.post("http://127.0.0.1/echo", "body")?
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
        } if value_name == "HttpResponse" && args.is_empty() && call_name == BUILTIN_HTTP_GET_EXPR
    ));
    assert!(matches!(
        main.body[1],
        Statement::QuestionLet {
            value_type: ValueType::Struct(ref value_name, ref args),
            result_expr: ValueExpr::Call { name: ref call_name, .. },
            ..
        } if value_name == "HttpResponse" && args.is_empty() && call_name == BUILTIN_HTTP_POST_EXPR
    ));
}

#[test]
fn accepts_specific_http_builtin_imports() {
    let source = r#"package app.main

import std.http.HttpError
import std.http.HttpResponse
import std.http.get
import std.http.post

fn main() -> Result<void, HttpError> {
    let first: HttpResponse = get("http://127.0.0.1/hello")?
    let second: HttpResponse = post("http://127.0.0.1/echo", "body")?
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
        } if name == BUILTIN_HTTP_GET_EXPR
    ));
    assert!(matches!(
        main.body[1],
        Statement::QuestionLet {
            result_expr: ValueExpr::Call { ref name, .. },
            ..
        } if name == BUILTIN_HTTP_POST_EXPR
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
