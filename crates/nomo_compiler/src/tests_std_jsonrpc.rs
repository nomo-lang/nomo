use super::*;

#[test]
fn lowers_the_complete_jsonrpc_codec_surface() {
    let source = r#"package app.main

import std.json
import std.jsonrpc

fn run() -> Result<void, JsonRpcProtocolError> {
    let decoder_value: JsonRpcDecoder = jsonrpc.decoder(4096 as u64)?
    let batch: JsonRpcDecodeBatch = jsonrpc.feed(decoder_value, "")?
    jsonrpc.finish(batch.decoder)?
    let id: JsonValue = json.from_i64(1)
    let absent: Option<JsonValue> = None
    let request_value: JsonRpcMessage = jsonrpc.request(id, "initialize", absent)?
    let notification_value: JsonRpcMessage = jsonrpc.notification("ready", absent)?
    let success_value: JsonRpcMessage = jsonrpc.success(id, json.from_null())?
    let failure_value: JsonRpcMessage = jsonrpc.failure(id, -32601, "missing", absent)?
    let parsed: JsonRpcMessage = jsonrpc.parse(jsonrpc.value(request_value), 4096 as u64)?
    let encoded: string = jsonrpc.encode(parsed, 4096 as u64)?
    let kind: JsonRpcMessageKind = jsonrpc.kind(notification_value)
    return Ok(void)
}

fn main() -> void {
    let result: Result<void, JsonRpcProtocolError> = run()
}
"#;

    let program = parse_inline(source).unwrap();
    for expected in [
        "JsonRpcDecodeBatch",
        "JsonRpcDecoder",
        "JsonRpcMessage",
        "JsonRpcProtocolError",
    ] {
        assert!(program.structs.iter().any(|item| item.name == expected));
    }
    assert!(
        program
            .enums
            .iter()
            .any(|item| item.name == "JsonRpcMessageKind")
    );
    let debug = format!("{:?}", program.functions);
    for operation in [
        "Decoder",
        "Feed",
        "Finish",
        "Parse",
        "Encode",
        "Value",
        "Kind",
        "Request",
        "Notification",
        "Success",
        "Failure",
    ] {
        assert!(
            debug.contains(&format!("operation: {operation}")),
            "missing JSON-RPC operation {operation}"
        );
    }
}

#[test]
fn rejects_jsonrpc_argument_type_mismatches() {
    let source = r#"package app.main

import std.jsonrpc

fn main() -> void {
    let decoder_value: Result<JsonRpcDecoder, JsonRpcProtocolError> = jsonrpc.decoder("large")
}
"#;
    let error = parse_inline(source).unwrap_err();
    assert_eq!(error.code, "E0404");
    assert!(error.message.contains("u64"));
}

#[test]
fn lowers_specifically_imported_jsonrpc_function() {
    let source = r#"package app.main

import std.jsonrpc.JsonRpcDecoder
import std.jsonrpc.JsonRpcProtocolError
import std.jsonrpc.decoder

fn main() -> void {
    let created: Result<JsonRpcDecoder, JsonRpcProtocolError> = decoder(4096 as u64)
}
"#;

    let program = parse_inline(source).unwrap();
    let debug = format!("{:?}", program.functions);
    assert!(debug.contains("operation: Decoder"));
}

#[test]
fn permits_pure_jsonrpc_codecs_inside_isolated_tasks() {
    let source = r#"package app.main

import std.jsonrpc
import std.task

fn worker(context: TaskContext, input: string) -> string {
    let created: Result<JsonRpcDecoder, JsonRpcProtocolError> = jsonrpc.decoder(4096 as u64)
    return input
}

fn main() -> void {
    let started: Result<Task, TaskError> = task.spawn(worker, "payload")
}
"#;

    parse_inline(source).unwrap();
}

#[test]
fn rejects_forged_jsonrpc_values_and_private_field_access() {
    let forged = r#"package app.main

import std.jsonrpc

fn main() -> void {
    let message: JsonRpcMessage = JsonRpcMessage { raw: "{}" }
}
"#;
    let error = parse_inline(forged).unwrap_err();
    assert_eq!(error.code, "E0840");
    assert!(error.message.contains("cannot be constructed"));

    let exposed = r#"package app.main

import std.io
import std.jsonrpc

fn main() -> void {
    let decoder_value: Result<JsonRpcDecoder, JsonRpcProtocolError> = jsonrpc.decoder(16 as u64)
    match decoder_value {
        Ok(value) => {
            io.println(value.pending)
        }
        Err(error) => {
        }
    }
}
"#;
    let error = parse_inline(exposed).unwrap_err();
    assert_eq!(error.code, "E0840");
    assert!(error.message.contains("does not expose its fields"));

    let mutated = r#"package app.main

import std.jsonrpc

fn run() -> Result<void, JsonRpcProtocolError> {
    let mut decoder_value: JsonRpcDecoder = jsonrpc.decoder(16 as u64)?
    decoder_value.pending = ""
    return Ok(void)
}

fn main() -> void {
    let result: Result<void, JsonRpcProtocolError> = run()
}
"#;
    let error = parse_inline(mutated).unwrap_err();
    assert_eq!(error.code, "E0840");
    assert!(error.message.contains("does not expose its fields"));
}

#[test]
fn reports_missing_jsonrpc_type_import() {
    let source = r#"package app.main

fn keep(message: JsonRpcMessage) -> void {
}

fn main() -> void {
}
"#;
    let error = parse_inline(source).unwrap_err();
    assert_eq!(error.code, "E0301");
    assert_eq!(
        error.message,
        "`JsonRpcMessage` requires `import std.jsonrpc`"
    );
}
