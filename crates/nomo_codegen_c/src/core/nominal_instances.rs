use super::*;

pub(super) fn collect_struct_instances(program: &Program) -> Vec<(String, Vec<ValueType>)> {
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for struct_type in &program.structs {
        if struct_type.type_params.is_empty() {
            push_struct_instance(&mut seen, &mut out, &struct_type.name, &[]);
        }
    }
    for function in &program.functions {
        collect_type_struct(&function.return_type, &mut seen, &mut out);
        for param in &function.params {
            collect_type_struct(&param.value_type, &mut seen, &mut out);
        }
        for statement in &function.body {
            collect_stmt_struct(statement, &mut seen, &mut out);
        }
    }
    out
}

fn collect_stmt_struct(
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
            collect_type_struct(value_type, seen, out);
            collect_expr_struct(initializer, seen, out);
        }
        Statement::LetIf {
            value_type,
            condition,
            body,
            else_body,
            ..
        } => {
            collect_type_struct(value_type, seen, out);
            collect_expr_struct(condition, seen, out);
            for stmt in body {
                collect_stmt_struct(stmt, seen, out);
            }
            for stmt in else_body {
                collect_stmt_struct(stmt, seen, out);
            }
        }
        Statement::LetMatch {
            value_type,
            value,
            enum_args,
            arms,
            ..
        } => {
            collect_type_struct(value_type, seen, out);
            for arg in enum_args {
                collect_type_struct(arg, seen, out);
            }
            collect_expr_struct(value, seen, out);
            for arm in arms {
                for stmt in &arm.body {
                    collect_stmt_struct(stmt, seen, out);
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
            collect_type_struct(value_type, seen, out);
            collect_type_struct(result_type, seen, out);
            collect_type_struct(return_type, seen, out);
            collect_expr_struct(result_expr, seen, out);
        }
        Statement::QuestionReturn {
            ok_type,
            result_type,
            return_type,
            result_expr,
            ..
        } => {
            collect_type_struct(ok_type, seen, out);
            collect_type_struct(result_type, seen, out);
            collect_type_struct(return_type, seen, out);
            collect_expr_struct(result_expr, seen, out);
        }
        Statement::LetElse {
            value_type,
            value,
            enum_args,
            else_body,
            ..
        } => {
            collect_type_struct(value_type, seen, out);
            for arg in enum_args {
                collect_type_struct(arg, seen, out);
            }
            collect_expr_struct(value, seen, out);
            for stmt in else_body {
                collect_stmt_struct(stmt, seen, out);
            }
        }
        Statement::IfLet {
            value_type,
            value,
            enum_args,
            body,
            else_body,
            ..
        } => {
            if let Some(value_type) = value_type {
                collect_type_struct(value_type, seen, out);
            }
            for arg in enum_args {
                collect_type_struct(arg, seen, out);
            }
            collect_expr_struct(value, seen, out);
            for stmt in body {
                collect_stmt_struct(stmt, seen, out);
            }
            if let Some(else_body) = else_body {
                for stmt in else_body {
                    collect_stmt_struct(stmt, seen, out);
                }
            }
        }
        Statement::If {
            condition,
            body,
            else_body,
        } => {
            collect_expr_struct(condition, seen, out);
            for stmt in body {
                collect_stmt_struct(stmt, seen, out);
            }
            for stmt in else_body {
                collect_stmt_struct(stmt, seen, out);
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
            collect_expr_struct(value, seen, out);
        }
        Statement::Loop { kind, body } => {
            match kind {
                LoopKind::Infinite => {}
                LoopKind::While(condition) => collect_expr_struct(condition, seen, out),
                LoopKind::CStyle {
                    value_type,
                    initializer,
                    condition,
                    update,
                    ..
                } => {
                    collect_type_struct(value_type, seen, out);
                    collect_expr_struct(initializer, seen, out);
                    collect_expr_struct(condition, seen, out);
                    collect_expr_struct(update, seen, out);
                }
                LoopKind::Iterate {
                    element_type,
                    iterable,
                    ..
                } => {
                    collect_type_struct(element_type, seen, out);
                    collect_expr_struct(iterable, seen, out);
                }
            }
            for stmt in body {
                collect_stmt_struct(stmt, seen, out);
            }
        }
        Statement::Match { value, arms, .. } => {
            collect_expr_struct(value, seen, out);
            for arm in arms {
                for stmt in &arm.body {
                    collect_stmt_struct(stmt, seen, out);
                }
            }
        }
        Statement::Defer { call } => collect_deferred_struct(call, seen, out),
        Statement::Break | Statement::Continue => {}
        Statement::Return(None) => {}
    }
}

fn collect_deferred_struct(
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
            collect_expr_struct(expr, seen, out);
        }
    }
}

fn collect_type_struct(
    value_type: &ValueType,
    seen: &mut BTreeSet<String>,
    out: &mut Vec<(String, Vec<ValueType>)>,
) {
    match value_type {
        ValueType::Struct(name, args) => {
            push_struct_instance(seen, out, name, args);
            for arg in args {
                collect_type_struct(arg, seen, out);
            }
        }
        ValueType::Enum(_, args) => {
            for arg in args {
                collect_type_struct(arg, seen, out);
            }
        }
        ValueType::Array(element) => collect_type_struct(element, seen, out),
        _ => {}
    }
}

fn collect_http_call_structs(
    name: &str,
    seen: &mut BTreeSet<String>,
    out: &mut Vec<(String, Vec<ValueType>)>,
) {
    match name {
        BUILTIN_HTTP_GET_EXPR | BUILTIN_HTTP_POST_EXPR => {
            push_struct_instance(seen, out, "HttpError", &[]);
            push_struct_instance(seen, out, "HttpResponse", &[]);
        }
        BUILTIN_HTTP_LISTEN_EXPR => {
            push_struct_instance(seen, out, "HttpError", &[]);
            push_struct_instance(seen, out, "HttpServer", &[]);
        }
        BUILTIN_HTTP_ACCEPT_EXPR => {
            push_struct_instance(seen, out, "HttpError", &[]);
            push_struct_instance(seen, out, "HttpExchange", &[]);
        }
        BUILTIN_HTTP_RESPOND_STRING_EXPR => {
            push_struct_instance(seen, out, "HttpError", &[]);
        }
        _ => {}
    }
}

fn collect_expr_struct(
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
        | ValueExpr::NumBinary { left, right, .. }
        | ValueExpr::MathBinary { left, right, .. }
        | ValueExpr::CollectionsStringMapGet {
            map: left,
            key: right,
        }
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
            collect_expr_struct(left, seen, out);
            collect_expr_struct(right, seen, out);
        }
        ValueExpr::Call { name, args } => {
            collect_http_call_structs(name, seen, out);
            for arg in args {
                collect_expr_struct(arg, seen, out);
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
        | ValueExpr::Unary { expr: value, .. } => collect_expr_struct(value, seen, out),
        ValueExpr::RegexCompile { pattern } => {
            push_struct_instance(seen, out, "Regex", &[]);
            push_struct_instance(seen, out, "RegexError", &[]);
            collect_expr_struct(pattern, seen, out);
        }
        ValueExpr::RegexIsMatch { regex, value } | ValueExpr::RegexCaptures { regex, value } => {
            push_struct_instance(seen, out, "Regex", &[]);
            collect_expr_struct(regex, seen, out);
            collect_expr_struct(value, seen, out);
        }
        ValueExpr::HashNew => {
            push_struct_instance(seen, out, "HashState", &[]);
        }
        ValueExpr::HashWriteString { state, value } => {
            push_struct_instance(seen, out, "HashState", &[]);
            collect_expr_struct(state, seen, out);
            collect_expr_struct(value, seen, out);
        }
        ValueExpr::HashWriteBytes { state, value } => {
            push_struct_instance(seen, out, "HashState", &[]);
            collect_expr_struct(state, seen, out);
            collect_expr_struct(value, seen, out);
        }
        ValueExpr::CollectionsStringMapNew => {
            push_struct_instance(seen, out, "StringMap", &[]);
        }
        ValueExpr::CollectionsStringMapSet { map, key, value } => {
            push_struct_instance(seen, out, "StringMap", &[]);
            collect_expr_struct(map, seen, out);
            collect_expr_struct(key, seen, out);
            collect_expr_struct(value, seen, out);
        }
        ValueExpr::CollectionsStringSetNew => {
            push_struct_instance(seen, out, "StringSet", &[]);
        }
        ValueExpr::ProcessSpawn { command }
        | ValueExpr::ProcessStatus { command }
        | ValueExpr::ProcessExec { command } => {
            push_struct_instance(seen, out, "ProcessError", &[]);
            collect_expr_struct(command, seen, out);
        }
        ValueExpr::ProcessOutput { command } => {
            push_struct_instance(seen, out, "ProcessError", &[]);
            push_struct_instance(seen, out, "ProcessOutput", &[]);
            collect_expr_struct(command, seen, out);
        }
        ValueExpr::NetConnect { host, port } => {
            push_struct_instance(seen, out, "NetError", &[]);
            push_struct_instance(seen, out, "TcpStream", &[]);
            collect_expr_struct(host, seen, out);
            collect_expr_struct(port, seen, out);
        }
        ValueExpr::NetListen { host, port } => {
            push_struct_instance(seen, out, "NetError", &[]);
            push_struct_instance(seen, out, "TcpListener", &[]);
            collect_expr_struct(host, seen, out);
            collect_expr_struct(port, seen, out);
        }
        ValueExpr::NetUdpBind { host, port } => {
            push_struct_instance(seen, out, "NetError", &[]);
            push_struct_instance(seen, out, "UdpSocket", &[]);
            collect_expr_struct(host, seen, out);
            collect_expr_struct(port, seen, out);
        }
        ValueExpr::TcpListenerAccept { listener } => {
            push_struct_instance(seen, out, "NetError", &[]);
            push_struct_instance(seen, out, "TcpStream", &[]);
            collect_expr_struct(listener, seen, out);
        }
        ValueExpr::TcpListenerClose { listener } => collect_expr_struct(listener, seen, out),
        ValueExpr::TcpStreamWriteString { stream, content } => {
            push_struct_instance(seen, out, "NetError", &[]);
            collect_expr_struct(stream, seen, out);
            collect_expr_struct(content, seen, out);
        }
        ValueExpr::TcpStreamReadToString { stream } => {
            push_struct_instance(seen, out, "NetError", &[]);
            collect_expr_struct(stream, seen, out);
        }
        ValueExpr::TcpStreamClose { stream } => collect_expr_struct(stream, seen, out),
        ValueExpr::UdpSocketRecvFromString { socket, max_bytes } => {
            push_struct_instance(seen, out, "NetError", &[]);
            push_struct_instance(seen, out, "UdpDatagram", &[]);
            collect_expr_struct(socket, seen, out);
            collect_expr_struct(max_bytes, seen, out);
        }
        ValueExpr::UdpSocketSendToString {
            socket,
            content,
            host,
            port,
        } => {
            push_struct_instance(seen, out, "NetError", &[]);
            collect_expr_struct(socket, seen, out);
            collect_expr_struct(content, seen, out);
            collect_expr_struct(host, seen, out);
            collect_expr_struct(port, seen, out);
        }
        ValueExpr::UdpSocketClose { socket } => collect_expr_struct(socket, seen, out),
        ValueExpr::FsReadToString { path } => {
            push_struct_instance(seen, out, "FsError", &[]);
            collect_expr_struct(path, seen, out);
        }
        ValueExpr::FsReadBytes { path } => {
            push_struct_instance(seen, out, "FsError", &[]);
            collect_expr_struct(path, seen, out);
        }
        ValueExpr::FsWriteString { path, content } => {
            push_struct_instance(seen, out, "FsError", &[]);
            collect_expr_struct(path, seen, out);
            collect_expr_struct(content, seen, out);
        }
        ValueExpr::FsWriteBytes { path, bytes } => {
            push_struct_instance(seen, out, "FsError", &[]);
            collect_expr_struct(path, seen, out);
            collect_expr_struct(bytes, seen, out);
        }
        ValueExpr::FsExists { path } => collect_expr_struct(path, seen, out),
        ValueExpr::FsMetadata { path } => {
            push_struct_instance(seen, out, "FsError", &[]);
            push_struct_instance(seen, out, "FileMetadata", &[]);
            collect_expr_struct(path, seen, out);
        }
        ValueExpr::FsCreateDir { path }
        | ValueExpr::FsRemoveDir { path }
        | ValueExpr::FsReadDir { path } => {
            push_struct_instance(seen, out, "FsError", &[]);
            collect_expr_struct(path, seen, out);
        }
        ValueExpr::FsOpen { path } => {
            push_struct_instance(seen, out, "FsError", &[]);
            push_struct_instance(seen, out, "File", &[]);
            collect_expr_struct(path, seen, out);
        }
        ValueExpr::FileReadToString { file } => {
            push_struct_instance(seen, out, "FsError", &[]);
            collect_expr_struct(file, seen, out);
        }
        ValueExpr::FileWriteString { file, content } => {
            push_struct_instance(seen, out, "FsError", &[]);
            collect_expr_struct(file, seen, out);
            collect_expr_struct(content, seen, out);
        }
        ValueExpr::IoReadLine => {
            push_struct_instance(seen, out, "IoError", &[]);
        }
        ValueExpr::NumParseI64 { value }
        | ValueExpr::NumParseU64 { value }
        | ValueExpr::NumParseF64 { value } => {
            push_struct_instance(seen, out, "NumError", &[]);
            collect_expr_struct(value, seen, out);
        }
        ValueExpr::NumToString { value, .. } => collect_expr_struct(value, seen, out),
        ValueExpr::FileClose { file } => collect_expr_struct(file, seen, out),
        ValueExpr::ResultMapErr {
            result,
            ok_type,
            source_err_type,
            target_err_type,
            ..
        } => {
            collect_type_struct(ok_type, seen, out);
            collect_type_struct(source_err_type, seen, out);
            collect_type_struct(target_err_type, seen, out);
            collect_expr_struct(result, seen, out);
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
            collect_type_struct(ok_type, seen, out);
            collect_type_struct(err_type, seen, out);
            collect_expr_struct(result, seen, out);
        }
        ValueExpr::ResultUnwrapOr {
            result,
            default,
            ok_type,
            err_type,
        } => {
            collect_type_struct(ok_type, seen, out);
            collect_type_struct(err_type, seen, out);
            collect_expr_struct(result, seen, out);
            collect_expr_struct(default, seen, out);
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
            collect_type_struct(source_ok_type, seen, out);
            collect_type_struct(target_ok_type, seen, out);
            collect_type_struct(err_type, seen, out);
            collect_expr_struct(result, seen, out);
        }
        ValueExpr::OptionIsSome {
            option,
            payload_type,
        }
        | ValueExpr::OptionIsNone {
            option,
            payload_type,
        } => {
            collect_type_struct(payload_type, seen, out);
            collect_expr_struct(option, seen, out);
        }
        ValueExpr::OptionUnwrapOr {
            option,
            default,
            payload_type,
        } => {
            collect_type_struct(payload_type, seen, out);
            collect_expr_struct(option, seen, out);
            collect_expr_struct(default, seen, out);
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
            collect_type_struct(source_type, seen, out);
            collect_type_struct(target_type, seen, out);
            collect_expr_struct(option, seen, out);
        }
        ValueExpr::EnvGet { name } => collect_expr_struct(name, seen, out),
        ValueExpr::EnvSet { name, value } => {
            collect_expr_struct(name, seen, out);
            collect_expr_struct(value, seen, out);
        }
        ValueExpr::EnvCwd | ValueExpr::EnvHomeDir | ValueExpr::EnvTempDir => {}
        ValueExpr::EnvArgs => {}
        ValueExpr::ArrayNew { element_type } => collect_type_struct(element_type, seen, out),
        ValueExpr::ArrayLen { array } => collect_expr_struct(array, seen, out),
        ValueExpr::ArrayIter {
            array,
            element_type,
        } => {
            collect_type_struct(element_type, seen, out);
            collect_expr_struct(array, seen, out);
        }
        ValueExpr::ArrayGet {
            array,
            index,
            element_type,
        } => {
            collect_type_struct(element_type, seen, out);
            collect_expr_struct(array, seen, out);
            collect_expr_struct(index, seen, out);
        }
        ValueExpr::ArrayPop { element_type, .. } | ValueExpr::ArrayClear { element_type, .. } => {
            collect_type_struct(element_type, seen, out)
        }
        ValueExpr::ArrayRemove {
            index,
            element_type,
            ..
        } => {
            collect_type_struct(element_type, seen, out);
            collect_expr_struct(index, seen, out);
        }
        ValueExpr::ArrayPush {
            value,
            element_type,
            ..
        } => {
            collect_type_struct(element_type, seen, out);
            collect_expr_struct(value, seen, out);
        }
        ValueExpr::ArraySet {
            index,
            value,
            element_type,
            ..
        } => {
            collect_type_struct(element_type, seen, out);
            collect_expr_struct(index, seen, out);
            collect_expr_struct(value, seen, out);
        }
        ValueExpr::ArrayInsert {
            index,
            value,
            element_type,
            ..
        } => {
            collect_type_struct(element_type, seen, out);
            collect_expr_struct(index, seen, out);
            collect_expr_struct(value, seen, out);
        }
        ValueExpr::Cast { expr, target_type } => {
            collect_type_struct(target_type, seen, out);
            collect_expr_struct(expr, seen, out);
        }
        ValueExpr::StructLiteral {
            type_name,
            struct_args,
            fields,
        } => {
            push_struct_instance(seen, out, type_name, struct_args);
            for arg in struct_args {
                collect_type_struct(arg, seen, out);
            }
            for (_, value) in fields {
                collect_expr_struct(value, seen, out);
            }
        }
        ValueExpr::EnumVariant {
            enum_args, payload, ..
        } => {
            for arg in enum_args {
                collect_type_struct(arg, seen, out);
            }
            if let Some(payload) = payload {
                collect_expr_struct(payload, seen, out);
            }
        }
        ValueExpr::EnumPayload { value, .. } => collect_expr_struct(value, seen, out),
        ValueExpr::EnumPayloadFieldAccess { value, .. } => collect_expr_struct(value, seen, out),
        ValueExpr::If {
            condition,
            then_branch,
            else_branch,
        } => {
            collect_expr_struct(condition, seen, out);
            collect_expr_struct(then_branch, seen, out);
            collect_expr_struct(else_branch, seen, out);
        }
        ValueExpr::Panic {
            message,
            fallback_type,
        } => {
            collect_type_struct(fallback_type, seen, out);
            collect_expr_struct(message, seen, out);
        }
        ValueExpr::Match { value, arms } => {
            collect_expr_struct(value, seen, out);
            for arm in arms {
                for arg in &arm.enum_args {
                    collect_type_struct(arg, seen, out);
                }
                collect_expr_struct(&arm.value, seen, out);
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
