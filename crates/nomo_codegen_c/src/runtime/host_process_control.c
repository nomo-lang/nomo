#define NOMO_PROCESS_MAX_PAYLOAD_BYTES (UINT64_C(1024) * UINT64_C(1024))
#define NOMO_PROCESS_MAX_TIMEOUT_MILLIS (UINT64_C(15) * UINT64_C(60) * UINT64_C(1000))
#define NOMO_PROCESS_BUFFER_SLACK 4U

#ifndef _WIN32
#include <fcntl.h>
#include <poll.h>
#include <signal.h>
extern char **environ;
#endif

typedef struct nomo_process_buffer {
    char *data;
    size_t len;
    size_t cap;
} nomo_process_buffer;

struct nomo_process_control_state;

#ifdef _WIN32
typedef struct nomo_process_reader_context {
    struct nomo_process_control_state *state;
    int is_stderr;
} nomo_process_reader_context;
#endif

typedef struct nomo_process_control_state {
    uint64_t id;
    int stdin_closed;
    int stdin_pending;
    int stdin_flushed;
    char *stdin_data;
    size_t stdin_len;
    size_t stdin_offset;
    nomo_process_buffer stdout_buffer;
    nomo_process_buffer stderr_buffer;
    int stdout_eof;
    int stderr_eof;
    int exited;
    int exit_emitted;
    @PROCESS_EXIT@ exit_info;
    int prefer_stderr;
#ifdef _WIN32
    HANDLE process_handle;
    HANDLE stdin_write;
    HANDLE stdout_read;
    HANDLE stderr_read;
    HANDLE activity_event;
    HANDLE write_event;
    HANDLE stdout_space_event;
    HANDLE stderr_space_event;
    HANDLE writer_thread;
    HANDLE stdout_thread;
    HANDLE stderr_thread;
    CRITICAL_SECTION lock;
    int lock_initialized;
    int closing;
    int stdin_error;
    nomo_process_reader_context stdout_context;
    nomo_process_reader_context stderr_context;
#else
    pid_t pid;
    int stdin_fd;
    int stdout_fd;
    int stderr_fd;
#endif
    struct nomo_process_control_state *next;
} nomo_process_control_state;

static nomo_process_control_state *nomo_process_control_states = NULL;
static uint64_t nomo_process_control_next_id = UINT64_C(1);

static @PROCESS_CONTROL_ERROR@ nomo_process_control_error_value(
    const char *code,
    const char *message
) {
    return (@PROCESS_CONTROL_ERROR@){
        .@CODE_MEMBER@ = nomo_string_from_cstr(code),
        .@MESSAGE_MEMBER@ = nomo_string_from_cstr(message)
    };
}

static @START_RESULT@ nomo_process_control_start_error(
    const char *code,
    const char *message
) {
    return (@START_RESULT@){
        .tag = @START_ERR@,
        .payload.@ERR_PAYLOAD@ = nomo_process_control_error_value(code, message)
    };
}

static @VOID_RESULT@ nomo_process_control_void_error(
    const char *code,
    const char *message
) {
    return (@VOID_RESULT@){
        .tag = @VOID_ERR@,
        .payload.@ERR_PAYLOAD@ = nomo_process_control_error_value(code, message)
    };
}

static @VOID_RESULT@ nomo_process_control_void_ok(void) {
    return (@VOID_RESULT@){
        .tag = @VOID_OK@,
        .payload.@OK_PAYLOAD@ = 0
    };
}

static @EVENT_RESULT@ nomo_process_control_event_error(
    const char *code,
    const char *message
) {
    return (@EVENT_RESULT@){
        .tag = @EVENT_ERR@,
        .payload.@ERR_PAYLOAD@ = nomo_process_control_error_value(code, message)
    };
}

static @WAIT_RESULT@ nomo_process_control_wait_error(
    const char *code,
    const char *message
) {
    return (@WAIT_RESULT@){
        .tag = @WAIT_ERR@,
        .payload.@ERR_PAYLOAD@ = nomo_process_control_error_value(code, message)
    };
}

static @PROCESS_EXIT@ nomo_process_control_exit_value(
    int32_t code,
    int32_t signal
) {
    return (@PROCESS_EXIT@){
        .@CODE_MEMBER@ = code,
        .@SIGNAL_MEMBER@ = signal
    };
}

static @EVENT_RESULT@ nomo_process_control_stdin_flushed_event(void) {
    return (@EVENT_RESULT@){
        .tag = @EVENT_OK@,
        .payload.@OK_PAYLOAD@ = (@PROCESS_EVENT@){
            .tag = @EVENT_STDIN_FLUSHED@
        }
    };
}

static @EVENT_RESULT@ nomo_process_control_text_event(
    int is_stderr,
    nomo_string text
) {
    @PROCESS_EVENT@ event;
    if (is_stderr) {
        event = (@PROCESS_EVENT@){
            .tag = @EVENT_STDERR@,
            .payload.@STDERR_PAYLOAD@ = text
        };
    } else {
        event = (@PROCESS_EVENT@){
            .tag = @EVENT_STDOUT@,
            .payload.@STDOUT_PAYLOAD@ = text
        };
    }
    return (@EVENT_RESULT@){
        .tag = @EVENT_OK@,
        .payload.@OK_PAYLOAD@ = event
    };
}

static @EVENT_RESULT@ nomo_process_control_exited_event(
    @PROCESS_EXIT@ exit_info
) {
    return (@EVENT_RESULT@){
        .tag = @EVENT_OK@,
        .payload.@OK_PAYLOAD@ = (@PROCESS_EVENT@){
            .tag = @EVENT_EXITED@,
            .payload.@EXITED_PAYLOAD@ = exit_info
        }
    };
}

static @WAIT_RESULT@ nomo_process_control_wait_none(void) {
    return (@WAIT_RESULT@){
        .tag = @WAIT_OK@,
        .payload.@OK_PAYLOAD@ = (@EXIT_OPTION@){
            .tag = @EXIT_NONE@
        }
    };
}

static @WAIT_RESULT@ nomo_process_control_wait_some(
    @PROCESS_EXIT@ exit_info
) {
    return (@WAIT_RESULT@){
        .tag = @WAIT_OK@,
        .payload.@OK_PAYLOAD@ = (@EXIT_OPTION@){
            .tag = @EXIT_SOME@,
            .payload.@SOME_PAYLOAD@ = exit_info
        }
    };
}

static void nomo_process_buffer_reserve(
    nomo_process_buffer *buffer,
    size_t needed
) {
    if (needed <= buffer->cap) { return; }
    size_t cap = buffer->cap == 0 ? 4096U : buffer->cap;
    while (cap < needed) {
        if (cap > SIZE_MAX / 2U) { nomo_panic("out of memory"); }
        cap *= 2U;
    }
    char *next = (char *)realloc(buffer->data, cap);
    if (next == NULL) { nomo_panic("out of memory"); }
    buffer->data = next;
    buffer->cap = cap;
}

static void nomo_process_buffer_append(
    nomo_process_buffer *buffer,
    const char *data,
    size_t len
) {
    if (len == 0) { return; }
    if (len > SIZE_MAX - buffer->len) { nomo_panic("out of memory"); }
    nomo_process_buffer_reserve(buffer, buffer->len + len);
    memcpy(buffer->data + buffer->len, data, len);
    buffer->len += len;
}

static void nomo_process_buffer_consume(
    nomo_process_buffer *buffer,
    size_t len
) {
    if (len >= buffer->len) {
        buffer->len = 0;
        return;
    }
    memmove(buffer->data, buffer->data + len, buffer->len - len);
    buffer->len -= len;
}

static void nomo_process_buffer_release(nomo_process_buffer *buffer) {
    free(buffer->data);
    buffer->data = NULL;
    buffer->len = 0;
    buffer->cap = 0;
}

static int nomo_process_utf8_width(
    const unsigned char *data,
    size_t len,
    size_t *width
) {
    if (len == 0) { return 0; }
    unsigned char first = data[0];
    if (first == 0) { return -1; }
    if (first <= 0x7f) {
        *width = 1;
        return 1;
    }
    size_t count = 0;
    uint32_t value = 0;
    uint32_t minimum = 0;
    if (first >= 0xc2 && first <= 0xdf) {
        count = 2;
        value = (uint32_t)(first & 0x1f);
        minimum = 0x80;
    } else if (first >= 0xe0 && first <= 0xef) {
        count = 3;
        value = (uint32_t)(first & 0x0f);
        minimum = 0x800;
    } else if (first >= 0xf0 && first <= 0xf4) {
        count = 4;
        value = (uint32_t)(first & 0x07);
        minimum = 0x10000;
    } else {
        return -1;
    }
    if (len < count) { return 0; }
    for (size_t index = 1; index < count; index += 1) {
        if ((data[index] & 0xc0) != 0x80) { return -1; }
        value = (value << 6) | (uint32_t)(data[index] & 0x3f);
    }
    if (value < minimum || value > 0x10ffff
        || (value >= 0xd800 && value <= 0xdfff)) {
        return -1;
    }
    *width = count;
    return 1;
}

static int nomo_process_utf8_prefix(
    const char *data,
    size_t len,
    size_t limit,
    int eof,
    size_t *prefix
) {
    size_t index = 0;
    size_t maximum = len < limit ? len : limit;
    while (index < maximum) {
        size_t width = 0;
        int status = nomo_process_utf8_width(
            (const unsigned char *)data + index,
            len - index,
            &width
        );
        if (status < 0) { return 0; }
        if (status == 0) {
            if (eof) { return 0; }
            break;
        }
        if (index + width > maximum) { break; }
        index += width;
    }
    *prefix = index;
    return 1;
}

static nomo_process_control_state *nomo_process_control_find(uint64_t id) {
    for (nomo_process_control_state *state = nomo_process_control_states;
         state != NULL;
         state = state->next) {
        if (state->id == id) { return state; }
    }
    return NULL;
}

static void nomo_process_control_unlink(nomo_process_control_state *state) {
    nomo_process_control_state **cursor = &nomo_process_control_states;
    while (*cursor != NULL) {
        if (*cursor == state) {
            *cursor = state->next;
            state->next = NULL;
            return;
        }
        cursor = &(*cursor)->next;
    }
}

static uint64_t nomo_process_control_allocate_id(void) {
    for (;;) {
        uint64_t id = nomo_process_control_next_id++;
        if (nomo_process_control_next_id == 0) {
            nomo_process_control_next_id = UINT64_C(1);
        }
        if (id != 0 && nomo_process_control_find(id) == NULL) { return id; }
    }
}

#ifndef _WIN32

typedef struct nomo_process_env_list {
    char **items;
    size_t len;
} nomo_process_env_list;

static uint64_t nomo_process_control_now_millis(void) {
    struct timespec now;
    if (clock_gettime(CLOCK_MONOTONIC, &now) != 0) {
        nomo_panic("process monotonic clock failed");
    }
    return (uint64_t)now.tv_sec * UINT64_C(1000)
        + (uint64_t)now.tv_nsec / UINT64_C(1000000);
}

static char *nomo_process_copy_cstr(const char *value) {
    size_t len = strlen(value);
    char *copy = (char *)malloc(len + 1U);
    if (copy == NULL) { nomo_panic("out of memory"); }
    memcpy(copy, value, len + 1U);
    return copy;
}

static void nomo_process_env_list_release(nomo_process_env_list *env) {
    if (env->items == NULL) { return; }
    for (size_t index = 0; index < env->len; index += 1U) {
        free(env->items[index]);
    }
    free(env->items);
    env->items = NULL;
    env->len = 0;
}

static size_t nomo_process_env_name_len(const char *entry) {
    const char *equals = strchr(entry, '=');
    return equals == NULL ? strlen(entry) : (size_t)(equals - entry);
}

static int nomo_process_env_entry_matches(
    const char *entry,
    const char *name
) {
    size_t entry_len = nomo_process_env_name_len(entry);
    size_t name_len = strlen(name);
    return entry_len == name_len && memcmp(entry, name, name_len) == 0;
}

static const char *nomo_process_env_list_get(
    const nomo_process_env_list *env,
    const char *name
) {
    for (size_t index = 0; index < env->len; index += 1U) {
        if (nomo_process_env_entry_matches(env->items[index], name)) {
            const char *equals = strchr(env->items[index], '=');
            return equals == NULL ? "" : equals + 1;
        }
    }
    return NULL;
}

static void nomo_process_env_list_set(
    nomo_process_env_list *env,
    const char *name,
    const char *value
) {
    size_t name_len = strlen(name);
    size_t value_len = strlen(value);
    char *entry = (char *)malloc(name_len + value_len + 2U);
    if (entry == NULL) { nomo_panic("out of memory"); }
    memcpy(entry, name, name_len);
    entry[name_len] = '=';
    memcpy(entry + name_len + 1U, value, value_len + 1U);
    for (size_t index = 0; index < env->len; index += 1U) {
        if (nomo_process_env_entry_matches(env->items[index], name)) {
            free(env->items[index]);
            env->items[index] = entry;
            return;
        }
    }
    char **next = (char **)realloc(
        env->items,
        (env->len + 2U) * sizeof(char *)
    );
    if (next == NULL) {
        free(entry);
        nomo_panic("out of memory");
    }
    env->items = next;
    env->items[env->len] = entry;
    env->len += 1U;
    env->items[env->len] = NULL;
}

static nomo_process_env_list nomo_process_build_environment(
    @PROCESS_COMMAND@ command
) {
    nomo_process_env_list env = {0};
    if (command.@INHERIT_ENV_MEMBER@) {
        while (environ[env.len] != NULL) { env.len += 1U; }
        env.items = (char **)calloc(env.len + 1U, sizeof(char *));
        if (env.items == NULL) { nomo_panic("out of memory"); }
        for (size_t index = 0; index < env.len; index += 1U) {
            env.items[index] = nomo_process_copy_cstr(environ[index]);
        }
    } else {
        env.items = (char **)calloc(1U, sizeof(char *));
        if (env.items == NULL) { nomo_panic("out of memory"); }
    }
    for (size_t index = 0; index < command.@ENV_MEMBER@.len; index += 1U) {
        @PROCESS_ENV@ item = command.@ENV_MEMBER@.data[index];
        nomo_process_env_list_set(&env, item.@NAME_MEMBER@.data, item.@VALUE_MEMBER@.data);
    }
    return env;
}

static int nomo_process_command_is_valid(@PROCESS_COMMAND@ command) {
    if (command.@PROGRAM_MEMBER@.data[0] == '\0') { return 0; }
    for (size_t index = 0; index < command.@ENV_MEMBER@.len; index += 1U) {
        @PROCESS_ENV@ item = command.@ENV_MEMBER@.data[index];
        if (item.@NAME_MEMBER@.data[0] == '\0'
            || strchr(item.@NAME_MEMBER@.data, '=') != NULL) {
            return 0;
        }
        for (size_t other = index + 1U;
             other < command.@ENV_MEMBER@.len;
             other += 1U) {
            if (strcmp(
                    item.@NAME_MEMBER@.data,
                    command.@ENV_MEMBER@.data[other].@NAME_MEMBER@.data
                ) == 0) {
                return 0;
            }
        }
    }
    return 1;
}

static int nomo_process_set_cloexec(int fd) {
    int flags = fcntl(fd, F_GETFD);
    return flags >= 0 && fcntl(fd, F_SETFD, flags | FD_CLOEXEC) == 0;
}

static int nomo_process_set_nonblocking(int fd) {
    int flags = fcntl(fd, F_GETFL);
    return flags >= 0 && fcntl(fd, F_SETFL, flags | O_NONBLOCK) == 0;
}

static int nomo_process_make_pipe(int fds[2]) {
    if (pipe(fds) != 0) { return 0; }
    if (!nomo_process_set_cloexec(fds[0])
        || !nomo_process_set_cloexec(fds[1])) {
        close(fds[0]);
        close(fds[1]);
        return 0;
    }
    return 1;
}

static void nomo_process_close_fd(int *fd) {
    if (*fd >= 0) {
        close(*fd);
        *fd = -1;
    }
}

static void nomo_process_exec_search(
    const char *program,
    char *const argv[],
    char *const envp[]
) {
    if (strchr(program, '/') != NULL) {
        execve(program, argv, envp);
        return;
    }
    const char *path = NULL;
    for (size_t index = 0; envp[index] != NULL; index += 1U) {
        if (strncmp(envp[index], "PATH=", 5U) == 0) {
            path = envp[index] + 5U;
            break;
        }
    }
    if (path == NULL) {
        errno = ENOENT;
        return;
    }
    int permission_error = 0;
    const char *cursor = path;
    for (;;) {
        const char *separator = strchr(cursor, ':');
        size_t dir_len = separator == NULL
            ? strlen(cursor)
            : (size_t)(separator - cursor);
        size_t program_len = strlen(program);
        size_t cap = (dir_len == 0 ? 1U : dir_len) + program_len + 2U;
        char *candidate = (char *)malloc(cap);
        if (candidate == NULL) {
            errno = ENOMEM;
            return;
        }
        if (dir_len == 0) {
            snprintf(candidate, cap, "./%s", program);
        } else {
            memcpy(candidate, cursor, dir_len);
            candidate[dir_len] = '/';
            memcpy(candidate + dir_len + 1U, program, program_len + 1U);
        }
        execve(candidate, argv, envp);
        if (errno == EACCES) { permission_error = 1; }
        free(candidate);
        if (separator == NULL) { break; }
        cursor = separator + 1U;
    }
    errno = permission_error ? EACCES : ENOENT;
}

static void nomo_process_control_update_exit(
    nomo_process_control_state *state
) {
    if (state->exited) { return; }
    int status = 0;
    pid_t result;
    do {
        result = waitpid(state->pid, &status, WNOHANG);
    } while (result < 0 && errno == EINTR);
    if (result != state->pid) { return; }
    state->exited = 1;
    if (WIFEXITED(status)) {
        state->exit_info = nomo_process_control_exit_value(
            (int32_t)WEXITSTATUS(status),
            0
        );
    } else if (WIFSIGNALED(status)) {
        int signal = WTERMSIG(status);
        state->exit_info = nomo_process_control_exit_value(
            (int32_t)(128 + signal),
            (int32_t)signal
        );
    } else {
        state->exit_info = nomo_process_control_exit_value((int32_t)status, 0);
    }
}

static void nomo_process_control_destroy(
    nomo_process_control_state *state
) {
    if (state == NULL) { return; }
    nomo_process_control_update_exit(state);
    if (!state->exited) {
        if (kill(state->pid, SIGKILL) != 0 && errno != ESRCH) {
            /* Cleanup remains best-effort and secret-safe. */
        }
        int status = 0;
        while (waitpid(state->pid, &status, 0) < 0 && errno == EINTR) {}
        state->exited = 1;
    }
    nomo_process_close_fd(&state->stdin_fd);
    nomo_process_close_fd(&state->stdout_fd);
    nomo_process_close_fd(&state->stderr_fd);
    free(state->stdin_data);
    nomo_process_buffer_release(&state->stdout_buffer);
    nomo_process_buffer_release(&state->stderr_buffer);
    free(state);
}

static @START_RESULT@ @START_NAME@(@PROCESS_COMMAND@ command) {
    if (!nomo_process_command_is_valid(command)) {
        return nomo_process_control_start_error(
            "invalid_request",
            "invalid process command"
        );
    }
    (void)signal(SIGPIPE, SIG_IGN);
    nomo_process_env_list env = nomo_process_build_environment(command);
    if (strchr(command.@PROGRAM_MEMBER@.data, '/') == NULL
        && nomo_process_env_list_get(&env, "PATH") == NULL) {
        nomo_process_env_list_release(&env);
        return nomo_process_control_start_error(
            "spawn",
            "process executable was not found"
        );
    }

    size_t argc = command.@ARGS_MEMBER@.len + 1U;
    char **argv = (char **)calloc(argc + 1U, sizeof(char *));
    if (argv == NULL) { nomo_panic("out of memory"); }
    argv[0] = (char *)command.@PROGRAM_MEMBER@.data;
    for (size_t index = 0; index < command.@ARGS_MEMBER@.len; index += 1U) {
        argv[index + 1U] = (char *)command.@ARGS_MEMBER@.data[index].data;
    }

    int stdin_pipe[2] = {-1, -1};
    int stdout_pipe[2] = {-1, -1};
    int stderr_pipe[2] = {-1, -1};
    int exec_pipe[2] = {-1, -1};
    if (!nomo_process_make_pipe(stdin_pipe)
        || !nomo_process_make_pipe(stdout_pipe)
        || !nomo_process_make_pipe(stderr_pipe)
        || !nomo_process_make_pipe(exec_pipe)) {
        nomo_process_close_fd(&stdin_pipe[0]);
        nomo_process_close_fd(&stdin_pipe[1]);
        nomo_process_close_fd(&stdout_pipe[0]);
        nomo_process_close_fd(&stdout_pipe[1]);
        nomo_process_close_fd(&stderr_pipe[0]);
        nomo_process_close_fd(&stderr_pipe[1]);
        nomo_process_close_fd(&exec_pipe[0]);
        nomo_process_close_fd(&exec_pipe[1]);
        free(argv);
        nomo_process_env_list_release(&env);
        return nomo_process_control_start_error(
            "spawn",
            "failed to create process pipes"
        );
    }

    pid_t pid = fork();
    if (pid == 0) {
        close(stdin_pipe[1]);
        close(stdout_pipe[0]);
        close(stderr_pipe[0]);
        close(exec_pipe[0]);
        if (dup2(stdin_pipe[0], STDIN_FILENO) < 0
            || dup2(stdout_pipe[1], STDOUT_FILENO) < 0
            || dup2(stderr_pipe[1], STDERR_FILENO) < 0) {
            int child_errno = errno;
            (void)write(exec_pipe[1], &child_errno, sizeof(child_errno));
            _exit(127);
        }
        close(stdin_pipe[0]);
        close(stdout_pipe[1]);
        close(stderr_pipe[1]);
        if (command.@CWD_MEMBER@.tag == @CWD_SOME@
            && chdir(
                command.@CWD_MEMBER@.payload.@SOME_PAYLOAD@.data
            ) != 0) {
            int child_errno = errno;
            (void)write(exec_pipe[1], &child_errno, sizeof(child_errno));
            _exit(127);
        }
        nomo_process_exec_search(
            command.@PROGRAM_MEMBER@.data,
            argv,
            env.items
        );
        int child_errno = errno;
        (void)write(exec_pipe[1], &child_errno, sizeof(child_errno));
        _exit(127);
    }
    if (pid < 0) {
        nomo_process_close_fd(&stdin_pipe[0]);
        nomo_process_close_fd(&stdin_pipe[1]);
        nomo_process_close_fd(&stdout_pipe[0]);
        nomo_process_close_fd(&stdout_pipe[1]);
        nomo_process_close_fd(&stderr_pipe[0]);
        nomo_process_close_fd(&stderr_pipe[1]);
        nomo_process_close_fd(&exec_pipe[0]);
        nomo_process_close_fd(&exec_pipe[1]);
        free(argv);
        nomo_process_env_list_release(&env);
        return nomo_process_control_start_error(
            "spawn",
            "failed to start process"
        );
    }

    close(stdin_pipe[0]);
    stdin_pipe[0] = -1;
    close(stdout_pipe[1]);
    stdout_pipe[1] = -1;
    close(stderr_pipe[1]);
    stderr_pipe[1] = -1;
    close(exec_pipe[1]);
    exec_pipe[1] = -1;

    int child_errno = 0;
    ssize_t exec_read;
    do {
        exec_read = read(exec_pipe[0], &child_errno, sizeof(child_errno));
    } while (exec_read < 0 && errno == EINTR);
    close(exec_pipe[0]);
    exec_pipe[0] = -1;
    free(argv);
    nomo_process_env_list_release(&env);
    if (exec_read != 0) {
        nomo_process_close_fd(&stdin_pipe[1]);
        nomo_process_close_fd(&stdout_pipe[0]);
        nomo_process_close_fd(&stderr_pipe[0]);
        int status = 0;
        while (waitpid(pid, &status, 0) < 0 && errno == EINTR) {}
        return nomo_process_control_start_error(
            "spawn",
            "failed to start process"
        );
    }
    if (!nomo_process_set_nonblocking(stdin_pipe[1])
        || !nomo_process_set_nonblocking(stdout_pipe[0])
        || !nomo_process_set_nonblocking(stderr_pipe[0])) {
        nomo_process_close_fd(&stdin_pipe[1]);
        nomo_process_close_fd(&stdout_pipe[0]);
        nomo_process_close_fd(&stderr_pipe[0]);
        kill(pid, SIGKILL);
        int status = 0;
        while (waitpid(pid, &status, 0) < 0 && errno == EINTR) {}
        return nomo_process_control_start_error(
            "spawn",
            "failed to configure process pipes"
        );
    }

    nomo_process_control_state *state =
        (nomo_process_control_state *)calloc(
            1U,
            sizeof(nomo_process_control_state)
        );
    if (state == NULL) { nomo_panic("out of memory"); }
    state->id = nomo_process_control_allocate_id();
    state->pid = pid;
    state->stdin_fd = stdin_pipe[1];
    state->stdout_fd = stdout_pipe[0];
    state->stderr_fd = stderr_pipe[0];
    state->next = nomo_process_control_states;
    nomo_process_control_states = state;

    return (@START_RESULT@){
        .tag = @START_OK@,
        .payload.@OK_PAYLOAD@ = (@PROCESS_CHILD@){
            .@HANDLE_MEMBER@ = state->id
        }
    };
}

static @VOID_RESULT@ @WRITE_STDIN_NAME@(
    @PROCESS_CHILD@ child,
    nomo_string data
) {
    nomo_process_control_state *state =
        nomo_process_control_find(child.@HANDLE_MEMBER@);
    if (state == NULL || state->exit_emitted) {
        return nomo_process_control_void_error(
            "invalid_request",
            "invalid process child"
        );
    }
    size_t len = strlen(data.data);
    if (len == 0 || len > (size_t)NOMO_PROCESS_MAX_PAYLOAD_BYTES) {
        return nomo_process_control_void_error(
            "invalid_request",
            "invalid process stdin payload"
        );
    }
    if (state->stdin_closed || state->exited) {
        return nomo_process_control_void_error(
            "invalid_request",
            "process stdin is closed"
        );
    }
    if (state->stdin_pending) {
        return nomo_process_control_void_error(
            "busy",
            "process stdin already has pending data"
        );
    }
    state->stdin_data = (char *)malloc(len);
    if (state->stdin_data == NULL) { nomo_panic("out of memory"); }
    memcpy(state->stdin_data, data.data, len);
    state->stdin_len = len;
    state->stdin_offset = 0;
    state->stdin_pending = 1;
    state->stdin_flushed = 0;
    return nomo_process_control_void_ok();
}

static @VOID_RESULT@ @CLOSE_STDIN_NAME@(@PROCESS_CHILD@ child) {
    nomo_process_control_state *state =
        nomo_process_control_find(child.@HANDLE_MEMBER@);
    if (state == NULL) {
        return nomo_process_control_void_error(
            "invalid_request",
            "invalid process child"
        );
    }
    if (state->stdin_pending) {
        return nomo_process_control_void_error(
            "busy",
            "process stdin still has pending data"
        );
    }
    if (!state->stdin_closed) {
        nomo_process_close_fd(&state->stdin_fd);
        state->stdin_closed = 1;
    }
    return nomo_process_control_void_ok();
}

static int nomo_process_control_emit_stream(
    nomo_process_control_state *state,
    int is_stderr,
    uint64_t max_chunk_bytes,
    @EVENT_RESULT@ *result
) {
    nomo_process_buffer *buffer = is_stderr
        ? &state->stderr_buffer
        : &state->stdout_buffer;
    int eof = is_stderr ? state->stderr_eof : state->stdout_eof;
    if (buffer->len == 0) { return 0; }
    size_t prefix = 0;
    if (!nomo_process_utf8_prefix(
            buffer->data,
            buffer->len,
            (size_t)max_chunk_bytes,
            eof,
            &prefix
        )) {
        return -1;
    }
    if (prefix == 0) { return 0; }
    nomo_string text = nomo_string_from_slice(buffer->data, 0, prefix);
    nomo_process_buffer_consume(buffer, prefix);
    state->prefer_stderr = !is_stderr;
    *result = nomo_process_control_text_event(is_stderr, text);
    return 1;
}

static int nomo_process_control_read_stream(
    nomo_process_control_state *state,
    int is_stderr,
    size_t target
) {
    int *fd = is_stderr ? &state->stderr_fd : &state->stdout_fd;
    nomo_process_buffer *buffer = is_stderr
        ? &state->stderr_buffer
        : &state->stdout_buffer;
    int *eof = is_stderr ? &state->stderr_eof : &state->stdout_eof;
    if (*fd < 0 || buffer->len >= target) { return 1; }
    size_t available = target - buffer->len;
    if (available > 4096U) { available = 4096U; }
    char chunk[4096];
    ssize_t count;
    do {
        count = read(*fd, chunk, available);
    } while (count < 0 && errno == EINTR);
    if (count > 0) {
        nomo_process_buffer_append(buffer, chunk, (size_t)count);
        return 1;
    }
    if (count == 0) {
        nomo_process_close_fd(fd);
        *eof = 1;
        return 1;
    }
    if (errno == EAGAIN || errno == EWOULDBLOCK) { return 1; }
    return 0;
}

static @EVENT_RESULT@ @NEXT_EVENT_NAME@(
    @PROCESS_CHILD@ child,
    uint64_t max_chunk_bytes,
    uint64_t timeout_millis
) {
    nomo_process_control_state *state =
        nomo_process_control_find(child.@HANDLE_MEMBER@);
    if (state == NULL || state->exit_emitted) {
        return nomo_process_control_event_error(
            "invalid_request",
            "invalid process child"
        );
    }
    if (max_chunk_bytes < UINT64_C(4)
        || max_chunk_bytes > NOMO_PROCESS_MAX_PAYLOAD_BYTES
        || timeout_millis == 0
        || timeout_millis > NOMO_PROCESS_MAX_TIMEOUT_MILLIS) {
        return nomo_process_control_event_error(
            "invalid_request",
            "invalid process event limit or timeout"
        );
    }
    uint64_t started = nomo_process_control_now_millis();
    uint64_t deadline = started > UINT64_MAX - timeout_millis
        ? UINT64_MAX
        : started + timeout_millis;
    size_t target = (size_t)max_chunk_bytes + NOMO_PROCESS_BUFFER_SLACK;

    for (;;) {
        nomo_process_control_update_exit(state);
        if (state->stdin_flushed) {
            state->stdin_flushed = 0;
            return nomo_process_control_stdin_flushed_event();
        }

        @EVENT_RESULT@ stream_result;
        int first_stderr = state->prefer_stderr;
        int emitted = nomo_process_control_emit_stream(
            state,
            first_stderr,
            max_chunk_bytes,
            &stream_result
        );
        if (emitted == 0) {
            emitted = nomo_process_control_emit_stream(
                state,
                !first_stderr,
                max_chunk_bytes,
                &stream_result
            );
        }
        if (emitted < 0) {
            nomo_process_control_unlink(state);
            nomo_process_control_destroy(state);
            return nomo_process_control_event_error(
                "protocol",
                "process output is not valid supported text"
            );
        }
        if (emitted > 0) { return stream_result; }

        if (state->exited
            && state->stdout_eof
            && state->stderr_eof
            && state->stdout_buffer.len == 0
            && state->stderr_buffer.len == 0) {
            state->exit_emitted = 1;
            return nomo_process_control_exited_event(state->exit_info);
        }

        uint64_t now = nomo_process_control_now_millis();
        if (now >= deadline) {
            return nomo_process_control_event_error(
                "timeout",
                "process event timed out"
            );
        }
        uint64_t remaining = deadline - now;
        int wait_millis = remaining > UINT64_C(50)
            ? 50
            : (int)remaining;

        struct pollfd fds[3];
        int kinds[3];
        nfds_t count = 0;
        if (state->stdout_fd >= 0 && state->stdout_buffer.len < target) {
            fds[count] = (struct pollfd){
                .fd = state->stdout_fd,
                .events = POLLIN
            };
            kinds[count++] = 0;
        }
        if (state->stderr_fd >= 0 && state->stderr_buffer.len < target) {
            fds[count] = (struct pollfd){
                .fd = state->stderr_fd,
                .events = POLLIN
            };
            kinds[count++] = 1;
        }
        if (state->stdin_fd >= 0 && state->stdin_pending) {
            fds[count] = (struct pollfd){
                .fd = state->stdin_fd,
                .events = POLLOUT
            };
            kinds[count++] = 2;
        }

        int polled;
        do {
            polled = poll(fds, count, wait_millis);
        } while (polled < 0 && errno == EINTR);
        if (polled < 0) {
            return nomo_process_control_event_error(
                "io",
                "process event polling failed"
            );
        }
        for (nfds_t index = 0; index < count; index += 1U) {
            if (fds[index].revents == 0) { continue; }
            if (kinds[index] == 0 || kinds[index] == 1) {
                if (!nomo_process_control_read_stream(
                        state,
                        kinds[index] == 1,
                        target
                    )) {
                    return nomo_process_control_event_error(
                        "io",
                        "process output read failed"
                    );
                }
            } else if (kinds[index] == 2
                && (fds[index].revents & POLLOUT) != 0) {
                ssize_t written;
                do {
                    written = write(
                        state->stdin_fd,
                        state->stdin_data + state->stdin_offset,
                        state->stdin_len - state->stdin_offset
                    );
                } while (written < 0 && errno == EINTR);
                if (written > 0) {
                    state->stdin_offset += (size_t)written;
                    if (state->stdin_offset == state->stdin_len) {
                        free(state->stdin_data);
                        state->stdin_data = NULL;
                        state->stdin_len = 0;
                        state->stdin_offset = 0;
                        state->stdin_pending = 0;
                        state->stdin_flushed = 1;
                    }
                } else if (written < 0
                    && errno != EAGAIN
                    && errno != EWOULDBLOCK) {
                    free(state->stdin_data);
                    state->stdin_data = NULL;
                    state->stdin_len = 0;
                    state->stdin_offset = 0;
                    state->stdin_pending = 0;
                    state->stdin_closed = 1;
                    nomo_process_close_fd(&state->stdin_fd);
                    return nomo_process_control_event_error(
                        "io",
                        "process stdin write failed"
                    );
                }
            }
        }
    }
}

static @WAIT_RESULT@ @TRY_WAIT_NAME@(@PROCESS_CHILD@ child) {
    nomo_process_control_state *state =
        nomo_process_control_find(child.@HANDLE_MEMBER@);
    if (state == NULL) {
        return nomo_process_control_wait_error(
            "invalid_request",
            "invalid process child"
        );
    }
    nomo_process_control_update_exit(state);
    return state->exited
        ? nomo_process_control_wait_some(state->exit_info)
        : nomo_process_control_wait_none();
}

static @VOID_RESULT@ @TERMINATE_NAME@(@PROCESS_CHILD@ child) {
    nomo_process_control_state *state =
        nomo_process_control_find(child.@HANDLE_MEMBER@);
    if (state == NULL) {
        return nomo_process_control_void_error(
            "invalid_request",
            "invalid process child"
        );
    }
    nomo_process_control_update_exit(state);
    if (!state->exited && kill(state->pid, SIGKILL) != 0 && errno != ESRCH) {
        return nomo_process_control_void_error(
            "io",
            "process termination failed"
        );
    }
    return nomo_process_control_void_ok();
}

static void @CLOSE_CHILD_NAME@(@PROCESS_CHILD@ child) {
    nomo_process_control_state *state =
        nomo_process_control_find(child.@HANDLE_MEMBER@);
    if (state == NULL) { return; }
    nomo_process_control_unlink(state);
    nomo_process_control_destroy(state);
}

#else

extern char **_environ;

typedef struct nomo_process_windows_env {
    char **items;
    size_t len;
} nomo_process_windows_env;

typedef struct nomo_process_windows_text {
    char *data;
    size_t len;
    size_t cap;
} nomo_process_windows_text;

typedef BOOL (WINAPI *nomo_cancel_synchronous_io_fn)(HANDLE);

static uint64_t nomo_process_control_now_millis(void) {
    return (uint64_t)GetTickCount64();
}

static char *nomo_process_windows_copy_cstr(const char *value) {
    size_t len = strlen(value);
    char *copy = (char *)malloc(len + 1U);
    if (copy == NULL) { nomo_panic("out of memory"); }
    memcpy(copy, value, len + 1U);
    return copy;
}

static wchar_t *nomo_process_windows_utf8_to_wide(const char *value) {
    int needed = MultiByteToWideChar(
        CP_UTF8,
        MB_ERR_INVALID_CHARS,
        value,
        -1,
        NULL,
        0
    );
    if (needed <= 0) { return NULL; }
    wchar_t *wide = (wchar_t *)malloc((size_t)needed * sizeof(wchar_t));
    if (wide == NULL) { nomo_panic("out of memory"); }
    if (MultiByteToWideChar(
            CP_UTF8,
            MB_ERR_INVALID_CHARS,
            value,
            -1,
            wide,
            needed
        ) <= 0) {
        free(wide);
        return NULL;
    }
    return wide;
}

static char *nomo_process_windows_wide_to_utf8(const wchar_t *value) {
    int needed = WideCharToMultiByte(
        CP_UTF8,
        WC_ERR_INVALID_CHARS,
        value,
        -1,
        NULL,
        0,
        NULL,
        NULL
    );
    if (needed <= 0) { return NULL; }
    char *utf8 = (char *)malloc((size_t)needed);
    if (utf8 == NULL) { nomo_panic("out of memory"); }
    if (WideCharToMultiByte(
            CP_UTF8,
            WC_ERR_INVALID_CHARS,
            value,
            -1,
            utf8,
            needed,
            NULL,
            NULL
        ) <= 0) {
        free(utf8);
        return NULL;
    }
    return utf8;
}

static size_t nomo_process_windows_env_name_len(const char *entry) {
    const char *equals = strchr(entry, '=');
    return equals == NULL ? strlen(entry) : (size_t)(equals - entry);
}

static int nomo_process_windows_env_matches(
    const char *entry,
    const char *name
) {
    size_t entry_len = nomo_process_windows_env_name_len(entry);
    size_t name_len = strlen(name);
    return entry_len == name_len
        && _strnicmp(entry, name, name_len) == 0;
}

static void nomo_process_windows_env_release(
    nomo_process_windows_env *env
) {
    if (env->items == NULL) { return; }
    for (size_t index = 0; index < env->len; index += 1U) {
        free(env->items[index]);
    }
    free(env->items);
    env->items = NULL;
    env->len = 0;
}

static const char *nomo_process_windows_env_get(
    const nomo_process_windows_env *env,
    const char *name
) {
    for (size_t index = 0; index < env->len; index += 1U) {
        if (nomo_process_windows_env_matches(env->items[index], name)) {
            const char *equals = strchr(env->items[index], '=');
            return equals == NULL ? "" : equals + 1;
        }
    }
    return NULL;
}

static void nomo_process_windows_env_set(
    nomo_process_windows_env *env,
    const char *name,
    const char *value
) {
    size_t name_len = strlen(name);
    size_t value_len = strlen(value);
    char *entry = (char *)malloc(name_len + value_len + 2U);
    if (entry == NULL) { nomo_panic("out of memory"); }
    memcpy(entry, name, name_len);
    entry[name_len] = '=';
    memcpy(entry + name_len + 1U, value, value_len + 1U);
    for (size_t index = 0; index < env->len; index += 1U) {
        if (nomo_process_windows_env_matches(env->items[index], name)) {
            free(env->items[index]);
            env->items[index] = entry;
            return;
        }
    }
    char **next = (char **)realloc(
        env->items,
        (env->len + 1U) * sizeof(char *)
    );
    if (next == NULL) {
        free(entry);
        nomo_panic("out of memory");
    }
    env->items = next;
    env->items[env->len++] = entry;
}

static int nomo_process_windows_env_compare(
    const void *left,
    const void *right
) {
    const char *const *left_entry = (const char *const *)left;
    const char *const *right_entry = (const char *const *)right;
    return _stricmp(*left_entry, *right_entry);
}

static int nomo_process_windows_command_is_valid(
    @PROCESS_COMMAND@ command
) {
    if (command.@PROGRAM_MEMBER@.data[0] == '\0') { return 0; }
    for (size_t index = 0; index < command.@ENV_MEMBER@.len; index += 1U) {
        @PROCESS_ENV@ item = command.@ENV_MEMBER@.data[index];
        if (item.@NAME_MEMBER@.data[0] == '\0'
            || strchr(item.@NAME_MEMBER@.data, '=') != NULL) {
            return 0;
        }
        for (size_t other = index + 1U;
             other < command.@ENV_MEMBER@.len;
             other += 1U) {
            if (_stricmp(
                    item.@NAME_MEMBER@.data,
                    command.@ENV_MEMBER@.data[other].@NAME_MEMBER@.data
                ) == 0) {
                return 0;
            }
        }
    }
    return 1;
}

static nomo_process_windows_env nomo_process_windows_build_environment(
    @PROCESS_COMMAND@ command
) {
    nomo_process_windows_env env = {0};
    if (command.@INHERIT_ENV_MEMBER@ && _environ != NULL) {
        while (_environ[env.len] != NULL) { env.len += 1U; }
        env.items = (char **)calloc(env.len, sizeof(char *));
        if (env.len > 0 && env.items == NULL) {
            nomo_panic("out of memory");
        }
        for (size_t index = 0; index < env.len; index += 1U) {
            env.items[index] =
                nomo_process_windows_copy_cstr(_environ[index]);
        }
    }
    for (size_t index = 0; index < command.@ENV_MEMBER@.len; index += 1U) {
        @PROCESS_ENV@ item = command.@ENV_MEMBER@.data[index];
        nomo_process_windows_env_set(
            &env,
            item.@NAME_MEMBER@.data,
            item.@VALUE_MEMBER@.data
        );
    }
    if (!command.@INHERIT_ENV_MEMBER@
        && nomo_process_windows_env_get(&env, "SystemRoot") == NULL) {
        const char *system_root = getenv("SystemRoot");
        if (system_root != NULL) {
            nomo_process_windows_env_set(
                &env,
                "SystemRoot",
                system_root
            );
        }
    }
    qsort(
        env.items,
        env.len,
        sizeof(char *),
        nomo_process_windows_env_compare
    );
    return env;
}

static wchar_t *nomo_process_windows_environment_block(
    const nomo_process_windows_env *env
) {
    size_t total = 2U;
    wchar_t **entries = (wchar_t **)calloc(env->len, sizeof(wchar_t *));
    if (env->len > 0 && entries == NULL) { nomo_panic("out of memory"); }
    for (size_t index = 0; index < env->len; index += 1U) {
        entries[index] = nomo_process_windows_utf8_to_wide(env->items[index]);
        if (entries[index] == NULL) {
            for (size_t prior = 0; prior < index; prior += 1U) {
                free(entries[prior]);
            }
            free(entries);
            return NULL;
        }
        total += wcslen(entries[index]) + 1U;
    }
    wchar_t *block = (wchar_t *)calloc(total, sizeof(wchar_t));
    if (block == NULL) { nomo_panic("out of memory"); }
    size_t offset = 0;
    for (size_t index = 0; index < env->len; index += 1U) {
        size_t len = wcslen(entries[index]);
        memcpy(block + offset, entries[index], (len + 1U) * sizeof(wchar_t));
        offset += len + 1U;
        free(entries[index]);
    }
    block[offset] = L'\0';
    free(entries);
    return block;
}

static char *nomo_process_windows_current_directory(void) {
    DWORD needed = GetCurrentDirectoryW(0, NULL);
    if (needed == 0) { return NULL; }
    wchar_t *wide = (wchar_t *)malloc((size_t)needed * sizeof(wchar_t));
    if (wide == NULL) { nomo_panic("out of memory"); }
    if (GetCurrentDirectoryW(needed, wide) == 0) {
        free(wide);
        return NULL;
    }
    char *utf8 = nomo_process_windows_wide_to_utf8(wide);
    free(wide);
    return utf8;
}

static int nomo_process_windows_path_is_absolute(const char *path) {
    if ((path[0] == '\\' && path[1] == '\\')
        || (path[0] == '/' && path[1] == '/')) {
        return 1;
    }
    return path[0] != '\0'
        && path[1] == ':'
        && (path[2] == '\\' || path[2] == '/');
}

static char *nomo_process_windows_join_path(
    const char *left,
    size_t left_len,
    const char *right,
    size_t right_len
) {
    int needs_separator = left_len > 0
        && left[left_len - 1U] != '\\'
        && left[left_len - 1U] != '/';
    char *path = (char *)malloc(
        left_len + right_len + (needs_separator ? 2U : 1U)
    );
    if (path == NULL) { nomo_panic("out of memory"); }
    memcpy(path, left, left_len);
    size_t offset = left_len;
    if (needs_separator) { path[offset++] = '\\'; }
    memcpy(path + offset, right, right_len);
    path[offset + right_len] = '\0';
    return path;
}

static wchar_t *nomo_process_windows_existing_file(const char *candidate) {
    wchar_t *wide = nomo_process_windows_utf8_to_wide(candidate);
    if (wide == NULL) { return NULL; }
    DWORD attributes = GetFileAttributesW(wide);
    if (attributes != INVALID_FILE_ATTRIBUTES
        && (attributes & FILE_ATTRIBUTE_DIRECTORY) == 0) {
        return wide;
    }
    free(wide);
    size_t len = strlen(candidate);
    char *with_exe = (char *)malloc(len + 5U);
    if (with_exe == NULL) { nomo_panic("out of memory"); }
    memcpy(with_exe, candidate, len);
    memcpy(with_exe + len, ".exe", 5U);
    wide = nomo_process_windows_utf8_to_wide(with_exe);
    free(with_exe);
    if (wide == NULL) { return NULL; }
    attributes = GetFileAttributesW(wide);
    if (attributes != INVALID_FILE_ATTRIBUTES
        && (attributes & FILE_ATTRIBUTE_DIRECTORY) == 0) {
        return wide;
    }
    free(wide);
    return NULL;
}

static wchar_t *nomo_process_windows_resolve_program(
    @PROCESS_COMMAND@ command,
    const nomo_process_windows_env *env
) {
    char *cwd = command.@CWD_MEMBER@.tag == @CWD_SOME@
        ? nomo_process_windows_copy_cstr(
            command.@CWD_MEMBER@.payload.@SOME_PAYLOAD@.data
        )
        : nomo_process_windows_current_directory();
    if (cwd == NULL) { return NULL; }
    const char *program = command.@PROGRAM_MEMBER@.data;
    size_t program_len = strlen(program);
    wchar_t *resolved = NULL;
    if (strchr(program, '\\') != NULL || strchr(program, '/') != NULL) {
        char *candidate = nomo_process_windows_path_is_absolute(program)
            ? nomo_process_windows_copy_cstr(program)
            : nomo_process_windows_join_path(
                cwd,
                strlen(cwd),
                program,
                program_len
            );
        resolved = nomo_process_windows_existing_file(candidate);
        free(candidate);
        free(cwd);
        return resolved;
    }

    const char *path = nomo_process_windows_env_get(env, "PATH");
    if (path == NULL) {
        free(cwd);
        return NULL;
    }
    const char *cursor = path;
    for (;;) {
        const char *separator = strchr(cursor, ';');
        size_t dir_len = separator == NULL
            ? strlen(cursor)
            : (size_t)(separator - cursor);
        while (dir_len > 1U
            && cursor[0] == '"'
            && cursor[dir_len - 1U] == '"') {
            cursor += 1U;
            dir_len -= 2U;
        }
        char *directory;
        if (dir_len == 0) {
            directory = nomo_process_windows_copy_cstr(cwd);
        } else if (nomo_process_windows_path_is_absolute(cursor)) {
            directory = (char *)malloc(dir_len + 1U);
            if (directory == NULL) { nomo_panic("out of memory"); }
            memcpy(directory, cursor, dir_len);
            directory[dir_len] = '\0';
        } else {
            directory = nomo_process_windows_join_path(
                cwd,
                strlen(cwd),
                cursor,
                dir_len
            );
        }
        char *candidate = nomo_process_windows_join_path(
            directory,
            strlen(directory),
            program,
            program_len
        );
        free(directory);
        resolved = nomo_process_windows_existing_file(candidate);
        free(candidate);
        if (resolved != NULL || separator == NULL) { break; }
        cursor = separator + 1U;
    }
    free(cwd);
    return resolved;
}

static void nomo_process_windows_text_reserve(
    nomo_process_windows_text *text,
    size_t needed
) {
    if (needed <= text->cap) { return; }
    size_t cap = text->cap == 0 ? 128U : text->cap;
    while (cap < needed) {
        if (cap > SIZE_MAX / 2U) { nomo_panic("out of memory"); }
        cap *= 2U;
    }
    char *next = (char *)realloc(text->data, cap);
    if (next == NULL) { nomo_panic("out of memory"); }
    text->data = next;
    text->cap = cap;
}

static void nomo_process_windows_text_append_char(
    nomo_process_windows_text *text,
    char value
) {
    nomo_process_windows_text_reserve(text, text->len + 2U);
    text->data[text->len++] = value;
    text->data[text->len] = '\0';
}

static void nomo_process_windows_text_append_bytes(
    nomo_process_windows_text *text,
    const char *value,
    size_t len
) {
    nomo_process_windows_text_reserve(text, text->len + len + 1U);
    memcpy(text->data + text->len, value, len);
    text->len += len;
    text->data[text->len] = '\0';
}

static void nomo_process_windows_append_quoted_argument(
    nomo_process_windows_text *text,
    const char *argument
) {
    int quote = argument[0] == '\0'
        || strpbrk(argument, " \t\n\v\"") != NULL;
    if (!quote) {
        nomo_process_windows_text_append_bytes(
            text,
            argument,
            strlen(argument)
        );
        return;
    }
    nomo_process_windows_text_append_char(text, '"');
    size_t slashes = 0;
    for (const char *cursor = argument;; cursor += 1U) {
        if (*cursor == '\\') {
            slashes += 1U;
            continue;
        }
        if (*cursor == '"') {
            for (size_t index = 0; index < slashes * 2U + 1U; index += 1U) {
                nomo_process_windows_text_append_char(text, '\\');
            }
            nomo_process_windows_text_append_char(text, '"');
            slashes = 0;
            continue;
        }
        if (*cursor == '\0') {
            for (size_t index = 0; index < slashes * 2U; index += 1U) {
                nomo_process_windows_text_append_char(text, '\\');
            }
            break;
        }
        for (size_t index = 0; index < slashes; index += 1U) {
            nomo_process_windows_text_append_char(text, '\\');
        }
        slashes = 0;
        nomo_process_windows_text_append_char(text, *cursor);
    }
    nomo_process_windows_text_append_char(text, '"');
}

static wchar_t *nomo_process_windows_command_line(
    @PROCESS_COMMAND@ command
) {
    nomo_process_windows_text text = {0};
    nomo_process_windows_append_quoted_argument(
        &text,
        command.@PROGRAM_MEMBER@.data
    );
    for (size_t index = 0; index < command.@ARGS_MEMBER@.len; index += 1U) {
        nomo_process_windows_text_append_char(&text, ' ');
        nomo_process_windows_append_quoted_argument(
            &text,
            command.@ARGS_MEMBER@.data[index].data
        );
    }
    if (text.data == NULL) {
        text.data = nomo_process_windows_copy_cstr("");
    }
    wchar_t *wide = nomo_process_windows_utf8_to_wide(text.data);
    free(text.data);
    return wide;
}

static void nomo_process_windows_cancel_thread(HANDLE thread) {
    if (thread == NULL) { return; }
    HMODULE kernel = GetModuleHandleW(L"kernel32.dll");
    if (kernel == NULL) { return; }
    nomo_cancel_synchronous_io_fn cancel =
        (nomo_cancel_synchronous_io_fn)(void *)GetProcAddress(
            kernel,
            "CancelSynchronousIo"
        );
    if (cancel != NULL) { (void)cancel(thread); }
}

static void nomo_process_windows_update_exit_locked(
    nomo_process_control_state *state
) {
    if (state->exited || state->process_handle == NULL) { return; }
    DWORD code = STILL_ACTIVE;
    if (!GetExitCodeProcess(state->process_handle, &code)
        || code == STILL_ACTIVE) {
        return;
    }
    state->exited = 1;
    state->exit_info = nomo_process_control_exit_value((int32_t)code, 0);
}

static DWORD WINAPI nomo_process_windows_reader_thread(void *opaque) {
    nomo_process_reader_context *context =
        (nomo_process_reader_context *)opaque;
    nomo_process_control_state *state = context->state;
    char chunk[4096];
    for (;;) {
        EnterCriticalSection(&state->lock);
        int closing = state->closing;
        HANDLE handle = context->is_stderr
            ? state->stderr_read
            : state->stdout_read;
        nomo_process_buffer *buffer = context->is_stderr
            ? &state->stderr_buffer
            : &state->stdout_buffer;
        HANDLE space_event = context->is_stderr
            ? state->stderr_space_event
            : state->stdout_space_event;
        size_t maximum =
            (size_t)NOMO_PROCESS_MAX_PAYLOAD_BYTES
            + NOMO_PROCESS_BUFFER_SLACK;
        size_t available = buffer->len < maximum
            ? maximum - buffer->len
            : 0;
        LeaveCriticalSection(&state->lock);
        if (closing || handle == NULL) { break; }
        if (available == 0) {
            (void)WaitForSingleObject(space_event, INFINITE);
            continue;
        }
        if (available > sizeof(chunk)) { available = sizeof(chunk); }
        DWORD read = 0;
        BOOL ok = ReadFile(handle, chunk, (DWORD)available, &read, NULL);
        if (!ok || read == 0) {
            EnterCriticalSection(&state->lock);
            if (context->is_stderr) {
                state->stderr_eof = 1;
            } else {
                state->stdout_eof = 1;
            }
            LeaveCriticalSection(&state->lock);
            SetEvent(state->activity_event);
            break;
        }
        EnterCriticalSection(&state->lock);
        nomo_process_buffer_append(buffer, chunk, (size_t)read);
        LeaveCriticalSection(&state->lock);
        SetEvent(state->activity_event);
    }
    return 0;
}

static DWORD WINAPI nomo_process_windows_writer_thread(void *opaque) {
    nomo_process_control_state *state =
        (nomo_process_control_state *)opaque;
    for (;;) {
        (void)WaitForSingleObject(state->write_event, INFINITE);
        for (;;) {
            EnterCriticalSection(&state->lock);
            if (state->closing) {
                LeaveCriticalSection(&state->lock);
                return 0;
            }
            if (!state->stdin_pending) {
                LeaveCriticalSection(&state->lock);
                break;
            }
            HANDLE handle = state->stdin_write;
            const char *data = state->stdin_data + state->stdin_offset;
            size_t remaining = state->stdin_len - state->stdin_offset;
            if (remaining > 4096U) { remaining = 4096U; }
            LeaveCriticalSection(&state->lock);

            DWORD written = 0;
            BOOL ok = handle != NULL
                && WriteFile(
                    handle,
                    data,
                    (DWORD)remaining,
                    &written,
                    NULL
                );
            EnterCriticalSection(&state->lock);
            if (!ok || written == 0) {
                free(state->stdin_data);
                state->stdin_data = NULL;
                state->stdin_len = 0;
                state->stdin_offset = 0;
                state->stdin_pending = 0;
                state->stdin_closed = 1;
                state->stdin_error = 1;
                HANDLE failed = state->stdin_write;
                state->stdin_write = NULL;
                LeaveCriticalSection(&state->lock);
                if (failed != NULL) { CloseHandle(failed); }
                SetEvent(state->activity_event);
                break;
            }
            state->stdin_offset += (size_t)written;
            if (state->stdin_offset == state->stdin_len) {
                free(state->stdin_data);
                state->stdin_data = NULL;
                state->stdin_len = 0;
                state->stdin_offset = 0;
                state->stdin_pending = 0;
                state->stdin_flushed = 1;
                LeaveCriticalSection(&state->lock);
                SetEvent(state->activity_event);
                break;
            }
            LeaveCriticalSection(&state->lock);
        }
    }
}

static void nomo_process_control_destroy(
    nomo_process_control_state *state
) {
    if (state == NULL) { return; }
    if (state->lock_initialized) {
        EnterCriticalSection(&state->lock);
        state->closing = 1;
        nomo_process_windows_update_exit_locked(state);
        int running = !state->exited;
        LeaveCriticalSection(&state->lock);
        if (running && state->process_handle != NULL) {
            (void)TerminateProcess(state->process_handle, 137U);
        }
    } else if (state->process_handle != NULL) {
        (void)TerminateProcess(state->process_handle, 137U);
    }
    if (state->write_event != NULL) { SetEvent(state->write_event); }
    if (state->stdout_space_event != NULL) {
        SetEvent(state->stdout_space_event);
    }
    if (state->stderr_space_event != NULL) {
        SetEvent(state->stderr_space_event);
    }
    nomo_process_windows_cancel_thread(state->writer_thread);
    nomo_process_windows_cancel_thread(state->stdout_thread);
    nomo_process_windows_cancel_thread(state->stderr_thread);
    if (state->writer_thread != NULL) {
        (void)WaitForSingleObject(state->writer_thread, INFINITE);
    }
    if (state->stdout_thread != NULL) {
        (void)WaitForSingleObject(state->stdout_thread, INFINITE);
    }
    if (state->stderr_thread != NULL) {
        (void)WaitForSingleObject(state->stderr_thread, INFINITE);
    }
    if (state->process_handle != NULL) {
        (void)WaitForSingleObject(state->process_handle, INFINITE);
    }
    HANDLE handles[] = {
        state->stdin_write,
        state->stdout_read,
        state->stderr_read,
        state->activity_event,
        state->write_event,
        state->stdout_space_event,
        state->stderr_space_event,
        state->writer_thread,
        state->stdout_thread,
        state->stderr_thread,
        state->process_handle
    };
    for (size_t index = 0;
         index < sizeof(handles) / sizeof(handles[0]);
         index += 1U) {
        if (handles[index] != NULL) { CloseHandle(handles[index]); }
    }
    if (state->lock_initialized) {
        DeleteCriticalSection(&state->lock);
    }
    free(state->stdin_data);
    nomo_process_buffer_release(&state->stdout_buffer);
    nomo_process_buffer_release(&state->stderr_buffer);
    free(state);
}

static @START_RESULT@ @START_NAME@(@PROCESS_COMMAND@ command) {
    if (!nomo_process_windows_command_is_valid(command)) {
        return nomo_process_control_start_error(
            "invalid_request",
            "invalid process command"
        );
    }
    nomo_process_windows_env env =
        nomo_process_windows_build_environment(command);
    wchar_t *application =
        nomo_process_windows_resolve_program(command, &env);
    wchar_t *command_line =
        nomo_process_windows_command_line(command);
    wchar_t *environment =
        nomo_process_windows_environment_block(&env);
    wchar_t *cwd = command.@CWD_MEMBER@.tag == @CWD_SOME@
        ? nomo_process_windows_utf8_to_wide(
            command.@CWD_MEMBER@.payload.@SOME_PAYLOAD@.data
        )
        : NULL;
    nomo_process_windows_env_release(&env);
    if (application == NULL
        || command_line == NULL
        || environment == NULL
        || (command.@CWD_MEMBER@.tag == @CWD_SOME@ && cwd == NULL)) {
        free(application);
        free(command_line);
        free(environment);
        free(cwd);
        return nomo_process_control_start_error(
            "spawn",
            "failed to start process"
        );
    }

    SECURITY_ATTRIBUTES security = {
        .nLength = sizeof(SECURITY_ATTRIBUTES),
        .lpSecurityDescriptor = NULL,
        .bInheritHandle = TRUE
    };
    HANDLE stdin_read = NULL;
    HANDLE stdin_write = NULL;
    HANDLE stdout_read = NULL;
    HANDLE stdout_write = NULL;
    HANDLE stderr_read = NULL;
    HANDLE stderr_write = NULL;
    if (!CreatePipe(&stdin_read, &stdin_write, &security, 0)
        || !CreatePipe(&stdout_read, &stdout_write, &security, 0)
        || !CreatePipe(&stderr_read, &stderr_write, &security, 0)
        || !SetHandleInformation(
            stdin_write,
            HANDLE_FLAG_INHERIT,
            0
        )
        || !SetHandleInformation(
            stdout_read,
            HANDLE_FLAG_INHERIT,
            0
        )
        || !SetHandleInformation(
            stderr_read,
            HANDLE_FLAG_INHERIT,
            0
        )) {
        HANDLE handles[] = {
            stdin_read,
            stdin_write,
            stdout_read,
            stdout_write,
            stderr_read,
            stderr_write
        };
        for (size_t index = 0;
             index < sizeof(handles) / sizeof(handles[0]);
             index += 1U) {
            if (handles[index] != NULL) { CloseHandle(handles[index]); }
        }
        free(application);
        free(command_line);
        free(environment);
        free(cwd);
        return nomo_process_control_start_error(
            "spawn",
            "failed to create process pipes"
        );
    }

    STARTUPINFOW startup;
    PROCESS_INFORMATION process;
    memset(&startup, 0, sizeof(startup));
    memset(&process, 0, sizeof(process));
    startup.cb = sizeof(startup);
    startup.dwFlags = STARTF_USESTDHANDLES;
    startup.hStdInput = stdin_read;
    startup.hStdOutput = stdout_write;
    startup.hStdError = stderr_write;
    BOOL created = CreateProcessW(
        application,
        command_line,
        NULL,
        NULL,
        TRUE,
        CREATE_UNICODE_ENVIRONMENT,
        environment,
        cwd,
        &startup,
        &process
    );
    free(application);
    free(command_line);
    free(environment);
    free(cwd);
    CloseHandle(stdin_read);
    CloseHandle(stdout_write);
    CloseHandle(stderr_write);
    if (!created) {
        CloseHandle(stdin_write);
        CloseHandle(stdout_read);
        CloseHandle(stderr_read);
        return nomo_process_control_start_error(
            "spawn",
            "failed to start process"
        );
    }
    CloseHandle(process.hThread);

    nomo_process_control_state *state =
        (nomo_process_control_state *)calloc(
            1U,
            sizeof(nomo_process_control_state)
        );
    if (state == NULL) { nomo_panic("out of memory"); }
    state->id = nomo_process_control_allocate_id();
    state->process_handle = process.hProcess;
    state->stdin_write = stdin_write;
    state->stdout_read = stdout_read;
    state->stderr_read = stderr_read;
    InitializeCriticalSection(&state->lock);
    state->lock_initialized = 1;
    state->activity_event = CreateEventW(NULL, FALSE, FALSE, NULL);
    state->write_event = CreateEventW(NULL, FALSE, FALSE, NULL);
    state->stdout_space_event = CreateEventW(NULL, FALSE, FALSE, NULL);
    state->stderr_space_event = CreateEventW(NULL, FALSE, FALSE, NULL);
    state->stdout_context = (nomo_process_reader_context){
        .state = state,
        .is_stderr = 0
    };
    state->stderr_context = (nomo_process_reader_context){
        .state = state,
        .is_stderr = 1
    };
    if (state->activity_event != NULL
        && state->write_event != NULL
        && state->stdout_space_event != NULL
        && state->stderr_space_event != NULL) {
        state->writer_thread = CreateThread(
            NULL,
            0,
            nomo_process_windows_writer_thread,
            state,
            0,
            NULL
        );
        state->stdout_thread = CreateThread(
            NULL,
            0,
            nomo_process_windows_reader_thread,
            &state->stdout_context,
            0,
            NULL
        );
        state->stderr_thread = CreateThread(
            NULL,
            0,
            nomo_process_windows_reader_thread,
            &state->stderr_context,
            0,
            NULL
        );
    }
    if (state->writer_thread == NULL
        || state->stdout_thread == NULL
        || state->stderr_thread == NULL) {
        nomo_process_control_destroy(state);
        return nomo_process_control_start_error(
            "spawn",
            "failed to start process workers"
        );
    }
    state->next = nomo_process_control_states;
    nomo_process_control_states = state;
    return (@START_RESULT@){
        .tag = @START_OK@,
        .payload.@OK_PAYLOAD@ = (@PROCESS_CHILD@){
            .@HANDLE_MEMBER@ = state->id
        }
    };
}

static @VOID_RESULT@ @WRITE_STDIN_NAME@(
    @PROCESS_CHILD@ child,
    nomo_string data
) {
    nomo_process_control_state *state =
        nomo_process_control_find(child.@HANDLE_MEMBER@);
    if (state == NULL || state->exit_emitted) {
        return nomo_process_control_void_error(
            "invalid_request",
            "invalid process child"
        );
    }
    size_t len = strlen(data.data);
    if (len == 0 || len > (size_t)NOMO_PROCESS_MAX_PAYLOAD_BYTES) {
        return nomo_process_control_void_error(
            "invalid_request",
            "invalid process stdin payload"
        );
    }
    EnterCriticalSection(&state->lock);
    nomo_process_windows_update_exit_locked(state);
    if (state->stdin_closed || state->exited) {
        LeaveCriticalSection(&state->lock);
        return nomo_process_control_void_error(
            "invalid_request",
            "process stdin is closed"
        );
    }
    if (state->stdin_pending) {
        LeaveCriticalSection(&state->lock);
        return nomo_process_control_void_error(
            "busy",
            "process stdin already has pending data"
        );
    }
    state->stdin_data = (char *)malloc(len);
    if (state->stdin_data == NULL) { nomo_panic("out of memory"); }
    memcpy(state->stdin_data, data.data, len);
    state->stdin_len = len;
    state->stdin_offset = 0;
    state->stdin_pending = 1;
    state->stdin_flushed = 0;
    LeaveCriticalSection(&state->lock);
    SetEvent(state->write_event);
    return nomo_process_control_void_ok();
}

static @VOID_RESULT@ @CLOSE_STDIN_NAME@(@PROCESS_CHILD@ child) {
    nomo_process_control_state *state =
        nomo_process_control_find(child.@HANDLE_MEMBER@);
    if (state == NULL) {
        return nomo_process_control_void_error(
            "invalid_request",
            "invalid process child"
        );
    }
    EnterCriticalSection(&state->lock);
    if (state->stdin_pending) {
        LeaveCriticalSection(&state->lock);
        return nomo_process_control_void_error(
            "busy",
            "process stdin still has pending data"
        );
    }
    HANDLE handle = state->stdin_write;
    state->stdin_write = NULL;
    state->stdin_closed = 1;
    LeaveCriticalSection(&state->lock);
    if (handle != NULL) { CloseHandle(handle); }
    return nomo_process_control_void_ok();
}

static int nomo_process_windows_emit_stream_locked(
    nomo_process_control_state *state,
    int is_stderr,
    uint64_t max_chunk_bytes,
    @EVENT_RESULT@ *result
) {
    nomo_process_buffer *buffer = is_stderr
        ? &state->stderr_buffer
        : &state->stdout_buffer;
    int eof = is_stderr ? state->stderr_eof : state->stdout_eof;
    if (buffer->len == 0) { return 0; }
    size_t prefix = 0;
    if (!nomo_process_utf8_prefix(
            buffer->data,
            buffer->len,
            (size_t)max_chunk_bytes,
            eof,
            &prefix
        )) {
        return -1;
    }
    if (prefix == 0) { return 0; }
    nomo_string text = nomo_string_from_slice(buffer->data, 0, prefix);
    nomo_process_buffer_consume(buffer, prefix);
    state->prefer_stderr = !is_stderr;
    SetEvent(
        is_stderr
            ? state->stderr_space_event
            : state->stdout_space_event
    );
    *result = nomo_process_control_text_event(is_stderr, text);
    return 1;
}

static @EVENT_RESULT@ @NEXT_EVENT_NAME@(
    @PROCESS_CHILD@ child,
    uint64_t max_chunk_bytes,
    uint64_t timeout_millis
) {
    nomo_process_control_state *state =
        nomo_process_control_find(child.@HANDLE_MEMBER@);
    if (state == NULL || state->exit_emitted) {
        return nomo_process_control_event_error(
            "invalid_request",
            "invalid process child"
        );
    }
    if (max_chunk_bytes < UINT64_C(4)
        || max_chunk_bytes > NOMO_PROCESS_MAX_PAYLOAD_BYTES
        || timeout_millis == 0
        || timeout_millis > NOMO_PROCESS_MAX_TIMEOUT_MILLIS) {
        return nomo_process_control_event_error(
            "invalid_request",
            "invalid process event limit or timeout"
        );
    }
    uint64_t started = nomo_process_control_now_millis();
    uint64_t deadline = started > UINT64_MAX - timeout_millis
        ? UINT64_MAX
        : started + timeout_millis;
    for (;;) {
        EnterCriticalSection(&state->lock);
        nomo_process_windows_update_exit_locked(state);
        if (state->stdin_error) {
            state->stdin_error = 0;
            LeaveCriticalSection(&state->lock);
            return nomo_process_control_event_error(
                "io",
                "process stdin write failed"
            );
        }
        if (state->stdin_flushed) {
            state->stdin_flushed = 0;
            LeaveCriticalSection(&state->lock);
            return nomo_process_control_stdin_flushed_event();
        }
        @EVENT_RESULT@ stream_result;
        int first_stderr = state->prefer_stderr;
        int emitted = nomo_process_windows_emit_stream_locked(
            state,
            first_stderr,
            max_chunk_bytes,
            &stream_result
        );
        if (emitted == 0) {
            emitted = nomo_process_windows_emit_stream_locked(
                state,
                !first_stderr,
                max_chunk_bytes,
                &stream_result
            );
        }
        if (emitted < 0) {
            LeaveCriticalSection(&state->lock);
            nomo_process_control_unlink(state);
            nomo_process_control_destroy(state);
            return nomo_process_control_event_error(
                "protocol",
                "process output is not valid supported text"
            );
        }
        if (emitted > 0) {
            LeaveCriticalSection(&state->lock);
            return stream_result;
        }
        if (state->exited
            && state->stdout_eof
            && state->stderr_eof
            && state->stdout_buffer.len == 0
            && state->stderr_buffer.len == 0) {
            state->exit_emitted = 1;
            @PROCESS_EXIT@ exit_info = state->exit_info;
            LeaveCriticalSection(&state->lock);
            return nomo_process_control_exited_event(exit_info);
        }
        int exited = state->exited;
        LeaveCriticalSection(&state->lock);

        uint64_t now = nomo_process_control_now_millis();
        if (now >= deadline) {
            return nomo_process_control_event_error(
                "timeout",
                "process event timed out"
            );
        }
        uint64_t remaining = deadline - now;
        DWORD wait_millis = remaining > (uint64_t)MAXDWORD
            ? MAXDWORD
            : (DWORD)remaining;
        HANDLE handles[2] = {
            state->activity_event,
            state->process_handle
        };
        DWORD waited = WaitForMultipleObjects(
            exited ? 1U : 2U,
            handles,
            FALSE,
            wait_millis
        );
        if (waited == WAIT_TIMEOUT) {
            if (nomo_process_control_now_millis() >= deadline) {
                return nomo_process_control_event_error(
                    "timeout",
                    "process event timed out"
                );
            }
        } else if (waited == WAIT_FAILED) {
            return nomo_process_control_event_error(
                "io",
                "process event wait failed"
            );
        }
    }
}

static @WAIT_RESULT@ @TRY_WAIT_NAME@(@PROCESS_CHILD@ child) {
    nomo_process_control_state *state =
        nomo_process_control_find(child.@HANDLE_MEMBER@);
    if (state == NULL) {
        return nomo_process_control_wait_error(
            "invalid_request",
            "invalid process child"
        );
    }
    EnterCriticalSection(&state->lock);
    nomo_process_windows_update_exit_locked(state);
    int exited = state->exited;
    @PROCESS_EXIT@ exit_info = state->exit_info;
    LeaveCriticalSection(&state->lock);
    return exited
        ? nomo_process_control_wait_some(exit_info)
        : nomo_process_control_wait_none();
}

static @VOID_RESULT@ @TERMINATE_NAME@(@PROCESS_CHILD@ child) {
    nomo_process_control_state *state =
        nomo_process_control_find(child.@HANDLE_MEMBER@);
    if (state == NULL) {
        return nomo_process_control_void_error(
            "invalid_request",
            "invalid process child"
        );
    }
    EnterCriticalSection(&state->lock);
    nomo_process_windows_update_exit_locked(state);
    int exited = state->exited;
    HANDLE process = state->process_handle;
    LeaveCriticalSection(&state->lock);
    if (!exited && !TerminateProcess(process, 137U)) {
        return nomo_process_control_void_error(
            "io",
            "process termination failed"
        );
    }
    return nomo_process_control_void_ok();
}

static void @CLOSE_CHILD_NAME@(@PROCESS_CHILD@ child) {
    nomo_process_control_state *state =
        nomo_process_control_find(child.@HANDLE_MEMBER@);
    if (state == NULL) { return; }
    nomo_process_control_unlink(state);
    nomo_process_control_destroy(state);
}

#endif
