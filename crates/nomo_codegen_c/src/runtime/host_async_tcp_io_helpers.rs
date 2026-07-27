use super::*;

pub(super) fn emit_async_tcp_io_helpers(out: &mut String, target: &nomo_target::TargetTriple) {
    let net_error = ValueType::Struct("NetError".to_string(), Vec::new());
    let read_result_args = [
        ValueType::Struct("TcpChunk".to_string(), Vec::new()),
        net_error.clone(),
    ];
    let text_result_args = [
        ValueType::Struct("TcpTextChunk".to_string(), Vec::new()),
        net_error.clone(),
    ];
    let write_result_args = [ValueType::Void, net_error];

    out.push_str("typedef ");
    out.push_str(&c_struct_ident("TcpStream", &[]));
    out.push_str(" nomo_async_tcp_stream;\n");
    out.push_str("typedef ");
    out.push_str(&c_struct_ident("TcpChunk", &[]));
    out.push_str(" nomo_async_tcp_chunk;\n");
    out.push_str("typedef ");
    out.push_str(&c_struct_ident("TcpTextChunk", &[]));
    out.push_str(" nomo_async_tcp_text_chunk;\n");
    out.push_str("typedef ");
    out.push_str(&c_struct_ident("NetError", &[]));
    out.push_str(" nomo_async_tcp_error;\n");
    out.push_str("typedef ");
    out.push_str(&c_enum_ident("NetErrorKind", &[]));
    out.push_str(" nomo_async_tcp_error_kind;\n");
    out.push_str("typedef ");
    out.push_str(&c_enum_ident("Result", &read_result_args));
    out.push_str(" nomo_async_tcp_read_result;\n");
    out.push_str("typedef ");
    out.push_str(&c_enum_ident("Result", &text_result_args));
    out.push_str(" nomo_async_tcp_text_result;\n");
    out.push_str("typedef ");
    out.push_str(&c_enum_ident("Result", &write_result_args));
    out.push_str(" nomo_async_tcp_write_result;\n");

    for (name, value) in [
        (
            "NOMO_ASYNC_TCP_READ_OK",
            c_enum_variant_ident("Result", &read_result_args, "Ok"),
        ),
        (
            "NOMO_ASYNC_TCP_READ_ERR",
            c_enum_variant_ident("Result", &read_result_args, "Err"),
        ),
        (
            "NOMO_ASYNC_TCP_TEXT_OK",
            c_enum_variant_ident("Result", &text_result_args, "Ok"),
        ),
        (
            "NOMO_ASYNC_TCP_TEXT_ERR",
            c_enum_variant_ident("Result", &text_result_args, "Err"),
        ),
        (
            "NOMO_ASYNC_TCP_WRITE_OK",
            c_enum_variant_ident("Result", &write_result_args, "Ok"),
        ),
        (
            "NOMO_ASYNC_TCP_WRITE_ERR",
            c_enum_variant_ident("Result", &write_result_args, "Err"),
        ),
    ] {
        out.push_str("#define ");
        out.push_str(name);
        out.push(' ');
        out.push_str(&value);
        out.push('\n');
    }
    for variant in [
        "InvalidInput",
        "Unsupported",
        "Timeout",
        "Closed",
        "Busy",
        "Limit",
        "Read",
        "Write",
        "Reactor",
    ] {
        out.push_str("#define NOMO_ASYNC_TCP_KIND_");
        out.push_str(&variant.to_ascii_uppercase());
        out.push(' ');
        out.push_str(&c_enum_variant_ident("NetErrorKind", &[], variant));
        out.push('\n');
    }
    out.push_str(
        r#"#ifdef MSG_NOSIGNAL
#define NOMO_ASYNC_TCP_SEND_FLAGS MSG_NOSIGNAL
#else
#define NOMO_ASYNC_TCP_SEND_FLAGS 0
#endif

#define NOMO_ASYNC_TCP_WRITE_POLL_BUDGET 65536u

"#,
    );

    out.push_str(
        r#"
typedef enum {
    NOMO_ASYNC_TCP_IO_READ = 1,
    NOMO_ASYNC_TCP_IO_READ_STRING = 2,
    NOMO_ASYNC_TCP_IO_WRITE = 3,
    NOMO_ASYNC_TCP_IO_WRITE_STRING = 4
} nomo_async_tcp_io_kind;

typedef struct {
    nomo_async_reactor_registration reactor_registration;
    nomo_async_timer_registration timer;
    nomo_async_timer_outcome timer_outcome;
    nomo_async_context *context;
    void *frame;
    nomo_async_poll_fn poll;
    uint8_t *read_buffer;
    uint8_t *write_buffer;
    nomo_array_u32 write_bytes;
    nomo_string write_text;
    int64_t deadline_millis;
    size_t read_capacity;
    size_t write_length;
    size_t write_offset;
    uint64_t retained_bytes;
    uint32_t handle_slot;
    uint32_t handle_generation;
    uint32_t interests;
    uint32_t direction;
    nomo_async_tcp_io_kind kind;
    uint8_t active;
    uint8_t ready;
    uint8_t acquired;
    uint8_t payload_owned;
} nomo_async_tcp_io_registration;

static void nomo_async_tcp_read_error(
    nomo_async_tcp_read_result *result,
    nomo_async_tcp_error_kind kind,
    const char *message
) {
    memset(result, 0, sizeof(*result));
    result->tag = NOMO_ASYNC_TCP_READ_ERR;
    result->payload.nomo_payload_Err = (nomo_async_tcp_error){
        .nomo_member_kind = kind,
        .nomo_member_message = nomo_string_literal(message)
    };
}

static void nomo_async_tcp_text_error(
    nomo_async_tcp_text_result *result,
    nomo_async_tcp_error_kind kind,
    const char *message
) {
    memset(result, 0, sizeof(*result));
    result->tag = NOMO_ASYNC_TCP_TEXT_ERR;
    result->payload.nomo_payload_Err = (nomo_async_tcp_error){
        .nomo_member_kind = kind,
        .nomo_member_message = nomo_string_literal(message)
    };
}

static void nomo_async_tcp_write_error(
    nomo_async_tcp_write_result *result,
    nomo_async_tcp_error_kind kind,
    const char *message
) {
    memset(result, 0, sizeof(*result));
    result->tag = NOMO_ASYNC_TCP_WRITE_ERR;
    result->payload.nomo_payload_Err = (nomo_async_tcp_error){
        .nomo_member_kind = kind,
        .nomo_member_message = nomo_string_literal(message)
    };
}

static void nomo_async_tcp_write_success(nomo_async_tcp_write_result *result) {
    memset(result, 0, sizeof(*result));
    result->tag = NOMO_ASYNC_TCP_WRITE_OK;
    result->payload.nomo_payload_Ok = 0;
}

static void nomo_async_tcp_io_set_retained(
    nomo_async_tcp_io_registration *registration,
    uint64_t retained
) {
    nomo_async_context *context = registration->context;
    if (registration->retained_bytes > 0u) {
        context->retained_io_bytes -= registration->retained_bytes;
    }
    registration->retained_bytes = retained;
    context->retained_io_bytes += retained;
    if (context->retained_io_bytes > context->peak_retained_io_bytes) {
        context->peak_retained_io_bytes = context->retained_io_bytes;
    }
}

static void nomo_async_tcp_io_release_payload(
    nomo_async_tcp_io_registration *registration
) {
    if (registration->read_buffer != NULL) {
        free(registration->read_buffer);
        registration->read_buffer = NULL;
    }
    if (registration->write_buffer != NULL) {
        free(registration->write_buffer);
        registration->write_buffer = NULL;
    }
    if (registration->payload_owned == 0u) {
        return;
    }
    registration->payload_owned = 0u;
    if (registration->kind == NOMO_ASYNC_TCP_IO_WRITE) {
        nomo_array_u32_release(registration->write_bytes);
        registration->write_bytes = nomo_array_u32_new();
    } else if (registration->kind == NOMO_ASYNC_TCP_IO_WRITE_STRING) {
        nomo_string_release(registration->write_text);
        registration->write_text = nomo_string_literal("");
    }
}

static void nomo_async_tcp_io_finish(
    nomo_async_tcp_io_registration *registration
) {
    nomo_async_context *context = registration->context;
    if (context == NULL) {
        return;
    }
#ifdef _WIN32
    if (registration->reactor_registration.operation != NULL) {
        if (registration->read_buffer != NULL) {
            nomo_async_reactor_detach_buffer(
                &registration->reactor_registration,
                registration->read_buffer
            );
            registration->read_buffer = NULL;
        } else if (registration->write_buffer != NULL) {
            nomo_async_reactor_detach_buffer(
                &registration->reactor_registration,
                registration->write_buffer
            );
            registration->write_buffer = NULL;
        }
    }
#endif
    nomo_async_reactor_deregister(
        &context->reactor,
        &registration->reactor_registration
    );
    nomo_async_timer_disarm(&registration->timer, context);
    if (registration->active != 0u) {
        registration->active = 0u;
        if (context->live_io_operations > 0u) {
            context->live_io_operations -= 1u;
        }
    }
    if (registration->acquired != 0u) {
        nomo_async_io_handle_release(
            context,
            registration->handle_slot,
            registration->handle_generation,
            registration->direction
        );
        registration->acquired = 0u;
    }
    nomo_async_tcp_io_set_retained(registration, 0u);
}

static void nomo_async_tcp_io_wake(void *owner, uint32_t ready) {
    nomo_async_tcp_io_registration *registration =
        (nomo_async_tcp_io_registration *)owner;
    if (registration == NULL
        || registration->active == 0u
        || (ready & registration->interests) == 0u) {
        return;
    }
#ifndef _WIN32
    registration->deadline_millis = registration->timer.deadline_millis;
    nomo_async_timer_disarm(&registration->timer, registration->context);
#endif
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

static int nomo_async_tcp_io_arm(
    nomo_async_tcp_io_registration *registration,
    nomo_socket handle,
    uint64_t timeout_millis
) {
    registration->reactor_registration.owner = registration;
    registration->reactor_registration.wake = nomo_async_tcp_io_wake;
    if (nomo_async_reactor_register(
            &registration->context->reactor,
            &registration->reactor_registration,
            handle,
            registration->interests
        ) != 0) {
        return 1;
    }
    nomo_async_poll timer_status = nomo_async_timer_start(
        &registration->timer,
        (int64_t)timeout_millis,
        registration->context,
        &registration->timer_outcome,
        NULL,
        0u
    );
    if (timer_status != NOMO_ASYNC_POLL_PENDING) {
        nomo_async_reactor_deregister(
            &registration->context->reactor,
            &registration->reactor_registration
        );
        return 2;
    }
    registration->deadline_millis = registration->timer.deadline_millis;
    registration->active = 1u;
    registration->context->live_io_operations += 1u;
    if (registration->context->live_io_operations
        > registration->context->peak_live_io_operations) {
        registration->context->peak_live_io_operations =
            registration->context->live_io_operations;
    }
    registration->context->pending_reason = NOMO_ASYNC_PENDING_IO;
    return 0;
}

static int nomo_async_tcp_io_rearm(
    nomo_async_tcp_io_registration *registration
) {
    int64_t now = nomo_time_monotonic_millis();
    if (registration->deadline_millis <= now) {
        return 3;
    }
    if (nomo_async_reactor_reregister(
            &registration->context->reactor,
            &registration->reactor_registration,
            registration->interests
        ) != 0) {
        return 1;
    }
    int64_t remaining = registration->deadline_millis - now;
    nomo_async_poll timer_status = nomo_async_timer_start(
        &registration->timer,
        remaining,
        registration->context,
        &registration->timer_outcome,
        NULL,
        0u
    );
    if (timer_status != NOMO_ASYNC_POLL_PENDING) {
        nomo_async_reactor_deregister(
            &registration->context->reactor,
            &registration->reactor_registration
        );
        return 2;
    }
    registration->ready = 0u;
    registration->context->pending_reason = NOMO_ASYNC_PENDING_IO;
    return 0;
}

static void nomo_async_tcp_io_cancel(
    nomo_async_tcp_io_registration *registration,
    nomo_async_context *context
) {
    if (registration->context == NULL
        || (registration->active == 0u
            && registration->acquired == 0u
            && registration->read_buffer == NULL
            && registration->payload_owned == 0u)) {
        return;
    }
    nomo_async_tcp_io_finish(registration);
    nomo_async_tcp_io_release_payload(registration);
    context->io_cancellations += 1u;
}

static int nomo_async_tcp_utf8_valid(const uint8_t *data, size_t length) {
    size_t index = 0u;
    while (index < length) {
        uint8_t first = data[index];
        if (first == 0u) {
            return 0;
        }
        if (first <= 0x7fu) {
            index += 1u;
            continue;
        }
        size_t width = 0u;
        uint32_t codepoint = 0u;
        if (first >= 0xc2u && first <= 0xdfu) {
            width = 2u;
            codepoint = first & 0x1fu;
        } else if (first >= 0xe0u && first <= 0xefu) {
            width = 3u;
            codepoint = first & 0x0fu;
        } else if (first >= 0xf0u && first <= 0xf4u) {
            width = 4u;
            codepoint = first & 0x07u;
        } else {
            return 0;
        }
        if (index + width > length) {
            return 0;
        }
        for (size_t offset = 1u; offset < width; offset += 1u) {
            uint8_t next = data[index + offset];
            if ((next & 0xc0u) != 0x80u) {
                return 0;
            }
            codepoint = (codepoint << 6u) | (uint32_t)(next & 0x3fu);
        }
        if ((width == 3u && codepoint < 0x800u)
            || (width == 4u && codepoint < 0x10000u)
            || codepoint > 0x10ffffu
            || (codepoint >= 0xd800u && codepoint <= 0xdfffu)) {
            return 0;
        }
        index += width;
    }
    return 1;
}

"#,
    );

    if target.operating_system() == nomo_target::OperatingSystem::Windows {
        emit_windows_async_tcp_io(out);
    } else {
        emit_unix_async_tcp_io(out);
    }
}

fn emit_windows_async_tcp_io(out: &mut String) {
    out.push_str(
        r#"
static int nomo_async_tcp_iocp_prepare(
    nomo_async_tcp_io_registration *registration,
    nomo_async_tcp_stream stream,
    uint64_t timeout_millis,
    uint32_t direction,
    uint32_t interests,
    nomo_async_tcp_io_kind kind,
    nomo_async_context *context,
    nomo_async_tcp_error_kind *error_kind,
    const char **error_message
) {
    memset(registration, 0, sizeof(*registration));
    registration->context = context;
    registration->frame = context->current_frame;
    registration->poll = context->current_poll;
    registration->handle_slot = stream.nomo_member_slot;
    registration->handle_generation = stream.nomo_member_generation;
    registration->direction = direction;
    registration->interests = interests;
    registration->kind = kind;
    if (timeout_millis > 900000u) {
        *error_kind = (nomo_async_tcp_error_kind){
            .tag = NOMO_ASYNC_TCP_KIND_INVALIDINPUT
        };
        *error_message = "timeout_millis must be at most 900000";
        return 1;
    }
    if (stream.nomo_member_owner != context
        || stream.nomo_member_close_fn != nomo_async_io_handle_close_callback) {
        *error_kind = (nomo_async_tcp_error_kind){
            .tag = NOMO_ASYNC_TCP_KIND_CLOSED
        };
        *error_message = "TCP stream is closed or belongs to another executor";
        return 1;
    }
    int acquired = nomo_async_io_handle_acquire(
        context,
        registration->handle_slot,
        registration->handle_generation,
        direction
    );
    if (acquired != 0) {
        *error_kind = (nomo_async_tcp_error_kind){
            .tag = acquired == 2
                ? NOMO_ASYNC_TCP_KIND_BUSY
                : NOMO_ASYNC_TCP_KIND_CLOSED
        };
        *error_message = acquired == 2
            ? "TCP stream direction already has a pending operation"
            : "TCP stream is closed";
        return 1;
    }
    registration->acquired = 1u;
    return 0;
}

static int nomo_async_tcp_iocp_begin(
    nomo_async_tcp_io_registration *registration,
    nomo_socket handle,
    uint64_t timeout_millis
) {
    nomo_async_poll timer_status = nomo_async_timer_start(
        &registration->timer,
        (int64_t)timeout_millis,
        registration->context,
        &registration->timer_outcome,
        NULL,
        0u
    );
    if (timer_status != NOMO_ASYNC_POLL_PENDING) {
        return 3;
    }
    registration->deadline_millis = registration->timer.deadline_millis;
    registration->reactor_registration.owner = registration;
    registration->reactor_registration.wake = nomo_async_tcp_io_wake;
    if (nomo_async_io_handle_associate_reactor(
            registration->context,
            registration->handle_slot,
            registration->handle_generation
        ) != 0) {
        nomo_async_timer_disarm(
            &registration->timer,
            registration->context
        );
        return 1;
    }
    if (nomo_async_reactor_register(
            &registration->context->reactor,
            &registration->reactor_registration,
            handle,
            registration->interests
        ) != 0) {
        nomo_async_timer_disarm(
            &registration->timer,
            registration->context
        );
        return 2;
    }
    return 0;
}

static void nomo_async_tcp_iocp_mark_active(
    nomo_async_tcp_io_registration *registration
) {
    nomo_async_reactor_mark_submitted(
        &registration->reactor_registration
    );
    if (registration->active == 0u) {
        registration->active = 1u;
        registration->context->live_io_operations += 1u;
        if (registration->context->live_io_operations
            > registration->context->peak_live_io_operations) {
            registration->context->peak_live_io_operations =
                registration->context->live_io_operations;
        }
    }
    registration->context->pending_reason = NOMO_ASYNC_PENDING_IO;
}

static int nomo_async_tcp_iocp_issue_read(
    nomo_async_tcp_io_registration *registration,
    nomo_socket handle
) {
    WSABUF buffer;
    buffer.buf = (CHAR *)registration->read_buffer;
    buffer.len = (ULONG)registration->read_capacity;
    DWORD transferred = 0u;
    DWORD flags = 0u;
    int status = WSARecv(
        handle,
        &buffer,
        1u,
        &transferred,
        &flags,
        nomo_async_reactor_overlapped(
            &registration->reactor_registration
        ),
        NULL
    );
    int error = status == 0 ? 0 : WSAGetLastError();
    if (status == SOCKET_ERROR && error != WSA_IO_PENDING) {
        return 1;
    }
    nomo_async_tcp_iocp_mark_active(registration);
    return 0;
}

static int nomo_async_tcp_iocp_issue_write(
    nomo_async_tcp_io_registration *registration,
    nomo_socket handle
) {
    size_t remaining =
        registration->write_length - registration->write_offset;
    size_t chunk = remaining < NOMO_ASYNC_TCP_WRITE_POLL_BUDGET
        ? remaining
        : NOMO_ASYNC_TCP_WRITE_POLL_BUDGET;
    WSABUF buffer;
    buffer.buf = (CHAR *)registration->write_buffer
        + registration->write_offset;
    buffer.len = (ULONG)chunk;
    DWORD transferred = 0u;
    int status = WSASend(
        handle,
        &buffer,
        1u,
        &transferred,
        0u,
        nomo_async_reactor_overlapped(
            &registration->reactor_registration
        ),
        NULL
    );
    int error = status == 0 ? 0 : WSAGetLastError();
    if (status == SOCKET_ERROR && error != WSA_IO_PENDING) {
        return 1;
    }
    nomo_async_tcp_iocp_mark_active(registration);
    return 0;
}

static void nomo_async_tcp_iocp_error_from_begin(
    int begin,
    nomo_async_tcp_error_kind *kind,
    const char **message
) {
    *kind = (nomo_async_tcp_error_kind){
        .tag = begin == 1
            ? NOMO_ASYNC_TCP_KIND_REACTOR
            : NOMO_ASYNC_TCP_KIND_LIMIT
    };
    *message = begin == 1
        ? "IOCP socket association failed"
        : (begin == 2
            ? "owner executor IOCP operation capacity is exhausted"
            : "owner executor timer capacity is exhausted");
}

static int nomo_async_tcp_iocp_resume_common(
    nomo_async_tcp_io_registration *registration,
    nomo_async_context *context,
    nomo_async_tcp_error_kind *error_kind,
    const char **error_message
) {
    if (registration->timer.armed != 0u
        && nomo_time_monotonic_millis()
            >= registration->timer.deadline_millis) {
        (void)nomo_async_deadline_due(&registration->timer, context);
    }
    if (registration->timer.expired != 0u) {
        registration->timer.expired = 0u;
        nomo_async_tcp_io_finish(registration);
        nomo_async_tcp_io_release_payload(registration);
        context->io_timeouts += 1u;
        *error_kind = (nomo_async_tcp_error_kind){
            .tag = NOMO_ASYNC_TCP_KIND_TIMEOUT
        };
        *error_message = "TCP operation timed out";
        return 1;
    }
    if (registration->ready == 0u) {
        context->pending_reason = NOMO_ASYNC_PENDING_IO;
        return 2;
    }
    registration->ready = 0u;
    if (registration->reactor_registration.error != ERROR_SUCCESS) {
        nomo_async_tcp_io_finish(registration);
        nomo_async_tcp_io_release_payload(registration);
        context->io_errors += 1u;
        return 3;
    }
    nomo_socket handle = nomo_async_io_handle_get(
        context,
        registration->handle_slot,
        registration->handle_generation
    );
    if (handle == NOMO_INVALID_SOCKET) {
        nomo_async_tcp_io_finish(registration);
        nomo_async_tcp_io_release_payload(registration);
        *error_kind = (nomo_async_tcp_error_kind){
            .tag = NOMO_ASYNC_TCP_KIND_CLOSED
        };
        *error_message = "TCP stream is closed";
        return 1;
    }
    return 0;
}

static void nomo_async_tcp_iocp_complete_read(
    nomo_async_tcp_io_registration *registration,
    nomo_async_tcp_read_result *result
) {
    size_t length =
        (size_t)registration->reactor_registration.transferred;
    nomo_array_u32 data = nomo_array_u32_new();
    for (size_t index = 0u; index < length; index += 1u) {
        data = nomo_array_u32_push(
            data,
            (uint32_t)registration->read_buffer[index]
        );
    }
    nomo_async_tcp_io_finish(registration);
    nomo_async_tcp_io_release_payload(registration);
    memset(result, 0, sizeof(*result));
    result->tag = NOMO_ASYNC_TCP_READ_OK;
    result->payload.nomo_payload_Ok = (nomo_async_tcp_chunk){
        .nomo_member_data = data,
        .nomo_member_eof = length == 0u
    };
}

static void nomo_async_tcp_iocp_complete_text(
    nomo_async_tcp_io_registration *registration,
    nomo_async_tcp_text_result *result
) {
    size_t length =
        (size_t)registration->reactor_registration.transferred;
    if (!nomo_async_tcp_utf8_valid(registration->read_buffer, length)) {
        nomo_async_tcp_io_finish(registration);
        nomo_async_tcp_io_release_payload(registration);
        nomo_async_tcp_text_error(
            result,
            (nomo_async_tcp_error_kind){.tag = NOMO_ASYNC_TCP_KIND_READ},
            "TCP text chunk is not valid Nomo UTF-8 text"
        );
        registration->context->io_errors += 1u;
        return;
    }
    nomo_string data = nomo_string_literal("");
    if (length > 0u) {
        registration->read_buffer[length] = 0u;
        data = nomo_string_owned((char *)registration->read_buffer);
        registration->read_buffer = NULL;
    }
    nomo_async_tcp_io_finish(registration);
    nomo_async_tcp_io_release_payload(registration);
    memset(result, 0, sizeof(*result));
    result->tag = NOMO_ASYNC_TCP_TEXT_OK;
    result->payload.nomo_payload_Ok = (nomo_async_tcp_text_chunk){
        .nomo_member_data = data,
        .nomo_member_eof = length == 0u
    };
}

static nomo_async_poll nomo_async_tcp_read_start(
    nomo_async_tcp_io_registration *registration,
    nomo_async_tcp_stream stream,
    uint64_t max_bytes,
    uint64_t timeout_millis,
    nomo_async_context *context,
    nomo_async_tcp_read_result *result
) {
    context->io_read_starts += 1u;
    nomo_async_tcp_error_kind error_kind = {0};
    const char *error_message = NULL;
    if (max_bytes == 0u || max_bytes > 1048576u) {
        nomo_async_tcp_read_error(
            result,
            (nomo_async_tcp_error_kind){.tag = NOMO_ASYNC_TCP_KIND_INVALIDINPUT},
            "max_bytes must be in 1..=1048576"
        );
        return NOMO_ASYNC_POLL_READY;
    }
    if (nomo_async_tcp_iocp_prepare(
            registration,
            stream,
            timeout_millis,
            NOMO_ASYNC_IO_DIRECTION_READ,
            NOMO_ASYNC_REACTOR_READ,
            NOMO_ASYNC_TCP_IO_READ,
            context,
            &error_kind,
            &error_message
        ) != 0) {
        nomo_async_tcp_read_error(result, error_kind, error_message);
        return NOMO_ASYNC_POLL_READY;
    }
    registration->read_capacity = (size_t)max_bytes;
    registration->read_buffer = (uint8_t *)malloc((size_t)max_bytes + 1u);
    if (registration->read_buffer == NULL) {
        nomo_panic("out of memory");
    }
    nomo_socket handle = nomo_async_io_handle_get(
        context,
        registration->handle_slot,
        registration->handle_generation
    );
    if (timeout_millis == 0u) {
        int received = recv(
            handle,
            (char *)registration->read_buffer,
            (int)registration->read_capacity,
            0
        );
        if (received >= 0) {
            registration->reactor_registration.transferred = (DWORD)received;
            nomo_async_tcp_iocp_complete_read(registration, result);
        } else {
            int receive_error = WSAGetLastError();
            nomo_async_tcp_io_finish(registration);
            nomo_async_tcp_io_release_payload(registration);
            nomo_async_tcp_read_error(
                result,
                (nomo_async_tcp_error_kind){
                    .tag = receive_error == WSAEWOULDBLOCK
                        ? NOMO_ASYNC_TCP_KIND_TIMEOUT
                        : NOMO_ASYNC_TCP_KIND_READ
                },
                receive_error == WSAEWOULDBLOCK
                    ? "TCP read did not complete immediately"
                    : "TCP read failed"
            );
            context->io_timeouts += receive_error == WSAEWOULDBLOCK;
            context->io_errors += receive_error != WSAEWOULDBLOCK;
        }
        return NOMO_ASYNC_POLL_READY;
    }
    int begin = nomo_async_tcp_iocp_begin(
        registration,
        handle,
        timeout_millis
    );
    if (begin != 0) {
        nomo_async_tcp_io_finish(registration);
        nomo_async_tcp_io_release_payload(registration);
        nomo_async_tcp_iocp_error_from_begin(
            begin,
            &error_kind,
            &error_message
        );
        nomo_async_tcp_read_error(result, error_kind, error_message);
        context->io_errors += begin == 1;
        return NOMO_ASYNC_POLL_READY;
    }
    if (nomo_async_tcp_iocp_issue_read(registration, handle) != 0) {
        nomo_async_tcp_io_finish(registration);
        nomo_async_tcp_io_release_payload(registration);
        nomo_async_tcp_read_error(
            result,
            (nomo_async_tcp_error_kind){.tag = NOMO_ASYNC_TCP_KIND_READ},
            "TCP read failed to start"
        );
        context->io_errors += 1u;
        return NOMO_ASYNC_POLL_READY;
    }
    nomo_async_tcp_io_set_retained(registration, max_bytes);
    return NOMO_ASYNC_POLL_PENDING;
}

static nomo_async_poll nomo_async_tcp_read_string_start(
    nomo_async_tcp_io_registration *registration,
    nomo_async_tcp_stream stream,
    uint64_t max_bytes,
    uint64_t timeout_millis,
    nomo_async_context *context,
    nomo_async_tcp_text_result *result
) {
    context->io_read_starts += 1u;
    nomo_async_tcp_error_kind error_kind = {0};
    const char *error_message = NULL;
    if (max_bytes == 0u || max_bytes > 1048576u) {
        nomo_async_tcp_text_error(
            result,
            (nomo_async_tcp_error_kind){.tag = NOMO_ASYNC_TCP_KIND_INVALIDINPUT},
            "max_bytes must be in 1..=1048576"
        );
        return NOMO_ASYNC_POLL_READY;
    }
    if (nomo_async_tcp_iocp_prepare(
            registration,
            stream,
            timeout_millis,
            NOMO_ASYNC_IO_DIRECTION_READ,
            NOMO_ASYNC_REACTOR_READ,
            NOMO_ASYNC_TCP_IO_READ_STRING,
            context,
            &error_kind,
            &error_message
        ) != 0) {
        nomo_async_tcp_text_error(result, error_kind, error_message);
        return NOMO_ASYNC_POLL_READY;
    }
    registration->read_capacity = (size_t)max_bytes;
    registration->read_buffer = (uint8_t *)malloc((size_t)max_bytes + 1u);
    if (registration->read_buffer == NULL) {
        nomo_panic("out of memory");
    }
    nomo_socket handle = nomo_async_io_handle_get(
        context,
        registration->handle_slot,
        registration->handle_generation
    );
    if (timeout_millis == 0u) {
        int received = recv(
            handle,
            (char *)registration->read_buffer,
            (int)registration->read_capacity,
            0
        );
        if (received >= 0) {
            registration->reactor_registration.transferred = (DWORD)received;
            nomo_async_tcp_iocp_complete_text(registration, result);
        } else {
            int receive_error = WSAGetLastError();
            nomo_async_tcp_io_finish(registration);
            nomo_async_tcp_io_release_payload(registration);
            nomo_async_tcp_text_error(
                result,
                (nomo_async_tcp_error_kind){
                    .tag = receive_error == WSAEWOULDBLOCK
                        ? NOMO_ASYNC_TCP_KIND_TIMEOUT
                        : NOMO_ASYNC_TCP_KIND_READ
                },
                receive_error == WSAEWOULDBLOCK
                    ? "TCP text read did not complete immediately"
                    : "TCP text read failed"
            );
            context->io_timeouts += receive_error == WSAEWOULDBLOCK;
            context->io_errors += receive_error != WSAEWOULDBLOCK;
        }
        return NOMO_ASYNC_POLL_READY;
    }
    int begin = nomo_async_tcp_iocp_begin(
        registration,
        handle,
        timeout_millis
    );
    if (begin != 0) {
        nomo_async_tcp_io_finish(registration);
        nomo_async_tcp_io_release_payload(registration);
        nomo_async_tcp_iocp_error_from_begin(
            begin,
            &error_kind,
            &error_message
        );
        nomo_async_tcp_text_error(result, error_kind, error_message);
        context->io_errors += begin == 1;
        return NOMO_ASYNC_POLL_READY;
    }
    if (nomo_async_tcp_iocp_issue_read(registration, handle) != 0) {
        nomo_async_tcp_io_finish(registration);
        nomo_async_tcp_io_release_payload(registration);
        nomo_async_tcp_text_error(
            result,
            (nomo_async_tcp_error_kind){.tag = NOMO_ASYNC_TCP_KIND_READ},
            "TCP text read failed to start"
        );
        context->io_errors += 1u;
        return NOMO_ASYNC_POLL_READY;
    }
    nomo_async_tcp_io_set_retained(registration, max_bytes);
    return NOMO_ASYNC_POLL_PENDING;
}

static nomo_async_poll nomo_async_tcp_write_start(
    nomo_async_tcp_io_registration *registration,
    nomo_async_tcp_stream stream,
    nomo_array_u32 data,
    uint64_t timeout_millis,
    nomo_async_context *context,
    nomo_async_tcp_write_result *result
) {
    context->io_write_starts += 1u;
    if (data.len > 1048576u || timeout_millis > 900000u) {
        nomo_async_tcp_write_error(
            result,
            (nomo_async_tcp_error_kind){.tag = NOMO_ASYNC_TCP_KIND_INVALIDINPUT},
            "TCP write exceeds the payload or timeout bound"
        );
        return NOMO_ASYNC_POLL_READY;
    }
    for (size_t index = 0u; index < data.len; index += 1u) {
        if (data.data[index] > 255u) {
            nomo_async_tcp_write_error(
                result,
                (nomo_async_tcp_error_kind){.tag = NOMO_ASYNC_TCP_KIND_INVALIDINPUT},
                "TCP byte values must be in 0..=255"
            );
            return NOMO_ASYNC_POLL_READY;
        }
    }
    nomo_async_tcp_error_kind error_kind = {0};
    const char *error_message = NULL;
    if (nomo_async_tcp_iocp_prepare(
            registration,
            stream,
            timeout_millis,
            NOMO_ASYNC_IO_DIRECTION_WRITE,
            NOMO_ASYNC_REACTOR_WRITE,
            NOMO_ASYNC_TCP_IO_WRITE,
            context,
            &error_kind,
            &error_message
        ) != 0) {
        nomo_async_tcp_write_error(result, error_kind, error_message);
        return NOMO_ASYNC_POLL_READY;
    }
    registration->write_length = data.len;
    if (data.len == 0u) {
        nomo_async_tcp_io_finish(registration);
        nomo_async_tcp_write_success(result);
        return NOMO_ASYNC_POLL_READY;
    }
    registration->write_buffer = (uint8_t *)malloc(data.len);
    if (registration->write_buffer == NULL) {
        nomo_panic("out of memory");
    }
    for (size_t index = 0u; index < data.len; index += 1u) {
        registration->write_buffer[index] = (uint8_t)data.data[index];
    }
    nomo_socket handle = nomo_async_io_handle_get(
        context,
        registration->handle_slot,
        registration->handle_generation
    );
    if (timeout_millis == 0u) {
        int sent = send(
            handle,
            (const char *)registration->write_buffer,
            (int)(data.len < NOMO_ASYNC_TCP_WRITE_POLL_BUDGET
                ? data.len
                : NOMO_ASYNC_TCP_WRITE_POLL_BUDGET),
            0
        );
        if (sent >= 0 && (size_t)sent == data.len) {
            nomo_async_tcp_io_finish(registration);
            nomo_async_tcp_io_release_payload(registration);
            nomo_async_tcp_write_success(result);
        } else {
            int send_error = sent < 0 ? WSAGetLastError() : WSAEWOULDBLOCK;
            nomo_async_tcp_io_finish(registration);
            nomo_async_tcp_io_release_payload(registration);
            nomo_async_tcp_write_error(
                result,
                (nomo_async_tcp_error_kind){
                    .tag = send_error == WSAEWOULDBLOCK
                        ? NOMO_ASYNC_TCP_KIND_TIMEOUT
                        : NOMO_ASYNC_TCP_KIND_WRITE
                },
                send_error == WSAEWOULDBLOCK
                    ? "TCP write did not complete immediately"
                    : "TCP write failed"
            );
            context->io_timeouts += send_error == WSAEWOULDBLOCK;
            context->io_errors += send_error != WSAEWOULDBLOCK;
        }
        return NOMO_ASYNC_POLL_READY;
    }
    int begin = nomo_async_tcp_iocp_begin(
        registration,
        handle,
        timeout_millis
    );
    if (begin != 0) {
        nomo_async_tcp_io_finish(registration);
        nomo_async_tcp_io_release_payload(registration);
        nomo_async_tcp_iocp_error_from_begin(
            begin,
            &error_kind,
            &error_message
        );
        nomo_async_tcp_write_error(result, error_kind, error_message);
        context->io_errors += begin == 1;
        return NOMO_ASYNC_POLL_READY;
    }
    if (nomo_async_tcp_iocp_issue_write(registration, handle) != 0) {
        nomo_async_tcp_io_finish(registration);
        nomo_async_tcp_io_release_payload(registration);
        nomo_async_tcp_write_error(
            result,
            (nomo_async_tcp_error_kind){.tag = NOMO_ASYNC_TCP_KIND_WRITE},
            "TCP write failed to start"
        );
        context->io_errors += 1u;
        return NOMO_ASYNC_POLL_READY;
    }
    nomo_async_tcp_io_set_retained(registration, data.len);
    return NOMO_ASYNC_POLL_PENDING;
}

static nomo_async_poll nomo_async_tcp_write_string_start(
    nomo_async_tcp_io_registration *registration,
    nomo_async_tcp_stream stream,
    nomo_string content,
    uint64_t timeout_millis,
    nomo_async_context *context,
    nomo_async_tcp_write_result *result
) {
    context->io_write_starts += 1u;
    size_t length = strlen(content.data);
    if (length > 1048576u || timeout_millis > 900000u) {
        nomo_async_tcp_write_error(
            result,
            (nomo_async_tcp_error_kind){.tag = NOMO_ASYNC_TCP_KIND_INVALIDINPUT},
            "TCP string write exceeds the payload or timeout bound"
        );
        return NOMO_ASYNC_POLL_READY;
    }
    nomo_async_tcp_error_kind error_kind = {0};
    const char *error_message = NULL;
    if (nomo_async_tcp_iocp_prepare(
            registration,
            stream,
            timeout_millis,
            NOMO_ASYNC_IO_DIRECTION_WRITE,
            NOMO_ASYNC_REACTOR_WRITE,
            NOMO_ASYNC_TCP_IO_WRITE_STRING,
            context,
            &error_kind,
            &error_message
        ) != 0) {
        nomo_async_tcp_write_error(result, error_kind, error_message);
        return NOMO_ASYNC_POLL_READY;
    }
    registration->write_length = length;
    if (length == 0u) {
        nomo_async_tcp_io_finish(registration);
        nomo_async_tcp_write_success(result);
        return NOMO_ASYNC_POLL_READY;
    }
    registration->write_buffer = (uint8_t *)malloc(length);
    if (registration->write_buffer == NULL) {
        nomo_panic("out of memory");
    }
    memcpy(registration->write_buffer, content.data, length);
    nomo_socket handle = nomo_async_io_handle_get(
        context,
        registration->handle_slot,
        registration->handle_generation
    );
    if (timeout_millis == 0u) {
        int sent = send(
            handle,
            (const char *)registration->write_buffer,
            (int)(length < NOMO_ASYNC_TCP_WRITE_POLL_BUDGET
                ? length
                : NOMO_ASYNC_TCP_WRITE_POLL_BUDGET),
            0
        );
        if (sent >= 0 && (size_t)sent == length) {
            nomo_async_tcp_io_finish(registration);
            nomo_async_tcp_io_release_payload(registration);
            nomo_async_tcp_write_success(result);
        } else {
            int send_error = sent < 0 ? WSAGetLastError() : WSAEWOULDBLOCK;
            nomo_async_tcp_io_finish(registration);
            nomo_async_tcp_io_release_payload(registration);
            nomo_async_tcp_write_error(
                result,
                (nomo_async_tcp_error_kind){
                    .tag = send_error == WSAEWOULDBLOCK
                        ? NOMO_ASYNC_TCP_KIND_TIMEOUT
                        : NOMO_ASYNC_TCP_KIND_WRITE
                },
                send_error == WSAEWOULDBLOCK
                    ? "TCP string write did not complete immediately"
                    : "TCP string write failed"
            );
            context->io_timeouts += send_error == WSAEWOULDBLOCK;
            context->io_errors += send_error != WSAEWOULDBLOCK;
        }
        return NOMO_ASYNC_POLL_READY;
    }
    int begin = nomo_async_tcp_iocp_begin(
        registration,
        handle,
        timeout_millis
    );
    if (begin != 0) {
        nomo_async_tcp_io_finish(registration);
        nomo_async_tcp_io_release_payload(registration);
        nomo_async_tcp_iocp_error_from_begin(
            begin,
            &error_kind,
            &error_message
        );
        nomo_async_tcp_write_error(result, error_kind, error_message);
        context->io_errors += begin == 1;
        return NOMO_ASYNC_POLL_READY;
    }
    if (nomo_async_tcp_iocp_issue_write(registration, handle) != 0) {
        nomo_async_tcp_io_finish(registration);
        nomo_async_tcp_io_release_payload(registration);
        nomo_async_tcp_write_error(
            result,
            (nomo_async_tcp_error_kind){.tag = NOMO_ASYNC_TCP_KIND_WRITE},
            "TCP string write failed to start"
        );
        context->io_errors += 1u;
        return NOMO_ASYNC_POLL_READY;
    }
    nomo_async_tcp_io_set_retained(registration, length);
    return NOMO_ASYNC_POLL_PENDING;
}

static nomo_async_poll nomo_async_tcp_read_resume(
    nomo_async_tcp_io_registration *registration,
    nomo_async_context *context,
    nomo_async_tcp_read_result *result
) {
    nomo_async_tcp_error_kind error_kind = {0};
    const char *error_message = NULL;
    int common = nomo_async_tcp_iocp_resume_common(
        registration,
        context,
        &error_kind,
        &error_message
    );
    if (common == 2) {
        return NOMO_ASYNC_POLL_PENDING;
    }
    if (common == 1 || common == 3) {
        nomo_async_tcp_read_error(
            result,
            common == 3
                ? (nomo_async_tcp_error_kind){.tag = NOMO_ASYNC_TCP_KIND_READ}
                : error_kind,
            common == 3 ? "TCP read failed" : error_message
        );
        return NOMO_ASYNC_POLL_READY;
    }
    nomo_async_tcp_iocp_complete_read(registration, result);
    return NOMO_ASYNC_POLL_READY;
}

static nomo_async_poll nomo_async_tcp_read_string_resume(
    nomo_async_tcp_io_registration *registration,
    nomo_async_context *context,
    nomo_async_tcp_text_result *result
) {
    nomo_async_tcp_error_kind error_kind = {0};
    const char *error_message = NULL;
    int common = nomo_async_tcp_iocp_resume_common(
        registration,
        context,
        &error_kind,
        &error_message
    );
    if (common == 2) {
        return NOMO_ASYNC_POLL_PENDING;
    }
    if (common == 1 || common == 3) {
        nomo_async_tcp_text_error(
            result,
            common == 3
                ? (nomo_async_tcp_error_kind){.tag = NOMO_ASYNC_TCP_KIND_READ}
                : error_kind,
            common == 3 ? "TCP text read failed" : error_message
        );
        return NOMO_ASYNC_POLL_READY;
    }
    nomo_async_tcp_iocp_complete_text(registration, result);
    return NOMO_ASYNC_POLL_READY;
}

static nomo_async_poll nomo_async_tcp_write_resume(
    nomo_async_tcp_io_registration *registration,
    nomo_async_context *context,
    nomo_async_tcp_write_result *result
) {
    nomo_async_tcp_error_kind error_kind = {0};
    const char *error_message = NULL;
    int common = nomo_async_tcp_iocp_resume_common(
        registration,
        context,
        &error_kind,
        &error_message
    );
    if (common == 2) {
        return NOMO_ASYNC_POLL_PENDING;
    }
    if (common == 1 || common == 3) {
        nomo_async_tcp_write_error(
            result,
            common == 3
                ? (nomo_async_tcp_error_kind){.tag = NOMO_ASYNC_TCP_KIND_WRITE}
                : error_kind,
            common == 3 ? "TCP write failed" : error_message
        );
        return NOMO_ASYNC_POLL_READY;
    }
    size_t transferred =
        (size_t)registration->reactor_registration.transferred;
    if (transferred == 0u
        || transferred > registration->write_length
            - registration->write_offset) {
        nomo_async_tcp_io_finish(registration);
        nomo_async_tcp_io_release_payload(registration);
        nomo_async_tcp_write_error(
            result,
            (nomo_async_tcp_error_kind){.tag = NOMO_ASYNC_TCP_KIND_WRITE},
            "TCP write made invalid progress"
        );
        context->io_errors += 1u;
        return NOMO_ASYNC_POLL_READY;
    }
    registration->write_offset += transferred;
    if (registration->write_offset == registration->write_length) {
        nomo_async_tcp_io_finish(registration);
        nomo_async_tcp_io_release_payload(registration);
        nomo_async_tcp_write_success(result);
        return NOMO_ASYNC_POLL_READY;
    }
    nomo_async_tcp_io_set_retained(
        registration,
        registration->write_length - registration->write_offset
    );
    nomo_socket handle = nomo_async_io_handle_get(
        context,
        registration->handle_slot,
        registration->handle_generation
    );
    if (nomo_async_reactor_reregister(
            &context->reactor,
            &registration->reactor_registration,
            NOMO_ASYNC_REACTOR_WRITE
        ) != 0) {
        nomo_async_tcp_io_finish(registration);
        nomo_async_tcp_io_release_payload(registration);
        nomo_async_tcp_write_error(
            result,
            (nomo_async_tcp_error_kind){.tag = NOMO_ASYNC_TCP_KIND_LIMIT},
            "owner executor IOCP operation capacity is exhausted"
        );
        return NOMO_ASYNC_POLL_READY;
    }
    if (nomo_async_tcp_iocp_issue_write(registration, handle) != 0) {
        nomo_async_tcp_io_finish(registration);
        nomo_async_tcp_io_release_payload(registration);
        nomo_async_tcp_write_error(
            result,
            (nomo_async_tcp_error_kind){.tag = NOMO_ASYNC_TCP_KIND_WRITE},
            "TCP write failed to continue"
        );
        context->io_errors += 1u;
        return NOMO_ASYNC_POLL_READY;
    }
    return NOMO_ASYNC_POLL_PENDING;
}

static nomo_async_poll nomo_async_tcp_write_string_resume(
    nomo_async_tcp_io_registration *registration,
    nomo_async_context *context,
    nomo_async_tcp_write_result *result
) {
    return nomo_async_tcp_write_resume(registration, context, result);
}
"#,
    );
}

fn emit_unix_async_tcp_io(out: &mut String) {
    out.push_str(
        r#"
static int nomo_async_tcp_io_prepare(
    nomo_async_tcp_io_registration *registration,
    nomo_async_tcp_stream stream,
    uint64_t timeout_millis,
    uint32_t direction,
    uint32_t interests,
    nomo_async_tcp_io_kind kind,
    nomo_async_context *context,
    nomo_async_tcp_error_kind *error_kind,
    const char **error_message
) {
    memset(registration, 0, sizeof(*registration));
    registration->context = context;
    registration->frame = context->current_frame;
    registration->poll = context->current_poll;
    registration->handle_slot = stream.nomo_member_slot;
    registration->handle_generation = stream.nomo_member_generation;
    registration->direction = direction;
    registration->interests = interests;
    registration->kind = kind;
    if (timeout_millis > 900000u) {
        *error_kind = (nomo_async_tcp_error_kind){
            .tag = NOMO_ASYNC_TCP_KIND_INVALIDINPUT
        };
        *error_message = "timeout_millis must be at most 900000";
        return 1;
    }
    if (stream.nomo_member_owner != context
        || stream.nomo_member_close_fn != nomo_async_io_handle_close_callback) {
        *error_kind = (nomo_async_tcp_error_kind){
            .tag = NOMO_ASYNC_TCP_KIND_CLOSED
        };
        *error_message = "TCP stream is closed or belongs to another executor";
        return 1;
    }
    int acquired = nomo_async_io_handle_acquire(
        context,
        registration->handle_slot,
        registration->handle_generation,
        direction
    );
    if (acquired != 0) {
        *error_kind = (nomo_async_tcp_error_kind){
            .tag = acquired == 2
                ? NOMO_ASYNC_TCP_KIND_BUSY
                : NOMO_ASYNC_TCP_KIND_CLOSED
        };
        *error_message = acquired == 2
            ? "TCP stream direction already has a pending operation"
            : "TCP stream is closed";
        return 1;
    }
    registration->acquired = 1u;
    return 0;
}

static int nomo_async_tcp_read_attempt(
    nomo_async_tcp_io_registration *registration,
    nomo_async_tcp_read_result *result
) {
    nomo_socket handle = nomo_async_io_handle_get(
        registration->context,
        registration->handle_slot,
        registration->handle_generation
    );
    if (handle == NOMO_INVALID_SOCKET) {
        nomo_async_tcp_io_finish(registration);
        nomo_async_tcp_io_release_payload(registration);
        nomo_async_tcp_read_error(
            result,
            (nomo_async_tcp_error_kind){.tag = NOMO_ASYNC_TCP_KIND_CLOSED},
            "TCP stream is closed"
        );
        return 0;
    }
    ssize_t received = recv(
        handle,
        registration->read_buffer,
        registration->read_capacity,
        0
    );
    if (received < 0 && (errno == EAGAIN || errno == EWOULDBLOCK)) {
        return 1;
    }
    if (received < 0) {
        nomo_async_tcp_io_finish(registration);
        nomo_async_tcp_io_release_payload(registration);
        nomo_async_tcp_read_error(
            result,
            (nomo_async_tcp_error_kind){.tag = NOMO_ASYNC_TCP_KIND_READ},
            "TCP read failed"
        );
        registration->context->io_errors += 1u;
        return 0;
    }
    size_t length = (size_t)received;
    nomo_array_u32 data = nomo_array_u32_new();
    for (size_t index = 0u; index < length; index += 1u) {
        data = nomo_array_u32_push(
            data,
            (uint32_t)registration->read_buffer[index]
        );
    }
    nomo_async_tcp_io_finish(registration);
    nomo_async_tcp_io_release_payload(registration);
    memset(result, 0, sizeof(*result));
    result->tag = NOMO_ASYNC_TCP_READ_OK;
    result->payload.nomo_payload_Ok = (nomo_async_tcp_chunk){
        .nomo_member_data = data,
        .nomo_member_eof = received == 0
    };
    return 0;
}

static int nomo_async_tcp_read_string_attempt(
    nomo_async_tcp_io_registration *registration,
    nomo_async_tcp_text_result *result
) {
    nomo_socket handle = nomo_async_io_handle_get(
        registration->context,
        registration->handle_slot,
        registration->handle_generation
    );
    if (handle == NOMO_INVALID_SOCKET) {
        nomo_async_tcp_io_finish(registration);
        nomo_async_tcp_io_release_payload(registration);
        nomo_async_tcp_text_error(
            result,
            (nomo_async_tcp_error_kind){.tag = NOMO_ASYNC_TCP_KIND_CLOSED},
            "TCP stream is closed"
        );
        return 0;
    }
    ssize_t received = recv(
        handle,
        registration->read_buffer,
        registration->read_capacity,
        0
    );
    if (received < 0 && (errno == EAGAIN || errno == EWOULDBLOCK)) {
        return 1;
    }
    if (received < 0) {
        nomo_async_tcp_io_finish(registration);
        nomo_async_tcp_io_release_payload(registration);
        nomo_async_tcp_text_error(
            result,
            (nomo_async_tcp_error_kind){.tag = NOMO_ASYNC_TCP_KIND_READ},
            "TCP text read failed"
        );
        registration->context->io_errors += 1u;
        return 0;
    }
    size_t length = (size_t)received;
    if (!nomo_async_tcp_utf8_valid(registration->read_buffer, length)) {
        nomo_async_tcp_io_finish(registration);
        nomo_async_tcp_io_release_payload(registration);
        nomo_async_tcp_text_error(
            result,
            (nomo_async_tcp_error_kind){.tag = NOMO_ASYNC_TCP_KIND_READ},
            "TCP text chunk is not valid Nomo UTF-8 text"
        );
        registration->context->io_errors += 1u;
        return 0;
    }
    nomo_string data = nomo_string_literal("");
    if (length > 0u) {
        registration->read_buffer[length] = 0u;
        data = nomo_string_owned((char *)registration->read_buffer);
        registration->read_buffer = NULL;
    }
    nomo_async_tcp_io_finish(registration);
    nomo_async_tcp_io_release_payload(registration);
    memset(result, 0, sizeof(*result));
    result->tag = NOMO_ASYNC_TCP_TEXT_OK;
    result->payload.nomo_payload_Ok = (nomo_async_tcp_text_chunk){
        .nomo_member_data = data,
        .nomo_member_eof = received == 0
    };
    return 0;
}

static int nomo_async_tcp_write_attempt(
    nomo_async_tcp_io_registration *registration,
    nomo_async_tcp_write_result *result
) {
    nomo_socket handle = nomo_async_io_handle_get(
        registration->context,
        registration->handle_slot,
        registration->handle_generation
    );
    if (handle == NOMO_INVALID_SOCKET) {
        nomo_async_tcp_io_finish(registration);
        nomo_async_tcp_io_release_payload(registration);
        nomo_async_tcp_write_error(
            result,
            (nomo_async_tcp_error_kind){.tag = NOMO_ASYNC_TCP_KIND_CLOSED},
            "TCP stream is closed"
        );
        return 0;
    }
    size_t sent_this_poll = 0u;
    while (registration->write_offset < registration->write_length) {
        if (sent_this_poll >= NOMO_ASYNC_TCP_WRITE_POLL_BUDGET) {
            nomo_async_tcp_io_set_retained(
                registration,
                registration->write_length - registration->write_offset
            );
            return 1;
        }
        unsigned char scratch[4096];
        size_t remaining =
            registration->write_length - registration->write_offset;
        size_t poll_remaining =
            NOMO_ASYNC_TCP_WRITE_POLL_BUDGET - sent_this_poll;
        size_t chunk = remaining < sizeof(scratch) ? remaining : sizeof(scratch);
        if (chunk > poll_remaining) {
            chunk = poll_remaining;
        }
        for (size_t index = 0u; index < chunk; index += 1u) {
            scratch[index] = (unsigned char)registration->write_bytes.data[
                registration->write_offset + index
            ];
        }
        ssize_t sent = send(
            handle,
            scratch,
            chunk,
            NOMO_ASYNC_TCP_SEND_FLAGS
        );
        if (sent > 0) {
            registration->write_offset += (size_t)sent;
            sent_this_poll += (size_t)sent;
            continue;
        }
        if (sent < 0 && (errno == EAGAIN || errno == EWOULDBLOCK)) {
            nomo_async_tcp_io_set_retained(
                registration,
                registration->write_length - registration->write_offset
            );
            return 1;
        }
        nomo_async_tcp_io_finish(registration);
        nomo_async_tcp_io_release_payload(registration);
        nomo_async_tcp_write_error(
            result,
            (nomo_async_tcp_error_kind){.tag = NOMO_ASYNC_TCP_KIND_WRITE},
            "TCP write failed"
        );
        registration->context->io_errors += 1u;
        return 0;
    }
    nomo_async_tcp_io_finish(registration);
    nomo_async_tcp_io_release_payload(registration);
    nomo_async_tcp_write_success(result);
    return 0;
}

static int nomo_async_tcp_write_string_attempt(
    nomo_async_tcp_io_registration *registration,
    nomo_async_tcp_write_result *result
) {
    nomo_socket handle = nomo_async_io_handle_get(
        registration->context,
        registration->handle_slot,
        registration->handle_generation
    );
    if (handle == NOMO_INVALID_SOCKET) {
        nomo_async_tcp_io_finish(registration);
        nomo_async_tcp_io_release_payload(registration);
        nomo_async_tcp_write_error(
            result,
            (nomo_async_tcp_error_kind){.tag = NOMO_ASYNC_TCP_KIND_CLOSED},
            "TCP stream is closed"
        );
        return 0;
    }
    size_t sent_this_poll = 0u;
    while (registration->write_offset < registration->write_length) {
        if (sent_this_poll >= NOMO_ASYNC_TCP_WRITE_POLL_BUDGET) {
            nomo_async_tcp_io_set_retained(
                registration,
                registration->write_length - registration->write_offset
            );
            return 1;
        }
        size_t remaining =
            registration->write_length - registration->write_offset;
        size_t poll_remaining =
            NOMO_ASYNC_TCP_WRITE_POLL_BUDGET - sent_this_poll;
        size_t chunk = remaining < poll_remaining ? remaining : poll_remaining;
        ssize_t sent = send(
            handle,
            registration->write_text.data + registration->write_offset,
            chunk,
            NOMO_ASYNC_TCP_SEND_FLAGS
        );
        if (sent > 0) {
            registration->write_offset += (size_t)sent;
            sent_this_poll += (size_t)sent;
            continue;
        }
        if (sent < 0 && (errno == EAGAIN || errno == EWOULDBLOCK)) {
            nomo_async_tcp_io_set_retained(
                registration,
                registration->write_length - registration->write_offset
            );
            return 1;
        }
        nomo_async_tcp_io_finish(registration);
        nomo_async_tcp_io_release_payload(registration);
        nomo_async_tcp_write_error(
            result,
            (nomo_async_tcp_error_kind){.tag = NOMO_ASYNC_TCP_KIND_WRITE},
            "TCP string write failed"
        );
        registration->context->io_errors += 1u;
        return 0;
    }
    nomo_async_tcp_io_finish(registration);
    nomo_async_tcp_io_release_payload(registration);
    nomo_async_tcp_write_success(result);
    return 0;
}

static nomo_async_poll nomo_async_tcp_read_start(
    nomo_async_tcp_io_registration *registration,
    nomo_async_tcp_stream stream,
    uint64_t max_bytes,
    uint64_t timeout_millis,
    nomo_async_context *context,
    nomo_async_tcp_read_result *result
) {
    context->io_read_starts += 1u;
    nomo_async_tcp_error_kind error_kind = {0};
    const char *error_message = NULL;
    if (max_bytes == 0u || max_bytes > 1048576u) {
        nomo_async_tcp_read_error(
            result,
            (nomo_async_tcp_error_kind){.tag = NOMO_ASYNC_TCP_KIND_INVALIDINPUT},
            "max_bytes must be in 1..=1048576"
        );
        return NOMO_ASYNC_POLL_READY;
    }
    if (nomo_async_tcp_io_prepare(
            registration,
            stream,
            timeout_millis,
            NOMO_ASYNC_IO_DIRECTION_READ,
            NOMO_ASYNC_REACTOR_READ,
            NOMO_ASYNC_TCP_IO_READ,
            context,
            &error_kind,
            &error_message
        ) != 0) {
        nomo_async_tcp_read_error(result, error_kind, error_message);
        return NOMO_ASYNC_POLL_READY;
    }
    registration->read_capacity = (size_t)max_bytes;
    registration->read_buffer = (uint8_t *)malloc((size_t)max_bytes + 1u);
    if (registration->read_buffer == NULL) {
        nomo_panic("out of memory");
    }
    if (nomo_async_tcp_read_attempt(registration, result) == 0) {
        return NOMO_ASYNC_POLL_READY;
    }
    if (timeout_millis == 0u) {
        nomo_async_tcp_io_finish(registration);
        nomo_async_tcp_io_release_payload(registration);
        nomo_async_tcp_read_error(
            result,
            (nomo_async_tcp_error_kind){.tag = NOMO_ASYNC_TCP_KIND_TIMEOUT},
            "TCP read did not complete immediately"
        );
        context->io_timeouts += 1u;
        return NOMO_ASYNC_POLL_READY;
    }
    nomo_socket handle = nomo_async_io_handle_get(
        context,
        registration->handle_slot,
        registration->handle_generation
    );
    int arm = nomo_async_tcp_io_arm(registration, handle, timeout_millis);
    if (arm != 0) {
        nomo_async_tcp_io_finish(registration);
        nomo_async_tcp_io_release_payload(registration);
        nomo_async_tcp_read_error(
            result,
            (nomo_async_tcp_error_kind){
                .tag = arm == 1
                    ? NOMO_ASYNC_TCP_KIND_REACTOR
                    : NOMO_ASYNC_TCP_KIND_LIMIT
            },
            arm == 1
                ? "reactor registration failed"
                : "owner executor timer capacity is exhausted"
        );
        return NOMO_ASYNC_POLL_READY;
    }
    nomo_async_tcp_io_set_retained(registration, max_bytes);
    return NOMO_ASYNC_POLL_PENDING;
}

static nomo_async_poll nomo_async_tcp_read_string_start(
    nomo_async_tcp_io_registration *registration,
    nomo_async_tcp_stream stream,
    uint64_t max_bytes,
    uint64_t timeout_millis,
    nomo_async_context *context,
    nomo_async_tcp_text_result *result
) {
    context->io_read_starts += 1u;
    nomo_async_tcp_error_kind error_kind = {0};
    const char *error_message = NULL;
    if (max_bytes == 0u || max_bytes > 1048576u) {
        nomo_async_tcp_text_error(
            result,
            (nomo_async_tcp_error_kind){.tag = NOMO_ASYNC_TCP_KIND_INVALIDINPUT},
            "max_bytes must be in 1..=1048576"
        );
        return NOMO_ASYNC_POLL_READY;
    }
    if (nomo_async_tcp_io_prepare(
            registration,
            stream,
            timeout_millis,
            NOMO_ASYNC_IO_DIRECTION_READ,
            NOMO_ASYNC_REACTOR_READ,
            NOMO_ASYNC_TCP_IO_READ_STRING,
            context,
            &error_kind,
            &error_message
        ) != 0) {
        nomo_async_tcp_text_error(result, error_kind, error_message);
        return NOMO_ASYNC_POLL_READY;
    }
    registration->read_capacity = (size_t)max_bytes;
    registration->read_buffer = (uint8_t *)malloc((size_t)max_bytes + 1u);
    if (registration->read_buffer == NULL) {
        nomo_panic("out of memory");
    }
    if (nomo_async_tcp_read_string_attempt(registration, result) == 0) {
        return NOMO_ASYNC_POLL_READY;
    }
    if (timeout_millis == 0u) {
        nomo_async_tcp_io_finish(registration);
        nomo_async_tcp_io_release_payload(registration);
        nomo_async_tcp_text_error(
            result,
            (nomo_async_tcp_error_kind){.tag = NOMO_ASYNC_TCP_KIND_TIMEOUT},
            "TCP text read did not complete immediately"
        );
        context->io_timeouts += 1u;
        return NOMO_ASYNC_POLL_READY;
    }
    nomo_socket handle = nomo_async_io_handle_get(
        context,
        registration->handle_slot,
        registration->handle_generation
    );
    int arm = nomo_async_tcp_io_arm(registration, handle, timeout_millis);
    if (arm != 0) {
        nomo_async_tcp_io_finish(registration);
        nomo_async_tcp_io_release_payload(registration);
        nomo_async_tcp_text_error(
            result,
            (nomo_async_tcp_error_kind){
                .tag = arm == 1
                    ? NOMO_ASYNC_TCP_KIND_REACTOR
                    : NOMO_ASYNC_TCP_KIND_LIMIT
            },
            arm == 1
                ? "reactor registration failed"
                : "owner executor timer capacity is exhausted"
        );
        return NOMO_ASYNC_POLL_READY;
    }
    nomo_async_tcp_io_set_retained(registration, max_bytes);
    return NOMO_ASYNC_POLL_PENDING;
}

static nomo_async_poll nomo_async_tcp_write_start(
    nomo_async_tcp_io_registration *registration,
    nomo_async_tcp_stream stream,
    nomo_array_u32 data,
    uint64_t timeout_millis,
    nomo_async_context *context,
    nomo_async_tcp_write_result *result
) {
    context->io_write_starts += 1u;
    if (data.len > 1048576u || timeout_millis > 900000u) {
        nomo_async_tcp_write_error(
            result,
            (nomo_async_tcp_error_kind){.tag = NOMO_ASYNC_TCP_KIND_INVALIDINPUT},
            "TCP write exceeds the payload or timeout bound"
        );
        return NOMO_ASYNC_POLL_READY;
    }
    for (size_t index = 0u; index < data.len; index += 1u) {
        if (data.data[index] > 255u) {
            nomo_async_tcp_write_error(
                result,
                (nomo_async_tcp_error_kind){.tag = NOMO_ASYNC_TCP_KIND_INVALIDINPUT},
                "TCP byte values must be in 0..=255"
            );
            return NOMO_ASYNC_POLL_READY;
        }
    }
    nomo_async_tcp_error_kind error_kind = {0};
    const char *error_message = NULL;
    if (nomo_async_tcp_io_prepare(
            registration,
            stream,
            timeout_millis,
            NOMO_ASYNC_IO_DIRECTION_WRITE,
            NOMO_ASYNC_REACTOR_WRITE,
            NOMO_ASYNC_TCP_IO_WRITE,
            context,
            &error_kind,
            &error_message
        ) != 0) {
        nomo_async_tcp_write_error(result, error_kind, error_message);
        return NOMO_ASYNC_POLL_READY;
    }
    registration->write_bytes = data;
    registration->write_length = data.len;
    if (nomo_async_tcp_write_attempt(registration, result) == 0) {
        return NOMO_ASYNC_POLL_READY;
    }
    if (timeout_millis == 0u) {
        nomo_async_tcp_io_finish(registration);
        nomo_async_tcp_io_release_payload(registration);
        nomo_async_tcp_write_error(
            result,
            (nomo_async_tcp_error_kind){.tag = NOMO_ASYNC_TCP_KIND_TIMEOUT},
            "TCP write did not complete immediately"
        );
        context->io_timeouts += 1u;
        return NOMO_ASYNC_POLL_READY;
    }
    registration->write_bytes = nomo_array_u32_retain(data);
    registration->payload_owned = 1u;
    nomo_socket handle = nomo_async_io_handle_get(
        context,
        registration->handle_slot,
        registration->handle_generation
    );
    int arm = nomo_async_tcp_io_arm(registration, handle, timeout_millis);
    if (arm != 0) {
        nomo_async_tcp_io_finish(registration);
        nomo_async_tcp_io_release_payload(registration);
        nomo_async_tcp_write_error(
            result,
            (nomo_async_tcp_error_kind){
                .tag = arm == 1
                    ? NOMO_ASYNC_TCP_KIND_REACTOR
                    : NOMO_ASYNC_TCP_KIND_LIMIT
            },
            arm == 1
                ? "reactor registration failed"
                : "owner executor timer capacity is exhausted"
        );
        return NOMO_ASYNC_POLL_READY;
    }
    nomo_async_tcp_io_set_retained(
        registration,
        registration->write_length - registration->write_offset
    );
    return NOMO_ASYNC_POLL_PENDING;
}

static nomo_async_poll nomo_async_tcp_write_string_start(
    nomo_async_tcp_io_registration *registration,
    nomo_async_tcp_stream stream,
    nomo_string content,
    uint64_t timeout_millis,
    nomo_async_context *context,
    nomo_async_tcp_write_result *result
) {
    context->io_write_starts += 1u;
    size_t length = strlen(content.data);
    if (length > 1048576u || timeout_millis > 900000u) {
        nomo_async_tcp_write_error(
            result,
            (nomo_async_tcp_error_kind){.tag = NOMO_ASYNC_TCP_KIND_INVALIDINPUT},
            "TCP string write exceeds the payload or timeout bound"
        );
        return NOMO_ASYNC_POLL_READY;
    }
    nomo_async_tcp_error_kind error_kind = {0};
    const char *error_message = NULL;
    if (nomo_async_tcp_io_prepare(
            registration,
            stream,
            timeout_millis,
            NOMO_ASYNC_IO_DIRECTION_WRITE,
            NOMO_ASYNC_REACTOR_WRITE,
            NOMO_ASYNC_TCP_IO_WRITE_STRING,
            context,
            &error_kind,
            &error_message
        ) != 0) {
        nomo_async_tcp_write_error(result, error_kind, error_message);
        return NOMO_ASYNC_POLL_READY;
    }
    registration->write_text = content;
    registration->write_length = length;
    if (nomo_async_tcp_write_string_attempt(registration, result) == 0) {
        return NOMO_ASYNC_POLL_READY;
    }
    if (timeout_millis == 0u) {
        nomo_async_tcp_io_finish(registration);
        nomo_async_tcp_io_release_payload(registration);
        nomo_async_tcp_write_error(
            result,
            (nomo_async_tcp_error_kind){.tag = NOMO_ASYNC_TCP_KIND_TIMEOUT},
            "TCP string write did not complete immediately"
        );
        context->io_timeouts += 1u;
        return NOMO_ASYNC_POLL_READY;
    }
    registration->write_text = nomo_string_retain(content);
    registration->payload_owned = 1u;
    nomo_socket handle = nomo_async_io_handle_get(
        context,
        registration->handle_slot,
        registration->handle_generation
    );
    int arm = nomo_async_tcp_io_arm(registration, handle, timeout_millis);
    if (arm != 0) {
        nomo_async_tcp_io_finish(registration);
        nomo_async_tcp_io_release_payload(registration);
        nomo_async_tcp_write_error(
            result,
            (nomo_async_tcp_error_kind){
                .tag = arm == 1
                    ? NOMO_ASYNC_TCP_KIND_REACTOR
                    : NOMO_ASYNC_TCP_KIND_LIMIT
            },
            arm == 1
                ? "reactor registration failed"
                : "owner executor timer capacity is exhausted"
        );
        return NOMO_ASYNC_POLL_READY;
    }
    nomo_async_tcp_io_set_retained(
        registration,
        registration->write_length - registration->write_offset
    );
    return NOMO_ASYNC_POLL_PENDING;
}

static int nomo_async_tcp_io_resume_common(
    nomo_async_tcp_io_registration *registration,
    nomo_async_context *context,
    nomo_async_tcp_error_kind *error_kind,
    const char **error_message
) {
    if (registration->timer.expired != 0u) {
        registration->timer.expired = 0u;
        nomo_async_tcp_io_finish(registration);
        nomo_async_tcp_io_release_payload(registration);
        context->io_timeouts += 1u;
        *error_kind = (nomo_async_tcp_error_kind){
            .tag = NOMO_ASYNC_TCP_KIND_TIMEOUT
        };
        *error_message = "TCP operation timed out";
        return 1;
    }
    if (registration->ready == 0u) {
        context->pending_reason = NOMO_ASYNC_PENDING_IO;
        return 2;
    }
    registration->ready = 0u;
    nomo_socket handle = nomo_async_io_handle_get(
        context,
        registration->handle_slot,
        registration->handle_generation
    );
    if (handle == NOMO_INVALID_SOCKET) {
        nomo_async_tcp_io_finish(registration);
        nomo_async_tcp_io_release_payload(registration);
        *error_kind = (nomo_async_tcp_error_kind){
            .tag = NOMO_ASYNC_TCP_KIND_CLOSED
        };
        *error_message = "TCP stream is closed";
        return 1;
    }
    return 0;
}

static int nomo_async_tcp_io_handle_spurious(
    nomo_async_tcp_io_registration *registration,
    nomo_async_tcp_error_kind *error_kind,
    const char **error_message
) {
    int rearm = nomo_async_tcp_io_rearm(registration);
    if (rearm == 0) {
        return 0;
    }
    nomo_async_tcp_io_finish(registration);
    nomo_async_tcp_io_release_payload(registration);
    *error_kind = (nomo_async_tcp_error_kind){
        .tag = rearm == 1
            ? NOMO_ASYNC_TCP_KIND_REACTOR
            : (rearm == 2
                ? NOMO_ASYNC_TCP_KIND_LIMIT
                : NOMO_ASYNC_TCP_KIND_TIMEOUT)
    };
    *error_message = rearm == 1
        ? "reactor re-registration failed"
        : (rearm == 2
            ? "owner executor timer capacity is exhausted"
            : "TCP operation timed out");
    if (rearm == 3) {
        registration->context->io_timeouts += 1u;
    }
    return 1;
}

static nomo_async_poll nomo_async_tcp_read_resume(
    nomo_async_tcp_io_registration *registration,
    nomo_async_context *context,
    nomo_async_tcp_read_result *result
) {
    nomo_async_tcp_error_kind error_kind = {0};
    const char *error_message = NULL;
    int common = nomo_async_tcp_io_resume_common(
        registration,
        context,
        &error_kind,
        &error_message
    );
    if (common == 2) {
        return NOMO_ASYNC_POLL_PENDING;
    }
    if (common == 1) {
        nomo_async_tcp_read_error(result, error_kind, error_message);
        return NOMO_ASYNC_POLL_READY;
    }
    if (nomo_async_tcp_read_attempt(registration, result) == 0) {
        return NOMO_ASYNC_POLL_READY;
    }
    if (nomo_async_tcp_io_handle_spurious(
            registration,
            &error_kind,
            &error_message
        ) != 0) {
        nomo_async_tcp_read_error(result, error_kind, error_message);
        return NOMO_ASYNC_POLL_READY;
    }
    return NOMO_ASYNC_POLL_PENDING;
}

static nomo_async_poll nomo_async_tcp_read_string_resume(
    nomo_async_tcp_io_registration *registration,
    nomo_async_context *context,
    nomo_async_tcp_text_result *result
) {
    nomo_async_tcp_error_kind error_kind = {0};
    const char *error_message = NULL;
    int common = nomo_async_tcp_io_resume_common(
        registration,
        context,
        &error_kind,
        &error_message
    );
    if (common == 2) {
        return NOMO_ASYNC_POLL_PENDING;
    }
    if (common == 1) {
        nomo_async_tcp_text_error(result, error_kind, error_message);
        return NOMO_ASYNC_POLL_READY;
    }
    if (nomo_async_tcp_read_string_attempt(registration, result) == 0) {
        return NOMO_ASYNC_POLL_READY;
    }
    if (nomo_async_tcp_io_handle_spurious(
            registration,
            &error_kind,
            &error_message
        ) != 0) {
        nomo_async_tcp_text_error(result, error_kind, error_message);
        return NOMO_ASYNC_POLL_READY;
    }
    return NOMO_ASYNC_POLL_PENDING;
}

static nomo_async_poll nomo_async_tcp_write_resume(
    nomo_async_tcp_io_registration *registration,
    nomo_async_context *context,
    nomo_async_tcp_write_result *result
) {
    nomo_async_tcp_error_kind error_kind = {0};
    const char *error_message = NULL;
    int common = nomo_async_tcp_io_resume_common(
        registration,
        context,
        &error_kind,
        &error_message
    );
    if (common == 2) {
        return NOMO_ASYNC_POLL_PENDING;
    }
    if (common == 1) {
        nomo_async_tcp_write_error(result, error_kind, error_message);
        return NOMO_ASYNC_POLL_READY;
    }
    if (nomo_async_tcp_write_attempt(registration, result) == 0) {
        return NOMO_ASYNC_POLL_READY;
    }
    if (nomo_async_tcp_io_handle_spurious(
            registration,
            &error_kind,
            &error_message
        ) != 0) {
        nomo_async_tcp_write_error(result, error_kind, error_message);
        return NOMO_ASYNC_POLL_READY;
    }
    return NOMO_ASYNC_POLL_PENDING;
}

static nomo_async_poll nomo_async_tcp_write_string_resume(
    nomo_async_tcp_io_registration *registration,
    nomo_async_context *context,
    nomo_async_tcp_write_result *result
) {
    nomo_async_tcp_error_kind error_kind = {0};
    const char *error_message = NULL;
    int common = nomo_async_tcp_io_resume_common(
        registration,
        context,
        &error_kind,
        &error_message
    );
    if (common == 2) {
        return NOMO_ASYNC_POLL_PENDING;
    }
    if (common == 1) {
        nomo_async_tcp_write_error(result, error_kind, error_message);
        return NOMO_ASYNC_POLL_READY;
    }
    if (nomo_async_tcp_write_string_attempt(registration, result) == 0) {
        return NOMO_ASYNC_POLL_READY;
    }
    if (nomo_async_tcp_io_handle_spurious(
            registration,
            &error_kind,
            &error_message
        ) != 0) {
        nomo_async_tcp_write_error(result, error_kind, error_message);
        return NOMO_ASYNC_POLL_READY;
    }
    return NOMO_ASYNC_POLL_PENDING;
}
"#,
    );
}
