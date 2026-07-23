use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn emit_match_statement(
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
pub(super) fn emit_if_statement(
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
pub(super) fn emit_if_let(
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

#[allow(clippy::too_many_arguments)]
pub(super) fn emit_loop(
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
        LoopKind::CStyle {
            binding,
            value_type,
            initializer,
            condition,
            update,
        } => {
            write_indent(out, indent);
            out.push_str("for (");
            out.push_str(&c_type(value_type));
            out.push(' ');
            out.push_str(&c_var_ident(binding));
            out.push_str(" = ");
            emit_expr(out, initializer);
            out.push_str("; ");
            emit_expr(out, condition);
            out.push_str("; ");
            out.push_str(&c_var_ident(binding));
            out.push_str(" = ");
            emit_expr(out, update);
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
