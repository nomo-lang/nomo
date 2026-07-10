use super::*;

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
fn accepts_constrained_generic_static_dispatch() {
    let source = r#"package app.main

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

fn render<T: Display>(value: T) -> string {
    return value.to_string()
}

fn main() -> void {
    let user: User = User { name: "bounded" }
    let label: string = render<User>(user)
}
"#;

    let program = parse_inline(source).unwrap();
    let render = program
        .functions
        .iter()
        .find(|function| function.name == "render_struct_User")
        .unwrap();
    assert_eq!(
        render.params[0].value_type,
        ValueType::Struct("User".to_string(), Vec::new())
    );
    assert!(matches!(
        render.body[0],
        Statement::Return(Some(ValueExpr::Call { ref name, .. })) if name == "User_to_string"
    ));
}

#[test]
fn rejects_constrained_generic_type_without_interface_impl() {
    let source = r#"package app.main

interface Display {
    fn to_string(self) -> string
}

struct User {
    name: string
}

fn render<T: Display>(value: T) -> string {
    return "unused"
}

fn main() -> void {
    let user: User = User { name: "missing" }
    let label: string = render<User>(user)
}
"#;

    let error = parse_inline(source).unwrap_err();

    assert_eq!(error.code, "E1506");
    assert!(
        error
            .message
            .contains("type `User` does not implement interface `Display`")
    );
    assert!(error.message.contains("render<T>"));
}

#[test]
fn rejects_unknown_constrained_generic_interface() {
    let source = r#"package app.main

fn render<T: Display>(value: T) -> T {
    return value
}

fn main() -> void {
}
"#;

    let error = parse_inline(source).unwrap_err();

    assert_eq!(error.code, "E1506");
    assert!(error.message.contains("unknown interface bound `Display`"));
}

#[test]
fn rejects_concrete_method_outside_generic_interface_bound() {
    let source = r#"package app.main

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

impl User {
    fn secret(self) -> string {
        return self.name
    }
}

fn render<T: Display>(value: T) -> string {
    return value.secret()
}

fn main() -> void {
    let user: User = User { name: "private ability" }
    let label: string = render<User>(user)
}
"#;

    let error = parse_inline(source).unwrap_err();

    assert_eq!(error.code, "E1506");
    assert!(error.message.contains("unavailable through `T: Display`"));
    assert!(error.message.contains("has no method `secret`"));
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
