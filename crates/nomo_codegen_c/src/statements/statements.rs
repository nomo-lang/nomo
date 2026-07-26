use super::*;

pub(super) fn emit_stmt(
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
            early_exit_actions,
        } => {
            assert!(
                early_exit_actions.is_empty(),
                "structured question cleanup is emitted by the async C99 lowering"
            );
            emit_question_let(
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
            );
        }
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
        Statement::ArrayIndexAssign {
            root,
            indices,
            array_types,
            value,
        } => emit_array_index_assign(out, root, indices, array_types, value, indent),
        Statement::Println(arg) => emit_io_output(out, arg, false, true, indent),
        Statement::Print(arg) => emit_io_output(out, arg, false, false, indent),
        Statement::Eprintln(arg) => emit_io_output(out, arg, true, true, indent),
        Statement::Eprint(arg) => emit_io_output(out, arg, true, false, indent),
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

fn emit_array_index_assign(
    out: &mut String,
    root: &str,
    indices: &[ValueExpr],
    element_types: &[ValueType],
    value: &ValueExpr,
    indent: usize,
) {
    let padding = "    ".repeat(indent);
    out.push_str(&padding);
    out.push_str("{\n");
    for (position, index) in indices.iter().enumerate() {
        out.push_str(&padding);
        out.push_str("    uint64_t nomo__index_");
        out.push_str(&position.to_string());
        out.push_str(" = ");
        emit_expr(out, index);
        out.push_str(";\n");
    }
    for (depth, element_type) in element_types
        .iter()
        .enumerate()
        .take(indices.len().saturating_sub(1))
    {
        out.push_str(&padding);
        out.push_str("    ");
        out.push_str(&c_type(element_type));
        out.push_str(" nomo__child_");
        out.push_str(&depth.to_string());
        out.push_str(" = ");
        out.push_str(&c_array_ident(element_type));
        out.push_str("_index(");
        if depth == 0 {
            out.push_str(&c_var_ident(root));
        } else {
            out.push_str("nomo__child_");
            out.push_str(&(depth - 1).to_string());
        }
        out.push_str(", nomo__index_");
        out.push_str(&depth.to_string());
        out.push_str(");\n");
    }
    let leaf_depth = indices.len() - 1;
    out.push_str(&padding);
    out.push_str("    ");
    if leaf_depth == 0 {
        out.push_str(&c_var_ident(root));
    } else {
        out.push_str("nomo__child_");
        out.push_str(&(leaf_depth - 1).to_string());
    }
    out.push_str(" = ");
    out.push_str(&c_array_ident(&element_types[leaf_depth]));
    out.push_str("_set(");
    if leaf_depth == 0 {
        out.push_str(&c_var_ident(root));
    } else {
        out.push_str("nomo__child_");
        out.push_str(&(leaf_depth - 1).to_string());
    }
    out.push_str(", nomo__index_");
    out.push_str(&leaf_depth.to_string());
    out.push_str(", ");
    emit_expr(out, value);
    out.push_str(");\n");

    for depth in (0..leaf_depth).rev() {
        out.push_str(&padding);
        out.push_str("    ");
        if depth == 0 {
            out.push_str(&c_var_ident(root));
        } else {
            out.push_str("nomo__child_");
            out.push_str(&(depth - 1).to_string());
        }
        out.push_str(" = ");
        out.push_str(&c_array_ident(&element_types[depth]));
        out.push_str("_set(");
        if depth == 0 {
            out.push_str(&c_var_ident(root));
        } else {
            out.push_str("nomo__child_");
            out.push_str(&(depth - 1).to_string());
        }
        out.push_str(", nomo__index_");
        out.push_str(&depth.to_string());
        out.push_str(", nomo__child_");
        out.push_str(&depth.to_string());
        out.push_str(");\n");
        out.push_str(&padding);
        out.push_str("    ");
        out.push_str(&c_array_ident(&element_types[depth + 1]));
        out.push_str("_release(nomo__child_");
        out.push_str(&depth.to_string());
        out.push_str(");\n");
    }
    out.push_str(&padding);
    out.push_str("}\n");
}

pub(super) fn emit_block(
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
