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
