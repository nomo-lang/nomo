pub(super) fn emit_path_runtime(out: &mut String) {
    out.push_str("\nstatic nomo_string nomo_path_string_from_slice(const char *data, size_t start, size_t len) {\n");
    out.push_str("    return nomo_string_from_slice(data, start, len);\n");
    out.push_str("}\n\n");
    out.push_str("static size_t nomo_path_trim_trailing_slashes(const char *data) {\n");
    out.push_str("    size_t len = strlen(data);\n");
    out.push_str("    while (len > 1 && data[len - 1] == '/') { len -= 1; }\n");
    out.push_str("    return len;\n");
    out.push_str("}\n\n");
    out.push_str("static nomo_string nomo_path_join(nomo_string left, nomo_string right) {\n");
    out.push_str("    if (right.data[0] == '/' || left.data[0] == '\\0') {\n");
    out.push_str("        return nomo_string_from_cstr(right.data);\n");
    out.push_str("    }\n");
    out.push_str("    if (right.data[0] == '\\0') {\n");
    out.push_str("        return nomo_string_from_cstr(left.data);\n");
    out.push_str("    }\n");
    out.push_str("    size_t left_len = strlen(left.data);\n");
    out.push_str("    size_t right_len = strlen(right.data);\n");
    out.push_str("    int needs_sep = left.data[left_len - 1] != '/';\n");
    out.push_str("    char *out = (char *)malloc(left_len + (size_t)needs_sep + right_len + 1);\n");
    out.push_str("    if (out == NULL) { nomo_panic(\"out of memory\"); }\n");
    out.push_str("    memcpy(out, left.data, left_len);\n");
    out.push_str("    size_t offset = left_len;\n");
    out.push_str("    if (needs_sep) { out[offset] = '/'; offset += 1; }\n");
    out.push_str("    memcpy(out + offset, right.data, right_len + 1);\n");
    out.push_str("    return nomo_string_owned(out);\n");
    out.push_str("}\n\n");
    out.push_str("static nomo_string nomo_path_basename(nomo_string path) {\n");
    out.push_str("    size_t len = nomo_path_trim_trailing_slashes(path.data);\n");
    out.push_str("    if (len == 0) { return nomo_string_literal(\"\"); }\n");
    out.push_str(
        "    if (len == 1 && path.data[0] == '/') { return nomo_string_literal(\"/\"); }\n",
    );
    out.push_str("    size_t start = len;\n");
    out.push_str("    while (start > 0 && path.data[start - 1] != '/') { start -= 1; }\n");
    out.push_str("    return nomo_path_string_from_slice(path.data, start, len - start);\n");
    out.push_str("}\n\n");
    out.push_str("static nomo_string nomo_path_dirname(nomo_string path) {\n");
    out.push_str("    size_t len = nomo_path_trim_trailing_slashes(path.data);\n");
    out.push_str("    if (len == 0) { return nomo_string_literal(\".\"); }\n");
    out.push_str(
        "    if (len == 1 && path.data[0] == '/') { return nomo_string_literal(\"/\"); }\n",
    );
    out.push_str("    size_t slash = len;\n");
    out.push_str("    while (slash > 0 && path.data[slash - 1] != '/') { slash -= 1; }\n");
    out.push_str("    if (slash == 0) { return nomo_string_literal(\".\"); }\n");
    out.push_str("    while (slash > 1 && path.data[slash - 1] == '/') { slash -= 1; }\n");
    out.push_str("    return nomo_path_string_from_slice(path.data, 0, slash);\n");
    out.push_str("}\n\n");
    out.push_str("static nomo_string nomo_path_extension(nomo_string path) {\n");
    out.push_str("    size_t len = nomo_path_trim_trailing_slashes(path.data);\n");
    out.push_str("    size_t start = len;\n");
    out.push_str("    while (start > 0 && path.data[start - 1] != '/') { start -= 1; }\n");
    out.push_str("    size_t dot = len;\n");
    out.push_str("    while (dot > start && path.data[dot - 1] != '.') { dot -= 1; }\n");
    out.push_str("    if (dot == start || dot == len) { return nomo_string_literal(\"\"); }\n");
    out.push_str("    return nomo_path_string_from_slice(path.data, dot, len - dot);\n");
    out.push_str("}\n\n");
    out.push_str("static int nomo_path_is_absolute(nomo_string path) {\n");
    out.push_str("    return path.data[0] == '/';\n");
    out.push_str("}\n\n");
    out.push_str("static int nomo_path_prev_segment_is_dotdot(char *out, size_t *starts, size_t *lens, size_t count) {\n");
    out.push_str("    if (count == 0) { return 0; }\n");
    out.push_str("    size_t start = starts[count - 1];\n");
    out.push_str("    if (out[start] == '/') { start += 1; }\n");
    out.push_str(
        "    return lens[count - 1] == 2 && out[start] == '.' && out[start + 1] == '.';\n",
    );
    out.push_str("}\n\n");
    out.push_str("static void nomo_path_append_segment(char *out, size_t *out_len, size_t *starts, size_t *lens, size_t *count, const char *segment, size_t segment_len) {\n");
    out.push_str("    size_t restore = *out_len;\n");
    out.push_str("    if (*out_len > 0 && out[*out_len - 1] != '/') { out[*out_len] = '/'; *out_len += 1; }\n");
    out.push_str("    starts[*count] = restore;\n");
    out.push_str("    lens[*count] = segment_len;\n");
    out.push_str("    *count += 1;\n");
    out.push_str("    memcpy(out + *out_len, segment, segment_len);\n");
    out.push_str("    *out_len += segment_len;\n");
    out.push_str("    (void)restore;\n");
    out.push_str("}\n\n");
    out.push_str("static nomo_string nomo_path_normalize(nomo_string path) {\n");
    out.push_str("    const char *data = path.data;\n");
    out.push_str("    size_t len = strlen(data);\n");
    out.push_str("    if (len == 0) { return nomo_string_literal(\".\"); }\n");
    out.push_str("    int absolute = data[0] == '/';\n");
    out.push_str("    char *out = (char *)malloc(len + 2);\n");
    out.push_str("    size_t *starts = (size_t *)malloc((len + 1) * sizeof(size_t));\n");
    out.push_str("    size_t *lens = (size_t *)malloc((len + 1) * sizeof(size_t));\n");
    out.push_str("    if (out == NULL || starts == NULL || lens == NULL) {\n");
    out.push_str("        free(out); free(starts); free(lens);\n");
    out.push_str("        nomo_panic(\"out of memory\");\n");
    out.push_str("    }\n");
    out.push_str("    size_t out_len = 0;\n");
    out.push_str("    size_t count = 0;\n");
    out.push_str("    if (absolute) { out[out_len] = '/'; out_len += 1; }\n");
    out.push_str("    size_t index = 0;\n");
    out.push_str("    while (index < len) {\n");
    out.push_str("        while (index < len && data[index] == '/') { index += 1; }\n");
    out.push_str("        size_t start = index;\n");
    out.push_str("        while (index < len && data[index] != '/') { index += 1; }\n");
    out.push_str("        size_t segment_len = index - start;\n");
    out.push_str(
        "        if (segment_len == 0 || (segment_len == 1 && data[start] == '.')) { continue; }\n",
    );
    out.push_str(
        "        if (segment_len == 2 && data[start] == '.' && data[start + 1] == '.') {\n",
    );
    out.push_str("            if (count > 0 && !nomo_path_prev_segment_is_dotdot(out, starts, lens, count)) {\n");
    out.push_str("                count -= 1;\n");
    out.push_str("                out_len = starts[count];\n");
    out.push_str(
        "                if (absolute && out_len == 0) { out[out_len] = '/'; out_len += 1; }\n",
    );
    out.push_str("            } else if (!absolute) {\n");
    out.push_str("                nomo_path_append_segment(out, &out_len, starts, lens, &count, data + start, segment_len);\n");
    out.push_str("            }\n");
    out.push_str("        } else {\n");
    out.push_str("            nomo_path_append_segment(out, &out_len, starts, lens, &count, data + start, segment_len);\n");
    out.push_str("        }\n");
    out.push_str("    }\n");
    out.push_str("    if (out_len == 0) { out[out_len] = '.'; out_len += 1; }\n");
    out.push_str("    out[out_len] = '\\0';\n");
    out.push_str("    free(starts); free(lens);\n");
    out.push_str("    return nomo_string_owned(out);\n");
    out.push_str("}\n");
}
