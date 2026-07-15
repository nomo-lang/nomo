use super::*;

pub(super) fn deferred_uses_fs_write_string(call: &DeferredCall) -> bool {
    match call {
        DeferredCall::Expr(expr)
        | DeferredCall::Println(expr)
        | DeferredCall::Print(expr)
        | DeferredCall::Eprintln(expr)
        | DeferredCall::Eprint(expr) => expr_uses_fs_write_string(expr),
    }
}

pub(super) fn expr_uses_fs_write_string(expr: &ValueExpr) -> bool {
    match expr {
        ValueExpr::FsWriteString { .. } => true,
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
        }
        | ValueExpr::RegexIsMatch {
            regex: left,
            value: right,
        }
        | ValueExpr::RegexCaptures {
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
        } => expr_uses_fs_write_string(left) || expr_uses_fs_write_string(right),
        ValueExpr::UdpSocketSendToString {
            socket,
            content,
            host,
            port,
        } => {
            expr_uses_fs_write_string(socket)
                || expr_uses_fs_write_string(content)
                || expr_uses_fs_write_string(host)
                || expr_uses_fs_write_string(port)
        }
        ValueExpr::FsReadToString { path }
        | ValueExpr::FsReadBytes { path }
        | ValueExpr::FsExists { path }
        | ValueExpr::FsMetadata { path }
        | ValueExpr::FsCreateDir { path }
        | ValueExpr::FsRemoveDir { path }
        | ValueExpr::FsReadDir { path } => expr_uses_fs_write_string(path),
        ValueExpr::FileReadToString { file }
        | ValueExpr::TcpListenerAccept { listener: file }
        | ValueExpr::TcpListenerClose { listener: file }
        | ValueExpr::TcpStreamClose { stream: file }
        | ValueExpr::TcpStreamReadToString { stream: file }
        | ValueExpr::UdpSocketClose { socket: file } => expr_uses_fs_write_string(file),
        ValueExpr::FileWriteString { file, content } => {
            expr_uses_fs_write_string(file) || expr_uses_fs_write_string(content)
        }
        ValueExpr::FsWriteBytes { path, bytes } => {
            expr_uses_fs_write_string(path) || expr_uses_fs_write_string(bytes)
        }
        ValueExpr::EnvSet { name, value } => {
            expr_uses_fs_write_string(name) || expr_uses_fs_write_string(value)
        }
        ValueExpr::FsOpen { path } | ValueExpr::FileClose { file: path } => {
            expr_uses_fs_write_string(path)
        }
        ValueExpr::ResultMapErr { result, .. }
        | ValueExpr::ResultIsOk { result, .. }
        | ValueExpr::ResultIsErr { result, .. }
        | ValueExpr::ResultMap { result, .. }
        | ValueExpr::ResultAndThen { result, .. }
        | ValueExpr::OptionIsSome { option: result, .. }
        | ValueExpr::OptionIsNone { option: result, .. }
        | ValueExpr::OptionMap { option: result, .. }
        | ValueExpr::OptionAndThen { option: result, .. } => expr_uses_fs_write_string(result),
        ValueExpr::ResultUnwrapOr {
            result, default, ..
        } => expr_uses_fs_write_string(result) || expr_uses_fs_write_string(default),
        ValueExpr::OptionUnwrapOr {
            option, default, ..
        } => expr_uses_fs_write_string(option) || expr_uses_fs_write_string(default),
        ValueExpr::EnvGet { name }
        | ValueExpr::PathBasename { path: name }
        | ValueExpr::PathDirname { path: name }
        | ValueExpr::PathExtension { path: name }
        | ValueExpr::PathNormalize { path: name }
        | ValueExpr::PathIsAbsolute { path: name }
        | ValueExpr::MathUnary { value: name, .. }
        | ValueExpr::TimeDurationMillis { millis: name }
        | ValueExpr::TimeDurationSeconds { seconds: name }
        | ValueExpr::TimeDurationAsMillis { duration: name }
        | ValueExpr::TimeFormatDuration { duration: name }
        | ValueExpr::TimeSleep { duration: name }
        | ValueExpr::TimeSleepMillis { duration: name }
        | ValueExpr::LogEnabled { level: name }
        | ValueExpr::HashString { value: name }
        | ValueExpr::HashBytes { value: name }
        | ValueExpr::HashFinish { state: name }
        | ValueExpr::CryptoSha256 { value: name }
        | ValueExpr::CryptoSha512 { value: name }
        | ValueExpr::CryptoRandomBytes { count: name }
        | ValueExpr::JsonParse { value: name }
        | ValueExpr::JsonStringify { value: name }
        | ValueExpr::RegexCompile { pattern: name }
        | ValueExpr::CollectionsStringMapLen { map: name }
        | ValueExpr::CollectionsStringSetLen { set: name }
        | ValueExpr::ProcessExit { code: name }
        | ValueExpr::ProcessSpawn { command: name }
        | ValueExpr::ProcessStatus { command: name }
        | ValueExpr::ProcessExec { command: name }
        | ValueExpr::ProcessOutput { command: name }
        | ValueExpr::NumParseI64 { value: name }
        | ValueExpr::NumParseU64 { value: name }
        | ValueExpr::NumParseF64 { value: name }
        | ValueExpr::NumToString { value: name, .. } => expr_uses_fs_write_string(name),
        ValueExpr::EnvArgs => false,
        ValueExpr::ArrayNew { .. }
        | ValueExpr::HashNew
        | ValueExpr::CollectionsStringMapNew
        | ValueExpr::CollectionsStringSetNew => false,
        ValueExpr::HashWriteString { state, value } => {
            expr_uses_fs_write_string(state) || expr_uses_fs_write_string(value)
        }
        ValueExpr::HashWriteBytes { state, value } => {
            expr_uses_fs_write_string(state) || expr_uses_fs_write_string(value)
        }
        ValueExpr::CollectionsStringMapSet { map, key, value } => {
            expr_uses_fs_write_string(map)
                || expr_uses_fs_write_string(key)
                || expr_uses_fs_write_string(value)
        }
        ValueExpr::ArrayLen { array } => expr_uses_fs_write_string(array),
        ValueExpr::ArrayIter { array, .. } => expr_uses_fs_write_string(array),
        ValueExpr::ArrayGet { array, index, .. } => {
            expr_uses_fs_write_string(array) || expr_uses_fs_write_string(index)
        }
        ValueExpr::ArrayPop { .. } | ValueExpr::ArrayClear { .. } => false,
        ValueExpr::ArrayRemove { index, .. } => expr_uses_fs_write_string(index),
        ValueExpr::ArrayPush { value, .. } => expr_uses_fs_write_string(value),
        ValueExpr::ArraySet { index, value, .. } => {
            expr_uses_fs_write_string(index) || expr_uses_fs_write_string(value)
        }
        ValueExpr::ArrayInsert { index, value, .. } => {
            expr_uses_fs_write_string(index) || expr_uses_fs_write_string(value)
        }
        ValueExpr::Call { args, .. } => args.iter().any(expr_uses_fs_write_string),
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
        | ValueExpr::EnumPayload { value, .. } => expr_uses_fs_write_string(value),
        ValueExpr::StructLiteral { fields, .. } => fields
            .iter()
            .any(|(_, value)| expr_uses_fs_write_string(value)),
        ValueExpr::EnumVariant { payload, .. } => {
            payload.as_deref().is_some_and(expr_uses_fs_write_string)
        }
        ValueExpr::If {
            condition,
            then_branch,
            else_branch,
        } => {
            expr_uses_fs_write_string(condition)
                || expr_uses_fs_write_string(then_branch)
                || expr_uses_fs_write_string(else_branch)
        }
        ValueExpr::Panic { message, .. } => expr_uses_fs_write_string(message),
        ValueExpr::Match { value, arms } => {
            expr_uses_fs_write_string(value)
                || arms.iter().any(|arm| expr_uses_fs_write_string(&arm.value))
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
        | ValueExpr::FieldAccess { .. } => false,
        ValueExpr::EnumPayloadFieldAccess { value, .. } => expr_uses_fs_write_string(value),
    }
}
