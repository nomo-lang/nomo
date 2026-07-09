use super::*;

#[test]
fn emits_function_and_call() {
    let program = Program {
        consts: Vec::new(),
        package: "app.main".to_string(),
        imports: vec!["std.io".to_string()],
        extern_functions: Vec::new(),
        structs: Vec::new(),
        enums: Vec::new(),
        functions: vec![
            Function {
                package: "app.main".to_string(),
                name: "add".to_string(),
                params: vec![
                    Parameter {
                        name: "a".to_string(),
                        mutable: false,
                        value_type: ValueType::Int,
                    },
                    Parameter {
                        name: "b".to_string(),
                        mutable: false,
                        value_type: ValueType::Int,
                    },
                ],
                return_type: ValueType::Int,
                body: vec![Statement::Return(Some(ValueExpr::Binary {
                    left: Box::new(ValueExpr::Variable("a".to_string())),
                    op: BinaryOp::Add,
                    right: Box::new(ValueExpr::Variable("b".to_string())),
                    value_type: ValueType::Int,
                }))],
            },
            Function {
                package: "app.main".to_string(),
                name: "main".to_string(),
                params: Vec::new(),
                return_type: ValueType::Void,
                body: vec![
                    Statement::Let {
                        name: "answer".to_string(),
                        value_type: ValueType::Int,
                        initializer: ValueExpr::Call {
                            name: "add".to_string(),
                            args: vec![ValueExpr::IntLiteral(40), ValueExpr::IntLiteral(2)],
                        },
                    },
                    Statement::Println(ValueExpr::StringLiteral("done".to_string())),
                ],
            },
        ],
    };

    let c = emit_c(&program);
    assert!(c.contains("long long nomo_fn_add(long long nomo_a, long long nomo_b);"));
    assert!(c.contains("long long nomo_fn_add(long long nomo_a, long long nomo_b)"));
    assert!(c.contains("return nomo_add_i64(nomo_a, nomo_b);"));
    assert!(c.contains("long long nomo_answer = nomo_fn_add(40, 2);"));
}

#[test]
fn emits_mut_parameter_as_pointer_borrow() {
    let program = Program {
        consts: Vec::new(),
        package: "app.main".to_string(),
        imports: Vec::new(),
        extern_functions: Vec::new(),
        structs: Vec::new(),
        enums: Vec::new(),
        functions: vec![
            Function {
                package: "app.main".to_string(),
                name: "bump".to_string(),
                params: vec![Parameter {
                    name: "value".to_string(),
                    mutable: true,
                    value_type: ValueType::Int,
                }],
                return_type: ValueType::Void,
                body: vec![Statement::Assign {
                    name: "value".to_string(),
                    value: ValueExpr::Binary {
                        left: Box::new(ValueExpr::Variable("value".to_string())),
                        op: BinaryOp::Add,
                        right: Box::new(ValueExpr::IntLiteral(1)),
                        value_type: ValueType::Int,
                    },
                }],
            },
            Function {
                package: "app.main".to_string(),
                name: "main".to_string(),
                params: Vec::new(),
                return_type: ValueType::Void,
                body: vec![
                    Statement::Let {
                        name: "count".to_string(),
                        value_type: ValueType::Int,
                        initializer: ValueExpr::IntLiteral(1),
                    },
                    Statement::Expr(ValueExpr::Call {
                        name: "bump".to_string(),
                        args: vec![ValueExpr::MutBorrow(vec!["count".to_string()])],
                    }),
                ],
            },
        ],
    };

    let c = emit_c(&program);
    assert!(c.contains("void nomo_fn_bump(long long * nomo_value);"));
    assert!(c.contains("#define nomo_value (*nomo_value)"));
    assert!(c.contains("nomo_value = nomo_add_i64(nomo_value, 1);"));
    assert!(c.contains("#undef nomo_value"));
    assert!(c.contains("nomo_fn_bump(&nomo_count);"));
}

#[test]
fn emits_mut_field_path_as_pointer_borrow() {
    let program = Program {
        consts: Vec::new(),
        package: "app.main".to_string(),
        imports: Vec::new(),
        extern_functions: Vec::new(),
        structs: vec![StructType {
            package: "app.main".to_string(),
            name: "Point".to_string(),
            type_params: Vec::new(),
            fields: vec![
                StructField {
                    name: "x".to_string(),
                    value_type: ValueType::I32,
                },
                StructField {
                    name: "y".to_string(),
                    value_type: ValueType::I32,
                },
            ],
        }],
        enums: Vec::new(),
        functions: vec![
            Function {
                package: "app.main".to_string(),
                name: "bump".to_string(),
                params: vec![Parameter {
                    name: "value".to_string(),
                    mutable: true,
                    value_type: ValueType::I32,
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
                        name: "point".to_string(),
                        value_type: ValueType::Struct("Point".to_string(), Vec::new()),
                        initializer: ValueExpr::StructLiteral {
                            type_name: "Point".to_string(),
                            struct_args: Vec::new(),
                            fields: vec![
                                ("x".to_string(), ValueExpr::IntLiteral(1)),
                                ("y".to_string(), ValueExpr::IntLiteral(2)),
                            ],
                        },
                    },
                    Statement::Expr(ValueExpr::Call {
                        name: "bump".to_string(),
                        args: vec![ValueExpr::MutBorrow(vec![
                            "point".to_string(),
                            "x".to_string(),
                        ])],
                    }),
                ],
            },
        ],
    };

    let c = emit_c(&program);
    assert!(c.contains("nomo_fn_bump(&nomo_point.nomo_member_x);"));
}

#[test]
fn emits_return_value_before_deferred_calls() {
    let program = Program {
        consts: Vec::new(),
        package: "app.main".to_string(),
        imports: vec!["std.io".to_string()],
        extern_functions: Vec::new(),
        structs: Vec::new(),
        enums: Vec::new(),
        functions: vec![
            Function {
                package: "app.main".to_string(),
                name: "cleanup".to_string(),
                params: Vec::new(),
                return_type: ValueType::Void,
                body: vec![Statement::Println(ValueExpr::StringLiteral(
                    "cleanup".to_string(),
                ))],
            },
            Function {
                package: "app.main".to_string(),
                name: "value".to_string(),
                params: Vec::new(),
                return_type: ValueType::Int,
                body: vec![Statement::Return(Some(ValueExpr::IntLiteral(7)))],
            },
            Function {
                package: "app.main".to_string(),
                name: "compute".to_string(),
                params: Vec::new(),
                return_type: ValueType::Int,
                body: vec![
                    Statement::Defer {
                        call: DeferredCall::Expr(ValueExpr::Call {
                            name: "cleanup".to_string(),
                            args: Vec::new(),
                        }),
                    },
                    Statement::Return(Some(ValueExpr::Call {
                        name: "value".to_string(),
                        args: Vec::new(),
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
    let value = c.find("long long nomo__return = nomo_fn_value();").unwrap();
    let cleanup = c.find("nomo_fn_cleanup();").unwrap();
    let return_value = c.find("return nomo__return;").unwrap();
    assert!(value < cleanup);
    assert!(cleanup < return_value);
}

#[test]
fn emits_assignment() {
    let program = Program {
        consts: Vec::new(),
        package: "app.main".to_string(),
        imports: vec!["std.io".to_string()],
        extern_functions: Vec::new(),
        structs: Vec::new(),
        enums: Vec::new(),
        functions: vec![Function {
            package: "app.main".to_string(),
            name: "main".to_string(),
            params: Vec::new(),
            return_type: ValueType::Void,
            body: vec![
                Statement::Let {
                    name: "count".to_string(),
                    value_type: ValueType::Int,
                    initializer: ValueExpr::IntLiteral(1),
                },
                Statement::Assign {
                    name: "count".to_string(),
                    value: ValueExpr::Binary {
                        left: Box::new(ValueExpr::Variable("count".to_string())),
                        op: BinaryOp::Add,
                        right: Box::new(ValueExpr::IntLiteral(1)),
                        value_type: ValueType::Int,
                    },
                },
            ],
        }],
    };

    let c = emit_c(&program);
    assert!(c.contains("long long nomo_count = 1;"));
    assert!(c.contains("nomo_count = nomo_add_i64(nomo_count, 1);"));
}

#[test]
fn emits_field_assignment() {
    let program = Program {
        consts: Vec::new(),
        package: "app.main".to_string(),
        imports: vec!["std.io".to_string()],
        extern_functions: Vec::new(),
        structs: vec![StructType {
            package: "app.main".to_string(),
            name: "Counter".to_string(),
            type_params: Vec::new(),
            fields: vec![StructField {
                name: "value".to_string(),
                value_type: ValueType::Int,
            }],
        }],
        enums: Vec::new(),
        functions: vec![Function {
            package: "app.main".to_string(),
            name: "main".to_string(),
            params: Vec::new(),
            return_type: ValueType::Void,
            body: vec![
                Statement::Let {
                    name: "counter".to_string(),
                    value_type: ValueType::Struct("Counter".to_string(), Vec::new()),
                    initializer: ValueExpr::StructLiteral {
                        type_name: "Counter".to_string(),
                        struct_args: Vec::new(),
                        fields: vec![("value".to_string(), ValueExpr::IntLiteral(1))],
                    },
                },
                Statement::AssignField {
                    base: "counter".to_string(),
                    field: "value".to_string(),
                    value_type: ValueType::Int,
                    value: ValueExpr::Binary {
                        left: Box::new(ValueExpr::FieldAccess {
                            base: "counter".to_string(),
                            field: "value".to_string(),
                        }),
                        op: BinaryOp::Add,
                        right: Box::new(ValueExpr::IntLiteral(1)),
                        value_type: ValueType::Int,
                    },
                },
            ],
        }],
    };

    let c = emit_c(&program);
    assert!(c.contains(
        "nomo_counter.nomo_member_value = nomo_add_i64(nomo_counter.nomo_member_value, 1);"
    ));
}
