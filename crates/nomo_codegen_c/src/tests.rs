use super::*;
use nomo_ir::{EnumVariantType, MatchValueArm, Parameter, StructField, ValueExpr};

#[path = "tests_array_lifecycle.rs"]
mod tests_array_lifecycle;
#[path = "tests_basic_io_symbols.rs"]
mod tests_basic_io_symbols;
#[path = "tests_defer_control.rs"]
mod tests_defer_control;
#[path = "tests_host_helpers.rs"]
mod tests_host_helpers;
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
fn emits_if_expression_and_comparison() {
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
                name: "label".to_string(),
                params: vec![Parameter {
                    name: "score".to_string(),
                    mutable: false,
                    value_type: ValueType::Int,
                }],
                return_type: ValueType::String,
                body: vec![Statement::Return(Some(ValueExpr::If {
                    condition: Box::new(ValueExpr::Binary {
                        left: Box::new(ValueExpr::Variable("score".to_string())),
                        op: BinaryOp::GreaterEqual,
                        right: Box::new(ValueExpr::IntLiteral(60)),
                        value_type: ValueType::Bool,
                    }),
                    then_branch: Box::new(ValueExpr::StringLiteral("pass".to_string())),
                    else_branch: Box::new(ValueExpr::StringLiteral("fail".to_string())),
                }))],
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
    assert!(c.contains(
            "return ((nomo_score >= 60) ? nomo_string_literal(\"pass\") : nomo_string_literal(\"fail\"));"
        ));
}

#[test]
fn emits_string_equality_with_runtime_compare() {
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
            body: vec![Statement::Let {
                name: "same".to_string(),
                value_type: ValueType::Bool,
                initializer: ValueExpr::StringCompare {
                    left: Box::new(ValueExpr::StringLiteral("nomo".to_string())),
                    op: BinaryOp::Equal,
                    right: Box::new(ValueExpr::StringLiteral("nomo".to_string())),
                },
            }],
        }],
    };

    let c = emit_c(&program);
    assert!(c.contains("static int nomo_string_equal(nomo_string left, nomo_string right)"));
    assert!(c.contains(
            "int nomo_same = (nomo_string_equal(nomo_string_literal(\"nomo\"), nomo_string_literal(\"nomo\")));"
        ));
}

#[test]
fn emits_panic_statement_and_expression() {
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
                name: "label".to_string(),
                params: vec![Parameter {
                    name: "ok".to_string(),
                    mutable: false,
                    value_type: ValueType::Bool,
                }],
                return_type: ValueType::String,
                body: vec![Statement::Return(Some(ValueExpr::If {
                    condition: Box::new(ValueExpr::Variable("ok".to_string())),
                    then_branch: Box::new(ValueExpr::StringLiteral("yes".to_string())),
                    else_branch: Box::new(ValueExpr::Panic {
                        message: Box::new(ValueExpr::StringLiteral("no".to_string())),
                        fallback_type: ValueType::String,
                    }),
                }))],
            },
            Function {
                package: "app.main".to_string(),
                name: "main".to_string(),
                params: Vec::new(),
                return_type: ValueType::Void,
                body: vec![Statement::Panic(ValueExpr::StringLiteral(
                    "boom".to_string(),
                ))],
            },
        ],
    };

    let c = emit_c(&program);
    assert!(c.contains("static void nomo_panic"));
    assert!(c.contains(&panic_literal("boom")));
    assert!(
        c.contains("(nomo_panic((nomo_string_literal(\"no\")).data), nomo_string_literal(\"\"))")
    );
}

#[test]
fn emits_binary_arithmetic_operators() {
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
                name: "calc".to_string(),
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
                    Parameter {
                        name: "c".to_string(),
                        mutable: false,
                        value_type: ValueType::Int,
                    },
                ],
                return_type: ValueType::Int,
                body: vec![Statement::Return(Some(ValueExpr::Binary {
                    left: Box::new(ValueExpr::Binary {
                        left: Box::new(ValueExpr::Variable("a".to_string())),
                        op: BinaryOp::Subtract,
                        right: Box::new(ValueExpr::Variable("b".to_string())),
                        value_type: ValueType::Int,
                    }),
                    op: BinaryOp::Remainder,
                    right: Box::new(ValueExpr::Binary {
                        left: Box::new(ValueExpr::Binary {
                            left: Box::new(ValueExpr::Variable("c".to_string())),
                            op: BinaryOp::Multiply,
                            right: Box::new(ValueExpr::IntLiteral(4)),
                            value_type: ValueType::Int,
                        }),
                        op: BinaryOp::Divide,
                        right: Box::new(ValueExpr::IntLiteral(2)),
                        value_type: ValueType::Int,
                    }),
                    value_type: ValueType::Int,
                }))],
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

    assert!(c.contains(" - "));
    assert!(c.contains(" * "));
    assert!(c.contains("nomo_div_i64("));
    assert!(c.contains("nomo_rem_i64("));
}

#[test]
fn emits_logical_operators() {
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
                name: "check".to_string(),
                params: vec![
                    Parameter {
                        name: "a".to_string(),
                        mutable: false,
                        value_type: ValueType::Bool,
                    },
                    Parameter {
                        name: "b".to_string(),
                        mutable: false,
                        value_type: ValueType::Bool,
                    },
                ],
                return_type: ValueType::Bool,
                body: vec![Statement::Return(Some(ValueExpr::Binary {
                    left: Box::new(ValueExpr::Unary {
                        op: UnaryOp::Not,
                        expr: Box::new(ValueExpr::Variable("a".to_string())),
                    }),
                    op: BinaryOp::LogicalOr,
                    right: Box::new(ValueExpr::Binary {
                        left: Box::new(ValueExpr::Variable("a".to_string())),
                        op: BinaryOp::LogicalAnd,
                        right: Box::new(ValueExpr::Variable("b".to_string())),
                        value_type: ValueType::Bool,
                    }),
                    value_type: ValueType::Bool,
                }))],
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

    assert!(c.contains("!"));
    assert!(c.contains(" || "));
    assert!(c.contains(" && "));
}

#[test]
fn emits_bitwise_operators() {
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
                name: "mask".to_string(),
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
                    left: Box::new(ValueExpr::Binary {
                        left: Box::new(ValueExpr::Binary {
                            left: Box::new(ValueExpr::Variable("a".to_string())),
                            op: BinaryOp::BitAnd,
                            right: Box::new(ValueExpr::Variable("b".to_string())),
                            value_type: ValueType::Int,
                        }),
                        op: BinaryOp::BitOr,
                        right: Box::new(ValueExpr::Binary {
                            left: Box::new(ValueExpr::Variable("a".to_string())),
                            op: BinaryOp::BitXor,
                            right: Box::new(ValueExpr::Variable("b".to_string())),
                            value_type: ValueType::Int,
                        }),
                        value_type: ValueType::Int,
                    }),
                    op: BinaryOp::BitAndNot,
                    right: Box::new(ValueExpr::Binary {
                        left: Box::new(ValueExpr::Binary {
                            left: Box::new(ValueExpr::Variable("a".to_string())),
                            op: BinaryOp::ShiftLeft,
                            right: Box::new(ValueExpr::IntLiteral(1)),
                            value_type: ValueType::Int,
                        }),
                        op: BinaryOp::ShiftRight,
                        right: Box::new(ValueExpr::IntLiteral(1)),
                        value_type: ValueType::Int,
                    }),
                    value_type: ValueType::Int,
                }))],
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

    assert!(c.contains(" & "));
    assert!(c.contains(" | "));
    assert!(c.contains(" ^ "));
    assert!(c.contains("nomo_shl_i64("));
    assert!(c.contains("nomo_shr_i64("));
    assert!(c.contains("uint64_t shifted = (bits >> right) | (~UINT64_C(0) << (64U - right));"));
    assert!(c.contains("uint32_t shifted = (bits >> right) | (~UINT32_C(0) << (32U - right));"));
    assert!(c.contains(" & ~("));
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

#[test]
fn emits_struct_type_literal_and_field_access() {
    let program = Program {
        consts: Vec::new(),
        package: "app.main".to_string(),
        imports: vec!["std.io".to_string()],
        extern_functions: Vec::new(),
        structs: vec![StructType {
            package: "app.main".to_string(),
            name: "Point".to_string(),
            type_params: Vec::new(),
            fields: vec![
                StructField {
                    name: "x".to_string(),
                    value_type: ValueType::Int,
                },
                StructField {
                    name: "y".to_string(),
                    value_type: ValueType::Int,
                },
            ],
        }],
        enums: Vec::new(),
        functions: vec![Function {
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
                Statement::Let {
                    name: "x".to_string(),
                    value_type: ValueType::Int,
                    initializer: ValueExpr::FieldAccess {
                        base: "point".to_string(),
                        field: "x".to_string(),
                    },
                },
            ],
        }],
    };

    let c = emit_c(&program);
    assert!(c.contains("typedef struct nomo_struct_Point"));
    assert!(c.contains(
            "nomo_struct_Point nomo_point = (nomo_struct_Point){.nomo_member_x = 1, .nomo_member_y = 2};"
        ));
    assert!(c.contains("long long nomo_x = nomo_point.nomo_member_x;"));
}

#[test]
fn emits_generic_struct_instance() {
    let program = Program {
        consts: Vec::new(),
        package: "app.main".to_string(),
        imports: vec!["std.io".to_string()],
        extern_functions: Vec::new(),
        structs: vec![StructType {
            package: "app.main".to_string(),
            name: "Box".to_string(),
            type_params: vec!["T".to_string()],
            fields: vec![StructField {
                name: "value".to_string(),
                value_type: ValueType::TypeParam("T".to_string()),
            }],
        }],
        enums: Vec::new(),
        functions: vec![Function {
            package: "app.main".to_string(),
            name: "main".to_string(),
            params: Vec::new(),
            return_type: ValueType::Void,
            body: vec![Statement::Let {
                name: "item".to_string(),
                value_type: ValueType::Struct("Box".to_string(), vec![ValueType::I32]),
                initializer: ValueExpr::StructLiteral {
                    type_name: "Box".to_string(),
                    struct_args: vec![ValueType::I32],
                    fields: vec![("value".to_string(), ValueExpr::IntLiteral(7))],
                },
            }],
        }],
    };

    let c = emit_c(&program);
    assert!(c.contains("typedef struct nomo_struct_Box_i32"));
    assert!(c.contains("int32_t nomo_member_value;"));
    assert!(c.contains(
        "nomo_struct_Box_i32 nomo_item = (nomo_struct_Box_i32){.nomo_member_value = 7};"
    ));
}

#[test]
fn emits_enum_variant_and_match_expression() {
    let program = Program {
        consts: Vec::new(),
        package: "app.main".to_string(),
        imports: vec!["std.io".to_string()],
        extern_functions: Vec::new(),
        structs: Vec::new(),
        enums: vec![EnumType {
            package: "app.main".to_string(),
            name: "Color".to_string(),
            type_params: Vec::new(),
            variants: vec![
                EnumVariantType {
                    name: "Red".to_string(),
                    payload: None,
                },
                EnumVariantType {
                    name: "Blue".to_string(),
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
                    name: "color".to_string(),
                    value_type: ValueType::Enum("Color".to_string(), Vec::new()),
                    initializer: ValueExpr::EnumVariant {
                        enum_name: "Color".to_string(),
                        enum_args: Vec::new(),
                        variant: "Red".to_string(),
                        payload: None,
                    },
                },
                Statement::Let {
                    name: "label".to_string(),
                    value_type: ValueType::String,
                    initializer: ValueExpr::Match {
                        value: Box::new(ValueExpr::Variable("color".to_string())),
                        arms: vec![
                            MatchValueArm {
                                enum_name: "Color".to_string(),
                                enum_args: Vec::new(),
                                variant: "Red".to_string(),
                                binding: None,
                                value: ValueExpr::StringLiteral("red".to_string()),
                            },
                            MatchValueArm {
                                enum_name: "Color".to_string(),
                                enum_args: Vec::new(),
                                variant: "Blue".to_string(),
                                binding: None,
                                value: ValueExpr::StringLiteral("blue".to_string()),
                            },
                        ],
                    },
                },
            ],
        }],
    };

    let c = emit_c(&program);
    assert!(c.contains("typedef enum nomo_enum_Color_tag"));
    assert!(
        c.contains("nomo_enum_Color nomo_color = (nomo_enum_Color){.tag = nomo_enum_Color_Red};")
    );
    assert!(c.contains(
            "nomo_color.tag == nomo_enum_Color_Red ? nomo_string_literal(\"red\") : nomo_string_literal(\"blue\")"
        ));
}

#[test]
fn emits_payload_enum_and_match_binding_access() {
    let program = Program {
        consts: Vec::new(),
        package: "app.main".to_string(),
        imports: vec!["std.io".to_string()],
        extern_functions: Vec::new(),
        structs: Vec::new(),
        enums: vec![EnumType {
            package: "app.main".to_string(),
            name: "MaybeInt".to_string(),
            type_params: Vec::new(),
            variants: vec![
                EnumVariantType {
                    name: "Some".to_string(),
                    payload: Some(ValueType::Int),
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
                    name: "value".to_string(),
                    value_type: ValueType::Enum("MaybeInt".to_string(), Vec::new()),
                    initializer: ValueExpr::EnumVariant {
                        enum_name: "MaybeInt".to_string(),
                        enum_args: Vec::new(),
                        variant: "Some".to_string(),
                        payload: Some(Box::new(ValueExpr::IntLiteral(41))),
                    },
                },
                Statement::Let {
                    name: "answer".to_string(),
                    value_type: ValueType::Int,
                    initializer: ValueExpr::Match {
                        value: Box::new(ValueExpr::Variable("value".to_string())),
                        arms: vec![
                            MatchValueArm {
                                enum_name: "MaybeInt".to_string(),
                                enum_args: Vec::new(),
                                variant: "Some".to_string(),
                                binding: Some("n".to_string()),
                                value: ValueExpr::EnumPayload {
                                    value: Box::new(ValueExpr::Variable("value".to_string())),
                                    variant: "Some".to_string(),
                                },
                            },
                            MatchValueArm {
                                enum_name: "MaybeInt".to_string(),
                                enum_args: Vec::new(),
                                variant: "None".to_string(),
                                binding: None,
                                value: ValueExpr::IntLiteral(0),
                            },
                        ],
                    },
                },
            ],
        }],
    };

    let c = emit_c(&program);
    assert!(c.contains("union"));
    assert!(c.contains("long long nomo_payload_Some;"));
    assert!(c.contains(".payload.nomo_payload_Some = 41"));
    assert!(c.contains("nomo_value.payload.nomo_payload_Some"));
}

#[test]
fn emits_void_enum_payload_as_unit_storage() {
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
    assert!(c.contains("char nomo_payload_Ok;"));
    assert!(!c.contains("void nomo_payload_Ok;"));
    assert!(c.contains(".payload.nomo_payload_Ok = 0"));
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
