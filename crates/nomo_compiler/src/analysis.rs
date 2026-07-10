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
        json: imports
            .iter()
            .any(|item| item == "std.json" || item.starts_with("std.json."))
            || source_uses_json_builtin(ast),
        regex: imports
            .iter()
            .any(|item| item == "std.regex" || item.starts_with("std.regex."))
            || source_uses_regex_builtin(ast),
        collections: imports
            .iter()
            .any(|item| item == "std.collections" || item.starts_with("std.collections.")),
        time: imports
            .iter()
            .any(|item| item == "std.time" || item.starts_with("std.time."))
            || source_uses_time_builtin(ast),
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
            || source_uses_option_prelude_variant(ast),
        // std.collections/std.regex are backed by Array<string> and Option in v0.1.
        array: imports.iter().any(|item| {
            item == "std.array" || item == "std.array.Array" || item.starts_with("std.array.")
        }) || source_uses_array_builtin(ast)
            || imports.iter().any(|item| {
                item == "std.collections"
                    || item.starts_with("std.collections.")
                    || item == "std.regex"
                    || item.starts_with("std.regex.")
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
        names.push(("ProcessError".to_string(), 0));
        names.push(("ProcessOutput".to_string(), 0));
    }
    if needs.net {
        names.push(("NetError".to_string(), 0));
        names.push(("TcpListener".to_string(), 0));
        names.push(("TcpStream".to_string(), 0));
        names.push(("UdpDatagram".to_string(), 0));
        names.push(("UdpSocket".to_string(), 0));
    }
    if needs.http {
        names.push(("HttpExchange".to_string(), 0));
        names.push(("HttpError".to_string(), 0));
        names.push(("HttpResponse".to_string(), 0));
        names.push(("HttpServer".to_string(), 0));
    }
    if needs.hash {
        names.push(("HashState".to_string(), 0));
    }
    if needs.json {
        names.push(("JsonValue".to_string(), 0));
        names.push(("JsonError".to_string(), 0));
    }
    if needs.regex {
        names.push(("Regex".to_string(), 0));
        names.push(("RegexError".to_string(), 0));
    }
    if needs.collections {
        names.push(("StringMap".to_string(), 0));
        names.push(("StringSet".to_string(), 0));
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
    if needs.io
        || needs.fs
        || needs.net
        || needs.http
        || needs.num
        || needs.process
        || needs.json
        || needs.regex
        || needs.result
    {
        names.push(("Result".to_string(), 2));
    }
    if needs.env || needs.num || needs.option || needs.array || needs.collections || needs.regex {
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
            fields: vec![StructField {
                name: "message".to_string(),
                value_type: ValueType::String,
            }],
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
            fields: vec![StructField {
                name: "message".to_string(),
                value_type: ValueType::String,
            }],
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
                    name: "body".to_string(),
                    value_type: ValueType::String,
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
            fields: vec![StructField {
                name: "message".to_string(),
                value_type: ValueType::String,
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
    if (needs.io || needs.fs || needs.num || needs.json || needs.regex || needs.result)
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
    if (needs.env || needs.num || needs.option || needs.array || needs.collections || needs.regex)
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
