use super::*;

pub(super) fn collect_array_element_types(program: &Program) -> Vec<ValueType> {
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for function in &program.functions {
        collect_type_array_elements(&function.return_type, &mut seen, &mut out);
        for param in &function.params {
            collect_type_array_elements(&param.value_type, &mut seen, &mut out);
        }
        for statement in &function.body {
            collect_statement_array_elements(statement, &mut seen, &mut out);
        }
    }
    out
}

pub(super) fn collect_type_array_elements(
    value_type: &ValueType,
    seen: &mut BTreeSet<String>,
    out: &mut Vec<ValueType>,
) {
    match value_type {
        ValueType::Array(element) => {
            push_array_element_type(seen, out, element);
            collect_type_array_elements(element, seen, out);
        }
        ValueType::Enum(_, args) => {
            for arg in args {
                collect_type_array_elements(arg, seen, out);
            }
        }
        _ => {}
    }
}

pub(super) fn push_array_element_type(
    seen: &mut BTreeSet<String>,
    out: &mut Vec<ValueType>,
    element_type: &ValueType,
) {
    if is_supported_array_element(element_type) {
        let key = c_type_name_part(element_type);
        if seen.insert(key) {
            out.push(element_type.clone());
        }
    }
}

pub(super) fn collect_statement_array_elements(
    statement: &Statement,
    seen: &mut BTreeSet<String>,
    out: &mut Vec<ValueType>,
) {
    match statement {
        Statement::Let {
            value_type,
            initializer,
            ..
        } => {
            collect_type_array_elements(value_type, seen, out);
            collect_expr_array_elements(initializer, seen, out);
        }
        Statement::LetIf {
            value_type,
            condition,
            body,
            else_body,
            ..
        } => {
            collect_type_array_elements(value_type, seen, out);
            collect_expr_array_elements(condition, seen, out);
            for stmt in body {
                collect_statement_array_elements(stmt, seen, out);
            }
            for stmt in else_body {
                collect_statement_array_elements(stmt, seen, out);
            }
        }
        Statement::LetMatch {
            value_type,
            value,
            enum_args,
            arms,
            ..
        } => {
            collect_type_array_elements(value_type, seen, out);
            for arg in enum_args {
                collect_type_array_elements(arg, seen, out);
            }
            collect_expr_array_elements(value, seen, out);
            for arm in arms {
                for stmt in &arm.body {
                    collect_statement_array_elements(stmt, seen, out);
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
            collect_type_array_elements(value_type, seen, out);
            collect_type_array_elements(result_type, seen, out);
            collect_type_array_elements(return_type, seen, out);
            collect_expr_array_elements(result_expr, seen, out);
        }
        Statement::QuestionReturn {
            ok_type,
            result_type,
            return_type,
            result_expr,
            ..
        } => {
            collect_type_array_elements(ok_type, seen, out);
            collect_type_array_elements(result_type, seen, out);
            collect_type_array_elements(return_type, seen, out);
            collect_expr_array_elements(result_expr, seen, out);
        }
        Statement::LetElse {
            value_type,
            value,
            enum_args,
            else_body,
            ..
        } => {
            collect_type_array_elements(value_type, seen, out);
            for arg in enum_args {
                collect_type_array_elements(arg, seen, out);
            }
            collect_expr_array_elements(value, seen, out);
            for stmt in else_body {
                collect_statement_array_elements(stmt, seen, out);
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
                collect_type_array_elements(value_type, seen, out);
            }
            for arg in enum_args {
                collect_type_array_elements(arg, seen, out);
            }
            collect_expr_array_elements(value, seen, out);
            for stmt in body {
                collect_statement_array_elements(stmt, seen, out);
            }
            if let Some(else_body) = else_body {
                for stmt in else_body {
                    collect_statement_array_elements(stmt, seen, out);
                }
            }
        }
        Statement::If {
            condition,
            body,
            else_body,
        } => {
            collect_expr_array_elements(condition, seen, out);
            for stmt in body {
                collect_statement_array_elements(stmt, seen, out);
            }
            for stmt in else_body {
                collect_statement_array_elements(stmt, seen, out);
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
        | Statement::Return(Some(value)) => collect_expr_array_elements(value, seen, out),
        Statement::Loop { kind, body } => match kind {
            LoopKind::Infinite => {
                for stmt in body {
                    collect_statement_array_elements(stmt, seen, out);
                }
            }
            LoopKind::While(condition) => {
                collect_expr_array_elements(condition, seen, out);
                for stmt in body {
                    collect_statement_array_elements(stmt, seen, out);
                }
            }
            LoopKind::Iterate {
                element_type,
                iterable,
                ..
            } => {
                collect_type_array_elements(element_type, seen, out);
                collect_expr_array_elements(iterable, seen, out);
                for stmt in body {
                    collect_statement_array_elements(stmt, seen, out);
                }
            }
        },
        Statement::Match { value, arms, .. } => {
            collect_expr_array_elements(value, seen, out);
            for arm in arms {
                for stmt in &arm.body {
                    collect_statement_array_elements(stmt, seen, out);
                }
            }
        }
        Statement::Defer { call } => collect_deferred_array_elements(call, seen, out),
        Statement::Break | Statement::Continue => {}
        Statement::Return(None) => {}
    }
}

pub(super) fn collect_deferred_array_elements(
    call: &DeferredCall,
    seen: &mut BTreeSet<String>,
    out: &mut Vec<ValueType>,
) {
    match call {
        DeferredCall::Expr(expr)
        | DeferredCall::Println(expr)
        | DeferredCall::Print(expr)
        | DeferredCall::Eprintln(expr)
        | DeferredCall::Eprint(expr) => {
            collect_expr_array_elements(expr, seen, out);
        }
    }
}

pub(super) fn collect_expr_array_elements(
    expr: &ValueExpr,
    seen: &mut BTreeSet<String>,
    out: &mut Vec<ValueType>,
) {
    match expr {
        ValueExpr::EnvArgs => push_array_element_type(seen, out, &ValueType::String),
        ValueExpr::ArrayIter {
            array,
            element_type,
        } => {
            push_array_element_type(seen, out, element_type);
            collect_expr_array_elements(array, seen, out);
        }
        ValueExpr::ArrayNew { element_type }
        | ValueExpr::ArrayGet { element_type, .. }
        | ValueExpr::ArrayPop { element_type, .. }
        | ValueExpr::ArrayRemove { element_type, .. }
        | ValueExpr::ArrayPush { element_type, .. }
        | ValueExpr::ArraySet { element_type, .. }
        | ValueExpr::ArrayInsert { element_type, .. }
        | ValueExpr::ArrayClear { element_type, .. } => {
            push_array_element_type(seen, out, element_type);
        }
        ValueExpr::ArrayLen { array } => collect_expr_array_elements(array, seen, out),
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
        }
        | ValueExpr::RegexIsMatch {
            regex: left,
            value: right,
        }
        | ValueExpr::NetConnect {
            host: left,
            port: right,
        }
        | ValueExpr::NetListen {
            host: left,
            port: right,
        }
        | ValueExpr::NetUdpBind {
            host: left,
            port: right,
        }
        | ValueExpr::UdpSocketRecvFromString {
            socket: left,
            max_bytes: right,
        }
        | ValueExpr::TcpStreamWriteString {
            stream: left,
            content: right,
        } => {
            collect_expr_array_elements(left, seen, out);
            collect_expr_array_elements(right, seen, out);
        }
        ValueExpr::UdpSocketSendToString {
            socket,
            content,
            host,
            port,
        } => {
            collect_expr_array_elements(socket, seen, out);
            collect_expr_array_elements(content, seen, out);
            collect_expr_array_elements(host, seen, out);
            collect_expr_array_elements(port, seen, out);
        }
        ValueExpr::RegexCaptures { regex, value } => {
            push_array_element_type(seen, out, &ValueType::String);
            collect_expr_array_elements(regex, seen, out);
            collect_expr_array_elements(value, seen, out);
        }
        ValueExpr::CollectionsStringMapNew | ValueExpr::CollectionsStringSetNew => {
            push_array_element_type(seen, out, &ValueType::String);
        }
        ValueExpr::CryptoRandomBytes { count } => {
            push_array_element_type(seen, out, &ValueType::U32);
            collect_expr_array_elements(count, seen, out);
        }
        ValueExpr::HashBytes { value } => {
            push_array_element_type(seen, out, &ValueType::U32);
            collect_expr_array_elements(value, seen, out);
        }
        ValueExpr::FsReadBytes { path } => {
            push_array_element_type(seen, out, &ValueType::U32);
            collect_expr_array_elements(path, seen, out);
        }
        ValueExpr::CollectionsStringMapSet { map, key, value } => {
            push_array_element_type(seen, out, &ValueType::String);
            collect_expr_array_elements(map, seen, out);
            collect_expr_array_elements(key, seen, out);
            collect_expr_array_elements(value, seen, out);
        }
        ValueExpr::StringSplit { value, separator } => {
            push_array_element_type(seen, out, &ValueType::String);
            collect_expr_array_elements(value, seen, out);
            collect_expr_array_elements(separator, seen, out);
        }
        ValueExpr::FsReadToString { path }
        | ValueExpr::FsExists { path }
        | ValueExpr::FsMetadata { path }
        | ValueExpr::FsCreateDir { path }
        | ValueExpr::FsRemoveDir { path }
        | ValueExpr::EnvGet { name: path }
        | ValueExpr::PathBasename { path }
        | ValueExpr::PathDirname { path }
        | ValueExpr::PathExtension { path }
        | ValueExpr::PathNormalize { path }
        | ValueExpr::PathIsAbsolute { path }
        | ValueExpr::MathUnary { value: path, .. }
        | ValueExpr::TimeDurationMillis { millis: path }
        | ValueExpr::TimeDurationSeconds { seconds: path }
        | ValueExpr::TimeDurationAsMillis { duration: path }
        | ValueExpr::TimeFormatDuration { duration: path }
        | ValueExpr::TimeSleep { duration: path }
        | ValueExpr::TimeSleepMillis { duration: path }
        | ValueExpr::LogEnabled { level: path }
        | ValueExpr::HashString { value: path }
        | ValueExpr::HashFinish { state: path }
        | ValueExpr::CryptoSha256 { value: path }
        | ValueExpr::CryptoSha512 { value: path }
        | ValueExpr::JsonParse { value: path }
        | ValueExpr::JsonStringify { value: path }
        | ValueExpr::RegexCompile { pattern: path }
        | ValueExpr::CollectionsStringMapLen { map: path }
        | ValueExpr::CollectionsStringSetLen { set: path }
        | ValueExpr::ProcessExit { code: path }
        | ValueExpr::ProcessSpawn { command: path }
        | ValueExpr::ProcessStatus { command: path }
        | ValueExpr::ProcessExec { command: path }
        | ValueExpr::ProcessOutput { command: path }
        | ValueExpr::NumParseI64 { value: path }
        | ValueExpr::NumParseU64 { value: path }
        | ValueExpr::NumParseF64 { value: path }
        | ValueExpr::NumToString { value: path, .. }
        | ValueExpr::TcpListenerAccept { listener: path }
        | ValueExpr::TcpListenerClose { listener: path }
        | ValueExpr::TcpStreamClose { stream: path }
        | ValueExpr::TcpStreamReadToString { stream: path }
        | ValueExpr::UdpSocketClose { socket: path } => {
            collect_expr_array_elements(path, seen, out);
        }
        ValueExpr::FileReadToString { file } => {
            collect_expr_array_elements(file, seen, out);
        }
        ValueExpr::FsReadDir { path } => {
            push_array_element_type(seen, out, &ValueType::String);
            collect_expr_array_elements(path, seen, out);
        }
        ValueExpr::FsOpen { path } | ValueExpr::FileClose { file: path } => {
            collect_expr_array_elements(path, seen, out);
        }
        ValueExpr::HashNew => {}
        ValueExpr::HashWriteString { state, value } => {
            collect_expr_array_elements(state, seen, out);
            collect_expr_array_elements(value, seen, out);
        }
        ValueExpr::HashWriteBytes { state, value } => {
            push_array_element_type(seen, out, &ValueType::U32);
            collect_expr_array_elements(state, seen, out);
            collect_expr_array_elements(value, seen, out);
        }
        ValueExpr::ResultMapErr {
            result,
            ok_type,
            source_err_type,
            target_err_type,
            ..
        } => {
            collect_type_array_elements(ok_type, seen, out);
            collect_type_array_elements(source_err_type, seen, out);
            collect_type_array_elements(target_err_type, seen, out);
            collect_expr_array_elements(result, seen, out);
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
            collect_type_array_elements(ok_type, seen, out);
            collect_type_array_elements(err_type, seen, out);
            collect_expr_array_elements(result, seen, out);
        }
        ValueExpr::ResultUnwrapOr {
            result,
            default,
            ok_type,
            err_type,
        } => {
            collect_type_array_elements(ok_type, seen, out);
            collect_type_array_elements(err_type, seen, out);
            collect_expr_array_elements(result, seen, out);
            collect_expr_array_elements(default, seen, out);
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
            collect_type_array_elements(source_ok_type, seen, out);
            collect_type_array_elements(target_ok_type, seen, out);
            collect_type_array_elements(err_type, seen, out);
            collect_expr_array_elements(result, seen, out);
        }
        ValueExpr::OptionIsSome {
            option,
            payload_type,
        }
        | ValueExpr::OptionIsNone {
            option,
            payload_type,
        } => {
            collect_type_array_elements(payload_type, seen, out);
            collect_expr_array_elements(option, seen, out);
        }
        ValueExpr::OptionUnwrapOr {
            option,
            default,
            payload_type,
        } => {
            collect_type_array_elements(payload_type, seen, out);
            collect_expr_array_elements(option, seen, out);
            collect_expr_array_elements(default, seen, out);
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
            collect_type_array_elements(source_type, seen, out);
            collect_type_array_elements(target_type, seen, out);
            collect_expr_array_elements(option, seen, out);
        }
        ValueExpr::FsWriteString { path, content } => {
            collect_expr_array_elements(path, seen, out);
            collect_expr_array_elements(content, seen, out);
        }
        ValueExpr::FsWriteBytes { path, bytes } => {
            collect_expr_array_elements(path, seen, out);
            collect_expr_array_elements(bytes, seen, out);
        }
        ValueExpr::FileWriteString { file, content } => {
            collect_expr_array_elements(file, seen, out);
            collect_expr_array_elements(content, seen, out);
        }
        ValueExpr::EnvSet { name, value } => {
            collect_expr_array_elements(name, seen, out);
            collect_expr_array_elements(value, seen, out);
        }
        ValueExpr::Call { args, .. } => {
            for arg in args {
                collect_expr_array_elements(arg, seen, out);
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
        | ValueExpr::Unary { expr: value, .. }
        | ValueExpr::Cast { expr: value, .. }
        | ValueExpr::EnumPayload { value, .. }
        | ValueExpr::EnumPayloadFieldAccess { value, .. } => {
            collect_expr_array_elements(value, seen, out);
        }
        ValueExpr::StructLiteral { fields, .. } => {
            for (_, value) in fields {
                collect_expr_array_elements(value, seen, out);
            }
        }
        ValueExpr::EnumVariant { payload, .. } => {
            if let Some(payload) = payload {
                collect_expr_array_elements(payload, seen, out);
            }
        }
        ValueExpr::If {
            condition,
            then_branch,
            else_branch,
        } => {
            collect_expr_array_elements(condition, seen, out);
            collect_expr_array_elements(then_branch, seen, out);
            collect_expr_array_elements(else_branch, seen, out);
        }
        ValueExpr::Panic { message, .. } => collect_expr_array_elements(message, seen, out),
        ValueExpr::Match { value, arms } => {
            collect_expr_array_elements(value, seen, out);
            for arm in arms {
                collect_expr_array_elements(&arm.value, seen, out);
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
        | ValueExpr::EnvCwd
        | ValueExpr::EnvHomeDir
        | ValueExpr::EnvTempDir
        | ValueExpr::OsPlatform
        | ValueExpr::OsArch
        | ValueExpr::OsPathSeparator
        | ValueExpr::OsLineEnding
        | ValueExpr::TimeNowMillis
        | ValueExpr::TimeMonotonicMillis
        | ValueExpr::IoReadLine
        | ValueExpr::FieldAccess { .. } => {}
    }
}
