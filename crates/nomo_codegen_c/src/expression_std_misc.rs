use super::*;

pub(super) fn emit_std_misc_expr(out: &mut String, expr: &ValueExpr) -> bool {
    match expr {
        ValueExpr::OsPlatform => {
            out.push_str("nomo_os_platform()");
        }
        ValueExpr::OsArch => {
            out.push_str("nomo_os_arch()");
        }
        ValueExpr::OsPathSeparator => {
            out.push_str("nomo_os_path_separator()");
        }
        ValueExpr::OsLineEnding => {
            out.push_str("nomo_os_line_ending()");
        }
        ValueExpr::TimeNowMillis => {
            out.push_str("nomo_time_now_millis()");
        }
        ValueExpr::TimeMonotonicMillis => {
            out.push_str("nomo_time_monotonic_millis()");
        }
        ValueExpr::TimeDurationMillis { millis } => {
            out.push_str("(nomo_struct_Duration){ .nomo_member_millis = ");
            emit_expr(out, millis);
            out.push_str(" }");
        }
        ValueExpr::TimeDurationSeconds { seconds } => {
            out.push_str("(nomo_struct_Duration){ .nomo_member_millis = nomo_time_duration_seconds_to_millis(");
            emit_expr(out, seconds);
            out.push_str(") }");
        }
        ValueExpr::TimeDurationAsMillis { duration } => {
            out.push('(');
            emit_expr(out, duration);
            out.push_str(").nomo_member_millis");
        }
        ValueExpr::TimeFormatDuration { duration } => {
            out.push_str("nomo_time_format_duration_millis((");
            emit_expr(out, duration);
            out.push_str(").nomo_member_millis)");
        }
        ValueExpr::TimeSleep { duration } => {
            out.push_str("nomo_time_sleep_millis((");
            emit_expr(out, duration);
            out.push_str(").nomo_member_millis)");
        }
        ValueExpr::TimeSleepMillis { duration } => {
            out.push_str("nomo_time_sleep_millis(");
            emit_expr(out, duration);
            out.push(')');
        }
        ValueExpr::LogEnabled { level } => {
            out.push_str("nomo_log_enabled(");
            emit_expr(out, level);
            out.push(')');
        }
        ValueExpr::HashNew => {
            out.push_str("nomo_hash_new()");
        }
        ValueExpr::HashString { value } => {
            out.push_str("nomo_hash_string(");
            emit_expr(out, value);
            out.push(')');
        }
        ValueExpr::HashBytes { value } => {
            out.push_str("nomo_hash_bytes(");
            emit_expr(out, value);
            out.push(')');
        }
        ValueExpr::HashWriteString { state, value } => {
            out.push_str("nomo_hash_write_string(");
            emit_expr(out, state);
            out.push_str(", ");
            emit_expr(out, value);
            out.push(')');
        }
        ValueExpr::HashWriteBytes { state, value } => {
            out.push_str("nomo_hash_write_bytes(");
            emit_expr(out, state);
            out.push_str(", ");
            emit_expr(out, value);
            out.push(')');
        }
        ValueExpr::HashFinish { state } => {
            out.push_str("nomo_hash_finish(");
            emit_expr(out, state);
            out.push(')');
        }
        ValueExpr::CryptoSha256 { value } => {
            out.push_str("nomo_crypto_sha256(");
            emit_expr(out, value);
            out.push(')');
        }
        ValueExpr::CryptoSha512 { value } => {
            out.push_str("nomo_crypto_sha512(");
            emit_expr(out, value);
            out.push(')');
        }
        ValueExpr::CryptoRandomBytes { count } => {
            out.push_str("nomo_crypto_random_bytes(");
            emit_expr(out, count);
            out.push(')');
        }
        ValueExpr::JsonParse { value } => {
            out.push_str("nomo_json_parse(");
            emit_expr(out, value);
            out.push(')');
        }
        ValueExpr::JsonStringify { value } => {
            out.push_str("nomo_json_stringify(");
            emit_expr(out, value);
            out.push(')');
        }
        ValueExpr::RegexCompile { pattern } => {
            out.push_str("nomo_regex_compile(");
            emit_expr(out, pattern);
            out.push(')');
        }
        ValueExpr::RegexIsMatch { regex, value } => {
            out.push_str("nomo_regex_is_match(");
            emit_expr(out, regex);
            out.push_str(", ");
            emit_expr(out, value);
            out.push(')');
        }
        ValueExpr::RegexCaptures { regex, value } => {
            out.push_str("nomo_regex_captures(");
            emit_expr(out, regex);
            out.push_str(", ");
            emit_expr(out, value);
            out.push(')');
        }
        _ => return false,
    }
    true
}
