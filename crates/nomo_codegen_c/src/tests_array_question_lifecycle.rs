use super::*;

#[test]
fn emits_array_releases_before_return_and_question_error_exit() {
    let array_i32 = ValueType::Array(Box::new(ValueType::I32));
    let result_i32_string = ValueType::Enum(
        "Result".to_string(),
        vec![ValueType::I32, ValueType::String],
    );
    let program = Program {
        consts: Vec::new(),
        package: "app.main".to_string(),
        imports: vec!["std.array".to_string()],
        extern_functions: Vec::new(),
        structs: Vec::new(),
        enums: vec![
            EnumType {
                package: "app.main".to_string(),
                name: "Option".to_string(),
                type_params: vec!["T".to_string()],
                variants: vec![
                    EnumVariantType {
                        name: "Some".to_string(),
                        payload: Some(ValueType::TypeParam("T".to_string())),
                    },
                    EnumVariantType {
                        name: "None".to_string(),
                        payload: None,
                    },
                ],
            },
            EnumType {
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
            },
        ],
        functions: vec![
            Function {
                package: "app.main".to_string(),
                name: "parse".to_string(),
                params: Vec::new(),
                return_type: result_i32_string.clone(),
                body: vec![Statement::Return(Some(ValueExpr::EnumVariant {
                    enum_name: "Result".to_string(),
                    enum_args: vec![ValueType::I32, ValueType::String],
                    variant: "Ok".to_string(),
                    payload: Some(Box::new(ValueExpr::IntLiteral(7))),
                }))],
            },
            Function {
                package: "app.main".to_string(),
                name: "compute".to_string(),
                params: Vec::new(),
                return_type: result_i32_string,
                body: vec![
                    Statement::Let {
                        name: "items".to_string(),
                        value_type: array_i32.clone(),
                        initializer: ValueExpr::ArrayNew {
                            element_type: ValueType::I32,
                        },
                    },
                    Statement::QuestionLet {
                        carrier: QuestionCarrier::Result,
                        name: "value".to_string(),
                        value_type: ValueType::I32,
                        result_type: ValueType::Enum(
                            "Result".to_string(),
                            vec![ValueType::I32, ValueType::String],
                        ),
                        return_type: ValueType::Enum(
                            "Result".to_string(),
                            vec![ValueType::I32, ValueType::String],
                        ),
                        result_expr: ValueExpr::Call {
                            name: "parse".to_string(),
                            args: Vec::new(),
                        },
                    },
                    Statement::Return(Some(ValueExpr::EnumVariant {
                        enum_name: "Result".to_string(),
                        enum_args: vec![ValueType::I32, ValueType::String],
                        variant: "Ok".to_string(),
                        payload: Some(Box::new(ValueExpr::Variable("value".to_string()))),
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
    let question_error = c.find("if (nomo_value_result.tag").unwrap();
    let question_temp = c[question_error..]
        .find("nomo_enum_Result_i32_string nomo__question_return =")
        .unwrap();
    let release_in_error = c[question_error..]
        .find("nomo_array_i32_release(nomo_items);")
        .unwrap();
    let question_return = c[question_error..]
        .find("return nomo__question_return;")
        .unwrap();
    assert!(question_temp < release_in_error);
    assert!(release_in_error < question_return);
    let ok_return = c.rfind("return nomo__return;").unwrap();
    let release_before_ok = c[..ok_return]
        .rfind("nomo_array_i32_release(nomo_items);")
        .unwrap();
    assert!(release_before_ok < ok_return);
}

#[test]
fn emits_question_return_with_cleanup_on_error_and_success() {
    let array_i32 = ValueType::Array(Box::new(ValueType::I32));
    let result_i32_string = ValueType::Enum(
        "Result".to_string(),
        vec![ValueType::I32, ValueType::String],
    );
    let program = Program {
        consts: Vec::new(),
        package: "app.main".to_string(),
        imports: vec!["std.array".to_string()],
        extern_functions: Vec::new(),
        structs: Vec::new(),
        enums: vec![
            EnumType {
                package: "app.main".to_string(),
                name: "Option".to_string(),
                type_params: vec!["T".to_string()],
                variants: vec![
                    EnumVariantType {
                        name: "Some".to_string(),
                        payload: Some(ValueType::TypeParam("T".to_string())),
                    },
                    EnumVariantType {
                        name: "None".to_string(),
                        payload: None,
                    },
                ],
            },
            EnumType {
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
            },
        ],
        functions: vec![
            Function {
                package: "app.main".to_string(),
                name: "parse".to_string(),
                params: Vec::new(),
                return_type: result_i32_string.clone(),
                body: vec![Statement::Return(Some(ValueExpr::EnumVariant {
                    enum_name: "Result".to_string(),
                    enum_args: vec![ValueType::I32, ValueType::String],
                    variant: "Ok".to_string(),
                    payload: Some(Box::new(ValueExpr::IntLiteral(7))),
                }))],
            },
            Function {
                package: "app.main".to_string(),
                name: "compute".to_string(),
                params: Vec::new(),
                return_type: result_i32_string.clone(),
                body: vec![
                    Statement::Let {
                        name: "items".to_string(),
                        value_type: array_i32.clone(),
                        initializer: ValueExpr::ArrayNew {
                            element_type: ValueType::I32,
                        },
                    },
                    Statement::QuestionReturn {
                        carrier: QuestionCarrier::Result,
                        ok_type: ValueType::I32,
                        result_type: result_i32_string.clone(),
                        return_type: result_i32_string,
                        result_expr: ValueExpr::Call {
                            name: "parse".to_string(),
                            args: Vec::new(),
                        },
                    },
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
    let question_result = c.find("nomo__question_result = nomo_fn_parse();").unwrap();
    let error_branch = c[question_result..]
        .find("if (nomo__question_result.tag == nomo_enum_Result_i32_string_Err)")
        .unwrap();
    let question_return = c[question_result..]
        .find("return nomo__question_return;")
        .unwrap();
    let error_release = c[question_result..question_result + question_return]
        .find("nomo_array_i32_release(nomo_items);")
        .unwrap();
    assert!(error_branch < error_release);

    let ok_temp = c[question_result..]
        .find("int32_t nomo__question_ok = nomo__question_result.payload.nomo_payload_Ok;")
        .unwrap();
    let ok_return = c[question_result..].find("return nomo__return;").unwrap();
    let success_release = c[question_result + ok_temp..question_result + ok_return]
        .find("nomo_array_i32_release(nomo_items);")
        .unwrap();
    assert!(success_release < ok_return - ok_temp);
}

#[test]
fn question_let_retains_managed_payloads_when_result_expr_is_shared() {
    let array_i32 = ValueType::Array(Box::new(ValueType::I32));
    let result_array_array = ValueType::Enum(
        "Result".to_string(),
        vec![array_i32.clone(), array_i32.clone()],
    );
    let program = Program {
        consts: Vec::new(),
        package: "app.main".to_string(),
        imports: vec!["std.array".to_string()],
        extern_functions: Vec::new(),
        structs: Vec::new(),
        enums: vec![
            EnumType {
                package: "app.main".to_string(),
                name: "Option".to_string(),
                type_params: vec!["T".to_string()],
                variants: vec![
                    EnumVariantType {
                        name: "Some".to_string(),
                        payload: Some(ValueType::TypeParam("T".to_string())),
                    },
                    EnumVariantType {
                        name: "None".to_string(),
                        payload: None,
                    },
                ],
            },
            EnumType {
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
            },
        ],
        functions: vec![
            Function {
                package: "app.main".to_string(),
                name: "compute".to_string(),
                params: Vec::new(),
                return_type: result_array_array.clone(),
                body: vec![
                    Statement::Let {
                        name: "items".to_string(),
                        value_type: array_i32.clone(),
                        initializer: ValueExpr::ArrayNew {
                            element_type: ValueType::I32,
                        },
                    },
                    Statement::Let {
                        name: "raw".to_string(),
                        value_type: result_array_array.clone(),
                        initializer: ValueExpr::EnumVariant {
                            enum_name: "Result".to_string(),
                            enum_args: vec![array_i32.clone(), array_i32.clone()],
                            variant: "Ok".to_string(),
                            payload: Some(Box::new(ValueExpr::Variable("items".to_string()))),
                        },
                    },
                    Statement::QuestionLet {
                        carrier: QuestionCarrier::Result,
                        name: "value".to_string(),
                        value_type: array_i32.clone(),
                        result_type: result_array_array.clone(),
                        return_type: result_array_array.clone(),
                        result_expr: ValueExpr::Variable("raw".to_string()),
                    },
                    Statement::Return(Some(ValueExpr::EnumVariant {
                        enum_name: "Result".to_string(),
                        enum_args: vec![array_i32.clone(), array_i32],
                        variant: "Ok".to_string(),
                        payload: Some(Box::new(ValueExpr::Variable("value".to_string()))),
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
    assert!(c.contains("nomo_enum_Result_array_i32_array_i32 nomo_value_result = nomo_raw;"));
    let question_error = c.find("if (nomo_value_result.tag").unwrap();
    let question_return_retain = c[question_error..]
            .find(
                "nomo__question_return = nomo_enum_Result_array_i32_array_i32_retain(nomo__question_return);",
            )
            .unwrap();
    let raw_release = c[question_error..]
        .find("nomo_enum_Result_array_i32_array_i32_release(nomo_raw);")
        .unwrap();
    let question_return = c[question_error..]
        .find("return nomo__question_return;")
        .unwrap();
    assert!(question_return_retain < raw_release);
    assert!(raw_release < question_return);
    assert!(c.contains("nomo_value = nomo_array_i32_retain(nomo_value);"));
}
