use super::*;

pub(super) fn emit_async_net_connect_windows_iocp_helpers(out: &mut String) {
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
        r#"#include <mswsock.h>

typedef struct {
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
    nomo_async_reactor_deregister(
        &registration->context->reactor,
        &registration->reactor_registration
    );
    nomo_async_timer_disarm(&registration->timer, registration->context);
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
    nomo_async_tcp_connect_finish_operation(registration);
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

static LPFN_CONNECTEX nomo_async_tcp_connect_ex(nomo_socket handle) {
    GUID guid = WSAID_CONNECTEX;
    LPFN_CONNECTEX connect_ex = NULL;
    DWORD received = 0u;
    if (WSAIoctl(
            handle,
            SIO_GET_EXTENSION_FUNCTION_POINTER,
            &guid,
            sizeof(guid),
            &connect_ex,
            sizeof(connect_ex),
            &received,
            NULL,
            NULL
        ) == SOCKET_ERROR) {
        return NULL;
    }
    return connect_ex;
}

static int nomo_async_tcp_bind_any(nomo_socket handle, int family) {
    if (family == AF_INET) {
        struct sockaddr_in local;
        memset(&local, 0, sizeof(local));
        local.sin_family = AF_INET;
        local.sin_addr.s_addr = htonl(INADDR_ANY);
        return bind(
            handle,
            (const struct sockaddr *)&local,
            (int)sizeof(local)
        ) == 0 ? 0 : 1;
    }
    struct sockaddr_in6 local;
    memset(&local, 0, sizeof(local));
    local.sin6_family = AF_INET6;
    local.sin6_addr = in6addr_any;
    return bind(
        handle,
        (const struct sockaddr *)&local,
        (int)sizeof(local)
    ) == 0 ? 0 : 1;
}

"#,
    );

    out.push_str("static nomo_async_poll nomo_async_tcp_connect_start(\n");
    out.push_str(
        "    nomo_async_tcp_connect_registration *registration,\n    nomo_string host,\n    int64_t port,\n    uint64_t timeout_millis,\n    nomo_async_context *context,\n    ",
    );
    out.push_str(&result);
    out.push_str(" *result\n) {\n");
    out.push_str(
        "    context->io_connect_starts += 1u;\n    size_t host_length = strlen(host.data);\n    if (port < 0 || port > 65535 || timeout_millis > 900000u || host_length == 0u || host_length > 253u) {\n        nomo_async_tcp_connect_error(result, (",
    );
    out.push_str(&net_error_kind);
    out.push_str("){.tag = ");
    out.push_str(&invalid_input);
    out.push_str(
        "}, \"invalid TCP host, port, or timeout\");\n        return NOMO_ASYNC_POLL_READY;\n    }\n",
    );
    out.push_str(
        r#"    if (!nomo_net_init()) {
"#,
    );
    out.push_str("        nomo_async_tcp_connect_error(result, (");
    out.push_str(&net_error_kind);
    out.push_str("){.tag = ");
    out.push_str(&connect);
    out.push_str(
        "}, \"Windows network initialization failed\");\n        context->io_errors += 1u;\n        return NOMO_ASYNC_POLL_READY;\n    }\n",
    );
    out.push_str(
        r#"    memset(registration, 0, sizeof(*registration));
    registration->context = context;
    registration->frame = context->current_frame;
    registration->poll = context->current_poll;
    struct sockaddr_storage storage;
    memset(&storage, 0, sizeof(storage));
    int address_length = 0;
    int family = AF_UNSPEC;
    struct sockaddr_in *ipv4 = (struct sockaddr_in *)&storage;
    if (InetPtonA(AF_INET, host.data, &ipv4->sin_addr) == 1) {
        family = AF_INET;
        ipv4->sin_family = AF_INET;
        ipv4->sin_port = htons((u_short)port);
        address_length = (int)sizeof(*ipv4);
    } else {
        struct sockaddr_in6 *ipv6 = (struct sockaddr_in6 *)&storage;
        if (InetPtonA(AF_INET6, host.data, &ipv6->sin6_addr) == 1) {
            family = AF_INET6;
            ipv6->sin6_family = AF_INET6;
            ipv6->sin6_port = htons((u_short)port);
            address_length = (int)sizeof(*ipv6);
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
        "}, \"Windows hostname resolution remains a later P2-TCP-D slice\");\n        return NOMO_ASYNC_POLL_READY;\n    }\n",
    );
    out.push_str(
        r#"    nomo_socket handle = WSASocketW(
        family,
        SOCK_STREAM,
        IPPROTO_TCP,
        NULL,
        0,
        WSA_FLAG_OVERLAPPED
    );
    if (handle == NOMO_INVALID_SOCKET) {
"#,
    );
    out.push_str("        nomo_async_tcp_connect_error(result, (");
    out.push_str(&net_error_kind);
    out.push_str("){.tag = ");
    out.push_str(&connect);
    out.push_str(
        "}, \"TCP socket creation failed\");\n        context->io_errors += 1u;\n        return NOMO_ASYNC_POLL_READY;\n    }\n",
    );
    out.push_str(
        r#"    u_long nonblocking = 1u;
    if (ioctlsocket(handle, FIONBIO, &nonblocking) != 0) {
        NOMO_SOCKET_CLOSE(handle);
"#,
    );
    out.push_str("        nomo_async_tcp_connect_error(result, (");
    out.push_str(&net_error_kind);
    out.push_str("){.tag = ");
    out.push_str(&connect);
    out.push_str(
        "}, \"TCP socket configuration failed\");\n        context->io_errors += 1u;\n        return NOMO_ASYNC_POLL_READY;\n    }\n",
    );
    out.push_str(
        r#"    if (timeout_millis == 0u) {
        int immediate = connect(
            handle,
            (const struct sockaddr *)&storage,
            address_length
        );
        if (immediate == 0) {
            uint32_t slot = 0u;
            uint32_t generation = 0u;
            if (nomo_async_io_handle_insert(
                    context,
                    handle,
                    &slot,
                    &generation
                ) != 0) {
                NOMO_SOCKET_CLOSE(handle);
"#,
    );
    out.push_str("                nomo_async_tcp_connect_error(result, (");
    out.push_str(&net_error_kind);
    out.push_str("){.tag = ");
    out.push_str(&limit);
    out.push_str(
        "}, \"owner executor TCP handle capacity is exhausted\");\n                return NOMO_ASYNC_POLL_READY;\n            }\n            nomo_async_tcp_connect_success(result, context, slot, generation);\n            return NOMO_ASYNC_POLL_READY;\n        }\n        int immediate_error = WSAGetLastError();\n        NOMO_SOCKET_CLOSE(handle);\n        if (immediate_error == WSAEWOULDBLOCK || immediate_error == WSAEINPROGRESS) {\n",
    );
    out.push_str("            nomo_async_tcp_connect_error(result, (");
    out.push_str(&net_error_kind);
    out.push_str("){.tag = ");
    out.push_str(&timeout);
    out.push_str(
        "}, \"TCP connect did not complete immediately\");\n            context->io_timeouts += 1u;\n        } else {\n",
    );
    out.push_str("            nomo_async_tcp_connect_error(result, (");
    out.push_str(&net_error_kind);
    out.push_str("){.tag = ");
    out.push_str(&connect);
    out.push_str(
        "}, \"TCP connect failed\");\n            context->io_errors += 1u;\n        }\n        return NOMO_ASYNC_POLL_READY;\n    }\n",
    );
    out.push_str(
        r#"    nomo_async_poll timer_status = nomo_async_timer_start(
        &registration->timer,
        (int64_t)timeout_millis,
        context,
        &registration->timer_outcome,
        NULL,
        0u
    );
    if (timer_status != NOMO_ASYNC_POLL_PENDING) {
        NOMO_SOCKET_CLOSE(handle);
"#,
    );
    out.push_str("        nomo_async_tcp_connect_error(result, (");
    out.push_str(&net_error_kind);
    out.push_str("){.tag = ");
    out.push_str(&limit);
    out.push_str(
        "}, \"owner executor timer capacity is exhausted\");\n        return NOMO_ASYNC_POLL_READY;\n    }\n",
    );
    out.push_str(
        r#"    if (nomo_async_tcp_bind_any(handle, family) != 0) {
        nomo_async_timer_disarm(&registration->timer, context);
        NOMO_SOCKET_CLOSE(handle);
"#,
    );
    out.push_str("        nomo_async_tcp_connect_error(result, (");
    out.push_str(&net_error_kind);
    out.push_str("){.tag = ");
    out.push_str(&connect);
    out.push_str(
        "}, \"TCP local bind failed\");\n        context->io_errors += 1u;\n        return NOMO_ASYNC_POLL_READY;\n    }\n",
    );
    out.push_str(
        r#"    LPFN_CONNECTEX connect_ex = nomo_async_tcp_connect_ex(handle);
    if (connect_ex == NULL) {
        nomo_async_timer_disarm(&registration->timer, context);
        NOMO_SOCKET_CLOSE(handle);
"#,
    );
    out.push_str("        nomo_async_tcp_connect_error(result, (");
    out.push_str(&net_error_kind);
    out.push_str("){.tag = ");
    out.push_str(&reactor);
    out.push_str(
        "}, \"ConnectEx is not available for this socket\");\n        context->io_errors += 1u;\n        return NOMO_ASYNC_POLL_READY;\n    }\n",
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
        nomo_async_timer_disarm(&registration->timer, context);
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
        r#"    registration->handle_slot = handle_slot;
    registration->handle_generation = handle_generation;
    registration->owns_handle = 1u;
    registration->reactor_registration.owner = registration;
    registration->reactor_registration.wake = nomo_async_tcp_connect_wake;
    if (nomo_async_io_handle_associate_reactor(
            context,
            handle_slot,
            handle_generation
        ) != 0) {
        nomo_async_timer_disarm(&registration->timer, context);
        nomo_async_io_handle_close(context, handle_slot, handle_generation);
        registration->owns_handle = 0u;
"#,
    );
    out.push_str("        nomo_async_tcp_connect_error(result, (");
    out.push_str(&net_error_kind);
    out.push_str("){.tag = ");
    out.push_str(&reactor);
    out.push_str(
        "}, \"IOCP socket association failed\");\n        context->io_errors += 1u;\n        return NOMO_ASYNC_POLL_READY;\n    }\n",
    );
    out.push_str(
        r#"    if (nomo_async_reactor_register(
            &context->reactor,
            &registration->reactor_registration,
            handle,
            NOMO_ASYNC_REACTOR_WRITE
        ) != 0) {
        nomo_async_timer_disarm(&registration->timer, context);
        nomo_async_io_handle_close(context, handle_slot, handle_generation);
        registration->owns_handle = 0u;
"#,
    );
    out.push_str("        nomo_async_tcp_connect_error(result, (");
    out.push_str(&net_error_kind);
    out.push_str("){.tag = ");
    out.push_str(&limit);
    out.push_str(
        "}, \"owner executor IOCP operation capacity is exhausted\");\n        return NOMO_ASYNC_POLL_READY;\n    }\n",
    );
    out.push_str(
        r#"    BOOL started = connect_ex(
        handle,
        (const struct sockaddr *)&storage,
        address_length,
        NULL,
        0u,
        NULL,
        nomo_async_reactor_overlapped(
            &registration->reactor_registration
        )
    );
    int start_error = started != FALSE ? 0 : WSAGetLastError();
    if (started == FALSE && start_error != WSA_IO_PENDING) {
        nomo_async_tcp_connect_finish_operation(registration);
        nomo_async_io_handle_close(context, handle_slot, handle_generation);
        registration->owns_handle = 0u;
"#,
    );
    out.push_str("        nomo_async_tcp_connect_error(result, (");
    out.push_str(&net_error_kind);
    out.push_str("){.tag = ");
    out.push_str(&connect);
    out.push_str(
        "}, \"ConnectEx failed to start\");\n        context->io_errors += 1u;\n        return NOMO_ASYNC_POLL_READY;\n    }\n",
    );
    out.push_str(
        r#"    nomo_async_reactor_mark_submitted(
        &registration->reactor_registration
    );
    registration->active = 1u;
    context->live_io_operations += 1u;
    if (context->live_io_operations > context->peak_live_io_operations) {
        context->peak_live_io_operations = context->live_io_operations;
    }
    context->pending_reason = NOMO_ASYNC_PENDING_IO;
    return NOMO_ASYNC_POLL_PENDING;
}

"#,
    );

    out.push_str("static nomo_async_poll nomo_async_tcp_connect_resume(\n");
    out.push_str(
        "    nomo_async_tcp_connect_registration *registration,\n    nomo_async_context *context,\n    ",
    );
    out.push_str(&result);
    out.push_str(
        r#" *result
) {
    if (registration->timer.armed != 0u
        && nomo_time_monotonic_millis()
            >= registration->timer.deadline_millis) {
        (void)nomo_async_deadline_due(&registration->timer, context);
    }
    if (registration->timer.expired != 0u) {
        registration->timer.expired = 0u;
        nomo_async_tcp_connect_finish_operation(registration);
        if (registration->owns_handle != 0u) {
            nomo_async_io_handle_close(
                context,
                registration->handle_slot,
                registration->handle_generation
            );
            registration->owns_handle = 0u;
        }
"#,
    );
    out.push_str("        nomo_async_tcp_connect_error(result, (");
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
    DWORD completion_error = registration->reactor_registration.error;
    nomo_async_tcp_connect_finish_operation(registration);
    nomo_socket handle = nomo_async_io_handle_get(
        context,
        registration->handle_slot,
        registration->handle_generation
    );
    if (completion_error != ERROR_SUCCESS
        || handle == NOMO_INVALID_SOCKET
        || setsockopt(
            handle,
            SOL_SOCKET,
            SO_UPDATE_CONNECT_CONTEXT,
            NULL,
            0
        ) != 0) {
        if (registration->owns_handle != 0u) {
            nomo_async_io_handle_close(
                context,
                registration->handle_slot,
                registration->handle_generation
            );
            registration->owns_handle = 0u;
        }
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
        r#"    registration->owns_handle = 0u;
    nomo_async_tcp_connect_success(
        result,
        context,
        registration->handle_slot,
        registration->handle_generation
    );
    return NOMO_ASYNC_POLL_READY;
}
"#,
    );
}
