use super::*;

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
