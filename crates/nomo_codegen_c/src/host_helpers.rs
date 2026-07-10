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
