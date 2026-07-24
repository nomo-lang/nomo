use super::*;

pub(super) fn emit_sqlite_helpers(out: &mut String) {
    let database = ValueType::Struct("SqliteDatabase".to_string(), Vec::new());
    let query = ValueType::Struct("SqliteQuery".to_string(), Vec::new());
    let error = ValueType::Struct("SqliteError".to_string(), Vec::new());
    let execute = ValueType::Struct("SqliteExecuteResult".to_string(), Vec::new());
    let column = ValueType::Struct("SqliteColumn".to_string(), Vec::new());
    let row = ValueType::Struct("SqliteRow".to_string(), Vec::new());
    let sqlite_value = ValueType::Enum("SqliteValue".to_string(), Vec::new());
    let value_array_type = ValueType::Array(Box::new(sqlite_value.clone()));
    let column_array_type = ValueType::Array(Box::new(column.clone()));
    let byte_array_type = ValueType::Array(Box::new(ValueType::U32));
    let row_option = ValueType::Enum("Option".to_string(), vec![row.clone()]);

    let result_database_args = [database.clone(), error.clone()];
    let result_query_args = [query.clone(), error.clone()];
    let result_execute_args = [execute.clone(), error.clone()];
    let result_row_args = [row_option.clone(), error.clone()];
    let result_void_args = [ValueType::Void, error.clone()];

    let replacements: Vec<(&str, String)> = vec![
        ("@DATABASE@", c_struct_ident("SqliteDatabase", &[])),
        ("@QUERY@", c_struct_ident("SqliteQuery", &[])),
        ("@ERROR@", c_struct_ident("SqliteError", &[])),
        (
            "@EXECUTE_RESULT@",
            c_struct_ident("SqliteExecuteResult", &[]),
        ),
        ("@COLUMN@", c_struct_ident("SqliteColumn", &[])),
        ("@ROW@", c_struct_ident("SqliteRow", &[])),
        ("@OPEN_MODE@", c_enum_ident("SqliteOpenMode", &[])),
        ("@VALUE@", c_enum_ident("SqliteValue", &[])),
        ("@VALUE_ARRAY@", c_array_ident(&sqlite_value)),
        ("@COLUMN_ARRAY@", c_array_ident(&column)),
        ("@BYTE_ARRAY@", c_array_ident(&ValueType::U32)),
        ("@OPTION_ROW@", c_enum_ident("Option", &[row.clone()])),
        (
            "@RESULT_DATABASE@",
            c_enum_ident("Result", &result_database_args),
        ),
        ("@RESULT_QUERY@", c_enum_ident("Result", &result_query_args)),
        (
            "@RESULT_EXECUTE@",
            c_enum_ident("Result", &result_execute_args),
        ),
        ("@RESULT_ROW@", c_enum_ident("Result", &result_row_args)),
        ("@RESULT_VOID@", c_enum_ident("Result", &result_void_args)),
        (
            "@RESULT_DATABASE_OK@",
            c_enum_variant_ident("Result", &result_database_args, "Ok"),
        ),
        (
            "@RESULT_DATABASE_ERR@",
            c_enum_variant_ident("Result", &result_database_args, "Err"),
        ),
        (
            "@RESULT_QUERY_OK@",
            c_enum_variant_ident("Result", &result_query_args, "Ok"),
        ),
        (
            "@RESULT_QUERY_ERR@",
            c_enum_variant_ident("Result", &result_query_args, "Err"),
        ),
        (
            "@RESULT_EXECUTE_OK@",
            c_enum_variant_ident("Result", &result_execute_args, "Ok"),
        ),
        (
            "@RESULT_EXECUTE_ERR@",
            c_enum_variant_ident("Result", &result_execute_args, "Err"),
        ),
        (
            "@RESULT_ROW_OK@",
            c_enum_variant_ident("Result", &result_row_args, "Ok"),
        ),
        (
            "@RESULT_ROW_ERR@",
            c_enum_variant_ident("Result", &result_row_args, "Err"),
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
            "@OPTION_ROW_SOME@",
            c_enum_variant_ident("Option", &[row.clone()], "Some"),
        ),
        (
            "@OPTION_ROW_NONE@",
            c_enum_variant_ident("Option", &[row], "None"),
        ),
        (
            "@OPEN_READ_ONLY@",
            c_enum_variant_ident("SqliteOpenMode", &[], "ReadOnly"),
        ),
        (
            "@OPEN_READ_WRITE@",
            c_enum_variant_ident("SqliteOpenMode", &[], "ReadWrite"),
        ),
        (
            "@OPEN_READ_WRITE_CREATE@",
            c_enum_variant_ident("SqliteOpenMode", &[], "ReadWriteCreate"),
        ),
        (
            "@VALUE_NULL@",
            c_enum_variant_ident("SqliteValue", &[], "Null"),
        ),
        (
            "@VALUE_INTEGER@",
            c_enum_variant_ident("SqliteValue", &[], "Integer"),
        ),
        (
            "@VALUE_REAL@",
            c_enum_variant_ident("SqliteValue", &[], "Real"),
        ),
        (
            "@VALUE_TEXT@",
            c_enum_variant_ident("SqliteValue", &[], "Text"),
        ),
        (
            "@VALUE_BLOB@",
            c_enum_variant_ident("SqliteValue", &[], "Blob"),
        ),
        ("@OK_PAYLOAD@", c_payload_ident("Ok")),
        ("@ERR_PAYLOAD@", c_payload_ident("Err")),
        ("@SOME_PAYLOAD@", c_payload_ident("Some")),
        ("@INTEGER_PAYLOAD@", c_payload_ident("Integer")),
        ("@REAL_PAYLOAD@", c_payload_ident("Real")),
        ("@TEXT_PAYLOAD@", c_payload_ident("Text")),
        ("@BLOB_PAYLOAD@", c_payload_ident("Blob")),
        ("@HANDLE_MEMBER@", c_member_ident("handle")),
        ("@CODE_MEMBER@", c_member_ident("code")),
        ("@MESSAGE_MEMBER@", c_member_ident("message")),
        ("@NATIVE_CODE_MEMBER@", c_member_ident("native_code")),
        ("@CHANGES_MEMBER@", c_member_ident("changes")),
        (
            "@LAST_INSERT_ROWID_MEMBER@",
            c_member_ident("last_insert_rowid"),
        ),
        ("@NAME_MEMBER@", c_member_ident("name")),
        ("@VALUE_MEMBER@", c_member_ident("value")),
        ("@COLUMNS_MEMBER@", c_member_ident("columns")),
        ("@COLUMN_RELEASE@", c_release_ident(&column)),
        (
            "@COLUMN_ARRAY_RELEASE@",
            c_release_ident(&column_array_type),
        ),
        ("@VALUE_ARRAY_RELEASE@", c_release_ident(&value_array_type)),
        ("@BYTE_ARRAY_RELEASE@", c_release_ident(&byte_array_type)),
        ("@OPEN_NAME@", c_fn_ident(BUILTIN_SQLITE_OPEN_EXPR)),
        (
            "@OPEN_MEMORY_NAME@",
            c_fn_ident(BUILTIN_SQLITE_OPEN_MEMORY_EXPR),
        ),
        ("@EXECUTE_NAME@", c_fn_ident(BUILTIN_SQLITE_EXECUTE_EXPR)),
        ("@QUERY_NAME@", c_fn_ident(BUILTIN_SQLITE_QUERY_EXPR)),
        ("@NEXT_NAME@", c_fn_ident(BUILTIN_SQLITE_NEXT_EXPR)),
        ("@RESET_NAME@", c_fn_ident(BUILTIN_SQLITE_RESET_EXPR)),
        (
            "@CLOSE_QUERY_NAME@",
            c_fn_ident(BUILTIN_SQLITE_CLOSE_QUERY_EXPR),
        ),
        ("@CLOSE_NAME@", c_fn_ident(BUILTIN_SQLITE_CLOSE_EXPR)),
    ];

    let mut source = include_str!("host_sqlite.c").to_string();
    for (placeholder, replacement) in replacements {
        source = source.replace(placeholder, &replacement);
    }
    out.push_str(&source);
}
