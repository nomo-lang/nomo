use super::*;

pub(super) fn emit_expr(out: &mut String, expr: &ValueExpr) {
    if emit_array_expr(out, expr) {
        return;
    }
    if emit_result_option_expr(out, expr) {
        return;
    }

    match expr {
        ValueExpr::StringLiteral(value) => {
            out.push_str("nomo_string_literal(\"");
            out.push_str(&escape_c_string(value));
            out.push_str("\")");
        }
        ValueExpr::IntLiteral(value) => out.push_str(&value.to_string()),
        ValueExpr::FloatLiteral(value) => out.push_str(value),
        ValueExpr::CharLiteral(value) => out.push_str(&(*value as u32).to_string()),
        ValueExpr::BoolLiteral(value) => out.push_str(if *value { "1" } else { "0" }),
        ValueExpr::VoidLiteral => out.push('0'),
        ValueExpr::Variable(name) => out.push_str(&c_var_ident(name)),
        ValueExpr::MutBorrow(path) => {
            out.push('&');
            emit_lvalue_path(out, path);
        }
        ValueExpr::Cast { expr, target_type } => {
            out.push_str("((");
            out.push_str(&c_type(target_type));
            out.push(')');
            emit_expr(out, expr);
            out.push(')');
        }
        ValueExpr::StructLiteral {
            type_name,
            struct_args,
            fields,
        } => {
            out.push('(');
            out.push_str(&c_struct_ident(type_name, struct_args));
            out.push_str("){");
            for (index, (field_name, value)) in fields.iter().enumerate() {
                if index > 0 {
                    out.push_str(", ");
                }
                out.push('.');
                out.push_str(&c_member_ident(field_name));
                out.push_str(" = ");
                emit_expr(out, value);
            }
            out.push('}');
        }
        ValueExpr::FieldAccess { base, field } => {
            out.push_str(&c_var_ident(base));
            out.push('.');
            out.push_str(&c_member_ident(field));
        }
        ValueExpr::EnumPayloadFieldAccess {
            value,
            variant,
            field,
        } => {
            emit_expr(out, value);
            out.push_str(".payload.");
            out.push_str(&c_payload_ident(variant));
            out.push('.');
            out.push_str(&c_member_ident(field));
        }
        ValueExpr::EnumVariant {
            enum_name,
            enum_args,
            variant,
            payload,
        } => {
            out.push('(');
            out.push_str(&c_enum_ident(enum_name, enum_args));
            out.push_str("){.tag = ");
            out.push_str(&c_enum_variant_ident(enum_name, enum_args, variant));
            if let Some(payload) = payload {
                out.push_str(", .payload.");
                out.push_str(&c_payload_ident(variant));
                out.push_str(" = ");
                emit_expr(out, payload);
            }
            out.push('}');
        }
        ValueExpr::EnumPayload { value, variant } => {
            emit_expr(out, value);
            out.push_str(".payload.");
            out.push_str(&c_payload_ident(variant));
        }
        ValueExpr::If {
            condition,
            then_branch,
            else_branch,
        } => {
            out.push('(');
            emit_expr(out, condition);
            out.push_str(" ? ");
            emit_expr(out, then_branch);
            out.push_str(" : ");
            emit_expr(out, else_branch);
            out.push(')');
        }
        ValueExpr::Panic {
            message,
            fallback_type,
        } => {
            out.push_str("(nomo_panic(");
            emit_string_data_expr(out, message);
            out.push_str("), ");
            out.push_str(&c_zero_value(fallback_type));
            out.push(')');
        }
        ValueExpr::Match { value, arms } => emit_match_expr(out, value, arms),
        ValueExpr::Binary {
            left,
            op,
            right,
            value_type,
        } => {
            if let Some(helper) = checked_binary_helper(op, value_type) {
                out.push_str(helper);
                out.push('(');
                emit_expr(out, left);
                out.push_str(", ");
                emit_expr(out, right);
                out.push(')');
            } else {
                out.push('(');
                emit_expr(out, left);
                if matches!(op, BinaryOp::BitAndNot) {
                    out.push_str(" & ~(");
                    emit_expr(out, right);
                    out.push(')');
                } else {
                    out.push(' ');
                    out.push_str(c_binary_op(op));
                    out.push(' ');
                    emit_expr(out, right);
                }
                out.push(')');
            }
        }
        ValueExpr::Unary { op, expr } => {
            out.push('(');
            out.push_str(c_unary_op(op));
            emit_expr(out, expr);
            out.push(')');
        }
        ValueExpr::StringCompare { left, op, right } => {
            out.push('(');
            if matches!(op, BinaryOp::NotEqual) {
                out.push('!');
            }
            out.push_str("nomo_string_equal(");
            emit_expr(out, left);
            out.push_str(", ");
            emit_expr(out, right);
            out.push_str("))");
        }
        ValueExpr::Call { name, args } => {
            if name == BUILTIN_PRINTLN_EXPR {
                out.push_str("(puts(");
                emit_string_data_expr(out, &args[0]);
                out.push_str("), 0)");
            } else if name == BUILTIN_PRINT_EXPR {
                out.push_str("(fputs(");
                emit_string_data_expr(out, &args[0]);
                out.push_str(", stdout), 0)");
            } else if name == BUILTIN_EPRINTLN_EXPR {
                out.push_str("(fputs(");
                emit_string_data_expr(out, &args[0]);
                out.push_str(", stderr), fputc('\\n', stderr), 0)");
            } else if name == BUILTIN_EPRINT_EXPR {
                out.push_str("(fputs(");
                emit_string_data_expr(out, &args[0]);
                out.push_str(", stderr), 0)");
            } else if name == BUILTIN_FFI_PUTS_EXPR {
                out.push_str("puts(");
                emit_string_data_expr(out, &args[0]);
                out.push(')');
            } else if let Some(symbol) = name.strip_prefix(EXTERN_CALL_PREFIX) {
                out.push_str(symbol);
                out.push('(');
                for (index, arg) in args.iter().enumerate() {
                    if index > 0 {
                        out.push_str(", ");
                    }
                    emit_expr(out, arg);
                }
                out.push(')');
            } else {
                out.push_str(&c_fn_ident(name));
                out.push('(');
                for (index, arg) in args.iter().enumerate() {
                    if index > 0 {
                        out.push_str(", ");
                    }
                    emit_expr(out, arg);
                }
                out.push(')');
            }
        }
        ValueExpr::StringLen { value } => {
            out.push_str("((uint64_t)strlen(");
            emit_string_data_expr(out, value);
            out.push_str("))");
        }
        ValueExpr::StringConcat { left, right } => {
            out.push_str("nomo_string_concat(");
            emit_expr(out, left);
            out.push_str(", ");
            emit_expr(out, right);
            out.push(')');
        }
        ValueExpr::StringIsEmpty { value } => {
            out.push_str("nomo_string_is_empty(");
            emit_expr(out, value);
            out.push(')');
        }
        ValueExpr::StringContains { value, needle } => {
            out.push_str("nomo_string_contains(");
            emit_expr(out, value);
            out.push_str(", ");
            emit_expr(out, needle);
            out.push(')');
        }
        ValueExpr::StringStartsWith { value, prefix } => {
            out.push_str("nomo_string_starts_with(");
            emit_expr(out, value);
            out.push_str(", ");
            emit_expr(out, prefix);
            out.push(')');
        }
        ValueExpr::StringEndsWith { value, suffix } => {
            out.push_str("nomo_string_ends_with(");
            emit_expr(out, value);
            out.push_str(", ");
            emit_expr(out, suffix);
            out.push(')');
        }
        ValueExpr::StringSplit { value, separator } => {
            out.push_str("nomo_string_split(");
            emit_expr(out, value);
            out.push_str(", ");
            emit_expr(out, separator);
            out.push(')');
        }
        ValueExpr::StringTrim { value } => {
            out.push_str("nomo_string_trim(");
            emit_expr(out, value);
            out.push(')');
        }
        ValueExpr::StringToLower { value } => {
            out.push_str("nomo_string_to_lower(");
            emit_expr(out, value);
            out.push(')');
        }
        ValueExpr::StringToUpper { value } => {
            out.push_str("nomo_string_to_upper(");
            emit_expr(out, value);
            out.push(')');
        }
        ValueExpr::CharIsDigit { value } => {
            out.push_str("nomo_char_is_digit(");
            emit_expr(out, value);
            out.push(')');
        }
        ValueExpr::CharIsAlpha { value } => {
            out.push_str("nomo_char_is_alpha(");
            emit_expr(out, value);
            out.push(')');
        }
        ValueExpr::CharIsWhitespace { value } => {
            out.push_str("nomo_char_is_whitespace(");
            emit_expr(out, value);
            out.push(')');
        }
        ValueExpr::CharToString { value } => {
            out.push_str("nomo_char_to_string(");
            emit_expr(out, value);
            out.push(')');
        }
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
        ValueExpr::CollectionsStringMapNew => {
            out.push_str("nomo_collections_map_new()");
        }
        ValueExpr::CollectionsStringMapLen { map } => {
            out.push_str("nomo_collections_map_len(");
            emit_expr(out, map);
            out.push(')');
        }
        ValueExpr::CollectionsStringMapGet { map, key } => {
            out.push_str("nomo_collections_map_get(");
            emit_expr(out, map);
            out.push_str(", ");
            emit_expr(out, key);
            out.push(')');
        }
        ValueExpr::CollectionsStringMapContains { map, key } => {
            out.push_str("nomo_collections_map_contains(");
            emit_expr(out, map);
            out.push_str(", ");
            emit_expr(out, key);
            out.push(')');
        }
        ValueExpr::CollectionsStringMapSet { map, key, value } => {
            out.push_str("nomo_collections_map_set(");
            emit_expr(out, map);
            out.push_str(", ");
            emit_expr(out, key);
            out.push_str(", ");
            emit_expr(out, value);
            out.push(')');
        }
        ValueExpr::CollectionsStringMapRemove { map, key } => {
            out.push_str("nomo_collections_map_remove(");
            emit_expr(out, map);
            out.push_str(", ");
            emit_expr(out, key);
            out.push(')');
        }
        ValueExpr::CollectionsStringSetNew => {
            out.push_str("nomo_collections_set_new()");
        }
        ValueExpr::CollectionsStringSetLen { set } => {
            out.push_str("nomo_collections_set_len(");
            emit_expr(out, set);
            out.push(')');
        }
        ValueExpr::CollectionsStringSetContains { set, value } => {
            out.push_str("nomo_collections_set_contains(");
            emit_expr(out, set);
            out.push_str(", ");
            emit_expr(out, value);
            out.push(')');
        }
        ValueExpr::CollectionsStringSetInsert { set, value } => {
            out.push_str("nomo_collections_set_insert(");
            emit_expr(out, set);
            out.push_str(", ");
            emit_expr(out, value);
            out.push(')');
        }
        ValueExpr::CollectionsStringSetRemove { set, value } => {
            out.push_str("nomo_collections_set_remove(");
            emit_expr(out, set);
            out.push_str(", ");
            emit_expr(out, value);
            out.push(')');
        }
        ValueExpr::ProcessExit { code } => {
            out.push_str("exit((int)");
            emit_expr(out, code);
            out.push(')');
        }
        ValueExpr::ProcessSpawn { command } => {
            out.push_str("nomo_process_spawn(");
            emit_expr(out, command);
            out.push(')');
        }
        ValueExpr::ProcessStatus { command } => {
            out.push_str("nomo_process_status(");
            emit_expr(out, command);
            out.push(')');
        }
        ValueExpr::ProcessExec { command } => {
            out.push_str("nomo_process_exec(");
            emit_expr(out, command);
            out.push(')');
        }
        ValueExpr::ProcessOutput { command } => {
            out.push_str("nomo_process_output(");
            emit_expr(out, command);
            out.push(')');
        }
        ValueExpr::NumParseI64 { value } => {
            out.push_str("nomo_num_parse_i64(");
            emit_expr(out, value);
            out.push(')');
        }
        ValueExpr::NumParseU64 { value } => {
            out.push_str("nomo_num_parse_u64(");
            emit_expr(out, value);
            out.push(')');
        }
        ValueExpr::NumParseF64 { value } => {
            out.push_str("nomo_num_parse_f64(");
            emit_expr(out, value);
            out.push(')');
        }
        ValueExpr::NumToString { value, value_type } => {
            out.push_str(num_to_string_helper_name(value_type));
            out.push('(');
            emit_expr(out, value);
            out.push(')');
        }
        ValueExpr::NumBinary {
            function,
            op,
            left,
            right,
            value_type,
        } => {
            let helper = match function {
                NumBinaryFunction::Checked => num_checked_binary_helper_name(op, value_type),
                NumBinaryFunction::Wrapping => num_wrapping_binary_helper_name(op, value_type),
            };
            out.push_str(helper);
            out.push('(');
            emit_expr(out, left);
            out.push_str(", ");
            emit_expr(out, right);
            out.push(')');
        }
        ValueExpr::PathJoin { left, right } => {
            out.push_str("nomo_path_join(");
            emit_expr(out, left);
            out.push_str(", ");
            emit_expr(out, right);
            out.push(')');
        }
        ValueExpr::PathBasename { path } => {
            out.push_str("nomo_path_basename(");
            emit_expr(out, path);
            out.push(')');
        }
        ValueExpr::PathDirname { path } => {
            out.push_str("nomo_path_dirname(");
            emit_expr(out, path);
            out.push(')');
        }
        ValueExpr::PathExtension { path } => {
            out.push_str("nomo_path_extension(");
            emit_expr(out, path);
            out.push(')');
        }
        ValueExpr::PathNormalize { path } => {
            out.push_str("nomo_path_normalize(");
            emit_expr(out, path);
            out.push(')');
        }
        ValueExpr::PathIsAbsolute { path } => {
            out.push_str("nomo_path_is_absolute(");
            emit_expr(out, path);
            out.push(')');
        }
        ValueExpr::MathUnary {
            function,
            value,
            value_type,
        } => {
            out.push_str(math_unary_function_name(*function, value_type));
            out.push('(');
            emit_expr(out, value);
            out.push(')');
        }
        ValueExpr::MathBinary {
            function,
            left,
            right,
            value_type,
        } => {
            out.push_str(math_binary_function_name(*function, value_type));
            out.push('(');
            emit_expr(out, left);
            out.push_str(", ");
            emit_expr(out, right);
            out.push(')');
        }
        ValueExpr::FsReadToString { path } => {
            out.push_str("nomo_fs_read_to_string(");
            emit_expr(out, path);
            out.push(')');
        }
        ValueExpr::FsWriteString { path, content } => {
            out.push_str("nomo_fs_write_string(");
            emit_expr(out, path);
            out.push_str(", ");
            emit_expr(out, content);
            out.push(')');
        }
        ValueExpr::FsReadBytes { path } => {
            out.push_str("nomo_fs_read_bytes(");
            emit_expr(out, path);
            out.push(')');
        }
        ValueExpr::FsWriteBytes { path, bytes } => {
            out.push_str("nomo_fs_write_bytes(");
            emit_expr(out, path);
            out.push_str(", ");
            emit_expr(out, bytes);
            out.push(')');
        }
        ValueExpr::FsExists { path } => {
            out.push_str("nomo_fs_exists(");
            emit_expr(out, path);
            out.push(')');
        }
        ValueExpr::FsMetadata { path } => {
            out.push_str("nomo_fs_metadata(");
            emit_expr(out, path);
            out.push(')');
        }
        ValueExpr::FsCreateDir { path } => {
            out.push_str("nomo_fs_create_dir(");
            emit_expr(out, path);
            out.push(')');
        }
        ValueExpr::FsRemoveDir { path } => {
            out.push_str("nomo_fs_remove_dir(");
            emit_expr(out, path);
            out.push(')');
        }
        ValueExpr::FsReadDir { path } => {
            out.push_str("nomo_fs_read_dir(");
            emit_expr(out, path);
            out.push(')');
        }
        ValueExpr::FsOpen { path } => {
            out.push_str("nomo_fs_open(");
            emit_expr(out, path);
            out.push(')');
        }
        ValueExpr::IoReadLine => {
            out.push_str("nomo_io_read_line()");
        }
        ValueExpr::FileClose { file } => {
            out.push_str("nomo_file_close(");
            emit_expr(out, file);
            out.push(')');
        }
        ValueExpr::FileReadToString { file } => {
            out.push_str("nomo_file_read_to_string(");
            emit_expr(out, file);
            out.push(')');
        }
        ValueExpr::FileWriteString { file, content } => {
            out.push_str("nomo_file_write_string(");
            emit_expr(out, file);
            out.push_str(", ");
            emit_expr(out, content);
            out.push(')');
        }
        ValueExpr::NetConnect { host, port } => {
            out.push_str("nomo_net_connect(");
            emit_expr(out, host);
            out.push_str(", ");
            emit_expr(out, port);
            out.push(')');
        }
        ValueExpr::NetListen { host, port } => {
            out.push_str("nomo_net_listen(");
            emit_expr(out, host);
            out.push_str(", ");
            emit_expr(out, port);
            out.push(')');
        }
        ValueExpr::NetUdpBind { host, port } => {
            out.push_str("nomo_net_udp_bind(");
            emit_expr(out, host);
            out.push_str(", ");
            emit_expr(out, port);
            out.push(')');
        }
        ValueExpr::TcpListenerAccept { listener } => {
            out.push_str("nomo_tcp_listener_accept(");
            emit_expr(out, listener);
            out.push(')');
        }
        ValueExpr::TcpListenerClose { listener } => {
            out.push_str("nomo_tcp_listener_close(");
            emit_expr(out, listener);
            out.push(')');
        }
        ValueExpr::TcpStreamClose { stream } => {
            out.push_str("nomo_tcp_stream_close(");
            emit_expr(out, stream);
            out.push(')');
        }
        ValueExpr::TcpStreamReadToString { stream } => {
            out.push_str("nomo_tcp_stream_read_to_string(");
            emit_expr(out, stream);
            out.push(')');
        }
        ValueExpr::TcpStreamWriteString { stream, content } => {
            out.push_str("nomo_tcp_stream_write_string(");
            emit_expr(out, stream);
            out.push_str(", ");
            emit_expr(out, content);
            out.push(')');
        }
        ValueExpr::UdpSocketClose { socket } => {
            out.push_str("nomo_udp_socket_close(");
            emit_expr(out, socket);
            out.push(')');
        }
        ValueExpr::UdpSocketRecvFromString { socket, max_bytes } => {
            out.push_str("nomo_udp_socket_recv_from_string(");
            emit_expr(out, socket);
            out.push_str(", ");
            emit_expr(out, max_bytes);
            out.push(')');
        }
        ValueExpr::UdpSocketSendToString {
            socket,
            content,
            host,
            port,
        } => {
            out.push_str("nomo_udp_socket_send_to_string(");
            emit_expr(out, socket);
            out.push_str(", ");
            emit_expr(out, content);
            out.push_str(", ");
            emit_expr(out, host);
            out.push_str(", ");
            emit_expr(out, port);
            out.push(')');
        }
        ValueExpr::ResultMapErr { .. }
        | ValueExpr::ResultIsOk { .. }
        | ValueExpr::ResultIsErr { .. }
        | ValueExpr::ResultUnwrapOr { .. }
        | ValueExpr::ResultMap { .. }
        | ValueExpr::ResultAndThen { .. }
        | ValueExpr::OptionIsSome { .. }
        | ValueExpr::OptionIsNone { .. }
        | ValueExpr::OptionUnwrapOr { .. }
        | ValueExpr::OptionMap { .. }
        | ValueExpr::OptionAndThen { .. } => {
            unreachable!("Result/Option expressions are emitted before the main expression match")
        }
        ValueExpr::EnvGet { name } => {
            out.push_str("nomo_env_get(");
            emit_expr(out, name);
            out.push(')');
        }
        ValueExpr::EnvSet { name, value } => {
            out.push_str("nomo_env_set(");
            emit_expr(out, name);
            out.push_str(", ");
            emit_expr(out, value);
            out.push(')');
        }
        ValueExpr::EnvCwd => out.push_str("nomo_env_cwd()"),
        ValueExpr::EnvHomeDir => out.push_str("nomo_env_home_dir()"),
        ValueExpr::EnvTempDir => out.push_str("nomo_env_temp_dir()"),
        ValueExpr::EnvArgs => out.push_str("nomo_env_args(nomo_argc, nomo_argv)"),
        ValueExpr::ArrayNew { .. }
        | ValueExpr::ArrayLen { .. }
        | ValueExpr::ArrayIter { .. }
        | ValueExpr::ArrayGet { .. }
        | ValueExpr::ArrayPop { .. }
        | ValueExpr::ArrayRemove { .. }
        | ValueExpr::ArrayPush { .. }
        | ValueExpr::ArraySet { .. }
        | ValueExpr::ArrayInsert { .. }
        | ValueExpr::ArrayClear { .. } => {
            unreachable!("array expressions are emitted before the main expression match")
        }
    }
}

fn emit_lvalue_path(out: &mut String, path: &[String]) {
    let Some((root, fields)) = path.split_first() else {
        return;
    };
    out.push_str(&c_var_ident(root));
    for field in fields {
        out.push('.');
        out.push_str(&c_member_ident(field));
    }
}

fn emit_match_expr(out: &mut String, value: &ValueExpr, arms: &[MatchValueArm]) {
    emit_match_arm(out, value, arms, 0);
}

fn emit_match_arm(out: &mut String, value: &ValueExpr, arms: &[MatchValueArm], index: usize) {
    let arm = &arms[index];
    if index + 1 == arms.len() {
        emit_expr(out, &arm.value);
        return;
    }
    out.push('(');
    emit_expr(out, value);
    out.push_str(".tag == ");
    out.push_str(&c_enum_variant_ident(
        &arm.enum_name,
        &arm.enum_args,
        &arm.variant,
    ));
    out.push_str(" ? ");
    emit_expr(out, &arm.value);
    out.push_str(" : ");
    emit_match_arm(out, value, arms, index + 1);
    out.push(')');
}
