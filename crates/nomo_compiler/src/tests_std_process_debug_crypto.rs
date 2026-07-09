use super::*;

#[test]
fn accepts_process_builtins() {
    let source = r#"package app.main

import std.process
import std.result

fn capture() -> Result<ProcessOutput, ProcessError> {
    return process.output("printf captured-ok")?
}

fn run() -> Result<string, ProcessError> {
    let spawned: i32 = process.spawn("printf spawn-ok >/dev/null")?
    let status: i32 = process.status("printf status-ok >/dev/null")?
    return process.exec("printf process-ok")
}

fn quit() -> void {
    process.exit(0)
}

fn main() -> void {
}
"#;

    let program = parse_inline(source).unwrap();
    assert!(
        program
            .structs
            .iter()
            .any(|item| item.name == "ProcessError")
    );
    assert!(
        program
            .structs
            .iter()
            .any(|item| item.name == "ProcessOutput")
    );
    let capture = program
        .functions
        .iter()
        .find(|f| f.name == "capture")
        .unwrap();
    assert!(matches!(
        capture.body[0],
        Statement::QuestionReturn {
            result_expr: ValueExpr::ProcessOutput { .. },
            ..
        }
    ));
    let run = program.functions.iter().find(|f| f.name == "run").unwrap();
    assert!(matches!(
        run.body[0],
        Statement::QuestionLet {
            value_type: ValueType::I32,
            result_expr: ValueExpr::ProcessSpawn { .. },
            ..
        }
    ));
    assert!(matches!(
        run.body[1],
        Statement::QuestionLet {
            value_type: ValueType::I32,
            result_expr: ValueExpr::ProcessStatus { .. },
            ..
        }
    ));
    assert!(matches!(
        run.body[2],
        Statement::Return(Some(ValueExpr::ProcessExec { .. }))
    ));
    let quit = program.functions.iter().find(|f| f.name == "quit").unwrap();
    assert!(matches!(
        quit.body[0],
        Statement::Expr(ValueExpr::ProcessExit { .. })
    ));
}

#[test]
fn accepts_specific_process_builtin_imports() {
    let source = r#"package app.main

import std.process.exec
import std.process.output
import std.process.spawn
import std.process.status
import std.result

fn capture() -> Result<ProcessOutput, ProcessError> {
    return output("printf captured-ok")?
}

fn run() -> Result<string, ProcessError> {
    let spawned: i32 = spawn("printf spawn-ok >/dev/null")?
    let status: i32 = status("printf status-ok >/dev/null")?
    return exec("printf process-ok")
}

fn main() -> void {
}
"#;

    let program = parse_inline(source).unwrap();
    let capture = program
        .functions
        .iter()
        .find(|f| f.name == "capture")
        .unwrap();
    assert!(matches!(
        capture.body[0],
        Statement::QuestionReturn {
            result_expr: ValueExpr::ProcessOutput { .. },
            ..
        }
    ));
    let run = program.functions.iter().find(|f| f.name == "run").unwrap();
    assert!(matches!(
        run.body[0],
        Statement::QuestionLet {
            result_expr: ValueExpr::ProcessSpawn { .. },
            ..
        }
    ));
    assert!(matches!(
        run.body[1],
        Statement::QuestionLet {
            result_expr: ValueExpr::ProcessStatus { .. },
            ..
        }
    ));
    assert!(matches!(
        run.body[2],
        Statement::Return(Some(ValueExpr::ProcessExec { .. }))
    ));
}

#[test]
fn accepts_testing_builtins() {
    let source = r#"package app.main

import std.result
import std.testing

fn fail() -> Result<i64, string> {
    return Err("boom")
}

fn main() -> void {
    testing.assert(true, "expected true")
    testing.assert_equal(1, 1)
    testing.assert_equal("same", "same")
    testing.assert_error(fail())
}
"#;

    let program = parse_inline(source).unwrap();
    let main = program.functions.iter().find(|f| f.name == "main").unwrap();
    assert_eq!(main.body.len(), 4);
    assert!(matches!(
        main.body[0],
        Statement::Expr(ValueExpr::If { .. })
    ));
    assert!(matches!(
        main.body[1],
        Statement::Expr(ValueExpr::If { .. })
    ));
    assert!(matches!(
        main.body[2],
        Statement::Expr(ValueExpr::If { .. })
    ));
    assert!(matches!(
        main.body[3],
        Statement::Expr(ValueExpr::If { .. })
    ));
}

#[test]
fn accepts_specific_testing_builtin_imports() {
    let source = r#"package app.main

import std.result
import std.testing.assert
import std.testing.assert_equal
import std.testing.assert_error

fn fail() -> Result<i64, string> {
    return Err("boom")
}

fn main() -> void {
    assert(true, "expected true")
    assert_equal('n', 'n')
    assert_error(fail())
}
"#;

    let program = parse_inline(source).unwrap();
    let main = program.functions.iter().find(|f| f.name == "main").unwrap();
    assert_eq!(main.body.len(), 3);
    assert!(
        main.body
            .iter()
            .all(|stmt| matches!(stmt, Statement::Expr(ValueExpr::If { .. })))
    );
}

#[test]
fn rejects_testing_assert_non_bool_condition() {
    let source = r#"package app.main

import std.testing

fn main() -> void {
    testing.assert("nope", "expected bool")
}
"#;

    let err = parse_inline(source).unwrap_err();
    assert_eq!(err.code, "E0404");
    assert!(err.message.contains("bool condition"));
}

#[test]
fn rejects_testing_assert_error_non_result() {
    let source = r#"package app.main

import std.testing

fn main() -> void {
    testing.assert_error(1)
}
"#;

    let err = parse_inline(source).unwrap_err();
    assert_eq!(err.code, "E0404");
    assert!(err.message.contains("Result<T, E>"));
}

#[test]
fn accepts_debug_builtins() {
    let source = r#"package app.main

import std.debug

fn crash() -> void {
    debug.panic("boom")
}

fn main() -> void {
    debug.print("debug-")
    debug.println("ok")
    let trace: string = debug.backtrace()
}
"#;

    let program = parse_inline(source).unwrap();
    let crash = program
        .functions
        .iter()
        .find(|f| f.name == "crash")
        .unwrap();
    assert!(matches!(
        crash.body[0],
        Statement::Expr(ValueExpr::Panic { .. })
    ));
    let main = program.functions.iter().find(|f| f.name == "main").unwrap();
    assert!(matches!(
        main.body[0],
        Statement::Expr(ValueExpr::Call { ref name, .. }) if name == BUILTIN_EPRINT_EXPR
    ));
    assert!(matches!(
        main.body[1],
        Statement::Expr(ValueExpr::Call { ref name, .. }) if name == BUILTIN_EPRINTLN_EXPR
    ));
    assert!(matches!(
        main.body[2],
        Statement::Let {
            value_type: ValueType::String,
            initializer: ValueExpr::StringLiteral(ref value),
            ..
        } if value == "backtrace unavailable"
    ));
}

#[test]
fn accepts_specific_debug_backtrace_import() {
    let source = r#"package app.main

import std.debug.backtrace

fn main() -> void {
    let trace: string = backtrace()
}
"#;

    let program = parse_inline(source).unwrap();
    let main = program.functions.iter().find(|f| f.name == "main").unwrap();
    assert!(matches!(
        main.body[0],
        Statement::Let {
            initializer: ValueExpr::StringLiteral(ref value),
            ..
        } if value == "backtrace unavailable"
    ));
}

#[test]
fn rejects_debug_print_non_string_message() {
    let source = r#"package app.main

import std.debug

fn main() -> void {
    debug.println(1)
}
"#;

    let err = parse_inline(source).unwrap_err();
    assert_eq!(err.code, "E0404");
    assert!(err.message.contains("string message"));
}

#[test]
fn accepts_log_builtins() {
    let source = r#"package app.main

import std.log

fn main() -> void {
    log.debug("hidden")
    log.info("hello")
    log.warn("careful")
    log.error("bad")
    let enabled: bool = log.enabled("debug")
}
"#;

    let program = parse_inline(source).unwrap();
    let main = program.functions.iter().find(|f| f.name == "main").unwrap();
    assert_eq!(main.body.len(), 5);
    assert!(
        main.body[..4]
            .iter()
            .all(|stmt| matches!(stmt, Statement::Expr(ValueExpr::If { .. })))
    );
    assert!(matches!(
        main.body[4],
        Statement::Let {
            value_type: ValueType::Bool,
            initializer: ValueExpr::LogEnabled { .. },
            ..
        }
    ));
}

#[test]
fn accepts_specific_log_builtin_imports() {
    let source = r#"package app.main

import std.log.enabled
import std.log.info

fn main() -> void {
    info("hello")
    let enabled: bool = enabled("info")
}
"#;

    let program = parse_inline(source).unwrap();
    let main = program.functions.iter().find(|f| f.name == "main").unwrap();
    assert!(matches!(
        main.body[0],
        Statement::Expr(ValueExpr::If { .. })
    ));
    assert!(matches!(
        main.body[1],
        Statement::Let {
            value_type: ValueType::Bool,
            initializer: ValueExpr::LogEnabled { .. },
            ..
        }
    ));
}

#[test]
fn rejects_log_non_string_message() {
    let source = r#"package app.main

import std.log

fn main() -> void {
    log.info(1)
}
"#;

    let err = parse_inline(source).unwrap_err();
    assert_eq!(err.code, "E0404");
    assert!(err.message.contains("string message"));
}

#[test]
fn accepts_hash_builtins() {
    let source = r#"package app.main

import std.hash
import std.array.Array

fn main() -> void {
    let mut bytes: Array<u32> = Array.new<u32>()
    bytes.push(110 as u32)
    bytes.push(111 as u32)
    bytes.push(109 as u32)
    bytes.push(111 as u32)
    let direct: u64 = hash.string("nomo")
    let direct_bytes: u64 = hash.bytes(bytes)
    let state: HashState = hash.new()
    let written: HashState = hash.write_string(state, "nomo")
    let bytes_state: HashState = hash.write_bytes(state, bytes)
    let finished: u64 = hash.finish(written)
    let finished_bytes: u64 = hash.finish(bytes_state)
}
"#;

    let program = parse_inline(source).unwrap();
    assert!(program.structs.iter().any(|item| item.name == "HashState"));
    let main = program.functions.iter().find(|f| f.name == "main").unwrap();
    assert!(matches!(
        main.body[0],
        Statement::Let {
            value_type: ValueType::Array(ref item),
            ..
        } if item.as_ref() == &ValueType::U32
    ));
    assert!(matches!(
        main.body[5],
        Statement::Let {
            value_type: ValueType::U64,
            initializer: ValueExpr::HashString { .. },
            ..
        }
    ));
    assert!(matches!(
        main.body[6],
        Statement::Let {
            value_type: ValueType::U64,
            initializer: ValueExpr::HashBytes { .. },
            ..
        }
    ));
    assert!(matches!(
        main.body[7],
        Statement::Let {
            value_type: ValueType::Struct(ref name, ref args),
            initializer: ValueExpr::HashNew,
            ..
        } if name == "HashState" && args.is_empty()
    ));
    assert!(matches!(
        main.body[8],
        Statement::Let {
            value_type: ValueType::Struct(ref name, ref args),
            initializer: ValueExpr::HashWriteString { .. },
            ..
        } if name == "HashState" && args.is_empty()
    ));
    assert!(matches!(
        main.body[9],
        Statement::Let {
            value_type: ValueType::Struct(ref name, ref args),
            initializer: ValueExpr::HashWriteBytes { .. },
            ..
        } if name == "HashState" && args.is_empty()
    ));
    assert!(matches!(
        main.body[10],
        Statement::Let {
            value_type: ValueType::U64,
            initializer: ValueExpr::HashFinish { .. },
            ..
        }
    ));
}

#[test]
fn accepts_specific_hash_builtin_imports() {
    let source = r#"package app.main

import std.hash.HashState
import std.array.Array
import std.hash.bytes
import std.hash.finish
import std.hash.new
import std.hash.string
import std.hash.write_bytes
import std.hash.write_string

fn main() -> void {
    let mut data: Array<u32> = Array.new<u32>()
    data.push(1 as u32)
    let direct: u64 = string("nomo")
    let direct_bytes: u64 = bytes(data)
    let state: HashState = new()
    let written: HashState = write_string(state, "nomo")
    let written_bytes: HashState = write_bytes(state, data)
    let finished: u64 = finish(written)
}
"#;

    let program = parse_inline(source).unwrap();
    let main = program.functions.iter().find(|f| f.name == "main").unwrap();
    assert!(matches!(
        main.body[0],
        Statement::Let {
            value_type: ValueType::Array(ref item),
            ..
        } if item.as_ref() == &ValueType::U32
    ));
    assert!(matches!(
        main.body[2],
        Statement::Let {
            initializer: ValueExpr::HashString { .. },
            ..
        }
    ));
    assert!(matches!(
        main.body[3],
        Statement::Let {
            initializer: ValueExpr::HashBytes { .. },
            ..
        }
    ));
    assert!(matches!(
        main.body[4],
        Statement::Let {
            initializer: ValueExpr::HashNew,
            ..
        }
    ));
    assert!(matches!(
        main.body[5],
        Statement::Let {
            initializer: ValueExpr::HashWriteString { .. },
            ..
        }
    ));
    assert!(matches!(
        main.body[6],
        Statement::Let {
            initializer: ValueExpr::HashWriteBytes { .. },
            ..
        }
    ));
    assert!(matches!(
        main.body[7],
        Statement::Let {
            initializer: ValueExpr::HashFinish { .. },
            ..
        }
    ));
}

#[test]
fn rejects_hash_non_string_value() {
    let source = r#"package app.main

import std.hash

fn main() -> void {
    let value: u64 = hash.string(1)
}
"#;

    let err = parse_inline(source).unwrap_err();
    assert_eq!(err.code, "E0404");
    assert!(err.message.contains("string value"));
}

#[test]
fn rejects_hash_bytes_non_array_value() {
    let source = r#"package app.main

import std.hash

fn main() -> void {
    let value: u64 = hash.bytes("nomo")
}
"#;

    let err = parse_inline(source).unwrap_err();
    assert_eq!(err.code, "E0404");
    assert!(err.message.contains("Array<u32> value"));
}

#[test]
fn accepts_crypto_builtins() {
    let source = r#"package app.main

import std.crypto
import std.array.Array

fn main() -> void {
    let sha256: string = crypto.sha256("nomo")
    let sha512: string = crypto.sha512("nomo")
    let bytes: Array<u32> = crypto.random_bytes(4 as u64)
}
"#;

    let program = parse_inline(source).unwrap();
    let main = program.functions.iter().find(|f| f.name == "main").unwrap();
    assert!(matches!(
        main.body[0],
        Statement::Let {
            value_type: ValueType::String,
            initializer: ValueExpr::CryptoSha256 { .. },
            ..
        }
    ));
    assert!(matches!(
        main.body[1],
        Statement::Let {
            value_type: ValueType::String,
            initializer: ValueExpr::CryptoSha512 { .. },
            ..
        }
    ));
    assert!(matches!(
        &main.body[2],
        Statement::Let {
            value_type: ValueType::Array(element_type),
            initializer: ValueExpr::CryptoRandomBytes { .. },
            ..
        } if **element_type == ValueType::U32
    ));
}

#[test]
fn accepts_specific_crypto_builtin_imports() {
    let source = r#"package app.main

import std.crypto.sha256
import std.crypto.sha512
import std.crypto.random_bytes
import std.array.Array

fn main() -> void {
    let left: string = sha256("nomo")
    let right: string = sha512("nomo")
    let bytes: Array<u32> = random_bytes(4 as u64)
}
"#;

    let program = parse_inline(source).unwrap();
    let main = program.functions.iter().find(|f| f.name == "main").unwrap();
    assert!(matches!(
        main.body[0],
        Statement::Let {
            initializer: ValueExpr::CryptoSha256 { .. },
            ..
        }
    ));
    assert!(matches!(
        main.body[1],
        Statement::Let {
            initializer: ValueExpr::CryptoSha512 { .. },
            ..
        }
    ));
    assert!(matches!(
        main.body[2],
        Statement::Let {
            initializer: ValueExpr::CryptoRandomBytes { .. },
            ..
        }
    ));
}

#[test]
fn rejects_crypto_non_string_value() {
    let source = r#"package app.main

import std.crypto

fn main() -> void {
    let value: string = crypto.sha256(1)
}
"#;

    let err = parse_inline(source).unwrap_err();
    assert_eq!(err.code, "E0404");
    assert!(err.message.contains("string value"));
}

#[test]
fn rejects_crypto_random_bytes_non_u64_count() {
    let source = r#"package app.main

import std.crypto
import std.array.Array

fn main() -> void {
    let value: Array<u32> = crypto.random_bytes("four")
}
"#;

    let err = parse_inline(source).unwrap_err();
    assert_eq!(err.code, "E0404");
    assert!(err.message.contains("crypto.random_bytes"));
    assert_eq!(err.expected.as_deref(), Some("u64"));
    assert_eq!(err.found.as_deref(), Some("string"));
}
