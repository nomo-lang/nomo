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

    out.push_str(
        "typedef struct {\n\
             uint32_t state;\n\
             uint8_t dropped;\n\
         } nomo_async_frame_main;\n\
         \n\
         static nomo_async_poll nomo_async_poll_main(\n\
             void *raw_frame,\n\
             nomo_async_context *context\n\
         ) {\n\
             nomo_async_frame_main *frame = (nomo_async_frame_main *)raw_frame;\n\
             context->poll_count += 1u;\n\
             switch (frame->state) {\n",
    );

    let empty_deferred = Vec::new();
    let empty_locals = Vec::new();
    let mut state = 0u32;
    out.push_str("        case 0u: {\n");
    for statement in &function.body {
        if statement_is_async_yield(statement) {
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
            continue;
        }
        emit_stmt(
            out,
            statement,
            3,
            &empty_deferred,
            &ValueType::Void,
            &empty_locals,
            0,
            0,
            0,
            0,
        );
    }
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
             frame->dropped = 1u;\n\
             frame->state = UINT32_MAX;\n\
         }\n",
    );
}
