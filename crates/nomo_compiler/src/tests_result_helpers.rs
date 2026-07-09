use super::*;
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
