use super::*;

pub(super) fn emit_cron_helpers(out: &mut String) {
    let schedule = ValueType::Struct("CronSchedule".to_string(), Vec::new());
    let error = ValueType::Struct("CronError".to_string(), Vec::new());
    let result_schedule_args = [schedule.clone(), error.clone()];
    let result_bool_args = [ValueType::Bool, error.clone()];
    let result_int_args = [ValueType::Int, error];

    let replacements: Vec<(&str, String)> = vec![
        ("@SCHEDULE@", c_struct_ident("CronSchedule", &[])),
        ("@CRON_ERROR@", c_struct_ident("CronError", &[])),
        (
            "@RESULT_SCHEDULE@",
            c_enum_ident("Result", &result_schedule_args),
        ),
        ("@RESULT_BOOL@", c_enum_ident("Result", &result_bool_args)),
        ("@RESULT_INT@", c_enum_ident("Result", &result_int_args)),
        (
            "@RESULT_SCHEDULE_OK@",
            c_enum_variant_ident("Result", &result_schedule_args, "Ok"),
        ),
        (
            "@RESULT_SCHEDULE_ERR@",
            c_enum_variant_ident("Result", &result_schedule_args, "Err"),
        ),
        (
            "@RESULT_BOOL_OK@",
            c_enum_variant_ident("Result", &result_bool_args, "Ok"),
        ),
        (
            "@RESULT_BOOL_ERR@",
            c_enum_variant_ident("Result", &result_bool_args, "Err"),
        ),
        (
            "@RESULT_INT_OK@",
            c_enum_variant_ident("Result", &result_int_args, "Ok"),
        ),
        (
            "@RESULT_INT_ERR@",
            c_enum_variant_ident("Result", &result_int_args, "Err"),
        ),
        ("@EXPRESSION_MEMBER@", c_member_ident("expression")),
        ("@CODE_MEMBER@", c_member_ident("code")),
        ("@MESSAGE_MEMBER@", c_member_ident("message")),
        ("@FIELD_MEMBER@", c_member_ident("field")),
        ("@OK_PAYLOAD@", c_payload_ident("Ok")),
        ("@ERR_PAYLOAD@", c_payload_ident("Err")),
    ];

    let mut source = include_str!("host_cron.c").to_string();
    for (placeholder, replacement) in replacements {
        source = source.replace(placeholder, &replacement);
    }
    out.push_str(&source);
}
