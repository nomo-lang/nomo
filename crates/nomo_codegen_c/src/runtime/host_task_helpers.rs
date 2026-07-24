use super::*;

pub(super) fn emit_task_helpers(out: &mut String, preflight_http: bool) {
    let task = ValueType::Struct("Task".to_string(), Vec::new());
    let error = ValueType::Struct("TaskError".to_string(), Vec::new());
    let join = ValueType::Enum("TaskJoin".to_string(), Vec::new());
    let result_task_args = vec![task.clone(), error.clone()];
    let result_join_args = vec![join, error.clone()];
    let result_void_args = vec![ValueType::Void, error];

    let http_preflight = if preflight_http {
        "#ifdef _WIN32\n    (void)nomo_winhttp_load();\n#else\n    (void)nomo_http_load_curl();\n#endif"
    } else {
        ""
    };

    let rendered = include_str!("host_task.c")
        .replace("@TASK@", &c_struct_ident("Task", &[]))
        .replace("@CONTEXT@", &c_struct_ident("TaskContext", &[]))
        .replace("@ERROR@", &c_struct_ident("TaskError", &[]))
        .replace("@JOIN@", &c_enum_ident("TaskJoin", &[]))
        .replace("@RESULT_TASK@", &c_enum_ident("Result", &result_task_args))
        .replace("@RESULT_JOIN@", &c_enum_ident("Result", &result_join_args))
        .replace("@RESULT_VOID@", &c_enum_ident("Result", &result_void_args))
        .replace(
            "@RESULT_TASK_OK@",
            &c_enum_variant_ident("Result", &result_task_args, "Ok"),
        )
        .replace(
            "@RESULT_TASK_ERR@",
            &c_enum_variant_ident("Result", &result_task_args, "Err"),
        )
        .replace(
            "@RESULT_JOIN_OK@",
            &c_enum_variant_ident("Result", &result_join_args, "Ok"),
        )
        .replace(
            "@RESULT_JOIN_ERR@",
            &c_enum_variant_ident("Result", &result_join_args, "Err"),
        )
        .replace(
            "@RESULT_VOID_OK@",
            &c_enum_variant_ident("Result", &result_void_args, "Ok"),
        )
        .replace(
            "@RESULT_VOID_ERR@",
            &c_enum_variant_ident("Result", &result_void_args, "Err"),
        )
        .replace(
            "@JOIN_COMPLETED@",
            &c_enum_variant_ident("TaskJoin", &[], "Completed"),
        )
        .replace(
            "@JOIN_CANCELLED@",
            &c_enum_variant_ident("TaskJoin", &[], "Cancelled"),
        )
        .replace(
            "@JOIN_TIMEOUT@",
            &c_enum_variant_ident("TaskJoin", &[], "Timeout"),
        )
        .replace("@OK_PAYLOAD@", &c_payload_ident("Ok"))
        .replace("@ERR_PAYLOAD@", &c_payload_ident("Err"))
        .replace("@COMPLETED_PAYLOAD@", &c_payload_ident("Completed"))
        .replace("@HANDLE_MEMBER@", &c_member_ident("handle"))
        .replace("@CODE_MEMBER@", &c_member_ident("code"))
        .replace("@MESSAGE_MEMBER@", &c_member_ident("message"))
        .replace("@SPAWN_NAME@", &c_fn_ident(BUILTIN_TASK_SPAWN_EXPR))
        .replace(
            "@IS_CANCELLED_NAME@",
            &c_fn_ident(BUILTIN_TASK_IS_CANCELLED_EXPR),
        )
        .replace("@JOIN_NAME@", &c_fn_ident(BUILTIN_TASK_JOIN_EXPR))
        .replace("@CANCEL_NAME@", &c_fn_ident(BUILTIN_TASK_CANCEL_EXPR))
        .replace("@CLOSE_NAME@", &c_fn_ident(BUILTIN_TASK_CLOSE_EXPR))
        .replace("@HTTP_PREFLIGHT@", http_preflight);
    out.push_str(&rendered);
}
