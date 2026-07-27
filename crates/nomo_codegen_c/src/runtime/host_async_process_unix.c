#include <pthread.h>
#include <signal.h>
#include <sys/wait.h>
#if defined(__linux__)
#include <sys/syscall.h>
extern long syscall(long number, ...);
#endif

extern char **environ;

#define NOMO_ASYNC_PROCESS_MAX_PAYLOAD_BYTES (UINT64_C(1024) * UINT64_C(1024))
#define NOMO_ASYNC_PROCESS_MAX_TIMEOUT_MILLIS (UINT64_C(15) * UINT64_C(60) * UINT64_C(1000))
#define NOMO_ASYNC_PROCESS_BUFFER_SLACK 4u
#define NOMO_ASYNC_PROCESS_HANDLE_CAPACITY 16u
#define NOMO_ASYNC_PROCESS_JOB_CAPACITY 32u
#define NOMO_ASYNC_PROCESS_START_JOB_CAPACITY 16u
#define NOMO_ASYNC_PROCESS_COMMAND_ITEM_CAPACITY 4096u
#define NOMO_ASYNC_PROCESS_EXIT_SOURCE_NONE 0u
#define NOMO_ASYNC_PROCESS_EXIT_SOURCE_REACTOR 1u
#define NOMO_ASYNC_PROCESS_EXIT_SOURCE_WORKER 2u

typedef struct {
    char *data;
    size_t len;
    size_t cap;
} nomo_async_process_buffer;

typedef struct {
    char *program;
    char *cwd;
    char **argv;
    size_t argc;
    char **envp;
    size_t envc;
} nomo_async_process_command_copy;

typedef enum {
    NOMO_ASYNC_PROCESS_JOB_FREE = 0,
    NOMO_ASYNC_PROCESS_JOB_START_QUEUED = 1,
    NOMO_ASYNC_PROCESS_JOB_START_RUNNING = 2,
    NOMO_ASYNC_PROCESS_JOB_START_COMPLETED = 3,
    NOMO_ASYNC_PROCESS_JOB_START_DELIVERED = 4,
    NOMO_ASYNC_PROCESS_JOB_START_CANCELLED = 5,
    NOMO_ASYNC_PROCESS_JOB_REAP_QUEUED = 6,
    NOMO_ASYNC_PROCESS_JOB_REAP_RUNNING = 7,
    NOMO_ASYNC_PROCESS_JOB_REAP_COMPLETED = 8
} nomo_async_process_job_state;

typedef void (*nomo_async_process_completion_fn)(
    void *,
    uint32_t,
    uint32_t
);

typedef struct {
    uint32_t generation;
    nomo_async_process_job_state state;
    nomo_async_process_command_copy command;
    pid_t pid;
    int stdin_fd;
    int stdout_fd;
    int stderr_fd;
    int spawn_errno;
    uint32_t handle_slot;
    uint32_t handle_generation;
    void *owner;
    nomo_async_process_completion_fn complete;
} nomo_async_process_job;

typedef struct {
    pid_t pid;
    int status;
    uint32_t handle_slot;
    uint32_t handle_generation;
    uint8_t active;
    uint8_t completed;
} nomo_async_process_watch;

typedef struct {
    uint32_t generation;
    uint8_t occupied;
    uint8_t stdin_closed;
    uint8_t stdin_pending;
    uint8_t stdin_flushed;
    uint8_t stdout_eof;
    uint8_t stderr_eof;
    uint8_t exited;
    uint8_t exit_emitted;
    uint8_t prefer_stderr;
    uint8_t event_busy;
    uint8_t exit_source;
    uint8_t reap_pending;
    pid_t pid;
    int exit_fd;
    int stdin_fd;
    int stdout_fd;
    int stderr_fd;
    char *stdin_data;
    size_t stdin_len;
    size_t stdin_offset;
    nomo_async_process_buffer stdout_buffer;
    nomo_async_process_buffer stderr_buffer;
    @PROCESS_EXIT@ exit_info;
    void *event_registration;
} nomo_async_process_handle_state;

typedef struct {
    nomo_async_context *context;
    pthread_mutex_t mutex;
    pthread_cond_t available;
    pthread_t worker;
    int wake_read;
    int wake_write;
    nomo_async_reactor_registration wake_registration;
    nomo_async_process_job jobs[NOMO_ASYNC_PROCESS_JOB_CAPACITY];
    uint32_t queue[NOMO_ASYNC_PROCESS_JOB_CAPACITY];
    uint32_t completions[NOMO_ASYNC_PROCESS_JOB_CAPACITY];
    uint32_t queue_head;
    uint32_t queue_tail;
    uint32_t queue_count;
    uint32_t completion_head;
    uint32_t completion_tail;
    uint32_t completion_count;
    uint32_t next_generation;
    uint32_t active_jobs;
    uint32_t active_start_jobs;
    uint32_t watch_count;
    nomo_async_process_watch watches[NOMO_ASYNC_PROCESS_HANDLE_CAPACITY];
    uint8_t stopping;
} nomo_async_process_pool;

typedef struct {
    nomo_async_context *context;
    nomo_async_process_pool *pool;
    nomo_async_process_handle_state handles[NOMO_ASYNC_PROCESS_HANDLE_CAPACITY];
    uint32_t next_handle_generation;
} nomo_async_process_runtime;

typedef enum {
    NOMO_ASYNC_PROCESS_REGISTRATION_NONE = 0,
    NOMO_ASYNC_PROCESS_REGISTRATION_START = 1,
    NOMO_ASYNC_PROCESS_REGISTRATION_EVENT = 2
} nomo_async_process_registration_kind;

typedef struct {
    nomo_async_process_registration_kind kind;
    nomo_async_context *context;
    void *frame;
    nomo_async_poll_fn poll;
    nomo_async_timer_registration timer;
    nomo_async_timer_outcome timer_outcome;
    nomo_async_reactor_registration io[4];
    uint32_t io_count;
    uint32_t job_slot;
    uint32_t job_generation;
    uint32_t handle_slot;
    uint32_t handle_generation;
    uint64_t max_chunk_bytes;
    int64_t deadline_millis;
    uint8_t active;
    uint8_t ready;
} nomo_async_process_registration;

static @PROCESS_ERROR@ nomo_async_process_error_value(
    const char *code,
    const char *message
) {
    return (@PROCESS_ERROR@){
        .@CODE_MEMBER@ = nomo_string_from_cstr(code),
        .@MESSAGE_MEMBER@ = nomo_string_from_cstr(message)
    };
}

static void nomo_async_process_start_error(
    @START_RESULT@ *result,
    const char *code,
    const char *message
) {
    memset(result, 0, sizeof(*result));
    result->tag = @START_ERR@;
    result->payload.@ERR_PAYLOAD@ =
        nomo_async_process_error_value(code, message);
}

static void nomo_async_process_void_error(
    @VOID_RESULT@ *result,
    const char *code,
    const char *message
) {
    memset(result, 0, sizeof(*result));
    result->tag = @VOID_ERR@;
    result->payload.@ERR_PAYLOAD@ =
        nomo_async_process_error_value(code, message);
}

static @VOID_RESULT@ nomo_async_process_void_ok(void) {
    return (@VOID_RESULT@){
        .tag = @VOID_OK@,
        .payload.@OK_PAYLOAD@ = 0
    };
}

static void nomo_async_process_event_error(
    @EVENT_RESULT@ *result,
    const char *code,
    const char *message
) {
    memset(result, 0, sizeof(*result));
    result->tag = @EVENT_ERR@;
    result->payload.@ERR_PAYLOAD@ =
        nomo_async_process_error_value(code, message);
}

static @WAIT_RESULT@ nomo_async_process_wait_error(
    const char *code,
    const char *message
) {
    return (@WAIT_RESULT@){
        .tag = @WAIT_ERR@,
        .payload.@ERR_PAYLOAD@ =
            nomo_async_process_error_value(code, message)
    };
}

static @PROCESS_EXIT@ nomo_async_process_exit_value(
    int32_t code,
    int32_t signal_value
) {
    return (@PROCESS_EXIT@){
        .@CODE_MEMBER@ = code,
        .@SIGNAL_MEMBER@ = signal_value
    };
}

static void nomo_async_process_close_fd(int *fd) {
    if (*fd >= 0) {
        close(*fd);
        *fd = -1;
    }
}

static void nomo_async_process_set_exit_status(
    nomo_async_process_handle_state *state,
    int status
) {
    state->exited = 1u;
    state->exit_source = NOMO_ASYNC_PROCESS_EXIT_SOURCE_NONE;
    nomo_async_process_close_fd(&state->exit_fd);
    if (WIFEXITED(status)) {
        state->exit_info = nomo_async_process_exit_value(
            (int32_t)WEXITSTATUS(status),
            0
        );
    } else if (WIFSIGNALED(status)) {
        int signal_value = WTERMSIG(status);
        state->exit_info = nomo_async_process_exit_value(
            (int32_t)(128 + signal_value),
            (int32_t)signal_value
        );
    } else {
        state->exit_info =
            nomo_async_process_exit_value((int32_t)status, 0);
    }
}

static int nomo_async_process_set_cloexec(int fd) {
    int flags = fcntl(fd, F_GETFD);
    return flags >= 0 && fcntl(fd, F_SETFD, flags | FD_CLOEXEC) == 0;
}

static int nomo_async_process_open_pidfd(pid_t pid) {
#if defined(__linux__) && defined(SYS_pidfd_open)
    int fd = (int)syscall(SYS_pidfd_open, pid, 0u);
    if (fd >= 0 && !nomo_async_process_set_cloexec(fd)) {
        nomo_async_process_close_fd(&fd);
    }
    return fd;
#else
    (void)pid;
    return -1;
#endif
}

static int nomo_async_process_set_nonblocking(int fd) {
    int flags = fcntl(fd, F_GETFL);
    return flags >= 0 && fcntl(fd, F_SETFL, flags | O_NONBLOCK) == 0;
}

static int nomo_async_process_make_pipe(int fds[2]) {
    if (pipe(fds) != 0) {
        return 1;
    }
    if (!nomo_async_process_set_cloexec(fds[0])
        || !nomo_async_process_set_cloexec(fds[1])) {
        nomo_async_process_close_fd(&fds[0]);
        nomo_async_process_close_fd(&fds[1]);
        return 1;
    }
    return 0;
}

static char *nomo_async_process_copy_cstr(const char *value) {
    size_t length = strlen(value);
    char *copy = (char *)malloc(length + 1u);
    if (copy == NULL) {
        return NULL;
    }
    memcpy(copy, value, length + 1u);
    return copy;
}

static void nomo_async_process_command_release(
    nomo_async_process_command_copy *command
) {
    free(command->program);
    free(command->cwd);
    if (command->argv != NULL) {
        for (size_t index = 0u; index < command->argc; index += 1u) {
            free(command->argv[index]);
        }
    }
    free(command->argv);
    if (command->envp != NULL) {
        for (size_t index = 0u; index < command->envc; index += 1u) {
            free(command->envp[index]);
        }
    }
    free(command->envp);
    memset(command, 0, sizeof(*command));
}

static size_t nomo_async_process_env_name_length(const char *entry) {
    const char *equals = strchr(entry, '=');
    return equals == NULL ? strlen(entry) : (size_t)(equals - entry);
}

static int nomo_async_process_env_matches(
    const char *entry,
    const char *name
) {
    size_t entry_length = nomo_async_process_env_name_length(entry);
    size_t name_length = strlen(name);
    return entry_length == name_length
        && memcmp(entry, name, name_length) == 0;
}

static const char *nomo_async_process_env_get(
    char *const envp[],
    const char *name
) {
    if (envp == NULL) {
        return NULL;
    }
    for (size_t index = 0u; envp[index] != NULL; index += 1u) {
        if (nomo_async_process_env_matches(envp[index], name)) {
            const char *equals = strchr(envp[index], '=');
            return equals == NULL ? "" : equals + 1u;
        }
    }
    return NULL;
}

static int nomo_async_process_env_set(
    nomo_async_process_command_copy *copy,
    const char *name,
    const char *value
) {
    size_t name_length = strlen(name);
    size_t value_length = strlen(value);
    char *entry = (char *)malloc(name_length + value_length + 2u);
    if (entry == NULL) {
        return 1;
    }
    memcpy(entry, name, name_length);
    entry[name_length] = '=';
    memcpy(entry + name_length + 1u, value, value_length + 1u);
    for (size_t index = 0u; index < copy->envc; index += 1u) {
        if (nomo_async_process_env_matches(copy->envp[index], name)) {
            free(copy->envp[index]);
            copy->envp[index] = entry;
            return 0;
        }
    }
    char **next = (char **)realloc(
        copy->envp,
        (copy->envc + 2u) * sizeof(char *)
    );
    if (next == NULL) {
        free(entry);
        return 1;
    }
    copy->envp = next;
    copy->envp[copy->envc] = entry;
    copy->envc += 1u;
    copy->envp[copy->envc] = NULL;
    return 0;
}

static int nomo_async_process_copy_command(
    @PROCESS_COMMAND@ command,
    nomo_async_process_command_copy *copy
) {
    memset(copy, 0, sizeof(*copy));
    if (command.@PROGRAM_MEMBER@.data[0] == '\0'
        || command.@ARGS_MEMBER@.len > NOMO_ASYNC_PROCESS_COMMAND_ITEM_CAPACITY
        || command.@ENV_MEMBER@.len > NOMO_ASYNC_PROCESS_COMMAND_ITEM_CAPACITY) {
        return 1;
    }
    size_t retained = strlen(command.@PROGRAM_MEMBER@.data) + 1u;
    for (size_t index = 0u; index < command.@ARGS_MEMBER@.len; index += 1u) {
        retained += strlen(command.@ARGS_MEMBER@.data[index].data) + 1u;
    }
    for (size_t index = 0u; index < command.@ENV_MEMBER@.len; index += 1u) {
        @PROCESS_ENV@ item = command.@ENV_MEMBER@.data[index];
        if (item.@NAME_MEMBER@.data[0] == '\0'
            || strchr(item.@NAME_MEMBER@.data, '=') != NULL) {
            return 1;
        }
        retained += strlen(item.@NAME_MEMBER@.data)
            + strlen(item.@VALUE_MEMBER@.data) + 2u;
        for (size_t other = index + 1u;
             other < command.@ENV_MEMBER@.len;
             other += 1u) {
            if (strcmp(
                    item.@NAME_MEMBER@.data,
                    command.@ENV_MEMBER@.data[other].@NAME_MEMBER@.data
                ) == 0) {
                return 1;
            }
        }
    }
    if (command.@CWD_MEMBER@.tag == @CWD_SOME@) {
        retained += strlen(
            command.@CWD_MEMBER@.payload.@SOME_PAYLOAD@.data
        ) + 1u;
    }
    if (retained > NOMO_ASYNC_PROCESS_MAX_PAYLOAD_BYTES) {
        return 1;
    }

    copy->program = nomo_async_process_copy_cstr(
        command.@PROGRAM_MEMBER@.data
    );
    copy->argc = command.@ARGS_MEMBER@.len + 1u;
    copy->argv = (char **)calloc(copy->argc + 1u, sizeof(char *));
    if (copy->program == NULL || copy->argv == NULL) {
        nomo_async_process_command_release(copy);
        return 2;
    }
    copy->argv[0] = nomo_async_process_copy_cstr(
        command.@PROGRAM_MEMBER@.data
    );
    if (copy->argv[0] == NULL) {
        nomo_async_process_command_release(copy);
        return 2;
    }
    for (size_t index = 0u; index < command.@ARGS_MEMBER@.len; index += 1u) {
        copy->argv[index + 1u] = nomo_async_process_copy_cstr(
            command.@ARGS_MEMBER@.data[index].data
        );
        if (copy->argv[index + 1u] == NULL) {
            nomo_async_process_command_release(copy);
            return 2;
        }
    }
    if (command.@CWD_MEMBER@.tag == @CWD_SOME@) {
        copy->cwd = nomo_async_process_copy_cstr(
            command.@CWD_MEMBER@.payload.@SOME_PAYLOAD@.data
        );
        if (copy->cwd == NULL) {
            nomo_async_process_command_release(copy);
            return 2;
        }
    }
    if (command.@INHERIT_ENV_MEMBER@) {
        size_t inherited_count = 0u;
        while (environ[inherited_count] != NULL) {
            if (inherited_count
                    >= NOMO_ASYNC_PROCESS_COMMAND_ITEM_CAPACITY
                || command.@ENV_MEMBER@.len
                    > NOMO_ASYNC_PROCESS_COMMAND_ITEM_CAPACITY
                        - inherited_count) {
                nomo_async_process_command_release(copy);
                return 1;
            }
            retained += strlen(environ[inherited_count]) + 1u;
            if (retained > NOMO_ASYNC_PROCESS_MAX_PAYLOAD_BYTES) {
                nomo_async_process_command_release(copy);
                return 1;
            }
            inherited_count += 1u;
        }
        copy->envc = inherited_count;
        copy->envp = (char **)calloc(copy->envc + 1u, sizeof(char *));
        if (copy->envp == NULL) {
            nomo_async_process_command_release(copy);
            return 2;
        }
        for (size_t index = 0u; index < copy->envc; index += 1u) {
            copy->envp[index] = nomo_async_process_copy_cstr(environ[index]);
            if (copy->envp[index] == NULL) {
                nomo_async_process_command_release(copy);
                return 2;
            }
        }
    } else {
        copy->envp = (char **)calloc(1u, sizeof(char *));
        if (copy->envp == NULL) {
            nomo_async_process_command_release(copy);
            return 2;
        }
    }
    for (size_t index = 0u; index < command.@ENV_MEMBER@.len; index += 1u) {
        @PROCESS_ENV@ item = command.@ENV_MEMBER@.data[index];
        if (nomo_async_process_env_set(
                copy,
                item.@NAME_MEMBER@.data,
                item.@VALUE_MEMBER@.data
            ) != 0) {
            nomo_async_process_command_release(copy);
            return 2;
        }
    }
    return 0;
}

static char *nomo_async_process_resolve_program(
    const nomo_async_process_command_copy *command
) {
    if (strchr(command->program, '/') != NULL) {
        return nomo_async_process_copy_cstr(command->program);
    }
    const char *path = nomo_async_process_env_get(command->envp, "PATH");
    if (path == NULL) {
        errno = ENOENT;
        return NULL;
    }
    int permission_error = 0;
    const char *cursor = path;
    for (;;) {
        const char *separator = strchr(cursor, ':');
        size_t directory_length = separator == NULL
            ? strlen(cursor)
            : (size_t)(separator - cursor);
        size_t program_length = strlen(command->program);
        size_t capacity =
            (directory_length == 0u ? 1u : directory_length)
            + program_length + 2u;
        char *candidate = (char *)malloc(capacity);
        if (candidate == NULL) {
            errno = ENOMEM;
            return NULL;
        }
        if (directory_length == 0u) {
            snprintf(candidate, capacity, "./%s", command->program);
        } else {
            memcpy(candidate, cursor, directory_length);
            candidate[directory_length] = '/';
            memcpy(
                candidate + directory_length + 1u,
                command->program,
                program_length + 1u
            );
        }
        if (access(candidate, X_OK) == 0) {
            return candidate;
        }
        if (errno == EACCES) {
            permission_error = 1;
        }
        free(candidate);
        if (separator == NULL) {
            break;
        }
        cursor = separator + 1u;
    }
    errno = permission_error ? EACCES : ENOENT;
    return NULL;
}

static void nomo_async_process_spawn_cleanup(
    pid_t pid,
    int *stdin_fd,
    int *stdout_fd,
    int *stderr_fd
) {
    nomo_async_process_close_fd(stdin_fd);
    nomo_async_process_close_fd(stdout_fd);
    nomo_async_process_close_fd(stderr_fd);
    if (pid <= 0) {
        return;
    }
    if (kill(pid, SIGKILL) != 0 && errno != ESRCH) {
    }
    int status = 0;
    while (waitpid(pid, &status, 0) < 0 && errno == EINTR) {
    }
}

static void nomo_async_process_spawn_blocking(
    nomo_async_process_command_copy *command,
    pid_t *pid_out,
    int *stdin_out,
    int *stdout_out,
    int *stderr_out,
    int *spawn_errno
) {
    *pid_out = -1;
    *stdin_out = -1;
    *stdout_out = -1;
    *stderr_out = -1;
    *spawn_errno = 0;
    char *program = nomo_async_process_resolve_program(command);
    if (program == NULL) {
        *spawn_errno = errno == 0 ? ENOENT : errno;
        return;
    }
    int stdin_pipe[2] = {-1, -1};
    int stdout_pipe[2] = {-1, -1};
    int stderr_pipe[2] = {-1, -1};
    int exec_pipe[2] = {-1, -1};
    if (nomo_async_process_make_pipe(stdin_pipe) != 0
        || nomo_async_process_make_pipe(stdout_pipe) != 0
        || nomo_async_process_make_pipe(stderr_pipe) != 0
        || nomo_async_process_make_pipe(exec_pipe) != 0) {
        *spawn_errno = errno == 0 ? EMFILE : errno;
        nomo_async_process_close_fd(&stdin_pipe[0]);
        nomo_async_process_close_fd(&stdin_pipe[1]);
        nomo_async_process_close_fd(&stdout_pipe[0]);
        nomo_async_process_close_fd(&stdout_pipe[1]);
        nomo_async_process_close_fd(&stderr_pipe[0]);
        nomo_async_process_close_fd(&stderr_pipe[1]);
        nomo_async_process_close_fd(&exec_pipe[0]);
        nomo_async_process_close_fd(&exec_pipe[1]);
        free(program);
        return;
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
        if (command->cwd != NULL && chdir(command->cwd) != 0) {
            int child_errno = errno;
            (void)write(exec_pipe[1], &child_errno, sizeof(child_errno));
            _exit(127);
        }
        execve(program, command->argv, command->envp);
        int child_errno = errno;
        (void)write(exec_pipe[1], &child_errno, sizeof(child_errno));
        _exit(127);
    }
    free(program);
    if (pid < 0) {
        *spawn_errno = errno == 0 ? EAGAIN : errno;
        nomo_async_process_close_fd(&stdin_pipe[0]);
        nomo_async_process_close_fd(&stdin_pipe[1]);
        nomo_async_process_close_fd(&stdout_pipe[0]);
        nomo_async_process_close_fd(&stdout_pipe[1]);
        nomo_async_process_close_fd(&stderr_pipe[0]);
        nomo_async_process_close_fd(&stderr_pipe[1]);
        nomo_async_process_close_fd(&exec_pipe[0]);
        nomo_async_process_close_fd(&exec_pipe[1]);
        return;
    }
    nomo_async_process_close_fd(&stdin_pipe[0]);
    nomo_async_process_close_fd(&stdout_pipe[1]);
    nomo_async_process_close_fd(&stderr_pipe[1]);
    nomo_async_process_close_fd(&exec_pipe[1]);
    int child_errno = 0;
    ssize_t exec_read;
    do {
        exec_read = read(exec_pipe[0], &child_errno, sizeof(child_errno));
    } while (exec_read < 0 && errno == EINTR);
    nomo_async_process_close_fd(&exec_pipe[0]);
    if (exec_read != 0) {
        *spawn_errno = child_errno == 0 ? EIO : child_errno;
        nomo_async_process_spawn_cleanup(
            pid,
            &stdin_pipe[1],
            &stdout_pipe[0],
            &stderr_pipe[0]
        );
        return;
    }
    if (!nomo_async_process_set_nonblocking(stdin_pipe[1])
        || !nomo_async_process_set_nonblocking(stdout_pipe[0])
        || !nomo_async_process_set_nonblocking(stderr_pipe[0])) {
        *spawn_errno = errno == 0 ? EIO : errno;
        nomo_async_process_spawn_cleanup(
            pid,
            &stdin_pipe[1],
            &stdout_pipe[0],
            &stderr_pipe[0]
        );
        return;
    }
    *pid_out = pid;
    *stdin_out = stdin_pipe[1];
    *stdout_out = stdout_pipe[0];
    *stderr_out = stderr_pipe[0];
}

static void nomo_async_process_job_release_locked(
    nomo_async_process_pool *pool,
    nomo_async_process_job *job
) {
    uint8_t was_start =
        job->state == NOMO_ASYNC_PROCESS_JOB_START_QUEUED
        || job->state == NOMO_ASYNC_PROCESS_JOB_START_RUNNING
        || job->state == NOMO_ASYNC_PROCESS_JOB_START_COMPLETED
        || job->state == NOMO_ASYNC_PROCESS_JOB_START_DELIVERED
        || job->state == NOMO_ASYNC_PROCESS_JOB_START_CANCELLED;
    nomo_async_process_command_release(&job->command);
    job->pid = -1;
    job->stdin_fd = -1;
    job->stdout_fd = -1;
    job->stderr_fd = -1;
    job->spawn_errno = 0;
    job->handle_slot = 0u;
    job->handle_generation = 0u;
    job->owner = NULL;
    job->complete = NULL;
    job->state = NOMO_ASYNC_PROCESS_JOB_FREE;
    if (pool->active_jobs > 0u) {
        pool->active_jobs -= 1u;
    }
    if (was_start != 0u && pool->active_start_jobs > 0u) {
        pool->active_start_jobs -= 1u;
    }
    if (pool->context->live_blocking_jobs > 0u) {
        pool->context->live_blocking_jobs -= 1u;
    }
}

static int nomo_async_process_remove_queued_locked(
    nomo_async_process_pool *pool,
    uint32_t selected
) {
    uint32_t kept[NOMO_ASYNC_PROCESS_JOB_CAPACITY];
    uint32_t kept_count = 0u;
    uint8_t removed = 0u;
    while (pool->queue_count > 0u) {
        uint32_t slot = pool->queue[pool->queue_head];
        pool->queue_head =
            (pool->queue_head + 1u) % NOMO_ASYNC_PROCESS_JOB_CAPACITY;
        pool->queue_count -= 1u;
        if (slot == selected && removed == 0u) {
            removed = 1u;
            continue;
        }
        kept[kept_count] = slot;
        kept_count += 1u;
    }
    pool->queue_head = 0u;
    pool->queue_tail = 0u;
    for (uint32_t index = 0u; index < kept_count; index += 1u) {
        pool->queue[pool->queue_tail] = kept[index];
        pool->queue_tail =
            (pool->queue_tail + 1u) % NOMO_ASYNC_PROCESS_JOB_CAPACITY;
        pool->queue_count += 1u;
    }
    return removed == 0u;
}

static void nomo_async_process_signal_completion(
    nomo_async_process_pool *pool
) {
    unsigned char signal_value = 1u;
    ssize_t ignored = write(
        pool->wake_write,
        &signal_value,
        sizeof(signal_value)
    );
    (void)ignored;
}

static int nomo_async_process_scan_watches_locked(
    nomo_async_process_pool *pool
) {
    int completed = 0;
    for (uint32_t index = 0u;
         index < NOMO_ASYNC_PROCESS_HANDLE_CAPACITY;
         index += 1u) {
        nomo_async_process_watch *watch = &pool->watches[index];
        if (watch->active == 0u || watch->completed != 0u) {
            continue;
        }
        int status = 0;
        pid_t waited;
        do {
            waited = waitpid(watch->pid, &status, WNOHANG);
        } while (waited < 0 && errno == EINTR);
        if (waited == watch->pid || (waited < 0 && errno == ECHILD)) {
            watch->status = waited == watch->pid ? status : 0;
            watch->completed = 1u;
            completed = 1;
        }
    }
    return completed;
}

static void *nomo_async_process_worker(void *raw_pool) {
    nomo_async_process_pool *pool = (nomo_async_process_pool *)raw_pool;
    for (;;) {
        if (pthread_mutex_lock(&pool->mutex) != 0) {
            return NULL;
        }
        while (pool->queue_count == 0u && pool->stopping == 0u) {
            int wait_status = 0;
            if (pool->watch_count == 0u) {
                wait_status = pthread_cond_wait(
                    &pool->available,
                    &pool->mutex
                );
            } else {
                struct timespec deadline;
                if (clock_gettime(CLOCK_REALTIME, &deadline) != 0) {
                    pthread_mutex_unlock(&pool->mutex);
                    return NULL;
                }
                deadline.tv_nsec += 10000000L;
                if (deadline.tv_nsec >= 1000000000L) {
                    deadline.tv_sec += 1;
                    deadline.tv_nsec -= 1000000000L;
                }
                wait_status = pthread_cond_timedwait(
                    &pool->available,
                    &pool->mutex,
                    &deadline
                );
                if (wait_status == ETIMEDOUT) {
                    wait_status = 0;
                }
            }
            if (wait_status != 0) {
                pthread_mutex_unlock(&pool->mutex);
                return NULL;
            }
            if (nomo_async_process_scan_watches_locked(pool) != 0) {
                pthread_mutex_unlock(&pool->mutex);
                nomo_async_process_signal_completion(pool);
                goto next_iteration;
            }
        }
        if (pool->stopping != 0u && pool->queue_count == 0u) {
            pthread_mutex_unlock(&pool->mutex);
            return NULL;
        }
        uint32_t slot = pool->queue[pool->queue_head];
        pool->queue_head =
            (pool->queue_head + 1u) % NOMO_ASYNC_PROCESS_JOB_CAPACITY;
        pool->queue_count -= 1u;
        nomo_async_process_job *job = &pool->jobs[slot];
        nomo_async_process_job_state selected_state = job->state;
        if (selected_state == NOMO_ASYNC_PROCESS_JOB_START_QUEUED) {
            job->state = NOMO_ASYNC_PROCESS_JOB_START_RUNNING;
        } else if (selected_state == NOMO_ASYNC_PROCESS_JOB_REAP_QUEUED) {
            job->state = NOMO_ASYNC_PROCESS_JOB_REAP_RUNNING;
        } else {
            pthread_mutex_unlock(&pool->mutex);
            continue;
        }
        pthread_mutex_unlock(&pool->mutex);

        if (selected_state == NOMO_ASYNC_PROCESS_JOB_START_QUEUED) {
            pid_t pid = -1;
            int stdin_fd = -1;
            int stdout_fd = -1;
            int stderr_fd = -1;
            int spawn_errno = 0;
#ifdef NOMO_ASYNC_PROCESS_TEST_DELAY_MILLIS
            struct timespec process_test_delay = {
                .tv_sec = NOMO_ASYNC_PROCESS_TEST_DELAY_MILLIS / 1000,
                .tv_nsec =
                    (NOMO_ASYNC_PROCESS_TEST_DELAY_MILLIS % 1000)
                    * 1000000L
            };
            while (nanosleep(
                    &process_test_delay,
                    &process_test_delay
                ) != 0 && errno == EINTR) {
            }
#endif
            nomo_async_process_spawn_blocking(
                &job->command,
                &pid,
                &stdin_fd,
                &stdout_fd,
                &stderr_fd,
                &spawn_errno
            );
            nomo_async_process_command_release(&job->command);
            if (pthread_mutex_lock(&pool->mutex) != 0) {
                nomo_async_process_spawn_cleanup(
                    pid,
                    &stdin_fd,
                    &stdout_fd,
                    &stderr_fd
                );
                return NULL;
            }
            job = &pool->jobs[slot];
            if (job->state == NOMO_ASYNC_PROCESS_JOB_START_RUNNING) {
                job->pid = pid;
                job->stdin_fd = stdin_fd;
                job->stdout_fd = stdout_fd;
                job->stderr_fd = stderr_fd;
                job->spawn_errno = spawn_errno;
                job->state = NOMO_ASYNC_PROCESS_JOB_START_COMPLETED;
            } else {
                job->pid = -1;
                job->stdin_fd = -1;
                job->stdout_fd = -1;
                job->stderr_fd = -1;
                job->spawn_errno = ECANCELED;
                job->state = NOMO_ASYNC_PROCESS_JOB_START_CANCELLED;
            }
            uint8_t cancelled =
                job->state == NOMO_ASYNC_PROCESS_JOB_START_CANCELLED;
            if (pool->completion_count == NOMO_ASYNC_PROCESS_JOB_CAPACITY) {
                pthread_mutex_unlock(&pool->mutex);
                nomo_async_process_spawn_cleanup(
                    pid,
                    &stdin_fd,
                    &stdout_fd,
                    &stderr_fd
                );
                return NULL;
            }
            pool->completions[pool->completion_tail] = slot;
            pool->completion_tail =
                (pool->completion_tail + 1u)
                % NOMO_ASYNC_PROCESS_JOB_CAPACITY;
            pool->completion_count += 1u;
            pthread_mutex_unlock(&pool->mutex);
            if (cancelled != 0u) {
                nomo_async_process_spawn_cleanup(
                    pid,
                    &stdin_fd,
                    &stdout_fd,
                    &stderr_fd
                );
            }
            nomo_async_process_signal_completion(pool);
            continue;
        }

        pid_t pid = job->pid;
        if (pid > 0) {
            if (kill(pid, SIGKILL) != 0 && errno != ESRCH) {
            }
            int status = 0;
            while (waitpid(pid, &status, 0) < 0 && errno == EINTR) {
            }
        }
        if (pthread_mutex_lock(&pool->mutex) != 0) {
            return NULL;
        }
        job = &pool->jobs[slot];
        job->pid = -1;
        job->state = NOMO_ASYNC_PROCESS_JOB_REAP_COMPLETED;
        if (pool->completion_count == NOMO_ASYNC_PROCESS_JOB_CAPACITY) {
            pthread_mutex_unlock(&pool->mutex);
            return NULL;
        }
        pool->completions[pool->completion_tail] = slot;
        pool->completion_tail =
            (pool->completion_tail + 1u)
            % NOMO_ASYNC_PROCESS_JOB_CAPACITY;
        pool->completion_count += 1u;
        pthread_mutex_unlock(&pool->mutex);
        nomo_async_process_signal_completion(pool);
next_iteration:
        ;
    }
}

static void nomo_async_process_event_wake(void *owner, uint32_t ready);
static void nomo_async_process_reap_complete(
    nomo_async_context *context,
    uint32_t handle_slot,
    uint32_t handle_generation
);

static void nomo_async_process_pool_deliver_watches(
    nomo_async_process_pool *pool
) {
    for (;;) {
        nomo_async_process_watch completed = {0};
        if (pthread_mutex_lock(&pool->mutex) != 0) {
            pool->context->runtime_failed = 1u;
            return;
        }
        uint32_t selected = NOMO_ASYNC_PROCESS_HANDLE_CAPACITY;
        for (uint32_t index = 0u;
             index < NOMO_ASYNC_PROCESS_HANDLE_CAPACITY;
             index += 1u) {
            if (pool->watches[index].active != 0u
                && pool->watches[index].completed != 0u) {
                selected = index;
                break;
            }
        }
        if (selected != NOMO_ASYNC_PROCESS_HANDLE_CAPACITY) {
            completed = pool->watches[selected];
            memset(&pool->watches[selected], 0, sizeof(completed));
            pool->watches[selected].pid = -1;
            if (pool->watch_count > 0u) {
                pool->watch_count -= 1u;
            }
        }
        pthread_mutex_unlock(&pool->mutex);
        if (selected == NOMO_ASYNC_PROCESS_HANDLE_CAPACITY) {
            return;
        }
        nomo_async_process_runtime *runtime =
            (nomo_async_process_runtime *)pool->context->process_runtime;
        if (runtime == NULL
            || completed.handle_slot
                >= NOMO_ASYNC_PROCESS_HANDLE_CAPACITY) {
            continue;
        }
        nomo_async_process_handle_state *state =
            &runtime->handles[completed.handle_slot];
        if (state->occupied != 1u
            || state->generation != completed.handle_generation
            || state->pid != completed.pid
            || state->exit_source
                != NOMO_ASYNC_PROCESS_EXIT_SOURCE_WORKER) {
            continue;
        }
        nomo_async_process_set_exit_status(state, completed.status);
        if (state->event_registration != NULL) {
            nomo_async_process_event_wake(
                state->event_registration,
                NOMO_ASYNC_REACTOR_PROCESS
            );
        }
    }
}

static void nomo_async_process_pool_completion_wake(
    void *raw_pool,
    uint32_t ready
) {
    nomo_async_process_pool *pool = (nomo_async_process_pool *)raw_pool;
    if (pool == NULL || (ready & NOMO_ASYNC_REACTOR_READ) == 0u) {
        return;
    }
    unsigned char signals[32];
    while (read(pool->wake_read, signals, sizeof(signals)) > 0) {
    }
    for (;;) {
        void *owner = NULL;
        nomo_async_process_completion_fn complete = NULL;
        uint32_t slot = 0u;
        uint32_t generation = 0u;
        uint32_t handle_slot = 0u;
        uint32_t handle_generation = 0u;
        uint8_t reaped = 0u;
        if (pthread_mutex_lock(&pool->mutex) != 0) {
            pool->context->runtime_failed = 1u;
            return;
        }
        if (pool->completion_count == 0u) {
            pthread_mutex_unlock(&pool->mutex);
            break;
        }
        slot = pool->completions[pool->completion_head];
        pool->completion_head =
            (pool->completion_head + 1u)
            % NOMO_ASYNC_PROCESS_JOB_CAPACITY;
        pool->completion_count -= 1u;
        nomo_async_process_job *job = &pool->jobs[slot];
        pool->context->blocking_jobs_started += 1u;
        pool->context->blocking_jobs_completed += 1u;
        if (job->state == NOMO_ASYNC_PROCESS_JOB_START_COMPLETED) {
            job->state = NOMO_ASYNC_PROCESS_JOB_START_DELIVERED;
            owner = job->owner;
            complete = job->complete;
            generation = job->generation;
        } else {
            if (job->state == NOMO_ASYNC_PROCESS_JOB_REAP_COMPLETED) {
                handle_slot = job->handle_slot;
                handle_generation = job->handle_generation;
                reaped = 1u;
            }
            nomo_async_process_job_release_locked(pool, job);
        }
        pthread_mutex_unlock(&pool->mutex);
        if (complete != NULL) {
            complete(owner, slot, generation);
        }
        if (reaped != 0u) {
            nomo_async_process_reap_complete(
                pool->context,
                handle_slot,
                handle_generation
            );
        }
    }
    nomo_async_process_pool_deliver_watches(pool);
    uint32_t active_jobs = 0u;
    uint32_t watch_count = 0u;
    if (pthread_mutex_lock(&pool->mutex) != 0) {
        pool->context->runtime_failed = 1u;
        return;
    }
    active_jobs = pool->active_jobs;
    watch_count = pool->watch_count;
    pthread_mutex_unlock(&pool->mutex);
    if (active_jobs == 0u && watch_count == 0u) {
        nomo_async_reactor_deregister(
            &pool->context->reactor,
            &pool->wake_registration
        );
    } else if (nomo_async_reactor_reregister(
            &pool->context->reactor,
            &pool->wake_registration,
            NOMO_ASYNC_REACTOR_READ
        ) != 0) {
        pool->context->runtime_failed = 1u;
    }
}

static int nomo_async_process_pool_initialize(
    nomo_async_process_runtime *runtime
) {
    if (runtime->pool != NULL) {
        return 0;
    }
    nomo_async_process_pool *pool =
        (nomo_async_process_pool *)calloc(1u, sizeof(*pool));
    if (pool == NULL) {
        return 1;
    }
    pool->context = runtime->context;
    pool->wake_read = -1;
    pool->wake_write = -1;
    for (uint32_t index = 0u;
         index < NOMO_ASYNC_PROCESS_JOB_CAPACITY;
         index += 1u) {
        pool->jobs[index].pid = -1;
        pool->jobs[index].stdin_fd = -1;
        pool->jobs[index].stdout_fd = -1;
        pool->jobs[index].stderr_fd = -1;
    }
    for (uint32_t index = 0u;
         index < NOMO_ASYNC_PROCESS_HANDLE_CAPACITY;
         index += 1u) {
        pool->watches[index].pid = -1;
    }
    if (pthread_mutex_init(&pool->mutex, NULL) != 0) {
        free(pool);
        return 1;
    }
    if (pthread_cond_init(&pool->available, NULL) != 0) {
        pthread_mutex_destroy(&pool->mutex);
        free(pool);
        return 1;
    }
    int wake_pipe[2] = {-1, -1};
    if (nomo_async_process_make_pipe(wake_pipe) != 0
        || !nomo_async_process_set_nonblocking(wake_pipe[0])
        || !nomo_async_process_set_nonblocking(wake_pipe[1])) {
        nomo_async_process_close_fd(&wake_pipe[0]);
        nomo_async_process_close_fd(&wake_pipe[1]);
        pthread_cond_destroy(&pool->available);
        pthread_mutex_destroy(&pool->mutex);
        free(pool);
        return 1;
    }
    pool->wake_read = wake_pipe[0];
    pool->wake_write = wake_pipe[1];
    pool->wake_registration.owner = pool;
    pool->wake_registration.wake =
        nomo_async_process_pool_completion_wake;
    if (pthread_create(
            &pool->worker,
            NULL,
            nomo_async_process_worker,
            pool
        ) != 0) {
        close(pool->wake_read);
        close(pool->wake_write);
        pthread_cond_destroy(&pool->available);
        pthread_mutex_destroy(&pool->mutex);
        free(pool);
        return 1;
    }
    runtime->pool = pool;
    runtime->context->blocking_pool_initializations += 1u;
    runtime->context->blocking_threads_started += 1u;
    runtime->context->live_blocking_threads += 1u;
    if (runtime->context->live_blocking_threads
        > runtime->context->peak_live_blocking_threads) {
        runtime->context->peak_live_blocking_threads =
            runtime->context->live_blocking_threads;
    }
    return 0;
}

static int nomo_async_process_pool_ensure_wake(
    nomo_async_process_pool *pool
) {
    if (pool->wake_registration.active != 0u) {
        return 0;
    }
    return nomo_async_reactor_register(
        &pool->context->reactor,
        &pool->wake_registration,
        pool->wake_read,
        NOMO_ASYNC_REACTOR_READ
    );
}

static void nomo_async_process_pool_maybe_idle(
    nomo_async_process_pool *pool
) {
    uint32_t active_jobs = 0u;
    uint32_t watch_count = 0u;
    if (pool == NULL || pthread_mutex_lock(&pool->mutex) != 0) {
        return;
    }
    active_jobs = pool->active_jobs;
    watch_count = pool->watch_count;
    pthread_mutex_unlock(&pool->mutex);
    if (active_jobs == 0u
        && watch_count == 0u
        && pool->wake_registration.active != 0u) {
        nomo_async_reactor_deregister(
            &pool->context->reactor,
            &pool->wake_registration
        );
    }
}

static int nomo_async_process_pool_watch(
    nomo_async_process_runtime *runtime,
    pid_t pid,
    uint32_t handle_slot,
    uint32_t handle_generation
) {
    nomo_async_process_pool *pool = runtime->pool;
    if (pool == NULL
        || nomo_async_process_pool_ensure_wake(pool) != 0
        || pthread_mutex_lock(&pool->mutex) != 0) {
        return 1;
    }
    uint32_t selected = NOMO_ASYNC_PROCESS_HANDLE_CAPACITY;
    for (uint32_t index = 0u;
         index < NOMO_ASYNC_PROCESS_HANDLE_CAPACITY;
         index += 1u) {
        if (pool->watches[index].active == 0u) {
            selected = index;
            break;
        }
    }
    if (selected == NOMO_ASYNC_PROCESS_HANDLE_CAPACITY) {
        pthread_mutex_unlock(&pool->mutex);
        nomo_async_process_pool_maybe_idle(pool);
        return 1;
    }
    pool->watches[selected] = (nomo_async_process_watch){
        .pid = pid,
        .handle_slot = handle_slot,
        .handle_generation = handle_generation,
        .active = 1u
    };
    pool->watch_count += 1u;
    pthread_cond_signal(&pool->available);
    pthread_mutex_unlock(&pool->mutex);
    return 0;
}

static int nomo_async_process_pool_unwatch(
    nomo_async_process_runtime *runtime,
    nomo_async_process_handle_state *state,
    int *status_out
) {
    nomo_async_process_pool *pool = runtime->pool;
    if (pool == NULL || pthread_mutex_lock(&pool->mutex) != 0) {
        return 0;
    }
    int outcome = 0;
    for (uint32_t index = 0u;
         index < NOMO_ASYNC_PROCESS_HANDLE_CAPACITY;
         index += 1u) {
        nomo_async_process_watch *watch = &pool->watches[index];
        if (watch->active == 0u
            || watch->pid != state->pid
            || watch->handle_generation != state->generation) {
            continue;
        }
        outcome = watch->completed != 0u ? 2 : 1;
        if (outcome == 2 && status_out != NULL) {
            *status_out = watch->status;
        }
        memset(watch, 0, sizeof(*watch));
        watch->pid = -1;
        if (pool->watch_count > 0u) {
            pool->watch_count -= 1u;
        }
        pthread_cond_signal(&pool->available);
        break;
    }
    pthread_mutex_unlock(&pool->mutex);
    nomo_async_process_pool_maybe_idle(pool);
    return outcome;
}

static nomo_async_process_runtime *nomo_async_process_runtime_get(
    nomo_async_context *context
) {
    if (context->process_runtime != NULL) {
        return (nomo_async_process_runtime *)context->process_runtime;
    }
    nomo_async_process_runtime *runtime =
        (nomo_async_process_runtime *)calloc(1u, sizeof(*runtime));
    if (runtime == NULL) {
        return NULL;
    }
    runtime->context = context;
    for (uint32_t index = 0u;
         index < NOMO_ASYNC_PROCESS_HANDLE_CAPACITY;
         index += 1u) {
        runtime->handles[index].pid = -1;
        runtime->handles[index].exit_fd = -1;
        runtime->handles[index].stdin_fd = -1;
        runtime->handles[index].stdout_fd = -1;
        runtime->handles[index].stderr_fd = -1;
    }
    context->process_runtime = runtime;
    return runtime;
}

static int nomo_async_process_pool_submit_start(
    nomo_async_process_runtime *runtime,
    nomo_async_process_command_copy *command,
    void *owner,
    nomo_async_process_completion_fn complete,
    uint32_t *slot_out,
    uint32_t *generation_out
) {
    if (nomo_async_process_pool_initialize(runtime) != 0) {
        return 1;
    }
    nomo_async_process_pool *pool = runtime->pool;
    if (nomo_async_process_pool_ensure_wake(pool) != 0
        || pthread_mutex_lock(&pool->mutex) != 0) {
        return 1;
    }
    uint32_t selected = NOMO_ASYNC_PROCESS_JOB_CAPACITY;
    for (uint32_t index = 0u;
         index < NOMO_ASYNC_PROCESS_JOB_CAPACITY;
         index += 1u) {
        if (pool->jobs[index].state == NOMO_ASYNC_PROCESS_JOB_FREE) {
            selected = index;
            break;
        }
    }
    if (selected == NOMO_ASYNC_PROCESS_JOB_CAPACITY
        || pool->active_start_jobs
            >= NOMO_ASYNC_PROCESS_START_JOB_CAPACITY
        || pool->queue_count == NOMO_ASYNC_PROCESS_JOB_CAPACITY) {
        pthread_mutex_unlock(&pool->mutex);
        runtime->context->blocking_queue_saturations += 1u;
        return 2;
    }
    pool->next_generation += 1u;
    if (pool->next_generation == 0u) {
        pool->next_generation = 1u;
    }
    nomo_async_process_job *job = &pool->jobs[selected];
    memset(job, 0, sizeof(*job));
    job->generation = pool->next_generation;
    job->state = NOMO_ASYNC_PROCESS_JOB_START_QUEUED;
    job->command = *command;
    memset(command, 0, sizeof(*command));
    job->pid = -1;
    job->stdin_fd = -1;
    job->stdout_fd = -1;
    job->stderr_fd = -1;
    job->owner = owner;
    job->complete = complete;
    pool->queue[pool->queue_tail] = selected;
    pool->queue_tail =
        (pool->queue_tail + 1u) % NOMO_ASYNC_PROCESS_JOB_CAPACITY;
    pool->queue_count += 1u;
    pool->active_jobs += 1u;
    pool->active_start_jobs += 1u;
    *slot_out = selected;
    *generation_out = job->generation;
    pthread_cond_signal(&pool->available);
    pthread_mutex_unlock(&pool->mutex);
    runtime->context->blocking_jobs_queued += 1u;
    runtime->context->live_blocking_jobs += 1u;
    if (runtime->context->live_blocking_jobs
        > runtime->context->peak_live_blocking_jobs) {
        runtime->context->peak_live_blocking_jobs =
            runtime->context->live_blocking_jobs;
    }
    return 0;
}

static int nomo_async_process_pool_submit_reap(
    nomo_async_process_runtime *runtime,
    pid_t pid,
    uint32_t handle_slot,
    uint32_t handle_generation
) {
    if (pid <= 0 || nomo_async_process_pool_initialize(runtime) != 0) {
        return pid <= 0 ? 0 : 1;
    }
    nomo_async_process_pool *pool = runtime->pool;
    if (nomo_async_process_pool_ensure_wake(pool) != 0
        || pthread_mutex_lock(&pool->mutex) != 0) {
        return 1;
    }
    uint32_t selected = NOMO_ASYNC_PROCESS_JOB_CAPACITY;
    for (uint32_t index = 0u;
         index < NOMO_ASYNC_PROCESS_JOB_CAPACITY;
         index += 1u) {
        if (pool->jobs[index].state == NOMO_ASYNC_PROCESS_JOB_FREE) {
            selected = index;
            break;
        }
    }
    if (selected == NOMO_ASYNC_PROCESS_JOB_CAPACITY
        || pool->queue_count == NOMO_ASYNC_PROCESS_JOB_CAPACITY) {
        pthread_mutex_unlock(&pool->mutex);
        runtime->context->blocking_queue_saturations += 1u;
        return 2;
    }
    pool->next_generation += 1u;
    if (pool->next_generation == 0u) {
        pool->next_generation = 1u;
    }
    nomo_async_process_job *job = &pool->jobs[selected];
    memset(job, 0, sizeof(*job));
    job->generation = pool->next_generation;
    job->state = NOMO_ASYNC_PROCESS_JOB_REAP_QUEUED;
    job->pid = pid;
    job->handle_slot = handle_slot;
    job->handle_generation = handle_generation;
    job->stdin_fd = -1;
    job->stdout_fd = -1;
    job->stderr_fd = -1;
    pool->queue[pool->queue_tail] = selected;
    pool->queue_tail =
        (pool->queue_tail + 1u) % NOMO_ASYNC_PROCESS_JOB_CAPACITY;
    pool->queue_count += 1u;
    pool->active_jobs += 1u;
    pthread_cond_signal(&pool->available);
    pthread_mutex_unlock(&pool->mutex);
    runtime->context->blocking_jobs_queued += 1u;
    runtime->context->live_blocking_jobs += 1u;
    if (runtime->context->live_blocking_jobs
        > runtime->context->peak_live_blocking_jobs) {
        runtime->context->peak_live_blocking_jobs =
            runtime->context->live_blocking_jobs;
    }
    return 0;
}

static int nomo_async_process_pool_take_start(
    nomo_async_process_runtime *runtime,
    uint32_t slot,
    uint32_t generation,
    pid_t *pid,
    int *stdin_fd,
    int *stdout_fd,
    int *stderr_fd,
    int *spawn_errno
) {
    nomo_async_process_pool *pool = runtime->pool;
    if (pool == NULL
        || slot >= NOMO_ASYNC_PROCESS_JOB_CAPACITY
        || pthread_mutex_lock(&pool->mutex) != 0) {
        return 1;
    }
    nomo_async_process_job *job = &pool->jobs[slot];
    if (job->generation != generation
        || job->state != NOMO_ASYNC_PROCESS_JOB_START_DELIVERED) {
        pthread_mutex_unlock(&pool->mutex);
        return 1;
    }
    *pid = job->pid;
    *stdin_fd = job->stdin_fd;
    *stdout_fd = job->stdout_fd;
    *stderr_fd = job->stderr_fd;
    *spawn_errno = job->spawn_errno;
    job->pid = -1;
    job->stdin_fd = -1;
    job->stdout_fd = -1;
    job->stderr_fd = -1;
    nomo_async_process_job_release_locked(pool, job);
    pthread_mutex_unlock(&pool->mutex);
    nomo_async_process_pool_maybe_idle(pool);
    return 0;
}

static void nomo_async_process_pool_cancel_start(
    nomo_async_process_runtime *runtime,
    uint32_t slot,
    uint32_t generation
) {
    nomo_async_process_pool *pool = runtime->pool;
    if (pool == NULL
        || slot >= NOMO_ASYNC_PROCESS_JOB_CAPACITY
        || pthread_mutex_lock(&pool->mutex) != 0) {
        return;
    }
    nomo_async_process_job *job = &pool->jobs[slot];
    if (job->generation != generation) {
        pthread_mutex_unlock(&pool->mutex);
        return;
    }
    uint8_t cancelled = 1u;
    pid_t cleanup_pid = -1;
    int cleanup_stdin = -1;
    int cleanup_stdout = -1;
    int cleanup_stderr = -1;
    if (job->state == NOMO_ASYNC_PROCESS_JOB_START_QUEUED) {
        if (nomo_async_process_remove_queued_locked(pool, slot) == 0) {
            nomo_async_process_job_release_locked(pool, job);
        }
    } else if (job->state == NOMO_ASYNC_PROCESS_JOB_START_RUNNING) {
        job->state = NOMO_ASYNC_PROCESS_JOB_START_CANCELLED;
        job->owner = NULL;
        job->complete = NULL;
    } else if (job->state == NOMO_ASYNC_PROCESS_JOB_START_COMPLETED
        || job->state == NOMO_ASYNC_PROCESS_JOB_START_DELIVERED) {
        uint8_t delivered =
            job->state == NOMO_ASYNC_PROCESS_JOB_START_DELIVERED;
        cleanup_pid = job->pid;
        cleanup_stdin = job->stdin_fd;
        cleanup_stdout = job->stdout_fd;
        cleanup_stderr = job->stderr_fd;
        job->pid = -1;
        job->stdin_fd = -1;
        job->stdout_fd = -1;
        job->stderr_fd = -1;
        job->state = NOMO_ASYNC_PROCESS_JOB_START_CANCELLED;
        job->owner = NULL;
        job->complete = NULL;
        if (delivered != 0u) {
            nomo_async_process_job_release_locked(pool, job);
        }
    } else {
        cancelled = 0u;
    }
    pthread_mutex_unlock(&pool->mutex);
    nomo_async_process_close_fd(&cleanup_stdin);
    nomo_async_process_close_fd(&cleanup_stdout);
    nomo_async_process_close_fd(&cleanup_stderr);
    if (cleanup_pid > 0) {
        (void)nomo_async_process_pool_submit_reap(
            runtime,
            cleanup_pid,
            NOMO_ASYNC_PROCESS_HANDLE_CAPACITY,
            0u
        );
    }
    if (cancelled != 0u) {
        runtime->context->blocking_jobs_cancelled += 1u;
    }
    nomo_async_process_pool_maybe_idle(pool);
}

static void nomo_async_process_retained_add(
    nomo_async_context *context,
    size_t amount
) {
    context->retained_process_bytes += (uint64_t)amount;
    if (context->retained_process_bytes
        > context->peak_retained_process_bytes) {
        context->peak_retained_process_bytes =
            context->retained_process_bytes;
    }
}

static void nomo_async_process_retained_remove(
    nomo_async_context *context,
    size_t amount
) {
    uint64_t removed = (uint64_t)amount;
    context->retained_process_bytes =
        context->retained_process_bytes > removed
        ? context->retained_process_bytes - removed
        : 0u;
}

static int nomo_async_process_buffer_append(
    nomo_async_context *context,
    nomo_async_process_buffer *buffer,
    const char *data,
    size_t length
) {
    if (length == 0u) {
        return 0;
    }
    size_t maximum =
        (size_t)NOMO_ASYNC_PROCESS_MAX_PAYLOAD_BYTES
        + NOMO_ASYNC_PROCESS_BUFFER_SLACK;
    if (buffer->len > maximum - length) {
        return 1;
    }
    size_t needed = buffer->len + length;
    if (needed > buffer->cap) {
        size_t capacity = buffer->cap == 0u ? 4096u : buffer->cap;
        while (capacity < needed) {
            if (capacity > (size_t)NOMO_ASYNC_PROCESS_MAX_PAYLOAD_BYTES / 2u) {
                capacity = maximum;
                break;
            }
            capacity *= 2u;
        }
        char *next = (char *)realloc(buffer->data, capacity);
        if (next == NULL) {
            return 1;
        }
        buffer->data = next;
        buffer->cap = capacity;
    }
    memcpy(buffer->data + buffer->len, data, length);
    buffer->len += length;
    nomo_async_process_retained_add(context, length);
    return 0;
}

static void nomo_async_process_buffer_consume(
    nomo_async_context *context,
    nomo_async_process_buffer *buffer,
    size_t length
) {
    size_t consumed = length < buffer->len ? length : buffer->len;
    if (consumed < buffer->len) {
        memmove(buffer->data, buffer->data + consumed, buffer->len - consumed);
    }
    buffer->len -= consumed;
    nomo_async_process_retained_remove(context, consumed);
}

static void nomo_async_process_buffer_release(
    nomo_async_context *context,
    nomo_async_process_buffer *buffer
) {
    nomo_async_process_retained_remove(context, buffer->len);
    free(buffer->data);
    memset(buffer, 0, sizeof(*buffer));
}

static int nomo_async_process_utf8_width(
    const unsigned char *data,
    size_t length,
    size_t *width
) {
    if (length == 0u) {
        return 0;
    }
    unsigned char first = data[0];
    if (first == 0u) {
        return -1;
    }
    if (first <= 0x7fu) {
        *width = 1u;
        return 1;
    }
    size_t count = 0u;
    uint32_t value = 0u;
    uint32_t minimum = 0u;
    if (first >= 0xc2u && first <= 0xdfu) {
        count = 2u;
        value = (uint32_t)(first & 0x1fu);
        minimum = 0x80u;
    } else if (first >= 0xe0u && first <= 0xefu) {
        count = 3u;
        value = (uint32_t)(first & 0x0fu);
        minimum = 0x800u;
    } else if (first >= 0xf0u && first <= 0xf4u) {
        count = 4u;
        value = (uint32_t)(first & 0x07u);
        minimum = 0x10000u;
    } else {
        return -1;
    }
    if (length < count) {
        return 0;
    }
    for (size_t index = 1u; index < count; index += 1u) {
        if ((data[index] & 0xc0u) != 0x80u) {
            return -1;
        }
        value = (value << 6u) | (uint32_t)(data[index] & 0x3fu);
    }
    if (value < minimum
        || value > 0x10ffffu
        || (value >= 0xd800u && value <= 0xdfffu)) {
        return -1;
    }
    *width = count;
    return 1;
}

static int nomo_async_process_utf8_prefix(
    const char *data,
    size_t length,
    size_t limit,
    int eof,
    size_t *prefix
) {
    size_t index = 0u;
    size_t maximum = length < limit ? length : limit;
    while (index < maximum) {
        if (data[index] == '\0') {
            return 1;
        }
        size_t width = 0u;
        int status = nomo_async_process_utf8_width(
            (const unsigned char *)data + index,
            length - index,
            &width
        );
        if (status < 0) {
            return 1;
        }
        if (status == 0) {
            if (eof != 0) {
                return 1;
            }
            break;
        }
        if (index + width > maximum) {
            break;
        }
        index += width;
    }
    *prefix = index;
    return 0;
}

static void nomo_async_process_handle_storage_release(
    nomo_async_process_runtime *runtime,
    nomo_async_process_handle_state *state
) {
    nomo_async_process_close_fd(&state->exit_fd);
    nomo_async_process_close_fd(&state->stdin_fd);
    nomo_async_process_close_fd(&state->stdout_fd);
    nomo_async_process_close_fd(&state->stderr_fd);
    if (state->stdin_data != NULL) {
        nomo_async_process_retained_remove(
            runtime->context,
            state->stdin_len - state->stdin_offset
        );
    }
    free(state->stdin_data);
    nomo_async_process_buffer_release(runtime->context, &state->stdout_buffer);
    nomo_async_process_buffer_release(runtime->context, &state->stderr_buffer);
    uint32_t generation = state->generation;
    memset(state, 0, sizeof(*state));
    state->generation = generation;
    state->pid = -1;
    state->exit_fd = -1;
    state->stdin_fd = -1;
    state->stdout_fd = -1;
    state->stderr_fd = -1;
}

static void nomo_async_process_reap_complete(
    nomo_async_context *context,
    uint32_t handle_slot,
    uint32_t handle_generation
) {
    if (context == NULL
        || context->process_runtime == NULL
        || handle_slot >= NOMO_ASYNC_PROCESS_HANDLE_CAPACITY) {
        return;
    }
    nomo_async_process_runtime *runtime =
        (nomo_async_process_runtime *)context->process_runtime;
    nomo_async_process_handle_state *state =
        &runtime->handles[handle_slot];
    if (state->occupied == 2u
        && state->reap_pending != 0u
        && state->generation == handle_generation) {
        nomo_async_process_handle_storage_release(runtime, state);
    }
}

static void nomo_async_process_sweep_closing(
    nomo_async_process_runtime *runtime
) {
    for (uint32_t index = 0u;
         index < NOMO_ASYNC_PROCESS_HANDLE_CAPACITY;
         index += 1u) {
        nomo_async_process_handle_state *state = &runtime->handles[index];
        if (state->occupied != 2u
            || state->reap_pending != 0u
            || state->pid <= 0) {
            continue;
        }
        int status = 0;
        pid_t waited;
        do {
            waited = waitpid(state->pid, &status, WNOHANG);
        } while (waited < 0 && errno == EINTR);
        if (waited == state->pid || (waited < 0 && errno == ECHILD)) {
            nomo_async_process_handle_storage_release(runtime, state);
        }
    }
}

static int nomo_async_process_handle_reserve(
    nomo_async_process_runtime *runtime,
    uint32_t *slot_out,
    uint32_t *generation_out
) {
    nomo_async_process_sweep_closing(runtime);
    uint32_t selected = NOMO_ASYNC_PROCESS_HANDLE_CAPACITY;
    for (uint32_t index = 0u;
         index < NOMO_ASYNC_PROCESS_HANDLE_CAPACITY;
         index += 1u) {
        if (runtime->handles[index].occupied == 0u) {
            selected = index;
            break;
        }
    }
    if (selected == NOMO_ASYNC_PROCESS_HANDLE_CAPACITY) {
        return 1;
    }
    runtime->next_handle_generation += 1u;
    if (runtime->next_handle_generation == 0u) {
        runtime->next_handle_generation = 1u;
    }
    nomo_async_process_handle_state *state = &runtime->handles[selected];
    memset(state, 0, sizeof(*state));
    state->generation = runtime->next_handle_generation;
    state->occupied = 1u;
    state->pid = -1;
    state->exit_fd = -1;
    state->stdin_fd = -1;
    state->stdout_fd = -1;
    state->stderr_fd = -1;
    *slot_out = selected;
    *generation_out = state->generation;
    return 0;
}

static nomo_async_process_handle_state *nomo_async_process_handle_find(
    @PROCESS_CHILD@ child,
    nomo_async_process_runtime **runtime_out
) {
    nomo_async_context *context =
        (nomo_async_context *)child.@OWNER_MEMBER@;
    if (context == NULL
        || context->process_runtime == NULL
        || child.@SLOT_MEMBER@ >= NOMO_ASYNC_PROCESS_HANDLE_CAPACITY) {
        return NULL;
    }
    nomo_async_process_runtime *runtime =
        (nomo_async_process_runtime *)context->process_runtime;
    nomo_async_process_handle_state *state =
        &runtime->handles[child.@SLOT_MEMBER@];
    if (state->occupied != 1u
        || state->generation != child.@GENERATION_MEMBER@
        || state->pid <= 0) {
        return NULL;
    }
    *runtime_out = runtime;
    return state;
}

static void nomo_async_process_registration_finish(
    nomo_async_process_registration *registration
) {
    if (registration->context == NULL) {
        return;
    }
    for (uint32_t index = 0u; index < registration->io_count; index += 1u) {
        nomo_async_reactor_deregister(
            &registration->context->reactor,
            &registration->io[index]
        );
    }
    registration->io_count = 0u;
    nomo_async_timer_disarm(
        &registration->timer,
        registration->context
    );
    if (registration->active != 0u) {
        registration->active = 0u;
        if (registration->context->live_process_operations > 0u) {
            registration->context->live_process_operations -= 1u;
        }
    }
}

static void nomo_async_process_start_complete(
    void *owner,
    uint32_t slot,
    uint32_t generation
) {
    nomo_async_process_registration *registration =
        (nomo_async_process_registration *)owner;
    if (registration == NULL
        || registration->kind != NOMO_ASYNC_PROCESS_REGISTRATION_START
        || registration->active == 0u
        || registration->job_slot != slot
        || registration->job_generation != generation) {
        return;
    }
    nomo_async_process_registration_finish(registration);
    registration->ready = 1u;
    if (nomo_async_ready_enqueue(
            registration->context,
            registration->frame,
            registration->poll
        ) != 0) {
        registration->context->runtime_failed = 1u;
    }
}

static void nomo_async_process_release_reserved(
    nomo_async_process_registration *registration
) {
    nomo_async_process_runtime *runtime =
        (nomo_async_process_runtime *)registration->context->process_runtime;
    if (runtime == NULL
        || registration->handle_slot >= NOMO_ASYNC_PROCESS_HANDLE_CAPACITY) {
        return;
    }
    nomo_async_process_handle_state *state =
        &runtime->handles[registration->handle_slot];
    if (state->occupied == 1u
        && state->generation == registration->handle_generation
        && state->pid <= 0) {
        nomo_async_process_handle_storage_release(runtime, state);
    }
}

static nomo_async_poll nomo_async_process_spawn_start(
    nomo_async_process_registration *registration,
    @PROCESS_COMMAND@ command,
    uint64_t timeout_millis,
    nomo_async_context *context,
    @START_RESULT@ *result
) {
    memset(registration, 0, sizeof(*registration));
    context->process_starts += 1u;
    if (timeout_millis == 0u
        || timeout_millis > NOMO_ASYNC_PROCESS_MAX_TIMEOUT_MILLIS) {
        nomo_async_process_start_error(
            result,
            "invalid_request",
            "process start timeout must be in 1..=900000 milliseconds"
        );
        context->process_errors += 1u;
        return NOMO_ASYNC_POLL_READY;
    }
    nomo_async_process_command_copy copied = {0};
    int copied_status = nomo_async_process_copy_command(command, &copied);
    if (copied_status != 0) {
        nomo_async_process_start_error(
            result,
            copied_status == 1 ? "invalid_request" : "limit",
            copied_status == 1
                ? "invalid process command"
                : "process command allocation failed"
        );
        context->process_errors += 1u;
        return NOMO_ASYNC_POLL_READY;
    }
    nomo_async_process_runtime *runtime =
        nomo_async_process_runtime_get(context);
    if (runtime == NULL
        || nomo_async_process_handle_reserve(
            runtime,
            &registration->handle_slot,
            &registration->handle_generation
        ) != 0) {
        nomo_async_process_command_release(&copied);
        nomo_async_process_start_error(
            result,
            "limit",
            "owner executor process handle capacity is exhausted"
        );
        context->process_errors += 1u;
        return NOMO_ASYNC_POLL_READY;
    }
    registration->kind = NOMO_ASYNC_PROCESS_REGISTRATION_START;
    registration->context = context;
    registration->frame = context->current_frame;
    registration->poll = context->current_poll;
    int submitted = nomo_async_process_pool_submit_start(
        runtime,
        &copied,
        registration,
        nomo_async_process_start_complete,
        &registration->job_slot,
        &registration->job_generation
    );
    if (submitted != 0) {
        nomo_async_process_command_release(&copied);
        nomo_async_process_release_reserved(registration);
        nomo_async_process_start_error(
            result,
            submitted == 2 ? "limit" : "reactor",
            submitted == 2
                ? "bounded process start queue is full"
                : "process start worker initialization failed"
        );
        context->process_errors += 1u;
        return NOMO_ASYNC_POLL_READY;
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
        nomo_async_process_pool_cancel_start(
            runtime,
            registration->job_slot,
            registration->job_generation
        );
        nomo_async_process_release_reserved(registration);
        nomo_async_process_start_error(
            result,
            "limit",
            "owner executor timer capacity is exhausted"
        );
        context->process_errors += 1u;
        return NOMO_ASYNC_POLL_READY;
    }
    registration->deadline_millis = registration->timer.deadline_millis;
    registration->active = 1u;
    context->live_process_operations += 1u;
    if (context->live_process_operations
        > context->peak_live_process_operations) {
        context->peak_live_process_operations =
            context->live_process_operations;
    }
    context->pending_reason = NOMO_ASYNC_PENDING_IO;
    return NOMO_ASYNC_POLL_PENDING;
}

static nomo_async_poll nomo_async_process_spawn_resume(
    nomo_async_process_registration *registration,
    nomo_async_context *context,
    @START_RESULT@ *result
) {
    nomo_async_process_runtime *runtime =
        (nomo_async_process_runtime *)context->process_runtime;
    if (registration->timer.expired != 0u) {
        registration->timer.expired = 0u;
        nomo_async_process_registration_finish(registration);
        nomo_async_process_pool_cancel_start(
            runtime,
            registration->job_slot,
            registration->job_generation
        );
        nomo_async_process_release_reserved(registration);
        nomo_async_process_start_error(
            result,
            "timeout",
            "process start timed out"
        );
        context->process_timeouts += 1u;
        context->process_errors += 1u;
        registration->kind = NOMO_ASYNC_PROCESS_REGISTRATION_NONE;
        return NOMO_ASYNC_POLL_READY;
    }
    if (registration->ready == 0u) {
        context->pending_reason = NOMO_ASYNC_PENDING_IO;
        return NOMO_ASYNC_POLL_PENDING;
    }
    pid_t pid = -1;
    int stdin_fd = -1;
    int stdout_fd = -1;
    int stderr_fd = -1;
    int spawn_errno = 0;
    if (runtime == NULL
        || nomo_async_process_pool_take_start(
            runtime,
            registration->job_slot,
            registration->job_generation,
            &pid,
            &stdin_fd,
            &stdout_fd,
            &stderr_fd,
            &spawn_errno
        ) != 0) {
        nomo_async_process_release_reserved(registration);
        nomo_async_process_start_error(
            result,
            "reactor",
            "process start completion was lost"
        );
        context->process_errors += 1u;
        registration->kind = NOMO_ASYNC_PROCESS_REGISTRATION_NONE;
        return NOMO_ASYNC_POLL_READY;
    }
    if (spawn_errno != 0 || pid <= 0) {
        nomo_async_process_release_reserved(registration);
        nomo_async_process_start_error(
            result,
            "spawn",
            "failed to start process"
        );
        context->process_errors += 1u;
        registration->kind = NOMO_ASYNC_PROCESS_REGISTRATION_NONE;
        return NOMO_ASYNC_POLL_READY;
    }
    nomo_async_process_handle_state *state =
        &runtime->handles[registration->handle_slot];
    state->pid = pid;
    state->stdin_fd = stdin_fd;
    state->stdout_fd = stdout_fd;
    state->stderr_fd = stderr_fd;
#if defined(__APPLE__)
    state->exit_source = NOMO_ASYNC_PROCESS_EXIT_SOURCE_REACTOR;
#else
    state->exit_fd = nomo_async_process_open_pidfd(pid);
    if (state->exit_fd >= 0) {
        state->exit_source = NOMO_ASYNC_PROCESS_EXIT_SOURCE_REACTOR;
    } else if (nomo_async_process_pool_watch(
            runtime,
            pid,
            registration->handle_slot,
            registration->handle_generation
        ) == 0) {
        state->exit_source = NOMO_ASYNC_PROCESS_EXIT_SOURCE_WORKER;
    } else {
        nomo_async_process_close_fd(&state->stdin_fd);
        nomo_async_process_close_fd(&state->stdout_fd);
        nomo_async_process_close_fd(&state->stderr_fd);
        if (nomo_async_process_pool_submit_reap(
                runtime,
                pid,
                registration->handle_slot,
                registration->handle_generation
            ) == 0) {
            state->occupied = 2u;
            state->reap_pending = 1u;
        } else {
            if (kill(pid, SIGKILL) != 0 && errno != ESRCH) {
            }
            state->occupied = 2u;
        }
        nomo_async_process_start_error(
            result,
            "limit",
            "bounded process exit watcher capacity is exhausted"
        );
        context->process_errors += 1u;
        registration->kind = NOMO_ASYNC_PROCESS_REGISTRATION_NONE;
        return NOMO_ASYNC_POLL_READY;
    }
#endif
    memset(result, 0, sizeof(*result));
    result->tag = @START_OK@;
    result->payload.@OK_PAYLOAD@ = (@PROCESS_CHILD@){
        .@HANDLE_MEMBER@ =
            ((uint64_t)registration->handle_generation << 32u)
            | (uint64_t)(registration->handle_slot + 1u),
        .@OWNER_MEMBER@ = context,
        .@SLOT_MEMBER@ = registration->handle_slot,
        .@GENERATION_MEMBER@ = registration->handle_generation
    };
    context->process_start_completions += 1u;
    context->live_process_handles += 1u;
    if (context->live_process_handles
        > context->peak_live_process_handles) {
        context->peak_live_process_handles =
            context->live_process_handles;
    }
    registration->kind = NOMO_ASYNC_PROCESS_REGISTRATION_NONE;
    return NOMO_ASYNC_POLL_READY;
}

static void nomo_async_process_update_exit(
    nomo_async_process_handle_state *state
) {
    if (state->exited != 0u
        || state->pid <= 0
        || state->exit_source != NOMO_ASYNC_PROCESS_EXIT_SOURCE_REACTOR) {
        return;
    }
    int status = 0;
    pid_t waited;
    do {
        waited = waitpid(state->pid, &status, WNOHANG);
    } while (waited < 0 && errno == EINTR);
    if (waited != state->pid) {
        return;
    }
    nomo_async_process_set_exit_status(state, status);
}

static int nomo_async_process_read_stream(
    nomo_async_process_runtime *runtime,
    nomo_async_process_handle_state *state,
    int is_stderr,
    size_t target
) {
    int *fd = is_stderr ? &state->stderr_fd : &state->stdout_fd;
    nomo_async_process_buffer *buffer =
        is_stderr ? &state->stderr_buffer : &state->stdout_buffer;
    uint8_t *eof = is_stderr ? &state->stderr_eof : &state->stdout_eof;
    if (*fd < 0 || buffer->len >= target) {
        return 0;
    }
    size_t available = target - buffer->len;
    if (available > 4096u) {
        available = 4096u;
    }
    char chunk[4096];
    ssize_t count;
    do {
        count = read(*fd, chunk, available);
    } while (count < 0 && errno == EINTR);
    if (count > 0) {
        return nomo_async_process_buffer_append(
            runtime->context,
            buffer,
            chunk,
            (size_t)count
        );
    }
    if (count == 0) {
        nomo_async_process_close_fd(fd);
        *eof = 1u;
        return 0;
    }
    if (errno == EAGAIN || errno == EWOULDBLOCK) {
        return 0;
    }
    return 1;
}

static int nomo_async_process_flush_stdin(
    nomo_async_process_runtime *runtime,
    nomo_async_process_handle_state *state
) {
    if (state->stdin_fd < 0 || state->stdin_pending == 0u) {
        return 0;
    }
    ssize_t written;
    do {
        written = write(
            state->stdin_fd,
            state->stdin_data + state->stdin_offset,
            state->stdin_len - state->stdin_offset
        );
    } while (written < 0 && errno == EINTR);
    if (written > 0) {
        size_t amount = (size_t)written;
        state->stdin_offset += amount;
        nomo_async_process_retained_remove(runtime->context, amount);
        if (state->stdin_offset == state->stdin_len) {
            free(state->stdin_data);
            state->stdin_data = NULL;
            state->stdin_len = 0u;
            state->stdin_offset = 0u;
            state->stdin_pending = 0u;
            state->stdin_flushed = 1u;
        }
        return 0;
    }
    if (written < 0 && (errno == EAGAIN || errno == EWOULDBLOCK)) {
        return 0;
    }
    nomo_async_process_retained_remove(
        runtime->context,
        state->stdin_len - state->stdin_offset
    );
    free(state->stdin_data);
    state->stdin_data = NULL;
    state->stdin_len = 0u;
    state->stdin_offset = 0u;
    state->stdin_pending = 0u;
    state->stdin_closed = 1u;
    nomo_async_process_close_fd(&state->stdin_fd);
    return 1;
}

static int nomo_async_process_emit_stream(
    nomo_async_process_runtime *runtime,
    nomo_async_process_handle_state *state,
    int is_stderr,
    uint64_t max_chunk_bytes,
    @EVENT_RESULT@ *result
) {
    nomo_async_process_buffer *buffer =
        is_stderr ? &state->stderr_buffer : &state->stdout_buffer;
    int eof = is_stderr ? state->stderr_eof : state->stdout_eof;
    if (buffer->len == 0u) {
        return 0;
    }
    size_t prefix = 0u;
    if (nomo_async_process_utf8_prefix(
            buffer->data,
            buffer->len,
            (size_t)max_chunk_bytes,
            eof,
            &prefix
        ) != 0) {
        return -1;
    }
    if (prefix == 0u) {
        return 0;
    }
    nomo_string text =
        nomo_string_from_slice(buffer->data, 0u, prefix);
    nomo_async_process_buffer_consume(runtime->context, buffer, prefix);
    @PROCESS_EVENT@ event;
    if (is_stderr != 0) {
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
    memset(result, 0, sizeof(*result));
    result->tag = @EVENT_OK@;
    result->payload.@OK_PAYLOAD@ = event;
    state->prefer_stderr = is_stderr == 0;
    return 1;
}

static void nomo_async_process_cancel(
    nomo_async_process_registration *registration,
    nomo_async_context *context
);

static void nomo_async_process_handle_close(
    nomo_async_process_runtime *runtime,
    nomo_async_process_handle_state *state,
    uint8_t cancel_registration
) {
    if (cancel_registration != 0u
        && state->event_registration != NULL) {
        nomo_async_process_cancel(
            (nomo_async_process_registration *)state->event_registration,
            runtime->context
        );
    }
    nomo_async_process_update_exit(state);
    if (state->exit_source == NOMO_ASYNC_PROCESS_EXIT_SOURCE_WORKER) {
        int status = 0;
        int watch_outcome =
            nomo_async_process_pool_unwatch(runtime, state, &status);
        if (watch_outcome == 2) {
            nomo_async_process_set_exit_status(state, status);
        } else {
            state->exit_source = NOMO_ASYNC_PROCESS_EXIT_SOURCE_NONE;
        }
    }
    pid_t pid = state->pid;
    uint8_t already_reaped = state->exited;
    nomo_async_process_close_fd(&state->exit_fd);
    nomo_async_process_close_fd(&state->stdin_fd);
    nomo_async_process_close_fd(&state->stdout_fd);
    nomo_async_process_close_fd(&state->stderr_fd);
    if (state->stdin_data != NULL) {
        nomo_async_process_retained_remove(
            runtime->context,
            state->stdin_len - state->stdin_offset
        );
    }
    free(state->stdin_data);
    state->stdin_data = NULL;
    state->stdin_len = 0u;
    state->stdin_offset = 0u;
    nomo_async_process_buffer_release(
        runtime->context,
        &state->stdout_buffer
    );
    nomo_async_process_buffer_release(
        runtime->context,
        &state->stderr_buffer
    );
    uint32_t handle_slot =
        (uint32_t)(state - runtime->handles);
    if (already_reaped != 0u) {
        nomo_async_process_handle_storage_release(runtime, state);
    } else if (nomo_async_process_pool_submit_reap(
            runtime,
            pid,
            handle_slot,
            state->generation
        ) == 0) {
        state->occupied = 2u;
        state->reap_pending = 1u;
        state->event_busy = 0u;
        state->event_registration = NULL;
    } else {
        if (kill(pid, SIGKILL) != 0 && errno != ESRCH) {
        }
        state->occupied = 2u;
        state->event_busy = 0u;
        state->event_registration = NULL;
    }
    if (runtime->context->live_process_handles > 0u) {
        runtime->context->live_process_handles -= 1u;
    }
}

static int nomo_async_process_event_progress(
    nomo_async_process_runtime *runtime,
    nomo_async_process_handle_state *state,
    uint64_t max_chunk_bytes,
    @EVENT_RESULT@ *result
) {
    nomo_async_process_update_exit(state);
    if (nomo_async_process_flush_stdin(runtime, state) != 0) {
        nomo_async_process_event_error(
            result,
            "io",
            "process stdin write failed"
        );
        runtime->context->process_errors += 1u;
        return 0;
    }
    size_t target =
        (size_t)max_chunk_bytes + NOMO_ASYNC_PROCESS_BUFFER_SLACK;
    if (nomo_async_process_read_stream(runtime, state, 0, target) != 0
        || nomo_async_process_read_stream(runtime, state, 1, target) != 0) {
        nomo_async_process_event_error(
            result,
            "io",
            "process output read failed"
        );
        runtime->context->process_errors += 1u;
        return 0;
    }
    nomo_async_process_update_exit(state);
    if (state->stdin_flushed != 0u) {
        state->stdin_flushed = 0u;
        memset(result, 0, sizeof(*result));
        result->tag = @EVENT_OK@;
        result->payload.@OK_PAYLOAD@ = (@PROCESS_EVENT@){
            .tag = @EVENT_STDIN_FLUSHED@
        };
        return 0;
    }
    int first_stderr = state->prefer_stderr != 0u;
    int emitted = nomo_async_process_emit_stream(
        runtime,
        state,
        first_stderr,
        max_chunk_bytes,
        result
    );
    if (emitted == 0) {
        emitted = nomo_async_process_emit_stream(
            runtime,
            state,
            !first_stderr,
            max_chunk_bytes,
            result
        );
    }
    if (emitted < 0) {
        nomo_async_process_event_error(
            result,
            "protocol",
            "process output is not valid supported text"
        );
        runtime->context->process_errors += 1u;
        nomo_async_process_handle_close(runtime, state, 0u);
        return 0;
    }
    if (emitted > 0) {
        return 0;
    }
    if (state->exited != 0u
        && state->stdout_eof != 0u
        && state->stderr_eof != 0u
        && state->stdout_buffer.len == 0u
        && state->stderr_buffer.len == 0u) {
        state->exit_emitted = 1u;
        memset(result, 0, sizeof(*result));
        result->tag = @EVENT_OK@;
        result->payload.@OK_PAYLOAD@ = (@PROCESS_EVENT@){
            .tag = @EVENT_EXITED@,
            .payload.@EXITED_PAYLOAD@ = state->exit_info
        };
        return 0;
    }
    return 1;
}

static void nomo_async_process_event_release(
    nomo_async_process_registration *registration
) {
    nomo_async_process_runtime *runtime =
        registration->context == NULL
        ? NULL
        : (nomo_async_process_runtime *)
            registration->context->process_runtime;
    if (runtime == NULL
        || registration->handle_slot >= NOMO_ASYNC_PROCESS_HANDLE_CAPACITY) {
        return;
    }
    nomo_async_process_handle_state *state =
        &runtime->handles[registration->handle_slot];
    if (state->occupied == 1u
        && state->generation == registration->handle_generation
        && state->event_registration == registration) {
        state->event_busy = 0u;
        state->event_registration = NULL;
    }
}

static void nomo_async_process_event_wake(void *owner, uint32_t ready) {
    nomo_async_process_registration *registration =
        (nomo_async_process_registration *)owner;
    if (registration == NULL
        || registration->kind != NOMO_ASYNC_PROCESS_REGISTRATION_EVENT
        || registration->active == 0u
        || ready == 0u) {
        return;
    }
    nomo_async_process_registration_finish(registration);
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

static int nomo_async_process_event_arm(
    nomo_async_process_registration *registration,
    nomo_async_process_handle_state *state,
    int64_t timeout_millis
) {
    registration->io_count = 0u;
    int exit_handle = -1;
    if (state->exited == 0u
        && state->exit_source
            == NOMO_ASYNC_PROCESS_EXIT_SOURCE_REACTOR) {
#if defined(__APPLE__)
        exit_handle = (int)state->pid;
#else
        exit_handle = state->exit_fd;
#endif
    }
    int handles[4] = {
        state->stdout_fd,
        state->stderr_fd,
        state->stdin_pending != 0u ? state->stdin_fd : -1,
        exit_handle
    };
    uint32_t interests[4] = {
        NOMO_ASYNC_REACTOR_READ,
        NOMO_ASYNC_REACTOR_READ,
        NOMO_ASYNC_REACTOR_WRITE,
        NOMO_ASYNC_REACTOR_PROCESS
    };
    for (uint32_t index = 0u; index < 4u; index += 1u) {
        if (handles[index] < 0) {
            continue;
        }
        nomo_async_reactor_registration *io =
            &registration->io[registration->io_count];
        memset(io, 0, sizeof(*io));
        io->owner = registration;
        io->wake = nomo_async_process_event_wake;
        int register_status = nomo_async_reactor_register(
            &registration->context->reactor,
            io,
            handles[index],
            interests[index]
        );
        if (register_status != 0) {
            if (interests[index] == NOMO_ASYNC_REACTOR_PROCESS
                && register_status == 2) {
                nomo_async_process_update_exit(state);
                if (state->exited != 0u) {
                    continue;
                }
                nomo_async_process_runtime *runtime =
                    (nomo_async_process_runtime *)
                        registration->context->process_runtime;
                if (runtime != NULL
                    && nomo_async_process_pool_watch(
                        runtime,
                        state->pid,
                        registration->handle_slot,
                        registration->handle_generation
                    ) == 0) {
                    state->exit_source =
                        NOMO_ASYNC_PROCESS_EXIT_SOURCE_WORKER;
                    continue;
                }
            }
            nomo_async_process_registration_finish(registration);
            return 1;
        }
        registration->io_count += 1u;
    }
    /*
     * A Darwin child can exit after the WNOHANG probe in event_progress but
     * before EVFILT_PROC is installed. kqueue accepts that registration for a
     * zombie without necessarily delivering the already-completed edge. Probe
     * once more after registration so the caller can consume buffered output
     * and the exit event immediately instead of waiting for the timeout.
     *
     * The same check is harmless for pidfd-backed Linux and also closes the
     * equivalent registration race there.
     */
    if (exit_handle >= 0) {
        nomo_async_process_update_exit(state);
        if (state->exited != 0u) {
            nomo_async_process_registration_finish(registration);
            return 3;
        }
    }
    nomo_async_poll timer_status = nomo_async_timer_start(
        &registration->timer,
        timeout_millis,
        registration->context,
        &registration->timer_outcome,
        NULL,
        0u
    );
    if (timer_status != NOMO_ASYNC_POLL_PENDING) {
        nomo_async_process_registration_finish(registration);
        return 2;
    }
    registration->ready = 0u;
    registration->active = 1u;
    registration->context->live_process_operations += 1u;
    if (registration->context->live_process_operations
        > registration->context->peak_live_process_operations) {
        registration->context->peak_live_process_operations =
            registration->context->live_process_operations;
    }
    registration->context->pending_reason = NOMO_ASYNC_PENDING_IO;
    return 0;
}

static nomo_async_poll nomo_async_process_event_start(
    nomo_async_process_registration *registration,
    @PROCESS_CHILD@ child,
    uint64_t max_chunk_bytes,
    uint64_t timeout_millis,
    nomo_async_context *context,
    @EVENT_RESULT@ *result
) {
    memset(registration, 0, sizeof(*registration));
    context->process_events += 1u;
    if (max_chunk_bytes < 4u
        || max_chunk_bytes > NOMO_ASYNC_PROCESS_MAX_PAYLOAD_BYTES
        || timeout_millis == 0u
        || timeout_millis > NOMO_ASYNC_PROCESS_MAX_TIMEOUT_MILLIS) {
        nomo_async_process_event_error(
            result,
            "invalid_request",
            "invalid process event limit or timeout"
        );
        context->process_errors += 1u;
        return NOMO_ASYNC_POLL_READY;
    }
    nomo_async_process_runtime *runtime = NULL;
    nomo_async_process_handle_state *state =
        nomo_async_process_handle_find(child, &runtime);
    if (state == NULL) {
        nomo_async_process_event_error(
            result,
            "closed",
            "process child is closed"
        );
        context->process_errors += 1u;
        return NOMO_ASYNC_POLL_READY;
    }
    if (state->exit_emitted != 0u) {
        nomo_async_process_event_error(
            result,
            "invalid_request",
            "process exit event was already consumed"
        );
        context->process_errors += 1u;
        return NOMO_ASYNC_POLL_READY;
    }
    if (state->event_busy != 0u) {
        nomo_async_process_event_error(
            result,
            "busy",
            "process child already has a pending event operation"
        );
        context->process_errors += 1u;
        return NOMO_ASYNC_POLL_READY;
    }
    registration->kind = NOMO_ASYNC_PROCESS_REGISTRATION_EVENT;
    registration->context = context;
    registration->frame = context->current_frame;
    registration->poll = context->current_poll;
    registration->handle_slot = child.@SLOT_MEMBER@;
    registration->handle_generation = child.@GENERATION_MEMBER@;
    registration->max_chunk_bytes = max_chunk_bytes;
    int64_t now = nomo_time_monotonic_millis();
    registration->deadline_millis =
        timeout_millis > (uint64_t)(INT64_MAX - now)
        ? INT64_MAX
        : now + (int64_t)timeout_millis;
    state->event_busy = 1u;
    state->event_registration = registration;
    if (nomo_async_process_event_progress(
            runtime,
            state,
            max_chunk_bytes,
            result
        ) == 0) {
        nomo_async_process_event_release(registration);
        registration->kind = NOMO_ASYNC_PROCESS_REGISTRATION_NONE;
        return NOMO_ASYNC_POLL_READY;
    }
    int armed = nomo_async_process_event_arm(
        registration,
        state,
        (int64_t)timeout_millis
    );
    if (armed == 3) {
        if (nomo_async_process_event_progress(
                runtime,
                state,
                max_chunk_bytes,
                result
            ) == 0) {
            nomo_async_process_event_release(registration);
            registration->kind = NOMO_ASYNC_PROCESS_REGISTRATION_NONE;
            return NOMO_ASYNC_POLL_READY;
        }
        armed = nomo_async_process_event_arm(
            registration,
            state,
            (int64_t)timeout_millis
        );
    }
    if (armed != 0) {
        nomo_async_process_event_release(registration);
        nomo_async_process_event_error(
            result,
            armed == 1 ? "reactor" : "limit",
            armed == 1
                ? "process pipe reactor registration failed"
                : "owner executor timer capacity is exhausted"
        );
        context->process_errors += 1u;
        registration->kind = NOMO_ASYNC_PROCESS_REGISTRATION_NONE;
        return NOMO_ASYNC_POLL_READY;
    }
    return NOMO_ASYNC_POLL_PENDING;
}

static nomo_async_poll nomo_async_process_event_resume(
    nomo_async_process_registration *registration,
    nomo_async_context *context,
    @EVENT_RESULT@ *result
) {
    nomo_async_process_runtime *runtime =
        (nomo_async_process_runtime *)context->process_runtime;
    if (runtime == NULL
        || registration->handle_slot >= NOMO_ASYNC_PROCESS_HANDLE_CAPACITY) {
        nomo_async_process_registration_finish(registration);
        nomo_async_process_event_release(registration);
        nomo_async_process_event_error(
            result,
            "closed",
            "process child is closed"
        );
        context->process_errors += 1u;
        registration->kind = NOMO_ASYNC_PROCESS_REGISTRATION_NONE;
        return NOMO_ASYNC_POLL_READY;
    }
    nomo_async_process_handle_state *state =
        &runtime->handles[registration->handle_slot];
    if (registration->timer.expired != 0u) {
        registration->timer.expired = 0u;
        nomo_async_process_registration_finish(registration);
        int64_t now = nomo_time_monotonic_millis();
        if (now >= registration->deadline_millis) {
            if (state->occupied == 1u
                && state->generation == registration->handle_generation
                && nomo_async_process_event_progress(
                    runtime,
                    state,
                    registration->max_chunk_bytes,
                    result
                ) == 0) {
                nomo_async_process_event_release(registration);
                registration->kind = NOMO_ASYNC_PROCESS_REGISTRATION_NONE;
                return NOMO_ASYNC_POLL_READY;
            }
            nomo_async_process_event_release(registration);
            nomo_async_process_event_error(
                result,
                "timeout",
                "process event timed out"
            );
            context->process_timeouts += 1u;
            context->process_errors += 1u;
            registration->kind = NOMO_ASYNC_PROCESS_REGISTRATION_NONE;
            return NOMO_ASYNC_POLL_READY;
        }
        registration->ready = 1u;
    }
    if (registration->ready == 0u) {
        context->pending_reason = NOMO_ASYNC_PENDING_IO;
        return NOMO_ASYNC_POLL_PENDING;
    }
    if (state->occupied != 1u
        || state->generation != registration->handle_generation) {
        nomo_async_process_event_release(registration);
        nomo_async_process_event_error(
            result,
            "closed",
            "process child is closed"
        );
        context->process_errors += 1u;
        registration->kind = NOMO_ASYNC_PROCESS_REGISTRATION_NONE;
        return NOMO_ASYNC_POLL_READY;
    }
    if (nomo_async_process_event_progress(
            runtime,
            state,
            registration->max_chunk_bytes,
            result
        ) == 0) {
        nomo_async_process_event_release(registration);
        registration->kind = NOMO_ASYNC_PROCESS_REGISTRATION_NONE;
        return NOMO_ASYNC_POLL_READY;
    }
    int64_t now = nomo_time_monotonic_millis();
    int64_t remaining = registration->deadline_millis - now;
    if (remaining <= 0) {
        nomo_async_process_event_release(registration);
        nomo_async_process_event_error(
            result,
            "timeout",
            "process event timed out"
        );
        context->process_timeouts += 1u;
        context->process_errors += 1u;
        registration->kind = NOMO_ASYNC_PROCESS_REGISTRATION_NONE;
        return NOMO_ASYNC_POLL_READY;
    }
    int armed = nomo_async_process_event_arm(
        registration,
        state,
        remaining
    );
    if (armed == 3) {
        if (nomo_async_process_event_progress(
                runtime,
                state,
                registration->max_chunk_bytes,
                result
            ) == 0) {
            nomo_async_process_event_release(registration);
            registration->kind = NOMO_ASYNC_PROCESS_REGISTRATION_NONE;
            return NOMO_ASYNC_POLL_READY;
        }
        armed = nomo_async_process_event_arm(
            registration,
            state,
            remaining
        );
    }
    if (armed != 0) {
        nomo_async_process_event_release(registration);
        nomo_async_process_event_error(
            result,
            armed == 1 ? "reactor" : "limit",
            armed == 1
                ? "process pipe reactor re-registration failed"
                : "owner executor timer capacity is exhausted"
        );
        context->process_errors += 1u;
        registration->kind = NOMO_ASYNC_PROCESS_REGISTRATION_NONE;
        return NOMO_ASYNC_POLL_READY;
    }
    return NOMO_ASYNC_POLL_PENDING;
}

static void nomo_async_process_cancel(
    nomo_async_process_registration *registration,
    nomo_async_context *context
) {
    if (registration == NULL
        || registration->context == NULL
        || registration->kind == NOMO_ASYNC_PROCESS_REGISTRATION_NONE) {
        return;
    }
    nomo_async_process_runtime *runtime =
        (nomo_async_process_runtime *)context->process_runtime;
    if (registration->kind == NOMO_ASYNC_PROCESS_REGISTRATION_START) {
        nomo_async_process_registration_finish(registration);
        if (runtime != NULL) {
            nomo_async_process_pool_cancel_start(
                runtime,
                registration->job_slot,
                registration->job_generation
            );
        }
        nomo_async_process_release_reserved(registration);
    } else {
        nomo_async_process_registration_finish(registration);
        nomo_async_process_event_release(registration);
    }
    registration->kind = NOMO_ASYNC_PROCESS_REGISTRATION_NONE;
    context->process_cancellations += 1u;
}

static @VOID_RESULT@ @WRITE_STDIN_NAME@(
    @PROCESS_CHILD@ child,
    nomo_string data
) {
    nomo_async_process_runtime *runtime = NULL;
    nomo_async_process_handle_state *state =
        nomo_async_process_handle_find(child, &runtime);
    if (state == NULL) {
        @VOID_RESULT@ result;
        nomo_async_process_void_error(
            &result,
            "closed",
            "process child is closed"
        );
        return result;
    }
    size_t length = strlen(data.data);
    if (length == 0u
        || length > NOMO_ASYNC_PROCESS_MAX_PAYLOAD_BYTES) {
        @VOID_RESULT@ result;
        nomo_async_process_void_error(
            &result,
            "invalid_request",
            "invalid process stdin payload"
        );
        runtime->context->process_errors += 1u;
        return result;
    }
    nomo_async_process_update_exit(state);
    if (state->stdin_closed != 0u
        || state->stdin_fd < 0
        || state->exited != 0u) {
        @VOID_RESULT@ result;
        nomo_async_process_void_error(
            &result,
            "closed",
            "process stdin is closed"
        );
        runtime->context->process_errors += 1u;
        return result;
    }
    if (state->stdin_pending != 0u) {
        @VOID_RESULT@ result;
        nomo_async_process_void_error(
            &result,
            "busy",
            "process stdin already has pending data"
        );
        runtime->context->process_errors += 1u;
        return result;
    }
    state->stdin_data = (char *)malloc(length);
    if (state->stdin_data == NULL) {
        @VOID_RESULT@ result;
        nomo_async_process_void_error(
            &result,
            "limit",
            "process stdin allocation failed"
        );
        runtime->context->process_errors += 1u;
        return result;
    }
    memcpy(state->stdin_data, data.data, length);
    state->stdin_len = length;
    state->stdin_offset = 0u;
    state->stdin_pending = 1u;
    state->stdin_flushed = 0u;
    nomo_async_process_retained_add(runtime->context, length);
    runtime->context->process_stdin_writes += 1u;
    return nomo_async_process_void_ok();
}

static @VOID_RESULT@ @CLOSE_STDIN_NAME@(@PROCESS_CHILD@ child) {
    nomo_async_process_runtime *runtime = NULL;
    nomo_async_process_handle_state *state =
        nomo_async_process_handle_find(child, &runtime);
    if (state == NULL) {
        @VOID_RESULT@ result;
        nomo_async_process_void_error(
            &result,
            "closed",
            "process child is closed"
        );
        return result;
    }
    if (state->stdin_pending != 0u) {
        @VOID_RESULT@ result;
        nomo_async_process_void_error(
            &result,
            "busy",
            "process stdin still has pending data"
        );
        runtime->context->process_errors += 1u;
        return result;
    }
    if (state->stdin_closed == 0u) {
        state->stdin_closed = 1u;
        nomo_async_process_close_fd(&state->stdin_fd);
    }
    return nomo_async_process_void_ok();
}

static @WAIT_RESULT@ @TRY_WAIT_NAME@(@PROCESS_CHILD@ child) {
    nomo_async_process_runtime *runtime = NULL;
    nomo_async_process_handle_state *state =
        nomo_async_process_handle_find(child, &runtime);
    if (state == NULL) {
        return nomo_async_process_wait_error(
            "closed",
            "process child is closed"
        );
    }
    nomo_async_process_update_exit(state);
    @EXIT_OPTION@ option;
    if (state->exited != 0u) {
        option = (@EXIT_OPTION@){
            .tag = @EXIT_SOME@,
            .payload.@SOME_PAYLOAD@ = state->exit_info
        };
    } else {
        option = (@EXIT_OPTION@){.tag = @EXIT_NONE@};
    }
    return (@WAIT_RESULT@){
        .tag = @WAIT_OK@,
        .payload.@OK_PAYLOAD@ = option
    };
}

static @VOID_RESULT@ @TERMINATE_NAME@(@PROCESS_CHILD@ child) {
    nomo_async_process_runtime *runtime = NULL;
    nomo_async_process_handle_state *state =
        nomo_async_process_handle_find(child, &runtime);
    if (state == NULL) {
        @VOID_RESULT@ result;
        nomo_async_process_void_error(
            &result,
            "closed",
            "process child is closed"
        );
        return result;
    }
    nomo_async_process_update_exit(state);
    if (state->exited == 0u
        && kill(state->pid, SIGKILL) != 0
        && errno != ESRCH) {
        @VOID_RESULT@ result;
        nomo_async_process_void_error(
            &result,
            "io",
            "process termination failed"
        );
        runtime->context->process_errors += 1u;
        return result;
    }
    runtime->context->process_terminations += 1u;
    return nomo_async_process_void_ok();
}

static void @CLOSE_CHILD_NAME@(@PROCESS_CHILD@ child) {
    nomo_async_process_runtime *runtime = NULL;
    nomo_async_process_handle_state *state =
        nomo_async_process_handle_find(child, &runtime);
    if (state == NULL) {
        return;
    }
    nomo_async_process_handle_close(runtime, state, 1u);
}

static void nomo_async_process_runtime_shutdown(nomo_async_context *context) {
    nomo_async_process_runtime *runtime =
        (nomo_async_process_runtime *)context->process_runtime;
    if (runtime == NULL) {
        return;
    }
    nomo_async_process_pool *pool = runtime->pool;
    for (uint32_t index = 0u;
         index < NOMO_ASYNC_PROCESS_HANDLE_CAPACITY;
         index += 1u) {
        nomo_async_process_handle_state *state = &runtime->handles[index];
        if (state->occupied == 0u) {
            continue;
        }
        if (state->event_registration != NULL) {
            nomo_async_process_cancel(
                (nomo_async_process_registration *)
                    state->event_registration,
                context
            );
        }
        if (state->exit_source == NOMO_ASYNC_PROCESS_EXIT_SOURCE_WORKER) {
            int status = 0;
            int watch_outcome =
                nomo_async_process_pool_unwatch(runtime, state, &status);
            if (watch_outcome == 2) {
                nomo_async_process_set_exit_status(state, status);
            } else {
                state->exit_source = NOMO_ASYNC_PROCESS_EXIT_SOURCE_NONE;
            }
        }
        nomo_async_process_close_fd(&state->exit_fd);
    }
    if (pool != NULL) {
        if (pthread_mutex_lock(&pool->mutex) == 0) {
            pool->stopping = 1u;
            pthread_cond_broadcast(&pool->available);
            pthread_mutex_unlock(&pool->mutex);
        }
        pthread_join(pool->worker, NULL);
    }
    for (uint32_t index = 0u;
         index < NOMO_ASYNC_PROCESS_HANDLE_CAPACITY;
         index += 1u) {
        nomo_async_process_handle_state *state = &runtime->handles[index];
        if (state->occupied == 0u) {
            continue;
        }
        nomo_async_process_close_fd(&state->stdin_fd);
        nomo_async_process_close_fd(&state->stdout_fd);
        nomo_async_process_close_fd(&state->stderr_fd);
        if (state->pid > 0
            && state->exited == 0u
            && state->reap_pending == 0u) {
            if (kill(state->pid, SIGKILL) != 0 && errno != ESRCH) {
            }
            int status = 0;
            while (waitpid(state->pid, &status, 0) < 0 && errno == EINTR) {
            }
        }
        nomo_async_process_handle_storage_release(runtime, state);
    }
    if (pool != NULL) {
        nomo_async_reactor_deregister(
            &context->reactor,
            &pool->wake_registration
        );
        for (uint32_t index = 0u;
             index < NOMO_ASYNC_PROCESS_JOB_CAPACITY;
             index += 1u) {
            nomo_async_process_job *job = &pool->jobs[index];
            if (job->pid > 0) {
                nomo_async_process_spawn_cleanup(
                    job->pid,
                    &job->stdin_fd,
                    &job->stdout_fd,
                    &job->stderr_fd
                );
            }
            nomo_async_process_command_release(&job->command);
        }
        close(pool->wake_read);
        close(pool->wake_write);
        pthread_cond_destroy(&pool->available);
        pthread_mutex_destroy(&pool->mutex);
        context->blocking_threads_retired += 1u;
        if (context->live_blocking_threads > 0u) {
            context->live_blocking_threads -= 1u;
        }
        context->live_blocking_jobs = 0u;
        free(pool);
    }
    context->live_process_handles = 0u;
    context->live_process_operations = 0u;
    context->retained_process_bytes = 0u;
    free(runtime);
    context->process_runtime = NULL;
}
