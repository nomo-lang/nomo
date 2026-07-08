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

fn source_uses_fs_builtin(ast: &SourceFile) -> bool {
    ast_functions(ast)
        .flat_map(|function| function.body.iter())
        .any(stmt_uses_fs_builtin)
}

fn source_uses_io_read_line(ast: &SourceFile) -> bool {
    ast_functions(ast)
        .flat_map(|function| function.body.iter())
        .any(stmt_uses_io_read_line)
}

fn source_uses_env_builtin(ast: &SourceFile) -> bool {
    ast_functions(ast)
        .flat_map(|function| function.body.iter())
        .any(stmt_uses_env_builtin)
}

fn source_uses_process_builtin(ast: &SourceFile) -> bool {
    ast_functions(ast)
        .flat_map(|function| function.body.iter())
        .any(stmt_uses_process_builtin)
}

fn source_uses_hash_builtin(ast: &SourceFile) -> bool {
    ast_functions(ast)
        .flat_map(|function| function.body.iter())
        .any(stmt_uses_hash_builtin)
}

fn source_uses_json_builtin(ast: &SourceFile) -> bool {
    ast_functions(ast)
        .flat_map(|function| function.body.iter())
        .any(stmt_uses_json_builtin)
}

fn source_uses_regex_builtin(ast: &SourceFile) -> bool {
    ast_functions(ast)
        .flat_map(|function| function.body.iter())
        .any(stmt_uses_regex_builtin)
}

fn source_uses_num_builtin(ast: &SourceFile) -> bool {
    ast_functions(ast)
        .flat_map(|function| function.body.iter())
        .any(stmt_uses_num_builtin)
}

fn source_uses_time_builtin(ast: &SourceFile) -> bool {
    ast_functions(ast)
        .flat_map(|function| function.body.iter())
        .any(stmt_uses_time_builtin)
}

fn source_uses_array_builtin(ast: &SourceFile) -> bool {
    ast_functions(ast)
        .flat_map(|function| function.body.iter())
        .any(stmt_uses_array_builtin)
}

fn source_uses_result_prelude_variant(ast: &SourceFile) -> bool {
    ast_functions(ast)
        .flat_map(|function| function.body.iter())
        .any(stmt_uses_result_prelude_variant)
        || ast
            .consts
            .iter()
            .any(|const_def| expr_uses_result_prelude_variant(&const_def.value))
}

fn source_uses_option_prelude_variant(ast: &SourceFile) -> bool {
    ast_functions(ast)
        .flat_map(|function| function.body.iter())
        .any(stmt_uses_option_prelude_variant)
        || ast
            .consts
            .iter()
            .any(|const_def| expr_uses_option_prelude_variant(&const_def.value))
}

fn stmt_uses_fs_builtin(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Let { value, .. } | Stmt::Assign { value, .. } => expr_uses_fs_builtin(value),
        Stmt::LetElse {
            value, else_body, ..
        } => expr_uses_fs_builtin(value) || else_body.iter().any(stmt_uses_fs_builtin),
        Stmt::IfLet {
            value,
            body,
            else_body,
            ..
        } => {
            expr_uses_fs_builtin(value)
                || body.iter().any(stmt_uses_fs_builtin)
                || else_body
                    .as_ref()
                    .is_some_and(|else_body| else_body.iter().any(stmt_uses_fs_builtin))
        }
        Stmt::Return { value, .. } => value.as_ref().is_some_and(expr_uses_fs_builtin),
        Stmt::Expr { expr, .. } => expr_uses_fs_builtin(expr),
        Stmt::Match { value, arms, .. } => {
            expr_uses_fs_builtin(value)
                || arms
                    .iter()
                    .any(|arm| arm.body.iter().any(stmt_uses_fs_builtin))
        }
        Stmt::For { variant, .. } => match variant {
            ForVariant::Infinite { body } => body.iter().any(stmt_uses_fs_builtin),
            ForVariant::While { condition, body } => {
                expr_uses_fs_builtin(condition) || body.iter().any(stmt_uses_fs_builtin)
            }
            ForVariant::Iterate { iterable, body, .. } => {
                expr_uses_fs_builtin(iterable) || body.iter().any(stmt_uses_fs_builtin)
            }
        },
        Stmt::Defer { stmt, .. } => stmt_uses_fs_builtin(stmt),
        Stmt::Unsafe { body, .. } => body.iter().any(stmt_uses_fs_builtin),
        Stmt::Postfix { .. } | Stmt::Break { .. } | Stmt::Continue { .. } => false,
    }
}

fn stmt_uses_io_read_line(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Let { value, .. } | Stmt::Assign { value, .. } => expr_uses_io_read_line(value),
        Stmt::LetElse {
            value, else_body, ..
        } => expr_uses_io_read_line(value) || else_body.iter().any(stmt_uses_io_read_line),
        Stmt::IfLet {
            value,
            body,
            else_body,
            ..
        } => {
            expr_uses_io_read_line(value)
                || body.iter().any(stmt_uses_io_read_line)
                || else_body
                    .as_ref()
                    .is_some_and(|else_body| else_body.iter().any(stmt_uses_io_read_line))
        }
        Stmt::Return { value, .. } => value.as_ref().is_some_and(expr_uses_io_read_line),
        Stmt::Expr { expr, .. } => expr_uses_io_read_line(expr),
        Stmt::Match { value, arms, .. } => {
            expr_uses_io_read_line(value)
                || arms
                    .iter()
                    .any(|arm| arm.body.iter().any(stmt_uses_io_read_line))
        }
        Stmt::For { variant, .. } => match variant {
            ForVariant::Infinite { body } => body.iter().any(stmt_uses_io_read_line),
            ForVariant::While { condition, body } => {
                expr_uses_io_read_line(condition) || body.iter().any(stmt_uses_io_read_line)
            }
            ForVariant::Iterate { iterable, body, .. } => {
                expr_uses_io_read_line(iterable) || body.iter().any(stmt_uses_io_read_line)
            }
        },
        Stmt::Defer { stmt, .. } => stmt_uses_io_read_line(stmt),
        Stmt::Unsafe { body, .. } => body.iter().any(stmt_uses_io_read_line),
        Stmt::Postfix { .. } | Stmt::Break { .. } | Stmt::Continue { .. } => false,
    }
}

fn stmt_uses_env_builtin(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Let { value, .. } | Stmt::Assign { value, .. } => expr_uses_env_builtin(value),
        Stmt::LetElse {
            value, else_body, ..
        } => expr_uses_env_builtin(value) || else_body.iter().any(stmt_uses_env_builtin),
        Stmt::IfLet {
            value,
            body,
            else_body,
            ..
        } => {
            expr_uses_env_builtin(value)
                || body.iter().any(stmt_uses_env_builtin)
                || else_body
                    .as_ref()
                    .is_some_and(|else_body| else_body.iter().any(stmt_uses_env_builtin))
        }
        Stmt::Return { value, .. } => value.as_ref().is_some_and(expr_uses_env_builtin),
        Stmt::Expr { expr, .. } => expr_uses_env_builtin(expr),
        Stmt::Match { value, arms, .. } => {
            expr_uses_env_builtin(value)
                || arms
                    .iter()
                    .any(|arm| arm.body.iter().any(stmt_uses_env_builtin))
        }
        Stmt::For { variant, .. } => match variant {
            ForVariant::Infinite { body } => body.iter().any(stmt_uses_env_builtin),
            ForVariant::While { condition, body } => {
                expr_uses_env_builtin(condition) || body.iter().any(stmt_uses_env_builtin)
            }
            ForVariant::Iterate { iterable, body, .. } => {
                expr_uses_env_builtin(iterable) || body.iter().any(stmt_uses_env_builtin)
            }
        },
        Stmt::Defer { stmt, .. } => stmt_uses_env_builtin(stmt),
        Stmt::Unsafe { body, .. } => body.iter().any(stmt_uses_env_builtin),
        Stmt::Postfix { .. } | Stmt::Break { .. } | Stmt::Continue { .. } => false,
    }
}

fn stmt_uses_process_builtin(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Let { value, .. } | Stmt::Assign { value, .. } => expr_uses_process_builtin(value),
        Stmt::LetElse {
            value, else_body, ..
        } => expr_uses_process_builtin(value) || else_body.iter().any(stmt_uses_process_builtin),
        Stmt::IfLet {
            value,
            body,
            else_body,
            ..
        } => {
            expr_uses_process_builtin(value)
                || body.iter().any(stmt_uses_process_builtin)
                || else_body
                    .as_ref()
                    .is_some_and(|else_body| else_body.iter().any(stmt_uses_process_builtin))
        }
        Stmt::Return { value, .. } => value.as_ref().is_some_and(expr_uses_process_builtin),
        Stmt::Expr { expr, .. } => expr_uses_process_builtin(expr),
        Stmt::Match { value, arms, .. } => {
            expr_uses_process_builtin(value)
                || arms
                    .iter()
                    .any(|arm| arm.body.iter().any(stmt_uses_process_builtin))
        }
        Stmt::For { variant, .. } => match variant {
            ForVariant::Infinite { body } => body.iter().any(stmt_uses_process_builtin),
            ForVariant::While { condition, body } => {
                expr_uses_process_builtin(condition) || body.iter().any(stmt_uses_process_builtin)
            }
            ForVariant::Iterate { iterable, body, .. } => {
                expr_uses_process_builtin(iterable) || body.iter().any(stmt_uses_process_builtin)
            }
        },
        Stmt::Defer { stmt, .. } => stmt_uses_process_builtin(stmt),
        Stmt::Unsafe { body, .. } => body.iter().any(stmt_uses_process_builtin),
        Stmt::Postfix { .. } | Stmt::Break { .. } | Stmt::Continue { .. } => false,
    }
}

fn stmt_uses_hash_builtin(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Let { value, .. } | Stmt::Assign { value, .. } => expr_uses_hash_builtin(value),
        Stmt::LetElse {
            value, else_body, ..
        } => expr_uses_hash_builtin(value) || else_body.iter().any(stmt_uses_hash_builtin),
        Stmt::IfLet {
            value,
            body,
            else_body,
            ..
        } => {
            expr_uses_hash_builtin(value)
                || body.iter().any(stmt_uses_hash_builtin)
                || else_body
                    .as_ref()
                    .is_some_and(|else_body| else_body.iter().any(stmt_uses_hash_builtin))
        }
        Stmt::Return { value, .. } => value.as_ref().is_some_and(expr_uses_hash_builtin),
        Stmt::Expr { expr, .. } => expr_uses_hash_builtin(expr),
        Stmt::Match { value, arms, .. } => {
            expr_uses_hash_builtin(value)
                || arms
                    .iter()
                    .any(|arm| arm.body.iter().any(stmt_uses_hash_builtin))
        }
        Stmt::For { variant, .. } => match variant {
            ForVariant::Infinite { body } => body.iter().any(stmt_uses_hash_builtin),
            ForVariant::While { condition, body } => {
                expr_uses_hash_builtin(condition) || body.iter().any(stmt_uses_hash_builtin)
            }
            ForVariant::Iterate { iterable, body, .. } => {
                expr_uses_hash_builtin(iterable) || body.iter().any(stmt_uses_hash_builtin)
            }
        },
        Stmt::Defer { stmt, .. } => stmt_uses_hash_builtin(stmt),
        Stmt::Unsafe { body, .. } => body.iter().any(stmt_uses_hash_builtin),
        Stmt::Postfix { .. } | Stmt::Break { .. } | Stmt::Continue { .. } => false,
    }
}

fn stmt_uses_json_builtin(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Let { value, .. } | Stmt::Assign { value, .. } => expr_uses_json_builtin(value),
        Stmt::LetElse {
            value, else_body, ..
        } => expr_uses_json_builtin(value) || else_body.iter().any(stmt_uses_json_builtin),
        Stmt::IfLet {
            value,
            body,
            else_body,
            ..
        } => {
            expr_uses_json_builtin(value)
                || body.iter().any(stmt_uses_json_builtin)
                || else_body
                    .as_ref()
                    .is_some_and(|else_body| else_body.iter().any(stmt_uses_json_builtin))
        }
        Stmt::Return { value, .. } => value.as_ref().is_some_and(expr_uses_json_builtin),
        Stmt::Expr { expr, .. } => expr_uses_json_builtin(expr),
        Stmt::Match { value, arms, .. } => {
            expr_uses_json_builtin(value)
                || arms
                    .iter()
                    .any(|arm| arm.body.iter().any(stmt_uses_json_builtin))
        }
        Stmt::For { variant, .. } => match variant {
            ForVariant::Infinite { body } => body.iter().any(stmt_uses_json_builtin),
            ForVariant::While { condition, body } => {
                expr_uses_json_builtin(condition) || body.iter().any(stmt_uses_json_builtin)
            }
            ForVariant::Iterate { iterable, body, .. } => {
                expr_uses_json_builtin(iterable) || body.iter().any(stmt_uses_json_builtin)
            }
        },
        Stmt::Defer { stmt, .. } => stmt_uses_json_builtin(stmt),
        Stmt::Unsafe { body, .. } => body.iter().any(stmt_uses_json_builtin),
        Stmt::Postfix { .. } | Stmt::Break { .. } | Stmt::Continue { .. } => false,
    }
}

fn stmt_uses_regex_builtin(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Let { value, .. } | Stmt::Assign { value, .. } => expr_uses_regex_builtin(value),
        Stmt::LetElse {
            value, else_body, ..
        } => expr_uses_regex_builtin(value) || else_body.iter().any(stmt_uses_regex_builtin),
        Stmt::IfLet {
            value,
            body,
            else_body,
            ..
        } => {
            expr_uses_regex_builtin(value)
                || body.iter().any(stmt_uses_regex_builtin)
                || else_body
                    .as_ref()
                    .is_some_and(|else_body| else_body.iter().any(stmt_uses_regex_builtin))
        }
        Stmt::Return { value, .. } => value.as_ref().is_some_and(expr_uses_regex_builtin),
        Stmt::Expr { expr, .. } => expr_uses_regex_builtin(expr),
        Stmt::Match { value, arms, .. } => {
            expr_uses_regex_builtin(value)
                || arms
                    .iter()
                    .any(|arm| arm.body.iter().any(stmt_uses_regex_builtin))
        }
        Stmt::For { variant, .. } => match variant {
            ForVariant::Infinite { body } => body.iter().any(stmt_uses_regex_builtin),
            ForVariant::While { condition, body } => {
                expr_uses_regex_builtin(condition) || body.iter().any(stmt_uses_regex_builtin)
            }
            ForVariant::Iterate { iterable, body, .. } => {
                expr_uses_regex_builtin(iterable) || body.iter().any(stmt_uses_regex_builtin)
            }
        },
        Stmt::Defer { stmt, .. } => stmt_uses_regex_builtin(stmt),
        Stmt::Unsafe { body, .. } => body.iter().any(stmt_uses_regex_builtin),
        Stmt::Postfix { .. } | Stmt::Break { .. } | Stmt::Continue { .. } => false,
    }
}

fn stmt_uses_num_builtin(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Let { value, .. } | Stmt::Assign { value, .. } => expr_uses_num_builtin(value),
        Stmt::LetElse {
            value, else_body, ..
        } => expr_uses_num_builtin(value) || else_body.iter().any(stmt_uses_num_builtin),
        Stmt::IfLet {
            value,
            body,
            else_body,
            ..
        } => {
            expr_uses_num_builtin(value)
                || body.iter().any(stmt_uses_num_builtin)
                || else_body
                    .as_ref()
                    .is_some_and(|else_body| else_body.iter().any(stmt_uses_num_builtin))
        }
        Stmt::Return { value, .. } => value.as_ref().is_some_and(expr_uses_num_builtin),
        Stmt::Expr { expr, .. } => expr_uses_num_builtin(expr),
        Stmt::Match { value, arms, .. } => {
            expr_uses_num_builtin(value)
                || arms
                    .iter()
                    .any(|arm| arm.body.iter().any(stmt_uses_num_builtin))
        }
        Stmt::For { variant, .. } => match variant {
            ForVariant::Infinite { body } => body.iter().any(stmt_uses_num_builtin),
            ForVariant::While { condition, body } => {
                expr_uses_num_builtin(condition) || body.iter().any(stmt_uses_num_builtin)
            }
            ForVariant::Iterate { iterable, body, .. } => {
                expr_uses_num_builtin(iterable) || body.iter().any(stmt_uses_num_builtin)
            }
        },
        Stmt::Defer { stmt, .. } => stmt_uses_num_builtin(stmt),
        Stmt::Unsafe { body, .. } => body.iter().any(stmt_uses_num_builtin),
        Stmt::Postfix { .. } | Stmt::Break { .. } | Stmt::Continue { .. } => false,
    }
}

fn stmt_uses_time_builtin(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Let { value, .. } | Stmt::Assign { value, .. } => expr_uses_time_builtin(value),
        Stmt::LetElse {
            value, else_body, ..
        } => expr_uses_time_builtin(value) || else_body.iter().any(stmt_uses_time_builtin),
        Stmt::IfLet {
            value,
            body,
            else_body,
            ..
        } => {
            expr_uses_time_builtin(value)
                || body.iter().any(stmt_uses_time_builtin)
                || else_body
                    .as_ref()
                    .is_some_and(|else_body| else_body.iter().any(stmt_uses_time_builtin))
        }
        Stmt::Return { value, .. } => value.as_ref().is_some_and(expr_uses_time_builtin),
        Stmt::Expr { expr, .. } => expr_uses_time_builtin(expr),
        Stmt::Match { value, arms, .. } => {
            expr_uses_time_builtin(value)
                || arms
                    .iter()
                    .any(|arm| arm.body.iter().any(stmt_uses_time_builtin))
        }
        Stmt::For { variant, .. } => match variant {
            ForVariant::Infinite { body } => body.iter().any(stmt_uses_time_builtin),
            ForVariant::While { condition, body } => {
                expr_uses_time_builtin(condition) || body.iter().any(stmt_uses_time_builtin)
            }
            ForVariant::Iterate { iterable, body, .. } => {
                expr_uses_time_builtin(iterable) || body.iter().any(stmt_uses_time_builtin)
            }
        },
        Stmt::Defer { stmt, .. } => stmt_uses_time_builtin(stmt),
        Stmt::Unsafe { body, .. } => body.iter().any(stmt_uses_time_builtin),
        Stmt::Postfix { .. } | Stmt::Break { .. } | Stmt::Continue { .. } => false,
    }
}

fn stmt_uses_array_builtin(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Let { value, .. } | Stmt::Assign { value, .. } => expr_uses_array_builtin(value),
        Stmt::LetElse {
            value, else_body, ..
        } => expr_uses_array_builtin(value) || else_body.iter().any(stmt_uses_array_builtin),
        Stmt::IfLet {
            value,
            body,
            else_body,
            ..
        } => {
            expr_uses_array_builtin(value)
                || body.iter().any(stmt_uses_array_builtin)
                || else_body
                    .as_ref()
                    .is_some_and(|else_body| else_body.iter().any(stmt_uses_array_builtin))
        }
        Stmt::Return { value, .. } => value.as_ref().is_some_and(expr_uses_array_builtin),
        Stmt::Expr { expr, .. } => expr_uses_array_builtin(expr),
        Stmt::Match { value, arms, .. } => {
            expr_uses_array_builtin(value)
                || arms
                    .iter()
                    .any(|arm| arm.body.iter().any(stmt_uses_array_builtin))
        }
        Stmt::For { variant, .. } => match variant {
            ForVariant::Infinite { body } => body.iter().any(stmt_uses_array_builtin),
            ForVariant::While { condition, body } => {
                expr_uses_array_builtin(condition) || body.iter().any(stmt_uses_array_builtin)
            }
            ForVariant::Iterate { iterable, body, .. } => {
                expr_uses_array_builtin(iterable) || body.iter().any(stmt_uses_array_builtin)
            }
        },
        Stmt::Defer { stmt, .. } => stmt_uses_array_builtin(stmt),
        Stmt::Unsafe { body, .. } => body.iter().any(stmt_uses_array_builtin),
        Stmt::Postfix { .. } | Stmt::Break { .. } | Stmt::Continue { .. } => false,
    }
}

fn expr_uses_fs_builtin(expr: &AstExpr) -> bool {
    match expr {
        AstExpr::Call { callee, args, .. } => {
            (callee == &["fs", "read_to_string"]
                || callee == &["fs", "write_string"]
                || callee == &["fs", "read_bytes"]
                || callee == &["fs", "write_bytes"]
                || callee == &["fs", "exists"]
                || callee == &["fs", "metadata"]
                || callee == &["fs", "create_dir"]
                || callee == &["fs", "remove_dir"]
                || callee == &["fs", "read_dir"]
                || callee == &["fs", "open"])
                || args.iter().any(expr_uses_fs_builtin)
        }
        AstExpr::StructLiteral { fields, .. } => {
            fields.iter().any(|(_, value)| expr_uses_fs_builtin(value))
        }
        AstExpr::Match { value, arms } => {
            expr_uses_fs_builtin(value) || arms.iter().any(|arm| expr_uses_fs_builtin(&arm.value))
        }
        AstExpr::If {
            condition,
            then_branch,
            else_branch,
        } => {
            expr_uses_fs_builtin(condition)
                || expr_uses_fs_builtin(then_branch)
                || expr_uses_fs_builtin(else_branch)
        }
        AstExpr::Panic { message }
        | AstExpr::Question { expr: message }
        | AstExpr::Unary { expr: message, .. } => expr_uses_fs_builtin(message),
        AstExpr::MutArg { .. } => false,
        AstExpr::Cast { expr, .. } => expr_uses_fs_builtin(expr),
        AstExpr::Binary { left, right, .. } => {
            expr_uses_fs_builtin(left) || expr_uses_fs_builtin(right)
        }
        AstExpr::Name(_)
        | AstExpr::String(_)
        | AstExpr::Int(_)
        | AstExpr::Float(_)
        | AstExpr::Char(_)
        | AstExpr::Bool(_)
        | AstExpr::Void => false,
    }
}

fn expr_uses_io_read_line(expr: &AstExpr) -> bool {
    match expr {
        AstExpr::Call { callee, args, .. } => {
            callee == &["io", "read_line"] || args.iter().any(expr_uses_io_read_line)
        }
        AstExpr::StructLiteral { fields, .. } => fields
            .iter()
            .any(|(_, value)| expr_uses_io_read_line(value)),
        AstExpr::Match { value, arms } => {
            expr_uses_io_read_line(value)
                || arms.iter().any(|arm| expr_uses_io_read_line(&arm.value))
        }
        AstExpr::If {
            condition,
            then_branch,
            else_branch,
        } => {
            expr_uses_io_read_line(condition)
                || expr_uses_io_read_line(then_branch)
                || expr_uses_io_read_line(else_branch)
        }
        AstExpr::Panic { message }
        | AstExpr::Question { expr: message }
        | AstExpr::Unary { expr: message, .. } => expr_uses_io_read_line(message),
        AstExpr::Cast { expr, .. } => expr_uses_io_read_line(expr),
        AstExpr::Binary { left, right, .. } => {
            expr_uses_io_read_line(left) || expr_uses_io_read_line(right)
        }
        AstExpr::MutArg { .. }
        | AstExpr::Name(_)
        | AstExpr::String(_)
        | AstExpr::Int(_)
        | AstExpr::Float(_)
        | AstExpr::Char(_)
        | AstExpr::Bool(_)
        | AstExpr::Void => false,
    }
}

fn expr_uses_env_builtin(expr: &AstExpr) -> bool {
    match expr {
        AstExpr::Call { callee, args, .. } => {
            is_env_builtin_call(callee) || args.iter().any(expr_uses_env_builtin)
        }
        AstExpr::StructLiteral { fields, .. } => {
            fields.iter().any(|(_, value)| expr_uses_env_builtin(value))
        }
        AstExpr::Match { value, arms } => {
            expr_uses_env_builtin(value) || arms.iter().any(|arm| expr_uses_env_builtin(&arm.value))
        }
        AstExpr::If {
            condition,
            then_branch,
            else_branch,
        } => {
            expr_uses_env_builtin(condition)
                || expr_uses_env_builtin(then_branch)
                || expr_uses_env_builtin(else_branch)
        }
        AstExpr::Panic { message }
        | AstExpr::Question { expr: message }
        | AstExpr::Unary { expr: message, .. } => expr_uses_env_builtin(message),
        AstExpr::MutArg { .. } => false,
        AstExpr::Cast { expr, .. } => expr_uses_env_builtin(expr),
        AstExpr::Binary { left, right, .. } => {
            expr_uses_env_builtin(left) || expr_uses_env_builtin(right)
        }
        AstExpr::Name(_)
        | AstExpr::String(_)
        | AstExpr::Int(_)
        | AstExpr::Float(_)
        | AstExpr::Char(_)
        | AstExpr::Bool(_)
        | AstExpr::Void => false,
    }
}

fn expr_uses_process_builtin(expr: &AstExpr) -> bool {
    match expr {
        AstExpr::Call { callee, args, .. } => {
            is_process_builtin_call(callee) || args.iter().any(expr_uses_process_builtin)
        }
        AstExpr::StructLiteral { fields, .. } => fields
            .iter()
            .any(|(_, value)| expr_uses_process_builtin(value)),
        AstExpr::Match { value, arms } => {
            expr_uses_process_builtin(value)
                || arms.iter().any(|arm| expr_uses_process_builtin(&arm.value))
        }
        AstExpr::If {
            condition,
            then_branch,
            else_branch,
        } => {
            expr_uses_process_builtin(condition)
                || expr_uses_process_builtin(then_branch)
                || expr_uses_process_builtin(else_branch)
        }
        AstExpr::Panic { message }
        | AstExpr::Question { expr: message }
        | AstExpr::Unary { expr: message, .. } => expr_uses_process_builtin(message),
        AstExpr::MutArg { .. } => false,
        AstExpr::Cast { expr, .. } => expr_uses_process_builtin(expr),
        AstExpr::Binary { left, right, .. } => {
            expr_uses_process_builtin(left) || expr_uses_process_builtin(right)
        }
        AstExpr::Name(_)
        | AstExpr::String(_)
        | AstExpr::Int(_)
        | AstExpr::Float(_)
        | AstExpr::Char(_)
        | AstExpr::Bool(_)
        | AstExpr::Void => false,
    }
}

fn expr_uses_hash_builtin(expr: &AstExpr) -> bool {
    match expr {
        AstExpr::Call { callee, args, .. } => {
            is_hash_builtin_call(callee) || args.iter().any(expr_uses_hash_builtin)
        }
        AstExpr::StructLiteral { fields, .. } => fields
            .iter()
            .any(|(_, value)| expr_uses_hash_builtin(value)),
        AstExpr::Match { value, arms } => {
            expr_uses_hash_builtin(value)
                || arms.iter().any(|arm| expr_uses_hash_builtin(&arm.value))
        }
        AstExpr::If {
            condition,
            then_branch,
            else_branch,
        } => {
            expr_uses_hash_builtin(condition)
                || expr_uses_hash_builtin(then_branch)
                || expr_uses_hash_builtin(else_branch)
        }
        AstExpr::Panic { message }
        | AstExpr::Question { expr: message }
        | AstExpr::Unary { expr: message, .. } => expr_uses_hash_builtin(message),
        AstExpr::MutArg { .. } => false,
        AstExpr::Cast { expr, .. } => expr_uses_hash_builtin(expr),
        AstExpr::Binary { left, right, .. } => {
            expr_uses_hash_builtin(left) || expr_uses_hash_builtin(right)
        }
        AstExpr::Name(_)
        | AstExpr::String(_)
        | AstExpr::Int(_)
        | AstExpr::Float(_)
        | AstExpr::Char(_)
        | AstExpr::Bool(_)
        | AstExpr::Void => false,
    }
}

fn expr_uses_json_builtin(expr: &AstExpr) -> bool {
    match expr {
        AstExpr::Call { callee, args, .. } => {
            is_json_builtin_call(callee) || args.iter().any(expr_uses_json_builtin)
        }
        AstExpr::StructLiteral { fields, .. } => fields
            .iter()
            .any(|(_, value)| expr_uses_json_builtin(value)),
        AstExpr::Match { value, arms } => {
            expr_uses_json_builtin(value)
                || arms.iter().any(|arm| expr_uses_json_builtin(&arm.value))
        }
        AstExpr::If {
            condition,
            then_branch,
            else_branch,
        } => {
            expr_uses_json_builtin(condition)
                || expr_uses_json_builtin(then_branch)
                || expr_uses_json_builtin(else_branch)
        }
        AstExpr::Panic { message }
        | AstExpr::Question { expr: message }
        | AstExpr::Unary { expr: message, .. } => expr_uses_json_builtin(message),
        AstExpr::MutArg { .. } => false,
        AstExpr::Cast { expr, .. } => expr_uses_json_builtin(expr),
        AstExpr::Binary { left, right, .. } => {
            expr_uses_json_builtin(left) || expr_uses_json_builtin(right)
        }
        AstExpr::Name(_)
        | AstExpr::String(_)
        | AstExpr::Int(_)
        | AstExpr::Float(_)
        | AstExpr::Char(_)
        | AstExpr::Bool(_)
        | AstExpr::Void => false,
    }
}

fn expr_uses_regex_builtin(expr: &AstExpr) -> bool {
    match expr {
        AstExpr::Call { callee, args, .. } => {
            is_regex_builtin_call(callee) || args.iter().any(expr_uses_regex_builtin)
        }
        AstExpr::StructLiteral { fields, .. } => fields
            .iter()
            .any(|(_, value)| expr_uses_regex_builtin(value)),
        AstExpr::Match { value, arms } => {
            expr_uses_regex_builtin(value)
                || arms.iter().any(|arm| expr_uses_regex_builtin(&arm.value))
        }
        AstExpr::If {
            condition,
            then_branch,
            else_branch,
        } => {
            expr_uses_regex_builtin(condition)
                || expr_uses_regex_builtin(then_branch)
                || expr_uses_regex_builtin(else_branch)
        }
        AstExpr::Panic { message }
        | AstExpr::Question { expr: message }
        | AstExpr::Unary { expr: message, .. } => expr_uses_regex_builtin(message),
        AstExpr::MutArg { .. } => false,
        AstExpr::Cast { expr, .. } => expr_uses_regex_builtin(expr),
        AstExpr::Binary { left, right, .. } => {
            expr_uses_regex_builtin(left) || expr_uses_regex_builtin(right)
        }
        AstExpr::Name(_)
        | AstExpr::String(_)
        | AstExpr::Int(_)
        | AstExpr::Float(_)
        | AstExpr::Char(_)
        | AstExpr::Bool(_)
        | AstExpr::Void => false,
    }
}

fn expr_uses_num_builtin(expr: &AstExpr) -> bool {
    match expr {
        AstExpr::Call { callee, args, .. } => {
            (callee.len() == 2
                && callee[0] == "num"
                && matches!(
                    callee[1].as_str(),
                    "parse_i64"
                        | "parse_u64"
                        | "parse_f64"
                        | "to_string"
                        | "checked_add"
                        | "checked_sub"
                        | "checked_mul"
                        | "wrapping_add"
                        | "wrapping_sub"
                        | "wrapping_mul"
                ))
                || args.iter().any(expr_uses_num_builtin)
        }
        AstExpr::StructLiteral { fields, .. } => {
            fields.iter().any(|(_, value)| expr_uses_num_builtin(value))
        }
        AstExpr::Match { value, arms } => {
            expr_uses_num_builtin(value) || arms.iter().any(|arm| expr_uses_num_builtin(&arm.value))
        }
        AstExpr::If {
            condition,
            then_branch,
            else_branch,
        } => {
            expr_uses_num_builtin(condition)
                || expr_uses_num_builtin(then_branch)
                || expr_uses_num_builtin(else_branch)
        }
        AstExpr::Panic { message }
        | AstExpr::Question { expr: message }
        | AstExpr::Unary { expr: message, .. } => expr_uses_num_builtin(message),
        AstExpr::MutArg { .. } => false,
        AstExpr::Cast { expr, .. } => expr_uses_num_builtin(expr),
        AstExpr::Binary { left, right, .. } => {
            expr_uses_num_builtin(left) || expr_uses_num_builtin(right)
        }
        AstExpr::Name(_)
        | AstExpr::String(_)
        | AstExpr::Int(_)
        | AstExpr::Float(_)
        | AstExpr::Char(_)
        | AstExpr::Bool(_)
        | AstExpr::Void => false,
    }
}

fn expr_uses_time_builtin(expr: &AstExpr) -> bool {
    match expr {
        AstExpr::Call { callee, args, .. } => {
            (callee.len() == 2
                && callee[0] == "time"
                && matches!(
                    callee[1].as_str(),
                    "duration_millis"
                        | "duration_seconds"
                        | "duration_as_millis"
                        | "format_duration"
                        | "sleep"
                ))
                || args.iter().any(expr_uses_time_builtin)
        }
        AstExpr::StructLiteral { fields, .. } => fields
            .iter()
            .any(|(_, value)| expr_uses_time_builtin(value)),
        AstExpr::Match { value, arms } => {
            expr_uses_time_builtin(value)
                || arms.iter().any(|arm| expr_uses_time_builtin(&arm.value))
        }
        AstExpr::If {
            condition,
            then_branch,
            else_branch,
        } => {
            expr_uses_time_builtin(condition)
                || expr_uses_time_builtin(then_branch)
                || expr_uses_time_builtin(else_branch)
        }
        AstExpr::Panic { message }
        | AstExpr::Question { expr: message }
        | AstExpr::Unary { expr: message, .. } => expr_uses_time_builtin(message),
        AstExpr::MutArg { .. } => false,
        AstExpr::Cast { expr, .. } => expr_uses_time_builtin(expr),
        AstExpr::Binary { left, right, .. } => {
            expr_uses_time_builtin(left) || expr_uses_time_builtin(right)
        }
        AstExpr::Name(_)
        | AstExpr::String(_)
        | AstExpr::Int(_)
        | AstExpr::Float(_)
        | AstExpr::Char(_)
        | AstExpr::Bool(_)
        | AstExpr::Void => false,
    }
}

fn expr_uses_array_builtin(expr: &AstExpr) -> bool {
    match expr {
        AstExpr::Call { callee, args, .. } => {
            callee == &["Array", "new"]
                || (callee.len() == 2
                    && !is_known_std_value_module(&callee[0])
                    && matches!(callee[1].as_str(), "len" | "get" | "push" | "set"))
                || args.iter().any(expr_uses_array_builtin)
        }
        AstExpr::StructLiteral { fields, .. } => fields
            .iter()
            .any(|(_, value)| expr_uses_array_builtin(value)),
        AstExpr::Match { value, arms } => {
            expr_uses_array_builtin(value)
                || arms.iter().any(|arm| expr_uses_array_builtin(&arm.value))
        }
        AstExpr::If {
            condition,
            then_branch,
            else_branch,
        } => {
            expr_uses_array_builtin(condition)
                || expr_uses_array_builtin(then_branch)
                || expr_uses_array_builtin(else_branch)
        }
        AstExpr::Panic { message }
        | AstExpr::Question { expr: message }
        | AstExpr::Unary { expr: message, .. } => expr_uses_array_builtin(message),
        AstExpr::MutArg { .. } => false,
        AstExpr::Cast { expr, .. } => expr_uses_array_builtin(expr),
        AstExpr::Binary { left, right, .. } => {
            expr_uses_array_builtin(left) || expr_uses_array_builtin(right)
        }
        AstExpr::Name(_)
        | AstExpr::String(_)
        | AstExpr::Int(_)
        | AstExpr::Float(_)
        | AstExpr::Char(_)
        | AstExpr::Bool(_)
        | AstExpr::Void => false,
    }
}

fn stmt_uses_result_prelude_variant(stmt: &Stmt) -> bool {
    stmt_uses_core_prelude_variant(stmt, "Result")
}

fn stmt_uses_option_prelude_variant(stmt: &Stmt) -> bool {
    stmt_uses_core_prelude_variant(stmt, "Option")
}

fn stmt_uses_core_prelude_variant(stmt: &Stmt, enum_name: &str) -> bool {
    match stmt {
        Stmt::Let { value, .. } | Stmt::Assign { value, .. } => {
            expr_uses_core_prelude_variant(value, enum_name)
        }
        Stmt::LetElse {
            pattern,
            value,
            else_body,
            ..
        } => {
            pattern_uses_core_prelude_variant(pattern, enum_name)
                || expr_uses_core_prelude_variant(value, enum_name)
                || else_body
                    .iter()
                    .any(|stmt| stmt_uses_core_prelude_variant(stmt, enum_name))
        }
        Stmt::IfLet {
            pattern,
            value,
            body,
            else_body,
            ..
        } => {
            pattern_uses_core_prelude_variant(pattern, enum_name)
                || expr_uses_core_prelude_variant(value, enum_name)
                || body
                    .iter()
                    .any(|stmt| stmt_uses_core_prelude_variant(stmt, enum_name))
                || else_body.as_ref().is_some_and(|else_body| {
                    else_body
                        .iter()
                        .any(|stmt| stmt_uses_core_prelude_variant(stmt, enum_name))
                })
        }
        Stmt::Return { value, .. } => value
            .as_ref()
            .is_some_and(|value| expr_uses_core_prelude_variant(value, enum_name)),
        Stmt::Expr { expr, .. } => expr_uses_core_prelude_variant(expr, enum_name),
        Stmt::Match { value, arms, .. } => {
            expr_uses_core_prelude_variant(value, enum_name)
                || arms.iter().any(|arm| {
                    pattern_uses_core_prelude_variant(&arm.pattern, enum_name)
                        || arm
                            .body
                            .iter()
                            .any(|stmt| stmt_uses_core_prelude_variant(stmt, enum_name))
                })
        }
        Stmt::For { variant, .. } => match variant {
            ForVariant::Infinite { body } => body
                .iter()
                .any(|stmt| stmt_uses_core_prelude_variant(stmt, enum_name)),
            ForVariant::While { condition, body } => {
                expr_uses_core_prelude_variant(condition, enum_name)
                    || body
                        .iter()
                        .any(|stmt| stmt_uses_core_prelude_variant(stmt, enum_name))
            }
            ForVariant::Iterate { iterable, body, .. } => {
                expr_uses_core_prelude_variant(iterable, enum_name)
                    || body
                        .iter()
                        .any(|stmt| stmt_uses_core_prelude_variant(stmt, enum_name))
            }
        },
        Stmt::Defer { stmt, .. } => stmt_uses_core_prelude_variant(stmt, enum_name),
        Stmt::Unsafe { body, .. } => body
            .iter()
            .any(|stmt| stmt_uses_core_prelude_variant(stmt, enum_name)),
        Stmt::Postfix { .. } | Stmt::Break { .. } | Stmt::Continue { .. } => false,
    }
}

fn is_known_std_value_module(name: &str) -> bool {
    matches!(
        name,
        "io" | "fs" | "env" | "process" | "string" | "path" | "math" | "collections" | "Array"
    )
}

fn expr_uses_result_prelude_variant(expr: &AstExpr) -> bool {
    expr_uses_core_prelude_variant(expr, "Result")
}

fn expr_uses_option_prelude_variant(expr: &AstExpr) -> bool {
    expr_uses_core_prelude_variant(expr, "Option")
}

fn expr_uses_core_prelude_variant(expr: &AstExpr, enum_name: &str) -> bool {
    match expr {
        AstExpr::Call { callee, args, .. } => {
            pattern_uses_core_prelude_variant(callee, enum_name)
                || args
                    .iter()
                    .any(|arg| expr_uses_core_prelude_variant(arg, enum_name))
        }
        AstExpr::Name(path) => pattern_uses_core_prelude_variant(path, enum_name),
        AstExpr::StructLiteral { fields, .. } => fields
            .iter()
            .any(|(_, value)| expr_uses_core_prelude_variant(value, enum_name)),
        AstExpr::Match { value, arms } => {
            expr_uses_core_prelude_variant(value, enum_name)
                || arms.iter().any(|arm| {
                    pattern_uses_core_prelude_variant(&arm.pattern, enum_name)
                        || expr_uses_core_prelude_variant(&arm.value, enum_name)
                })
        }
        AstExpr::If {
            condition,
            then_branch,
            else_branch,
        } => {
            expr_uses_core_prelude_variant(condition, enum_name)
                || expr_uses_core_prelude_variant(then_branch, enum_name)
                || expr_uses_core_prelude_variant(else_branch, enum_name)
        }
        AstExpr::Panic { message }
        | AstExpr::Question { expr: message }
        | AstExpr::Unary { expr: message, .. } => {
            expr_uses_core_prelude_variant(message, enum_name)
        }
        AstExpr::Cast { expr, .. } => expr_uses_core_prelude_variant(expr, enum_name),
        AstExpr::Binary { left, right, .. } => {
            expr_uses_core_prelude_variant(left, enum_name)
                || expr_uses_core_prelude_variant(right, enum_name)
        }
        AstExpr::MutArg { .. }
        | AstExpr::String(_)
        | AstExpr::Int(_)
        | AstExpr::Float(_)
        | AstExpr::Char(_)
        | AstExpr::Bool(_)
        | AstExpr::Void => false,
    }
}

fn pattern_uses_core_prelude_variant(path: &[String], enum_name: &str) -> bool {
    matches!(
        path,
        [variant]
            if core_prelude_variant(variant)
                .is_some_and(|(resolved_enum, _)| resolved_enum == enum_name)
    )
}
