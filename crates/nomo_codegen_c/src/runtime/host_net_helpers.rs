use super::*;

fn emit_blocking_net_error_kind(out: &mut String, variant: &str) {
    out.push_str(&c_member_ident("kind"));
    out.push_str(" = (");
    out.push_str(&c_enum_ident("NetErrorKind", &[]));
    out.push_str("){.tag = ");
    out.push_str(&c_enum_variant_ident("NetErrorKind", &[], variant));
    out.push_str("}, .");
}

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

pub(super) fn emit_async_net_connect_windows_preview_helpers(
    out: &mut String,
    target: &nomo_target::TargetTriple,
) {
    let tcp_stream = c_struct_ident("TcpStream", &[]);
    let net_error_kind = c_enum_ident("NetErrorKind", &[]);
    let result_args = [
        ValueType::Struct("TcpStream".to_string(), Vec::new()),
        ValueType::Struct("NetError".to_string(), Vec::new()),
    ];
    let result = c_enum_ident("Result", &result_args);
    let ok = c_enum_variant_ident("Result", &result_args, "Ok");
    let err = c_enum_variant_ident("Result", &result_args, "Err");
    let invalid_input = c_enum_variant_ident("NetErrorKind", &[], "InvalidInput");
    let unsupported = c_enum_variant_ident("NetErrorKind", &[], "Unsupported");
    let timeout = c_enum_variant_ident("NetErrorKind", &[], "Timeout");
    let limit = c_enum_variant_ident("NetErrorKind", &[], "Limit");
    let connect = c_enum_variant_ident("NetErrorKind", &[], "Connect");
    let reactor = c_enum_variant_ident("NetErrorKind", &[], "Reactor");
    let kind_member = c_member_ident("kind");
    let message_member = c_member_ident("message");
    let handle_member = c_member_ident("handle");
    let owner_member = c_member_ident("owner");
    let close_fn_member = c_member_ident("close_fn");
    let slot_member = c_member_ident("slot");
    let generation_member = c_member_ident("generation");
    let ok_payload = c_payload_ident("Ok");
    let err_payload = c_payload_ident("Err");

    out.push_str(
        r#"typedef struct {
    nomo_async_reactor_registration reactor_registration;
    nomo_async_timer_registration timer;
    nomo_async_timer_outcome timer_outcome;
    nomo_async_context *context;
    void *frame;
    nomo_async_poll_fn poll;
    uint32_t handle_slot;
    uint32_t handle_generation;
    uint8_t active;
    uint8_t ready;
    uint8_t owns_handle;
} nomo_async_tcp_connect_registration;

"#,
    );
    out.push_str("static void nomo_async_tcp_connect_error(\n    ");
    out.push_str(&result);
    out.push_str(" *result,\n    ");
    out.push_str(&net_error_kind);
    out.push_str(" kind,\n    const char *message\n) {\n");
    out.push_str("    memset(result, 0, sizeof(*result));\n    result->tag = ");
    out.push_str(&err);
    out.push_str(";\n    result->payload.");
    out.push_str(&err_payload);
    out.push('.');
    out.push_str(&kind_member);
    out.push_str(" = kind;\n    result->payload.");
    out.push_str(&err_payload);
    out.push('.');
    out.push_str(&message_member);
    out.push_str(" = nomo_string_literal(message);\n}\n\n");

    out.push_str("static void nomo_async_tcp_connect_success(\n    ");
    out.push_str(&result);
    out.push_str(
        " *result,\n    nomo_async_context *context,\n    uint32_t slot,\n    uint32_t generation\n) {\n",
    );
    out.push_str("    memset(result, 0, sizeof(*result));\n    result->tag = ");
    out.push_str(&ok);
    out.push_str(";\n    result->payload.");
    out.push_str(&ok_payload);
    out.push_str(" = (");
    out.push_str(&tcp_stream);
    out.push_str("){.");
    out.push_str(&handle_member);
    out.push_str(" = NOMO_INVALID_SOCKET, .");
    out.push_str(&owner_member);
    out.push_str(" = context, .");
    out.push_str(&close_fn_member);
    out.push_str(" = nomo_async_io_handle_close_callback, .");
    out.push_str(&slot_member);
    out.push_str(" = slot, .");
    out.push_str(&generation_member);
    out.push_str(" = generation};\n}\n\n");

    out.push_str(
        r#"static void nomo_async_tcp_connect_finish_operation(
    nomo_async_tcp_connect_registration *registration
) {
    if (registration->active == 0u) {
        return;
    }
    registration->active = 0u;
    if (registration->context->live_io_operations > 0u) {
        registration->context->live_io_operations -= 1u;
    }
}

static void nomo_async_tcp_connect_wake(void *owner, uint32_t ready) {
    nomo_async_tcp_connect_registration *registration =
        (nomo_async_tcp_connect_registration *)owner;
    if (registration == NULL
        || registration->active == 0u
        || (ready & NOMO_ASYNC_REACTOR_WRITE) == 0u) {
        return;
    }
    nomo_async_reactor_deregister(
        &registration->context->reactor,
        &registration->reactor_registration
    );
    nomo_async_timer_disarm(&registration->timer, registration->context);
    nomo_async_tcp_connect_finish_operation(registration);
    registration->ready = 1u;
    registration->context->io_ready_completions += 1u;
    if (nomo_async_ready_enqueue(
            registration->context,
            registration->frame,
            registration->poll
        ) != 0) {
        registration->context->runtime_failed = 1u;
    }
}

static void nomo_async_tcp_connect_cancel(
    nomo_async_tcp_connect_registration *registration,
    nomo_async_context *context
) {
    if (registration->active == 0u && registration->owns_handle == 0u) {
        return;
    }
    if (registration->active != 0u) {
        nomo_async_reactor_deregister(
            &context->reactor,
            &registration->reactor_registration
        );
        nomo_async_timer_disarm(&registration->timer, context);
        nomo_async_tcp_connect_finish_operation(registration);
    }
    if (registration->owns_handle != 0u) {
        nomo_async_io_handle_close(
            context,
            registration->handle_slot,
            registration->handle_generation
        );
        registration->owns_handle = 0u;
    }
    context->io_cancellations += 1u;
}

"#,
    );

    if target.operating_system() == nomo_target::OperatingSystem::Windows {
        out.push_str("static nomo_async_poll nomo_async_tcp_connect_start(\n");
        out.push_str(
            "    nomo_async_tcp_connect_registration *registration,\n    nomo_string host,\n    int64_t port,\n    uint64_t timeout_millis,\n    nomo_async_context *context,\n    ",
        );
        out.push_str(&result);
        out.push_str(" *result\n) {\n");
        out.push_str(
            "    (void)registration;\n    (void)host;\n    (void)port;\n    (void)timeout_millis;\n    context->io_connect_starts += 1u;\n    nomo_async_tcp_connect_error(result, (",
        );
        out.push_str(&net_error_kind);
        out.push_str("){.tag = ");
        out.push_str(&unsupported);
        out.push_str(
            "}, \"async TCP connect is not available on the Windows preview backend\");\n    return NOMO_ASYNC_POLL_READY;\n}\n\n",
        );
    } else {
        out.push_str("static nomo_async_poll nomo_async_tcp_connect_start(\n");
        out.push_str(
            "    nomo_async_tcp_connect_registration *registration,\n    nomo_string host,\n    int64_t port,\n    uint64_t timeout_millis,\n    nomo_async_context *context,\n    ",
        );
        out.push_str(&result);
        out.push_str(" *result\n) {\n");
        out.push_str("    context->io_connect_starts += 1u;\n");
        out.push_str("    if (port < 0 || port > 65535 || timeout_millis > 900000u) {\n");
        out.push_str("        nomo_async_tcp_connect_error(result, (");
        out.push_str(&net_error_kind);
        out.push_str("){.tag = ");
        out.push_str(&invalid_input);
        out.push_str(
            "}, \"invalid TCP port or timeout\");\n        return NOMO_ASYNC_POLL_READY;\n    }\n",
        );
        out.push_str(
            r#"    struct sockaddr_storage storage;
    memset(&storage, 0, sizeof(storage));
    socklen_t address_length = 0;
    int family = AF_UNSPEC;
    struct sockaddr_in *ipv4 = (struct sockaddr_in *)&storage;
    if (inet_pton(AF_INET, host.data, &ipv4->sin_addr) == 1) {
        family = AF_INET;
        ipv4->sin_family = AF_INET;
        ipv4->sin_port = htons((uint16_t)port);
        address_length = (socklen_t)sizeof(*ipv4);
    } else {
        struct sockaddr_in6 *ipv6 = (struct sockaddr_in6 *)&storage;
        if (inet_pton(AF_INET6, host.data, &ipv6->sin6_addr) == 1) {
            family = AF_INET6;
            ipv6->sin6_family = AF_INET6;
            ipv6->sin6_port = htons((uint16_t)port);
            address_length = (socklen_t)sizeof(*ipv6);
        }
    }
    if (family == AF_UNSPEC) {
"#,
        );
        out.push_str("        nomo_async_tcp_connect_error(result, (");
        out.push_str(&net_error_kind);
        out.push_str("){.tag = ");
        out.push_str(&unsupported);
        out.push_str(
            "}, \"hostnames require the bounded resolver slice; use a numeric address\");\n        return NOMO_ASYNC_POLL_READY;\n    }\n",
        );
        out.push_str(
            r#"    nomo_socket handle = socket(family, SOCK_STREAM, 0);
    if (handle == NOMO_INVALID_SOCKET) {
"#,
        );
        out.push_str("        nomo_async_tcp_connect_error(result, (");
        out.push_str(&net_error_kind);
        out.push_str("){.tag = ");
        out.push_str(&connect);
        out.push_str(
            "}, \"could not create TCP socket\");\n        context->io_errors += 1u;\n        return NOMO_ASYNC_POLL_READY;\n    }\n",
        );
        out.push_str(
            r#"    int flags = fcntl(handle, F_GETFL, 0);
    if (flags < 0
        || fcntl(handle, F_SETFL, flags | O_NONBLOCK) != 0
        || fcntl(handle, F_SETFD, FD_CLOEXEC) != 0) {
        NOMO_SOCKET_CLOSE(handle);
"#,
        );
        out.push_str("        nomo_async_tcp_connect_error(result, (");
        out.push_str(&net_error_kind);
        out.push_str("){.tag = ");
        out.push_str(&connect);
        out.push_str(
            "}, \"could not configure nonblocking TCP socket\");\n        context->io_errors += 1u;\n        return NOMO_ASYNC_POLL_READY;\n    }\n",
        );
        if target.operating_system() == nomo_target::OperatingSystem::Darwin {
            out.push_str(
                r#"#ifndef SO_NOSIGPIPE
#define SO_NOSIGPIPE 0x1022
#endif
    int no_sigpipe = 1;
    if (setsockopt(
            handle,
            SOL_SOCKET,
            SO_NOSIGPIPE,
            &no_sigpipe,
            (socklen_t)sizeof(no_sigpipe)
        ) != 0) {
        NOMO_SOCKET_CLOSE(handle);
"#,
            );
            out.push_str("        nomo_async_tcp_connect_error(result, (");
            out.push_str(&net_error_kind);
            out.push_str("){.tag = ");
            out.push_str(&connect);
            out.push_str(
                "}, \"could not configure safe TCP writes\");\n        context->io_errors += 1u;\n        return NOMO_ASYNC_POLL_READY;\n    }\n",
            );
        }
        out.push_str(
            r#"    int connect_status = connect(
        handle,
        (const struct sockaddr *)&storage,
        address_length
    );
    if (connect_status != 0 && errno != EINPROGRESS) {
        NOMO_SOCKET_CLOSE(handle);
"#,
        );
        out.push_str("        nomo_async_tcp_connect_error(result, (");
        out.push_str(&net_error_kind);
        out.push_str("){.tag = ");
        out.push_str(&connect);
        out.push_str(
            "}, \"TCP connect failed\");\n        context->io_errors += 1u;\n        return NOMO_ASYNC_POLL_READY;\n    }\n",
        );
        out.push_str(
            r#"    uint32_t handle_slot = 0u;
    uint32_t handle_generation = 0u;
    if (nomo_async_io_handle_insert(
            context,
            handle,
            &handle_slot,
            &handle_generation
        ) != 0) {
        NOMO_SOCKET_CLOSE(handle);
"#,
        );
        out.push_str("        nomo_async_tcp_connect_error(result, (");
        out.push_str(&net_error_kind);
        out.push_str("){.tag = ");
        out.push_str(&limit);
        out.push_str(
            "}, \"owner executor TCP handle capacity is exhausted\");\n        return NOMO_ASYNC_POLL_READY;\n    }\n",
        );
        out.push_str(
            "    if (connect_status == 0) {\n        nomo_async_tcp_connect_success(result, context, handle_slot, handle_generation);\n        return NOMO_ASYNC_POLL_READY;\n    }\n",
        );
        out.push_str("    if (timeout_millis == 0u) {\n        nomo_async_io_handle_close(context, handle_slot, handle_generation);\n        nomo_async_tcp_connect_error(result, (");
        out.push_str(&net_error_kind);
        out.push_str("){.tag = ");
        out.push_str(&timeout);
        out.push_str(
            "}, \"TCP connect did not complete immediately\");\n        context->io_timeouts += 1u;\n        return NOMO_ASYNC_POLL_READY;\n    }\n",
        );
        out.push_str(
            r#"    memset(registration, 0, sizeof(*registration));
    registration->context = context;
    registration->frame = context->current_frame;
    registration->poll = context->current_poll;
    registration->handle_slot = handle_slot;
    registration->handle_generation = handle_generation;
    registration->owns_handle = 1u;
    registration->reactor_registration.owner = registration;
    registration->reactor_registration.wake = nomo_async_tcp_connect_wake;
    if (nomo_async_reactor_register(
            &context->reactor,
            &registration->reactor_registration,
            handle,
            NOMO_ASYNC_REACTOR_WRITE
        ) != 0) {
        nomo_async_io_handle_close(context, handle_slot, handle_generation);
        registration->owns_handle = 0u;
"#,
        );
        out.push_str("        nomo_async_tcp_connect_error(result, (");
        out.push_str(&net_error_kind);
        out.push_str("){.tag = ");
        out.push_str(&reactor);
        out.push_str(
            "}, \"reactor registration failed\");\n        context->io_errors += 1u;\n        return NOMO_ASYNC_POLL_READY;\n    }\n",
        );
        out.push_str(
            r#"    registration->active = 1u;
    context->live_io_operations += 1u;
    if (context->live_io_operations > context->peak_live_io_operations) {
        context->peak_live_io_operations = context->live_io_operations;
    }
    nomo_async_poll timer_status = nomo_async_timer_start(
        &registration->timer,
        (int64_t)timeout_millis,
        context,
        &registration->timer_outcome,
        NULL,
        0u
    );
    if (timer_status != NOMO_ASYNC_POLL_PENDING) {
        nomo_async_reactor_deregister(
            &context->reactor,
            &registration->reactor_registration
        );
        nomo_async_tcp_connect_finish_operation(registration);
        nomo_async_io_handle_close(context, handle_slot, handle_generation);
        registration->owns_handle = 0u;
"#,
        );
        out.push_str("        nomo_async_tcp_connect_error(result, (");
        out.push_str(&net_error_kind);
        out.push_str("){.tag = ");
        out.push_str(&limit);
        out.push_str(
            "}, \"owner executor timer capacity is exhausted\");\n        return NOMO_ASYNC_POLL_READY;\n    }\n    context->pending_reason = NOMO_ASYNC_PENDING_IO;\n    return NOMO_ASYNC_POLL_PENDING;\n}\n\n",
        );
    }

    out.push_str("static nomo_async_poll nomo_async_tcp_connect_resume(\n");
    out.push_str(
        "    nomo_async_tcp_connect_registration *registration,\n    nomo_async_context *context,\n    ",
    );
    out.push_str(&result);
    out.push_str(" *result\n) {\n");
    out.push_str(
        "    if (registration->active == 0u && registration->ready == 0u && registration->timer.expired == 0u) {\n        return NOMO_ASYNC_POLL_READY;\n    }\n",
    );
    out.push_str("    if (registration->timer.expired != 0u) {\n");
    out.push_str(
        "        registration->timer.expired = 0u;\n        nomo_async_reactor_deregister(&context->reactor, &registration->reactor_registration);\n        nomo_async_tcp_connect_finish_operation(registration);\n        nomo_async_io_handle_close(context, registration->handle_slot, registration->handle_generation);\n        registration->owns_handle = 0u;\n        nomo_async_tcp_connect_error(result, (",
    );
    out.push_str(&net_error_kind);
    out.push_str("){.tag = ");
    out.push_str(&timeout);
    out.push_str(
        "}, \"TCP connect timed out\");\n        context->io_timeouts += 1u;\n        return NOMO_ASYNC_POLL_READY;\n    }\n",
    );
    out.push_str(
        r#"    if (registration->ready == 0u) {
        context->pending_reason = NOMO_ASYNC_PENDING_IO;
        return NOMO_ASYNC_POLL_PENDING;
    }
    registration->ready = 0u;
    nomo_socket handle = nomo_async_io_handle_get(
        context,
        registration->handle_slot,
        registration->handle_generation
    );
    if (handle == NOMO_INVALID_SOCKET) {
"#,
    );
    out.push_str("        nomo_async_tcp_connect_error(result, (");
    out.push_str(&net_error_kind);
    out.push_str("){.tag = ");
    out.push_str(&connect);
    out.push_str(
        "}, \"TCP handle closed before connect completion\");\n        registration->owns_handle = 0u;\n        context->io_errors += 1u;\n        return NOMO_ASYNC_POLL_READY;\n    }\n",
    );
    if target.operating_system() == nomo_target::OperatingSystem::Windows {
        out.push_str("    int socket_error = WSAEOPNOTSUPP;\n");
    } else {
        out.push_str(
            "    int socket_error = 0;\n    socklen_t socket_error_length = (socklen_t)sizeof(socket_error);\n    if (getsockopt(handle, SOL_SOCKET, SO_ERROR, &socket_error, &socket_error_length) != 0) {\n        socket_error = errno == 0 ? EIO : errno;\n    }\n",
        );
    }
    out.push_str("    if (socket_error != 0) {\n        nomo_async_io_handle_close(context, registration->handle_slot, registration->handle_generation);\n        registration->owns_handle = 0u;\n        nomo_async_tcp_connect_error(result, (");
    out.push_str(&net_error_kind);
    out.push_str("){.tag = ");
    out.push_str(&connect);
    out.push_str(
        "}, \"TCP connect failed\");\n        context->io_errors += 1u;\n        return NOMO_ASYNC_POLL_READY;\n    }\n    registration->owns_handle = 0u;\n    nomo_async_tcp_connect_success(result, context, registration->handle_slot, registration->handle_generation);\n    return NOMO_ASYNC_POLL_READY;\n}\n",
    );
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
    emit_blocking_net_error_kind(out, "Unsupported");
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
    emit_blocking_net_error_kind(out, "InvalidInput");
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
    emit_blocking_net_error_kind(out, "Resolve");
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
    emit_blocking_net_error_kind(out, "Connect");
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
    emit_blocking_net_error_kind(out, "Unsupported");
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
    emit_blocking_net_error_kind(out, "InvalidInput");
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
    emit_blocking_net_error_kind(out, "Resolve");
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
    emit_blocking_net_error_kind(out, "Connect");
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
    emit_blocking_net_error_kind(out, "Closed");
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
    emit_blocking_net_error_kind(out, "Connect");
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
    emit_blocking_net_error_kind(out, "Closed");
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
    emit_blocking_net_error_kind(out, "Read");
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
    emit_blocking_net_error_kind(out, "Closed");
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
    emit_blocking_net_error_kind(out, "Write");
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
    out.push_str(&c_member_ident("close_fn"));
    out.push_str(" != NULL && stream.");
    out.push_str(&c_member_ident("owner"));
    out.push_str(" != NULL) {\n");
    out.push_str("        stream.");
    out.push_str(&c_member_ident("close_fn"));
    out.push_str("(stream.");
    out.push_str(&c_member_ident("owner"));
    out.push_str(", stream.");
    out.push_str(&c_member_ident("slot"));
    out.push_str(", stream.");
    out.push_str(&c_member_ident("generation"));
    out.push_str(");\n        return;\n    }\n");
    out.push_str("    if (stream.");
    out.push_str(&c_member_ident("handle"));
    out.push_str(" != NOMO_INVALID_SOCKET) {\n");
    out.push_str("        NOMO_SOCKET_CLOSE(stream.");
    out.push_str(&c_member_ident("handle"));
    out.push_str(");\n");
    out.push_str("    }\n");
    out.push_str("}\n");
}
