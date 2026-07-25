use super::*;

pub(super) fn function_uses_async_yield(function: &Function) -> bool {
    function.body.iter().any(|statement| {
        statement_contains_expr(statement, |expr| {
            expr_is_async_yield(expr) || expr_is_async_sleep(expr)
        })
    })
}

pub(super) fn collect_async_function_names(program: &Program) -> BTreeSet<String> {
    let mut names = program
        .functions
        .iter()
        .filter(|function| function.is_suspend && function_uses_async_yield(function))
        .map(|function| function.name.clone())
        .collect::<BTreeSet<_>>();

    loop {
        let discovered = program
            .functions
            .iter()
            .filter(|function| function.is_suspend && !names.contains(&function.name))
            .filter(|function| {
                function.body.iter().any(|statement| {
                    statement_contains_expr(statement, |expr| {
                        matches!(
                            expr,
                            ValueExpr::Call { name, .. } if names.contains(name)
                        )
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
            let Some(call) = statement_async_call(statement, async_names) else {
                continue;
            };
            if let Some(child) = functions.get(call.callee) {
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

fn statement_is_async_yield(statement: &Statement) -> bool {
    matches!(
        statement,
        Statement::Expr(ValueExpr::Call { name, args })
            if name == BUILTIN_TASK_YIELD_EXPR && args.is_empty()
    )
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
struct AsyncCall<'a> {
    callee: &'a str,
    args: &'a [ValueExpr],
    binding: Option<(&'a str, &'a ValueType)>,
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
        || statement_async_call(statement, async_names).is_some()
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
            let Statement::Let {
                name, value_type, ..
            } = statement
            else {
                return None;
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
                .any(|statement| statement_is_async_suspend(statement, async_names));
            crosses_suspend.then(|| AsyncFrameLocal {
                name: name.clone(),
                value_type: value_type.clone(),
                declaration_index,
                last_use_index,
            })
        })
        .collect()
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
             uint8_t started;\n\
             uint8_t dropped;\n",
    );
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
        let Some(call) = statement_async_call(statement, async_names) else {
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
        }
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
    write_indent(out, indent);
    out.push_str("frame->state = UINT32_MAX;\n");
    write_indent(out, indent);
    out.push_str("return NOMO_ASYNC_POLL_READY;\n");
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
    let result_type = async_sleep_result_type();
    let ValueType::Enum(_, result_args) = &result_type else {
        unreachable!("sleep result is always a Result enum");
    };
    let result = format!("frame->{}", async_timer_result_field(index));
    let outcome = format!("frame->{}", async_timer_outcome_field(index));
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
    out.push_str(&async_timer_result_owned_field(index));
    out.push_str(" = 1u;\n");
}

pub(super) fn emit_current_thread_executor(out: &mut String) {
    let runtime = r#"typedef enum {
    NOMO_ASYNC_POLL_READY = 0,
    NOMO_ASYNC_POLL_PENDING = 1
} nomo_async_poll;

typedef enum {
    NOMO_ASYNC_PENDING_NONE = 0,
    NOMO_ASYNC_PENDING_YIELD = 1,
    NOMO_ASYNC_PENDING_TIMER = 2
} nomo_async_pending_reason;

typedef enum {
    NOMO_ASYNC_TIMER_OUTCOME_NONE = 0,
    NOMO_ASYNC_TIMER_OUTCOME_OK = 1,
    NOMO_ASYNC_TIMER_OUTCOME_LIMIT = 2,
    NOMO_ASYNC_TIMER_OUTCOME_RUNTIME_ERROR = 3
} nomo_async_timer_outcome;

typedef struct nomo_async_context nomo_async_context;
typedef nomo_async_poll (*nomo_async_poll_fn)(void *, nomo_async_context *);

#define NOMO_ASYNC_TIMER_CAPACITY 64u

typedef struct {
    uint32_t slot;
    uint32_t generation;
    int64_t deadline_millis;
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

struct nomo_async_context {
    uint64_t poll_count;
    uint64_t yield_count;
    uint64_t frame_drops;
    uint64_t live_frames;
    uint64_t peak_live_frames;
    uint64_t ready_queue_enqueues;
    uint64_t ready_queue_dequeues;
    uint64_t timer_registrations;
    uint64_t timer_expirations;
    uint64_t timer_cancellations;
    uint64_t live_timers;
    uint64_t peak_live_timers;
    uint32_t next_timer_generation;
    void *ready_frame;
    nomo_async_poll_fn ready_poll;
    void *current_frame;
    nomo_async_poll_fn current_poll;
    nomo_async_pending_reason pending_reason;
    nomo_async_timer_slot timers[NOMO_ASYNC_TIMER_CAPACITY];
    uint8_t ready_occupied;
};

static int nomo_async_ready_enqueue(
    nomo_async_context *context,
    void *frame,
    nomo_async_poll_fn poll
) {
    if (context->ready_occupied != 0u) {
        return 1;
    }
    context->ready_frame = frame;
    context->ready_poll = poll;
    context->ready_occupied = 1u;
    context->ready_queue_enqueues += 1u;
    return 0;
}

static nomo_async_poll nomo_async_timer_start(
    nomo_async_timer_registration *registration,
    int64_t duration_millis,
    nomo_async_context *context,
    nomo_async_timer_outcome *outcome
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

static void nomo_async_timer_disarm(
    nomo_async_timer_registration *registration,
    nomo_async_context *context
) {
    if (registration->armed == 0u) {
        registration->expired = 0u;
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
}

static int nomo_async_timer_wait_next(nomo_async_context *context) {
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
    if (selected == NOMO_ASYNC_TIMER_CAPACITY) {
        return 1;
    }
    nomo_async_timer_slot *slot = &context->timers[selected];
    while (1) {
        int64_t now = nomo_time_monotonic_millis();
        if (now >= slot->deadline_millis) {
            break;
        }
        int64_t remaining = slot->deadline_millis - now;
        nomo_time_sleep_millis(remaining > 60000 ? 60000 : remaining);
    }
    nomo_async_timer_registration *registration = slot->registration;
    void *frame = slot->frame;
    nomo_async_poll_fn poll = slot->poll;
    if (registration == NULL
        || registration->armed == 0u
        || registration->generation != slot->generation) {
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
    if (status == NOMO_ASYNC_POLL_READY) {
        return 0;
    }
    if (context->pending_reason == NOMO_ASYNC_PENDING_YIELD) {
        if (nomo_async_ready_enqueue(context, frame, poll) != 0) {
            return 1;
        }
    } else if (context->pending_reason != NOMO_ASYNC_PENDING_TIMER) {
        return 1;
    }
    while (context->ready_occupied != 0u || context->live_timers != 0u) {
        if (context->ready_occupied == 0u
            && nomo_async_timer_wait_next(context) != 0) {
            return 1;
        }
        void *ready_frame = context->ready_frame;
        nomo_async_poll_fn ready_poll = context->ready_poll;
        context->ready_frame = NULL;
        context->ready_poll = NULL;
        context->ready_occupied = 0u;
        context->ready_queue_dequeues += 1u;
        status = nomo_async_poll_task(ready_frame, ready_poll, context);
        if (status == NOMO_ASYNC_POLL_PENDING) {
            if (context->pending_reason == NOMO_ASYNC_PENDING_YIELD) {
                if (nomo_async_ready_enqueue(context, ready_frame, ready_poll) != 0) {
                    return 1;
                }
            } else if (context->pending_reason != NOMO_ASYNC_PENDING_TIMER) {
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
        "    \"timer_registrations\": %" PRIu64 ",\n"
        "    \"timer_expirations\": %" PRIu64 ",\n"
        "    \"timer_cancellations\": %" PRIu64 ",\n"
        "    \"live_timers\": %" PRIu64 ",\n"
        "    \"peak_live_timers\": %" PRIu64 "\n"
        "  },\n"
        "  \"unavailable\": {\n"
        "    \"local_retain\": \"ARC primitive instrumentation is not implemented in this P1 slice\",\n"
        "    \"local_release\": \"ARC primitive instrumentation is not implemented in this P1 slice\"\n"
        "  }\n"
        "}\n",
        context->poll_count,
        context->yield_count,
        context->frame_drops,
        context->peak_live_frames,
        context->ready_queue_enqueues,
        context->ready_queue_dequeues,
        context->timer_registrations,
        context->timer_expirations,
        context->timer_cancellations,
        context->live_timers,
        context->peak_live_timers
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
        if statement_is_async_suspend(statement, async_names) {
            let sleep = statement_async_sleep(statement);
            if let Some((_, _, duration)) = sleep {
                out.push_str("            int64_t nomo_async_sleep_millis_");
                out.push_str(&index.to_string());
                out.push_str(" = (");
                emit_expr(out, duration);
                out.push_str(").nomo_member_millis;\n");
            }
            let call = statement_async_call(statement, async_names);
            if let Some(call) = call {
                let callee = functions
                    .get(call.callee)
                    .expect("validated suspend call target exists");
                emit_async_child_init(out, call, callee, index, 3);
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
            if statement_is_async_yield(statement) {
                out.push_str("            context->yield_count += 1u;\n");
                out.push_str("            context->pending_reason = NOMO_ASYNC_PENDING_YIELD;\n");
                out.push_str("            return NOMO_ASYNC_POLL_PENDING;\n");
            } else if sleep.is_some() {
                out.push_str("            if (nomo_async_timer_start(&frame->");
                out.push_str(&async_timer_field(index));
                out.push_str(", nomo_async_sleep_millis_");
                out.push_str(&index.to_string());
                out.push_str(", context, &frame->");
                out.push_str(&async_timer_outcome_field(index));
                out.push_str(
                    ") == NOMO_ASYNC_POLL_PENDING) {\n\
                                 return NOMO_ASYNC_POLL_PENDING;\n\
                             }\n",
                );
                emit_async_timer_result_materialize(out, index, 3);
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
            if sleep.is_some() || call.is_some() {
                out.push_str("nomo_async_resume_");
                out.push_str(&state.to_string());
                out.push_str(":\n            ;\n");
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
                    out.push_str(
                        "            frame->state = UINT32_MAX;\n\
                                     return NOMO_ASYNC_POLL_READY;\n",
                    );
                }
            }
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
         }\n\
         \n\
         static void ",
    );
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
        let Some(call) = statement_async_call(statement, async_names) else {
            continue;
        };
        out.push_str("    ");
        out.push_str(&async_drop_ident(call.callee));
        out.push_str("(&frame->");
        out.push_str(&async_child_field(index));
        out.push_str(");\n");
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
