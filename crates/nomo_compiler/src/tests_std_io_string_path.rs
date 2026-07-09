use super::*;

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
