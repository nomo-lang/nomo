use super::*;

#[test]
fn emits_os_helpers() {
    let program = Program {
        consts: Vec::new(),
        package: "app.main".to_string(),
        imports: vec!["std.os".to_string()],
        extern_functions: Vec::new(),
        structs: Vec::new(),
        enums: Vec::new(),
        functions: vec![Function {
            package: "app.main".to_string(),
            name: "main".to_string(),
            params: Vec::new(),
            return_type: ValueType::Void,
            body: vec![
                Statement::Let {
                    name: "platform".to_string(),
                    value_type: ValueType::String,
                    initializer: ValueExpr::OsPlatform,
                },
                Statement::Let {
                    name: "arch".to_string(),
                    value_type: ValueType::String,
                    initializer: ValueExpr::OsArch,
                },
                Statement::Let {
                    name: "separator".to_string(),
                    value_type: ValueType::String,
                    initializer: ValueExpr::OsPathSeparator,
                },
                Statement::Let {
                    name: "ending".to_string(),
                    value_type: ValueType::String,
                    initializer: ValueExpr::OsLineEnding,
                },
            ],
        }],
    };

    let c = emit_c(&program);
    assert!(c.contains("static nomo_string nomo_os_platform(void)"));
    assert!(c.contains("static nomo_string nomo_os_arch(void)"));
    assert!(c.contains("nomo_string nomo_platform = nomo_os_platform();"));
    assert!(c.contains("nomo_string nomo_separator = nomo_os_path_separator();"));
}

#[test]
fn emits_time_helpers() {
    let program = Program {
        consts: Vec::new(),
        package: "app.main".to_string(),
        imports: vec!["std.time".to_string()],
        extern_functions: Vec::new(),
        structs: vec![StructType {
            package: "std.time".to_string(),
            name: "Duration".to_string(),
            type_params: Vec::new(),
            fields: vec![StructField {
                name: "millis".to_string(),
                value_type: ValueType::Int,
            }],
        }],
        enums: Vec::new(),
        functions: vec![Function {
            package: "app.main".to_string(),
            name: "main".to_string(),
            params: Vec::new(),
            return_type: ValueType::Void,
            body: vec![
                Statement::Let {
                    name: "now".to_string(),
                    value_type: ValueType::Int,
                    initializer: ValueExpr::TimeNowMillis,
                },
                Statement::Let {
                    name: "tick".to_string(),
                    value_type: ValueType::Int,
                    initializer: ValueExpr::TimeMonotonicMillis,
                },
                Statement::Expr(ValueExpr::TimeSleepMillis {
                    duration: Box::new(ValueExpr::IntLiteral(0)),
                }),
                Statement::Let {
                    name: "duration".to_string(),
                    value_type: ValueType::Struct("Duration".to_string(), Vec::new()),
                    initializer: ValueExpr::TimeDurationMillis {
                        millis: Box::new(ValueExpr::IntLiteral(1500)),
                    },
                },
                Statement::Let {
                    name: "seconds".to_string(),
                    value_type: ValueType::Struct("Duration".to_string(), Vec::new()),
                    initializer: ValueExpr::TimeDurationSeconds {
                        seconds: Box::new(ValueExpr::IntLiteral(2)),
                    },
                },
                Statement::Let {
                    name: "millis".to_string(),
                    value_type: ValueType::Int,
                    initializer: ValueExpr::TimeDurationAsMillis {
                        duration: Box::new(ValueExpr::Variable("duration".to_string())),
                    },
                },
                Statement::Let {
                    name: "label".to_string(),
                    value_type: ValueType::String,
                    initializer: ValueExpr::TimeFormatDuration {
                        duration: Box::new(ValueExpr::Variable("duration".to_string())),
                    },
                },
                Statement::Expr(ValueExpr::TimeSleep {
                    duration: Box::new(ValueExpr::Variable("duration".to_string())),
                }),
            ],
        }],
    };

    let c = emit_c(&program);
    assert!(c.contains("#define nomo_struct_Duration nomo_pkg_std_time_struct_Duration"));
    assert!(c.contains("static int64_t nomo_time_now_millis(void)"));
    assert!(c.contains("static int64_t nomo_time_monotonic_millis(void)"));
    assert!(c.contains("static void nomo_time_sleep_millis(int64_t duration)"));
    assert!(c.contains("static int64_t nomo_time_duration_seconds_to_millis(int64_t seconds)"));
    assert!(c.contains("static nomo_string nomo_time_format_duration_millis(int64_t millis)"));
    assert!(c.contains("nomo_now = nomo_time_now_millis();"));
    assert!(c.contains("nomo_time_sleep_millis(0);"));
    assert!(c.contains(
        "nomo_struct_Duration nomo_duration = (nomo_struct_Duration){ .nomo_member_millis = 1500 };"
    ));
    assert!(c.contains(
            "nomo_struct_Duration nomo_seconds = (nomo_struct_Duration){ .nomo_member_millis = nomo_time_duration_seconds_to_millis(2) };"
        ));
    assert!(c.contains("long long nomo_millis = (nomo_duration).nomo_member_millis;"));
    assert!(c.contains(
            "nomo_string nomo_label = nomo_time_format_duration_millis((nomo_duration).nomo_member_millis);"
        ));
    assert!(c.contains("nomo_time_sleep_millis((nomo_duration).nomo_member_millis);"));
}

#[test]
fn emits_process_helpers() {
    let process_error = ValueType::Struct("ProcessError".to_string(), Vec::new());
    let program = Program {
        consts: Vec::new(),
        package: "app.main".to_string(),
        imports: vec!["std.process".to_string()],
        extern_functions: Vec::new(),
        structs: vec![
            StructType {
                package: "std.process".to_string(),
                name: "ProcessError".to_string(),
                type_params: Vec::new(),
                fields: vec![StructField {
                    name: "message".to_string(),
                    value_type: ValueType::String,
                }],
            },
            StructType {
                package: "std.process".to_string(),
                name: "ProcessOutput".to_string(),
                type_params: Vec::new(),
                fields: vec![
                    StructField {
                        name: "status".to_string(),
                        value_type: ValueType::I32,
                    },
                    StructField {
                        name: "stdout".to_string(),
                        value_type: ValueType::String,
                    },
                    StructField {
                        name: "stderr".to_string(),
                        value_type: ValueType::String,
                    },
                ],
            },
        ],
        enums: vec![EnumType {
            package: "std.result".to_string(),
            name: "Result".to_string(),
            type_params: vec!["T".to_string(), "E".to_string()],
            variants: vec![
                EnumVariantType {
                    name: "Ok".to_string(),
                    payload: Some(ValueType::TypeParam("T".to_string())),
                },
                EnumVariantType {
                    name: "Err".to_string(),
                    payload: Some(ValueType::TypeParam("E".to_string())),
                },
            ],
        }],
        functions: vec![Function {
            package: "app.main".to_string(),
            name: "main".to_string(),
            params: Vec::new(),
            return_type: ValueType::Void,
            body: vec![
                Statement::Let {
                    name: "spawned".to_string(),
                    value_type: ValueType::Enum(
                        "Result".to_string(),
                        vec![ValueType::I32, process_error.clone()],
                    ),
                    initializer: ValueExpr::ProcessSpawn {
                        command: Box::new(ValueExpr::StringLiteral("printf ok".to_string())),
                    },
                },
                Statement::Let {
                    name: "status".to_string(),
                    value_type: ValueType::Enum(
                        "Result".to_string(),
                        vec![ValueType::I32, process_error.clone()],
                    ),
                    initializer: ValueExpr::ProcessStatus {
                        command: Box::new(ValueExpr::StringLiteral("printf ok".to_string())),
                    },
                },
                Statement::Let {
                    name: "output".to_string(),
                    value_type: ValueType::Enum(
                        "Result".to_string(),
                        vec![ValueType::String, process_error.clone()],
                    ),
                    initializer: ValueExpr::ProcessExec {
                        command: Box::new(ValueExpr::StringLiteral("printf ok".to_string())),
                    },
                },
                Statement::Let {
                    name: "captured".to_string(),
                    value_type: ValueType::Enum(
                        "Result".to_string(),
                        vec![
                            ValueType::Struct("ProcessOutput".to_string(), Vec::new()),
                            process_error,
                        ],
                    ),
                    initializer: ValueExpr::ProcessOutput {
                        command: Box::new(ValueExpr::StringLiteral(
                            "printf ok; printf err 1>&2".to_string(),
                        )),
                    },
                },
                Statement::Expr(ValueExpr::ProcessExit {
                    code: Box::new(ValueExpr::IntLiteral(0)),
                }),
            ],
        }],
    };

    let c = emit_c(&program);
    assert!(c.contains("static int32_t nomo_process_exit_code(int status)"));
    assert!(c.contains("nomo_process_spawn(nomo_string command)"));
    assert!(c.contains("nomo_process_status(nomo_string command)"));
    assert!(c.contains("nomo_process_exec(nomo_string command)"));
    assert!(c.contains("nomo_process_output(nomo_string command)"));
    assert!(c.contains("nomo_spawned = nomo_process_spawn(nomo_string_literal(\"printf ok\"));"));
    assert!(c.contains("return nomo_process_spawn(command);"));
    assert!(c.contains("nomo_status = nomo_process_status(nomo_string_literal(\"printf ok\"));"));
    assert!(c.contains("nomo_output = nomo_process_exec(nomo_string_literal(\"printf ok\"));"));
    assert!(c.contains(
        "nomo_captured = nomo_process_output(nomo_string_literal(\"printf ok; printf err 1>&2\"));"
    ));
    assert!(c.contains("exit((int)0);"));
}

#[test]
fn emits_fixed_width_integer_types() {
    let program = Program {
        consts: Vec::new(),
        package: "app.main".to_string(),
        imports: vec!["std.io".to_string()],
        extern_functions: Vec::new(),
        structs: Vec::new(),
        enums: Vec::new(),
        functions: vec![
            Function {
                package: "app.main".to_string(),
                name: "add32".to_string(),
                params: vec![
                    Parameter {
                        name: "a".to_string(),
                        mutable: false,
                        value_type: ValueType::I32,
                    },
                    Parameter {
                        name: "b".to_string(),
                        mutable: false,
                        value_type: ValueType::I32,
                    },
                ],
                return_type: ValueType::I32,
                body: vec![Statement::Return(Some(ValueExpr::Binary {
                    left: Box::new(ValueExpr::Variable("a".to_string())),
                    op: BinaryOp::Add,
                    right: Box::new(ValueExpr::Variable("b".to_string())),
                    value_type: ValueType::I32,
                }))],
            },
            Function {
                package: "app.main".to_string(),
                name: "main".to_string(),
                params: Vec::new(),
                return_type: ValueType::Void,
                body: vec![
                    Statement::Let {
                        name: "signed".to_string(),
                        value_type: ValueType::I32,
                        initializer: ValueExpr::IntLiteral(1),
                    },
                    Statement::Let {
                        name: "word".to_string(),
                        value_type: ValueType::U32,
                        initializer: ValueExpr::IntLiteral(2),
                    },
                    Statement::Let {
                        name: "wide".to_string(),
                        value_type: ValueType::U64,
                        initializer: ValueExpr::IntLiteral(3),
                    },
                    Statement::Println(ValueExpr::StringLiteral("done".to_string())),
                ],
            },
        ],
    };

    let c = emit_c(&program);
    assert!(c.contains("#include <stdint.h>"));
    assert!(c.contains("int32_t nomo_fn_add32(int32_t nomo_a, int32_t nomo_b);"));
    assert!(c.contains("int32_t nomo_signed = 1;"));
    assert!(c.contains("uint32_t nomo_word = 2;"));
    assert!(c.contains("uint64_t nomo_wide = 3;"));
}

#[test]
fn emits_crypto_random_bytes_helper_after_array_u32_helper() {
    let program = Program {
        consts: Vec::new(),
        package: "app.main".to_string(),
        imports: vec!["std.crypto".to_string()],
        extern_functions: Vec::new(),
        structs: Vec::new(),
        enums: vec![EnumType {
            package: "std.option".to_string(),
            name: "Option".to_string(),
            type_params: vec!["T".to_string()],
            variants: vec![
                EnumVariantType {
                    name: "Some".to_string(),
                    payload: Some(ValueType::TypeParam("T".to_string())),
                },
                EnumVariantType {
                    name: "None".to_string(),
                    payload: None,
                },
            ],
        }],
        functions: vec![Function {
            package: "app.main".to_string(),
            name: "main".to_string(),
            params: Vec::new(),
            return_type: ValueType::Void,
            body: vec![Statement::Let {
                name: "bytes".to_string(),
                value_type: ValueType::Array(Box::new(ValueType::U32)),
                initializer: ValueExpr::CryptoRandomBytes {
                    count: Box::new(ValueExpr::Cast {
                        expr: Box::new(ValueExpr::IntLiteral(4)),
                        target_type: ValueType::U64,
                    }),
                },
            }],
        }],
    };

    let c = emit_c(&program);
    let array_helper = c
        .find("static nomo_array_u32 nomo_array_u32_new(void)")
        .unwrap();
    let crypto_helper = c
        .find("static nomo_array_u32 nomo_crypto_random_bytes(uint64_t count)")
        .unwrap();
    assert!(array_helper < crypto_helper);
    assert!(c.contains("#define _CRT_RAND_S"));
    assert!(c.contains("nomo_array_u32_push"));
    assert!(c.contains("nomo_array_u32 nomo_bytes = nomo_crypto_random_bytes(((uint64_t)4));"));
}

#[test]
fn emits_string_len_and_concat() {
    let program = Program {
        consts: Vec::new(),
        package: "app.main".to_string(),
        imports: vec!["std.io".to_string(), "std.string".to_string()],
        extern_functions: Vec::new(),
        structs: Vec::new(),
        enums: Vec::new(),
        functions: vec![Function {
            package: "app.main".to_string(),
            name: "main".to_string(),
            params: Vec::new(),
            return_type: ValueType::Void,
            body: vec![
                Statement::Let {
                    name: "message".to_string(),
                    value_type: ValueType::String,
                    initializer: ValueExpr::StringConcat {
                        left: Box::new(ValueExpr::StringLiteral("No".to_string())),
                        right: Box::new(ValueExpr::StringLiteral("mo".to_string())),
                    },
                },
                Statement::Let {
                    name: "count".to_string(),
                    value_type: ValueType::U64,
                    initializer: ValueExpr::StringLen {
                        value: Box::new(ValueExpr::Variable("message".to_string())),
                    },
                },
            ],
        }],
    };

    let c = emit_c(&program);
    assert!(c.contains("#include <string.h>"));
    assert!(c.contains("static nomo_string nomo_string_concat"));
    assert!(c.contains(
            "nomo_string nomo_message = nomo_string_concat(nomo_string_literal(\"No\"), nomo_string_literal(\"mo\"));"
        ));
    assert!(c.contains("uint64_t nomo_count = ((uint64_t)strlen((nomo_message).data));"));
    assert!(c.contains("nomo_string_release(nomo_message);"));
}

#[test]
fn emits_string_retain_and_release_for_shared_bindings() {
    let program = Program {
        consts: Vec::new(),
        package: "app.main".to_string(),
        imports: vec!["std.io".to_string(), "std.string".to_string()],
        extern_functions: Vec::new(),
        structs: Vec::new(),
        enums: Vec::new(),
        functions: vec![Function {
            package: "app.main".to_string(),
            name: "main".to_string(),
            params: Vec::new(),
            return_type: ValueType::Void,
            body: vec![
                Statement::Let {
                    name: "first".to_string(),
                    value_type: ValueType::String,
                    initializer: ValueExpr::StringConcat {
                        left: Box::new(ValueExpr::StringLiteral("No".to_string())),
                        right: Box::new(ValueExpr::StringLiteral("mo".to_string())),
                    },
                },
                Statement::Let {
                    name: "second".to_string(),
                    value_type: ValueType::String,
                    initializer: ValueExpr::Variable("first".to_string()),
                },
            ],
        }],
    };

    let c = emit_c(&program);
    assert!(c.contains("typedef struct nomo_string"));
    assert!(c.contains("static nomo_string nomo_string_retain(nomo_string value)"));
    assert!(c.contains("static void nomo_string_release(nomo_string value)"));
    assert!(c.contains("nomo_second = nomo_string_retain(nomo_second);"));
    let retain = c
        .find("nomo_second = nomo_string_retain(nomo_second);")
        .unwrap();
    let release_second = c[retain..]
        .find("nomo_string_release(nomo_second);")
        .unwrap()
        + retain;
    let release_first = c[release_second..]
        .find("nomo_string_release(nomo_first);")
        .unwrap()
        + release_second;
    assert!(retain < release_second);
    assert!(release_second < release_first);
}

#[test]
fn emits_string_parameter_retain_before_return_release() {
    let program = Program {
        consts: Vec::new(),
        package: "app.main".to_string(),
        imports: Vec::new(),
        extern_functions: Vec::new(),
        structs: Vec::new(),
        enums: Vec::new(),
        functions: vec![
            Function {
                package: "app.main".to_string(),
                name: "echo".to_string(),
                params: vec![Parameter {
                    name: "value".to_string(),
                    mutable: false,
                    value_type: ValueType::String,
                }],
                return_type: ValueType::String,
                body: vec![Statement::Return(Some(ValueExpr::Variable(
                    "value".to_string(),
                )))],
            },
            Function {
                package: "app.main".to_string(),
                name: "main".to_string(),
                params: Vec::new(),
                return_type: ValueType::Void,
                body: Vec::new(),
            },
        ],
    };

    let c = emit_c(&program);
    let fn_start = c
        .find("nomo_string nomo_fn_echo(nomo_string nomo_value)")
        .unwrap();
    let param_retain = c[fn_start..]
        .find("nomo_value = nomo_string_retain(nomo_value);")
        .unwrap()
        + fn_start;
    let return_retain = c[param_retain..]
        .find("nomo__return = nomo_string_retain(nomo__return);")
        .unwrap()
        + param_retain;
    let param_release = c[return_retain..]
        .find("nomo_string_release(nomo_value);")
        .unwrap()
        + return_retain;
    let return_stmt = c[param_release..].find("return nomo__return;").unwrap() + param_release;
    assert!(fn_start < param_retain);
    assert!(param_retain < return_retain);
    assert!(return_retain < param_release);
    assert!(param_release < return_stmt);
}

#[test]
fn emits_fs_read_and_write_helpers() {
    let fs_error = ValueType::Struct("FsError".to_string(), Vec::new());
    let result_string_error = ValueType::Enum(
        "Result".to_string(),
        vec![ValueType::String, fs_error.clone()],
    );
    let result_array_u32_error = ValueType::Enum(
        "Result".to_string(),
        vec![ValueType::Array(Box::new(ValueType::U32)), fs_error.clone()],
    );
    let result_void_error = ValueType::Enum(
        "Result".to_string(),
        vec![ValueType::Void, fs_error.clone()],
    );
    let result_metadata_error = ValueType::Enum(
        "Result".to_string(),
        vec![
            ValueType::Struct("FileMetadata".to_string(), Vec::new()),
            fs_error.clone(),
        ],
    );
    let result_array_string_error = ValueType::Enum(
        "Result".to_string(),
        vec![
            ValueType::Array(Box::new(ValueType::String)),
            fs_error.clone(),
        ],
    );
    let program = Program {
        consts: Vec::new(),
        package: "app.main".to_string(),
        imports: vec!["std.fs".to_string()],
        extern_functions: Vec::new(),
        structs: vec![
            StructType {
                package: "app.main".to_string(),
                name: "FsError".to_string(),
                type_params: Vec::new(),
                fields: vec![StructField {
                    name: "message".to_string(),
                    value_type: ValueType::String,
                }],
            },
            StructType {
                package: "app.main".to_string(),
                name: "FileMetadata".to_string(),
                type_params: Vec::new(),
                fields: vec![
                    StructField {
                        name: "is_file".to_string(),
                        value_type: ValueType::Bool,
                    },
                    StructField {
                        name: "is_dir".to_string(),
                        value_type: ValueType::Bool,
                    },
                    StructField {
                        name: "size".to_string(),
                        value_type: ValueType::U64,
                    },
                ],
            },
        ],
        enums: vec![
            EnumType {
                package: "app.main".to_string(),
                name: "Option".to_string(),
                type_params: vec!["T".to_string()],
                variants: vec![
                    EnumVariantType {
                        name: "Some".to_string(),
                        payload: Some(ValueType::TypeParam("T".to_string())),
                    },
                    EnumVariantType {
                        name: "None".to_string(),
                        payload: None,
                    },
                ],
            },
            EnumType {
                package: "app.main".to_string(),
                name: "Result".to_string(),
                type_params: vec!["T".to_string(), "E".to_string()],
                variants: vec![
                    EnumVariantType {
                        name: "Ok".to_string(),
                        payload: Some(ValueType::TypeParam("T".to_string())),
                    },
                    EnumVariantType {
                        name: "Err".to_string(),
                        payload: Some(ValueType::TypeParam("E".to_string())),
                    },
                ],
            },
        ],
        functions: vec![Function {
            package: "app.main".to_string(),
            name: "main".to_string(),
            params: Vec::new(),
            return_type: ValueType::Void,
            body: vec![
                Statement::Let {
                    name: "read_result".to_string(),
                    value_type: result_string_error,
                    initializer: ValueExpr::FsReadToString {
                        path: Box::new(ValueExpr::StringLiteral("input.txt".to_string())),
                    },
                },
                Statement::Let {
                    name: "write_result".to_string(),
                    value_type: result_void_error.clone(),
                    initializer: ValueExpr::FsWriteString {
                        path: Box::new(ValueExpr::StringLiteral("output.txt".to_string())),
                        content: Box::new(ValueExpr::StringLiteral("hello".to_string())),
                    },
                },
                Statement::Let {
                    name: "bytes_result".to_string(),
                    value_type: result_array_u32_error,
                    initializer: ValueExpr::FsReadBytes {
                        path: Box::new(ValueExpr::StringLiteral("input.bin".to_string())),
                    },
                },
                Statement::Let {
                    name: "write_bytes_result".to_string(),
                    value_type: result_void_error.clone(),
                    initializer: ValueExpr::FsWriteBytes {
                        path: Box::new(ValueExpr::StringLiteral("output.bin".to_string())),
                        bytes: Box::new(ValueExpr::CryptoRandomBytes {
                            count: Box::new(ValueExpr::Cast {
                                expr: Box::new(ValueExpr::IntLiteral(2)),
                                target_type: ValueType::U64,
                            }),
                        }),
                    },
                },
                Statement::Let {
                    name: "exists".to_string(),
                    value_type: ValueType::Bool,
                    initializer: ValueExpr::FsExists {
                        path: Box::new(ValueExpr::StringLiteral("tmp".to_string())),
                    },
                },
                Statement::Let {
                    name: "metadata_result".to_string(),
                    value_type: result_metadata_error,
                    initializer: ValueExpr::FsMetadata {
                        path: Box::new(ValueExpr::StringLiteral("tmp".to_string())),
                    },
                },
                Statement::Let {
                    name: "create_result".to_string(),
                    value_type: result_void_error.clone(),
                    initializer: ValueExpr::FsCreateDir {
                        path: Box::new(ValueExpr::StringLiteral("tmp".to_string())),
                    },
                },
                Statement::Let {
                    name: "entries_result".to_string(),
                    value_type: result_array_string_error,
                    initializer: ValueExpr::FsReadDir {
                        path: Box::new(ValueExpr::StringLiteral("tmp".to_string())),
                    },
                },
                Statement::Let {
                    name: "remove_result".to_string(),
                    value_type: result_void_error,
                    initializer: ValueExpr::FsRemoveDir {
                        path: Box::new(ValueExpr::StringLiteral("tmp".to_string())),
                    },
                },
            ],
        }],
    };

    let c = emit_c(&program);
    assert!(c.contains("#include <errno.h>"));
    assert!(c.contains("typedef struct nomo_struct_FsError"));
    assert!(c.contains("typedef struct nomo_struct_FileMetadata"));
    assert!(c.contains("static nomo_enum_Result_string_struct_FsError nomo_fs_read_to_string"));
    assert!(c.contains("static nomo_enum_Result_void_struct_FsError nomo_fs_write_string"));
    assert!(c.contains("static nomo_enum_Result_array_u32_struct_FsError nomo_fs_read_bytes"));
    assert!(c.contains("static nomo_enum_Result_void_struct_FsError nomo_fs_write_bytes"));
    assert!(c.contains("static int nomo_fs_exists"));
    assert!(
        c.contains("static nomo_enum_Result_struct_FileMetadata_struct_FsError nomo_fs_metadata")
    );
    assert!(c.contains("static nomo_enum_Result_void_struct_FsError nomo_fs_create_dir"));
    assert!(c.contains("static nomo_enum_Result_void_struct_FsError nomo_fs_remove_dir"));
    assert!(c.contains("static nomo_enum_Result_array_string_struct_FsError nomo_fs_read_dir"));
    assert!(c.contains("typedef struct nomo_array_string"));
    assert!(c.contains("nomo_fs_read_to_string(nomo_string_literal(\"input.txt\"))"));
    assert!(c.contains(
        "nomo_fs_write_string(nomo_string_literal(\"output.txt\"), nomo_string_literal(\"hello\"))"
    ));
    assert!(c.contains("nomo_array_u32_new"));
    let array_helper = c
        .find("static nomo_array_u32 nomo_array_u32_new(void)")
        .unwrap();
    let read_bytes_helper = c
        .find("static nomo_enum_Result_array_u32_struct_FsError nomo_fs_read_bytes")
        .unwrap();
    assert!(array_helper < read_bytes_helper);
    assert!(c.contains("nomo_fs_read_bytes(nomo_string_literal(\"input.bin\"))"));
    assert!(c.contains(
            "nomo_fs_write_bytes(nomo_string_literal(\"output.bin\"), nomo_crypto_random_bytes(((uint64_t)2)))"
        ));
    assert!(c.contains("nomo_fs_exists(nomo_string_literal(\"tmp\"))"));
    assert!(c.contains("nomo_fs_metadata(nomo_string_literal(\"tmp\"))"));
    assert!(c.contains("nomo_fs_create_dir(nomo_string_literal(\"tmp\"))"));
    assert!(c.contains("nomo_fs_read_dir(nomo_string_literal(\"tmp\"))"));
    assert!(c.contains("nomo_fs_remove_dir(nomo_string_literal(\"tmp\"))"));
}

#[test]
fn emits_file_read_write_close_helpers() {
    let fs_error = ValueType::Struct("FsError".to_string(), Vec::new());
    let file_type = ValueType::Struct("File".to_string(), Vec::new());
    let result_string_error = ValueType::Enum(
        "Result".to_string(),
        vec![ValueType::String, fs_error.clone()],
    );
    let result_void_error = ValueType::Enum(
        "Result".to_string(),
        vec![ValueType::Void, fs_error.clone()],
    );
    let program = Program {
        consts: Vec::new(),
        package: "app.main".to_string(),
        imports: vec!["std.fs".to_string()],
        extern_functions: Vec::new(),
        structs: vec![
            StructType {
                package: "app.main".to_string(),
                name: "FsError".to_string(),
                type_params: Vec::new(),
                fields: vec![StructField {
                    name: "message".to_string(),
                    value_type: ValueType::String,
                }],
            },
            StructType {
                package: "app.main".to_string(),
                name: "File".to_string(),
                type_params: Vec::new(),
                fields: Vec::new(),
            },
        ],
        enums: vec![EnumType {
            package: "app.main".to_string(),
            name: "Result".to_string(),
            type_params: vec!["T".to_string(), "E".to_string()],
            variants: vec![
                EnumVariantType {
                    name: "Ok".to_string(),
                    payload: Some(ValueType::TypeParam("T".to_string())),
                },
                EnumVariantType {
                    name: "Err".to_string(),
                    payload: Some(ValueType::TypeParam("E".to_string())),
                },
            ],
        }],
        functions: vec![
            Function {
                package: "app.main".to_string(),
                name: "process_file".to_string(),
                params: vec![Parameter {
                    name: "file".to_string(),
                    mutable: false,
                    value_type: file_type,
                }],
                return_type: ValueType::Void,
                body: vec![
                    Statement::Let {
                        name: "write_result".to_string(),
                        value_type: result_void_error,
                        initializer: ValueExpr::FileWriteString {
                            file: Box::new(ValueExpr::Variable("file".to_string())),
                            content: Box::new(ValueExpr::StringLiteral("hello".to_string())),
                        },
                    },
                    Statement::Let {
                        name: "read_result".to_string(),
                        value_type: result_string_error,
                        initializer: ValueExpr::FileReadToString {
                            file: Box::new(ValueExpr::Variable("file".to_string())),
                        },
                    },
                    Statement::Expr(ValueExpr::FileClose {
                        file: Box::new(ValueExpr::Variable("file".to_string())),
                    }),
                ],
            },
            Function {
                package: "app.main".to_string(),
                name: "main".to_string(),
                params: Vec::new(),
                return_type: ValueType::Void,
                body: Vec::new(),
            },
        ],
    };

    let c = emit_c(&program);
    assert!(c.contains("typedef struct nomo_struct_File"));
    assert!(c.contains("static nomo_enum_Result_string_struct_FsError nomo_file_read_to_string"));
    assert!(c.contains("static nomo_enum_Result_void_struct_FsError nomo_file_write_string"));
    assert!(c.contains("static void nomo_file_close"));
    assert!(c.contains("nomo_file_write_string(nomo_file, nomo_string_literal(\"hello\"))"));
    assert!(c.contains("nomo_file_read_to_string(nomo_file)"));
    assert!(c.contains("nomo_file_close(nomo_file)"));
}

#[test]
fn emits_net_tcp_stream_helpers() {
    let net_error = ValueType::Struct("NetError".to_string(), Vec::new());
    let tcp_listener = ValueType::Struct("TcpListener".to_string(), Vec::new());
    let tcp_stream = ValueType::Struct("TcpStream".to_string(), Vec::new());
    let result_listener_error = ValueType::Enum(
        "Result".to_string(),
        vec![tcp_listener.clone(), net_error.clone()],
    );
    let result_stream_error = ValueType::Enum(
        "Result".to_string(),
        vec![tcp_stream.clone(), net_error.clone()],
    );
    let result_string_error = ValueType::Enum(
        "Result".to_string(),
        vec![ValueType::String, net_error.clone()],
    );
    let result_void_error = ValueType::Enum("Result".to_string(), vec![ValueType::Void, net_error]);
    let program = Program {
        consts: Vec::new(),
        package: "app.main".to_string(),
        imports: vec!["std.net".to_string()],
        extern_functions: Vec::new(),
        structs: vec![
            StructType {
                package: "std.net".to_string(),
                name: "NetError".to_string(),
                type_params: Vec::new(),
                fields: vec![StructField {
                    name: "message".to_string(),
                    value_type: ValueType::String,
                }],
            },
            StructType {
                package: "std.net".to_string(),
                name: "TcpListener".to_string(),
                type_params: Vec::new(),
                fields: Vec::new(),
            },
            StructType {
                package: "std.net".to_string(),
                name: "TcpStream".to_string(),
                type_params: Vec::new(),
                fields: Vec::new(),
            },
        ],
        enums: vec![EnumType {
            package: "std.result".to_string(),
            name: "Result".to_string(),
            type_params: vec!["T".to_string(), "E".to_string()],
            variants: vec![
                EnumVariantType {
                    name: "Ok".to_string(),
                    payload: Some(ValueType::TypeParam("T".to_string())),
                },
                EnumVariantType {
                    name: "Err".to_string(),
                    payload: Some(ValueType::TypeParam("E".to_string())),
                },
            ],
        }],
        functions: vec![
            Function {
                package: "app.main".to_string(),
                name: "process_stream".to_string(),
                params: vec![Parameter {
                    name: "stream".to_string(),
                    mutable: false,
                    value_type: tcp_stream,
                }],
                return_type: ValueType::Void,
                body: vec![
                    Statement::Let {
                        name: "write_result".to_string(),
                        value_type: result_void_error,
                        initializer: ValueExpr::TcpStreamWriteString {
                            stream: Box::new(ValueExpr::Variable("stream".to_string())),
                            content: Box::new(ValueExpr::StringLiteral("ping".to_string())),
                        },
                    },
                    Statement::Let {
                        name: "read_result".to_string(),
                        value_type: result_string_error,
                        initializer: ValueExpr::TcpStreamReadToString {
                            stream: Box::new(ValueExpr::Variable("stream".to_string())),
                        },
                    },
                    Statement::Expr(ValueExpr::TcpStreamClose {
                        stream: Box::new(ValueExpr::Variable("stream".to_string())),
                    }),
                ],
            },
            Function {
                package: "app.main".to_string(),
                name: "process_listener".to_string(),
                params: vec![Parameter {
                    name: "listener".to_string(),
                    mutable: false,
                    value_type: tcp_listener,
                }],
                return_type: ValueType::Void,
                body: vec![
                    Statement::Let {
                        name: "accepted".to_string(),
                        value_type: result_stream_error.clone(),
                        initializer: ValueExpr::TcpListenerAccept {
                            listener: Box::new(ValueExpr::Variable("listener".to_string())),
                        },
                    },
                    Statement::Expr(ValueExpr::TcpListenerClose {
                        listener: Box::new(ValueExpr::Variable("listener".to_string())),
                    }),
                ],
            },
            Function {
                package: "app.main".to_string(),
                name: "main".to_string(),
                params: Vec::new(),
                return_type: ValueType::Void,
                body: vec![
                    Statement::Let {
                        name: "connected".to_string(),
                        value_type: result_stream_error,
                        initializer: ValueExpr::NetConnect {
                            host: Box::new(ValueExpr::StringLiteral("127.0.0.1".to_string())),
                            port: Box::new(ValueExpr::IntLiteral(7)),
                        },
                    },
                    Statement::Let {
                        name: "listening".to_string(),
                        value_type: result_listener_error,
                        initializer: ValueExpr::NetListen {
                            host: Box::new(ValueExpr::StringLiteral("127.0.0.1".to_string())),
                            port: Box::new(ValueExpr::IntLiteral(7)),
                        },
                    },
                ],
            },
        ],
    };

    let c = emit_c(&program);
    assert!(c.contains("typedef struct nomo_struct_TcpListener"));
    assert!(c.contains("typedef struct nomo_struct_TcpStream"));
    assert!(c.contains("nomo_socket nomo_member_handle"));
    assert!(c.contains("static nomo_string nomo_net_error_message(void)"));
    assert!(c.contains("nomo_net_connect(nomo_string host, int64_t port)"));
    assert!(c.contains("nomo_net_listen(nomo_string host, int64_t port)"));
    assert!(c.contains("nomo_tcp_listener_accept(nomo_struct_TcpListener listener)"));
    assert!(c.contains("static void nomo_tcp_listener_close(nomo_struct_TcpListener listener)"));
    assert!(c.contains("nomo_tcp_stream_read_to_string(nomo_struct_TcpStream stream)"));
    assert!(c.contains(
        "nomo_tcp_stream_write_string(nomo_struct_TcpStream stream, nomo_string content)"
    ));
    assert!(c.contains("static void nomo_tcp_stream_close(nomo_struct_TcpStream stream)"));
    assert!(c.contains("nomo_net_connect(nomo_string_literal(\"127.0.0.1\"), 7)"));
    assert!(c.contains("nomo_net_listen(nomo_string_literal(\"127.0.0.1\"), 7)"));
    assert!(c.contains("nomo_tcp_listener_accept(nomo_listener)"));
    assert!(c.contains("nomo_tcp_listener_close(nomo_listener)"));
    assert!(c.contains("nomo_tcp_stream_write_string(nomo_stream, nomo_string_literal(\"ping\"))"));
    assert!(c.contains("nomo_tcp_stream_read_to_string(nomo_stream)"));
    assert!(c.contains("nomo_tcp_stream_close(nomo_stream)"));
}

#[test]
fn emits_net_udp_socket_helpers() {
    let net_error = ValueType::Struct("NetError".to_string(), Vec::new());
    let udp_socket = ValueType::Struct("UdpSocket".to_string(), Vec::new());
    let udp_datagram = ValueType::Struct("UdpDatagram".to_string(), Vec::new());
    let result_socket_error = ValueType::Enum(
        "Result".to_string(),
        vec![udp_socket.clone(), net_error.clone()],
    );
    let result_datagram_error = ValueType::Enum(
        "Result".to_string(),
        vec![udp_datagram.clone(), net_error.clone()],
    );
    let result_void_error = ValueType::Enum(
        "Result".to_string(),
        vec![ValueType::Void, net_error.clone()],
    );
    let program = Program {
        consts: Vec::new(),
        package: "app.main".to_string(),
        imports: vec!["std.net".to_string()],
        extern_functions: Vec::new(),
        structs: vec![
            StructType {
                package: "std.net".to_string(),
                name: "NetError".to_string(),
                type_params: Vec::new(),
                fields: vec![StructField {
                    name: "message".to_string(),
                    value_type: ValueType::String,
                }],
            },
            StructType {
                package: "std.net".to_string(),
                name: "UdpDatagram".to_string(),
                type_params: Vec::new(),
                fields: vec![
                    StructField {
                        name: "data".to_string(),
                        value_type: ValueType::String,
                    },
                    StructField {
                        name: "host".to_string(),
                        value_type: ValueType::String,
                    },
                    StructField {
                        name: "port".to_string(),
                        value_type: ValueType::Int,
                    },
                ],
            },
            StructType {
                package: "std.net".to_string(),
                name: "UdpSocket".to_string(),
                type_params: Vec::new(),
                fields: Vec::new(),
            },
        ],
        enums: vec![EnumType {
            package: "std.result".to_string(),
            name: "Result".to_string(),
            type_params: vec!["T".to_string(), "E".to_string()],
            variants: vec![
                EnumVariantType {
                    name: "Ok".to_string(),
                    payload: Some(ValueType::TypeParam("T".to_string())),
                },
                EnumVariantType {
                    name: "Err".to_string(),
                    payload: Some(ValueType::TypeParam("E".to_string())),
                },
            ],
        }],
        functions: vec![
            Function {
                package: "app.main".to_string(),
                name: "process_socket".to_string(),
                params: vec![Parameter {
                    name: "socket".to_string(),
                    mutable: false,
                    value_type: udp_socket,
                }],
                return_type: ValueType::Void,
                body: vec![
                    Statement::Let {
                        name: "packet".to_string(),
                        value_type: result_datagram_error,
                        initializer: ValueExpr::UdpSocketRecvFromString {
                            socket: Box::new(ValueExpr::Variable("socket".to_string())),
                            max_bytes: Box::new(ValueExpr::IntLiteral(1024)),
                        },
                    },
                    Statement::Let {
                        name: "sent".to_string(),
                        value_type: result_void_error,
                        initializer: ValueExpr::UdpSocketSendToString {
                            socket: Box::new(ValueExpr::Variable("socket".to_string())),
                            content: Box::new(ValueExpr::StringLiteral("pong".to_string())),
                            host: Box::new(ValueExpr::StringLiteral("127.0.0.1".to_string())),
                            port: Box::new(ValueExpr::IntLiteral(7)),
                        },
                    },
                    Statement::Expr(ValueExpr::UdpSocketClose {
                        socket: Box::new(ValueExpr::Variable("socket".to_string())),
                    }),
                ],
            },
            Function {
                package: "app.main".to_string(),
                name: "main".to_string(),
                params: Vec::new(),
                return_type: ValueType::Void,
                body: vec![Statement::Let {
                    name: "bound".to_string(),
                    value_type: result_socket_error,
                    initializer: ValueExpr::NetUdpBind {
                        host: Box::new(ValueExpr::StringLiteral("127.0.0.1".to_string())),
                        port: Box::new(ValueExpr::IntLiteral(7)),
                    },
                }],
            },
        ],
    };

    let c = emit_c(&program);
    assert!(c.contains("typedef struct nomo_struct_UdpDatagram"));
    assert!(c.contains("typedef struct nomo_struct_UdpSocket"));
    assert!(c.contains("nomo_socket nomo_member_handle"));
    assert!(c.contains("nomo_net_udp_bind(nomo_string host, int64_t port)"));
    assert!(c.contains(
        "nomo_udp_socket_recv_from_string(nomo_struct_UdpSocket socket, int64_t max_bytes)"
    ));
    assert!(c.contains(
            "nomo_udp_socket_send_to_string(nomo_struct_UdpSocket socket, nomo_string content, nomo_string host, int64_t port)"
        ));
    assert!(c.contains("static void nomo_udp_socket_close(nomo_struct_UdpSocket socket)"));
    assert!(c.contains("nomo_net_udp_bind(nomo_string_literal(\"127.0.0.1\"), 7)"));
    assert!(c.contains("nomo_udp_socket_recv_from_string(nomo_socket, 1024)"));
    assert!(c.contains(
            "nomo_udp_socket_send_to_string(nomo_socket, nomo_string_literal(\"pong\"), nomo_string_literal(\"127.0.0.1\"), 7)"
        ));
    assert!(c.contains("nomo_udp_socket_close(nomo_socket)"));
}

#[test]
fn emits_io_read_line_helper() {
    let io_error = ValueType::Struct("IoError".to_string(), Vec::new());
    let result_string_error =
        ValueType::Enum("Result".to_string(), vec![ValueType::String, io_error]);
    let program = Program {
        consts: Vec::new(),
        package: "app.main".to_string(),
        imports: vec!["std.io".to_string()],
        extern_functions: Vec::new(),
        structs: vec![StructType {
            package: "std.io".to_string(),
            name: "IoError".to_string(),
            type_params: Vec::new(),
            fields: vec![StructField {
                name: "message".to_string(),
                value_type: ValueType::String,
            }],
        }],
        enums: vec![EnumType {
            package: "std.result".to_string(),
            name: "Result".to_string(),
            type_params: vec!["T".to_string(), "E".to_string()],
            variants: vec![
                EnumVariantType {
                    name: "Ok".to_string(),
                    payload: Some(ValueType::TypeParam("T".to_string())),
                },
                EnumVariantType {
                    name: "Err".to_string(),
                    payload: Some(ValueType::TypeParam("E".to_string())),
                },
            ],
        }],
        functions: vec![Function {
            package: "app.main".to_string(),
            name: "main".to_string(),
            params: Vec::new(),
            return_type: ValueType::Void,
            body: vec![Statement::Let {
                name: "read_result".to_string(),
                value_type: result_string_error,
                initializer: ValueExpr::IoReadLine,
            }],
        }],
    };

    let c = emit_c(&program);
    assert!(c.contains("typedef struct nomo_struct_IoError"));
    assert!(c.contains("static nomo_enum_Result_string_struct_IoError nomo_io_read_line"));
    assert!(c.contains("nomo_io_read_line()"));
}
