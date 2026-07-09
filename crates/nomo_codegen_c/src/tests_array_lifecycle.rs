use super::*;

#[test]
fn emits_string_array_helpers() {
    let array_string = ValueType::Array(Box::new(ValueType::String));
    let option_string = ValueType::Enum("Option".to_string(), vec![ValueType::String]);
    let program = Program {
        consts: Vec::new(),
        package: "app.main".to_string(),
        imports: vec!["std.array".to_string()],
        extern_functions: Vec::new(),
        structs: Vec::new(),
        enums: vec![EnumType {
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
        }],
        functions: vec![Function {
            package: "app.main".to_string(),
            name: "main".to_string(),
            params: Vec::new(),
            return_type: ValueType::Void,
            body: vec![
                Statement::Let {
                    name: "items".to_string(),
                    value_type: array_string.clone(),
                    initializer: ValueExpr::ArrayNew {
                        element_type: ValueType::String,
                    },
                },
                Statement::Assign {
                    name: "items".to_string(),
                    value: ValueExpr::ArrayPush {
                        array: "items".to_string(),
                        value: Box::new(ValueExpr::StringLiteral("first".to_string())),
                        element_type: ValueType::String,
                    },
                },
                Statement::Let {
                    name: "size".to_string(),
                    value_type: ValueType::U64,
                    initializer: ValueExpr::ArrayLen {
                        array: Box::new(ValueExpr::Variable("items".to_string())),
                    },
                },
                Statement::Let {
                    name: "first".to_string(),
                    value_type: option_string,
                    initializer: ValueExpr::ArrayGet {
                        array: Box::new(ValueExpr::Variable("items".to_string())),
                        index: Box::new(ValueExpr::IntLiteral(0)),
                        element_type: ValueType::String,
                    },
                },
            ],
        }],
    };

    let c = emit_c(&program);
    assert!(c.contains("typedef struct nomo_array_string"));
    assert!(c.contains("nomo_array_string nomo_items = nomo_array_string_new();"));
    assert!(c.contains(
        "nomo_items = nomo_array_string_push(nomo_items, nomo_string_literal(\"first\"));"
    ));
    assert!(c.contains("uint64_t nomo_size = ((uint64_t)nomo_items.len);"));
    assert!(c.contains("nomo_array_string_get(nomo_items, 0)"));
}

#[test]
fn emits_i32_array_helpers() {
    let array_i32 = ValueType::Array(Box::new(ValueType::I32));
    let option_i32 = ValueType::Enum("Option".to_string(), vec![ValueType::I32]);
    let program = Program {
        consts: Vec::new(),
        package: "app.main".to_string(),
        imports: vec!["std.array".to_string()],
        extern_functions: Vec::new(),
        structs: Vec::new(),
        enums: vec![EnumType {
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
        }],
        functions: vec![Function {
            package: "app.main".to_string(),
            name: "main".to_string(),
            params: Vec::new(),
            return_type: ValueType::Void,
            body: vec![
                Statement::Let {
                    name: "items".to_string(),
                    value_type: array_i32.clone(),
                    initializer: ValueExpr::ArrayNew {
                        element_type: ValueType::I32,
                    },
                },
                Statement::Assign {
                    name: "items".to_string(),
                    value: ValueExpr::ArrayPush {
                        array: "items".to_string(),
                        value: Box::new(ValueExpr::IntLiteral(7)),
                        element_type: ValueType::I32,
                    },
                },
                Statement::Assign {
                    name: "items".to_string(),
                    value: ValueExpr::ArrayInsert {
                        array: "items".to_string(),
                        index: Box::new(ValueExpr::IntLiteral(0)),
                        value: Box::new(ValueExpr::IntLiteral(5)),
                        element_type: ValueType::I32,
                    },
                },
                Statement::Let {
                    name: "removed".to_string(),
                    value_type: option_i32.clone(),
                    initializer: ValueExpr::ArrayRemove {
                        array: "items".to_string(),
                        index: Box::new(ValueExpr::IntLiteral(0)),
                        element_type: ValueType::I32,
                    },
                },
                Statement::Let {
                    name: "popped".to_string(),
                    value_type: option_i32.clone(),
                    initializer: ValueExpr::ArrayPop {
                        array: "items".to_string(),
                        element_type: ValueType::I32,
                    },
                },
                Statement::Let {
                    name: "snapshot".to_string(),
                    value_type: array_i32.clone(),
                    initializer: ValueExpr::ArrayIter {
                        array: Box::new(ValueExpr::Variable("items".to_string())),
                        element_type: ValueType::I32,
                    },
                },
                Statement::Assign {
                    name: "items".to_string(),
                    value: ValueExpr::ArrayClear {
                        array: "items".to_string(),
                        element_type: ValueType::I32,
                    },
                },
                Statement::Let {
                    name: "first".to_string(),
                    value_type: option_i32,
                    initializer: ValueExpr::ArrayGet {
                        array: Box::new(ValueExpr::Variable("items".to_string())),
                        index: Box::new(ValueExpr::IntLiteral(0)),
                        element_type: ValueType::I32,
                    },
                },
            ],
        }],
    };

    let c = emit_c(&program);
    assert!(c.contains("typedef struct nomo_array_i32"));
    assert!(c.contains("int32_t *data;"));
    assert!(c.contains("size_t *refcount;"));
    assert!(c.contains("static nomo_array_i32 nomo_array_i32_retain(nomo_array_i32 array)"));
    assert!(c.contains("static void nomo_array_i32_release(nomo_array_i32 array)"));
    assert!(c.contains("if (*array.refcount != 0) { return; }"));
    assert!(c.contains("free(array.data);"));
    assert!(c.contains("free(array.refcount);"));
    assert!(c.contains(
        "static nomo_array_i32 nomo_array_i32_make_unique(nomo_array_i32 array, size_t needed)"
    ));
    assert!(c.contains("array = nomo_array_i32_make_unique(array, array.len + 1);"));
    assert!(c.contains("array = nomo_array_i32_make_unique(array, array.len);"));
    assert!(c.contains("static nomo_array_i32 nomo_array_i32_insert("));
    assert!(c.contains("static nomo_array_i32 nomo_array_i32_clear("));
    assert!(c.contains("static nomo_enum_Option_i32 nomo_array_i32_pop("));
    assert!(c.contains("static nomo_enum_Option_i32 nomo_array_i32_remove("));
    assert!(c.contains("nomo_array_i32 nomo_items = nomo_array_i32_new();"));
    assert!(c.contains("nomo_items = nomo_array_i32_push(nomo_items, 7);"));
    assert!(c.contains("nomo_items = nomo_array_i32_insert(nomo_items, 0, 5);"));
    assert!(c.contains("nomo_array_i32_remove(&nomo_items, 0)"));
    assert!(c.contains("nomo_array_i32_pop(&nomo_items)"));
    assert!(c.contains("nomo_array_i32 nomo_snapshot = nomo_array_i32_retain(nomo_items);"));
    assert!(c.contains("nomo_items = nomo_array_i32_clear(nomo_items);"));
    assert!(c.contains("nomo_array_i32_get(nomo_items, 0)"));
}

#[test]
fn emits_array_retain_for_shared_array_bindings_and_nested_elements() {
    let array_i32 = ValueType::Array(Box::new(ValueType::I32));
    let array_array_i32 = ValueType::Array(Box::new(array_i32.clone()));
    let option_array_i32 = ValueType::Enum("Option".to_string(), vec![array_i32.clone()]);
    let program = Program {
        consts: Vec::new(),
        package: "app.main".to_string(),
        imports: vec!["std.array".to_string()],
        extern_functions: Vec::new(),
        structs: Vec::new(),
        enums: vec![EnumType {
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
        }],
        functions: vec![Function {
            package: "app.main".to_string(),
            name: "main".to_string(),
            params: Vec::new(),
            return_type: ValueType::Void,
            body: vec![
                Statement::Let {
                    name: "inner".to_string(),
                    value_type: array_i32.clone(),
                    initializer: ValueExpr::ArrayNew {
                        element_type: ValueType::I32,
                    },
                },
                Statement::Let {
                    name: "snapshot".to_string(),
                    value_type: array_i32.clone(),
                    initializer: ValueExpr::Variable("inner".to_string()),
                },
                Statement::Let {
                    name: "outer".to_string(),
                    value_type: array_array_i32,
                    initializer: ValueExpr::ArrayNew {
                        element_type: array_i32.clone(),
                    },
                },
                Statement::Assign {
                    name: "outer".to_string(),
                    value: ValueExpr::ArrayPush {
                        array: "outer".to_string(),
                        value: Box::new(ValueExpr::Variable("inner".to_string())),
                        element_type: array_i32.clone(),
                    },
                },
                Statement::Let {
                    name: "first".to_string(),
                    value_type: option_array_i32,
                    initializer: ValueExpr::ArrayGet {
                        array: Box::new(ValueExpr::Variable("outer".to_string())),
                        index: Box::new(ValueExpr::IntLiteral(0)),
                        element_type: array_i32,
                    },
                },
            ],
        }],
    };

    let c = emit_c(&program);
    assert!(c.contains("nomo_snapshot = nomo_array_i32_retain(nomo_snapshot);"));
    assert!(c.contains("array.data[array.len] = nomo_array_i32_retain(value);"));
    assert!(c.contains("nomo_array_i32_retain(array.data[index])"));
    assert!(c.contains("nomo_array_i32_release(nomo_snapshot);"));
    assert!(c.contains("nomo_array_i32_release(nomo_inner);"));
    assert!(c.contains("nomo_array_array_i32_release(nomo_outer);"));
    assert!(c.contains("nomo_array_i32_release(array.data[i]);"));
    assert!(c.contains("nomo_enum_Option_array_i32_release(nomo_first);"));
    assert!(c.contains("if (value.tag == nomo_enum_Option_array_i32_Some) {"));
    assert!(c.contains("nomo_array_i32_release(value.payload.nomo_payload_Some);"));
}

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

#[test]
fn break_releases_only_loop_body_array_locals() {
    let array_i32 = ValueType::Array(Box::new(ValueType::I32));
    let program = Program {
        consts: Vec::new(),
        package: "app.main".to_string(),
        imports: vec!["std.array".to_string()],
        extern_functions: Vec::new(),
        structs: Vec::new(),
        enums: vec![EnumType {
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
        }],
        functions: vec![Function {
            package: "app.main".to_string(),
            name: "main".to_string(),
            params: Vec::new(),
            return_type: ValueType::Void,
            body: vec![
                Statement::Let {
                    name: "items".to_string(),
                    value_type: array_i32.clone(),
                    initializer: ValueExpr::ArrayNew {
                        element_type: ValueType::I32,
                    },
                },
                Statement::Loop {
                    kind: LoopKind::Infinite,
                    body: vec![
                        Statement::Let {
                            name: "temp".to_string(),
                            value_type: array_i32,
                            initializer: ValueExpr::ArrayNew {
                                element_type: ValueType::I32,
                            },
                        },
                        Statement::Break,
                    ],
                },
                Statement::Let {
                    name: "size".to_string(),
                    value_type: ValueType::U64,
                    initializer: ValueExpr::ArrayLen {
                        array: Box::new(ValueExpr::Variable("items".to_string())),
                    },
                },
            ],
        }],
    };

    let c = emit_c(&program);
    let break_index = c.find("break;").unwrap();
    let temp_release = c.find("nomo_array_i32_release(nomo_temp);").unwrap();
    assert!(temp_release < break_index);
    assert!(!c[..break_index].contains("nomo_array_i32_release(nomo_items);"));
    let size_index = c
        .find("uint64_t nomo_size = ((uint64_t)nomo_items.len);")
        .unwrap();
    let items_release = c.rfind("nomo_array_i32_release(nomo_items);").unwrap();
    assert!(break_index < size_index);
    assert!(size_index < items_release);
}

#[test]
fn for_in_releases_owned_iterable_temp_but_not_shared_iterable() {
    let array_string = ValueType::Array(Box::new(ValueType::String));
    let program = Program {
        consts: Vec::new(),
        package: "app.main".to_string(),
        imports: vec!["std.env".to_string(), "std.array".to_string()],
        extern_functions: Vec::new(),
        structs: Vec::new(),
        enums: vec![EnumType {
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
        }],
        functions: vec![Function {
            package: "app.main".to_string(),
            name: "main".to_string(),
            params: Vec::new(),
            return_type: ValueType::Void,
            body: vec![
                Statement::Loop {
                    kind: LoopKind::Iterate {
                        binding: "arg".to_string(),
                        element_type: ValueType::String,
                        iterable: ValueExpr::EnvArgs,
                    },
                    body: vec![Statement::Println(ValueExpr::Variable("arg".to_string()))],
                },
                Statement::Let {
                    name: "words".to_string(),
                    value_type: array_string,
                    initializer: ValueExpr::EnvArgs,
                },
                Statement::Loop {
                    kind: LoopKind::Iterate {
                        binding: "word".to_string(),
                        element_type: ValueType::String,
                        iterable: ValueExpr::Variable("words".to_string()),
                    },
                    body: vec![Statement::Println(ValueExpr::Variable("word".to_string()))],
                },
            ],
        }],
    };

    let c = emit_c(&program);
    let owned_seq = "nomo_array_string nomo__seq = nomo_env_args(nomo_argc, nomo_argv);";
    let owned_release = "nomo_array_string_release(nomo__seq);";
    let shared_seq = "nomo_array_string nomo__seq = nomo_words;";
    let owned_seq_index = c.find(owned_seq).unwrap();
    let owned_release_index = c[owned_seq_index..].find(owned_release).unwrap() + owned_seq_index;
    let shared_seq_index = c.find(shared_seq).unwrap();
    assert!(owned_seq_index < owned_release_index);
    assert!(owned_release_index < shared_seq_index);
    assert!(!c[shared_seq_index..].contains(owned_release));
}

#[test]
fn for_in_releases_managed_binding_after_each_iteration() {
    let array_i32 = ValueType::Array(Box::new(ValueType::I32));
    let array_array_i32 = ValueType::Array(Box::new(array_i32.clone()));
    let program = Program {
        consts: Vec::new(),
        package: "app.main".to_string(),
        imports: vec!["std.array".to_string()],
        extern_functions: Vec::new(),
        structs: Vec::new(),
        enums: vec![EnumType {
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
        }],
        functions: vec![Function {
            package: "app.main".to_string(),
            name: "main".to_string(),
            params: Vec::new(),
            return_type: ValueType::Void,
            body: vec![
                Statement::Let {
                    name: "items".to_string(),
                    value_type: array_array_i32,
                    initializer: ValueExpr::ArrayNew {
                        element_type: array_i32.clone(),
                    },
                },
                Statement::Loop {
                    kind: LoopKind::Iterate {
                        binding: "item".to_string(),
                        element_type: array_i32,
                        iterable: ValueExpr::Variable("items".to_string()),
                    },
                    body: vec![Statement::Println(ValueExpr::StringLiteral(
                        "tick".to_string(),
                    ))],
                },
            ],
        }],
    };

    let c = emit_c(&program);
    let binding = "nomo_array_i32 nomo_item = nomo__seq.data[nomo_i];";
    let retain = "nomo_item = nomo_array_i32_retain(nomo_item);";
    let body = puts_literal("tick");
    let release = "nomo_array_i32_release(nomo_item);";
    let binding_index = c.find(binding).unwrap();
    let retain_index = c[binding_index..].find(retain).unwrap() + binding_index;
    let body_index = c[retain_index..].find(&body).unwrap() + retain_index;
    let release_index = c[body_index..].find(release).unwrap() + body_index;
    assert!(binding_index < retain_index);
    assert!(retain_index < body_index);
    assert!(body_index < release_index);
}

#[test]
fn for_in_return_releases_owned_iterable_temp_and_managed_binding() {
    let array_i32 = ValueType::Array(Box::new(ValueType::I32));
    let program = Program {
        consts: Vec::new(),
        package: "app.main".to_string(),
        imports: vec!["std.array".to_string()],
        extern_functions: Vec::new(),
        structs: Vec::new(),
        enums: vec![EnumType {
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
        }],
        functions: vec![
            Function {
                package: "app.main".to_string(),
                name: "take".to_string(),
                params: Vec::new(),
                return_type: array_i32.clone(),
                body: vec![Statement::Loop {
                    kind: LoopKind::Iterate {
                        binding: "item".to_string(),
                        element_type: array_i32.clone(),
                        iterable: ValueExpr::ArrayNew {
                            element_type: array_i32.clone(),
                        },
                    },
                    body: vec![Statement::Return(Some(ValueExpr::Variable(
                        "item".to_string(),
                    )))],
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
    let return_temp = "nomo_array_i32 nomo__return = nomo_item;";
    let retain_return = "nomo__return = nomo_array_i32_retain(nomo__return);";
    let release_binding = "nomo_array_i32_release(nomo_item);";
    let release_seq = "nomo_array_array_i32_release(nomo__seq);";
    let return_stmt = "return nomo__return;";
    let return_temp_index = c.find(return_temp).unwrap();
    let retain_index = c[return_temp_index..].find(retain_return).unwrap() + return_temp_index;
    let binding_release_index = c[retain_index..].find(release_binding).unwrap() + retain_index;
    let seq_release_index =
        c[binding_release_index..].find(release_seq).unwrap() + binding_release_index;
    let return_index = c[seq_release_index..].find(return_stmt).unwrap() + seq_release_index;
    assert!(return_temp_index < retain_index);
    assert!(retain_index < binding_release_index);
    assert!(binding_release_index < seq_release_index);
    assert!(seq_release_index < return_index);
}

#[test]
fn array_reassignment_releases_old_storage_and_retains_shared_rhs() {
    let array_i32 = ValueType::Array(Box::new(ValueType::I32));
    let program = Program {
        consts: Vec::new(),
        package: "app.main".to_string(),
        imports: vec!["std.array".to_string()],
        extern_functions: Vec::new(),
        structs: Vec::new(),
        enums: vec![EnumType {
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
        }],
        functions: vec![Function {
            package: "app.main".to_string(),
            name: "main".to_string(),
            params: Vec::new(),
            return_type: ValueType::Void,
            body: vec![
                Statement::Let {
                    name: "left".to_string(),
                    value_type: array_i32.clone(),
                    initializer: ValueExpr::ArrayNew {
                        element_type: ValueType::I32,
                    },
                },
                Statement::Let {
                    name: "right".to_string(),
                    value_type: array_i32,
                    initializer: ValueExpr::ArrayNew {
                        element_type: ValueType::I32,
                    },
                },
                Statement::Assign {
                    name: "left".to_string(),
                    value: ValueExpr::Variable("right".to_string()),
                },
                Statement::Assign {
                    name: "left".to_string(),
                    value: ValueExpr::Variable("left".to_string()),
                },
            ],
        }],
    };

    let c = emit_c(&program);
    let temp = "nomo_array_i32 nomo__assign_nomo_left = nomo_right;";
    let retain = "nomo__assign_nomo_left = nomo_array_i32_retain(nomo__assign_nomo_left);";
    let release = "nomo_array_i32_release(nomo_left);";
    let assign = "nomo_left = nomo__assign_nomo_left;";
    let temp_index = c.find(temp).unwrap();
    let retain_index = c[temp_index..].find(retain).unwrap() + temp_index;
    let release_index = c[retain_index..].find(release).unwrap() + retain_index;
    let assign_index = c[release_index..].find(assign).unwrap() + release_index;
    assert!(temp_index < retain_index);
    assert!(retain_index < release_index);
    assert!(release_index < assign_index);
    assert!(c.contains("nomo_array_i32 nomo__assign_nomo_left = nomo_left;"));
}

#[test]
fn option_array_reassignment_retains_and_releases_payload() {
    let array_i32 = ValueType::Array(Box::new(ValueType::I32));
    let option_array_i32 = ValueType::Enum("Option".to_string(), vec![array_i32.clone()]);
    let program = Program {
        consts: Vec::new(),
        package: "app.main".to_string(),
        imports: vec!["std.array".to_string()],
        extern_functions: Vec::new(),
        structs: Vec::new(),
        enums: vec![EnumType {
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
        }],
        functions: vec![Function {
            package: "app.main".to_string(),
            name: "main".to_string(),
            params: Vec::new(),
            return_type: ValueType::Void,
            body: vec![
                Statement::Let {
                    name: "values".to_string(),
                    value_type: array_i32,
                    initializer: ValueExpr::ArrayNew {
                        element_type: ValueType::I32,
                    },
                },
                Statement::Let {
                    name: "maybe".to_string(),
                    value_type: option_array_i32.clone(),
                    initializer: ValueExpr::EnumVariant {
                        enum_name: "Option".to_string(),
                        enum_args: vec![ValueType::Array(Box::new(ValueType::I32))],
                        variant: "Some".to_string(),
                        payload: Some(Box::new(ValueExpr::Variable("values".to_string()))),
                    },
                },
                Statement::Let {
                    name: "snapshot".to_string(),
                    value_type: option_array_i32,
                    initializer: ValueExpr::Variable("maybe".to_string()),
                },
                Statement::Assign {
                    name: "maybe".to_string(),
                    value: ValueExpr::Variable("maybe".to_string()),
                },
            ],
        }],
    };

    let c = emit_c(&program);
    assert!(c.contains("nomo_maybe = nomo_enum_Option_array_i32_retain(nomo_maybe);"));
    assert!(c.contains("nomo_snapshot = nomo_enum_Option_array_i32_retain(nomo_snapshot);"));
    assert!(c.contains(
        "nomo__assign_nomo_maybe = nomo_enum_Option_array_i32_retain(nomo__assign_nomo_maybe);"
    ));
    assert!(c.contains("nomo_enum_Option_array_i32_release(nomo_maybe);"));
    assert!(c.contains("if (value.tag == nomo_enum_Option_array_i32_Some) {"));
    assert!(c.contains(
        "value.payload.nomo_payload_Some = nomo_array_i32_retain(value.payload.nomo_payload_Some);"
    ));
    assert!(c.contains("nomo_array_i32_release(value.payload.nomo_payload_Some);"));
}

#[test]
fn array_get_returns_owned_option_payload_without_extra_binding_retain() {
    let array_i32 = ValueType::Array(Box::new(ValueType::I32));
    let array_array_i32 = ValueType::Array(Box::new(array_i32.clone()));
    let option_array_i32 = ValueType::Enum("Option".to_string(), vec![array_i32.clone()]);
    let program = Program {
        consts: Vec::new(),
        package: "app.main".to_string(),
        imports: vec!["std.array".to_string()],
        extern_functions: Vec::new(),
        structs: Vec::new(),
        enums: vec![EnumType {
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
        }],
        functions: vec![Function {
            package: "app.main".to_string(),
            name: "main".to_string(),
            params: Vec::new(),
            return_type: ValueType::Void,
            body: vec![
                Statement::Let {
                    name: "inner".to_string(),
                    value_type: array_i32.clone(),
                    initializer: ValueExpr::ArrayNew {
                        element_type: ValueType::I32,
                    },
                },
                Statement::Let {
                    name: "outer".to_string(),
                    value_type: array_array_i32,
                    initializer: ValueExpr::ArrayNew {
                        element_type: array_i32.clone(),
                    },
                },
                Statement::Assign {
                    name: "outer".to_string(),
                    value: ValueExpr::ArrayPush {
                        array: "outer".to_string(),
                        value: Box::new(ValueExpr::Variable("inner".to_string())),
                        element_type: array_i32.clone(),
                    },
                },
                Statement::Let {
                    name: "maybe".to_string(),
                    value_type: option_array_i32.clone(),
                    initializer: ValueExpr::ArrayGet {
                        array: Box::new(ValueExpr::Variable("outer".to_string())),
                        index: Box::new(ValueExpr::IntLiteral(0)),
                        element_type: array_i32,
                    },
                },
                Statement::Let {
                    name: "snapshot".to_string(),
                    value_type: option_array_i32,
                    initializer: ValueExpr::Variable("maybe".to_string()),
                },
            ],
        }],
    };

    let c = emit_c(&program);
    assert!(c.contains(
        "nomo_enum_Option_array_i32 nomo_maybe = nomo_array_array_i32_get(nomo_outer, 0);"
    ));
    assert!(!c.contains("nomo_maybe = nomo_enum_Option_array_i32_retain(nomo_maybe);"));
    assert!(c.contains("nomo_snapshot = nomo_enum_Option_array_i32_retain(nomo_snapshot);"));
}

#[test]
fn if_let_releases_owned_enum_temp_after_retaining_payload_binding() {
    let array_i32 = ValueType::Array(Box::new(ValueType::I32));
    let array_array_i32 = ValueType::Array(Box::new(array_i32.clone()));
    let program = Program {
        consts: Vec::new(),
        package: "app.main".to_string(),
        imports: vec!["std.array".to_string()],
        extern_functions: Vec::new(),
        structs: Vec::new(),
        enums: vec![EnumType {
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
        }],
        functions: vec![Function {
            package: "app.main".to_string(),
            name: "main".to_string(),
            params: Vec::new(),
            return_type: ValueType::Void,
            body: vec![
                Statement::Let {
                    name: "outer".to_string(),
                    value_type: array_array_i32,
                    initializer: ValueExpr::ArrayNew {
                        element_type: array_i32.clone(),
                    },
                },
                Statement::IfLet {
                    binding: Some("values".to_string()),
                    value_type: Some(array_i32.clone()),
                    value: ValueExpr::ArrayGet {
                        array: Box::new(ValueExpr::Variable("outer".to_string())),
                        index: Box::new(ValueExpr::IntLiteral(0)),
                        element_type: array_i32,
                    },
                    enum_name: "Option".to_string(),
                    enum_args: vec![ValueType::Array(Box::new(ValueType::I32))],
                    variant: "Some".to_string(),
                    body: vec![Statement::Println(ValueExpr::StringLiteral(
                        "some".to_string(),
                    ))],
                    else_body: Some(vec![Statement::Println(ValueExpr::StringLiteral(
                        "none".to_string(),
                    ))]),
                },
            ],
        }],
    };

    let c = emit_c(&program);
    let retain = "nomo_values = nomo_array_i32_retain(nomo_values);";
    let temp_release =
        "nomo_enum_Option_array_i32_release(nomo__if_let_nomo_enum_Option_array_i32_Some);";
    let body = puts_literal("some");
    let binding_release = "nomo_array_i32_release(nomo_values);";
    let retain_index = c.find(retain).unwrap();
    let release_index = c[retain_index..].find(temp_release).unwrap() + retain_index;
    let body_index = c[release_index..].find(&body).unwrap() + release_index;
    let binding_release_index = c[body_index..].find(binding_release).unwrap() + body_index;
    assert!(retain_index < release_index);
    assert!(release_index < body_index);
    assert!(body_index < binding_release_index);
    let else_index = c.find(" else {").unwrap();
    let else_release = c[else_index..].find(temp_release).unwrap() + else_index;
    let else_body = c[else_release..].find(&puts_literal("none")).unwrap() + else_release;
    assert!(else_index < else_release);
    assert!(else_release < else_body);
}

#[test]
fn let_else_releases_owned_enum_temp_after_retaining_payload_binding() {
    let array_i32 = ValueType::Array(Box::new(ValueType::I32));
    let array_array_i32 = ValueType::Array(Box::new(array_i32.clone()));
    let program = Program {
        consts: Vec::new(),
        package: "app.main".to_string(),
        imports: vec!["std.array".to_string()],
        extern_functions: Vec::new(),
        structs: Vec::new(),
        enums: vec![EnumType {
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
        }],
        functions: vec![Function {
            package: "app.main".to_string(),
            name: "main".to_string(),
            params: Vec::new(),
            return_type: ValueType::Void,
            body: vec![
                Statement::Let {
                    name: "outer".to_string(),
                    value_type: array_array_i32,
                    initializer: ValueExpr::ArrayNew {
                        element_type: array_i32.clone(),
                    },
                },
                Statement::LetElse {
                    binding: "values".to_string(),
                    value_type: array_i32.clone(),
                    value: ValueExpr::ArrayGet {
                        array: Box::new(ValueExpr::Variable("outer".to_string())),
                        index: Box::new(ValueExpr::IntLiteral(0)),
                        element_type: array_i32,
                    },
                    enum_name: "Option".to_string(),
                    enum_args: vec![ValueType::Array(Box::new(ValueType::I32))],
                    variant: "Some".to_string(),
                    else_body: vec![Statement::Panic(ValueExpr::StringLiteral(
                        "missing".to_string(),
                    ))],
                },
                Statement::Println(ValueExpr::StringLiteral("ok".to_string())),
            ],
        }],
    };

    let c = emit_c(&program);
    let else_release = "nomo_enum_Option_array_i32_release(nomo__let_else_nomo_values);";
    let else_panic = panic_literal("missing");
    let binding_retain = "nomo_values = nomo_array_i32_retain(nomo_values);";
    let binding_release = "nomo_enum_Option_array_i32_release(nomo__let_else_nomo_values);";
    let else_index = c.find(else_release).unwrap();
    let panic_index = c[else_index..].find(&else_panic).unwrap() + else_index;
    assert!(else_index < panic_index);
    let retain_index = c.rfind(binding_retain).unwrap();
    let release_index = c[retain_index..].find(binding_release).unwrap() + retain_index;
    assert!(retain_index < release_index);
}

#[test]
fn struct_and_custom_enum_lifecycle_helpers_manage_array_payloads() {
    let array_i32 = ValueType::Array(Box::new(ValueType::I32));
    let bag = ValueType::Struct("Bag".to_string(), Vec::new());
    let slot = ValueType::Enum("Slot".to_string(), Vec::new());
    let program = Program {
        consts: Vec::new(),
        package: "app.main".to_string(),
        imports: vec!["std.array".to_string()],
        extern_functions: Vec::new(),
        structs: vec![StructType {
            package: "app.main".to_string(),
            name: "Bag".to_string(),
            type_params: Vec::new(),
            fields: vec![StructField {
                name: "items".to_string(),
                value_type: array_i32.clone(),
            }],
        }],
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
                name: "Slot".to_string(),
                type_params: Vec::new(),
                variants: vec![
                    EnumVariantType {
                        name: "Full".to_string(),
                        payload: Some(bag.clone()),
                    },
                    EnumVariantType {
                        name: "Empty".to_string(),
                        payload: None,
                    },
                ],
            },
        ],
        functions: vec![
            Function {
                package: "app.main".to_string(),
                name: "label".to_string(),
                params: vec![Parameter {
                    name: "bag".to_string(),
                    mutable: false,
                    value_type: bag.clone(),
                }],
                return_type: ValueType::Void,
                body: Vec::new(),
            },
            Function {
                package: "app.main".to_string(),
                name: "main".to_string(),
                params: Vec::new(),
                return_type: ValueType::Void,
                body: vec![
                    Statement::Let {
                        name: "items".to_string(),
                        value_type: array_i32.clone(),
                        initializer: ValueExpr::ArrayNew {
                            element_type: ValueType::I32,
                        },
                    },
                    Statement::Let {
                        name: "bag".to_string(),
                        value_type: bag.clone(),
                        initializer: ValueExpr::StructLiteral {
                            type_name: "Bag".to_string(),
                            struct_args: Vec::new(),
                            fields: vec![(
                                "items".to_string(),
                                ValueExpr::Variable("items".to_string()),
                            )],
                        },
                    },
                    Statement::Let {
                        name: "replacement".to_string(),
                        value_type: array_i32.clone(),
                        initializer: ValueExpr::ArrayNew {
                            element_type: ValueType::I32,
                        },
                    },
                    Statement::AssignField {
                        base: "bag".to_string(),
                        field: "items".to_string(),
                        value_type: array_i32,
                        value: ValueExpr::Variable("replacement".to_string()),
                    },
                    Statement::Let {
                        name: "slot".to_string(),
                        value_type: slot,
                        initializer: ValueExpr::EnumVariant {
                            enum_name: "Slot".to_string(),
                            enum_args: Vec::new(),
                            variant: "Full".to_string(),
                            payload: Some(Box::new(ValueExpr::Variable("bag".to_string()))),
                        },
                    },
                ],
            },
        ],
    };

    let c = emit_c(&program);
    assert!(c.contains("static nomo_struct_Bag nomo_struct_Bag_retain(nomo_struct_Bag value)"));
    assert!(
        c.contains("value.nomo_member_items = nomo_array_i32_retain(value.nomo_member_items);")
    );
    assert!(c.contains("static void nomo_struct_Bag_release(nomo_struct_Bag value)"));
    assert!(c.contains("nomo_array_i32_release(value.nomo_member_items);"));
    assert!(c.contains("static nomo_enum_Slot nomo_enum_Slot_retain(nomo_enum_Slot value)"));
    assert!(c.contains(
        "value.payload.nomo_payload_Full = nomo_struct_Bag_retain(value.payload.nomo_payload_Full);"
    ));
    assert!(c.contains("nomo_struct_Bag_release(value.payload.nomo_payload_Full);"));
    assert!(c.contains("nomo_bag = nomo_struct_Bag_retain(nomo_bag);"));
    assert!(c.contains("nomo_slot = nomo_enum_Slot_retain(nomo_slot);"));
    assert!(c.contains("nomo_enum_Slot_release(nomo_slot);"));
    let field_temp = "nomo_array_i32 nomo__assign_nomo_bag_nomo_member_items = nomo_replacement;";
    let field_retain = "nomo__assign_nomo_bag_nomo_member_items = nomo_array_i32_retain(nomo__assign_nomo_bag_nomo_member_items);";
    let field_release = "nomo_array_i32_release(nomo_bag.nomo_member_items);";
    let field_assign = "nomo_bag.nomo_member_items = nomo__assign_nomo_bag_nomo_member_items;";
    let temp_index = c.find(field_temp).unwrap();
    let retain_index = c[temp_index..].find(field_retain).unwrap() + temp_index;
    let release_index = c[retain_index..].find(field_release).unwrap() + retain_index;
    let assign_index = c[release_index..].find(field_assign).unwrap() + release_index;
    assert!(temp_index < retain_index);
    assert!(retain_index < release_index);
    assert!(release_index < assign_index);
}

#[test]
fn array_parameters_are_retained_and_released_by_value_but_not_mut_borrows() {
    let array_i32 = ValueType::Array(Box::new(ValueType::I32));
    let program = Program {
        consts: Vec::new(),
        package: "app.main".to_string(),
        imports: vec!["std.array".to_string()],
        extern_functions: Vec::new(),
        structs: Vec::new(),
        enums: vec![EnumType {
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
        }],
        functions: vec![
            Function {
                package: "app.main".to_string(),
                name: "id".to_string(),
                params: vec![Parameter {
                    name: "values".to_string(),
                    mutable: false,
                    value_type: array_i32.clone(),
                }],
                return_type: array_i32.clone(),
                body: vec![Statement::Return(Some(ValueExpr::Variable(
                    "values".to_string(),
                )))],
            },
            Function {
                package: "app.main".to_string(),
                name: "borrow".to_string(),
                params: vec![Parameter {
                    name: "values".to_string(),
                    mutable: true,
                    value_type: array_i32,
                }],
                return_type: ValueType::Void,
                body: Vec::new(),
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
    let id_start = c
        .find("nomo_array_i32 nomo_fn_id(nomo_array_i32 nomo_values)")
        .unwrap();
    let id_body = &c[id_start
        ..c[id_start..]
            .find("#undef")
            .map_or(c.len(), |end| id_start + end)];
    assert!(id_body.contains("nomo_values = nomo_array_i32_retain(nomo_values);"));
    assert!(id_body.contains("nomo__return = nomo_array_i32_retain(nomo__return);"));
    assert!(id_body.contains("nomo_array_i32_release(nomo_values);"));

    let borrow_start = c
        .rfind("void nomo_fn_borrow(nomo_array_i32 * nomo_values)")
        .unwrap();
    let main_start = c[borrow_start..]
        .find("int main")
        .map(|offset| borrow_start + offset)
        .unwrap_or(c.len());
    let borrow_body = &c[borrow_start..main_start];
    assert!(!borrow_body.contains("nomo_values = nomo_array_i32_retain(nomo_values);"));
    assert!(!borrow_body.contains("nomo_array_i32_release(nomo_values);"));
}

#[test]
fn emits_array_helpers_for_all_v0_1_primitive_elements() {
    let elements = vec![
        ValueType::String,
        ValueType::Int,
        ValueType::I32,
        ValueType::U32,
        ValueType::U64,
        ValueType::Float,
        ValueType::Char,
        ValueType::Bool,
    ];
    let program = Program {
        consts: Vec::new(),
        package: "app.main".to_string(),
        imports: vec!["std.array".to_string()],
        extern_functions: Vec::new(),
        structs: Vec::new(),
        enums: vec![EnumType {
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
        }],
        functions: vec![Function {
            package: "app.main".to_string(),
            name: "main".to_string(),
            params: Vec::new(),
            return_type: ValueType::Void,
            body: elements
                .iter()
                .map(|element_type| Statement::Let {
                    name: format!("items_{}", c_type_name_part(element_type)),
                    value_type: ValueType::Array(Box::new(element_type.clone())),
                    initializer: ValueExpr::ArrayNew {
                        element_type: element_type.clone(),
                    },
                })
                .collect(),
        }],
    };

    let c = emit_c(&program);
    for (element_type, c_data_type) in [
        (ValueType::String, "nomo_string"),
        (ValueType::Int, "long long"),
        (ValueType::I32, "int32_t"),
        (ValueType::U32, "uint32_t"),
        (ValueType::U64, "uint64_t"),
        (ValueType::Float, "double"),
        (ValueType::Char, "uint32_t"),
        (ValueType::Bool, "int"),
    ] {
        let array = c_array_ident(&element_type);
        assert!(c.contains(&format!("typedef struct {array}")));
        assert!(c.contains(&format!("{c_data_type} *data;")));
        assert!(c.contains(&format!("static {array} {array}_new(void)")));
    }
}
