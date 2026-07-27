use super::*;

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

pub(super) fn emit_process_control_helpers(out: &mut String) {
    let process_env_type = ValueType::Struct("ProcessEnv".to_string(), Vec::new());
    let process_child_type = ValueType::Struct("BlockingProcessChild".to_string(), Vec::new());
    let process_exit_type = ValueType::Struct("ProcessExit".to_string(), Vec::new());
    let process_event_type = ValueType::Enum("ProcessEvent".to_string(), Vec::new());
    let control_error_type = ValueType::Struct("ProcessControlError".to_string(), Vec::new());
    let exit_option_type = ValueType::Enum("Option".to_string(), vec![process_exit_type.clone()]);
    let start_result_type = ValueType::Enum(
        "Result".to_string(),
        vec![process_child_type.clone(), control_error_type.clone()],
    );
    let void_result_type = ValueType::Enum(
        "Result".to_string(),
        vec![ValueType::Void, control_error_type.clone()],
    );
    let event_result_type = ValueType::Enum(
        "Result".to_string(),
        vec![process_event_type.clone(), control_error_type.clone()],
    );
    let wait_result_type = ValueType::Enum(
        "Result".to_string(),
        vec![exit_option_type.clone(), control_error_type.clone()],
    );

    let rendered = include_str!("host_process_control.c")
        .replace("@PROCESS_ENV@", &c_struct_ident("ProcessEnv", &[]))
        .replace("@PROCESS_COMMAND@", &c_struct_ident("ProcessCommand", &[]))
        .replace(
            "@PROCESS_CHILD@",
            &c_struct_ident("BlockingProcessChild", &[]),
        )
        .replace("@PROCESS_EXIT@", &c_struct_ident("ProcessExit", &[]))
        .replace("@PROCESS_EVENT@", &c_enum_ident("ProcessEvent", &[]))
        .replace(
            "@PROCESS_CONTROL_ERROR@",
            &c_struct_ident("ProcessControlError", &[]),
        )
        .replace("@STRING_ARRAY@", &c_array_ident(&ValueType::String))
        .replace("@ENV_ARRAY@", &c_array_ident(&process_env_type))
        .replace(
            "@CWD_OPTION@",
            &c_enum_ident("Option", &[ValueType::String]),
        )
        .replace("@EXIT_OPTION@", &c_type(&exit_option_type))
        .replace("@START_RESULT@", &c_type(&start_result_type))
        .replace("@VOID_RESULT@", &c_type(&void_result_type))
        .replace("@EVENT_RESULT@", &c_type(&event_result_type))
        .replace("@WAIT_RESULT@", &c_type(&wait_result_type))
        .replace(
            "@START_OK@",
            &c_enum_variant_ident(
                "Result",
                &[process_child_type.clone(), control_error_type.clone()],
                "Ok",
            ),
        )
        .replace(
            "@START_ERR@",
            &c_enum_variant_ident(
                "Result",
                &[process_child_type, control_error_type.clone()],
                "Err",
            ),
        )
        .replace(
            "@VOID_OK@",
            &c_enum_variant_ident(
                "Result",
                &[ValueType::Void, control_error_type.clone()],
                "Ok",
            ),
        )
        .replace(
            "@VOID_ERR@",
            &c_enum_variant_ident(
                "Result",
                &[ValueType::Void, control_error_type.clone()],
                "Err",
            ),
        )
        .replace(
            "@EVENT_OK@",
            &c_enum_variant_ident(
                "Result",
                &[process_event_type.clone(), control_error_type.clone()],
                "Ok",
            ),
        )
        .replace(
            "@EVENT_ERR@",
            &c_enum_variant_ident(
                "Result",
                &[process_event_type, control_error_type.clone()],
                "Err",
            ),
        )
        .replace(
            "@WAIT_OK@",
            &c_enum_variant_ident(
                "Result",
                &[exit_option_type.clone(), control_error_type.clone()],
                "Ok",
            ),
        )
        .replace(
            "@WAIT_ERR@",
            &c_enum_variant_ident("Result", &[exit_option_type, control_error_type], "Err"),
        )
        .replace(
            "@CWD_SOME@",
            &c_enum_variant_ident("Option", &[ValueType::String], "Some"),
        )
        .replace(
            "@CWD_NONE@",
            &c_enum_variant_ident("Option", &[ValueType::String], "None"),
        )
        .replace(
            "@EXIT_SOME@",
            &c_enum_variant_ident("Option", &[process_exit_type.clone()], "Some"),
        )
        .replace(
            "@EXIT_NONE@",
            &c_enum_variant_ident("Option", &[process_exit_type], "None"),
        )
        .replace(
            "@EVENT_STDIN_FLUSHED@",
            &c_enum_variant_ident("ProcessEvent", &[], "StdinFlushed"),
        )
        .replace(
            "@EVENT_STDOUT@",
            &c_enum_variant_ident("ProcessEvent", &[], "Stdout"),
        )
        .replace(
            "@EVENT_STDERR@",
            &c_enum_variant_ident("ProcessEvent", &[], "Stderr"),
        )
        .replace(
            "@EVENT_EXITED@",
            &c_enum_variant_ident("ProcessEvent", &[], "Exited"),
        )
        .replace("@OK_PAYLOAD@", &c_payload_ident("Ok"))
        .replace("@ERR_PAYLOAD@", &c_payload_ident("Err"))
        .replace("@SOME_PAYLOAD@", &c_payload_ident("Some"))
        .replace("@STDOUT_PAYLOAD@", &c_payload_ident("Stdout"))
        .replace("@STDERR_PAYLOAD@", &c_payload_ident("Stderr"))
        .replace("@EXITED_PAYLOAD@", &c_payload_ident("Exited"))
        .replace("@PROGRAM_MEMBER@", &c_member_ident("program"))
        .replace("@ARGS_MEMBER@", &c_member_ident("args"))
        .replace("@CWD_MEMBER@", &c_member_ident("cwd"))
        .replace("@ENV_MEMBER@", &c_member_ident("env"))
        .replace("@INHERIT_ENV_MEMBER@", &c_member_ident("inherit_env"))
        .replace("@NAME_MEMBER@", &c_member_ident("name"))
        .replace("@VALUE_MEMBER@", &c_member_ident("value"))
        .replace("@HANDLE_MEMBER@", &c_member_ident("handle"))
        .replace("@CODE_MEMBER@", &c_member_ident("code"))
        .replace("@SIGNAL_MEMBER@", &c_member_ident("signal"))
        .replace("@MESSAGE_MEMBER@", &c_member_ident("message"))
        .replace(
            "@START_NAME@",
            &c_fn_ident(BUILTIN_PROCESS_START_BLOCKING_EXPR),
        )
        .replace(
            "@WRITE_STDIN_NAME@",
            &c_fn_ident(BUILTIN_PROCESS_WRITE_STDIN_BLOCKING_EXPR),
        )
        .replace(
            "@CLOSE_STDIN_NAME@",
            &c_fn_ident(BUILTIN_PROCESS_CLOSE_STDIN_BLOCKING_EXPR),
        )
        .replace(
            "@NEXT_EVENT_NAME@",
            &c_fn_ident(BUILTIN_PROCESS_NEXT_EVENT_BLOCKING_EXPR),
        )
        .replace(
            "@TRY_WAIT_NAME@",
            &c_fn_ident(BUILTIN_PROCESS_TRY_WAIT_BLOCKING_EXPR),
        )
        .replace(
            "@TERMINATE_NAME@",
            &c_fn_ident(BUILTIN_PROCESS_TERMINATE_BLOCKING_EXPR),
        )
        .replace(
            "@CLOSE_CHILD_NAME@",
            &c_fn_ident(BUILTIN_PROCESS_CLOSE_CHILD_BLOCKING_EXPR),
        );
    out.push_str(&rendered);
}
