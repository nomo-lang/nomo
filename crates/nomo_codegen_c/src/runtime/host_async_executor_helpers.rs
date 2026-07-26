use super::*;

pub(super) fn function_uses_async_runtime(function: &Function) -> bool {
    function.body.iter().any(|statement| {
        statement_task_select(statement).is_some()
            || statement_contains_expr(statement, |expr| {
                expr_is_async_yield(expr)
                    || expr_is_async_sleep(expr)
                    || expr_is_async_tcp_connect(expr)
                    || expr_is_async_tcp_io(expr)
                    || expr_is_async_channel_send(expr)
                    || expr_is_async_channel_receive(expr)
                    || expr_is_async_check_cancelled(expr)
                    || expr_is_async_deadline_enter(expr)
                    || expr_is_structured_join(expr)
                    || expr_is_structured_cancel(expr)
            })
    })
}

pub(super) fn collect_async_function_names(program: &Program) -> BTreeSet<String> {
    let mut names = program
        .functions
        .iter()
        .filter(|function| function.is_suspend && function_uses_async_runtime(function))
        .map(|function| function.name.clone())
        .collect::<BTreeSet<_>>();
    for function in &program.functions {
        for statement in &function.body {
            if let Some(spawn) = statement_structured_spawn(statement) {
                names.insert(spawn.callee.to_string());
            }
        }
    }

    loop {
        let discovered = program
            .functions
            .iter()
            .filter(|function| function.is_suspend && !names.contains(&function.name))
            .filter(|function| {
                function.body.iter().any(|statement| {
                    statement_contains_expr(statement, |expr| match expr {
                        ValueExpr::Call { name, .. } if names.contains(name) => true,
                        ValueExpr::Call { name, .. }
                            if name.starts_with(BUILTIN_TASK_STRUCTURED_SPAWN_PREFIX) =>
                        {
                            names.contains(
                                name.trim_start_matches(BUILTIN_TASK_STRUCTURED_SPAWN_PREFIX),
                            )
                        }
                        _ => false,
                    })
                })
            })
            .map(|function| function.name.clone())
            .collect::<Vec<_>>();
        if discovered.is_empty() {
            break;
        }
        names.extend(discovered);
    }
    names
}

pub(super) fn ordered_async_functions<'a>(
    program: &'a Program,
    async_names: &BTreeSet<String>,
) -> Vec<&'a Function> {
    fn visit<'a>(
        function: &'a Function,
        functions: &HashMap<&str, &'a Function>,
        async_names: &BTreeSet<String>,
        visiting: &mut BTreeSet<String>,
        visited: &mut BTreeSet<String>,
        ordered: &mut Vec<&'a Function>,
    ) {
        if visited.contains(&function.name) {
            return;
        }
        assert!(
            visiting.insert(function.name.clone()),
            "recursive suspend call graphs must be rejected before C code generation"
        );
        for statement in &function.body {
            let callee = statement_async_call(statement, async_names)
                .map(|call| call.callee)
                .or_else(|| statement_structured_spawn(statement).map(|spawn| spawn.callee));
            if let Some(child) = callee.and_then(|callee| functions.get(callee)) {
                visit(child, functions, async_names, visiting, visited, ordered);
            }
        }
        visiting.remove(&function.name);
        visited.insert(function.name.clone());
        ordered.push(function);
    }

    let functions = program
        .functions
        .iter()
        .map(|function| (function.name.as_str(), function))
        .collect::<HashMap<_, _>>();
    let mut visiting = BTreeSet::new();
    let mut visited = BTreeSet::new();
    let mut ordered = Vec::new();
    for function in program
        .functions
        .iter()
        .filter(|function| async_names.contains(&function.name))
    {
        visit(
            function,
            &functions,
            async_names,
            &mut visiting,
            &mut visited,
            &mut ordered,
        );
    }
    ordered
}

fn expr_is_async_yield(expr: &ValueExpr) -> bool {
    matches!(
        expr,
        ValueExpr::Call { name, args }
            if name == BUILTIN_TASK_YIELD_EXPR && args.is_empty()
    )
}

fn expr_is_async_sleep(expr: &ValueExpr) -> bool {
    matches!(
        expr,
        ValueExpr::Call { name, args }
            if name == BUILTIN_TASK_SLEEP_EXPR && args.len() == 1
    )
}

fn expr_is_async_tcp_connect(expr: &ValueExpr) -> bool {
    matches!(
        expr,
        ValueExpr::Call { name, args }
            if name == BUILTIN_NET_CONNECT_EXPR && args.len() == 3
    )
}

fn expr_is_async_tcp_io(expr: &ValueExpr) -> bool {
    matches!(
        expr,
        ValueExpr::Call { name, args }
            if matches!(
                name.as_str(),
                BUILTIN_TCP_STREAM_READ_EXPR
                    | BUILTIN_TCP_STREAM_READ_STRING_EXPR
                    | BUILTIN_TCP_STREAM_WRITE_EXPR
                    | BUILTIN_TCP_STREAM_WRITE_STRING_EXPR
            ) && args.len() == 3
    )
}

fn expr_is_async_channel_send(expr: &ValueExpr) -> bool {
    matches!(
        expr,
        ValueExpr::Call { name, args }
            if name.starts_with(BUILTIN_TASK_SEND_PREFIX) && args.len() == 2
    )
}

fn expr_is_async_channel_receive(expr: &ValueExpr) -> bool {
    matches!(
        expr,
        ValueExpr::Call { name, args }
            if name.starts_with(BUILTIN_TASK_RECEIVE_PREFIX) && args.len() == 1
    )
}

fn expr_is_async_check_cancelled(expr: &ValueExpr) -> bool {
    matches!(
        expr,
        ValueExpr::Call { name, args }
            if name == BUILTIN_TASK_CHECK_CANCELLED_EXPR && args.is_empty()
    )
}

fn expr_is_async_deadline_enter(expr: &ValueExpr) -> bool {
    matches!(
        expr,
        ValueExpr::Call { name, args }
            if name == BUILTIN_TASK_DEADLINE_ENTER_EXPR && args.len() == 1
    )
}

fn expr_is_structured_join(expr: &ValueExpr) -> bool {
    matches!(
        expr,
        ValueExpr::Call { name, args }
            if name == BUILTIN_TASK_STRUCTURED_JOIN_EXPR && args.len() == 1
    )
}

fn expr_is_structured_cancel(expr: &ValueExpr) -> bool {
    structured_cancel_handle(expr).is_some()
}

fn structured_cancel_handle(expr: &ValueExpr) -> Option<&str> {
    match expr {
        ValueExpr::Call { name, args } if name == BUILTIN_TASK_STRUCTURED_CANCEL_EXPR => {
            let [ValueExpr::Variable(handle)] = args.as_slice() else {
                return None;
            };
            Some(handle)
        }
        _ => None,
    }
}

fn statement_is_async_yield(statement: &Statement) -> bool {
    matches!(
        statement,
        Statement::Expr(ValueExpr::Call { name, args })
            if name == BUILTIN_TASK_YIELD_EXPR && args.is_empty()
    )
}

fn statement_is_async_check_cancelled(statement: &Statement) -> bool {
    matches!(
        statement,
        Statement::Expr(ValueExpr::Call { name, args })
            if name == BUILTIN_TASK_CHECK_CANCELLED_EXPR && args.is_empty()
    )
}

fn statement_async_deadline_enter(statement: &Statement) -> Option<&ValueExpr> {
    let Statement::Expr(ValueExpr::Call { name, args }) = statement else {
        return None;
    };
    if name != BUILTIN_TASK_DEADLINE_ENTER_EXPR {
        return None;
    }
    let [duration] = args.as_slice() else {
        return None;
    };
    Some(duration)
}

fn statement_is_async_deadline_exit(statement: &Statement) -> bool {
    matches!(
        statement,
        Statement::Expr(ValueExpr::Call { name, args })
            if name == BUILTIN_TASK_DEADLINE_EXIT_EXPR && args.is_empty()
    )
}

fn function_has_async_deadline(function: &Function) -> bool {
    function
        .body
        .iter()
        .any(|statement| statement_async_deadline_enter(statement).is_some())
}

fn statement_is_within_async_deadline(function: &Function, index: usize) -> bool {
    let Some(enter) = function
        .body
        .iter()
        .position(|statement| statement_async_deadline_enter(statement).is_some())
    else {
        return false;
    };
    let exit = function
        .body
        .iter()
        .position(statement_is_async_deadline_exit)
        .expect("validated deadline enter has one exit marker");
    index > enter && index <= exit
}

fn statement_async_sleep(statement: &Statement) -> Option<(&str, &ValueType, &ValueExpr)> {
    match statement {
        Statement::Let {
            name,
            value_type,
            initializer: ValueExpr::Call { name: call, args },
        } if call == BUILTIN_TASK_SLEEP_EXPR => {
            let [duration] = args.as_slice() else {
                return None;
            };
            Some((name, value_type, duration))
        }
        _ => None,
    }
}

#[derive(Debug, Clone, Copy)]
struct AsyncTcpConnect<'a> {
    host: &'a ValueExpr,
    port: &'a ValueExpr,
    timeout_millis: &'a ValueExpr,
    binding: &'a str,
    value_type: &'a ValueType,
}

fn statement_async_tcp_connect(statement: &Statement) -> Option<AsyncTcpConnect<'_>> {
    let Statement::Let {
        name: binding,
        value_type,
        initializer: ValueExpr::Call { name, args },
    } = statement
    else {
        return None;
    };
    if name != BUILTIN_NET_CONNECT_EXPR {
        return None;
    }
    let [host, port, timeout_millis] = args.as_slice() else {
        return None;
    };
    Some(AsyncTcpConnect {
        host,
        port,
        timeout_millis,
        binding,
        value_type,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AsyncTcpIoKind {
    Read,
    ReadString,
    Write,
    WriteString,
}

impl AsyncTcpIoKind {
    fn start_function(self) -> &'static str {
        match self {
            Self::Read => "nomo_async_tcp_read_start",
            Self::ReadString => "nomo_async_tcp_read_string_start",
            Self::Write => "nomo_async_tcp_write_start",
            Self::WriteString => "nomo_async_tcp_write_string_start",
        }
    }

    fn resume_function(self) -> &'static str {
        match self {
            Self::Read => "nomo_async_tcp_read_resume",
            Self::ReadString => "nomo_async_tcp_read_string_resume",
            Self::Write => "nomo_async_tcp_write_resume",
            Self::WriteString => "nomo_async_tcp_write_string_resume",
        }
    }

    fn payload_type(self) -> Option<ValueType> {
        match self {
            Self::Read | Self::ReadString => None,
            Self::Write => Some(ValueType::Array(Box::new(ValueType::U32))),
            Self::WriteString => Some(ValueType::String),
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct AsyncTcpIo<'a> {
    kind: AsyncTcpIoKind,
    stream: &'a ValueExpr,
    value: &'a ValueExpr,
    timeout_millis: &'a ValueExpr,
    binding: &'a str,
    value_type: &'a ValueType,
}

fn statement_async_tcp_io(statement: &Statement) -> Option<AsyncTcpIo<'_>> {
    let Statement::Let {
        name: binding,
        value_type,
        initializer: ValueExpr::Call { name, args },
    } = statement
    else {
        return None;
    };
    let kind = match name.as_str() {
        BUILTIN_TCP_STREAM_READ_EXPR => AsyncTcpIoKind::Read,
        BUILTIN_TCP_STREAM_READ_STRING_EXPR => AsyncTcpIoKind::ReadString,
        BUILTIN_TCP_STREAM_WRITE_EXPR => AsyncTcpIoKind::Write,
        BUILTIN_TCP_STREAM_WRITE_STRING_EXPR => AsyncTcpIoKind::WriteString,
        _ => return None,
    };
    let [stream, value, timeout_millis] = args.as_slice() else {
        return None;
    };
    Some(AsyncTcpIo {
        kind,
        stream,
        value,
        timeout_millis,
        binding,
        value_type,
    })
}

#[derive(Debug, Clone, Copy)]
struct AsyncCall<'a> {
    callee: &'a str,
    args: &'a [ValueExpr],
    binding: Option<(&'a str, &'a ValueType)>,
}

#[derive(Debug, Clone, Copy)]
struct StructuredSpawn<'a> {
    callee: &'a str,
    args: &'a [ValueExpr],
    handle: &'a str,
}

#[derive(Debug, Clone, Copy)]
struct StructuredJoin<'a> {
    handle: &'a str,
    binding: &'a str,
    value_type: &'a ValueType,
}

#[derive(Debug, Clone, Copy)]
struct StructuredCancel<'a> {
    handle: &'a str,
}

#[derive(Debug, Clone, Copy)]
struct StructuredCancelJoin<'a> {
    handle: &'a str,
    binding: &'a str,
    value_type: &'a ValueType,
}

#[derive(Debug, Clone, Copy)]
struct AsyncChannelSend<'a> {
    suffix: &'a str,
    channel: &'a ValueExpr,
    value: &'a ValueExpr,
    binding: &'a str,
    value_type: &'a ValueType,
}

#[derive(Debug, Clone, Copy)]
struct AsyncChannelReceive<'a> {
    suffix: &'a str,
    channel: &'a ValueExpr,
    binding: &'a str,
    value_type: &'a ValueType,
}

fn statement_async_channel_send(statement: &Statement) -> Option<AsyncChannelSend<'_>> {
    let Statement::Let {
        name: binding,
        value_type,
        initializer: ValueExpr::Call { name, args },
    } = statement
    else {
        return None;
    };
    let suffix = name.strip_prefix(BUILTIN_TASK_SEND_PREFIX)?;
    let [channel, value] = args.as_slice() else {
        return None;
    };
    Some(AsyncChannelSend {
        suffix,
        channel,
        value,
        binding,
        value_type,
    })
}

fn statement_async_channel_receive(statement: &Statement) -> Option<AsyncChannelReceive<'_>> {
    let Statement::Let {
        name: binding,
        value_type,
        initializer: ValueExpr::Call { name, args },
    } = statement
    else {
        return None;
    };
    let suffix = name.strip_prefix(BUILTIN_TASK_RECEIVE_PREFIX)?;
    let [channel] = args.as_slice() else {
        return None;
    };
    Some(AsyncChannelReceive {
        suffix,
        channel,
        binding,
        value_type,
    })
}

fn statement_task_select(statement: &Statement) -> Option<&[TaskSelectArm]> {
    match statement {
        Statement::TaskSelect { arms } => Some(arms),
        _ => None,
    }
}

fn statement_structured_spawn(statement: &Statement) -> Option<StructuredSpawn<'_>> {
    let Statement::Let {
        name: handle,
        initializer: ValueExpr::Call { name, args },
        ..
    } = statement
    else {
        return None;
    };
    let callee = name.strip_prefix(BUILTIN_TASK_STRUCTURED_SPAWN_PREFIX)?;
    Some(StructuredSpawn {
        callee,
        args,
        handle,
    })
}

fn statement_structured_join(statement: &Statement) -> Option<StructuredJoin<'_>> {
    let Statement::Let {
        name: binding,
        value_type,
        initializer: ValueExpr::Call { name, args },
    } = statement
    else {
        return None;
    };
    if name != BUILTIN_TASK_STRUCTURED_JOIN_EXPR {
        return None;
    }
    let [ValueExpr::Variable(handle)] = args.as_slice() else {
        return None;
    };
    Some(StructuredJoin {
        handle,
        binding,
        value_type,
    })
}

fn statement_structured_cancel(statement: &Statement) -> Option<StructuredCancel<'_>> {
    let Statement::Expr(ValueExpr::Call { name, args }) = statement else {
        return None;
    };
    if name != BUILTIN_TASK_STRUCTURED_CANCEL_EXPR {
        return None;
    }
    let [ValueExpr::Variable(handle)] = args.as_slice() else {
        return None;
    };
    Some(StructuredCancel { handle })
}

fn statement_structured_cancel_join(statement: &Statement) -> Option<StructuredCancelJoin<'_>> {
    let Statement::Let {
        name: binding,
        value_type,
        initializer: ValueExpr::Call { name, args },
    } = statement
    else {
        return None;
    };
    if name != BUILTIN_TASK_STRUCTURED_CANCEL_JOIN_EXPR {
        return None;
    }
    let [ValueExpr::Variable(handle)] = args.as_slice() else {
        return None;
    };
    Some(StructuredCancelJoin {
        handle,
        binding,
        value_type,
    })
}

fn emit_async_structured_cancel(
    out: &mut String,
    function: &Function,
    handle: &str,
    indent: usize,
) {
    let spawn_index = structured_spawn_index(function, handle)
        .expect("validated structured cancellation handle has a spawn");
    let spawn = statement_structured_spawn(&function.body[spawn_index])
        .expect("structured cancellation spawn exists");
    write_indent(out, indent);
    out.push_str(&async_cancel_ident(spawn.callee));
    out.push_str("(&frame->");
    out.push_str(&async_child_field(spawn_index));
    out.push_str(", context);\n");
    write_indent(out, indent);
    out.push_str(&async_drop_ident(spawn.callee));
    out.push_str("(&frame->");
    out.push_str(&async_child_field(spawn_index));
    out.push_str(");\n");
}

fn structured_spawn_index(function: &Function, handle: &str) -> Option<usize> {
    function
        .body
        .iter()
        .enumerate()
        .find_map(|(index, statement)| {
            statement_structured_spawn(statement)
                .filter(|spawn| spawn.handle == handle)
                .map(|_| index)
        })
}

fn statement_async_call<'a>(
    statement: &'a Statement,
    async_names: &BTreeSet<String>,
) -> Option<AsyncCall<'a>> {
    match statement {
        Statement::Expr(ValueExpr::Call { name, args }) if async_names.contains(name) => {
            Some(AsyncCall {
                callee: name,
                args,
                binding: None,
            })
        }
        Statement::Let {
            name: binding,
            value_type,
            initializer: ValueExpr::Call { name, args },
        } if async_names.contains(name) => Some(AsyncCall {
            callee: name,
            args,
            binding: Some((binding, value_type)),
        }),
        _ => None,
    }
}

fn statement_is_async_suspend(statement: &Statement, async_names: &BTreeSet<String>) -> bool {
    statement_is_async_yield(statement)
        || statement_async_sleep(statement).is_some()
        || statement_async_tcp_connect(statement).is_some()
        || statement_async_tcp_io(statement).is_some()
        || statement_async_channel_send(statement).is_some()
        || statement_async_channel_receive(statement).is_some()
        || statement_task_select(statement).is_some()
        || statement_async_call(statement, async_names).is_some()
        || statement_structured_join(statement).is_some()
        || statement_structured_cancel_join(statement).is_some()
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AsyncFrameLocal {
    name: String,
    value_type: ValueType,
    declaration_index: usize,
    last_use_index: usize,
}

fn collect_async_frame_locals(
    function: &Function,
    async_names: &BTreeSet<String>,
) -> Vec<AsyncFrameLocal> {
    function
        .body
        .iter()
        .enumerate()
        .filter_map(|(declaration_index, statement)| {
            if statement_structured_spawn(statement).is_some() {
                return None;
            }
            let (name, value_type) = match statement {
                Statement::Let {
                    name, value_type, ..
                }
                | Statement::QuestionLet {
                    name, value_type, ..
                } => (name, value_type),
                _ => return None,
            };
            let last_use_index = function
                .body
                .iter()
                .enumerate()
                .skip(declaration_index + 1)
                .rev()
                .find_map(|(index, statement)| {
                    statement_uses_binding(statement, name).then_some(index)
                })?;
            let crosses_suspend = function.body[declaration_index + 1..last_use_index]
                .iter()
                .any(|statement| statement_is_async_suspend(statement, async_names))
                || task_select_arm_bodies_use_binding(&function.body[last_use_index], name);
            crosses_suspend.then(|| AsyncFrameLocal {
                name: name.clone(),
                value_type: value_type.clone(),
                declaration_index,
                last_use_index,
            })
        })
        .collect()
}

fn task_select_arm_bodies_use_binding(statement: &Statement, binding: &str) -> bool {
    statement_task_select(statement).is_some_and(|arms| {
        arms.iter().any(|arm| {
            arm.body
                .iter()
                .any(|statement| statement_uses_binding(statement, binding))
        })
    })
}

fn statement_uses_binding(statement: &Statement, binding: &str) -> bool {
    statement_contains_expr(statement, |expr| expr_uses_binding(expr, binding))
}

fn expr_uses_binding(expr: &ValueExpr, binding: &str) -> bool {
    match expr {
        ValueExpr::Variable(name) | ValueExpr::FieldAccess { base: name, .. } => name == binding,
        ValueExpr::MutBorrow(path) => path.first().is_some_and(|name| name == binding),
        ValueExpr::ArrayPop { array, .. }
        | ValueExpr::ArrayRemove { array, .. }
        | ValueExpr::ArrayPush { array, .. }
        | ValueExpr::ArraySet { array, .. }
        | ValueExpr::ArrayInsert { array, .. }
        | ValueExpr::ArrayClear { array, .. } => array == binding,
        _ => false,
    }
}

fn async_frame_value_field(name: &str) -> String {
    format!("nomo_async_local_{}", c_var_ident(name))
}

fn async_frame_owned_field(name: &str) -> String {
    format!("nomo_async_owned_{}", c_var_ident(name))
}

fn async_parameter_field(name: &str) -> String {
    format!("nomo_async_parameter_{}", c_var_ident(name))
}

fn async_parameter_owned_field(name: &str) -> String {
    format!("nomo_async_parameter_owned_{}", c_var_ident(name))
}

fn async_result_field() -> &'static str {
    "nomo_async_result"
}

fn async_result_owned_field() -> &'static str {
    "nomo_async_result_owned"
}

fn async_frame_ident(function: &str) -> String {
    format!("nomo_async_frame_{function}")
}

fn async_poll_ident(function: &str) -> String {
    format!("nomo_async_poll_{function}")
}

fn async_cancel_ident(function: &str) -> String {
    format!("nomo_async_cancel_{function}")
}

fn async_drop_ident(function: &str) -> String {
    format!("nomo_async_drop_{function}")
}

fn async_child_field(index: usize) -> String {
    format!("nomo_async_child_{index}")
}

fn async_timer_field(index: usize) -> String {
    format!("nomo_async_timer_{index}")
}

fn async_timer_result_field(index: usize) -> String {
    format!("nomo_async_timer_result_{index}")
}

fn async_timer_outcome_field(index: usize) -> String {
    format!("nomo_async_timer_outcome_{index}")
}

fn async_timer_result_owned_field(index: usize) -> String {
    format!("nomo_async_timer_result_owned_{index}")
}

fn async_tcp_connect_registration_field(index: usize) -> String {
    format!("nomo_async_tcp_connect_registration_{index}")
}

fn async_tcp_connect_result_field(index: usize) -> String {
    format!("nomo_async_tcp_connect_result_{index}")
}

fn async_tcp_connect_result_owned_field(index: usize) -> String {
    format!("nomo_async_tcp_connect_result_owned_{index}")
}

fn async_tcp_connect_host_temp(index: usize) -> String {
    format!("nomo_async_tcp_connect_host_{index}")
}

fn async_tcp_io_registration_field(index: usize) -> String {
    format!("nomo_async_tcp_io_registration_{index}")
}

fn async_tcp_io_result_field(index: usize) -> String {
    format!("nomo_async_tcp_io_result_{index}")
}

fn async_tcp_io_result_owned_field(index: usize) -> String {
    format!("nomo_async_tcp_io_result_owned_{index}")
}

fn async_tcp_io_payload_temp(index: usize) -> String {
    format!("nomo_async_tcp_io_payload_{index}")
}

fn async_tcp_io_start_status_temp(index: usize) -> String {
    format!("nomo_async_tcp_io_start_status_{index}")
}

fn async_join_result_field(index: usize) -> String {
    format!("nomo_async_join_result_{index}")
}

fn async_join_result_owned_field(index: usize) -> String {
    format!("nomo_async_join_result_owned_{index}")
}

fn async_cancel_join_result_field(index: usize) -> String {
    format!("nomo_async_cancel_join_result_{index}")
}

fn async_cancel_join_result_owned_field(index: usize) -> String {
    format!("nomo_async_cancel_join_result_owned_{index}")
}

fn async_spawn_failed_field(index: usize) -> String {
    format!("nomo_async_spawn_failed_{index}")
}

fn async_channel_send_registration_field(index: usize) -> String {
    format!("nomo_async_channel_send_registration_{index}")
}

fn async_channel_receive_registration_field(index: usize) -> String {
    format!("nomo_async_channel_receive_registration_{index}")
}

fn async_channel_result_field(index: usize) -> String {
    format!("nomo_async_channel_result_{index}")
}

fn async_channel_result_owned_field(index: usize) -> String {
    format!("nomo_async_channel_result_owned_{index}")
}

fn async_select_token_field(index: usize) -> String {
    format!("nomo_async_select_token_{index}")
}

fn async_select_receive_registration_field(index: usize, arm: usize) -> String {
    format!("nomo_async_select_receive_registration_{index}_{arm}")
}

fn async_select_timer_field(index: usize, arm: usize) -> String {
    format!("nomo_async_select_timer_{index}_{arm}")
}

fn async_select_timer_outcome_field(index: usize, arm: usize) -> String {
    format!("nomo_async_select_timer_outcome_{index}_{arm}")
}

fn async_select_result_field(index: usize, arm: usize) -> String {
    format!("nomo_async_select_result_{index}_{arm}")
}

fn async_select_result_owned_field(index: usize, arm: usize) -> String {
    format!("nomo_async_select_result_owned_{index}_{arm}")
}

fn async_sleep_result_type() -> ValueType {
    ValueType::Enum(
        "Result".to_string(),
        vec![
            ValueType::Void,
            ValueType::Struct("TaskError".to_string(), Vec::new()),
        ],
    )
}

fn emit_async_frame_type(
    out: &mut String,
    function: &Function,
    frame_locals: &[AsyncFrameLocal],
    async_names: &BTreeSet<String>,
) {
    out.push_str(
        "typedef struct {\n\
             uint32_t state;\n\
             nomo_async_context *context;\n\
             void *structured_waiter_frame;\n\
             nomo_async_poll_fn structured_waiter_poll;\n\
             uint8_t initialized;\n\
             uint8_t started;\n\
             uint8_t dropped;\n\
             uint8_t cancelled;\n\
             uint8_t structured_completed;\n",
    );
    out.push_str("    nomo_async_task_failure structured_failure;\n");
    if function_has_async_deadline(function) {
        out.push_str(
            "    nomo_async_timer_registration nomo_async_deadline_timer;\n\
             nomo_async_timer_outcome nomo_async_deadline_outcome;\n\
             uint8_t nomo_async_deadline_active;\n",
        );
    }
    for parameter in &function.params {
        out.push_str("    ");
        out.push_str(&c_type(&parameter.value_type));
        out.push(' ');
        out.push_str(&async_parameter_field(&parameter.name));
        out.push_str(";\n");
        if value_type_needs_release(&parameter.value_type) {
            out.push_str("    uint8_t ");
            out.push_str(&async_parameter_owned_field(&parameter.name));
            out.push_str(";\n");
        }
    }
    if function.return_type != ValueType::Void {
        out.push_str("    ");
        out.push_str(&c_type(&function.return_type));
        out.push(' ');
        out.push_str(async_result_field());
        out.push_str(";\n");
        if value_type_needs_release(&function.return_type) {
            out.push_str("    uint8_t ");
            out.push_str(async_result_owned_field());
            out.push_str(";\n");
        }
    }
    for local in frame_locals {
        out.push_str("    ");
        out.push_str(&c_type(&local.value_type));
        out.push(' ');
        out.push_str(&async_frame_value_field(&local.name));
        out.push_str(";\n");
        if value_type_needs_release(&local.value_type) {
            out.push_str("    uint8_t ");
            out.push_str(&async_frame_owned_field(&local.name));
            out.push_str(";\n");
        }
    }
    for (index, statement) in function.body.iter().enumerate() {
        if let Some(arms) = statement_task_select(statement) {
            out.push_str("    nomo_async_select_token ");
            out.push_str(&async_select_token_field(index));
            out.push_str(";\n");
            for (arm_index, arm) in arms.iter().enumerate() {
                match &arm.operation {
                    TaskSelectOperation::Receive { element_type, .. } => {
                        out.push_str("    nomo_channel_receive_registration_");
                        out.push_str(&c_type_name_part(element_type));
                        out.push(' ');
                        out.push_str(&async_select_receive_registration_field(index, arm_index));
                        out.push_str(";\n");
                    }
                    TaskSelectOperation::Sleep { .. } => {
                        out.push_str("    nomo_async_timer_registration ");
                        out.push_str(&async_select_timer_field(index, arm_index));
                        out.push_str(";\n    nomo_async_timer_outcome ");
                        out.push_str(&async_select_timer_outcome_field(index, arm_index));
                        out.push_str(";\n");
                    }
                }
                out.push_str("    ");
                out.push_str(&c_type(&arm.binding_type));
                out.push(' ');
                out.push_str(&async_select_result_field(index, arm_index));
                out.push_str(";\n    uint8_t ");
                out.push_str(&async_select_result_owned_field(index, arm_index));
                out.push_str(";\n");
            }
        }
        if let Some(send) = statement_async_channel_send(statement) {
            out.push_str("    nomo_channel_send_registration_");
            out.push_str(send.suffix);
            out.push(' ');
            out.push_str(&async_channel_send_registration_field(index));
            out.push_str(";\n    ");
            out.push_str(&c_type(send.value_type));
            out.push(' ');
            out.push_str(&async_channel_result_field(index));
            out.push_str(";\n    uint8_t ");
            out.push_str(&async_channel_result_owned_field(index));
            out.push_str(";\n");
        }
        if let Some(receive) = statement_async_channel_receive(statement) {
            out.push_str("    nomo_channel_receive_registration_");
            out.push_str(receive.suffix);
            out.push(' ');
            out.push_str(&async_channel_receive_registration_field(index));
            out.push_str(";\n    ");
            out.push_str(&c_type(receive.value_type));
            out.push(' ');
            out.push_str(&async_channel_result_field(index));
            out.push_str(";\n    uint8_t ");
            out.push_str(&async_channel_result_owned_field(index));
            out.push_str(";\n");
        }
        if statement_async_sleep(statement).is_some() {
            out.push_str("    nomo_async_timer_registration ");
            out.push_str(&async_timer_field(index));
            out.push_str(";\n    nomo_async_timer_outcome ");
            out.push_str(&async_timer_outcome_field(index));
            out.push_str(";\n    ");
            out.push_str(&c_type(&async_sleep_result_type()));
            out.push(' ');
            out.push_str(&async_timer_result_field(index));
            out.push_str(";\n    uint8_t ");
            out.push_str(&async_timer_result_owned_field(index));
            out.push_str(";\n");
        }
        if let Some(connect) = statement_async_tcp_connect(statement) {
            out.push_str("    nomo_async_tcp_connect_registration ");
            out.push_str(&async_tcp_connect_registration_field(index));
            out.push_str(";\n    ");
            out.push_str(&c_type(connect.value_type));
            out.push(' ');
            out.push_str(&async_tcp_connect_result_field(index));
            out.push_str(";\n    uint8_t ");
            out.push_str(&async_tcp_connect_result_owned_field(index));
            out.push_str(";\n");
        }
        if let Some(operation) = statement_async_tcp_io(statement) {
            out.push_str("    nomo_async_tcp_io_registration ");
            out.push_str(&async_tcp_io_registration_field(index));
            out.push_str(";\n    ");
            out.push_str(&c_type(operation.value_type));
            out.push(' ');
            out.push_str(&async_tcp_io_result_field(index));
            out.push_str(";\n    uint8_t ");
            out.push_str(&async_tcp_io_result_owned_field(index));
            out.push_str(";\n");
        }
        if statement_structured_join(statement).is_some() {
            let join = statement_structured_join(statement)
                .expect("structured join was checked immediately above");
            out.push_str("    ");
            out.push_str(&c_type(join.value_type));
            out.push(' ');
            out.push_str(&async_join_result_field(index));
            out.push_str(";\n    uint8_t ");
            out.push_str(&async_join_result_owned_field(index));
            out.push_str(";\n");
        }
        if statement_structured_cancel_join(statement).is_some() {
            let cancel = statement_structured_cancel_join(statement)
                .expect("structured cancel was checked immediately above");
            out.push_str("    ");
            out.push_str(&c_type(cancel.value_type));
            out.push(' ');
            out.push_str(&async_cancel_join_result_field(index));
            out.push_str(";\n    uint8_t ");
            out.push_str(&async_cancel_join_result_owned_field(index));
            out.push_str(";\n");
        }
        if statement_structured_spawn(statement).is_some() {
            out.push_str("    uint8_t ");
            out.push_str(&async_spawn_failed_field(index));
            out.push_str(";\n");
        }
        let Some(call) = statement_async_call(statement, async_names) else {
            if let Some(spawn) = statement_structured_spawn(statement) {
                out.push_str("    ");
                out.push_str(&async_frame_ident(spawn.callee));
                out.push(' ');
                out.push_str(&async_child_field(index));
                out.push_str(";\n");
            }
            continue;
        };
        out.push_str("    ");
        out.push_str(&async_frame_ident(call.callee));
        out.push(' ');
        out.push_str(&async_child_field(index));
        out.push_str(";\n");
    }
    out.push_str("} ");
    out.push_str(&async_frame_ident(&function.name));
    out.push_str(";\n\n");
}

fn emit_async_frame_store(out: &mut String, local: &AsyncFrameLocal, indent: usize) {
    write_indent(out, indent);
    out.push_str("frame->");
    out.push_str(&async_frame_value_field(&local.name));
    out.push_str(" = ");
    out.push_str(&c_var_ident(&local.name));
    out.push_str(";\n");
    if value_type_needs_release(&local.value_type) {
        write_indent(out, indent);
        out.push_str("frame->");
        out.push_str(&async_frame_owned_field(&local.name));
        out.push_str(" = 1u;\n");
    }
}

fn emit_async_frame_alias(out: &mut String, local: &AsyncFrameLocal, indent: usize) {
    write_indent(out, indent);
    out.push_str(&c_type(&local.value_type));
    out.push(' ');
    out.push_str(&c_var_ident(&local.name));
    out.push_str(" = frame->");
    out.push_str(&async_frame_value_field(&local.name));
    out.push_str(";\n");
}

fn emit_async_frame_field_drop(out: &mut String, local: &AsyncFrameLocal, indent: usize) {
    if !value_type_needs_release(&local.value_type) {
        return;
    }
    emit_async_owned_field_drop(
        out,
        &local.value_type,
        &async_frame_owned_field(&local.name),
        &async_frame_value_field(&local.name),
        indent,
    );
}

fn emit_async_owned_field_drop(
    out: &mut String,
    value_type: &ValueType,
    owned_field: &str,
    value_field: &str,
    indent: usize,
) {
    write_indent(out, indent);
    out.push_str("if (frame->");
    out.push_str(owned_field);
    out.push_str(" != 0u) {\n");
    write_indent(out, indent + 1);
    out.push_str("frame->");
    out.push_str(owned_field);
    out.push_str(" = 0u;\n");
    emit_value_release_in_place(
        out,
        value_type,
        &format!("frame->{value_field}"),
        indent + 1,
    );
    write_indent(out, indent);
    out.push_str("}\n");
}

fn emit_async_parameter_aliases(out: &mut String, function: &Function, indent: usize) {
    for parameter in &function.params {
        write_indent(out, indent);
        out.push_str(&c_type(&parameter.value_type));
        out.push(' ');
        out.push_str(&c_var_ident(&parameter.name));
        out.push_str(" = frame->");
        out.push_str(&async_parameter_field(&parameter.name));
        out.push_str(";\n");
    }
}

fn emit_async_child_init(
    out: &mut String,
    call: AsyncCall<'_>,
    callee: &Function,
    index: usize,
    indent: usize,
    function: &Function,
    frame_locals: &[AsyncFrameLocal],
    local_owned: &mut Vec<LocalArray>,
) {
    debug_assert_eq!(call.args.len(), callee.params.len());
    let child = async_child_field(index);
    for (argument, parameter) in call.args.iter().zip(&callee.params) {
        let field = async_parameter_field(&parameter.name);
        write_indent(out, indent);
        out.push_str("frame->");
        out.push_str(&child);
        out.push('.');
        out.push_str(&field);
        out.push_str(" = ");
        emit_expr(out, argument);
        out.push_str(";\n");
        if value_type_needs_release(&parameter.value_type) {
            let c_value = format!("frame->{child}.{field}");
            if expr_may_share_array_storage(argument) {
                emit_value_retain_in_place(out, &parameter.value_type, &c_value, indent);
            }
            write_indent(out, indent);
            out.push_str("frame->");
            out.push_str(&child);
            out.push('.');
            out.push_str(&async_parameter_owned_field(&parameter.name));
            out.push_str(" = 1u;\n");
            if let Some(binding) = publication_move_binding(argument) {
                if function
                    .params
                    .iter()
                    .any(|candidate| candidate.name == binding)
                {
                    write_indent(out, indent);
                    out.push_str("frame->");
                    out.push_str(&async_parameter_owned_field(binding));
                    out.push_str(" = 0u;\n");
                } else if frame_locals
                    .iter()
                    .any(|candidate| candidate.name == binding)
                {
                    write_indent(out, indent);
                    out.push_str("frame->");
                    out.push_str(&async_frame_owned_field(binding));
                    out.push_str(" = 0u;\n");
                } else {
                    local_owned.retain(|candidate| candidate.name != binding);
                }
                write_indent(out, indent);
                out.push_str("context->publication_moves += 1u;\n");
            }
        }
    }
    write_indent(out, indent);
    out.push_str("frame->");
    out.push_str(&child);
    out.push_str(".initialized = 1u;\n");
}

fn publication_move_binding(argument: &ValueExpr) -> Option<&str> {
    match argument {
        ValueExpr::Call { name, args }
            if name == BUILTIN_TASK_PUBLICATION_MOVE_EXPR
                && matches!(args.as_slice(), [ValueExpr::Variable(_)]) =>
        {
            let [ValueExpr::Variable(binding)] = args.as_slice() else {
                unreachable!("publication move shape was checked by the guard")
            };
            Some(binding)
        }
        _ => None,
    }
}

fn emit_async_publication_move_transfer(
    out: &mut String,
    argument: &ValueExpr,
    function: &Function,
    frame_locals: &[AsyncFrameLocal],
    local_owned: &mut Vec<LocalArray>,
    indent: usize,
) {
    let Some(binding) = publication_move_binding(argument) else {
        return;
    };
    if function
        .params
        .iter()
        .any(|candidate| candidate.name == binding)
    {
        write_indent(out, indent);
        out.push_str("frame->");
        out.push_str(&async_parameter_owned_field(binding));
        out.push_str(" = 0u;\n");
    } else if frame_locals
        .iter()
        .any(|candidate| candidate.name == binding)
    {
        write_indent(out, indent);
        out.push_str("frame->");
        out.push_str(&async_frame_owned_field(binding));
        out.push_str(" = 0u;\n");
    } else {
        local_owned.retain(|candidate| candidate.name != binding);
    }
    write_indent(out, indent);
    out.push_str("context->publication_moves += 1u;\n");
}

fn emit_async_channel_result_binding(
    out: &mut String,
    index: usize,
    binding: &str,
    value_type: &ValueType,
    frame_locals: &[AsyncFrameLocal],
    local_owned: &mut Vec<LocalArray>,
    indent: usize,
) {
    if let Some(frame_local) = frame_locals
        .iter()
        .find(|local| local.declaration_index == index)
    {
        write_indent(out, indent);
        out.push_str("frame->");
        out.push_str(&async_frame_value_field(binding));
        out.push_str(" = frame->");
        out.push_str(&async_channel_result_field(index));
        out.push_str(";\n");
        if value_type_needs_release(value_type) {
            write_indent(out, indent);
            out.push_str("frame->");
            out.push_str(&async_frame_owned_field(binding));
            out.push_str(" = frame->");
            out.push_str(&async_channel_result_owned_field(index));
            out.push_str(";\n");
            write_indent(out, indent);
            out.push_str("frame->");
            out.push_str(&async_channel_result_owned_field(index));
            out.push_str(" = 0u;\n");
        }
        emit_async_frame_alias(out, frame_local, indent);
    } else {
        write_indent(out, indent);
        out.push_str(&c_type(value_type));
        out.push(' ');
        out.push_str(&c_var_ident(binding));
        out.push_str(" = frame->");
        out.push_str(&async_channel_result_field(index));
        out.push_str(";\n");
        if value_type_needs_release(value_type) {
            write_indent(out, indent);
            out.push_str("frame->");
            out.push_str(&async_channel_result_owned_field(index));
            out.push_str(" = 0u;\n");
        }
        if let Some(local) = local_array(binding, value_type) {
            local_owned.push(local);
        }
    }
}

fn emit_async_tcp_connect_result_binding(
    out: &mut String,
    index: usize,
    connect: AsyncTcpConnect<'_>,
    frame_locals: &[AsyncFrameLocal],
    local_owned: &mut Vec<LocalArray>,
    indent: usize,
) {
    if let Some(frame_local) = frame_locals
        .iter()
        .find(|local| local.declaration_index == index)
    {
        write_indent(out, indent);
        out.push_str("frame->");
        out.push_str(&async_frame_value_field(connect.binding));
        out.push_str(" = frame->");
        out.push_str(&async_tcp_connect_result_field(index));
        out.push_str(";\n");
        if value_type_needs_release(connect.value_type) {
            write_indent(out, indent);
            out.push_str("frame->");
            out.push_str(&async_frame_owned_field(connect.binding));
            out.push_str(" = frame->");
            out.push_str(&async_tcp_connect_result_owned_field(index));
            out.push_str(";\n");
            write_indent(out, indent);
            out.push_str("frame->");
            out.push_str(&async_tcp_connect_result_owned_field(index));
            out.push_str(" = 0u;\n");
        }
        emit_async_frame_alias(out, frame_local, indent);
    } else {
        write_indent(out, indent);
        out.push_str(&c_type(connect.value_type));
        out.push(' ');
        out.push_str(&c_var_ident(connect.binding));
        out.push_str(" = frame->");
        out.push_str(&async_tcp_connect_result_field(index));
        out.push_str(";\n");
        if value_type_needs_release(connect.value_type) {
            write_indent(out, indent);
            out.push_str("frame->");
            out.push_str(&async_tcp_connect_result_owned_field(index));
            out.push_str(" = 0u;\n");
        }
        if let Some(local) = local_array(connect.binding, connect.value_type) {
            local_owned.push(local);
        }
    }
}

fn emit_async_tcp_io_result_binding(
    out: &mut String,
    index: usize,
    operation: AsyncTcpIo<'_>,
    frame_locals: &[AsyncFrameLocal],
    local_owned: &mut Vec<LocalArray>,
    indent: usize,
) {
    if let Some(frame_local) = frame_locals
        .iter()
        .find(|local| local.declaration_index == index)
    {
        write_indent(out, indent);
        out.push_str("frame->");
        out.push_str(&async_frame_value_field(operation.binding));
        out.push_str(" = frame->");
        out.push_str(&async_tcp_io_result_field(index));
        out.push_str(";\n");
        if value_type_needs_release(operation.value_type) {
            write_indent(out, indent);
            out.push_str("frame->");
            out.push_str(&async_frame_owned_field(operation.binding));
            out.push_str(" = frame->");
            out.push_str(&async_tcp_io_result_owned_field(index));
            out.push_str(";\n");
            write_indent(out, indent);
            out.push_str("frame->");
            out.push_str(&async_tcp_io_result_owned_field(index));
            out.push_str(" = 0u;\n");
        }
        emit_async_frame_alias(out, frame_local, indent);
    } else {
        write_indent(out, indent);
        out.push_str(&c_type(operation.value_type));
        out.push(' ');
        out.push_str(&c_var_ident(operation.binding));
        out.push_str(" = frame->");
        out.push_str(&async_tcp_io_result_field(index));
        out.push_str(";\n");
        if value_type_needs_release(operation.value_type) {
            write_indent(out, indent);
            out.push_str("frame->");
            out.push_str(&async_tcp_io_result_owned_field(index));
            out.push_str(" = 0u;\n");
        }
        if let Some(local) = local_array(operation.binding, operation.value_type) {
            local_owned.push(local);
        }
    }
}

fn emit_async_tcp_connect_cancellations(out: &mut String, function: &Function, indent: usize) {
    for (index, statement) in function.body.iter().enumerate().rev() {
        if statement_async_tcp_connect(statement).is_none() {
            continue;
        }
        write_indent(out, indent);
        out.push_str("nomo_async_tcp_connect_cancel(&frame->");
        out.push_str(&async_tcp_connect_registration_field(index));
        out.push_str(", context);\n");
    }
}

fn emit_async_tcp_io_cancellations(out: &mut String, function: &Function, indent: usize) {
    for (index, statement) in function.body.iter().enumerate().rev() {
        if statement_async_tcp_io(statement).is_none() {
            continue;
        }
        write_indent(out, indent);
        out.push_str("nomo_async_tcp_io_cancel(&frame->");
        out.push_str(&async_tcp_io_registration_field(index));
        out.push_str(", context);\n");
    }
}

fn emit_async_channel_cancellations(out: &mut String, function: &Function, indent: usize) {
    for (index, statement) in function.body.iter().enumerate().rev() {
        if let Some(send) = statement_async_channel_send(statement) {
            write_indent(out, indent);
            out.push_str("nomo_channel_send_cancel_");
            out.push_str(send.suffix);
            out.push_str("(&frame->");
            out.push_str(&async_channel_send_registration_field(index));
            out.push_str(");\n");
        }
        if let Some(receive) = statement_async_channel_receive(statement) {
            write_indent(out, indent);
            out.push_str("nomo_channel_receive_cancel_");
            out.push_str(receive.suffix);
            out.push_str("(&frame->");
            out.push_str(&async_channel_receive_registration_field(index));
            out.push_str(");\n");
        }
    }
}

fn emit_async_select_cancellations(out: &mut String, function: &Function, indent: usize) {
    for (index, statement) in function.body.iter().enumerate().rev() {
        if statement_task_select(statement).is_none() {
            continue;
        }
        write_indent(out, indent);
        out.push_str("nomo_async_select_cancel(&frame->");
        out.push_str(&async_select_token_field(index));
        out.push_str(");\n");
    }
}

#[allow(clippy::too_many_arguments)]
fn emit_async_question_let(
    out: &mut String,
    function: &Function,
    statement_index: usize,
    carrier: QuestionCarrier,
    name: &str,
    value_type: &ValueType,
    result_type: &ValueType,
    return_type: &ValueType,
    result_expr: &ValueExpr,
    early_exit_actions: &[ValueExpr],
    local_owned: &[LocalArray],
    indent: usize,
) {
    let ValueType::Enum(result_name, result_args) = result_type else {
        unreachable!("question result must be an enum carrier");
    };
    let ValueType::Enum(return_name, return_args) = return_type else {
        unreachable!("question propagation requires an enum return type");
    };
    let (early_variant, payload_variant) = match carrier {
        QuestionCarrier::Result => ("Err", "Ok"),
        QuestionCarrier::Option => ("None", "Some"),
    };
    let temporary = format!("nomo_async_question_result_{statement_index}");
    write_indent(out, indent);
    out.push_str(&c_type(result_type));
    out.push(' ');
    out.push_str(&temporary);
    out.push_str(" = ");
    emit_expr(out, result_expr);
    out.push_str(";\n");
    write_indent(out, indent);
    out.push_str("if (");
    out.push_str(&temporary);
    out.push_str(".tag == ");
    out.push_str(&c_enum_variant_ident(
        result_name,
        result_args,
        early_variant,
    ));
    out.push_str(") {\n");
    write_indent(out, indent + 1);
    out.push_str("frame->");
    out.push_str(async_result_field());
    out.push_str(" = (");
    out.push_str(&c_enum_ident(return_name, return_args));
    out.push_str("){.tag = ");
    out.push_str(&c_enum_variant_ident(
        return_name,
        return_args,
        early_variant,
    ));
    if carrier == QuestionCarrier::Result {
        out.push_str(", .payload.");
        out.push_str(&c_payload_ident("Err"));
        out.push_str(" = ");
        out.push_str(&temporary);
        out.push_str(".payload.");
        out.push_str(&c_payload_ident("Err"));
    }
    out.push_str("};\n");
    if expr_may_share_array_storage(result_expr) && value_type_needs_release(return_type) {
        emit_value_retain_in_place(
            out,
            return_type,
            &format!("frame->{}", async_result_field()),
            indent + 1,
        );
    }
    if value_type_needs_release(return_type) {
        write_indent(out, indent + 1);
        out.push_str("frame->");
        out.push_str(async_result_owned_field());
        out.push_str(" = 1u;\n");
    }
    for action in early_exit_actions {
        let handle = structured_cancel_handle(action)
            .expect("structured question early-exit actions are validated cancellations");
        emit_async_structured_cancel(out, function, handle, indent + 1);
    }
    emit_async_local_releases(out, local_owned, &[], indent + 1);
    emit_structured_completion(out, indent + 1);
    write_indent(out, indent + 1);
    out.push_str("frame->state = UINT32_MAX;\n");
    write_indent(out, indent + 1);
    out.push_str("return NOMO_ASYNC_POLL_READY;\n");
    write_indent(out, indent);
    out.push_str("}\n");
    write_indent(out, indent);
    out.push_str(&c_payload_type(value_type));
    out.push(' ');
    out.push_str(&c_var_ident(name));
    out.push_str(" = ");
    out.push_str(&temporary);
    out.push_str(".payload.");
    out.push_str(&c_payload_ident(payload_variant));
    out.push_str(";\n");
    if expr_may_share_array_storage(result_expr) && value_type_needs_release(value_type) {
        emit_value_retain_in_place(out, value_type, &c_var_ident(name), indent);
    }
}

fn emit_async_return_value(
    out: &mut String,
    function: &Function,
    value: &ValueExpr,
    local_owned: &[LocalArray],
    indent: usize,
) {
    debug_assert_ne!(function.return_type, ValueType::Void);
    write_indent(out, indent);
    out.push_str("frame->");
    out.push_str(async_result_field());
    out.push_str(" = ");
    emit_expr(out, value);
    out.push_str(";\n");
    if value_type_needs_release(&function.return_type) {
        if expr_may_share_array_storage(value) {
            emit_value_retain_in_place(
                out,
                &function.return_type,
                &format!("frame->{}", async_result_field()),
                indent,
            );
        }
        write_indent(out, indent);
        out.push_str("frame->");
        out.push_str(async_result_owned_field());
        out.push_str(" = 1u;\n");
    }
    emit_async_local_releases(out, local_owned, &[], indent);
    emit_structured_completion(out, indent);
    write_indent(out, indent);
    out.push_str("frame->state = UINT32_MAX;\n");
    write_indent(out, indent);
    out.push_str("return NOMO_ASYNC_POLL_READY;\n");
}

fn emit_structured_completion(out: &mut String, indent: usize) {
    write_indent(out, indent);
    out.push_str("frame->structured_completed = 1u;\n");
    write_indent(out, indent);
    out.push_str("if (frame->structured_waiter_frame != NULL) {\n");
    write_indent(out, indent + 1);
    out.push_str(
        "if (nomo_async_ready_enqueue(\n\
             ",
    );
    write_indent(out, indent + 2);
    out.push_str("context,\n");
    write_indent(out, indent + 2);
    out.push_str("frame->structured_waiter_frame,\n");
    write_indent(out, indent + 2);
    out.push_str("frame->structured_waiter_poll\n");
    write_indent(out, indent + 1);
    out.push_str(") != 0) {\n");
    write_indent(out, indent + 2);
    out.push_str("context->runtime_failed = 1u;\n");
    write_indent(out, indent + 1);
    out.push_str("}\n");
    write_indent(out, indent + 1);
    out.push_str("frame->structured_waiter_frame = NULL;\n");
    write_indent(out, indent + 1);
    out.push_str("frame->structured_waiter_poll = NULL;\n");
    write_indent(out, indent);
    out.push_str("}\n");
}

fn emit_async_deadline_failure(
    out: &mut String,
    function: &Function,
    async_names: &BTreeSet<String>,
    failure: &str,
    indent: usize,
) {
    write_indent(out, indent);
    out.push_str("frame->structured_failure = ");
    out.push_str(failure);
    out.push_str(";\n");
    if failure == "NOMO_ASYNC_TASK_FAILURE_TIMEOUT" {
        write_indent(out, indent);
        out.push_str("context->deadline_expirations += 1u;\n");
    }
    write_indent(out, indent);
    out.push_str("nomo_async_ready_cancel_frame(context, frame);\n");
    if function_has_async_deadline(function) {
        write_indent(out, indent);
        out.push_str("nomo_async_timer_disarm(&frame->nomo_async_deadline_timer, context);\n");
        write_indent(out, indent);
        out.push_str("frame->nomo_async_deadline_active = 0u;\n");
    }
    for (index, statement) in function.body.iter().enumerate().rev() {
        if statement_async_sleep(statement).is_none() {
            continue;
        }
        write_indent(out, indent);
        out.push_str("nomo_async_timer_disarm(&frame->");
        out.push_str(&async_timer_field(index));
        out.push_str(", context);\n");
    }
    emit_async_select_cancellations(out, function, indent);
    emit_async_channel_cancellations(out, function, indent);
    emit_async_tcp_connect_cancellations(out, function, indent);
    emit_async_tcp_io_cancellations(out, function, indent);
    for (index, statement) in function.body.iter().enumerate().rev() {
        let Some(spawn) = statement_structured_spawn(statement) else {
            continue;
        };
        write_indent(out, indent);
        out.push_str(&async_cancel_ident(spawn.callee));
        out.push_str("(&frame->");
        out.push_str(&async_child_field(index));
        out.push_str(", context);\n");
    }
    for (index, statement) in function.body.iter().enumerate().rev() {
        let Some(call) = statement_async_call(statement, async_names) else {
            continue;
        };
        write_indent(out, indent);
        out.push_str(&async_cancel_ident(call.callee));
        out.push_str("(&frame->");
        out.push_str(&async_child_field(index));
        out.push_str(", context);\n");
    }
    emit_structured_completion(out, indent);
    write_indent(out, indent);
    out.push_str("frame->state = UINT32_MAX;\n");
    write_indent(out, indent);
    out.push_str("return NOMO_ASYNC_POLL_READY;\n");
}

fn emit_async_deadline_due_check(
    out: &mut String,
    function: &Function,
    async_names: &BTreeSet<String>,
    indent: usize,
) {
    write_indent(out, indent);
    out.push_str(
        "if (frame->nomo_async_deadline_active != 0u\n\
         ",
    );
    write_indent(out, indent + 1);
    out.push_str(
        "&& nomo_async_deadline_due(&frame->nomo_async_deadline_timer, context) != 0) {\n",
    );
    emit_async_deadline_failure(
        out,
        function,
        async_names,
        "NOMO_ASYNC_TASK_FAILURE_TIMEOUT",
        indent + 1,
    );
    write_indent(out, indent);
    out.push_str("}\n");
}

fn emit_async_child_failure_propagation(
    out: &mut String,
    function: &Function,
    async_names: &BTreeSet<String>,
    child_index: usize,
    indent: usize,
) {
    write_indent(out, indent);
    out.push_str("if (frame->");
    out.push_str(&async_child_field(child_index));
    out.push_str(".structured_failure != NOMO_ASYNC_TASK_FAILURE_NONE) {\n");
    write_indent(out, indent + 1);
    out.push_str("frame->structured_failure = frame->");
    out.push_str(&async_child_field(child_index));
    out.push_str(".structured_failure;\n");
    write_indent(out, indent + 1);
    out.push_str("nomo_async_ready_cancel_frame(context, frame);\n");
    if function_has_async_deadline(function) {
        write_indent(out, indent + 1);
        out.push_str("if (frame->nomo_async_deadline_active != 0u) {\n");
        write_indent(out, indent + 2);
        out.push_str("nomo_async_timer_disarm(&frame->nomo_async_deadline_timer, context);\n");
        write_indent(out, indent + 2);
        out.push_str("frame->nomo_async_deadline_active = 0u;\n");
        write_indent(out, indent + 2);
        out.push_str("context->deadline_cancellations += 1u;\n");
        write_indent(out, indent + 1);
        out.push_str("}\n");
    }
    for (index, statement) in function.body.iter().enumerate().rev() {
        if statement_async_sleep(statement).is_none() {
            continue;
        }
        write_indent(out, indent + 1);
        out.push_str("nomo_async_timer_disarm(&frame->");
        out.push_str(&async_timer_field(index));
        out.push_str(", context);\n");
    }
    emit_async_select_cancellations(out, function, indent + 1);
    emit_async_channel_cancellations(out, function, indent + 1);
    emit_async_tcp_connect_cancellations(out, function, indent + 1);
    emit_async_tcp_io_cancellations(out, function, indent + 1);
    for (index, statement) in function.body.iter().enumerate().rev() {
        let Some(spawn) = statement_structured_spawn(statement) else {
            continue;
        };
        write_indent(out, indent + 1);
        out.push_str(&async_cancel_ident(spawn.callee));
        out.push_str("(&frame->");
        out.push_str(&async_child_field(index));
        out.push_str(", context);\n");
    }
    for (index, statement) in function.body.iter().enumerate().rev() {
        let Some(call) = statement_async_call(statement, async_names) else {
            continue;
        };
        write_indent(out, indent + 1);
        out.push_str(&async_cancel_ident(call.callee));
        out.push_str("(&frame->");
        out.push_str(&async_child_field(index));
        out.push_str(", context);\n");
    }
    emit_structured_completion(out, indent + 1);
    write_indent(out, indent + 1);
    out.push_str("frame->state = UINT32_MAX;\n");
    write_indent(out, indent + 1);
    out.push_str("return NOMO_ASYNC_POLL_READY;\n");
    write_indent(out, indent);
    out.push_str("}\n");
}

fn emit_async_cancel_function(
    out: &mut String,
    function: &Function,
    async_names: &BTreeSet<String>,
) {
    out.push_str("static void ");
    out.push_str(&async_cancel_ident(&function.name));
    out.push('(');
    out.push_str(&async_frame_ident(&function.name));
    out.push_str(
        " *frame, nomo_async_context *context) {\n\
             if (frame->initialized == 0u || frame->cancelled != 0u || frame->structured_completed != 0u) {\n\
                 return;\n\
             }\n\
             frame->cancelled = 1u;\n\
             context->task_cancellations += 1u;\n\
             nomo_async_ready_cancel_frame(context, frame);\n",
    );
    if function_has_async_deadline(function) {
        out.push_str(
            "    if (frame->nomo_async_deadline_active != 0u) {\n\
                 nomo_async_timer_disarm(&frame->nomo_async_deadline_timer, context);\n\
                 frame->nomo_async_deadline_active = 0u;\n\
                 context->deadline_cancellations += 1u;\n\
             }\n",
        );
    }
    for (index, statement) in function.body.iter().enumerate().rev() {
        let Some(spawn) = statement_structured_spawn(statement) else {
            continue;
        };
        out.push_str("    ");
        out.push_str(&async_cancel_ident(spawn.callee));
        out.push_str("(&frame->");
        out.push_str(&async_child_field(index));
        out.push_str(", context);\n");
    }
    for (index, statement) in function.body.iter().enumerate().rev() {
        let Some(call) = statement_async_call(statement, async_names) else {
            continue;
        };
        out.push_str("    ");
        out.push_str(&async_cancel_ident(call.callee));
        out.push_str("(&frame->");
        out.push_str(&async_child_field(index));
        out.push_str(", context);\n");
    }
    for (index, statement) in function.body.iter().enumerate().rev() {
        if statement_async_sleep(statement).is_none() {
            continue;
        }
        out.push_str("    nomo_async_timer_disarm(&frame->");
        out.push_str(&async_timer_field(index));
        out.push_str(", context);\n");
    }
    emit_async_select_cancellations(out, function, 1);
    emit_async_channel_cancellations(out, function, 1);
    emit_async_tcp_connect_cancellations(out, function, 1);
    emit_async_tcp_io_cancellations(out, function, 1);
    out.push_str(
        "    frame->structured_waiter_frame = NULL;\n\
             frame->structured_waiter_poll = NULL;\n\
             frame->structured_completed = 1u;\n\
             frame->state = UINT32_MAX;\n\
         }\n\n",
    );
}

fn emit_async_local_releases(
    out: &mut String,
    locals: &[LocalArray],
    moved_to_frame: &[AsyncFrameLocal],
    indent: usize,
) {
    for local in locals.iter().rev() {
        if moved_to_frame
            .iter()
            .any(|frame_local| frame_local.name == local.name)
        {
            continue;
        }
        emit_value_release_binding(out, &local.name, &local.value_type, indent);
    }
}

fn next_async_suspend(function: &Function, start: usize, async_names: &BTreeSet<String>) -> usize {
    function
        .body
        .iter()
        .enumerate()
        .skip(start)
        .find_map(|(index, statement)| {
            statement_is_async_suspend(statement, async_names).then_some(index)
        })
        .unwrap_or(function.body.len())
}

fn emit_async_timer_result_materialize(out: &mut String, index: usize, indent: usize) {
    emit_async_timer_result_materialize_fields(
        out,
        &async_timer_result_field(index),
        &async_timer_outcome_field(index),
        &async_timer_result_owned_field(index),
        indent,
    );
}

fn emit_async_timer_result_materialize_fields(
    out: &mut String,
    result_field: &str,
    outcome_field: &str,
    owned_field: &str,
    indent: usize,
) {
    let result_type = async_sleep_result_type();
    let ValueType::Enum(_, result_args) = &result_type else {
        unreachable!("sleep result is always a Result enum");
    };
    let result = format!("frame->{result_field}");
    let outcome = format!("frame->{outcome_field}");
    write_indent(out, indent);
    out.push_str("memset(&");
    out.push_str(&result);
    out.push_str(", 0, sizeof(");
    out.push_str(&result);
    out.push_str("));\n");
    write_indent(out, indent);
    out.push_str("if (");
    out.push_str(&outcome);
    out.push_str(" == NOMO_ASYNC_TIMER_OUTCOME_OK) {\n");
    write_indent(out, indent + 1);
    out.push_str(&result);
    out.push_str(".tag = ");
    out.push_str(&c_enum_variant_ident("Result", result_args, "Ok"));
    out.push_str(";\n");
    write_indent(out, indent);
    out.push_str("} else {\n");
    write_indent(out, indent + 1);
    out.push_str(&result);
    out.push_str(".tag = ");
    out.push_str(&c_enum_variant_ident("Result", result_args, "Err"));
    out.push_str(";\n");
    write_indent(out, indent + 1);
    out.push_str(&result);
    out.push_str(".payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push('.');
    out.push_str(&c_member_ident("code"));
    out.push_str(" = ");
    out.push_str(&outcome);
    out.push_str(" == NOMO_ASYNC_TIMER_OUTCOME_LIMIT\n");
    write_indent(out, indent + 2);
    out.push_str("? nomo_string_literal(\"timer_limit\")\n");
    write_indent(out, indent + 2);
    out.push_str(": nomo_string_literal(\"runtime\");\n");
    write_indent(out, indent + 1);
    out.push_str(&result);
    out.push_str(".payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push('.');
    out.push_str(&c_member_ident("message"));
    out.push_str(" = ");
    out.push_str(&outcome);
    out.push_str(" == NOMO_ASYNC_TIMER_OUTCOME_LIMIT\n");
    write_indent(out, indent + 2);
    out.push_str("? nomo_string_literal(\"owner executor timer capacity is exhausted\")\n");
    write_indent(out, indent + 2);
    out.push_str(": nomo_string_literal(\"timer runtime entered an invalid state\");\n");
    write_indent(out, indent);
    out.push_str("}\n");
    write_indent(out, indent);
    out.push_str("frame->");
    out.push_str(owned_field);
    out.push_str(" = 1u;\n");
}

fn async_select_channel_local(index: usize, arm: usize) -> String {
    format!("nomo_async_select_channel_{index}_{arm}")
}

fn async_select_duration_local(index: usize, arm: usize) -> String {
    format!("nomo_async_select_duration_{index}_{arm}")
}

fn emit_async_select_operands(
    out: &mut String,
    index: usize,
    arms: &[TaskSelectArm],
    indent: usize,
) {
    for (arm_index, arm) in arms.iter().enumerate() {
        write_indent(out, indent);
        match &arm.operation {
            TaskSelectOperation::Receive {
                channel,
                element_type,
            } => {
                out.push_str(&c_type(&ValueType::Struct(
                    "Channel".to_string(),
                    vec![element_type.clone()],
                )));
                out.push(' ');
                out.push_str(&async_select_channel_local(index, arm_index));
                out.push_str(" = ");
                emit_expr(out, channel);
                out.push_str(";\n");
            }
            TaskSelectOperation::Sleep { duration } => {
                out.push_str("int64_t ");
                out.push_str(&async_select_duration_local(index, arm_index));
                out.push_str(" = (");
                emit_expr(out, duration);
                out.push_str(").nomo_member_millis;\n");
            }
        }
    }
}

fn emit_async_select_start(
    out: &mut String,
    index: usize,
    state: u32,
    arms: &[TaskSelectArm],
    indent: usize,
) {
    let token = async_select_token_field(index);
    write_indent(out, indent);
    out.push_str("nomo_async_select_init(&frame->");
    out.push_str(&token);
    out.push_str(", ");
    out.push_str(&arms.len().to_string());
    out.push_str("u, context);\n");
    for (arm_index, arm) in arms.iter().enumerate() {
        match &arm.operation {
            TaskSelectOperation::Receive { element_type, .. } => {
                let suffix = c_type_name_part(element_type);
                let registration = async_select_receive_registration_field(index, arm_index);
                write_indent(out, indent);
                out.push_str("if (nomo_channel_receive_start_");
                out.push_str(&suffix);
                out.push_str("(&frame->");
                out.push_str(&registration);
                out.push_str(", ");
                out.push_str(&async_select_channel_local(index, arm_index));
                out.push_str(", context, &frame->");
                out.push_str(&async_select_result_field(index, arm_index));
                out.push_str(", &frame->");
                out.push_str(&token);
                out.push_str(", ");
                out.push_str(&arm_index.to_string());
                out.push_str("u) == NOMO_ASYNC_POLL_READY) {\n");
                write_indent(out, indent + 1);
                out.push_str("frame->");
                out.push_str(&async_select_result_owned_field(index, arm_index));
                out.push_str(" = 1u;\n");
                write_indent(out, indent + 1);
                out.push_str("if (nomo_async_select_immediate_win(&frame->");
                out.push_str(&token);
                out.push_str(", ");
                out.push_str(&arm_index.to_string());
                out.push_str("u) == 0) {\n");
                write_indent(out, indent + 2);
                out.push_str("context->runtime_failed = 1u;\n");
                write_indent(out, indent + 2);
                out.push_str("return NOMO_ASYNC_POLL_READY;\n");
                write_indent(out, indent + 1);
                out.push_str("}\n");
                write_indent(out, indent + 1);
                out.push_str("goto nomo_async_resume_");
                out.push_str(&state.to_string());
                out.push_str(";\n");
                write_indent(out, indent);
                out.push_str("}\n");
                write_indent(out, indent);
                out.push_str("nomo_async_select_register(&frame->");
                out.push_str(&token);
                out.push_str(", ");
                out.push_str(&arm_index.to_string());
                out.push_str("u, &frame->");
                out.push_str(&registration);
                out.push_str(", nomo_channel_receive_select_cancel_");
                out.push_str(&suffix);
                out.push_str(");\n");
            }
            TaskSelectOperation::Sleep { .. } => {
                let timer = async_select_timer_field(index, arm_index);
                let outcome = async_select_timer_outcome_field(index, arm_index);
                write_indent(out, indent);
                out.push_str("if (nomo_async_timer_start(&frame->");
                out.push_str(&timer);
                out.push_str(", ");
                out.push_str(&async_select_duration_local(index, arm_index));
                out.push_str(", context, &frame->");
                out.push_str(&outcome);
                out.push_str(", &frame->");
                out.push_str(&token);
                out.push_str(", ");
                out.push_str(&arm_index.to_string());
                out.push_str("u) == NOMO_ASYNC_POLL_READY) {\n");
                emit_async_timer_result_materialize_fields(
                    out,
                    &async_select_result_field(index, arm_index),
                    &outcome,
                    &async_select_result_owned_field(index, arm_index),
                    indent + 1,
                );
                write_indent(out, indent + 1);
                out.push_str("if (nomo_async_select_immediate_win(&frame->");
                out.push_str(&token);
                out.push_str(", ");
                out.push_str(&arm_index.to_string());
                out.push_str("u) == 0) {\n");
                write_indent(out, indent + 2);
                out.push_str("context->runtime_failed = 1u;\n");
                write_indent(out, indent + 2);
                out.push_str("return NOMO_ASYNC_POLL_READY;\n");
                write_indent(out, indent + 1);
                out.push_str("}\n");
                write_indent(out, indent + 1);
                out.push_str("goto nomo_async_resume_");
                out.push_str(&state.to_string());
                out.push_str(";\n");
                write_indent(out, indent);
                out.push_str("}\n");
                write_indent(out, indent);
                out.push_str("nomo_async_select_register(&frame->");
                out.push_str(&token);
                out.push_str(", ");
                out.push_str(&arm_index.to_string());
                out.push_str("u, &frame->");
                out.push_str(&timer);
                out.push_str(", nomo_async_timer_select_cancel);\n");
            }
        }
    }
    write_indent(out, indent);
    out.push_str("nomo_async_select_suspend(&frame->");
    out.push_str(&token);
    out.push_str(");\n");
    write_indent(out, indent);
    out.push_str("return NOMO_ASYNC_POLL_PENDING;\n");
}

fn emit_async_select_resume_and_body(
    out: &mut String,
    index: usize,
    arms: &[TaskSelectArm],
    function: &Function,
    frame_locals: &[AsyncFrameLocal],
    indent: usize,
) {
    let token = async_select_token_field(index);
    write_indent(out, indent);
    out.push_str("if (frame->");
    out.push_str(&token);
    out.push_str(".winner == NOMO_ASYNC_SELECT_PENDING) {\n");
    write_indent(out, indent + 1);
    out.push_str("nomo_async_select_suspend(&frame->");
    out.push_str(&token);
    out.push_str(");\n");
    write_indent(out, indent + 1);
    out.push_str("return NOMO_ASYNC_POLL_PENDING;\n");
    write_indent(out, indent);
    out.push_str("}\n");
    for (arm_index, arm) in arms.iter().enumerate() {
        write_indent(out, indent);
        out.push_str("if (frame->");
        out.push_str(&token);
        out.push_str(".winner == ");
        out.push_str(&arm_index.to_string());
        out.push_str("u) {\n");
        for local in frame_locals
            .iter()
            .filter(|local| local.declaration_index < index)
            .filter(|local| {
                arm.body
                    .iter()
                    .any(|statement| statement_uses_binding(statement, &local.name))
            })
        {
            emit_async_frame_alias(out, local, indent + 1);
        }
        write_indent(out, indent + 1);
        out.push_str("if (frame->");
        out.push_str(&token);
        out.push_str(".suspended != 0u) {\n");
        match &arm.operation {
            TaskSelectOperation::Receive { element_type, .. } => {
                let suffix = c_type_name_part(element_type);
                write_indent(out, indent + 2);
                out.push_str("if (nomo_channel_receive_resume_");
                out.push_str(&suffix);
                out.push_str("(&frame->");
                out.push_str(&async_select_receive_registration_field(index, arm_index));
                out.push_str(", context, &frame->");
                out.push_str(&async_select_result_field(index, arm_index));
                out.push_str(") == NOMO_ASYNC_POLL_PENDING) {\n");
                write_indent(out, indent + 3);
                out.push_str("context->runtime_failed = 1u;\n");
                write_indent(out, indent + 3);
                out.push_str("return NOMO_ASYNC_POLL_READY;\n");
                write_indent(out, indent + 2);
                out.push_str("}\n");
                write_indent(out, indent + 2);
                out.push_str("frame->");
                out.push_str(&async_select_result_owned_field(index, arm_index));
                out.push_str(" = 1u;\n");
            }
            TaskSelectOperation::Sleep { .. } => {
                write_indent(out, indent + 2);
                out.push_str("if (nomo_async_timer_resume(&frame->");
                out.push_str(&async_select_timer_field(index, arm_index));
                out.push_str(", context, &frame->");
                out.push_str(&async_select_timer_outcome_field(index, arm_index));
                out.push_str(") == NOMO_ASYNC_POLL_PENDING) {\n");
                write_indent(out, indent + 3);
                out.push_str("context->runtime_failed = 1u;\n");
                write_indent(out, indent + 3);
                out.push_str("return NOMO_ASYNC_POLL_READY;\n");
                write_indent(out, indent + 2);
                out.push_str("}\n");
                emit_async_timer_result_materialize_fields(
                    out,
                    &async_select_result_field(index, arm_index),
                    &async_select_timer_outcome_field(index, arm_index),
                    &async_select_result_owned_field(index, arm_index),
                    indent + 2,
                );
            }
        }
        write_indent(out, indent + 1);
        out.push_str("}\n");
        write_indent(out, indent + 1);
        out.push_str("nomo_async_select_complete(&frame->");
        out.push_str(&token);
        out.push_str(", ");
        out.push_str(&arm_index.to_string());
        out.push_str("u);\n");
        write_indent(out, indent + 1);
        out.push_str(&c_type(&arm.binding_type));
        out.push(' ');
        out.push_str(&c_var_ident(&arm.binding));
        out.push_str(" = frame->");
        out.push_str(&async_select_result_field(index, arm_index));
        out.push_str(";\n");
        if value_type_needs_release(&arm.binding_type) {
            write_indent(out, indent + 1);
            out.push_str("frame->");
            out.push_str(&async_select_result_owned_field(index, arm_index));
            out.push_str(" = 0u;\n");
        }
        let mut active_arrays = Vec::new();
        if let Some(local) = local_array(&arm.binding, &arm.binding_type) {
            active_arrays.push(local);
        }
        emit_block(
            out,
            &arm.body,
            indent + 1,
            &[],
            &function.return_type,
            &active_arrays,
            0,
            0,
            0,
            0,
        );
        if value_type_needs_release(&arm.binding_type) {
            emit_value_release_binding(out, &arm.binding, &arm.binding_type, indent + 1);
        }
        write_indent(out, indent + 1);
        out.push_str("goto nomo_async_select_done_");
        out.push_str(&index.to_string());
        out.push_str(";\n");
        write_indent(out, indent);
        out.push_str("}\n");
    }
    write_indent(out, indent);
    out.push_str("context->runtime_failed = 1u;\n");
    write_indent(out, indent);
    out.push_str("return NOMO_ASYNC_POLL_READY;\n");
    out.push_str("nomo_async_select_done_");
    out.push_str(&index.to_string());
    out.push_str(":\n");
    write_indent(out, indent);
    out.push_str(";\n");
    for local in frame_locals
        .iter()
        .filter(|local| local.last_use_index == index)
    {
        emit_async_frame_field_drop(out, local, indent);
    }
}

fn emit_structured_join_result_materialize(
    out: &mut String,
    spawn_index: usize,
    join_index: usize,
    result_type: &ValueType,
    child_return_type: &ValueType,
    indent: usize,
) {
    let ValueType::Enum(_, result_args) = result_type else {
        unreachable!("structured join result is always a Result enum");
    };
    let result = format!("frame->{}", async_join_result_field(join_index));
    write_indent(out, indent);
    out.push_str("memset(&");
    out.push_str(&result);
    out.push_str(", 0, sizeof(");
    out.push_str(&result);
    out.push_str("));\n");
    write_indent(out, indent);
    out.push_str("if (frame->");
    out.push_str(&async_spawn_failed_field(spawn_index));
    out.push_str(" == 0u && frame->");
    out.push_str(&async_child_field(spawn_index));
    out.push_str(".structured_failure == NOMO_ASYNC_TASK_FAILURE_NONE) {\n");
    write_indent(out, indent + 1);
    out.push_str(&result);
    out.push_str(".tag = ");
    out.push_str(&c_enum_variant_ident("Result", result_args, "Ok"));
    out.push_str(";\n");
    if child_return_type != &ValueType::Void {
        write_indent(out, indent + 1);
        out.push_str(&result);
        out.push_str(".payload.");
        out.push_str(&c_payload_ident("Ok"));
        out.push_str(" = frame->");
        out.push_str(&async_child_field(spawn_index));
        out.push('.');
        out.push_str(async_result_field());
        out.push_str(";\n");
        if value_type_needs_release(child_return_type) {
            write_indent(out, indent + 1);
            out.push_str("frame->");
            out.push_str(&async_child_field(spawn_index));
            out.push('.');
            out.push_str(async_result_owned_field());
            out.push_str(" = 0u;\n");
        }
    }
    write_indent(out, indent);
    out.push_str("} else {\n");
    write_indent(out, indent + 1);
    out.push_str(&result);
    out.push_str(".tag = ");
    out.push_str(&c_enum_variant_ident("Result", result_args, "Err"));
    out.push_str(";\n");
    write_indent(out, indent + 1);
    out.push_str(&result);
    out.push_str(".payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push('.');
    out.push_str(&c_member_ident("code"));
    out.push_str(" = frame->");
    out.push_str(&async_spawn_failed_field(spawn_index));
    out.push_str(" != 0u\n");
    write_indent(out, indent + 2);
    out.push_str("? nomo_string_literal(\"queue_full\")\n");
    write_indent(out, indent + 2);
    out.push_str(": nomo_string_literal(nomo_async_task_failure_code(frame->");
    out.push_str(&async_child_field(spawn_index));
    out.push_str(".structured_failure));\n");
    write_indent(out, indent + 1);
    out.push_str(&result);
    out.push_str(".payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push('.');
    out.push_str(&c_member_ident("message"));
    out.push_str(" = frame->");
    out.push_str(&async_spawn_failed_field(spawn_index));
    out.push_str(" != 0u\n");
    write_indent(out, indent + 2);
    out.push_str("? nomo_string_literal(\"owner executor ready queue is full\")\n");
    write_indent(out, indent + 2);
    out.push_str(": nomo_string_literal(nomo_async_task_failure_message(frame->");
    out.push_str(&async_child_field(spawn_index));
    out.push_str(".structured_failure));\n");
    write_indent(out, indent);
    out.push_str("}\n");
    write_indent(out, indent);
    out.push_str("frame->");
    out.push_str(&async_join_result_owned_field(join_index));
    out.push_str(" = 1u;\n");
}

fn emit_structured_cancel_join_result_materialize(
    out: &mut String,
    spawn_index: usize,
    cancel_index: usize,
    result_type: &ValueType,
    indent: usize,
) {
    let ValueType::Enum(_, result_args) = result_type else {
        unreachable!("structured cancel result is always a Result enum");
    };
    let result = format!("frame->{}", async_cancel_join_result_field(cancel_index));
    write_indent(out, indent);
    out.push_str("memset(&");
    out.push_str(&result);
    out.push_str(", 0, sizeof(");
    out.push_str(&result);
    out.push_str("));\n");
    write_indent(out, indent);
    out.push_str("if (frame->");
    out.push_str(&async_spawn_failed_field(spawn_index));
    out.push_str(" == 0u && frame->");
    out.push_str(&async_child_field(spawn_index));
    out.push_str(".structured_failure == NOMO_ASYNC_TASK_FAILURE_NONE) {\n");
    write_indent(out, indent + 1);
    out.push_str(&result);
    out.push_str(".tag = ");
    out.push_str(&c_enum_variant_ident("Result", result_args, "Ok"));
    out.push_str(";\n");
    write_indent(out, indent);
    out.push_str("} else {\n");
    write_indent(out, indent + 1);
    out.push_str(&result);
    out.push_str(".tag = ");
    out.push_str(&c_enum_variant_ident("Result", result_args, "Err"));
    out.push_str(";\n");
    write_indent(out, indent + 1);
    out.push_str(&result);
    out.push_str(".payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push('.');
    out.push_str(&c_member_ident("code"));
    out.push_str(" = frame->");
    out.push_str(&async_spawn_failed_field(spawn_index));
    out.push_str(" != 0u\n");
    write_indent(out, indent + 2);
    out.push_str("? nomo_string_literal(\"queue_full\")\n");
    write_indent(out, indent + 2);
    out.push_str(": nomo_string_literal(nomo_async_task_failure_code(frame->");
    out.push_str(&async_child_field(spawn_index));
    out.push_str(".structured_failure));\n");
    write_indent(out, indent + 1);
    out.push_str(&result);
    out.push_str(".payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push('.');
    out.push_str(&c_member_ident("message"));
    out.push_str(" = frame->");
    out.push_str(&async_spawn_failed_field(spawn_index));
    out.push_str(" != 0u\n");
    write_indent(out, indent + 2);
    out.push_str("? nomo_string_literal(\"owner executor ready queue is full\")\n");
    write_indent(out, indent + 2);
    out.push_str(": nomo_string_literal(nomo_async_task_failure_message(frame->");
    out.push_str(&async_child_field(spawn_index));
    out.push_str(".structured_failure));\n");
    write_indent(out, indent);
    out.push_str("}\n");
    write_indent(out, indent);
    out.push_str("frame->");
    out.push_str(&async_cancel_join_result_owned_field(cancel_index));
    out.push_str(" = 1u;\n");
}

pub(super) fn emit_current_thread_executor(out: &mut String, target: &nomo_target::TargetTriple) {
    emit_async_reactor_helpers(out, target);
    let runtime = r#"typedef enum {
    NOMO_ASYNC_POLL_READY = 0,
    NOMO_ASYNC_POLL_PENDING = 1
} nomo_async_poll;

typedef enum {
    NOMO_ASYNC_PENDING_NONE = 0,
    NOMO_ASYNC_PENDING_YIELD = 1,
    NOMO_ASYNC_PENDING_TIMER = 2,
    NOMO_ASYNC_PENDING_JOIN = 3,
    NOMO_ASYNC_PENDING_PANIC = 4,
    NOMO_ASYNC_PENDING_CANCEL = 5,
    NOMO_ASYNC_PENDING_CHANNEL = 6,
    NOMO_ASYNC_PENDING_SELECT = 7,
    NOMO_ASYNC_PENDING_IO = 8
} nomo_async_pending_reason;

typedef enum {
    NOMO_ASYNC_TIMER_OUTCOME_NONE = 0,
    NOMO_ASYNC_TIMER_OUTCOME_OK = 1,
    NOMO_ASYNC_TIMER_OUTCOME_LIMIT = 2,
    NOMO_ASYNC_TIMER_OUTCOME_RUNTIME_ERROR = 3
} nomo_async_timer_outcome;

typedef enum {
    NOMO_ASYNC_TASK_FAILURE_NONE = 0,
    NOMO_ASYNC_TASK_FAILURE_CANCELLED = 1,
    NOMO_ASYNC_TASK_FAILURE_TIMEOUT = 2,
    NOMO_ASYNC_TASK_FAILURE_TIMER_LIMIT = 3,
    NOMO_ASYNC_TASK_FAILURE_RUNTIME = 4
} nomo_async_task_failure;

typedef struct nomo_async_context nomo_async_context;
typedef nomo_async_poll (*nomo_async_poll_fn)(void *, nomo_async_context *);
typedef struct nomo_async_select_token nomo_async_select_token;
typedef void (*nomo_async_select_cancel_fn)(void *, nomo_async_context *);

#define NOMO_ASYNC_READY_CAPACITY 64u
#define NOMO_ASYNC_TIMER_CAPACITY 64u
#define NOMO_ASYNC_IO_HANDLE_CAPACITY 64u
#define NOMO_ASYNC_SELECT_MAX_ARMS 8u
#define NOMO_ASYNC_SELECT_PENDING UINT32_MAX

struct nomo_async_select_token {
    nomo_async_context *context;
    void *frame;
    nomo_async_poll_fn poll;
    void *registrations[NOMO_ASYNC_SELECT_MAX_ARMS];
    nomo_async_select_cancel_fn cancel[NOMO_ASYNC_SELECT_MAX_ARMS];
    uint32_t arm_count;
    uint32_t winner;
    uint8_t active;
    uint8_t suspended;
    uint8_t enqueued;
};

typedef struct {
    uint32_t slot;
    uint32_t generation;
    int64_t deadline_millis;
    nomo_async_select_token *select_token;
    uint32_t select_arm;
    uint8_t armed;
    uint8_t expired;
} nomo_async_timer_registration;

typedef struct {
    nomo_async_timer_registration *registration;
    void *frame;
    nomo_async_poll_fn poll;
    int64_t deadline_millis;
    uint32_t generation;
    uint8_t occupied;
} nomo_async_timer_slot;

typedef struct {
    void *frame;
    nomo_async_poll_fn poll;
} nomo_async_ready_slot;

typedef struct {
    nomo_socket handle;
    uint32_t generation;
    uint8_t occupied;
    uint8_t read_busy;
    uint8_t write_busy;
} nomo_async_io_handle_slot;

struct nomo_async_context {
    nomo_async_reactor reactor;
    uint64_t poll_count;
    uint64_t yield_count;
    uint64_t frame_drops;
    uint64_t live_frames;
    uint64_t peak_live_frames;
    uint64_t ready_queue_enqueues;
    uint64_t ready_queue_dequeues;
    uint64_t ready_queue_saturations;
    uint64_t ready_queue_cancellations;
    uint64_t task_spawns;
    uint64_t publication_moves;
    uint64_t task_joins;
    uint64_t join_suspensions;
    uint64_t task_cancellations;
    uint64_t deadline_registrations;
    uint64_t deadline_expirations;
    uint64_t deadline_cancellations;
    uint64_t timer_registrations;
    uint64_t timer_expirations;
    uint64_t timer_cancellations;
    uint64_t channel_constructions;
    uint64_t channel_sends;
    uint64_t channel_receives;
    uint64_t channel_buffered_sends;
    uint64_t channel_buffered_receives;
    uint64_t channel_direct_handoffs;
    uint64_t channel_send_suspensions;
    uint64_t channel_receive_suspensions;
    uint64_t channel_wakeups;
    uint64_t channel_closes;
    uint64_t channel_cancellations;
    uint64_t select_registrations;
    uint64_t select_immediate_wins;
    uint64_t select_suspended_wins;
    uint64_t select_loser_cancellations;
    uint64_t select_cancellations;
    uint64_t live_channel_buffered_elements;
    uint64_t peak_live_channel_buffered_elements;
    uint64_t live_channel_send_waiters;
    uint64_t peak_live_channel_send_waiters;
    uint64_t live_channel_receive_waiters;
    uint64_t peak_live_channel_receive_waiters;
    uint64_t live_timers;
    uint64_t peak_live_timers;
    uint64_t io_connect_starts;
    uint64_t io_read_starts;
    uint64_t io_write_starts;
    uint64_t io_ready_completions;
    uint64_t io_timeouts;
    uint64_t io_cancellations;
    uint64_t io_errors;
    uint64_t live_io_handles;
    uint64_t peak_live_io_handles;
    uint64_t live_io_operations;
    uint64_t peak_live_io_operations;
    uint64_t retained_io_bytes;
    uint64_t peak_retained_io_bytes;
    uint64_t blocking_pool_initializations;
    uint64_t blocking_threads_started;
    uint64_t blocking_threads_retired;
    uint64_t blocking_jobs_queued;
    uint64_t blocking_jobs_started;
    uint64_t blocking_jobs_completed;
    uint64_t blocking_jobs_cancelled;
    uint64_t blocking_queue_saturations;
    uint64_t live_blocking_threads;
    uint64_t peak_live_blocking_threads;
    uint64_t live_blocking_jobs;
    uint64_t peak_live_blocking_jobs;
    uint32_t next_timer_generation;
    uint32_t next_io_handle_generation;
    void *blocking_pool;
    void *current_frame;
    nomo_async_poll_fn current_poll;
    nomo_async_pending_reason pending_reason;
    nomo_async_ready_slot ready[NOMO_ASYNC_READY_CAPACITY];
    nomo_async_timer_slot timers[NOMO_ASYNC_TIMER_CAPACITY];
    nomo_async_io_handle_slot io_handles[NOMO_ASYNC_IO_HANDLE_CAPACITY];
    uint32_t ready_head;
    uint32_t ready_tail;
    uint32_t ready_count;
    uint8_t runtime_failed;
    uint8_t panicking;
    uint8_t panic_message_owned;
    nomo_string panic_message;
};

static int nomo_async_io_handle_insert(
    nomo_async_context *context,
    nomo_socket handle,
    uint32_t *slot_out,
    uint32_t *generation_out
) {
    uint32_t selected = NOMO_ASYNC_IO_HANDLE_CAPACITY;
    for (uint32_t index = 0u; index < NOMO_ASYNC_IO_HANDLE_CAPACITY; index += 1u) {
        if (context->io_handles[index].occupied == 0u) {
            selected = index;
            break;
        }
    }
    if (selected == NOMO_ASYNC_IO_HANDLE_CAPACITY) {
        return 1;
    }
    context->next_io_handle_generation += 1u;
    if (context->next_io_handle_generation == 0u) {
        context->next_io_handle_generation = 1u;
    }
    nomo_async_io_handle_slot *slot = &context->io_handles[selected];
    slot->handle = handle;
    slot->generation = context->next_io_handle_generation;
    slot->occupied = 1u;
    slot->read_busy = 0u;
    slot->write_busy = 0u;
    *slot_out = selected;
    *generation_out = slot->generation;
    context->live_io_handles += 1u;
    if (context->live_io_handles > context->peak_live_io_handles) {
        context->peak_live_io_handles = context->live_io_handles;
    }
    return 0;
}

static nomo_socket nomo_async_io_handle_get(
    nomo_async_context *context,
    uint32_t slot_index,
    uint32_t generation
) {
    if (slot_index >= NOMO_ASYNC_IO_HANDLE_CAPACITY) {
        return NOMO_INVALID_SOCKET;
    }
    nomo_async_io_handle_slot *slot = &context->io_handles[slot_index];
    if (slot->occupied == 0u || slot->generation != generation) {
        return NOMO_INVALID_SOCKET;
    }
    return slot->handle;
}

#define NOMO_ASYNC_IO_DIRECTION_READ 1u
#define NOMO_ASYNC_IO_DIRECTION_WRITE 2u

static int nomo_async_io_handle_acquire(
    nomo_async_context *context,
    uint32_t slot_index,
    uint32_t generation,
    uint32_t direction
) {
    if (slot_index >= NOMO_ASYNC_IO_HANDLE_CAPACITY) {
        return 1;
    }
    nomo_async_io_handle_slot *slot = &context->io_handles[slot_index];
    if (slot->occupied == 0u || slot->generation != generation) {
        return 1;
    }
    uint8_t *busy = direction == NOMO_ASYNC_IO_DIRECTION_READ
        ? &slot->read_busy
        : &slot->write_busy;
    if (*busy != 0u) {
        return 2;
    }
    *busy = 1u;
    return 0;
}

static void nomo_async_io_handle_release(
    nomo_async_context *context,
    uint32_t slot_index,
    uint32_t generation,
    uint32_t direction
) {
    if (slot_index >= NOMO_ASYNC_IO_HANDLE_CAPACITY) {
        return;
    }
    nomo_async_io_handle_slot *slot = &context->io_handles[slot_index];
    if (slot->occupied == 0u || slot->generation != generation) {
        return;
    }
    uint8_t *busy = direction == NOMO_ASYNC_IO_DIRECTION_READ
        ? &slot->read_busy
        : &slot->write_busy;
    *busy = 0u;
}

static void nomo_async_io_handle_close(
    nomo_async_context *context,
    uint32_t slot_index,
    uint32_t generation
) {
    if (slot_index >= NOMO_ASYNC_IO_HANDLE_CAPACITY) {
        return;
    }
    nomo_async_io_handle_slot *slot = &context->io_handles[slot_index];
    if (slot->occupied == 0u || slot->generation != generation) {
        return;
    }
    NOMO_SOCKET_CLOSE(slot->handle);
    slot->handle = NOMO_INVALID_SOCKET;
    slot->occupied = 0u;
    slot->read_busy = 0u;
    slot->write_busy = 0u;
    slot->generation += 1u;
    if (slot->generation == 0u) {
        slot->generation = 1u;
    }
    if (context->live_io_handles > 0u) {
        context->live_io_handles -= 1u;
    }
}

static void nomo_async_io_handle_close_callback(
    void *raw_context,
    uint32_t slot_index,
    uint32_t generation
) {
    nomo_async_io_handle_close(
        (nomo_async_context *)raw_context,
        slot_index,
        generation
    );
}

static void nomo_async_io_handle_shutdown(nomo_async_context *context) {
    for (uint32_t index = 0u; index < NOMO_ASYNC_IO_HANDLE_CAPACITY; index += 1u) {
        nomo_async_io_handle_slot *slot = &context->io_handles[index];
        if (slot->occupied == 0u) {
            continue;
        }
        nomo_async_io_handle_close(context, index, slot->generation);
    }
}

static int nomo_async_ready_enqueue(
    nomo_async_context *context,
    void *frame,
    nomo_async_poll_fn poll
) {
    if (context->ready_count == NOMO_ASYNC_READY_CAPACITY) {
        context->ready_queue_saturations += 1u;
        return 1;
    }
    nomo_async_ready_slot *slot = &context->ready[context->ready_tail];
    slot->frame = frame;
    slot->poll = poll;
    context->ready_tail = (context->ready_tail + 1u) % NOMO_ASYNC_READY_CAPACITY;
    context->ready_count += 1u;
    context->ready_queue_enqueues += 1u;
    return 0;
}

static void nomo_async_select_init(
    nomo_async_select_token *token,
    uint32_t arm_count,
    nomo_async_context *context
) {
    memset(token, 0, sizeof(*token));
    token->context = context;
    token->frame = context->current_frame;
    token->poll = context->current_poll;
    token->arm_count = arm_count;
    token->winner = NOMO_ASYNC_SELECT_PENDING;
    token->active = 1u;
}

static void nomo_async_select_register(
    nomo_async_select_token *token,
    uint32_t arm,
    void *registration,
    nomo_async_select_cancel_fn cancel
) {
    if (token == NULL
        || token->active == 0u
        || token->winner != NOMO_ASYNC_SELECT_PENDING
        || arm >= token->arm_count
        || arm >= NOMO_ASYNC_SELECT_MAX_ARMS) {
        return;
    }
    token->registrations[arm] = registration;
    token->cancel[arm] = cancel;
    token->context->select_registrations += 1u;
}

static int nomo_async_select_claim(
    nomo_async_select_token *token,
    uint32_t arm
) {
    if (token == NULL
        || token->active == 0u
        || token->winner != NOMO_ASYNC_SELECT_PENDING
        || arm >= token->arm_count) {
        return 0;
    }
    token->winner = arm;
    for (uint32_t index = 0u; index < token->arm_count; index += 1u) {
        if (index == arm || token->registrations[index] == NULL) {
            continue;
        }
        token->cancel[index](token->registrations[index], token->context);
        token->registrations[index] = NULL;
        token->cancel[index] = NULL;
        token->context->select_loser_cancellations += 1u;
    }
    return 1;
}

static int nomo_async_select_immediate_win(
    nomo_async_select_token *token,
    uint32_t arm
) {
    if (nomo_async_select_claim(token, arm) == 0) {
        return 0;
    }
    token->context->select_immediate_wins += 1u;
    return 1;
}

static void nomo_async_select_suspend(nomo_async_select_token *token) {
    token->suspended = 1u;
    token->context->pending_reason = NOMO_ASYNC_PENDING_SELECT;
}

static void nomo_async_select_wake(nomo_async_select_token *token) {
    if (token == NULL
        || token->active == 0u
        || token->suspended == 0u
        || token->enqueued != 0u) {
        return;
    }
    token->enqueued = 1u;
    token->context->select_suspended_wins += 1u;
    if (nomo_async_ready_enqueue(token->context, token->frame, token->poll) != 0) {
        token->context->runtime_failed = 1u;
    }
}

static void nomo_async_select_complete(
    nomo_async_select_token *token,
    uint32_t winner
) {
    if (token == NULL || token->active == 0u || token->winner != winner) {
        return;
    }
    token->registrations[winner] = NULL;
    token->cancel[winner] = NULL;
    token->active = 0u;
    token->suspended = 0u;
    token->enqueued = 0u;
}

static void nomo_async_select_cancel(nomo_async_select_token *token) {
    if (token == NULL || token->active == 0u) {
        return;
    }
    for (uint32_t index = 0u; index < token->arm_count; index += 1u) {
        if (token->registrations[index] == NULL) {
            continue;
        }
        token->cancel[index](token->registrations[index], token->context);
        token->registrations[index] = NULL;
        token->cancel[index] = NULL;
        token->context->select_cancellations += 1u;
    }
    token->active = 0u;
    token->suspended = 0u;
    token->enqueued = 0u;
}

static int nomo_async_ready_dequeue(
    nomo_async_context *context,
    void **frame,
    nomo_async_poll_fn *poll
) {
    if (context->ready_count == 0u) {
        return 1;
    }
    nomo_async_ready_slot *slot = &context->ready[context->ready_head];
    *frame = slot->frame;
    *poll = slot->poll;
    slot->frame = NULL;
    slot->poll = NULL;
    context->ready_head = (context->ready_head + 1u) % NOMO_ASYNC_READY_CAPACITY;
    context->ready_count -= 1u;
    context->ready_queue_dequeues += 1u;
    return 0;
}

static void nomo_async_ready_cancel_frame(
    nomo_async_context *context,
    void *frame
) {
    uint32_t original_count = context->ready_count;
    uint32_t kept_count = 0u;
    for (uint32_t offset = 0u; offset < original_count; offset += 1u) {
        uint32_t source_index =
            (context->ready_head + offset) % NOMO_ASYNC_READY_CAPACITY;
        nomo_async_ready_slot *source = &context->ready[source_index];
        if (source->frame == frame) {
            source->frame = NULL;
            source->poll = NULL;
            context->ready_queue_cancellations += 1u;
            continue;
        }
        uint32_t destination_index =
            (context->ready_head + kept_count) % NOMO_ASYNC_READY_CAPACITY;
        if (destination_index != source_index) {
            context->ready[destination_index] = *source;
            source->frame = NULL;
            source->poll = NULL;
        }
        kept_count += 1u;
    }
    context->ready_count = kept_count;
    context->ready_tail =
        (context->ready_head + kept_count) % NOMO_ASYNC_READY_CAPACITY;
}

static nomo_async_poll nomo_async_timer_start(
    nomo_async_timer_registration *registration,
    int64_t duration_millis,
    nomo_async_context *context,
    nomo_async_timer_outcome *outcome,
    nomo_async_select_token *select_token,
    uint32_t select_arm
) {
    if (duration_millis <= 0) {
        *outcome = NOMO_ASYNC_TIMER_OUTCOME_OK;
        return NOMO_ASYNC_POLL_READY;
    }
    if (registration->armed != 0u || registration->expired != 0u) {
        *outcome = NOMO_ASYNC_TIMER_OUTCOME_RUNTIME_ERROR;
        return NOMO_ASYNC_POLL_READY;
    }
    uint32_t slot_index = NOMO_ASYNC_TIMER_CAPACITY;
    for (uint32_t index = 0u; index < NOMO_ASYNC_TIMER_CAPACITY; index += 1u) {
        if (context->timers[index].occupied == 0u) {
            slot_index = index;
            break;
        }
    }
    if (slot_index == NOMO_ASYNC_TIMER_CAPACITY) {
        *outcome = NOMO_ASYNC_TIMER_OUTCOME_LIMIT;
        return NOMO_ASYNC_POLL_READY;
    }
    int64_t now = nomo_time_monotonic_millis();
    int64_t deadline = duration_millis > INT64_MAX - now
        ? INT64_MAX
        : now + duration_millis;
    context->next_timer_generation += 1u;
    if (context->next_timer_generation == 0u) {
        context->next_timer_generation = 1u;
    }
    registration->slot = slot_index;
    registration->generation = context->next_timer_generation;
    registration->deadline_millis = deadline;
    registration->select_token = select_token;
    registration->select_arm = select_arm;
    registration->armed = 1u;
    registration->expired = 0u;
    nomo_async_timer_slot *slot = &context->timers[slot_index];
    slot->registration = registration;
    slot->frame = context->current_frame;
    slot->poll = context->current_poll;
    slot->deadline_millis = deadline;
    slot->generation = registration->generation;
    slot->occupied = 1u;
    context->timer_registrations += 1u;
    context->live_timers += 1u;
    if (context->live_timers > context->peak_live_timers) {
        context->peak_live_timers = context->live_timers;
    }
    *outcome = NOMO_ASYNC_TIMER_OUTCOME_NONE;
    context->pending_reason = NOMO_ASYNC_PENDING_TIMER;
    return NOMO_ASYNC_POLL_PENDING;
}

static nomo_async_poll nomo_async_timer_resume(
    nomo_async_timer_registration *registration,
    nomo_async_context *context,
    nomo_async_timer_outcome *outcome
) {
    if (registration->expired != 0u) {
        registration->expired = 0u;
        registration->select_token = NULL;
        *outcome = NOMO_ASYNC_TIMER_OUTCOME_OK;
        return NOMO_ASYNC_POLL_READY;
    }
    if (registration->armed != 0u) {
        context->pending_reason = NOMO_ASYNC_PENDING_TIMER;
        return NOMO_ASYNC_POLL_PENDING;
    }
    *outcome = NOMO_ASYNC_TIMER_OUTCOME_RUNTIME_ERROR;
    return NOMO_ASYNC_POLL_READY;
}

static nomo_async_timer_outcome nomo_async_deadline_arm(
    nomo_async_timer_registration *registration,
    int64_t duration_millis,
    nomo_async_context *context
) {
    if (duration_millis <= 0) {
        return NOMO_ASYNC_TIMER_OUTCOME_OK;
    }
    if (registration->armed != 0u || registration->expired != 0u) {
        return NOMO_ASYNC_TIMER_OUTCOME_RUNTIME_ERROR;
    }
    uint32_t slot_index = NOMO_ASYNC_TIMER_CAPACITY;
    for (uint32_t index = 0u; index < NOMO_ASYNC_TIMER_CAPACITY; index += 1u) {
        if (context->timers[index].occupied == 0u) {
            slot_index = index;
            break;
        }
    }
    if (slot_index == NOMO_ASYNC_TIMER_CAPACITY) {
        return NOMO_ASYNC_TIMER_OUTCOME_LIMIT;
    }
    int64_t now = nomo_time_monotonic_millis();
    int64_t deadline = duration_millis > INT64_MAX - now
        ? INT64_MAX
        : now + duration_millis;
    context->next_timer_generation += 1u;
    if (context->next_timer_generation == 0u) {
        context->next_timer_generation = 1u;
    }
    registration->slot = slot_index;
    registration->generation = context->next_timer_generation;
    registration->deadline_millis = deadline;
    registration->select_token = NULL;
    registration->select_arm = 0u;
    registration->armed = 1u;
    registration->expired = 0u;
    nomo_async_timer_slot *slot = &context->timers[slot_index];
    slot->registration = registration;
    slot->frame = context->current_frame;
    slot->poll = context->current_poll;
    slot->deadline_millis = deadline;
    slot->generation = registration->generation;
    slot->occupied = 1u;
    context->timer_registrations += 1u;
    context->deadline_registrations += 1u;
    context->live_timers += 1u;
    if (context->live_timers > context->peak_live_timers) {
        context->peak_live_timers = context->live_timers;
    }
    return NOMO_ASYNC_TIMER_OUTCOME_NONE;
}

static void nomo_async_timer_disarm(
    nomo_async_timer_registration *registration,
    nomo_async_context *context
) {
    if (registration->armed == 0u) {
        registration->expired = 0u;
        registration->select_token = NULL;
        return;
    }
    if (registration->slot < NOMO_ASYNC_TIMER_CAPACITY) {
        nomo_async_timer_slot *slot = &context->timers[registration->slot];
        if (slot->occupied != 0u
            && slot->generation == registration->generation
            && slot->registration == registration) {
            slot->occupied = 0u;
            slot->registration = NULL;
            slot->frame = NULL;
            slot->poll = NULL;
            if (context->live_timers > 0u) {
                context->live_timers -= 1u;
            }
            context->timer_cancellations += 1u;
        }
    }
    registration->armed = 0u;
    registration->expired = 0u;
    registration->select_token = NULL;
}

static void nomo_async_timer_select_cancel(
    void *raw_registration,
    nomo_async_context *context
) {
    nomo_async_timer_disarm(
        (nomo_async_timer_registration *)raw_registration,
        context
    );
}

static int nomo_async_deadline_due(
    nomo_async_timer_registration *registration,
    nomo_async_context *context
) {
    if (registration->expired != 0u) {
        return 1;
    }
    if (registration->armed == 0u
        || nomo_time_monotonic_millis() < registration->deadline_millis) {
        return 0;
    }
    if (registration->slot < NOMO_ASYNC_TIMER_CAPACITY) {
        nomo_async_timer_slot *slot = &context->timers[registration->slot];
        if (slot->occupied != 0u
            && slot->generation == registration->generation
            && slot->registration == registration) {
            slot->occupied = 0u;
            slot->registration = NULL;
            slot->frame = NULL;
            slot->poll = NULL;
            if (context->live_timers > 0u) {
                context->live_timers -= 1u;
            }
            context->timer_expirations += 1u;
        }
    }
    registration->armed = 0u;
    registration->expired = 1u;
    return 1;
}

static const char *nomo_async_task_failure_code(nomo_async_task_failure failure) {
    switch (failure) {
        case NOMO_ASYNC_TASK_FAILURE_CANCELLED:
            return "cancelled";
        case NOMO_ASYNC_TASK_FAILURE_TIMEOUT:
            return "timeout";
        case NOMO_ASYNC_TASK_FAILURE_TIMER_LIMIT:
            return "timer_limit";
        case NOMO_ASYNC_TASK_FAILURE_RUNTIME:
            return "runtime";
        case NOMO_ASYNC_TASK_FAILURE_NONE:
        default:
            return "runtime";
    }
}

static const char *nomo_async_task_failure_message(nomo_async_task_failure failure) {
    switch (failure) {
        case NOMO_ASYNC_TASK_FAILURE_CANCELLED:
            return "structured task was cancelled";
        case NOMO_ASYNC_TASK_FAILURE_TIMEOUT:
            return "structured task deadline elapsed";
        case NOMO_ASYNC_TASK_FAILURE_TIMER_LIMIT:
            return "owner executor timer capacity is exhausted";
        case NOMO_ASYNC_TASK_FAILURE_RUNTIME:
            return "owner executor entered an invalid deadline state";
        case NOMO_ASYNC_TASK_FAILURE_NONE:
        default:
            return "structured task failed";
    }
}

static int nomo_async_wait_next(nomo_async_context *context) {
    uint32_t selected = NOMO_ASYNC_TIMER_CAPACITY;
    for (uint32_t index = 0u; index < NOMO_ASYNC_TIMER_CAPACITY; index += 1u) {
        nomo_async_timer_slot *candidate = &context->timers[index];
        if (candidate->occupied == 0u) {
            continue;
        }
        if (selected == NOMO_ASYNC_TIMER_CAPACITY
            || candidate->deadline_millis < context->timers[selected].deadline_millis
            || (candidate->deadline_millis == context->timers[selected].deadline_millis
                && candidate->generation < context->timers[selected].generation)) {
            selected = index;
        }
    }
    if (selected == NOMO_ASYNC_TIMER_CAPACITY
        && context->reactor.live_registrations == 0u) {
        return 1;
    }
    nomo_async_timer_slot *slot = selected == NOMO_ASYNC_TIMER_CAPACITY
        ? NULL
        : &context->timers[selected];
    while (1) {
        int64_t remaining = -1;
        if (slot != NULL) {
            int64_t now = nomo_time_monotonic_millis();
            if (now >= slot->deadline_millis) {
                break;
            }
            remaining = slot->deadline_millis - now;
            if (remaining > 60000) {
                remaining = 60000;
            }
        }
        uint8_t had_completion = 0u;
        if (nomo_async_reactor_wait(
                &context->reactor,
                remaining,
                &had_completion
            ) != 0) {
            context->runtime_failed = 1u;
            return 1;
        }
        if (had_completion != 0u) {
            return context->runtime_failed != 0u;
        }
        if (slot == NULL) {
            context->runtime_failed = 1u;
            return 1;
        }
        if (nomo_time_monotonic_millis() >= slot->deadline_millis) {
            break;
        }
    }
    nomo_async_timer_registration *registration = slot->registration;
    void *frame = slot->frame;
    nomo_async_poll_fn poll = slot->poll;
    if (registration == NULL
        || registration->armed == 0u
        || registration->generation != slot->generation) {
        return 1;
    }
    nomo_async_select_token *select_token = registration->select_token;
    if (select_token != NULL
        && nomo_async_select_claim(select_token, registration->select_arm) == 0) {
        context->runtime_failed = 1u;
        return 1;
    }
    registration->armed = 0u;
    registration->expired = 1u;
    slot->occupied = 0u;
    slot->registration = NULL;
    slot->frame = NULL;
    slot->poll = NULL;
    if (context->live_timers > 0u) {
        context->live_timers -= 1u;
    }
    context->timer_expirations += 1u;
    if (select_token != NULL) {
        nomo_async_select_wake(select_token);
        return context->runtime_failed != 0u;
    }
    return nomo_async_ready_enqueue(context, frame, poll);
}

static nomo_async_poll nomo_async_poll_task(
    void *frame,
    nomo_async_poll_fn poll,
    nomo_async_context *context
) {
    context->current_frame = frame;
    context->current_poll = poll;
    context->pending_reason = NOMO_ASYNC_PENDING_NONE;
    nomo_async_poll status = poll(frame, context);
    context->current_frame = NULL;
    context->current_poll = NULL;
    return status;
}

static int nomo_async_executor_run_root(
    void *frame,
    nomo_async_poll_fn poll,
    nomo_async_context *context
) {
    nomo_async_poll status = nomo_async_poll_task(frame, poll, context);
    if (context->runtime_failed != 0u) {
        return 1;
    }
    if (context->panicking != 0u) {
        return 1;
    }
    if (status == NOMO_ASYNC_POLL_READY) {
        return 0;
    }
    if (context->pending_reason == NOMO_ASYNC_PENDING_YIELD) {
        if (nomo_async_ready_enqueue(context, frame, poll) != 0) {
            return 1;
        }
    } else if (context->pending_reason != NOMO_ASYNC_PENDING_TIMER
        && context->pending_reason != NOMO_ASYNC_PENDING_JOIN
        && context->pending_reason != NOMO_ASYNC_PENDING_CHANNEL
        && context->pending_reason != NOMO_ASYNC_PENDING_SELECT
        && context->pending_reason != NOMO_ASYNC_PENDING_IO) {
        return 1;
    }
    while (context->ready_count != 0u
        || context->live_timers != 0u
        || context->reactor.live_registrations != 0u) {
        if (context->ready_count == 0u
            && nomo_async_wait_next(context) != 0) {
            return 1;
        }
        if (context->ready_count == 0u) {
            continue;
        }
        void *ready_frame = NULL;
        nomo_async_poll_fn ready_poll = NULL;
        if (nomo_async_ready_dequeue(context, &ready_frame, &ready_poll) != 0) {
            return 1;
        }
        status = nomo_async_poll_task(ready_frame, ready_poll, context);
        if (context->runtime_failed != 0u) {
            return 1;
        }
        if (context->panicking != 0u) {
            return 1;
        }
        if (status == NOMO_ASYNC_POLL_PENDING) {
            if (context->pending_reason == NOMO_ASYNC_PENDING_YIELD) {
                if (nomo_async_ready_enqueue(context, ready_frame, ready_poll) != 0) {
                    return 1;
                }
            } else if (context->pending_reason != NOMO_ASYNC_PENDING_TIMER
                && context->pending_reason != NOMO_ASYNC_PENDING_JOIN
                && context->pending_reason != NOMO_ASYNC_PENDING_CHANNEL
                && context->pending_reason != NOMO_ASYNC_PENDING_SELECT
                && context->pending_reason != NOMO_ASYNC_PENDING_IO) {
                return 1;
            }
        }
    }
    return status == NOMO_ASYNC_POLL_READY ? 0 : 1;
}

static int nomo_async_metrics_export(const nomo_async_context *context) {
    const char *path = getenv("NOMO_ASYNC_METRICS_PATH");
    if (path == NULL || path[0] == '\0') {
        return 0;
    }
    FILE *output = fopen(path, "wb");
    if (output == NULL) {
        return 1;
    }
    int write_status = fprintf(
        output,
        "{\n"
        "  \"schema\": 1,\n"
        "  \"runtime\": \"nomo-c99-current-thread\",\n"
        "  \"runtime_abi\": 1,\n"
        "  \"counter_catalog_schema\": 1,\n"
        "  \"counters\": {\n"
        "    \"poll_calls\": %" PRIu64 ",\n"
        "    \"cooperative_yields\": %" PRIu64 ",\n"
        "    \"frame_allocations\": 0,\n"
        "    \"frame_drops\": %" PRIu64 ",\n"
        "    \"peak_live_frames\": %" PRIu64 ",\n"
        "    \"ready_queue_enqueues\": %" PRIu64 ",\n"
        "    \"ready_queue_dequeues\": %" PRIu64 ",\n"
        "    \"ready_queue_saturations\": %" PRIu64 ",\n"
        "    \"ready_queue_cancellations\": %" PRIu64 ",\n"
        "    \"task_spawns\": %" PRIu64 ",\n"
        "    \"publication_moves\": %" PRIu64 ",\n"
        "    \"task_joins\": %" PRIu64 ",\n"
        "    \"join_suspensions\": %" PRIu64 ",\n"
        "    \"task_cancellations\": %" PRIu64 ",\n"
        "    \"deadline_registrations\": %" PRIu64 ",\n"
        "    \"deadline_expirations\": %" PRIu64 ",\n"
        "    \"deadline_cancellations\": %" PRIu64 ",\n"
        "    \"timer_registrations\": %" PRIu64 ",\n"
        "    \"timer_expirations\": %" PRIu64 ",\n"
        "    \"timer_cancellations\": %" PRIu64 ",\n"
        "    \"channel_constructions\": %" PRIu64 ",\n"
        "    \"channel_sends\": %" PRIu64 ",\n"
        "    \"channel_receives\": %" PRIu64 ",\n"
        "    \"channel_buffered_sends\": %" PRIu64 ",\n"
        "    \"channel_buffered_receives\": %" PRIu64 ",\n"
        "    \"channel_direct_handoffs\": %" PRIu64 ",\n"
        "    \"channel_send_suspensions\": %" PRIu64 ",\n"
        "    \"channel_receive_suspensions\": %" PRIu64 ",\n"
        "    \"channel_wakeups\": %" PRIu64 ",\n"
        "    \"channel_closes\": %" PRIu64 ",\n"
        "    \"channel_cancellations\": %" PRIu64 ",\n"
        "    \"select_registrations\": %" PRIu64 ",\n"
        "    \"select_immediate_wins\": %" PRIu64 ",\n"
        "    \"select_suspended_wins\": %" PRIu64 ",\n"
        "    \"select_loser_cancellations\": %" PRIu64 ",\n"
        "    \"select_cancellations\": %" PRIu64 ",\n"
        "    \"live_channel_buffered_elements\": %" PRIu64 ",\n"
        "    \"peak_live_channel_buffered_elements\": %" PRIu64 ",\n"
        "    \"live_channel_send_waiters\": %" PRIu64 ",\n"
        "    \"peak_live_channel_send_waiters\": %" PRIu64 ",\n"
        "    \"live_channel_receive_waiters\": %" PRIu64 ",\n"
        "    \"peak_live_channel_receive_waiters\": %" PRIu64 ",\n"
        "    \"live_timers\": %" PRIu64 ",\n"
        "    \"peak_live_timers\": %" PRIu64 ",\n"
        "    \"reactor_initializations\": %" PRIu64 ",\n"
        "    \"reactor_waits\": %" PRIu64 ",\n"
        "    \"reactor_timeouts\": %" PRIu64 ",\n"
        "    \"reactor_completions\": %" PRIu64 ",\n"
        "    \"reactor_errors\": %" PRIu64 ",\n"
        "    \"reactor_shutdowns\": %" PRIu64 ",\n"
        "    \"reactor_registrations\": %" PRIu64 ",\n"
        "    \"reactor_deregistrations\": %" PRIu64 ",\n"
        "    \"reactor_reregistrations\": %" PRIu64 ",\n"
        "    \"live_reactor_registrations\": %" PRIu64 ",\n"
        "    \"peak_live_reactor_registrations\": %" PRIu64 ",\n"
        "    \"live_reactors\": %" PRIu64 ",\n"
        "    \"peak_live_reactors\": %" PRIu64 ",\n"
        "    \"io_connect_starts\": %" PRIu64 ",\n"
        "    \"io_read_starts\": %" PRIu64 ",\n"
        "    \"io_write_starts\": %" PRIu64 ",\n"
        "    \"io_ready_completions\": %" PRIu64 ",\n"
        "    \"io_timeouts\": %" PRIu64 ",\n"
        "    \"io_cancellations\": %" PRIu64 ",\n"
        "    \"io_errors\": %" PRIu64 ",\n"
        "    \"live_io_handles\": %" PRIu64 ",\n"
        "    \"peak_live_io_handles\": %" PRIu64 ",\n"
        "    \"live_io_operations\": %" PRIu64 ",\n"
        "    \"peak_live_io_operations\": %" PRIu64 ",\n"
        "    \"retained_io_bytes\": %" PRIu64 ",\n"
        "    \"peak_retained_io_bytes\": %" PRIu64 ",\n"
        "    \"blocking_pool_initializations\": %" PRIu64 ",\n"
        "    \"blocking_threads_started\": %" PRIu64 ",\n"
        "    \"blocking_threads_retired\": %" PRIu64 ",\n"
        "    \"blocking_jobs_queued\": %" PRIu64 ",\n"
        "    \"blocking_jobs_started\": %" PRIu64 ",\n"
        "    \"blocking_jobs_completed\": %" PRIu64 ",\n"
        "    \"blocking_jobs_cancelled\": %" PRIu64 ",\n"
        "    \"blocking_queue_saturations\": %" PRIu64 ",\n"
        "    \"live_blocking_threads\": %" PRIu64 ",\n"
        "    \"peak_live_blocking_threads\": %" PRIu64 ",\n"
        "    \"live_blocking_jobs\": %" PRIu64 ",\n"
        "    \"peak_live_blocking_jobs\": %" PRIu64 "\n"
        "  },\n"
        "  \"unavailable\": {\n"
        "    \"local_retain\": \"ARC primitive instrumentation is not implemented in this P1 slice\",\n"
        "    \"local_release\": \"ARC primitive instrumentation is not implemented in this P1 slice\",\n"
        "    \"atomic_retain\": \"cross-shard shared ARC is not implemented in the current-thread channel slice\",\n"
        "    \"atomic_release\": \"cross-shard shared ARC is not implemented in the current-thread channel slice\",\n"
        "    \"cow_detach_count\": \"cross-shard COW detach instrumentation is not implemented in the current-thread channel slice\",\n"
        "    \"cow_detach_bytes\": \"cross-shard COW detach instrumentation is not implemented in the current-thread channel slice\",\n"
        "    \"publish_copy_count\": \"cross-shard publication copying is not implemented in the current-thread channel slice\",\n"
        "    \"publish_copy_bytes\": \"cross-shard publication copying is not implemented in the current-thread channel slice\"\n"
        "  }\n"
        "}\n",
        context->poll_count,
        context->yield_count,
        context->frame_drops,
        context->peak_live_frames,
        context->ready_queue_enqueues,
        context->ready_queue_dequeues,
        context->ready_queue_saturations,
        context->ready_queue_cancellations,
        context->task_spawns,
        context->publication_moves,
        context->task_joins,
        context->join_suspensions,
        context->task_cancellations,
        context->deadline_registrations,
        context->deadline_expirations,
        context->deadline_cancellations,
        context->timer_registrations,
        context->timer_expirations,
        context->timer_cancellations,
        context->channel_constructions,
        context->channel_sends,
        context->channel_receives,
        context->channel_buffered_sends,
        context->channel_buffered_receives,
        context->channel_direct_handoffs,
        context->channel_send_suspensions,
        context->channel_receive_suspensions,
        context->channel_wakeups,
        context->channel_closes,
        context->channel_cancellations,
        context->select_registrations,
        context->select_immediate_wins,
        context->select_suspended_wins,
        context->select_loser_cancellations,
        context->select_cancellations,
        context->live_channel_buffered_elements,
        context->peak_live_channel_buffered_elements,
        context->live_channel_send_waiters,
        context->peak_live_channel_send_waiters,
        context->live_channel_receive_waiters,
        context->peak_live_channel_receive_waiters,
        context->live_timers,
        context->peak_live_timers,
        context->reactor.initializations,
        context->reactor.waits,
        context->reactor.timeouts,
        context->reactor.completions,
        context->reactor.errors,
        context->reactor.shutdowns,
        context->reactor.registrations,
        context->reactor.deregistrations,
        context->reactor.reregistrations,
        context->reactor.live_registrations,
        context->reactor.peak_live_registrations,
        context->reactor.live,
        context->reactor.peak_live,
        context->io_connect_starts,
        context->io_read_starts,
        context->io_write_starts,
        context->io_ready_completions,
        context->io_timeouts,
        context->io_cancellations,
        context->io_errors,
        context->live_io_handles,
        context->peak_live_io_handles,
        context->live_io_operations,
        context->peak_live_io_operations,
        context->retained_io_bytes,
        context->peak_retained_io_bytes,
        context->blocking_pool_initializations,
        context->blocking_threads_started,
        context->blocking_threads_retired,
        context->blocking_jobs_queued,
        context->blocking_jobs_started,
        context->blocking_jobs_completed,
        context->blocking_jobs_cancelled,
        context->blocking_queue_saturations,
        context->live_blocking_threads,
        context->peak_live_blocking_threads,
        context->live_blocking_jobs,
        context->peak_live_blocking_jobs
    );
    int close_status = fclose(output);
    return write_status < 0 || close_status != 0;
}
"#;
    out.push_str(&runtime);
}

pub(super) fn emit_async_function(
    out: &mut String,
    function: &Function,
    async_names: &BTreeSet<String>,
    functions: &HashMap<&str, &Function>,
    target: &nomo_target::TargetTriple,
) {
    debug_assert!(function.params.iter().all(|parameter| !parameter.mutable));
    debug_assert!(async_names.contains(&function.name));

    let frame_locals = collect_async_frame_locals(function, async_names);
    emit_async_frame_type(out, function, &frame_locals, async_names);
    out.push_str("static nomo_async_poll ");
    out.push_str(&async_poll_ident(&function.name));
    out.push_str(
        "(\n\
             void *raw_frame,\n\
             nomo_async_context *context\n\
         ) {\n\
             ",
    );
    out.push_str(&async_frame_ident(&function.name));
    out.push_str(" *frame = (");
    out.push_str(&async_frame_ident(&function.name));
    out.push_str(
        " *)raw_frame;\n\
             if (frame->started == 0u) {\n\
                 frame->initialized = 1u;\n\
                 frame->started = 1u;\n\
                 frame->context = context;\n\
                 context->live_frames += 1u;\n\
                 if (context->live_frames > context->peak_live_frames) {\n\
                     context->peak_live_frames = context->live_frames;\n\
                 }\n\
             }\n\
             context->poll_count += 1u;\n",
    );
    emit_async_parameter_aliases(out, function, 1);
    out.push_str("    switch (frame->state) {\n");

    let empty_deferred = Vec::new();
    let mut local_owned = Vec::new();
    let mut state = 0u32;
    let mut segment_start = 0usize;
    let mut emitted_terminal_return = false;
    out.push_str("        case 0u: {\n");
    for (index, statement) in function.body.iter().enumerate() {
        if statement_is_within_async_deadline(function, index)
            && statement_task_select(statement).is_none()
        {
            emit_async_deadline_due_check(out, function, async_names, 3);
        }
        if let Some(duration) = statement_async_deadline_enter(statement) {
            out.push_str("            int64_t nomo_async_deadline_millis = (");
            emit_expr(out, duration);
            out.push_str(").nomo_member_millis;\n");
            out.push_str("            if (nomo_async_deadline_millis <= 0) {\n");
            emit_async_deadline_failure(
                out,
                function,
                async_names,
                "NOMO_ASYNC_TASK_FAILURE_TIMEOUT",
                4,
            );
            out.push_str("            }\n");
            out.push_str(
                "            frame->nomo_async_deadline_outcome = nomo_async_deadline_arm(\n\
                             &frame->nomo_async_deadline_timer,\n\
                             nomo_async_deadline_millis,\n\
                             context\n\
                         );\n\
                         if (frame->nomo_async_deadline_outcome == NOMO_ASYNC_TIMER_OUTCOME_LIMIT) {\n",
            );
            emit_async_deadline_failure(
                out,
                function,
                async_names,
                "NOMO_ASYNC_TASK_FAILURE_TIMER_LIMIT",
                4,
            );
            out.push_str(
                "            }\n\
                         if (frame->nomo_async_deadline_outcome != NOMO_ASYNC_TIMER_OUTCOME_NONE) {\n",
            );
            emit_async_deadline_failure(
                out,
                function,
                async_names,
                "NOMO_ASYNC_TASK_FAILURE_RUNTIME",
                4,
            );
            out.push_str(
                "            }\n\
                         frame->nomo_async_deadline_active = 1u;\n",
            );
            continue;
        }
        if statement_is_async_deadline_exit(statement) {
            out.push_str(
                "            if (frame->nomo_async_deadline_active != 0u) {\n\
                             nomo_async_timer_disarm(&frame->nomo_async_deadline_timer, context);\n\
                             frame->nomo_async_deadline_active = 0u;\n\
                             context->deadline_cancellations += 1u;\n\
                         }\n",
            );
            continue;
        }
        if statement_is_async_check_cancelled(statement) {
            continue;
        }
        if let Some(spawn) = statement_structured_spawn(statement) {
            let callee = functions
                .get(spawn.callee)
                .expect("validated structured spawn target exists");
            emit_async_child_init(
                out,
                AsyncCall {
                    callee: spawn.callee,
                    args: spawn.args,
                    binding: None,
                },
                callee,
                index,
                3,
                function,
                &frame_locals,
                &mut local_owned,
            );
            out.push_str("            context->task_spawns += 1u;\n");
            out.push_str("            if (nomo_async_ready_enqueue(context, &frame->");
            out.push_str(&async_child_field(index));
            out.push_str(", ");
            out.push_str(&async_poll_ident(spawn.callee));
            out.push_str(") != 0) {\n            frame->");
            out.push_str(&async_spawn_failed_field(index));
            out.push_str(" = 1u;\n            frame->");
            out.push_str(&async_child_field(index));
            out.push_str(".structured_completed = 1u;\n        }\n");
            continue;
        }
        if let Some(cancel) = statement_structured_cancel(statement) {
            emit_async_structured_cancel(out, function, cancel.handle, 3);
            continue;
        }
        if statement_is_async_suspend(statement, async_names) {
            // A frame local can be used for the first time in a segment by the
            // suspension itself (for example, a Channel handle passed to
            // receive immediately after a structured join). The normal resume
            // alias pass only scans statements before the next suspension, so
            // materialize the missing alias here. A value produced by the
            // immediately preceding suspension already has an alias emitted by
            // its result-binding path.
            for local in frame_locals
                .iter()
                .filter(|local| local.declaration_index < segment_start)
                .filter(|local| local.declaration_index + 1 != segment_start)
                .filter(|local| statement_uses_binding(statement, &local.name))
                .filter(|local| {
                    !function.body[segment_start..index]
                        .iter()
                        .any(|segment_statement| {
                            statement_uses_binding(segment_statement, &local.name)
                        })
                })
            {
                emit_async_frame_alias(out, local, 3);
            }
            let sleep = statement_async_sleep(statement);
            let tcp_connect = statement_async_tcp_connect(statement);
            let tcp_io = statement_async_tcp_io(statement);
            let channel_send = statement_async_channel_send(statement);
            let channel_receive = statement_async_channel_receive(statement);
            let select = statement_task_select(statement);
            let join = statement_structured_join(statement);
            let cancel_join = statement_structured_cancel_join(statement);
            if join.is_some() {
                out.push_str("            context->task_joins += 1u;\n");
            }
            if let Some((_, _, duration)) = sleep {
                out.push_str("            int64_t nomo_async_sleep_millis_");
                out.push_str(&index.to_string());
                out.push_str(" = (");
                emit_expr(out, duration);
                out.push_str(").nomo_member_millis;\n");
            }
            if let Some(arms) = select {
                emit_async_select_operands(out, index, arms, 3);
                if statement_is_within_async_deadline(function, index) {
                    emit_async_deadline_due_check(out, function, async_names, 3);
                }
            }
            let call = statement_async_call(statement, async_names);
            if let Some(call) = call {
                let callee = functions
                    .get(call.callee)
                    .expect("validated suspend call target exists");
                emit_async_child_init(
                    out,
                    call,
                    callee,
                    index,
                    3,
                    function,
                    &frame_locals,
                    &mut local_owned,
                );
            }
            if let Some(send) = channel_send {
                emit_async_publication_move_transfer(
                    out,
                    send.value,
                    function,
                    &frame_locals,
                    &mut local_owned,
                    3,
                );
            }
            let windows_unsupported_tcp =
                target.operating_system() == nomo_target::OperatingSystem::Windows;
            if let Some(connect) = tcp_connect.filter(|_| !windows_unsupported_tcp) {
                let host_temp = async_tcp_connect_host_temp(index);
                out.push_str("            nomo_string ");
                out.push_str(&host_temp);
                out.push_str(" = ");
                emit_expr(out, connect.host);
                out.push_str(";\n");
                emit_value_retain_value_if_needed(
                    out,
                    &host_temp,
                    &ValueType::String,
                    connect.host,
                    3,
                );
            }
            if let Some(operation) = tcp_io.filter(|_| !windows_unsupported_tcp) {
                if let Some(payload_type) = operation.kind.payload_type() {
                    let payload_temp = async_tcp_io_payload_temp(index);
                    out.push_str("            ");
                    out.push_str(&c_type(&payload_type));
                    out.push(' ');
                    out.push_str(&payload_temp);
                    out.push_str(" = ");
                    emit_expr(out, operation.value);
                    out.push_str(";\n");
                    emit_value_retain_value_if_needed(
                        out,
                        &payload_temp,
                        &payload_type,
                        operation.value,
                        3,
                    );
                }
            }
            let moved_to_frame = frame_locals
                .iter()
                .filter(|local| {
                    local.declaration_index >= segment_start && local.declaration_index < index
                })
                .cloned()
                .collect::<Vec<_>>();
            for local in &moved_to_frame {
                emit_async_frame_store(out, local, 3);
            }
            emit_async_local_releases(out, &local_owned, &moved_to_frame, 3);
            local_owned.clear();
            for local in frame_locals
                .iter()
                .filter(|local| local.declaration_index < segment_start)
                .filter(|local| local.last_use_index < index)
            {
                emit_async_frame_field_drop(out, local, 3);
            }
            state += 1;
            out.push_str("            frame->state = ");
            out.push_str(&state.to_string());
            out.push_str("u;\n");
            if let Some(arms) = select {
                emit_async_select_start(out, index, state, arms, 3);
            } else if statement_is_async_yield(statement) {
                out.push_str("            context->yield_count += 1u;\n");
                out.push_str("            context->pending_reason = NOMO_ASYNC_PENDING_YIELD;\n");
                out.push_str("            return NOMO_ASYNC_POLL_PENDING;\n");
            } else if let Some(connect) = tcp_connect {
                let host_temp = async_tcp_connect_host_temp(index);
                out.push_str("            nomo_async_poll nomo_async_tcp_connect_start_status_");
                out.push_str(&index.to_string());
                out.push_str(" = nomo_async_tcp_connect_start(&frame->");
                out.push_str(&async_tcp_connect_registration_field(index));
                out.push_str(", ");
                if windows_unsupported_tcp {
                    out.push_str("nomo_string_literal(\"\")");
                } else {
                    out.push_str(&host_temp);
                }
                out.push_str(", ");
                if windows_unsupported_tcp {
                    out.push('0');
                } else {
                    emit_expr(out, connect.port);
                }
                out.push_str(", ");
                if windows_unsupported_tcp {
                    out.push_str("0u");
                } else {
                    emit_expr(out, connect.timeout_millis);
                }
                out.push_str(", context, &frame->");
                out.push_str(&async_tcp_connect_result_field(index));
                out.push_str(");\n");
                if !windows_unsupported_tcp {
                    emit_value_release_in_place(out, &ValueType::String, &host_temp, 3);
                }
                out.push_str("            if (nomo_async_tcp_connect_start_status_");
                out.push_str(&index.to_string());
                out.push_str(
                    " == NOMO_ASYNC_POLL_PENDING) {\n\
                                 return NOMO_ASYNC_POLL_PENDING;\n\
                             }\n",
                );
                out.push_str("            frame->");
                out.push_str(&async_tcp_connect_result_owned_field(index));
                out.push_str(" = 1u;\n");
                out.push_str("            goto nomo_async_resume_");
                out.push_str(&state.to_string());
                out.push_str(";\n");
            } else if let Some(operation) = tcp_io {
                let start_status = async_tcp_io_start_status_temp(index);
                out.push_str("            nomo_async_poll ");
                out.push_str(&start_status);
                out.push_str(" = ");
                out.push_str(operation.kind.start_function());
                out.push_str("(&frame->");
                out.push_str(&async_tcp_io_registration_field(index));
                out.push_str(", ");
                if windows_unsupported_tcp {
                    out.push_str("(nomo_async_tcp_stream){0}");
                } else {
                    emit_expr(out, operation.stream);
                }
                out.push_str(", ");
                if windows_unsupported_tcp {
                    match operation.kind {
                        AsyncTcpIoKind::Read | AsyncTcpIoKind::ReadString => out.push_str("1u"),
                        AsyncTcpIoKind::Write => out.push_str("nomo_array_u32_new()"),
                        AsyncTcpIoKind::WriteString => out.push_str("nomo_string_literal(\"\")"),
                    }
                } else if operation.kind.payload_type().is_some() {
                    out.push_str(&async_tcp_io_payload_temp(index));
                } else {
                    emit_expr(out, operation.value);
                }
                out.push_str(", ");
                if windows_unsupported_tcp {
                    out.push_str("0u");
                } else {
                    emit_expr(out, operation.timeout_millis);
                }
                out.push_str(", context, &frame->");
                out.push_str(&async_tcp_io_result_field(index));
                out.push_str(");\n");
                if let Some(payload_type) = operation
                    .kind
                    .payload_type()
                    .filter(|_| !windows_unsupported_tcp)
                {
                    emit_value_release_in_place(
                        out,
                        &payload_type,
                        &async_tcp_io_payload_temp(index),
                        3,
                    );
                }
                out.push_str("            if (");
                out.push_str(&start_status);
                out.push_str(
                    " == NOMO_ASYNC_POLL_PENDING) {\n\
                                 return NOMO_ASYNC_POLL_PENDING;\n\
                             }\n",
                );
                out.push_str("            frame->");
                out.push_str(&async_tcp_io_result_owned_field(index));
                out.push_str(" = 1u;\n");
                out.push_str("            goto nomo_async_resume_");
                out.push_str(&state.to_string());
                out.push_str(";\n");
            } else if let Some(send) = channel_send {
                out.push_str("            if (nomo_channel_send_start_");
                out.push_str(send.suffix);
                out.push_str("(&frame->");
                out.push_str(&async_channel_send_registration_field(index));
                out.push_str(", ");
                emit_expr(out, send.channel);
                out.push_str(", ");
                emit_expr(out, send.value);
                out.push_str(", context, &frame->");
                out.push_str(&async_channel_result_field(index));
                out.push_str(
                    ") == NOMO_ASYNC_POLL_PENDING) {\n\
                                 return NOMO_ASYNC_POLL_PENDING;\n\
                             }\n",
                );
                out.push_str("            frame->");
                out.push_str(&async_channel_result_owned_field(index));
                out.push_str(" = 1u;\n");
                out.push_str("            goto nomo_async_resume_");
                out.push_str(&state.to_string());
                out.push_str(";\n");
            } else if let Some(receive) = channel_receive {
                out.push_str("            if (nomo_channel_receive_start_");
                out.push_str(receive.suffix);
                out.push_str("(&frame->");
                out.push_str(&async_channel_receive_registration_field(index));
                out.push_str(", ");
                emit_expr(out, receive.channel);
                out.push_str(", context, &frame->");
                out.push_str(&async_channel_result_field(index));
                out.push_str(
                    ", NULL, 0u) == NOMO_ASYNC_POLL_PENDING) {\n\
                                 return NOMO_ASYNC_POLL_PENDING;\n\
                             }\n",
                );
                out.push_str("            frame->");
                out.push_str(&async_channel_result_owned_field(index));
                out.push_str(" = 1u;\n");
                out.push_str("            goto nomo_async_resume_");
                out.push_str(&state.to_string());
                out.push_str(";\n");
            } else if sleep.is_some() {
                out.push_str("            if (nomo_async_timer_start(&frame->");
                out.push_str(&async_timer_field(index));
                out.push_str(", nomo_async_sleep_millis_");
                out.push_str(&index.to_string());
                out.push_str(", context, &frame->");
                out.push_str(&async_timer_outcome_field(index));
                out.push_str(
                    ", NULL, 0u) == NOMO_ASYNC_POLL_PENDING) {\n\
                                 return NOMO_ASYNC_POLL_PENDING;\n\
                             }\n",
                );
                emit_async_timer_result_materialize(out, index, 3);
                out.push_str("            goto nomo_async_resume_");
                out.push_str(&state.to_string());
                out.push_str(";\n");
            } else if let Some(join) = join {
                let spawn_index = structured_spawn_index(function, join.handle)
                    .expect("validated structured join handle has a spawn");
                out.push_str("            if (frame->");
                out.push_str(&async_child_field(spawn_index));
                out.push_str(".structured_completed == 0u) {\n                frame->");
                out.push_str(&async_child_field(spawn_index));
                out.push_str(
                    ".structured_waiter_frame = context->current_frame;\n                frame->",
                );
                out.push_str(&async_child_field(spawn_index));
                out.push_str(
                    ".structured_waiter_poll = context->current_poll;\n\
                     context->join_suspensions += 1u;\n\
                     context->pending_reason = NOMO_ASYNC_PENDING_JOIN;\n\
                     return NOMO_ASYNC_POLL_PENDING;\n\
                 }\n",
                );
                let spawn = statement_structured_spawn(&function.body[spawn_index])
                    .expect("structured join spawn exists");
                let callee = functions
                    .get(spawn.callee)
                    .expect("validated structured spawn target exists");
                emit_structured_join_result_materialize(
                    out,
                    spawn_index,
                    index,
                    join.value_type,
                    &callee.return_type,
                    3,
                );
                out.push_str("            goto nomo_async_resume_");
                out.push_str(&state.to_string());
                out.push_str(";\n");
            } else if let Some(cancel) = cancel_join {
                let spawn_index = structured_spawn_index(function, cancel.handle)
                    .expect("validated structured cancel handle has a spawn");
                let spawn = statement_structured_spawn(&function.body[spawn_index])
                    .expect("structured cancel spawn exists");
                out.push_str("            ");
                out.push_str(&async_cancel_ident(spawn.callee));
                out.push_str("(&frame->");
                out.push_str(&async_child_field(spawn_index));
                out.push_str(", context);\n            if (frame->");
                out.push_str(&async_child_field(spawn_index));
                out.push_str(".structured_completed == 0u) {\n                frame->");
                out.push_str(&async_child_field(spawn_index));
                out.push_str(
                    ".structured_waiter_frame = context->current_frame;\n                frame->",
                );
                out.push_str(&async_child_field(spawn_index));
                out.push_str(
                    ".structured_waiter_poll = context->current_poll;\n\
                     context->pending_reason = NOMO_ASYNC_PENDING_CANCEL;\n\
                     return NOMO_ASYNC_POLL_PENDING;\n\
                 }\n",
                );
                emit_structured_cancel_join_result_materialize(
                    out,
                    spawn_index,
                    index,
                    cancel.value_type,
                    3,
                );
                out.push_str("            goto nomo_async_resume_");
                out.push_str(&state.to_string());
                out.push_str(";\n");
            } else {
                out.push_str("            goto nomo_async_resume_");
                out.push_str(&state.to_string());
                out.push_str(";\n");
            }
            out.push_str("        }\n");
            out.push_str("        case ");
            out.push_str(&state.to_string());
            out.push_str("u: {\n");
            if statement_is_within_async_deadline(function, index) {
                emit_async_deadline_due_check(out, function, async_names, 3);
            }
            if tcp_connect.is_some() {
                out.push_str("            if (nomo_async_tcp_connect_resume(&frame->");
                out.push_str(&async_tcp_connect_registration_field(index));
                out.push_str(", context, &frame->");
                out.push_str(&async_tcp_connect_result_field(index));
                out.push_str(
                    ") == NOMO_ASYNC_POLL_PENDING) {\n\
                                 return NOMO_ASYNC_POLL_PENDING;\n\
                             }\n",
                );
                out.push_str("            frame->");
                out.push_str(&async_tcp_connect_result_owned_field(index));
                out.push_str(" = 1u;\n");
            }
            if let Some(operation) = tcp_io {
                out.push_str("            if (");
                out.push_str(operation.kind.resume_function());
                out.push_str("(&frame->");
                out.push_str(&async_tcp_io_registration_field(index));
                out.push_str(", context, &frame->");
                out.push_str(&async_tcp_io_result_field(index));
                out.push_str(
                    ") == NOMO_ASYNC_POLL_PENDING) {\n\
                                 return NOMO_ASYNC_POLL_PENDING;\n\
                             }\n",
                );
                out.push_str("            frame->");
                out.push_str(&async_tcp_io_result_owned_field(index));
                out.push_str(" = 1u;\n");
            }
            if sleep.is_some() {
                out.push_str("            if (nomo_async_timer_resume(&frame->");
                out.push_str(&async_timer_field(index));
                out.push_str(", context, &frame->");
                out.push_str(&async_timer_outcome_field(index));
                out.push_str(
                    ") == NOMO_ASYNC_POLL_PENDING) {\n\
                                 return NOMO_ASYNC_POLL_PENDING;\n\
                             }\n",
                );
                emit_async_timer_result_materialize(out, index, 3);
            }
            if let Some(send) = channel_send {
                out.push_str("            if (nomo_channel_send_resume_");
                out.push_str(send.suffix);
                out.push_str("(&frame->");
                out.push_str(&async_channel_send_registration_field(index));
                out.push_str(", context, &frame->");
                out.push_str(&async_channel_result_field(index));
                out.push_str(
                    ") == NOMO_ASYNC_POLL_PENDING) {\n\
                                 return NOMO_ASYNC_POLL_PENDING;\n\
                             }\n",
                );
                out.push_str("            frame->");
                out.push_str(&async_channel_result_owned_field(index));
                out.push_str(" = 1u;\n");
            }
            if let Some(receive) = channel_receive {
                out.push_str("            if (nomo_channel_receive_resume_");
                out.push_str(receive.suffix);
                out.push_str("(&frame->");
                out.push_str(&async_channel_receive_registration_field(index));
                out.push_str(", context, &frame->");
                out.push_str(&async_channel_result_field(index));
                out.push_str(
                    ") == NOMO_ASYNC_POLL_PENDING) {\n\
                                 return NOMO_ASYNC_POLL_PENDING;\n\
                             }\n",
                );
                out.push_str("            frame->");
                out.push_str(&async_channel_result_owned_field(index));
                out.push_str(" = 1u;\n");
            }
            if let Some(join) = join {
                let spawn_index = structured_spawn_index(function, join.handle)
                    .expect("validated structured join handle has a spawn");
                out.push_str("            if (frame->");
                out.push_str(&async_child_field(spawn_index));
                out.push_str(
                    ".structured_completed == 0u) {\n\
                     context->join_suspensions += 1u;\n\
                     context->pending_reason = NOMO_ASYNC_PENDING_JOIN;\n\
                     return NOMO_ASYNC_POLL_PENDING;\n\
                 }\n",
                );
                let spawn = statement_structured_spawn(&function.body[spawn_index])
                    .expect("structured join spawn exists");
                let callee = functions
                    .get(spawn.callee)
                    .expect("validated structured spawn target exists");
                emit_structured_join_result_materialize(
                    out,
                    spawn_index,
                    index,
                    join.value_type,
                    &callee.return_type,
                    3,
                );
            }
            if let Some(cancel) = cancel_join {
                let spawn_index = structured_spawn_index(function, cancel.handle)
                    .expect("validated structured cancel handle has a spawn");
                out.push_str("            if (frame->");
                out.push_str(&async_child_field(spawn_index));
                out.push_str(
                    ".structured_completed == 0u) {\n\
                     context->pending_reason = NOMO_ASYNC_PENDING_CANCEL;\n\
                     return NOMO_ASYNC_POLL_PENDING;\n\
                 }\n",
                );
                emit_structured_cancel_join_result_materialize(
                    out,
                    spawn_index,
                    index,
                    cancel.value_type,
                    3,
                );
            }
            if sleep.is_some()
                || tcp_connect.is_some()
                || tcp_io.is_some()
                || channel_send.is_some()
                || channel_receive.is_some()
                || call.is_some()
                || join.is_some()
                || cancel_join.is_some()
                || select.is_some()
            {
                out.push_str("nomo_async_resume_");
                out.push_str(&state.to_string());
                out.push_str(":\n            ;\n");
            }
            if let Some(arms) = select {
                emit_async_select_resume_and_body(out, index, arms, function, &frame_locals, 3);
            }
            segment_start = index + 1;
            let segment_end = next_async_suspend(function, segment_start, async_names);
            for local in frame_locals
                .iter()
                .filter(|local| local.declaration_index < segment_start)
                .filter(|local| local.declaration_index != index)
                .filter(|local| {
                    function.body[segment_start..segment_end]
                        .iter()
                        .any(|statement| statement_uses_binding(statement, &local.name))
                })
            {
                emit_async_frame_alias(out, local, 3);
            }
            if let Some(call) = call {
                let callee = functions
                    .get(call.callee)
                    .expect("validated suspend call target exists");
                out.push_str("            if (");
                out.push_str(&async_poll_ident(call.callee));
                out.push_str("(&frame->");
                out.push_str(&async_child_field(index));
                out.push_str(
                    ", context) == NOMO_ASYNC_POLL_PENDING) {\n\
                                 return NOMO_ASYNC_POLL_PENDING;\n\
                             }\n\
                             ",
                );
                emit_async_child_failure_propagation(out, function, async_names, index, 3);
                if callee.return_type != ValueType::Void {
                    let (binding, value_type) = call
                        .binding
                        .expect("value-returning suspend calls require a binding");
                    debug_assert_eq!(value_type, &callee.return_type);
                    if let Some(frame_local) = frame_locals
                        .iter()
                        .find(|local| local.declaration_index == index)
                    {
                        out.push_str("            frame->");
                        out.push_str(&async_frame_value_field(binding));
                        out.push_str(" = frame->");
                        out.push_str(&async_child_field(index));
                        out.push('.');
                        out.push_str(async_result_field());
                        out.push_str(";\n");
                        if value_type_needs_release(value_type) {
                            out.push_str("            frame->");
                            out.push_str(&async_frame_owned_field(binding));
                            out.push_str(" = frame->");
                            out.push_str(&async_child_field(index));
                            out.push('.');
                            out.push_str(async_result_owned_field());
                            out.push_str(";\n            frame->");
                            out.push_str(&async_child_field(index));
                            out.push('.');
                            out.push_str(async_result_owned_field());
                            out.push_str(" = 0u;\n");
                        }
                        emit_async_frame_alias(out, frame_local, 3);
                    } else {
                        out.push_str("            ");
                        out.push_str(&c_type(value_type));
                        out.push(' ');
                        out.push_str(&c_var_ident(binding));
                        out.push_str(" = frame->");
                        out.push_str(&async_child_field(index));
                        out.push('.');
                        out.push_str(async_result_field());
                        out.push_str(";\n");
                        if value_type_needs_release(value_type) {
                            out.push_str("            frame->");
                            out.push_str(&async_child_field(index));
                            out.push('.');
                            out.push_str(async_result_owned_field());
                            out.push_str(" = 0u;\n");
                        }
                        if let Some(local) = local_array(binding, value_type) {
                            local_owned.push(local);
                        }
                    }
                }
                out.push_str("            ");
                out.push_str(&async_drop_ident(call.callee));
                out.push_str("(&frame->");
                out.push_str(&async_child_field(index));
                out.push_str(");\n");
            }
            if let Some(connect) = tcp_connect {
                emit_async_tcp_connect_result_binding(
                    out,
                    index,
                    connect,
                    &frame_locals,
                    &mut local_owned,
                    3,
                );
            }
            if let Some(operation) = tcp_io {
                emit_async_tcp_io_result_binding(
                    out,
                    index,
                    operation,
                    &frame_locals,
                    &mut local_owned,
                    3,
                );
            }
            if let Some(send) = channel_send {
                emit_async_channel_result_binding(
                    out,
                    index,
                    send.binding,
                    send.value_type,
                    &frame_locals,
                    &mut local_owned,
                    3,
                );
            }
            if let Some(receive) = channel_receive {
                emit_async_channel_result_binding(
                    out,
                    index,
                    receive.binding,
                    receive.value_type,
                    &frame_locals,
                    &mut local_owned,
                    3,
                );
            }
            if let Some((name, value_type, _)) = sleep {
                if let Some(frame_local) = frame_locals
                    .iter()
                    .find(|local| local.declaration_index == index)
                {
                    out.push_str("            frame->");
                    out.push_str(&async_frame_value_field(name));
                    out.push_str(" = frame->");
                    out.push_str(&async_timer_result_field(index));
                    out.push_str(";\n");
                    if value_type_needs_release(value_type) {
                        out.push_str("            frame->");
                        out.push_str(&async_frame_owned_field(name));
                        out.push_str(" = frame->");
                        out.push_str(&async_timer_result_owned_field(index));
                        out.push_str(";\n            frame->");
                        out.push_str(&async_timer_result_owned_field(index));
                        out.push_str(" = 0u;\n");
                    }
                    emit_async_frame_alias(out, frame_local, 3);
                } else {
                    out.push_str("            ");
                    out.push_str(&c_type(value_type));
                    out.push(' ');
                    out.push_str(&c_var_ident(name));
                    out.push_str(" = frame->");
                    out.push_str(&async_timer_result_field(index));
                    out.push_str(";\n            frame->");
                    out.push_str(&async_timer_result_owned_field(index));
                    out.push_str(" = 0u;\n");
                    if let Some(local) = local_array(name, value_type) {
                        local_owned.push(local);
                    }
                }
            }
            if let Some(join) = join {
                let ValueType::Enum(result_name, result_args) = join.value_type else {
                    unreachable!("structured join result is always a Result enum");
                };
                debug_assert_eq!(result_name, "Result");
                debug_assert_eq!(result_args.len(), 2);
                if let Some(frame_local) = frame_locals
                    .iter()
                    .find(|local| local.declaration_index == index)
                {
                    out.push_str("            frame->");
                    out.push_str(&async_frame_value_field(join.binding));
                    out.push_str(" = frame->");
                    out.push_str(&async_join_result_field(index));
                    out.push_str(";\n");
                    if value_type_needs_release(join.value_type) {
                        out.push_str("            frame->");
                        out.push_str(&async_frame_owned_field(join.binding));
                        out.push_str(" = frame->");
                        out.push_str(&async_join_result_owned_field(index));
                        out.push_str(";\n            frame->");
                        out.push_str(&async_join_result_owned_field(index));
                        out.push_str(" = 0u;\n");
                    }
                    emit_async_frame_alias(out, frame_local, 3);
                } else {
                    out.push_str("            ");
                    out.push_str(&c_type(join.value_type));
                    out.push(' ');
                    out.push_str(&c_var_ident(join.binding));
                    out.push_str(" = frame->");
                    out.push_str(&async_join_result_field(index));
                    out.push_str(";\n            frame->");
                    out.push_str(&async_join_result_owned_field(index));
                    out.push_str(" = 0u;\n");
                    if let Some(local) = local_array(join.binding, join.value_type) {
                        local_owned.push(local);
                    }
                }
                let spawn_index = structured_spawn_index(function, join.handle)
                    .expect("validated structured join handle has a spawn");
                out.push_str("            ");
                out.push_str(&async_drop_ident(
                    statement_structured_spawn(&function.body[spawn_index])
                        .expect("structured join spawn exists")
                        .callee,
                ));
                out.push_str("(&frame->");
                out.push_str(&async_child_field(spawn_index));
                out.push_str(");\n");
            }
            if let Some(cancel) = cancel_join {
                let ValueType::Enum(result_name, result_args) = cancel.value_type else {
                    unreachable!("structured cancel result is always a Result enum");
                };
                debug_assert_eq!(result_name, "Result");
                debug_assert_eq!(result_args.len(), 2);
                if let Some(frame_local) = frame_locals
                    .iter()
                    .find(|local| local.declaration_index == index)
                {
                    out.push_str("            frame->");
                    out.push_str(&async_frame_value_field(cancel.binding));
                    out.push_str(" = frame->");
                    out.push_str(&async_cancel_join_result_field(index));
                    out.push_str(";\n");
                    if value_type_needs_release(cancel.value_type) {
                        out.push_str("            frame->");
                        out.push_str(&async_frame_owned_field(cancel.binding));
                        out.push_str(" = frame->");
                        out.push_str(&async_cancel_join_result_owned_field(index));
                        out.push_str(";\n            frame->");
                        out.push_str(&async_cancel_join_result_owned_field(index));
                        out.push_str(" = 0u;\n");
                    }
                    emit_async_frame_alias(out, frame_local, 3);
                } else {
                    out.push_str("            ");
                    out.push_str(&c_type(cancel.value_type));
                    out.push(' ');
                    out.push_str(&c_var_ident(cancel.binding));
                    out.push_str(" = frame->");
                    out.push_str(&async_cancel_join_result_field(index));
                    out.push_str(";\n            frame->");
                    out.push_str(&async_cancel_join_result_owned_field(index));
                    out.push_str(" = 0u;\n");
                    if let Some(local) = local_array(cancel.binding, cancel.value_type) {
                        local_owned.push(local);
                    }
                }
                let spawn_index = structured_spawn_index(function, cancel.handle)
                    .expect("validated structured cancel handle has a spawn");
                out.push_str("            ");
                out.push_str(&async_drop_ident(
                    statement_structured_spawn(&function.body[spawn_index])
                        .expect("structured cancel spawn exists")
                        .callee,
                ));
                out.push_str("(&frame->");
                out.push_str(&async_child_field(spawn_index));
                out.push_str(");\n");
            }
            continue;
        }
        if let Statement::QuestionLet {
            carrier,
            name,
            value_type,
            result_type,
            return_type,
            result_expr,
            early_exit_actions,
        } = statement
        {
            emit_async_question_let(
                out,
                function,
                index,
                *carrier,
                name,
                value_type,
                result_type,
                return_type,
                result_expr,
                early_exit_actions,
                &local_owned,
                3,
            );
            if let Some(local) = local_array(name, value_type) {
                local_owned.push(local);
            }
            continue;
        }
        if let Statement::Return(value) = statement {
            match value {
                Some(value) => {
                    emit_async_return_value(out, function, value, &local_owned, 3);
                }
                None => {
                    debug_assert_eq!(function.return_type, ValueType::Void);
                    emit_async_local_releases(out, &local_owned, &[], 3);
                    emit_structured_completion(out, 3);
                    out.push_str(
                        "            frame->state = UINT32_MAX;\n\
                                     return NOMO_ASYNC_POLL_READY;\n",
                    );
                }
            }
            emitted_terminal_return = true;
            break;
        }
        if let Statement::Panic(message) = statement {
            out.push_str("            nomo_string nomo_async_panic_message_");
            out.push_str(&index.to_string());
            out.push_str(" = ");
            emit_expr(out, message);
            out.push_str(";\n");
            if expr_may_share_array_storage(message) {
                emit_value_retain_in_place(
                    out,
                    &ValueType::String,
                    &format!("nomo_async_panic_message_{index}"),
                    3,
                );
            }
            out.push_str(
                "            if (context->panic_message_owned == 0u) {\n\
                                 context->panic_message = nomo_async_panic_message_",
            );
            out.push_str(&index.to_string());
            out.push_str(
                ";\n\
                                 context->panic_message_owned = 1u;\n\
                             } else {\n\
                                 nomo_string_release(nomo_async_panic_message_",
            );
            out.push_str(&index.to_string());
            out.push_str(
                ");\n\
                             }\n\
                             context->panicking = 1u;\n",
            );
            emit_async_local_releases(out, &local_owned, &[], 3);
            out.push_str(
                "            context->pending_reason = NOMO_ASYNC_PENDING_PANIC;\n\
                             return NOMO_ASYNC_POLL_PENDING;\n",
            );
            emitted_terminal_return = true;
            break;
        }
        emit_stmt(
            out,
            statement,
            3,
            &empty_deferred,
            &function.return_type,
            &local_owned,
            0,
            0,
            0,
            0,
        );
        if let Some(local) = local_array_from_statement(statement) {
            local_owned.push(local);
        }
    }
    if !emitted_terminal_return {
        debug_assert_eq!(function.return_type, ValueType::Void);
        emit_async_local_releases(out, &local_owned, &[], 3);
        emit_structured_completion(out, 3);
        out.push_str(
            "            frame->state = UINT32_MAX;\n\
                     return NOMO_ASYNC_POLL_READY;\n",
        );
    }
    out.push_str(
        "        }\n\
                 default:\n\
                     return NOMO_ASYNC_POLL_READY;\n\
             }\n\
         }\n\n",
    );
    emit_async_cancel_function(out, function, async_names);
    out.push_str("static void ");
    out.push_str(&async_drop_ident(&function.name));
    out.push('(');
    out.push_str(&async_frame_ident(&function.name));
    out.push_str(
        " *frame) {\n\
             if (frame->dropped != 0u) {\n\
                 return;\n\
             }\n\
             frame->dropped = 1u;\n\
             if (frame->started != 0u && frame->context != NULL) {\n\
                 frame->context->frame_drops += 1u;\n\
                 if (frame->context->live_frames > 0u) {\n\
                     frame->context->live_frames -= 1u;\n\
                 }\n\
             }\n",
    );
    for (index, statement) in function.body.iter().enumerate().rev() {
        let Some(spawn) = statement_structured_spawn(statement) else {
            continue;
        };
        out.push_str("    frame->");
        out.push_str(&async_child_field(index));
        out.push_str(
            ".structured_waiter_frame = NULL;\n\
             frame->",
        );
        out.push_str(&async_child_field(index));
        out.push_str(".structured_waiter_poll = NULL;\n    ");
        out.push_str(&async_drop_ident(spawn.callee));
        out.push_str("(&frame->");
        out.push_str(&async_child_field(index));
        out.push_str(");\n");
    }
    for (index, statement) in function.body.iter().enumerate().rev() {
        let Some(call) = statement_async_call(statement, async_names) else {
            continue;
        };
        out.push_str("    ");
        out.push_str(&async_drop_ident(call.callee));
        out.push_str("(&frame->");
        out.push_str(&async_child_field(index));
        out.push_str(");\n");
    }
    emit_async_select_cancellations(out, function, 1);
    emit_async_channel_cancellations(out, function, 1);
    for (index, statement) in function.body.iter().enumerate().rev() {
        if statement_async_tcp_connect(statement).is_none() {
            continue;
        }
        out.push_str("    if (frame->context != NULL) {\n");
        out.push_str("        nomo_async_tcp_connect_cancel(&frame->");
        out.push_str(&async_tcp_connect_registration_field(index));
        out.push_str(", frame->context);\n    }\n");
    }
    for (index, statement) in function.body.iter().enumerate().rev() {
        if statement_async_tcp_io(statement).is_none() {
            continue;
        }
        out.push_str("    if (frame->context != NULL) {\n");
        out.push_str("        nomo_async_tcp_io_cancel(&frame->");
        out.push_str(&async_tcp_io_registration_field(index));
        out.push_str(", frame->context);\n    }\n");
    }
    for (index, statement) in function.body.iter().enumerate().rev() {
        let Some(arms) = statement_task_select(statement) else {
            continue;
        };
        for (arm_index, arm) in arms.iter().enumerate().rev() {
            emit_async_owned_field_drop(
                out,
                &arm.binding_type,
                &async_select_result_owned_field(index, arm_index),
                &async_select_result_field(index, arm_index),
                1,
            );
        }
    }
    for (index, statement) in function.body.iter().enumerate().rev() {
        let Some(connect) = statement_async_tcp_connect(statement) else {
            continue;
        };
        emit_async_owned_field_drop(
            out,
            connect.value_type,
            &async_tcp_connect_result_owned_field(index),
            &async_tcp_connect_result_field(index),
            1,
        );
    }
    for (index, statement) in function.body.iter().enumerate().rev() {
        let Some(operation) = statement_async_tcp_io(statement) else {
            continue;
        };
        emit_async_owned_field_drop(
            out,
            operation.value_type,
            &async_tcp_io_result_owned_field(index),
            &async_tcp_io_result_field(index),
            1,
        );
    }
    for (index, statement) in function.body.iter().enumerate().rev() {
        let channel_value_type = statement_async_channel_send(statement)
            .map(|send| send.value_type)
            .or_else(|| {
                statement_async_channel_receive(statement).map(|receive| receive.value_type)
            });
        let Some(value_type) = channel_value_type else {
            continue;
        };
        emit_async_owned_field_drop(
            out,
            value_type,
            &async_channel_result_owned_field(index),
            &async_channel_result_field(index),
            1,
        );
    }
    for (index, statement) in function.body.iter().enumerate().rev() {
        let Some(join) = statement_structured_join(statement) else {
            continue;
        };
        emit_async_owned_field_drop(
            out,
            join.value_type,
            &async_join_result_owned_field(index),
            &async_join_result_field(index),
            1,
        );
    }
    for (index, statement) in function.body.iter().enumerate().rev() {
        let Some(cancel) = statement_structured_cancel_join(statement) else {
            continue;
        };
        emit_async_owned_field_drop(
            out,
            cancel.value_type,
            &async_cancel_join_result_owned_field(index),
            &async_cancel_join_result_field(index),
            1,
        );
    }
    if function_has_async_deadline(function) {
        out.push_str(
            "    if (frame->context != NULL && frame->nomo_async_deadline_active != 0u) {\n\
                 nomo_async_timer_disarm(&frame->nomo_async_deadline_timer, frame->context);\n\
                 frame->nomo_async_deadline_active = 0u;\n\
                 frame->context->deadline_cancellations += 1u;\n\
             }\n",
        );
    }
    for (index, statement) in function.body.iter().enumerate().rev() {
        if statement_async_sleep(statement).is_none() {
            continue;
        }
        out.push_str("    if (frame->context != NULL) {\n        nomo_async_timer_disarm(&frame->");
        out.push_str(&async_timer_field(index));
        out.push_str(", frame->context);\n    }\n    if (frame->");
        out.push_str(&async_timer_result_owned_field(index));
        out.push_str(" != 0u) {\n        frame->");
        out.push_str(&async_timer_result_owned_field(index));
        out.push_str(" = 0u;\n");
        emit_value_release_in_place(
            out,
            &async_sleep_result_type(),
            &format!("frame->{}", async_timer_result_field(index)),
            2,
        );
        out.push_str("    }\n");
    }
    if value_type_needs_release(&function.return_type) {
        emit_async_owned_field_drop(
            out,
            &function.return_type,
            async_result_owned_field(),
            async_result_field(),
            1,
        );
    }
    for local in frame_locals.iter().rev() {
        emit_async_frame_field_drop(out, local, 1);
    }
    for parameter in function.params.iter().rev() {
        if value_type_needs_release(&parameter.value_type) {
            emit_async_owned_field_drop(
                out,
                &parameter.value_type,
                &async_parameter_owned_field(&parameter.name),
                &async_parameter_field(&parameter.name),
                1,
            );
        }
    }
    out.push_str(
        "    frame->context = NULL;\n\
             frame->state = UINT32_MAX;\n\
         }\n",
    );
}
