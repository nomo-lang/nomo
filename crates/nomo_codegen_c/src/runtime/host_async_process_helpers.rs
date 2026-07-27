use super::*;

pub(super) fn emit_async_process_helpers(
    out: &mut String,
    include_suspend_abi: bool,
    target: &nomo_target::TargetTriple,
) {
    if include_suspend_abi {
        match target.operating_system() {
            nomo_target::OperatingSystem::Windows => {
                emit_async_process_native_helpers(
                    out,
                    include_str!("host_async_process_windows.c"),
                );
            }
            nomo_target::OperatingSystem::Linux | nomo_target::OperatingSystem::Darwin => {
                emit_async_process_native_helpers(out, include_str!("host_async_process_unix.c"));
            }
        }
        return;
    }
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
    if include_suspend_abi {
        out.push_str(
            "\nstatic void nomo_async_process_runtime_shutdown(\n\
                 nomo_async_context *context\n\
             ) {\n\
                 context->process_runtime = NULL;\n\
             }\n",
        );
    }
}

fn emit_async_process_native_helpers(out: &mut String, template: &str) {
    let process_env = ValueType::Struct("ProcessEnv".to_string(), Vec::new());
    let process_child = ValueType::Struct("ProcessChild".to_string(), Vec::new());
    let process_command = ValueType::Struct("ProcessCommand".to_string(), Vec::new());
    let process_error = ValueType::Struct("ProcessControlError".to_string(), Vec::new());
    let process_event = ValueType::Enum("ProcessEvent".to_string(), Vec::new());
    let process_exit = ValueType::Struct("ProcessExit".to_string(), Vec::new());
    let exit_option = ValueType::Enum("Option".to_string(), vec![process_exit.clone()]);
    let start_result = ValueType::Enum(
        "Result".to_string(),
        vec![process_child, process_error.clone()],
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
        vec![exit_option.clone(), process_error],
    );
    let rendered = template
        .replace("@PROCESS_ENV@", &c_type(&process_env))
        .replace("@PROCESS_CHILD@", &c_struct_ident("ProcessChild", &[]))
        .replace("@PROCESS_COMMAND@", &c_type(&process_command))
        .replace(
            "@PROCESS_ERROR@",
            &c_struct_ident("ProcessControlError", &[]),
        )
        .replace("@PROCESS_EVENT@", &c_enum_ident("ProcessEvent", &[]))
        .replace("@PROCESS_EXIT@", &c_struct_ident("ProcessExit", &[]))
        .replace("@EXIT_OPTION@", &c_type(&exit_option))
        .replace("@START_RESULT@", &c_type(&start_result))
        .replace("@VOID_RESULT@", &c_type(&void_result))
        .replace("@EVENT_RESULT@", &c_type(&event_result))
        .replace("@WAIT_RESULT@", &c_type(&wait_result))
        .replace(
            "@START_OK@",
            &c_enum_variant_ident(
                "Result",
                &[
                    ValueType::Struct("ProcessChild".to_string(), Vec::new()),
                    ValueType::Struct("ProcessControlError".to_string(), Vec::new()),
                ],
                "Ok",
            ),
        )
        .replace(
            "@START_ERR@",
            &c_enum_variant_ident(
                "Result",
                &[
                    ValueType::Struct("ProcessChild".to_string(), Vec::new()),
                    ValueType::Struct("ProcessControlError".to_string(), Vec::new()),
                ],
                "Err",
            ),
        )
        .replace(
            "@VOID_OK@",
            &c_enum_variant_ident(
                "Result",
                &[
                    ValueType::Void,
                    ValueType::Struct("ProcessControlError".to_string(), Vec::new()),
                ],
                "Ok",
            ),
        )
        .replace(
            "@VOID_ERR@",
            &c_enum_variant_ident(
                "Result",
                &[
                    ValueType::Void,
                    ValueType::Struct("ProcessControlError".to_string(), Vec::new()),
                ],
                "Err",
            ),
        )
        .replace(
            "@EVENT_OK@",
            &c_enum_variant_ident(
                "Result",
                &[
                    ValueType::Enum("ProcessEvent".to_string(), Vec::new()),
                    ValueType::Struct("ProcessControlError".to_string(), Vec::new()),
                ],
                "Ok",
            ),
        )
        .replace(
            "@EVENT_ERR@",
            &c_enum_variant_ident(
                "Result",
                &[
                    ValueType::Enum("ProcessEvent".to_string(), Vec::new()),
                    ValueType::Struct("ProcessControlError".to_string(), Vec::new()),
                ],
                "Err",
            ),
        )
        .replace(
            "@WAIT_OK@",
            &c_enum_variant_ident(
                "Result",
                &[
                    exit_option.clone(),
                    ValueType::Struct("ProcessControlError".to_string(), Vec::new()),
                ],
                "Ok",
            ),
        )
        .replace(
            "@WAIT_ERR@",
            &c_enum_variant_ident(
                "Result",
                &[
                    exit_option,
                    ValueType::Struct("ProcessControlError".to_string(), Vec::new()),
                ],
                "Err",
            ),
        )
        .replace(
            "@CWD_SOME@",
            &c_enum_variant_ident("Option", &[ValueType::String], "Some"),
        )
        .replace(
            "@EXIT_SOME@",
            &c_enum_variant_ident(
                "Option",
                &[ValueType::Struct("ProcessExit".to_string(), Vec::new())],
                "Some",
            ),
        )
        .replace(
            "@EXIT_NONE@",
            &c_enum_variant_ident(
                "Option",
                &[ValueType::Struct("ProcessExit".to_string(), Vec::new())],
                "None",
            ),
        )
        .replace(
            "@EVENT_STDIN_FLUSHED@",
            &c_enum_variant_ident("ProcessEvent", &[], "StdinFlushed"),
        )
        .replace(
            "@EVENT_STDOUT@",
            &c_enum_variant_ident("ProcessEvent", &[], "Stdout"),
        )
        .replace(
            "@EVENT_STDERR@",
            &c_enum_variant_ident("ProcessEvent", &[], "Stderr"),
        )
        .replace(
            "@EVENT_EXITED@",
            &c_enum_variant_ident("ProcessEvent", &[], "Exited"),
        )
        .replace("@OK_PAYLOAD@", &c_payload_ident("Ok"))
        .replace("@ERR_PAYLOAD@", &c_payload_ident("Err"))
        .replace("@SOME_PAYLOAD@", &c_payload_ident("Some"))
        .replace("@STDOUT_PAYLOAD@", &c_payload_ident("Stdout"))
        .replace("@STDERR_PAYLOAD@", &c_payload_ident("Stderr"))
        .replace("@EXITED_PAYLOAD@", &c_payload_ident("Exited"))
        .replace("@PROGRAM_MEMBER@", &c_member_ident("program"))
        .replace("@ARGS_MEMBER@", &c_member_ident("args"))
        .replace("@CWD_MEMBER@", &c_member_ident("cwd"))
        .replace("@ENV_MEMBER@", &c_member_ident("env"))
        .replace("@INHERIT_ENV_MEMBER@", &c_member_ident("inherit_env"))
        .replace("@NAME_MEMBER@", &c_member_ident("name"))
        .replace("@VALUE_MEMBER@", &c_member_ident("value"))
        .replace("@HANDLE_MEMBER@", &c_member_ident("handle"))
        .replace("@OWNER_MEMBER@", &c_member_ident("owner"))
        .replace("@SLOT_MEMBER@", &c_member_ident("slot"))
        .replace("@GENERATION_MEMBER@", &c_member_ident("generation"))
        .replace("@CODE_MEMBER@", &c_member_ident("code"))
        .replace("@SIGNAL_MEMBER@", &c_member_ident("signal"))
        .replace("@MESSAGE_MEMBER@", &c_member_ident("message"))
        .replace("@START_NAME@", &c_fn_ident(BUILTIN_PROCESS_START_EXPR))
        .replace(
            "@WRITE_STDIN_NAME@",
            &c_fn_ident(BUILTIN_PROCESS_WRITE_STDIN_EXPR),
        )
        .replace(
            "@CLOSE_STDIN_NAME@",
            &c_fn_ident(BUILTIN_PROCESS_CLOSE_STDIN_EXPR),
        )
        .replace(
            "@NEXT_EVENT_NAME@",
            &c_fn_ident(BUILTIN_PROCESS_NEXT_EVENT_EXPR),
        )
        .replace(
            "@TRY_WAIT_NAME@",
            &c_fn_ident(BUILTIN_PROCESS_TRY_WAIT_EXPR),
        )
        .replace(
            "@TERMINATE_NAME@",
            &c_fn_ident(BUILTIN_PROCESS_TERMINATE_EXPR),
        )
        .replace(
            "@CLOSE_CHILD_NAME@",
            &c_fn_ident(BUILTIN_PROCESS_CLOSE_CHILD_EXPR),
        );
    out.push_str(&rendered);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unix_process_events_close_the_exit_registration_race() {
        let mut emitted = String::new();
        emit_async_process_native_helpers(&mut emitted, include_str!("host_async_process_unix.c"));

        assert!(
            emitted.contains(
                "if (exit_handle >= 0) {\n        nomo_async_process_update_exit(state);"
            )
        );
        assert!(emitted.contains("if (armed == 3) {"));
        assert!(emitted.contains(
            "now >= registration->deadline_millis) {\n            if (state->occupied == 1u"
        ));
    }

    #[test]
    fn windows_process_output_reads_outlive_one_event_pull() {
        let mut emitted = String::new();
        emit_async_process_native_helpers(
            &mut emitted,
            include_str!("host_async_process_windows.c"),
        );

        assert!(emitted.contains("nomo_async_reactor_registration output_registration[2];"));
        assert!(emitted.contains("char *output_read_buffers[2];"));
        assert!(emitted.contains(
            "static int nomo_async_process_output_issue_read(\n    nomo_async_process_handle_state *state,"
        ));
        assert!(emitted.contains("io->owner = &state->output_read_context[index];"));
        assert!(emitted.contains("nomo_async_process_buffer_append(\n            state->context,"));

        let finish = emitted
            .split("static void nomo_async_process_registration_finish(")
            .nth(1)
            .unwrap()
            .split("static void nomo_async_process_start_complete(")
            .next()
            .unwrap();
        assert!(!finish.contains("nomo_async_reactor_deregister"));
        assert!(!finish.contains("output_read_buffers"));
    }
}
