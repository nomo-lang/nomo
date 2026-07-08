use super::*;

pub(super) fn emit_function(out: &mut String, function: &Function) {
    emit_signature(out, function);
    out.push_str(" {\n");
    emit_mut_param_macros(out, function);
    emit_body(out, function);
    if function.return_type == ValueType::Void {
        out.push_str("    return;\n");
    }
    emit_mut_param_undefs(out, function);
    out.push_str("}\n");
}

pub(super) fn emit_signature(out: &mut String, function: &Function) {
    out.push_str(&c_type(&function.return_type));
    out.push(' ');
    out.push_str(&c_fn_ident(&function.name));
    out.push('(');
    if function.params.is_empty() {
        out.push_str("void");
    } else {
        for (index, param) in function.params.iter().enumerate() {
            if index > 0 {
                out.push_str(", ");
            }
            out.push_str(&c_type(&param.value_type));
            if param.mutable {
                out.push_str(" *");
            }
            out.push(' ');
            out.push_str(&c_var_ident(&param.name));
        }
    }
    out.push(')');
}

fn emit_mut_param_macros(out: &mut String, function: &Function) {
    for param in &function.params {
        if param.mutable {
            let name = c_var_ident(&param.name);
            out.push_str("#define ");
            out.push_str(&name);
            out.push_str(" (*");
            out.push_str(&name);
            out.push_str(")\n");
        }
    }
}

fn emit_mut_param_undefs(out: &mut String, function: &Function) {
    for param in &function.params {
        if param.mutable {
            out.push_str("#undef ");
            out.push_str(&c_var_ident(&param.name));
            out.push('\n');
        }
    }
}

pub(super) fn emit_body(out: &mut String, function: &Function) {
    let mut deferred: Vec<DeferredCall> = Vec::new();
    let mut active_arrays = array_params(function);
    for local in &active_arrays {
        emit_array_retain_binding(out, &local.name, &local.value_type, 1);
    }
    let mut last_statement_exits = false;
    for statement in &function.body {
        if let Statement::Defer { call } = statement {
            deferred.push(call.clone());
        } else {
            emit_stmt(
                out,
                statement,
                1,
                &deferred,
                &function.return_type,
                &active_arrays,
                0,
                0,
                0,
                0,
            );
            if let Some(local) = local_array_from_statement(statement) {
                active_arrays.push(local);
            }
            last_statement_exits = statement_exits_function(statement);
        }
    }
    if !last_statement_exits {
        emit_deferred(out, 1, &deferred);
        emit_array_releases(out, 1, &active_arrays);
    }
}

pub(super) fn write_indent(out: &mut String, indent: usize) {
    for _ in 0..indent {
        out.push_str("    ");
    }
}

fn emit_deferred(out: &mut String, indent: usize, deferred: &[DeferredCall]) {
    for call in deferred.iter().rev() {
        emit_deferred_call(out, indent, call);
    }
}

fn emit_deferred_call(out: &mut String, indent: usize, call: &DeferredCall) {
    match call {
        DeferredCall::Expr(expr) => {
            write_indent(out, indent);
            emit_expr(out, expr);
            out.push_str(";\n");
        }
        DeferredCall::Println(arg) => {
            write_indent(out, indent);
            out.push_str("puts(");
            emit_string_data_expr(out, arg);
            out.push_str(");\n");
        }
        DeferredCall::Print(arg) => {
            write_indent(out, indent);
            out.push_str("fputs(");
            emit_string_data_expr(out, arg);
            out.push_str(", stdout);\n");
        }
        DeferredCall::Eprintln(arg) => {
            write_indent(out, indent);
            out.push_str("fputs(");
            emit_string_data_expr(out, arg);
            out.push_str(", stderr);\n");
            write_indent(out, indent);
            out.push_str("fputc('\\n', stderr);\n");
        }
        DeferredCall::Eprint(arg) => {
            write_indent(out, indent);
            out.push_str("fputs(");
            emit_string_data_expr(out, arg);
            out.push_str(", stderr);\n");
        }
    }
}

fn statement_exits_function(statement: &Statement) -> bool {
    match statement {
        Statement::Return(_) | Statement::QuestionReturn { .. } | Statement::Panic(_) => true,
        Statement::Match { arms, .. } => arms.iter().all(|arm| statements_exit_function(&arm.body)),
        Statement::If {
            body, else_body, ..
        } => statements_exit_function(body) && statements_exit_function(else_body),
        Statement::IfLet {
            body,
            else_body: Some(else_body),
            ..
        } => statements_exit_function(body) && statements_exit_function(else_body),
        _ => false,
    }
}

fn statements_exit_function(statements: &[Statement]) -> bool {
    statements.last().is_some_and(statement_exits_function)
}

fn statement_exits_block(statement: &Statement) -> bool {
    match statement {
        Statement::Return(_)
        | Statement::QuestionReturn { .. }
        | Statement::Panic(_)
        | Statement::Break
        | Statement::Continue => true,
        Statement::Match { arms, .. } => arms.iter().all(|arm| statements_exit_block(&arm.body)),
        Statement::If {
            body, else_body, ..
        } => statements_exit_block(body) && statements_exit_block(else_body),
        Statement::IfLet {
            body,
            else_body: Some(else_body),
            ..
        } => statements_exit_block(body) && statements_exit_block(else_body),
        _ => false,
    }
}

fn statements_exit_block(statements: &[Statement]) -> bool {
    statements.last().is_some_and(statement_exits_block)
}

fn emit_stmt(
    out: &mut String,
    statement: &Statement,
    indent: usize,
    deferred: &[DeferredCall],
    function_return_type: &ValueType,
    active_arrays: &[LocalArray],
    break_deferred_start: usize,
    continue_deferred_start: usize,
    break_cleanup_start: usize,
    continue_cleanup_start: usize,
) {
    match statement {
        Statement::Let {
            name,
            value_type,
            initializer,
        } => emit_let(out, name, value_type, initializer, indent),
        Statement::LetIf {
            name,
            value_type,
            condition,
            body,
            else_body,
        } => emit_let_if(
            out,
            name,
            value_type,
            condition,
            body,
            else_body,
            indent,
            deferred,
            function_return_type,
            active_arrays,
            break_deferred_start,
            continue_deferred_start,
            break_cleanup_start,
            continue_cleanup_start,
        ),
        Statement::LetMatch {
            name,
            value_type,
            value,
            enum_name,
            enum_args,
            arms,
        } => emit_let_match(
            out,
            name,
            value_type,
            value,
            enum_name,
            enum_args,
            arms,
            indent,
            deferred,
            function_return_type,
            active_arrays,
            break_deferred_start,
            continue_deferred_start,
            break_cleanup_start,
            continue_cleanup_start,
        ),
        Statement::LetElse {
            binding,
            value_type,
            value,
            enum_name,
            enum_args,
            variant,
            else_body,
        } => emit_let_else(
            out,
            binding,
            value_type,
            value,
            enum_name,
            enum_args,
            variant,
            else_body,
            indent,
            deferred,
            function_return_type,
            active_arrays,
            break_deferred_start,
            continue_deferred_start,
            break_cleanup_start,
            continue_cleanup_start,
        ),
        Statement::IfLet {
            binding,
            value_type,
            value,
            enum_name,
            enum_args,
            variant,
            body,
            else_body,
        } => emit_if_let(
            out,
            binding.as_deref(),
            value_type.as_ref(),
            value,
            enum_name,
            enum_args,
            variant,
            body,
            else_body.as_deref(),
            indent,
            deferred,
            function_return_type,
            active_arrays,
            break_deferred_start,
            continue_deferred_start,
            break_cleanup_start,
            continue_cleanup_start,
        ),
        Statement::If {
            condition,
            body,
            else_body,
        } => emit_if_statement(
            out,
            condition,
            body,
            else_body,
            indent,
            deferred,
            function_return_type,
            active_arrays,
            break_deferred_start,
            continue_deferred_start,
            break_cleanup_start,
            continue_cleanup_start,
        ),
        Statement::QuestionLet {
            carrier,
            name,
            value_type,
            result_type,
            return_type,
            result_expr,
        } => emit_question_let(
            out,
            *carrier,
            name,
            value_type,
            result_type,
            return_type,
            result_expr,
            indent,
            deferred,
            active_arrays,
        ),
        Statement::QuestionReturn {
            carrier,
            ok_type,
            result_type,
            return_type,
            result_expr,
        } => emit_question_return(
            out,
            *carrier,
            ok_type,
            result_type,
            return_type,
            result_expr,
            indent,
            deferred,
            active_arrays,
        ),
        Statement::Assign { name, value } => emit_assign(out, name, value, indent, active_arrays),
        Statement::AssignField {
            base,
            field,
            value_type,
            value,
        } => emit_assign_field(out, base, field, value_type, value, indent),
        Statement::Println(arg) => {
            write_indent(out, indent);
            out.push_str("puts(");
            emit_string_data_expr(out, arg);
            out.push_str(");\n");
        }
        Statement::Print(arg) => {
            write_indent(out, indent);
            out.push_str("fputs(");
            emit_string_data_expr(out, arg);
            out.push_str(", stdout);\n");
        }
        Statement::Eprintln(arg) => {
            write_indent(out, indent);
            out.push_str("fputs(");
            emit_string_data_expr(out, arg);
            out.push_str(", stderr);\n");
            write_indent(out, indent);
            out.push_str("fputc('\\n', stderr);\n");
        }
        Statement::Eprint(arg) => {
            write_indent(out, indent);
            out.push_str("fputs(");
            emit_string_data_expr(out, arg);
            out.push_str(", stderr);\n");
        }
        Statement::Panic(message) => {
            emit_deferred(out, indent, deferred);
            emit_array_releases(out, indent, active_arrays);
            write_indent(out, indent);
            out.push_str("nomo_panic(");
            emit_string_data_expr(out, message);
            out.push_str(");\n");
        }
        Statement::Return(Some(value)) => emit_return_value(
            out,
            value,
            indent,
            deferred,
            function_return_type,
            active_arrays,
        ),
        Statement::Return(None) => {
            emit_deferred(out, indent, deferred);
            emit_array_releases(out, indent, active_arrays);
            write_indent(out, indent);
            out.push_str("return;\n");
        }
        Statement::Expr(value) => {
            write_indent(out, indent);
            emit_expr(out, value);
            out.push_str(";\n");
        }
        Statement::Match {
            value,
            enum_name,
            enum_args,
            arms,
        } => emit_match_statement(
            out,
            value,
            enum_name,
            enum_args,
            arms,
            indent,
            deferred,
            function_return_type,
            active_arrays,
            break_deferred_start,
            continue_deferred_start,
            break_cleanup_start,
            continue_cleanup_start,
        ),
        Statement::Loop { kind, body } => emit_loop(
            out,
            kind,
            body,
            indent,
            deferred,
            function_return_type,
            active_arrays,
            break_deferred_start,
            continue_deferred_start,
            break_cleanup_start,
            continue_cleanup_start,
        ),
        Statement::Break => {
            emit_deferred(out, indent, &deferred[break_deferred_start..]);
            emit_array_releases(out, indent, &active_arrays[break_cleanup_start..]);
            write_indent(out, indent);
            out.push_str("break;\n");
        }
        Statement::Continue => {
            emit_deferred(out, indent, &deferred[continue_deferred_start..]);
            emit_array_releases(out, indent, &active_arrays[continue_cleanup_start..]);
            write_indent(out, indent);
            out.push_str("continue;\n");
        }
        Statement::Defer { .. } => {
            // Deferred calls are collected by emit_body and emitted at exit points.
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn emit_match_statement(
    out: &mut String,
    value: &ValueExpr,
    enum_name: &str,
    enum_args: &[ValueType],
    arms: &[MatchStatementArm],
    indent: usize,
    deferred: &[DeferredCall],
    function_return_type: &ValueType,
    active_arrays: &[LocalArray],
    break_deferred_start: usize,
    continue_deferred_start: usize,
    break_cleanup_start: usize,
    continue_cleanup_start: usize,
) {
    for (index, arm) in arms.iter().enumerate() {
        write_indent(out, indent);
        if index == 0 {
            out.push_str("if (");
        } else {
            out.push_str("else if (");
        }
        emit_expr(out, value);
        out.push_str(".tag == ");
        out.push_str(&c_enum_variant_ident(enum_name, enum_args, &arm.variant));
        out.push_str(") {\n");
        emit_block(
            out,
            &arm.body,
            indent + 1,
            deferred,
            function_return_type,
            active_arrays,
            break_deferred_start,
            continue_deferred_start,
            break_cleanup_start,
            continue_cleanup_start,
        );
        write_indent(out, indent);
        out.push_str("}\n");
    }
}

#[allow(clippy::too_many_arguments)]
fn emit_if_statement(
    out: &mut String,
    condition: &ValueExpr,
    body: &[Statement],
    else_body: &[Statement],
    indent: usize,
    deferred: &[DeferredCall],
    function_return_type: &ValueType,
    active_arrays: &[LocalArray],
    break_deferred_start: usize,
    continue_deferred_start: usize,
    break_cleanup_start: usize,
    continue_cleanup_start: usize,
) {
    write_indent(out, indent);
    out.push_str("if (");
    emit_expr(out, condition);
    out.push_str(") {\n");
    emit_block(
        out,
        body,
        indent + 1,
        deferred,
        function_return_type,
        active_arrays,
        break_deferred_start,
        continue_deferred_start,
        break_cleanup_start,
        continue_cleanup_start,
    );
    write_indent(out, indent);
    out.push_str("} else {\n");
    emit_block(
        out,
        else_body,
        indent + 1,
        deferred,
        function_return_type,
        active_arrays,
        break_deferred_start,
        continue_deferred_start,
        break_cleanup_start,
        continue_cleanup_start,
    );
    write_indent(out, indent);
    out.push_str("}\n");
}

#[allow(clippy::too_many_arguments)]
fn emit_if_let(
    out: &mut String,
    binding: Option<&str>,
    value_type: Option<&ValueType>,
    value: &ValueExpr,
    enum_name: &str,
    enum_args: &[ValueType],
    variant: &str,
    body: &[Statement],
    else_body: Option<&[Statement]>,
    indent: usize,
    deferred: &[DeferredCall],
    function_return_type: &ValueType,
    active_arrays: &[LocalArray],
    break_deferred_start: usize,
    continue_deferred_start: usize,
    break_cleanup_start: usize,
    continue_cleanup_start: usize,
) {
    let temp = format!(
        "nomo__if_let_{}",
        c_enum_variant_ident(enum_name, enum_args, variant)
    );
    write_indent(out, indent);
    out.push_str("{\n");
    write_indent(out, indent + 1);
    out.push_str(&c_enum_ident(enum_name, enum_args));
    out.push(' ');
    out.push_str(&temp);
    out.push_str(" = ");
    emit_expr(out, value);
    out.push_str(";\n");
    write_indent(out, indent + 1);
    out.push_str("if (");
    out.push_str(&temp);
    out.push_str(".tag == ");
    out.push_str(&c_enum_variant_ident(enum_name, enum_args, variant));
    out.push_str(") {\n");
    if let (Some(binding), Some(value_type)) = (binding, value_type) {
        write_indent(out, indent + 2);
        out.push_str(&c_type(value_type));
        out.push(' ');
        out.push_str(&c_var_ident(binding));
        out.push_str(" = ");
        out.push_str(&temp);
        out.push_str(".payload.");
        out.push_str(&c_payload_ident(variant));
        out.push_str(";\n");
        emit_array_retain_binding(out, binding, value_type, indent + 2);
    }
    emit_enum_temp_release_if_owned(out, &temp, enum_name, enum_args, value, indent + 2);
    let mut then_active_arrays = active_arrays.to_vec();
    if let (Some(binding), Some(value_type)) = (binding, value_type) {
        if let Some(local) = local_array(binding, value_type) {
            then_active_arrays.push(local);
        }
    }
    emit_block(
        out,
        body,
        indent + 2,
        deferred,
        function_return_type,
        &then_active_arrays,
        break_deferred_start,
        continue_deferred_start,
        break_cleanup_start,
        continue_cleanup_start,
    );
    if let (Some(binding), Some(value_type)) = (binding, value_type) {
        emit_value_release_binding(out, binding, value_type, indent + 2);
    }
    write_indent(out, indent + 1);
    out.push_str("}");
    if let Some(else_body) = else_body {
        out.push_str(" else {\n");
        emit_enum_temp_release_if_owned(out, &temp, enum_name, enum_args, value, indent + 2);
        emit_block(
            out,
            else_body,
            indent + 2,
            deferred,
            function_return_type,
            active_arrays,
            break_deferred_start,
            continue_deferred_start,
            break_cleanup_start,
            continue_cleanup_start,
        );
        write_indent(out, indent + 1);
        out.push('}');
    } else {
        out.push_str(" else {\n");
        emit_enum_temp_release_if_owned(out, &temp, enum_name, enum_args, value, indent + 2);
        write_indent(out, indent + 1);
        out.push('}');
    }
    out.push('\n');
    write_indent(out, indent);
    out.push_str("}\n");
}

fn emit_loop(
    out: &mut String,
    kind: &LoopKind,
    body: &[Statement],
    indent: usize,
    deferred: &[DeferredCall],
    function_return_type: &ValueType,
    active_arrays: &[LocalArray],
    _break_deferred_start: usize,
    _continue_deferred_start: usize,
    _break_cleanup_start: usize,
    _continue_cleanup_start: usize,
) {
    match kind {
        LoopKind::Infinite => {
            write_indent(out, indent);
            out.push_str("for (;;) {\n");
            emit_block(
                out,
                body,
                indent + 1,
                deferred,
                function_return_type,
                active_arrays,
                deferred.len(),
                deferred.len(),
                active_arrays.len(),
                active_arrays.len(),
            );
            write_indent(out, indent);
            out.push_str("}\n");
        }
        LoopKind::While(condition) => {
            write_indent(out, indent);
            out.push_str("while (");
            emit_expr(out, condition);
            out.push_str(") {\n");
            emit_block(
                out,
                body,
                indent + 1,
                deferred,
                function_return_type,
                active_arrays,
                deferred.len(),
                deferred.len(),
                active_arrays.len(),
                active_arrays.len(),
            );
            write_indent(out, indent);
            out.push_str("}\n");
        }
        LoopKind::Iterate {
            binding,
            element_type,
            iterable,
        } => {
            let array_type = ValueType::Array(Box::new(element_type.clone()));
            let owned_iterable = !expr_may_share_array_storage(iterable);
            write_indent(out, indent);
            out.push_str("{\n");
            write_indent(out, indent + 1);
            out.push_str(&c_type(&array_type));
            out.push_str(" nomo__seq = ");
            emit_expr(out, iterable);
            out.push_str(";\n");
            write_indent(out, indent + 1);
            out.push_str("for (uint64_t nomo_i = 0; nomo_i < nomo__seq.len; nomo_i++) {\n");
            write_indent(out, indent + 2);
            out.push_str(&c_type(element_type));
            out.push(' ');
            out.push_str(&c_var_ident(binding));
            out.push_str(" = nomo__seq.data[nomo_i];\n");
            emit_array_retain_binding(out, binding, element_type, indent + 2);
            let mut body_active_arrays = active_arrays.to_vec();
            if owned_iterable {
                if let Some(local) = local_c_value("nomo__seq", &array_type) {
                    body_active_arrays.push(local);
                }
            }
            let loop_binding_cleanup_start = body_active_arrays.len();
            if let Some(local) = local_array(binding, element_type) {
                body_active_arrays.push(local);
            }
            emit_block(
                out,
                body,
                indent + 2,
                deferred,
                function_return_type,
                &body_active_arrays,
                deferred.len(),
                deferred.len(),
                loop_binding_cleanup_start,
                loop_binding_cleanup_start,
            );
            emit_value_release_binding(out, binding, element_type, indent + 2);
            write_indent(out, indent + 1);
            out.push_str("}\n");
            if owned_iterable {
                emit_value_release_in_place(out, &array_type, "nomo__seq", indent + 1);
            }
            write_indent(out, indent);
            out.push_str("}\n");
        }
    }
}

fn emit_block(
    out: &mut String,
    body: &[Statement],
    indent: usize,
    deferred: &[DeferredCall],
    function_return_type: &ValueType,
    active_arrays: &[LocalArray],
    break_deferred_start: usize,
    continue_deferred_start: usize,
    break_cleanup_start: usize,
    continue_cleanup_start: usize,
) {
    let inherited_len = active_arrays.len();
    let mut scope_arrays = active_arrays.to_vec();
    let mut block_deferred: Vec<DeferredCall> = Vec::new();
    let mut last_statement_exits = false;
    for statement in body {
        if let Statement::Defer { call } = statement {
            block_deferred.push(call.clone());
            last_statement_exits = false;
            continue;
        }
        let mut active_deferred = deferred.to_vec();
        active_deferred.extend(block_deferred.iter().cloned());
        emit_stmt(
            out,
            statement,
            indent,
            &active_deferred,
            function_return_type,
            &scope_arrays,
            break_deferred_start,
            continue_deferred_start,
            break_cleanup_start,
            continue_cleanup_start,
        );
        if let Some(local) = local_array_from_statement(statement) {
            scope_arrays.push(local);
        }
        last_statement_exits = statement_exits_block(statement);
        if last_statement_exits {
            break;
        }
    }
    if !last_statement_exits {
        emit_deferred(out, indent, &block_deferred);
        if scope_arrays.len() > inherited_len {
            emit_array_releases(out, indent, &scope_arrays[inherited_len..]);
        }
    }
}

fn emit_return_value(
    out: &mut String,
    value: &ValueExpr,
    indent: usize,
    deferred: &[DeferredCall],
    return_type: &ValueType,
    active_arrays: &[LocalArray],
) {
    if deferred.is_empty() {
        if !active_arrays.is_empty() {
            write_indent(out, indent);
            out.push_str("{\n");
            write_indent(out, indent + 1);
            out.push_str(&c_type(return_type));
            out.push_str(" nomo__return = ");
            emit_expr(out, value);
            out.push_str(";\n");
            emit_array_retain_return_if_needed(out, value, return_type, indent + 1);
            emit_array_releases(out, indent + 1, active_arrays);
            write_indent(out, indent + 1);
            out.push_str("return nomo__return;\n");
            write_indent(out, indent);
            out.push_str("}\n");
            return;
        }
        write_indent(out, indent);
        out.push_str("return ");
        emit_expr(out, value);
        out.push_str(";\n");
        return;
    }

    write_indent(out, indent);
    out.push_str("{\n");
    write_indent(out, indent + 1);
    out.push_str(&c_type(return_type));
    out.push_str(" nomo__return = ");
    emit_expr(out, value);
    out.push_str(";\n");
    emit_array_retain_return_if_needed(out, value, return_type, indent + 1);
    emit_deferred(out, indent + 1, deferred);
    emit_array_releases(out, indent + 1, active_arrays);
    write_indent(out, indent + 1);
    out.push_str("return nomo__return;\n");
    write_indent(out, indent);
    out.push_str("}\n");
}

fn emit_let(
    out: &mut String,
    name: &str,
    value_type: &ValueType,
    initializer: &ValueExpr,
    indent: usize,
) {
    write_indent(out, indent);
    out.push_str(&c_type(value_type));
    out.push(' ');
    out.push_str(&c_var_ident(name));
    out.push_str(" = ");
    emit_expr(out, initializer);
    out.push_str(";\n");
    emit_array_retain_after_binding(out, name, value_type, initializer, indent);
}

#[allow(clippy::too_many_arguments)]
fn emit_let_if(
    out: &mut String,
    name: &str,
    value_type: &ValueType,
    condition: &ValueExpr,
    body: &[Statement],
    else_body: &[Statement],
    indent: usize,
    deferred: &[DeferredCall],
    function_return_type: &ValueType,
    active_arrays: &[LocalArray],
    break_deferred_start: usize,
    continue_deferred_start: usize,
    break_cleanup_start: usize,
    continue_cleanup_start: usize,
) {
    write_indent(out, indent);
    out.push_str(&c_type(value_type));
    out.push(' ');
    out.push_str(&c_var_ident(name));
    out.push_str(";\n");
    emit_if_statement(
        out,
        condition,
        body,
        else_body,
        indent,
        deferred,
        function_return_type,
        active_arrays,
        break_deferred_start,
        continue_deferred_start,
        break_cleanup_start,
        continue_cleanup_start,
    );
}

#[allow(clippy::too_many_arguments)]
fn emit_let_match(
    out: &mut String,
    name: &str,
    value_type: &ValueType,
    value: &ValueExpr,
    enum_name: &str,
    enum_args: &[ValueType],
    arms: &[MatchStatementArm],
    indent: usize,
    deferred: &[DeferredCall],
    function_return_type: &ValueType,
    active_arrays: &[LocalArray],
    break_deferred_start: usize,
    continue_deferred_start: usize,
    break_cleanup_start: usize,
    continue_cleanup_start: usize,
) {
    write_indent(out, indent);
    out.push_str(&c_type(value_type));
    out.push(' ');
    out.push_str(&c_var_ident(name));
    out.push_str(";\n");
    emit_match_statement(
        out,
        value,
        enum_name,
        enum_args,
        arms,
        indent,
        deferred,
        function_return_type,
        active_arrays,
        break_deferred_start,
        continue_deferred_start,
        break_cleanup_start,
        continue_cleanup_start,
    );
}

fn emit_assign(
    out: &mut String,
    name: &str,
    value: &ValueExpr,
    indent: usize,
    active_arrays: &[LocalArray],
) {
    let Some(value_type) = active_array_type(active_arrays, name) else {
        write_indent(out, indent);
        out.push_str(&c_var_ident(name));
        out.push_str(" = ");
        emit_expr(out, value);
        out.push_str(";\n");
        return;
    };
    if is_array_mutating_assignment(value) {
        write_indent(out, indent);
        out.push_str(&c_var_ident(name));
        out.push_str(" = ");
        emit_expr(out, value);
        out.push_str(";\n");
        return;
    }

    let temp = format!("nomo__assign_{}", c_var_ident(name));
    write_indent(out, indent);
    out.push_str("{\n");
    write_indent(out, indent + 1);
    out.push_str(&c_type(value_type));
    out.push(' ');
    out.push_str(&temp);
    out.push_str(" = ");
    emit_expr(out, value);
    out.push_str(";\n");
    emit_value_retain_value_if_needed(out, &temp, value_type, value, indent + 1);
    emit_value_release_binding(out, name, value_type, indent + 1);
    write_indent(out, indent + 1);
    out.push_str(&c_var_ident(name));
    out.push_str(" = ");
    out.push_str(&temp);
    out.push_str(";\n");
    write_indent(out, indent);
    out.push_str("}\n");
}

fn emit_assign_field(
    out: &mut String,
    base: &str,
    field: &str,
    value_type: &ValueType,
    value: &ValueExpr,
    indent: usize,
) {
    let field_access = format!("{}.{}", c_var_ident(base), c_member_ident(field));
    if !value_type_needs_release(value_type) {
        write_indent(out, indent);
        out.push_str(&field_access);
        out.push_str(" = ");
        emit_expr(out, value);
        out.push_str(";\n");
        return;
    }

    let temp = format!(
        "nomo__assign_{}_{}",
        c_var_ident(base),
        c_member_ident(field)
    );
    write_indent(out, indent);
    out.push_str("{\n");
    write_indent(out, indent + 1);
    out.push_str(&c_type(value_type));
    out.push(' ');
    out.push_str(&temp);
    out.push_str(" = ");
    emit_expr(out, value);
    out.push_str(";\n");
    emit_value_retain_value_if_needed(out, &temp, value_type, value, indent + 1);
    emit_value_release_in_place(out, value_type, &field_access, indent + 1);
    write_indent(out, indent + 1);
    out.push_str(&field_access);
    out.push_str(" = ");
    out.push_str(&temp);
    out.push_str(";\n");
    write_indent(out, indent);
    out.push_str("}\n");
}

fn is_array_mutating_assignment(value: &ValueExpr) -> bool {
    matches!(
        value,
        ValueExpr::ArrayPush { .. }
            | ValueExpr::ArraySet { .. }
            | ValueExpr::ArrayInsert { .. }
            | ValueExpr::ArrayClear { .. }
    )
}

#[allow(clippy::too_many_arguments)]
fn emit_let_else(
    out: &mut String,
    binding: &str,
    value_type: &ValueType,
    value: &ValueExpr,
    enum_name: &str,
    enum_args: &[ValueType],
    variant: &str,
    else_body: &[Statement],
    indent: usize,
    deferred: &[DeferredCall],
    function_return_type: &ValueType,
    active_arrays: &[LocalArray],
    break_deferred_start: usize,
    continue_deferred_start: usize,
    break_cleanup_start: usize,
    continue_cleanup_start: usize,
) {
    let temp = format!("nomo__let_else_{}", c_var_ident(binding));
    write_indent(out, indent);
    out.push_str(&c_enum_ident(enum_name, enum_args));
    out.push(' ');
    out.push_str(&temp);
    out.push_str(" = ");
    emit_expr(out, value);
    out.push_str(";\n");
    write_indent(out, indent);
    out.push_str("if (");
    out.push_str(&temp);
    out.push_str(".tag != ");
    out.push_str(&c_enum_variant_ident(enum_name, enum_args, variant));
    out.push_str(") {\n");
    emit_enum_temp_release_if_owned(out, &temp, enum_name, enum_args, value, indent + 1);
    emit_block(
        out,
        else_body,
        indent + 1,
        deferred,
        function_return_type,
        active_arrays,
        break_deferred_start,
        continue_deferred_start,
        break_cleanup_start,
        continue_cleanup_start,
    );
    write_indent(out, indent);
    out.push_str("}\n");
    write_indent(out, indent);
    out.push_str(&c_type(value_type));
    out.push(' ');
    out.push_str(&c_var_ident(binding));
    out.push_str(" = ");
    out.push_str(&temp);
    out.push_str(".payload.");
    out.push_str(&c_payload_ident(variant));
    out.push_str(";\n");
    emit_array_retain_binding(out, binding, value_type, indent);
    emit_enum_temp_release_if_owned(out, &temp, enum_name, enum_args, value, indent);
}

fn emit_question_let(
    out: &mut String,
    carrier: QuestionCarrier,
    name: &str,
    value_type: &ValueType,
    result_type: &ValueType,
    return_type: &ValueType,
    result_expr: &ValueExpr,
    indent: usize,
    deferred: &[DeferredCall],
    active_arrays: &[LocalArray],
) {
    let temp = format!("{}_result", c_var_ident(name));
    write_indent(out, indent);
    out.push_str(&c_type(result_type));
    out.push(' ');
    out.push_str(&temp);
    out.push_str(" = ");
    emit_expr(out, result_expr);
    out.push_str(";\n");
    let ValueType::Enum(result_name, result_args) = result_type else {
        return;
    };
    let (early_variant, payload_variant) = match carrier {
        QuestionCarrier::Result => ("Err", "Ok"),
        QuestionCarrier::Option => ("None", "Some"),
    };
    write_indent(out, indent);
    out.push_str("if (");
    out.push_str(&temp);
    out.push_str(".tag == ");
    out.push_str(&c_enum_variant_ident(
        result_name,
        result_args,
        early_variant,
    ));
    out.push_str(") {\n");
    write_indent(out, indent + 1);
    let (return_name, return_args) = match return_type {
        ValueType::Enum(name, args) => (name, args),
        _ => (result_name, result_args),
    };
    out.push_str(&c_enum_ident(return_name, return_args));
    out.push_str(" nomo__question_return = (");
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
        out.push_str(&temp);
        out.push_str(".payload.");
        out.push_str(&c_payload_ident("Err"));
    }
    out.push_str("};\n");
    if carrier == QuestionCarrier::Result
        && expr_may_share_array_storage(result_expr)
        && value_type_needs_release(return_type)
    {
        emit_value_retain_in_place(out, return_type, "nomo__question_return", indent + 1);
    }
    emit_deferred(out, indent + 1, deferred);
    emit_array_releases(out, indent + 1, active_arrays);
    write_indent(out, indent + 1);
    out.push_str("return nomo__question_return;\n");
    write_indent(out, indent);
    out.push_str("}\n");
    write_indent(out, indent);
    out.push_str(&c_payload_type(value_type));
    out.push(' ');
    out.push_str(&c_var_ident(name));
    out.push_str(" = ");
    out.push_str(&temp);
    out.push_str(".payload.");
    out.push_str(&c_payload_ident(payload_variant));
    out.push_str(";\n");
    if expr_may_share_array_storage(result_expr) && value_type_needs_release(value_type) {
        emit_value_retain_in_place(out, value_type, &c_var_ident(name), indent);
    }
}

fn emit_question_return(
    out: &mut String,
    carrier: QuestionCarrier,
    ok_type: &ValueType,
    result_type: &ValueType,
    return_type: &ValueType,
    result_expr: &ValueExpr,
    indent: usize,
    deferred: &[DeferredCall],
    active_arrays: &[LocalArray],
) {
    write_indent(out, indent);
    out.push_str("{\n");
    write_indent(out, indent + 1);
    out.push_str(&c_type(result_type));
    out.push_str(" nomo__question_result = ");
    emit_expr(out, result_expr);
    out.push_str(";\n");
    let ValueType::Enum(result_name, result_args) = result_type else {
        write_indent(out, indent);
        out.push_str("}\n");
        return;
    };
    let (return_name, return_args) = match return_type {
        ValueType::Enum(name, args) => (name, args),
        _ => (result_name, result_args),
    };
    let (early_variant, payload_variant) = match carrier {
        QuestionCarrier::Result => ("Err", "Ok"),
        QuestionCarrier::Option => ("None", "Some"),
    };
    write_indent(out, indent + 1);
    out.push_str("if (nomo__question_result.tag == ");
    out.push_str(&c_enum_variant_ident(
        result_name,
        result_args,
        early_variant,
    ));
    out.push_str(") {\n");
    write_indent(out, indent + 2);
    out.push_str(&c_enum_ident(return_name, return_args));
    out.push_str(" nomo__question_return = (");
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
        out.push_str(" = nomo__question_result.payload.");
        out.push_str(&c_payload_ident("Err"));
    }
    out.push_str("};\n");
    if carrier == QuestionCarrier::Result
        && expr_may_share_array_storage(result_expr)
        && value_type_needs_release(return_type)
    {
        emit_value_retain_in_place(out, return_type, "nomo__question_return", indent + 2);
    }
    emit_deferred(out, indent + 2, deferred);
    emit_array_releases(out, indent + 2, active_arrays);
    write_indent(out, indent + 2);
    out.push_str("return nomo__question_return;\n");
    write_indent(out, indent + 1);
    out.push_str("}\n");
    write_indent(out, indent + 1);
    out.push_str(&c_payload_type(ok_type));
    out.push_str(" nomo__question_ok = nomo__question_result.payload.");
    out.push_str(&c_payload_ident(payload_variant));
    out.push_str(";\n");
    write_indent(out, indent + 1);
    out.push_str(&c_enum_ident(return_name, return_args));
    out.push_str(" nomo__return = (");
    out.push_str(&c_enum_ident(return_name, return_args));
    out.push_str("){.tag = ");
    out.push_str(&c_enum_variant_ident(
        return_name,
        return_args,
        payload_variant,
    ));
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident(payload_variant));
    out.push_str(" = nomo__question_ok};\n");
    if expr_may_share_array_storage(result_expr) && value_type_needs_release(return_type) {
        emit_value_retain_in_place(out, return_type, "nomo__return", indent + 1);
    }
    emit_deferred(out, indent + 1, deferred);
    emit_array_releases(out, indent + 1, active_arrays);
    write_indent(out, indent + 1);
    out.push_str("return nomo__return;\n");
    write_indent(out, indent);
    out.push_str("}\n");
}
