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

pub(super) fn emit_deferred(out: &mut String, indent: usize, deferred: &[DeferredCall]) {
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

pub(super) fn statement_exits_block(statement: &Statement) -> bool {
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
