use super::*;

pub(super) fn emit_num_parse_i64_helper(out: &mut String) {
    emit_num_parse_helper(out, "i64", &ValueType::Int, "strtoll", "int64_t");
}

pub(super) fn emit_num_parse_u64_helper(out: &mut String) {
    emit_num_parse_helper(out, "u64", &ValueType::U64, "strtoull", "uint64_t");
}

pub(super) fn emit_num_parse_f64_helper(out: &mut String) {
    emit_num_parse_helper(out, "f64", &ValueType::Float, "strtod", "double");
}

fn emit_num_parse_helper(
    out: &mut String,
    suffix: &str,
    ok_type: &ValueType,
    c_parse_fn: &str,
    c_value_type: &str,
) {
    let num_error = ValueType::Struct("NumError".to_string(), Vec::new());
    let result = c_enum_ident("Result", &[ok_type.clone(), num_error.clone()]);
    let ok = c_enum_variant_ident("Result", &[ok_type.clone(), num_error.clone()], "Ok");
    let err = c_enum_variant_ident("Result", &[ok_type.clone(), num_error], "Err");
    let num_error_struct = c_struct_ident("NumError", &[]);
    out.push_str("static ");
    out.push_str(&result);
    out.push_str(" nomo_num_parse_");
    out.push_str(suffix);
    out.push_str("(nomo_string text) {\n");
    out.push_str("    errno = 0;\n");
    out.push_str("    char *end = NULL;\n");
    if suffix == "u64" {
        out.push_str("    if (text.data[0] == '-') {\n");
        emit_num_parse_error(out, &result, &err, &num_error_struct, suffix);
        out.push_str("    }\n");
    }
    out.push_str("    ");
    out.push_str(c_value_type);
    out.push_str(" parsed = (");
    out.push_str(c_value_type);
    out.push(')');
    out.push_str(c_parse_fn);
    if suffix == "f64" {
        out.push_str("(text.data, &end);\n");
    } else {
        out.push_str("(text.data, &end, 10);\n");
    }
    out.push_str("    if (text.data[0] == '\\0' || *end != '\\0' || errno == ERANGE) {\n");
    emit_num_parse_error(out, &result, &err, &num_error_struct, suffix);
    out.push_str("    }\n");
    out.push_str("    return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&ok);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Ok"));
    out.push_str(" = parsed};\n");
    out.push_str("}\n");
}

fn emit_num_parse_error(out: &mut String, result: &str, err: &str, num_error: &str, suffix: &str) {
    out.push_str("        return (");
    out.push_str(result);
    out.push_str("){.tag = ");
    out.push_str(err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(num_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_literal(\"invalid ");
    out.push_str(suffix);
    out.push_str("\")}};\n");
}
