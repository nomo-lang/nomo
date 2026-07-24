use super::*;

pub(super) fn emit_json_helpers(out: &mut String, structured: bool) {
    let json_value = ValueType::Struct("JsonValue".to_string(), Vec::new());
    let json_error = ValueType::Struct("JsonError".to_string(), Vec::new());
    let json_member = ValueType::Struct("JsonMember".to_string(), Vec::new());
    let json_value_struct = c_struct_ident("JsonValue", &[]);
    let json_error_struct = c_struct_ident("JsonError", &[]);
    let json_member_struct = c_struct_ident("JsonMember", &[]);
    let json_kind = c_enum_ident("JsonKind", &[]);
    let result_args = [json_value.clone(), json_error];
    let result = c_enum_ident("Result", &result_args);
    let option_bool = c_enum_ident("Option", &[ValueType::Bool]);
    let option_string = c_enum_ident("Option", &[ValueType::String]);
    let json_value_array_type = ValueType::Array(Box::new(json_value.clone()));
    let json_member_array_type = ValueType::Array(Box::new(json_member.clone()));
    let option_value_array = c_enum_ident("Option", std::slice::from_ref(&json_value_array_type));
    let option_member_array = c_enum_ident("Option", std::slice::from_ref(&json_member_array_type));
    let option_value = c_enum_ident("Option", std::slice::from_ref(&json_value));
    let json_value_array = c_array_ident(&json_value);
    let json_member_array = c_array_ident(&json_member);

    let replacements: Vec<(&str, String)> = vec![
        ("@JSON_VALUE@", json_value_struct),
        ("@JSON_ERROR@", json_error_struct),
        ("@JSON_MEMBER@", json_member_struct),
        ("@JSON_KIND@", json_kind),
        ("@RESULT@", result),
        ("@OPTION_BOOL@", option_bool),
        ("@OPTION_STRING@", option_string),
        ("@OPTION_VALUE_ARRAY@", option_value_array),
        ("@OPTION_MEMBER_ARRAY@", option_member_array),
        ("@OPTION_VALUE@", option_value),
        ("@VALUE_ARRAY@", json_value_array),
        ("@MEMBER_ARRAY@", json_member_array),
        ("@RAW_MEMBER@", c_member_ident("raw")),
        ("@CODE_MEMBER@", c_member_ident("code")),
        ("@MESSAGE_MEMBER@", c_member_ident("message")),
        ("@OFFSET_MEMBER@", c_member_ident("offset")),
        ("@KEY_MEMBER@", c_member_ident("key")),
        ("@VALUE_MEMBER@", c_member_ident("value")),
        (
            "@OK_TAG@",
            c_enum_variant_ident("Result", &result_args, "Ok"),
        ),
        (
            "@ERR_TAG@",
            c_enum_variant_ident("Result", &result_args, "Err"),
        ),
        ("@OK_PAYLOAD@", c_payload_ident("Ok")),
        ("@ERR_PAYLOAD@", c_payload_ident("Err")),
        (
            "@SOME_BOOL_TAG@",
            c_enum_variant_ident("Option", &[ValueType::Bool], "Some"),
        ),
        (
            "@NONE_BOOL_TAG@",
            c_enum_variant_ident("Option", &[ValueType::Bool], "None"),
        ),
        (
            "@SOME_STRING_TAG@",
            c_enum_variant_ident("Option", &[ValueType::String], "Some"),
        ),
        (
            "@NONE_STRING_TAG@",
            c_enum_variant_ident("Option", &[ValueType::String], "None"),
        ),
        (
            "@SOME_VALUE_ARRAY_TAG@",
            c_enum_variant_ident(
                "Option",
                std::slice::from_ref(&json_value_array_type),
                "Some",
            ),
        ),
        (
            "@NONE_VALUE_ARRAY_TAG@",
            c_enum_variant_ident(
                "Option",
                std::slice::from_ref(&json_value_array_type),
                "None",
            ),
        ),
        (
            "@SOME_MEMBER_ARRAY_TAG@",
            c_enum_variant_ident(
                "Option",
                std::slice::from_ref(&json_member_array_type),
                "Some",
            ),
        ),
        (
            "@NONE_MEMBER_ARRAY_TAG@",
            c_enum_variant_ident(
                "Option",
                std::slice::from_ref(&json_member_array_type),
                "None",
            ),
        ),
        (
            "@SOME_VALUE_TAG@",
            c_enum_variant_ident("Option", std::slice::from_ref(&json_value), "Some"),
        ),
        (
            "@NONE_VALUE_TAG@",
            c_enum_variant_ident("Option", std::slice::from_ref(&json_value), "None"),
        ),
        ("@SOME_PAYLOAD@", c_payload_ident("Some")),
        ("@KIND_NULL@", c_enum_variant_ident("JsonKind", &[], "Null")),
        (
            "@KIND_BOOLEAN@",
            c_enum_variant_ident("JsonKind", &[], "Boolean"),
        ),
        (
            "@KIND_NUMBER@",
            c_enum_variant_ident("JsonKind", &[], "Number"),
        ),
        (
            "@KIND_STRING@",
            c_enum_variant_ident("JsonKind", &[], "String"),
        ),
        (
            "@KIND_ARRAY@",
            c_enum_variant_ident("JsonKind", &[], "Array"),
        ),
        (
            "@KIND_OBJECT@",
            c_enum_variant_ident("JsonKind", &[], "Object"),
        ),
    ];

    let mut source = include_str!("host_json.c").to_string();
    for (placeholder, replacement) in replacements {
        source = source.replace(placeholder, &replacement);
    }
    if !structured {
        source.truncate(
            source
                .find("/* NOMO_STRUCTURED_JSON_BEGIN */")
                .expect("structured JSON template marker exists"),
        );
    }
    out.push_str(&source);
}
