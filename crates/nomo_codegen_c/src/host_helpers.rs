use super::*;

pub(super) fn emit_io_read_line_helper(out: &mut String) {
    let io_error = c_struct_ident("IoError", &[]);
    let result = c_enum_ident(
        "Result",
        &[
            ValueType::String,
            ValueType::Struct("IoError".to_string(), Vec::new()),
        ],
    );
    let ok = c_enum_variant_ident(
        "Result",
        &[
            ValueType::String,
            ValueType::Struct("IoError".to_string(), Vec::new()),
        ],
        "Ok",
    );
    let err = c_enum_variant_ident(
        "Result",
        &[
            ValueType::String,
            ValueType::Struct("IoError".to_string(), Vec::new()),
        ],
        "Err",
    );
    out.push_str("static ");
    out.push_str(&result);
    out.push_str(" nomo_io_read_line(void) {\n");
    out.push_str("    size_t capacity = 128;\n");
    out.push_str("    size_t len = 0;\n");
    out.push_str("    char *buffer = (char *)malloc(capacity);\n");
    out.push_str("    if (buffer == NULL) { nomo_panic(\"out of memory\"); }\n");
    out.push_str("    int ch = 0;\n");
    out.push_str("    while ((ch = fgetc(stdin)) != EOF) {\n");
    out.push_str("        if (ch == '\\n') { break; }\n");
    out.push_str("        if (len + 1 >= capacity) {\n");
    out.push_str("            capacity *= 2;\n");
    out.push_str("            char *next = (char *)realloc(buffer, capacity);\n");
    out.push_str("            if (next == NULL) {\n");
    out.push_str("                free(buffer);\n");
    out.push_str("                nomo_panic(\"out of memory\");\n");
    out.push_str("            }\n");
    out.push_str("            buffer = next;\n");
    out.push_str("        }\n");
    out.push_str("        buffer[len] = (char)ch;\n");
    out.push_str("        len += 1;\n");
    out.push_str("    }\n");
    out.push_str("    if (ch == EOF && len == 0) {\n");
    out.push_str(
        "        const char *message = ferror(stdin) ? strerror(errno) : \"end of input\";\n",
    );
    out.push_str("        free(buffer);\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&io_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(message)}};\n");
    out.push_str("    }\n");
    out.push_str("    if (len > 0 && buffer[len - 1] == '\\r') { len -= 1; }\n");
    out.push_str("    buffer[len] = '\\0';\n");
    out.push_str("    return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&ok);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Ok"));
    out.push_str(" = nomo_string_owned(buffer)};\n");
    out.push_str("}\n");
}

pub(super) fn emit_num_parse_i64_helper(out: &mut String) {
    emit_num_parse_helper(out, "i64", &ValueType::Int, "strtoll", "int64_t");
}

pub(super) fn emit_num_parse_u64_helper(out: &mut String) {
    emit_num_parse_helper(out, "u64", &ValueType::U64, "strtoull", "uint64_t");
}

pub(super) fn emit_num_parse_f64_helper(out: &mut String) {
    emit_num_parse_helper(out, "f64", &ValueType::Float, "strtod", "double");
}

pub(super) fn emit_num_parse_helper(
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

pub(super) fn emit_num_parse_error(
    out: &mut String,
    result: &str,
    err: &str,
    num_error: &str,
    suffix: &str,
) {
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

pub(super) fn emit_json_parse_error(out: &mut String, result: &str, err: &str, json_error: &str) {
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

pub(super) fn emit_regex_helpers(out: &mut String) {
    let regex_type = ValueType::Struct("Regex".to_string(), Vec::new());
    let regex_error = ValueType::Struct("RegexError".to_string(), Vec::new());
    let result = c_enum_ident("Result", &[regex_type.clone(), regex_error.clone()]);
    let ok = c_enum_variant_ident("Result", &[regex_type.clone(), regex_error.clone()], "Ok");
    let err = c_enum_variant_ident("Result", &[regex_type.clone(), regex_error.clone()], "Err");
    let regex_struct = c_struct_ident("Regex", &[]);
    let regex_error_struct = c_struct_ident("RegexError", &[]);
    let string_array_type = ValueType::Array(Box::new(ValueType::String));
    let option_array = c_enum_ident("Option", std::slice::from_ref(&string_array_type));
    let some_array =
        c_enum_variant_ident("Option", std::slice::from_ref(&string_array_type), "Some");
    let none_array =
        c_enum_variant_ident("Option", std::slice::from_ref(&string_array_type), "None");
    let string_array = c_array_ident(&ValueType::String);

    out.push_str("static ");
    out.push_str(&result);
    out.push_str(" nomo_regex_compile(nomo_string pattern) {\n");
    out.push_str("#if defined(_WIN32)\n");
    out.push_str("    (void)pattern;\n");
    out.push_str("#else\n");
    out.push_str("    regex_t compiled;\n");
    out.push_str("    int status = regcomp(&compiled, pattern.data, REG_EXTENDED);\n");
    out.push_str("    if (status != 0) {\n");
    out.push_str("        char message[256];\n");
    out.push_str("        regerror(status, &compiled, message, sizeof(message));\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&regex_error_struct);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(message)}};\n");
    out.push_str("    }\n");
    out.push_str("    regfree(&compiled);\n");
    out.push_str("#endif\n");
    out.push_str("    return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&ok);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Ok"));
    out.push_str(" = (");
    out.push_str(&regex_struct);
    out.push_str("){.");
    out.push_str(&c_member_ident("pattern"));
    out.push_str(" = nomo_string_retain(pattern)}};\n");
    out.push_str("}\n\nstatic int nomo_regex_is_match(");
    out.push_str(&regex_struct);
    out.push_str(" regex, nomo_string value) {\n");
    out.push_str("#if defined(_WIN32)\n");
    out.push_str("    return strstr(value.data, regex.");
    out.push_str(&c_member_ident("pattern"));
    out.push_str(".data) != NULL;\n");
    out.push_str("#else\n");
    out.push_str("    regex_t compiled;\n");
    out.push_str("    int status = regcomp(&compiled, regex.");
    out.push_str(&c_member_ident("pattern"));
    out.push_str(".data, REG_EXTENDED);\n");
    out.push_str("    if (status != 0) { return 0; }\n");
    out.push_str("    status = regexec(&compiled, value.data, 0, NULL, 0);\n");
    out.push_str("    regfree(&compiled);\n");
    out.push_str("    return status == 0;\n");
    out.push_str("#endif\n");
    out.push_str("}\n\nstatic ");
    out.push_str(&option_array);
    out.push_str(" nomo_regex_captures(");
    out.push_str(&regex_struct);
    out.push_str(" regex, nomo_string value) {\n");
    out.push_str("#if defined(_WIN32)\n");
    out.push_str("    char *found = strstr(value.data, regex.");
    out.push_str(&c_member_ident("pattern"));
    out.push_str(".data);\n");
    out.push_str("    if (found == NULL) { return (");
    out.push_str(&option_array);
    out.push_str("){.tag = ");
    out.push_str(&none_array);
    out.push_str("}; }\n");
    out.push_str("    ");
    out.push_str(&string_array);
    out.push_str(" captures = ");
    out.push_str(&string_array);
    out.push_str("_new();\n");
    out.push_str("    size_t start = (size_t)(found - value.data);\n");
    out.push_str(
        "    nomo_string capture = nomo_string_from_slice(value.data, start, strlen(regex.",
    );
    out.push_str(&c_member_ident("pattern"));
    out.push_str(".data));\n");
    out.push_str("    captures = ");
    out.push_str(&string_array);
    out.push_str("_push(captures, capture);\n");
    out.push_str("    nomo_string_release(capture);\n");
    out.push_str("#else\n");
    out.push_str("    regex_t compiled;\n");
    out.push_str("    int status = regcomp(&compiled, regex.");
    out.push_str(&c_member_ident("pattern"));
    out.push_str(".data, REG_EXTENDED);\n");
    out.push_str("    if (status != 0) { return (");
    out.push_str(&option_array);
    out.push_str("){.tag = ");
    out.push_str(&none_array);
    out.push_str("}; }\n");
    out.push_str("    size_t count = compiled.re_nsub + 1;\n");
    out.push_str("    regmatch_t *matches = (regmatch_t *)calloc(count, sizeof(regmatch_t));\n");
    out.push_str(
        "    if (matches == NULL) { regfree(&compiled); nomo_panic(\"out of memory\"); }\n",
    );
    out.push_str("    status = regexec(&compiled, value.data, count, matches, 0);\n");
    out.push_str("    if (status != 0) { free(matches); regfree(&compiled); return (");
    out.push_str(&option_array);
    out.push_str("){.tag = ");
    out.push_str(&none_array);
    out.push_str("}; }\n");
    out.push_str("    ");
    out.push_str(&string_array);
    out.push_str(" captures = ");
    out.push_str(&string_array);
    out.push_str("_new();\n");
    out.push_str("    for (size_t i = 0; i < count; i += 1) {\n");
    out.push_str("        nomo_string capture;\n");
    out.push_str("        if (matches[i].rm_so >= 0 && matches[i].rm_eo >= matches[i].rm_so) {\n");
    out.push_str("            capture = nomo_string_from_slice(value.data, (size_t)matches[i].rm_so, (size_t)(matches[i].rm_eo - matches[i].rm_so));\n");
    out.push_str("        } else {\n");
    out.push_str("            capture = nomo_string_literal(\"\");\n");
    out.push_str("        }\n");
    out.push_str("        captures = ");
    out.push_str(&string_array);
    out.push_str("_push(captures, capture);\n");
    out.push_str("        nomo_string_release(capture);\n");
    out.push_str("    }\n");
    out.push_str("    free(matches);\n");
    out.push_str("    regfree(&compiled);\n");
    out.push_str("#endif\n");
    out.push_str("    return (");
    out.push_str(&option_array);
    out.push_str("){.tag = ");
    out.push_str(&some_array);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Some"));
    out.push_str(" = captures};\n");
    out.push_str("}\n");
}

pub(super) fn emit_num_checked_binary_helper(
    out: &mut String,
    instance: &NumCheckedBinaryInstance,
) {
    let option = c_enum_ident("Option", std::slice::from_ref(&instance.value_type));
    let some = c_enum_variant_ident("Option", std::slice::from_ref(&instance.value_type), "Some");
    let none = c_enum_variant_ident("Option", std::slice::from_ref(&instance.value_type), "None");
    let c_type = c_type(&instance.value_type);
    let helper = num_checked_binary_helper_name(&instance.op, &instance.value_type);
    out.push_str("static ");
    out.push_str(&option);
    out.push(' ');
    out.push_str(helper);
    out.push('(');
    out.push_str(&c_type);
    out.push_str(" left, ");
    out.push_str(&c_type);
    out.push_str(" right) {\n");
    emit_num_checked_overflow_guard(out, instance, &option, &none);
    out.push_str("    return (");
    out.push_str(&option);
    out.push_str("){.tag = ");
    out.push_str(&some);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Some"));
    out.push_str(" = ");
    out.push_str(num_wrapping_binary_helper_name(
        &instance.op,
        &instance.value_type,
    ));
    out.push_str("(left, right)};\n");
    out.push_str("}\n");
}

pub(super) fn emit_num_checked_overflow_guard(
    out: &mut String,
    instance: &NumCheckedBinaryInstance,
    option: &str,
    none: &str,
) {
    let condition = num_checked_overflow_condition(&instance.op, &instance.value_type);
    out.push_str("    if (");
    out.push_str(condition);
    out.push_str(") { return (");
    out.push_str(option);
    out.push_str("){.tag = ");
    out.push_str(none);
    out.push_str("}; }\n");
}

pub(super) fn num_checked_overflow_condition(
    op: &BinaryOp,
    value_type: &ValueType,
) -> &'static str {
    match (op, value_type) {
        (BinaryOp::Add, ValueType::Int) => {
            "(right > 0 && left > LLONG_MAX - right) || (right < 0 && left < LLONG_MIN - right)"
        }
        (BinaryOp::Subtract, ValueType::Int) => {
            "(right < 0 && left > LLONG_MAX + right) || (right > 0 && left < LLONG_MIN + right)"
        }
        (BinaryOp::Multiply, ValueType::Int) => {
            "left != 0 && right != 0 && ((left == -1 && right == LLONG_MIN) || (right == -1 && left == LLONG_MIN) || (left > 0 ? (right > 0 ? left > LLONG_MAX / right : right < LLONG_MIN / left) : (right > 0 ? left < LLONG_MIN / right : left < LLONG_MAX / right)))"
        }
        (BinaryOp::Add, ValueType::I32) => {
            "(right > 0 && left > INT32_MAX - right) || (right < 0 && left < INT32_MIN - right)"
        }
        (BinaryOp::Subtract, ValueType::I32) => {
            "(right < 0 && left > INT32_MAX + right) || (right > 0 && left < INT32_MIN + right)"
        }
        (BinaryOp::Multiply, ValueType::I32) => {
            "left != 0 && right != 0 && ((left == -1 && right == INT32_MIN) || (right == -1 && left == INT32_MIN) || (left > 0 ? (right > 0 ? left > INT32_MAX / right : right < INT32_MIN / left) : (right > 0 ? left < INT32_MIN / right : left < INT32_MAX / right)))"
        }
        (BinaryOp::Add, ValueType::U32) => "left > UINT32_MAX - right",
        (BinaryOp::Subtract, ValueType::U32) => "left < right",
        (BinaryOp::Multiply, ValueType::U32) => "right != 0 && left > UINT32_MAX / right",
        (BinaryOp::Add, ValueType::U64) => "left > UINT64_MAX - right",
        (BinaryOp::Subtract, ValueType::U64) => "left < right",
        (BinaryOp::Multiply, ValueType::U64) => "right != 0 && left > UINT64_MAX / right",
        _ => unreachable!("num checked helpers only support integer add/sub/mul"),
    }
}

pub(super) fn emit_fs_read_to_string_helper(out: &mut String) {
    let fs_error = c_struct_ident("FsError", &[]);
    let result = c_enum_ident(
        "Result",
        &[
            ValueType::String,
            ValueType::Struct("FsError".to_string(), Vec::new()),
        ],
    );
    let ok = c_enum_variant_ident(
        "Result",
        &[
            ValueType::String,
            ValueType::Struct("FsError".to_string(), Vec::new()),
        ],
        "Ok",
    );
    let err = c_enum_variant_ident(
        "Result",
        &[
            ValueType::String,
            ValueType::Struct("FsError".to_string(), Vec::new()),
        ],
        "Err",
    );
    out.push_str("static ");
    out.push_str(&result);
    out.push_str(" nomo_fs_read_to_string(nomo_string path) {\n");
    out.push_str("    FILE *file = fopen(path.data, \"rb\");\n");
    out.push_str("    if (file == NULL) {\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&fs_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(strerror(errno))}};\n");
    out.push_str("    }\n");
    out.push_str("    if (fseek(file, 0, SEEK_END) != 0) {\n");
    out.push_str("        const char *message = strerror(errno);\n");
    out.push_str("        fclose(file);\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&fs_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(message)}};\n");
    out.push_str("    }\n");
    out.push_str("    long size = ftell(file);\n");
    out.push_str("    if (size < 0 || fseek(file, 0, SEEK_SET) != 0) {\n");
    out.push_str("        const char *message = strerror(errno);\n");
    out.push_str("        fclose(file);\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&fs_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(message)}};\n");
    out.push_str("    }\n");
    out.push_str("    char *buffer = (char *)malloc((size_t)size + 1);\n");
    out.push_str("    if (buffer == NULL) {\n");
    out.push_str("        fclose(file);\n");
    out.push_str("        nomo_panic(\"out of memory\");\n");
    out.push_str("    }\n");
    out.push_str("    size_t read = fread(buffer, 1, (size_t)size, file);\n");
    out.push_str("    if (read != (size_t)size) {\n");
    out.push_str(
        "        const char *message = ferror(file) ? strerror(errno) : \"short read\";\n",
    );
    out.push_str("        free(buffer);\n");
    out.push_str("        fclose(file);\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&fs_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(message)}};\n");
    out.push_str("    }\n");
    out.push_str("    buffer[size] = '\\0';\n");
    out.push_str("    fclose(file);\n");
    out.push_str("    return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&ok);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Ok"));
    out.push_str(" = nomo_string_owned(buffer)};\n");
    out.push_str("}\n");
}

pub(super) fn emit_fs_write_string_helper(out: &mut String) {
    let fs_error = c_struct_ident("FsError", &[]);
    let result = c_enum_ident(
        "Result",
        &[
            ValueType::Void,
            ValueType::Struct("FsError".to_string(), Vec::new()),
        ],
    );
    let ok = c_enum_variant_ident(
        "Result",
        &[
            ValueType::Void,
            ValueType::Struct("FsError".to_string(), Vec::new()),
        ],
        "Ok",
    );
    let err = c_enum_variant_ident(
        "Result",
        &[
            ValueType::Void,
            ValueType::Struct("FsError".to_string(), Vec::new()),
        ],
        "Err",
    );
    out.push_str("static ");
    out.push_str(&result);
    out.push_str(" nomo_fs_write_string(nomo_string path, nomo_string content) {\n");
    out.push_str("    FILE *file = fopen(path.data, \"wb\");\n");
    out.push_str("    if (file == NULL) {\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&fs_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(strerror(errno))}};\n");
    out.push_str("    }\n");
    out.push_str("    size_t len = strlen(content.data);\n");
    out.push_str("    if (fwrite(content.data, 1, len, file) != len) {\n");
    out.push_str("        const char *message = strerror(errno);\n");
    out.push_str("        fclose(file);\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&fs_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(message)}};\n");
    out.push_str("    }\n");
    out.push_str("    if (fclose(file) != 0) {\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&fs_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(strerror(errno))}};\n");
    out.push_str("    }\n");
    out.push_str("    return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&ok);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Ok"));
    out.push_str(" = 0};\n");
    out.push_str("}\n");
}

pub(super) fn emit_fs_read_bytes_helper(out: &mut String) {
    let fs_error = c_struct_ident("FsError", &[]);
    let array_u32 = ValueType::Array(Box::new(ValueType::U32));
    let result = c_enum_ident(
        "Result",
        &[
            array_u32.clone(),
            ValueType::Struct("FsError".to_string(), Vec::new()),
        ],
    );
    let ok = c_enum_variant_ident(
        "Result",
        &[
            array_u32.clone(),
            ValueType::Struct("FsError".to_string(), Vec::new()),
        ],
        "Ok",
    );
    let err = c_enum_variant_ident(
        "Result",
        &[
            array_u32,
            ValueType::Struct("FsError".to_string(), Vec::new()),
        ],
        "Err",
    );
    out.push_str("static ");
    out.push_str(&result);
    out.push_str(" nomo_fs_read_bytes(nomo_string path) {\n");
    out.push_str("    FILE *file = fopen(path.data, \"rb\");\n");
    out.push_str("    if (file == NULL) {\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&fs_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(strerror(errno))}};\n");
    out.push_str("    }\n");
    out.push_str("    nomo_array_u32 bytes = nomo_array_u32_new();\n");
    out.push_str("    int ch = 0;\n");
    out.push_str("    while ((ch = fgetc(file)) != EOF) {\n");
    out.push_str("        bytes = nomo_array_u32_push(bytes, (uint32_t)(unsigned char)ch);\n");
    out.push_str("    }\n");
    out.push_str("    if (ferror(file)) {\n");
    out.push_str("        const char *message = strerror(errno);\n");
    out.push_str("        fclose(file);\n");
    out.push_str("        nomo_array_u32_release(bytes);\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&fs_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(message)}};\n");
    out.push_str("    }\n");
    out.push_str("    if (fclose(file) != 0) {\n");
    out.push_str("        const char *message = strerror(errno);\n");
    out.push_str("        nomo_array_u32_release(bytes);\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&fs_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(message)}};\n");
    out.push_str("    }\n");
    out.push_str("    return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&ok);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Ok"));
    out.push_str(" = bytes};\n");
    out.push_str("}\n");
}

pub(super) fn emit_fs_write_bytes_helper(out: &mut String) {
    let fs_error = c_struct_ident("FsError", &[]);
    let result = c_enum_ident(
        "Result",
        &[
            ValueType::Void,
            ValueType::Struct("FsError".to_string(), Vec::new()),
        ],
    );
    let ok = c_enum_variant_ident(
        "Result",
        &[
            ValueType::Void,
            ValueType::Struct("FsError".to_string(), Vec::new()),
        ],
        "Ok",
    );
    let err = c_enum_variant_ident(
        "Result",
        &[
            ValueType::Void,
            ValueType::Struct("FsError".to_string(), Vec::new()),
        ],
        "Err",
    );
    out.push_str("static ");
    out.push_str(&result);
    out.push_str(" nomo_fs_write_bytes(nomo_string path, nomo_array_u32 bytes) {\n");
    out.push_str("    FILE *file = fopen(path.data, \"wb\");\n");
    out.push_str("    if (file == NULL) {\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&fs_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(strerror(errno))}};\n");
    out.push_str("    }\n");
    out.push_str("    for (size_t i = 0; i < bytes.len; i += 1) {\n");
    out.push_str("        unsigned char value = (unsigned char)(bytes.data[i] & 0xffU);\n");
    out.push_str("        if (fwrite(&value, 1, 1, file) != 1) {\n");
    out.push_str("            const char *message = strerror(errno);\n");
    out.push_str("            fclose(file);\n");
    out.push_str("            return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&fs_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(message)}};\n");
    out.push_str("        }\n");
    out.push_str("    }\n");
    out.push_str("    if (fclose(file) != 0) {\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&fs_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(strerror(errno))}};\n");
    out.push_str("    }\n");
    out.push_str("    return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&ok);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Ok"));
    out.push_str(" = 0};\n");
    out.push_str("}\n");
}

pub(super) fn emit_fs_exists_helper(out: &mut String) {
    out.push_str("static int nomo_fs_exists(nomo_string path) {\n");
    out.push_str("#ifdef _WIN32\n");
    out.push_str("    DWORD attrs = GetFileAttributesA(path.data);\n");
    out.push_str("    return attrs != INVALID_FILE_ATTRIBUTES;\n");
    out.push_str("#else\n");
    out.push_str("    struct stat info;\n");
    out.push_str("    return stat(path.data, &info) == 0;\n");
    out.push_str("#endif\n");
    out.push_str("}\n");
}

pub(super) fn emit_fs_metadata_helper(out: &mut String) {
    let fs_error = c_struct_ident("FsError", &[]);
    let metadata = c_struct_ident("FileMetadata", &[]);
    let result = c_enum_ident(
        "Result",
        &[
            ValueType::Struct("FileMetadata".to_string(), Vec::new()),
            ValueType::Struct("FsError".to_string(), Vec::new()),
        ],
    );
    let ok = c_enum_variant_ident(
        "Result",
        &[
            ValueType::Struct("FileMetadata".to_string(), Vec::new()),
            ValueType::Struct("FsError".to_string(), Vec::new()),
        ],
        "Ok",
    );
    let err = c_enum_variant_ident(
        "Result",
        &[
            ValueType::Struct("FileMetadata".to_string(), Vec::new()),
            ValueType::Struct("FsError".to_string(), Vec::new()),
        ],
        "Err",
    );
    out.push_str("static ");
    out.push_str(&result);
    out.push_str(" nomo_fs_metadata(nomo_string path) {\n");
    out.push_str("#ifdef _WIN32\n");
    out.push_str("    WIN32_FILE_ATTRIBUTE_DATA data;\n");
    out.push_str("    if (!GetFileAttributesExA(path.data, GetFileExInfoStandard, &data)) {\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&fs_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(\"failed to read metadata\")}};\n");
    out.push_str("    }\n");
    out.push_str("    int is_dir = (data.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY) != 0;\n");
    out.push_str("    ");
    out.push_str(&metadata);
    out.push_str(" metadata = {.");
    out.push_str(&c_member_ident("is_file"));
    out.push_str(" = !is_dir, .");
    out.push_str(&c_member_ident("is_dir"));
    out.push_str(" = is_dir, .");
    out.push_str(&c_member_ident("size"));
    out.push_str(" = ((uint64_t)data.nFileSizeHigh << 32) | (uint64_t)data.nFileSizeLow};\n");
    out.push_str("#else\n");
    out.push_str("    struct stat info;\n");
    out.push_str("    if (stat(path.data, &info) != 0) {\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&fs_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(strerror(errno))}};\n");
    out.push_str("    }\n");
    out.push_str("    ");
    out.push_str(&metadata);
    out.push_str(" metadata = {.");
    out.push_str(&c_member_ident("is_file"));
    out.push_str(" = S_ISREG(info.st_mode), .");
    out.push_str(&c_member_ident("is_dir"));
    out.push_str(" = S_ISDIR(info.st_mode), .");
    out.push_str(&c_member_ident("size"));
    out.push_str(" = (uint64_t)info.st_size};\n");
    out.push_str("#endif\n");
    out.push_str("    return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&ok);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Ok"));
    out.push_str(" = metadata};\n");
    out.push_str("}\n");
}

pub(super) fn emit_fs_create_dir_helper(out: &mut String) {
    emit_fs_dir_result_helper(out, "create_dir", "mkdir");
}

pub(super) fn emit_fs_remove_dir_helper(out: &mut String) {
    emit_fs_dir_result_helper(out, "remove_dir", "rmdir");
}

pub(super) fn emit_fs_dir_result_helper(out: &mut String, function: &str, c_call: &str) {
    let fs_error = c_struct_ident("FsError", &[]);
    let result = c_enum_ident(
        "Result",
        &[
            ValueType::Void,
            ValueType::Struct("FsError".to_string(), Vec::new()),
        ],
    );
    let ok = c_enum_variant_ident(
        "Result",
        &[
            ValueType::Void,
            ValueType::Struct("FsError".to_string(), Vec::new()),
        ],
        "Ok",
    );
    let err = c_enum_variant_ident(
        "Result",
        &[
            ValueType::Void,
            ValueType::Struct("FsError".to_string(), Vec::new()),
        ],
        "Err",
    );
    out.push_str("static ");
    out.push_str(&result);
    out.push_str(" nomo_fs_");
    out.push_str(function);
    out.push_str("(nomo_string path) {\n");
    out.push_str("#ifdef _WIN32\n");
    out.push_str("    int status = ");
    if c_call == "mkdir" {
        out.push_str("_mkdir(path.data);\n");
    } else {
        out.push_str("_rmdir(path.data);\n");
    }
    out.push_str("#else\n");
    out.push_str("    int status = ");
    if c_call == "mkdir" {
        out.push_str("mkdir(path.data, 0777);\n");
    } else {
        out.push_str("rmdir(path.data);\n");
    }
    out.push_str("#endif\n");
    out.push_str("    if (status != 0) {\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&fs_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(strerror(errno))}};\n");
    out.push_str("    }\n");
    out.push_str("    return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&ok);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Ok"));
    out.push_str(" = 0};\n");
    out.push_str("}\n");
}

pub(super) fn emit_fs_read_dir_helper(out: &mut String) {
    let fs_error = c_struct_ident("FsError", &[]);
    let entries_type = ValueType::Array(Box::new(ValueType::String));
    let result = c_enum_ident(
        "Result",
        &[
            entries_type.clone(),
            ValueType::Struct("FsError".to_string(), Vec::new()),
        ],
    );
    let ok = c_enum_variant_ident(
        "Result",
        &[
            entries_type.clone(),
            ValueType::Struct("FsError".to_string(), Vec::new()),
        ],
        "Ok",
    );
    let err = c_enum_variant_ident(
        "Result",
        &[
            entries_type,
            ValueType::Struct("FsError".to_string(), Vec::new()),
        ],
        "Err",
    );
    out.push_str("static ");
    out.push_str(&result);
    out.push_str(" nomo_fs_read_dir(nomo_string path) {\n");
    out.push_str("    nomo_array_string entries = nomo_array_string_new();\n");
    out.push_str("#ifdef _WIN32\n");
    out.push_str("    size_t path_len = strlen(path.data);\n");
    out.push_str("    int needs_sep = path_len > 0 && path.data[path_len - 1] != '/' && path.data[path_len - 1] != '\\\\';\n");
    out.push_str("    char *pattern = (char *)malloc(path_len + (needs_sep ? 3 : 2));\n");
    out.push_str("    if (pattern == NULL) { nomo_panic(\"out of memory\"); }\n");
    out.push_str("    memcpy(pattern, path.data, path_len);\n");
    out.push_str("    if (needs_sep) {\n");
    out.push_str("        pattern[path_len] = '\\\\';\n");
    out.push_str("        pattern[path_len + 1] = '*';\n");
    out.push_str("        pattern[path_len + 2] = '\\0';\n");
    out.push_str("    } else {\n");
    out.push_str("        pattern[path_len] = '*';\n");
    out.push_str("        pattern[path_len + 1] = '\\0';\n");
    out.push_str("    }\n");
    out.push_str("    WIN32_FIND_DATAA data;\n");
    out.push_str("    HANDLE handle = FindFirstFileA(pattern, &data);\n");
    out.push_str("    free(pattern);\n");
    out.push_str("    if (handle == INVALID_HANDLE_VALUE) {\n");
    out.push_str("        nomo_array_string_release(entries);\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&fs_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(\"failed to read directory\")}};\n");
    out.push_str("    }\n");
    out.push_str("    do {\n");
    out.push_str("        if (strcmp(data.cFileName, \".\") != 0 && strcmp(data.cFileName, \"..\") != 0) {\n");
    out.push_str("            nomo_string entry = nomo_string_from_cstr(data.cFileName);\n");
    out.push_str("            entries = nomo_array_string_push(entries, entry);\n");
    out.push_str("            nomo_string_release(entry);\n");
    out.push_str("        }\n");
    out.push_str("    } while (FindNextFileA(handle, &data));\n");
    out.push_str("    FindClose(handle);\n");
    out.push_str("#else\n");
    out.push_str("    DIR *dir = opendir(path.data);\n");
    out.push_str("    if (dir == NULL) {\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&fs_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(strerror(errno))}};\n");
    out.push_str("    }\n");
    out.push_str("    errno = 0;\n");
    out.push_str("    struct dirent *entry;\n");
    out.push_str("    while ((entry = readdir(dir)) != NULL) {\n");
    out.push_str("        if (strcmp(entry->d_name, \".\") == 0 || strcmp(entry->d_name, \"..\") == 0) { continue; }\n");
    out.push_str("        nomo_string name = nomo_string_from_cstr(entry->d_name);\n");
    out.push_str("        entries = nomo_array_string_push(entries, name);\n");
    out.push_str("        nomo_string_release(name);\n");
    out.push_str("    }\n");
    out.push_str("    if (errno != 0) {\n");
    out.push_str("        const char *message = strerror(errno);\n");
    out.push_str("        closedir(dir);\n");
    out.push_str("        nomo_array_string_release(entries);\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&fs_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(message)}};\n");
    out.push_str("    }\n");
    out.push_str("    if (closedir(dir) != 0) {\n");
    out.push_str("        nomo_array_string_release(entries);\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&fs_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(strerror(errno))}};\n");
    out.push_str("    }\n");
    out.push_str("#endif\n");
    out.push_str("    return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&ok);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Ok"));
    out.push_str(" = entries};\n");
    out.push_str("}\n");
}

pub(super) fn emit_fs_open_helper(out: &mut String) {
    let file_type = c_struct_ident("File", &[]);
    let fs_error = c_struct_ident("FsError", &[]);
    let result = c_enum_ident(
        "Result",
        &[
            ValueType::Struct("File".to_string(), Vec::new()),
            ValueType::Struct("FsError".to_string(), Vec::new()),
        ],
    );
    let ok = c_enum_variant_ident(
        "Result",
        &[
            ValueType::Struct("File".to_string(), Vec::new()),
            ValueType::Struct("FsError".to_string(), Vec::new()),
        ],
        "Ok",
    );
    let err = c_enum_variant_ident(
        "Result",
        &[
            ValueType::Struct("File".to_string(), Vec::new()),
            ValueType::Struct("FsError".to_string(), Vec::new()),
        ],
        "Err",
    );
    out.push_str("static ");
    out.push_str(&result);
    out.push_str(" nomo_fs_open(nomo_string path) {\n");
    out.push_str("    FILE *file = fopen(path.data, \"rb+\");\n");
    out.push_str("    if (file == NULL) {\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&fs_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(strerror(errno))}};\n");
    out.push_str("    }\n");
    out.push_str("    return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&ok);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Ok"));
    out.push_str(" = (");
    out.push_str(&file_type);
    out.push_str("){.");
    out.push_str(&c_member_ident("handle"));
    out.push_str(" = file}};\n");
    out.push_str("}\n");
}

pub(super) fn emit_file_read_to_string_helper(out: &mut String) {
    let file_type = c_struct_ident("File", &[]);
    let fs_error = c_struct_ident("FsError", &[]);
    let result = c_enum_ident(
        "Result",
        &[
            ValueType::String,
            ValueType::Struct("FsError".to_string(), Vec::new()),
        ],
    );
    let ok = c_enum_variant_ident(
        "Result",
        &[
            ValueType::String,
            ValueType::Struct("FsError".to_string(), Vec::new()),
        ],
        "Ok",
    );
    let err = c_enum_variant_ident(
        "Result",
        &[
            ValueType::String,
            ValueType::Struct("FsError".to_string(), Vec::new()),
        ],
        "Err",
    );
    out.push_str("static ");
    out.push_str(&result);
    out.push_str(" nomo_file_read_to_string(");
    out.push_str(&file_type);
    out.push_str(" file) {\n");
    out.push_str("    if (file.");
    out.push_str(&c_member_ident("handle"));
    out.push_str(" == NULL) {\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&fs_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(\"file is closed\")}};\n");
    out.push_str("    }\n");
    out.push_str("    FILE *handle = file.");
    out.push_str(&c_member_ident("handle"));
    out.push_str(";\n");
    out.push_str("    if (fseek(handle, 0, SEEK_END) != 0) {\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&fs_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(strerror(errno))}};\n");
    out.push_str("    }\n");
    out.push_str("    long size = ftell(handle);\n");
    out.push_str("    if (size < 0 || fseek(handle, 0, SEEK_SET) != 0) {\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&fs_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(strerror(errno))}};\n");
    out.push_str("    }\n");
    out.push_str("    char *buffer = (char *)malloc((size_t)size + 1);\n");
    out.push_str("    if (buffer == NULL) { nomo_panic(\"out of memory\"); }\n");
    out.push_str("    size_t read = fread(buffer, 1, (size_t)size, handle);\n");
    out.push_str("    if (read != (size_t)size) {\n");
    out.push_str(
        "        const char *message = ferror(handle) ? strerror(errno) : \"short read\";\n",
    );
    out.push_str("        free(buffer);\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&fs_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(message)}};\n");
    out.push_str("    }\n");
    out.push_str("    buffer[size] = '\\0';\n");
    out.push_str("    return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&ok);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Ok"));
    out.push_str(" = nomo_string_owned(buffer)};\n");
    out.push_str("}\n");
}

pub(super) fn emit_file_write_string_helper(out: &mut String) {
    let file_type = c_struct_ident("File", &[]);
    let fs_error = c_struct_ident("FsError", &[]);
    let result = c_enum_ident(
        "Result",
        &[
            ValueType::Void,
            ValueType::Struct("FsError".to_string(), Vec::new()),
        ],
    );
    let ok = c_enum_variant_ident(
        "Result",
        &[
            ValueType::Void,
            ValueType::Struct("FsError".to_string(), Vec::new()),
        ],
        "Ok",
    );
    let err = c_enum_variant_ident(
        "Result",
        &[
            ValueType::Void,
            ValueType::Struct("FsError".to_string(), Vec::new()),
        ],
        "Err",
    );
    out.push_str("static ");
    out.push_str(&result);
    out.push_str(" nomo_file_write_string(");
    out.push_str(&file_type);
    out.push_str(" file, nomo_string content) {\n");
    out.push_str("    if (file.");
    out.push_str(&c_member_ident("handle"));
    out.push_str(" == NULL) {\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&fs_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(\"file is closed\")}};\n");
    out.push_str("    }\n");
    out.push_str("    FILE *handle = file.");
    out.push_str(&c_member_ident("handle"));
    out.push_str(";\n");
    out.push_str("    if (fseek(handle, 0, SEEK_SET) != 0) {\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&fs_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(strerror(errno))}};\n");
    out.push_str("    }\n");
    out.push_str("    size_t len = strlen(content.data);\n");
    out.push_str("    if (fwrite(content.data, 1, len, handle) != len) {\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&fs_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(strerror(errno))}};\n");
    out.push_str("    }\n");
    out.push_str("    if (fflush(handle) != 0) {\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&fs_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(strerror(errno))}};\n");
    out.push_str("    }\n");
    out.push_str("    return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&ok);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Ok"));
    out.push_str(" = 0};\n");
    out.push_str("}\n");
}

pub(super) fn emit_file_close_helper(out: &mut String) {
    let file_type = c_struct_ident("File", &[]);
    out.push_str("static void nomo_file_close(");
    out.push_str(&file_type);
    out.push_str(" file) {\n");
    out.push_str("    if (file.");
    out.push_str(&c_member_ident("handle"));
    out.push_str(" != NULL) {\n");
    out.push_str("        fclose(file.");
    out.push_str(&c_member_ident("handle"));
    out.push_str(");\n");
    out.push_str("    }\n");
    out.push_str("}\n");
}

pub(super) fn emit_net_common_helpers(out: &mut String) {
    out.push_str("static nomo_string nomo_net_error_message(void) {\n");
    out.push_str("#ifdef _WIN32\n");
    out.push_str("    char buffer[64];\n");
    out.push_str(
        "    snprintf(buffer, sizeof(buffer), \"network error %d\", WSAGetLastError());\n",
    );
    out.push_str("    return nomo_string_from_cstr(buffer);\n");
    out.push_str("#else\n");
    out.push_str("    return nomo_string_from_cstr(strerror(errno));\n");
    out.push_str("#endif\n");
    out.push_str("}\n");
    out.push_str("\nstatic int nomo_net_init(void) {\n");
    out.push_str("#ifdef _WIN32\n");
    out.push_str("    static int initialized = 0;\n");
    out.push_str("    if (!initialized) {\n");
    out.push_str("        WSADATA data;\n");
    out.push_str("        if (WSAStartup(MAKEWORD(2, 2), &data) != 0) { return 0; }\n");
    out.push_str("        initialized = 1;\n");
    out.push_str("    }\n");
    out.push_str("#endif\n");
    out.push_str("    return 1;\n");
    out.push_str("}\n");
}

pub(super) fn emit_net_connect_helper(out: &mut String) {
    let tcp_stream = c_struct_ident("TcpStream", &[]);
    let net_error = c_struct_ident("NetError", &[]);
    let result = c_enum_ident(
        "Result",
        &[
            ValueType::Struct("TcpStream".to_string(), Vec::new()),
            ValueType::Struct("NetError".to_string(), Vec::new()),
        ],
    );
    let ok = c_enum_variant_ident(
        "Result",
        &[
            ValueType::Struct("TcpStream".to_string(), Vec::new()),
            ValueType::Struct("NetError".to_string(), Vec::new()),
        ],
        "Ok",
    );
    let err = c_enum_variant_ident(
        "Result",
        &[
            ValueType::Struct("TcpStream".to_string(), Vec::new()),
            ValueType::Struct("NetError".to_string(), Vec::new()),
        ],
        "Err",
    );
    out.push_str("static ");
    out.push_str(&result);
    out.push_str(" nomo_net_connect(nomo_string host, int64_t port) {\n");
    out.push_str("    if (!nomo_net_init()) {\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&net_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(\"network initialization failed\")}};\n");
    out.push_str("    }\n");
    out.push_str("    if (port < 0 || port > 65535) {\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&net_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(\"invalid port\")}};\n");
    out.push_str("    }\n");
    out.push_str("    char port_text[16];\n");
    out.push_str("    snprintf(port_text, sizeof(port_text), \"%\" PRId64, port);\n");
    out.push_str("    struct addrinfo hints;\n");
    out.push_str("    memset(&hints, 0, sizeof(hints));\n");
    out.push_str("    hints.ai_family = AF_UNSPEC;\n");
    out.push_str("    hints.ai_socktype = SOCK_STREAM;\n");
    out.push_str("    struct addrinfo *addresses = NULL;\n");
    out.push_str("    int rc = getaddrinfo(host.data, port_text, &hints, &addresses);\n");
    out.push_str("    if (rc != 0) {\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&net_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(gai_strerror(rc))}};\n");
    out.push_str("    }\n");
    out.push_str("    nomo_socket handle = NOMO_INVALID_SOCKET;\n");
    out.push_str("    for (struct addrinfo *address = addresses; address != NULL; address = address->ai_next) {\n");
    out.push_str("        handle = socket(address->ai_family, address->ai_socktype, address->ai_protocol);\n");
    out.push_str("        if (handle == NOMO_INVALID_SOCKET) { continue; }\n");
    out.push_str(
        "        if (connect(handle, address->ai_addr, address->ai_addrlen) == 0) { break; }\n",
    );
    out.push_str("        NOMO_SOCKET_CLOSE(handle);\n");
    out.push_str("        handle = NOMO_INVALID_SOCKET;\n");
    out.push_str("    }\n");
    out.push_str("    freeaddrinfo(addresses);\n");
    out.push_str("    if (handle == NOMO_INVALID_SOCKET) {\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&net_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_net_error_message()}};\n");
    out.push_str("    }\n");
    out.push_str("    return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&ok);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Ok"));
    out.push_str(" = (");
    out.push_str(&tcp_stream);
    out.push_str("){.");
    out.push_str(&c_member_ident("handle"));
    out.push_str(" = handle}};\n");
    out.push_str("}\n");
}

pub(super) fn emit_net_listen_helper(out: &mut String) {
    let tcp_listener = c_struct_ident("TcpListener", &[]);
    let net_error = c_struct_ident("NetError", &[]);
    let result = c_enum_ident(
        "Result",
        &[
            ValueType::Struct("TcpListener".to_string(), Vec::new()),
            ValueType::Struct("NetError".to_string(), Vec::new()),
        ],
    );
    let ok = c_enum_variant_ident(
        "Result",
        &[
            ValueType::Struct("TcpListener".to_string(), Vec::new()),
            ValueType::Struct("NetError".to_string(), Vec::new()),
        ],
        "Ok",
    );
    let err = c_enum_variant_ident(
        "Result",
        &[
            ValueType::Struct("TcpListener".to_string(), Vec::new()),
            ValueType::Struct("NetError".to_string(), Vec::new()),
        ],
        "Err",
    );
    out.push_str("static ");
    out.push_str(&result);
    out.push_str(" nomo_net_listen(nomo_string host, int64_t port) {\n");
    out.push_str("    if (!nomo_net_init()) {\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&net_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(\"network initialization failed\")}};\n");
    out.push_str("    }\n");
    out.push_str("    if (port < 0 || port > 65535) {\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&net_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(\"invalid port\")}};\n");
    out.push_str("    }\n");
    out.push_str("    char port_text[16];\n");
    out.push_str("    snprintf(port_text, sizeof(port_text), \"%\" PRId64, port);\n");
    out.push_str("    struct addrinfo hints;\n");
    out.push_str("    memset(&hints, 0, sizeof(hints));\n");
    out.push_str("    hints.ai_family = AF_UNSPEC;\n");
    out.push_str("    hints.ai_socktype = SOCK_STREAM;\n");
    out.push_str("    hints.ai_flags = AI_PASSIVE;\n");
    out.push_str("    struct addrinfo *addresses = NULL;\n");
    out.push_str("    int rc = getaddrinfo(host.data, port_text, &hints, &addresses);\n");
    out.push_str("    if (rc != 0) {\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&net_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(gai_strerror(rc))}};\n");
    out.push_str("    }\n");
    out.push_str("    nomo_socket handle = NOMO_INVALID_SOCKET;\n");
    out.push_str("    for (struct addrinfo *address = addresses; address != NULL; address = address->ai_next) {\n");
    out.push_str("        handle = socket(address->ai_family, address->ai_socktype, address->ai_protocol);\n");
    out.push_str("        if (handle == NOMO_INVALID_SOCKET) { continue; }\n");
    out.push_str("        int yes = 1;\n");
    out.push_str("#ifdef _WIN32\n");
    out.push_str(
        "        setsockopt(handle, SOL_SOCKET, SO_REUSEADDR, (const char *)&yes, sizeof(yes));\n",
    );
    out.push_str("#else\n");
    out.push_str("        setsockopt(handle, SOL_SOCKET, SO_REUSEADDR, &yes, sizeof(yes));\n");
    out.push_str("#endif\n");
    out.push_str("        if (bind(handle, address->ai_addr, address->ai_addrlen) == 0 && listen(handle, 128) == 0) { break; }\n");
    out.push_str("        NOMO_SOCKET_CLOSE(handle);\n");
    out.push_str("        handle = NOMO_INVALID_SOCKET;\n");
    out.push_str("    }\n");
    out.push_str("    freeaddrinfo(addresses);\n");
    out.push_str("    if (handle == NOMO_INVALID_SOCKET) {\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&net_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_net_error_message()}};\n");
    out.push_str("    }\n");
    out.push_str("    return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&ok);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Ok"));
    out.push_str(" = (");
    out.push_str(&tcp_listener);
    out.push_str("){.");
    out.push_str(&c_member_ident("handle"));
    out.push_str(" = handle}};\n");
    out.push_str("}\n");
}

pub(super) fn emit_tcp_listener_accept_helper(out: &mut String) {
    let tcp_listener = c_struct_ident("TcpListener", &[]);
    let tcp_stream = c_struct_ident("TcpStream", &[]);
    let net_error = c_struct_ident("NetError", &[]);
    let result = c_enum_ident(
        "Result",
        &[
            ValueType::Struct("TcpStream".to_string(), Vec::new()),
            ValueType::Struct("NetError".to_string(), Vec::new()),
        ],
    );
    let ok = c_enum_variant_ident(
        "Result",
        &[
            ValueType::Struct("TcpStream".to_string(), Vec::new()),
            ValueType::Struct("NetError".to_string(), Vec::new()),
        ],
        "Ok",
    );
    let err = c_enum_variant_ident(
        "Result",
        &[
            ValueType::Struct("TcpStream".to_string(), Vec::new()),
            ValueType::Struct("NetError".to_string(), Vec::new()),
        ],
        "Err",
    );
    out.push_str("static ");
    out.push_str(&result);
    out.push_str(" nomo_tcp_listener_accept(");
    out.push_str(&tcp_listener);
    out.push_str(" listener) {\n");
    out.push_str("    if (listener.");
    out.push_str(&c_member_ident("handle"));
    out.push_str(" == NOMO_INVALID_SOCKET) {\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&net_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(\"listener is closed\")}};\n");
    out.push_str("    }\n");
    out.push_str("    nomo_socket handle = accept(listener.");
    out.push_str(&c_member_ident("handle"));
    out.push_str(", NULL, NULL);\n");
    out.push_str("    if (handle == NOMO_INVALID_SOCKET) {\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&net_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_net_error_message()}};\n");
    out.push_str("    }\n");
    out.push_str("    return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&ok);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Ok"));
    out.push_str(" = (");
    out.push_str(&tcp_stream);
    out.push_str("){.");
    out.push_str(&c_member_ident("handle"));
    out.push_str(" = handle}};\n");
    out.push_str("}\n");
}

pub(super) fn emit_tcp_listener_close_helper(out: &mut String) {
    let tcp_listener = c_struct_ident("TcpListener", &[]);
    out.push_str("static void nomo_tcp_listener_close(");
    out.push_str(&tcp_listener);
    out.push_str(" listener) {\n");
    out.push_str("    if (listener.");
    out.push_str(&c_member_ident("handle"));
    out.push_str(" != NOMO_INVALID_SOCKET) {\n");
    out.push_str("        NOMO_SOCKET_CLOSE(listener.");
    out.push_str(&c_member_ident("handle"));
    out.push_str(");\n");
    out.push_str("    }\n");
    out.push_str("}\n");
}

pub(super) fn emit_net_udp_bind_helper(out: &mut String) {
    let udp_socket = c_struct_ident("UdpSocket", &[]);
    let net_error = c_struct_ident("NetError", &[]);
    let result = c_enum_ident(
        "Result",
        &[
            ValueType::Struct("UdpSocket".to_string(), Vec::new()),
            ValueType::Struct("NetError".to_string(), Vec::new()),
        ],
    );
    let ok = c_enum_variant_ident(
        "Result",
        &[
            ValueType::Struct("UdpSocket".to_string(), Vec::new()),
            ValueType::Struct("NetError".to_string(), Vec::new()),
        ],
        "Ok",
    );
    let err = c_enum_variant_ident(
        "Result",
        &[
            ValueType::Struct("UdpSocket".to_string(), Vec::new()),
            ValueType::Struct("NetError".to_string(), Vec::new()),
        ],
        "Err",
    );
    out.push_str("static ");
    out.push_str(&result);
    out.push_str(" nomo_net_udp_bind(nomo_string host, int64_t port) {\n");
    out.push_str("    if (!nomo_net_init()) {\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&net_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(\"network initialization failed\")}};\n");
    out.push_str("    }\n");
    out.push_str("    if (port < 0 || port > 65535) {\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&net_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(\"invalid port\")}};\n");
    out.push_str("    }\n");
    out.push_str("    char port_text[16];\n");
    out.push_str("    snprintf(port_text, sizeof(port_text), \"%\" PRId64, port);\n");
    out.push_str("    struct addrinfo hints;\n");
    out.push_str("    memset(&hints, 0, sizeof(hints));\n");
    out.push_str("    hints.ai_family = AF_UNSPEC;\n");
    out.push_str("    hints.ai_socktype = SOCK_DGRAM;\n");
    out.push_str("    hints.ai_flags = AI_PASSIVE;\n");
    out.push_str("    struct addrinfo *addresses = NULL;\n");
    out.push_str("    int rc = getaddrinfo(host.data, port_text, &hints, &addresses);\n");
    out.push_str("    if (rc != 0) {\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&net_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(gai_strerror(rc))}};\n");
    out.push_str("    }\n");
    out.push_str("    nomo_socket handle = NOMO_INVALID_SOCKET;\n");
    out.push_str("    for (struct addrinfo *address = addresses; address != NULL; address = address->ai_next) {\n");
    out.push_str("        handle = socket(address->ai_family, address->ai_socktype, address->ai_protocol);\n");
    out.push_str("        if (handle == NOMO_INVALID_SOCKET) { continue; }\n");
    out.push_str("        int yes = 1;\n");
    out.push_str("#ifdef _WIN32\n");
    out.push_str(
        "        setsockopt(handle, SOL_SOCKET, SO_REUSEADDR, (const char *)&yes, sizeof(yes));\n",
    );
    out.push_str("#else\n");
    out.push_str("        setsockopt(handle, SOL_SOCKET, SO_REUSEADDR, &yes, sizeof(yes));\n");
    out.push_str("#endif\n");
    out.push_str(
        "        if (bind(handle, address->ai_addr, address->ai_addrlen) == 0) { break; }\n",
    );
    out.push_str("        NOMO_SOCKET_CLOSE(handle);\n");
    out.push_str("        handle = NOMO_INVALID_SOCKET;\n");
    out.push_str("    }\n");
    out.push_str("    freeaddrinfo(addresses);\n");
    out.push_str("    if (handle == NOMO_INVALID_SOCKET) {\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&net_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_net_error_message()}};\n");
    out.push_str("    }\n");
    out.push_str("    return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&ok);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Ok"));
    out.push_str(" = (");
    out.push_str(&udp_socket);
    out.push_str("){.");
    out.push_str(&c_member_ident("handle"));
    out.push_str(" = handle}};\n");
    out.push_str("}\n");
}

pub(super) fn emit_udp_socket_recv_from_string_helper(out: &mut String) {
    let udp_socket = c_struct_ident("UdpSocket", &[]);
    let udp_datagram = c_struct_ident("UdpDatagram", &[]);
    let net_error = c_struct_ident("NetError", &[]);
    let result = c_enum_ident(
        "Result",
        &[
            ValueType::Struct("UdpDatagram".to_string(), Vec::new()),
            ValueType::Struct("NetError".to_string(), Vec::new()),
        ],
    );
    let ok = c_enum_variant_ident(
        "Result",
        &[
            ValueType::Struct("UdpDatagram".to_string(), Vec::new()),
            ValueType::Struct("NetError".to_string(), Vec::new()),
        ],
        "Ok",
    );
    let err = c_enum_variant_ident(
        "Result",
        &[
            ValueType::Struct("UdpDatagram".to_string(), Vec::new()),
            ValueType::Struct("NetError".to_string(), Vec::new()),
        ],
        "Err",
    );
    out.push_str("static ");
    out.push_str(&result);
    out.push_str(" nomo_udp_socket_recv_from_string(");
    out.push_str(&udp_socket);
    out.push_str(" socket, int64_t max_bytes) {\n");
    out.push_str("    if (socket.");
    out.push_str(&c_member_ident("handle"));
    out.push_str(" == NOMO_INVALID_SOCKET) {\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&net_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(\"socket is closed\")}};\n");
    out.push_str("    }\n");
    out.push_str("    if (max_bytes < 0 || max_bytes > INT32_MAX) {\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&net_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(\"invalid max byte count\")}};\n");
    out.push_str("    }\n");
    out.push_str("    char *buffer = (char *)malloc((size_t)max_bytes + 1);\n");
    out.push_str("    if (buffer == NULL) { nomo_panic(\"out of memory\"); }\n");
    out.push_str("    struct sockaddr_storage address;\n");
    out.push_str("#ifdef _WIN32\n");
    out.push_str("    int address_len = sizeof(address);\n");
    out.push_str("#else\n");
    out.push_str("    socklen_t address_len = sizeof(address);\n");
    out.push_str("#endif\n");
    out.push_str("    int received = recvfrom(socket.");
    out.push_str(&c_member_ident("handle"));
    out.push_str(", buffer, (int)max_bytes, 0, (struct sockaddr *)&address, &address_len);\n");
    out.push_str("    if (received < 0) {\n");
    out.push_str("        nomo_string message = nomo_net_error_message();\n");
    out.push_str("        free(buffer);\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&net_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = message}};\n");
    out.push_str("    }\n");
    out.push_str("    buffer[received] = '\\0';\n");
    out.push_str("    char host[1025];\n");
    out.push_str("    char service[32];\n");
    out.push_str("    int rc = getnameinfo((struct sockaddr *)&address, address_len, host, sizeof(host), service, sizeof(service), NI_NUMERICHOST | NI_NUMERICSERV);\n");
    out.push_str("    if (rc != 0) {\n");
    out.push_str("        free(buffer);\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&net_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(gai_strerror(rc))}};\n");
    out.push_str("    }\n");
    out.push_str("    return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&ok);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Ok"));
    out.push_str(" = (");
    out.push_str(&udp_datagram);
    out.push_str("){.");
    out.push_str(&c_member_ident("data"));
    out.push_str(" = nomo_string_owned(buffer), .");
    out.push_str(&c_member_ident("host"));
    out.push_str(" = nomo_string_from_cstr(host), .");
    out.push_str(&c_member_ident("port"));
    out.push_str(" = (int64_t)strtoll(service, NULL, 10)}};\n");
    out.push_str("}\n");
}

pub(super) fn emit_udp_socket_send_to_string_helper(out: &mut String) {
    let udp_socket = c_struct_ident("UdpSocket", &[]);
    let net_error = c_struct_ident("NetError", &[]);
    let result = c_enum_ident(
        "Result",
        &[
            ValueType::Void,
            ValueType::Struct("NetError".to_string(), Vec::new()),
        ],
    );
    let ok = c_enum_variant_ident(
        "Result",
        &[
            ValueType::Void,
            ValueType::Struct("NetError".to_string(), Vec::new()),
        ],
        "Ok",
    );
    let err = c_enum_variant_ident(
        "Result",
        &[
            ValueType::Void,
            ValueType::Struct("NetError".to_string(), Vec::new()),
        ],
        "Err",
    );
    out.push_str("static ");
    out.push_str(&result);
    out.push_str(" nomo_udp_socket_send_to_string(");
    out.push_str(&udp_socket);
    out.push_str(" socket, nomo_string content, nomo_string host, int64_t port) {\n");
    out.push_str("    if (socket.");
    out.push_str(&c_member_ident("handle"));
    out.push_str(" == NOMO_INVALID_SOCKET) {\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&net_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(\"socket is closed\")}};\n");
    out.push_str("    }\n");
    out.push_str("    if (port < 0 || port > 65535) {\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&net_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(\"invalid port\")}};\n");
    out.push_str("    }\n");
    out.push_str("    char port_text[16];\n");
    out.push_str("    snprintf(port_text, sizeof(port_text), \"%\" PRId64, port);\n");
    out.push_str("    struct addrinfo hints;\n");
    out.push_str("    memset(&hints, 0, sizeof(hints));\n");
    out.push_str("    hints.ai_family = AF_UNSPEC;\n");
    out.push_str("    hints.ai_socktype = SOCK_DGRAM;\n");
    out.push_str("    struct addrinfo *addresses = NULL;\n");
    out.push_str("    int rc = getaddrinfo(host.data, port_text, &hints, &addresses);\n");
    out.push_str("    if (rc != 0) {\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&net_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(gai_strerror(rc))}};\n");
    out.push_str("    }\n");
    out.push_str("    size_t len = strlen(content.data);\n");
    out.push_str("    int sent = -1;\n");
    out.push_str("    for (struct addrinfo *address = addresses; address != NULL; address = address->ai_next) {\n");
    out.push_str("        sent = sendto(socket.");
    out.push_str(&c_member_ident("handle"));
    out.push_str(", content.data, (int)len, 0, address->ai_addr, address->ai_addrlen);\n");
    out.push_str("        if (sent == (int)len) { break; }\n");
    out.push_str("    }\n");
    out.push_str("    freeaddrinfo(addresses);\n");
    out.push_str("    if (sent != (int)len) {\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&net_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_net_error_message()}};\n");
    out.push_str("    }\n");
    out.push_str("    return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&ok);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Ok"));
    out.push_str(" = 0};\n");
    out.push_str("}\n");
}

pub(super) fn emit_udp_socket_close_helper(out: &mut String) {
    let udp_socket = c_struct_ident("UdpSocket", &[]);
    out.push_str("static void nomo_udp_socket_close(");
    out.push_str(&udp_socket);
    out.push_str(" socket) {\n");
    out.push_str("    if (socket.");
    out.push_str(&c_member_ident("handle"));
    out.push_str(" != NOMO_INVALID_SOCKET) {\n");
    out.push_str("        NOMO_SOCKET_CLOSE(socket.");
    out.push_str(&c_member_ident("handle"));
    out.push_str(");\n");
    out.push_str("    }\n");
    out.push_str("}\n");
}

pub(super) fn emit_http_client_helpers(out: &mut String) {
    let http_response = c_struct_ident("HttpResponse", &[]);
    let http_error = c_struct_ident("HttpError", &[]);
    let result = c_enum_ident(
        "Result",
        &[
            ValueType::Struct("HttpResponse".to_string(), Vec::new()),
            ValueType::Struct("HttpError".to_string(), Vec::new()),
        ],
    );
    let ok = c_enum_variant_ident(
        "Result",
        &[
            ValueType::Struct("HttpResponse".to_string(), Vec::new()),
            ValueType::Struct("HttpError".to_string(), Vec::new()),
        ],
        "Ok",
    );
    let err = c_enum_variant_ident(
        "Result",
        &[
            ValueType::Struct("HttpResponse".to_string(), Vec::new()),
            ValueType::Struct("HttpError".to_string(), Vec::new()),
        ],
        "Err",
    );
    let err_payload = c_payload_ident("Err");
    let ok_payload = c_payload_ident("Ok");
    let status_member = c_member_ident("status");
    let body_member = c_member_ident("body");
    let message_member = c_member_ident("message");
    let get_name = c_fn_ident(BUILTIN_HTTP_GET_EXPR);
    let post_name = c_fn_ident(BUILTIN_HTTP_POST_EXPR);
    out.push_str("typedef struct nomo_http_url {\n");
    out.push_str("    char *host;\n");
    out.push_str("    char *port;\n");
    out.push_str("    char *path;\n");
    out.push_str("} nomo_http_url;\n\n");
    out.push_str("static char *nomo_http_copy_slice(const char *data, size_t len) {\n");
    out.push_str("    char *out = (char *)malloc(len + 1);\n");
    out.push_str("    if (out == NULL) { nomo_panic(\"out of memory\"); }\n");
    out.push_str("    memcpy(out, data, len);\n");
    out.push_str("    out[len] = '\\0';\n");
    out.push_str("    return out;\n");
    out.push_str("}\n\n");
    out.push_str("static void nomo_http_url_free(nomo_http_url url) {\n");
    out.push_str("    free(url.host);\n");
    out.push_str("    free(url.port);\n");
    out.push_str("    free(url.path);\n");
    out.push_str("}\n\n");
    out.push_str("static ");
    out.push_str(&result);
    out.push_str(" nomo_http_error_from_string(nomo_string message) {\n");
    out.push_str("    return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&err_payload);
    out.push_str(" = (");
    out.push_str(&http_error);
    out.push_str("){.");
    out.push_str(&message_member);
    out.push_str(" = message}};\n");
    out.push_str("}\n\n");
    out.push_str("static ");
    out.push_str(&result);
    out.push_str(" nomo_http_error_from_cstr(const char *message) {\n");
    out.push_str("    return nomo_http_error_from_string(nomo_string_from_cstr(message));\n");
    out.push_str("}\n\n");
    out.push_str("static int nomo_http_parse_url(nomo_string value, nomo_http_url *out) {\n");
    out.push_str("    const char *text = value.data;\n");
    out.push_str("    const char *prefix = \"http://\";\n");
    out.push_str("    size_t prefix_len = strlen(prefix);\n");
    out.push_str("    if (strncmp(text, prefix, prefix_len) != 0) { return 0; }\n");
    out.push_str("    const char *host_start = text + prefix_len;\n");
    out.push_str("    const char *cursor = host_start;\n");
    out.push_str(
        "    while (*cursor != '\\0' && *cursor != ':' && *cursor != '/') { cursor += 1; }\n",
    );
    out.push_str("    if (cursor == host_start) { return 0; }\n");
    out.push_str(
        "    out->host = nomo_http_copy_slice(host_start, (size_t)(cursor - host_start));\n",
    );
    out.push_str("    if (*cursor == ':') {\n");
    out.push_str("        const char *port_start = cursor + 1;\n");
    out.push_str("        cursor = port_start;\n");
    out.push_str("        while (*cursor >= '0' && *cursor <= '9') { cursor += 1; }\n");
    out.push_str("        if (cursor == port_start || (*cursor != '\\0' && *cursor != '/')) { free(out->host); out->host = NULL; return 0; }\n");
    out.push_str(
        "        out->port = nomo_http_copy_slice(port_start, (size_t)(cursor - port_start));\n",
    );
    out.push_str("    } else {\n");
    out.push_str("        out->port = nomo_http_copy_slice(\"80\", 2);\n");
    out.push_str("    }\n");
    out.push_str("    if (*cursor == '/') {\n");
    out.push_str("        out->path = nomo_http_copy_slice(cursor, strlen(cursor));\n");
    out.push_str("    } else {\n");
    out.push_str("        out->path = nomo_http_copy_slice(\"/\", 1);\n");
    out.push_str("    }\n");
    out.push_str("    return 1;\n");
    out.push_str("}\n\n");
    out.push_str("static ");
    out.push_str(&result);
    out.push_str(" nomo_http_request(const char *method, nomo_string url_value, nomo_string body, int has_body) {\n");
    out.push_str("    if (!nomo_net_init()) { return nomo_http_error_from_cstr(\"network initialization failed\"); }\n");
    out.push_str("    nomo_http_url url = {0};\n");
    out.push_str("    if (!nomo_http_parse_url(url_value, &url)) { return nomo_http_error_from_cstr(\"unsupported or invalid HTTP URL\"); }\n");
    out.push_str("    struct addrinfo hints;\n");
    out.push_str("    memset(&hints, 0, sizeof(hints));\n");
    out.push_str("    hints.ai_family = AF_UNSPEC;\n");
    out.push_str("    hints.ai_socktype = SOCK_STREAM;\n");
    out.push_str("    struct addrinfo *addresses = NULL;\n");
    out.push_str("    int rc = getaddrinfo(url.host, url.port, &hints, &addresses);\n");
    out.push_str("    if (rc != 0) { nomo_http_url_free(url); return nomo_http_error_from_cstr(gai_strerror(rc)); }\n");
    out.push_str("    nomo_socket handle = NOMO_INVALID_SOCKET;\n");
    out.push_str("    for (struct addrinfo *address = addresses; address != NULL; address = address->ai_next) {\n");
    out.push_str("        handle = socket(address->ai_family, address->ai_socktype, address->ai_protocol);\n");
    out.push_str("        if (handle == NOMO_INVALID_SOCKET) { continue; }\n");
    out.push_str(
        "        if (connect(handle, address->ai_addr, address->ai_addrlen) == 0) { break; }\n",
    );
    out.push_str("        NOMO_SOCKET_CLOSE(handle);\n");
    out.push_str("        handle = NOMO_INVALID_SOCKET;\n");
    out.push_str("    }\n");
    out.push_str("    freeaddrinfo(addresses);\n");
    out.push_str("    if (handle == NOMO_INVALID_SOCKET) { nomo_http_url_free(url); return nomo_http_error_from_string(nomo_net_error_message()); }\n");
    out.push_str("    size_t body_len = has_body ? strlen(body.data) : 0;\n");
    out.push_str("    int header_len = snprintf(NULL, 0, \"%s %s HTTP/1.0\\r\\nHost: %s\\r\\nConnection: close\\r\\nContent-Length: %zu\\r\\n\\r\\n\", method, url.path, url.host, body_len);\n");
    out.push_str("    if (header_len < 0) { NOMO_SOCKET_CLOSE(handle); nomo_http_url_free(url); return nomo_http_error_from_cstr(\"failed to build HTTP request\"); }\n");
    out.push_str("    char *request = (char *)malloc((size_t)header_len + body_len + 1);\n");
    out.push_str("    if (request == NULL) { nomo_panic(\"out of memory\"); }\n");
    out.push_str("    snprintf(request, (size_t)header_len + 1, \"%s %s HTTP/1.0\\r\\nHost: %s\\r\\nConnection: close\\r\\nContent-Length: %zu\\r\\n\\r\\n\", method, url.path, url.host, body_len);\n");
    out.push_str("    if (body_len > 0) { memcpy(request + header_len, body.data, body_len); }\n");
    out.push_str("    size_t request_len = (size_t)header_len + body_len;\n");
    out.push_str("    request[request_len] = '\\0';\n");
    out.push_str("    size_t sent_total = 0;\n");
    out.push_str("    while (sent_total < request_len) {\n");
    out.push_str("        int sent = send(handle, request + sent_total, (int)(request_len - sent_total), 0);\n");
    out.push_str("        if (sent <= 0) { nomo_string message = nomo_net_error_message(); free(request); NOMO_SOCKET_CLOSE(handle); nomo_http_url_free(url); return nomo_http_error_from_string(message); }\n");
    out.push_str("        sent_total += (size_t)sent;\n");
    out.push_str("    }\n");
    out.push_str("    free(request);\n");
    out.push_str("    size_t cap = 4096;\n");
    out.push_str("    size_t len = 0;\n");
    out.push_str("    char *response = (char *)malloc(cap + 1);\n");
    out.push_str("    if (response == NULL) { nomo_panic(\"out of memory\"); }\n");
    out.push_str("    for (;;) {\n");
    out.push_str("        if (len + 4096 + 1 > cap) { while (len + 4096 + 1 > cap) { cap *= 2; } response = (char *)realloc(response, cap + 1); if (response == NULL) { nomo_panic(\"out of memory\"); } }\n");
    out.push_str("        int received = recv(handle, response + len, 4096, 0);\n");
    out.push_str("        if (received < 0) { nomo_string message = nomo_net_error_message(); free(response); NOMO_SOCKET_CLOSE(handle); nomo_http_url_free(url); return nomo_http_error_from_string(message); }\n");
    out.push_str("        if (received == 0) { break; }\n");
    out.push_str("        len += (size_t)received;\n");
    out.push_str("    }\n");
    out.push_str("    NOMO_SOCKET_CLOSE(handle);\n");
    out.push_str("    nomo_http_url_free(url);\n");
    out.push_str("    response[len] = '\\0';\n");
    out.push_str("    char *status_space = strchr(response, ' ');\n");
    out.push_str("    if (status_space == NULL) { free(response); return nomo_http_error_from_cstr(\"invalid HTTP response status line\"); }\n");
    out.push_str("    long status = strtol(status_space + 1, NULL, 10);\n");
    out.push_str("    char *body_start = strstr(response, \"\\r\\n\\r\\n\");\n");
    out.push_str("    if (body_start == NULL) { free(response); return nomo_http_error_from_cstr(\"invalid HTTP response headers\"); }\n");
    out.push_str("    body_start += 4;\n");
    out.push_str("    size_t body_size = len - (size_t)(body_start - response);\n");
    out.push_str("    char *body_copy = nomo_http_copy_slice(body_start, body_size);\n");
    out.push_str("    free(response);\n");
    out.push_str("    return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&ok);
    out.push_str(", .payload.");
    out.push_str(&ok_payload);
    out.push_str(" = (");
    out.push_str(&http_response);
    out.push_str("){.");
    out.push_str(&status_member);
    out.push_str(" = (int64_t)status, .");
    out.push_str(&body_member);
    out.push_str(" = nomo_string_owned(body_copy)}};\n");
    out.push_str("}\n\n");
    out.push_str("static ");
    out.push_str(&result);
    out.push(' ');
    out.push_str(&get_name);
    out.push_str("(nomo_string url) {\n");
    out.push_str("    return nomo_http_request(\"GET\", url, nomo_string_literal(\"\"), 0);\n");
    out.push_str("}\n\n");
    out.push_str("static ");
    out.push_str(&result);
    out.push(' ');
    out.push_str(&post_name);
    out.push_str("(nomo_string url, nomo_string body) {\n");
    out.push_str("    return nomo_http_request(\"POST\", url, body, 1);\n");
    out.push_str("}\n");
}

pub(super) fn emit_http_server_helpers(out: &mut String) {
    let http_server = c_struct_ident("HttpServer", &[]);
    let http_exchange = c_struct_ident("HttpExchange", &[]);
    let http_error = c_struct_ident("HttpError", &[]);
    let result_server = c_enum_ident(
        "Result",
        &[
            ValueType::Struct("HttpServer".to_string(), Vec::new()),
            ValueType::Struct("HttpError".to_string(), Vec::new()),
        ],
    );
    let result_exchange = c_enum_ident(
        "Result",
        &[
            ValueType::Struct("HttpExchange".to_string(), Vec::new()),
            ValueType::Struct("HttpError".to_string(), Vec::new()),
        ],
    );
    let result_void = c_enum_ident(
        "Result",
        &[
            ValueType::Void,
            ValueType::Struct("HttpError".to_string(), Vec::new()),
        ],
    );
    let server_ok = c_enum_variant_ident(
        "Result",
        &[
            ValueType::Struct("HttpServer".to_string(), Vec::new()),
            ValueType::Struct("HttpError".to_string(), Vec::new()),
        ],
        "Ok",
    );
    let server_err = c_enum_variant_ident(
        "Result",
        &[
            ValueType::Struct("HttpServer".to_string(), Vec::new()),
            ValueType::Struct("HttpError".to_string(), Vec::new()),
        ],
        "Err",
    );
    let exchange_ok = c_enum_variant_ident(
        "Result",
        &[
            ValueType::Struct("HttpExchange".to_string(), Vec::new()),
            ValueType::Struct("HttpError".to_string(), Vec::new()),
        ],
        "Ok",
    );
    let exchange_err = c_enum_variant_ident(
        "Result",
        &[
            ValueType::Struct("HttpExchange".to_string(), Vec::new()),
            ValueType::Struct("HttpError".to_string(), Vec::new()),
        ],
        "Err",
    );
    let void_ok = c_enum_variant_ident(
        "Result",
        &[
            ValueType::Void,
            ValueType::Struct("HttpError".to_string(), Vec::new()),
        ],
        "Ok",
    );
    let void_err = c_enum_variant_ident(
        "Result",
        &[
            ValueType::Void,
            ValueType::Struct("HttpError".to_string(), Vec::new()),
        ],
        "Err",
    );
    let listen_name = c_fn_ident(BUILTIN_HTTP_LISTEN_EXPR);
    let accept_name = c_fn_ident(BUILTIN_HTTP_ACCEPT_EXPR);
    let respond_name = c_fn_ident(BUILTIN_HTTP_RESPOND_STRING_EXPR);
    let close_server_name = c_fn_ident(BUILTIN_HTTP_CLOSE_SERVER_EXPR);
    let close_exchange_name = c_fn_ident(BUILTIN_HTTP_CLOSE_EXCHANGE_EXPR);
    let handle_member = c_member_ident("handle");
    let method_member = c_member_ident("method");
    let path_member = c_member_ident("path");
    let body_member = c_member_ident("body");
    let message_member = c_member_ident("message");
    let ok_payload = c_payload_ident("Ok");
    let err_payload = c_payload_ident("Err");

    out.push_str("static char *nomo_http_server_copy_slice(const char *data, size_t len) {\n");
    out.push_str("    char *out = (char *)malloc(len + 1);\n");
    out.push_str("    if (out == NULL) { nomo_panic(\"out of memory\"); }\n");
    out.push_str("    memcpy(out, data, len);\n");
    out.push_str("    out[len] = '\\0';\n");
    out.push_str("    return out;\n");
    out.push_str("}\n\n");
    out.push_str("static ");
    out.push_str(&result_server);
    out.push_str(" nomo_http_server_listen_error(nomo_string message) {\n");
    out.push_str("    return (");
    out.push_str(&result_server);
    out.push_str("){.tag = ");
    out.push_str(&server_err);
    out.push_str(", .payload.");
    out.push_str(&err_payload);
    out.push_str(" = (");
    out.push_str(&http_error);
    out.push_str("){.");
    out.push_str(&message_member);
    out.push_str(" = message}};\n");
    out.push_str("}\n\n");
    out.push_str("static ");
    out.push_str(&result_exchange);
    out.push_str(" nomo_http_server_accept_error(nomo_string message) {\n");
    out.push_str("    return (");
    out.push_str(&result_exchange);
    out.push_str("){.tag = ");
    out.push_str(&exchange_err);
    out.push_str(", .payload.");
    out.push_str(&err_payload);
    out.push_str(" = (");
    out.push_str(&http_error);
    out.push_str("){.");
    out.push_str(&message_member);
    out.push_str(" = message}};\n");
    out.push_str("}\n\n");
    out.push_str("static ");
    out.push_str(&result_void);
    out.push_str(" nomo_http_server_void_error(nomo_string message) {\n");
    out.push_str("    return (");
    out.push_str(&result_void);
    out.push_str("){.tag = ");
    out.push_str(&void_err);
    out.push_str(", .payload.");
    out.push_str(&err_payload);
    out.push_str(" = (");
    out.push_str(&http_error);
    out.push_str("){.");
    out.push_str(&message_member);
    out.push_str(" = message}};\n");
    out.push_str("}\n\n");

    out.push_str("static ");
    out.push_str(&result_server);
    out.push(' ');
    out.push_str(&listen_name);
    out.push_str("(nomo_string host, int64_t port) {\n");
    out.push_str("    if (!nomo_net_init()) { return nomo_http_server_listen_error(nomo_string_from_cstr(\"network initialization failed\")); }\n");
    out.push_str("    if (port < 0 || port > 65535) { return nomo_http_server_listen_error(nomo_string_from_cstr(\"invalid port\")); }\n");
    out.push_str("    char port_text[16];\n");
    out.push_str("    snprintf(port_text, sizeof(port_text), \"%\" PRId64, port);\n");
    out.push_str("    struct addrinfo hints;\n");
    out.push_str("    memset(&hints, 0, sizeof(hints));\n");
    out.push_str("    hints.ai_family = AF_UNSPEC;\n");
    out.push_str("    hints.ai_socktype = SOCK_STREAM;\n");
    out.push_str("    hints.ai_flags = AI_PASSIVE;\n");
    out.push_str("    struct addrinfo *addresses = NULL;\n");
    out.push_str("    int rc = getaddrinfo(host.data, port_text, &hints, &addresses);\n");
    out.push_str("    if (rc != 0) { return nomo_http_server_listen_error(nomo_string_from_cstr(gai_strerror(rc))); }\n");
    out.push_str("    nomo_socket handle = NOMO_INVALID_SOCKET;\n");
    out.push_str("    for (struct addrinfo *address = addresses; address != NULL; address = address->ai_next) {\n");
    out.push_str("        handle = socket(address->ai_family, address->ai_socktype, address->ai_protocol);\n");
    out.push_str("        if (handle == NOMO_INVALID_SOCKET) { continue; }\n");
    out.push_str("        int yes = 1;\n");
    out.push_str("#ifdef _WIN32\n");
    out.push_str(
        "        setsockopt(handle, SOL_SOCKET, SO_REUSEADDR, (const char *)&yes, sizeof(yes));\n",
    );
    out.push_str("#else\n");
    out.push_str("        setsockopt(handle, SOL_SOCKET, SO_REUSEADDR, &yes, sizeof(yes));\n");
    out.push_str("#endif\n");
    out.push_str("        if (bind(handle, address->ai_addr, address->ai_addrlen) == 0 && listen(handle, 16) == 0) { break; }\n");
    out.push_str("        NOMO_SOCKET_CLOSE(handle);\n");
    out.push_str("        handle = NOMO_INVALID_SOCKET;\n");
    out.push_str("    }\n");
    out.push_str("    freeaddrinfo(addresses);\n");
    out.push_str("    if (handle == NOMO_INVALID_SOCKET) { return nomo_http_server_listen_error(nomo_net_error_message()); }\n");
    out.push_str("    return (");
    out.push_str(&result_server);
    out.push_str("){.tag = ");
    out.push_str(&server_ok);
    out.push_str(", .payload.");
    out.push_str(&ok_payload);
    out.push_str(" = (");
    out.push_str(&http_server);
    out.push_str("){.");
    out.push_str(&handle_member);
    out.push_str(" = handle}};\n");
    out.push_str("}\n\n");

    out.push_str("static ");
    out.push_str(&result_exchange);
    out.push(' ');
    out.push_str(&accept_name);
    out.push('(');
    out.push_str(&http_server);
    out.push_str(" server) {\n");
    out.push_str("    if (server.");
    out.push_str(&handle_member);
    out.push_str(" == NOMO_INVALID_SOCKET) { return nomo_http_server_accept_error(nomo_string_from_cstr(\"server is closed\")); }\n");
    out.push_str("    nomo_socket client = accept(server.");
    out.push_str(&handle_member);
    out.push_str(", NULL, NULL);\n");
    out.push_str("    if (client == NOMO_INVALID_SOCKET) { return nomo_http_server_accept_error(nomo_net_error_message()); }\n");
    out.push_str("    size_t cap = 4096;\n");
    out.push_str("    size_t len = 0;\n");
    out.push_str("    char *request = (char *)malloc(cap + 1);\n");
    out.push_str("    if (request == NULL) { nomo_panic(\"out of memory\"); }\n");
    out.push_str("    size_t expected_len = 0;\n");
    out.push_str("    for (;;) {\n");
    out.push_str("        if (len + 1024 + 1 > cap) { while (len + 1024 + 1 > cap) { cap *= 2; } request = (char *)realloc(request, cap + 1); if (request == NULL) { nomo_panic(\"out of memory\"); } }\n");
    out.push_str("        int received = recv(client, request + len, 1024, 0);\n");
    out.push_str("        if (received < 0) { nomo_string message = nomo_net_error_message(); free(request); NOMO_SOCKET_CLOSE(client); return nomo_http_server_accept_error(message); }\n");
    out.push_str("        if (received == 0) { break; }\n");
    out.push_str("        len += (size_t)received;\n");
    out.push_str("        request[len] = '\\0';\n");
    out.push_str("        char *headers_end = strstr(request, \"\\r\\n\\r\\n\");\n");
    out.push_str("        if (headers_end != NULL) {\n");
    out.push_str("            if (expected_len == 0) {\n");
    out.push_str("                expected_len = (size_t)(headers_end - request) + 4;\n");
    out.push_str("                char *content_length = strstr(request, \"Content-Length: \");\n");
    out.push_str("                if (content_length != NULL && content_length < headers_end) { expected_len += (size_t)strtoull(content_length + 16, NULL, 10); }\n");
    out.push_str("            }\n");
    out.push_str("            if (len >= expected_len) { break; }\n");
    out.push_str("        }\n");
    out.push_str("    }\n");
    out.push_str("    request[len] = '\\0';\n");
    out.push_str("    char *method_end = strchr(request, ' ');\n");
    out.push_str("    if (method_end == NULL) { free(request); NOMO_SOCKET_CLOSE(client); return nomo_http_server_accept_error(nomo_string_from_cstr(\"invalid HTTP request line\")); }\n");
    out.push_str("    char *path_start = method_end + 1;\n");
    out.push_str("    char *path_end = strchr(path_start, ' ');\n");
    out.push_str("    if (path_end == NULL) { free(request); NOMO_SOCKET_CLOSE(client); return nomo_http_server_accept_error(nomo_string_from_cstr(\"invalid HTTP request path\")); }\n");
    out.push_str("    char *headers_end = strstr(request, \"\\r\\n\\r\\n\");\n");
    out.push_str("    if (headers_end == NULL) { free(request); NOMO_SOCKET_CLOSE(client); return nomo_http_server_accept_error(nomo_string_from_cstr(\"invalid HTTP request headers\")); }\n");
    out.push_str("    char *body_start = headers_end + 4;\n");
    out.push_str("    size_t body_len = len - (size_t)(body_start - request);\n");
    out.push_str("    char *method_copy = nomo_http_server_copy_slice(request, (size_t)(method_end - request));\n");
    out.push_str("    char *path_copy = nomo_http_server_copy_slice(path_start, (size_t)(path_end - path_start));\n");
    out.push_str("    char *body_copy = nomo_http_server_copy_slice(body_start, body_len);\n");
    out.push_str("    free(request);\n");
    out.push_str("    return (");
    out.push_str(&result_exchange);
    out.push_str("){.tag = ");
    out.push_str(&exchange_ok);
    out.push_str(", .payload.");
    out.push_str(&ok_payload);
    out.push_str(" = (");
    out.push_str(&http_exchange);
    out.push_str("){.");
    out.push_str(&handle_member);
    out.push_str(" = client, .");
    out.push_str(&method_member);
    out.push_str(" = nomo_string_owned(method_copy), .");
    out.push_str(&path_member);
    out.push_str(" = nomo_string_owned(path_copy), .");
    out.push_str(&body_member);
    out.push_str(" = nomo_string_owned(body_copy)}};\n");
    out.push_str("}\n\n");

    out.push_str("static ");
    out.push_str(&result_void);
    out.push(' ');
    out.push_str(&respond_name);
    out.push('(');
    out.push_str(&http_exchange);
    out.push_str(" exchange, int64_t status, nomo_string body) {\n");
    out.push_str("    if (exchange.");
    out.push_str(&handle_member);
    out.push_str(" == NOMO_INVALID_SOCKET) { return nomo_http_server_void_error(nomo_string_from_cstr(\"exchange is closed\")); }\n");
    out.push_str("    if (status < 100 || status > 999) { return nomo_http_server_void_error(nomo_string_from_cstr(\"invalid HTTP status\")); }\n");
    out.push_str("    size_t body_len = strlen(body.data);\n");
    out.push_str("    int header_len = snprintf(NULL, 0, \"HTTP/1.0 %\" PRId64 \" OK\\r\\nContent-Length: %zu\\r\\nConnection: close\\r\\n\\r\\n\", status, body_len);\n");
    out.push_str("    if (header_len < 0) { return nomo_http_server_void_error(nomo_string_from_cstr(\"failed to build HTTP response\")); }\n");
    out.push_str("    char *response = (char *)malloc((size_t)header_len + body_len + 1);\n");
    out.push_str("    if (response == NULL) { nomo_panic(\"out of memory\"); }\n");
    out.push_str("    snprintf(response, (size_t)header_len + 1, \"HTTP/1.0 %\" PRId64 \" OK\\r\\nContent-Length: %zu\\r\\nConnection: close\\r\\n\\r\\n\", status, body_len);\n");
    out.push_str("    if (body_len > 0) { memcpy(response + header_len, body.data, body_len); }\n");
    out.push_str("    size_t response_len = (size_t)header_len + body_len;\n");
    out.push_str("    size_t sent_total = 0;\n");
    out.push_str("    while (sent_total < response_len) {\n");
    out.push_str("        int sent = send(exchange.");
    out.push_str(&handle_member);
    out.push_str(", response + sent_total, (int)(response_len - sent_total), 0);\n");
    out.push_str("        if (sent <= 0) { nomo_string message = nomo_net_error_message(); free(response); NOMO_SOCKET_CLOSE(exchange.");
    out.push_str(&handle_member);
    out.push_str("); return nomo_http_server_void_error(message); }\n");
    out.push_str("        sent_total += (size_t)sent;\n");
    out.push_str("    }\n");
    out.push_str("    free(response);\n");
    out.push_str("    return (");
    out.push_str(&result_void);
    out.push_str("){.tag = ");
    out.push_str(&void_ok);
    out.push_str(", .payload.");
    out.push_str(&ok_payload);
    out.push_str(" = 0};\n");
    out.push_str("}\n\n");

    out.push_str("static void ");
    out.push_str(&close_server_name);
    out.push('(');
    out.push_str(&http_server);
    out.push_str(" server) {\n");
    out.push_str("    if (server.");
    out.push_str(&handle_member);
    out.push_str(" != NOMO_INVALID_SOCKET) { NOMO_SOCKET_CLOSE(server.");
    out.push_str(&handle_member);
    out.push_str("); }\n");
    out.push_str("}\n\n");
    out.push_str("static void ");
    out.push_str(&close_exchange_name);
    out.push('(');
    out.push_str(&http_exchange);
    out.push_str(" exchange) {\n");
    out.push_str("    if (exchange.");
    out.push_str(&handle_member);
    out.push_str(" != NOMO_INVALID_SOCKET) { NOMO_SOCKET_CLOSE(exchange.");
    out.push_str(&handle_member);
    out.push_str("); }\n");
    out.push_str("}\n");
}

pub(super) fn emit_tcp_stream_read_to_string_helper(out: &mut String) {
    let tcp_stream = c_struct_ident("TcpStream", &[]);
    let net_error = c_struct_ident("NetError", &[]);
    let result = c_enum_ident(
        "Result",
        &[
            ValueType::String,
            ValueType::Struct("NetError".to_string(), Vec::new()),
        ],
    );
    let ok = c_enum_variant_ident(
        "Result",
        &[
            ValueType::String,
            ValueType::Struct("NetError".to_string(), Vec::new()),
        ],
        "Ok",
    );
    let err = c_enum_variant_ident(
        "Result",
        &[
            ValueType::String,
            ValueType::Struct("NetError".to_string(), Vec::new()),
        ],
        "Err",
    );
    out.push_str("static ");
    out.push_str(&result);
    out.push_str(" nomo_tcp_stream_read_to_string(");
    out.push_str(&tcp_stream);
    out.push_str(" stream) {\n");
    out.push_str("    if (stream.");
    out.push_str(&c_member_ident("handle"));
    out.push_str(" == NOMO_INVALID_SOCKET) {\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&net_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(\"stream is closed\")}};\n");
    out.push_str("    }\n");
    out.push_str("    size_t len = 0;\n");
    out.push_str("    size_t cap = 1;\n");
    out.push_str("    char *buffer = (char *)malloc(cap);\n");
    out.push_str("    if (buffer == NULL) { nomo_panic(\"out of memory\"); }\n");
    out.push_str("    char chunk[512];\n");
    out.push_str("    for (;;) {\n");
    out.push_str("        int received = recv(stream.");
    out.push_str(&c_member_ident("handle"));
    out.push_str(", chunk, sizeof(chunk), 0);\n");
    out.push_str("        if (received == 0) { break; }\n");
    out.push_str("        if (received < 0) {\n");
    out.push_str("            nomo_string message = nomo_net_error_message();\n");
    out.push_str("            free(buffer);\n");
    out.push_str("            return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&net_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = message}};\n");
    out.push_str("        }\n");
    out.push_str("        if (len + (size_t)received + 1 > cap) {\n");
    out.push_str("            while (len + (size_t)received + 1 > cap) { cap *= 2; }\n");
    out.push_str("            char *next = (char *)realloc(buffer, cap);\n");
    out.push_str(
        "            if (next == NULL) { free(buffer); nomo_panic(\"out of memory\"); }\n",
    );
    out.push_str("            buffer = next;\n");
    out.push_str("        }\n");
    out.push_str("        memcpy(buffer + len, chunk, (size_t)received);\n");
    out.push_str("        len += (size_t)received;\n");
    out.push_str("    }\n");
    out.push_str("    buffer[len] = '\\0';\n");
    out.push_str("    return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&ok);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Ok"));
    out.push_str(" = nomo_string_owned(buffer)};\n");
    out.push_str("}\n");
}

pub(super) fn emit_tcp_stream_write_string_helper(out: &mut String) {
    let tcp_stream = c_struct_ident("TcpStream", &[]);
    let net_error = c_struct_ident("NetError", &[]);
    let result = c_enum_ident(
        "Result",
        &[
            ValueType::Void,
            ValueType::Struct("NetError".to_string(), Vec::new()),
        ],
    );
    let ok = c_enum_variant_ident(
        "Result",
        &[
            ValueType::Void,
            ValueType::Struct("NetError".to_string(), Vec::new()),
        ],
        "Ok",
    );
    let err = c_enum_variant_ident(
        "Result",
        &[
            ValueType::Void,
            ValueType::Struct("NetError".to_string(), Vec::new()),
        ],
        "Err",
    );
    out.push_str("static ");
    out.push_str(&result);
    out.push_str(" nomo_tcp_stream_write_string(");
    out.push_str(&tcp_stream);
    out.push_str(" stream, nomo_string content) {\n");
    out.push_str("    if (stream.");
    out.push_str(&c_member_ident("handle"));
    out.push_str(" == NOMO_INVALID_SOCKET) {\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&net_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(\"stream is closed\")}};\n");
    out.push_str("    }\n");
    out.push_str("    size_t len = strlen(content.data);\n");
    out.push_str("    size_t written = 0;\n");
    out.push_str("    while (written < len) {\n");
    out.push_str("        int sent = send(stream.");
    out.push_str(&c_member_ident("handle"));
    out.push_str(", content.data + written, (int)(len - written), 0);\n");
    out.push_str("        if (sent <= 0) {\n");
    out.push_str("            return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&net_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_net_error_message()}};\n");
    out.push_str("        }\n");
    out.push_str("        written += (size_t)sent;\n");
    out.push_str("    }\n");
    out.push_str("    return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&ok);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Ok"));
    out.push_str(" = 0};\n");
    out.push_str("}\n");
}

pub(super) fn emit_tcp_stream_close_helper(out: &mut String) {
    let tcp_stream = c_struct_ident("TcpStream", &[]);
    out.push_str("static void nomo_tcp_stream_close(");
    out.push_str(&tcp_stream);
    out.push_str(" stream) {\n");
    out.push_str("    if (stream.");
    out.push_str(&c_member_ident("handle"));
    out.push_str(" != NOMO_INVALID_SOCKET) {\n");
    out.push_str("        NOMO_SOCKET_CLOSE(stream.");
    out.push_str(&c_member_ident("handle"));
    out.push_str(");\n");
    out.push_str("    }\n");
    out.push_str("}\n");
}

pub(super) fn emit_env_get_helper(out: &mut String) {
    let result = c_enum_ident("Option", &[ValueType::String]);
    let some = c_enum_variant_ident("Option", &[ValueType::String], "Some");
    let none = c_enum_variant_ident("Option", &[ValueType::String], "None");
    out.push_str("static ");
    out.push_str(&result);
    out.push_str(" nomo_env_get(nomo_string name) {\n");
    out.push_str("    const char *value = getenv(name.data);\n");
    out.push_str("    if (value == NULL) {\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&none);
    out.push_str("};\n");
    out.push_str("    }\n");
    out.push_str("    return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&some);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Some"));
    out.push_str(" = nomo_string_from_cstr(value)};\n");
    out.push_str("}\n");
}

pub(super) fn emit_env_set_helper(out: &mut String) {
    out.push_str("static void nomo_env_set(nomo_string name, nomo_string value) {\n");
    out.push_str("#ifdef _WIN32\n");
    out.push_str(
        "    if (_putenv_s(name.data, value.data) != 0) { nomo_panic(\"env.set failed\"); }\n",
    );
    out.push_str("#else\n");
    out.push_str(
        "    if (setenv(name.data, value.data, 1) != 0) { nomo_panic(\"env.set failed\"); }\n",
    );
    out.push_str("#endif\n");
    out.push_str("}\n");
}

pub(super) fn emit_env_cwd_helper(out: &mut String) {
    out.push_str("static nomo_string nomo_env_cwd(void) {\n");
    out.push_str("    char buffer[PATH_MAX];\n");
    out.push_str("    if (NOMO_GETCWD(buffer, sizeof(buffer)) == NULL) { nomo_panic(\"env.cwd failed\"); }\n");
    out.push_str("    return nomo_string_from_cstr(buffer);\n");
    out.push_str("}\n");
}

pub(super) fn emit_env_home_dir_helper(out: &mut String) {
    let result = c_enum_ident("Option", &[ValueType::String]);
    let some = c_enum_variant_ident("Option", &[ValueType::String], "Some");
    let none = c_enum_variant_ident("Option", &[ValueType::String], "None");
    out.push_str("static ");
    out.push_str(&result);
    out.push_str(" nomo_env_home_dir(void) {\n");
    out.push_str("#ifdef _WIN32\n");
    out.push_str("    const char *value = getenv(\"USERPROFILE\");\n");
    out.push_str("    if (value == NULL) { value = getenv(\"HOME\"); }\n");
    out.push_str("#else\n");
    out.push_str("    const char *value = getenv(\"HOME\");\n");
    out.push_str("#endif\n");
    out.push_str("    if (value == NULL) {\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&none);
    out.push_str("};\n");
    out.push_str("    }\n");
    out.push_str("    return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&some);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Some"));
    out.push_str(" = nomo_string_from_cstr(value)};\n");
    out.push_str("}\n");
}

pub(super) fn emit_env_temp_dir_helper(out: &mut String) {
    out.push_str("static nomo_string nomo_env_temp_dir(void) {\n");
    out.push_str("    const char *value = getenv(\"TMPDIR\");\n");
    out.push_str("    if (value == NULL) { value = getenv(\"TEMP\"); }\n");
    out.push_str("    if (value == NULL) { value = getenv(\"TMP\"); }\n");
    out.push_str("    if (value == NULL) { value = \"/tmp\"; }\n");
    out.push_str("    return nomo_string_from_cstr(value);\n");
    out.push_str("}\n");
}

pub(super) fn emit_process_common_helpers(out: &mut String) {
    out.push_str("static int32_t nomo_process_exit_code(int status) {\n");
    out.push_str("#ifdef _WIN32\n");
    out.push_str("    return (int32_t)status;\n");
    out.push_str("#else\n");
    out.push_str("    if (WIFEXITED(status)) { return (int32_t)WEXITSTATUS(status); }\n");
    out.push_str("    return (int32_t)status;\n");
    out.push_str("#endif\n");
    out.push_str("}\n\n");
    out.push_str("static nomo_string nomo_process_status_message(int32_t status) {\n");
    out.push_str("    char buffer[64];\n");
    out.push_str(
        "    snprintf(buffer, sizeof(buffer), \"process exited with status %\" PRId32, status);\n",
    );
    out.push_str("    return nomo_string_from_cstr(buffer);\n");
    out.push_str("}\n");
    out.push_str("\nstatic char *nomo_process_temp_path(const char *label) {\n");
    out.push_str("    static unsigned long counter = 0;\n");
    out.push_str("    const char *dir = getenv(\"TMPDIR\");\n");
    out.push_str("    if (dir == NULL) { dir = getenv(\"TEMP\"); }\n");
    out.push_str("    if (dir == NULL) { dir = getenv(\"TMP\"); }\n");
    out.push_str("#ifdef _WIN32\n");
    out.push_str("    if (dir == NULL) { dir = \".\"; }\n");
    out.push_str("    unsigned long pid = (unsigned long)GetCurrentProcessId();\n");
    out.push_str("    const char *sep = \"\\\\\";\n");
    out.push_str("#else\n");
    out.push_str("    if (dir == NULL) { dir = \"/tmp\"; }\n");
    out.push_str("    unsigned long pid = (unsigned long)getpid();\n");
    out.push_str("    const char *sep = \"/\";\n");
    out.push_str("#endif\n");
    out.push_str("    unsigned long id = counter++;\n");
    out.push_str("    size_t cap = strlen(dir) + strlen(label) + 96;\n");
    out.push_str("    char *path = (char *)malloc(cap);\n");
    out.push_str("    if (path == NULL) { nomo_panic(\"out of memory\"); }\n");
    out.push_str("    snprintf(path, cap, \"%s%s%s-%lu-%lu.tmp\", dir, sep, label, pid, id);\n");
    out.push_str("    return path;\n");
    out.push_str("}\n");
    out.push_str("\nstatic int nomo_process_read_file(const char *path, nomo_string *text, nomo_string *error_message) {\n");
    out.push_str("    FILE *file = fopen(path, \"rb\");\n");
    out.push_str("    if (file == NULL) { *error_message = nomo_string_from_cstr(strerror(errno)); return 0; }\n");
    out.push_str("    if (fseek(file, 0, SEEK_END) != 0) {\n");
    out.push_str("        const char *message = strerror(errno);\n");
    out.push_str("        fclose(file);\n");
    out.push_str("        *error_message = nomo_string_from_cstr(message);\n");
    out.push_str("        return 0;\n");
    out.push_str("    }\n");
    out.push_str("    long size = ftell(file);\n");
    out.push_str("    if (size < 0 || fseek(file, 0, SEEK_SET) != 0) {\n");
    out.push_str("        const char *message = strerror(errno);\n");
    out.push_str("        fclose(file);\n");
    out.push_str("        *error_message = nomo_string_from_cstr(message);\n");
    out.push_str("        return 0;\n");
    out.push_str("    }\n");
    out.push_str("    char *buffer = (char *)malloc((size_t)size + 1);\n");
    out.push_str("    if (buffer == NULL) { fclose(file); nomo_panic(\"out of memory\"); }\n");
    out.push_str("    size_t read = fread(buffer, 1, (size_t)size, file);\n");
    out.push_str("    if (read != (size_t)size) {\n");
    out.push_str(
        "        const char *message = ferror(file) ? strerror(errno) : \"short read\";\n",
    );
    out.push_str("        free(buffer);\n");
    out.push_str("        fclose(file);\n");
    out.push_str("        *error_message = nomo_string_from_cstr(message);\n");
    out.push_str("        return 0;\n");
    out.push_str("    }\n");
    out.push_str("    buffer[size] = '\\0';\n");
    out.push_str("    fclose(file);\n");
    out.push_str("    *text = nomo_string_owned(buffer);\n");
    out.push_str("    return 1;\n");
    out.push_str("}\n");
}

pub(super) fn emit_process_spawn_helper(out: &mut String) {
    let process_error = c_struct_ident("ProcessError", &[]);
    let result = c_enum_ident(
        "Result",
        &[
            ValueType::I32,
            ValueType::Struct("ProcessError".to_string(), Vec::new()),
        ],
    );
    let ok = c_enum_variant_ident(
        "Result",
        &[
            ValueType::I32,
            ValueType::Struct("ProcessError".to_string(), Vec::new()),
        ],
        "Ok",
    );
    let err = c_enum_variant_ident(
        "Result",
        &[
            ValueType::I32,
            ValueType::Struct("ProcessError".to_string(), Vec::new()),
        ],
        "Err",
    );
    out.push_str("static ");
    out.push_str(&result);
    out.push_str(" nomo_process_spawn(nomo_string command) {\n");
    out.push_str("    int status = system(command.data);\n");
    out.push_str("    if (status == -1) {\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&process_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(strerror(errno))}};\n");
    out.push_str("    }\n");
    out.push_str("    return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&ok);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Ok"));
    out.push_str(" = nomo_process_exit_code(status)};\n");
    out.push_str("}\n");
}

pub(super) fn emit_process_status_helper(out: &mut String) {
    let result = c_enum_ident(
        "Result",
        &[
            ValueType::I32,
            ValueType::Struct("ProcessError".to_string(), Vec::new()),
        ],
    );
    out.push_str("static ");
    out.push_str(&result);
    out.push_str(" nomo_process_status(nomo_string command) {\n");
    out.push_str("    return nomo_process_spawn(command);\n");
    out.push_str("}\n");
}

pub(super) fn emit_process_exec_helper(out: &mut String) {
    let process_error = c_struct_ident("ProcessError", &[]);
    let result = c_enum_ident(
        "Result",
        &[
            ValueType::String,
            ValueType::Struct("ProcessError".to_string(), Vec::new()),
        ],
    );
    let ok = c_enum_variant_ident(
        "Result",
        &[
            ValueType::String,
            ValueType::Struct("ProcessError".to_string(), Vec::new()),
        ],
        "Ok",
    );
    let err = c_enum_variant_ident(
        "Result",
        &[
            ValueType::String,
            ValueType::Struct("ProcessError".to_string(), Vec::new()),
        ],
        "Err",
    );
    out.push_str("static ");
    out.push_str(&result);
    out.push_str(" nomo_process_exec(nomo_string command) {\n");
    out.push_str("    FILE *pipe = NOMO_POPEN(command.data, \"r\");\n");
    out.push_str("    if (pipe == NULL) {\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&process_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(strerror(errno))}};\n");
    out.push_str("    }\n");
    out.push_str("    size_t len = 0;\n");
    out.push_str("    size_t cap = 1;\n");
    out.push_str("    char *buffer = (char *)malloc(cap);\n");
    out.push_str("    if (buffer == NULL) { NOMO_PCLOSE(pipe); nomo_panic(\"out of memory\"); }\n");
    out.push_str("    char chunk[256];\n");
    out.push_str("    size_t read = 0;\n");
    out.push_str("    while ((read = fread(chunk, 1, sizeof(chunk), pipe)) > 0) {\n");
    out.push_str("        if (len + read + 1 > cap) {\n");
    out.push_str("            while (len + read + 1 > cap) { cap *= 2; }\n");
    out.push_str("            char *next = (char *)realloc(buffer, cap);\n");
    out.push_str("            if (next == NULL) { free(buffer); NOMO_PCLOSE(pipe); nomo_panic(\"out of memory\"); }\n");
    out.push_str("            buffer = next;\n");
    out.push_str("        }\n");
    out.push_str("        memcpy(buffer + len, chunk, read);\n");
    out.push_str("        len += read;\n");
    out.push_str("    }\n");
    out.push_str("    if (ferror(pipe)) {\n");
    out.push_str("        const char *message = strerror(errno);\n");
    out.push_str("        free(buffer);\n");
    out.push_str("        NOMO_PCLOSE(pipe);\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&process_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(message)}};\n");
    out.push_str("    }\n");
    out.push_str("    int status = NOMO_PCLOSE(pipe);\n");
    out.push_str("    if (status == -1) {\n");
    out.push_str("        const char *message = strerror(errno);\n");
    out.push_str("        free(buffer);\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&process_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(message)}};\n");
    out.push_str("    }\n");
    out.push_str("    int32_t code = nomo_process_exit_code(status);\n");
    out.push_str("    if (code != 0) {\n");
    out.push_str("        nomo_string message = nomo_process_status_message(code);\n");
    out.push_str("        free(buffer);\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&process_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = message}};\n");
    out.push_str("    }\n");
    out.push_str("    buffer[len] = '\\0';\n");
    out.push_str("    return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&ok);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Ok"));
    out.push_str(" = nomo_string_owned(buffer)};\n");
    out.push_str("}\n");
}

pub(super) fn emit_process_output_helper(out: &mut String) {
    let process_error = c_struct_ident("ProcessError", &[]);
    let process_output = c_struct_ident("ProcessOutput", &[]);
    let result = c_enum_ident(
        "Result",
        &[
            ValueType::Struct("ProcessOutput".to_string(), Vec::new()),
            ValueType::Struct("ProcessError".to_string(), Vec::new()),
        ],
    );
    let ok = c_enum_variant_ident(
        "Result",
        &[
            ValueType::Struct("ProcessOutput".to_string(), Vec::new()),
            ValueType::Struct("ProcessError".to_string(), Vec::new()),
        ],
        "Ok",
    );
    let err = c_enum_variant_ident(
        "Result",
        &[
            ValueType::Struct("ProcessOutput".to_string(), Vec::new()),
            ValueType::Struct("ProcessError".to_string(), Vec::new()),
        ],
        "Err",
    );
    out.push_str("static ");
    out.push_str(&result);
    out.push_str(" nomo_process_output(nomo_string command) {\n");
    out.push_str("    char *stdout_path = nomo_process_temp_path(\"nomo-stdout\");\n");
    out.push_str("    char *stderr_path = nomo_process_temp_path(\"nomo-stderr\");\n");
    out.push_str("    size_t command_len = strlen(command.data) + strlen(stdout_path) + strlen(stderr_path) + 40;\n");
    out.push_str("    char *wrapped = (char *)malloc(command_len);\n");
    out.push_str("    if (wrapped == NULL) { free(stdout_path); free(stderr_path); nomo_panic(\"out of memory\"); }\n");
    out.push_str("    snprintf(wrapped, command_len, \"(%s) > \\\"%s\\\" 2> \\\"%s\\\"\", command.data, stdout_path, stderr_path);\n");
    out.push_str("    int status = system(wrapped);\n");
    out.push_str("    free(wrapped);\n");
    out.push_str("    if (status == -1) {\n");
    out.push_str("        nomo_string message = nomo_string_from_cstr(strerror(errno));\n");
    out.push_str("        remove(stdout_path); remove(stderr_path);\n");
    out.push_str("        free(stdout_path); free(stderr_path);\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&process_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = message}};\n");
    out.push_str("    }\n");
    out.push_str("    nomo_string stdout_text;\n");
    out.push_str("    nomo_string stderr_text;\n");
    out.push_str("    nomo_string message;\n");
    out.push_str("    if (!nomo_process_read_file(stdout_path, &stdout_text, &message)) {\n");
    out.push_str("        remove(stdout_path); remove(stderr_path);\n");
    out.push_str("        free(stdout_path); free(stderr_path);\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&process_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = message}};\n");
    out.push_str("    }\n");
    out.push_str("    if (!nomo_process_read_file(stderr_path, &stderr_text, &message)) {\n");
    out.push_str("        nomo_string_release(stdout_text);\n");
    out.push_str("        remove(stdout_path); remove(stderr_path);\n");
    out.push_str("        free(stdout_path); free(stderr_path);\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&process_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = message}};\n");
    out.push_str("    }\n");
    out.push_str("    remove(stdout_path); remove(stderr_path);\n");
    out.push_str("    free(stdout_path); free(stderr_path);\n");
    out.push_str("    ");
    out.push_str(&process_output);
    out.push_str(" output = {.");
    out.push_str(&c_member_ident("status"));
    out.push_str(" = nomo_process_exit_code(status), .");
    out.push_str(&c_member_ident("stdout"));
    out.push_str(" = stdout_text, .");
    out.push_str(&c_member_ident("stderr"));
    out.push_str(" = stderr_text};\n");
    out.push_str("    return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&ok);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Ok"));
    out.push_str(" = output};\n");
    out.push_str("}\n");
}

pub(super) fn emit_env_args_helper(out: &mut String) {
    out.push_str("static nomo_array_string nomo_env_args(int argc, char **argv) {\n");
    out.push_str("    nomo_array_string args = nomo_array_string_new();\n");
    out.push_str("    for (int i = 0; i < argc; i += 1) {\n");
    out.push_str("        nomo_string arg = nomo_string_from_cstr(argv[i]);\n");
    out.push_str("        args = nomo_array_string_push(args, arg);\n");
    out.push_str("        nomo_string_release(arg);\n");
    out.push_str("    }\n");
    out.push_str("    return args;\n");
    out.push_str("}\n");
}
