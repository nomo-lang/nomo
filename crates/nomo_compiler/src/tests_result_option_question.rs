use super::*;

#[test]
fn accepts_question_in_let_initializer_call_argument() {
    let source = r#"package app.main

fn parse_label() -> Result<string, string> {
    return Ok("value")
}

fn decorate(value: string) -> string {
    return value
}

fn compute() -> Result<string, string> {
    let label: string = decorate(parse_label()?)
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
            Statement::QuestionLet {
                name,
                value_type,
                result_type,
                result_expr: ValueExpr::Call { name: call_name, .. },
                ..
            },
            Statement::Let {
                name: label_name,
                value_type: label_type,
                initializer: ValueExpr::Call { args, .. },
            },
            Statement::Return(Some(_)),
        ] if name.starts_with("__question_value_")
            && value_type == &ValueType::String
            && result_type == &ValueType::Enum(
                "Result".to_string(),
                vec![ValueType::String, ValueType::String]
            )
            && call_name == "parse_label"
            && label_name == "label"
            && label_type == &ValueType::String
            && matches!(args.as_slice(), [ValueExpr::Variable(arg)] if arg == name)
    ));
}

#[test]
fn accepts_question_in_struct_literal_field_and_enum_payload() {
    let source = r#"package app.main

struct Label {
    value: string
}

fn parse_label() -> Result<string, string> {
    return Ok("value")
}

fn compute() -> Result<Label, string> {
    let label: Label = Label { value: parse_label()? }
    return Ok(Label { value: parse_label()? })
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
    assert_eq!(
        compute
            .body
            .iter()
            .filter(|stmt| matches!(stmt, Statement::QuestionLet { .. }))
            .count(),
        2
    );
    assert!(matches!(
        &compute.body[1],
        Statement::Let {
            initializer: ValueExpr::StructLiteral { fields, .. },
            ..
        } if matches!(fields.as_slice(), [(field, ValueExpr::Variable(_))] if field == "value")
    ));
    assert!(matches!(
        &compute.body[3],
        Statement::Return(Some(ValueExpr::EnumVariant {
            payload: Some(payload),
            ..
        })) if matches!(payload.as_ref(), ValueExpr::StructLiteral { fields, .. }
            if matches!(fields.as_slice(), [(field, ValueExpr::Variable(_))] if field == "value"))
    ));
}

#[test]
fn accepts_question_in_binary_cast_and_return_ok_call_argument() {
    let source = r#"package app.main

fn parse_number() -> Result<i32, string> {
    return Ok(1)
}

fn wrap(value: i32) -> i32 {
    return value
}

fn compute() -> Result<i32, string> {
    let total: i32 = parse_number()? + parse_number()? as i32
    return Ok(wrap(parse_number()?))
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
    assert_eq!(
        compute
            .body
            .iter()
            .filter(|stmt| matches!(stmt, Statement::QuestionLet { .. }))
            .count(),
        3
    );
    assert!(matches!(
        &compute.body[2],
        Statement::Let {
            initializer: ValueExpr::Binary { left, right, .. },
            ..
        } if matches!(left.as_ref(), ValueExpr::Variable(_))
            && matches!(right.as_ref(), ValueExpr::Cast { expr, .. }
                if matches!(expr.as_ref(), ValueExpr::Variable(_)))
    ));
    assert!(matches!(
        &compute.body[4],
        Statement::Return(Some(ValueExpr::EnumVariant {
            payload: Some(payload),
            ..
        })) if matches!(payload.as_ref(), ValueExpr::Call { args, .. }
            if matches!(args.as_slice(), [ValueExpr::Variable(_)]))
    ));
}

#[test]
fn accepts_question_in_if_initializer_branch() {
    let source = r#"package app.main

fn parse_label() -> Result<string, string> {
    return Ok("value")
}

fn compute(flag: bool) -> Result<string, string> {
    let label: string = if flag {
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
            Statement::LetIf {
                name,
                value_type,
                condition: ValueExpr::Variable(condition),
                body,
                else_body,
            },
            Statement::Return(Some(_)),
        ] if name == "label"
            && value_type == &ValueType::String
            && condition == "flag"
            && matches!(
                body.as_slice(),
                [
                    Statement::QuestionLet {
                        name: temp,
                        result_expr: ValueExpr::Call { name: call_name, .. },
                        ..
                    },
                    Statement::Assign {
                        name: assign_name,
                        value: ValueExpr::Variable(assign_value),
                    },
                ] if temp.starts_with("__question_value_")
                    && call_name == "parse_label"
                    && assign_name == "label"
                    && assign_value == temp
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
fn accepts_question_in_if_initializer_condition() {
    let source = r#"package app.main

fn parse_flag() -> Result<bool, string> {
    return Ok(true)
}

fn compute() -> Result<string, string> {
    let label: string = if parse_flag()? {
        "value"
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
            Statement::QuestionLet {
                name: temp,
                result_expr: ValueExpr::Call { name: call_name, .. },
                ..
            },
            Statement::LetIf {
                name,
                condition: ValueExpr::Variable(condition),
                ..
            },
            Statement::Return(Some(_)),
        ] if temp.starts_with("__question_value_")
            && call_name == "parse_flag"
            && name == "label"
            && condition == temp
    ));
}

#[test]
fn accepts_question_in_tail_if_expression_branch() {
    let source = r#"package app.main

fn parse_label() -> Result<string, string> {
    return Ok("value")
}

fn compute(flag: bool) -> Result<string, string> {
    if flag {
        Ok(parse_label()?)
    } else {
        Ok("fallback")
    }
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
        [Statement::If {
            condition: ValueExpr::Variable(condition),
            body,
            else_body,
        }] if condition == "flag"
            && matches!(
                body.as_slice(),
                [Statement::QuestionReturn {
                    result_expr: ValueExpr::Call { name: call_name, .. },
                    ..
                }] if call_name == "parse_label"
            )
            && matches!(
                else_body.as_slice(),
                [Statement::Return(Some(ValueExpr::EnumVariant {
                    payload: Some(payload),
                    ..
                }))] if matches!(payload.as_ref(), ValueExpr::StringLiteral(value) if value == "fallback")
            )
    ));
}

#[test]
fn accepts_question_in_tail_if_expression_condition() {
    let source = r#"package app.main

fn parse_flag() -> Result<bool, string> {
    return Ok(true)
}

fn compute() -> Result<string, string> {
    if parse_flag()? {
        Ok("value")
    } else {
        Ok("fallback")
    }
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
            Statement::If {
                condition: ValueExpr::Variable(condition),
                ..
            },
        ] if temp.starts_with("__question_value_")
            && call_name == "parse_flag"
            && condition == temp
    ));
}

#[test]
fn accepts_question_in_explicit_return_if_expression() {
    let source = r#"package app.main

fn parse_flag() -> Result<bool, string> {
    return Ok(true)
}

fn parse_label() -> Result<string, string> {
    return Ok("value")
}

fn compute() -> Result<string, string> {
    return if parse_flag()? {
        Ok(parse_label()?)
    } else {
        Ok("fallback")
    }
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
                name: condition_temp,
                result_expr: ValueExpr::Call { name: condition_call, .. },
                ..
            },
            Statement::If {
                condition: ValueExpr::Variable(condition_name),
                body,
                else_body,
            },
        ] if condition_temp.starts_with("__question_value_")
            && condition_call == "parse_flag"
            && condition_name == condition_temp
            && matches!(
                body.as_slice(),
                [Statement::QuestionReturn {
                    result_expr: ValueExpr::Call { name: branch_call, .. },
                    ..
                }] if branch_call == "parse_label"
            )
            && matches!(
                else_body.as_slice(),
                [Statement::Return(Some(ValueExpr::EnumVariant {
                    variant,
                    ..
                }))] if variant == "Ok"
            )
    ));
}

#[test]
fn accepts_question_in_return_ok_if_expression() {
    let source = r#"package app.main

fn parse_flag() -> Result<bool, string> {
    return Ok(true)
}

fn parse_label() -> Result<string, string> {
    return Ok("value")
}

fn compute() -> Result<string, string> {
    return Ok(if parse_flag()? {
        parse_label()?
    } else {
        "fallback"
    })
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
                name: condition_temp,
                result_expr: ValueExpr::Call { name: condition_call, .. },
                ..
            },
            Statement::If {
                condition: ValueExpr::Variable(condition_name),
                body,
                else_body,
            },
        ] if condition_temp.starts_with("__question_value_")
            && condition_call == "parse_flag"
            && condition_name == condition_temp
            && matches!(
                body.as_slice(),
                [Statement::QuestionReturn {
                    result_expr: ValueExpr::Call { name: branch_call, .. },
                    ..
                }] if branch_call == "parse_label"
            )
            && matches!(
                else_body.as_slice(),
                [Statement::Return(Some(ValueExpr::EnumVariant {
                    variant,
                    ..
                }))] if variant == "Ok"
            )
    ));
}

#[test]
fn accepts_question_in_return_ok_match_expression() {
    let source = r#"package app.main

fn parse_label() -> Result<string, string> {
    return Ok("value")
}

fn maybe_label() -> Result<Option<string>, string> {
    return Ok(None)
}

fn compute() -> Result<string, string> {
    return Ok(match maybe_label()? {
        Some(text) => text
        None => parse_label()?
    })
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
        ] if scrutinee_temp.starts_with("__question_value_")
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
                        [Statement::Return(Some(ValueExpr::EnumVariant {
                            variant,
                            payload: Some(payload),
                            ..
                        }))] if variant == "Ok"
                            && matches!(payload.as_ref(), ValueExpr::EnumPayload { variant, .. } if variant == "Some")
                    )
                    && none_variant == "None"
                    && matches!(
                        none_body.as_slice(),
                        [Statement::QuestionReturn {
                            result_expr: ValueExpr::Call { name: branch_call, .. },
                            ..
                        }] if branch_call == "parse_label"
                    )
            )
    ));
}

#[test]
fn accepts_question_in_tail_match_expression_arm() {
    let source = r#"package app.main

fn parse_label() -> Result<string, string> {
    return Ok("value")
}

fn compute(value: Option<string>) -> Result<string, string> {
    return match value {
        Some(text) => Ok(text)
        None => Ok(parse_label()?)
    }
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
        [Statement::Match {
            value: ValueExpr::Variable(value),
            enum_name,
            arms,
            ..
        }] if value == "value"
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
                        [Statement::Return(Some(ValueExpr::EnumVariant {
                            payload: Some(payload),
                            ..
                        }))] if matches!(
                            payload.as_ref(),
                            ValueExpr::EnumPayload {
                                variant,
                                ..
                            } if variant == "Some"
                        )
                    )
                    && none_variant == "None"
                    && matches!(
                        none_body.as_slice(),
                        [Statement::QuestionReturn {
                            result_expr: ValueExpr::Call { name: call_name, .. },
                            ..
                        }] if call_name == "parse_label"
                    )
            )
    ));
}

#[test]
fn accepts_question_in_tail_match_scrutinee() {
    let source = r#"package app.main

fn maybe_label() -> Result<Option<string>, string> {
    return Ok(Some("value"))
}

fn compute() -> Result<string, string> {
    return match maybe_label()? {
        Some(text) => Ok(text)
        None => Ok("fallback")
    }
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
            Statement::Match {
                value: ValueExpr::Variable(value),
                enum_name,
                ..
            },
        ] if temp.starts_with("__question_value_")
            && call_name == "maybe_label"
            && value == temp
            && enum_name == "Option"
    ));
}

#[test]
fn accepts_question_in_match_initializer_arm() {
    let source = r#"package app.main

fn parse_label() -> Result<string, string> {
    return Ok("value")
}

fn compute(value: Option<string>) -> Result<string, string> {
    let label: string = match value {
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
            Statement::LetMatch {
                name,
                value_type,
                value: ValueExpr::Variable(value),
                enum_name,
                arms,
                ..
            },
            Statement::Return(Some(_)),
        ] if name == "label"
            && value_type == &ValueType::String
            && value == "value"
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
                            value: ValueExpr::EnumPayload {
                                variant,
                                ..
                            },
                        }] if assign_name == "label" && variant == "Some"
                    )
                    && none_variant == "None"
                    && matches!(
                        none_body.as_slice(),
                        [
                            Statement::QuestionLet {
                                name: temp,
                                result_expr: ValueExpr::Call { name: call_name, .. },
                                ..
                            },
                            Statement::Assign {
                                name: assign_name,
                                value: ValueExpr::Variable(assign_value),
                            },
                        ] if temp.starts_with("__question_value_")
                            && call_name == "parse_label"
                            && assign_name == "label"
                            && assign_value == temp
                    )
            )
    ));
}

#[test]
fn accepts_question_in_match_initializer_scrutinee() {
    let source = r#"package app.main

fn maybe_label() -> Result<Option<string>, string> {
    return Ok(Some("value"))
}

fn compute() -> Result<string, string> {
    let label: string = match maybe_label()? {
        Some(text) => text
        None => "fallback"
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
            Statement::QuestionLet {
                name: temp,
                result_expr: ValueExpr::Call { name: call_name, .. },
                ..
            },
            Statement::LetMatch {
                name,
                value: ValueExpr::Variable(value),
                enum_name,
                ..
            },
            Statement::Return(Some(_)),
        ] if temp.starts_with("__question_value_")
            && call_name == "maybe_label"
            && name == "label"
            && value == temp
            && enum_name == "Option"
    ));
}

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
