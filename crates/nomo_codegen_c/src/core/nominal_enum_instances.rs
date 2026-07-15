use super::*;

pub(super) fn collect_enum_instances(program: &Program) -> Vec<(String, Vec<ValueType>)> {
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for enum_type in &program.enums {
        if enum_type.type_params.is_empty() {
            push_enum_instance(&mut seen, &mut out, &enum_type.name, &[]);
        }
    }
    for function in &program.functions {
        collect_type_enum(&function.return_type, &mut seen, &mut out);
        for param in &function.params {
            collect_type_enum(&param.value_type, &mut seen, &mut out);
        }
        for statement in &function.body {
            collect_stmt_enum(statement, &mut seen, &mut out);
        }
    }
    for element_type in collect_array_element_types(program) {
        push_enum_instance(&mut seen, &mut out, "Option", &[element_type]);
    }
    out
}

fn collect_stmt_enum(
    statement: &Statement,
    seen: &mut BTreeSet<String>,
    out: &mut Vec<(String, Vec<ValueType>)>,
) {
    match statement {
        Statement::Let {
            value_type,
            initializer,
            ..
        } => {
            collect_type_enum(value_type, seen, out);
            collect_expr_enum(initializer, seen, out);
        }
        Statement::LetIf {
            value_type,
            condition,
            body,
            else_body,
            ..
        } => {
            collect_type_enum(value_type, seen, out);
            collect_expr_enum(condition, seen, out);
            for stmt in body {
                collect_stmt_enum(stmt, seen, out);
            }
            for stmt in else_body {
                collect_stmt_enum(stmt, seen, out);
            }
        }
        Statement::LetMatch {
            value_type,
            value,
            enum_name,
            enum_args,
            arms,
            ..
        } => {
            push_enum_instance(seen, out, enum_name, enum_args);
            collect_type_enum(value_type, seen, out);
            for arg in enum_args {
                collect_type_enum(arg, seen, out);
            }
            collect_expr_enum(value, seen, out);
            for arm in arms {
                for stmt in &arm.body {
                    collect_stmt_enum(stmt, seen, out);
                }
            }
        }
        Statement::QuestionLet {
            value_type,
            result_type,
            return_type,
            result_expr,
            ..
        } => {
            collect_type_enum(value_type, seen, out);
            collect_type_enum(result_type, seen, out);
            collect_type_enum(return_type, seen, out);
            collect_expr_enum(result_expr, seen, out);
        }
        Statement::QuestionReturn {
            ok_type,
            result_type,
            return_type,
            result_expr,
            ..
        } => {
            collect_type_enum(ok_type, seen, out);
            collect_type_enum(result_type, seen, out);
            collect_type_enum(return_type, seen, out);
            collect_expr_enum(result_expr, seen, out);
        }
        Statement::LetElse {
            value_type,
            value,
            enum_name,
            enum_args,
            else_body,
            ..
        } => {
            push_enum_instance(seen, out, enum_name, enum_args);
            collect_type_enum(value_type, seen, out);
            for arg in enum_args {
                collect_type_enum(arg, seen, out);
            }
            collect_expr_enum(value, seen, out);
            for stmt in else_body {
                collect_stmt_enum(stmt, seen, out);
            }
        }
        Statement::IfLet {
            value_type,
            value,
            enum_name,
            enum_args,
            body,
            else_body,
            ..
        } => {
            push_enum_instance(seen, out, enum_name, enum_args);
            if let Some(value_type) = value_type {
                collect_type_enum(value_type, seen, out);
            }
            for arg in enum_args {
                collect_type_enum(arg, seen, out);
            }
            collect_expr_enum(value, seen, out);
            for stmt in body {
                collect_stmt_enum(stmt, seen, out);
            }
            if let Some(else_body) = else_body {
                for stmt in else_body {
                    collect_stmt_enum(stmt, seen, out);
                }
            }
        }
        Statement::If {
            condition,
            body,
            else_body,
        } => {
            collect_expr_enum(condition, seen, out);
            for stmt in body {
                collect_stmt_enum(stmt, seen, out);
            }
            for stmt in else_body {
                collect_stmt_enum(stmt, seen, out);
            }
        }
        Statement::Assign { value, .. }
        | Statement::AssignField { value, .. }
        | Statement::Println(value)
        | Statement::Print(value)
        | Statement::Eprintln(value)
        | Statement::Eprint(value)
        | Statement::Panic(value)
        | Statement::Expr(value)
        | Statement::Return(Some(value)) => {
            collect_expr_enum(value, seen, out);
        }
        Statement::Loop { kind, body } => {
            match kind {
                LoopKind::Infinite => {}
                LoopKind::While(condition) => collect_expr_enum(condition, seen, out),
                LoopKind::Iterate {
                    element_type,
                    iterable,
                    ..
                } => {
                    collect_type_enum(element_type, seen, out);
                    collect_expr_enum(iterable, seen, out);
                }
            }
            for stmt in body {
                collect_stmt_enum(stmt, seen, out);
            }
        }
        Statement::Match {
            value,
            enum_name,
            enum_args,
            arms,
        } => {
            push_enum_instance(seen, out, enum_name, enum_args);
            collect_expr_enum(value, seen, out);
            for arm in arms {
                for stmt in &arm.body {
                    collect_stmt_enum(stmt, seen, out);
                }
            }
        }
        Statement::Defer { call } => collect_deferred_enum(call, seen, out),
        Statement::Break | Statement::Continue => {}
        Statement::Return(None) => {}
    }
}

fn collect_deferred_enum(
    call: &DeferredCall,
    seen: &mut BTreeSet<String>,
    out: &mut Vec<(String, Vec<ValueType>)>,
) {
    match call {
        DeferredCall::Expr(expr)
        | DeferredCall::Println(expr)
        | DeferredCall::Print(expr)
        | DeferredCall::Eprintln(expr)
        | DeferredCall::Eprint(expr) => {
            collect_expr_enum(expr, seen, out);
        }
    }
}

fn collect_type_enum(
    value_type: &ValueType,
    seen: &mut BTreeSet<String>,
    out: &mut Vec<(String, Vec<ValueType>)>,
) {
    match value_type {
        ValueType::Enum(name, args) => {
            push_enum_instance(seen, out, name, args);
            for arg in args {
                collect_type_enum(arg, seen, out);
            }
        }
        ValueType::Array(element) => collect_type_enum(element, seen, out),
        ValueType::Never => {}
        _ => {}
    }
}

fn collect_http_call_enums(
    name: &str,
    seen: &mut BTreeSet<String>,
    out: &mut Vec<(String, Vec<ValueType>)>,
) {
    let http_error = ValueType::Struct("HttpError".to_string(), Vec::new());
    match name {
        BUILTIN_HTTP_GET_EXPR | BUILTIN_HTTP_POST_EXPR => {
            push_enum_instance(
                seen,
                out,
                "Result",
                &[
                    ValueType::Struct("HttpResponse".to_string(), Vec::new()),
                    http_error,
                ],
            );
        }
        BUILTIN_HTTP_LISTEN_EXPR => {
            push_enum_instance(
                seen,
                out,
                "Result",
                &[
                    ValueType::Struct("HttpServer".to_string(), Vec::new()),
                    http_error,
                ],
            );
        }
        BUILTIN_HTTP_ACCEPT_EXPR => {
            push_enum_instance(
                seen,
                out,
                "Result",
                &[
                    ValueType::Struct("HttpExchange".to_string(), Vec::new()),
                    http_error,
                ],
            );
        }
        BUILTIN_HTTP_RESPOND_STRING_EXPR => {
            push_enum_instance(seen, out, "Result", &[ValueType::Void, http_error]);
        }
        _ => {}
    }
}

fn collect_expr_enum(
    expr: &ValueExpr,
    seen: &mut BTreeSet<String>,
    out: &mut Vec<(String, Vec<ValueType>)>,
) {
    match expr {
        ValueExpr::Binary { left, right, .. }
        | ValueExpr::StringCompare { left, right, .. }
        | ValueExpr::StringConcat { left, right }
        | ValueExpr::StringContains {
            value: left,
            needle: right,
        }
        | ValueExpr::StringStartsWith {
            value: left,
            prefix: right,
        }
        | ValueExpr::StringEndsWith {
            value: left,
            suffix: right,
        }
        | ValueExpr::StringSplit {
            value: left,
            separator: right,
        }
        | ValueExpr::PathJoin { left, right }
        | ValueExpr::MathBinary { left, right, .. }
        | ValueExpr::CollectionsStringMapContains {
            map: left,
            key: right,
        }
        | ValueExpr::CollectionsStringMapRemove {
            map: left,
            key: right,
        }
        | ValueExpr::CollectionsStringSetContains {
            set: left,
            value: right,
        }
        | ValueExpr::CollectionsStringSetInsert {
            set: left,
            value: right,
        }
        | ValueExpr::CollectionsStringSetRemove {
            set: left,
            value: right,
        } => {
            collect_expr_enum(left, seen, out);
            collect_expr_enum(right, seen, out);
        }
        ValueExpr::CollectionsStringMapGet { map, key } => {
            push_enum_instance(seen, out, "Option", &[ValueType::String]);
            collect_expr_enum(map, seen, out);
            collect_expr_enum(key, seen, out);
        }
        ValueExpr::CollectionsStringMapSet { map, key, value } => {
            collect_expr_enum(map, seen, out);
            collect_expr_enum(key, seen, out);
            collect_expr_enum(value, seen, out);
        }
        ValueExpr::NumBinary {
            function,
            left,
            right,
            value_type,
            ..
        } => {
            if function == &NumBinaryFunction::Checked {
                push_enum_instance(seen, out, "Option", std::slice::from_ref(value_type));
                collect_type_enum(value_type, seen, out);
            }
            collect_expr_enum(left, seen, out);
            collect_expr_enum(right, seen, out);
        }
        ValueExpr::Call { name, args } => {
            collect_http_call_enums(name, seen, out);
            for arg in args {
                collect_expr_enum(arg, seen, out);
            }
        }
        ValueExpr::StringLen { value }
        | ValueExpr::StringIsEmpty { value }
        | ValueExpr::StringTrim { value }
        | ValueExpr::StringToLower { value }
        | ValueExpr::StringToUpper { value }
        | ValueExpr::CharIsDigit { value }
        | ValueExpr::CharIsAlpha { value }
        | ValueExpr::CharIsWhitespace { value }
        | ValueExpr::CharToString { value }
        | ValueExpr::PathBasename { path: value }
        | ValueExpr::PathDirname { path: value }
        | ValueExpr::PathExtension { path: value }
        | ValueExpr::PathNormalize { path: value }
        | ValueExpr::PathIsAbsolute { path: value }
        | ValueExpr::MathUnary { value, .. }
        | ValueExpr::TimeDurationMillis { millis: value }
        | ValueExpr::TimeDurationSeconds { seconds: value }
        | ValueExpr::TimeDurationAsMillis { duration: value }
        | ValueExpr::TimeFormatDuration { duration: value }
        | ValueExpr::TimeSleep { duration: value }
        | ValueExpr::TimeSleepMillis { duration: value }
        | ValueExpr::LogEnabled { level: value }
        | ValueExpr::HashString { value }
        | ValueExpr::HashBytes { value }
        | ValueExpr::HashFinish { state: value }
        | ValueExpr::CryptoSha256 { value }
        | ValueExpr::CryptoSha512 { value }
        | ValueExpr::CryptoRandomBytes { count: value }
        | ValueExpr::JsonParse { value }
        | ValueExpr::JsonStringify { value }
        | ValueExpr::CollectionsStringMapLen { map: value }
        | ValueExpr::CollectionsStringSetLen { set: value }
        | ValueExpr::ProcessExit { code: value }
        | ValueExpr::Unary { expr: value, .. } => collect_expr_enum(value, seen, out),
        ValueExpr::ProcessSpawn { command } => {
            push_enum_instance(
                seen,
                out,
                "Result",
                &[
                    ValueType::I32,
                    ValueType::Struct("ProcessError".to_string(), Vec::new()),
                ],
            );
            collect_expr_enum(command, seen, out);
        }
        ValueExpr::RegexCompile { pattern } => {
            push_enum_instance(
                seen,
                out,
                "Result",
                &[
                    ValueType::Struct("Regex".to_string(), Vec::new()),
                    ValueType::Struct("RegexError".to_string(), Vec::new()),
                ],
            );
            collect_expr_enum(pattern, seen, out);
        }
        ValueExpr::RegexIsMatch { regex, value } => {
            collect_expr_enum(regex, seen, out);
            collect_expr_enum(value, seen, out);
        }
        ValueExpr::RegexCaptures { regex, value } => {
            push_enum_instance(
                seen,
                out,
                "Option",
                &[ValueType::Array(Box::new(ValueType::String))],
            );
            collect_expr_enum(regex, seen, out);
            collect_expr_enum(value, seen, out);
        }
        ValueExpr::HashNew
        | ValueExpr::CollectionsStringMapNew
        | ValueExpr::CollectionsStringSetNew => {}
        ValueExpr::HashWriteString { state, value }
        | ValueExpr::HashWriteBytes { state, value } => {
            collect_expr_enum(state, seen, out);
            collect_expr_enum(value, seen, out);
        }
        ValueExpr::ProcessStatus { command } => {
            push_enum_instance(
                seen,
                out,
                "Result",
                &[
                    ValueType::I32,
                    ValueType::Struct("ProcessError".to_string(), Vec::new()),
                ],
            );
            collect_expr_enum(command, seen, out);
        }
        ValueExpr::ProcessExec { command } => {
            push_enum_instance(
                seen,
                out,
                "Result",
                &[
                    ValueType::String,
                    ValueType::Struct("ProcessError".to_string(), Vec::new()),
                ],
            );
            collect_expr_enum(command, seen, out);
        }
        ValueExpr::ProcessOutput { command } => {
            push_enum_instance(
                seen,
                out,
                "Result",
                &[
                    ValueType::Struct("ProcessOutput".to_string(), Vec::new()),
                    ValueType::Struct("ProcessError".to_string(), Vec::new()),
                ],
            );
            collect_expr_enum(command, seen, out);
        }
        ValueExpr::FsReadToString { path } => {
            push_enum_instance(
                seen,
                out,
                "Result",
                &[
                    ValueType::String,
                    ValueType::Struct("FsError".to_string(), Vec::new()),
                ],
            );
            collect_expr_enum(path, seen, out);
        }
        ValueExpr::FsReadBytes { path } => {
            push_enum_instance(
                seen,
                out,
                "Result",
                &[
                    ValueType::Array(Box::new(ValueType::U32)),
                    ValueType::Struct("FsError".to_string(), Vec::new()),
                ],
            );
            collect_expr_enum(path, seen, out);
        }
        ValueExpr::FsWriteString { path, content } => {
            push_enum_instance(
                seen,
                out,
                "Result",
                &[
                    ValueType::Void,
                    ValueType::Struct("FsError".to_string(), Vec::new()),
                ],
            );
            collect_expr_enum(path, seen, out);
            collect_expr_enum(content, seen, out);
        }
        ValueExpr::FsWriteBytes { path, bytes } => {
            push_enum_instance(
                seen,
                out,
                "Result",
                &[
                    ValueType::Void,
                    ValueType::Struct("FsError".to_string(), Vec::new()),
                ],
            );
            collect_expr_enum(path, seen, out);
            collect_expr_enum(bytes, seen, out);
        }
        ValueExpr::FsExists { path } => collect_expr_enum(path, seen, out),
        ValueExpr::FsMetadata { path } => {
            push_enum_instance(
                seen,
                out,
                "Result",
                &[
                    ValueType::Struct("FileMetadata".to_string(), Vec::new()),
                    ValueType::Struct("FsError".to_string(), Vec::new()),
                ],
            );
            collect_expr_enum(path, seen, out);
        }
        ValueExpr::FsCreateDir { path } | ValueExpr::FsRemoveDir { path } => {
            push_enum_instance(
                seen,
                out,
                "Result",
                &[
                    ValueType::Void,
                    ValueType::Struct("FsError".to_string(), Vec::new()),
                ],
            );
            collect_expr_enum(path, seen, out);
        }
        ValueExpr::FsReadDir { path } => {
            push_enum_instance(
                seen,
                out,
                "Result",
                &[
                    ValueType::Array(Box::new(ValueType::String)),
                    ValueType::Struct("FsError".to_string(), Vec::new()),
                ],
            );
            collect_expr_enum(path, seen, out);
        }
        ValueExpr::FsOpen { path } => {
            push_enum_instance(
                seen,
                out,
                "Result",
                &[
                    ValueType::Struct("File".to_string(), Vec::new()),
                    ValueType::Struct("FsError".to_string(), Vec::new()),
                ],
            );
            collect_expr_enum(path, seen, out);
        }
        ValueExpr::FileReadToString { file } => {
            push_enum_instance(
                seen,
                out,
                "Result",
                &[
                    ValueType::String,
                    ValueType::Struct("FsError".to_string(), Vec::new()),
                ],
            );
            collect_expr_enum(file, seen, out);
        }
        ValueExpr::FileWriteString { file, content } => {
            push_enum_instance(
                seen,
                out,
                "Result",
                &[
                    ValueType::Void,
                    ValueType::Struct("FsError".to_string(), Vec::new()),
                ],
            );
            collect_expr_enum(file, seen, out);
            collect_expr_enum(content, seen, out);
        }
        ValueExpr::FileClose { file } => collect_expr_enum(file, seen, out),
        ValueExpr::NetConnect { host, port } => {
            push_enum_instance(
                seen,
                out,
                "Result",
                &[
                    ValueType::Struct("TcpStream".to_string(), Vec::new()),
                    ValueType::Struct("NetError".to_string(), Vec::new()),
                ],
            );
            collect_expr_enum(host, seen, out);
            collect_expr_enum(port, seen, out);
        }
        ValueExpr::NetListen { host, port } => {
            push_enum_instance(
                seen,
                out,
                "Result",
                &[
                    ValueType::Struct("TcpListener".to_string(), Vec::new()),
                    ValueType::Struct("NetError".to_string(), Vec::new()),
                ],
            );
            collect_expr_enum(host, seen, out);
            collect_expr_enum(port, seen, out);
        }
        ValueExpr::NetUdpBind { host, port } => {
            push_enum_instance(
                seen,
                out,
                "Result",
                &[
                    ValueType::Struct("UdpSocket".to_string(), Vec::new()),
                    ValueType::Struct("NetError".to_string(), Vec::new()),
                ],
            );
            collect_expr_enum(host, seen, out);
            collect_expr_enum(port, seen, out);
        }
        ValueExpr::TcpListenerAccept { listener } => {
            push_enum_instance(
                seen,
                out,
                "Result",
                &[
                    ValueType::Struct("TcpStream".to_string(), Vec::new()),
                    ValueType::Struct("NetError".to_string(), Vec::new()),
                ],
            );
            collect_expr_enum(listener, seen, out);
        }
        ValueExpr::TcpListenerClose { listener } => collect_expr_enum(listener, seen, out),
        ValueExpr::TcpStreamReadToString { stream } => {
            push_enum_instance(
                seen,
                out,
                "Result",
                &[
                    ValueType::String,
                    ValueType::Struct("NetError".to_string(), Vec::new()),
                ],
            );
            collect_expr_enum(stream, seen, out);
        }
        ValueExpr::TcpStreamWriteString { stream, content } => {
            push_enum_instance(
                seen,
                out,
                "Result",
                &[
                    ValueType::Void,
                    ValueType::Struct("NetError".to_string(), Vec::new()),
                ],
            );
            collect_expr_enum(stream, seen, out);
            collect_expr_enum(content, seen, out);
        }
        ValueExpr::TcpStreamClose { stream } => collect_expr_enum(stream, seen, out),
        ValueExpr::UdpSocketRecvFromString { socket, max_bytes } => {
            push_enum_instance(
                seen,
                out,
                "Result",
                &[
                    ValueType::Struct("UdpDatagram".to_string(), Vec::new()),
                    ValueType::Struct("NetError".to_string(), Vec::new()),
                ],
            );
            collect_expr_enum(socket, seen, out);
            collect_expr_enum(max_bytes, seen, out);
        }
        ValueExpr::UdpSocketSendToString {
            socket,
            content,
            host,
            port,
        } => {
            push_enum_instance(
                seen,
                out,
                "Result",
                &[
                    ValueType::Void,
                    ValueType::Struct("NetError".to_string(), Vec::new()),
                ],
            );
            collect_expr_enum(socket, seen, out);
            collect_expr_enum(content, seen, out);
            collect_expr_enum(host, seen, out);
            collect_expr_enum(port, seen, out);
        }
        ValueExpr::UdpSocketClose { socket } => collect_expr_enum(socket, seen, out),
        ValueExpr::ResultMapErr {
            result,
            ok_type,
            source_err_type,
            target_err_type,
            ..
        } => {
            push_enum_instance(
                seen,
                out,
                "Result",
                &[ok_type.clone(), source_err_type.clone()],
            );
            push_enum_instance(
                seen,
                out,
                "Result",
                &[ok_type.clone(), target_err_type.clone()],
            );
            collect_type_enum(ok_type, seen, out);
            collect_type_enum(source_err_type, seen, out);
            collect_type_enum(target_err_type, seen, out);
            collect_expr_enum(result, seen, out);
        }
        ValueExpr::ResultIsOk {
            result,
            ok_type,
            err_type,
        }
        | ValueExpr::ResultIsErr {
            result,
            ok_type,
            err_type,
        } => {
            push_enum_instance(seen, out, "Result", &[ok_type.clone(), err_type.clone()]);
            collect_type_enum(ok_type, seen, out);
            collect_type_enum(err_type, seen, out);
            collect_expr_enum(result, seen, out);
        }
        ValueExpr::ResultUnwrapOr {
            result,
            default,
            ok_type,
            err_type,
        } => {
            push_enum_instance(seen, out, "Result", &[ok_type.clone(), err_type.clone()]);
            collect_type_enum(ok_type, seen, out);
            collect_type_enum(err_type, seen, out);
            collect_expr_enum(result, seen, out);
            collect_expr_enum(default, seen, out);
        }
        ValueExpr::ResultMap {
            result,
            source_ok_type,
            target_ok_type,
            err_type,
            ..
        }
        | ValueExpr::ResultAndThen {
            result,
            source_ok_type,
            target_ok_type,
            err_type,
            ..
        } => {
            push_enum_instance(
                seen,
                out,
                "Result",
                &[source_ok_type.clone(), err_type.clone()],
            );
            push_enum_instance(
                seen,
                out,
                "Result",
                &[target_ok_type.clone(), err_type.clone()],
            );
            collect_type_enum(source_ok_type, seen, out);
            collect_type_enum(target_ok_type, seen, out);
            collect_type_enum(err_type, seen, out);
            collect_expr_enum(result, seen, out);
        }
        ValueExpr::OptionIsSome {
            option,
            payload_type,
        }
        | ValueExpr::OptionIsNone {
            option,
            payload_type,
        } => {
            push_enum_instance(seen, out, "Option", &[payload_type.clone()]);
            collect_type_enum(payload_type, seen, out);
            collect_expr_enum(option, seen, out);
        }
        ValueExpr::OptionUnwrapOr {
            option,
            default,
            payload_type,
        } => {
            push_enum_instance(seen, out, "Option", &[payload_type.clone()]);
            collect_type_enum(payload_type, seen, out);
            collect_expr_enum(option, seen, out);
            collect_expr_enum(default, seen, out);
        }
        ValueExpr::OptionMap {
            option,
            source_type,
            target_type,
            ..
        }
        | ValueExpr::OptionAndThen {
            option,
            source_type,
            target_type,
            ..
        } => {
            push_enum_instance(seen, out, "Option", &[source_type.clone()]);
            push_enum_instance(seen, out, "Option", &[target_type.clone()]);
            collect_type_enum(source_type, seen, out);
            collect_type_enum(target_type, seen, out);
            collect_expr_enum(option, seen, out);
        }
        ValueExpr::EnvGet { name } => {
            push_enum_instance(seen, out, "Option", &[ValueType::String]);
            collect_expr_enum(name, seen, out);
        }
        ValueExpr::EnvSet { name, value } => {
            collect_expr_enum(name, seen, out);
            collect_expr_enum(value, seen, out);
        }
        ValueExpr::EnvHomeDir => {
            push_enum_instance(seen, out, "Option", &[ValueType::String]);
        }
        ValueExpr::EnvCwd | ValueExpr::EnvTempDir => {}
        ValueExpr::EnvArgs => {}
        ValueExpr::IoReadLine => {
            push_enum_instance(
                seen,
                out,
                "Result",
                &[
                    ValueType::String,
                    ValueType::Struct("IoError".to_string(), Vec::new()),
                ],
            );
        }
        ValueExpr::NumParseI64 { value } => {
            push_enum_instance(
                seen,
                out,
                "Result",
                &[
                    ValueType::Int,
                    ValueType::Struct("NumError".to_string(), Vec::new()),
                ],
            );
            collect_expr_enum(value, seen, out);
        }
        ValueExpr::NumParseU64 { value } => {
            push_enum_instance(
                seen,
                out,
                "Result",
                &[
                    ValueType::U64,
                    ValueType::Struct("NumError".to_string(), Vec::new()),
                ],
            );
            collect_expr_enum(value, seen, out);
        }
        ValueExpr::NumParseF64 { value } => {
            push_enum_instance(
                seen,
                out,
                "Result",
                &[
                    ValueType::Float,
                    ValueType::Struct("NumError".to_string(), Vec::new()),
                ],
            );
            collect_expr_enum(value, seen, out);
        }
        ValueExpr::NumToString { value, .. } => collect_expr_enum(value, seen, out),
        ValueExpr::ArrayNew { .. } => {}
        ValueExpr::ArrayLen { array } => collect_expr_enum(array, seen, out),
        ValueExpr::ArrayIter {
            array,
            element_type,
        } => {
            collect_type_enum(element_type, seen, out);
            collect_expr_enum(array, seen, out);
        }
        ValueExpr::ArrayGet {
            array,
            index,
            element_type,
        } => {
            push_enum_instance(seen, out, "Option", &[element_type.clone()]);
            collect_expr_enum(array, seen, out);
            collect_expr_enum(index, seen, out);
        }
        ValueExpr::ArrayPop { element_type, .. } => {
            push_enum_instance(seen, out, "Option", &[element_type.clone()]);
        }
        ValueExpr::ArrayRemove {
            index,
            element_type,
            ..
        } => {
            push_enum_instance(seen, out, "Option", &[element_type.clone()]);
            collect_expr_enum(index, seen, out);
        }
        ValueExpr::ArrayClear { .. } => {}
        ValueExpr::ArrayPush { value, .. } => collect_expr_enum(value, seen, out),
        ValueExpr::ArraySet { index, value, .. } => {
            collect_expr_enum(index, seen, out);
            collect_expr_enum(value, seen, out);
        }
        ValueExpr::ArrayInsert { index, value, .. } => {
            collect_expr_enum(index, seen, out);
            collect_expr_enum(value, seen, out);
        }
        ValueExpr::Cast { expr, .. } => collect_expr_enum(expr, seen, out),
        ValueExpr::StructLiteral { fields, .. } => {
            for (_, value) in fields {
                collect_expr_enum(value, seen, out);
            }
        }
        ValueExpr::EnumVariant {
            enum_name,
            enum_args,
            payload,
            ..
        } => {
            push_enum_instance(seen, out, enum_name, enum_args);
            if let Some(payload) = payload {
                collect_expr_enum(payload, seen, out);
            }
        }
        ValueExpr::EnumPayload { value, .. } => collect_expr_enum(value, seen, out),
        ValueExpr::EnumPayloadFieldAccess { value, .. } => collect_expr_enum(value, seen, out),
        ValueExpr::If {
            condition,
            then_branch,
            else_branch,
        } => {
            collect_expr_enum(condition, seen, out);
            collect_expr_enum(then_branch, seen, out);
            collect_expr_enum(else_branch, seen, out);
        }
        ValueExpr::Panic { message, .. } => collect_expr_enum(message, seen, out),
        ValueExpr::Match { value, arms } => {
            collect_expr_enum(value, seen, out);
            for arm in arms {
                push_enum_instance(seen, out, &arm.enum_name, &arm.enum_args);
                collect_expr_enum(&arm.value, seen, out);
            }
        }
        ValueExpr::StringLiteral(_)
        | ValueExpr::IntLiteral(_)
        | ValueExpr::FloatLiteral(_)
        | ValueExpr::CharLiteral(_)
        | ValueExpr::BoolLiteral(_)
        | ValueExpr::VoidLiteral
        | ValueExpr::Variable(_)
        | ValueExpr::FunctionRef(_)
        | ValueExpr::MutBorrow(_)
        | ValueExpr::OsPlatform
        | ValueExpr::OsArch
        | ValueExpr::OsPathSeparator
        | ValueExpr::OsLineEnding
        | ValueExpr::TimeNowMillis
        | ValueExpr::TimeMonotonicMillis
        | ValueExpr::FieldAccess { .. } => {}
    }
}
