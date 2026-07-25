//! Browser-safe Nomo compiler and interpreter.
//!
//! The crate compiles Nomo source with the production lexer, parser, and
//! semantic lowering pipeline, then evaluates the resulting typed IR without
//! granting host filesystem, process, environment, or network capabilities.

mod cron;
mod interpreter;
mod json;
mod jsonrpc;

use interpreter::{ExecutionLimits, Interpreter};
use nomo_compiler::{Program, check_source_text};
use serde::Serialize;
use std::path::Path;

pub const ENGINE_NAME: &str = "nomo-wasm";
pub const ENGINE_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const MAX_SOURCE_BYTES: usize = 256 * 1024;

#[derive(Debug, Clone, Serialize)]
pub struct DiagnosticRecord {
    pub code: String,
    pub severity: String,
    pub message: String,
    pub file: String,
    pub line: usize,
    pub column: usize,
    pub length: usize,
    pub text: String,
    pub expected: Option<String>,
    pub found: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RuntimeErrorRecord {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct ExecutionStats {
    pub steps: u64,
    pub output_bytes: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct RunResponse {
    pub status: &'static str,
    pub engine: &'static str,
    pub engine_version: &'static str,
    pub stdout: String,
    pub stderr: String,
    pub diagnostic: Option<DiagnosticRecord>,
    pub runtime_error: Option<RuntimeErrorRecord>,
    pub stats: ExecutionStats,
}

impl RunResponse {
    fn compile_error(diagnostic: nomo_compiler::diagnostic::Diagnostic) -> Self {
        Self {
            status: "compile_error",
            engine: ENGINE_NAME,
            engine_version: ENGINE_VERSION,
            stdout: String::new(),
            stderr: String::new(),
            diagnostic: Some(DiagnosticRecord {
                code: diagnostic.code.to_string(),
                severity: diagnostic.severity.to_string(),
                message: diagnostic.message,
                file: diagnostic.file,
                line: diagnostic.line,
                column: diagnostic.column,
                length: diagnostic.length,
                text: diagnostic.text,
                expected: diagnostic.expected,
                found: diagnostic.found,
            }),
            runtime_error: None,
            stats: ExecutionStats {
                steps: 0,
                output_bytes: 0,
            },
        }
    }
}

fn compile(source: &str) -> Result<Program, Box<nomo_compiler::diagnostic::Diagnostic>> {
    check_source_text(Path::new("main.nomo"), source).map_err(Box::new)
}

pub fn check_source(source: &str) -> RunResponse {
    match compile(source) {
        Ok(_) => RunResponse {
            status: "ready",
            engine: ENGINE_NAME,
            engine_version: ENGINE_VERSION,
            stdout: String::new(),
            stderr: String::new(),
            diagnostic: None,
            runtime_error: None,
            stats: ExecutionStats {
                steps: 0,
                output_bytes: 0,
            },
        },
        Err(diagnostic) => RunResponse::compile_error(*diagnostic),
    }
}

pub fn run_source(source: &str, limits: ExecutionLimits) -> RunResponse {
    let program = match compile(source) {
        Ok(program) => program,
        Err(diagnostic) => return RunResponse::compile_error(*diagnostic),
    };

    let mut interpreter = Interpreter::new(&program, limits);
    match interpreter.run_main() {
        Ok(()) => {
            let stats = ExecutionStats {
                steps: interpreter.steps(),
                output_bytes: interpreter.output_bytes(),
            };
            let (stdout, stderr) = interpreter.into_output();
            RunResponse {
                status: "success",
                engine: ENGINE_NAME,
                engine_version: ENGINE_VERSION,
                stdout,
                stderr,
                diagnostic: None,
                runtime_error: None,
                stats,
            }
        }
        Err(error) => {
            let stats = ExecutionStats {
                steps: interpreter.steps(),
                output_bytes: interpreter.output_bytes(),
            };
            let (stdout, stderr) = interpreter.into_output();
            RunResponse {
                status: "runtime_error",
                engine: ENGINE_NAME,
                engine_version: ENGINE_VERSION,
                stdout,
                stderr,
                diagnostic: None,
                runtime_error: Some(RuntimeErrorRecord {
                    code: error.code.to_string(),
                    message: error.message,
                }),
                stats,
            }
        }
    }
}

pub fn check_json(source: &str) -> String {
    serde_json::to_string(&check_source(source)).expect("RunResponse is serializable")
}

pub fn run_json(source: &str, max_steps: u64, max_output_bytes: usize) -> String {
    serde_json::to_string(&run_source(
        source,
        ExecutionLimits {
            max_steps,
            max_output_bytes,
            ..ExecutionLimits::default()
        },
    ))
    .expect("RunResponse is serializable")
}

#[cfg(target_arch = "wasm32")]
mod wasm_abi {
    use super::*;
    use std::slice;
    use std::sync::Mutex;

    static LAST_RESULT: Mutex<Vec<u8>> = Mutex::new(Vec::new());

    #[unsafe(no_mangle)]
    pub extern "C" fn nomo_alloc(length: usize) -> *mut u8 {
        if length > MAX_SOURCE_BYTES {
            return std::ptr::null_mut();
        }
        let mut bytes = Vec::<u8>::with_capacity(length);
        let pointer = bytes.as_mut_ptr();
        std::mem::forget(bytes);
        pointer
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn nomo_dealloc(pointer: *mut u8, capacity: usize) {
        if pointer.is_null() || capacity == 0 {
            return;
        }
        // SAFETY: `pointer` and `capacity` must come from `nomo_alloc`.
        drop(unsafe { Vec::from_raw_parts(pointer, 0, capacity) });
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn nomo_check(pointer: *const u8, length: usize) {
        store_result(unsafe { source_from_raw(pointer, length) }.map(check_json));
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn nomo_run(
        pointer: *const u8,
        length: usize,
        max_steps: u64,
        max_output_bytes: usize,
    ) {
        store_result(
            unsafe { source_from_raw(pointer, length) }
                .map(|source| run_json(source, max_steps, max_output_bytes)),
        );
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn nomo_result_ptr() -> *const u8 {
        LAST_RESULT
            .lock()
            .expect("result mutex is not poisoned")
            .as_ptr()
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn nomo_result_len() -> usize {
        LAST_RESULT
            .lock()
            .expect("result mutex is not poisoned")
            .len()
    }

    unsafe fn source_from_raw<'a>(pointer: *const u8, length: usize) -> Result<&'a str, String> {
        if length > MAX_SOURCE_BYTES {
            return Err(format!(
                "source exceeds the browser limit of {MAX_SOURCE_BYTES} bytes"
            ));
        }
        if pointer.is_null() && length != 0 {
            return Err("source pointer is null".to_string());
        }
        // SAFETY: the JavaScript wrapper writes `length` bytes into a buffer
        // returned by `nomo_alloc` and keeps it alive for this call.
        let bytes = unsafe { slice::from_raw_parts(pointer, length) };
        std::str::from_utf8(bytes).map_err(|error| format!("source is not UTF-8: {error}"))
    }

    fn store_result(result: Result<String, String>) {
        let json = result.unwrap_or_else(|message| {
            serde_json::json!({
                "status": "runtime_error",
                "engine": ENGINE_NAME,
                "engine_version": ENGINE_VERSION,
                "stdout": "",
                "stderr": "",
                "diagnostic": null,
                "runtime_error": {
                    "code": "NOMO-WASM-004",
                    "message": message,
                },
                "stats": {
                    "steps": 0,
                    "output_bytes": 0,
                },
            })
            .to_string()
        });
        *LAST_RESULT.lock().expect("result mutex is not poisoned") = json.into_bytes();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runs_functions_bindings_and_a_bounded_loop() {
        let source = r#"package app.main

import std.io
import std.num

fn greeting() -> string {
    return "Hello, WASM"
}

fn main() -> void {
    let message: string = greeting()
    let mut i: u64 = 0
    for i < 3 {
        io.println(message)
        io.println(num.to_string(i))
        i++
    }
}
"#;
        let response = run_source(source, ExecutionLimits::default());

        assert_eq!(response.status, "success", "{response:#?}");
        assert_eq!(
            response.stdout,
            "Hello, WASM\n0\nHello, WASM\n1\nHello, WASM\n2\n"
        );
        assert!(response.diagnostic.is_none());
    }

    #[test]
    fn runs_three_clause_loop_with_ui64_alias_and_multi_argument_println() {
        let source = r#"package app.main

import std.io

fn greeting() -> string {
    return "Hello, final audit"
}

fn main() -> void {
    let message = greeting()
    for let i: ui64 = 0; i < 3; i++ {
        io.println(message, i)
    }
}
"#;
        let response = run_source(source, ExecutionLimits::default());

        assert_eq!(response.status, "success", "{response:#?}");
        assert_eq!(
            response.stdout,
            "Hello, final audit 0\nHello, final audit 1\nHello, final audit 2\n"
        );
        assert!(response.diagnostic.is_none());
    }

    #[test]
    fn runs_the_p1_cooperative_yield_surface_in_the_browser_sandbox() {
        let source = r#"package app.main

import std.io
import std.task

suspend fn yield_once(result: string) -> string {
    io.println("child-before")
    task.yield_now()
    io.println("child-after")
    return result
}

suspend fn main() -> void {
    io.println("before")
    let result: string = yield_once("after")
    io.println(result)
}
"#;
        let response = run_source(source, ExecutionLimits::default());

        assert_eq!(response.status, "success", "{response:#?}");
        assert_eq!(
            response.stdout,
            "before\nchild-before\nchild-after\nafter\n"
        );
        assert!(response.diagnostic.is_none());
        assert!(response.stats.steps >= 2);
    }

    #[test]
    fn returns_timer_runtime_unavailable_without_evaluating_the_duration() {
        let source = r#"package app.main

import std.io
import std.result
import std.task
import std.time

fn duration() -> Duration {
    panic("browser-timer-duration-must-not-run")
}

suspend fn main() -> void {
    let waited: Result<void, TaskError> = task.sleep(duration())
    io.println(result.is_err(waited))
}
"#;
        let response = run_source(source, ExecutionLimits::default());

        assert_eq!(response.status, "success", "{response:#?}");
        assert_eq!(response.stdout, "true\n");
        assert!(
            !response
                .stderr
                .contains("browser-timer-duration-must-not-run")
        );
        assert!(response.diagnostic.is_none());
    }

    #[test]
    fn returns_structured_task_runtime_unavailable_without_invoking_the_child() {
        let source = r#"package app.main

import std.io
import std.result
import std.task

suspend fn child(secret: string) -> string {
    io.println(secret)
    return secret
}

suspend fn gather() -> string {
    task.scope {
        let child_task = task.spawn child("browser-structured-task-secret")
        let joined: Result<string, TaskError> = task.join(child_task)
        return result.unwrap_or(joined, "runtime_unavailable")
    }
}

suspend fn main() -> void {
    let gathered: string = gather()
    io.println(gathered)
}
"#;
        let response = run_source(source, ExecutionLimits::default());

        assert_eq!(response.status, "success", "{response:#?}");
        assert_eq!(response.stdout, "runtime_unavailable\n");
        assert!(!response.stderr.contains("browser-structured-task-secret"));
        assert!(response.diagnostic.is_none());
    }

    #[test]
    fn structured_scope_auto_cancel_does_not_invoke_browser_children() {
        let source = r#"package app.main

import std.io
import std.task

suspend fn child(secret: string) -> void {
    io.println(secret)
}

suspend fn main() -> void {
    task.scope {
        let child_task = task.spawn child("browser-auto-cancel-secret")
    }
    io.println("scope closed")
}
"#;
        let response = run_source(source, ExecutionLimits::default());

        assert_eq!(response.status, "success", "{response:#?}");
        assert_eq!(response.stdout, "scope closed\n");
        assert!(!response.stderr.contains("browser-auto-cancel-secret"));
        assert!(response.diagnostic.is_none());
    }

    #[test]
    fn runs_structured_json_with_native_semantics() {
        let source = r#"package app.main

import std.array.Array
import std.io
import std.json

fn main() -> void {
    let text_result: Result<JsonValue, JsonError> = json.from_string("Nomo\n😀")
    let number_result: Result<JsonValue, JsonError> = json.from_number_text("1e+2")
    match text_result {
        Err(err) => {
            io.println(err.code, err.offset)
        }
        Ok(text) => {
            match number_result {
                Err(err) => {
                    io.println(err.code, err.offset)
                }
                Ok(number) => {
                    let mut members: Array<JsonMember> = Array.new<JsonMember>()
                    members.push(JsonMember { key: "text", value: text })
                    members.push(JsonMember { key: "count", value: number })
                    let built: Result<JsonValue, JsonError> = json.from_object(members)
                    match built {
                        Err(err) => {
                            io.println(err.code, err.offset)
                        }
                        Ok(value) => {
                            io.println(json.stringify(value))
                        }
                    }
                }
            }
        }
    }

    let parsed: Result<JsonValue, JsonError> = json.parse("{\"x\":1,\"x\":\"last\",\"items\":[true]}")
    match parsed {
        Err(err) => {
            io.println(err.code)
        }
        Ok(root) => {
            let selected: Option<JsonValue> = json.get(root, "x")
            match selected {
                None => {
                    io.println("missing")
                }
                Some(item) => {
                    let text: Option<string> = json.as_string(item)
                    match text {
                        None => {
                            io.println("wrong kind")
                        }
                        Some(value) => {
                            io.println(value)
                        }
                    }
                }
            }
        }
    }
}
"#;
        let response = run_source(source, ExecutionLimits::default());

        assert_eq!(response.status, "success", "{response:#?}");
        assert_eq!(
            response.stdout,
            "{\"text\":\"Nomo\\n😀\",\"count\":1e+2}\nlast\n"
        );
        assert!(response.diagnostic.is_none());
    }

    #[test]
    fn structured_json_conformance_fixture_matches_browser_runtime() {
        let source = include_str!("../../../tests/fixtures/structured_json_conformance.nomo");
        let response = run_source(source, ExecutionLimits::default());

        assert_eq!(response.status, "success", "{response:#?}");
        assert_eq!(
            response.stdout,
            "true\n6\nnull\nboolean\nnumber\nstring\narray\nobject\ntrue\ntrue\nwrong-kind-none\n1E+2\ntrue\n0\n0\nnon-object-none\n2\nname\nname\n2\nmissing-none\n\"A\\n\\\"\\\\😀\"\n{\"null\":null,\"bool\":false,\"i64\":-9223372036854775808,\"u64\":18446744073709551615}\n😀\ninvalid_number 1 invalid json number\nunsupported_string 1\nunsupported_string 1\nsyntax 29 invalid json syntax\n"
        );
        assert!(!response.stdout.contains("NOMO_JSON_SECRET_SENTINEL"));
        assert!(response.diagnostic.is_none());
    }

    #[test]
    fn jsonrpc_conformance_fixture_matches_browser_runtime() {
        let source = include_str!("../../../tests/fixtures/jsonrpc_conformance.nomo");
        let response = run_source(source, ExecutionLimits::default());

        assert_eq!(response.status, "success", "{response:#?}");
        assert_eq!(
            response.stdout,
            "0\n2\nrequest\n{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\"}\nnotification\n{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n{\"jsonrpc\":\"2.0\",\"id\":7,\"method\":\"tools/list\"}\n{\"jsonrpc\":\"2.0\",\"id\":7,\"result\":true}\n{\"jsonrpc\":\"2.0\",\"id\":7,\"error\":{\"code\":-32601,\"message\":\"missing\"}}\n"
        );
        assert!(response.diagnostic.is_none());
    }

    #[test]
    fn jsonrpc_error_fixture_matches_browser_runtime_without_secret_echo() {
        let source = include_str!("../../../tests/fixtures/jsonrpc_errors.nomo");
        let response = run_source(source, ExecutionLimits::default());

        assert_eq!(response.status, "success", "{response:#?}");
        for expected in [
            "zero invalid_request invalid JSON-RPC argument\n",
            "array protocol invalid JSON-RPC 2.0 envelope\n",
            "newline framing invalid JSON-RPC newline framing\n",
            "duplicate protocol invalid JSON-RPC 2.0 envelope\n",
            "fractional-code protocol invalid JSON-RPC 2.0 envelope\n",
            "extension ok\n",
            "empty-line framing invalid JSON-RPC newline framing\n",
            "malformed json invalid bounded JSON input\n",
            "partial framing invalid JSON-RPC newline framing\n",
            "line-limit limit JSON-RPC limit exceeded\n",
            "bool-id protocol invalid JSON-RPC 2.0 envelope\n",
            "scalar-params protocol invalid JSON-RPC 2.0 envelope\n",
        ] {
            assert!(
                response.stdout.contains(expected),
                "missing {expected:?} in:\n{}",
                response.stdout
            );
        }
        assert!(!response.stdout.contains("NOMO_JSONRPC_SECRET_SENTINEL"));
        assert!(response.diagnostic.is_none());
    }

    #[test]
    fn cron_conformance_fixture_matches_browser_runtime_without_secret_echo() {
        let source = include_str!("../../../tests/fixtures/cron_conformance.nomo");
        let response = run_source(source, ExecutionLimits::default());

        assert_eq!(response.status, "success", "{response:#?}");
        assert_eq!(
            response.stdout,
            "true\n60000\n900000\n3600000\n68169600000\n4233686400000\ntrue\nfalse\ntrue\nfalse\nrange 0\nrange 0\nsyntax 0\nrange 0\nlimit 5\nsyntax 5 invalid cron expression syntax\ntimestamp_range 5\nno_match 5\n"
        );
        assert!(!response.stdout.contains("NOMO_CRON_SECRET_SENTINEL"));
        assert!(response.diagnostic.is_none());
    }

    #[test]
    fn runs_std_fmt_templates_and_display_structs() {
        let source = r#"package app.main

import std.fmt
import std.io

struct User {
    name: string
}

impl fmt.Display for User {
    fn to_string(self) -> string {
        return self.name
    }
}

fn main() -> void {
    let user: User = User { name: "WASM" }
    io.println(fmt.format("Hello, {} {}", user, 7))
    io.println(user)
}
"#;
        let response = run_source(source, ExecutionLimits::default());

        assert_eq!(response.status, "success", "{response:#?}");
        assert_eq!(response.stdout, "Hello, WASM 7\nWASM\n");
        assert!(response.diagnostic.is_none());
    }

    #[test]
    fn three_clause_loop_runs_update_after_continue() {
        let source = r#"package app.main

import std.io

fn main() -> void {
    for let i = 0; i < 1; i++ {
        continue
    }
    io.println("done")
}
"#;
        let response = run_source(source, ExecutionLimits::default());

        assert_eq!(response.status, "success", "{response:#?}");
        assert_eq!(response.stdout, "done\n");
    }

    #[test]
    fn stops_infinite_programs_with_fuel() {
        let source = r#"package app.main

fn main() -> void {
    for {
    }
}
"#;
        let response = run_source(
            source,
            ExecutionLimits {
                max_steps: 32,
                ..ExecutionLimits::default()
            },
        );

        assert_eq!(response.status, "runtime_error");
        assert_eq!(
            response
                .runtime_error
                .as_ref()
                .map(|error| error.code.as_str()),
            Some("NOMO-WASM-001")
        );
    }

    #[test]
    fn enforces_the_output_limit() {
        let source = r#"package app.main

import std.io

fn main() -> void {
    for {
        io.println("0123456789")
    }
}
"#;
        let response = run_source(
            source,
            ExecutionLimits {
                max_steps: 1_000,
                max_output_bytes: 24,
                ..ExecutionLimits::default()
            },
        );

        assert_eq!(response.status, "runtime_error");
        assert_eq!(
            response
                .runtime_error
                .as_ref()
                .map(|error| error.code.as_str()),
            Some("NOMO-WASM-002")
        );
    }

    #[test]
    fn rejects_structured_http_without_leaking_request_secrets() {
        let source = r#"package app.main

import std.array.Array
import std.http
import std.result

fn main() -> void {
    let mut headers: Array<HttpHeader> = Array.new<HttpHeader>()
    headers.push(HttpHeader {
        name: "Authorization",
        value: "Bearer browser-secret"
    })
    let request: HttpRequest = HttpRequest {
        method: "POST",
        url: "https://example.invalid/v1/chat/completions?token=query-secret",
        headers: headers,
        body: "body-secret",
        timeout_millis: 1000,
        max_response_bytes: 1024
    }
    let result: Result<HttpResponse, HttpError> = http.send(request)
}
"#;
        let response = run_source(source, ExecutionLimits::default());

        assert_eq!(response.status, "runtime_error", "{response:#?}");
        let error = response
            .runtime_error
            .as_ref()
            .expect("structured HTTP should return a capability error");
        assert_eq!(error.code, "NOMO-WASM-003");
        assert!(error.message.contains("network"));
        assert!(error.message.contains("browser sandbox"));
        assert!(!error.message.contains("__nomo_http_send"));
        for secret in ["browser-secret", "query-secret", "body-secret"] {
            assert!(!error.message.contains(secret), "{error:#?}");
        }
    }

    #[test]
    fn rejects_http_streaming_without_evaluating_or_leaking_request_secrets() {
        let source = r#"package app.main

import std.array.Array
import std.http
import std.result

fn main() -> void {
    let mut headers: Array<HttpHeader> = Array.new<HttpHeader>()
    headers.push(HttpHeader {
        name: "Authorization",
        value: "Bearer browser-stream-secret"
    })
    let request: HttpRequest = HttpRequest {
        method: "POST",
        url: "https://example.invalid/v1/chat/completions?token=stream-query-secret",
        headers: headers,
        body: "stream-body-secret",
        timeout_millis: 1000,
        max_response_bytes: 1024
    }
    let result: Result<HttpStream, HttpError> = http.open_stream(request, 1000)
}
"#;
        let response = run_source(source, ExecutionLimits::default());

        assert_eq!(response.status, "runtime_error", "{response:#?}");
        let error = response
            .runtime_error
            .as_ref()
            .expect("HTTP streaming should return a capability error");
        assert_eq!(error.code, "NOMO-WASM-003");
        assert!(error.message.contains("network"));
        assert!(error.message.contains("browser sandbox"));
        assert!(!error.message.contains("__nomo_http_open_stream"));
        for secret in [
            "browser-stream-secret",
            "stream-query-secret",
            "stream-body-secret",
        ] {
            assert!(!error.message.contains(secret), "{error:#?}");
        }
    }

    #[test]
    fn rejects_controlled_processes_without_leaking_command_secrets() {
        let source = r#"package app.main

import std.array.Array
import std.process

fn main() -> void {
    let mut args: Array<string> = Array.new<string>()
    args.push("argv-browser-secret")
    let mut environment: Array<ProcessEnv> = Array.new<ProcessEnv>()
    environment.push(ProcessEnv {
        name: "NOMO_BROWSER_TOKEN",
        value: "environment-browser-secret"
    })
    let command: ProcessCommand = ProcessCommand {
        program: "program-browser-secret",
        args: args,
        cwd: Some("cwd-browser-secret"),
        env: environment,
        inherit_env: false
    }
    let result: Result<ProcessChild, ProcessControlError> = process.start(command)
}
"#;
        let response = run_source(source, ExecutionLimits::default());

        assert_eq!(response.status, "runtime_error", "{response:#?}");
        let error = response
            .runtime_error
            .as_ref()
            .expect("controlled process should return a capability error");
        assert_eq!(error.code, "NOMO-WASM-003");
        assert!(error.message.contains("process"));
        assert!(error.message.contains("browser sandbox"));
        assert!(!error.message.contains("__nomo_process_start"));
        for secret in [
            "argv-browser-secret",
            "environment-browser-secret",
            "program-browser-secret",
            "cwd-browser-secret",
        ] {
            assert!(!error.message.contains(secret), "{error:#?}");
        }
    }

    #[test]
    fn returns_task_runtime_unavailable_without_invoking_the_worker() {
        let source = r#"package app.main

import std.io
import std.task

fn worker(context: TaskContext, input: string) -> string {
    panic("worker must not run")
}

fn main() -> void {
    let started: Result<Task, TaskError> = task.spawn(worker, "browser-task-secret")
    match started {
        Ok(task_value) => {
            io.println("unexpected")
        }
        Err(error) => {
            io.println(error.code)
        }
    }
}
"#;
        let response = run_source(source, ExecutionLimits::default());

        assert_eq!(response.status, "success", "{response:#?}");
        assert_eq!(response.stdout, "runtime_unavailable\n");
        assert!(!response.stderr.contains("worker must not run"));
        assert!(!response.stderr.contains("browser-task-secret"));
    }

    #[test]
    fn returns_sqlite_runtime_unavailable_without_leaking_paths() {
        let source = r#"package app.main

import std.io
import std.sqlite

fn main() -> void {
    let opened: Result<SqliteDatabase, SqliteError> = sqlite.open(
        "browser-secret-agent.db",
        SqliteOpenMode.ReadWriteCreate,
        0
    )
    match opened {
        Ok(database) => {
            io.println("unexpected")
        }
        Err(error) => {
            io.println(error.code, error.native_code)
        }
    }
}
"#;
        let response = run_source(source, ExecutionLimits::default());

        assert_eq!(response.status, "success", "{response:#?}");
        assert_eq!(response.stdout, "runtime_unavailable 0\n");
        assert!(!response.stderr.contains("browser-secret-agent.db"));
        assert!(!response.stderr.contains("__nomo_sqlite_open"));
    }

    #[test]
    fn matches_native_checked_wrapping_math_and_utf8_semantics() {
        let source = r#"package app.main

import std.io
import std.math
import std.num
import std.string

fn main() -> void {
    let checked: Option<i64> = num.checked_add(9223372036854775807, 1)
    match checked {
        Option.Some(value) => {
            io.println(num.to_string(value))
        }
        Option.None => {
            io.println("none")
        }
    }
    io.println(num.to_string(num.wrapping_add(9223372036854775807, 1)))
    io.println(num.to_string(num.wrapping_sub(0 as u64, 1 as u64)))
    io.println(num.to_string(math.abs(0 - 7)))
    io.println(num.to_string(math.min(3 as i32, 9 as i32)))
    io.println(num.to_string(string.len("你好")))
}
"#;
        let response = run_source(source, ExecutionLimits::default());

        assert_eq!(response.status, "success", "{response:#?}");
        assert_eq!(
            response.stdout,
            "none\n-9223372036854775808\n18446744073709551615\n7\n3\n6\n"
        );
    }

    #[test]
    fn executes_value_semantics_array_mutations() {
        let source = r#"package app.main

import std.array
import std.io

fn main() -> void {
    let mut items: Array<i32> = Array.new<i32>()
    items.push(1)
    items.push(2)
    items.set(0, 7)
    let first: Option<i32> = items.get(0)
    let message: string = match first {
        Some(value) => if value == 7 {
            "array ok"
        } else {
            "wrong"
        }
        None => "missing"
    }
    io.println(message)
}
"#;
        let response = run_source(source, ExecutionLimits::default());

        assert_eq!(response.status, "success", "{response:#?}");
        assert_eq!(response.stdout, "array ok\n");
    }

    #[test]
    fn executes_nested_array_literals_and_copy_on_write_index_assignment() {
        let source = r#"package app.main

import std.io
import std.num

fn main() -> void {
    let mut matrix = [[1, 2], [3, 4]]
    let jagged = [[8], [9, 10]]
    let snapshot = matrix
    matrix[0][1] = 7
    io.println(num.to_string(matrix[0][1]))
    io.println(num.to_string(snapshot[0][1]))
    io.println(num.to_string(jagged[1][1]))
}
"#;
        let response = run_source(source, ExecutionLimits::default());

        assert_eq!(response.status, "success", "{response:#?}");
        assert_eq!(response.stdout, "7\n2\n10\n");
    }

    #[test]
    fn reports_stable_array_index_bounds_failure() {
        let source = r#"package app.main

fn main() -> void {
    let values = [1]
    let missing: i32 = values[1]
}
"#;
        let response = run_source(source, ExecutionLimits::default());

        assert_eq!(response.status, "runtime_error", "{response:#?}");
        assert_eq!(
            response
                .runtime_error
                .as_ref()
                .map(|error| error.message.as_str()),
            Some("array index out of bounds")
        );
    }

    #[test]
    fn executes_insertion_ordered_generic_map() {
        let source = r#"package app.main

import std.io
import std.map
import std.array

fn main() -> void {
    let mut values: Map<string, string> = Map.new<string, string>()
    let first: Option<string> = map.set<string, string>(mut values, "b", "two")
    let second: Option<string> = map.set<string, string>(mut values, "a", "one")
    let replaced: Option<string> = map.set<string, string>(mut values, "b", "updated")
    let keys: Array<string> = map.keys<string, string>(values)
    io.println(keys[0])
    io.println(keys[1])
    io.println(if map.contains_key<string, string>(values, "a") {
        "present"
    } else {
        "missing"
    })
    match map.get<string, string>(values, "b") {
        Some(value) => {
            io.println(value)
        }
        None => {
            io.println("missing")
        }
    }
    match replaced {
        Some(value) => {
            io.println(value)
        }
        None => {
            io.println("missing")
        }
    }
    let removed: Option<string> = map.remove<string, string>(mut values, "a")
    io.println(if map.len<string, string>(values) == 1 {
        "removed"
    } else {
        "wrong"
    })
    map.clear<string, string>(mut values)
    io.println(if map.is_empty<string, string>(values) {
        "empty"
    } else {
        "wrong"
    })
}
"#;
        let response = run_source(source, ExecutionLimits::default());

        assert_eq!(response.status, "success", "{response:#?}");
        assert_eq!(
            response.stdout,
            "b\na\npresent\nupdated\ntwo\nremoved\nempty\n"
        );
    }
}
