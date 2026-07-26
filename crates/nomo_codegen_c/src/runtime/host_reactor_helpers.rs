use nomo_target::{OperatingSystem, TargetTriple};

pub(super) fn emit_async_reactor_helpers(out: &mut String, target: &TargetTriple) {
    emit_common_reactor_types(out, target);
    match target.operating_system() {
        OperatingSystem::Linux => emit_epoll_reactor(out),
        OperatingSystem::Darwin => emit_kqueue_reactor(out),
        OperatingSystem::Windows => emit_iocp_reactor(out),
    }
}

fn emit_common_reactor_types(out: &mut String, target: &TargetTriple) {
    match target.operating_system() {
        OperatingSystem::Linux | OperatingSystem::Darwin => {
            out.push_str("#include <arpa/inet.h>\n");
            out.push_str("#include <fcntl.h>\n");
        }
        OperatingSystem::Windows => {}
    }
    if target.operating_system() == OperatingSystem::Linux {
        out.push_str("#include <sys/epoll.h>\n");
    } else if target.operating_system() == OperatingSystem::Darwin {
        out.push_str("#include <sys/event.h>\n");
    }
    out.push_str(
        r#"
#define NOMO_ASYNC_REACTOR_READ 1u
#define NOMO_ASYNC_REACTOR_WRITE 2u

typedef void (*nomo_async_reactor_wake_fn)(void *, uint32_t);

typedef struct nomo_async_reactor_registration nomo_async_reactor_registration;
"#,
    );
    if target.operating_system() == OperatingSystem::Windows {
        out.push_str(
            r#"
#define NOMO_ASYNC_IOCP_OPERATION_CAPACITY 64u

typedef struct {
    OVERLAPPED overlapped;
    nomo_async_reactor_registration *registration;
    void *detached_buffer;
    uint8_t active;
    uint8_t submitted;
} nomo_async_iocp_operation;

"#,
        );
    }
    out.push_str(
        r#"struct nomo_async_reactor_registration {
    void *owner;
    nomo_async_reactor_wake_fn wake;
    nomo_socket handle;
    uint32_t interests;
    uint8_t active;
"#,
    );
    if target.operating_system() == OperatingSystem::Windows {
        out.push_str(
            r#"    nomo_async_iocp_operation *operation;
    DWORD transferred;
    DWORD error;
"#,
        );
    }
    out.push_str(
        r#"};

typedef struct {
"#,
    );
    match target.operating_system() {
        OperatingSystem::Linux | OperatingSystem::Darwin => out.push_str("    int handle;\n"),
        OperatingSystem::Windows => out.push_str(
            r#"    HANDLE handle;
    nomo_async_iocp_operation operations[NOMO_ASYNC_IOCP_OPERATION_CAPACITY];
"#,
        ),
    }
    out.push_str(
        r#"    uint64_t iocp_operations_started;
    uint64_t iocp_operations_completed;
    uint64_t iocp_operations_cancelled;
    uint64_t live_iocp_operations;
    uint64_t peak_live_iocp_operations;
    uint64_t initializations;
    uint64_t waits;
    uint64_t timeouts;
    uint64_t completions;
    uint64_t errors;
    uint64_t shutdowns;
    uint64_t registrations;
    uint64_t deregistrations;
    uint64_t reregistrations;
    uint64_t live_registrations;
    uint64_t peak_live_registrations;
    uint64_t live;
    uint64_t peak_live;
    uint8_t initialized;
} nomo_async_reactor;

"#,
    );
}

fn emit_epoll_reactor(out: &mut String) {
    out.push_str(
        r#"static int nomo_async_reactor_init(nomo_async_reactor *reactor) {
    if (reactor->initialized != 0u) {
        return 0;
    }
    int handle = epoll_create(1);
    if (handle < 0) {
        reactor->errors += 1u;
        return 1;
    }
    if (fcntl(handle, F_SETFD, FD_CLOEXEC) != 0) {
        close(handle);
        reactor->errors += 1u;
        return 1;
    }
    reactor->handle = handle;
    reactor->initializations += 1u;
    reactor->live = 1u;
    reactor->peak_live = 1u;
    reactor->initialized = 1u;
    return 0;
}

static uint32_t nomo_async_reactor_epoll_events(uint32_t interests) {
    uint32_t events = EPOLLERR | EPOLLHUP | EPOLLONESHOT;
    if ((interests & NOMO_ASYNC_REACTOR_READ) != 0u) {
        events |= EPOLLIN;
    }
    if ((interests & NOMO_ASYNC_REACTOR_WRITE) != 0u) {
        events |= EPOLLOUT;
    }
    return events;
}

static int nomo_async_reactor_register(
    nomo_async_reactor *reactor,
    nomo_async_reactor_registration *registration,
    nomo_socket handle,
    uint32_t interests
) {
    if (registration->active != 0u
        || nomo_async_reactor_init(reactor) != 0) {
        reactor->errors += registration->active != 0u;
        return 1;
    }
    struct epoll_event event;
    memset(&event, 0, sizeof(event));
    event.events = nomo_async_reactor_epoll_events(interests);
    event.data.ptr = registration;
    if (epoll_ctl(reactor->handle, EPOLL_CTL_ADD, handle, &event) != 0) {
        reactor->errors += 1u;
        return 1;
    }
    registration->handle = handle;
    registration->interests = interests;
    registration->active = 1u;
    reactor->registrations += 1u;
    reactor->live_registrations += 1u;
    if (reactor->live_registrations > reactor->peak_live_registrations) {
        reactor->peak_live_registrations = reactor->live_registrations;
    }
    return 0;
}

static int nomo_async_reactor_reregister(
    nomo_async_reactor *reactor,
    nomo_async_reactor_registration *registration,
    uint32_t interests
) {
    if (registration->active == 0u) {
        return 1;
    }
    struct epoll_event event;
    memset(&event, 0, sizeof(event));
    event.events = nomo_async_reactor_epoll_events(interests);
    event.data.ptr = registration;
    if (epoll_ctl(
            reactor->handle,
            EPOLL_CTL_MOD,
            registration->handle,
            &event
        ) != 0) {
        reactor->errors += 1u;
        return 1;
    }
    registration->interests = interests;
    reactor->reregistrations += 1u;
    return 0;
}

static void nomo_async_reactor_deregister(
    nomo_async_reactor *reactor,
    nomo_async_reactor_registration *registration
) {
    if (registration->active == 0u) {
        return;
    }
    if (epoll_ctl(
            reactor->handle,
            EPOLL_CTL_DEL,
            registration->handle,
            NULL
        ) != 0 && errno != ENOENT) {
        reactor->errors += 1u;
    }
    registration->active = 0u;
    reactor->deregistrations += 1u;
    if (reactor->live_registrations > 0u) {
        reactor->live_registrations -= 1u;
    }
}

static int nomo_async_reactor_wait(
    nomo_async_reactor *reactor,
    int64_t timeout_millis,
    uint8_t *had_completion
) {
    *had_completion = 0u;
    if (nomo_async_reactor_init(reactor) != 0) {
        return 1;
    }
    reactor->waits += 1u;
    int timeout = timeout_millis < 0
        ? -1
        : (timeout_millis > INT_MAX ? INT_MAX : (int)timeout_millis);
    struct epoll_event event;
    int status;
    do {
        status = epoll_wait(reactor->handle, &event, 1, timeout);
    } while (status < 0 && errno == EINTR);
    if (status < 0) {
        reactor->errors += 1u;
        return 1;
    }
    if (status == 0) {
        reactor->timeouts += 1u;
        return 0;
    }
    reactor->completions += 1u;
    *had_completion = 1u;
    nomo_async_reactor_registration *registration =
        (nomo_async_reactor_registration *)event.data.ptr;
    if (registration != NULL
        && registration->active != 0u
        && registration->wake != NULL) {
        uint32_t ready = 0u;
        if ((event.events & (EPOLLIN | EPOLLHUP | EPOLLERR)) != 0u) {
            ready |= NOMO_ASYNC_REACTOR_READ;
        }
        if ((event.events & (EPOLLOUT | EPOLLHUP | EPOLLERR)) != 0u) {
            ready |= NOMO_ASYNC_REACTOR_WRITE;
        }
        registration->wake(registration->owner, ready);
    }
    return 0;
}

static void nomo_async_reactor_shutdown(nomo_async_reactor *reactor) {
    if (reactor->initialized == 0u) {
        return;
    }
    if (close(reactor->handle) != 0) {
        reactor->errors += 1u;
    }
    reactor->handle = -1;
    reactor->shutdowns += 1u;
    reactor->live = 0u;
    reactor->initialized = 0u;
}

"#,
    );
}

fn emit_kqueue_reactor(out: &mut String) {
    out.push_str(
        r#"static int nomo_async_reactor_init(nomo_async_reactor *reactor) {
    if (reactor->initialized != 0u) {
        return 0;
    }
    int handle = kqueue();
    if (handle < 0) {
        reactor->errors += 1u;
        return 1;
    }
    if (fcntl(handle, F_SETFD, FD_CLOEXEC) != 0) {
        close(handle);
        reactor->errors += 1u;
        return 1;
    }
    reactor->handle = handle;
    reactor->initializations += 1u;
    reactor->live = 1u;
    reactor->peak_live = 1u;
    reactor->initialized = 1u;
    return 0;
}

static int16_t nomo_async_reactor_kqueue_filter(uint32_t interests) {
    return (interests & NOMO_ASYNC_REACTOR_WRITE) != 0u
        ? EVFILT_WRITE
        : EVFILT_READ;
}

static int nomo_async_reactor_register(
    nomo_async_reactor *reactor,
    nomo_async_reactor_registration *registration,
    nomo_socket handle,
    uint32_t interests
) {
    if (registration->active != 0u
        || nomo_async_reactor_init(reactor) != 0) {
        reactor->errors += registration->active != 0u;
        return 1;
    }
    struct kevent change;
    EV_SET(
        &change,
        (uintptr_t)handle,
        nomo_async_reactor_kqueue_filter(interests),
        EV_ADD | EV_ONESHOT,
        0,
        0,
        registration
    );
    if (kevent(reactor->handle, &change, 1, NULL, 0, NULL) != 0) {
        reactor->errors += 1u;
        return 1;
    }
    registration->handle = handle;
    registration->interests = interests;
    registration->active = 1u;
    reactor->registrations += 1u;
    reactor->live_registrations += 1u;
    if (reactor->live_registrations > reactor->peak_live_registrations) {
        reactor->peak_live_registrations = reactor->live_registrations;
    }
    return 0;
}

static int nomo_async_reactor_reregister(
    nomo_async_reactor *reactor,
    nomo_async_reactor_registration *registration,
    uint32_t interests
) {
    if (registration->active == 0u) {
        return 1;
    }
    struct kevent change;
    EV_SET(
        &change,
        (uintptr_t)registration->handle,
        nomo_async_reactor_kqueue_filter(interests),
        EV_ADD | EV_ONESHOT,
        0,
        0,
        registration
    );
    if (kevent(reactor->handle, &change, 1, NULL, 0, NULL) != 0) {
        reactor->errors += 1u;
        return 1;
    }
    registration->interests = interests;
    reactor->reregistrations += 1u;
    return 0;
}

static void nomo_async_reactor_deregister(
    nomo_async_reactor *reactor,
    nomo_async_reactor_registration *registration
) {
    if (registration->active == 0u) {
        return;
    }
    struct kevent change;
    EV_SET(
        &change,
        (uintptr_t)registration->handle,
        nomo_async_reactor_kqueue_filter(registration->interests),
        EV_DELETE,
        0,
        0,
        NULL
    );
    if (kevent(reactor->handle, &change, 1, NULL, 0, NULL) != 0
        && errno != ENOENT) {
        reactor->errors += 1u;
    }
    registration->active = 0u;
    reactor->deregistrations += 1u;
    if (reactor->live_registrations > 0u) {
        reactor->live_registrations -= 1u;
    }
}

static int nomo_async_reactor_wait(
    nomo_async_reactor *reactor,
    int64_t timeout_millis,
    uint8_t *had_completion
) {
    *had_completion = 0u;
    if (nomo_async_reactor_init(reactor) != 0) {
        return 1;
    }
    reactor->waits += 1u;
    struct timespec timeout = {
        .tv_sec = timeout_millis < 0 ? 0 : (time_t)(timeout_millis / 1000),
        .tv_nsec = timeout_millis < 0
            ? 0
            : (long)((timeout_millis % 1000) * 1000000)
    };
    const struct timespec *timeout_ptr = timeout_millis < 0 ? NULL : &timeout;
    struct kevent event;
    int status;
    do {
        status = kevent(reactor->handle, NULL, 0, &event, 1, timeout_ptr);
    } while (status < 0 && errno == EINTR);
    if (status < 0) {
        reactor->errors += 1u;
        return 1;
    }
    if (status == 0) {
        reactor->timeouts += 1u;
        return 0;
    }
    reactor->completions += 1u;
    *had_completion = 1u;
    nomo_async_reactor_registration *registration =
        (nomo_async_reactor_registration *)event.udata;
    if (registration != NULL
        && registration->active != 0u
        && registration->wake != NULL) {
        uint32_t ready = event.filter == EVFILT_WRITE
            ? NOMO_ASYNC_REACTOR_WRITE
            : NOMO_ASYNC_REACTOR_READ;
        registration->wake(registration->owner, ready);
    }
    return 0;
}

static void nomo_async_reactor_shutdown(nomo_async_reactor *reactor) {
    if (reactor->initialized == 0u) {
        return;
    }
    if (close(reactor->handle) != 0) {
        reactor->errors += 1u;
    }
    reactor->handle = -1;
    reactor->shutdowns += 1u;
    reactor->live = 0u;
    reactor->initialized = 0u;
}

"#,
    );
}

fn emit_iocp_reactor(out: &mut String) {
    out.push_str(
        r#"static int nomo_async_reactor_init(nomo_async_reactor *reactor) {
    if (reactor->initialized != 0u) {
        return 0;
    }
    HANDLE handle = CreateIoCompletionPort(INVALID_HANDLE_VALUE, NULL, 0, 1);
    if (handle == NULL) {
        reactor->errors += 1u;
        return 1;
    }
    reactor->handle = handle;
    reactor->initializations += 1u;
    reactor->live = 1u;
    reactor->peak_live = 1u;
    reactor->initialized = 1u;
    return 0;
}

static nomo_async_iocp_operation *nomo_async_iocp_reserve(
    nomo_async_reactor *reactor,
    nomo_async_reactor_registration *registration
) {
    for (uint32_t index = 0u;
         index < NOMO_ASYNC_IOCP_OPERATION_CAPACITY;
         index += 1u) {
        nomo_async_iocp_operation *operation = &reactor->operations[index];
        if (operation->active != 0u) {
            continue;
        }
        memset(operation, 0, sizeof(*operation));
        operation->active = 1u;
        operation->registration = registration;
        registration->operation = operation;
        reactor->iocp_operations_started += 1u;
        reactor->live_iocp_operations += 1u;
        if (reactor->live_iocp_operations
            > reactor->peak_live_iocp_operations) {
            reactor->peak_live_iocp_operations =
                reactor->live_iocp_operations;
        }
        return operation;
    }
    return NULL;
}

static void nomo_async_iocp_release(
    nomo_async_reactor *reactor,
    nomo_async_iocp_operation *operation
) {
    if (operation == NULL || operation->active == 0u) {
        return;
    }
    if (operation->detached_buffer != NULL) {
        free(operation->detached_buffer);
    }
    memset(operation, 0, sizeof(*operation));
    if (reactor->live_iocp_operations > 0u) {
        reactor->live_iocp_operations -= 1u;
    }
}

static int nomo_async_reactor_associate_socket(
    nomo_async_reactor *reactor,
    nomo_socket handle
) {
    if (nomo_async_reactor_init(reactor) != 0) {
        return 1;
    }
    HANDLE associated = CreateIoCompletionPort(
        (HANDLE)handle,
        reactor->handle,
        0,
        0
    );
    if (associated != reactor->handle) {
        reactor->errors += 1u;
        return 1;
    }
    return 0;
}

static int nomo_async_reactor_register(
    nomo_async_reactor *reactor,
    nomo_async_reactor_registration *registration,
    nomo_socket handle,
    uint32_t interests
) {
    if (registration->active != 0u
        || nomo_async_reactor_init(reactor) != 0) {
        reactor->errors += registration->active != 0u;
        return 1;
    }
    nomo_async_iocp_operation *operation =
        nomo_async_iocp_reserve(reactor, registration);
    if (operation == NULL) {
        return 1;
    }
    registration->handle = handle;
    registration->interests = interests;
    registration->transferred = 0u;
    registration->error = 0u;
    registration->active = 1u;
    reactor->registrations += 1u;
    reactor->live_registrations += 1u;
    if (reactor->live_registrations > reactor->peak_live_registrations) {
        reactor->peak_live_registrations = reactor->live_registrations;
    }
    return 0;
}

static int nomo_async_reactor_reregister(
    nomo_async_reactor *reactor,
    nomo_async_reactor_registration *registration,
    uint32_t interests
) {
    if (registration->active == 0u || registration->operation != NULL) {
        return 1;
    }
    nomo_async_iocp_operation *operation =
        nomo_async_iocp_reserve(reactor, registration);
    if (operation == NULL) {
        return 1;
    }
    registration->interests = interests;
    registration->transferred = 0u;
    registration->error = 0u;
    reactor->reregistrations += 1u;
    return 0;
}

static OVERLAPPED *nomo_async_reactor_overlapped(
    nomo_async_reactor_registration *registration
) {
    return registration->operation == NULL
        ? NULL
        : &registration->operation->overlapped;
}

static void nomo_async_reactor_mark_submitted(
    nomo_async_reactor_registration *registration
) {
    if (registration->operation != NULL) {
        registration->operation->submitted = 1u;
    }
}

static void nomo_async_reactor_detach_buffer(
    nomo_async_reactor_registration *registration,
    void *buffer
) {
    if (registration->operation != NULL) {
        registration->operation->detached_buffer = buffer;
    } else {
        free(buffer);
    }
}

static void nomo_async_reactor_deregister(
    nomo_async_reactor *reactor,
    nomo_async_reactor_registration *registration
) {
    if (registration->active == 0u) {
        return;
    }
    nomo_async_iocp_operation *operation = registration->operation;
    if (operation != NULL) {
        operation->registration = NULL;
        registration->operation = NULL;
        if (operation->submitted != 0u) {
            BOOL cancelled = CancelIoEx(
                (HANDLE)registration->handle,
                &operation->overlapped
            );
            DWORD error = cancelled != FALSE ? ERROR_SUCCESS : GetLastError();
            if (cancelled != FALSE || error == ERROR_NOT_FOUND) {
                reactor->iocp_operations_cancelled += 1u;
            } else {
                reactor->errors += 1u;
            }
        } else {
            nomo_async_iocp_release(reactor, operation);
        }
    }
    registration->active = 0u;
    reactor->deregistrations += 1u;
    if (reactor->live_registrations > 0u) {
        reactor->live_registrations -= 1u;
    }
}

static int nomo_async_iocp_dispatch(
    nomo_async_reactor *reactor,
    BOOL completed,
    DWORD transferred,
    LPOVERLAPPED overlapped,
    DWORD error,
    uint8_t *had_completion
) {
    if (overlapped == NULL) {
        return 1;
    }
    nomo_async_iocp_operation *operation =
        CONTAINING_RECORD(overlapped, nomo_async_iocp_operation, overlapped);
    uintptr_t operation_address = (uintptr_t)operation;
    uintptr_t operations_begin = (uintptr_t)&reactor->operations[0];
    uintptr_t operations_end = (uintptr_t)
        &reactor->operations[NOMO_ASYNC_IOCP_OPERATION_CAPACITY];
    if (operation_address < operations_begin
        || operation_address >= operations_end
        || (operation_address - operations_begin) % sizeof(*operation) != 0u
        || operation->active == 0u) {
        reactor->errors += 1u;
        return 1;
    }
    reactor->completions += 1u;
    reactor->iocp_operations_completed += 1u;
    *had_completion = 1u;
    nomo_async_reactor_registration *registration = operation->registration;
    if (registration != NULL
        && registration->active != 0u
        && registration->operation == operation) {
        registration->operation = NULL;
        registration->transferred = transferred;
        registration->error = completed != FALSE ? ERROR_SUCCESS : error;
        nomo_async_iocp_release(reactor, operation);
        if (registration->wake != NULL) {
            registration->wake(registration->owner, registration->interests);
        }
    } else {
        nomo_async_iocp_release(reactor, operation);
    }
    return 0;
}

static int nomo_async_reactor_wait(
    nomo_async_reactor *reactor,
    int64_t timeout_millis,
    uint8_t *had_completion
) {
    *had_completion = 0u;
    if (nomo_async_reactor_init(reactor) != 0) {
        return 1;
    }
    reactor->waits += 1u;
    DWORD timeout = timeout_millis < 0
        ? INFINITE
        : (timeout_millis > MAXDWORD ? MAXDWORD : (DWORD)timeout_millis);
    DWORD transferred = 0;
    ULONG_PTR completion_key = 0u;
    LPOVERLAPPED overlapped = NULL;
    BOOL completed = GetQueuedCompletionStatus(
        reactor->handle,
        &transferred,
        &completion_key,
        &overlapped,
        timeout
    );
    (void)completion_key;
    DWORD error = completed != FALSE ? ERROR_SUCCESS : GetLastError();
    if (overlapped != NULL) {
        return nomo_async_iocp_dispatch(
            reactor,
            completed,
            transferred,
            overlapped,
            error,
            had_completion
        );
    }
    if (error == WAIT_TIMEOUT) {
        reactor->timeouts += 1u;
        return 0;
    }
    reactor->errors += 1u;
    return 1;
}

static void nomo_async_reactor_shutdown(nomo_async_reactor *reactor) {
    if (reactor->initialized == 0u) {
        return;
    }
    for (uint32_t index = 0u;
         index < NOMO_ASYNC_IOCP_OPERATION_CAPACITY;
         index += 1u) {
        nomo_async_iocp_operation *operation = &reactor->operations[index];
        if (operation->active == 0u) {
            continue;
        }
        if (operation->submitted == 0u) {
            nomo_async_iocp_release(reactor, operation);
            continue;
        }
        if (operation->registration != NULL) {
            nomo_async_reactor_deregister(
                reactor,
                operation->registration
            );
        }
    }
    while (reactor->live_iocp_operations > 0u) {
        uint8_t had_completion = 0u;
        if (nomo_async_reactor_wait(reactor, -1, &had_completion) != 0) {
            break;
        }
    }
    if (CloseHandle(reactor->handle) == 0) {
        reactor->errors += 1u;
    }
    reactor->handle = NULL;
    reactor->shutdowns += 1u;
    reactor->live = 0u;
    reactor->initialized = 0u;
}

"#,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn emit_for(target: &str) -> String {
        let target = target.parse::<TargetTriple>().unwrap();
        let mut out = String::new();
        emit_async_reactor_helpers(&mut out, &target);
        out
    }

    #[test]
    fn emits_epoll_registration_for_linux() {
        let emitted = emit_for("x86_64-unknown-linux-gnu");
        assert!(emitted.contains("epoll_create(1)"));
        assert!(emitted.contains("EPOLLONESHOT"));
        assert!(emitted.contains("EPOLL_CTL_ADD"));
        assert!(emitted.contains("EPOLL_CTL_DEL"));
        assert!(!emitted.contains("kqueue()"));
    }

    #[test]
    fn emits_kqueue_registration_for_macos() {
        let emitted = emit_for("aarch64-apple-darwin");
        assert!(emitted.contains("kqueue()"));
        assert!(emitted.contains("EV_ADD | EV_ONESHOT"));
        assert!(emitted.contains("EV_DELETE"));
        assert!(!emitted.contains("epoll_create(1)"));
    }

    #[test]
    fn emits_bounded_iocp_operation_lifecycle_for_windows() {
        let emitted = emit_for("x86_64-pc-windows-msvc");
        assert!(emitted.contains("CreateIoCompletionPort"));
        assert!(emitted.contains("GetQueuedCompletionStatus"));
        assert!(emitted.contains("#define NOMO_ASYNC_IOCP_OPERATION_CAPACITY 64u"));
        assert!(emitted.contains("static int nomo_async_reactor_register"));
        assert!(emitted.contains("CancelIoEx"));
        assert!(emitted.contains("CONTAINING_RECORD"));
        assert!(emitted.contains("while (reactor->live_iocp_operations > 0u)"));
        assert!(!emitted.contains("epoll_create(1)"));
        assert!(!emitted.contains("kqueue()"));
    }
}
