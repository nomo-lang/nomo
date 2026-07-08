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

pub(super) fn statement_contains_expr(
    statement: &Statement,
    predicate: fn(&ValueExpr) -> bool,
) -> bool {
    match statement {
        Statement::Let { initializer, .. } => expr_contains(initializer, predicate),
        Statement::LetIf {
            condition,
            body,
            else_body,
            ..
        } => {
            expr_contains(condition, predicate)
                || body
                    .iter()
                    .any(|statement| statement_contains_expr(statement, predicate))
                || else_body
                    .iter()
                    .any(|statement| statement_contains_expr(statement, predicate))
        }
        Statement::LetMatch { value, arms, .. } => {
            expr_contains(value, predicate)
                || arms.iter().any(|arm| {
                    arm.body
                        .iter()
                        .any(|statement| statement_contains_expr(statement, predicate))
                })
        }
        Statement::QuestionLet { result_expr, .. }
        | Statement::QuestionReturn { result_expr, .. } => expr_contains(result_expr, predicate),
        Statement::LetElse {
            value, else_body, ..
        } => {
            expr_contains(value, predicate)
                || else_body
                    .iter()
                    .any(|statement| statement_contains_expr(statement, predicate))
        }
        Statement::IfLet {
            value,
            body,
            else_body,
            ..
        } => {
            expr_contains(value, predicate)
                || body
                    .iter()
                    .any(|statement| statement_contains_expr(statement, predicate))
                || else_body.as_ref().is_some_and(|else_body| {
                    else_body
                        .iter()
                        .any(|statement| statement_contains_expr(statement, predicate))
                })
        }
        Statement::If {
            condition,
            body,
            else_body,
        } => {
            expr_contains(condition, predicate)
                || body
                    .iter()
                    .any(|statement| statement_contains_expr(statement, predicate))
                || else_body
                    .iter()
                    .any(|statement| statement_contains_expr(statement, predicate))
        }
        Statement::Assign { value, .. }
        | Statement::AssignField { value, .. }
        | Statement::Println(value)
        | Statement::Print(value)
        | Statement::Eprintln(value)
        | Statement::Eprint(value)
        | Statement::Panic(value)
        | Statement::Expr(value)
        | Statement::Return(Some(value)) => expr_contains(value, predicate),
        Statement::Loop { kind, body } => match kind {
            LoopKind::Infinite => body
                .iter()
                .any(|statement| statement_contains_expr(statement, predicate)),
            LoopKind::While(condition) => {
                expr_contains(condition, predicate)
                    || body
                        .iter()
                        .any(|statement| statement_contains_expr(statement, predicate))
            }
            LoopKind::Iterate { iterable, .. } => {
                expr_contains(iterable, predicate)
                    || body
                        .iter()
                        .any(|statement| statement_contains_expr(statement, predicate))
            }
        },
        Statement::Match { value, arms, .. } => {
            expr_contains(value, predicate)
                || arms.iter().any(|arm| {
                    arm.body
                        .iter()
                        .any(|statement| statement_contains_expr(statement, predicate))
                })
        }
        Statement::Defer { call } => deferred_contains_expr(call, predicate),
        Statement::Break | Statement::Continue | Statement::Return(None) => false,
    }
}

pub(super) fn deferred_contains_expr(
    call: &DeferredCall,
    predicate: fn(&ValueExpr) -> bool,
) -> bool {
    match call {
        DeferredCall::Expr(expr)
        | DeferredCall::Println(expr)
        | DeferredCall::Print(expr)
        | DeferredCall::Eprintln(expr)
        | DeferredCall::Eprint(expr) => expr_contains(expr, predicate),
    }
}

pub(super) fn expr_contains(expr: &ValueExpr, predicate: fn(&ValueExpr) -> bool) -> bool {
    if predicate(expr) {
        return true;
    }
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
        } => expr_contains(left, predicate) || expr_contains(right, predicate),
        ValueExpr::UdpSocketSendToString {
            socket,
            content,
            host,
            port,
        } => {
            expr_contains(socket, predicate)
                || expr_contains(content, predicate)
                || expr_contains(host, predicate)
                || expr_contains(port, predicate)
        }
        ValueExpr::FsWriteString { path, content } => {
            expr_contains(path, predicate) || expr_contains(content, predicate)
        }
        ValueExpr::FsWriteBytes { path, bytes } => {
            expr_contains(path, predicate) || expr_contains(bytes, predicate)
        }
        ValueExpr::EnvSet { name, value } => {
            expr_contains(name, predicate) || expr_contains(value, predicate)
        }
        ValueExpr::HashWriteString { state, value } => {
            expr_contains(state, predicate) || expr_contains(value, predicate)
        }
        ValueExpr::HashWriteBytes { state, value } => {
            expr_contains(state, predicate) || expr_contains(value, predicate)
        }
        ValueExpr::CollectionsStringMapSet { map, key, value } => {
            expr_contains(map, predicate)
                || expr_contains(key, predicate)
                || expr_contains(value, predicate)
        }
        ValueExpr::Call { args, .. } => args.iter().any(|arg| expr_contains(arg, predicate)),
        ValueExpr::ArrayGet { array, index, .. } => {
            expr_contains(array, predicate) || expr_contains(index, predicate)
        }
        ValueExpr::ArrayPop { .. } | ValueExpr::ArrayClear { .. } => false,
        ValueExpr::ArrayRemove { index, .. } => expr_contains(index, predicate),
        ValueExpr::ArraySet { index, value, .. } => {
            expr_contains(index, predicate) || expr_contains(value, predicate)
        }
        ValueExpr::ArrayPush { value, .. } => expr_contains(value, predicate),
        ValueExpr::ArrayInsert { index, value, .. } => {
            expr_contains(index, predicate) || expr_contains(value, predicate)
        }
        ValueExpr::FsReadToString { path }
        | ValueExpr::FsReadBytes { path }
        | ValueExpr::FsOpen { path }
        | ValueExpr::FsExists { path }
        | ValueExpr::FsMetadata { path }
        | ValueExpr::FsCreateDir { path }
        | ValueExpr::FsRemoveDir { path }
        | ValueExpr::FsReadDir { path }
        | ValueExpr::FileClose { file: path }
        | ValueExpr::FileReadToString { file: path }
        | ValueExpr::TcpListenerAccept { listener: path }
        | ValueExpr::TcpListenerClose { listener: path }
        | ValueExpr::TcpStreamClose { stream: path }
        | ValueExpr::TcpStreamReadToString { stream: path }
        | ValueExpr::UdpSocketClose { socket: path }
        | ValueExpr::EnvGet { name: path }
        | ValueExpr::StringLen { value: path }
        | ValueExpr::StringIsEmpty { value: path }
        | ValueExpr::StringTrim { value: path }
        | ValueExpr::StringToLower { value: path }
        | ValueExpr::StringToUpper { value: path }
        | ValueExpr::CharIsDigit { value: path }
        | ValueExpr::CharIsAlpha { value: path }
        | ValueExpr::CharIsWhitespace { value: path }
        | ValueExpr::CharToString { value: path }
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
        | ValueExpr::Unary { expr: path, .. }
        | ValueExpr::Cast { expr: path, .. }
        | ValueExpr::ResultIsOk { result: path, .. }
        | ValueExpr::ResultIsErr { result: path, .. }
        | ValueExpr::ResultMap { result: path, .. }
        | ValueExpr::ResultAndThen { result: path, .. }
        | ValueExpr::OptionIsSome { option: path, .. }
        | ValueExpr::OptionIsNone { option: path, .. }
        | ValueExpr::OptionMap { option: path, .. }
        | ValueExpr::OptionAndThen { option: path, .. }
        | ValueExpr::EnumPayload { value: path, .. }
        | ValueExpr::EnumPayloadFieldAccess { value: path, .. }
        | ValueExpr::ArrayIter { array: path, .. }
        | ValueExpr::ArrayLen { array: path } => expr_contains(path, predicate),
        ValueExpr::ResultUnwrapOr {
            result, default, ..
        } => expr_contains(result, predicate) || expr_contains(default, predicate),
        ValueExpr::OptionUnwrapOr {
            option, default, ..
        } => expr_contains(option, predicate) || expr_contains(default, predicate),
        ValueExpr::FileWriteString { file, content } => {
            expr_contains(file, predicate) || expr_contains(content, predicate)
        }
        ValueExpr::StructLiteral { fields, .. } => fields
            .iter()
            .any(|(_, value)| expr_contains(value, predicate)),
        ValueExpr::EnumVariant { payload, .. } => payload
            .as_ref()
            .is_some_and(|payload| expr_contains(payload, predicate)),
        ValueExpr::If {
            condition,
            then_branch,
            else_branch,
        } => {
            expr_contains(condition, predicate)
                || expr_contains(then_branch, predicate)
                || expr_contains(else_branch, predicate)
        }
        ValueExpr::Panic { message, .. } => expr_contains(message, predicate),
        ValueExpr::Match { value, arms } => {
            expr_contains(value, predicate)
                || arms.iter().any(|arm| expr_contains(&arm.value, predicate))
        }
        ValueExpr::ResultMapErr { result, .. } => expr_contains(result, predicate),
        ValueExpr::StringLiteral(_)
        | ValueExpr::IntLiteral(_)
        | ValueExpr::FloatLiteral(_)
        | ValueExpr::CharLiteral(_)
        | ValueExpr::BoolLiteral(_)
        | ValueExpr::VoidLiteral
        | ValueExpr::HashNew
        | ValueExpr::CollectionsStringMapNew
        | ValueExpr::CollectionsStringSetNew
        | ValueExpr::Variable(_)
        | ValueExpr::MutBorrow(_)
        | ValueExpr::EnvArgs
        | ValueExpr::IoReadLine
        | ValueExpr::EnvCwd
        | ValueExpr::EnvHomeDir
        | ValueExpr::EnvTempDir
        | ValueExpr::OsPlatform
        | ValueExpr::OsArch
        | ValueExpr::OsPathSeparator
        | ValueExpr::OsLineEnding
        | ValueExpr::TimeNowMillis
        | ValueExpr::TimeMonotonicMillis
        | ValueExpr::ArrayNew { .. }
        | ValueExpr::FieldAccess { .. } => false,
    }
}

pub(super) fn expr_is_env_set(expr: &ValueExpr) -> bool {
    matches!(expr, ValueExpr::EnvSet { .. })
}

pub(super) fn expr_is_process_status(expr: &ValueExpr) -> bool {
    matches!(expr, ValueExpr::ProcessStatus { .. })
}

pub(super) fn expr_is_process_spawn(expr: &ValueExpr) -> bool {
    matches!(expr, ValueExpr::ProcessSpawn { .. })
}

pub(super) fn expr_is_process_exec(expr: &ValueExpr) -> bool {
    matches!(expr, ValueExpr::ProcessExec { .. })
}

pub(super) fn expr_is_process_output(expr: &ValueExpr) -> bool {
    matches!(expr, ValueExpr::ProcessOutput { .. })
}

pub(super) fn expr_is_net_connect(expr: &ValueExpr) -> bool {
    matches!(expr, ValueExpr::NetConnect { .. })
}

pub(super) fn expr_is_net_listen(expr: &ValueExpr) -> bool {
    matches!(expr, ValueExpr::NetListen { .. })
}

pub(super) fn expr_is_net_udp_bind(expr: &ValueExpr) -> bool {
    matches!(expr, ValueExpr::NetUdpBind { .. })
}

pub(super) fn expr_is_http_client_call(expr: &ValueExpr) -> bool {
    matches!(
        expr,
        ValueExpr::Call { name, .. }
            if name == BUILTIN_HTTP_GET_EXPR || name == BUILTIN_HTTP_POST_EXPR
    )
}

pub(super) fn expr_is_http_server_call(expr: &ValueExpr) -> bool {
    matches!(
        expr,
        ValueExpr::Call { name, .. }
            if name == BUILTIN_HTTP_LISTEN_EXPR
                || name == BUILTIN_HTTP_ACCEPT_EXPR
                || name == BUILTIN_HTTP_RESPOND_STRING_EXPR
                || name == BUILTIN_HTTP_CLOSE_SERVER_EXPR
                || name == BUILTIN_HTTP_CLOSE_EXCHANGE_EXPR
    )
}

pub(super) fn expr_is_tcp_listener_accept(expr: &ValueExpr) -> bool {
    matches!(expr, ValueExpr::TcpListenerAccept { .. })
}

pub(super) fn expr_is_tcp_listener_close(expr: &ValueExpr) -> bool {
    matches!(expr, ValueExpr::TcpListenerClose { .. })
}

pub(super) fn expr_is_tcp_stream_read_to_string(expr: &ValueExpr) -> bool {
    matches!(expr, ValueExpr::TcpStreamReadToString { .. })
}

pub(super) fn expr_is_tcp_stream_write_string(expr: &ValueExpr) -> bool {
    matches!(expr, ValueExpr::TcpStreamWriteString { .. })
}

pub(super) fn expr_is_tcp_stream_close(expr: &ValueExpr) -> bool {
    matches!(expr, ValueExpr::TcpStreamClose { .. })
}

pub(super) fn expr_is_udp_socket_recv_from_string(expr: &ValueExpr) -> bool {
    matches!(expr, ValueExpr::UdpSocketRecvFromString { .. })
}

pub(super) fn expr_is_udp_socket_send_to_string(expr: &ValueExpr) -> bool {
    matches!(expr, ValueExpr::UdpSocketSendToString { .. })
}

pub(super) fn expr_is_udp_socket_close(expr: &ValueExpr) -> bool {
    matches!(expr, ValueExpr::UdpSocketClose { .. })
}

pub(super) fn expr_is_fs_exists(expr: &ValueExpr) -> bool {
    matches!(expr, ValueExpr::FsExists { .. })
}

pub(super) fn expr_is_fs_metadata(expr: &ValueExpr) -> bool {
    matches!(expr, ValueExpr::FsMetadata { .. })
}

pub(super) fn expr_is_fs_create_dir(expr: &ValueExpr) -> bool {
    matches!(expr, ValueExpr::FsCreateDir { .. })
}

pub(super) fn expr_is_fs_remove_dir(expr: &ValueExpr) -> bool {
    matches!(expr, ValueExpr::FsRemoveDir { .. })
}

pub(super) fn expr_is_fs_read_dir(expr: &ValueExpr) -> bool {
    matches!(expr, ValueExpr::FsReadDir { .. })
}

pub(super) fn expr_is_fs_read_bytes(expr: &ValueExpr) -> bool {
    matches!(expr, ValueExpr::FsReadBytes { .. })
}

pub(super) fn expr_is_fs_write_bytes(expr: &ValueExpr) -> bool {
    matches!(expr, ValueExpr::FsWriteBytes { .. })
}

pub(super) fn expr_is_file_read_to_string(expr: &ValueExpr) -> bool {
    matches!(expr, ValueExpr::FileReadToString { .. })
}

pub(super) fn expr_is_file_write_string(expr: &ValueExpr) -> bool {
    matches!(expr, ValueExpr::FileWriteString { .. })
}

pub(super) fn expr_is_file_close(expr: &ValueExpr) -> bool {
    matches!(expr, ValueExpr::FileClose { .. })
}

pub(super) fn expr_is_io_read_line(expr: &ValueExpr) -> bool {
    matches!(expr, ValueExpr::IoReadLine)
}

pub(super) fn expr_is_log_enabled(expr: &ValueExpr) -> bool {
    matches!(expr, ValueExpr::LogEnabled { .. })
}

pub(super) fn expr_is_hash_builtin(expr: &ValueExpr) -> bool {
    matches!(
        expr,
        ValueExpr::HashNew
            | ValueExpr::HashString { .. }
            | ValueExpr::HashBytes { .. }
            | ValueExpr::HashWriteString { .. }
            | ValueExpr::HashWriteBytes { .. }
            | ValueExpr::HashFinish { .. }
    )
}

pub(super) fn expr_is_crypto_builtin(expr: &ValueExpr) -> bool {
    matches!(
        expr,
        ValueExpr::CryptoSha256 { .. }
            | ValueExpr::CryptoSha512 { .. }
            | ValueExpr::CryptoRandomBytes { .. }
    )
}

pub(super) fn expr_is_json_builtin(expr: &ValueExpr) -> bool {
    matches!(
        expr,
        ValueExpr::JsonParse { .. } | ValueExpr::JsonStringify { .. }
    )
}

pub(super) fn expr_is_regex_builtin(expr: &ValueExpr) -> bool {
    matches!(
        expr,
        ValueExpr::RegexCompile { .. }
            | ValueExpr::RegexIsMatch { .. }
            | ValueExpr::RegexCaptures { .. }
    )
}

pub(super) fn expr_is_collections_builtin(expr: &ValueExpr) -> bool {
    matches!(
        expr,
        ValueExpr::CollectionsStringMapNew
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
    )
}

pub(super) fn expr_is_num_parse_i64(expr: &ValueExpr) -> bool {
    matches!(expr, ValueExpr::NumParseI64 { .. })
}

pub(super) fn expr_is_num_parse_u64(expr: &ValueExpr) -> bool {
    matches!(expr, ValueExpr::NumParseU64 { .. })
}

pub(super) fn expr_is_num_parse_f64(expr: &ValueExpr) -> bool {
    matches!(expr, ValueExpr::NumParseF64 { .. })
}

pub(super) fn expr_is_env_cwd(expr: &ValueExpr) -> bool {
    matches!(expr, ValueExpr::EnvCwd)
}

pub(super) fn expr_is_env_home_dir(expr: &ValueExpr) -> bool {
    matches!(expr, ValueExpr::EnvHomeDir)
}

pub(super) fn expr_is_env_temp_dir(expr: &ValueExpr) -> bool {
    matches!(expr, ValueExpr::EnvTempDir)
}

pub(super) fn statement_uses_env_get(statement: &Statement) -> bool {
    match statement {
        Statement::Let { initializer, .. } => expr_uses_env_get(initializer),
        Statement::LetIf {
            condition,
            body,
            else_body,
            ..
        } => {
            expr_uses_env_get(condition)
                || body.iter().any(statement_uses_env_get)
                || else_body.iter().any(statement_uses_env_get)
        }
        Statement::LetMatch { value, arms, .. } => {
            expr_uses_env_get(value)
                || arms
                    .iter()
                    .any(|arm| arm.body.iter().any(statement_uses_env_get))
        }
        Statement::QuestionLet { result_expr, .. } => expr_uses_env_get(result_expr),
        Statement::QuestionReturn { result_expr, .. } => expr_uses_env_get(result_expr),
        Statement::LetElse {
            value, else_body, ..
        } => expr_uses_env_get(value) || else_body.iter().any(statement_uses_env_get),
        Statement::IfLet {
            value,
            body,
            else_body,
            ..
        } => {
            expr_uses_env_get(value)
                || body.iter().any(statement_uses_env_get)
                || else_body
                    .as_ref()
                    .is_some_and(|else_body| else_body.iter().any(statement_uses_env_get))
        }
        Statement::If {
            condition,
            body,
            else_body,
        } => {
            expr_uses_env_get(condition)
                || body.iter().any(statement_uses_env_get)
                || else_body.iter().any(statement_uses_env_get)
        }
        Statement::Assign { value, .. }
        | Statement::AssignField { value, .. }
        | Statement::Println(value)
        | Statement::Print(value)
        | Statement::Eprintln(value)
        | Statement::Eprint(value)
        | Statement::Panic(value)
        | Statement::Expr(value)
        | Statement::Return(Some(value)) => expr_uses_env_get(value),
        Statement::Loop { kind, body } => match kind {
            LoopKind::Infinite => body.iter().any(statement_uses_env_get),
            LoopKind::While(condition) => {
                expr_uses_env_get(condition) || body.iter().any(statement_uses_env_get)
            }
            LoopKind::Iterate { iterable, .. } => {
                expr_uses_env_get(iterable) || body.iter().any(statement_uses_env_get)
            }
        },
        Statement::Match { value, arms, .. } => {
            expr_uses_env_get(value)
                || arms
                    .iter()
                    .any(|arm| arm.body.iter().any(statement_uses_env_get))
        }
        Statement::Defer { call } => deferred_uses_env_get(call),
        Statement::Break | Statement::Continue => false,
        Statement::Return(None) => false,
    }
}

pub(super) fn statement_uses_env_args(statement: &Statement) -> bool {
    match statement {
        Statement::Let { initializer, .. } => expr_uses_env_args(initializer),
        Statement::LetIf {
            condition,
            body,
            else_body,
            ..
        } => {
            expr_uses_env_args(condition)
                || body.iter().any(statement_uses_env_args)
                || else_body.iter().any(statement_uses_env_args)
        }
        Statement::LetMatch { value, arms, .. } => {
            expr_uses_env_args(value)
                || arms
                    .iter()
                    .any(|arm| arm.body.iter().any(statement_uses_env_args))
        }
        Statement::QuestionLet { result_expr, .. } => expr_uses_env_args(result_expr),
        Statement::QuestionReturn { result_expr, .. } => expr_uses_env_args(result_expr),
        Statement::LetElse {
            value, else_body, ..
        } => expr_uses_env_args(value) || else_body.iter().any(statement_uses_env_args),
        Statement::IfLet {
            value,
            body,
            else_body,
            ..
        } => {
            expr_uses_env_args(value)
                || body.iter().any(statement_uses_env_args)
                || else_body
                    .as_ref()
                    .is_some_and(|else_body| else_body.iter().any(statement_uses_env_args))
        }
        Statement::If {
            condition,
            body,
            else_body,
        } => {
            expr_uses_env_args(condition)
                || body.iter().any(statement_uses_env_args)
                || else_body.iter().any(statement_uses_env_args)
        }
        Statement::Assign { value, .. }
        | Statement::AssignField { value, .. }
        | Statement::Println(value)
        | Statement::Print(value)
        | Statement::Eprintln(value)
        | Statement::Eprint(value)
        | Statement::Panic(value)
        | Statement::Expr(value)
        | Statement::Return(Some(value)) => expr_uses_env_args(value),
        Statement::Loop { kind, body } => match kind {
            LoopKind::Infinite => body.iter().any(statement_uses_env_args),
            LoopKind::While(condition) => {
                expr_uses_env_args(condition) || body.iter().any(statement_uses_env_args)
            }
            LoopKind::Iterate { iterable, .. } => {
                expr_uses_env_args(iterable) || body.iter().any(statement_uses_env_args)
            }
        },
        Statement::Match { value, arms, .. } => {
            expr_uses_env_args(value)
                || arms
                    .iter()
                    .any(|arm| arm.body.iter().any(statement_uses_env_args))
        }
        Statement::Defer { call } => deferred_uses_env_args(call),
        Statement::Break | Statement::Continue => false,
        Statement::Return(None) => false,
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

pub(super) fn deferred_uses_fs_read_to_string(call: &DeferredCall) -> bool {
    match call {
        DeferredCall::Expr(expr)
        | DeferredCall::Println(expr)
        | DeferredCall::Print(expr)
        | DeferredCall::Eprintln(expr)
        | DeferredCall::Eprint(expr) => expr_uses_fs_read_to_string(expr),
    }
}

pub(super) fn deferred_uses_fs_write_string(call: &DeferredCall) -> bool {
    match call {
        DeferredCall::Expr(expr)
        | DeferredCall::Println(expr)
        | DeferredCall::Print(expr)
        | DeferredCall::Eprintln(expr)
        | DeferredCall::Eprint(expr) => expr_uses_fs_write_string(expr),
    }
}

pub(super) fn deferred_uses_fs_open(call: &DeferredCall) -> bool {
    match call {
        DeferredCall::Expr(expr)
        | DeferredCall::Println(expr)
        | DeferredCall::Print(expr)
        | DeferredCall::Eprintln(expr)
        | DeferredCall::Eprint(expr) => expr_uses_fs_open(expr),
    }
}

pub(super) fn deferred_uses_env_get(call: &DeferredCall) -> bool {
    match call {
        DeferredCall::Expr(expr)
        | DeferredCall::Println(expr)
        | DeferredCall::Print(expr)
        | DeferredCall::Eprintln(expr)
        | DeferredCall::Eprint(expr) => expr_uses_env_get(expr),
    }
}

pub(super) fn deferred_uses_env_args(call: &DeferredCall) -> bool {
    match call {
        DeferredCall::Expr(expr)
        | DeferredCall::Println(expr)
        | DeferredCall::Print(expr)
        | DeferredCall::Eprintln(expr)
        | DeferredCall::Eprint(expr) => expr_uses_env_args(expr),
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

pub(super) fn expr_uses_fs_read_to_string(expr: &ValueExpr) -> bool {
    match expr {
        ValueExpr::FsReadToString { .. } => true,
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
        } => expr_uses_fs_read_to_string(left) || expr_uses_fs_read_to_string(right),
        ValueExpr::UdpSocketSendToString {
            socket,
            content,
            host,
            port,
        } => {
            expr_uses_fs_read_to_string(socket)
                || expr_uses_fs_read_to_string(content)
                || expr_uses_fs_read_to_string(host)
                || expr_uses_fs_read_to_string(port)
        }
        ValueExpr::FsWriteString { path, content } => {
            expr_uses_fs_read_to_string(path) || expr_uses_fs_read_to_string(content)
        }
        ValueExpr::FsWriteBytes { path, bytes } => {
            expr_uses_fs_read_to_string(path) || expr_uses_fs_read_to_string(bytes)
        }
        ValueExpr::EnvSet { name, value } => {
            expr_uses_fs_read_to_string(name) || expr_uses_fs_read_to_string(value)
        }
        ValueExpr::FsExists { path }
        | ValueExpr::FsReadBytes { path }
        | ValueExpr::FsMetadata { path }
        | ValueExpr::FsCreateDir { path }
        | ValueExpr::FsRemoveDir { path }
        | ValueExpr::FsReadDir { path }
        | ValueExpr::FsOpen { path }
        | ValueExpr::FileClose { file: path }
        | ValueExpr::FileReadToString { file: path }
        | ValueExpr::TcpListenerAccept { listener: path }
        | ValueExpr::TcpListenerClose { listener: path }
        | ValueExpr::TcpStreamClose { stream: path }
        | ValueExpr::TcpStreamReadToString { stream: path }
        | ValueExpr::UdpSocketClose { socket: path } => expr_uses_fs_read_to_string(path),
        ValueExpr::FileWriteString { file, content } => {
            expr_uses_fs_read_to_string(file) || expr_uses_fs_read_to_string(content)
        }
        ValueExpr::ResultMapErr { result, .. }
        | ValueExpr::ResultIsOk { result, .. }
        | ValueExpr::ResultIsErr { result, .. }
        | ValueExpr::ResultMap { result, .. }
        | ValueExpr::ResultAndThen { result, .. }
        | ValueExpr::OptionIsSome { option: result, .. }
        | ValueExpr::OptionIsNone { option: result, .. }
        | ValueExpr::OptionMap { option: result, .. }
        | ValueExpr::OptionAndThen { option: result, .. } => expr_uses_fs_read_to_string(result),
        ValueExpr::ResultUnwrapOr {
            result, default, ..
        } => expr_uses_fs_read_to_string(result) || expr_uses_fs_read_to_string(default),
        ValueExpr::OptionUnwrapOr {
            option, default, ..
        } => expr_uses_fs_read_to_string(option) || expr_uses_fs_read_to_string(default),
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
        | ValueExpr::NumToString { value: name, .. } => expr_uses_fs_read_to_string(name),
        ValueExpr::EnvArgs => false,
        ValueExpr::ArrayNew { .. }
        | ValueExpr::HashNew
        | ValueExpr::CollectionsStringMapNew
        | ValueExpr::CollectionsStringSetNew => false,
        ValueExpr::HashWriteString { state, value } => {
            expr_uses_fs_read_to_string(state) || expr_uses_fs_read_to_string(value)
        }
        ValueExpr::HashWriteBytes { state, value } => {
            expr_uses_fs_read_to_string(state) || expr_uses_fs_read_to_string(value)
        }
        ValueExpr::CollectionsStringMapSet { map, key, value } => {
            expr_uses_fs_read_to_string(map)
                || expr_uses_fs_read_to_string(key)
                || expr_uses_fs_read_to_string(value)
        }
        ValueExpr::ArrayLen { array } => expr_uses_fs_read_to_string(array),
        ValueExpr::ArrayIter { array, .. } => expr_uses_fs_read_to_string(array),
        ValueExpr::ArrayGet { array, index, .. } => {
            expr_uses_fs_read_to_string(array) || expr_uses_fs_read_to_string(index)
        }
        ValueExpr::ArrayPop { .. } | ValueExpr::ArrayClear { .. } => false,
        ValueExpr::ArrayRemove { index, .. } => expr_uses_fs_read_to_string(index),
        ValueExpr::ArrayPush { value, .. } => expr_uses_fs_read_to_string(value),
        ValueExpr::ArraySet { index, value, .. } => {
            expr_uses_fs_read_to_string(index) || expr_uses_fs_read_to_string(value)
        }
        ValueExpr::ArrayInsert { index, value, .. } => {
            expr_uses_fs_read_to_string(index) || expr_uses_fs_read_to_string(value)
        }
        ValueExpr::Call { args, .. } => args.iter().any(expr_uses_fs_read_to_string),
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
        | ValueExpr::EnumPayload { value, .. } => expr_uses_fs_read_to_string(value),
        ValueExpr::StructLiteral { fields, .. } => fields
            .iter()
            .any(|(_, value)| expr_uses_fs_read_to_string(value)),
        ValueExpr::EnumVariant { payload, .. } => {
            payload.as_deref().is_some_and(expr_uses_fs_read_to_string)
        }
        ValueExpr::If {
            condition,
            then_branch,
            else_branch,
        } => {
            expr_uses_fs_read_to_string(condition)
                || expr_uses_fs_read_to_string(then_branch)
                || expr_uses_fs_read_to_string(else_branch)
        }
        ValueExpr::Panic { message, .. } => expr_uses_fs_read_to_string(message),
        ValueExpr::Match { value, arms } => {
            expr_uses_fs_read_to_string(value)
                || arms
                    .iter()
                    .any(|arm| expr_uses_fs_read_to_string(&arm.value))
        }
        ValueExpr::StringLiteral(_)
        | ValueExpr::IntLiteral(_)
        | ValueExpr::FloatLiteral(_)
        | ValueExpr::CharLiteral(_)
        | ValueExpr::BoolLiteral(_)
        | ValueExpr::VoidLiteral
        | ValueExpr::Variable(_)
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
        ValueExpr::EnumPayloadFieldAccess { value, .. } => expr_uses_fs_read_to_string(value),
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

pub(super) fn expr_uses_env_get(expr: &ValueExpr) -> bool {
    match expr {
        ValueExpr::EnvGet { .. } => true,
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
        } => expr_uses_env_get(left) || expr_uses_env_get(right),
        ValueExpr::UdpSocketSendToString {
            socket,
            content,
            host,
            port,
        } => {
            expr_uses_env_get(socket)
                || expr_uses_env_get(content)
                || expr_uses_env_get(host)
                || expr_uses_env_get(port)
        }
        ValueExpr::FsReadToString { path }
        | ValueExpr::FsReadBytes { path }
        | ValueExpr::FsExists { path }
        | ValueExpr::FsMetadata { path }
        | ValueExpr::FsCreateDir { path }
        | ValueExpr::FsRemoveDir { path }
        | ValueExpr::FsReadDir { path }
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
        | ValueExpr::FileReadToString { file: path }
        | ValueExpr::TcpListenerAccept { listener: path }
        | ValueExpr::TcpListenerClose { listener: path }
        | ValueExpr::TcpStreamClose { stream: path }
        | ValueExpr::TcpStreamReadToString { stream: path }
        | ValueExpr::UdpSocketClose { socket: path } => expr_uses_env_get(path),
        ValueExpr::FsWriteString { path, content } => {
            expr_uses_env_get(path) || expr_uses_env_get(content)
        }
        ValueExpr::FsWriteBytes { path, bytes } => {
            expr_uses_env_get(path) || expr_uses_env_get(bytes)
        }
        ValueExpr::FileWriteString { file, content } => {
            expr_uses_env_get(file) || expr_uses_env_get(content)
        }
        ValueExpr::EnvSet { name, value } => expr_uses_env_get(name) || expr_uses_env_get(value),
        ValueExpr::HashWriteString { state, value } => {
            expr_uses_env_get(state) || expr_uses_env_get(value)
        }
        ValueExpr::HashWriteBytes { state, value } => {
            expr_uses_env_get(state) || expr_uses_env_get(value)
        }
        ValueExpr::CollectionsStringMapSet { map, key, value } => {
            expr_uses_env_get(map) || expr_uses_env_get(key) || expr_uses_env_get(value)
        }
        ValueExpr::FsOpen { path } | ValueExpr::FileClose { file: path } => expr_uses_env_get(path),
        ValueExpr::ResultMapErr { result, .. }
        | ValueExpr::ResultIsOk { result, .. }
        | ValueExpr::ResultIsErr { result, .. }
        | ValueExpr::ResultMap { result, .. }
        | ValueExpr::ResultAndThen { result, .. }
        | ValueExpr::OptionIsSome { option: result, .. }
        | ValueExpr::OptionIsNone { option: result, .. }
        | ValueExpr::OptionMap { option: result, .. }
        | ValueExpr::OptionAndThen { option: result, .. } => expr_uses_env_get(result),
        ValueExpr::ResultUnwrapOr {
            result, default, ..
        } => expr_uses_env_get(result) || expr_uses_env_get(default),
        ValueExpr::OptionUnwrapOr {
            option, default, ..
        } => expr_uses_env_get(option) || expr_uses_env_get(default),
        ValueExpr::EnvArgs => false,
        ValueExpr::ArrayNew { .. }
        | ValueExpr::CollectionsStringMapNew
        | ValueExpr::CollectionsStringSetNew => false,
        ValueExpr::ArrayLen { array } => expr_uses_env_get(array),
        ValueExpr::ArrayIter { array, .. } => expr_uses_env_get(array),
        ValueExpr::ArrayGet { array, index, .. } => {
            expr_uses_env_get(array) || expr_uses_env_get(index)
        }
        ValueExpr::ArrayPop { .. } | ValueExpr::ArrayClear { .. } => false,
        ValueExpr::ArrayRemove { index, .. } => expr_uses_env_get(index),
        ValueExpr::ArrayPush { value, .. } => expr_uses_env_get(value),
        ValueExpr::ArraySet { index, value, .. } => {
            expr_uses_env_get(index) || expr_uses_env_get(value)
        }
        ValueExpr::ArrayInsert { index, value, .. } => {
            expr_uses_env_get(index) || expr_uses_env_get(value)
        }
        ValueExpr::Call { args, .. } => args.iter().any(expr_uses_env_get),
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
        | ValueExpr::EnumPayloadFieldAccess { value, .. } => expr_uses_env_get(value),
        ValueExpr::StructLiteral { fields, .. } => {
            fields.iter().any(|(_, value)| expr_uses_env_get(value))
        }
        ValueExpr::EnumVariant { payload, .. } => payload.as_deref().is_some_and(expr_uses_env_get),
        ValueExpr::If {
            condition,
            then_branch,
            else_branch,
        } => {
            expr_uses_env_get(condition)
                || expr_uses_env_get(then_branch)
                || expr_uses_env_get(else_branch)
        }
        ValueExpr::Panic { message, .. } => expr_uses_env_get(message),
        ValueExpr::Match { value, arms } => {
            expr_uses_env_get(value) || arms.iter().any(|arm| expr_uses_env_get(&arm.value))
        }
        ValueExpr::StringLiteral(_)
        | ValueExpr::IntLiteral(_)
        | ValueExpr::FloatLiteral(_)
        | ValueExpr::CharLiteral(_)
        | ValueExpr::BoolLiteral(_)
        | ValueExpr::VoidLiteral
        | ValueExpr::HashNew
        | ValueExpr::Variable(_)
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
    }
}

pub(super) fn expr_uses_env_args(expr: &ValueExpr) -> bool {
    match expr {
        ValueExpr::EnvArgs => true,
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
        } => expr_uses_env_args(left) || expr_uses_env_args(right),
        ValueExpr::UdpSocketSendToString {
            socket,
            content,
            host,
            port,
        } => {
            expr_uses_env_args(socket)
                || expr_uses_env_args(content)
                || expr_uses_env_args(host)
                || expr_uses_env_args(port)
        }
        ValueExpr::FsReadToString { path }
        | ValueExpr::FsReadBytes { path }
        | ValueExpr::FsOpen { path }
        | ValueExpr::FsExists { path }
        | ValueExpr::FsMetadata { path }
        | ValueExpr::FsCreateDir { path }
        | ValueExpr::FsRemoveDir { path }
        | ValueExpr::FsReadDir { path }
        | ValueExpr::FileClose { file: path }
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
        | ValueExpr::UdpSocketClose { socket: path } => expr_uses_env_args(path),
        ValueExpr::ResultMapErr { result, .. }
        | ValueExpr::ResultIsOk { result, .. }
        | ValueExpr::ResultIsErr { result, .. }
        | ValueExpr::ResultMap { result, .. }
        | ValueExpr::ResultAndThen { result, .. }
        | ValueExpr::OptionIsSome { option: result, .. }
        | ValueExpr::OptionIsNone { option: result, .. }
        | ValueExpr::OptionMap { option: result, .. }
        | ValueExpr::OptionAndThen { option: result, .. } => expr_uses_env_args(result),
        ValueExpr::ResultUnwrapOr {
            result, default, ..
        } => expr_uses_env_args(result) || expr_uses_env_args(default),
        ValueExpr::OptionUnwrapOr {
            option, default, ..
        } => expr_uses_env_args(option) || expr_uses_env_args(default),
        ValueExpr::FsWriteString { path, content } => {
            expr_uses_env_args(path) || expr_uses_env_args(content)
        }
        ValueExpr::FsWriteBytes { path, bytes } => {
            expr_uses_env_args(path) || expr_uses_env_args(bytes)
        }
        ValueExpr::FileWriteString { file, content } => {
            expr_uses_env_args(file) || expr_uses_env_args(content)
        }
        ValueExpr::EnvSet { name, value } => expr_uses_env_args(name) || expr_uses_env_args(value),
        ValueExpr::HashWriteString { state, value } => {
            expr_uses_env_args(state) || expr_uses_env_args(value)
        }
        ValueExpr::HashWriteBytes { state, value } => {
            expr_uses_env_args(state) || expr_uses_env_args(value)
        }
        ValueExpr::CollectionsStringMapSet { map, key, value } => {
            expr_uses_env_args(map) || expr_uses_env_args(key) || expr_uses_env_args(value)
        }
        ValueExpr::ArrayIter { array, .. } => expr_uses_env_args(array),
        ValueExpr::ArrayGet { array, index, .. } => {
            expr_uses_env_args(array) || expr_uses_env_args(index)
        }
        ValueExpr::ArrayPop { .. } | ValueExpr::ArrayClear { .. } => false,
        ValueExpr::ArrayRemove { index, .. } => expr_uses_env_args(index),
        ValueExpr::ArrayPush { value, .. } => expr_uses_env_args(value),
        ValueExpr::ArraySet { index, value, .. } => {
            expr_uses_env_args(index) || expr_uses_env_args(value)
        }
        ValueExpr::ArrayInsert { index, value, .. } => {
            expr_uses_env_args(index) || expr_uses_env_args(value)
        }
        ValueExpr::Call { args, .. } => args.iter().any(expr_uses_env_args),
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
        | ValueExpr::EnumPayloadFieldAccess { value, .. } => expr_uses_env_args(value),
        ValueExpr::StructLiteral { fields, .. } => {
            fields.iter().any(|(_, value)| expr_uses_env_args(value))
        }
        ValueExpr::EnumVariant { payload, .. } => {
            payload.as_deref().is_some_and(expr_uses_env_args)
        }
        ValueExpr::If {
            condition,
            then_branch,
            else_branch,
        } => {
            expr_uses_env_args(condition)
                || expr_uses_env_args(then_branch)
                || expr_uses_env_args(else_branch)
        }
        ValueExpr::Panic { message, .. } => expr_uses_env_args(message),
        ValueExpr::Match { value, arms } => {
            expr_uses_env_args(value) || arms.iter().any(|arm| expr_uses_env_args(&arm.value))
        }
        ValueExpr::ArrayNew { .. }
        | ValueExpr::HashNew
        | ValueExpr::CollectionsStringMapNew
        | ValueExpr::CollectionsStringSetNew
        | ValueExpr::StringLiteral(_)
        | ValueExpr::IntLiteral(_)
        | ValueExpr::FloatLiteral(_)
        | ValueExpr::CharLiteral(_)
        | ValueExpr::BoolLiteral(_)
        | ValueExpr::VoidLiteral
        | ValueExpr::Variable(_)
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
