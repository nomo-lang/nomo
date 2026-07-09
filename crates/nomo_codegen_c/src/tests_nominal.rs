use super::*;

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
