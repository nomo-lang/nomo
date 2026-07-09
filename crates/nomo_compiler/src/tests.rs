use super::*;

#[path = "tests_arrays.rs"]
mod tests_arrays;
#[path = "tests_assignments.rs"]
mod tests_assignments;
#[path = "tests_enums_patterns.rs"]
mod tests_enums_patterns;
#[path = "tests_expressions.rs"]
mod tests_expressions;
#[path = "tests_nominal_interfaces.rs"]
mod tests_nominal_interfaces;
#[path = "tests_result_option_question.rs"]
mod tests_result_option_question;
#[path = "tests_std_builtins.rs"]
mod tests_std_builtins;
#[path = "tests_std_io_string_path.rs"]
mod tests_std_io_string_path;
#[path = "tests_std_process_debug_crypto.rs"]
mod tests_std_process_debug_crypto;

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
