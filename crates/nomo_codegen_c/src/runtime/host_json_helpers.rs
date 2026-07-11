use super::*;

pub(super) fn emit_json_helpers(out: &mut String) {
    let json_value = ValueType::Struct("JsonValue".to_string(), Vec::new());
    let json_error = ValueType::Struct("JsonError".to_string(), Vec::new());
    let result = c_enum_ident("Result", &[json_value.clone(), json_error.clone()]);
    let ok = c_enum_variant_ident("Result", &[json_value.clone(), json_error.clone()], "Ok");
    let err = c_enum_variant_ident("Result", &[json_value.clone(), json_error.clone()], "Err");
    let json_value_struct = c_struct_ident("JsonValue", &[]);
    let json_error_struct = c_struct_ident("JsonError", &[]);
    out.push_str(
        "static void nomo_json_skip_ws(const char *text, size_t *index) {\n\
    while (text[*index] == ' ' || text[*index] == '\\n' || text[*index] == '\\r' || text[*index] == '\\t') { *index += 1; }\n\
}\n\
\n\
static int nomo_json_parse_value(const char *text, size_t *index);\n\
\n\
static int nomo_json_parse_hex4(const char *text, size_t *index) {\n\
    for (int i = 0; i < 4; i += 1) {\n\
        unsigned char ch = (unsigned char)text[*index];\n\
        if (!((ch >= '0' && ch <= '9') || (ch >= 'a' && ch <= 'f') || (ch >= 'A' && ch <= 'F'))) { return 0; }\n\
        *index += 1;\n\
    }\n\
    return 1;\n\
}\n\
\n\
static int nomo_json_parse_string_token(const char *text, size_t *index) {\n\
    if (text[*index] != '\\\"') { return 0; }\n\
    *index += 1;\n\
    while (text[*index] != '\\0') {\n\
        unsigned char ch = (unsigned char)text[*index];\n\
        if (ch == '\\\"') { *index += 1; return 1; }\n\
        if (ch < 0x20) { return 0; }\n\
        if (ch == '\\\\') {\n\
            *index += 1;\n\
            char esc = text[*index];\n\
            if (esc == '\\\"' || esc == '\\\\' || esc == '/' || esc == 'b' || esc == 'f' || esc == 'n' || esc == 'r' || esc == 't') { *index += 1; continue; }\n\
            if (esc == 'u') { *index += 1; if (!nomo_json_parse_hex4(text, index)) { return 0; } continue; }\n\
            return 0;\n\
        }\n\
        *index += 1;\n\
    }\n\
    return 0;\n\
}\n\
\n\
static int nomo_json_parse_literal(const char *text, size_t *index, const char *literal) {\n\
    size_t len = strlen(literal);\n\
    if (strncmp(text + *index, literal, len) != 0) { return 0; }\n\
    *index += len;\n\
    return 1;\n\
}\n\
\n\
static int nomo_json_parse_number(const char *text, size_t *index) {\n\
    if (text[*index] == '-') { *index += 1; }\n\
    if (text[*index] == '0') {\n\
        *index += 1;\n\
    } else if (text[*index] >= '1' && text[*index] <= '9') {\n\
        while (text[*index] >= '0' && text[*index] <= '9') { *index += 1; }\n\
    } else {\n\
        return 0;\n\
    }\n\
    if (text[*index] == '.') {\n\
        *index += 1;\n\
        if (!(text[*index] >= '0' && text[*index] <= '9')) { return 0; }\n\
        while (text[*index] >= '0' && text[*index] <= '9') { *index += 1; }\n\
    }\n\
    if (text[*index] == 'e' || text[*index] == 'E') {\n\
        *index += 1;\n\
        if (text[*index] == '+' || text[*index] == '-') { *index += 1; }\n\
        if (!(text[*index] >= '0' && text[*index] <= '9')) { return 0; }\n\
        while (text[*index] >= '0' && text[*index] <= '9') { *index += 1; }\n\
    }\n\
    return 1;\n\
}\n\
\n\
static int nomo_json_parse_array(const char *text, size_t *index) {\n\
    if (text[*index] != '[') { return 0; }\n\
    *index += 1;\n\
    nomo_json_skip_ws(text, index);\n\
    if (text[*index] == ']') { *index += 1; return 1; }\n\
    while (1) {\n\
        if (!nomo_json_parse_value(text, index)) { return 0; }\n\
        nomo_json_skip_ws(text, index);\n\
        if (text[*index] == ']') { *index += 1; return 1; }\n\
        if (text[*index] != ',') { return 0; }\n\
        *index += 1;\n\
        nomo_json_skip_ws(text, index);\n\
    }\n\
}\n\
\n\
static int nomo_json_parse_object(const char *text, size_t *index) {\n\
    if (text[*index] != '{') { return 0; }\n\
    *index += 1;\n\
    nomo_json_skip_ws(text, index);\n\
    if (text[*index] == '}') { *index += 1; return 1; }\n\
    while (1) {\n\
        if (!nomo_json_parse_string_token(text, index)) { return 0; }\n\
        nomo_json_skip_ws(text, index);\n\
        if (text[*index] != ':') { return 0; }\n\
        *index += 1;\n\
        if (!nomo_json_parse_value(text, index)) { return 0; }\n\
        nomo_json_skip_ws(text, index);\n\
        if (text[*index] == '}') { *index += 1; return 1; }\n\
        if (text[*index] != ',') { return 0; }\n\
        *index += 1;\n\
        nomo_json_skip_ws(text, index);\n\
    }\n\
}\n\
\n\
static int nomo_json_parse_value(const char *text, size_t *index) {\n\
    nomo_json_skip_ws(text, index);\n\
    char ch = text[*index];\n\
    if (ch == '\\\"') { return nomo_json_parse_string_token(text, index); }\n\
    if (ch == '{') { return nomo_json_parse_object(text, index); }\n\
    if (ch == '[') { return nomo_json_parse_array(text, index); }\n\
    if (ch == '-' || (ch >= '0' && ch <= '9')) { return nomo_json_parse_number(text, index); }\n\
    if (ch == 't') { return nomo_json_parse_literal(text, index, \"true\"); }\n\
    if (ch == 'f') { return nomo_json_parse_literal(text, index, \"false\"); }\n\
    if (ch == 'n') { return nomo_json_parse_literal(text, index, \"null\"); }\n\
    return 0;\n\
}\n\
",
    );
    out.push_str("static ");
    out.push_str(&result);
    out.push_str(" nomo_json_parse(nomo_string text) {\n");
    out.push_str("    size_t index = 0;\n");
    out.push_str("    if (!nomo_json_parse_value(text.data, &index)) {\n");
    emit_json_parse_error(out, &result, &err, &json_error_struct);
    out.push_str("    }\n");
    out.push_str("    nomo_json_skip_ws(text.data, &index);\n");
    out.push_str("    if (text.data[index] != '\\0') {\n");
    emit_json_parse_error(out, &result, &err, &json_error_struct);
    out.push_str("    }\n");
    out.push_str("    return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&ok);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Ok"));
    out.push_str(" = (");
    out.push_str(&json_value_struct);
    out.push_str("){.");
    out.push_str(&c_member_ident("raw"));
    out.push_str(" = nomo_string_retain(text)}};\n");
    out.push_str("}\n\nstatic nomo_string nomo_json_stringify(");
    out.push_str(&json_value_struct);
    out.push_str(" value) {\n");
    out.push_str("    return nomo_string_retain(value.");
    out.push_str(&c_member_ident("raw"));
    out.push_str(");\n");
    out.push_str("}\n");
}

fn emit_json_parse_error(out: &mut String, result: &str, err: &str, json_error: &str) {
    out.push_str("        return (");
    out.push_str(result);
    out.push_str("){.tag = ");
    out.push_str(err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(json_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_literal(\"invalid json\")}};\n");
}
