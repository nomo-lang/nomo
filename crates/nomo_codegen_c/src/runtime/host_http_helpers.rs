use super::*;

pub(super) fn emit_http_client_helpers(out: &mut String) {
    let header_type = ValueType::Struct("HttpHeader".to_string(), Vec::new());
    let http_header = c_struct_ident("HttpHeader", &[]);
    let http_request = c_struct_ident("HttpRequest", &[]);
    let http_response = c_struct_ident("HttpResponse", &[]);
    let http_error = c_struct_ident("HttpError", &[]);
    let header_array = c_array_ident(&header_type);
    let result = c_enum_ident(
        "Result",
        &[
            ValueType::Struct("HttpResponse".to_string(), Vec::new()),
            ValueType::Struct("HttpError".to_string(), Vec::new()),
        ],
    );
    let rendered = include_str!("host_http_client.c")
        .replace("@HTTP_HEADER@", &http_header)
        .replace("@HTTP_REQUEST@", &http_request)
        .replace("@HTTP_RESPONSE@", &http_response)
        .replace("@HTTP_ERROR@", &http_error)
        .replace("@HTTP_HEADER_ARRAY@", &header_array)
        .replace("@HTTP_HEADER_ARRAY_NEW@", &format!("{header_array}_new"))
        .replace("@HTTP_HEADER_ARRAY_PUSH@", &format!("{header_array}_push"))
        .replace(
            "@HTTP_HEADER_ARRAY_RELEASE@",
            &format!("{header_array}_release"),
        )
        .replace("@HTTP_HEADER_RELEASE@", &c_release_ident(&header_type))
        .replace("@RESULT@", &result)
        .replace(
            "@OK@",
            &c_enum_variant_ident(
                "Result",
                &[
                    ValueType::Struct("HttpResponse".to_string(), Vec::new()),
                    ValueType::Struct("HttpError".to_string(), Vec::new()),
                ],
                "Ok",
            ),
        )
        .replace(
            "@ERR@",
            &c_enum_variant_ident(
                "Result",
                &[
                    ValueType::Struct("HttpResponse".to_string(), Vec::new()),
                    ValueType::Struct("HttpError".to_string(), Vec::new()),
                ],
                "Err",
            ),
        )
        .replace("@OK_PAYLOAD@", &c_payload_ident("Ok"))
        .replace("@ERR_PAYLOAD@", &c_payload_ident("Err"))
        .replace("@STATUS_MEMBER@", &c_member_ident("status"))
        .replace("@HEADERS_MEMBER@", &c_member_ident("headers"))
        .replace("@BODY_MEMBER@", &c_member_ident("body"))
        .replace("@CODE_MEMBER@", &c_member_ident("code"))
        .replace("@MESSAGE_MEMBER@", &c_member_ident("message"))
        .replace("@NAME_MEMBER@", &c_member_ident("name"))
        .replace("@VALUE_MEMBER@", &c_member_ident("value"))
        .replace("@METHOD_MEMBER@", &c_member_ident("method"))
        .replace("@URL_MEMBER@", &c_member_ident("url"))
        .replace("@TIMEOUT_MEMBER@", &c_member_ident("timeout_millis"))
        .replace(
            "@MAX_RESPONSE_MEMBER@",
            &c_member_ident("max_response_bytes"),
        )
        .replace("@SEND_NAME@", &c_fn_ident(BUILTIN_HTTP_SEND_BLOCKING_EXPR))
        .replace("@GET_NAME@", &c_fn_ident(BUILTIN_HTTP_GET_BLOCKING_EXPR))
        .replace("@POST_NAME@", &c_fn_ident(BUILTIN_HTTP_POST_BLOCKING_EXPR));
    out.push_str(&rendered);
}
