use super::*;

#[test]
fn emits_defer_before_panic_statement() {
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
                name: "main".to_string(),
                params: Vec::new(),
                return_type: ValueType::Void,
                body: vec![
                    Statement::Defer {
                        call: DeferredCall::Expr(ValueExpr::Call {
                            name: "cleanup".to_string(),
                            args: Vec::new(),
                        }),
                    },
                    Statement::Panic(ValueExpr::StringLiteral("boom".to_string())),
                ],
            },
        ],
    };

    let c = emit_c(&program);
    let cleanup = c.find("nomo_fn_cleanup();").unwrap();
    let panic = c.find(&panic_literal("boom")).unwrap();
    assert!(cleanup < panic);
    assert_eq!(c.matches("nomo_fn_cleanup();").count(), 1);
}

#[test]
fn emits_defer_at_fallthrough_function_exit() {
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
                name: "main".to_string(),
                params: Vec::new(),
                return_type: ValueType::Void,
                body: vec![
                    Statement::Defer {
                        call: DeferredCall::Expr(ValueExpr::Call {
                            name: "cleanup".to_string(),
                            args: Vec::new(),
                        }),
                    },
                    Statement::Println(ValueExpr::StringLiteral("working".to_string())),
                ],
            },
        ],
    };

    let c = emit_c(&program);
    let working = c.find(&puts_literal("working")).unwrap();
    let cleanup = c.find("nomo_fn_cleanup();").unwrap();
    assert!(working < cleanup);
    assert_eq!(c.matches("nomo_fn_cleanup();").count(), 1);
}

#[test]
fn emits_deferred_println_at_fallthrough_exit() {
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
                Statement::Defer {
                    call: DeferredCall::Println(ValueExpr::StringLiteral("cleanup".to_string())),
                },
                Statement::Println(ValueExpr::StringLiteral("working".to_string())),
            ],
        }],
    };

    let c = emit_c(&program);
    let working = c.find(&puts_literal("working")).unwrap();
    let cleanup = c.find(&puts_literal("cleanup")).unwrap();
    assert!(working < cleanup);
}

#[test]
fn emits_nested_block_defer_at_block_fallthrough_exit() {
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
                Statement::Defer {
                    call: DeferredCall::Println(ValueExpr::StringLiteral("outer".to_string())),
                },
                Statement::Match {
                    value: ValueExpr::EnumVariant {
                        enum_name: "Color".to_string(),
                        enum_args: Vec::new(),
                        variant: "Red".to_string(),
                        payload: None,
                    },
                    enum_name: "Color".to_string(),
                    enum_args: Vec::new(),
                    arms: vec![
                        MatchStatementArm {
                            variant: "Red".to_string(),
                            binding: None,
                            body: vec![
                                Statement::Defer {
                                    call: DeferredCall::Println(ValueExpr::StringLiteral(
                                        "inner".to_string(),
                                    )),
                                },
                                Statement::Println(ValueExpr::StringLiteral("red".to_string())),
                            ],
                        },
                        MatchStatementArm {
                            variant: "Blue".to_string(),
                            binding: None,
                            body: vec![Statement::Println(ValueExpr::StringLiteral(
                                "blue".to_string(),
                            ))],
                        },
                    ],
                },
                Statement::Println(ValueExpr::StringLiteral("after".to_string())),
            ],
        }],
    };

    let c = emit_c(&program);
    let red = c.find(&puts_literal("red")).unwrap();
    let inner = c[red..].find(&puts_literal("inner")).unwrap() + red;
    let after = c[inner..].find(&puts_literal("after")).unwrap() + inner;
    let outer = c[after..].find(&puts_literal("outer")).unwrap() + after;
    assert!(red < inner);
    assert!(inner < after);
    assert!(after < outer);
    assert_eq!(c.matches(&puts_literal("inner")).count(), 1);
    assert_eq!(c.matches(&puts_literal("outer")).count(), 1);
}

#[test]
fn emits_nested_block_defer_before_return_and_outer_defer() {
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
                Statement::Defer {
                    call: DeferredCall::Println(ValueExpr::StringLiteral("outer".to_string())),
                },
                Statement::Match {
                    value: ValueExpr::EnumVariant {
                        enum_name: "Color".to_string(),
                        enum_args: Vec::new(),
                        variant: "Red".to_string(),
                        payload: None,
                    },
                    enum_name: "Color".to_string(),
                    enum_args: Vec::new(),
                    arms: vec![
                        MatchStatementArm {
                            variant: "Red".to_string(),
                            binding: None,
                            body: vec![
                                Statement::Defer {
                                    call: DeferredCall::Println(ValueExpr::StringLiteral(
                                        "inner".to_string(),
                                    )),
                                },
                                Statement::Return(None),
                            ],
                        },
                        MatchStatementArm {
                            variant: "Blue".to_string(),
                            binding: None,
                            body: Vec::new(),
                        },
                    ],
                },
            ],
        }],
    };

    let c = emit_c(&program);
    let inner = c.find(&puts_literal("inner")).unwrap();
    let outer = c[inner..].find(&puts_literal("outer")).unwrap() + inner;
    let return_stmt = c[outer..].find("return;").unwrap() + outer;
    assert!(inner < outer);
    assert!(outer < return_stmt);
    assert_eq!(c.matches(&puts_literal("inner")).count(), 1);
    assert_eq!(c.matches(&puts_literal("outer")).count(), 2);
}

#[test]
fn emits_loop_defer_before_break_without_function_defer() {
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
                Statement::Defer {
                    call: DeferredCall::Println(ValueExpr::StringLiteral("outer".to_string())),
                },
                Statement::Loop {
                    kind: LoopKind::Infinite,
                    body: vec![
                        Statement::Defer {
                            call: DeferredCall::Println(ValueExpr::StringLiteral(
                                "inner".to_string(),
                            )),
                        },
                        Statement::Break,
                    ],
                },
            ],
        }],
    };

    let c = emit_c(&program);
    let inner = c.find(&puts_literal("inner")).unwrap();
    let break_stmt = c[inner..].find("break;").unwrap() + inner;
    let outer = c[break_stmt..].find(&puts_literal("outer")).unwrap() + break_stmt;
    assert!(inner < break_stmt);
    assert!(break_stmt < outer);
    assert_eq!(c.matches(&puts_literal("inner")).count(), 1);
    assert_eq!(c.matches(&puts_literal("outer")).count(), 1);
}

#[test]
fn emits_loop_defer_before_continue_without_function_defer() {
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
                Statement::Defer {
                    call: DeferredCall::Println(ValueExpr::StringLiteral("outer".to_string())),
                },
                Statement::Loop {
                    kind: LoopKind::Infinite,
                    body: vec![
                        Statement::Defer {
                            call: DeferredCall::Println(ValueExpr::StringLiteral(
                                "inner".to_string(),
                            )),
                        },
                        Statement::Continue,
                    ],
                },
            ],
        }],
    };

    let c = emit_c(&program);
    let inner = c.find(&puts_literal("inner")).unwrap();
    let continue_stmt = c[inner..].find("continue;").unwrap() + inner;
    let outer = c[continue_stmt..].find(&puts_literal("outer")).unwrap() + continue_stmt;
    assert!(inner < continue_stmt);
    assert!(continue_stmt < outer);
    assert_eq!(c.matches(&puts_literal("inner")).count(), 1);
    assert_eq!(c.matches(&puts_literal("outer")).count(), 1);
}

#[test]
fn inner_loop_break_only_runs_inner_loop_defer() {
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
            body: vec![Statement::Loop {
                kind: LoopKind::Infinite,
                body: vec![
                    Statement::Defer {
                        call: DeferredCall::Println(ValueExpr::StringLiteral(
                            "outer loop".to_string(),
                        )),
                    },
                    Statement::Loop {
                        kind: LoopKind::Infinite,
                        body: vec![
                            Statement::Defer {
                                call: DeferredCall::Println(ValueExpr::StringLiteral(
                                    "inner loop".to_string(),
                                )),
                            },
                            Statement::Break,
                        ],
                    },
                    Statement::Break,
                ],
            }],
        }],
    };

    let c = emit_c(&program);
    let inner = c.find(&puts_literal("inner loop")).unwrap();
    let inner_break = c[inner..].find("break;").unwrap() + inner;
    let outer = c[inner_break..].find(&puts_literal("outer loop")).unwrap() + inner_break;
    let outer_break = c[outer..].find("break;").unwrap() + outer;
    assert!(inner < inner_break);
    assert!(inner_break < outer);
    assert!(outer < outer_break);
    assert_eq!(c.matches(&puts_literal("inner loop")).count(), 1);
    assert_eq!(c.matches(&puts_literal("outer loop")).count(), 1);
}
