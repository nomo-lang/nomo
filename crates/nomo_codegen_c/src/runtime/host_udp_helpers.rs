use super::*;
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
