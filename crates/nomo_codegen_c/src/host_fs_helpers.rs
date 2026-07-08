use super::*;

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
