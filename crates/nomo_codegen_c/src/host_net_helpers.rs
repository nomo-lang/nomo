use super::*;

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
