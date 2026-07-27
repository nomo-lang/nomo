use super::*;

pub(super) fn emit_http_stream_helpers(out: &mut String) {
    let header_type = ValueType::Struct("HttpHeader".to_string(), Vec::new());
    let http_error_type = ValueType::Struct("HttpError".to_string(), Vec::new());
    let http_stream_type = ValueType::Struct("BlockingHttpStream".to_string(), Vec::new());
    let chunk_type = ValueType::Struct("HttpStreamChunk".to_string(), Vec::new());
    let event_type = ValueType::Struct("SseEvent".to_string(), Vec::new());
    let event_option_type = ValueType::Enum("Option".to_string(), vec![event_type.clone()]);
    let open_result_type = ValueType::Enum(
        "Result".to_string(),
        vec![http_stream_type.clone(), http_error_type.clone()],
    );
    let read_result_type = ValueType::Enum(
        "Result".to_string(),
        vec![chunk_type.clone(), http_error_type.clone()],
    );
    let sse_result_type = ValueType::Enum(
        "Result".to_string(),
        vec![event_option_type.clone(), http_error_type.clone()],
    );
    let header_array = c_array_ident(&header_type);

    let rendered = include_str!("host_http_stream.c")
        .replace("@HTTP_HEADER@", &c_struct_ident("HttpHeader", &[]))
        .replace("@HTTP_REQUEST@", &c_struct_ident("HttpRequest", &[]))
        .replace("@HTTP_ERROR@", &c_struct_ident("HttpError", &[]))
        .replace("@HTTP_STREAM@", &c_struct_ident("BlockingHttpStream", &[]))
        .replace(
            "@HTTP_STREAM_CHUNK@",
            &c_struct_ident("HttpStreamChunk", &[]),
        )
        .replace("@SSE_EVENT@", &c_struct_ident("SseEvent", &[]))
        .replace("@HTTP_HEADER_ARRAY@", &header_array)
        .replace("@HTTP_HEADER_ARRAY_NEW@", &format!("{header_array}_new"))
        .replace(
            "@HTTP_HEADER_ARRAY_RELEASE@",
            &format!("{header_array}_release"),
        )
        .replace("@OPEN_RESULT@", &c_type(&open_result_type))
        .replace("@READ_RESULT@", &c_type(&read_result_type))
        .replace("@SSE_RESULT@", &c_type(&sse_result_type))
        .replace("@EVENT_OPTION@", &c_type(&event_option_type))
        .replace("@RETRY_OPTION@", &c_enum_ident("Option", &[ValueType::U64]))
        .replace(
            "@OPEN_OK@",
            &c_enum_variant_ident(
                "Result",
                &[http_stream_type.clone(), http_error_type.clone()],
                "Ok",
            ),
        )
        .replace(
            "@OPEN_ERR@",
            &c_enum_variant_ident(
                "Result",
                &[http_stream_type, http_error_type.clone()],
                "Err",
            ),
        )
        .replace(
            "@READ_OK@",
            &c_enum_variant_ident(
                "Result",
                &[chunk_type.clone(), http_error_type.clone()],
                "Ok",
            ),
        )
        .replace(
            "@READ_ERR@",
            &c_enum_variant_ident("Result", &[chunk_type, http_error_type.clone()], "Err"),
        )
        .replace(
            "@SSE_OK@",
            &c_enum_variant_ident(
                "Result",
                &[event_option_type.clone(), http_error_type.clone()],
                "Ok",
            ),
        )
        .replace(
            "@SSE_ERR@",
            &c_enum_variant_ident("Result", &[event_option_type, http_error_type], "Err"),
        )
        .replace(
            "@EVENT_SOME@",
            &c_enum_variant_ident("Option", &[event_type.clone()], "Some"),
        )
        .replace(
            "@EVENT_NONE@",
            &c_enum_variant_ident("Option", &[event_type], "None"),
        )
        .replace(
            "@RETRY_SOME@",
            &c_enum_variant_ident("Option", &[ValueType::U64], "Some"),
        )
        .replace(
            "@RETRY_NONE@",
            &c_enum_variant_ident("Option", &[ValueType::U64], "None"),
        )
        .replace("@OK_PAYLOAD@", &c_payload_ident("Ok"))
        .replace("@ERR_PAYLOAD@", &c_payload_ident("Err"))
        .replace("@SOME_PAYLOAD@", &c_payload_ident("Some"))
        .replace("@HANDLE_MEMBER@", &c_member_ident("handle"))
        .replace("@STATUS_MEMBER@", &c_member_ident("status"))
        .replace("@HEADERS_MEMBER@", &c_member_ident("headers"))
        .replace("@DATA_MEMBER@", &c_member_ident("data"))
        .replace("@DONE_MEMBER@", &c_member_ident("done"))
        .replace("@EVENT_MEMBER@", &c_member_ident("event"))
        .replace("@ID_MEMBER@", &c_member_ident("id"))
        .replace("@RETRY_MEMBER@", &c_member_ident("retry_millis"))
        .replace("@CODE_MEMBER@", &c_member_ident("code"))
        .replace("@MESSAGE_MEMBER@", &c_member_ident("message"))
        .replace("@NAME_MEMBER@", &c_member_ident("name"))
        .replace("@VALUE_MEMBER@", &c_member_ident("value"))
        .replace("@METHOD_MEMBER@", &c_member_ident("method"))
        .replace("@URL_MEMBER@", &c_member_ident("url"))
        .replace("@HEADERS_REQUEST_MEMBER@", &c_member_ident("headers"))
        .replace("@BODY_MEMBER@", &c_member_ident("body"))
        .replace("@TIMEOUT_MEMBER@", &c_member_ident("timeout_millis"))
        .replace(
            "@MAX_RESPONSE_MEMBER@",
            &c_member_ident("max_response_bytes"),
        )
        .replace(
            "@OPEN_NAME@",
            &c_fn_ident(BUILTIN_HTTP_OPEN_STREAM_BLOCKING_EXPR),
        )
        .replace(
            "@READ_NAME@",
            &c_fn_ident(BUILTIN_HTTP_READ_TEXT_BLOCKING_EXPR),
        )
        .replace(
            "@SSE_NAME@",
            &c_fn_ident(BUILTIN_HTTP_NEXT_SSE_BLOCKING_EXPR),
        )
        .replace(
            "@CANCEL_NAME@",
            &c_fn_ident(BUILTIN_HTTP_CANCEL_STREAM_BLOCKING_EXPR),
        )
        .replace(
            "@CLOSE_NAME@",
            &c_fn_ident(BUILTIN_HTTP_CLOSE_STREAM_BLOCKING_EXPR),
        );
    out.push_str(&rendered);
}
