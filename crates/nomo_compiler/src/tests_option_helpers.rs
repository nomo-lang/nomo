use super::*;
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
