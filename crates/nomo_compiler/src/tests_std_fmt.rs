use super::*;

#[test]
fn accepts_scalar_formatting_and_compile_time_templates() {
    let source = r#"package app.main

import std.fmt
import std.io

fn main() -> void {
    let message = fmt.format("name={} count={} debug={:?} braces={{}}", "Nomo", 3, true)
    io.println(message)
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
                value_type: ValueType::String,
                initializer: ValueExpr::StringConcat { .. },
                ..
            },
            Statement::Println(ValueExpr::Variable(name)),
        ] if name == "message"
    ));
}

#[test]
fn accepts_specific_format_function_imports() {
    let source = r#"package app.main

import std.fmt.format
import std.fmt.to_string

fn main() -> void {
    let first = format("{}", 7)
    let second = to_string(false)
}
"#;

    let program = parse_inline(source).unwrap();
    let main = program
        .functions
        .iter()
        .find(|function| function.name == "main")
        .unwrap();
    assert_eq!(main.body.len(), 2);
    assert!(main.body.iter().all(|statement| matches!(
        statement,
        Statement::Let {
            value_type: ValueType::String,
            ..
        }
    )));
}

#[test]
fn display_and_debug_interfaces_format_user_structs_and_io_values() {
    let source = r#"package app.main

import std.fmt
import std.io

struct User {
    name: string
}

impl fmt.Display for User {
    fn to_string(self) -> string {
        return self.name
    }
}

impl fmt.Debug for User {
    fn debug_string(self) -> string {
        return self.name
    }
}

fn main() -> void {
    let user: User = User { name: "Nomo" }
    let message = fmt.format("display={} debug={:?}", user, user)
    io.println(user)
    io.println(message)
}
"#;

    let program = parse_inline(source).unwrap();
    let main = program
        .functions
        .iter()
        .find(|function| function.name == "main")
        .unwrap();
    assert!(
        program
            .functions
            .iter()
            .any(|function| function.name == "User_to_string")
    );
    assert!(
        program
            .functions
            .iter()
            .any(|function| function.name == "User_debug_string")
    );
    assert!(matches!(
        &main.body[2],
        Statement::Println(ValueExpr::Call { name, .. }) if name == "User_to_string"
    ));
}

#[test]
fn rejects_struct_without_required_format_interface() {
    let source = r#"package app.main

import std.fmt

struct User {
    name: string
}

fn main() -> void {
    let user: User = User { name: "Nomo" }
    let message = fmt.to_string(user)
}
"#;

    let error = parse_inline(source).unwrap_err();
    assert_eq!(error.code, "E0402");
    assert!(error.message.contains("std.fmt.Display"), "{error:?}");
}

#[test]
fn rejects_invalid_format_templates_and_value_counts() {
    for (template, expected) in [
        ("\"value={\"", "unterminated format placeholder"),
        ("\"value={name}\"", "unterminated format placeholder"),
        ("\"value={}\"", "expects 1 value(s), got 0"),
    ] {
        let source = format!(
            "package app.main\n\nimport std.fmt\n\nfn main() -> void {{\n    let value = fmt.format({template})\n}}\n"
        );
        let error = parse_inline(&source).unwrap_err();
        assert_eq!(error.code, "E0408");
        assert!(error.message.contains(expected), "{error:?}");
    }
}

#[test]
fn rejects_dynamic_format_templates() {
    let source = r#"package app.main

import std.fmt

fn main() -> void {
    let template = "{}"
    let value = fmt.format(template, 1)
}
"#;

    let error = parse_inline(source).unwrap_err();
    assert_eq!(error.code, "E0408");
    assert!(
        error.message.contains("compile-time string literal"),
        "{error:?}"
    );
}
