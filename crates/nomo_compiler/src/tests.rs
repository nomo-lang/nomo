use super::*;

fn parse_inline(source: &str) -> Result<Program, Diagnostic> {
    let path = Path::new("main.nomo");
    let tokens = lexer::lex(path, source)?;
    let ast = parser::parse(path, &tokens)?;
    lower_program(path, ast, &[], None, EntryMode::MainFunctionRequired)
}

#[test]
fn parses_v0_1_hello() {
    let source = r#"package app.main

import std.io

fn main() -> void {
    io.println("Hello, Nomo")
}
"#;

    let program = parse_inline(source).unwrap();
    assert_eq!(program.package, "app.main");
    assert_eq!(program.imports, vec!["std.io"]);
    let main = program.functions.iter().find(|f| f.name == "main").unwrap();
    assert_eq!(
        main.body,
        vec![Statement::Println(ValueExpr::StringLiteral(
            "Hello, Nomo".to_string()
        ))]
    );
}

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

#[test]
fn accepts_string_array_builtins() {
    let source = r#"package app.main

import std.array
import std.io

fn main() -> void {
    let mut items: Array<string> = Array.new<string>()
    items.push("first")
    items.push("second")
    items.set(0, "updated")
    let size: u64 = items.len()
    let first: Option<string> = items.get(0)
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
            initializer: ValueExpr::ArrayNew {
                element_type: ValueType::String,
            },
            ..
        } if element.as_ref() == &ValueType::String
    ));
    assert!(matches!(
        main.body[1],
        Statement::Assign {
            ref name,
            value: ValueExpr::ArrayPush { .. },
        } if name == "items"
    ));
    assert!(matches!(
        main.body[3],
        Statement::Assign {
            ref name,
            value: ValueExpr::ArraySet { .. },
        } if name == "items"
    ));
    assert!(matches!(
        main.body[4],
        Statement::Let {
            value_type: ValueType::U64,
            initializer: ValueExpr::ArrayLen { .. },
            ..
        }
    ));
    assert!(matches!(
        main.body[5],
        Statement::Let {
            value_type: ValueType::Enum(ref name, ref args),
            initializer: ValueExpr::ArrayGet { .. },
            ..
        } if name == "Option" && args == &vec![ValueType::String]
    ));
}

#[test]
fn accepts_i32_array_builtins() {
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
        Option.Some(value) => if value == 7 {
            "array ok"
        } else {
            "wrong"
        }
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
            initializer: ValueExpr::ArrayNew {
                element_type: ValueType::I32,
            },
            ..
        } if element.as_ref() == &ValueType::I32
    ));
    assert!(matches!(
        main.body[1],
        Statement::Assign {
            ref name,
            value: ValueExpr::ArrayPush {
                element_type: ValueType::I32,
                ..
            },
        } if name == "items"
    ));
    assert!(matches!(
        main.body[4],
        Statement::Let {
            value_type: ValueType::Enum(ref name, ref args),
            initializer: ValueExpr::ArrayGet {
                element_type: ValueType::I32,
                ..
            },
            ..
        } if name == "Option" && args == &vec![ValueType::I32]
    ));
}

#[test]
fn accepts_extended_array_methods() {
    let source = r#"package app.main

import std.array

fn main() -> void {
    let mut items: Array<i32> = Array.new<i32>()
    items.push(1)
    items.insert(1, 2)
    let removed: Option<i32> = items.remove(0)
    let popped: Option<i32> = items.pop()
    items.clear()
    let size: u64 = items.len()
}
"#;

    let program = parse_inline(source).unwrap();
    assert!(program.enums.iter().any(|item| item.name == "Option"));
    let main = program.functions.iter().find(|f| f.name == "main").unwrap();
    assert!(matches!(
        main.body[2],
        Statement::Assign {
            ref name,
            value: ValueExpr::ArrayInsert {
                element_type: ValueType::I32,
                ..
            },
        } if name == "items"
    ));
    assert!(matches!(
        main.body[3],
        Statement::Let {
            value_type: ValueType::Enum(ref name, ref args),
            initializer: ValueExpr::ArrayRemove {
                element_type: ValueType::I32,
                ..
            },
            ..
        } if name == "Option" && args == &vec![ValueType::I32]
    ));
    assert!(matches!(
        main.body[4],
        Statement::Let {
            value_type: ValueType::Enum(ref name, ref args),
            initializer: ValueExpr::ArrayPop {
                element_type: ValueType::I32,
                ..
            },
            ..
        } if name == "Option" && args == &vec![ValueType::I32]
    ));
    assert!(matches!(
        main.body[5],
        Statement::Assign {
            ref name,
            value: ValueExpr::ArrayClear {
                element_type: ValueType::I32,
                ..
            },
        } if name == "items"
    ));
    assert!(matches!(
        main.body[6],
        Statement::Let {
            value_type: ValueType::U64,
            initializer: ValueExpr::ArrayLen { .. },
            ..
        }
    ));
}

#[test]
fn accepts_array_iter_method() {
    let source = r#"package app.main

import std.array
import std.io

fn main() -> void {
    let mut items: Array<i32> = Array.new<i32>()
    items.push(1)
    let snapshot: Array<i32> = items.iter()
    for item in items.iter() {
        io.println("item")
    }
}
"#;

    let program = parse_inline(source).unwrap();
    let main = program.functions.iter().find(|f| f.name == "main").unwrap();
    assert!(matches!(
        main.body[2],
        Statement::Let {
            value_type: ValueType::Array(ref element),
            initializer: ValueExpr::ArrayIter {
                element_type: ValueType::I32,
                ..
            },
            ..
        } if element.as_ref() == &ValueType::I32
    ));
    assert!(matches!(
        main.body[3],
        Statement::Loop {
            kind: LoopKind::Iterate {
                element_type: ValueType::I32,
                iterable: ValueExpr::ArrayIter {
                    element_type: ValueType::I32,
                    ..
                },
                ..
            },
            ..
        }
    ));
}

#[test]
fn rejects_mutating_array_method_on_immutable_variable() {
    let source = r#"package app.main

import std.array

fn main() -> void {
    let items: Array<i32> = Array.new<i32>()
    items.push(1)
}
"#;

    let err = parse_inline(source).unwrap_err();
    assert_eq!(err.code, "E0501");
    assert!(err.message.contains("immutable variable"));
}

#[test]
fn accepts_struct_array_builtins() {
    let source = r#"package app.main

import std.array
import std.io

struct Point {
    x: i32
    y: i32
}

fn main() -> void {
    let mut points: Array<Point> = Array.new<Point>()
    points.push(Point { x: 3, y: 4 })
    let first: Option<Point> = points.get(0)
    let message: string = match first {
        Option.Some(point) => if point.x == 3 {
            "struct array ok"
        } else {
            "wrong"
        }
        Option.None => "missing"
    }
    io.println(message)
}
"#;

    let program = parse_inline(source).unwrap();
    let point_type = ValueType::Struct("Point".to_string(), Vec::new());
    let main = program.functions.iter().find(|f| f.name == "main").unwrap();
    assert!(matches!(
        main.body[0],
        Statement::Let {
            value_type: ValueType::Array(ref element),
            initializer: ValueExpr::ArrayNew {
                element_type: ValueType::Struct(ref name, ref args),
            },
            ..
        } if element.as_ref() == &point_type && name == "Point" && args.is_empty()
    ));
    assert!(matches!(
        main.body[1],
        Statement::Assign {
            ref name,
            value: ValueExpr::ArrayPush {
                element_type: ValueType::Struct(ref struct_name, ref args),
                ..
            },
        } if name == "points" && struct_name == "Point" && args.is_empty()
    ));
    assert!(matches!(
        main.body[2],
        Statement::Let {
            value_type: ValueType::Enum(ref name, ref args),
            initializer: ValueExpr::ArrayGet {
                element_type: ValueType::Struct(ref struct_name, ref struct_args),
                ..
            },
            ..
        } if name == "Option"
            && args == &vec![point_type]
            && struct_name == "Point"
            && struct_args.is_empty()
    ));
}

#[test]
fn accepts_enum_array_builtins() {
    let source = r#"package app.main

import std.array
import std.io
import std.option

fn main() -> void {
    let mut values: Array<Option<i32>> = Array.new<Option<i32>>()
    values.push(Option.Some(7))
    values.push(Option.None)
    let first: Option<Option<i32>> = values.get(0)
    let message: string = match first {
        Option.Some(value) => match value {
            Option.Some(number) => if number == 7 {
                "enum array ok"
            } else {
                "wrong"
            }
            Option.None => "inner missing"
        }
        Option.None => "outer missing"
    }
    io.println(message)
}
"#;

    let program = parse_inline(source).unwrap();
    let option_i32 = ValueType::Enum("Option".to_string(), vec![ValueType::I32]);
    let option_option_i32 = ValueType::Enum("Option".to_string(), vec![option_i32.clone()]);
    let main = program.functions.iter().find(|f| f.name == "main").unwrap();
    assert!(matches!(
        main.body[0],
        Statement::Let {
            value_type: ValueType::Array(ref element),
            initializer: ValueExpr::ArrayNew {
                element_type: ValueType::Enum(ref name, ref args),
            },
            ..
        } if element.as_ref() == &option_i32 && name == "Option" && args == &vec![ValueType::I32]
    ));
    assert!(matches!(
        main.body[1],
        Statement::Assign {
            ref name,
            value: ValueExpr::ArrayPush {
                element_type: ValueType::Enum(ref enum_name, ref enum_args),
                ..
            },
        } if name == "values" && enum_name == "Option" && enum_args == &vec![ValueType::I32]
    ));
    assert!(matches!(
        main.body[3],
        Statement::Let {
            ref value_type,
            initializer: ValueExpr::ArrayGet {
                element_type: ValueType::Enum(ref enum_name, ref enum_args),
                ..
            },
            ..
        } if value_type == &option_option_i32
            && enum_name == "Option"
            && enum_args == &vec![ValueType::I32]
    ));
}

#[test]
fn accepts_arrays_for_all_v0_1_primitive_elements() {
    let source = r#"package app.main

import std.array
import std.io

fn main() -> void {
    let mut strings: Array<string> = Array.new<string>()
    strings.push("nomo")
    let mut ints: Array<i64> = Array.new<i64>()
    ints.push(1)
    let mut i32s: Array<i32> = Array.new<i32>()
    i32s.push(2)
    let mut u32s: Array<u32> = Array.new<u32>()
    u32s.push(3 as u32)
    let mut u64s: Array<u64> = Array.new<u64>()
    u64s.push(4 as u64)
    let mut floats: Array<f64> = Array.new<f64>()
    floats.push(1.5)
    let mut chars: Array<char> = Array.new<char>()
    chars.push('n')
    let mut bools: Array<bool> = Array.new<bool>()
    bools.push(true)
    io.println("arrays ok")
}
"#;

    let program = parse_inline(source).unwrap();
    let main = program.functions.iter().find(|f| f.name == "main").unwrap();
    let array_elements = main
        .body
        .iter()
        .filter_map(|statement| match statement {
            Statement::Let {
                value_type: ValueType::Array(element),
                initializer: ValueExpr::ArrayNew { element_type },
                ..
            } if element.as_ref() == element_type => Some(element_type.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(
        array_elements,
        vec![
            ValueType::String,
            ValueType::Int,
            ValueType::I32,
            ValueType::U32,
            ValueType::U64,
            ValueType::Float,
            ValueType::Char,
            ValueType::Bool,
        ]
    );
}

#[test]
fn rejects_array_void_in_type_positions_before_codegen() {
    for source in [
        r#"package app.main

import std.array

fn main() -> void {
    let values: Array<void> = Array.new<void>()
}
"#,
        r#"package app.main

import std.array

fn bad(values: Array<void>) -> void {
}

fn main() -> void {
}
"#,
        r#"package app.main

import std.array

fn bad() -> Array<void> {
    return Array.new<void>()
}

fn main() -> void {
}
"#,
        r#"package app.main

import std.array

struct Bad {
    values: Array<void>
}

fn main() -> void {
}
"#,
        r#"package app.main

import std.array

enum Bad {
    Values(Array<void>)
}

fn main() -> void {
}
"#,
    ] {
        let err = parse_inline(source).unwrap_err();
        assert!(err.code == "E0403" || err.code == "E0404");
        assert!(err.message.contains("Array elements"));
    }
}

#[test]
fn accepts_generic_array_type_positions_before_instantiation() {
    let source = r#"package app.main

import std.array

struct Bag<T> {
    values: Array<T>
}

fn id<T>(values: Array<T>) -> Array<T> {
    return values
}

fn main() -> void {
    let values: Array<i32> = Array.new<i32>()
    let copy: Array<i32> = id<i32>(values)
}
"#;

    let program = parse_inline(source).unwrap();
    assert_eq!(program.structs[0].type_params, ["T"]);
    let id = program
        .functions
        .iter()
        .find(|f| f.name == "id_i32")
        .unwrap();
    assert_eq!(id.return_type, ValueType::Array(Box::new(ValueType::I32)));
}

#[test]
fn accepts_specific_array_new_import() {
    let source = r#"package app.main

import std.array.new
import std.array.Array
import std.io

fn main() -> void {
    let mut items: Array<i32> = new<i32>()
    items.push(7)
    let first: Option<i32> = items.get(0)
    let message: string = match first {
        Option.Some(value) => if value == 7 {
            "array new import ok"
        } else {
            "wrong"
        }
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
            initializer: ValueExpr::ArrayNew {
                element_type: ValueType::I32,
            },
            ..
        } if element.as_ref() == &ValueType::I32
    ));
}

#[test]
fn accepts_specific_array_method_imports() {
    let source = r#"package app.main

import std.env.args
import std.array.Array
import std.array.get
import std.array.clear
import std.array.insert
import std.array.iter
import std.array.len
import std.array.pop
import std.array.push
import std.array.remove
import std.array.set

fn main() -> void {
    let mut values = args()
    values.push("extra")
    values.insert(1, "middle")
    values.set(0, "program")
    let removed: Option<string> = values.remove(1)
    let popped: Option<string> = values.pop()
    values.clear()
    let snapshot: Array<string> = values.iter()
    let size: u64 = values.len()
    let first: Option<string> = values.get(0)
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
        Statement::Assign {
            ref name,
            value: ValueExpr::ArrayPush { .. },
        } if name == "values"
    ));
    assert!(matches!(
        main.body[2],
        Statement::Assign {
            ref name,
            value: ValueExpr::ArrayInsert { .. },
        } if name == "values"
    ));
    assert!(matches!(
        main.body[3],
        Statement::Assign {
            ref name,
            value: ValueExpr::ArraySet { .. },
        } if name == "values"
    ));
    assert!(matches!(
        main.body[4],
        Statement::Let {
            value_type: ValueType::Enum(ref name, ref args),
            initializer: ValueExpr::ArrayRemove {
                element_type: ValueType::String,
                ..
            },
            ..
        } if name == "Option" && args == &vec![ValueType::String]
    ));
    assert!(matches!(
        main.body[5],
        Statement::Let {
            value_type: ValueType::Enum(ref name, ref args),
            initializer: ValueExpr::ArrayPop {
                element_type: ValueType::String,
                ..
            },
            ..
        } if name == "Option" && args == &vec![ValueType::String]
    ));
    assert!(matches!(
        main.body[6],
        Statement::Assign {
            ref name,
            value: ValueExpr::ArrayClear { .. },
        } if name == "values"
    ));
    assert!(matches!(
        main.body[7],
        Statement::Let {
            value_type: ValueType::Array(ref element),
            initializer: ValueExpr::ArrayIter {
                element_type: ValueType::String,
                ..
            },
            ..
        } if element.as_ref() == &ValueType::String
    ));
    assert!(matches!(
        main.body[8],
        Statement::Let {
            value_type: ValueType::U64,
            initializer: ValueExpr::ArrayLen { .. },
            ..
        }
    ));
    assert!(matches!(
        main.body[9],
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
fn rejects_unqualified_array_new_without_specific_import() {
    let source = r#"package app.main

import std.array
import std.io

fn main() -> void {
    let mut items: Array<i32> = new<i32>()
    io.println("done")
}
"#;

    let err = parse_inline(source).unwrap_err();
    assert_eq!(err.code, "E0305");
    assert!(err.message.contains("new"));
}

#[test]
fn rejects_array_method_without_array_import() {
    let source = r#"package app.main

import std.array.new

fn main() -> void {
    let mut items: Array<i32> = new<i32>()
    items.push(1)
}
"#;

    let err = parse_inline(source).unwrap_err();
    assert_eq!(err.code, "E0301");
    assert!(err.message.contains("std.array"));
}

#[test]
fn rejects_string_len_as_i64() {
    let source = r#"package app.main

import std.string

fn main() -> void {
    let count: i64 = string.len("hello")
}
"#;

    let err = parse_inline(source).unwrap_err();
    assert_eq!(err.code, "E0404");
}

#[test]
fn accepts_basic_equality_for_string_char_and_bool() {
    let source = r#"package app.main

fn main() -> void {
    let same: bool = "nomo" == "nomo"
    let different: bool = "nomo" != "rust"
    let same_char: bool = '語' == '語'
    let same_bool: bool = true == true
}
"#;

    let program = parse_inline(source).unwrap();
    let main = program
        .functions
        .iter()
        .find(|function| function.name == "main")
        .unwrap();
    assert!(matches!(
        main.body.as_slice(),
        [
            Statement::Let {
                initializer: ValueExpr::StringCompare {
                    op: BinaryOp::Equal,
                    ..
                },
                ..
            },
            Statement::Let {
                initializer: ValueExpr::StringCompare {
                    op: BinaryOp::NotEqual,
                    ..
                },
                ..
            },
            Statement::Let {
                initializer: ValueExpr::Binary {
                    op: BinaryOp::Equal,
                    ..
                },
                ..
            },
            Statement::Let {
                initializer: ValueExpr::Binary {
                    op: BinaryOp::Equal,
                    ..
                },
                ..
            },
        ]
    ));
}

#[test]
fn rejects_ordering_comparison_for_strings() {
    let source = r#"package app.main

fn main() -> void {
    let ordered: bool = "a" < "b"
}
"#;

    let err = parse_inline(source).unwrap_err();
    assert_eq!(err.code, "E0404");
    assert!(err.message.contains("comparable operands"));
}

#[test]
fn accepts_function_call_and_integer_return() {
    let source = r#"package app.main

import std.io

fn add(a: i64, b: i64) -> i64 {
    return a + b
}

fn main() -> void {
    let answer: i64 = add(40, 2)
    io.println("done")
}
"#;

    let program = parse_inline(source).unwrap();
    let add = program.functions.iter().find(|f| f.name == "add").unwrap();
    assert_eq!(add.params.len(), 2);
    assert_eq!(add.return_type, ValueType::Int);
    assert!(matches!(
        add.body[0],
        Statement::Return(Some(ValueExpr::Binary {
            op: BinaryOp::Add,
            ..
        }))
    ));
}

#[test]
fn accepts_binary_arithmetic_operators() {
    let source = r#"package app.main

fn calc(a: i64, b: i64, c: i64, d: i64, e: i64) -> i64 {
    return a - b * c / d % e
}

fn ratio(total: f64, count: f64) -> f64 {
    return total / count
}

fn main() -> void {
}
"#;

    let program = parse_inline(source).unwrap();
    let calc = program.functions.iter().find(|f| f.name == "calc").unwrap();
    let ratio = program
        .functions
        .iter()
        .find(|f| f.name == "ratio")
        .unwrap();

    assert!(matches!(
        calc.body[0],
        Statement::Return(Some(ValueExpr::Binary {
            op: BinaryOp::Subtract,
            ..
        }))
    ));
    assert!(matches!(
        ratio.body[0],
        Statement::Return(Some(ValueExpr::Binary {
            op: BinaryOp::Divide,
            ..
        }))
    ));
    assert_eq!(calc.return_type, ValueType::Int);
    assert_eq!(ratio.return_type, ValueType::Float);
}

#[test]
fn rejects_float_remainder() {
    let source = r#"package app.main

fn bad(left: f64, right: f64) -> f64 {
    return left % right
}

fn main() -> void {
}
"#;

    let err = parse_inline(source).unwrap_err();

    assert_eq!(err.code, "E0404");
    assert!(err.message.contains("numeric operands"));
}

#[test]
fn accepts_logical_operators() {
    let source = r#"package app.main

fn check(a: bool, b: bool, c: bool) -> bool {
    return !a && b || c
}

fn main() -> void {
}
"#;

    let program = parse_inline(source).unwrap();
    let check = program
        .functions
        .iter()
        .find(|f| f.name == "check")
        .unwrap();

    assert_eq!(check.return_type, ValueType::Bool);
    assert!(matches!(
        check.body[0],
        Statement::Return(Some(ValueExpr::Binary {
            op: BinaryOp::LogicalOr,
            ..
        }))
    ));
}

#[test]
fn rejects_non_bool_logical_operands() {
    let source = r#"package app.main

fn bad(value: i64) -> bool {
    return value && true
}

fn main() -> void {
}
"#;

    let err = parse_inline(source).unwrap_err();

    assert_eq!(err.code, "E0404");
    assert!(err.message.contains("bool operands"));
}

#[test]
fn rejects_non_bool_not_operand() {
    let source = r#"package app.main

fn bad(value: i64) -> bool {
    return !value
}

fn main() -> void {
}
"#;

    let err = parse_inline(source).unwrap_err();

    assert_eq!(err.code, "E0404");
    assert!(err.message.contains("bool operand"));
}

#[test]
fn accepts_bitwise_operators() {
    let source = r#"package app.main

fn mask(a: i64, b: i64, c: i64, shift: u32) -> i64 {
    return a & b | c ^ a &^ b << shift >> 1
}

fn main() -> void {
}
"#;

    let program = parse_inline(source).unwrap();
    let mask = program.functions.iter().find(|f| f.name == "mask").unwrap();

    assert_eq!(mask.return_type, ValueType::Int);
    assert!(matches!(
        mask.body[0],
        Statement::Return(Some(ValueExpr::Binary {
            op: BinaryOp::BitXor,
            ..
        }))
    ));
}

#[test]
fn rejects_non_integer_bitwise_operands() {
    let source = r#"package app.main

fn bad(left: bool, right: bool) -> bool {
    return left & right
}

fn main() -> void {
}
"#;

    let err = parse_inline(source).unwrap_err();

    assert_eq!(err.code, "E0404");
    assert!(err.message.contains("integer operands"));
}

#[test]
fn rejects_non_integer_shift_rhs() {
    let source = r#"package app.main

fn bad(left: i64, right: bool) -> i64 {
    return left << right
}

fn main() -> void {
}
"#;

    let err = parse_inline(source).unwrap_err();

    assert_eq!(err.code, "E0404");
    assert!(err.message.contains("integer operands"));
}

#[test]
fn accepts_generic_function_instances() {
    let source = r#"package app.main

import std.io

fn identity<T>(value: T) -> T {
    return value
}

fn main() -> void {
    let number: i32 = identity<i32>(7)
    let text: string = identity<string>("generic")
    io.println(text)
}
"#;

    let program = parse_inline(source).unwrap();
    assert!(
        program
            .functions
            .iter()
            .all(|function| function.name != "identity")
    );
    let identity_i32 = program
        .functions
        .iter()
        .find(|function| function.name == "identity_i32")
        .unwrap();
    assert_eq!(identity_i32.params[0].value_type, ValueType::I32);
    assert_eq!(identity_i32.return_type, ValueType::I32);
    let identity_string = program
        .functions
        .iter()
        .find(|function| function.name == "identity_string")
        .unwrap();
    assert_eq!(identity_string.params[0].value_type, ValueType::String);
    assert_eq!(identity_string.return_type, ValueType::String);
    let main = program.functions.iter().find(|f| f.name == "main").unwrap();
    assert!(matches!(
        main.body[0],
        Statement::Let {
            initializer: ValueExpr::Call { ref name, .. },
            ..
        } if name == "identity_i32"
    ));
    assert!(matches!(
        main.body[1],
        Statement::Let {
            initializer: ValueExpr::Call { ref name, .. },
            ..
        } if name == "identity_string"
    ));
}

#[test]
fn rejects_generic_function_call_without_type_arguments() {
    let source = r#"package app.main

import std.io

fn identity<T>(value: T) -> T {
    return value
}

fn main() -> void {
    let number: i32 = identity(7)
    io.println("done")
}
"#;

    let err = parse_inline(source).unwrap_err();
    assert_eq!(err.code, "E0407");
}

#[test]
fn accepts_mut_call_argument_for_mut_parameter() {
    let source = r#"package app.main

import std.io

fn inspect(mut value: i64) -> i64 {
    return value
}

fn main() -> void {
    let mut count: i64 = 41
    let answer: i64 = inspect(mut count) + 1
    io.println("done")
}
"#;

    let program = parse_inline(source).unwrap();
    let main = program.functions.iter().find(|f| f.name == "main").unwrap();
    assert!(matches!(
        &main.body[1],
        Statement::Let {
            initializer: ValueExpr::Binary {
                left,
                ..
            },
            ..
        } if matches!(
            left.as_ref(),
            ValueExpr::Call {
                name,
                args,
            } if name == "inspect"
                && args == &vec![ValueExpr::MutBorrow(vec!["count".to_string()])]
        )
    ));
}

#[test]
fn accepts_mut_field_path_call_argument_for_mut_parameter() {
    let source = r#"package app.main

struct Point {
    x: i32
    y: i32
}

fn bump(mut value: i32) -> void {
    value = value + 1
}

fn main() -> void {
    let mut point: Point = Point { x: 1, y: 2 }
    bump(mut point.x)
}
"#;

    let program = parse_inline(source).unwrap();
    let main = program.functions.iter().find(|f| f.name == "main").unwrap();
    assert!(matches!(
        &main.body[1],
        Statement::Expr(ValueExpr::Call { name, args })
            if name == "bump"
                && args == &vec![ValueExpr::MutBorrow(vec![
                    "point".to_string(),
                    "x".to_string()
                ])]
    ));
}

#[test]
fn accepts_forwarding_mut_parameter_as_mut_argument() {
    let source = r#"package app.main

fn bump(mut value: i32) -> void {
    value = value + 1
}

fn bump_twice(mut value: i32) -> void {
    bump(mut value)
    bump(mut value)
}

fn main() -> void {
}
"#;

    let program = parse_inline(source).unwrap();
    let bump_twice = program
        .functions
        .iter()
        .find(|function| function.name == "bump_twice")
        .unwrap();
    assert!(matches!(
        bump_twice.body.as_slice(),
        [
            Statement::Expr(ValueExpr::Call {
                name: first_name,
                args: first_args,
            }),
            Statement::Expr(ValueExpr::Call {
                name: second_name,
                args: second_args,
            }),
        ] if first_name == "bump"
            && second_name == "bump"
            && first_args == &vec![ValueExpr::MutBorrow(vec!["value".to_string()])]
            && second_args == &vec![ValueExpr::MutBorrow(vec!["value".to_string()])]
    ));
}

#[test]
fn rejects_missing_mut_call_argument_for_mut_parameter() {
    let source = r#"package app.main

import std.io

fn inspect(mut value: i64) -> i64 {
    return value
}

fn main() -> void {
    let mut count: i64 = 41
    let answer: i64 = inspect(count)
    io.println("done")
}
"#;

    let err = parse_inline(source).unwrap_err();
    assert_eq!(err.code, "E0500");
}

#[test]
fn rejects_immutable_variable_as_mut_call_argument() {
    let source = r#"package app.main

import std.io

fn inspect(mut value: i64) -> i64 {
    return value
}

fn main() -> void {
    let count: i64 = 41
    let answer: i64 = inspect(mut count)
    io.println("done")
}
"#;

    let err = parse_inline(source).unwrap_err();
    assert_eq!(err.code, "E0501");
}

#[test]
fn rejects_duplicate_mut_call_argument() {
    let source = r#"package app.main

import std.io

fn combine(mut left: i64, mut right: i64) -> i64 {
    return left + right
}

fn main() -> void {
    let mut count: i64 = 41
    let answer: i64 = combine(mut count, mut count)
    io.println("done")
}
"#;

    let err = parse_inline(source).unwrap_err();
    assert_eq!(err.code, "E0502");
}

#[test]
fn rejects_prefix_conflicting_mut_field_borrow_in_same_call() {
    let source = r#"package app.main

struct Point {
    x: i32
    y: i32
}

fn overwrite(mut point: Point, mut value: i32) -> void {
}

fn main() -> void {
    let mut point: Point = Point { x: 1, y: 2 }
    overwrite(mut point, mut point.x)
}
"#;

    let err = parse_inline(source).unwrap_err();
    assert_eq!(err.code, "E0502");
    assert!(err.message.contains("point.x"));
    assert!(err.message.contains("point"));
}

#[test]
fn accepts_non_overlapping_mut_field_borrows_in_same_call() {
    let source = r#"package app.main

struct Point {
    x: i32
    y: i32
}

fn swap_values(mut left: i32, mut right: i32) -> void {
    let temp: i32 = left
    left = right
    right = temp
}

fn main() -> void {
    let mut point: Point = Point { x: 1, y: 2 }
    swap_values(mut point.x, mut point.y)
}
"#;

    parse_inline(source).unwrap();
}

#[test]
fn accepts_f64_literal_cast_addition_and_comparison() {
    let source = r#"package app.main

import std.io

fn ratio(age: i64) -> f64 {
    return age as f64
}

fn add(a: f64, b: f64) -> f64 {
    return a + b
}

fn check(value: f64) -> bool {
    return value >= 1.5
}

fn main() -> void {
    let pi: f64 = 3.14
    let value: f64 = ratio(42)
    let total: f64 = add(pi, value)
    let ok: bool = check(total)
    io.println("done")
}
"#;

    let program = parse_inline(source).unwrap();
    let ratio = program
        .functions
        .iter()
        .find(|f| f.name == "ratio")
        .unwrap();
    assert_eq!(ratio.return_type, ValueType::Float);
    assert!(matches!(
        ratio.body[0],
        Statement::Return(Some(ValueExpr::Cast {
            target_type: ValueType::Float,
            ..
        }))
    ));
    let add = program.functions.iter().find(|f| f.name == "add").unwrap();
    assert!(matches!(
        add.body[0],
        Statement::Return(Some(ValueExpr::Binary {
            op: BinaryOp::Add,
            ..
        }))
    ));
    let main = program.functions.iter().find(|f| f.name == "main").unwrap();
    assert!(matches!(
        main.body[0],
        Statement::Let {
            value_type: ValueType::Float,
            initializer: ValueExpr::FloatLiteral(ref value),
            ..
        } if value == "3.14"
    ));
}

#[test]
fn rejects_primitive_type_arguments() {
    for (source, type_name) in [
        (
            r#"package app.main

fn main() -> void {
    let value: i32<string> = 1
}
"#,
            "i32",
        ),
        (
            r#"package app.main

fn main() -> void {
    let value: string<i32> = "x"
}
"#,
            "string",
        ),
        (
            r#"package app.main

fn main() -> void {
    let value: bool<i32> = true
}
"#,
            "bool",
        ),
    ] {
        let err = parse_inline(source).unwrap_err();

        assert_eq!(err.code, "E0403");
        assert!(err.message.contains(type_name));
    }
}

#[test]
fn accepts_distinct_integer_types() {
    let source = r#"package app.main

import std.io

fn add32(a: i32, b: i32) -> i32 {
    return a + b
}

fn check64(value: u64) -> bool {
    return value >= 1
}

fn main() -> void {
    let signed: i32 = 1
    let word: u32 = 2
    let wide: u64 = 3
    let total: i32 = add32(signed, 4)
    let ok: bool = check64(wide)
    io.println("done")
}
"#;

    let program = parse_inline(source).unwrap();
    let add32 = program
        .functions
        .iter()
        .find(|f| f.name == "add32")
        .unwrap();
    assert_eq!(add32.params[0].value_type, ValueType::I32);
    assert_eq!(add32.return_type, ValueType::I32);
    let check64 = program
        .functions
        .iter()
        .find(|f| f.name == "check64")
        .unwrap();
    assert_eq!(check64.params[0].value_type, ValueType::U64);
    let main = program.functions.iter().find(|f| f.name == "main").unwrap();
    assert!(matches!(
        main.body[0],
        Statement::Let {
            value_type: ValueType::I32,
            initializer: ValueExpr::IntLiteral(1),
            ..
        }
    ));
    assert!(matches!(
        main.body[1],
        Statement::Let {
            value_type: ValueType::U32,
            initializer: ValueExpr::IntLiteral(2),
            ..
        }
    ));
    assert!(matches!(
        main.body[2],
        Statement::Let {
            value_type: ValueType::U64,
            initializer: ValueExpr::IntLiteral(3),
            ..
        }
    ));
}

#[test]
fn rejects_int_alias_in_v0_1() {
    for source in [
        r#"package app.main

fn main() -> void {
    let value: int = 1
}
"#,
        r#"package app.main

fn inspect(value: int) -> void {
}

fn main() -> void {
}
"#,
        r#"package app.main

fn inspect() -> int {
    return 1
}

fn main() -> void {
}
"#,
    ] {
        let err = parse_inline(source).unwrap_err();

        assert_eq!(err.code, "E0403");
        assert!(err.message.contains("`int` is not a v0.1 builtin type"));
        assert!(err.message.contains("i64"));
        assert!(err.message.contains("i32"));
        assert!(err.message.contains("u32"));
        assert!(err.message.contains("u64"));
    }
}

#[test]
fn rejects_i32_literal_overflow() {
    let source = r#"package app.main

fn main() -> void {
    let value: i32 = 2147483648
}
"#;

    let err = parse_inline(source).unwrap_err();
    assert_eq!(err.code, "E0404");
}

#[test]
fn rejects_mixed_integer_binary_without_cast() {
    let source = r#"package app.main

fn main() -> void {
    let left: i32 = 1
    let right: i64 = 2
    let value: i64 = left + right
}
"#;

    let err = parse_inline(source).unwrap_err();
    assert_eq!(err.code, "E0404");
}

#[test]
fn accepts_char_literal_and_return() {
    let source = r#"package app.main

import std.io

fn initial() -> char {
    return 'N'
}

fn main() -> void {
    let letter: char = initial()
    io.println("done")
}
"#;

    let program = parse_inline(source).unwrap();
    let initial = program
        .functions
        .iter()
        .find(|f| f.name == "initial")
        .unwrap();
    assert_eq!(initial.return_type, ValueType::Char);
    assert!(matches!(
        initial.body[0],
        Statement::Return(Some(ValueExpr::CharLiteral('N')))
    ));
}

#[test]
fn rejects_implicit_int_to_f64_initializer() {
    let source = r#"package app.main

fn main() -> void {
    let ratio: f64 = 42
}
"#;

    let err = parse_inline(source).unwrap_err();
    assert_eq!(err.code, "E0404");
}

#[test]
fn rejects_char_string_mismatch() {
    let source = r#"package app.main

fn main() -> void {
    let text: string = 'N'
}
"#;

    let err = parse_inline(source).unwrap_err();
    assert_eq!(err.code, "E0404");
}

#[test]
fn accepts_tail_expression_return() {
    let source = r#"package app.main

import std.io

fn add(a: i64, b: i64) -> i64 {
    a + b
}

fn main() -> void {
    io.println("done")
}
"#;

    let program = parse_inline(source).unwrap();
    let add = program.functions.iter().find(|f| f.name == "add").unwrap();
    assert!(matches!(add.body[0], Statement::Return(Some(_))));
}

#[test]
fn accepts_if_expression_and_integer_comparison() {
    let source = r#"package app.main

import std.io

fn label(score: i64) -> string {
    return if score >= 60 {
        "pass"
    } else {
        "fail"
    }
}

fn main() -> void {
    let text: string = label(75)
    io.println(text)
}
"#;

    let program = parse_inline(source).unwrap();
    let label = program
        .functions
        .iter()
        .find(|f| f.name == "label")
        .unwrap();
    assert!(matches!(
        label.body[0],
        Statement::Return(Some(ValueExpr::If {
            ref condition,
            ref then_branch,
            ref else_branch,
        })) if matches!(
            condition.as_ref(),
            ValueExpr::Binary {
                op: BinaryOp::GreaterEqual,
                ..
            }
        ) && then_branch.as_ref() == &ValueExpr::StringLiteral("pass".to_string())
            && else_branch.as_ref() == &ValueExpr::StringLiteral("fail".to_string())
    ));
}

#[test]
fn accepts_panic_statement() {
    let source = r#"package app.main

import std.io

fn main() -> void {
    panic("boom")
}
"#;

    let program = parse_inline(source).unwrap();
    let main = program.functions.iter().find(|f| f.name == "main").unwrap();
    assert!(matches!(
        main.body[0],
        Statement::Panic(ValueExpr::StringLiteral(ref message)) if message == "boom"
    ));
}

#[test]
fn accepts_panic_as_diverging_match_arm() {
    let source = r#"package app.main

import std.io

enum Option<T> {
    Some(T)
    None
}

fn unwrap_text(value: Option<string>) -> string {
    return match value {
        Option.Some(text) => text
        Option.None => panic("missing text")
    }
}

fn main() -> void {
    let value: Option<string> = Option.Some("hello")
    let text: string = unwrap_text(value)
    io.println(text)
}
"#;

    let program = parse_inline(source).unwrap();
    let unwrap = program
        .functions
        .iter()
        .find(|function| function.name == "unwrap_text")
        .unwrap();
    assert!(matches!(
        unwrap.body[0],
        Statement::Return(Some(ValueExpr::Match { .. }))
    ));
}

#[test]
fn rejects_if_condition_that_is_not_bool() {
    let source = r#"package app.main

import std.io

fn label(score: i64) -> string {
    return if score {
        "pass"
    } else {
        "fail"
    }
}

fn main() -> void {
    io.println("done")
}
"#;

    let err = parse_inline(source).unwrap_err();
    assert_eq!(err.code, "E0404");
}

#[test]
fn rejects_if_branch_type_mismatch() {
    let source = r#"package app.main

import std.io

fn value(flag: bool) -> i64 {
    return if flag {
        1
    } else {
        "nope"
    }
}

fn main() -> void {
    io.println("done")
}
"#;

    let err = parse_inline(source).unwrap_err();
    assert_eq!(err.code, "E0404");
}

#[test]
fn rejects_unknown_variable() {
    let source = r#"package app.main

import std.io

fn main() -> void {
    io.println(message)
}
"#;

    let err = parse_inline(source).unwrap_err();
    assert_eq!(err.code, "E0303");
}

#[test]
fn rejects_let_type_mismatch() {
    let source = r#"package app.main

import std.io

fn main() -> void {
    let message: string = 42
}
"#;

    let err = parse_inline(source).unwrap_err();
    assert_eq!(err.code, "E0404");
}

#[test]
fn rejects_wrong_function_argument_type() {
    let source = r#"package app.main

import std.io

fn id(value: i64) -> i64 {
    return value
}

fn main() -> void {
    let answer: i64 = id("nope")
    io.println("done")
}
"#;

    let err = parse_inline(source).unwrap_err();
    assert_eq!(err.code, "E0404");
}

#[test]
fn accepts_assignment_to_mut_variable() {
    let source = r#"package app.main

import std.io

fn main() -> void {
    let mut count: i64 = 1
    count = count + 1
    io.println("done")
}
"#;

    let program = parse_inline(source).unwrap();
    let main = program.functions.iter().find(|f| f.name == "main").unwrap();
    assert!(matches!(
        main.body[1],
        Statement::Assign {
            ref name,
            value: ValueExpr::Binary { .. },
        } if name == "count"
    ));
}

#[test]
fn accepts_compound_assignment_to_mut_variable() {
    let source = r#"package app.main

fn main() -> void {
    let mut count: i64 = 1
    count += 2
    count -= 1
    count *= 3
    count /= 2
    count %= 2
}
"#;

    let program = parse_inline(source).unwrap();
    let main = program.functions.iter().find(|f| f.name == "main").unwrap();
    for stmt in &main.body[1..] {
        assert!(matches!(
            stmt,
            Statement::Assign {
                name,
                value: ValueExpr::Binary { .. },
            } if name == "count"
        ));
    }
}

#[test]
fn accepts_postfix_update_to_mut_variable() {
    let source = r#"package app.main

fn main() -> void {
    let mut count: i64 = 1
    count++
    count--
}
"#;

    let program = parse_inline(source).unwrap();
    let main = program.functions.iter().find(|f| f.name == "main").unwrap();
    for stmt in &main.body[1..] {
        assert!(matches!(
            stmt,
            Statement::Assign {
                name,
                value: ValueExpr::Binary { .. },
            } if name == "count"
        ));
    }
}

#[test]
fn rejects_assignment_to_immutable_variable() {
    let source = r#"package app.main

import std.io

fn main() -> void {
    let count: i64 = 1
    count = count + 1
    io.println("done")
}
"#;

    let err = parse_inline(source).unwrap_err();
    assert_eq!(err.code, "E0501");
}

#[test]
fn rejects_postfix_update_to_immutable_variable() {
    let source = r#"package app.main

fn main() -> void {
    let count: i64 = 1
    count++
}
"#;

    let err = parse_inline(source).unwrap_err();
    assert_eq!(err.code, "E0501");
}

#[test]
fn rejects_postfix_update_to_non_numeric_variable() {
    let source = r#"package app.main

fn main() -> void {
    let mut message: string = "hi"
    message++
}
"#;

    let err = parse_inline(source).unwrap_err();
    assert_eq!(err.code, "E0404");
}

#[test]
fn accepts_assignment_to_mut_parameter() {
    let source = r#"package app.main

fn bump(mut value: i64) -> i64 {
    value = value + 1
    return value
}

fn main() -> void {
}
"#;

    let program = parse_inline(source).unwrap();
    let bump = program.functions.iter().find(|f| f.name == "bump").unwrap();

    assert!(matches!(
        bump.body[0],
        Statement::Assign {
            ref name,
            value: ValueExpr::Binary { .. },
        } if name == "value"
    ));
}

#[test]
fn rejects_assignment_to_immutable_parameter() {
    let source = r#"package app.main

fn bump(value: i64) -> i64 {
    value = value + 1
    return value
}

fn main() -> void {
}
"#;

    let err = parse_inline(source).unwrap_err();
    assert_eq!(err.code, "E0501");
    assert!(err.message.contains("value"));
}

#[test]
fn rejects_assignment_to_field_of_immutable_parameter() {
    let source = r#"package app.main

struct Counter {
    value: i64
}

fn bump(counter: Counter) -> void {
    counter.value = counter.value + 1
}

fn main() -> void {
}
"#;

    let err = parse_inline(source).unwrap_err();
    assert_eq!(err.code, "E0501");
    assert!(
        err.message
            .contains("cannot assign to field of immutable parameter `counter`")
    );
}

#[test]
fn rejects_duplicate_local_binding() {
    let source = r#"package app.main

fn main() -> void {
    let count: i64 = 1
    let count: i64 = 2
}
"#;

    let err = parse_inline(source).unwrap_err();
    assert_eq!(err.code, "E0302");
}

#[test]
fn duplicate_function_diagnostic_uses_second_declaration_span() {
    let source = r#"package app.main

fn helper() -> void {
}

fn helper() -> void {
}

fn main() -> void {
}
"#;

    let err = parse_inline(source).unwrap_err();
    assert_eq!(err.code, "E0304");
    assert_eq!(err.line, 6);
    assert_eq!(err.column, 1);
    assert_eq!(err.length, 1);
    assert_eq!(err.text, "fn helper() -> void {");
}

#[test]
fn rejects_parameter_shadowing_by_local_binding() {
    let source = r#"package app.main

fn echo(value: i64) -> i64 {
    let value: i64 = 2
    return value
}

fn main() -> void {
}
"#;

    let err = parse_inline(source).unwrap_err();
    assert_eq!(err.code, "E0302");
}

#[test]
fn accepts_assignment_to_mut_struct_field() {
    let source = r#"package app.main

import std.io

struct Counter {
    value: i64
}

fn main() -> void {
    let mut counter: Counter = Counter { value: 1 }
    counter.value = counter.value + 1
    io.println("done")
}
"#;

    let program = parse_inline(source).unwrap();
    let main = program.functions.iter().find(|f| f.name == "main").unwrap();
    assert!(matches!(
        main.body[1],
        Statement::AssignField {
            ref base,
            ref field,
            value_type: ValueType::Int,
            value: ValueExpr::Binary { .. },
            ..
        } if base == "counter" && field == "value"
    ));
}

#[test]
fn accepts_compound_assignment_to_mut_struct_field() {
    let source = r#"package app.main

struct Counter {
    value: i64
}

fn main() -> void {
    let mut counter: Counter = Counter { value: 7 }
    counter.value <<= 1
    counter.value >>= 1
    counter.value &= 6
    counter.value |= 8
    counter.value ^= 3
    counter.value &^= 1
}
"#;

    let program = parse_inline(source).unwrap();
    let main = program.functions.iter().find(|f| f.name == "main").unwrap();
    for stmt in &main.body[1..] {
        assert!(matches!(
            stmt,
            Statement::AssignField {
                base,
                field,
                value_type: ValueType::Int,
                value: ValueExpr::Binary { .. },
            } if base == "counter" && field == "value"
        ));
    }
}

#[test]
fn accepts_postfix_update_to_mut_struct_field() {
    let source = r#"package app.main

struct Counter {
    value: i64
}

fn main() -> void {
    let mut counter: Counter = Counter { value: 7 }
    counter.value++
    counter.value--
}
"#;

    let program = parse_inline(source).unwrap();
    let main = program.functions.iter().find(|f| f.name == "main").unwrap();
    for stmt in &main.body[1..] {
        assert!(matches!(
            stmt,
            Statement::AssignField {
                base,
                field,
                value_type: ValueType::Int,
                value: ValueExpr::Binary { .. },
            } if base == "counter" && field == "value"
        ));
    }
}

#[test]
fn rejects_assignment_to_immutable_struct_field() {
    let source = r#"package app.main

import std.io

struct Counter {
    value: i64
}

fn main() -> void {
    let counter: Counter = Counter { value: 1 }
    counter.value = counter.value + 1
    io.println("done")
}
"#;

    let err = parse_inline(source).unwrap_err();
    assert_eq!(err.code, "E0501");
}

#[test]
fn rejects_assignment_type_mismatch() {
    let source = r#"package app.main

import std.io

fn main() -> void {
    let mut count: i64 = 1
    count = "nope"
    io.println("done")
}
"#;

    let err = parse_inline(source).unwrap_err();
    assert_eq!(err.code, "E0404");
}

#[test]
fn accepts_struct_literal_and_field_access() {
    let source = r#"package app.main

import std.io

struct Point {
    x: i64
    y: i64
}

fn sum(point: Point) -> i64 {
    return point.x + point.y
}

fn main() -> void {
    let point: Point = Point { x: 40, y: 2 }
    let answer: i64 = sum(point)
    io.println("done")
}
"#;

    let program = parse_inline(source).unwrap();
    assert_eq!(program.structs.len(), 1);
    assert_eq!(program.structs[0].name, "Point");
    let sum = program.functions.iter().find(|f| f.name == "sum").unwrap();
    assert_eq!(
        sum.params[0].value_type,
        ValueType::Struct("Point".to_string(), Vec::new())
    );
    assert!(matches!(
        sum.body[0],
        Statement::Return(Some(ValueExpr::Binary { .. }))
    ));
}

#[test]
fn accepts_generic_struct_literal_and_field_access() {
    let source = r#"package app.main

import std.io

struct Box<T> {
    value: T
}

fn main() -> void {
    let item: Box<i32> = Box { value: 7 }
    let value: i32 = item.value
    io.println("done")
}
"#;

    let program = parse_inline(source).unwrap();
    assert_eq!(program.structs[0].type_params, ["T"]);
    let main = program.functions.iter().find(|f| f.name == "main").unwrap();
    assert!(matches!(
        main.body[0],
        Statement::Let {
            value_type: ValueType::Struct(ref name, ref args),
            initializer: ValueExpr::StructLiteral {
                struct_args: ref literal_args,
                ..
            },
            ..
        } if name == "Box"
            && args == &vec![ValueType::I32]
            && literal_args == &vec![ValueType::I32]
    ));
    assert!(matches!(
        main.body[1],
        Statement::Let {
            value_type: ValueType::I32,
            initializer: ValueExpr::FieldAccess { .. },
            ..
        }
    ));
}

#[test]
fn rejects_direct_recursive_struct_value_field() {
    let source = r#"package app.main

struct Node {
    next: Node
}

fn main() -> void {
}
"#;

    let err = parse_inline(source).unwrap_err();
    assert_eq!(err.code, "E0410");
    assert!(err.message.contains("Node"));
    assert!(err.message.contains("recursively embedded"));
}

#[test]
fn rejects_recursive_struct_through_option_payload() {
    let source = r#"package app.main

import std.option

struct Node {
    next: Option<Node>
}

fn main() -> void {
}
"#;

    let err = parse_inline(source).unwrap_err();
    assert_eq!(err.code, "E0410");
    assert!(err.message.contains("Node"));
    assert!(err.message.contains("recursively embedded"));
}

#[test]
fn accepts_recursive_struct_behind_array_boundary() {
    let source = r#"package app.main

import std.array

struct Node {
    children: Array<Node>
}

fn main() -> void {
    let children: Array<Node> = Array.new<Node>()
    let node: Node = Node { children: children }
}
"#;

    let program = parse_inline(source).unwrap();
    assert_eq!(program.structs[0].name, "Node");
    assert_eq!(
        program.structs[0].fields[0].value_type,
        ValueType::Array(Box::new(ValueType::Struct("Node".to_string(), Vec::new())))
    );
}

#[test]
fn rejects_generic_struct_literal_without_type_annotation() {
    let source = r#"package app.main

import std.io

struct Box<T> {
    value: T
}

fn main() -> void {
    let item = Box { value: 7 }
    io.println("done")
}
"#;

    let err = parse_inline(source).unwrap_err();
    assert_eq!(err.code, "E0317");
}

#[test]
fn accepts_impl_method_call() {
    let source = r#"package app.main

import std.io

struct User {
    email: string
}

impl User {
    pub fn get_email(self) -> string {
        return self.email
    }
}

fn main() -> void {
    let user: User = User { email: "a@nomo.dev" }
    let email: string = user.get_email()
    io.println(email)
}
"#;

    let program = parse_inline(source).unwrap();
    let method = program
        .functions
        .iter()
        .find(|function| function.name == "User_get_email")
        .unwrap();
    assert_eq!(
        method.params[0].value_type,
        ValueType::Struct("User".to_string(), Vec::new())
    );
    let main = program.functions.iter().find(|f| f.name == "main").unwrap();
    assert!(matches!(
        main.body[1],
        Statement::Let {
            value_type: ValueType::String,
            initializer: ValueExpr::Call {
                ref name,
                ref args,
            },
            ..
        } if name == "User_get_email"
            && args == &vec![ValueExpr::Variable("user".to_string())]
    ));
}

#[test]
fn accepts_interface_impl_when_methods_match() {
    let source = r#"package app.main

import std.io

interface Display {
    fn to_string(self) -> string
}

struct User {
    name: string
}

impl Display for User {
    fn to_string(self) -> string {
        return self.name
    }
}

fn main() -> void {
    let user: User = User { name: "ok" }
    io.println(user.to_string())
}
"#;

    let program = parse_inline(source).unwrap();
    assert!(
        program
            .functions
            .iter()
            .any(|function| function.name == "User_to_string")
    );
}

#[test]
fn accepts_extern_c_primitive_call_inside_unsafe() {
    let source = r#"package app.main

extern "C" {
    fn abs(value: i32) -> i32
}

fn main() -> void {
    unsafe {
        let value: i32 = abs(-7)
    }
}
"#;

    let program = parse_inline(source).unwrap();
    let main = program.functions.iter().find(|f| f.name == "main").unwrap();
    assert!(matches!(
        main.body[0],
        Statement::Let {
            value_type: ValueType::I32,
            initializer: ValueExpr::Call {
                ref name,
                ref args,
            },
            ..
        } if name == "__nomo_extern::abs"
            && matches!(args.as_slice(), [ValueExpr::IntLiteral(-7)])
    ));
}

#[test]
fn rejects_extern_c_call_outside_unsafe() {
    let source = r#"package app.main

extern "C" {
    fn abs(value: i32) -> i32
}

fn main() -> void {
    let value: i32 = abs(-7)
}
"#;

    let err = parse_inline(source).unwrap_err();
    assert_eq!(err.code, "E1519");
    assert!(
        err.message
            .contains("extern function `abs` must be called inside an `unsafe` block")
    );
}

#[test]
fn rejects_interface_impl_missing_method() {
    let source = r#"package app.main

interface Display {
    fn to_string(self) -> string
}

struct User {
    name: string
}

impl Display for User {
}

fn main() -> void {
}
"#;

    let err = parse_inline(source).unwrap_err();
    assert_eq!(err.code, "E0258");
    assert!(err.message.contains("missing method `to_string`"));
}

#[test]
fn rejects_interface_impl_return_type_mismatch() {
    let source = r#"package app.main

interface Display {
    fn to_string(self) -> string
}

struct User {
    name: string
}

impl Display for User {
    fn to_string(self) -> i32 {
        return 1
    }
}

fn main() -> void {
}
"#;

    let err = parse_inline(source).unwrap_err();
    assert_eq!(err.code, "E0258");
    assert!(err.message.contains("returns `i32`"));
    assert!(err.message.contains("expects `string`"));
}

#[test]
fn rejects_impl_for_unknown_interface() {
    let source = r#"package app.main

struct User {
    name: string
}

impl Display for User {
    fn to_string(self) -> string {
        return self.name
    }
}

fn main() -> void {
}
"#;

    let err = parse_inline(source).unwrap_err();
    assert_eq!(err.code, "E0258");
    assert!(err.message.contains("unknown interface `Display`"));
}

#[test]
fn accepts_imported_public_interface_impl() {
    let root = std::env::temp_dir().join(format!("nomo-interface-import-{}", std::process::id()));
    let src = root.join("src");
    std::fs::create_dir_all(&src).unwrap();
    let display = src.join("display.nomo");
    std::fs::write(
        &display,
        "package app.display\n\npub interface Display {\n    fn to_string(self) -> string\n}\n",
    )
    .unwrap();
    let main = src.join("main.nomo");
    let source = r#"package app.main

import app.display
import std.io

struct User {
    name: string
}

impl Display for User {
    fn to_string(self) -> string {
        return self.name
    }
}

fn main() -> void {
    let user: User = User { name: "ok" }
    io.println(user.to_string())
}
"#;

    let program =
        check_source_text_with_project_modules(&main, source, Some(&src), &[], &[]).unwrap();
    assert!(
        program
            .functions
            .iter()
            .any(|function| function.name == "User_to_string")
    );
    std::fs::remove_dir_all(&root).unwrap();
}

#[test]
fn accepts_mut_impl_method_receiver_call() {
    let source = r#"package app.main

import std.io

struct User {
    email: string
}

impl User {
    pub fn set_email(mut self, email: string) -> void {
        self.email = email
    }
}

fn main() -> void {
    let mut user: User = User { email: "old@nomo.dev" }
    user.set_email("new@nomo.dev")
    io.println(user.email)
}
"#;

    let program = parse_inline(source).unwrap();
    let method = program
        .functions
        .iter()
        .find(|function| function.name == "User_set_email")
        .unwrap();
    assert!(method.params[0].mutable);
    let main = program.functions.iter().find(|f| f.name == "main").unwrap();
    assert!(matches!(
        main.body[1],
        Statement::Expr(ValueExpr::Call {
            ref name,
            ref args,
        }) if name == "User_set_email"
            && args == &vec![
                ValueExpr::MutBorrow(vec!["user".to_string()]),
                ValueExpr::StringLiteral("new@nomo.dev".to_string())
            ]
    ));
}

#[test]
fn rejects_mut_impl_method_receiver_on_immutable_parameter() {
    let source = r#"package app.main

struct Counter {
    value: i32
}

impl Counter {
    pub fn bump(mut self) -> void {
        self.value = self.value + 1
    }
}

fn touch(counter: Counter) -> void {
    counter.bump()
}

fn main() -> void {
}
"#;

    let err = parse_inline(source).unwrap_err();
    assert_eq!(err.code, "E0501");
    assert!(
        err.message.contains(
            "cannot call mutating method `Counter.bump` on immutable parameter `counter`"
        )
    );
}

#[test]
fn rejects_duplicate_mut_borrow_between_receiver_and_argument() {
    let source = r#"package app.main

struct Counter {
    value: i32
}

impl Counter {
    pub fn absorb(mut self, mut other: Counter) -> void {
    }
}

fn main() -> void {
    let mut counter: Counter = Counter { value: 1 }
    counter.absorb(mut counter)
}
"#;

    let err = parse_inline(source).unwrap_err();
    assert_eq!(err.code, "E0502");
    assert!(err.message.contains("counter"));
}

#[test]
fn rejects_impl_for_non_local_std_struct() {
    let source = r#"package app.main

import std.fs
import std.io

impl File {
    pub fn label(self) -> string {
        return "file"
    }
}

fn main() -> void {
    io.println("done")
}
"#;

    let err = parse_inline(source).unwrap_err();

    assert_eq!(err.code, "E0255");
    assert!(err.message.contains("local struct"));
    assert!(err.message.contains("File"));
}

#[test]
fn rejects_struct_and_enum_with_same_name() {
    let source = r#"package app.main

struct Status {
    code: i32
}

enum Status {
    Ok
}

fn main() -> void {
}
"#;

    let err = parse_inline(source).unwrap_err();

    assert_eq!(err.code, "E0312");
    assert!(err.message.contains("Status"));
}

#[test]
fn rejects_user_type_conflicting_with_imported_std_type() {
    let source = r#"package app.main

import std.result

struct Result {
    value: i32
}

fn main() -> void {
}
"#;

    let err = parse_inline(source).unwrap_err();

    assert_eq!(err.code, "E0312");
    assert!(err.message.contains("Result"));
}

#[test]
fn rejects_user_enum_conflicting_with_required_std_result() {
    let source = r#"package app.main

import std.result

enum Result {
    Local
}

fn main() -> void {
}
"#;

    let err = parse_inline(source).unwrap_err();

    assert_eq!(err.code, "E0312");
    assert!(err.message.contains("Result"));
    assert!(err.message.contains("standard library"));
}

#[test]
fn rejects_user_enum_conflicting_with_required_std_option() {
    let source = r#"package app.main

import std.array

enum Option {
    Local
}

fn main() -> void {
    let mut items: Array<i32> = Array.new<i32>()
    items.push(1)
}
"#;

    let err = parse_inline(source).unwrap_err();

    assert_eq!(err.code, "E0312");
    assert!(err.message.contains("Option"));
    assert!(err.message.contains("standard library"));
}

#[test]
fn rejects_user_struct_conflicting_with_required_std_fs_error() {
    let source = r#"package app.main

import std.fs

struct FsError {
    code: i32
}

fn main() -> void {
}
"#;

    let err = parse_inline(source).unwrap_err();

    assert_eq!(err.code, "E0312");
    assert!(err.message.contains("FsError"));
    assert!(err.message.contains("standard library"));
}

#[test]
fn accepts_pub_declarations_as_visibility_metadata() {
    let source = r#"package app.main

import std.io

pub struct User {
    pub id: string
    email: string
}

pub enum Color {
    Red
    Blue
}

pub fn label(color: Color) -> string {
    return match color {
        Color.Red => "red"
        Color.Blue => "blue"
    }
}

pub fn main() -> void {
    let user: User = User { id: "42", email: "a@nomo.dev" }
    let color: Color = Color.Red
    let text: string = label(color)
    io.println(text)
}
"#;

    let program = parse_inline(source).unwrap();
    assert_eq!(program.structs.len(), 1);
    assert_eq!(program.enums.len(), 1);
    assert!(
        program
            .functions
            .iter()
            .any(|function| function.name == "main")
    );
}

#[test]
fn rejects_struct_literal_field_type_mismatch() {
    let source = r#"package app.main

import std.io

struct Point {
    x: i64
    y: i64
}

fn main() -> void {
    let point: Point = Point { x: "bad", y: 2 }
    io.println("done")
}
"#;

    let err = parse_inline(source).unwrap_err();
    assert_eq!(err.code, "E0404");
}

#[test]
fn rejects_unknown_struct_field_access() {
    let source = r#"package app.main

import std.io

struct Point {
    x: i64
    y: i64
}

fn main() -> void {
    let point: Point = Point { x: 1, y: 2 }
    let z: i64 = point.z
    io.println("done")
}
"#;

    let err = parse_inline(source).unwrap_err();
    assert_eq!(err.code, "E0308");
}

#[test]
fn accepts_enum_variant_and_exhaustive_match() {
    let source = r#"package app.main

import std.io

enum Color {
    Red
    Blue
}

fn label(color: Color) -> string {
    return match color {
        Color.Red => "red"
        Color.Blue => "blue"
    }
}

fn main() -> void {
    let color: Color = Color.Red
    let text: string = label(color)
    io.println(text)
}
"#;

    let program = parse_inline(source).unwrap();
    assert_eq!(program.enums.len(), 1);
    assert_eq!(
        program.enums[0]
            .variants
            .iter()
            .map(|variant| variant.name.as_str())
            .collect::<Vec<_>>(),
        vec!["Red", "Blue"]
    );
    let label = program
        .functions
        .iter()
        .find(|f| f.name == "label")
        .unwrap();
    assert_eq!(
        label.params[0].value_type,
        ValueType::Enum("Color".to_string(), Vec::new())
    );
    assert!(matches!(
        label.body[0],
        Statement::Return(Some(ValueExpr::Match { .. }))
    ));
}

#[test]
fn rejects_generic_enum_type_with_missing_type_argument() {
    let source = r#"package app.main

enum Option<T> {
    Some(T)
    None
}

fn main() -> void {
    let value: Option = Option.None
}
"#;

    let err = parse_inline(source).unwrap_err();

    assert_eq!(err.code, "E0403");
    assert!(err.message.contains("Option"));
}

#[test]
fn rejects_non_generic_enum_type_with_extra_type_argument() {
    let source = r#"package app.main

enum Color {
    Red
}

fn main() -> void {
    let value: Color<i32> = Color.Red
}
"#;

    let err = parse_inline(source).unwrap_err();

    assert_eq!(err.code, "E0403");
    assert!(err.message.contains("Color"));
}

#[test]
fn rejects_std_result_type_with_missing_type_argument() {
    let source = r#"package app.main

import std.result

fn main() -> void {
    let value: Result<i32> = Result.Ok(1)
}
"#;

    let err = parse_inline(source).unwrap_err();

    assert_eq!(err.code, "E0403");
    assert!(err.message.contains("Result"));
}

#[test]
fn rejects_non_exhaustive_match() {
    let source = r#"package app.main

import std.io

enum Color {
    Red
    Blue
}

fn label(color: Color) -> string {
    return match color {
        Color.Red => "red"
    }
}

fn main() -> void {
    io.println("done")
}
"#;

    let err = parse_inline(source).unwrap_err();
    assert_eq!(err.code, "E0318");
}

#[test]
fn accepts_payload_enum_and_match_binding() {
    let source = r#"package app.main

import std.io

enum MaybeInt {
    Some(i64)
    None
}

fn unwrap_or_zero(value: MaybeInt) -> i64 {
    return match value {
        MaybeInt.Some(n) => n
        MaybeInt.None => 0
    }
}

fn main() -> void {
    let value: MaybeInt = MaybeInt.Some(41)
    let answer: i64 = unwrap_or_zero(value) + 1
    io.println("done")
}
"#;

    let program = parse_inline(source).unwrap();
    assert_eq!(program.enums[0].variants[0].payload, Some(ValueType::Int));
    let unwrap = program
        .functions
        .iter()
        .find(|function| function.name == "unwrap_or_zero")
        .unwrap();
    assert!(matches!(
        unwrap.body[0],
        Statement::Return(Some(ValueExpr::Match { .. }))
    ));
}

#[test]
fn accepts_struct_payload_enum_and_match_field_access() {
    let source = r#"package app.main

import std.io

struct User {
    email: string
}

enum MaybeUser {
    Some(User)
    None
}

fn label(value: MaybeUser) -> string {
    return match value {
        MaybeUser.Some(user) => user.email
        MaybeUser.None => "missing"
    }
}

fn main() -> void {
    let value: MaybeUser = MaybeUser.Some(User { email: "a@nomo.dev" })
    io.println(label(value))
}
"#;

    let program = parse_inline(source).unwrap();
    assert_eq!(
        program.enums[0].variants[0].payload,
        Some(ValueType::Struct("User".to_string(), Vec::new()))
    );
    let label = program
        .functions
        .iter()
        .find(|function| function.name == "label")
        .unwrap();
    assert!(matches!(
        label.body[0],
        Statement::Return(Some(ValueExpr::Match { ref arms, .. }))
            if matches!(
                arms[0].value,
                ValueExpr::EnumPayloadFieldAccess {
                    ref variant,
                    ref field,
                    ..
                } if variant == "Some" && field == "email"
            )
    ));
}

#[test]
fn accepts_struct_payload_enum_and_match_method_call() {
    let source = r#"package app.main

import std.io

struct User {
    email: string
}

impl User {
    pub fn label(self) -> string {
        return self.email
    }
}

enum MaybeUser {
    Some(User)
    None
}

fn label(value: MaybeUser) -> string {
    return match value {
        MaybeUser.Some(user) => user.label()
        MaybeUser.None => "missing"
    }
}

fn main() -> void {
    let value: MaybeUser = MaybeUser.Some(User { email: "a@nomo.dev" })
    io.println(label(value))
}
"#;

    let program = parse_inline(source).unwrap();
    let label = program
        .functions
        .iter()
        .find(|function| function.name == "label")
        .unwrap();
    assert!(matches!(
        label.body[0],
        Statement::Return(Some(ValueExpr::Match { ref arms, .. }))
            if matches!(
                arms[0].value,
                ValueExpr::Call {
                    ref name,
                    ref args,
                } if name == "User_label"
                    && matches!(
                        args.as_slice(),
                        [ValueExpr::EnumPayload { variant, .. }] if variant == "Some"
                    )
            )
    ));
}

#[test]
fn rejects_match_payload_binding_shadowing_outer_variable() {
    let source = r#"package app.main

import std.io

enum Option<T> {
    Some(T)
    None
}

fn main() -> void {
    let text: string = "outer"
    let value: Option<string> = Option.Some("inner")
    let result: string = match value {
        Option.Some(text) => text
        Option.None => text
    }
    io.println(result)
}
"#;

    let err = parse_inline(source).unwrap_err();
    assert_eq!(err.code, "E0302");
    assert!(err.message.contains("text"));
}

#[test]
fn rejects_let_else_binding_shadowing_outer_variable() {
    let source = r#"package app.main

fn label(value: Option<string>) -> string {
    let text: string = "outer"
    let Some(text) = value else {
        return "missing"
    }
    return text
}

fn main() -> void {
}
"#;

    let err = parse_inline(source).unwrap_err();
    assert_eq!(err.code, "E0302");
    assert!(err.message.contains("text"));
}

#[test]
fn rejects_if_let_binding_shadowing_outer_variable() {
    let source = r#"package app.main

fn label(value: Option<string>) -> string {
    let text: string = "outer"
    if let Some(text) = value {
        return text
    } else {
        return "missing"
    }
}

fn main() -> void {
}
"#;

    let err = parse_inline(source).unwrap_err();
    assert_eq!(err.code, "E0302");
    assert!(err.message.contains("text"));
}

#[test]
fn rejects_for_iter_binding_shadowing_outer_variable() {
    let source = r#"package app.main

import std.array

fn main() -> void {
    let item: i32 = 0
    let mut items: Array<i32> = Array.new<i32>()
    items.push(1)
    for item in items {
    }
}
"#;

    let err = parse_inline(source).unwrap_err();
    assert_eq!(err.code, "E0302");
    assert!(err.message.contains("item"));
}

#[test]
fn accepts_generic_enum_instantiation_and_match_binding() {
    let source = r#"package app.main

import std.io

enum Option<T> {
    Some(T)
    None
}

fn unwrap_or_zero(value: Option<i64>) -> i64 {
    return match value {
        Option.Some(n) => n
        Option.None => 0
    }
}

fn main() -> void {
    let value: Option<i64> = Option.Some(41)
    let answer: i64 = unwrap_or_zero(value) + 1
    io.println("done")
}
"#;

    let program = parse_inline(source).unwrap();
    assert_eq!(program.enums[0].type_params, vec!["T"]);
    let unwrap = program
        .functions
        .iter()
        .find(|function| function.name == "unwrap_or_zero")
        .unwrap();
    assert_eq!(
        unwrap.params[0].value_type,
        ValueType::Enum("Option".to_string(), vec![ValueType::Int])
    );
}

#[test]
fn accepts_unqualified_option_and_result_prelude_variants() {
    let source = r#"package app.main

fn parse() -> Result<i32, string> {
    return Ok(41)
}

fn label(value: Option<i32>) -> string {
    return match value {
        Some(number) => if number == 41 {
            "some"
        } else {
            "other"
        }
        None => "none"
    }
}

fn main() -> Result<void, string> {
    let value: i32 = parse()?
    let maybe: Option<i32> = Some(value)
    let text: string = label(maybe)
    return Ok(void)
}
"#;

    let program = parse_inline(source).unwrap();
    let parse = program
        .functions
        .iter()
        .find(|function| function.name == "parse")
        .unwrap();
    assert!(matches!(
        parse.body[0],
        Statement::Return(Some(ValueExpr::EnumVariant {
            ref enum_name,
            ref variant,
            ..
        })) if enum_name == "Result" && variant == "Ok"
    ));
    let label = program
        .functions
        .iter()
        .find(|function| function.name == "label")
        .unwrap();
    assert!(matches!(
        label.body[0],
        Statement::Return(Some(ValueExpr::Match { ref arms, .. }))
            if arms[0].enum_name == "Option"
                && arms[0].variant == "Some"
                && arms[1].variant == "None"
    ));
}

#[test]
fn accepts_let_else_with_option_payload_binding() {
    let source = r#"package app.main

fn unwrap_or_fallback(value: Option<string>) -> string {
    let Some(text) = value else {
        return "fallback"
    }
    return text
}

fn main() -> void {
}
"#;

    let program = parse_inline(source).unwrap();
    let unwrap = program
        .functions
        .iter()
        .find(|function| function.name == "unwrap_or_fallback")
        .unwrap();
    assert!(matches!(
        unwrap.body[0],
        Statement::LetElse {
            ref binding,
            ref value_type,
            ref enum_name,
            ref variant,
            ..
        } if binding == "text"
            && value_type == &ValueType::String
            && enum_name == "Option"
            && variant == "Some"
    ));
    assert!(matches!(
        unwrap.body[1],
        Statement::Return(Some(ValueExpr::Variable(ref name))) if name == "text"
    ));
}

#[test]
fn rejects_let_else_with_non_diverging_else_body() {
    let source = r#"package app.main

fn main() -> void {
    let value: Option<i32> = None
    let Some(number) = value else {
        let fallback: i32 = 0
    }
}
"#;

    let err = parse_inline(source).unwrap_err();
    assert_eq!(err.code, "E0521");
    assert!(err.message.contains("must diverge"));
}

#[test]
fn accepts_if_let_with_option_payload_binding() {
    let source = r#"package app.main

fn label(value: Option<string>) -> string {
    if let Some(text) = value {
        return text
    } else {
        return "missing"
    }
}

fn main() -> void {
}
"#;

    let program = parse_inline(source).unwrap();
    let label = program
        .functions
        .iter()
        .find(|function| function.name == "label")
        .unwrap();
    assert!(matches!(
        label.body[0],
        Statement::IfLet {
            ref binding,
            ref value_type,
            ref enum_name,
            ref variant,
            ref else_body,
            ..
        } if binding.as_deref() == Some("text")
            && value_type.as_ref() == Some(&ValueType::String)
            && enum_name == "Option"
            && variant == "Some"
            && matches!(else_body.as_deref(), Some([Statement::Return(Some(ValueExpr::StringLiteral(_)))]))
    ));
    let Statement::IfLet { body, .. } = &label.body[0] else {
        panic!("expected if-let statement");
    };
    assert!(matches!(
        body.as_slice(),
        [Statement::Return(Some(ValueExpr::Variable(name)))] if name == "text"
    ));
}

#[test]
fn accepts_if_let_with_unit_variant() {
    let source = r#"package app.main

fn is_missing(value: Option<string>) -> bool {
    if let None = value {
        return true
    }
    return false
}

fn main() -> void {
}
"#;

    let program = parse_inline(source).unwrap();
    let is_missing = program
        .functions
        .iter()
        .find(|function| function.name == "is_missing")
        .unwrap();
    assert!(matches!(
        is_missing.body[0],
        Statement::IfLet {
            ref binding,
            ref value_type,
            ref variant,
            ..
        } if binding.is_none() && value_type.is_none() && variant == "None"
    ));
}

#[test]
fn accepts_question_in_pattern_scrutinees() {
    let source = r#"package app.main

fn load() -> Result<Option<string>, string> {
    return Ok(Some("value"))
}

fn unwrap_with_let_else() -> Result<string, string> {
    let Some(text) = load()? else {
        return Err("missing")
    }
    return Ok(text)
}

fn unwrap_with_if_let() -> Result<string, string> {
    if let Some(text) = load()? {
        return Ok(text)
    } else {
        return Err("missing")
    }
}

fn unwrap_with_match() -> Result<string, string> {
    match load()? {
        Some(text) => {
            return Ok(text)
        }
        None => {
            return Err("missing")
        }
    }
}

fn main() -> void {
}
"#;

    let program = parse_inline(source).unwrap();
    let unwrap_with_let_else = program
        .functions
        .iter()
        .find(|function| function.name == "unwrap_with_let_else")
        .unwrap();
    assert!(matches!(
        unwrap_with_let_else.body.as_slice(),
        [
            Statement::QuestionLet {
                name: temp,
                result_expr: ValueExpr::Call { name: call_name, .. },
                ..
            },
            Statement::LetElse {
                value: ValueExpr::Variable(value),
                binding,
                ..
            },
            Statement::Return(Some(_)),
        ] if temp.starts_with("__question_value_")
            && call_name == "load"
            && value == temp
            && binding == "text"
    ));

    let unwrap_with_if_let = program
        .functions
        .iter()
        .find(|function| function.name == "unwrap_with_if_let")
        .unwrap();
    assert!(matches!(
        unwrap_with_if_let.body.as_slice(),
        [
            Statement::QuestionLet {
                name: temp,
                result_expr: ValueExpr::Call { name: call_name, .. },
                ..
            },
            Statement::IfLet {
                value: ValueExpr::Variable(value),
                binding: Some(binding),
                ..
            },
        ] if temp.starts_with("__question_value_")
            && call_name == "load"
            && value == temp
            && binding == "text"
    ));

    let unwrap_with_match = program
        .functions
        .iter()
        .find(|function| function.name == "unwrap_with_match")
        .unwrap();
    assert!(matches!(
        unwrap_with_match.body.as_slice(),
        [
            Statement::QuestionLet {
                name: temp,
                result_expr: ValueExpr::Call { name: call_name, .. },
                ..
            },
            Statement::Match {
                value: ValueExpr::Variable(value),
                enum_name,
                arms,
                ..
            },
        ] if temp.starts_with("__question_value_")
            && call_name == "load"
            && value == temp
            && enum_name == "Option"
            && arms.len() == 2
    ));
}

#[test]
fn rejects_if_let_binding_outside_body() {
    let source = r#"package app.main

fn main() -> void {
    let value: Option<string> = Some("inner")
    if let Some(text) = value {
    }
    let copy: string = text
}
"#;

    let err = parse_inline(source).unwrap_err();
    assert_eq!(err.code, "E0303");
    assert!(err.message.contains("text"));
}

#[test]
fn unqualified_variant_does_not_target_user_enum() {
    let source = r#"package app.main

enum MaybeInt {
    Some(i32)
    None
}

fn main() -> void {
    let value: MaybeInt = Some(1)
}
"#;

    let err = parse_inline(source).unwrap_err();
    assert_eq!(err.code, "E0324");
    assert!(err.message.contains("Option.Some"));
}

#[test]
fn function_name_shadows_unqualified_prelude_variant() {
    let source = r#"package app.main

fn Ok(value: i32) -> i32 {
    return value
}

fn main() -> void {
    let value: i32 = Ok(1)
}
"#;

    let program = parse_inline(source).unwrap();
    let main = program.functions.iter().find(|f| f.name == "main").unwrap();
    assert!(matches!(
        main.body[0],
        Statement::Let {
            initializer: ValueExpr::Call { ref name, .. },
            ..
        } if name == "Ok"
    ));
}

#[test]
fn local_binding_shadows_unqualified_prelude_variant_call() {
    let source = r#"package app.main

fn main() -> void {
    let Ok: i32 = 1
    let value: Result<i32, string> = Ok(2)
}
"#;

    let err = parse_inline(source).unwrap_err();
    assert_eq!(err.code, "E0305");
    assert!(err.message.contains("local variable `Ok` is not callable"));
}

#[test]
fn local_binding_shadows_unqualified_prelude_variant_pattern() {
    let source = r#"package app.main

fn main() -> void {
    let Some: string = "shadow"
    let value: Option<i32> = Option.Some(1)
    let label: string = match value {
        Some(number) => "some"
        None => "none"
    }
}
"#;

    let err = parse_inline(source).unwrap_err();
    assert_eq!(err.code, "E0316");
    assert!(err.message.contains("Option.Variant"));
}

#[test]
fn qualified_core_variant_still_works_when_local_name_shadows_prelude() {
    let source = r#"package app.main

import std.option

fn main() -> void {
    let Some: string = "shadow"
    let value: Option<i32> = Option.Some(1)
    let label: string = match value {
        Option.Some(number) => "some"
        Option.None => "none"
    }
}
"#;

    parse_inline(source).unwrap();
}

#[test]
fn accepts_result_map_err_with_question_propagation() {
    let source = r#"package app.main

import std.result

struct AppError {
    message: string
}

fn parse_label() -> Result<string, string> {
    return Err("bad")
}

fn app_error_from_string(message: string) -> AppError {
    return AppError { message: message }
}

fn decorate_label() -> Result<string, AppError> {
    let raw: Result<string, string> = parse_label()
    let label: string = raw.map_err(app_error_from_string)?
    return Ok(label)
}

fn main() -> void {
}
"#;

    let program = parse_inline(source).unwrap();
    let decorate = program
        .functions
        .iter()
        .find(|function| function.name == "decorate_label")
        .unwrap();
    assert!(matches!(
        decorate.body[1],
        Statement::QuestionLet {
            ref result_type,
            result_expr: ValueExpr::ResultMapErr {
                ref ok_type,
                ref source_err_type,
                ref target_err_type,
                ref converter,
                ..
            },
            ..
        } if result_type == &ValueType::Enum(
                "Result".to_string(),
                vec![
                    ValueType::String,
                    ValueType::Struct("AppError".to_string(), Vec::new())
                ]
            )
            && ok_type == &ValueType::String
            && source_err_type == &ValueType::String
            && target_err_type == &ValueType::Struct("AppError".to_string(), Vec::new())
            && converter == "app_error_from_string"
    ));
}

#[test]
fn accepts_specific_result_map_err_import() {
    let source = r#"package app.main

import std.result.Result
import std.result.map_err

struct AppError {
    message: string
}

fn parse_label() -> Result<string, string> {
    return Err("bad")
}

fn app_error_from_string(message: string) -> AppError {
    return AppError { message: message }
}

fn decorate_label() -> Result<string, AppError> {
    let raw: Result<string, string> = parse_label()
    let label: string = raw.map_err(app_error_from_string)?
    return Ok(label)
}

fn main() -> void {
}
"#;

    let program = parse_inline(source).unwrap();
    let decorate = program
        .functions
        .iter()
        .find(|function| function.name == "decorate_label")
        .unwrap();
    assert!(matches!(
        decorate.body[1],
        Statement::QuestionLet {
            result_expr: ValueExpr::ResultMapErr {
                ref converter,
                ..
            },
            ..
        } if converter == "app_error_from_string"
    ));
}

#[test]
fn accepts_result_value_methods() {
    let source = r#"package app.main

import std.result
import std.string

fn exclaim(text: string) -> string {
    return text.concat("!")
}

fn decorate(text: string) -> Result<string, string> {
    return Ok(text.concat(" ok"))
}

fn main() -> void {
    let ok: Result<string, string> = Ok("seed")
    let err: Result<string, string> = Err("bad")
    let present: bool = ok.is_ok()
    let absent: bool = err.is_err()
    let fallback: string = err.unwrap_or("fallback")
    let mapped: Result<string, string> = ok.map(exclaim)
    let chained: Result<string, string> = ok.and_then(decorate)
}
"#;

    let program = parse_inline(source).unwrap();
    let main = program
        .functions
        .iter()
        .find(|function| function.name == "main")
        .unwrap();
    assert!(matches!(
        main.body[2],
        Statement::Let {
            ref value_type,
            initializer: ValueExpr::ResultIsOk { .. },
            ..
        } if value_type == &ValueType::Bool
    ));
    assert!(matches!(
        main.body[4],
        Statement::Let {
            ref value_type,
            initializer: ValueExpr::ResultUnwrapOr { .. },
            ..
        } if value_type == &ValueType::String
    ));
    assert!(matches!(
        main.body[5],
        Statement::Let {
            ref value_type,
            initializer: ValueExpr::ResultMap { .. },
            ..
        } if value_type == &ValueType::Enum(
            "Result".to_string(),
            vec![ValueType::String, ValueType::String]
        )
    ));
    assert!(matches!(
        main.body[6],
        Statement::Let {
            ref value_type,
            initializer: ValueExpr::ResultAndThen { .. },
            ..
        } if value_type == &ValueType::Enum(
            "Result".to_string(),
            vec![ValueType::String, ValueType::String]
        )
    ));
}

#[test]
fn accepts_specific_result_helper_imports() {
    let source = r#"package app.main

import std.result.Result
import std.result.is_ok
import std.result.is_err
import std.result.unwrap_or
import std.result.map
import std.result.map_err
import std.result.and_then
import std.string

struct AppError {
    message: string
}

fn exclaim(text: string) -> string {
    return text.concat("!")
}

fn decorate(text: string) -> Result<string, string> {
    return Ok(text.concat(" ok"))
}

fn app_error_from_string(message: string) -> AppError {
    return AppError { message: message }
}

fn main() -> void {
    let ok: Result<string, string> = Ok("seed")
    let err: Result<string, string> = Err("bad")
    let present: bool = is_ok(ok)
    let absent: bool = is_err(err)
    let fallback: string = unwrap_or(err, "fallback")
    let mapped: Result<string, string> = map(ok, exclaim)
    let converted: Result<string, AppError> = map_err(err, app_error_from_string)
    let chained: Result<string, string> = and_then(ok, decorate)
}
"#;

    parse_inline(source).unwrap();
}

#[test]
fn accepts_result_module_helpers() {
    let source = r#"package app.main

import std.result
import std.string

struct AppError {
    message: string
}

fn exclaim(text: string) -> string {
    return text.concat("!")
}

fn decorate(text: string) -> Result<string, string> {
    return Ok(text.concat(" ok"))
}

fn app_error_from_string(message: string) -> AppError {
    return AppError { message: message }
}

fn main() -> void {
    let ok: Result<string, string> = Ok("seed")
    let err: Result<string, string> = Err("bad")
    let present: bool = result.is_ok(ok)
    let absent: bool = result.is_err(err)
    let fallback: string = result.unwrap_or(err, "fallback")
    let mapped: Result<string, string> = result.map(ok, exclaim)
    let converted: Result<string, AppError> = result.map_err(err, app_error_from_string)
    let chained: Result<string, string> = result.and_then(ok, decorate)
}
"#;

    parse_inline(source).unwrap();
}

#[test]
fn accepts_option_value_methods() {
    let source = r#"package app.main

import std.option
import std.string

fn exclaim(text: string) -> string {
    return text.concat("!")
}

fn decorate(text: string) -> Option<string> {
    return Some(text.concat(" ok"))
}

fn main() -> void {
    let some: Option<string> = Some("seed")
    let none: Option<string> = None
    let present: bool = some.is_some()
    let absent: bool = none.is_none()
    let fallback: string = none.unwrap_or("fallback")
    let mapped: Option<string> = some.map(exclaim)
    let chained: Option<string> = some.and_then(decorate)
}
"#;

    let program = parse_inline(source).unwrap();
    let main = program
        .functions
        .iter()
        .find(|function| function.name == "main")
        .unwrap();
    assert!(matches!(
        main.body[2],
        Statement::Let {
            ref value_type,
            initializer: ValueExpr::OptionIsSome { .. },
            ..
        } if value_type == &ValueType::Bool
    ));
    assert!(matches!(
        main.body[4],
        Statement::Let {
            ref value_type,
            initializer: ValueExpr::OptionUnwrapOr { .. },
            ..
        } if value_type == &ValueType::String
    ));
    assert!(matches!(
        main.body[5],
        Statement::Let {
            ref value_type,
            initializer: ValueExpr::OptionMap { .. },
            ..
        } if value_type == &ValueType::Enum("Option".to_string(), vec![ValueType::String])
    ));
    assert!(matches!(
        main.body[6],
        Statement::Let {
            ref value_type,
            initializer: ValueExpr::OptionAndThen { .. },
            ..
        } if value_type == &ValueType::Enum("Option".to_string(), vec![ValueType::String])
    ));
}

#[test]
fn accepts_specific_option_helper_imports() {
    let source = r#"package app.main

import std.option.Option
import std.option.is_some
import std.option.is_none
import std.option.unwrap_or
import std.option.map
import std.option.and_then
import std.string

fn exclaim(text: string) -> string {
    return text.concat("!")
}

fn decorate(text: string) -> Option<string> {
    return Some(text.concat(" ok"))
}

fn main() -> void {
    let some: Option<string> = Some("seed")
    let none: Option<string> = None
    let present: bool = is_some(some)
    let absent: bool = is_none(none)
    let fallback: string = unwrap_or(none, "fallback")
    let mapped: Option<string> = map(some, exclaim)
    let chained: Option<string> = and_then(some, decorate)
}
"#;

    parse_inline(source).unwrap();
}

#[test]
fn accepts_option_module_helpers() {
    let source = r#"package app.main

import std.option
import std.string

fn exclaim(text: string) -> string {
    return text.concat("!")
}

fn decorate(text: string) -> Option<string> {
    return Some(text.concat(" ok"))
}

fn main() -> void {
    let some: Option<string> = Some("seed")
    let none: Option<string> = None
    let present: bool = option.is_some(some)
    let absent: bool = option.is_none(none)
    let fallback: string = option.unwrap_or(none, "fallback")
    let mapped: Option<string> = option.map(some, exclaim)
    let chained: Option<string> = option.and_then(some, decorate)
}
"#;

    parse_inline(source).unwrap();
}

#[test]
fn rejects_option_method_without_option_import() {
    let source = r#"package app.main

import std.option.Option

fn main() -> void {
    let some: Option<string> = Some("seed")
    let present: bool = some.is_some()
}
"#;

    let err = parse_inline(source).unwrap_err();
    assert_eq!(err.code, "E0301");
    assert!(err.message.contains("std.option"));
}

#[test]
fn accepts_question_in_let_initializer_call_argument() {
    let source = r#"package app.main

fn parse_label() -> Result<string, string> {
    return Ok("value")
}

fn decorate(value: string) -> string {
    return value
}

fn compute() -> Result<string, string> {
    let label: string = decorate(parse_label()?)
    return Ok(label)
}

fn main() -> void {
}
"#;

    let program = parse_inline(source).unwrap();
    let compute = program
        .functions
        .iter()
        .find(|function| function.name == "compute")
        .unwrap();
    assert!(matches!(
        compute.body.as_slice(),
        [
            Statement::QuestionLet {
                name,
                value_type,
                result_type,
                result_expr: ValueExpr::Call { name: call_name, .. },
                ..
            },
            Statement::Let {
                name: label_name,
                value_type: label_type,
                initializer: ValueExpr::Call { args, .. },
            },
            Statement::Return(Some(_)),
        ] if name.starts_with("__question_value_")
            && value_type == &ValueType::String
            && result_type == &ValueType::Enum(
                "Result".to_string(),
                vec![ValueType::String, ValueType::String]
            )
            && call_name == "parse_label"
            && label_name == "label"
            && label_type == &ValueType::String
            && matches!(args.as_slice(), [ValueExpr::Variable(arg)] if arg == name)
    ));
}

#[test]
fn accepts_question_in_struct_literal_field_and_enum_payload() {
    let source = r#"package app.main

struct Label {
    value: string
}

fn parse_label() -> Result<string, string> {
    return Ok("value")
}

fn compute() -> Result<Label, string> {
    let label: Label = Label { value: parse_label()? }
    return Ok(Label { value: parse_label()? })
}

fn main() -> void {
}
"#;

    let program = parse_inline(source).unwrap();
    let compute = program
        .functions
        .iter()
        .find(|function| function.name == "compute")
        .unwrap();
    assert_eq!(
        compute
            .body
            .iter()
            .filter(|stmt| matches!(stmt, Statement::QuestionLet { .. }))
            .count(),
        2
    );
    assert!(matches!(
        &compute.body[1],
        Statement::Let {
            initializer: ValueExpr::StructLiteral { fields, .. },
            ..
        } if matches!(fields.as_slice(), [(field, ValueExpr::Variable(_))] if field == "value")
    ));
    assert!(matches!(
        &compute.body[3],
        Statement::Return(Some(ValueExpr::EnumVariant {
            payload: Some(payload),
            ..
        })) if matches!(payload.as_ref(), ValueExpr::StructLiteral { fields, .. }
            if matches!(fields.as_slice(), [(field, ValueExpr::Variable(_))] if field == "value"))
    ));
}

#[test]
fn accepts_question_in_binary_cast_and_return_ok_call_argument() {
    let source = r#"package app.main

fn parse_number() -> Result<i32, string> {
    return Ok(1)
}

fn wrap(value: i32) -> i32 {
    return value
}

fn compute() -> Result<i32, string> {
    let total: i32 = parse_number()? + parse_number()? as i32
    return Ok(wrap(parse_number()?))
}

fn main() -> void {
}
"#;

    let program = parse_inline(source).unwrap();
    let compute = program
        .functions
        .iter()
        .find(|function| function.name == "compute")
        .unwrap();
    assert_eq!(
        compute
            .body
            .iter()
            .filter(|stmt| matches!(stmt, Statement::QuestionLet { .. }))
            .count(),
        3
    );
    assert!(matches!(
        &compute.body[2],
        Statement::Let {
            initializer: ValueExpr::Binary { left, right, .. },
            ..
        } if matches!(left.as_ref(), ValueExpr::Variable(_))
            && matches!(right.as_ref(), ValueExpr::Cast { expr, .. }
                if matches!(expr.as_ref(), ValueExpr::Variable(_)))
    ));
    assert!(matches!(
        &compute.body[4],
        Statement::Return(Some(ValueExpr::EnumVariant {
            payload: Some(payload),
            ..
        })) if matches!(payload.as_ref(), ValueExpr::Call { args, .. }
            if matches!(args.as_slice(), [ValueExpr::Variable(_)]))
    ));
}

#[test]
fn accepts_question_in_if_initializer_branch() {
    let source = r#"package app.main

fn parse_label() -> Result<string, string> {
    return Ok("value")
}

fn compute(flag: bool) -> Result<string, string> {
    let label: string = if flag {
        parse_label()?
    } else {
        "fallback"
    }
    return Ok(label)
}

fn main() -> void {
}
"#;

    let program = parse_inline(source).unwrap();
    let compute = program
        .functions
        .iter()
        .find(|function| function.name == "compute")
        .unwrap();
    assert!(matches!(
        compute.body.as_slice(),
        [
            Statement::LetIf {
                name,
                value_type,
                condition: ValueExpr::Variable(condition),
                body,
                else_body,
            },
            Statement::Return(Some(_)),
        ] if name == "label"
            && value_type == &ValueType::String
            && condition == "flag"
            && matches!(
                body.as_slice(),
                [
                    Statement::QuestionLet {
                        name: temp,
                        result_expr: ValueExpr::Call { name: call_name, .. },
                        ..
                    },
                    Statement::Assign {
                        name: assign_name,
                        value: ValueExpr::Variable(assign_value),
                    },
                ] if temp.starts_with("__question_value_")
                    && call_name == "parse_label"
                    && assign_name == "label"
                    && assign_value == temp
            )
            && matches!(
                else_body.as_slice(),
                [Statement::Assign {
                    name: assign_name,
                    value: ValueExpr::StringLiteral(value),
                }] if assign_name == "label" && value == "fallback"
            )
    ));
}

#[test]
fn accepts_question_in_if_initializer_condition() {
    let source = r#"package app.main

fn parse_flag() -> Result<bool, string> {
    return Ok(true)
}

fn compute() -> Result<string, string> {
    let label: string = if parse_flag()? {
        "value"
    } else {
        "fallback"
    }
    return Ok(label)
}

fn main() -> void {
}
"#;

    let program = parse_inline(source).unwrap();
    let compute = program
        .functions
        .iter()
        .find(|function| function.name == "compute")
        .unwrap();
    assert!(matches!(
        compute.body.as_slice(),
        [
            Statement::QuestionLet {
                name: temp,
                result_expr: ValueExpr::Call { name: call_name, .. },
                ..
            },
            Statement::LetIf {
                name,
                condition: ValueExpr::Variable(condition),
                ..
            },
            Statement::Return(Some(_)),
        ] if temp.starts_with("__question_value_")
            && call_name == "parse_flag"
            && name == "label"
            && condition == temp
    ));
}

#[test]
fn accepts_question_in_tail_if_expression_branch() {
    let source = r#"package app.main

fn parse_label() -> Result<string, string> {
    return Ok("value")
}

fn compute(flag: bool) -> Result<string, string> {
    if flag {
        Ok(parse_label()?)
    } else {
        Ok("fallback")
    }
}

fn main() -> void {
}
"#;

    let program = parse_inline(source).unwrap();
    let compute = program
        .functions
        .iter()
        .find(|function| function.name == "compute")
        .unwrap();
    assert!(matches!(
        compute.body.as_slice(),
        [Statement::If {
            condition: ValueExpr::Variable(condition),
            body,
            else_body,
        }] if condition == "flag"
            && matches!(
                body.as_slice(),
                [Statement::QuestionReturn {
                    result_expr: ValueExpr::Call { name: call_name, .. },
                    ..
                }] if call_name == "parse_label"
            )
            && matches!(
                else_body.as_slice(),
                [Statement::Return(Some(ValueExpr::EnumVariant {
                    payload: Some(payload),
                    ..
                }))] if matches!(payload.as_ref(), ValueExpr::StringLiteral(value) if value == "fallback")
            )
    ));
}

#[test]
fn accepts_question_in_tail_if_expression_condition() {
    let source = r#"package app.main

fn parse_flag() -> Result<bool, string> {
    return Ok(true)
}

fn compute() -> Result<string, string> {
    if parse_flag()? {
        Ok("value")
    } else {
        Ok("fallback")
    }
}

fn main() -> void {
}
"#;

    let program = parse_inline(source).unwrap();
    let compute = program
        .functions
        .iter()
        .find(|function| function.name == "compute")
        .unwrap();
    assert!(matches!(
        compute.body.as_slice(),
        [
            Statement::QuestionLet {
                name: temp,
                result_expr: ValueExpr::Call { name: call_name, .. },
                ..
            },
            Statement::If {
                condition: ValueExpr::Variable(condition),
                ..
            },
        ] if temp.starts_with("__question_value_")
            && call_name == "parse_flag"
            && condition == temp
    ));
}

#[test]
fn accepts_question_in_explicit_return_if_expression() {
    let source = r#"package app.main

fn parse_flag() -> Result<bool, string> {
    return Ok(true)
}

fn parse_label() -> Result<string, string> {
    return Ok("value")
}

fn compute() -> Result<string, string> {
    return if parse_flag()? {
        Ok(parse_label()?)
    } else {
        Ok("fallback")
    }
}

fn main() -> void {
}
"#;

    let program = parse_inline(source).unwrap();
    let compute = program
        .functions
        .iter()
        .find(|function| function.name == "compute")
        .unwrap();
    assert!(matches!(
        compute.body.as_slice(),
        [
            Statement::QuestionLet {
                name: condition_temp,
                result_expr: ValueExpr::Call { name: condition_call, .. },
                ..
            },
            Statement::If {
                condition: ValueExpr::Variable(condition_name),
                body,
                else_body,
            },
        ] if condition_temp.starts_with("__question_value_")
            && condition_call == "parse_flag"
            && condition_name == condition_temp
            && matches!(
                body.as_slice(),
                [Statement::QuestionReturn {
                    result_expr: ValueExpr::Call { name: branch_call, .. },
                    ..
                }] if branch_call == "parse_label"
            )
            && matches!(
                else_body.as_slice(),
                [Statement::Return(Some(ValueExpr::EnumVariant {
                    variant,
                    ..
                }))] if variant == "Ok"
            )
    ));
}

#[test]
fn accepts_question_in_return_ok_if_expression() {
    let source = r#"package app.main

fn parse_flag() -> Result<bool, string> {
    return Ok(true)
}

fn parse_label() -> Result<string, string> {
    return Ok("value")
}

fn compute() -> Result<string, string> {
    return Ok(if parse_flag()? {
        parse_label()?
    } else {
        "fallback"
    })
}

fn main() -> void {
}
"#;

    let program = parse_inline(source).unwrap();
    let compute = program
        .functions
        .iter()
        .find(|function| function.name == "compute")
        .unwrap();
    assert!(matches!(
        compute.body.as_slice(),
        [
            Statement::QuestionLet {
                name: condition_temp,
                result_expr: ValueExpr::Call { name: condition_call, .. },
                ..
            },
            Statement::If {
                condition: ValueExpr::Variable(condition_name),
                body,
                else_body,
            },
        ] if condition_temp.starts_with("__question_value_")
            && condition_call == "parse_flag"
            && condition_name == condition_temp
            && matches!(
                body.as_slice(),
                [Statement::QuestionReturn {
                    result_expr: ValueExpr::Call { name: branch_call, .. },
                    ..
                }] if branch_call == "parse_label"
            )
            && matches!(
                else_body.as_slice(),
                [Statement::Return(Some(ValueExpr::EnumVariant {
                    variant,
                    ..
                }))] if variant == "Ok"
            )
    ));
}

#[test]
fn accepts_question_in_return_ok_match_expression() {
    let source = r#"package app.main

fn parse_label() -> Result<string, string> {
    return Ok("value")
}

fn maybe_label() -> Result<Option<string>, string> {
    return Ok(None)
}

fn compute() -> Result<string, string> {
    return Ok(match maybe_label()? {
        Some(text) => text
        None => parse_label()?
    })
}

fn main() -> void {
}
"#;

    let program = parse_inline(source).unwrap();
    let compute = program
        .functions
        .iter()
        .find(|function| function.name == "compute")
        .unwrap();
    assert!(matches!(
        compute.body.as_slice(),
        [
            Statement::QuestionLet {
                name: scrutinee_temp,
                result_expr: ValueExpr::Call { name: scrutinee_call, .. },
                ..
            },
            Statement::Match {
                value: ValueExpr::Variable(scrutinee_name),
                enum_name,
                arms,
                ..
            },
        ] if scrutinee_temp.starts_with("__question_value_")
            && scrutinee_call == "maybe_label"
            && scrutinee_name == scrutinee_temp
            && enum_name == "Option"
            && matches!(
                arms.as_slice(),
                [
                    MatchStatementArm {
                        variant: some_variant,
                        binding: Some(binding),
                        body: some_body,
                    },
                    MatchStatementArm {
                        variant: none_variant,
                        binding: None,
                        body: none_body,
                    },
                ] if some_variant == "Some"
                    && binding == "text"
                    && matches!(
                        some_body.as_slice(),
                        [Statement::Return(Some(ValueExpr::EnumVariant {
                            variant,
                            payload: Some(payload),
                            ..
                        }))] if variant == "Ok"
                            && matches!(payload.as_ref(), ValueExpr::EnumPayload { variant, .. } if variant == "Some")
                    )
                    && none_variant == "None"
                    && matches!(
                        none_body.as_slice(),
                        [Statement::QuestionReturn {
                            result_expr: ValueExpr::Call { name: branch_call, .. },
                            ..
                        }] if branch_call == "parse_label"
                    )
            )
    ));
}

#[test]
fn accepts_question_in_tail_match_expression_arm() {
    let source = r#"package app.main

fn parse_label() -> Result<string, string> {
    return Ok("value")
}

fn compute(value: Option<string>) -> Result<string, string> {
    return match value {
        Some(text) => Ok(text)
        None => Ok(parse_label()?)
    }
}

fn main() -> void {
}
"#;

    let program = parse_inline(source).unwrap();
    let compute = program
        .functions
        .iter()
        .find(|function| function.name == "compute")
        .unwrap();
    assert!(matches!(
        compute.body.as_slice(),
        [Statement::Match {
            value: ValueExpr::Variable(value),
            enum_name,
            arms,
            ..
        }] if value == "value"
            && enum_name == "Option"
            && matches!(
                arms.as_slice(),
                [
                    MatchStatementArm {
                        variant: some_variant,
                        binding: Some(binding),
                        body: some_body,
                    },
                    MatchStatementArm {
                        variant: none_variant,
                        binding: None,
                        body: none_body,
                    },
                ] if some_variant == "Some"
                    && binding == "text"
                    && matches!(
                        some_body.as_slice(),
                        [Statement::Return(Some(ValueExpr::EnumVariant {
                            payload: Some(payload),
                            ..
                        }))] if matches!(
                            payload.as_ref(),
                            ValueExpr::EnumPayload {
                                variant,
                                ..
                            } if variant == "Some"
                        )
                    )
                    && none_variant == "None"
                    && matches!(
                        none_body.as_slice(),
                        [Statement::QuestionReturn {
                            result_expr: ValueExpr::Call { name: call_name, .. },
                            ..
                        }] if call_name == "parse_label"
                    )
            )
    ));
}

#[test]
fn accepts_question_in_tail_match_scrutinee() {
    let source = r#"package app.main

fn maybe_label() -> Result<Option<string>, string> {
    return Ok(Some("value"))
}

fn compute() -> Result<string, string> {
    return match maybe_label()? {
        Some(text) => Ok(text)
        None => Ok("fallback")
    }
}

fn main() -> void {
}
"#;

    let program = parse_inline(source).unwrap();
    let compute = program
        .functions
        .iter()
        .find(|function| function.name == "compute")
        .unwrap();
    assert!(matches!(
        compute.body.as_slice(),
        [
            Statement::QuestionLet {
                name: temp,
                result_expr: ValueExpr::Call { name: call_name, .. },
                ..
            },
            Statement::Match {
                value: ValueExpr::Variable(value),
                enum_name,
                ..
            },
        ] if temp.starts_with("__question_value_")
            && call_name == "maybe_label"
            && value == temp
            && enum_name == "Option"
    ));
}

#[test]
fn accepts_question_in_match_initializer_arm() {
    let source = r#"package app.main

fn parse_label() -> Result<string, string> {
    return Ok("value")
}

fn compute(value: Option<string>) -> Result<string, string> {
    let label: string = match value {
        Some(text) => text
        None => parse_label()?
    }
    return Ok(label)
}

fn main() -> void {
}
"#;

    let program = parse_inline(source).unwrap();
    let compute = program
        .functions
        .iter()
        .find(|function| function.name == "compute")
        .unwrap();
    assert!(matches!(
        compute.body.as_slice(),
        [
            Statement::LetMatch {
                name,
                value_type,
                value: ValueExpr::Variable(value),
                enum_name,
                arms,
                ..
            },
            Statement::Return(Some(_)),
        ] if name == "label"
            && value_type == &ValueType::String
            && value == "value"
            && enum_name == "Option"
            && matches!(
                arms.as_slice(),
                [
                    MatchStatementArm {
                        variant: some_variant,
                        binding: Some(binding),
                        body: some_body,
                    },
                    MatchStatementArm {
                        variant: none_variant,
                        binding: None,
                        body: none_body,
                    },
                ] if some_variant == "Some"
                    && binding == "text"
                    && matches!(
                        some_body.as_slice(),
                        [Statement::Assign {
                            name: assign_name,
                            value: ValueExpr::EnumPayload {
                                variant,
                                ..
                            },
                        }] if assign_name == "label" && variant == "Some"
                    )
                    && none_variant == "None"
                    && matches!(
                        none_body.as_slice(),
                        [
                            Statement::QuestionLet {
                                name: temp,
                                result_expr: ValueExpr::Call { name: call_name, .. },
                                ..
                            },
                            Statement::Assign {
                                name: assign_name,
                                value: ValueExpr::Variable(assign_value),
                            },
                        ] if temp.starts_with("__question_value_")
                            && call_name == "parse_label"
                            && assign_name == "label"
                            && assign_value == temp
                    )
            )
    ));
}

#[test]
fn accepts_question_in_match_initializer_scrutinee() {
    let source = r#"package app.main

fn maybe_label() -> Result<Option<string>, string> {
    return Ok(Some("value"))
}

fn compute() -> Result<string, string> {
    let label: string = match maybe_label()? {
        Some(text) => text
        None => "fallback"
    }
    return Ok(label)
}

fn main() -> void {
}
"#;

    let program = parse_inline(source).unwrap();
    let compute = program
        .functions
        .iter()
        .find(|function| function.name == "compute")
        .unwrap();
    assert!(matches!(
        compute.body.as_slice(),
        [
            Statement::QuestionLet {
                name: temp,
                result_expr: ValueExpr::Call { name: call_name, .. },
                ..
            },
            Statement::LetMatch {
                name,
                value: ValueExpr::Variable(value),
                enum_name,
                ..
            },
            Statement::Return(Some(_)),
        ] if temp.starts_with("__question_value_")
            && call_name == "maybe_label"
            && name == "label"
            && value == temp
            && enum_name == "Option"
    ));
}

#[test]
fn rejects_result_map_err_without_result_import() {
    let source = r#"package app.main

struct AppError {
    message: string
}

fn parse_label() -> Result<string, string> {
    return Err("bad")
}

fn app_error_from_string(message: string) -> AppError {
    return AppError { message: message }
}

fn main() -> void {
    let raw: Result<string, string> = parse_label()
    let mapped: Result<string, AppError> = raw.map_err(app_error_from_string)
}
"#;

    let err = parse_inline(source).unwrap_err();
    assert_eq!(err.code, "E0301");
    assert!(err.message.contains("std.result"));
}

#[test]
fn accepts_result_question_let_binding() {
    let source = r#"package app.main

import std.io

enum Result<T, E> {
    Ok(T)
    Err(E)
}

fn parse() -> Result<i64, string> {
    return Result.Ok(41)
}

fn compute() -> Result<i64, string> {
    let value: i64 = parse()?
    return Result.Ok(value + 1)
}

fn main() -> void {
    io.println("done")
}
"#;

    let program = parse_inline(source).unwrap();
    let compute = program
        .functions
        .iter()
        .find(|function| function.name == "compute")
        .unwrap();
    assert!(matches!(
        compute.body[0],
        Statement::QuestionLet {
            ref name,
            value_type: ValueType::Int,
            result_type: ValueType::Enum(ref enum_name, ref enum_args),
            return_type: ValueType::Enum(ref return_name, ref return_args),
            ..
        } if name == "value"
            && enum_name == "Result"
            && enum_args == &vec![ValueType::Int, ValueType::String]
            && return_name == "Result"
            && return_args == &vec![ValueType::Int, ValueType::String]
    ));
}

#[test]
fn accepts_option_question_let_binding() {
    let source = r#"package app.main

fn load() -> Option<string> {
    return Some("value")
}

fn compute() -> Option<string> {
    let text: string = load()?
    return Some(text)
}

fn main() -> void {
}
"#;

    let program = parse_inline(source).unwrap();
    let compute = program
        .functions
        .iter()
        .find(|function| function.name == "compute")
        .unwrap();
    assert!(matches!(
        compute.body[0],
        Statement::QuestionLet {
            carrier: QuestionCarrier::Option,
            ref name,
            value_type: ValueType::String,
            result_type: ValueType::Enum(ref enum_name, ref enum_args),
            return_type: ValueType::Enum(ref return_name, ref return_args),
            ..
        } if name == "text"
            && enum_name == "Option"
            && enum_args == &vec![ValueType::String]
            && return_name == "Option"
            && return_args == &vec![ValueType::String]
    ));
}

#[test]
fn accepts_option_question_return_payload() {
    let source = r#"package app.main

fn load() -> Option<string> {
    return Some("value")
}

fn compute() -> Option<string> {
    return Some(load()?)
}

fn main() -> void {
}
"#;

    let program = parse_inline(source).unwrap();
    let compute = program
        .functions
        .iter()
        .find(|function| function.name == "compute")
        .unwrap();
    assert!(matches!(
        compute.body[0],
        Statement::QuestionReturn {
            carrier: QuestionCarrier::Option,
            ok_type: ValueType::String,
            result_type: ValueType::Enum(ref result_name, ref result_args),
            return_type: ValueType::Enum(ref return_name, ref return_args),
            result_expr: ValueExpr::Call { ref name, .. },
        } if result_name == "Option"
            && result_args == &vec![ValueType::String]
            && return_name == "Option"
            && return_args == &vec![ValueType::String]
            && name == "load"
    ));
}

#[test]
fn accepts_question_in_result_ok_return_payload() {
    let source = r#"package app.main

fn parse() -> Result<i64, string> {
    return Ok(41)
}

fn compute() -> Result<i64, string> {
    return Ok(parse()?)
}

fn main() -> void {
}
"#;

    let program = parse_inline(source).unwrap();
    let compute = program
        .functions
        .iter()
        .find(|function| function.name == "compute")
        .unwrap();
    assert!(matches!(
        compute.body[0],
        Statement::QuestionReturn {
            carrier: QuestionCarrier::Result,
            ok_type: ValueType::Int,
            result_type: ValueType::Enum(ref result_name, ref result_args),
            return_type: ValueType::Enum(ref return_name, ref return_args),
            result_expr: ValueExpr::Call { ref name, .. },
        } if result_name == "Result"
            && result_args == &vec![ValueType::Int, ValueType::String]
            && return_name == "Result"
            && return_args == &vec![ValueType::Int, ValueType::String]
            && name == "parse"
    ));
}

#[test]
fn question_in_shadowed_ok_call_is_not_treated_as_result_variant() {
    let source = r#"package app.main

fn Ok(value: i64) -> Result<i64, string> {
    return Result.Ok(value)
}

fn parse() -> Result<i64, string> {
    return Result.Ok(41)
}

fn compute() -> Result<i64, string> {
    return Ok(parse()?)
}

fn main() -> void {
}
"#;

    let program = parse_inline(source).unwrap();
    let compute = program
        .functions
        .iter()
        .find(|function| function.name == "compute")
        .unwrap();
    assert!(matches!(
        compute.body.as_slice(),
        [
            Statement::QuestionLet {
                name,
                result_expr: ValueExpr::Call { name: parse_name, .. },
                ..
            },
            Statement::Return(Some(ValueExpr::Call { name: ok_name, args })),
        ] if name.starts_with("__question_value_")
            && parse_name == "parse"
            && ok_name == "Ok"
            && matches!(args.as_slice(), [ValueExpr::Variable(arg)] if arg == name)
    ));
}

#[test]
fn accepts_result_void_ok() {
    let source = r#"package app.main

import std.io

enum Result<T, E> {
    Ok(T)
    Err(E)
}

fn write() -> Result<void, string> {
    return Result.Ok(void)
}

fn main() -> void {
    io.println("done")
}
"#;

    let program = parse_inline(source).unwrap();
    let write = program
        .functions
        .iter()
        .find(|function| function.name == "write")
        .unwrap();
    assert_eq!(
        write.return_type,
        ValueType::Enum(
            "Result".to_string(),
            vec![ValueType::Void, ValueType::String]
        )
    );
    assert!(matches!(
        write.body[0],
        Statement::Return(Some(ValueExpr::EnumVariant {
            payload: Some(ref payload),
            ..
        })) if payload.as_ref() == &ValueExpr::VoidLiteral
    ));
}

#[test]
fn rejects_question_in_non_result_function() {
    let source = r#"package app.main

import std.io

enum Result<T, E> {
    Ok(T)
    Err(E)
}

fn parse() -> Result<i64, string> {
    return Result.Ok(41)
}

fn main() -> void {
    let value: i64 = parse()?
    io.println("done")
}
"#;

    let err = parse_inline(source).unwrap_err();
    assert_eq!(err.code, "E0421");
}

#[test]
fn rejects_question_let_without_type_annotation() {
    let source = r#"package app.main

import std.io

enum Result<T, E> {
    Ok(T)
    Err(E)
}

fn parse() -> Result<i64, string> {
    return Result.Ok(41)
}

fn compute() -> Result<i64, string> {
    let value = parse()?
    return Result.Ok(value + 1)
}

fn main() -> void {
    io.println("done")
}
"#;

    let err = parse_inline(source).unwrap_err();
    assert_eq!(err.code, "E0403");
}

#[test]
fn rejects_missing_payload_binding_in_match() {
    let source = r#"package app.main

import std.io

enum MaybeInt {
    Some(i64)
    None
}

fn unwrap_or_zero(value: MaybeInt) -> i64 {
    return match value {
        MaybeInt.Some => 1
        MaybeInt.None => 0
    }
}

fn main() -> void {
    io.println("done")
}
"#;

    let err = parse_inline(source).unwrap_err();
    assert_eq!(err.code, "E0321");
}

#[test]
fn rejects_missing_main() {
    let source = "package app.main\nimport std.io\n";
    let err = parse_inline(source).unwrap_err();
    assert_eq!(err.code, "E0201");
}

#[test]
fn accepts_script_body_as_synthesized_main_in_script_mode() {
    let source = "package app.main\n\nlet value: i32 = 1\n";
    let program = check_script_source_text(Path::new("script.nomo"), source).unwrap();
    let main = program
        .functions
        .iter()
        .find(|function| function.name == "main")
        .unwrap();

    assert!(main.params.is_empty());
    assert_eq!(main.return_type, ValueType::Void);
    assert!(matches!(
        main.body.as_slice(),
        [Statement::Let { name, value_type: ValueType::I32, .. }] if name == "value"
    ));
}

#[test]
fn rejects_top_level_script_body_outside_script_mode() {
    let source = "package app.main\n\nlet value: i32 = 1\n";
    let err = parse_inline(source).unwrap_err();

    assert_eq!(err.code, "E0201");
    assert!(err.message.contains("top-level script statements"));
}

#[test]
fn rejects_script_body_with_explicit_main_in_script_mode() {
    let source = "package app.main\n\nfn main() -> void {\n}\n\nlet value: i32 = 1\n";
    let err = check_script_source_text(Path::new("script.nomo"), source).unwrap_err();

    assert_eq!(err.code, "E0201");
    assert!(err.message.contains("explicit `main`"));
}

#[test]
fn rejects_missing_io_import() {
    let source = r#"package app.main

fn main() -> void {
    io.println("Hello")
}
"#;
    let err = parse_inline(source).unwrap_err();
    assert_eq!(err.code, "E0301");
    assert_eq!(err.suggestions.len(), 1);
    assert_eq!(err.suggestions[0].text, "import std.io\n");
    assert!(err.suggestions[0].description.contains("io.println"));
}

#[test]
fn rejects_unqualified_println_without_specific_import() {
    let source = r#"package app.main

import std.io

fn main() -> void {
    println("Hello")
}
"#;
    let err = parse_inline(source).unwrap_err();
    assert_eq!(err.code, "E0301");
    assert!(err.message.contains("std.io.println"));
    assert_eq!(err.suggestions.len(), 1);
    assert_eq!(err.suggestions[0].text, "import std.io.println\n");
    assert!(err.suggestions[0].description.contains("println"));
}

#[test]
fn rejects_unqualified_print_without_specific_import() {
    let source = r#"package app.main

import std.io

fn main() -> void {
    print("Hello")
}
"#;
    let err = parse_inline(source).unwrap_err();
    assert_eq!(err.code, "E0301");
    assert!(err.message.contains("std.io.print"));
    assert_eq!(err.suggestions.len(), 1);
    assert_eq!(err.suggestions[0].text, "import std.io.print\n");
    assert!(err.suggestions[0].description.contains("print"));
}

#[test]
fn rejects_unqualified_string_len_without_specific_import() {
    let source = r#"package app.main

import std.io
import std.string

fn main() -> void {
    let size: u64 = len("Nomo")
    io.println("done")
}
"#;
    let err = parse_inline(source).unwrap_err();
    assert_eq!(err.code, "E0305");
    assert!(err.message.contains("len"));
}

#[test]
fn accepts_for_while_iterate_and_infinite() {
    let source = r#"package app.main

import std.array
import std.io

fn main() -> void {
    let mut i: i32 = 0
    for i < 2 {
        i = i + 1
    }
    let mut nums: Array<i32> = Array.new<i32>()
    nums.push(1)
    for n in nums {
        io.println("item")
    }
    for {
        break
    }
}
"#;
    parse_inline(source).unwrap();
}

#[test]
fn accepts_question_in_for_in_iterable() {
    let source = r#"package app.main

import std.array

fn make_items() -> Result<Array<i32>, string> {
    let mut items: Array<i32> = Array.new<i32>()
    items.push(1)
    return Ok(items)
}

fn sum_items() -> Result<i32, string> {
    let mut total: i32 = 0
    for item in make_items()? {
        total = total + item
    }
    return Ok(total)
}

fn main() -> void {
}
"#;

    let program = parse_inline(source).unwrap();
    let sum_items = program
        .functions
        .iter()
        .find(|function| function.name == "sum_items")
        .unwrap();
    assert!(matches!(
        sum_items.body.as_slice(),
        [
            Statement::Let { name: total_name, .. },
            Statement::QuestionLet {
                name: temp,
                result_expr: ValueExpr::Call { name: call_name, .. },
                ..
            },
            Statement::Loop {
                kind: LoopKind::Iterate {
                    binding,
                    iterable: ValueExpr::Variable(iterable),
                    ..
                },
                ..
            },
            Statement::Return(Some(_)),
        ] if total_name == "total"
            && temp.starts_with("__question_value_")
            && call_name == "make_items"
            && binding == "item"
            && iterable == temp
    ));
}

#[test]
fn accepts_question_in_for_while_condition() {
    let source = r#"package app.main

fn should_continue() -> Result<bool, string> {
    return Ok(true)
}

fn compute() -> Result<void, string> {
    for should_continue()? {
        break
    }
    return Ok(void)
}

fn main() -> void {
}
"#;

    let program = parse_inline(source).unwrap();
    let compute = program
        .functions
        .iter()
        .find(|function| function.name == "compute")
        .unwrap();
    assert!(matches!(
        compute.body.as_slice(),
        [
            Statement::Loop {
                kind: LoopKind::Infinite,
                body,
            },
            Statement::Return(Some(_)),
        ] if matches!(
            body.as_slice(),
            [
                Statement::QuestionLet {
                    name: temp,
                    result_expr: ValueExpr::Call { name: call_name, .. },
                    ..
                },
                Statement::If {
                    condition: ValueExpr::Variable(condition),
                    body: then_body,
                    else_body,
                },
            ] if temp.starts_with("__question_value_")
                && call_name == "should_continue"
                && condition == temp
                && matches!(then_body.as_slice(), [Statement::Break])
                && matches!(else_body.as_slice(), [Statement::Break])
        )
    ));
}

#[test]
fn accepts_question_in_assignment_value() {
    let source = r#"package app.main

fn parse_label() -> Result<string, string> {
    return Ok("value")
}

fn compute() -> Result<string, string> {
    let mut label: string = "initial"
    label = parse_label()?
    return Ok(label)
}

fn main() -> void {
}
"#;

    let program = parse_inline(source).unwrap();
    let compute = program
        .functions
        .iter()
        .find(|function| function.name == "compute")
        .unwrap();
    assert!(matches!(
        compute.body.as_slice(),
        [
            Statement::Let { name: label_name, .. },
            Statement::QuestionLet {
                name: temp,
                result_expr: ValueExpr::Call { name: call_name, .. },
                ..
            },
            Statement::Assign {
                name: assign_name,
                value: ValueExpr::Variable(value_name),
            },
            Statement::Return(Some(_)),
        ] if label_name == "label"
            && temp.starts_with("__question_value_")
            && call_name == "parse_label"
            && assign_name == "label"
            && value_name == temp
    ));
}

#[test]
fn accepts_question_in_field_assignment_value() {
    let source = r#"package app.main

struct Label {
    value: string
}

fn parse_label() -> Result<string, string> {
    return Ok("value")
}

fn compute() -> Result<string, string> {
    let mut label: Label = Label { value: "initial" }
    label.value = parse_label()?
    return Ok(label.value)
}

fn main() -> void {
}
"#;

    let program = parse_inline(source).unwrap();
    let compute = program
        .functions
        .iter()
        .find(|function| function.name == "compute")
        .unwrap();
    assert!(matches!(
        compute.body.as_slice(),
        [
            Statement::Let { name: label_name, .. },
            Statement::QuestionLet {
                name: temp,
                result_expr: ValueExpr::Call { name: call_name, .. },
                ..
            },
            Statement::AssignField {
                base,
                field,
                value: ValueExpr::Variable(value_name),
                ..
            },
            Statement::Return(Some(_)),
        ] if label_name == "label"
            && temp.starts_with("__question_value_")
            && call_name == "parse_label"
            && base == "label"
            && field == "value"
            && value_name == temp
    ));
}

#[test]
fn accepts_question_in_if_assignment_branch() {
    let source = r#"package app.main

fn parse_label() -> Result<string, string> {
    return Ok("value")
}

fn should_use_label() -> Result<bool, string> {
    return Ok(true)
}

fn compute() -> Result<string, string> {
    let mut label: string = "initial"
    label = if should_use_label()? {
        parse_label()?
    } else {
        "fallback"
    }
    return Ok(label)
}

fn main() -> void {
}
"#;

    let program = parse_inline(source).unwrap();
    let compute = program
        .functions
        .iter()
        .find(|function| function.name == "compute")
        .unwrap();
    assert!(matches!(
        compute.body.as_slice(),
        [
            Statement::Let { name: label_name, .. },
            Statement::QuestionLet {
                name: condition_temp,
                result_expr: ValueExpr::Call { name: condition_call, .. },
                ..
            },
            Statement::If {
                condition: ValueExpr::Variable(condition_name),
                body,
                else_body,
            },
            Statement::Return(Some(_)),
        ] if label_name == "label"
            && condition_temp.starts_with("__question_value_")
            && condition_call == "should_use_label"
            && condition_name == condition_temp
            && matches!(
                body.as_slice(),
                [
                    Statement::QuestionLet {
                        name: branch_temp,
                        result_expr: ValueExpr::Call { name: branch_call, .. },
                        ..
                    },
                    Statement::Assign {
                        name: assign_name,
                        value: ValueExpr::Variable(assign_value),
                    },
                ] if branch_temp.starts_with("__question_value_")
                    && branch_call == "parse_label"
                    && assign_name == "label"
                    && assign_value == branch_temp
            )
            && matches!(
                else_body.as_slice(),
                [Statement::Assign {
                    name: assign_name,
                    value: ValueExpr::StringLiteral(value),
                }] if assign_name == "label" && value == "fallback"
            )
    ));
}

#[test]
fn accepts_question_in_match_assignment_arm() {
    let source = r#"package app.main

fn parse_label() -> Result<string, string> {
    return Ok("value")
}

fn maybe_label() -> Result<Option<string>, string> {
    return Ok(None)
}

fn compute() -> Result<string, string> {
    let mut label: string = "initial"
    label = match maybe_label()? {
        Some(text) => text
        None => parse_label()?
    }
    return Ok(label)
}

fn main() -> void {
}
"#;

    let program = parse_inline(source).unwrap();
    let compute = program
        .functions
        .iter()
        .find(|function| function.name == "compute")
        .unwrap();
    assert!(matches!(
        compute.body.as_slice(),
        [
            Statement::Let { name: label_name, .. },
            Statement::QuestionLet {
                name: scrutinee_temp,
                result_expr: ValueExpr::Call { name: scrutinee_call, .. },
                ..
            },
            Statement::Match {
                value: ValueExpr::Variable(scrutinee_name),
                enum_name,
                arms,
                ..
            },
            Statement::Return(Some(_)),
        ] if label_name == "label"
            && scrutinee_temp.starts_with("__question_value_")
            && scrutinee_call == "maybe_label"
            && scrutinee_name == scrutinee_temp
            && enum_name == "Option"
            && matches!(
                arms.as_slice(),
                [
                    MatchStatementArm {
                        variant: some_variant,
                        binding: Some(binding),
                        body: some_body,
                    },
                    MatchStatementArm {
                        variant: none_variant,
                        binding: None,
                        body: none_body,
                    },
                ] if some_variant == "Some"
                    && binding == "text"
                    && matches!(
                        some_body.as_slice(),
                        [Statement::Assign {
                            name: assign_name,
                            value: ValueExpr::EnumPayload { variant, .. },
                        }] if assign_name == "label" && variant == "Some"
                    )
                    && none_variant == "None"
                    && matches!(
                        none_body.as_slice(),
                        [
                            Statement::QuestionLet {
                                name: branch_temp,
                                result_expr: ValueExpr::Call { name: branch_call, .. },
                                ..
                            },
                            Statement::Assign {
                                name: assign_name,
                                value: ValueExpr::Variable(assign_value),
                            },
                        ] if branch_temp.starts_with("__question_value_")
                            && branch_call == "parse_label"
                            && assign_name == "label"
                            && assign_value == branch_temp
                    )
            )
    ));
}

#[test]
fn accepts_question_in_void_expression_statement_argument() {
    let source = r#"package app.main

import std.array

fn parse_label() -> Result<string, string> {
    return Ok("value")
}

fn collect() -> Result<void, string> {
    let mut values: Array<string> = Array.new<string>()
    values.push(parse_label()?)
    return Ok(void)
}

fn main() -> void {
}
"#;

    let program = parse_inline(source).unwrap();
    let collect = program
        .functions
        .iter()
        .find(|function| function.name == "collect")
        .unwrap();
    assert!(matches!(
        collect.body.as_slice(),
        [
            Statement::Let { name: values_name, .. },
            Statement::QuestionLet {
                name: temp,
                result_expr: ValueExpr::Call { name: call_name, .. },
                ..
            },
            Statement::Assign {
                name: assign_name,
                value: ValueExpr::ArrayPush { value, .. },
            },
            Statement::Return(Some(_)),
        ] if values_name == "values"
            && temp.starts_with("__question_value_")
            && call_name == "parse_label"
            && assign_name == "values"
            && matches!(value.as_ref(), ValueExpr::Variable(name) if name == temp)
    ));
}

#[test]
fn accepts_question_in_defer_call_argument() {
    let source = r#"package app.main

fn parse_label() -> Result<string, string> {
    return Ok("value")
}

fn consume(value: string) -> void {
}

fn compute() -> Result<void, string> {
    defer consume(parse_label()?)
    return Ok(void)
}

fn main() -> void {
}
"#;

    let program = parse_inline(source).unwrap();
    let compute = program
        .functions
        .iter()
        .find(|function| function.name == "compute")
        .unwrap();
    assert!(matches!(
        compute.body.as_slice(),
        [
            Statement::QuestionLet {
                name: temp,
                result_expr: ValueExpr::Call { name: call_name, .. },
                ..
            },
            Statement::Defer {
                call: DeferredCall::Expr(ValueExpr::Call { name: consume_name, args }),
            },
            Statement::Return(Some(_)),
        ] if temp.starts_with("__question_value_")
            && call_name == "parse_label"
            && consume_name == "consume"
            && matches!(args.as_slice(), [ValueExpr::Variable(name)] if name == temp)
    ));
}

#[test]
fn accepts_break_and_continue_in_loop() {
    let source = r#"package app.main

fn main() -> void {
    for {
        break
    }
    for {
        continue
    }
}
"#;
    parse_inline(source).unwrap();
}

#[test]
fn accepts_nested_loop_break() {
    let source = r#"package app.main

fn main() -> void {
    for {
        for {
            break
        }
        break
    }
}
"#;
    parse_inline(source).unwrap();
}

#[test]
fn rejects_break_outside_loop() {
    let source = "package app.main\nfn main() -> void {\n    break\n}\n";
    let err = parse_inline(source).unwrap_err();
    assert_eq!(err.code, "E0510");
}

#[test]
fn rejects_continue_outside_loop() {
    let source = "package app.main\nfn main() -> void {\n    continue\n}\n";
    let err = parse_inline(source).unwrap_err();
    assert_eq!(err.code, "E0511");
}

#[test]
fn accepts_defer_inside_loop() {
    let source = "package app.main\nimport std.io\nfn main() -> void {\n    for {\n        defer io.println(\"cleanup\")\n        break\n    }\n}\n";
    let program = parse_inline(source).unwrap();
    let Statement::Loop { body, .. } = &program.functions[0].body[0] else {
        panic!("expected loop");
    };
    assert!(matches!(body[0], Statement::Defer { .. }));
    assert!(matches!(body[1], Statement::Break));
}

#[test]
fn rejects_defer_non_expression() {
    let source = "package app.main\nfn main() -> void {\n    defer return\n}\n";
    let err = parse_inline(source).unwrap_err();
    assert_eq!(err.code, "E0265");
}

#[test]
fn accepts_defer_method_call() {
    let source = r#"package app.main

import std.io

struct R {
    pub id: i32
}

impl R {
    pub fn close(self) -> void {
        io.println("closed")
    }
}

fn main() -> void {
    let r: R = R { id: 1 }
    defer r.close()
    io.println("working")
}
"#;
    parse_inline(source).unwrap();
}

#[test]
fn accepts_defer_io_print_calls() {
    let source = r#"package app.main

import std.io

fn main() -> void {
    defer io.println("cleanup")
    defer io.eprintln("error cleanup")
    io.println("working")
}
"#;
    let program = parse_inline(source).unwrap();
    let main = program
        .functions
        .iter()
        .find(|function| function.name == "main")
        .unwrap();
    assert!(matches!(
        main.body[0],
        Statement::Defer {
            call: DeferredCall::Println(_)
        }
    ));
    assert!(matches!(
        main.body[1],
        Statement::Defer {
            call: DeferredCall::Eprintln(_)
        }
    ));
}

#[test]
fn accepts_defer_specific_print_import() {
    let source = r#"package app.main

import std.io.println

fn main() -> void {
    defer println("cleanup")
    println("working")
}
"#;
    parse_inline(source).unwrap();
}

#[test]
fn rejects_defer_io_print_without_import() {
    let source = r#"package app.main

fn main() -> void {
    defer io.println("cleanup")
}
"#;
    let err = parse_inline(source).unwrap_err();
    assert_eq!(err.code, "E0301");
}

#[test]
fn accepts_package_const_reference() {
    let source = r#"package app.main

import std.io

const LIMIT: i32 = 5
const NAME: string = "nomo"

fn main() -> void {
    let mut i: i32 = 0
    for i < LIMIT {
        i = i + 1
    }
    io.println(NAME)
}
"#;
    let program = parse_inline(source).unwrap();
    assert_eq!(program.consts.len(), 2);
    assert_eq!(program.consts[0].name, "LIMIT");
    assert_eq!(program.consts[1].name, "NAME");
}

#[test]
fn rejects_const_non_literal_initializer() {
    let source = "package app.main\nfn one() -> i32 {\n    return 1\n}\nconst X: i32 = one()\nfn main() -> void {\n}\n";
    let err = parse_inline(source).unwrap_err();
    assert_eq!(err.code, "E0430");
}

#[test]
fn rejects_const_duplicate() {
    let source = "package app.main\nconst A: i32 = 1\nconst A: i32 = 2\nfn main() -> void {\n}\n";
    let err = parse_inline(source).unwrap_err();
    assert_eq!(err.code, "E0304");
}

#[test]
fn rejects_for_in_over_non_array() {
    let source = "package app.main\nfn main() -> void {\n    for n in 5 {\n    }\n}\n";
    let err = parse_inline(source).unwrap_err();
    assert!(err.message.contains("Array"));
}

#[test]
fn rejects_for_iter_binding_outside_loop_body() {
    let source = r#"package app.main

import std.array
import std.io

fn main() -> void {
    let mut words: Array<string> = Array.new<string>()
    words.push("hello")
    for word in words {
        io.println(word)
    }
    io.println(word)
}
"#;
    let err = parse_inline(source).unwrap_err();
    assert_eq!(err.code, "E0303");
    assert!(err.message.contains("word"));
}

#[test]
fn rejects_loop_local_let_outside_loop_body() {
    let source = r#"package app.main

import std.io

fn main() -> void {
    for {
        let message: string = "inside"
        break
    }
    io.println(message)
}
"#;
    let err = parse_inline(source).unwrap_err();
    assert_eq!(err.code, "E0303");
    assert!(err.message.contains("message"));
}

#[test]
fn rejects_for_condition_must_be_bool() {
    let source = "package app.main\nfn main() -> void {\n    for 5 {\n    }\n}\n";
    let err = parse_inline(source).unwrap_err();
    assert!(err.message.contains("bool"));
}
