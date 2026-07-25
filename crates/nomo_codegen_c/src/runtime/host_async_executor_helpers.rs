use super::*;

pub(super) fn function_uses_async_yield(function: &Function) -> bool {
    function
        .body
        .iter()
        .any(|statement| statement_contains_expr(statement, expr_is_async_yield))
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct AsyncFrameLocal {
    name: String,
    value_type: ValueType,
    declaration_index: usize,
    last_use_index: usize,
}

fn collect_async_frame_locals(function: &Function) -> Vec<AsyncFrameLocal> {
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
            let crosses_yield = function.body[declaration_index + 1..last_use_index]
                .iter()
                .any(statement_is_async_yield);
            crosses_yield.then(|| AsyncFrameLocal {
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

fn emit_async_frame_type(out: &mut String, frame_locals: &[AsyncFrameLocal]) {
    out.push_str(
        "typedef struct {\n\
             uint32_t state;\n\
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
    out.push_str("} nomo_async_frame_main;\n\n");
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

fn next_async_yield(function: &Function, start: usize) -> usize {
    function
        .body
        .iter()
        .enumerate()
        .skip(start)
        .find_map(|(index, statement)| statement_is_async_yield(statement).then_some(index))
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
         }\n",
    );
}

pub(super) fn emit_async_main(out: &mut String, function: &Function) {
    debug_assert_eq!(function.name, "main");
    debug_assert_eq!(function.return_type, ValueType::Void);
    debug_assert!(function.params.is_empty());
    debug_assert!(function_uses_async_yield(function));

    let frame_locals = collect_async_frame_locals(function);
    emit_async_frame_type(out, &frame_locals);
    out.push_str(
        "static nomo_async_poll nomo_async_poll_main(\n\
             void *raw_frame,\n\
             nomo_async_context *context\n\
         ) {\n\
             nomo_async_frame_main *frame = (nomo_async_frame_main *)raw_frame;\n\
             context->poll_count += 1u;\n\
             switch (frame->state) {\n",
    );

    let empty_deferred = Vec::new();
    let mut local_owned = Vec::new();
    let mut state = 0u32;
    let mut segment_start = 0usize;
    out.push_str("        case 0u: {\n");
    for (index, statement) in function.body.iter().enumerate() {
        if statement_is_async_yield(statement) {
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
            out.push_str("            context->yield_count += 1u;\n");
            out.push_str("            return NOMO_ASYNC_POLL_PENDING;\n");
            out.push_str("        }\n");
            out.push_str("        case ");
            out.push_str(&state.to_string());
            out.push_str("u: {\n");
            segment_start = index + 1;
            let segment_end = next_async_yield(function, segment_start);
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
         static void nomo_async_drop_main(nomo_async_frame_main *frame) {\n\
             if (frame->dropped != 0u) {\n\
                 return;\n\
             }\n\
             frame->dropped = 1u;\n",
    );
    for local in frame_locals.iter().rev() {
        emit_async_frame_field_drop(out, local, 1);
    }
    out.push_str(
        "    frame->state = UINT32_MAX;\n\
         }\n",
    );
}
