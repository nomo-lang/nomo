use super::*;

pub(super) fn emit_jsonrpc_helpers(out: &mut String) {
    let json_value = ValueType::Struct("JsonValue".to_string(), Vec::new());
    let json_member = ValueType::Struct("JsonMember".to_string(), Vec::new());
    let message = ValueType::Struct("JsonRpcMessage".to_string(), Vec::new());
    let decoder = ValueType::Struct("JsonRpcDecoder".to_string(), Vec::new());
    let error = ValueType::Struct("JsonRpcProtocolError".to_string(), Vec::new());
    let batch = ValueType::Struct("JsonRpcDecodeBatch".to_string(), Vec::new());
    let message_array_type = ValueType::Array(Box::new(message.clone()));
    let member_array_type = ValueType::Array(Box::new(json_member.clone()));
    let option_value_args = [json_value.clone()];
    let option_string_args = [ValueType::String];
    let option_member_array_args = [member_array_type.clone()];
    let result_decoder_args = [decoder.clone(), error.clone()];
    let result_batch_args = [batch.clone(), error.clone()];
    let result_void_args = [ValueType::Void, error.clone()];
    let result_message_args = [message.clone(), error.clone()];
    let result_string_args = [ValueType::String, error];

    let replacements: Vec<(&str, String)> = vec![
        ("@JSON_VALUE@", c_struct_ident("JsonValue", &[])),
        ("@JSON_MEMBER@", c_struct_ident("JsonMember", &[])),
        ("@MESSAGE@", c_struct_ident("JsonRpcMessage", &[])),
        ("@DECODER@", c_struct_ident("JsonRpcDecoder", &[])),
        (
            "@PROTOCOL_ERROR@",
            c_struct_ident("JsonRpcProtocolError", &[]),
        ),
        ("@BATCH@", c_struct_ident("JsonRpcDecodeBatch", &[])),
        ("@MESSAGE_KIND@", c_enum_ident("JsonRpcMessageKind", &[])),
        ("@MESSAGE_ARRAY@", c_array_ident(&message)),
        ("@MEMBER_ARRAY@", c_array_ident(&json_member)),
        ("@OPTION_VALUE@", c_enum_ident("Option", &option_value_args)),
        (
            "@OPTION_STRING@",
            c_enum_ident("Option", &option_string_args),
        ),
        (
            "@OPTION_MEMBER_ARRAY@",
            c_enum_ident("Option", &option_member_array_args),
        ),
        (
            "@RESULT_DECODER@",
            c_enum_ident("Result", &result_decoder_args),
        ),
        ("@RESULT_BATCH@", c_enum_ident("Result", &result_batch_args)),
        ("@RESULT_VOID@", c_enum_ident("Result", &result_void_args)),
        (
            "@RESULT_MESSAGE@",
            c_enum_ident("Result", &result_message_args),
        ),
        (
            "@RESULT_STRING@",
            c_enum_ident("Result", &result_string_args),
        ),
        (
            "@RESULT_DECODER_OK@",
            c_enum_variant_ident("Result", &result_decoder_args, "Ok"),
        ),
        (
            "@RESULT_DECODER_ERR@",
            c_enum_variant_ident("Result", &result_decoder_args, "Err"),
        ),
        (
            "@RESULT_BATCH_OK@",
            c_enum_variant_ident("Result", &result_batch_args, "Ok"),
        ),
        (
            "@RESULT_BATCH_ERR@",
            c_enum_variant_ident("Result", &result_batch_args, "Err"),
        ),
        (
            "@RESULT_VOID_OK@",
            c_enum_variant_ident("Result", &result_void_args, "Ok"),
        ),
        (
            "@RESULT_VOID_ERR@",
            c_enum_variant_ident("Result", &result_void_args, "Err"),
        ),
        (
            "@RESULT_MESSAGE_OK@",
            c_enum_variant_ident("Result", &result_message_args, "Ok"),
        ),
        (
            "@RESULT_MESSAGE_ERR@",
            c_enum_variant_ident("Result", &result_message_args, "Err"),
        ),
        (
            "@RESULT_STRING_OK@",
            c_enum_variant_ident("Result", &result_string_args, "Ok"),
        ),
        (
            "@RESULT_STRING_ERR@",
            c_enum_variant_ident("Result", &result_string_args, "Err"),
        ),
        (
            "@OPTION_VALUE_SOME@",
            c_enum_variant_ident("Option", &option_value_args, "Some"),
        ),
        (
            "@OPTION_STRING_SOME@",
            c_enum_variant_ident("Option", &option_string_args, "Some"),
        ),
        (
            "@OPTION_MEMBER_ARRAY_SOME@",
            c_enum_variant_ident("Option", &option_member_array_args, "Some"),
        ),
        (
            "@KIND_REQUEST@",
            c_enum_variant_ident("JsonRpcMessageKind", &[], "Request"),
        ),
        (
            "@KIND_NOTIFICATION@",
            c_enum_variant_ident("JsonRpcMessageKind", &[], "Notification"),
        ),
        (
            "@KIND_SUCCESS@",
            c_enum_variant_ident("JsonRpcMessageKind", &[], "Success"),
        ),
        (
            "@KIND_ERROR@",
            c_enum_variant_ident("JsonRpcMessageKind", &[], "Error"),
        ),
        ("@RAW_MEMBER@", c_member_ident("raw")),
        ("@KEY_MEMBER@", c_member_ident("key")),
        ("@VALUE_MEMBER@", c_member_ident("value")),
        ("@PENDING_MEMBER@", c_member_ident("pending")),
        (
            "@MAX_MESSAGE_BYTES_MEMBER@",
            c_member_ident("max_message_bytes"),
        ),
        ("@DECODER_MEMBER@", c_member_ident("decoder")),
        ("@MESSAGES_MEMBER@", c_member_ident("messages")),
        ("@CODE_MEMBER@", c_member_ident("code")),
        ("@MESSAGE_MEMBER@", c_member_ident("message")),
        ("@OK_PAYLOAD@", c_payload_ident("Ok")),
        ("@ERR_PAYLOAD@", c_payload_ident("Err")),
        ("@SOME_PAYLOAD@", c_payload_ident("Some")),
        ("@MESSAGE_RELEASE@", c_release_ident(&message)),
        (
            "@MESSAGE_ARRAY_RELEASE@",
            c_release_ident(&message_array_type),
        ),
        (
            "@MEMBER_ARRAY_RELEASE@",
            c_release_ident(&member_array_type),
        ),
    ];

    let mut source = include_str!("host_jsonrpc.c").to_string();
    for (placeholder, replacement) in replacements {
        source = source.replace(placeholder, &replacement);
    }
    out.push_str(&source);
}
