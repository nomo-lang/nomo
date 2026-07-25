use super::*;

pub(super) fn function_uses_async_yield(function: &Function) -> bool {
    function
        .body
        .iter()
        .any(|statement| statement_contains_expr(statement, expr_is_async_yield))
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
                            ValueExpr::Call { name, args }
                                if args.is_empty() && names.contains(name)
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
            let Some(callee) = statement_async_call(statement, async_names) else {
                continue;
            };
            if let Some(child) = functions.get(callee) {
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

fn statement_is_async_yield(statement: &Statement) -> bool {
    matches!(
        statement,
        Statement::Expr(ValueExpr::Call { name, args })
            if name == BUILTIN_TASK_YIELD_EXPR && args.is_empty()
    )
}

fn statement_async_call<'a>(
    statement: &'a Statement,
    async_names: &BTreeSet<String>,
) -> Option<&'a str> {
    match statement {
        Statement::Expr(ValueExpr::Call { name, args })
            if args.is_empty() && async_names.contains(name) =>
        {
            Some(name)
        }
        _ => None,
    }
}

fn statement_is_async_suspend(statement: &Statement, async_names: &BTreeSet<String>) -> bool {
    statement_is_async_yield(statement) || statement_async_call(statement, async_names).is_some()
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
        let Some(callee) = statement_async_call(statement, async_names) else {
            continue;
        };
        out.push_str("    ");
        out.push_str(&async_frame_ident(callee));
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
    let owned = async_frame_owned_field(&local.name);
    write_indent(out, indent);
    out.push_str("if (frame->");
    out.push_str(&owned);
    out.push_str(" != 0u) {\n");
    write_indent(out, indent + 1);
    out.push_str("frame->");
    out.push_str(&owned);
    out.push_str(" = 0u;\n");
    emit_value_release_in_place(
        out,
        &local.value_type,
        &format!("frame->{}", async_frame_value_field(&local.name)),
        indent + 1,
    );
    write_indent(out, indent);
    out.push_str("}\n");
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

pub(super) fn emit_current_thread_executor(out: &mut String) {
    out.push_str(
        "typedef enum {\n\
             NOMO_ASYNC_POLL_READY = 0,\n\
             NOMO_ASYNC_POLL_PENDING = 1\n\
         } nomo_async_poll;\n\
         \n\
         typedef struct nomo_async_context nomo_async_context;\n\
         typedef nomo_async_poll (*nomo_async_poll_fn)(void *, nomo_async_context *);\n\
         \n\
         struct nomo_async_context {\n\
             uint64_t poll_count;\n\
             uint64_t yield_count;\n\
             uint64_t frame_drops;\n\
             uint64_t live_frames;\n\
             uint64_t peak_live_frames;\n\
             uint64_t ready_queue_enqueues;\n\
             uint64_t ready_queue_dequeues;\n\
             void *ready_frame;\n\
             nomo_async_poll_fn ready_poll;\n\
             uint8_t ready_occupied;\n\
         };\n\
         \n\
         static int nomo_async_ready_enqueue(\n\
             nomo_async_context *context,\n\
             void *frame,\n\
             nomo_async_poll_fn poll\n\
         ) {\n\
             if (context->ready_occupied != 0u) {\n\
                 return 1;\n\
             }\n\
             context->ready_frame = frame;\n\
             context->ready_poll = poll;\n\
             context->ready_occupied = 1u;\n\
             context->ready_queue_enqueues += 1u;\n\
             return 0;\n\
         }\n\
         \n\
         static int nomo_async_executor_run_root(\n\
             void *frame,\n\
             nomo_async_poll_fn poll,\n\
             nomo_async_context *context\n\
         ) {\n\
             nomo_async_poll status = poll(frame, context);\n\
             if (status == NOMO_ASYNC_POLL_READY) {\n\
                 return 0;\n\
             }\n\
             if (nomo_async_ready_enqueue(context, frame, poll) != 0) {\n\
                 return 1;\n\
             }\n\
             while (context->ready_occupied != 0u) {\n\
                 void *ready_frame = context->ready_frame;\n\
                 nomo_async_poll_fn ready_poll = context->ready_poll;\n\
                 context->ready_frame = NULL;\n\
                 context->ready_poll = NULL;\n\
                 context->ready_occupied = 0u;\n\
                 context->ready_queue_dequeues += 1u;\n\
                 status = ready_poll(ready_frame, context);\n\
                 if (status == NOMO_ASYNC_POLL_PENDING\n\
                     && nomo_async_ready_enqueue(context, ready_frame, ready_poll) != 0) {\n\
                     return 1;\n\
                 }\n\
             }\n\
             return 0;\n\
         }\n\
         \n\
         static int nomo_async_metrics_export(const nomo_async_context *context) {\n\
             const char *path = getenv(\"NOMO_ASYNC_METRICS_PATH\");\n\
             if (path == NULL || path[0] == '\\0') {\n\
                 return 0;\n\
             }\n\
             FILE *output = fopen(path, \"wb\");\n\
             if (output == NULL) {\n\
                 return 1;\n\
             }\n\
             int write_status = fprintf(\n\
                 output,\n\
                 \"{\\n\"\n\
                 \"  \\\"schema\\\": 1,\\n\"\n\
                 \"  \\\"runtime\\\": \\\"nomo-c99-current-thread\\\",\\n\"\n\
                 \"  \\\"runtime_abi\\\": 1,\\n\"\n\
                 \"  \\\"counter_catalog_schema\\\": 1,\\n\"\n\
                 \"  \\\"counters\\\": {\\n\"\n\
                 \"    \\\"poll_calls\\\": %\" PRIu64 \",\\n\"\n\
                 \"    \\\"cooperative_yields\\\": %\" PRIu64 \",\\n\"\n\
                 \"    \\\"frame_allocations\\\": 0,\\n\"\n\
                 \"    \\\"frame_drops\\\": %\" PRIu64 \",\\n\"\n\
                 \"    \\\"peak_live_frames\\\": %\" PRIu64 \",\\n\"\n\
                 \"    \\\"ready_queue_enqueues\\\": %\" PRIu64 \",\\n\"\n\
                 \"    \\\"ready_queue_dequeues\\\": %\" PRIu64 \"\\n\"\n\
                 \"  },\\n\"\n\
                 \"  \\\"unavailable\\\": {\\n\"\n\
                 \"    \\\"local_retain\\\": \\\"ARC primitive instrumentation is not implemented in this P1 slice\\\",\\n\"\n\
                 \"    \\\"local_release\\\": \\\"ARC primitive instrumentation is not implemented in this P1 slice\\\",\\n\"\n\
                 \"    \\\"live_timers\\\": \\\"the monotonic timer runtime has not landed\\\"\\n\"\n\
                 \"  }\\n\"\n\
                 \"}\\n\",\n\
                 context->poll_count,\n\
                 context->yield_count,\n\
                 context->frame_drops,\n\
                 context->peak_live_frames,\n\
                 context->ready_queue_enqueues,\n\
                 context->ready_queue_dequeues\n\
             );\n\
             int close_status = fclose(output);\n\
             return write_status < 0 || close_status != 0;\n\
         }\n",
    );
}

pub(super) fn emit_async_function(
    out: &mut String,
    function: &Function,
    async_names: &BTreeSet<String>,
) {
    debug_assert_eq!(function.return_type, ValueType::Void);
    debug_assert!(function.params.is_empty());
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
             context->poll_count += 1u;\n\
             switch (frame->state) {\n",
    );

    let empty_deferred = Vec::new();
    let mut local_owned = Vec::new();
    let mut state = 0u32;
    let mut segment_start = 0usize;
    out.push_str("        case 0u: {\n");
    for (index, statement) in function.body.iter().enumerate() {
        if statement_is_async_suspend(statement, async_names) {
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
                out.push_str("            return NOMO_ASYNC_POLL_PENDING;\n");
            } else {
                out.push_str("            goto nomo_async_resume_");
                out.push_str(&state.to_string());
                out.push_str(";\n");
            }
            out.push_str("        }\n");
            out.push_str("        case ");
            out.push_str(&state.to_string());
            out.push_str("u: {\n");
            if statement_async_call(statement, async_names).is_some() {
                out.push_str("nomo_async_resume_");
                out.push_str(&state.to_string());
                out.push_str(":\n            ;\n");
            }
            segment_start = index + 1;
            let segment_end = next_async_suspend(function, segment_start, async_names);
            for local in frame_locals
                .iter()
                .filter(|local| local.declaration_index < segment_start)
                .filter(|local| {
                    function.body[segment_start..segment_end]
                        .iter()
                        .any(|statement| statement_uses_binding(statement, &local.name))
                })
            {
                emit_async_frame_alias(out, local, 3);
            }
            if let Some(callee) = statement_async_call(statement, async_names) {
                out.push_str("            if (");
                out.push_str(&async_poll_ident(callee));
                out.push_str("(&frame->");
                out.push_str(&async_child_field(index));
                out.push_str(
                    ", context) == NOMO_ASYNC_POLL_PENDING) {\n\
                                 return NOMO_ASYNC_POLL_PENDING;\n\
                             }\n\
                             ",
                );
                out.push_str(&async_drop_ident(callee));
                out.push_str("(&frame->");
                out.push_str(&async_child_field(index));
                out.push_str(");\n");
            }
            continue;
        }
        emit_stmt(
            out,
            statement,
            3,
            &empty_deferred,
            &ValueType::Void,
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
    emit_async_local_releases(out, &local_owned, &[], 3);
    out.push_str(
        "            frame->state = UINT32_MAX;\n\
                     return NOMO_ASYNC_POLL_READY;\n\
                 }\n\
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
                 frame->context = NULL;\n\
             }\n",
    );
    for (index, statement) in function.body.iter().enumerate().rev() {
        let Some(callee) = statement_async_call(statement, async_names) else {
            continue;
        };
        out.push_str("    ");
        out.push_str(&async_drop_ident(callee));
        out.push_str("(&frame->");
        out.push_str(&async_child_field(index));
        out.push_str(");\n");
    }
    for local in frame_locals.iter().rev() {
        emit_async_frame_field_drop(out, local, 1);
    }
    out.push_str(
        "    frame->state = UINT32_MAX;\n\
         }\n",
    );
}
