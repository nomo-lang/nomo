use super::*;

#[test]
fn emits_puts_for_println() {
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
            body: vec![Statement::Println(ValueExpr::StringLiteral(
                "Hello".to_string(),
            ))],
        }],
    };

    let c = emit_c(&program);
    assert!(c.contains("#include <stdio.h>"));
    assert!(c.contains(&puts_literal("Hello")));
}

#[test]
fn emits_package_prefixed_function_symbol_macros() {
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
                name: "add".to_string(),
                params: vec![
                    Parameter {
                        name: "a".to_string(),
                        value_type: ValueType::I32,
                        mutable: false,
                    },
                    Parameter {
                        name: "b".to_string(),
                        value_type: ValueType::I32,
                        mutable: false,
                    },
                ],
                return_type: ValueType::I32,
                body: vec![Statement::Return(Some(ValueExpr::Binary {
                    op: BinaryOp::Add,
                    left: Box::new(ValueExpr::Variable("a".to_string())),
                    right: Box::new(ValueExpr::Variable("b".to_string())),
                    value_type: ValueType::I32,
                }))],
            },
            Function {
                package: "app.main".to_string(),
                name: "main".to_string(),
                params: Vec::new(),
                return_type: ValueType::Void,
                body: vec![Statement::Expr(ValueExpr::Call {
                    name: "add".to_string(),
                    args: vec![ValueExpr::IntLiteral(1), ValueExpr::IntLiteral(2)],
                })],
            },
        ],
    };

    let c = emit_c(&program);

    assert!(c.contains("#define nomo_fn_add nomo_pkg_app_main_fn_add"));
    assert!(c.contains("#define nomo_fn_main nomo_pkg_app_main_fn_main"));
    assert!(c.contains("int32_t nomo_fn_add(int32_t nomo_a, int32_t nomo_b);"));
    assert!(c.contains("nomo_fn_add(1, 2);"));
}

#[test]
fn emits_package_prefixed_type_symbol_macros() {
    let program = Program {
        consts: Vec::new(),
        package: "app.main".to_string(),
        imports: Vec::new(),
        extern_functions: Vec::new(),
        structs: vec![StructType {
            package: "app.main".to_string(),
            name: "Point".to_string(),
            type_params: Vec::new(),
            fields: vec![StructField {
                name: "x".to_string(),
                value_type: ValueType::I32,
            }],
        }],
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
                    name: "point".to_string(),
                    value_type: ValueType::Struct("Point".to_string(), Vec::new()),
                    initializer: ValueExpr::StructLiteral {
                        type_name: "Point".to_string(),
                        struct_args: Vec::new(),
                        fields: vec![("x".to_string(), ValueExpr::IntLiteral(1))],
                    },
                },
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
            ],
        }],
    };

    let c = emit_c(&program);

    assert!(c.contains("#define nomo_struct_Point nomo_pkg_app_main_struct_Point"));
    assert!(c.contains("#define nomo_enum_Color_tag nomo_pkg_app_main_enum_Color_tag"));
    assert!(c.contains("#define nomo_enum_Color nomo_pkg_app_main_enum_Color"));
    assert!(c.contains("#define nomo_enum_Color_Red nomo_pkg_app_main_enum_Color_Red"));
    assert!(c.contains("#define nomo_enum_Color_Blue nomo_pkg_app_main_enum_Color_Blue"));
}

#[test]
fn emits_fputs_for_eprintln() {
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
            body: vec![Statement::Eprintln(ValueExpr::StringLiteral(
                "error".to_string(),
            ))],
        }],
    };

    let c = emit_c(&program);
    assert!(c.contains(&fputs_literal("error")));
    assert!(c.contains("fputc('\\n', stderr);"));
}

#[test]
fn emits_fputs_for_print_without_newline() {
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
            body: vec![Statement::Print(ValueExpr::StringLiteral(
                "partial".to_string(),
            ))],
        }],
    };

    let c = emit_c(&program);
    assert!(c.contains(&fputs_stdout_literal("partial")));
    assert!(!c.contains(&puts_literal("partial")));
}

#[test]
fn emits_fputs_for_eprint_without_newline() {
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
            body: vec![Statement::Eprint(ValueExpr::StringLiteral(
                "partial error".to_string(),
            ))],
        }],
    };

    let c = emit_c(&program);
    assert!(c.contains(&fputs_literal("partial error")));
    assert!(!c.contains(&format!(
        "{}\n    fputc('\\n', stderr);",
        fputs_literal("partial error")
    )));
}
