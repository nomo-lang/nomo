use super::*;

pub(super) fn emit_async_http_helpers(out: &mut String, include_suspend_abi: bool) {
    let request = ValueType::Struct("HttpRequest".to_string(), Vec::new());
    let response = ValueType::Struct("HttpResponse".to_string(), Vec::new());
    let error = ValueType::Struct("HttpError".to_string(), Vec::new());
    let stream = ValueType::Struct("HttpStream".to_string(), Vec::new());
    let chunk = ValueType::Struct("HttpStreamChunk".to_string(), Vec::new());
    let event = ValueType::Struct("SseEvent".to_string(), Vec::new());
    let event_option = ValueType::Enum("Option".to_string(), vec![event]);
    let response_result = result_type(response, error.clone());
    let stream_result = result_type(stream.clone(), error.clone());
    let chunk_result = result_type(chunk, error.clone());
    let event_result = result_type(event_option, error.clone());

    if include_suspend_abi {
        out.push_str(
            "typedef struct {\n\
                 uint8_t active;\n\
             } nomo_async_http_registration;\n\n",
        );
        emit_runtime_unavailable_error(out, &error);
        emit_error_result_helper(
            out,
            "nomo_async_http_response_unavailable",
            &response_result,
        );
        emit_error_result_helper(out, "nomo_async_http_stream_unavailable", &stream_result);
        emit_error_result_helper(out, "nomo_async_http_chunk_unavailable", &chunk_result);
        emit_error_result_helper(out, "nomo_async_http_sse_unavailable", &event_result);

        emit_get_abi(out, &response_result);
        emit_post_abi(out, &response_result);
        emit_send_abi(out, &request, &response_result);
        emit_open_abi(out, &request, &stream_result);
        emit_stream_pull_abi(
            out,
            "read_text",
            "nomo_async_http_chunk_unavailable",
            &stream,
            &chunk_result,
        );
        emit_stream_pull_abi(
            out,
            "next_sse",
            "nomo_async_http_sse_unavailable",
            &stream,
            &event_result,
        );
        out.push_str(
            "static void nomo_async_http_cancel(\n\
                 nomo_async_http_registration *registration,\n\
                 nomo_async_context *context\n\
             ) {\n\
                 (void)context;\n\
                 registration->active = 0u;\n\
             }\n\n\
             static void nomo_async_http_runtime_shutdown(nomo_async_context *context) {\n\
                 (void)context;\n\
             }\n\n",
        );
    }

    emit_stream_lifecycle_helpers(out, &stream);
}

fn result_type(ok: ValueType, error: ValueType) -> ValueType {
    ValueType::Enum("Result".to_string(), vec![ok, error])
}

fn emit_runtime_unavailable_error(out: &mut String, error: &ValueType) {
    out.push_str("static ");
    out.push_str(&c_type(error));
    out.push_str(" nomo_async_http_runtime_unavailable_error(void) {\n    return (");
    out.push_str(&c_type(error));
    out.push_str("){.");
    out.push_str(&c_member_ident("code"));
    out.push_str(" = nomo_string_literal(\"runtime_unavailable\"), .");
    out.push_str(&c_member_ident("message"));
    out.push_str(
        " = nomo_string_literal(\"owner-affine async HTTP is unavailable in this runtime slice\")};\n\
         }\n\n",
    );
}

fn emit_error_result_helper(out: &mut String, name: &str, result: &ValueType) {
    let ValueType::Enum(_, args) = result else {
        unreachable!("async HTTP result helper requires Result");
    };
    out.push_str("static ");
    out.push_str(&c_type(result));
    out.push(' ');
    out.push_str(name);
    out.push_str("(void) {\n    return (");
    out.push_str(&c_type(result));
    out.push_str("){.tag = ");
    out.push_str(&c_enum_variant_ident("Result", args, "Err"));
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = nomo_async_http_runtime_unavailable_error()};\n}\n\n");
}

fn emit_get_abi(out: &mut String, result: &ValueType) {
    emit_start_header(out, "get");
    out.push_str(
        "    nomo_string url,\n\
         nomo_async_context *context,\n    ",
    );
    out.push_str(&c_type(result));
    out.push_str(
        " *result\n\
         ) {\n\
             (void)url;\n\
             (void)context;\n\
             memset(registration, 0, sizeof(*registration));\n\
             *result = nomo_async_http_response_unavailable();\n\
             return NOMO_ASYNC_POLL_READY;\n\
         }\n\n",
    );
    emit_resume_abi(out, "get", result);
}

fn emit_post_abi(out: &mut String, result: &ValueType) {
    emit_start_header(out, "post");
    out.push_str(
        "    nomo_string url,\n\
         nomo_string body,\n\
         nomo_async_context *context,\n    ",
    );
    out.push_str(&c_type(result));
    out.push_str(
        " *result\n\
         ) {\n\
             (void)url;\n\
             (void)body;\n\
             (void)context;\n\
             memset(registration, 0, sizeof(*registration));\n\
             *result = nomo_async_http_response_unavailable();\n\
             return NOMO_ASYNC_POLL_READY;\n\
         }\n\n",
    );
    emit_resume_abi(out, "post", result);
}

fn emit_send_abi(out: &mut String, request: &ValueType, result: &ValueType) {
    emit_start_header(out, "send");
    out.push_str("    ");
    out.push_str(&c_type(request));
    out.push_str(
        " request,\n\
         nomo_async_context *context,\n    ",
    );
    out.push_str(&c_type(result));
    out.push_str(
        " *result\n\
         ) {\n\
             (void)request;\n\
             (void)context;\n\
             memset(registration, 0, sizeof(*registration));\n\
             *result = nomo_async_http_response_unavailable();\n\
             return NOMO_ASYNC_POLL_READY;\n\
         }\n\n",
    );
    emit_resume_abi(out, "send", result);
}

fn emit_open_abi(out: &mut String, request: &ValueType, result: &ValueType) {
    emit_start_header(out, "open_stream");
    out.push_str("    ");
    out.push_str(&c_type(request));
    out.push_str(
        " request,\n\
         uint64_t idle_timeout_millis,\n\
         nomo_async_context *context,\n    ",
    );
    out.push_str(&c_type(result));
    out.push_str(
        " *result\n\
         ) {\n\
             (void)request;\n\
             (void)idle_timeout_millis;\n\
             (void)context;\n\
             memset(registration, 0, sizeof(*registration));\n\
             *result = nomo_async_http_stream_unavailable();\n\
             return NOMO_ASYNC_POLL_READY;\n\
         }\n\n",
    );
    emit_resume_abi(out, "open_stream", result);
}

fn emit_stream_pull_abi(
    out: &mut String,
    operation: &str,
    unavailable_helper: &str,
    stream: &ValueType,
    result: &ValueType,
) {
    emit_start_header(out, operation);
    out.push_str("    ");
    out.push_str(&c_type(stream));
    out.push_str(
        " stream,\n\
         uint64_t limit,\n\
         nomo_async_context *context,\n    ",
    );
    out.push_str(&c_type(result));
    out.push_str(
        " *result\n\
         ) {\n\
             (void)stream;\n\
             (void)limit;\n\
             (void)context;\n\
             memset(registration, 0, sizeof(*registration));\n\
             *result = ",
    );
    out.push_str(unavailable_helper);
    out.push_str(
        "();\n\
             return NOMO_ASYNC_POLL_READY;\n\
         }\n\n",
    );
    emit_resume_abi(out, operation, result);
}

fn emit_start_header(out: &mut String, operation: &str) {
    out.push_str("static nomo_async_poll nomo_async_http_");
    out.push_str(operation);
    out.push_str(
        "_start(\n\
             nomo_async_http_registration *registration,\n",
    );
}

fn emit_resume_abi(out: &mut String, operation: &str, result: &ValueType) {
    out.push_str("static nomo_async_poll nomo_async_http_");
    out.push_str(operation);
    out.push_str(
        "_resume(\n\
             nomo_async_http_registration *registration,\n\
             nomo_async_context *context,\n    ",
    );
    out.push_str(&c_type(result));
    out.push_str(
        " *result\n\
         ) {\n\
             (void)registration;\n\
             (void)context;\n\
             (void)result;\n\
             return NOMO_ASYNC_POLL_READY;\n\
         }\n\n",
    );
}

fn emit_stream_lifecycle_helpers(out: &mut String, stream: &ValueType) {
    for builtin in [
        BUILTIN_HTTP_CANCEL_STREAM_EXPR,
        BUILTIN_HTTP_CLOSE_STREAM_EXPR,
    ] {
        out.push_str("static void ");
        out.push_str(&c_fn_ident(builtin));
        out.push('(');
        out.push_str(&c_type(stream));
        out.push_str(
            " stream) {\n\
                 (void)stream;\n\
             }\n\n",
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn p2_http_a_emits_placeholder_suspend_abi_without_blocking_transport() {
        let mut emitted = String::new();
        emit_async_http_helpers(&mut emitted, true);

        for symbol in [
            "nomo_async_http_send_start",
            "nomo_async_http_send_resume",
            "nomo_async_http_open_stream_start",
            "nomo_async_http_read_text_resume",
            "nomo_async_http_next_sse_start",
            "nomo_async_http_cancel",
        ] {
            assert!(emitted.contains(symbol), "missing {symbol}");
        }
        assert!(emitted.contains("runtime_unavailable"));
        assert!(!emitted.contains("curl_easy_perform"));
        assert!(!emitted.contains("curl_multi_poll"));
        assert!(!emitted.contains("WinHttpSendRequest"));
    }
}
