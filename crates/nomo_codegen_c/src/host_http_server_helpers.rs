use super::*;
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
