use super::*;

#[derive(Clone, Copy)]
enum IoStream {
    Stdout,
    Stderr,
}

pub(super) fn emit_io_output(
    out: &mut String,
    value: &ValueExpr,
    stderr: bool,
    newline: bool,
    indent: usize,
) {
    let stream = if stderr {
        IoStream::Stderr
    } else {
        IoStream::Stdout
    };
    emit_io_value(out, value, stream, newline, indent);
}

fn emit_io_value(
    out: &mut String,
    value: &ValueExpr,
    stream: IoStream,
    newline: bool,
    indent: usize,
) {
    match value {
        ValueExpr::StringConcat { left, right } => {
            emit_io_fragment(out, left, stream, indent);
            emit_io_fragment(out, right, stream, indent);
            if newline {
                emit_io_newline(out, stream, indent);
            }
        }
        ValueExpr::If {
            condition,
            then_branch,
            else_branch,
        } => {
            write_indent(out, indent);
            out.push_str("if (");
            emit_expr(out, condition);
            out.push_str(") {\n");
            emit_io_value(out, then_branch, stream, newline, indent + 1);
            write_indent(out, indent);
            out.push_str("} else {\n");
            emit_io_value(out, else_branch, stream, newline, indent + 1);
            write_indent(out, indent);
            out.push_str("}\n");
        }
        ValueExpr::Match { value, arms } if !arms.is_empty() => {
            emit_io_match_value(out, value, arms, stream, newline, indent, 0);
        }
        _ => emit_io_leaf(out, value, stream, newline, indent),
    }
}

fn emit_io_fragment(out: &mut String, value: &ValueExpr, stream: IoStream, indent: usize) {
    match value {
        ValueExpr::StringConcat { left, right } => {
            emit_io_fragment(out, left, stream, indent);
            emit_io_fragment(out, right, stream, indent);
        }
        ValueExpr::If {
            condition,
            then_branch,
            else_branch,
        } => {
            write_indent(out, indent);
            out.push_str("if (");
            emit_expr(out, condition);
            out.push_str(") {\n");
            emit_io_fragment(out, then_branch, stream, indent + 1);
            write_indent(out, indent);
            out.push_str("} else {\n");
            emit_io_fragment(out, else_branch, stream, indent + 1);
            write_indent(out, indent);
            out.push_str("}\n");
        }
        ValueExpr::Match { value, arms } if !arms.is_empty() => {
            emit_io_match_value(out, value, arms, stream, false, indent, 0);
        }
        _ => emit_io_leaf(out, value, stream, false, indent),
    }
}

fn emit_io_match_value(
    out: &mut String,
    value: &ValueExpr,
    arms: &[MatchValueArm],
    stream: IoStream,
    newline: bool,
    indent: usize,
    index: usize,
) {
    let arm = &arms[index];
    if index + 1 == arms.len() {
        emit_io_value(out, &arm.value, stream, newline, indent);
        return;
    }

    write_indent(out, indent);
    out.push_str("if (");
    emit_expr(out, value);
    out.push_str(".tag == ");
    out.push_str(&c_enum_variant_ident(
        &arm.enum_name,
        &arm.enum_args,
        &arm.variant,
    ));
    out.push_str(") {\n");
    emit_io_value(out, &arm.value, stream, newline, indent + 1);
    write_indent(out, indent);
    out.push_str("} else {\n");
    emit_io_match_value(out, value, arms, stream, newline, indent + 1, index + 1);
    write_indent(out, indent);
    out.push_str("}\n");
}

fn emit_io_leaf(
    out: &mut String,
    value: &ValueExpr,
    stream: IoStream,
    newline: bool,
    indent: usize,
) {
    if io_value_needs_release(value) {
        write_indent(out, indent);
        out.push_str("{\n");
        write_indent(out, indent + 1);
        out.push_str("nomo_string nomo__io_value = ");
        emit_expr(out, value);
        out.push_str(";\n");
        emit_io_leaf_data(out, "nomo__io_value.data", stream, newline, indent + 1);
        write_indent(out, indent + 1);
        out.push_str("nomo_string_release(nomo__io_value);\n");
        write_indent(out, indent);
        out.push_str("}\n");
        return;
    }

    let mut data = String::new();
    emit_string_data_expr(&mut data, value);
    emit_io_leaf_data(out, &data, stream, newline, indent);
}

fn emit_io_leaf_data(out: &mut String, data: &str, stream: IoStream, newline: bool, indent: usize) {
    write_indent(out, indent);
    match (stream, newline) {
        (IoStream::Stdout, true) => {
            out.push_str("puts(");
            out.push_str(data);
            out.push_str(");\n");
        }
        (IoStream::Stdout, false) => {
            out.push_str("fputs(");
            out.push_str(data);
            out.push_str(", stdout);\n");
        }
        (IoStream::Stderr, _) => {
            out.push_str("fputs(");
            out.push_str(data);
            out.push_str(", stderr);\n");
            if newline {
                emit_io_newline(out, stream, indent);
            }
        }
    }
}

fn emit_io_newline(out: &mut String, stream: IoStream, indent: usize) {
    write_indent(out, indent);
    match stream {
        IoStream::Stdout => out.push_str("fputc('\\n', stdout);\n"),
        IoStream::Stderr => out.push_str("fputc('\\n', stderr);\n"),
    }
}

fn io_value_needs_release(value: &ValueExpr) -> bool {
    // Print arguments are already lowered to strings. Variables and projections
    // borrow existing storage, while conversions and ordinary calls return
    // caller-owned values under the C99 ABI. JSON stringify is intentionally a
    // borrowed view of JsonValue.raw.
    match value {
        ValueExpr::StringLiteral(_)
        | ValueExpr::Variable(_)
        | ValueExpr::FunctionRef(_)
        | ValueExpr::FieldAccess { .. }
        | ValueExpr::EnumPayload { .. }
        | ValueExpr::EnumPayloadFieldAccess { .. }
        | ValueExpr::ArrayIndex { .. }
        | ValueExpr::JsonStringify { .. }
        | ValueExpr::MutBorrow(_) => false,
        ValueExpr::Cast { expr, .. } => io_value_needs_release(expr),
        ValueExpr::StringConcat { .. } | ValueExpr::If { .. } | ValueExpr::Match { .. } => {
            unreachable!("compound output values are emitted recursively")
        }
        _ => true,
    }
}
