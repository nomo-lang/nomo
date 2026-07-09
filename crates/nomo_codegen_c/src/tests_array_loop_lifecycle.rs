use super::*;
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
