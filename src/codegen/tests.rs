use super::*;
use nomo_ir::{EnumVariantType, MatchValueArm, Parameter, StructField, ValueExpr};

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
fn emits_float_literal_and_cast() {
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
                name: "ratio".to_string(),
                params: vec![Parameter {
                    name: "age".to_string(),
                    mutable: false,
                    value_type: ValueType::Int,
                }],
                return_type: ValueType::Float,
                body: vec![Statement::Return(Some(ValueExpr::Cast {
                    expr: Box::new(ValueExpr::Variable("age".to_string())),
                    target_type: ValueType::Float,
                }))],
            },
            Function {
                package: "app.main".to_string(),
                name: "main".to_string(),
                params: Vec::new(),
                return_type: ValueType::Void,
                body: vec![
                    Statement::Let {
                        name: "pi".to_string(),
                        value_type: ValueType::Float,
                        initializer: ValueExpr::FloatLiteral("3.14".to_string()),
                    },
                    Statement::Println(ValueExpr::StringLiteral("done".to_string())),
                ],
            },
        ],
    };

    let c = emit_c(&program);
    assert!(c.contains("double nomo_fn_ratio(long long nomo_age);"));
    assert!(c.contains("return ((double)nomo_age);"));
    assert!(c.contains("double nomo_pi = 3.14;"));
}

#[test]
fn emits_char_literal() {
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
                name: "initial".to_string(),
                params: Vec::new(),
                return_type: ValueType::Char,
                body: vec![Statement::Return(Some(ValueExpr::CharLiteral('語')))],
            },
            Function {
                package: "app.main".to_string(),
                name: "main".to_string(),
                params: Vec::new(),
                return_type: ValueType::Void,
                body: vec![
                    Statement::Let {
                        name: "letter".to_string(),
                        value_type: ValueType::Char,
                        initializer: ValueExpr::Call {
                            name: "initial".to_string(),
                            args: Vec::new(),
                        },
                    },
                    Statement::Println(ValueExpr::StringLiteral("done".to_string())),
                ],
            },
        ],
    };

    let c = emit_c(&program);
    assert!(c.contains("uint32_t nomo_fn_initial(void);"));
    assert!(c.contains("return 35486;"));
    assert!(c.contains("uint32_t nomo_letter = nomo_fn_initial();"));
}

#[test]
fn emits_char_helpers() {
    let program = Program {
        consts: Vec::new(),
        package: "app.main".to_string(),
        imports: vec!["std.char".to_string()],
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
                    name: "digit".to_string(),
                    value_type: ValueType::Bool,
                    initializer: ValueExpr::CharIsDigit {
                        value: Box::new(ValueExpr::CharLiteral('7')),
                    },
                },
                Statement::Let {
                    name: "alpha".to_string(),
                    value_type: ValueType::Bool,
                    initializer: ValueExpr::CharIsAlpha {
                        value: Box::new(ValueExpr::CharLiteral('N')),
                    },
                },
                Statement::Let {
                    name: "space".to_string(),
                    value_type: ValueType::Bool,
                    initializer: ValueExpr::CharIsWhitespace {
                        value: Box::new(ValueExpr::CharLiteral(' ')),
                    },
                },
                Statement::Let {
                    name: "text".to_string(),
                    value_type: ValueType::String,
                    initializer: ValueExpr::CharToString {
                        value: Box::new(ValueExpr::CharLiteral('語')),
                    },
                },
            ],
        }],
    };

    let c = emit_c(&program);
    assert!(c.contains("static int nomo_char_is_digit(uint32_t value)"));
    assert!(c.contains("static nomo_string nomo_char_to_string(uint32_t value)"));
    assert!(c.contains("int nomo_digit = nomo_char_is_digit(55);"));
    assert!(c.contains("nomo_string nomo_text = nomo_char_to_string(35486);"));
}

#[test]
fn emits_os_helpers() {
    let program = Program {
        consts: Vec::new(),
        package: "app.main".to_string(),
        imports: vec!["std.os".to_string()],
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
                    name: "platform".to_string(),
                    value_type: ValueType::String,
                    initializer: ValueExpr::OsPlatform,
                },
                Statement::Let {
                    name: "arch".to_string(),
                    value_type: ValueType::String,
                    initializer: ValueExpr::OsArch,
                },
                Statement::Let {
                    name: "separator".to_string(),
                    value_type: ValueType::String,
                    initializer: ValueExpr::OsPathSeparator,
                },
                Statement::Let {
                    name: "ending".to_string(),
                    value_type: ValueType::String,
                    initializer: ValueExpr::OsLineEnding,
                },
            ],
        }],
    };

    let c = emit_c(&program);
    assert!(c.contains("static nomo_string nomo_os_platform(void)"));
    assert!(c.contains("static nomo_string nomo_os_arch(void)"));
    assert!(c.contains("nomo_string nomo_platform = nomo_os_platform();"));
    assert!(c.contains("nomo_string nomo_separator = nomo_os_path_separator();"));
}

#[test]
fn emits_time_helpers() {
    let program = Program {
        consts: Vec::new(),
        package: "app.main".to_string(),
        imports: vec!["std.time".to_string()],
        extern_functions: Vec::new(),
        structs: vec![StructType {
            package: "std.time".to_string(),
            name: "Duration".to_string(),
            type_params: Vec::new(),
            fields: vec![StructField {
                name: "millis".to_string(),
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
                    name: "now".to_string(),
                    value_type: ValueType::Int,
                    initializer: ValueExpr::TimeNowMillis,
                },
                Statement::Let {
                    name: "tick".to_string(),
                    value_type: ValueType::Int,
                    initializer: ValueExpr::TimeMonotonicMillis,
                },
                Statement::Expr(ValueExpr::TimeSleepMillis {
                    duration: Box::new(ValueExpr::IntLiteral(0)),
                }),
                Statement::Let {
                    name: "duration".to_string(),
                    value_type: ValueType::Struct("Duration".to_string(), Vec::new()),
                    initializer: ValueExpr::TimeDurationMillis {
                        millis: Box::new(ValueExpr::IntLiteral(1500)),
                    },
                },
                Statement::Let {
                    name: "seconds".to_string(),
                    value_type: ValueType::Struct("Duration".to_string(), Vec::new()),
                    initializer: ValueExpr::TimeDurationSeconds {
                        seconds: Box::new(ValueExpr::IntLiteral(2)),
                    },
                },
                Statement::Let {
                    name: "millis".to_string(),
                    value_type: ValueType::Int,
                    initializer: ValueExpr::TimeDurationAsMillis {
                        duration: Box::new(ValueExpr::Variable("duration".to_string())),
                    },
                },
                Statement::Let {
                    name: "label".to_string(),
                    value_type: ValueType::String,
                    initializer: ValueExpr::TimeFormatDuration {
                        duration: Box::new(ValueExpr::Variable("duration".to_string())),
                    },
                },
                Statement::Expr(ValueExpr::TimeSleep {
                    duration: Box::new(ValueExpr::Variable("duration".to_string())),
                }),
            ],
        }],
    };

    let c = emit_c(&program);
    assert!(c.contains("#define nomo_struct_Duration nomo_pkg_std_time_struct_Duration"));
    assert!(c.contains("static int64_t nomo_time_now_millis(void)"));
    assert!(c.contains("static int64_t nomo_time_monotonic_millis(void)"));
    assert!(c.contains("static void nomo_time_sleep_millis(int64_t duration)"));
    assert!(c.contains("static int64_t nomo_time_duration_seconds_to_millis(int64_t seconds)"));
    assert!(c.contains("static nomo_string nomo_time_format_duration_millis(int64_t millis)"));
    assert!(c.contains("nomo_now = nomo_time_now_millis();"));
    assert!(c.contains("nomo_time_sleep_millis(0);"));
    assert!(c.contains(
        "nomo_struct_Duration nomo_duration = (nomo_struct_Duration){ .nomo_member_millis = 1500 };"
    ));
    assert!(c.contains(
            "nomo_struct_Duration nomo_seconds = (nomo_struct_Duration){ .nomo_member_millis = nomo_time_duration_seconds_to_millis(2) };"
        ));
    assert!(c.contains("long long nomo_millis = (nomo_duration).nomo_member_millis;"));
    assert!(c.contains(
            "nomo_string nomo_label = nomo_time_format_duration_millis((nomo_duration).nomo_member_millis);"
        ));
    assert!(c.contains("nomo_time_sleep_millis((nomo_duration).nomo_member_millis);"));
}

#[test]
fn emits_process_helpers() {
    let process_error = ValueType::Struct("ProcessError".to_string(), Vec::new());
    let program = Program {
        consts: Vec::new(),
        package: "app.main".to_string(),
        imports: vec!["std.process".to_string()],
        extern_functions: Vec::new(),
        structs: vec![
            StructType {
                package: "std.process".to_string(),
                name: "ProcessError".to_string(),
                type_params: Vec::new(),
                fields: vec![StructField {
                    name: "message".to_string(),
                    value_type: ValueType::String,
                }],
            },
            StructType {
                package: "std.process".to_string(),
                name: "ProcessOutput".to_string(),
                type_params: Vec::new(),
                fields: vec![
                    StructField {
                        name: "status".to_string(),
                        value_type: ValueType::I32,
                    },
                    StructField {
                        name: "stdout".to_string(),
                        value_type: ValueType::String,
                    },
                    StructField {
                        name: "stderr".to_string(),
                        value_type: ValueType::String,
                    },
                ],
            },
        ],
        enums: vec![EnumType {
            package: "std.result".to_string(),
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
        functions: vec![Function {
            package: "app.main".to_string(),
            name: "main".to_string(),
            params: Vec::new(),
            return_type: ValueType::Void,
            body: vec![
                Statement::Let {
                    name: "spawned".to_string(),
                    value_type: ValueType::Enum(
                        "Result".to_string(),
                        vec![ValueType::I32, process_error.clone()],
                    ),
                    initializer: ValueExpr::ProcessSpawn {
                        command: Box::new(ValueExpr::StringLiteral("printf ok".to_string())),
                    },
                },
                Statement::Let {
                    name: "status".to_string(),
                    value_type: ValueType::Enum(
                        "Result".to_string(),
                        vec![ValueType::I32, process_error.clone()],
                    ),
                    initializer: ValueExpr::ProcessStatus {
                        command: Box::new(ValueExpr::StringLiteral("printf ok".to_string())),
                    },
                },
                Statement::Let {
                    name: "output".to_string(),
                    value_type: ValueType::Enum(
                        "Result".to_string(),
                        vec![ValueType::String, process_error.clone()],
                    ),
                    initializer: ValueExpr::ProcessExec {
                        command: Box::new(ValueExpr::StringLiteral("printf ok".to_string())),
                    },
                },
                Statement::Let {
                    name: "captured".to_string(),
                    value_type: ValueType::Enum(
                        "Result".to_string(),
                        vec![
                            ValueType::Struct("ProcessOutput".to_string(), Vec::new()),
                            process_error,
                        ],
                    ),
                    initializer: ValueExpr::ProcessOutput {
                        command: Box::new(ValueExpr::StringLiteral(
                            "printf ok; printf err 1>&2".to_string(),
                        )),
                    },
                },
                Statement::Expr(ValueExpr::ProcessExit {
                    code: Box::new(ValueExpr::IntLiteral(0)),
                }),
            ],
        }],
    };

    let c = emit_c(&program);
    assert!(c.contains("static int32_t nomo_process_exit_code(int status)"));
    assert!(c.contains("nomo_process_spawn(nomo_string command)"));
    assert!(c.contains("nomo_process_status(nomo_string command)"));
    assert!(c.contains("nomo_process_exec(nomo_string command)"));
    assert!(c.contains("nomo_process_output(nomo_string command)"));
    assert!(c.contains("nomo_spawned = nomo_process_spawn(nomo_string_literal(\"printf ok\"));"));
    assert!(c.contains("return nomo_process_spawn(command);"));
    assert!(c.contains("nomo_status = nomo_process_status(nomo_string_literal(\"printf ok\"));"));
    assert!(c.contains("nomo_output = nomo_process_exec(nomo_string_literal(\"printf ok\"));"));
    assert!(c.contains(
        "nomo_captured = nomo_process_output(nomo_string_literal(\"printf ok; printf err 1>&2\"));"
    ));
    assert!(c.contains("exit((int)0);"));
}

#[test]
fn emits_fixed_width_integer_types() {
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
                name: "add32".to_string(),
                params: vec![
                    Parameter {
                        name: "a".to_string(),
                        mutable: false,
                        value_type: ValueType::I32,
                    },
                    Parameter {
                        name: "b".to_string(),
                        mutable: false,
                        value_type: ValueType::I32,
                    },
                ],
                return_type: ValueType::I32,
                body: vec![Statement::Return(Some(ValueExpr::Binary {
                    left: Box::new(ValueExpr::Variable("a".to_string())),
                    op: BinaryOp::Add,
                    right: Box::new(ValueExpr::Variable("b".to_string())),
                    value_type: ValueType::I32,
                }))],
            },
            Function {
                package: "app.main".to_string(),
                name: "main".to_string(),
                params: Vec::new(),
                return_type: ValueType::Void,
                body: vec![
                    Statement::Let {
                        name: "signed".to_string(),
                        value_type: ValueType::I32,
                        initializer: ValueExpr::IntLiteral(1),
                    },
                    Statement::Let {
                        name: "word".to_string(),
                        value_type: ValueType::U32,
                        initializer: ValueExpr::IntLiteral(2),
                    },
                    Statement::Let {
                        name: "wide".to_string(),
                        value_type: ValueType::U64,
                        initializer: ValueExpr::IntLiteral(3),
                    },
                    Statement::Println(ValueExpr::StringLiteral("done".to_string())),
                ],
            },
        ],
    };

    let c = emit_c(&program);
    assert!(c.contains("#include <stdint.h>"));
    assert!(c.contains("int32_t nomo_fn_add32(int32_t nomo_a, int32_t nomo_b);"));
    assert!(c.contains("int32_t nomo_signed = 1;"));
    assert!(c.contains("uint32_t nomo_word = 2;"));
    assert!(c.contains("uint64_t nomo_wide = 3;"));
}

#[test]
fn emits_crypto_random_bytes_helper_after_array_u32_helper() {
    let program = Program {
        consts: Vec::new(),
        package: "app.main".to_string(),
        imports: vec!["std.crypto".to_string()],
        extern_functions: Vec::new(),
        structs: Vec::new(),
        enums: vec![EnumType {
            package: "std.option".to_string(),
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
            body: vec![Statement::Let {
                name: "bytes".to_string(),
                value_type: ValueType::Array(Box::new(ValueType::U32)),
                initializer: ValueExpr::CryptoRandomBytes {
                    count: Box::new(ValueExpr::Cast {
                        expr: Box::new(ValueExpr::IntLiteral(4)),
                        target_type: ValueType::U64,
                    }),
                },
            }],
        }],
    };

    let c = emit_c(&program);
    let array_helper = c
        .find("static nomo_array_u32 nomo_array_u32_new(void)")
        .unwrap();
    let crypto_helper = c
        .find("static nomo_array_u32 nomo_crypto_random_bytes(uint64_t count)")
        .unwrap();
    assert!(array_helper < crypto_helper);
    assert!(c.contains("#define _CRT_RAND_S"));
    assert!(c.contains("nomo_array_u32_push"));
    assert!(c.contains("nomo_array_u32 nomo_bytes = nomo_crypto_random_bytes(((uint64_t)4));"));
}

#[test]
fn emits_string_len_and_concat() {
    let program = Program {
        consts: Vec::new(),
        package: "app.main".to_string(),
        imports: vec!["std.io".to_string(), "std.string".to_string()],
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
                    name: "message".to_string(),
                    value_type: ValueType::String,
                    initializer: ValueExpr::StringConcat {
                        left: Box::new(ValueExpr::StringLiteral("No".to_string())),
                        right: Box::new(ValueExpr::StringLiteral("mo".to_string())),
                    },
                },
                Statement::Let {
                    name: "count".to_string(),
                    value_type: ValueType::U64,
                    initializer: ValueExpr::StringLen {
                        value: Box::new(ValueExpr::Variable("message".to_string())),
                    },
                },
            ],
        }],
    };

    let c = emit_c(&program);
    assert!(c.contains("#include <string.h>"));
    assert!(c.contains("static nomo_string nomo_string_concat"));
    assert!(c.contains(
            "nomo_string nomo_message = nomo_string_concat(nomo_string_literal(\"No\"), nomo_string_literal(\"mo\"));"
        ));
    assert!(c.contains("uint64_t nomo_count = ((uint64_t)strlen((nomo_message).data));"));
    assert!(c.contains("nomo_string_release(nomo_message);"));
}

#[test]
fn emits_string_retain_and_release_for_shared_bindings() {
    let program = Program {
        consts: Vec::new(),
        package: "app.main".to_string(),
        imports: vec!["std.io".to_string(), "std.string".to_string()],
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
                    name: "first".to_string(),
                    value_type: ValueType::String,
                    initializer: ValueExpr::StringConcat {
                        left: Box::new(ValueExpr::StringLiteral("No".to_string())),
                        right: Box::new(ValueExpr::StringLiteral("mo".to_string())),
                    },
                },
                Statement::Let {
                    name: "second".to_string(),
                    value_type: ValueType::String,
                    initializer: ValueExpr::Variable("first".to_string()),
                },
            ],
        }],
    };

    let c = emit_c(&program);
    assert!(c.contains("typedef struct nomo_string"));
    assert!(c.contains("static nomo_string nomo_string_retain(nomo_string value)"));
    assert!(c.contains("static void nomo_string_release(nomo_string value)"));
    assert!(c.contains("nomo_second = nomo_string_retain(nomo_second);"));
    let retain = c
        .find("nomo_second = nomo_string_retain(nomo_second);")
        .unwrap();
    let release_second = c[retain..]
        .find("nomo_string_release(nomo_second);")
        .unwrap()
        + retain;
    let release_first = c[release_second..]
        .find("nomo_string_release(nomo_first);")
        .unwrap()
        + release_second;
    assert!(retain < release_second);
    assert!(release_second < release_first);
}

#[test]
fn emits_string_parameter_retain_before_return_release() {
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
                name: "echo".to_string(),
                params: vec![Parameter {
                    name: "value".to_string(),
                    mutable: false,
                    value_type: ValueType::String,
                }],
                return_type: ValueType::String,
                body: vec![Statement::Return(Some(ValueExpr::Variable(
                    "value".to_string(),
                )))],
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
    let fn_start = c
        .find("nomo_string nomo_fn_echo(nomo_string nomo_value)")
        .unwrap();
    let param_retain = c[fn_start..]
        .find("nomo_value = nomo_string_retain(nomo_value);")
        .unwrap()
        + fn_start;
    let return_retain = c[param_retain..]
        .find("nomo__return = nomo_string_retain(nomo__return);")
        .unwrap()
        + param_retain;
    let param_release = c[return_retain..]
        .find("nomo_string_release(nomo_value);")
        .unwrap()
        + return_retain;
    let return_stmt = c[param_release..].find("return nomo__return;").unwrap() + param_release;
    assert!(fn_start < param_retain);
    assert!(param_retain < return_retain);
    assert!(return_retain < param_release);
    assert!(param_release < return_stmt);
}

#[test]
fn emits_fs_read_and_write_helpers() {
    let fs_error = ValueType::Struct("FsError".to_string(), Vec::new());
    let result_string_error = ValueType::Enum(
        "Result".to_string(),
        vec![ValueType::String, fs_error.clone()],
    );
    let result_array_u32_error = ValueType::Enum(
        "Result".to_string(),
        vec![ValueType::Array(Box::new(ValueType::U32)), fs_error.clone()],
    );
    let result_void_error = ValueType::Enum(
        "Result".to_string(),
        vec![ValueType::Void, fs_error.clone()],
    );
    let result_metadata_error = ValueType::Enum(
        "Result".to_string(),
        vec![
            ValueType::Struct("FileMetadata".to_string(), Vec::new()),
            fs_error.clone(),
        ],
    );
    let result_array_string_error = ValueType::Enum(
        "Result".to_string(),
        vec![
            ValueType::Array(Box::new(ValueType::String)),
            fs_error.clone(),
        ],
    );
    let program = Program {
        consts: Vec::new(),
        package: "app.main".to_string(),
        imports: vec!["std.fs".to_string()],
        extern_functions: Vec::new(),
        structs: vec![
            StructType {
                package: "app.main".to_string(),
                name: "FsError".to_string(),
                type_params: Vec::new(),
                fields: vec![StructField {
                    name: "message".to_string(),
                    value_type: ValueType::String,
                }],
            },
            StructType {
                package: "app.main".to_string(),
                name: "FileMetadata".to_string(),
                type_params: Vec::new(),
                fields: vec![
                    StructField {
                        name: "is_file".to_string(),
                        value_type: ValueType::Bool,
                    },
                    StructField {
                        name: "is_dir".to_string(),
                        value_type: ValueType::Bool,
                    },
                    StructField {
                        name: "size".to_string(),
                        value_type: ValueType::U64,
                    },
                ],
            },
        ],
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
        functions: vec![Function {
            package: "app.main".to_string(),
            name: "main".to_string(),
            params: Vec::new(),
            return_type: ValueType::Void,
            body: vec![
                Statement::Let {
                    name: "read_result".to_string(),
                    value_type: result_string_error,
                    initializer: ValueExpr::FsReadToString {
                        path: Box::new(ValueExpr::StringLiteral("input.txt".to_string())),
                    },
                },
                Statement::Let {
                    name: "write_result".to_string(),
                    value_type: result_void_error.clone(),
                    initializer: ValueExpr::FsWriteString {
                        path: Box::new(ValueExpr::StringLiteral("output.txt".to_string())),
                        content: Box::new(ValueExpr::StringLiteral("hello".to_string())),
                    },
                },
                Statement::Let {
                    name: "bytes_result".to_string(),
                    value_type: result_array_u32_error,
                    initializer: ValueExpr::FsReadBytes {
                        path: Box::new(ValueExpr::StringLiteral("input.bin".to_string())),
                    },
                },
                Statement::Let {
                    name: "write_bytes_result".to_string(),
                    value_type: result_void_error.clone(),
                    initializer: ValueExpr::FsWriteBytes {
                        path: Box::new(ValueExpr::StringLiteral("output.bin".to_string())),
                        bytes: Box::new(ValueExpr::CryptoRandomBytes {
                            count: Box::new(ValueExpr::Cast {
                                expr: Box::new(ValueExpr::IntLiteral(2)),
                                target_type: ValueType::U64,
                            }),
                        }),
                    },
                },
                Statement::Let {
                    name: "exists".to_string(),
                    value_type: ValueType::Bool,
                    initializer: ValueExpr::FsExists {
                        path: Box::new(ValueExpr::StringLiteral("tmp".to_string())),
                    },
                },
                Statement::Let {
                    name: "metadata_result".to_string(),
                    value_type: result_metadata_error,
                    initializer: ValueExpr::FsMetadata {
                        path: Box::new(ValueExpr::StringLiteral("tmp".to_string())),
                    },
                },
                Statement::Let {
                    name: "create_result".to_string(),
                    value_type: result_void_error.clone(),
                    initializer: ValueExpr::FsCreateDir {
                        path: Box::new(ValueExpr::StringLiteral("tmp".to_string())),
                    },
                },
                Statement::Let {
                    name: "entries_result".to_string(),
                    value_type: result_array_string_error,
                    initializer: ValueExpr::FsReadDir {
                        path: Box::new(ValueExpr::StringLiteral("tmp".to_string())),
                    },
                },
                Statement::Let {
                    name: "remove_result".to_string(),
                    value_type: result_void_error,
                    initializer: ValueExpr::FsRemoveDir {
                        path: Box::new(ValueExpr::StringLiteral("tmp".to_string())),
                    },
                },
            ],
        }],
    };

    let c = emit_c(&program);
    assert!(c.contains("#include <errno.h>"));
    assert!(c.contains("typedef struct nomo_struct_FsError"));
    assert!(c.contains("typedef struct nomo_struct_FileMetadata"));
    assert!(c.contains("static nomo_enum_Result_string_struct_FsError nomo_fs_read_to_string"));
    assert!(c.contains("static nomo_enum_Result_void_struct_FsError nomo_fs_write_string"));
    assert!(c.contains("static nomo_enum_Result_array_u32_struct_FsError nomo_fs_read_bytes"));
    assert!(c.contains("static nomo_enum_Result_void_struct_FsError nomo_fs_write_bytes"));
    assert!(c.contains("static int nomo_fs_exists"));
    assert!(
        c.contains("static nomo_enum_Result_struct_FileMetadata_struct_FsError nomo_fs_metadata")
    );
    assert!(c.contains("static nomo_enum_Result_void_struct_FsError nomo_fs_create_dir"));
    assert!(c.contains("static nomo_enum_Result_void_struct_FsError nomo_fs_remove_dir"));
    assert!(c.contains("static nomo_enum_Result_array_string_struct_FsError nomo_fs_read_dir"));
    assert!(c.contains("typedef struct nomo_array_string"));
    assert!(c.contains("nomo_fs_read_to_string(nomo_string_literal(\"input.txt\"))"));
    assert!(c.contains(
        "nomo_fs_write_string(nomo_string_literal(\"output.txt\"), nomo_string_literal(\"hello\"))"
    ));
    assert!(c.contains("nomo_array_u32_new"));
    let array_helper = c
        .find("static nomo_array_u32 nomo_array_u32_new(void)")
        .unwrap();
    let read_bytes_helper = c
        .find("static nomo_enum_Result_array_u32_struct_FsError nomo_fs_read_bytes")
        .unwrap();
    assert!(array_helper < read_bytes_helper);
    assert!(c.contains("nomo_fs_read_bytes(nomo_string_literal(\"input.bin\"))"));
    assert!(c.contains(
            "nomo_fs_write_bytes(nomo_string_literal(\"output.bin\"), nomo_crypto_random_bytes(((uint64_t)2)))"
        ));
    assert!(c.contains("nomo_fs_exists(nomo_string_literal(\"tmp\"))"));
    assert!(c.contains("nomo_fs_metadata(nomo_string_literal(\"tmp\"))"));
    assert!(c.contains("nomo_fs_create_dir(nomo_string_literal(\"tmp\"))"));
    assert!(c.contains("nomo_fs_read_dir(nomo_string_literal(\"tmp\"))"));
    assert!(c.contains("nomo_fs_remove_dir(nomo_string_literal(\"tmp\"))"));
}

#[test]
fn emits_file_read_write_close_helpers() {
    let fs_error = ValueType::Struct("FsError".to_string(), Vec::new());
    let file_type = ValueType::Struct("File".to_string(), Vec::new());
    let result_string_error = ValueType::Enum(
        "Result".to_string(),
        vec![ValueType::String, fs_error.clone()],
    );
    let result_void_error = ValueType::Enum(
        "Result".to_string(),
        vec![ValueType::Void, fs_error.clone()],
    );
    let program = Program {
        consts: Vec::new(),
        package: "app.main".to_string(),
        imports: vec!["std.fs".to_string()],
        extern_functions: Vec::new(),
        structs: vec![
            StructType {
                package: "app.main".to_string(),
                name: "FsError".to_string(),
                type_params: Vec::new(),
                fields: vec![StructField {
                    name: "message".to_string(),
                    value_type: ValueType::String,
                }],
            },
            StructType {
                package: "app.main".to_string(),
                name: "File".to_string(),
                type_params: Vec::new(),
                fields: Vec::new(),
            },
        ],
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
                name: "process_file".to_string(),
                params: vec![Parameter {
                    name: "file".to_string(),
                    mutable: false,
                    value_type: file_type,
                }],
                return_type: ValueType::Void,
                body: vec![
                    Statement::Let {
                        name: "write_result".to_string(),
                        value_type: result_void_error,
                        initializer: ValueExpr::FileWriteString {
                            file: Box::new(ValueExpr::Variable("file".to_string())),
                            content: Box::new(ValueExpr::StringLiteral("hello".to_string())),
                        },
                    },
                    Statement::Let {
                        name: "read_result".to_string(),
                        value_type: result_string_error,
                        initializer: ValueExpr::FileReadToString {
                            file: Box::new(ValueExpr::Variable("file".to_string())),
                        },
                    },
                    Statement::Expr(ValueExpr::FileClose {
                        file: Box::new(ValueExpr::Variable("file".to_string())),
                    }),
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
    assert!(c.contains("typedef struct nomo_struct_File"));
    assert!(c.contains("static nomo_enum_Result_string_struct_FsError nomo_file_read_to_string"));
    assert!(c.contains("static nomo_enum_Result_void_struct_FsError nomo_file_write_string"));
    assert!(c.contains("static void nomo_file_close"));
    assert!(c.contains("nomo_file_write_string(nomo_file, nomo_string_literal(\"hello\"))"));
    assert!(c.contains("nomo_file_read_to_string(nomo_file)"));
    assert!(c.contains("nomo_file_close(nomo_file)"));
}

#[test]
fn emits_net_tcp_stream_helpers() {
    let net_error = ValueType::Struct("NetError".to_string(), Vec::new());
    let tcp_listener = ValueType::Struct("TcpListener".to_string(), Vec::new());
    let tcp_stream = ValueType::Struct("TcpStream".to_string(), Vec::new());
    let result_listener_error = ValueType::Enum(
        "Result".to_string(),
        vec![tcp_listener.clone(), net_error.clone()],
    );
    let result_stream_error = ValueType::Enum(
        "Result".to_string(),
        vec![tcp_stream.clone(), net_error.clone()],
    );
    let result_string_error = ValueType::Enum(
        "Result".to_string(),
        vec![ValueType::String, net_error.clone()],
    );
    let result_void_error = ValueType::Enum("Result".to_string(), vec![ValueType::Void, net_error]);
    let program = Program {
        consts: Vec::new(),
        package: "app.main".to_string(),
        imports: vec!["std.net".to_string()],
        extern_functions: Vec::new(),
        structs: vec![
            StructType {
                package: "std.net".to_string(),
                name: "NetError".to_string(),
                type_params: Vec::new(),
                fields: vec![StructField {
                    name: "message".to_string(),
                    value_type: ValueType::String,
                }],
            },
            StructType {
                package: "std.net".to_string(),
                name: "TcpListener".to_string(),
                type_params: Vec::new(),
                fields: Vec::new(),
            },
            StructType {
                package: "std.net".to_string(),
                name: "TcpStream".to_string(),
                type_params: Vec::new(),
                fields: Vec::new(),
            },
        ],
        enums: vec![EnumType {
            package: "std.result".to_string(),
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
                name: "process_stream".to_string(),
                params: vec![Parameter {
                    name: "stream".to_string(),
                    mutable: false,
                    value_type: tcp_stream,
                }],
                return_type: ValueType::Void,
                body: vec![
                    Statement::Let {
                        name: "write_result".to_string(),
                        value_type: result_void_error,
                        initializer: ValueExpr::TcpStreamWriteString {
                            stream: Box::new(ValueExpr::Variable("stream".to_string())),
                            content: Box::new(ValueExpr::StringLiteral("ping".to_string())),
                        },
                    },
                    Statement::Let {
                        name: "read_result".to_string(),
                        value_type: result_string_error,
                        initializer: ValueExpr::TcpStreamReadToString {
                            stream: Box::new(ValueExpr::Variable("stream".to_string())),
                        },
                    },
                    Statement::Expr(ValueExpr::TcpStreamClose {
                        stream: Box::new(ValueExpr::Variable("stream".to_string())),
                    }),
                ],
            },
            Function {
                package: "app.main".to_string(),
                name: "process_listener".to_string(),
                params: vec![Parameter {
                    name: "listener".to_string(),
                    mutable: false,
                    value_type: tcp_listener,
                }],
                return_type: ValueType::Void,
                body: vec![
                    Statement::Let {
                        name: "accepted".to_string(),
                        value_type: result_stream_error.clone(),
                        initializer: ValueExpr::TcpListenerAccept {
                            listener: Box::new(ValueExpr::Variable("listener".to_string())),
                        },
                    },
                    Statement::Expr(ValueExpr::TcpListenerClose {
                        listener: Box::new(ValueExpr::Variable("listener".to_string())),
                    }),
                ],
            },
            Function {
                package: "app.main".to_string(),
                name: "main".to_string(),
                params: Vec::new(),
                return_type: ValueType::Void,
                body: vec![
                    Statement::Let {
                        name: "connected".to_string(),
                        value_type: result_stream_error,
                        initializer: ValueExpr::NetConnect {
                            host: Box::new(ValueExpr::StringLiteral("127.0.0.1".to_string())),
                            port: Box::new(ValueExpr::IntLiteral(7)),
                        },
                    },
                    Statement::Let {
                        name: "listening".to_string(),
                        value_type: result_listener_error,
                        initializer: ValueExpr::NetListen {
                            host: Box::new(ValueExpr::StringLiteral("127.0.0.1".to_string())),
                            port: Box::new(ValueExpr::IntLiteral(7)),
                        },
                    },
                ],
            },
        ],
    };

    let c = emit_c(&program);
    assert!(c.contains("typedef struct nomo_struct_TcpListener"));
    assert!(c.contains("typedef struct nomo_struct_TcpStream"));
    assert!(c.contains("nomo_socket nomo_member_handle"));
    assert!(c.contains("static nomo_string nomo_net_error_message(void)"));
    assert!(c.contains("nomo_net_connect(nomo_string host, int64_t port)"));
    assert!(c.contains("nomo_net_listen(nomo_string host, int64_t port)"));
    assert!(c.contains("nomo_tcp_listener_accept(nomo_struct_TcpListener listener)"));
    assert!(c.contains("static void nomo_tcp_listener_close(nomo_struct_TcpListener listener)"));
    assert!(c.contains("nomo_tcp_stream_read_to_string(nomo_struct_TcpStream stream)"));
    assert!(c.contains(
        "nomo_tcp_stream_write_string(nomo_struct_TcpStream stream, nomo_string content)"
    ));
    assert!(c.contains("static void nomo_tcp_stream_close(nomo_struct_TcpStream stream)"));
    assert!(c.contains("nomo_net_connect(nomo_string_literal(\"127.0.0.1\"), 7)"));
    assert!(c.contains("nomo_net_listen(nomo_string_literal(\"127.0.0.1\"), 7)"));
    assert!(c.contains("nomo_tcp_listener_accept(nomo_listener)"));
    assert!(c.contains("nomo_tcp_listener_close(nomo_listener)"));
    assert!(c.contains("nomo_tcp_stream_write_string(nomo_stream, nomo_string_literal(\"ping\"))"));
    assert!(c.contains("nomo_tcp_stream_read_to_string(nomo_stream)"));
    assert!(c.contains("nomo_tcp_stream_close(nomo_stream)"));
}

#[test]
fn emits_net_udp_socket_helpers() {
    let net_error = ValueType::Struct("NetError".to_string(), Vec::new());
    let udp_socket = ValueType::Struct("UdpSocket".to_string(), Vec::new());
    let udp_datagram = ValueType::Struct("UdpDatagram".to_string(), Vec::new());
    let result_socket_error = ValueType::Enum(
        "Result".to_string(),
        vec![udp_socket.clone(), net_error.clone()],
    );
    let result_datagram_error = ValueType::Enum(
        "Result".to_string(),
        vec![udp_datagram.clone(), net_error.clone()],
    );
    let result_void_error = ValueType::Enum(
        "Result".to_string(),
        vec![ValueType::Void, net_error.clone()],
    );
    let program = Program {
        consts: Vec::new(),
        package: "app.main".to_string(),
        imports: vec!["std.net".to_string()],
        extern_functions: Vec::new(),
        structs: vec![
            StructType {
                package: "std.net".to_string(),
                name: "NetError".to_string(),
                type_params: Vec::new(),
                fields: vec![StructField {
                    name: "message".to_string(),
                    value_type: ValueType::String,
                }],
            },
            StructType {
                package: "std.net".to_string(),
                name: "UdpDatagram".to_string(),
                type_params: Vec::new(),
                fields: vec![
                    StructField {
                        name: "data".to_string(),
                        value_type: ValueType::String,
                    },
                    StructField {
                        name: "host".to_string(),
                        value_type: ValueType::String,
                    },
                    StructField {
                        name: "port".to_string(),
                        value_type: ValueType::Int,
                    },
                ],
            },
            StructType {
                package: "std.net".to_string(),
                name: "UdpSocket".to_string(),
                type_params: Vec::new(),
                fields: Vec::new(),
            },
        ],
        enums: vec![EnumType {
            package: "std.result".to_string(),
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
                name: "process_socket".to_string(),
                params: vec![Parameter {
                    name: "socket".to_string(),
                    mutable: false,
                    value_type: udp_socket,
                }],
                return_type: ValueType::Void,
                body: vec![
                    Statement::Let {
                        name: "packet".to_string(),
                        value_type: result_datagram_error,
                        initializer: ValueExpr::UdpSocketRecvFromString {
                            socket: Box::new(ValueExpr::Variable("socket".to_string())),
                            max_bytes: Box::new(ValueExpr::IntLiteral(1024)),
                        },
                    },
                    Statement::Let {
                        name: "sent".to_string(),
                        value_type: result_void_error,
                        initializer: ValueExpr::UdpSocketSendToString {
                            socket: Box::new(ValueExpr::Variable("socket".to_string())),
                            content: Box::new(ValueExpr::StringLiteral("pong".to_string())),
                            host: Box::new(ValueExpr::StringLiteral("127.0.0.1".to_string())),
                            port: Box::new(ValueExpr::IntLiteral(7)),
                        },
                    },
                    Statement::Expr(ValueExpr::UdpSocketClose {
                        socket: Box::new(ValueExpr::Variable("socket".to_string())),
                    }),
                ],
            },
            Function {
                package: "app.main".to_string(),
                name: "main".to_string(),
                params: Vec::new(),
                return_type: ValueType::Void,
                body: vec![Statement::Let {
                    name: "bound".to_string(),
                    value_type: result_socket_error,
                    initializer: ValueExpr::NetUdpBind {
                        host: Box::new(ValueExpr::StringLiteral("127.0.0.1".to_string())),
                        port: Box::new(ValueExpr::IntLiteral(7)),
                    },
                }],
            },
        ],
    };

    let c = emit_c(&program);
    assert!(c.contains("typedef struct nomo_struct_UdpDatagram"));
    assert!(c.contains("typedef struct nomo_struct_UdpSocket"));
    assert!(c.contains("nomo_socket nomo_member_handle"));
    assert!(c.contains("nomo_net_udp_bind(nomo_string host, int64_t port)"));
    assert!(c.contains(
        "nomo_udp_socket_recv_from_string(nomo_struct_UdpSocket socket, int64_t max_bytes)"
    ));
    assert!(c.contains(
            "nomo_udp_socket_send_to_string(nomo_struct_UdpSocket socket, nomo_string content, nomo_string host, int64_t port)"
        ));
    assert!(c.contains("static void nomo_udp_socket_close(nomo_struct_UdpSocket socket)"));
    assert!(c.contains("nomo_net_udp_bind(nomo_string_literal(\"127.0.0.1\"), 7)"));
    assert!(c.contains("nomo_udp_socket_recv_from_string(nomo_socket, 1024)"));
    assert!(c.contains(
            "nomo_udp_socket_send_to_string(nomo_socket, nomo_string_literal(\"pong\"), nomo_string_literal(\"127.0.0.1\"), 7)"
        ));
    assert!(c.contains("nomo_udp_socket_close(nomo_socket)"));
}

#[test]
fn emits_io_read_line_helper() {
    let io_error = ValueType::Struct("IoError".to_string(), Vec::new());
    let result_string_error =
        ValueType::Enum("Result".to_string(), vec![ValueType::String, io_error]);
    let program = Program {
        consts: Vec::new(),
        package: "app.main".to_string(),
        imports: vec!["std.io".to_string()],
        extern_functions: Vec::new(),
        structs: vec![StructType {
            package: "std.io".to_string(),
            name: "IoError".to_string(),
            type_params: Vec::new(),
            fields: vec![StructField {
                name: "message".to_string(),
                value_type: ValueType::String,
            }],
        }],
        enums: vec![EnumType {
            package: "std.result".to_string(),
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
        functions: vec![Function {
            package: "app.main".to_string(),
            name: "main".to_string(),
            params: Vec::new(),
            return_type: ValueType::Void,
            body: vec![Statement::Let {
                name: "read_result".to_string(),
                value_type: result_string_error,
                initializer: ValueExpr::IoReadLine,
            }],
        }],
    };

    let c = emit_c(&program);
    assert!(c.contains("typedef struct nomo_struct_IoError"));
    assert!(c.contains("static nomo_enum_Result_string_struct_IoError nomo_io_read_line"));
    assert!(c.contains("nomo_io_read_line()"));
}

#[test]
fn emits_num_helpers() {
    let num_error = ValueType::Struct("NumError".to_string(), Vec::new());
    let result_i64_error = ValueType::Enum(
        "Result".to_string(),
        vec![ValueType::Int, num_error.clone()],
    );
    let result_u64_error = ValueType::Enum(
        "Result".to_string(),
        vec![ValueType::U64, num_error.clone()],
    );
    let result_f64_error = ValueType::Enum("Result".to_string(), vec![ValueType::Float, num_error]);
    let program = Program {
        consts: Vec::new(),
        package: "app.main".to_string(),
        imports: vec!["std.num".to_string()],
        extern_functions: Vec::new(),
        structs: vec![StructType {
            package: "std.num".to_string(),
            name: "NumError".to_string(),
            type_params: Vec::new(),
            fields: vec![StructField {
                name: "message".to_string(),
                value_type: ValueType::String,
            }],
        }],
        enums: vec![EnumType {
            package: "std.result".to_string(),
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
        functions: vec![Function {
            package: "app.main".to_string(),
            name: "main".to_string(),
            params: Vec::new(),
            return_type: ValueType::Void,
            body: vec![
                Statement::Let {
                    name: "integer".to_string(),
                    value_type: result_i64_error,
                    initializer: ValueExpr::NumParseI64 {
                        value: Box::new(ValueExpr::StringLiteral("42".to_string())),
                    },
                },
                Statement::Let {
                    name: "unsigned".to_string(),
                    value_type: result_u64_error,
                    initializer: ValueExpr::NumParseU64 {
                        value: Box::new(ValueExpr::StringLiteral("7".to_string())),
                    },
                },
                Statement::Let {
                    name: "decimal".to_string(),
                    value_type: result_f64_error,
                    initializer: ValueExpr::NumParseF64 {
                        value: Box::new(ValueExpr::StringLiteral("3.5".to_string())),
                    },
                },
                Statement::Let {
                    name: "text".to_string(),
                    value_type: ValueType::String,
                    initializer: ValueExpr::NumToString {
                        value: Box::new(ValueExpr::IntLiteral(42)),
                        value_type: ValueType::Int,
                    },
                },
            ],
        }],
    };

    let c = emit_c(&program);
    assert!(c.contains("typedef struct nomo_struct_NumError"));
    assert!(c.contains("static nomo_enum_Result_i64_struct_NumError nomo_num_parse_i64"));
    assert!(c.contains("static nomo_enum_Result_u64_struct_NumError nomo_num_parse_u64"));
    assert!(c.contains("static nomo_enum_Result_f64_struct_NumError nomo_num_parse_f64"));
    assert!(c.contains("nomo_num_parse_i64(nomo_string_literal(\"42\"))"));
    assert!(c.contains("nomo_num_i64_to_string(42)"));
}

#[test]
fn emits_num_checked_and_wrapping_helpers() {
    let option_i64 = ValueType::Enum("Option".to_string(), vec![ValueType::Int]);
    let program = Program {
        consts: Vec::new(),
        package: "app.main".to_string(),
        imports: vec!["std.num".to_string()],
        extern_functions: Vec::new(),
        structs: Vec::new(),
        enums: vec![EnumType {
            package: "std.option".to_string(),
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
                    name: "checked".to_string(),
                    value_type: option_i64,
                    initializer: ValueExpr::NumBinary {
                        function: NumBinaryFunction::Checked,
                        op: BinaryOp::Add,
                        left: Box::new(ValueExpr::IntLiteral(i64::MAX)),
                        right: Box::new(ValueExpr::IntLiteral(1)),
                        value_type: ValueType::Int,
                    },
                },
                Statement::Let {
                    name: "wrapped".to_string(),
                    value_type: ValueType::Int,
                    initializer: ValueExpr::NumBinary {
                        function: NumBinaryFunction::Wrapping,
                        op: BinaryOp::Subtract,
                        left: Box::new(ValueExpr::IntLiteral(i64::MIN)),
                        right: Box::new(ValueExpr::IntLiteral(1)),
                        value_type: ValueType::Int,
                    },
                },
            ],
        }],
    };

    let c = emit_c(&program);
    assert!(c.contains("typedef enum nomo_enum_Option_i64_tag"));
    assert!(c.contains("static nomo_enum_Option_i64 nomo_num_checked_add_i64"));
    assert!(c.contains("nomo_num_checked_add_i64(9223372036854775807, 1)"));
    assert!(c.contains("static long long nomo_num_wrapping_sub_i64"));
    assert!(c.contains("nomo_wrapped = nomo_num_wrapping_sub_i64("));
}

#[test]
fn emits_env_get_helper() {
    let option_string = ValueType::Enum("Option".to_string(), vec![ValueType::String]);
    let program = Program {
        consts: Vec::new(),
        package: "app.main".to_string(),
        imports: vec!["std.env".to_string()],
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
            body: vec![Statement::Let {
                name: "value".to_string(),
                value_type: option_string,
                initializer: ValueExpr::EnvGet {
                    name: Box::new(ValueExpr::StringLiteral("NOMO_TEST_ENV".to_string())),
                },
            }],
        }],
    };

    let c = emit_c(&program);
    assert!(c.contains("static nomo_enum_Option_string nomo_env_get"));
    assert!(c.contains("getenv(name.data)"));
    assert!(c.contains("nomo_env_get(nomo_string_literal(\"NOMO_TEST_ENV\"))"));
}

#[test]
#[should_panic(expected = "unsupported Array element type reached C type lowering")]
fn panics_instead_of_emitting_unsupported_array_placeholders() {
    let program = Program {
        consts: Vec::new(),
        package: "app.main".to_string(),
        imports: Vec::new(),
        extern_functions: Vec::new(),
        structs: Vec::new(),
        enums: Vec::new(),
        functions: vec![Function {
            package: "app.main".to_string(),
            name: "main".to_string(),
            params: Vec::new(),
            return_type: ValueType::Void,
            body: vec![Statement::Let {
                name: "bad".to_string(),
                value_type: ValueType::Array(Box::new(ValueType::Void)),
                initializer: ValueExpr::ArrayNew {
                    element_type: ValueType::Void,
                },
            }],
        }],
    };

    let _ = emit_c(&program);
}

#[test]
fn emits_env_args_helper_and_main_arguments() {
    let array_string = ValueType::Array(Box::new(ValueType::String));
    let program = Program {
        consts: Vec::new(),
        package: "app.main".to_string(),
        imports: vec!["std.env".to_string()],
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
            body: vec![Statement::Let {
                name: "args".to_string(),
                value_type: array_string,
                initializer: ValueExpr::EnvArgs,
            }],
        }],
    };

    let c = emit_c(&program);
    assert!(c.contains("int main(int argc, char **argv)"));
    assert!(c.contains("static int nomo_argc = 0;"));
    assert!(c.contains("static char **nomo_argv = NULL;"));
    assert!(c.contains("static nomo_array_string nomo_env_args(int argc, char **argv)"));
    assert!(c.contains("nomo_argc = argc;"));
    assert!(c.contains("nomo_argv = argv;"));
    assert!(c.contains("nomo_array_string nomo_args = nomo_env_args(nomo_argc, nomo_argv);"));
}

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
