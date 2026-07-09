use super::*;

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
