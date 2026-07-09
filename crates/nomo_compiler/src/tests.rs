use super::*;

#[path = "tests_arrays.rs"]
mod tests_arrays;
#[path = "tests_result_option_question.rs"]
mod tests_result_option_question;
#[path = "tests_std_builtins.rs"]
mod tests_std_builtins;

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
