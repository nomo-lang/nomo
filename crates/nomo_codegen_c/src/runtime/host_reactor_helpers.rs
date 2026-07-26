use nomo_target::{OperatingSystem, TargetTriple};

pub(super) fn emit_async_reactor_helpers(out: &mut String, target: &TargetTriple) {
    match target.operating_system() {
        OperatingSystem::Linux => emit_epoll_reactor(out),
        OperatingSystem::Darwin => emit_kqueue_reactor(out),
        OperatingSystem::Windows => emit_iocp_reactor(out),
    }
}

fn emit_epoll_reactor(out: &mut String) {
    out.push_str(
        r#"#include <fcntl.h>
#include <sys/epoll.h>

typedef struct {
    int handle;
    uint64_t initializations;
    uint64_t waits;
    uint64_t timeouts;
    uint64_t completions;
    uint64_t errors;
    uint64_t shutdowns;
    uint64_t live;
    uint64_t peak_live;
    uint8_t initialized;
} nomo_async_reactor;

static int nomo_async_reactor_init(nomo_async_reactor *reactor) {
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

static int nomo_async_reactor_wait(
    nomo_async_reactor *reactor,
    int64_t timeout_millis
) {
    if (nomo_async_reactor_init(reactor) != 0) {
        return 1;
    }
    reactor->waits += 1u;
    int timeout = timeout_millis > INT_MAX ? INT_MAX : (int)timeout_millis;
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
    } else {
        reactor->completions += (uint64_t)status;
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
        r#"#include <fcntl.h>
#include <sys/event.h>

typedef struct {
    int handle;
    uint64_t initializations;
    uint64_t waits;
    uint64_t timeouts;
    uint64_t completions;
    uint64_t errors;
    uint64_t shutdowns;
    uint64_t live;
    uint64_t peak_live;
    uint8_t initialized;
} nomo_async_reactor;

static int nomo_async_reactor_init(nomo_async_reactor *reactor) {
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

static int nomo_async_reactor_wait(
    nomo_async_reactor *reactor,
    int64_t timeout_millis
) {
    if (nomo_async_reactor_init(reactor) != 0) {
        return 1;
    }
    reactor->waits += 1u;
    struct timespec timeout = {
        .tv_sec = (time_t)(timeout_millis / 1000),
        .tv_nsec = (long)((timeout_millis % 1000) * 1000000)
    };
    struct kevent event;
    int status;
    do {
        status = kevent(reactor->handle, NULL, 0, &event, 1, &timeout);
    } while (status < 0 && errno == EINTR);
    if (status < 0) {
        reactor->errors += 1u;
        return 1;
    }
    if (status == 0) {
        reactor->timeouts += 1u;
    } else {
        reactor->completions += (uint64_t)status;
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
        r#"typedef struct {
    HANDLE handle;
    uint64_t initializations;
    uint64_t waits;
    uint64_t timeouts;
    uint64_t completions;
    uint64_t errors;
    uint64_t shutdowns;
    uint64_t live;
    uint64_t peak_live;
    uint8_t initialized;
} nomo_async_reactor;

static int nomo_async_reactor_init(nomo_async_reactor *reactor) {
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

static int nomo_async_reactor_wait(
    nomo_async_reactor *reactor,
    int64_t timeout_millis
) {
    if (nomo_async_reactor_init(reactor) != 0) {
        return 1;
    }
    reactor->waits += 1u;
    DWORD timeout = timeout_millis > MAXDWORD ? MAXDWORD : (DWORD)timeout_millis;
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
    fn emits_epoll_for_linux() {
        let emitted = emit_for("x86_64-unknown-linux-gnu");

        assert!(emitted.contains("#include <sys/epoll.h>"));
        assert!(emitted.contains("epoll_create(1)"));
        assert!(emitted.contains("epoll_wait("));
        assert!(!emitted.contains("kqueue()"));
        assert!(!emitted.contains("CreateIoCompletionPort"));
    }

    #[test]
    fn emits_kqueue_for_macos() {
        let emitted = emit_for("aarch64-apple-darwin");

        assert!(emitted.contains("#include <sys/event.h>"));
        assert!(emitted.contains("kqueue()"));
        assert!(emitted.contains("kevent("));
        assert!(!emitted.contains("epoll_create"));
        assert!(!emitted.contains("CreateIoCompletionPort"));
    }

    #[test]
    fn emits_iocp_for_windows() {
        let emitted = emit_for("x86_64-pc-windows-msvc");

        assert!(emitted.contains("CreateIoCompletionPort"));
        assert!(emitted.contains("GetQueuedCompletionStatus"));
        assert!(emitted.contains("CloseHandle"));
        assert!(!emitted.contains("epoll_create"));
        assert!(!emitted.contains("kqueue()"));
    }
}
