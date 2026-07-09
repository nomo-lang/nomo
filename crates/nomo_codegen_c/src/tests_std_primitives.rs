use super::*;

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
