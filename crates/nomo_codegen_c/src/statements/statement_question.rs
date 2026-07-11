use super::*;

pub(super) fn emit_question_let(
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

pub(super) fn emit_question_return(
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
