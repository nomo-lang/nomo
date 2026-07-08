use super::*;

pub(super) fn uses_fs_read_to_string(program: &Program) -> bool {
    program.functions.iter().any(|function| {
        function
            .body
            .iter()
            .any(|statement| statement_uses_fs_read_to_string(statement))
    })
}

pub(super) fn uses_fs_write_string(program: &Program) -> bool {
    program.functions.iter().any(|function| {
        function
            .body
            .iter()
            .any(|statement| statement_uses_fs_write_string(statement))
    })
}

pub(super) fn uses_fs_read_bytes(program: &Program) -> bool {
    program.functions.iter().any(|function| {
        function
            .body
            .iter()
            .any(|statement| statement_contains_expr(statement, expr_is_fs_read_bytes))
    })
}

pub(super) fn uses_fs_write_bytes(program: &Program) -> bool {
    program.functions.iter().any(|function| {
        function
            .body
            .iter()
            .any(|statement| statement_contains_expr(statement, expr_is_fs_write_bytes))
    })
}

pub(super) fn uses_fs_exists(program: &Program) -> bool {
    program.functions.iter().any(|function| {
        function
            .body
            .iter()
            .any(|statement| statement_contains_expr(statement, expr_is_fs_exists))
    })
}

pub(super) fn uses_fs_metadata(program: &Program) -> bool {
    program.functions.iter().any(|function| {
        function
            .body
            .iter()
            .any(|statement| statement_contains_expr(statement, expr_is_fs_metadata))
    })
}

pub(super) fn uses_fs_create_dir(program: &Program) -> bool {
    program.functions.iter().any(|function| {
        function
            .body
            .iter()
            .any(|statement| statement_contains_expr(statement, expr_is_fs_create_dir))
    })
}

pub(super) fn uses_fs_remove_dir(program: &Program) -> bool {
    program.functions.iter().any(|function| {
        function
            .body
            .iter()
            .any(|statement| statement_contains_expr(statement, expr_is_fs_remove_dir))
    })
}

pub(super) fn uses_fs_read_dir(program: &Program) -> bool {
    program.functions.iter().any(|function| {
        function
            .body
            .iter()
            .any(|statement| statement_contains_expr(statement, expr_is_fs_read_dir))
    })
}

pub(super) fn uses_fs_open(program: &Program) -> bool {
    program.functions.iter().any(|function| {
        function
            .body
            .iter()
            .any(|statement| statement_uses_fs_open(statement))
    })
}

pub(super) fn uses_file_read_to_string(program: &Program) -> bool {
    program.functions.iter().any(|function| {
        function
            .body
            .iter()
            .any(|statement| statement_contains_expr(statement, expr_is_file_read_to_string))
    })
}

pub(super) fn uses_file_write_string(program: &Program) -> bool {
    program.functions.iter().any(|function| {
        function
            .body
            .iter()
            .any(|statement| statement_contains_expr(statement, expr_is_file_write_string))
    })
}

pub(super) fn uses_file_close(program: &Program) -> bool {
    program.functions.iter().any(|function| {
        function
            .body
            .iter()
            .any(|statement| statement_contains_expr(statement, expr_is_file_close))
    })
}

pub(super) fn uses_net_connect(program: &Program) -> bool {
    program.functions.iter().any(|function| {
        function
            .body
            .iter()
            .any(|statement| statement_contains_expr(statement, expr_is_net_connect))
    })
}

pub(super) fn uses_net_listen(program: &Program) -> bool {
    program.functions.iter().any(|function| {
        function
            .body
            .iter()
            .any(|statement| statement_contains_expr(statement, expr_is_net_listen))
    })
}

pub(super) fn uses_net_udp_bind(program: &Program) -> bool {
    program.functions.iter().any(|function| {
        function
            .body
            .iter()
            .any(|statement| statement_contains_expr(statement, expr_is_net_udp_bind))
    })
}

pub(super) fn uses_http_client(program: &Program) -> bool {
    program.functions.iter().any(|function| {
        function
            .body
            .iter()
            .any(|statement| statement_contains_expr(statement, expr_is_http_client_call))
    })
}

pub(super) fn uses_http_server(program: &Program) -> bool {
    program.functions.iter().any(|function| {
        function
            .body
            .iter()
            .any(|statement| statement_contains_expr(statement, expr_is_http_server_call))
    })
}

pub(super) fn uses_tcp_listener_accept(program: &Program) -> bool {
    program.functions.iter().any(|function| {
        function
            .body
            .iter()
            .any(|statement| statement_contains_expr(statement, expr_is_tcp_listener_accept))
    })
}

pub(super) fn uses_tcp_listener_close(program: &Program) -> bool {
    program.functions.iter().any(|function| {
        function
            .body
            .iter()
            .any(|statement| statement_contains_expr(statement, expr_is_tcp_listener_close))
    })
}

pub(super) fn uses_tcp_stream_read_to_string(program: &Program) -> bool {
    program.functions.iter().any(|function| {
        function
            .body
            .iter()
            .any(|statement| statement_contains_expr(statement, expr_is_tcp_stream_read_to_string))
    })
}

pub(super) fn uses_tcp_stream_write_string(program: &Program) -> bool {
    program.functions.iter().any(|function| {
        function
            .body
            .iter()
            .any(|statement| statement_contains_expr(statement, expr_is_tcp_stream_write_string))
    })
}

pub(super) fn uses_tcp_stream_close(program: &Program) -> bool {
    program.functions.iter().any(|function| {
        function
            .body
            .iter()
            .any(|statement| statement_contains_expr(statement, expr_is_tcp_stream_close))
    })
}

pub(super) fn uses_udp_socket_recv_from_string(program: &Program) -> bool {
    program.functions.iter().any(|function| {
        function.body.iter().any(|statement| {
            statement_contains_expr(statement, expr_is_udp_socket_recv_from_string)
        })
    })
}

pub(super) fn uses_udp_socket_send_to_string(program: &Program) -> bool {
    program.functions.iter().any(|function| {
        function
            .body
            .iter()
            .any(|statement| statement_contains_expr(statement, expr_is_udp_socket_send_to_string))
    })
}

pub(super) fn uses_udp_socket_close(program: &Program) -> bool {
    program.functions.iter().any(|function| {
        function
            .body
            .iter()
            .any(|statement| statement_contains_expr(statement, expr_is_udp_socket_close))
    })
}

pub(super) fn uses_io_read_line(program: &Program) -> bool {
    program
        .functions
        .iter()
        .any(|function| function.body.iter().any(statement_uses_io_read_line))
}

pub(super) fn uses_log_enabled(program: &Program) -> bool {
    program.functions.iter().any(|function| {
        function
            .body
            .iter()
            .any(|statement| statement_contains_expr(statement, expr_is_log_enabled))
    })
}

pub(super) fn uses_hash_builtin(program: &Program) -> bool {
    program.functions.iter().any(|function| {
        function
            .body
            .iter()
            .any(|statement| statement_contains_expr(statement, expr_is_hash_builtin))
    })
}

pub(super) fn uses_crypto_builtin(program: &Program) -> bool {
    program.functions.iter().any(|function| {
        function
            .body
            .iter()
            .any(|statement| statement_contains_expr(statement, expr_is_crypto_builtin))
    })
}

pub(super) fn uses_json_builtin(program: &Program) -> bool {
    program.functions.iter().any(|function| {
        function
            .body
            .iter()
            .any(|statement| statement_contains_expr(statement, expr_is_json_builtin))
    })
}

pub(super) fn uses_regex_builtin(program: &Program) -> bool {
    program.functions.iter().any(|function| {
        function
            .body
            .iter()
            .any(|statement| statement_contains_expr(statement, expr_is_regex_builtin))
    })
}

pub(super) fn uses_collections_builtin(program: &Program) -> bool {
    program.functions.iter().any(|function| {
        function
            .body
            .iter()
            .any(|statement| statement_contains_expr(statement, expr_is_collections_builtin))
    })
}

pub(super) fn uses_num_parse_i64(program: &Program) -> bool {
    program.functions.iter().any(|function| {
        function
            .body
            .iter()
            .any(|statement| statement_contains_expr(statement, expr_is_num_parse_i64))
    })
}

pub(super) fn uses_num_parse_u64(program: &Program) -> bool {
    program.functions.iter().any(|function| {
        function
            .body
            .iter()
            .any(|statement| statement_contains_expr(statement, expr_is_num_parse_u64))
    })
}

pub(super) fn uses_num_parse_f64(program: &Program) -> bool {
    program.functions.iter().any(|function| {
        function
            .body
            .iter()
            .any(|statement| statement_contains_expr(statement, expr_is_num_parse_f64))
    })
}

pub(super) fn uses_env_get(program: &Program) -> bool {
    program.functions.iter().any(|function| {
        function
            .body
            .iter()
            .any(|statement| statement_uses_env_get(statement))
    })
}

pub(super) fn uses_env_args(program: &Program) -> bool {
    program.functions.iter().any(|function| {
        function
            .body
            .iter()
            .any(|statement| statement_uses_env_args(statement))
    })
}

pub(super) fn uses_env_set(program: &Program) -> bool {
    program
        .functions
        .iter()
        .any(|function| function.body.iter().any(statement_uses_env_set))
}

pub(super) fn uses_env_cwd(program: &Program) -> bool {
    program
        .functions
        .iter()
        .any(|function| function.body.iter().any(statement_uses_env_cwd))
}

pub(super) fn uses_env_home_dir(program: &Program) -> bool {
    program
        .functions
        .iter()
        .any(|function| function.body.iter().any(statement_uses_env_home_dir))
}

pub(super) fn uses_env_temp_dir(program: &Program) -> bool {
    program
        .functions
        .iter()
        .any(|function| function.body.iter().any(statement_uses_env_temp_dir))
}

pub(super) fn uses_process_status(program: &Program) -> bool {
    program.functions.iter().any(|function| {
        function
            .body
            .iter()
            .any(|statement| statement_contains_expr(statement, expr_is_process_status))
    })
}

pub(super) fn uses_process_spawn(program: &Program) -> bool {
    program.functions.iter().any(|function| {
        function
            .body
            .iter()
            .any(|statement| statement_contains_expr(statement, expr_is_process_spawn))
    })
}

pub(super) fn uses_process_exec(program: &Program) -> bool {
    program.functions.iter().any(|function| {
        function
            .body
            .iter()
            .any(|statement| statement_contains_expr(statement, expr_is_process_exec))
    })
}

pub(super) fn uses_process_output(program: &Program) -> bool {
    program.functions.iter().any(|function| {
        function
            .body
            .iter()
            .any(|statement| statement_contains_expr(statement, expr_is_process_output))
    })
}

pub(super) fn statement_uses_fs_read_to_string(statement: &Statement) -> bool {
    match statement {
        Statement::Let { initializer, .. } => expr_uses_fs_read_to_string(initializer),
        Statement::LetIf {
            condition,
            body,
            else_body,
            ..
        } => {
            expr_uses_fs_read_to_string(condition)
                || body.iter().any(statement_uses_fs_read_to_string)
                || else_body.iter().any(statement_uses_fs_read_to_string)
        }
        Statement::LetMatch { value, arms, .. } => {
            expr_uses_fs_read_to_string(value)
                || arms
                    .iter()
                    .any(|arm| arm.body.iter().any(statement_uses_fs_read_to_string))
        }
        Statement::QuestionLet { result_expr, .. } => expr_uses_fs_read_to_string(result_expr),
        Statement::QuestionReturn { result_expr, .. } => expr_uses_fs_read_to_string(result_expr),
        Statement::LetElse {
            value, else_body, ..
        } => {
            expr_uses_fs_read_to_string(value)
                || else_body.iter().any(statement_uses_fs_read_to_string)
        }
        Statement::IfLet {
            value,
            body,
            else_body,
            ..
        } => {
            expr_uses_fs_read_to_string(value)
                || body.iter().any(statement_uses_fs_read_to_string)
                || else_body
                    .as_ref()
                    .is_some_and(|else_body| else_body.iter().any(statement_uses_fs_read_to_string))
        }
        Statement::If {
            condition,
            body,
            else_body,
        } => {
            expr_uses_fs_read_to_string(condition)
                || body.iter().any(statement_uses_fs_read_to_string)
                || else_body.iter().any(statement_uses_fs_read_to_string)
        }
        Statement::Assign { value, .. }
        | Statement::AssignField { value, .. }
        | Statement::Println(value)
        | Statement::Print(value)
        | Statement::Eprintln(value)
        | Statement::Eprint(value)
        | Statement::Panic(value)
        | Statement::Expr(value)
        | Statement::Return(Some(value)) => expr_uses_fs_read_to_string(value),
        Statement::Loop { kind, body } => match kind {
            LoopKind::Infinite => body.iter().any(statement_uses_fs_read_to_string),
            LoopKind::While(condition) => {
                expr_uses_fs_read_to_string(condition)
                    || body.iter().any(statement_uses_fs_read_to_string)
            }
            LoopKind::Iterate { iterable, .. } => {
                expr_uses_fs_read_to_string(iterable)
                    || body.iter().any(statement_uses_fs_read_to_string)
            }
        },
        Statement::Match { value, arms, .. } => {
            expr_uses_fs_read_to_string(value)
                || arms
                    .iter()
                    .any(|arm| arm.body.iter().any(statement_uses_fs_read_to_string))
        }
        Statement::Defer { call } => deferred_uses_fs_read_to_string(call),
        Statement::Break | Statement::Continue => false,
        Statement::Return(None) => false,
    }
}

pub(super) fn statement_uses_fs_write_string(statement: &Statement) -> bool {
    match statement {
        Statement::Let { initializer, .. } => expr_uses_fs_write_string(initializer),
        Statement::LetIf {
            condition,
            body,
            else_body,
            ..
        } => {
            expr_uses_fs_write_string(condition)
                || body.iter().any(statement_uses_fs_write_string)
                || else_body.iter().any(statement_uses_fs_write_string)
        }
        Statement::LetMatch { value, arms, .. } => {
            expr_uses_fs_write_string(value)
                || arms
                    .iter()
                    .any(|arm| arm.body.iter().any(statement_uses_fs_write_string))
        }
        Statement::QuestionLet { result_expr, .. } => expr_uses_fs_write_string(result_expr),
        Statement::QuestionReturn { result_expr, .. } => expr_uses_fs_write_string(result_expr),
        Statement::LetElse {
            value, else_body, ..
        } => {
            expr_uses_fs_write_string(value) || else_body.iter().any(statement_uses_fs_write_string)
        }
        Statement::IfLet {
            value,
            body,
            else_body,
            ..
        } => {
            expr_uses_fs_write_string(value)
                || body.iter().any(statement_uses_fs_write_string)
                || else_body
                    .as_ref()
                    .is_some_and(|else_body| else_body.iter().any(statement_uses_fs_write_string))
        }
        Statement::If {
            condition,
            body,
            else_body,
        } => {
            expr_uses_fs_write_string(condition)
                || body.iter().any(statement_uses_fs_write_string)
                || else_body.iter().any(statement_uses_fs_write_string)
        }
        Statement::Assign { value, .. }
        | Statement::AssignField { value, .. }
        | Statement::Println(value)
        | Statement::Print(value)
        | Statement::Eprintln(value)
        | Statement::Eprint(value)
        | Statement::Panic(value)
        | Statement::Expr(value)
        | Statement::Return(Some(value)) => expr_uses_fs_write_string(value),
        Statement::Loop { kind, body } => match kind {
            LoopKind::Infinite => body.iter().any(statement_uses_fs_write_string),
            LoopKind::While(condition) => {
                expr_uses_fs_write_string(condition)
                    || body.iter().any(statement_uses_fs_write_string)
            }
            LoopKind::Iterate { iterable, .. } => {
                expr_uses_fs_write_string(iterable)
                    || body.iter().any(statement_uses_fs_write_string)
            }
        },
        Statement::Match { value, arms, .. } => {
            expr_uses_fs_write_string(value)
                || arms
                    .iter()
                    .any(|arm| arm.body.iter().any(statement_uses_fs_write_string))
        }
        Statement::Defer { call } => deferred_uses_fs_write_string(call),
        Statement::Break | Statement::Continue => false,
        Statement::Return(None) => false,
    }
}

pub(super) fn statement_uses_fs_open(statement: &Statement) -> bool {
    match statement {
        Statement::Let { initializer, .. } => expr_uses_fs_open(initializer),
        Statement::LetIf {
            condition,
            body,
            else_body,
            ..
        } => {
            expr_uses_fs_open(condition)
                || body.iter().any(statement_uses_fs_open)
                || else_body.iter().any(statement_uses_fs_open)
        }
        Statement::LetMatch { value, arms, .. } => {
            expr_uses_fs_open(value)
                || arms
                    .iter()
                    .any(|arm| arm.body.iter().any(statement_uses_fs_open))
        }
        Statement::QuestionLet { result_expr, .. } => expr_uses_fs_open(result_expr),
        Statement::QuestionReturn { result_expr, .. } => expr_uses_fs_open(result_expr),
        Statement::LetElse {
            value, else_body, ..
        } => expr_uses_fs_open(value) || else_body.iter().any(statement_uses_fs_open),
        Statement::IfLet {
            value,
            body,
            else_body,
            ..
        } => {
            expr_uses_fs_open(value)
                || body.iter().any(statement_uses_fs_open)
                || else_body
                    .as_ref()
                    .is_some_and(|else_body| else_body.iter().any(statement_uses_fs_open))
        }
        Statement::If {
            condition,
            body,
            else_body,
        } => {
            expr_uses_fs_open(condition)
                || body.iter().any(statement_uses_fs_open)
                || else_body.iter().any(statement_uses_fs_open)
        }
        Statement::Assign { value, .. }
        | Statement::AssignField { value, .. }
        | Statement::Println(value)
        | Statement::Print(value)
        | Statement::Eprintln(value)
        | Statement::Eprint(value)
        | Statement::Panic(value)
        | Statement::Expr(value)
        | Statement::Return(Some(value)) => expr_uses_fs_open(value),
        Statement::Loop { kind, body } => match kind {
            LoopKind::Infinite => body.iter().any(statement_uses_fs_open),
            LoopKind::While(condition) => {
                expr_uses_fs_open(condition) || body.iter().any(statement_uses_fs_open)
            }
            LoopKind::Iterate { iterable, .. } => {
                expr_uses_fs_open(iterable) || body.iter().any(statement_uses_fs_open)
            }
        },
        Statement::Match { value, arms, .. } => {
            expr_uses_fs_open(value)
                || arms
                    .iter()
                    .any(|arm| arm.body.iter().any(statement_uses_fs_open))
        }
        Statement::Defer { call } => deferred_uses_fs_open(call),
        Statement::Break | Statement::Continue => false,
        Statement::Return(None) => false,
    }
}

pub(super) fn statement_uses_env_set(statement: &Statement) -> bool {
    statement_contains_expr(statement, expr_is_env_set)
}

pub(super) fn statement_uses_env_cwd(statement: &Statement) -> bool {
    statement_contains_expr(statement, expr_is_env_cwd)
}

pub(super) fn statement_uses_env_home_dir(statement: &Statement) -> bool {
    statement_contains_expr(statement, expr_is_env_home_dir)
}

pub(super) fn statement_uses_env_temp_dir(statement: &Statement) -> bool {
    statement_contains_expr(statement, expr_is_env_temp_dir)
}

pub(super) fn statement_uses_io_read_line(statement: &Statement) -> bool {
    statement_contains_expr(statement, expr_is_io_read_line)
}

pub(super) fn push_enum_instance(
    seen: &mut BTreeSet<String>,
    out: &mut Vec<(String, Vec<ValueType>)>,
    name: &str,
    args: &[ValueType],
) {
    let key = format!("{name}{}", c_type_suffix(args));
    if seen.insert(key) {
        out.push((name.to_string(), args.to_vec()));
    }
}

pub(super) fn push_struct_instance(
    seen: &mut BTreeSet<String>,
    out: &mut Vec<(String, Vec<ValueType>)>,
    name: &str,
    args: &[ValueType],
) {
    let key = format!("{name}{}", c_type_suffix(args));
    if seen.insert(key) {
        out.push((name.to_string(), args.to_vec()));
    }
}
