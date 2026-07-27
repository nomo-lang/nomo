use super::*;

pub(super) fn standard_type_needs(imports: &[String], ast: &SourceFile) -> StandardTypeNeeds {
    StandardTypeNeeds {
        io: imports.iter().any(|item| item == "std.io.read_line")
            || (imports.iter().any(|item| item == "std.io") && source_uses_io_read_line(ast)),
        fs: imports
            .iter()
            .any(|item| item == "std.fs" || item.starts_with("std.fs."))
            || source_uses_fs_builtin(ast),
        env: imports
            .iter()
            .any(|item| item == "std.env" || item.starts_with("std.env."))
            || source_uses_env_builtin(ast),
        process: imports
            .iter()
            .any(|item| item == "std.process" || item.starts_with("std.process."))
            || source_uses_process_builtin(ast),
        task: imports
            .iter()
            .any(|item| item == "std.task" || item.starts_with("std.task.")),
        net: imports
            .iter()
            .any(|item| item == "std.net" || item.starts_with("std.net.")),
        http: imports
            .iter()
            .any(|item| item == "std.http" || item.starts_with("std.http.")),
        hash: imports
            .iter()
            .any(|item| item == "std.hash" || item.starts_with("std.hash."))
            || source_uses_hash_builtin(ast),
        json: imports.iter().any(|item| {
            item == "std.json"
                || item.starts_with("std.json.")
                || item == "std.jsonrpc"
                || item.starts_with("std.jsonrpc.")
        }) || source_uses_json_builtin(ast)
            || source_uses_jsonrpc_builtin(ast),
        jsonrpc: imports
            .iter()
            .any(|item| item == "std.jsonrpc" || item.starts_with("std.jsonrpc."))
            || source_uses_jsonrpc_builtin(ast),
        cron: imports
            .iter()
            .any(|item| item == "std.cron" || item.starts_with("std.cron.")),
        sqlite: imports
            .iter()
            .any(|item| item == "std.sqlite" || item.starts_with("std.sqlite.")),
        regex: imports
            .iter()
            .any(|item| item == "std.regex" || item.starts_with("std.regex."))
            || source_uses_regex_builtin(ast),
        collections: imports
            .iter()
            .any(|item| item == "std.collections" || item.starts_with("std.collections.")),
        map: imports
            .iter()
            .any(|item| item == "std.map" || item.starts_with("std.map.")),
        time: imports
            .iter()
            .any(|item| item == "std.time" || item.starts_with("std.time."))
            || imports.iter().any(|item| item == "std.task.sleep")
            || source_uses_time_builtin(ast)
            || source_uses_task_sleep(ast),
        num: imports
            .iter()
            .any(|item| item == "std.num" || item.starts_with("std.num."))
            || source_uses_num_builtin(ast),
        result: imports
            .iter()
            .any(|item| item == "std.result" || item.starts_with("std.result."))
            || source_uses_result_prelude_variant(ast),
        option: imports
            .iter()
            .any(|item| item == "std.option" || item == "std.option.Option")
            || source_uses_option_prelude_variant(ast)
            || imports
                .iter()
                .any(|item| item == "std.map" || item.starts_with("std.map.")),
        // std.collections/std.regex are backed by Array<string> and Option in v0.1.
        array: imports.iter().any(|item| {
            item == "std.array" || item == "std.array.Array" || item.starts_with("std.array.")
        }) || source_uses_array_builtin(ast)
            || imports.iter().any(|item| {
                item == "std.collections"
                    || item.starts_with("std.collections.")
                    || item == "std.map"
                    || item.starts_with("std.map.")
                    || item == "std.regex"
                    || item.starts_with("std.regex.")
                    || item == "std.sqlite"
                    || item.starts_with("std.sqlite.")
                    || item == "std.jsonrpc"
                    || item.starts_with("std.jsonrpc.")
            }),
        ffi: imports
            .iter()
            .any(|item| item == "std.ffi" || item.starts_with("std.ffi.")),
    }
}

pub(super) fn standard_struct_names(
    needs: StandardTypeNeeds,
) -> impl Iterator<Item = (String, usize)> {
    let mut names = Vec::new();
    if needs.io {
        names.push(("IoError".to_string(), 0));
    }
    if needs.fs {
        names.push(("FsError".to_string(), 0));
        names.push(("File".to_string(), 0));
        names.push(("FileMetadata".to_string(), 0));
    }
    if needs.num {
        names.push(("NumError".to_string(), 0));
    }
    if needs.process {
        names.push(("BlockingProcessChild".to_string(), 0));
        names.push(("ProcessChild".to_string(), 0));
        names.push(("ProcessCommand".to_string(), 0));
        names.push(("ProcessControlError".to_string(), 0));
        names.push(("ProcessEnv".to_string(), 0));
        names.push(("ProcessError".to_string(), 0));
        names.push(("ProcessExit".to_string(), 0));
        names.push(("ProcessOutput".to_string(), 0));
    }
    if needs.task {
        names.push(("Task".to_string(), 0));
        names.push(("TaskContext".to_string(), 0));
        names.push(("TaskError".to_string(), 0));
        names.push(("Channel".to_string(), 1));
        names.push(("ChannelError".to_string(), 0));
        names.push(("ChannelSendError".to_string(), 1));
    }
    if needs.net {
        names.push(("NetError".to_string(), 0));
        names.push(("TcpChunk".to_string(), 0));
        names.push(("TcpListener".to_string(), 0));
        names.push(("TcpStream".to_string(), 0));
        names.push(("TcpTextChunk".to_string(), 0));
        names.push(("UdpDatagram".to_string(), 0));
        names.push(("UdpSocket".to_string(), 0));
    }
    if needs.http {
        names.push(("BlockingHttpStream".to_string(), 0));
        names.push(("HttpExchange".to_string(), 0));
        names.push(("HttpError".to_string(), 0));
        names.push(("HttpHeader".to_string(), 0));
        names.push(("HttpRequest".to_string(), 0));
        names.push(("HttpResponse".to_string(), 0));
        names.push(("HttpServer".to_string(), 0));
        names.push(("HttpStream".to_string(), 0));
        names.push(("HttpStreamChunk".to_string(), 0));
        names.push(("SseEvent".to_string(), 0));
    }
    if needs.hash {
        names.push(("HashState".to_string(), 0));
    }
    if needs.json {
        names.push(("JsonValue".to_string(), 0));
        names.push(("JsonError".to_string(), 0));
        names.push(("JsonMember".to_string(), 0));
    }
    if needs.jsonrpc {
        names.push(("JsonRpcDecodeBatch".to_string(), 0));
        names.push(("JsonRpcDecoder".to_string(), 0));
        names.push(("JsonRpcMessage".to_string(), 0));
        names.push(("JsonRpcProtocolError".to_string(), 0));
    }
    if needs.cron {
        names.push(("CronError".to_string(), 0));
        names.push(("CronSchedule".to_string(), 0));
    }
    if needs.sqlite {
        names.push(("SqliteColumn".to_string(), 0));
        names.push(("SqliteDatabase".to_string(), 0));
        names.push(("SqliteError".to_string(), 0));
        names.push(("SqliteExecuteResult".to_string(), 0));
        names.push(("SqliteQuery".to_string(), 0));
        names.push(("SqliteRow".to_string(), 0));
    }
    if needs.regex {
        names.push(("Regex".to_string(), 0));
        names.push(("RegexError".to_string(), 0));
    }
    if needs.collections {
        names.push(("StringMap".to_string(), 0));
        names.push(("StringSet".to_string(), 0));
    }
    if needs.map {
        names.push(("Map".to_string(), 2));
    }
    if needs.time {
        names.push(("Duration".to_string(), 0));
    }
    names.into_iter()
}

pub(super) fn standard_enum_names(
    needs: StandardTypeNeeds,
) -> impl Iterator<Item = (String, usize)> {
    let mut names = Vec::new();
    if needs.net {
        names.push(("NetErrorKind".to_string(), 0));
    }
    if needs.process {
        names.push(("ProcessEvent".to_string(), 0));
    }
    if needs.task {
        names.push(("TaskJoin".to_string(), 0));
        names.push(("ChannelTrySend".to_string(), 1));
        names.push(("ChannelTryReceive".to_string(), 1));
    }
    if needs.json {
        names.push(("JsonKind".to_string(), 0));
    }
    if needs.jsonrpc {
        names.push(("JsonRpcMessageKind".to_string(), 0));
    }
    if needs.sqlite {
        names.push(("SqliteOpenMode".to_string(), 0));
        names.push(("SqliteValue".to_string(), 0));
    }
    if needs.io
        || needs.fs
        || needs.net
        || needs.http
        || needs.num
        || needs.process
        || needs.task
        || needs.json
        || needs.jsonrpc
        || needs.cron
        || needs.sqlite
        || needs.regex
        || needs.net
        || needs.result
    {
        names.push(("Result".to_string(), 2));
    }
    if needs.env
        || needs.http
        || needs.num
        || needs.process
        || needs.option
        || needs.array
        || needs.collections
        || needs.json
        || needs.jsonrpc
        || needs.sqlite
        || needs.regex
        || needs.net
        || needs.task
    {
        names.push(("Option".to_string(), 1));
    }
    names.into_iter()
}

pub(super) fn inject_standard_types(
    needs: StandardTypeNeeds,
    structs: &mut Vec<StructType>,
    enums: &mut Vec<EnumType>,
) {
    if needs.io && !structs.iter().any(|item| item.name == "IoError") {
        structs.push(StructType {
            package: "std.io".to_string(),
            name: "IoError".to_string(),
            type_params: Vec::new(),
            fields: vec![StructField {
                name: "message".to_string(),
                value_type: ValueType::String,
            }],
        });
    }
    if needs.fs && !structs.iter().any(|item| item.name == "FsError") {
        structs.push(StructType {
            package: "std.fs".to_string(),
            name: "FsError".to_string(),
            type_params: Vec::new(),
            fields: vec![StructField {
                name: "message".to_string(),
                value_type: ValueType::String,
            }],
        });
    }
    if needs.fs && !structs.iter().any(|item| item.name == "File") {
        structs.push(StructType {
            package: "std.fs".to_string(),
            name: "File".to_string(),
            type_params: Vec::new(),
            fields: Vec::new(),
        });
    }
    if needs.fs && !structs.iter().any(|item| item.name == "FileMetadata") {
        structs.push(StructType {
            package: "std.fs".to_string(),
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
        });
    }
    if needs.net && !structs.iter().any(|item| item.name == "NetError") {
        structs.push(StructType {
            package: "std.net".to_string(),
            name: "NetError".to_string(),
            type_params: Vec::new(),
            fields: vec![
                StructField {
                    name: "kind".to_string(),
                    value_type: ValueType::Enum("NetErrorKind".to_string(), Vec::new()),
                },
                StructField {
                    name: "message".to_string(),
                    value_type: ValueType::String,
                },
            ],
        });
    }
    if needs.net && !structs.iter().any(|item| item.name == "TcpStream") {
        structs.push(StructType {
            package: "std.net".to_string(),
            name: "TcpStream".to_string(),
            type_params: Vec::new(),
            fields: Vec::new(),
        });
    }
    if needs.net && !structs.iter().any(|item| item.name == "TcpChunk") {
        structs.push(StructType {
            package: "std.net".to_string(),
            name: "TcpChunk".to_string(),
            type_params: Vec::new(),
            fields: vec![
                StructField {
                    name: "data".to_string(),
                    value_type: ValueType::Array(Box::new(ValueType::U32)),
                },
                StructField {
                    name: "eof".to_string(),
                    value_type: ValueType::Bool,
                },
            ],
        });
    }
    if needs.net && !structs.iter().any(|item| item.name == "TcpTextChunk") {
        structs.push(StructType {
            package: "std.net".to_string(),
            name: "TcpTextChunk".to_string(),
            type_params: Vec::new(),
            fields: vec![
                StructField {
                    name: "data".to_string(),
                    value_type: ValueType::String,
                },
                StructField {
                    name: "eof".to_string(),
                    value_type: ValueType::Bool,
                },
            ],
        });
    }
    if needs.net && !structs.iter().any(|item| item.name == "TcpListener") {
        structs.push(StructType {
            package: "std.net".to_string(),
            name: "TcpListener".to_string(),
            type_params: Vec::new(),
            fields: Vec::new(),
        });
    }
    if needs.net && !structs.iter().any(|item| item.name == "UdpDatagram") {
        structs.push(StructType {
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
        });
    }
    if needs.net && !structs.iter().any(|item| item.name == "UdpSocket") {
        structs.push(StructType {
            package: "std.net".to_string(),
            name: "UdpSocket".to_string(),
            type_params: Vec::new(),
            fields: Vec::new(),
        });
    }
    if needs.http && !structs.iter().any(|item| item.name == "HttpError") {
        structs.push(StructType {
            package: "std.http".to_string(),
            name: "HttpError".to_string(),
            type_params: Vec::new(),
            fields: vec![
                StructField {
                    name: "code".to_string(),
                    value_type: ValueType::String,
                },
                StructField {
                    name: "message".to_string(),
                    value_type: ValueType::String,
                },
            ],
        });
    }
    if needs.http && !structs.iter().any(|item| item.name == "HttpHeader") {
        structs.push(StructType {
            package: "std.http".to_string(),
            name: "HttpHeader".to_string(),
            type_params: Vec::new(),
            fields: vec![
                StructField {
                    name: "name".to_string(),
                    value_type: ValueType::String,
                },
                StructField {
                    name: "value".to_string(),
                    value_type: ValueType::String,
                },
            ],
        });
    }
    if needs.http && !structs.iter().any(|item| item.name == "HttpRequest") {
        structs.push(StructType {
            package: "std.http".to_string(),
            name: "HttpRequest".to_string(),
            type_params: Vec::new(),
            fields: vec![
                StructField {
                    name: "method".to_string(),
                    value_type: ValueType::String,
                },
                StructField {
                    name: "url".to_string(),
                    value_type: ValueType::String,
                },
                StructField {
                    name: "headers".to_string(),
                    value_type: ValueType::Array(Box::new(ValueType::Struct(
                        "HttpHeader".to_string(),
                        Vec::new(),
                    ))),
                },
                StructField {
                    name: "body".to_string(),
                    value_type: ValueType::String,
                },
                StructField {
                    name: "timeout_millis".to_string(),
                    value_type: ValueType::U64,
                },
                StructField {
                    name: "max_response_bytes".to_string(),
                    value_type: ValueType::U64,
                },
            ],
        });
    }
    if needs.http && !structs.iter().any(|item| item.name == "HttpServer") {
        structs.push(StructType {
            package: "std.http".to_string(),
            name: "HttpServer".to_string(),
            type_params: Vec::new(),
            fields: Vec::new(),
        });
    }
    if needs.http && !structs.iter().any(|item| item.name == "HttpExchange") {
        structs.push(StructType {
            package: "std.http".to_string(),
            name: "HttpExchange".to_string(),
            type_params: Vec::new(),
            fields: vec![
                StructField {
                    name: "method".to_string(),
                    value_type: ValueType::String,
                },
                StructField {
                    name: "path".to_string(),
                    value_type: ValueType::String,
                },
                StructField {
                    name: "body".to_string(),
                    value_type: ValueType::String,
                },
            ],
        });
    }
    if needs.http && !structs.iter().any(|item| item.name == "HttpResponse") {
        structs.push(StructType {
            package: "std.http".to_string(),
            name: "HttpResponse".to_string(),
            type_params: Vec::new(),
            fields: vec![
                StructField {
                    name: "status".to_string(),
                    value_type: ValueType::Int,
                },
                StructField {
                    name: "headers".to_string(),
                    value_type: ValueType::Array(Box::new(ValueType::Struct(
                        "HttpHeader".to_string(),
                        Vec::new(),
                    ))),
                },
                StructField {
                    name: "body".to_string(),
                    value_type: ValueType::String,
                },
            ],
        });
    }
    if needs.http && !structs.iter().any(|item| item.name == "HttpStream") {
        structs.push(StructType {
            package: "std.http".to_string(),
            name: "HttpStream".to_string(),
            type_params: Vec::new(),
            fields: vec![
                StructField {
                    name: "status".to_string(),
                    value_type: ValueType::Int,
                },
                StructField {
                    name: "headers".to_string(),
                    value_type: ValueType::Array(Box::new(ValueType::Struct(
                        "HttpHeader".to_string(),
                        Vec::new(),
                    ))),
                },
            ],
        });
    }
    if needs.http && !structs.iter().any(|item| item.name == "BlockingHttpStream") {
        structs.push(StructType {
            package: "std.http".to_string(),
            name: "BlockingHttpStream".to_string(),
            type_params: Vec::new(),
            fields: vec![
                StructField {
                    name: "status".to_string(),
                    value_type: ValueType::Int,
                },
                StructField {
                    name: "headers".to_string(),
                    value_type: ValueType::Array(Box::new(ValueType::Struct(
                        "HttpHeader".to_string(),
                        Vec::new(),
                    ))),
                },
            ],
        });
    }
    if needs.http && !structs.iter().any(|item| item.name == "HttpStreamChunk") {
        structs.push(StructType {
            package: "std.http".to_string(),
            name: "HttpStreamChunk".to_string(),
            type_params: Vec::new(),
            fields: vec![
                StructField {
                    name: "data".to_string(),
                    value_type: ValueType::String,
                },
                StructField {
                    name: "done".to_string(),
                    value_type: ValueType::Bool,
                },
            ],
        });
    }
    if needs.http && !structs.iter().any(|item| item.name == "SseEvent") {
        structs.push(StructType {
            package: "std.http".to_string(),
            name: "SseEvent".to_string(),
            type_params: Vec::new(),
            fields: vec![
                StructField {
                    name: "event".to_string(),
                    value_type: ValueType::String,
                },
                StructField {
                    name: "data".to_string(),
                    value_type: ValueType::String,
                },
                StructField {
                    name: "id".to_string(),
                    value_type: ValueType::String,
                },
                StructField {
                    name: "retry_millis".to_string(),
                    value_type: ValueType::Enum("Option".to_string(), vec![ValueType::U64]),
                },
            ],
        });
    }
    if needs.num && !structs.iter().any(|item| item.name == "NumError") {
        structs.push(StructType {
            package: "std.num".to_string(),
            name: "NumError".to_string(),
            type_params: Vec::new(),
            fields: vec![StructField {
                name: "message".to_string(),
                value_type: ValueType::String,
            }],
        });
    }
    if needs.process && !structs.iter().any(|item| item.name == "ProcessError") {
        structs.push(StructType {
            package: "std.process".to_string(),
            name: "ProcessError".to_string(),
            type_params: Vec::new(),
            fields: vec![StructField {
                name: "message".to_string(),
                value_type: ValueType::String,
            }],
        });
    }
    if needs.process && !structs.iter().any(|item| item.name == "ProcessOutput") {
        structs.push(StructType {
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
        });
    }
    if needs.process && !structs.iter().any(|item| item.name == "ProcessEnv") {
        structs.push(StructType {
            package: "std.process".to_string(),
            name: "ProcessEnv".to_string(),
            type_params: Vec::new(),
            fields: vec![
                StructField {
                    name: "name".to_string(),
                    value_type: ValueType::String,
                },
                StructField {
                    name: "value".to_string(),
                    value_type: ValueType::String,
                },
            ],
        });
    }
    if needs.process && !structs.iter().any(|item| item.name == "ProcessCommand") {
        structs.push(StructType {
            package: "std.process".to_string(),
            name: "ProcessCommand".to_string(),
            type_params: Vec::new(),
            fields: vec![
                StructField {
                    name: "program".to_string(),
                    value_type: ValueType::String,
                },
                StructField {
                    name: "args".to_string(),
                    value_type: ValueType::Array(Box::new(ValueType::String)),
                },
                StructField {
                    name: "cwd".to_string(),
                    value_type: ValueType::Enum("Option".to_string(), vec![ValueType::String]),
                },
                StructField {
                    name: "env".to_string(),
                    value_type: ValueType::Array(Box::new(ValueType::Struct(
                        "ProcessEnv".to_string(),
                        Vec::new(),
                    ))),
                },
                StructField {
                    name: "inherit_env".to_string(),
                    value_type: ValueType::Bool,
                },
            ],
        });
    }
    if needs.process && !structs.iter().any(|item| item.name == "ProcessChild") {
        structs.push(StructType {
            package: "std.process".to_string(),
            name: "ProcessChild".to_string(),
            type_params: Vec::new(),
            fields: vec![StructField {
                name: "handle".to_string(),
                value_type: ValueType::U64,
            }],
        });
    }
    if needs.process
        && !structs
            .iter()
            .any(|item| item.name == "BlockingProcessChild")
    {
        structs.push(StructType {
            package: "std.process".to_string(),
            name: "BlockingProcessChild".to_string(),
            type_params: Vec::new(),
            fields: vec![StructField {
                name: "handle".to_string(),
                value_type: ValueType::U64,
            }],
        });
    }
    if needs.process && !structs.iter().any(|item| item.name == "ProcessExit") {
        structs.push(StructType {
            package: "std.process".to_string(),
            name: "ProcessExit".to_string(),
            type_params: Vec::new(),
            fields: vec![
                StructField {
                    name: "code".to_string(),
                    value_type: ValueType::I32,
                },
                StructField {
                    name: "signal".to_string(),
                    value_type: ValueType::I32,
                },
            ],
        });
    }
    if needs.process
        && !structs
            .iter()
            .any(|item| item.name == "ProcessControlError")
    {
        structs.push(StructType {
            package: "std.process".to_string(),
            name: "ProcessControlError".to_string(),
            type_params: Vec::new(),
            fields: vec![
                StructField {
                    name: "code".to_string(),
                    value_type: ValueType::String,
                },
                StructField {
                    name: "message".to_string(),
                    value_type: ValueType::String,
                },
            ],
        });
    }
    if needs.task && !structs.iter().any(|item| item.name == "Task") {
        structs.push(StructType {
            package: "std.task".to_string(),
            name: "Task".to_string(),
            type_params: Vec::new(),
            fields: vec![StructField {
                name: "handle".to_string(),
                value_type: ValueType::U64,
            }],
        });
    }
    if needs.task && !structs.iter().any(|item| item.name == "TaskContext") {
        structs.push(StructType {
            package: "std.task".to_string(),
            name: "TaskContext".to_string(),
            type_params: Vec::new(),
            fields: vec![StructField {
                name: "handle".to_string(),
                value_type: ValueType::U64,
            }],
        });
    }
    if needs.task && !structs.iter().any(|item| item.name == "TaskError") {
        structs.push(StructType {
            package: "std.task".to_string(),
            name: "TaskError".to_string(),
            type_params: Vec::new(),
            fields: vec![
                StructField {
                    name: "code".to_string(),
                    value_type: ValueType::String,
                },
                StructField {
                    name: "message".to_string(),
                    value_type: ValueType::String,
                },
            ],
        });
    }
    if needs.task && !structs.iter().any(|item| item.name == "Channel") {
        structs.push(StructType {
            package: "std.task".to_string(),
            name: "Channel".to_string(),
            type_params: vec!["T".to_string()],
            fields: vec![StructField {
                name: "handle".to_string(),
                value_type: ValueType::U64,
            }],
        });
    }
    if needs.task && !structs.iter().any(|item| item.name == "ChannelError") {
        structs.push(StructType {
            package: "std.task".to_string(),
            name: "ChannelError".to_string(),
            type_params: Vec::new(),
            fields: vec![
                StructField {
                    name: "code".to_string(),
                    value_type: ValueType::String,
                },
                StructField {
                    name: "message".to_string(),
                    value_type: ValueType::String,
                },
            ],
        });
    }
    if needs.task && !structs.iter().any(|item| item.name == "ChannelSendError") {
        structs.push(StructType {
            package: "std.task".to_string(),
            name: "ChannelSendError".to_string(),
            type_params: vec!["T".to_string()],
            fields: vec![
                StructField {
                    name: "error".to_string(),
                    value_type: ValueType::Struct("ChannelError".to_string(), Vec::new()),
                },
                StructField {
                    name: "value".to_string(),
                    value_type: ValueType::TypeParam("T".to_string()),
                },
            ],
        });
    }
    if needs.hash && !structs.iter().any(|item| item.name == "HashState") {
        structs.push(StructType {
            package: "std.hash".to_string(),
            name: "HashState".to_string(),
            type_params: Vec::new(),
            fields: vec![StructField {
                name: "value".to_string(),
                value_type: ValueType::U64,
            }],
        });
    }
    if needs.json && !structs.iter().any(|item| item.name == "JsonValue") {
        structs.push(StructType {
            package: "std.json".to_string(),
            name: "JsonValue".to_string(),
            type_params: Vec::new(),
            fields: vec![StructField {
                name: "raw".to_string(),
                value_type: ValueType::String,
            }],
        });
    }
    if needs.json && !structs.iter().any(|item| item.name == "JsonError") {
        structs.push(StructType {
            package: "std.json".to_string(),
            name: "JsonError".to_string(),
            type_params: Vec::new(),
            fields: vec![
                StructField {
                    name: "code".to_string(),
                    value_type: ValueType::String,
                },
                StructField {
                    name: "message".to_string(),
                    value_type: ValueType::String,
                },
                StructField {
                    name: "offset".to_string(),
                    value_type: ValueType::U64,
                },
            ],
        });
    }
    if needs.json && !structs.iter().any(|item| item.name == "JsonMember") {
        structs.push(StructType {
            package: "std.json".to_string(),
            name: "JsonMember".to_string(),
            type_params: Vec::new(),
            fields: vec![
                StructField {
                    name: "key".to_string(),
                    value_type: ValueType::String,
                },
                StructField {
                    name: "value".to_string(),
                    value_type: ValueType::Struct("JsonValue".to_string(), Vec::new()),
                },
            ],
        });
    }
    if needs.jsonrpc
        && !structs
            .iter()
            .any(|item| item.name == "JsonRpcProtocolError")
    {
        structs.push(StructType {
            package: "std.jsonrpc".to_string(),
            name: "JsonRpcProtocolError".to_string(),
            type_params: Vec::new(),
            fields: vec![
                StructField {
                    name: "code".to_string(),
                    value_type: ValueType::String,
                },
                StructField {
                    name: "message".to_string(),
                    value_type: ValueType::String,
                },
            ],
        });
    }
    if needs.jsonrpc && !structs.iter().any(|item| item.name == "JsonRpcMessage") {
        structs.push(StructType {
            package: "std.jsonrpc".to_string(),
            name: "JsonRpcMessage".to_string(),
            type_params: Vec::new(),
            fields: vec![StructField {
                name: "raw".to_string(),
                value_type: ValueType::String,
            }],
        });
    }
    if needs.jsonrpc && !structs.iter().any(|item| item.name == "JsonRpcDecoder") {
        structs.push(StructType {
            package: "std.jsonrpc".to_string(),
            name: "JsonRpcDecoder".to_string(),
            type_params: Vec::new(),
            fields: vec![
                StructField {
                    name: "pending".to_string(),
                    value_type: ValueType::String,
                },
                StructField {
                    name: "max_message_bytes".to_string(),
                    value_type: ValueType::U64,
                },
            ],
        });
    }
    if needs.jsonrpc && !structs.iter().any(|item| item.name == "JsonRpcDecodeBatch") {
        structs.push(StructType {
            package: "std.jsonrpc".to_string(),
            name: "JsonRpcDecodeBatch".to_string(),
            type_params: Vec::new(),
            fields: vec![
                StructField {
                    name: "decoder".to_string(),
                    value_type: ValueType::Struct("JsonRpcDecoder".to_string(), Vec::new()),
                },
                StructField {
                    name: "messages".to_string(),
                    value_type: ValueType::Array(Box::new(ValueType::Struct(
                        "JsonRpcMessage".to_string(),
                        Vec::new(),
                    ))),
                },
            ],
        });
    }
    if needs.cron && !structs.iter().any(|item| item.name == "CronSchedule") {
        structs.push(StructType {
            package: "std.cron".to_string(),
            name: "CronSchedule".to_string(),
            type_params: Vec::new(),
            fields: vec![StructField {
                name: "expression".to_string(),
                value_type: ValueType::String,
            }],
        });
    }
    if needs.cron && !structs.iter().any(|item| item.name == "CronError") {
        structs.push(StructType {
            package: "std.cron".to_string(),
            name: "CronError".to_string(),
            type_params: Vec::new(),
            fields: vec![
                StructField {
                    name: "code".to_string(),
                    value_type: ValueType::String,
                },
                StructField {
                    name: "message".to_string(),
                    value_type: ValueType::String,
                },
                StructField {
                    name: "field".to_string(),
                    value_type: ValueType::U64,
                },
            ],
        });
    }
    if needs.sqlite && !structs.iter().any(|item| item.name == "SqliteDatabase") {
        structs.push(StructType {
            package: "std.sqlite".to_string(),
            name: "SqliteDatabase".to_string(),
            type_params: Vec::new(),
            fields: vec![StructField {
                name: "handle".to_string(),
                value_type: ValueType::U64,
            }],
        });
    }
    if needs.sqlite && !structs.iter().any(|item| item.name == "SqliteQuery") {
        structs.push(StructType {
            package: "std.sqlite".to_string(),
            name: "SqliteQuery".to_string(),
            type_params: Vec::new(),
            fields: vec![StructField {
                name: "handle".to_string(),
                value_type: ValueType::U64,
            }],
        });
    }
    if needs.sqlite && !structs.iter().any(|item| item.name == "SqliteError") {
        structs.push(StructType {
            package: "std.sqlite".to_string(),
            name: "SqliteError".to_string(),
            type_params: Vec::new(),
            fields: vec![
                StructField {
                    name: "code".to_string(),
                    value_type: ValueType::String,
                },
                StructField {
                    name: "message".to_string(),
                    value_type: ValueType::String,
                },
                StructField {
                    name: "native_code".to_string(),
                    value_type: ValueType::Int,
                },
            ],
        });
    }
    if needs.sqlite
        && !structs
            .iter()
            .any(|item| item.name == "SqliteExecuteResult")
    {
        structs.push(StructType {
            package: "std.sqlite".to_string(),
            name: "SqliteExecuteResult".to_string(),
            type_params: Vec::new(),
            fields: vec![
                StructField {
                    name: "changes".to_string(),
                    value_type: ValueType::U64,
                },
                StructField {
                    name: "last_insert_rowid".to_string(),
                    value_type: ValueType::Int,
                },
            ],
        });
    }
    if needs.sqlite && !structs.iter().any(|item| item.name == "SqliteColumn") {
        structs.push(StructType {
            package: "std.sqlite".to_string(),
            name: "SqliteColumn".to_string(),
            type_params: Vec::new(),
            fields: vec![
                StructField {
                    name: "name".to_string(),
                    value_type: ValueType::String,
                },
                StructField {
                    name: "value".to_string(),
                    value_type: ValueType::Enum("SqliteValue".to_string(), Vec::new()),
                },
            ],
        });
    }
    if needs.sqlite && !structs.iter().any(|item| item.name == "SqliteRow") {
        structs.push(StructType {
            package: "std.sqlite".to_string(),
            name: "SqliteRow".to_string(),
            type_params: Vec::new(),
            fields: vec![StructField {
                name: "columns".to_string(),
                value_type: ValueType::Array(Box::new(ValueType::Struct(
                    "SqliteColumn".to_string(),
                    Vec::new(),
                ))),
            }],
        });
    }
    if needs.regex && !structs.iter().any(|item| item.name == "Regex") {
        structs.push(StructType {
            package: "std.regex".to_string(),
            name: "Regex".to_string(),
            type_params: Vec::new(),
            fields: vec![StructField {
                name: "pattern".to_string(),
                value_type: ValueType::String,
            }],
        });
    }
    if needs.regex && !structs.iter().any(|item| item.name == "RegexError") {
        structs.push(StructType {
            package: "std.regex".to_string(),
            name: "RegexError".to_string(),
            type_params: Vec::new(),
            fields: vec![StructField {
                name: "message".to_string(),
                value_type: ValueType::String,
            }],
        });
    }
    if needs.collections && !structs.iter().any(|item| item.name == "StringMap") {
        structs.push(StructType {
            package: "std.collections".to_string(),
            name: "StringMap".to_string(),
            type_params: Vec::new(),
            fields: vec![
                StructField {
                    name: "keys".to_string(),
                    value_type: ValueType::Array(Box::new(ValueType::String)),
                },
                StructField {
                    name: "values".to_string(),
                    value_type: ValueType::Array(Box::new(ValueType::String)),
                },
            ],
        });
    }
    if needs.map && !structs.iter().any(|item| item.name == "Map") {
        structs.push(StructType {
            package: "std.map".to_string(),
            name: "Map".to_string(),
            type_params: vec!["K".to_string(), "V".to_string()],
            fields: vec![
                StructField {
                    name: "keys".to_string(),
                    value_type: ValueType::Array(Box::new(ValueType::TypeParam("K".to_string()))),
                },
                StructField {
                    name: "values".to_string(),
                    value_type: ValueType::Array(Box::new(ValueType::TypeParam("V".to_string()))),
                },
            ],
        });
    }
    if needs.collections && !structs.iter().any(|item| item.name == "StringSet") {
        structs.push(StructType {
            package: "std.collections".to_string(),
            name: "StringSet".to_string(),
            type_params: Vec::new(),
            fields: vec![StructField {
                name: "values".to_string(),
                value_type: ValueType::Array(Box::new(ValueType::String)),
            }],
        });
    }
    if needs.time && !structs.iter().any(|item| item.name == "Duration") {
        structs.push(StructType {
            package: "std.time".to_string(),
            name: "Duration".to_string(),
            type_params: Vec::new(),
            fields: vec![StructField {
                name: "millis".to_string(),
                value_type: ValueType::Int,
            }],
        });
    }
    if needs.net && !enums.iter().any(|item| item.name == "NetErrorKind") {
        enums.push(EnumType {
            package: "std.net".to_string(),
            name: "NetErrorKind".to_string(),
            type_params: Vec::new(),
            variants: [
                "InvalidInput",
                "Unsupported",
                "Timeout",
                "Cancelled",
                "Closed",
                "Busy",
                "Limit",
                "Resolve",
                "Connect",
                "Read",
                "Write",
                "Reactor",
            ]
            .into_iter()
            .map(|name| EnumVariantType {
                name: name.to_string(),
                payload: None,
            })
            .collect(),
        });
    }
    if needs.process && !enums.iter().any(|item| item.name == "ProcessEvent") {
        enums.push(EnumType {
            package: "std.process".to_string(),
            name: "ProcessEvent".to_string(),
            type_params: Vec::new(),
            variants: vec![
                EnumVariantType {
                    name: "StdinFlushed".to_string(),
                    payload: None,
                },
                EnumVariantType {
                    name: "Stdout".to_string(),
                    payload: Some(ValueType::String),
                },
                EnumVariantType {
                    name: "Stderr".to_string(),
                    payload: Some(ValueType::String),
                },
                EnumVariantType {
                    name: "Exited".to_string(),
                    payload: Some(ValueType::Struct("ProcessExit".to_string(), Vec::new())),
                },
            ],
        });
    }
    if needs.task && !enums.iter().any(|item| item.name == "TaskJoin") {
        enums.push(EnumType {
            package: "std.task".to_string(),
            name: "TaskJoin".to_string(),
            type_params: Vec::new(),
            variants: vec![
                EnumVariantType {
                    name: "Completed".to_string(),
                    payload: Some(ValueType::String),
                },
                EnumVariantType {
                    name: "Cancelled".to_string(),
                    payload: None,
                },
                EnumVariantType {
                    name: "Timeout".to_string(),
                    payload: None,
                },
            ],
        });
    }
    if needs.task && !enums.iter().any(|item| item.name == "ChannelTrySend") {
        enums.push(EnumType {
            package: "std.task".to_string(),
            name: "ChannelTrySend".to_string(),
            type_params: vec!["T".to_string()],
            variants: vec![
                EnumVariantType {
                    name: "Sent".to_string(),
                    payload: None,
                },
                EnumVariantType {
                    name: "Full".to_string(),
                    payload: Some(ValueType::TypeParam("T".to_string())),
                },
                EnumVariantType {
                    name: "Closed".to_string(),
                    payload: Some(ValueType::TypeParam("T".to_string())),
                },
                EnumVariantType {
                    name: "Failed".to_string(),
                    payload: Some(ValueType::Struct(
                        "ChannelSendError".to_string(),
                        vec![ValueType::TypeParam("T".to_string())],
                    )),
                },
            ],
        });
    }
    if needs.task && !enums.iter().any(|item| item.name == "ChannelTryReceive") {
        enums.push(EnumType {
            package: "std.task".to_string(),
            name: "ChannelTryReceive".to_string(),
            type_params: vec!["T".to_string()],
            variants: vec![
                EnumVariantType {
                    name: "Value".to_string(),
                    payload: Some(ValueType::TypeParam("T".to_string())),
                },
                EnumVariantType {
                    name: "Empty".to_string(),
                    payload: None,
                },
                EnumVariantType {
                    name: "Closed".to_string(),
                    payload: None,
                },
            ],
        });
    }
    if needs.json && !enums.iter().any(|item| item.name == "JsonKind") {
        enums.push(EnumType {
            package: "std.json".to_string(),
            name: "JsonKind".to_string(),
            type_params: Vec::new(),
            variants: vec![
                EnumVariantType {
                    name: "Null".to_string(),
                    payload: None,
                },
                EnumVariantType {
                    name: "Boolean".to_string(),
                    payload: None,
                },
                EnumVariantType {
                    name: "Number".to_string(),
                    payload: None,
                },
                EnumVariantType {
                    name: "String".to_string(),
                    payload: None,
                },
                EnumVariantType {
                    name: "Array".to_string(),
                    payload: None,
                },
                EnumVariantType {
                    name: "Object".to_string(),
                    payload: None,
                },
            ],
        });
    }
    if needs.jsonrpc && !enums.iter().any(|item| item.name == "JsonRpcMessageKind") {
        enums.push(EnumType {
            package: "std.jsonrpc".to_string(),
            name: "JsonRpcMessageKind".to_string(),
            type_params: Vec::new(),
            variants: vec![
                EnumVariantType {
                    name: "Request".to_string(),
                    payload: None,
                },
                EnumVariantType {
                    name: "Notification".to_string(),
                    payload: None,
                },
                EnumVariantType {
                    name: "Success".to_string(),
                    payload: None,
                },
                EnumVariantType {
                    name: "Error".to_string(),
                    payload: None,
                },
            ],
        });
    }
    if needs.sqlite && !enums.iter().any(|item| item.name == "SqliteOpenMode") {
        enums.push(EnumType {
            package: "std.sqlite".to_string(),
            name: "SqliteOpenMode".to_string(),
            type_params: Vec::new(),
            variants: vec![
                EnumVariantType {
                    name: "ReadOnly".to_string(),
                    payload: None,
                },
                EnumVariantType {
                    name: "ReadWrite".to_string(),
                    payload: None,
                },
                EnumVariantType {
                    name: "ReadWriteCreate".to_string(),
                    payload: None,
                },
            ],
        });
    }
    if needs.sqlite && !enums.iter().any(|item| item.name == "SqliteValue") {
        enums.push(EnumType {
            package: "std.sqlite".to_string(),
            name: "SqliteValue".to_string(),
            type_params: Vec::new(),
            variants: vec![
                EnumVariantType {
                    name: "Null".to_string(),
                    payload: None,
                },
                EnumVariantType {
                    name: "Integer".to_string(),
                    payload: Some(ValueType::Int),
                },
                EnumVariantType {
                    name: "Real".to_string(),
                    payload: Some(ValueType::Float),
                },
                EnumVariantType {
                    name: "Text".to_string(),
                    payload: Some(ValueType::String),
                },
                EnumVariantType {
                    name: "Blob".to_string(),
                    payload: Some(ValueType::Array(Box::new(ValueType::U32))),
                },
            ],
        });
    }
    if (needs.io
        || needs.fs
        || needs.num
        || needs.process
        || needs.task
        || needs.json
        || needs.jsonrpc
        || needs.cron
        || needs.sqlite
        || needs.regex
        || needs.net
        || needs.result)
        && !enums.iter().any(|item| item.name == "Result")
    {
        enums.push(EnumType {
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
        });
    }
    if (needs.env
        || needs.http
        || needs.num
        || needs.process
        || needs.option
        || needs.array
        || needs.collections
        || needs.json
        || needs.jsonrpc
        || needs.sqlite
        || needs.regex
        || needs.net
        || needs.task)
        && !enums.iter().any(|item| item.name == "Option")
    {
        enums.push(EnumType {
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
        });
    }
}
