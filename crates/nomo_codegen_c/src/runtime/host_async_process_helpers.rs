use super::*;

pub(super) fn emit_async_process_helpers(out: &mut String, include_suspend_abi: bool) {
    let process_child = ValueType::Struct("ProcessChild".to_string(), Vec::new());
    let process_command = ValueType::Struct("ProcessCommand".to_string(), Vec::new());
    let process_error = ValueType::Struct("ProcessControlError".to_string(), Vec::new());
    let process_event = ValueType::Enum("ProcessEvent".to_string(), Vec::new());
    let process_exit = ValueType::Struct("ProcessExit".to_string(), Vec::new());
    let exit_option = ValueType::Enum("Option".to_string(), vec![process_exit]);
    let start_result = ValueType::Enum(
        "Result".to_string(),
        vec![process_child.clone(), process_error.clone()],
    );
    let void_result = ValueType::Enum(
        "Result".to_string(),
        vec![ValueType::Void, process_error.clone()],
    );
    let event_result = ValueType::Enum(
        "Result".to_string(),
        vec![process_event, process_error.clone()],
    );
    let wait_result = ValueType::Enum(
        "Result".to_string(),
        vec![exit_option, process_error.clone()],
    );
    let error_member = c_payload_ident("Err");
    let code_member = c_member_ident("code");
    let message_member = c_member_ident("message");

    if include_suspend_abi {
        out.push_str(
            "typedef struct {\n\
                 uint8_t active;\n\
             } nomo_async_process_registration;\n\n",
        );
    }
    out.push_str("static ");
    out.push_str(&c_type(&process_error));
    out.push_str(
        " nomo_async_process_unsupported_error(void) {\n\
             return (",
    );
    out.push_str(&c_type(&process_error));
    out.push_str("){.");
    out.push_str(&code_member);
    out.push_str(" = nomo_string_literal(\"unsupported\"), .");
    out.push_str(&message_member);
    out.push_str(
        " = nomo_string_literal(\"async process pipes are not available in this runtime slice\")};\n\
         }\n\n",
    );

    emit_async_process_error_result_helper(
        out,
        "nomo_async_process_start_unsupported",
        &start_result,
        &error_member,
    );
    emit_async_process_error_result_helper(
        out,
        "nomo_async_process_void_unsupported",
        &void_result,
        &error_member,
    );
    emit_async_process_error_result_helper(
        out,
        "nomo_async_process_event_unsupported",
        &event_result,
        &error_member,
    );
    emit_async_process_error_result_helper(
        out,
        "nomo_async_process_wait_unsupported",
        &wait_result,
        &error_member,
    );

    if include_suspend_abi {
        out.push_str("static nomo_async_poll nomo_async_process_spawn_start(\n");
        out.push_str("    nomo_async_process_registration *registration,\n    ");
        out.push_str(&c_type(&process_command));
        out.push_str(
            " command,\n\
             uint64_t timeout_millis,\n\
             nomo_async_context *context,\n\
             ",
        );
        out.push_str(&c_type(&start_result));
        out.push_str(
            " *result\n\
         ) {\n\
             (void)command;\n\
             (void)timeout_millis;\n\
             (void)context;\n\
             memset(registration, 0, sizeof(*registration));\n\
             *result = nomo_async_process_start_unsupported();\n\
             return NOMO_ASYNC_POLL_READY;\n\
         }\n\n\
         static nomo_async_poll nomo_async_process_spawn_resume(\n\
             nomo_async_process_registration *registration,\n\
             nomo_async_context *context,\n\
             ",
        );
        out.push_str(&c_type(&start_result));
        out.push_str(
            " *result\n\
         ) {\n\
             (void)registration;\n\
             (void)context;\n\
             (void)result;\n\
             return NOMO_ASYNC_POLL_READY;\n\
         }\n\n",
        );

        out.push_str("static nomo_async_poll nomo_async_process_event_start(\n");
        out.push_str("    nomo_async_process_registration *registration,\n    ");
        out.push_str(&c_type(&process_child));
        out.push_str(
            " child,\n\
             uint64_t max_chunk_bytes,\n\
             uint64_t timeout_millis,\n\
             nomo_async_context *context,\n\
             ",
        );
        out.push_str(&c_type(&event_result));
        out.push_str(
            " *result\n\
         ) {\n\
             (void)child;\n\
             (void)max_chunk_bytes;\n\
             (void)timeout_millis;\n\
             (void)context;\n\
             memset(registration, 0, sizeof(*registration));\n\
             *result = nomo_async_process_event_unsupported();\n\
             return NOMO_ASYNC_POLL_READY;\n\
         }\n\n\
         static nomo_async_poll nomo_async_process_event_resume(\n\
             nomo_async_process_registration *registration,\n\
             nomo_async_context *context,\n\
             ",
        );
        out.push_str(&c_type(&event_result));
        out.push_str(
            " *result\n\
         ) {\n\
             (void)registration;\n\
             (void)context;\n\
             (void)result;\n\
             return NOMO_ASYNC_POLL_READY;\n\
         }\n\n\
         static void nomo_async_process_cancel(\n\
             nomo_async_process_registration *registration,\n\
             nomo_async_context *context\n\
         ) {\n\
             (void)context;\n\
             registration->active = 0u;\n\
         }\n\n",
        );
    }

    emit_async_process_nonwaiting_helpers(out, &process_child, &void_result, &wait_result);
}

fn emit_async_process_error_result_helper(
    out: &mut String,
    name: &str,
    result_type: &ValueType,
    error_member: &str,
) {
    let ValueType::Enum(_, args) = result_type else {
        unreachable!("process result helper requires a Result enum");
    };
    out.push_str("static ");
    out.push_str(&c_type(result_type));
    out.push(' ');
    out.push_str(name);
    out.push_str("(void) {\n    return (");
    out.push_str(&c_type(result_type));
    out.push_str("){.tag = ");
    out.push_str(&c_enum_variant_ident("Result", args, "Err"));
    out.push_str(", .payload.");
    out.push_str(error_member);
    out.push_str(" = nomo_async_process_unsupported_error()};\n}\n\n");
}

fn emit_async_process_nonwaiting_helpers(
    out: &mut String,
    process_child: &ValueType,
    void_result: &ValueType,
    wait_result: &ValueType,
) {
    for (builtin, has_payload) in [
        (BUILTIN_PROCESS_WRITE_STDIN_EXPR, true),
        (BUILTIN_PROCESS_CLOSE_STDIN_EXPR, false),
        (BUILTIN_PROCESS_TERMINATE_EXPR, false),
    ] {
        out.push_str("static ");
        out.push_str(&c_type(void_result));
        out.push(' ');
        out.push_str(&c_fn_ident(builtin));
        out.push('(');
        out.push_str(&c_type(process_child));
        out.push_str(" child");
        if has_payload {
            out.push_str(", nomo_string data");
        }
        out.push_str(") {\n    (void)child;\n");
        if has_payload {
            out.push_str("    (void)data;\n");
        }
        out.push_str("    return nomo_async_process_void_unsupported();\n}\n\n");
    }

    out.push_str("static ");
    out.push_str(&c_type(wait_result));
    out.push(' ');
    out.push_str(&c_fn_ident(BUILTIN_PROCESS_TRY_WAIT_EXPR));
    out.push('(');
    out.push_str(&c_type(process_child));
    out.push_str(
        " child) {\n\
             (void)child;\n\
             return nomo_async_process_wait_unsupported();\n\
         }\n\n\
         static void ",
    );
    out.push_str(&c_fn_ident(BUILTIN_PROCESS_CLOSE_CHILD_EXPR));
    out.push('(');
    out.push_str(&c_type(process_child));
    out.push_str(
        " child) {\n\
             (void)child;\n\
         }\n",
    );
}
