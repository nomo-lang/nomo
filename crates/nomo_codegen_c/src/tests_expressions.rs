use super::*;

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
