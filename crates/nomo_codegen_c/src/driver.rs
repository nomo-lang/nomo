use super::*;

pub fn emit_c(program: &Program) -> String {
    let mut out = String::new();
    emit_c_prelude(&mut out);
    emit_operator_runtime(&mut out);
    out.push('\n');
    emit_math_runtime(&mut out);
    out.push('\n');
    emit_string_runtime(&mut out);
    out.push('\n');
    if uses_log_enabled(program) {
        emit_log_enabled_helper(&mut out);
        out.push('\n');
    }

    for const_def in &program.consts {
        out.push_str("#define ");
        out.push_str(&c_var_ident(&const_def.name));
        out.push(' ');
        emit_expr(&mut out, &const_def.initializer);
        out.push('\n');
    }
    if !program.consts.is_empty() {
        out.push('\n');
    }

    emit_function_name_macros(&mut out, program);
    emit_type_name_macros(&mut out, program);

    let array_element_types = collect_array_element_types(program);
    emit_type_forward_declarations(&mut out, program, &array_element_types);
    emit_lifecycle_helper_prototypes(&mut out, program, &array_element_types);
    emit_extern_function_prototypes(&mut out, program);

    for element_type in &array_element_types {
        emit_array_type(&mut out, element_type);
        out.push('\n');
    }
    emit_struct_and_enum_types(&mut out, program);
    emit_nominal_lifecycle_helpers(&mut out, program);
    if uses_hash_builtin(program) {
        emit_hash_helpers(&mut out);
        out.push('\n');
    }
    for element_type in &array_element_types {
        emit_array_helpers(&mut out, element_type);
        out.push('\n');
    }
    if uses_crypto_builtin(program) {
        emit_crypto_helpers(&mut out);
        out.push('\n');
    }
    if uses_collections_builtin(program) {
        emit_collections_helpers(&mut out);
        out.push('\n');
    }
    if array_element_types
        .iter()
        .any(|item| item == &ValueType::String)
    {
        emit_string_split_helper(&mut out);
        out.push('\n');
    }
    if uses_io_read_line(program) {
        emit_io_read_line_helper(&mut out);
        out.push('\n');
    }
    if uses_env_args(program) {
        out.push_str("static int nomo_argc = 0;\n");
        out.push_str("static char **nomo_argv = NULL;\n\n");
        emit_env_args_helper(&mut out);
        out.push('\n');
    }
    if uses_fs_read_to_string(program) {
        emit_fs_read_to_string_helper(&mut out);
        out.push('\n');
    }
    if uses_fs_write_string(program) {
        emit_fs_write_string_helper(&mut out);
        out.push('\n');
    }
    if uses_fs_read_bytes(program) {
        emit_fs_read_bytes_helper(&mut out);
        out.push('\n');
    }
    if uses_fs_write_bytes(program) {
        emit_fs_write_bytes_helper(&mut out);
        out.push('\n');
    }
    if uses_fs_exists(program) {
        emit_fs_exists_helper(&mut out);
        out.push('\n');
    }
    if uses_fs_metadata(program) {
        emit_fs_metadata_helper(&mut out);
        out.push('\n');
    }
    if uses_fs_create_dir(program) {
        emit_fs_create_dir_helper(&mut out);
        out.push('\n');
    }
    if uses_fs_remove_dir(program) {
        emit_fs_remove_dir_helper(&mut out);
        out.push('\n');
    }
    if uses_fs_read_dir(program) {
        emit_fs_read_dir_helper(&mut out);
        out.push('\n');
    }
    if uses_fs_open(program) {
        emit_fs_open_helper(&mut out);
        out.push('\n');
    }
    if uses_file_read_to_string(program) {
        emit_file_read_to_string_helper(&mut out);
        out.push('\n');
    }
    if uses_file_write_string(program) {
        emit_file_write_string_helper(&mut out);
        out.push('\n');
    }
    if uses_file_close(program) {
        emit_file_close_helper(&mut out);
        out.push('\n');
    }
    if uses_net_connect(program)
        || uses_net_listen(program)
        || uses_net_udp_bind(program)
        || uses_http_client(program)
        || uses_http_server(program)
        || uses_tcp_listener_accept(program)
        || uses_tcp_stream_read_to_string(program)
        || uses_tcp_stream_write_string(program)
        || uses_udp_socket_recv_from_string(program)
        || uses_udp_socket_send_to_string(program)
    {
        emit_net_common_helpers(&mut out);
        out.push('\n');
    }
    if uses_net_connect(program) {
        emit_net_connect_helper(&mut out);
        out.push('\n');
    }
    if uses_net_listen(program) {
        emit_net_listen_helper(&mut out);
        out.push('\n');
    }
    if uses_net_udp_bind(program) {
        emit_net_udp_bind_helper(&mut out);
        out.push('\n');
    }
    if uses_tcp_listener_accept(program) {
        emit_tcp_listener_accept_helper(&mut out);
        out.push('\n');
    }
    if uses_tcp_listener_close(program) {
        emit_tcp_listener_close_helper(&mut out);
        out.push('\n');
    }
    if uses_tcp_stream_read_to_string(program) {
        emit_tcp_stream_read_to_string_helper(&mut out);
        out.push('\n');
    }
    if uses_tcp_stream_write_string(program) {
        emit_tcp_stream_write_string_helper(&mut out);
        out.push('\n');
    }
    if uses_tcp_stream_close(program) {
        emit_tcp_stream_close_helper(&mut out);
        out.push('\n');
    }
    if uses_udp_socket_recv_from_string(program) {
        emit_udp_socket_recv_from_string_helper(&mut out);
        out.push('\n');
    }
    if uses_udp_socket_send_to_string(program) {
        emit_udp_socket_send_to_string_helper(&mut out);
        out.push('\n');
    }
    if uses_udp_socket_close(program) {
        emit_udp_socket_close_helper(&mut out);
        out.push('\n');
    }
    if uses_http_client(program) {
        emit_http_client_helpers(&mut out);
        out.push('\n');
    }
    if uses_http_server(program) {
        emit_http_server_helpers(&mut out);
        out.push('\n');
    }
    if uses_env_get(program) {
        emit_env_get_helper(&mut out);
        out.push('\n');
    }
    if uses_env_set(program) {
        emit_env_set_helper(&mut out);
        out.push('\n');
    }
    if uses_env_cwd(program) {
        emit_env_cwd_helper(&mut out);
        out.push('\n');
    }
    if uses_env_home_dir(program) {
        emit_env_home_dir_helper(&mut out);
        out.push('\n');
    }
    if uses_env_temp_dir(program) {
        emit_env_temp_dir_helper(&mut out);
        out.push('\n');
    }
    if uses_process_spawn(program)
        || uses_process_status(program)
        || uses_process_exec(program)
        || uses_process_output(program)
    {
        emit_process_common_helpers(&mut out);
        out.push('\n');
    }
    if uses_process_spawn(program) || uses_process_status(program) {
        emit_process_spawn_helper(&mut out);
        out.push('\n');
    }
    if uses_process_status(program) {
        emit_process_status_helper(&mut out);
        out.push('\n');
    }
    if uses_process_exec(program) {
        emit_process_exec_helper(&mut out);
        out.push('\n');
    }
    if uses_process_output(program) {
        emit_process_output_helper(&mut out);
        out.push('\n');
    }
    if uses_json_builtin(program) {
        emit_json_helpers(&mut out);
        out.push('\n');
    }
    if uses_regex_builtin(program) {
        emit_regex_helpers(&mut out);
        out.push('\n');
    }
    if uses_num_parse_i64(program) {
        emit_num_parse_i64_helper(&mut out);
        out.push('\n');
    }
    if uses_num_parse_u64(program) {
        emit_num_parse_u64_helper(&mut out);
        out.push('\n');
    }
    if uses_num_parse_f64(program) {
        emit_num_parse_f64_helper(&mut out);
        out.push('\n');
    }
    let num_checked_binary_instances = collect_num_checked_binary_instances(program);
    for instance in &num_checked_binary_instances {
        emit_num_checked_binary_helper(&mut out, instance);
        out.push('\n');
    }

    let main = program
        .functions
        .iter()
        .find(|function| function.name == "main")
        .expect("checked programs always contain main");
    let main_returns_result = result_void_error(&main.return_type).is_some();

    for function in program
        .functions
        .iter()
        .filter(|function| function.name != "main" || main_returns_result)
    {
        emit_prototype(&mut out, function);
    }
    if program
        .functions
        .iter()
        .any(|function| function.name != "main" || main_returns_result)
    {
        out.push('\n');
    }

    let result_map_err_instances = collect_result_map_err_instances(program);
    for instance in &result_map_err_instances {
        emit_result_map_err_helper(&mut out, instance);
        out.push('\n');
    }
    let result_unwrap_or_instances = collect_result_unwrap_or_instances(program);
    for instance in &result_unwrap_or_instances {
        emit_result_unwrap_or_helper(&mut out, instance);
        out.push('\n');
    }
    let result_map_instances = collect_result_map_instances(program);
    for instance in &result_map_instances {
        emit_result_map_helper(&mut out, instance);
        out.push('\n');
    }
    let result_and_then_instances = collect_result_and_then_instances(program);
    for instance in &result_and_then_instances {
        emit_result_and_then_helper(&mut out, instance);
        out.push('\n');
    }
    let option_unwrap_or_instances = collect_option_unwrap_or_instances(program);
    for instance in &option_unwrap_or_instances {
        emit_option_unwrap_or_helper(&mut out, instance);
        out.push('\n');
    }
    let option_map_instances = collect_option_map_instances(program);
    for instance in &option_map_instances {
        emit_option_map_helper(&mut out, instance);
        out.push('\n');
    }
    let option_and_then_instances = collect_option_and_then_instances(program);
    for instance in &option_and_then_instances {
        emit_option_and_then_helper(&mut out, instance);
        out.push('\n');
    }

    for function in program
        .functions
        .iter()
        .filter(|function| function.name != "main" || main_returns_result)
    {
        emit_function(&mut out, function);
        out.push('\n');
    }

    if uses_env_args(program) {
        out.push_str("int main(int argc, char **argv) {\n");
    } else {
        out.push_str("int main(void) {\n");
    }
    if uses_env_args(program) {
        out.push_str("    nomo_argc = argc;\n");
        out.push_str("    nomo_argv = argv;\n");
    }
    if let Some(result_args) = result_void_error(&main.return_type) {
        let result_type = c_enum_ident("Result", &result_args);
        out.push_str("    ");
        out.push_str(&result_type);
        out.push_str(" nomo__result = ");
        out.push_str(&c_fn_ident("main"));
        out.push_str("();\n");
        out.push_str("    return nomo__result.tag == ");
        out.push_str(&c_enum_variant_ident("Result", &result_args, "Ok"));
        out.push_str(" ? 0 : 1;\n");
    } else {
        emit_body(&mut out, main);
        out.push_str("    return 0;\n");
    }
    out.push_str("}\n");
    out
}
