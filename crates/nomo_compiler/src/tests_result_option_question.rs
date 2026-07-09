use super::*;

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
