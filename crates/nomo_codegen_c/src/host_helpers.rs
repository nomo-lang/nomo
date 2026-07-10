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
