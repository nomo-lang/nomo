//! Browser-safe Nomo compiler and interpreter.
//!
//! The crate compiles Nomo source with the production lexer, parser, and
//! semantic lowering pipeline, then evaluates the resulting typed IR without
//! granting host filesystem, process, environment, or network capabilities.

mod interpreter;

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
}
