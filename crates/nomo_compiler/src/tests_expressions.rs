use super::*;

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
