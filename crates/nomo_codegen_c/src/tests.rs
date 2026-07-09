use super::*;
use nomo_ir::{EnumVariantType, MatchValueArm, Parameter, StructField, ValueExpr};

#[path = "tests_array_lifecycle.rs"]
mod tests_array_lifecycle;
#[path = "tests_basic_io_symbols.rs"]
mod tests_basic_io_symbols;
#[path = "tests_defer_control.rs"]
mod tests_defer_control;
#[path = "tests_expressions.rs"]
mod tests_expressions;
#[path = "tests_host_helpers.rs"]
mod tests_host_helpers;
#[path = "tests_nominal.rs"]
mod tests_nominal;
#[path = "tests_statements.rs"]
mod tests_statements;
#[path = "tests_std_primitives.rs"]
mod tests_std_primitives;

fn string_literal(value: &str) -> String {
    format!("nomo_string_literal(\"{value}\")")
}

fn puts_literal(value: &str) -> String {
    format!("puts(({}).data);", string_literal(value))
}

fn fputs_literal(value: &str) -> String {
    format!("fputs(({}).data, stderr);", string_literal(value))
}

fn fputs_stdout_literal(value: &str) -> String {
    format!("fputs(({}).data, stdout);", string_literal(value))
}

fn panic_literal(value: &str) -> String {
    format!("nomo_panic(({}).data);", string_literal(value))
}

#[test]
fn emits_result_question_let_early_return() {
    let result_i64_string = ValueType::Enum(
        "Result".to_string(),
        vec![ValueType::Int, ValueType::String],
    );
    let program = Program {
        consts: Vec::new(),
        package: "app.main".to_string(),
        imports: vec!["std.io".to_string()],
        extern_functions: Vec::new(),
        structs: Vec::new(),
        enums: vec![EnumType {
            package: "app.main".to_string(),
            name: "Result".to_string(),
            type_params: vec!["T".to_string(), "E".to_string()],
            variants: vec![
                EnumVariantType {
                    name: "Ok".to_string(),
                    payload: Some(ValueType::TypeParam("T".to_string())),
                },
                EnumVariantType {
                    name: "Err".to_string(),
                    payload: Some(ValueType::TypeParam("E".to_string())),
                },
            ],
        }],
        functions: vec![
            Function {
                package: "app.main".to_string(),
                name: "parse".to_string(),
                params: Vec::new(),
                return_type: result_i64_string.clone(),
                body: vec![Statement::Return(Some(ValueExpr::EnumVariant {
                    enum_name: "Result".to_string(),
                    enum_args: vec![ValueType::Int, ValueType::String],
                    variant: "Ok".to_string(),
                    payload: Some(Box::new(ValueExpr::IntLiteral(41))),
                }))],
            },
            Function {
                package: "app.main".to_string(),
                name: "compute".to_string(),
                params: Vec::new(),
                return_type: result_i64_string.clone(),
                body: vec![
                    Statement::QuestionLet {
                        carrier: QuestionCarrier::Result,
                        name: "value".to_string(),
                        value_type: ValueType::Int,
                        result_type: result_i64_string.clone(),
                        return_type: result_i64_string,
                        result_expr: ValueExpr::Call {
                            name: "parse".to_string(),
                            args: Vec::new(),
                        },
                    },
                    Statement::Return(Some(ValueExpr::EnumVariant {
                        enum_name: "Result".to_string(),
                        enum_args: vec![ValueType::Int, ValueType::String],
                        variant: "Ok".to_string(),
                        payload: Some(Box::new(ValueExpr::Binary {
                            left: Box::new(ValueExpr::Variable("value".to_string())),
                            op: BinaryOp::Add,
                            right: Box::new(ValueExpr::IntLiteral(1)),
                            value_type: ValueType::Int,
                        })),
                    })),
                ],
            },
            Function {
                package: "app.main".to_string(),
                name: "main".to_string(),
                params: Vec::new(),
                return_type: ValueType::Void,
                body: vec![Statement::Println(ValueExpr::StringLiteral(
                    "done".to_string(),
                ))],
            },
        ],
    };

    let c = emit_c(&program);
    assert!(c.contains("nomo_enum_Result_i64_string nomo_value_result = nomo_fn_parse();"));
    assert!(c.contains("if (nomo_value_result.tag == nomo_enum_Result_i64_string_Err) {"));
    assert!(c.contains(
            "nomo_enum_Result_i64_string nomo__question_return = (nomo_enum_Result_i64_string){.tag = nomo_enum_Result_i64_string_Err, .payload.nomo_payload_Err = nomo_value_result.payload.nomo_payload_Err};"
        ));
    assert!(c.contains("return nomo__question_return;"));
    assert!(c.contains("long long nomo_value = nomo_value_result.payload.nomo_payload_Ok;"));
}

#[test]
fn emits_result_void_question_let_without_void_temp() {
    let result_void_string = ValueType::Enum(
        "Result".to_string(),
        vec![ValueType::Void, ValueType::String],
    );
    let program = Program {
        consts: Vec::new(),
        package: "app.main".to_string(),
        imports: vec!["std.io".to_string()],
        extern_functions: Vec::new(),
        structs: Vec::new(),
        enums: vec![EnumType {
            package: "app.main".to_string(),
            name: "Result".to_string(),
            type_params: vec!["T".to_string(), "E".to_string()],
            variants: vec![
                EnumVariantType {
                    name: "Ok".to_string(),
                    payload: Some(ValueType::TypeParam("T".to_string())),
                },
                EnumVariantType {
                    name: "Err".to_string(),
                    payload: Some(ValueType::TypeParam("E".to_string())),
                },
            ],
        }],
        functions: vec![
            Function {
                package: "app.main".to_string(),
                name: "write".to_string(),
                params: Vec::new(),
                return_type: result_void_string.clone(),
                body: vec![Statement::Return(Some(ValueExpr::EnumVariant {
                    enum_name: "Result".to_string(),
                    enum_args: vec![ValueType::Void, ValueType::String],
                    variant: "Ok".to_string(),
                    payload: Some(Box::new(ValueExpr::VoidLiteral)),
                }))],
            },
            Function {
                package: "app.main".to_string(),
                name: "compute".to_string(),
                params: Vec::new(),
                return_type: result_void_string.clone(),
                body: vec![
                    Statement::QuestionLet {
                        carrier: QuestionCarrier::Result,
                        name: "ignored".to_string(),
                        value_type: ValueType::Void,
                        result_type: result_void_string.clone(),
                        return_type: result_void_string.clone(),
                        result_expr: ValueExpr::Call {
                            name: "write".to_string(),
                            args: Vec::new(),
                        },
                    },
                    Statement::Return(Some(ValueExpr::EnumVariant {
                        enum_name: "Result".to_string(),
                        enum_args: vec![ValueType::Void, ValueType::String],
                        variant: "Ok".to_string(),
                        payload: Some(Box::new(ValueExpr::VoidLiteral)),
                    })),
                ],
            },
            Function {
                package: "app.main".to_string(),
                name: "main".to_string(),
                params: Vec::new(),
                return_type: ValueType::Void,
                body: Vec::new(),
            },
        ],
    };

    let c = emit_c(&program);
    assert!(c.contains("char nomo_ignored = nomo_ignored_result.payload.nomo_payload_Ok;"));
    assert!(!c.contains("void nomo_ignored ="));
}

#[test]
fn emits_result_void_question_return_without_void_temp() {
    let result_void_string = ValueType::Enum(
        "Result".to_string(),
        vec![ValueType::Void, ValueType::String],
    );
    let program = Program {
        consts: Vec::new(),
        package: "app.main".to_string(),
        imports: vec!["std.io".to_string()],
        extern_functions: Vec::new(),
        structs: Vec::new(),
        enums: vec![EnumType {
            package: "app.main".to_string(),
            name: "Result".to_string(),
            type_params: vec!["T".to_string(), "E".to_string()],
            variants: vec![
                EnumVariantType {
                    name: "Ok".to_string(),
                    payload: Some(ValueType::TypeParam("T".to_string())),
                },
                EnumVariantType {
                    name: "Err".to_string(),
                    payload: Some(ValueType::TypeParam("E".to_string())),
                },
            ],
        }],
        functions: vec![
            Function {
                package: "app.main".to_string(),
                name: "write".to_string(),
                params: Vec::new(),
                return_type: result_void_string.clone(),
                body: vec![Statement::Return(Some(ValueExpr::EnumVariant {
                    enum_name: "Result".to_string(),
                    enum_args: vec![ValueType::Void, ValueType::String],
                    variant: "Ok".to_string(),
                    payload: Some(Box::new(ValueExpr::VoidLiteral)),
                }))],
            },
            Function {
                package: "app.main".to_string(),
                name: "compute".to_string(),
                params: Vec::new(),
                return_type: result_void_string.clone(),
                body: vec![Statement::QuestionReturn {
                    carrier: QuestionCarrier::Result,
                    ok_type: ValueType::Void,
                    result_type: result_void_string.clone(),
                    return_type: result_void_string,
                    result_expr: ValueExpr::Call {
                        name: "write".to_string(),
                        args: Vec::new(),
                    },
                }],
            },
            Function {
                package: "app.main".to_string(),
                name: "main".to_string(),
                params: Vec::new(),
                return_type: ValueType::Void,
                body: Vec::new(),
            },
        ],
    };

    let c = emit_c(&program);
    assert!(c.contains("char nomo__question_ok = nomo__question_result.payload.nomo_payload_Ok;"));
    assert!(c.contains(".payload.nomo_payload_Ok = nomo__question_ok"));
    assert!(!c.contains("void nomo__question_ok ="));
}
