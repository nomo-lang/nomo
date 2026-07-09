use super::*;

#[test]
fn rejects_unknown_std_import() {
    let source = r#"package app.main

import std.typo

fn main() -> void {
}
"#;

    let err = parse_inline(source).unwrap_err();
    assert_eq!(err.code, "E0301");
    assert!(err.message.contains("std.typo"));
}

#[test]
fn rejects_unknown_specific_std_import() {
    let source = r#"package app.main

import std.io.flush

fn main() -> void {
}
"#;

    let err = parse_inline(source).unwrap_err();
    assert_eq!(err.code, "E0301");
    assert!(err.message.contains("std.io.flush"));
}

#[test]
fn rejects_non_std_import_in_v0_1() {
    let source = r#"package app.main

import app.other

fn main() -> void {
}
"#;

    let err = parse_inline(source).unwrap_err();
    assert_eq!(err.code, "E0301");
    assert!(err.message.contains("app.other"));
}

#[test]
fn rejects_std_module_calls_without_imports() {
    for (source, symbol, import) in [
        (
            "package app.main\nfn main() -> void {\n    let count: u64 = string.len(\"hi\")\n}\n",
            "string.len",
            "std.string",
        ),
        (
            "package app.main\nfn main() -> void {\n    let result: Result<string, FsError> = fs.read_to_string(\"missing.txt\")\n}\n",
            "fs.read_to_string",
            "std.fs",
        ),
        (
            "package app.main\nfn main() -> void {\n    let value: Option<string> = env.get(\"HOME\")\n}\n",
            "env.get",
            "std.env",
        ),
        (
            "package app.main\nfn main() -> void {\n    let name: string = path.basename(\"/tmp/nomo.txt\")\n}\n",
            "path.basename",
            "std.path",
        ),
        (
            "package app.main\nfn main() -> void {\n    let value: i64 = math.abs(0 - 1)\n}\n",
            "math.abs",
            "std.math",
        ),
        (
            "package app.main\nfn main() -> void {\n    let items = Array.new<i32>()\n}\n",
            "Array.new",
            "std.array",
        ),
    ] {
        let err = parse_inline(source).unwrap_err();
        assert_eq!(err.code, "E0301");
        assert!(err.message.contains(symbol), "{:?}", err.message);
        assert!(err.message.contains(import), "{:?}", err.message);
    }
}

#[test]
fn rejects_standard_library_types_without_imports() {
    for (source, type_name, import) in [
        (
            "package app.main\nfn parse() -> Result<i32, string> {\n    return 1\n}\nfn main() -> void {\n}\n",
            "Result",
            "std.result",
        ),
        (
            "package app.main\nfn label(value: Option<i32>) -> void {\n}\nfn main() -> void {\n}\n",
            "Option",
            "std.option",
        ),
        (
            "package app.main\nstruct Bag {\n    items: Array<i32>\n}\nfn main() -> void {\n}\n",
            "Array",
            "std.array",
        ),
        (
            "package app.main\nfn report(error: FsError) -> void {\n}\nfn main() -> void {\n}\n",
            "FsError",
            "std.fs",
        ),
    ] {
        let err = parse_inline(source).unwrap_err();
        assert_eq!(err.code, "E0301", "{:?}", err);
        assert!(err.message.contains(type_name), "{:?}", err.message);
        assert!(err.message.contains(import), "{:?}", err.message);
    }
}

#[test]
fn accepts_string_variable_println() {
    let source = r#"package app.main

import std.io

fn main() -> void {
    let message: string = "Hello, Nomo"
    io.println(message)
}
"#;

    let program = parse_inline(source).unwrap();
    let main = program.functions.iter().find(|f| f.name == "main").unwrap();
    assert_eq!(
        main.body,
        vec![
            Statement::Let {
                name: "message".to_string(),
                value_type: ValueType::String,
                initializer: ValueExpr::StringLiteral("Hello, Nomo".to_string()),
            },
            Statement::Println(ValueExpr::Variable("message".to_string())),
        ]
    );
}

#[test]
fn accepts_omitted_void_return_type() {
    let source = r#"package app.main

import std.io

fn log() {
    io.println("hello")
}

fn main() {
    log()
}
"#;

    let program = parse_inline(source).unwrap();
    let log = program
        .functions
        .iter()
        .find(|function| function.name == "log")
        .unwrap();
    let main = program
        .functions
        .iter()
        .find(|function| function.name == "main")
        .unwrap();
    assert_eq!(log.return_type, ValueType::Void);
    assert_eq!(main.return_type, ValueType::Void);
}

#[test]
fn accepts_specific_println_import() {
    let source = r#"package app.main

import std.io.println

fn main() -> void {
    println("Hello, Nomo")
}
"#;

    let program = parse_inline(source).unwrap();
    assert_eq!(program.imports, vec!["std.io.println"]);
    let main = program.functions.iter().find(|f| f.name == "main").unwrap();
    assert_eq!(
        main.body,
        vec![Statement::Println(ValueExpr::StringLiteral(
            "Hello, Nomo".to_string()
        ))]
    );
}

#[test]
fn accepts_eprintln() {
    let source = r#"package app.main

import std.io

fn main() -> void {
    io.eprintln("error")
}
"#;

    let program = parse_inline(source).unwrap();
    let main = program.functions.iter().find(|f| f.name == "main").unwrap();
    assert_eq!(
        main.body,
        vec![Statement::Eprintln(ValueExpr::StringLiteral(
            "error".to_string()
        ))]
    );
}

#[test]
fn accepts_print_and_eprint() {
    let source = r#"package app.main

import std.io

fn main() -> void {
    io.print("out")
    io.eprint("err")
}
"#;

    let program = parse_inline(source).unwrap();
    let main = program.functions.iter().find(|f| f.name == "main").unwrap();
    assert_eq!(
        main.body,
        vec![
            Statement::Print(ValueExpr::StringLiteral("out".to_string())),
            Statement::Eprint(ValueExpr::StringLiteral("err".to_string())),
        ]
    );
}

#[test]
fn accepts_specific_print_and_eprint_imports() {
    let source = r#"package app.main

import std.io.print
import std.io.eprint

fn main() -> void {
    print("out")
    eprint("err")
}
"#;

    let program = parse_inline(source).unwrap();
    assert_eq!(program.imports, vec!["std.io.print", "std.io.eprint"]);
    let main = program.functions.iter().find(|f| f.name == "main").unwrap();
    assert_eq!(
        main.body,
        vec![
            Statement::Print(ValueExpr::StringLiteral("out".to_string())),
            Statement::Eprint(ValueExpr::StringLiteral("err".to_string())),
        ]
    );
}

#[test]
fn accepts_io_read_line_builtin() {
    let source = r#"package app.main

import std.io

fn main() -> void {
    let result: Result<string, IoError> = io.read_line()
}
"#;

    let program = parse_inline(source).unwrap();
    assert!(program.structs.iter().any(|item| item.name == "IoError"));
    let main = program.functions.iter().find(|f| f.name == "main").unwrap();
    assert!(matches!(
        main.body.as_slice(),
        [Statement::Let {
            name,
            value_type: ValueType::Enum(result_name, args),
            initializer: ValueExpr::IoReadLine,
        }] if name == "result"
            && result_name == "Result"
            && matches!(
                args.as_slice(),
                [
                    ValueType::String,
                    ValueType::Struct(error_name, error_args),
                ] if error_name == "IoError" && error_args.is_empty()
            )
    ));
}

#[test]
fn accepts_specific_io_read_line_import() {
    let source = r#"package app.main

import std.io.read_line

fn main() -> void {
    let result: Result<string, IoError> = read_line()
}
"#;

    let program = parse_inline(source).unwrap();
    assert_eq!(program.imports, vec!["std.io.read_line"]);
    let main = program.functions.iter().find(|f| f.name == "main").unwrap();
    assert!(matches!(
        main.body.as_slice(),
        [Statement::Let {
            initializer: ValueExpr::IoReadLine,
            ..
        }]
    ));
}

#[test]
fn accepts_string_len_and_concat_builtins() {
    let source = r#"package app.main

import std.io
import std.string

fn main() -> void {
    let message: string = string.concat("No", "mo")
    let count: u64 = string.len(message)
    io.println(message)
}
"#;

    let program = parse_inline(source).unwrap();
    let main = program.functions.iter().find(|f| f.name == "main").unwrap();
    assert!(matches!(
        main.body[0],
        Statement::Let {
            value_type: ValueType::String,
            initializer: ValueExpr::StringConcat { .. },
            ..
        }
    ));
    assert!(matches!(
        main.body[1],
        Statement::Let {
            value_type: ValueType::U64,
            initializer: ValueExpr::StringLen { .. },
            ..
        }
    ));
}

#[test]
fn accepts_specific_string_builtin_imports() {
    let source = r#"package app.main

import std.io
import std.string.concat
import std.string.len

fn main() -> void {
    let message: string = concat("No", "mo")
    let count: u64 = len(message)
    io.println(message)
}
"#;

    let program = parse_inline(source).unwrap();
    let main = program.functions.iter().find(|f| f.name == "main").unwrap();
    assert!(matches!(
        main.body[0],
        Statement::Let {
            value_type: ValueType::String,
            initializer: ValueExpr::StringConcat { .. },
            ..
        }
    ));
    assert!(matches!(
        main.body[1],
        Statement::Let {
            value_type: ValueType::U64,
            initializer: ValueExpr::StringLen { .. },
            ..
        }
    ));
}

#[test]
fn accepts_extended_string_builtins() {
    let source = r#"package app.main

import std.array
import std.string

fn main() -> void {
    let empty: bool = string.is_empty("")
    let contains: bool = string.contains("nomo", "om")
    let starts: bool = string.starts_with("nomo", "no")
    let ends: bool = string.ends_with("nomo", "mo")
    let parts: Array<string> = string.split("no,mo", ",")
    let trimmed: string = string.trim(" nomo ")
    let lower: string = string.to_lower("NOMO")
    let upper: string = string.to_upper("nomo")
}
"#;

    let program = parse_inline(source).unwrap();
    let main = program.functions.iter().find(|f| f.name == "main").unwrap();
    assert!(matches!(
        main.body[0],
        Statement::Let {
            value_type: ValueType::Bool,
            initializer: ValueExpr::StringIsEmpty { .. },
            ..
        }
    ));
    assert!(matches!(
        main.body[1],
        Statement::Let {
            value_type: ValueType::Bool,
            initializer: ValueExpr::StringContains { .. },
            ..
        }
    ));
    assert!(matches!(
        main.body[2],
        Statement::Let {
            value_type: ValueType::Bool,
            initializer: ValueExpr::StringStartsWith { .. },
            ..
        }
    ));
    assert!(matches!(
        main.body[3],
        Statement::Let {
            value_type: ValueType::Bool,
            initializer: ValueExpr::StringEndsWith { .. },
            ..
        }
    ));
    assert!(matches!(
        &main.body[4],
        Statement::Let {
            value_type: ValueType::Array(element),
            initializer: ValueExpr::StringSplit { .. },
            ..
        } if **element == ValueType::String
    ));
    assert!(matches!(
        main.body[5],
        Statement::Let {
            value_type: ValueType::String,
            initializer: ValueExpr::StringTrim { .. },
            ..
        }
    ));
    assert!(matches!(
        main.body[6],
        Statement::Let {
            value_type: ValueType::String,
            initializer: ValueExpr::StringToLower { .. },
            ..
        }
    ));
    assert!(matches!(
        main.body[7],
        Statement::Let {
            value_type: ValueType::String,
            initializer: ValueExpr::StringToUpper { .. },
            ..
        }
    ));
}

#[test]
fn accepts_specific_extended_string_builtin_imports() {
    let source = r#"package app.main

import std.array
import std.string.contains
import std.string.split
import std.string.to_upper

fn main() -> void {
    let found: bool = contains("nomo", "no")
    let parts: Array<string> = split("no/mo", "/")
    let loud: string = to_upper("nomo")
}
"#;

    let program = parse_inline(source).unwrap();
    let main = program.functions.iter().find(|f| f.name == "main").unwrap();
    assert!(matches!(
        main.body[0],
        Statement::Let {
            value_type: ValueType::Bool,
            initializer: ValueExpr::StringContains { .. },
            ..
        }
    ));
    assert!(matches!(
        &main.body[1],
        Statement::Let {
            value_type: ValueType::Array(element),
            initializer: ValueExpr::StringSplit { .. },
            ..
        } if **element == ValueType::String
    ));
    assert!(matches!(
        main.body[2],
        Statement::Let {
            value_type: ValueType::String,
            initializer: ValueExpr::StringToUpper { .. },
            ..
        }
    ));
}

#[test]
fn accepts_extended_string_value_methods() {
    let source = r#"package app.main

import std.array
import std.string

fn main() -> void {
    let text: string = " NoMo "
    let empty: bool = text.is_empty()
    let contains: bool = text.contains("No")
    let starts: bool = text.starts_with(" ")
    let ends: bool = text.ends_with(" ")
    let parts: Array<string> = text.split("o")
    let trimmed: string = text.trim()
    let lower: string = text.to_lower()
    let upper: string = text.to_upper()
}
"#;

    let program = parse_inline(source).unwrap();
    let main = program.functions.iter().find(|f| f.name == "main").unwrap();
    assert!(matches!(
        main.body[1],
        Statement::Let {
            value_type: ValueType::Bool,
            initializer: ValueExpr::StringIsEmpty { .. },
            ..
        }
    ));
    assert!(matches!(
        main.body[2],
        Statement::Let {
            value_type: ValueType::Bool,
            initializer: ValueExpr::StringContains { .. },
            ..
        }
    ));
    assert!(matches!(
        &main.body[5],
        Statement::Let {
            value_type: ValueType::Array(element),
            initializer: ValueExpr::StringSplit { .. },
            ..
        } if **element == ValueType::String
    ));
    assert!(matches!(
        main.body[8],
        Statement::Let {
            value_type: ValueType::String,
            initializer: ValueExpr::StringToUpper { .. },
            ..
        }
    ));
}

#[test]
fn accepts_path_builtins() {
    let source = r#"package app.main

import std.path

fn main() -> void {
    let joined: string = path.join("/tmp", "nomo.txt")
    let base: string = path.basename(joined)
    let dir: string = path.dirname(joined)
    let ext: string = path.extension("archive.tar.gz")
    let clean: string = path.normalize("/tmp//a/../b/./")
    let absolute: bool = path.is_absolute(clean)
}
"#;

    let program = parse_inline(source).unwrap();
    let main = program.functions.iter().find(|f| f.name == "main").unwrap();
    assert!(matches!(
        main.body[0],
        Statement::Let {
            value_type: ValueType::String,
            initializer: ValueExpr::PathJoin { .. },
            ..
        }
    ));
    assert!(matches!(
        main.body[1],
        Statement::Let {
            value_type: ValueType::String,
            initializer: ValueExpr::PathBasename { .. },
            ..
        }
    ));
    assert!(matches!(
        main.body[2],
        Statement::Let {
            value_type: ValueType::String,
            initializer: ValueExpr::PathDirname { .. },
            ..
        }
    ));
    assert!(matches!(
        main.body[3],
        Statement::Let {
            value_type: ValueType::String,
            initializer: ValueExpr::PathExtension { .. },
            ..
        }
    ));
    assert!(matches!(
        main.body[4],
        Statement::Let {
            value_type: ValueType::String,
            initializer: ValueExpr::PathNormalize { .. },
            ..
        }
    ));
    assert!(matches!(
        main.body[5],
        Statement::Let {
            value_type: ValueType::Bool,
            initializer: ValueExpr::PathIsAbsolute { .. },
            ..
        }
    ));
}

#[test]
fn accepts_specific_path_builtin_imports() {
    let source = r#"package app.main

import std.path.basename
import std.path.is_absolute

fn main() -> void {
    let name: string = basename("/tmp/nomo.txt")
    let absolute: bool = is_absolute("/tmp")
}
"#;

    let program = parse_inline(source).unwrap();
    let main = program.functions.iter().find(|f| f.name == "main").unwrap();
    assert!(matches!(
        main.body[0],
        Statement::Let {
            initializer: ValueExpr::PathBasename { .. },
            ..
        }
    ));
    assert!(matches!(
        main.body[1],
        Statement::Let {
            value_type: ValueType::Bool,
            initializer: ValueExpr::PathIsAbsolute { .. },
            ..
        }
    ));
}

#[test]
fn rejects_path_builtin_non_string_argument() {
    let source = r#"package app.main

import std.path

fn main() -> void {
    let name: string = path.basename(1)
}
"#;

    let err = parse_inline(source).unwrap_err();
    assert_eq!(err.code, "E0404");
    assert!(err.message.contains("path.basename"));
    assert!(err.message.contains("string"));
}

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

#[test]
fn accepts_collections_builtins() {
    let source = r#"package app.main

import std.collections

fn main() -> void {
    let map: StringMap = collections.map_new()
    let updated: StringMap = collections.map_set(map, "lang", "nomo")
    let found: Option<string> = collections.map_get(updated, "lang")
    let has_lang: bool = collections.map_contains(updated, "lang")
    let smaller: StringMap = collections.map_remove(updated, "lang")
    let set: StringSet = collections.set_new()
    let inserted: StringSet = collections.set_insert(set, "nomo")
    let has_nomo: bool = collections.set_contains(inserted, "nomo")
    let removed: StringSet = collections.set_remove(inserted, "nomo")
}
"#;

    let program = parse_inline(source).unwrap();
    assert!(program.structs.iter().any(|item| item.name == "StringMap"));
    assert!(program.structs.iter().any(|item| item.name == "StringSet"));
    let main = program.functions.iter().find(|f| f.name == "main").unwrap();
    assert!(matches!(
        main.body[0],
        Statement::Let {
            initializer: ValueExpr::CollectionsStringMapNew,
            ..
        }
    ));
    assert!(matches!(
        main.body[1],
        Statement::Let {
            initializer: ValueExpr::CollectionsStringMapSet { .. },
            ..
        }
    ));
    assert!(matches!(
        main.body[2],
        Statement::Let {
            value_type: ValueType::Enum(ref name, ref args),
            initializer: ValueExpr::CollectionsStringMapGet { .. },
            ..
        } if name == "Option" && args == &[ValueType::String]
    ));
    assert!(matches!(
        main.body[3],
        Statement::Let {
            value_type: ValueType::Bool,
            initializer: ValueExpr::CollectionsStringMapContains { .. },
            ..
        }
    ));
    assert!(matches!(
        main.body[4],
        Statement::Let {
            initializer: ValueExpr::CollectionsStringMapRemove { .. },
            ..
        }
    ));
    assert!(matches!(
        main.body[5],
        Statement::Let {
            initializer: ValueExpr::CollectionsStringSetNew,
            ..
        }
    ));
    assert!(matches!(
        main.body[6],
        Statement::Let {
            initializer: ValueExpr::CollectionsStringSetInsert { .. },
            ..
        }
    ));
    assert!(matches!(
        main.body[7],
        Statement::Let {
            value_type: ValueType::Bool,
            initializer: ValueExpr::CollectionsStringSetContains { .. },
            ..
        }
    ));
    assert!(matches!(
        main.body[8],
        Statement::Let {
            initializer: ValueExpr::CollectionsStringSetRemove { .. },
            ..
        }
    ));
}

#[test]
fn accepts_specific_collections_builtin_imports() {
    let source = r#"package app.main

import std.collections.StringMap
import std.collections.StringSet
import std.collections.map_get
import std.collections.map_new
import std.collections.map_set
import std.collections.set_insert
import std.collections.set_new

fn main() -> void {
    let map: StringMap = map_new()
    let updated: StringMap = map_set(map, "lang", "nomo")
    let found: Option<string> = map_get(updated, "lang")
    let set: StringSet = set_new()
    let inserted: StringSet = set_insert(set, "nomo")
}
"#;

    let program = parse_inline(source).unwrap();
    let main = program.functions.iter().find(|f| f.name == "main").unwrap();
    assert!(matches!(
        main.body[0],
        Statement::Let {
            initializer: ValueExpr::CollectionsStringMapNew,
            ..
        }
    ));
    assert!(matches!(
        main.body[1],
        Statement::Let {
            initializer: ValueExpr::CollectionsStringMapSet { .. },
            ..
        }
    ));
    assert!(matches!(
        main.body[2],
        Statement::Let {
            initializer: ValueExpr::CollectionsStringMapGet { .. },
            ..
        }
    ));
    assert!(matches!(
        main.body[3],
        Statement::Let {
            initializer: ValueExpr::CollectionsStringSetNew,
            ..
        }
    ));
    assert!(matches!(
        main.body[4],
        Statement::Let {
            initializer: ValueExpr::CollectionsStringSetInsert { .. },
            ..
        }
    ));
}

#[test]
fn accepts_char_builtins() {
    let source = r#"package app.main

import std.char

fn main() -> void {
    let digit: bool = char.is_digit('7')
    let alpha: bool = char.is_alpha('N')
    let space: bool = char.is_whitespace(' ')
    let text: string = char.to_string('語')
}
"#;

    let program = parse_inline(source).unwrap();
    let main = program.functions.iter().find(|f| f.name == "main").unwrap();
    assert!(matches!(
        main.body[0],
        Statement::Let {
            value_type: ValueType::Bool,
            initializer: ValueExpr::CharIsDigit { .. },
            ..
        }
    ));
    assert!(matches!(
        main.body[1],
        Statement::Let {
            value_type: ValueType::Bool,
            initializer: ValueExpr::CharIsAlpha { .. },
            ..
        }
    ));
    assert!(matches!(
        main.body[2],
        Statement::Let {
            value_type: ValueType::Bool,
            initializer: ValueExpr::CharIsWhitespace { .. },
            ..
        }
    ));
    assert!(matches!(
        main.body[3],
        Statement::Let {
            value_type: ValueType::String,
            initializer: ValueExpr::CharToString { .. },
            ..
        }
    ));
}

#[test]
fn accepts_specific_char_builtin_imports() {
    let source = r#"package app.main

import std.char.is_digit
import std.char.to_string

fn main() -> void {
    let digit: bool = is_digit('7')
    let text: string = to_string('N')
}
"#;

    let program = parse_inline(source).unwrap();
    let main = program.functions.iter().find(|f| f.name == "main").unwrap();
    assert!(matches!(
        main.body[0],
        Statement::Let {
            initializer: ValueExpr::CharIsDigit { .. },
            ..
        }
    ));
    assert!(matches!(
        main.body[1],
        Statement::Let {
            initializer: ValueExpr::CharToString { .. },
            ..
        }
    ));
}

#[test]
fn rejects_char_builtin_non_char_argument() {
    let source = r#"package app.main

import std.char

fn main() -> void {
    let value: bool = char.is_digit("7")
}
"#;

    let err = parse_inline(source).unwrap_err();
    assert_eq!(err.code, "E0404");
    assert_eq!(err.expected.as_deref(), Some("char"));
    assert_eq!(err.found.as_deref(), Some("string"));
}

#[test]
fn accepts_os_builtins() {
    let source = r#"package app.main

import std.os

fn main() -> void {
    let platform: string = os.platform()
    let arch: string = os.arch()
    let separator: string = os.path_separator()
    let ending: string = os.line_ending()
}
"#;

    let program = parse_inline(source).unwrap();
    let main = program.functions.iter().find(|f| f.name == "main").unwrap();
    assert!(matches!(
        main.body[0],
        Statement::Let {
            value_type: ValueType::String,
            initializer: ValueExpr::OsPlatform,
            ..
        }
    ));
    assert!(matches!(
        main.body[1],
        Statement::Let {
            value_type: ValueType::String,
            initializer: ValueExpr::OsArch,
            ..
        }
    ));
    assert!(matches!(
        main.body[2],
        Statement::Let {
            value_type: ValueType::String,
            initializer: ValueExpr::OsPathSeparator,
            ..
        }
    ));
    assert!(matches!(
        main.body[3],
        Statement::Let {
            value_type: ValueType::String,
            initializer: ValueExpr::OsLineEnding,
            ..
        }
    ));
}

#[test]
fn accepts_specific_os_builtin_imports() {
    let source = r#"package app.main

import std.os.platform
import std.os.path_separator

fn main() -> void {
    let platform: string = platform()
    let separator: string = path_separator()
}
"#;

    let program = parse_inline(source).unwrap();
    let main = program.functions.iter().find(|f| f.name == "main").unwrap();
    assert!(matches!(
        main.body[0],
        Statement::Let {
            initializer: ValueExpr::OsPlatform,
            ..
        }
    ));
    assert!(matches!(
        main.body[1],
        Statement::Let {
            initializer: ValueExpr::OsPathSeparator,
            ..
        }
    ));
}

#[test]
fn rejects_os_builtin_arguments() {
    let source = r#"package app.main

import std.os

fn main() -> void {
    let platform: string = os.platform("extra")
}
"#;

    let err = parse_inline(source).unwrap_err();
    assert_eq!(err.code, "E0407");
    assert!(err.message.contains("os.platform"));
}

#[test]
fn accepts_time_builtins() {
    let source = r#"package app.main

import std.time

fn main() -> void {
    let now: i64 = time.now_millis()
    let monotonic: i64 = time.monotonic_millis()
    let short: Duration = time.duration_millis(1500)
    let long: Duration = time.duration_seconds(2)
    let short_millis: i64 = time.duration_as_millis(short)
    let label: string = time.format_duration(short)
    time.sleep(time.duration_millis(0))
    time.sleep_millis(0)
}
"#;

    let program = parse_inline(source).unwrap();
    let main = program.functions.iter().find(|f| f.name == "main").unwrap();
    assert!(matches!(
        main.body[0],
        Statement::Let {
            value_type: ValueType::Int,
            initializer: ValueExpr::TimeNowMillis,
            ..
        }
    ));
    assert!(matches!(
        main.body[1],
        Statement::Let {
            value_type: ValueType::Int,
            initializer: ValueExpr::TimeMonotonicMillis,
            ..
        }
    ));
    assert!(matches!(
        main.body[2],
        Statement::Let {
            value_type: ValueType::Struct(ref name, ref args),
            initializer: ValueExpr::TimeDurationMillis { .. },
            ..
        } if name == "Duration" && args.is_empty()
    ));
    assert!(matches!(
        main.body[3],
        Statement::Let {
            value_type: ValueType::Struct(ref name, ref args),
            initializer: ValueExpr::TimeDurationSeconds { .. },
            ..
        } if name == "Duration" && args.is_empty()
    ));
    assert!(matches!(
        main.body[4],
        Statement::Let {
            value_type: ValueType::Int,
            initializer: ValueExpr::TimeDurationAsMillis { .. },
            ..
        }
    ));
    assert!(matches!(
        main.body[5],
        Statement::Let {
            value_type: ValueType::String,
            initializer: ValueExpr::TimeFormatDuration { .. },
            ..
        }
    ));
    assert!(matches!(
        main.body[6],
        Statement::Expr(ValueExpr::TimeSleep { .. })
    ));
    assert!(matches!(
        main.body[7],
        Statement::Expr(ValueExpr::TimeSleepMillis { .. })
    ));
}

#[test]
fn accepts_specific_time_builtin_imports() {
    let source = r#"package app.main

import std.time.now_millis
import std.time.duration_millis
import std.time.duration_as_millis
import std.time.sleep
import std.time.sleep_millis

fn main() -> void {
    let now: i64 = now_millis()
    let duration: Duration = duration_millis(5)
    let millis: i64 = duration_as_millis(duration)
    sleep(duration_millis(0))
    sleep_millis(0)
}
"#;

    let program = parse_inline(source).unwrap();
    let main = program.functions.iter().find(|f| f.name == "main").unwrap();
    assert!(matches!(
        main.body[0],
        Statement::Let {
            initializer: ValueExpr::TimeNowMillis,
            ..
        }
    ));
    assert!(matches!(
        main.body[1],
        Statement::Let {
            initializer: ValueExpr::TimeDurationMillis { .. },
            ..
        }
    ));
    assert!(matches!(
        main.body[2],
        Statement::Let {
            initializer: ValueExpr::TimeDurationAsMillis { .. },
            ..
        }
    ));
    assert!(matches!(
        main.body[3],
        Statement::Expr(ValueExpr::TimeSleep { .. })
    ));
    assert!(matches!(
        main.body[4],
        Statement::Expr(ValueExpr::TimeSleepMillis { .. })
    ));
}

#[test]
fn rejects_time_sleep_non_i64_argument() {
    let source = r#"package app.main

import std.time

fn main() -> void {
    time.sleep_millis("soon")
}
"#;

    let err = parse_inline(source).unwrap_err();
    assert_eq!(err.code, "E0404");
    assert!(err.message.contains("time.sleep_millis"));
    assert_eq!(err.expected.as_deref(), Some("i64"));
    assert_eq!(err.found.as_deref(), Some("string"));
}

#[test]
fn rejects_time_sleep_non_duration_argument() {
    let source = r#"package app.main

import std.time

fn main() -> void {
    time.sleep(1)
}
"#;

    let err = parse_inline(source).unwrap_err();
    assert_eq!(err.code, "E0404");
    assert!(err.message.contains("time.sleep"));
    assert_eq!(err.expected.as_deref(), Some("Duration"));
    assert_eq!(err.found.as_deref(), Some("i64"));
}

#[test]
fn accepts_num_builtins() {
    let source = r#"package app.main

import std.num

fn main() -> void {
    let integer: Result<i64, NumError> = num.parse_i64("42")
    let unsigned: Result<u64, NumError> = num.parse_u64("7")
    let decimal: Result<f64, NumError> = num.parse_f64("3.5")
    let text: string = num.to_string(42)
}
"#;

    let program = parse_inline(source).unwrap();
    assert!(
        program
            .structs
            .iter()
            .any(|struct_type| struct_type.name == "NumError")
    );
    let main = program.functions.iter().find(|f| f.name == "main").unwrap();
    assert!(matches!(
        main.body[0],
        Statement::Let {
            initializer: ValueExpr::NumParseI64 { .. },
            ..
        }
    ));
    assert!(matches!(
        main.body[1],
        Statement::Let {
            initializer: ValueExpr::NumParseU64 { .. },
            ..
        }
    ));
    assert!(matches!(
        main.body[2],
        Statement::Let {
            initializer: ValueExpr::NumParseF64 { .. },
            ..
        }
    ));
    assert!(matches!(
        main.body[3],
        Statement::Let {
            initializer: ValueExpr::NumToString { .. },
            ..
        }
    ));
}

#[test]
fn accepts_specific_num_parse_imports() {
    let source = r#"package app.main

import std.num.parse_i64
import std.num.parse_u64
import std.num.parse_f64

fn main() -> void {
    let integer: Result<i64, NumError> = parse_i64("42")
    let unsigned: Result<u64, NumError> = parse_u64("7")
    let decimal: Result<f64, NumError> = parse_f64("3.5")
}
"#;

    let program = parse_inline(source).unwrap();
    let main = program.functions.iter().find(|f| f.name == "main").unwrap();
    assert!(matches!(
        main.body[0],
        Statement::Let {
            initializer: ValueExpr::NumParseI64 { .. },
            ..
        }
    ));
    assert!(matches!(
        main.body[1],
        Statement::Let {
            initializer: ValueExpr::NumParseU64 { .. },
            ..
        }
    ));
    assert!(matches!(
        main.body[2],
        Statement::Let {
            initializer: ValueExpr::NumParseF64 { .. },
            ..
        }
    ));
}

#[test]
fn accepts_num_checked_and_wrapping_builtins() {
    let source = r#"package app.main

import std.num

fn main() -> void {
    let checked: Option<i64> = num.checked_add(9223372036854775807, 1)
    let wrapped: i64 = num.wrapping_add(9223372036854775807, 1)
    let checked32: Option<i32> = num.checked_mul(100000 as i32, 100000 as i32)
    let wrapped64: u64 = num.wrapping_sub(0 as u64, 1 as u64)
}
"#;

    let program = parse_inline(source).unwrap();
    assert!(program.enums.iter().any(|item| item.name == "Option"));
    let main = program.functions.iter().find(|f| f.name == "main").unwrap();
    assert!(matches!(
        &main.body[0],
        Statement::Let {
            value_type: ValueType::Enum(name, args),
            initializer:
                ValueExpr::NumBinary {
                    function: NumBinaryFunction::Checked,
                    op: BinaryOp::Add,
                    ..
                },
            ..
        } if name == "Option" && args == &vec![ValueType::Int]
    ));
    assert!(matches!(
        main.body[1],
        Statement::Let {
            value_type: ValueType::Int,
            initializer: ValueExpr::NumBinary {
                function: NumBinaryFunction::Wrapping,
                op: BinaryOp::Add,
                ..
            },
            ..
        }
    ));
    assert!(matches!(
        &main.body[2],
        Statement::Let {
            value_type: ValueType::Enum(name, args),
            initializer:
                ValueExpr::NumBinary {
                    function: NumBinaryFunction::Checked,
                    op: BinaryOp::Multiply,
                    ..
                },
            ..
        } if name == "Option" && args == &vec![ValueType::I32]
    ));
    assert!(matches!(
        main.body[3],
        Statement::Let {
            value_type: ValueType::U64,
            initializer: ValueExpr::NumBinary {
                function: NumBinaryFunction::Wrapping,
                op: BinaryOp::Subtract,
                ..
            },
            ..
        }
    ));
}

#[test]
fn accepts_specific_num_checked_and_wrapping_imports() {
    let source = r#"package app.main

import std.num.checked_add
import std.num.wrapping_mul

fn main() -> void {
    let checked: Option<u32> = checked_add(1 as u32, 2 as u32)
    let wrapped: u32 = wrapping_mul(3 as u32, 4 as u32)
}
"#;

    let program = parse_inline(source).unwrap();
    let main = program.functions.iter().find(|f| f.name == "main").unwrap();
    assert!(matches!(
        main.body[0],
        Statement::Let {
            initializer: ValueExpr::NumBinary {
                function: NumBinaryFunction::Checked,
                op: BinaryOp::Add,
                ..
            },
            ..
        }
    ));
    assert!(matches!(
        main.body[1],
        Statement::Let {
            initializer: ValueExpr::NumBinary {
                function: NumBinaryFunction::Wrapping,
                op: BinaryOp::Multiply,
                ..
            },
            ..
        }
    ));
}

#[test]
fn rejects_num_parse_non_string_argument() {
    let source = r#"package app.main

import std.num

fn main() -> void {
    let parsed: Result<i64, NumError> = num.parse_i64(42)
}
"#;

    let err = parse_inline(source).unwrap_err();
    assert_eq!(err.code, "E0404");
    assert!(err.message.contains("num.parse_i64"));
    assert_eq!(err.expected.as_deref(), Some("string"));
    assert_eq!(err.found.as_deref(), Some("i64"));
}

#[test]
fn rejects_num_to_string_non_numeric_argument() {
    let source = r#"package app.main

import std.num

fn main() -> void {
    let text: string = num.to_string(true)
}
"#;

    let err = parse_inline(source).unwrap_err();
    assert_eq!(err.code, "E0404");
    assert!(err.message.contains("num.to_string"));
}

#[test]
fn rejects_num_checked_mismatched_operands() {
    let source = r#"package app.main

import std.num

fn main() -> void {
    let value: Option<i64> = num.checked_add(1, true)
}
"#;

    let err = parse_inline(source).unwrap_err();
    assert_eq!(err.code, "E0404");
    assert!(err.message.contains("num.checked_add"));
}

#[test]
fn rejects_user_num_error_when_std_num_is_needed() {
    let source = r#"package app.main

import std.num

struct NumError {
    message: string
}

fn main() -> void {
    let parsed: Result<i64, NumError> = num.parse_i64("42")
}
"#;

    let err = parse_inline(source).unwrap_err();
    assert_eq!(err.code, "E0312");
    assert!(err.message.contains("NumError"));
    assert!(err.message.contains("standard library"));
}

#[test]
fn accepts_math_builtins() {
    let source = r#"package app.main

import std.math

fn main() -> void {
    let absolute: i64 = math.abs(0 - 7)
    let lower: i32 = math.min(3 as i32, 9 as i32)
    let upper: u64 = math.max(5 as u64, 2 as u64)
    let floored: f64 = math.floor(3.8)
    let ceiled: f64 = math.ceil(3.1)
    let rounded: f64 = math.round(3.5)
    let root: f64 = math.sqrt(9.0)
    let powered: f64 = math.pow(2.0, 3.0)
    let sine: f64 = math.sin(0.0)
    let cosine: f64 = math.cos(0.0)
}
"#;

    let program = parse_inline(source).unwrap();
    let main = program.functions.iter().find(|f| f.name == "main").unwrap();
    assert!(matches!(
        main.body[0],
        Statement::Let {
            value_type: ValueType::Int,
            initializer: ValueExpr::MathUnary {
                function: MathUnaryFunction::Abs,
                ..
            },
            ..
        }
    ));
    assert!(matches!(
        main.body[1],
        Statement::Let {
            value_type: ValueType::I32,
            initializer: ValueExpr::MathBinary {
                function: MathBinaryFunction::Min,
                ..
            },
            ..
        }
    ));
    assert!(matches!(
        main.body[7],
        Statement::Let {
            value_type: ValueType::Float,
            initializer: ValueExpr::MathBinary {
                function: MathBinaryFunction::Pow,
                ..
            },
            ..
        }
    ));
}

#[test]
fn accepts_specific_math_builtin_imports() {
    let source = r#"package app.main

import std.math.abs
import std.math.sqrt

fn main() -> void {
    let value: i64 = abs(0 - 2)
    let root: f64 = sqrt(16.0)
}
"#;

    let program = parse_inline(source).unwrap();
    let main = program.functions.iter().find(|f| f.name == "main").unwrap();
    assert!(matches!(
        main.body[0],
        Statement::Let {
            initializer: ValueExpr::MathUnary {
                function: MathUnaryFunction::Abs,
                ..
            },
            ..
        }
    ));
    assert!(matches!(
        main.body[1],
        Statement::Let {
            initializer: ValueExpr::MathUnary {
                function: MathUnaryFunction::Sqrt,
                ..
            },
            ..
        }
    ));
}

#[test]
fn rejects_math_builtin_type_mismatch() {
    let source = r#"package app.main

import std.math

fn main() -> void {
    let value: f64 = math.sqrt(9)
}
"#;

    let err = parse_inline(source).unwrap_err();
    assert_eq!(err.code, "E0404");
    assert_eq!(err.expected.as_deref(), Some("f64"));
    assert_eq!(err.found.as_deref(), Some("i64"));
}

#[test]
fn rejects_math_min_mixed_numeric_types() {
    let source = r#"package app.main

import std.math

fn main() -> void {
    let value: i64 = math.min(1, 2 as i32)
}
"#;

    let err = parse_inline(source).unwrap_err();
    assert_eq!(err.code, "E0404");
    assert!(err.message.contains("math.min"));
}

#[test]
fn accepts_string_value_methods() {
    let source = r#"package app.main

import std.io
import std.string

fn main() -> void {
    let prefix: string = "string "
    let message: string = prefix.concat("methods ok")
    let count: u64 = message.len()
    io.println(message)
}
"#;

    let program = parse_inline(source).unwrap();
    let main = program.functions.iter().find(|f| f.name == "main").unwrap();
    assert!(matches!(
        main.body[1],
        Statement::Let {
            value_type: ValueType::String,
            initializer: ValueExpr::StringConcat { .. },
            ..
        }
    ));
    assert!(matches!(
        main.body[2],
        Statement::Let {
            value_type: ValueType::U64,
            initializer: ValueExpr::StringLen { .. },
            ..
        }
    ));
}

#[test]
fn rejects_string_value_method_without_import() {
    let source = r#"package app.main

fn main() -> void {
    let message: string = "hello"
    let count: u64 = message.len()
}
"#;

    let err = parse_inline(source).unwrap_err();
    assert_eq!(err.code, "E0301");
    assert!(err.message.contains("std.string"));
}

#[test]
fn rejects_string_concat_method_non_string_argument() {
    let source = r#"package app.main

import std.string

fn main() -> void {
    let prefix: string = "nomo"
    let message: string = prefix.concat(1)
}
"#;

    let err = parse_inline(source).unwrap_err();
    assert_eq!(err.code, "E0404");
    assert_eq!(err.expected.as_deref(), Some("string"));
    assert_eq!(err.found.as_deref(), Some("i64"));
}

#[test]
fn accepts_fs_read_and_write_builtins() {
    let source = r#"package app.main

import std.fs
import std.io
import std.array.Array

fn load(path: string) -> Result<string, FsError> {
    let text: string = fs.read_to_string(path)?
    return Result.Ok(text)
}

fn load_bytes(path: string) -> Result<Array<u32>, FsError> {
    let bytes: Array<u32> = fs.read_bytes(path)?
    return Result.Ok(bytes)
}

fn save(path: string, content: string) -> Result<void, FsError> {
    return fs.write_string(path, content)
}

fn save_bytes(path: string, bytes: Array<u32>) -> Result<void, FsError> {
    return fs.write_bytes(path, bytes)
}

fn main() -> void {
    let write_result: Result<void, FsError> = save("/tmp/nomo-fs-test.txt", "hello")
    let read_result: Result<string, FsError> = load("/tmp/nomo-fs-test.txt")
    let byte_read_result: Result<Array<u32>, FsError> = load_bytes("/tmp/nomo-fs-test.txt")
    io.println("done")
}
"#;

    let program = parse_inline(source).unwrap();
    assert!(program.structs.iter().any(|item| item.name == "FsError"));
    assert!(program.enums.iter().any(|item| item.name == "Result"));
    let load = program.functions.iter().find(|f| f.name == "load").unwrap();
    assert_eq!(
        load.return_type,
        ValueType::Enum(
            "Result".to_string(),
            vec![
                ValueType::String,
                ValueType::Struct("FsError".to_string(), Vec::new()),
            ],
        )
    );
    assert!(matches!(
        load.body[0],
        Statement::QuestionLet {
            result_expr: ValueExpr::FsReadToString { .. },
            ..
        }
    ));
    let load_bytes = program
        .functions
        .iter()
        .find(|f| f.name == "load_bytes")
        .unwrap();
    assert!(matches!(
        load_bytes.body[0],
        Statement::QuestionLet {
            result_expr: ValueExpr::FsReadBytes { .. },
            ..
        }
    ));
    let save = program.functions.iter().find(|f| f.name == "save").unwrap();
    assert!(matches!(
        save.body[0],
        Statement::Return(Some(ValueExpr::FsWriteString { .. }))
    ));
    let save_bytes = program
        .functions
        .iter()
        .find(|f| f.name == "save_bytes")
        .unwrap();
    assert!(matches!(
        save_bytes.body[0],
        Statement::Return(Some(ValueExpr::FsWriteBytes { .. }))
    ));
}

#[test]
fn accepts_fs_open_and_file_close_defer() {
    let source = r#"package app.main

import std.fs
import std.io

fn close_and_label(file: File) -> string {
    defer file.close()
    return "ok"
}

fn main() -> void {
    let result: Result<File, FsError> = fs.open("/tmp/nomo-file.txt")
    let message: string = match result {
        Result.Ok(file) => close_and_label(file)
        Result.Err(err) => err.message
    }
    io.println(message)
}
"#;

    let program = parse_inline(source).unwrap();
    assert!(program.structs.iter().any(|item| item.name == "File"));
    let close_and_label = program
        .functions
        .iter()
        .find(|f| f.name == "close_and_label")
        .unwrap();
    assert_eq!(
        close_and_label.params[0].value_type,
        ValueType::Struct("File".to_string(), Vec::new())
    );
    assert!(matches!(
        close_and_label.body[0],
        Statement::Defer {
            call: DeferredCall::Expr(ValueExpr::FileClose { .. })
        }
    ));
    let main = program.functions.iter().find(|f| f.name == "main").unwrap();
    assert!(matches!(
        main.body[0],
        Statement::Let {
            value_type: ValueType::Enum(ref name, ref args),
            initializer: ValueExpr::FsOpen { .. },
            ..
        } if name == "Result"
            && args == &vec![
                ValueType::Struct("File".to_string(), Vec::new()),
                ValueType::Struct("FsError".to_string(), Vec::new()),
            ]
    ));
}

#[test]
fn accepts_specific_fs_builtin_imports() {
    let source = r#"package app.main

import std.fs.read_to_string
import std.fs.write_string
import std.fs.read_bytes
import std.fs.write_bytes
import std.io
import std.array.Array

fn load(path: string) -> Result<string, FsError> {
    let text: string = read_to_string(path)?
    return Result.Ok(text)
}

fn load_bytes(path: string) -> Result<Array<u32>, FsError> {
    let bytes: Array<u32> = read_bytes(path)?
    return Result.Ok(bytes)
}

fn save(path: string, content: string) -> Result<void, FsError> {
    return write_string(path, content)
}

fn save_bytes(path: string, bytes: Array<u32>) -> Result<void, FsError> {
    return write_bytes(path, bytes)
}

fn main() -> void {
    let write_result: Result<void, FsError> = save("/tmp/nomo-fs-test.txt", "hello")
    let read_result: Result<string, FsError> = load("/tmp/nomo-fs-test.txt")
    let byte_read_result: Result<Array<u32>, FsError> = load_bytes("/tmp/nomo-fs-test.txt")
    io.println("done")
}
"#;

    let program = parse_inline(source).unwrap();
    assert!(program.structs.iter().any(|item| item.name == "FsError"));
    assert!(program.enums.iter().any(|item| item.name == "Result"));
    let load = program.functions.iter().find(|f| f.name == "load").unwrap();
    assert!(matches!(
        load.body[0],
        Statement::QuestionLet {
            result_expr: ValueExpr::FsReadToString { .. },
            ..
        }
    ));
    let load_bytes = program
        .functions
        .iter()
        .find(|f| f.name == "load_bytes")
        .unwrap();
    assert!(matches!(
        load_bytes.body[0],
        Statement::QuestionLet {
            result_expr: ValueExpr::FsReadBytes { .. },
            ..
        }
    ));
    let save = program.functions.iter().find(|f| f.name == "save").unwrap();
    assert!(matches!(
        save.body[0],
        Statement::Return(Some(ValueExpr::FsWriteString { .. }))
    ));
    let save_bytes = program
        .functions
        .iter()
        .find(|f| f.name == "save_bytes")
        .unwrap();
    assert!(matches!(
        save_bytes.body[0],
        Statement::Return(Some(ValueExpr::FsWriteBytes { .. }))
    ));
}

#[test]
fn accepts_file_read_and_write_string_methods() {
    let source = r#"package app.main

import std.fs

fn rewrite(file: File) -> Result<string, FsError> {
    file.write_string("file ok")?
    let text: string = file.read_to_string()?
    file.close()
    return Ok(text)
}

fn main() -> void {
}
"#;

    let program = parse_inline(source).unwrap();
    let rewrite = program
        .functions
        .iter()
        .find(|f| f.name == "rewrite")
        .unwrap();
    assert!(matches!(
        rewrite.body[0],
        Statement::QuestionLet {
            result_expr: ValueExpr::FileWriteString { .. },
            ..
        }
    ));
    assert!(rewrite.body.iter().any(|stmt| matches!(
        stmt,
        Statement::QuestionLet {
            result_expr: ValueExpr::FileReadToString { .. },
            ..
        }
    )));
    assert!(
        rewrite
            .body
            .iter()
            .any(|stmt| matches!(stmt, Statement::Expr(ValueExpr::FileClose { .. })))
    );
}

#[test]
fn accepts_net_tcp_stream_builtins() {
    let source = r#"package app.main

import std.net

fn request(host: string, port: i64) -> Result<string, NetError> {
    let stream: TcpStream = net.connect(host, port)?
    stream.write_string("ping")?
    let text: string = stream.read_to_string()?
    stream.close()
    return Ok(text)
}

fn main() -> void {
}
"#;

    let program = parse_inline(source).unwrap();
    assert!(program.structs.iter().any(|item| item.name == "NetError"));
    assert!(program.structs.iter().any(|item| item.name == "TcpStream"));
    let request = program
        .functions
        .iter()
        .find(|f| f.name == "request")
        .unwrap();
    assert!(matches!(
        request.body[0],
        Statement::QuestionLet {
            value_type: ValueType::Struct(ref name, ref args),
            result_expr: ValueExpr::NetConnect { .. },
            ..
        } if name == "TcpStream" && args.is_empty()
    ));
    assert!(request.body.iter().any(|stmt| matches!(
        stmt,
        Statement::QuestionLet {
            value_type: ValueType::Void,
            result_expr: ValueExpr::TcpStreamWriteString { .. },
            ..
        }
    )));
    assert!(request.body.iter().any(|stmt| matches!(
        stmt,
        Statement::QuestionLet {
            value_type: ValueType::String,
            result_expr: ValueExpr::TcpStreamReadToString { .. },
            ..
        }
    )));
    assert!(
        request
            .body
            .iter()
            .any(|stmt| matches!(stmt, Statement::Expr(ValueExpr::TcpStreamClose { .. })))
    );
}

#[test]
fn accepts_specific_net_connect_import() {
    let source = r#"package app.main

import std.net.connect
import std.result

fn request(host: string, port: i64) -> Result<TcpStream, NetError> {
    return connect(host, port)
}

fn main() -> void {
}
"#;

    let program = parse_inline(source).unwrap();
    let request = program
        .functions
        .iter()
        .find(|f| f.name == "request")
        .unwrap();
    assert!(matches!(
        request.body[0],
        Statement::Return(Some(ValueExpr::NetConnect { .. }))
    ));
}

#[test]
fn accepts_net_tcp_listener_builtins() {
    let source = r#"package app.main

import std.net

fn serve(host: string, port: i64) -> Result<void, NetError> {
    let listener: TcpListener = net.listen(host, port)?
    let stream: TcpStream = listener.accept()?
    stream.write_string("pong")?
    stream.close()
    listener.close()
    return Ok(void)
}

fn main() -> void {
}
"#;

    let program = parse_inline(source).unwrap();
    assert!(
        program
            .structs
            .iter()
            .any(|item| item.name == "TcpListener")
    );
    let serve = program
        .functions
        .iter()
        .find(|f| f.name == "serve")
        .unwrap();
    assert!(matches!(
        serve.body[0],
        Statement::QuestionLet {
            value_type: ValueType::Struct(ref name, ref args),
            result_expr: ValueExpr::NetListen { .. },
            ..
        } if name == "TcpListener" && args.is_empty()
    ));
    assert!(serve.body.iter().any(|stmt| matches!(
        stmt,
        Statement::QuestionLet {
            value_type: ValueType::Struct(name, args),
            result_expr: ValueExpr::TcpListenerAccept { .. },
            ..
        } if name == "TcpStream" && args.is_empty()
    )));
    assert!(
        serve
            .body
            .iter()
            .any(|stmt| matches!(stmt, Statement::Expr(ValueExpr::TcpListenerClose { .. })))
    );
}

#[test]
fn accepts_specific_net_listen_import() {
    let source = r#"package app.main

import std.net.listen
import std.result

fn open(host: string, port: i64) -> Result<TcpListener, NetError> {
    return listen(host, port)
}

fn main() -> void {
}
"#;

    let program = parse_inline(source).unwrap();
    let open = program.functions.iter().find(|f| f.name == "open").unwrap();
    assert!(matches!(
        open.body[0],
        Statement::Return(Some(ValueExpr::NetListen { .. }))
    ));
}

#[test]
fn accepts_net_udp_socket_builtins() {
    let source = r#"package app.main

import std.net

fn serve(host: string, port: i64) -> Result<void, NetError> {
    let socket: UdpSocket = net.udp_bind(host, port)?
    let packet: UdpDatagram = socket.recv_from_string(1024)?
    socket.send_to_string(packet.data, packet.host, packet.port)?
    socket.close()
    return Ok(void)
}

fn main() -> void {
}
"#;

    let program = parse_inline(source).unwrap();
    assert!(program.structs.iter().any(|item| item.name == "UdpSocket"));
    assert!(
        program
            .structs
            .iter()
            .any(|item| item.name == "UdpDatagram")
    );
    let serve = program
        .functions
        .iter()
        .find(|f| f.name == "serve")
        .unwrap();
    assert!(matches!(
        serve.body[0],
        Statement::QuestionLet {
            value_type: ValueType::Struct(ref name, ref args),
            result_expr: ValueExpr::NetUdpBind { .. },
            ..
        } if name == "UdpSocket" && args.is_empty()
    ));
    assert!(serve.body.iter().any(|stmt| matches!(
        stmt,
        Statement::QuestionLet {
            value_type: ValueType::Struct(name, args),
            result_expr: ValueExpr::UdpSocketRecvFromString { .. },
            ..
        } if name == "UdpDatagram" && args.is_empty()
    )));
    assert!(serve.body.iter().any(|stmt| matches!(
        stmt,
        Statement::QuestionLet {
            value_type: ValueType::Void,
            result_expr: ValueExpr::UdpSocketSendToString { .. },
            ..
        }
    )));
    assert!(
        serve
            .body
            .iter()
            .any(|stmt| matches!(stmt, Statement::Expr(ValueExpr::UdpSocketClose { .. })))
    );
}

#[test]
fn accepts_specific_net_udp_bind_import() {
    let source = r#"package app.main

import std.net.udp_bind
import std.result

fn open(host: string, port: i64) -> Result<UdpSocket, NetError> {
    return udp_bind(host, port)
}

fn main() -> void {
}
"#;

    let program = parse_inline(source).unwrap();
    let open = program.functions.iter().find(|f| f.name == "open").unwrap();
    assert!(matches!(
        open.body[0],
        Statement::Return(Some(ValueExpr::NetUdpBind { .. }))
    ));
}

#[test]
fn accepts_fs_directory_builtins() {
    let source = r#"package app.main

import std.fs
import std.array
import std.io

fn prepare(path: string) -> Result<Array<string>, FsError> {
    let present: bool = fs.exists(path)
    let metadata: FileMetadata = fs.metadata(path)?
    fs.create_dir(path)?
    let entries: Array<string> = fs.read_dir(path)?
    fs.remove_dir(path)?
    return Ok(entries)
}

fn main() -> void {
    let entries: Result<Array<string>, FsError> = prepare("/tmp/nomo-dir")
    io.println("done")
}
"#;

    let program = parse_inline(source).unwrap();
    assert!(program.structs.iter().any(|item| item.name == "FsError"));
    assert!(
        program
            .structs
            .iter()
            .any(|item| item.name == "FileMetadata")
    );
    assert!(program.enums.iter().any(|item| item.name == "Result"));
    let prepare = program
        .functions
        .iter()
        .find(|f| f.name == "prepare")
        .unwrap();
    assert_eq!(
        prepare.return_type,
        ValueType::Enum(
            "Result".to_string(),
            vec![
                ValueType::Array(Box::new(ValueType::String)),
                ValueType::Struct("FsError".to_string(), Vec::new()),
            ],
        )
    );
    assert!(matches!(
        prepare.body[1],
        Statement::QuestionLet {
            result_expr: ValueExpr::FsMetadata { .. },
            ..
        }
    ));
    assert!(matches!(
        prepare.body[0],
        Statement::Let {
            initializer: ValueExpr::FsExists { .. },
            ..
        }
    ));
}

#[test]
fn accepts_specific_fs_directory_imports() {
    let source = r#"package app.main

import std.fs.exists
import std.fs.metadata
import std.fs.create_dir
import std.fs.remove_dir
import std.fs.read_dir
import std.array

fn prepare(path: string) -> Result<Array<string>, FsError> {
    let present: bool = exists(path)
    let metadata: FileMetadata = metadata(path)?
    create_dir(path)?
    let entries: Array<string> = read_dir(path)?
    remove_dir(path)?
    return Ok(entries)
}

fn main() -> void {
}
"#;

    let program = parse_inline(source).unwrap();
    let prepare = program
        .functions
        .iter()
        .find(|f| f.name == "prepare")
        .unwrap();
    assert!(matches!(
        prepare.body[0],
        Statement::Let {
            initializer: ValueExpr::FsExists { .. },
            ..
        }
    ));
    assert!(matches!(
        prepare.body[1],
        Statement::QuestionLet {
            result_expr: ValueExpr::FsMetadata { .. },
            ..
        }
    ));
}

#[test]
fn accepts_env_get_builtin() {
    let source = r#"package app.main

import std.env
import std.io

fn main() -> void {
    let value: Option<string> = env.get("NOMO_TEST_ENV")
    let message: string = match value {
        Option.Some(text) => text
        Option.None => "missing"
    }
    io.println(message)
}
"#;

    let program = parse_inline(source).unwrap();
    assert!(program.enums.iter().any(|item| item.name == "Option"));
    let main = program.functions.iter().find(|f| f.name == "main").unwrap();
    assert!(matches!(
        main.body[0],
        Statement::Let {
            value_type: ValueType::Enum(ref name, ref args),
            initializer: ValueExpr::EnvGet { .. },
            ..
        } if name == "Option" && args == &vec![ValueType::String]
    ));
}

#[test]
fn accepts_env_args_builtin() {
    let source = r#"package app.main

import std.env
import std.io
import std.array

fn main() -> void {
    let args: Array<string> = env.args()
    let first: Option<string> = args.get(1)
    let message: string = match first {
        Option.Some(text) => text
        Option.None => "missing"
    }
    io.println(message)
}
"#;

    let program = parse_inline(source).unwrap();
    assert!(program.enums.iter().any(|item| item.name == "Option"));
    let main = program.functions.iter().find(|f| f.name == "main").unwrap();
    assert!(matches!(
        main.body[0],
        Statement::Let {
            value_type: ValueType::Array(ref element),
            initializer: ValueExpr::EnvArgs,
            ..
        } if element.as_ref() == &ValueType::String
    ));
    assert!(matches!(
        main.body[1],
        Statement::Let {
            value_type: ValueType::Enum(ref name, ref args),
            initializer: ValueExpr::ArrayGet {
                element_type: ValueType::String,
                ..
            },
            ..
        } if name == "Option" && args == &vec![ValueType::String]
    ));
}

#[test]
fn accepts_extended_env_builtins() {
    let source = r#"package app.main

import std.env
import std.io

fn main() -> void {
    env.set("NOMO_TEST_ENV", "value")
    let cwd: string = env.cwd()
    let home: Option<string> = env.home_dir()
    let temp: string = env.temp_dir()
    io.println(cwd)
}
"#;

    let program = parse_inline(source).unwrap();
    assert!(program.enums.iter().any(|item| item.name == "Option"));
    let main = program.functions.iter().find(|f| f.name == "main").unwrap();
    assert!(matches!(
        main.body[0],
        Statement::Expr(ValueExpr::EnvSet { .. })
    ));
    assert!(matches!(
        main.body[1],
        Statement::Let {
            value_type: ValueType::String,
            initializer: ValueExpr::EnvCwd,
            ..
        }
    ));
    assert!(matches!(
        main.body[2],
        Statement::Let {
            value_type: ValueType::Enum(ref name, ref args),
            initializer: ValueExpr::EnvHomeDir,
            ..
        } if name == "Option" && args == &vec![ValueType::String]
    ));
    assert!(matches!(
        main.body[3],
        Statement::Let {
            value_type: ValueType::String,
            initializer: ValueExpr::EnvTempDir,
            ..
        }
    ));
}

#[test]
fn accepts_specific_env_builtin_imports() {
    let source = r#"package app.main

import std.env.args
import std.env.cwd
import std.env.get
import std.env.home_dir
import std.env.set
import std.env.temp_dir
import std.io
import std.array

fn main() -> void {
    set("NOMO_TEST_ENV", "value")
    let values: Array<string> = args()
    let home: Option<string> = get("HOME")
    let cwd_path: string = cwd()
    let maybe_home: Option<string> = home_dir()
    let temp_path: string = temp_dir()
    let message: string = match home {
        Option.Some(text) => text
        Option.None => "missing"
    }
    io.println(message)
}
"#;

    let program = parse_inline(source).unwrap();
    assert!(program.enums.iter().any(|item| item.name == "Option"));
    let main = program.functions.iter().find(|f| f.name == "main").unwrap();
    assert!(matches!(
        main.body[0],
        Statement::Expr(ValueExpr::EnvSet { .. })
    ));
    assert!(matches!(
        main.body[1],
        Statement::Let {
            value_type: ValueType::Array(ref element),
            initializer: ValueExpr::EnvArgs,
            ..
        } if element.as_ref() == &ValueType::String
    ));
    assert!(matches!(
        main.body[2],
        Statement::Let {
            value_type: ValueType::Enum(ref name, ref args),
            initializer: ValueExpr::EnvGet { .. },
            ..
        } if name == "Option" && args == &vec![ValueType::String]
    ));
    assert!(matches!(
        main.body[3],
        Statement::Let {
            value_type: ValueType::String,
            initializer: ValueExpr::EnvCwd,
            ..
        }
    ));
    assert!(matches!(
        main.body[4],
        Statement::Let {
            value_type: ValueType::Enum(ref name, ref args),
            initializer: ValueExpr::EnvHomeDir,
            ..
        } if name == "Option" && args == &vec![ValueType::String]
    ));
    assert!(matches!(
        main.body[5],
        Statement::Let {
            value_type: ValueType::String,
            initializer: ValueExpr::EnvTempDir,
            ..
        }
    ));
}

#[test]
fn accepts_imported_result_lang_item() {
    let source = r#"package app.main

import std.result

fn parse() -> Result<i64, string> {
    return Result.Ok(41)
}

fn main() -> void {
    let value: Result<i64, string> = parse()
}
"#;

    let program = parse_inline(source).unwrap();
    assert!(program.enums.iter().any(|item| item.name == "Result"));
    let parse = program
        .functions
        .iter()
        .find(|f| f.name == "parse")
        .unwrap();
    assert_eq!(
        parse.return_type,
        ValueType::Enum(
            "Result".to_string(),
            vec![ValueType::Int, ValueType::String],
        )
    );
    assert!(matches!(
        parse.body[0],
        Statement::Return(Some(ValueExpr::EnumVariant {
            ref enum_name,
            ref variant,
            ..
        })) if enum_name == "Result" && variant == "Ok"
    ));
}

#[test]
fn accepts_imported_option_lang_item() {
    let source = r#"package app.main

import std.option
import std.io

fn label(value: Option<string>) -> string {
    return match value {
        Option.Some(text) => text
        Option.None => "missing"
    }
}

fn main() -> void {
    let value: Option<string> = Option.None
    let text: string = label(value)
    io.println(text)
}
"#;

    let program = parse_inline(source).unwrap();
    assert!(program.enums.iter().any(|item| item.name == "Option"));
    let main = program.functions.iter().find(|f| f.name == "main").unwrap();
    assert!(matches!(
        main.body[0],
        Statement::Let {
            value_type: ValueType::Enum(ref name, ref args),
            initializer: ValueExpr::EnumVariant {
                ref enum_name,
                ref variant,
                ..
            },
            ..
        } if name == "Option"
            && args == &vec![ValueType::String]
            && enum_name == "Option"
            && variant == "None"
    ));
}
