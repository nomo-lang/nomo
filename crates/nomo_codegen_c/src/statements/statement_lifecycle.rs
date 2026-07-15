use super::*;

pub(super) fn emit_enum_temp_release_if_owned(
    out: &mut String,
    temp: &str,
    enum_name: &str,
    enum_args: &[ValueType],
    value: &ValueExpr,
    indent: usize,
) {
    let enum_type = ValueType::Enum(enum_name.to_string(), enum_args.to_vec());
    if expr_may_share_array_storage(value) || !value_type_needs_release(&enum_type) {
        return;
    }
    emit_value_release_in_place(out, &enum_type, temp, indent);
}

pub(super) fn emit_array_retain_after_binding(
    out: &mut String,
    name: &str,
    value_type: &ValueType,
    initializer: &ValueExpr,
    indent: usize,
) {
    if !value_type_needs_release(value_type) || !expr_may_share_array_storage(initializer) {
        return;
    }
    emit_value_retain_in_place(out, value_type, &c_var_ident(name), indent);
}

pub(super) fn emit_array_retain_binding(
    out: &mut String,
    name: &str,
    value_type: &ValueType,
    indent: usize,
) {
    if !value_type_needs_release(value_type) {
        return;
    }
    emit_value_retain_in_place(out, value_type, &c_var_ident(name), indent);
}

pub(super) fn emit_value_retain_value_if_needed(
    out: &mut String,
    c_value: &str,
    value_type: &ValueType,
    initializer: &ValueExpr,
    indent: usize,
) {
    if !expr_may_share_array_storage(initializer) || !value_type_needs_release(value_type) {
        return;
    }
    emit_value_retain_in_place(out, value_type, c_value, indent);
}

pub(super) fn local_array_from_statement(statement: &Statement) -> Option<LocalArray> {
    match statement {
        Statement::Let {
            name, value_type, ..
        }
        | Statement::LetIf {
            name, value_type, ..
        }
        | Statement::LetMatch {
            name, value_type, ..
        }
        | Statement::QuestionLet {
            name, value_type, ..
        } => local_array(name, value_type),
        Statement::LetElse {
            binding,
            value_type,
            ..
        } => local_array(binding, value_type),
        _ => None,
    }
}

pub(super) fn array_params(function: &Function) -> Vec<LocalArray> {
    function
        .params
        .iter()
        .filter(|param| !param.mutable)
        .filter_map(|param| local_array(&param.name, &param.value_type))
        .collect()
}

pub(super) fn local_array(name: &str, value_type: &ValueType) -> Option<LocalArray> {
    if value_type_needs_release(value_type) {
        Some(LocalArray {
            name: name.to_string(),
            value_type: value_type.clone(),
            c_value: None,
        })
    } else {
        None
    }
}

pub(super) fn local_c_value(c_value: &str, value_type: &ValueType) -> Option<LocalArray> {
    if value_type_needs_release(value_type) {
        Some(LocalArray {
            name: c_value.to_string(),
            value_type: value_type.clone(),
            c_value: Some(c_value.to_string()),
        })
    } else {
        None
    }
}

pub(super) fn emit_array_releases(out: &mut String, indent: usize, active_arrays: &[LocalArray]) {
    for local in active_arrays.iter().rev() {
        if let Some(c_value) = &local.c_value {
            emit_value_release_in_place(out, &local.value_type, c_value, indent);
        } else {
            emit_value_release_binding(out, &local.name, &local.value_type, indent);
        }
    }
}

pub(super) fn value_type_needs_release(value_type: &ValueType) -> bool {
    match value_type {
        ValueType::String | ValueType::CString => true,
        ValueType::Array(element_type) => is_supported_array_element(element_type),
        ValueType::Struct(_, _) | ValueType::Enum(_, _) => true,
        _ => false,
    }
}

pub(super) fn emit_value_release_binding(
    out: &mut String,
    name: &str,
    value_type: &ValueType,
    indent: usize,
) {
    emit_value_release_in_place(out, value_type, &c_var_ident(name), indent);
}

pub(super) fn emit_value_release_in_place(
    out: &mut String,
    value_type: &ValueType,
    c_value: &str,
    indent: usize,
) {
    match value_type {
        ValueType::Array(element_type) if is_supported_array_element(element_type) => {
            write_indent(out, indent);
            emit_array_release_expr(out, element_type, c_value);
            out.push_str(";\n");
        }
        ValueType::String | ValueType::CString => {
            write_indent(out, indent);
            out.push_str("nomo_string_release(");
            out.push_str(c_value);
            out.push_str(");\n");
        }
        ValueType::Struct(_, _) | ValueType::Enum(_, _) => {
            write_indent(out, indent);
            out.push_str(&c_release_ident(value_type));
            out.push('(');
            out.push_str(c_value);
            out.push_str(");\n");
        }
        _ => {}
    }
}

pub(super) fn emit_value_retain_in_place(
    out: &mut String,
    value_type: &ValueType,
    c_value: &str,
    indent: usize,
) {
    match value_type {
        ValueType::Array(element_type) if is_supported_array_element(element_type) => {
            write_indent(out, indent);
            out.push_str(c_value);
            out.push_str(" = ");
            out.push_str(&c_array_ident(element_type));
            out.push_str("_retain(");
            out.push_str(c_value);
            out.push_str(");\n");
        }
        ValueType::String | ValueType::CString => {
            write_indent(out, indent);
            out.push_str(c_value);
            out.push_str(" = nomo_string_retain(");
            out.push_str(c_value);
            out.push_str(");\n");
        }
        ValueType::Struct(_, _) | ValueType::Enum(_, _) => {
            write_indent(out, indent);
            out.push_str(c_value);
            out.push_str(" = ");
            out.push_str(&c_retain_ident(value_type));
            out.push('(');
            out.push_str(c_value);
            out.push_str(");\n");
        }
        _ => {}
    }
}

fn emit_array_release_expr(out: &mut String, element_type: &ValueType, c_value: &str) {
    out.push_str(&c_array_ident(element_type));
    out.push_str("_release(");
    out.push_str(c_value);
    out.push(')');
}

pub(super) fn active_array_type<'a>(
    active_arrays: &'a [LocalArray],
    name: &str,
) -> Option<&'a ValueType> {
    active_arrays
        .iter()
        .find(|local| local.name == name)
        .map(|local| &local.value_type)
}

pub(super) fn emit_array_retain_return_if_needed(
    out: &mut String,
    value: &ValueExpr,
    return_type: &ValueType,
    indent: usize,
) {
    if !value_type_needs_release(return_type) || !expr_may_share_array_storage(value) {
        return;
    }
    emit_value_retain_in_place(out, return_type, "nomo__return", indent);
}

pub(super) fn expr_may_share_array_storage(value: &ValueExpr) -> bool {
    match value {
        ValueExpr::Variable(_)
        | ValueExpr::FunctionRef(_)
        | ValueExpr::FieldAccess { .. }
        | ValueExpr::EnumPayload { .. }
        | ValueExpr::EnumPayloadFieldAccess { .. } => true,
        ValueExpr::Cast { expr, .. }
        | ValueExpr::Unary { expr, .. }
        | ValueExpr::StringLen { value: expr }
        | ValueExpr::FsReadToString { path: expr }
        | ValueExpr::FsReadBytes { path: expr }
        | ValueExpr::FsExists { path: expr }
        | ValueExpr::FsMetadata { path: expr }
        | ValueExpr::FsCreateDir { path: expr }
        | ValueExpr::FsRemoveDir { path: expr }
        | ValueExpr::FsReadDir { path: expr }
        | ValueExpr::FsOpen { path: expr }
        | ValueExpr::FileClose { file: expr }
        | ValueExpr::FileReadToString { file: expr }
        | ValueExpr::TcpListenerAccept { listener: expr }
        | ValueExpr::TcpListenerClose { listener: expr }
        | ValueExpr::TcpStreamClose { stream: expr }
        | ValueExpr::TcpStreamReadToString { stream: expr }
        | ValueExpr::UdpSocketClose { socket: expr }
        | ValueExpr::EnvGet { name: expr }
        | ValueExpr::TimeDurationMillis { millis: expr }
        | ValueExpr::TimeDurationSeconds { seconds: expr }
        | ValueExpr::TimeDurationAsMillis { duration: expr }
        | ValueExpr::TimeFormatDuration { duration: expr }
        | ValueExpr::TimeSleep { duration: expr }
        | ValueExpr::TimeSleepMillis { duration: expr }
        | ValueExpr::LogEnabled { level: expr }
        | ValueExpr::HashString { value: expr }
        | ValueExpr::HashBytes { value: expr }
        | ValueExpr::HashFinish { state: expr }
        | ValueExpr::CryptoSha256 { value: expr }
        | ValueExpr::CryptoSha512 { value: expr }
        | ValueExpr::CryptoRandomBytes { count: expr }
        | ValueExpr::JsonParse { value: expr }
        | ValueExpr::JsonStringify { value: expr }
        | ValueExpr::ProcessExit { code: expr }
        | ValueExpr::ProcessSpawn { command: expr }
        | ValueExpr::ProcessStatus { command: expr }
        | ValueExpr::ProcessExec { command: expr }
        | ValueExpr::ProcessOutput { command: expr }
        | ValueExpr::NumParseI64 { value: expr }
        | ValueExpr::NumParseU64 { value: expr }
        | ValueExpr::NumParseF64 { value: expr }
        | ValueExpr::NumToString { value: expr, .. }
        | ValueExpr::ArrayLen { array: expr }
        | ValueExpr::EnumVariant {
            payload: Some(expr),
            ..
        } => expr_may_share_array_storage(expr),
        ValueExpr::Binary { left, right, .. }
        | ValueExpr::StringCompare { left, right, .. }
        | ValueExpr::NumBinary { left, right, .. }
        | ValueExpr::FsWriteString {
            path: left,
            content: right,
        }
        | ValueExpr::FsWriteBytes {
            path: left,
            bytes: right,
        }
        | ValueExpr::HashWriteString {
            state: left,
            value: right,
        }
        | ValueExpr::HashWriteBytes {
            state: left,
            value: right,
        }
        | ValueExpr::FileWriteString {
            file: left,
            content: right,
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
        } => expr_may_share_array_storage(left) || expr_may_share_array_storage(right),
        ValueExpr::UdpSocketSendToString {
            socket,
            content,
            host,
            port,
        } => {
            expr_may_share_array_storage(socket)
                || expr_may_share_array_storage(content)
                || expr_may_share_array_storage(host)
                || expr_may_share_array_storage(port)
        }
        ValueExpr::StringConcat { .. }
        | ValueExpr::StringIsEmpty { .. }
        | ValueExpr::StringContains { .. }
        | ValueExpr::StringStartsWith { .. }
        | ValueExpr::StringEndsWith { .. }
        | ValueExpr::StringSplit { .. }
        | ValueExpr::StringTrim { .. }
        | ValueExpr::StringToLower { .. }
        | ValueExpr::StringToUpper { .. }
        | ValueExpr::RegexCompile { .. }
        | ValueExpr::RegexIsMatch { .. }
        | ValueExpr::RegexCaptures { .. }
        | ValueExpr::CharIsDigit { .. }
        | ValueExpr::CharIsAlpha { .. }
        | ValueExpr::CharIsWhitespace { .. }
        | ValueExpr::CharToString { .. }
        | ValueExpr::PathJoin { .. }
        | ValueExpr::PathBasename { .. }
        | ValueExpr::PathDirname { .. }
        | ValueExpr::PathExtension { .. }
        | ValueExpr::PathNormalize { .. }
        | ValueExpr::PathIsAbsolute { .. }
        | ValueExpr::MathUnary { .. }
        | ValueExpr::MathBinary { .. } => false,
        ValueExpr::ArrayPush { value, .. }
        | ValueExpr::ArraySet { value, .. }
        | ValueExpr::ArrayInsert { value, .. } => expr_may_share_array_storage(value),
        ValueExpr::ArrayPop { .. }
        | ValueExpr::ArrayRemove { .. }
        | ValueExpr::ArrayClear { .. } => false,
        ValueExpr::EnvSet { .. } => false,
        ValueExpr::ResultMapErr { result, .. }
        | ValueExpr::ResultMap { result, .. }
        | ValueExpr::ResultAndThen { result, .. }
        | ValueExpr::OptionMap { option: result, .. }
        | ValueExpr::OptionAndThen { option: result, .. }
        | ValueExpr::ResultIsOk { result, .. }
        | ValueExpr::ResultIsErr { result, .. }
        | ValueExpr::OptionIsSome { option: result, .. }
        | ValueExpr::OptionIsNone { option: result, .. } => expr_may_share_array_storage(result),
        ValueExpr::ResultUnwrapOr {
            result, default, ..
        } => expr_may_share_array_storage(result) || expr_may_share_array_storage(default),
        ValueExpr::OptionUnwrapOr {
            option, default, ..
        } => expr_may_share_array_storage(option) || expr_may_share_array_storage(default),
        ValueExpr::StructLiteral { fields, .. } => fields
            .iter()
            .any(|(_, field)| expr_may_share_array_storage(field)),
        ValueExpr::Match { value, arms } => {
            expr_may_share_array_storage(value)
                || arms
                    .iter()
                    .any(|arm| expr_may_share_array_storage(&arm.value))
        }
        ValueExpr::If {
            condition,
            then_branch,
            else_branch,
        } => {
            expr_may_share_array_storage(condition)
                || expr_may_share_array_storage(then_branch)
                || expr_may_share_array_storage(else_branch)
        }
        ValueExpr::StringLiteral(_)
        | ValueExpr::IntLiteral(_)
        | ValueExpr::FloatLiteral(_)
        | ValueExpr::CharLiteral(_)
        | ValueExpr::BoolLiteral(_)
        | ValueExpr::VoidLiteral
        | ValueExpr::Panic { .. }
        | ValueExpr::HashNew
        | ValueExpr::CollectionsStringMapNew
        | ValueExpr::CollectionsStringMapLen { .. }
        | ValueExpr::CollectionsStringMapGet { .. }
        | ValueExpr::CollectionsStringMapContains { .. }
        | ValueExpr::CollectionsStringMapSet { .. }
        | ValueExpr::CollectionsStringMapRemove { .. }
        | ValueExpr::CollectionsStringSetNew
        | ValueExpr::CollectionsStringSetLen { .. }
        | ValueExpr::CollectionsStringSetContains { .. }
        | ValueExpr::CollectionsStringSetInsert { .. }
        | ValueExpr::CollectionsStringSetRemove { .. }
        | ValueExpr::MutBorrow(_)
        | ValueExpr::Call { .. }
        | ValueExpr::EnvCwd
        | ValueExpr::EnvHomeDir
        | ValueExpr::EnvTempDir
        | ValueExpr::OsPlatform
        | ValueExpr::OsArch
        | ValueExpr::OsPathSeparator
        | ValueExpr::OsLineEnding
        | ValueExpr::TimeNowMillis
        | ValueExpr::TimeMonotonicMillis
        | ValueExpr::EnvArgs
        | ValueExpr::IoReadLine
        | ValueExpr::ArrayNew { .. }
        | ValueExpr::ArrayIter { .. }
        | ValueExpr::ArrayGet { .. }
        | ValueExpr::EnumVariant { payload: None, .. } => false,
    }
}
