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

typedef struct {
    void *owner;
    nomo_async_reactor_wake_fn wake;
    nomo_socket handle;
    uint32_t interests;
    uint8_t active;
} nomo_async_reactor_registration;

typedef struct {
"#,
    );
    match target.operating_system() {
        OperatingSystem::Linux | OperatingSystem::Darwin => out.push_str("    int handle;\n"),
        OperatingSystem::Windows => out.push_str("    HANDLE handle;\n"),
    }
    out.push_str(
        r#"    uint64_t initializations;
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

static int nomo_async_reactor_register(
    nomo_async_reactor *reactor,
    nomo_async_reactor_registration *registration,
    nomo_socket handle,
    uint32_t interests
) {
    (void)registration;
    (void)handle;
    (void)interests;
    if (nomo_async_reactor_init(reactor) != 0) {
        return 1;
    }
    reactor->errors += 1u;
    return 1;
}

static int nomo_async_reactor_reregister(
    nomo_async_reactor *reactor,
    nomo_async_reactor_registration *registration,
    uint32_t interests
) {
    (void)registration;
    (void)interests;
    reactor->errors += 1u;
    return 1;
}

static void nomo_async_reactor_deregister(
    nomo_async_reactor *reactor,
    nomo_async_reactor_registration *registration
) {
    (void)reactor;
    registration->active = 0u;
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
    ULONG_PTR completion_key = 0;
    LPOVERLAPPED overlapped = NULL;
    BOOL completed = GetQueuedCompletionStatus(
        reactor->handle,
        &transferred,
        &completion_key,
        &overlapped,
        timeout
    );
    if (completed != FALSE) {
        reactor->completions += 1u;
        *had_completion = 1u;
        nomo_async_reactor_registration *registration =
            (nomo_async_reactor_registration *)completion_key;
        if (registration != NULL && registration->wake != NULL) {
            registration->wake(registration->owner, registration->interests);
        }
        return 0;
    }
    if (GetLastError() == WAIT_TIMEOUT) {
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
    fn emits_iocp_timer_wait_and_explicit_registration_stub_for_windows() {
        let emitted = emit_for("x86_64-pc-windows-msvc");
        assert!(emitted.contains("CreateIoCompletionPort"));
        assert!(emitted.contains("GetQueuedCompletionStatus"));
        assert!(emitted.contains("static int nomo_async_reactor_register"));
        assert!(!emitted.contains("epoll_create(1)"));
        assert!(!emitted.contains("kqueue()"));
    }
}
