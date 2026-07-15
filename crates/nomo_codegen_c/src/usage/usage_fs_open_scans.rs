use super::*;

pub(super) fn deferred_uses_fs_open(call: &DeferredCall) -> bool {
    match call {
        DeferredCall::Expr(expr)
        | DeferredCall::Println(expr)
        | DeferredCall::Print(expr)
        | DeferredCall::Eprintln(expr)
        | DeferredCall::Eprint(expr) => expr_uses_fs_open(expr),
    }
}

pub(super) fn expr_uses_fs_open(expr: &ValueExpr) -> bool {
    match expr {
        ValueExpr::FsOpen { .. } | ValueExpr::FileClose { .. } => true,
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
        } => expr_uses_fs_open(left) || expr_uses_fs_open(right),
        ValueExpr::UdpSocketSendToString {
            socket,
            content,
            host,
            port,
        } => {
            expr_uses_fs_open(socket)
                || expr_uses_fs_open(content)
                || expr_uses_fs_open(host)
                || expr_uses_fs_open(port)
        }
        ValueExpr::FsReadToString { path }
        | ValueExpr::FsReadBytes { path }
        | ValueExpr::FsExists { path }
        | ValueExpr::FsMetadata { path }
        | ValueExpr::FsCreateDir { path }
        | ValueExpr::FsRemoveDir { path }
        | ValueExpr::FsReadDir { path }
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
        | ValueExpr::HashBytes { value: path }
        | ValueExpr::HashFinish { state: path }
        | ValueExpr::CryptoSha256 { value: path }
        | ValueExpr::CryptoSha512 { value: path }
        | ValueExpr::CryptoRandomBytes { count: path }
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
        | ValueExpr::ArrayLen { array: path }
        | ValueExpr::FileReadToString { file: path }
        | ValueExpr::TcpListenerAccept { listener: path }
        | ValueExpr::TcpListenerClose { listener: path }
        | ValueExpr::TcpStreamClose { stream: path }
        | ValueExpr::TcpStreamReadToString { stream: path }
        | ValueExpr::UdpSocketClose { socket: path } => expr_uses_fs_open(path),
        ValueExpr::ResultMapErr { result, .. }
        | ValueExpr::ResultIsOk { result, .. }
        | ValueExpr::ResultIsErr { result, .. }
        | ValueExpr::ResultMap { result, .. }
        | ValueExpr::ResultAndThen { result, .. }
        | ValueExpr::OptionIsSome { option: result, .. }
        | ValueExpr::OptionIsNone { option: result, .. }
        | ValueExpr::OptionMap { option: result, .. }
        | ValueExpr::OptionAndThen { option: result, .. } => expr_uses_fs_open(result),
        ValueExpr::ResultUnwrapOr {
            result, default, ..
        } => expr_uses_fs_open(result) || expr_uses_fs_open(default),
        ValueExpr::OptionUnwrapOr {
            option, default, ..
        } => expr_uses_fs_open(option) || expr_uses_fs_open(default),
        ValueExpr::FsWriteString { path, content } => {
            expr_uses_fs_open(path) || expr_uses_fs_open(content)
        }
        ValueExpr::FsWriteBytes { path, bytes } => {
            expr_uses_fs_open(path) || expr_uses_fs_open(bytes)
        }
        ValueExpr::FileWriteString { file, content } => {
            expr_uses_fs_open(file) || expr_uses_fs_open(content)
        }
        ValueExpr::EnvSet { name, value } => expr_uses_fs_open(name) || expr_uses_fs_open(value),
        ValueExpr::EnvArgs => false,
        ValueExpr::EnvCwd | ValueExpr::EnvHomeDir | ValueExpr::EnvTempDir => false,
        ValueExpr::ArrayNew { .. }
        | ValueExpr::HashNew
        | ValueExpr::CollectionsStringMapNew
        | ValueExpr::CollectionsStringSetNew => false,
        ValueExpr::HashWriteString { state, value } => {
            expr_uses_fs_open(state) || expr_uses_fs_open(value)
        }
        ValueExpr::HashWriteBytes { state, value } => {
            expr_uses_fs_open(state) || expr_uses_fs_open(value)
        }
        ValueExpr::CollectionsStringMapSet { map, key, value } => {
            expr_uses_fs_open(map) || expr_uses_fs_open(key) || expr_uses_fs_open(value)
        }
        ValueExpr::ArrayIter { array, .. } => expr_uses_fs_open(array),
        ValueExpr::ArrayGet { array, index, .. } => {
            expr_uses_fs_open(array) || expr_uses_fs_open(index)
        }
        ValueExpr::ArrayPop { .. } | ValueExpr::ArrayClear { .. } => false,
        ValueExpr::ArrayRemove { index, .. } => expr_uses_fs_open(index),
        ValueExpr::ArrayPush { value, .. } => expr_uses_fs_open(value),
        ValueExpr::ArraySet { index, value, .. } => {
            expr_uses_fs_open(index) || expr_uses_fs_open(value)
        }
        ValueExpr::ArrayInsert { index, value, .. } => {
            expr_uses_fs_open(index) || expr_uses_fs_open(value)
        }
        ValueExpr::Call { args, .. } => args.iter().any(expr_uses_fs_open),
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
        | ValueExpr::EnumPayloadFieldAccess { value, .. } => expr_uses_fs_open(value),
        ValueExpr::StructLiteral { fields, .. } => {
            fields.iter().any(|(_, value)| expr_uses_fs_open(value))
        }
        ValueExpr::EnumVariant { payload, .. } => payload.as_deref().is_some_and(expr_uses_fs_open),
        ValueExpr::If {
            condition,
            then_branch,
            else_branch,
        } => {
            expr_uses_fs_open(condition)
                || expr_uses_fs_open(then_branch)
                || expr_uses_fs_open(else_branch)
        }
        ValueExpr::Panic { message, .. } => expr_uses_fs_open(message),
        ValueExpr::Match { value, arms } => {
            expr_uses_fs_open(value) || arms.iter().any(|arm| expr_uses_fs_open(&arm.value))
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
        | ValueExpr::IoReadLine
        | ValueExpr::FieldAccess { .. } => false,
    }
}
