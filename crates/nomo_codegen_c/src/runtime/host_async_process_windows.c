#define NOMO_ASYNC_PROCESS_MAX_PAYLOAD_BYTES (UINT64_C(1024) * UINT64_C(1024))
#define NOMO_ASYNC_PROCESS_MAX_TIMEOUT_MILLIS (UINT64_C(15) * UINT64_C(60) * UINT64_C(1000))
#define NOMO_ASYNC_PROCESS_HANDLE_CAPACITY 16u
#define NOMO_ASYNC_PROCESS_JOB_CAPACITY 32u
#define NOMO_ASYNC_PROCESS_START_CAPACITY 16u
#define NOMO_ASYNC_PROCESS_BUFFER_SLACK 4u
#define NOMO_ASYNC_PROCESS_READ_CHUNK 4096u

typedef struct {
    char *data;
    size_t len;
    size_t cap;
} nomo_async_process_buffer;

typedef struct {
    char *program;
    char **args;
    size_t argc;
    char *cwd;
    char **env_names;
    char **env_values;
    size_t envc;
    uint8_t inherit_env;
} nomo_async_process_command_copy;

typedef enum {
    NOMO_ASYNC_PROCESS_JOB_FREE = 0,
    NOMO_ASYNC_PROCESS_JOB_QUEUED = 1,
    NOMO_ASYNC_PROCESS_JOB_RUNNING = 2,
    NOMO_ASYNC_PROCESS_JOB_COMPLETED = 3,
    NOMO_ASYNC_PROCESS_JOB_CANCELLED = 4
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
    HANDLE process;
    HANDLE stdin_write;
    HANDLE stdout_read;
    HANDLE stderr_read;
    DWORD spawn_error;
    void *owner;
    nomo_async_process_completion_fn complete;
    uint8_t notified;
} nomo_async_process_job;

typedef struct nomo_async_process_handle_state
    nomo_async_process_handle_state;

typedef struct {
    nomo_async_process_handle_state *state;
    uint32_t generation;
} nomo_async_process_wait_context;

typedef struct {
    nomo_async_context *context;
    CRITICAL_SECTION lock;
    CONDITION_VARIABLE available;
    HANDLE worker;
    nomo_async_reactor_registration completion_registration;
    nomo_async_process_job jobs[NOMO_ASYNC_PROCESS_JOB_CAPACITY];
    uint32_t queue[NOMO_ASYNC_PROCESS_JOB_CAPACITY];
    uint32_t queue_head;
    uint32_t queue_tail;
    uint32_t queue_count;
    uint32_t next_generation;
    uint32_t active_jobs;
    uint32_t active_start_jobs;
    uint8_t stopping;
} nomo_async_process_pool;

struct nomo_async_process_handle_state {
    nomo_async_context *context;
    uint32_t generation;
    uint8_t occupied;
    uint8_t closing;
    uint8_t stdin_closed;
    uint8_t stdin_pending;
    uint8_t stdin_flushed;
    uint8_t stdout_eof;
    uint8_t stderr_eof;
    uint8_t exited;
    uint8_t exit_emitted;
    uint8_t prefer_stderr;
    uint8_t event_busy;
    uint8_t stdin_error;
    HANDLE process;
    HANDLE wait_handle;
    HANDLE stdin_write;
    HANDLE stdout_read;
    HANDLE stderr_read;
    char *stdin_data;
    size_t stdin_len;
    size_t stdin_offset;
    nomo_async_reactor_registration stdin_registration;
    nomo_async_reactor_registration exit_registration;
    nomo_async_process_buffer stdout_buffer;
    nomo_async_process_buffer stderr_buffer;
    @PROCESS_EXIT@ exit_info;
    void *event_registration;
    nomo_async_process_wait_context wait_context;
};

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
    nomo_async_reactor_registration io[2];
    char *read_buffers[2];
    uint32_t job_slot;
    uint32_t job_generation;
    uint32_t handle_slot;
    uint32_t handle_generation;
    uint64_t max_chunk_bytes;
    int64_t deadline_millis;
    uint8_t active;
    uint8_t ready;
} nomo_async_process_registration;

typedef struct {
    wchar_t **items;
    size_t len;
    size_t cap;
} nomo_async_process_wide_env;

typedef struct {
    wchar_t *data;
    size_t len;
    size_t cap;
} nomo_async_process_wide_text;

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

static @PROCESS_EXIT@ nomo_async_process_exit_value(DWORD code) {
    return (@PROCESS_EXIT@){
        .@CODE_MEMBER@ = (int32_t)code,
        .@SIGNAL_MEMBER@ = 0
    };
}

static void nomo_async_process_close_handle(HANDLE *handle) {
    if (*handle != NULL && *handle != INVALID_HANDLE_VALUE) {
        CloseHandle(*handle);
    }
    *handle = NULL;
}

static char *nomo_async_process_copy_cstr(const char *value) {
    size_t length = strlen(value);
    char *copy = (char *)malloc(length + 1u);
    if (copy != NULL) {
        memcpy(copy, value, length + 1u);
    }
    return copy;
}

static void nomo_async_process_command_release(
    nomo_async_process_command_copy *command
) {
    free(command->program);
    free(command->cwd);
    if (command->args != NULL) {
        for (size_t index = 0u; index < command->argc; index += 1u) {
            free(command->args[index]);
        }
    }
    if (command->env_names != NULL) {
        for (size_t index = 0u; index < command->envc; index += 1u) {
            free(command->env_names[index]);
            free(command->env_values[index]);
        }
    }
    free(command->args);
    free(command->env_names);
    free(command->env_values);
    memset(command, 0, sizeof(*command));
}

static int nomo_async_process_command_text_valid(nomo_string value) {
    return value.data != NULL
        && strlen(value.data) <= NOMO_ASYNC_PROCESS_MAX_PAYLOAD_BYTES;
}

static int nomo_async_process_copy_command(
    @PROCESS_COMMAND@ source,
    nomo_async_process_command_copy *target
) {
    memset(target, 0, sizeof(*target));
    if (!nomo_async_process_command_text_valid(source.@PROGRAM_MEMBER@)
        || source.@PROGRAM_MEMBER@.data[0] == '\0'
        || source.@ARGS_MEMBER@.len > 4096u
        || source.@ENV_MEMBER@.len > 4096u) {
        return 1;
    }
    size_t retained = strlen(source.@PROGRAM_MEMBER@.data) + 1u;
    for (size_t index = 0u; index < source.@ARGS_MEMBER@.len; index += 1u) {
        if (!nomo_async_process_command_text_valid(
                source.@ARGS_MEMBER@.data[index]
            )) {
            return 1;
        }
        retained += strlen(source.@ARGS_MEMBER@.data[index].data) + 1u;
    }
    for (size_t index = 0u; index < source.@ENV_MEMBER@.len; index += 1u) {
        @PROCESS_ENV@ item = source.@ENV_MEMBER@.data[index];
        if (!nomo_async_process_command_text_valid(item.@NAME_MEMBER@)
            || !nomo_async_process_command_text_valid(
                item.@VALUE_MEMBER@
            )) {
            return 1;
        }
        retained += strlen(item.@NAME_MEMBER@.data)
            + strlen(item.@VALUE_MEMBER@.data) + 2u;
    }
    if (source.@CWD_MEMBER@.tag == @CWD_SOME@) {
        if (!nomo_async_process_command_text_valid(
                source.@CWD_MEMBER@.payload.@SOME_PAYLOAD@
            )) {
            return 1;
        }
        retained += strlen(
            source.@CWD_MEMBER@.payload.@SOME_PAYLOAD@.data
        ) + 1u;
    }
    if (retained > NOMO_ASYNC_PROCESS_MAX_PAYLOAD_BYTES) {
        return 1;
    }
    target->program =
        nomo_async_process_copy_cstr(source.@PROGRAM_MEMBER@.data);
    target->argc = source.@ARGS_MEMBER@.len;
    target->envc = source.@ENV_MEMBER@.len;
    target->inherit_env = source.@INHERIT_ENV_MEMBER@ != 0u;
    if (target->program == NULL) {
        return 2;
    }
    if (source.@CWD_MEMBER@.tag == @CWD_SOME@) {
        nomo_string cwd =
            source.@CWD_MEMBER@.payload.@SOME_PAYLOAD@;
        if (!nomo_async_process_command_text_valid(cwd)
            || cwd.data[0] == '\0') {
            nomo_async_process_command_release(target);
            return 1;
        }
        target->cwd = nomo_async_process_copy_cstr(cwd.data);
        if (target->cwd == NULL) {
            nomo_async_process_command_release(target);
            return 2;
        }
    }
    if (target->argc > 0u) {
        target->args = (char **)calloc(target->argc, sizeof(char *));
        if (target->args == NULL) {
            nomo_async_process_command_release(target);
            return 2;
        }
    }
    for (size_t index = 0u; index < target->argc; index += 1u) {
        nomo_string item = source.@ARGS_MEMBER@.data[index];
        if (!nomo_async_process_command_text_valid(item)) {
            nomo_async_process_command_release(target);
            return 1;
        }
        target->args[index] = nomo_async_process_copy_cstr(item.data);
        if (target->args[index] == NULL) {
            nomo_async_process_command_release(target);
            return 2;
        }
    }
    if (target->envc > 0u) {
        target->env_names =
            (char **)calloc(target->envc, sizeof(char *));
        target->env_values =
            (char **)calloc(target->envc, sizeof(char *));
        if (target->env_names == NULL || target->env_values == NULL) {
            nomo_async_process_command_release(target);
            return 2;
        }
    }
    for (size_t index = 0u; index < target->envc; index += 1u) {
        @PROCESS_ENV@ item = source.@ENV_MEMBER@.data[index];
        if (!nomo_async_process_command_text_valid(item.@NAME_MEMBER@)
            || item.@NAME_MEMBER@.data[0] == '\0'
            || strchr(item.@NAME_MEMBER@.data, '=') != NULL
            || !nomo_async_process_command_text_valid(item.@VALUE_MEMBER@)) {
            nomo_async_process_command_release(target);
            return 1;
        }
        for (size_t previous = 0u; previous < index; previous += 1u) {
            if (_stricmp(
                    target->env_names[previous],
                    item.@NAME_MEMBER@.data
                ) == 0) {
                nomo_async_process_command_release(target);
                return 1;
            }
        }
        target->env_names[index] =
            nomo_async_process_copy_cstr(item.@NAME_MEMBER@.data);
        target->env_values[index] =
            nomo_async_process_copy_cstr(item.@VALUE_MEMBER@.data);
        if (target->env_names[index] == NULL
            || target->env_values[index] == NULL) {
            nomo_async_process_command_release(target);
            return 2;
        }
    }
    return 0;
}

static wchar_t *nomo_async_process_utf8_to_wide(const char *value) {
    if (value == NULL) {
        return NULL;
    }
    int needed = MultiByteToWideChar(
        CP_UTF8,
        MB_ERR_INVALID_CHARS,
        value,
        -1,
        NULL,
        0
    );
    if (needed <= 0) {
        return NULL;
    }
    wchar_t *wide = (wchar_t *)malloc((size_t)needed * sizeof(wchar_t));
    if (wide == NULL
        || MultiByteToWideChar(
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

static void nomo_async_process_wide_text_reserve(
    nomo_async_process_wide_text *text,
    size_t needed
) {
    if (needed <= text->cap) {
        return;
    }
    size_t capacity = text->cap == 0u ? 128u : text->cap;
    while (capacity < needed) {
        if (capacity > SIZE_MAX / 2u) {
            capacity = needed;
            break;
        }
        capacity *= 2u;
    }
    wchar_t *next = (wchar_t *)realloc(
        text->data,
        capacity * sizeof(wchar_t)
    );
    if (next == NULL) {
        return;
    }
    text->data = next;
    text->cap = capacity;
}

static int nomo_async_process_wide_text_append(
    nomo_async_process_wide_text *text,
    const wchar_t *value,
    size_t length
) {
    nomo_async_process_wide_text_reserve(text, text->len + length + 1u);
    if (text->cap < text->len + length + 1u) {
        return 1;
    }
    memcpy(
        text->data + text->len,
        value,
        length * sizeof(wchar_t)
    );
    text->len += length;
    text->data[text->len] = L'\0';
    return 0;
}

static int nomo_async_process_wide_text_char(
    nomo_async_process_wide_text *text,
    wchar_t value
) {
    return nomo_async_process_wide_text_append(text, &value, 1u);
}

static int nomo_async_process_append_quoted(
    nomo_async_process_wide_text *text,
    const wchar_t *argument
) {
    int quote = argument[0] == L'\0'
        || wcspbrk(argument, L" \t\n\v\"") != NULL;
    if (!quote) {
        return nomo_async_process_wide_text_append(
            text,
            argument,
            wcslen(argument)
        );
    }
    if (nomo_async_process_wide_text_char(text, L'"') != 0) {
        return 1;
    }
    size_t slashes = 0u;
    for (const wchar_t *cursor = argument;; cursor += 1u) {
        if (*cursor == L'\\') {
            slashes += 1u;
            continue;
        }
        size_t count = *cursor == L'"' ? slashes * 2u + 1u : slashes;
        if (*cursor == L'\0') {
            count = slashes * 2u;
        }
        for (size_t index = 0u; index < count; index += 1u) {
            if (nomo_async_process_wide_text_char(text, L'\\') != 0) {
                return 1;
            }
        }
        slashes = 0u;
        if (*cursor == L'\0') {
            break;
        }
        if (nomo_async_process_wide_text_char(text, *cursor) != 0) {
            return 1;
        }
    }
    return nomo_async_process_wide_text_char(text, L'"');
}

static wchar_t *nomo_async_process_command_line(
    const nomo_async_process_command_copy *command
) {
    nomo_async_process_wide_text text = {0};
    wchar_t *program = nomo_async_process_utf8_to_wide(command->program);
    if (program == NULL
        || nomo_async_process_append_quoted(&text, program) != 0) {
        free(program);
        free(text.data);
        return NULL;
    }
    free(program);
    for (size_t index = 0u; index < command->argc; index += 1u) {
        wchar_t *argument =
            nomo_async_process_utf8_to_wide(command->args[index]);
        if (argument == NULL
            || nomo_async_process_wide_text_char(&text, L' ') != 0
            || nomo_async_process_append_quoted(&text, argument) != 0) {
            free(argument);
            free(text.data);
            return NULL;
        }
        free(argument);
    }
    return text.data;
}

static void nomo_async_process_wide_env_release(
    nomo_async_process_wide_env *env
) {
    for (size_t index = 0u; index < env->len; index += 1u) {
        free(env->items[index]);
    }
    free(env->items);
    memset(env, 0, sizeof(*env));
}

static int nomo_async_process_wide_env_push(
    nomo_async_process_wide_env *env,
    const wchar_t *value
) {
    if (env->len == env->cap) {
        size_t capacity = env->cap == 0u ? 64u : env->cap * 2u;
        wchar_t **next = (wchar_t **)realloc(
            env->items,
            capacity * sizeof(wchar_t *)
        );
        if (next == NULL) {
            return 1;
        }
        env->items = next;
        env->cap = capacity;
    }
    size_t length = wcslen(value);
    wchar_t *copy = (wchar_t *)malloc(
        (length + 1u) * sizeof(wchar_t)
    );
    if (copy == NULL) {
        return 1;
    }
    memcpy(copy, value, (length + 1u) * sizeof(wchar_t));
    env->items[env->len++] = copy;
    return 0;
}

static size_t nomo_async_process_env_name_length(const wchar_t *item) {
    const wchar_t *equals = wcschr(item, L'=');
    return equals == NULL ? wcslen(item) : (size_t)(equals - item);
}

static int nomo_async_process_env_name_equal(
    const wchar_t *item,
    const wchar_t *name
) {
    size_t item_length = nomo_async_process_env_name_length(item);
    size_t name_length = wcslen(name);
    return item_length == name_length
        && _wcsnicmp(item, name, name_length) == 0;
}

static int __cdecl nomo_async_process_env_compare(
    const void *left,
    const void *right
) {
    const wchar_t *left_value = *(const wchar_t * const *)left;
    const wchar_t *right_value = *(const wchar_t * const *)right;
    return _wcsicmp(left_value, right_value);
}

static wchar_t *nomo_async_process_environment_block(
    const nomo_async_process_command_copy *command
) {
    if (command->inherit_env != 0u && command->envc == 0u) {
        return NULL;
    }
    nomo_async_process_wide_env env = {0};
    if (command->inherit_env != 0u) {
        wchar_t *block = GetEnvironmentStringsW();
        if (block == NULL) {
            return NULL;
        }
        for (const wchar_t *item = block; *item != L'\0';
             item += wcslen(item) + 1u) {
            if (*item == L'=') {
                continue;
            }
            if (env.len >= 4096u - command->envc
                || nomo_async_process_wide_env_push(&env, item) != 0) {
                FreeEnvironmentStringsW(block);
                nomo_async_process_wide_env_release(&env);
                return NULL;
            }
        }
        FreeEnvironmentStringsW(block);
    }
    for (size_t index = 0u; index < command->envc; index += 1u) {
        wchar_t *name =
            nomo_async_process_utf8_to_wide(command->env_names[index]);
        wchar_t *value =
            nomo_async_process_utf8_to_wide(command->env_values[index]);
        if (name == NULL || value == NULL) {
            free(name);
            free(value);
            nomo_async_process_wide_env_release(&env);
            return NULL;
        }
        size_t name_length = wcslen(name);
        size_t value_length = wcslen(value);
        wchar_t *item = (wchar_t *)malloc(
            (name_length + value_length + 2u) * sizeof(wchar_t)
        );
        if (item == NULL) {
            free(name);
            free(value);
            nomo_async_process_wide_env_release(&env);
            return NULL;
        }
        memcpy(item, name, name_length * sizeof(wchar_t));
        item[name_length] = L'=';
        memcpy(
            item + name_length + 1u,
            value,
            (value_length + 1u) * sizeof(wchar_t)
        );
        size_t replacement = env.len;
        for (size_t existing = 0u; existing < env.len; existing += 1u) {
            if (nomo_async_process_env_name_equal(
                    env.items[existing],
                    name
                )) {
                replacement = existing;
                break;
            }
        }
        free(name);
        free(value);
        if (replacement < env.len) {
            free(env.items[replacement]);
            env.items[replacement] = item;
        } else {
            if (env.len == env.cap) {
                size_t capacity = env.cap == 0u ? 16u : env.cap * 2u;
                wchar_t **next = (wchar_t **)realloc(
                    env.items,
                    capacity * sizeof(wchar_t *)
                );
                if (next == NULL) {
                    free(item);
                    nomo_async_process_wide_env_release(&env);
                    return NULL;
                }
                env.items = next;
                env.cap = capacity;
            }
            env.items[env.len++] = item;
        }
    }
    qsort(
        env.items,
        env.len,
        sizeof(wchar_t *),
        nomo_async_process_env_compare
    );
    size_t units = 1u;
    for (size_t index = 0u; index < env.len; index += 1u) {
        units += wcslen(env.items[index]) + 1u;
    }
    if (env.len == 0u) {
        units = 2u;
    }
    if (units > NOMO_ASYNC_PROCESS_MAX_PAYLOAD_BYTES) {
        nomo_async_process_wide_env_release(&env);
        return NULL;
    }
    wchar_t *block = (wchar_t *)calloc(units, sizeof(wchar_t));
    if (block == NULL) {
        nomo_async_process_wide_env_release(&env);
        return NULL;
    }
    size_t offset = 0u;
    for (size_t index = 0u; index < env.len; index += 1u) {
        size_t length = wcslen(env.items[index]);
        memcpy(
            block + offset,
            env.items[index],
            (length + 1u) * sizeof(wchar_t)
        );
        offset += length + 1u;
    }
    nomo_async_process_wide_env_release(&env);
    return block;
}

static LONG nomo_async_process_pipe_counter = 0;

static int nomo_async_process_make_pipe(
    int parent_reads,
    HANDLE *parent,
    HANDLE *child
) {
    wchar_t name[128];
    LONG sequence = InterlockedIncrement(&nomo_async_process_pipe_counter);
    int length = _snwprintf_s(
        name,
        sizeof(name) / sizeof(name[0]),
        _TRUNCATE,
        L"\\\\.\\pipe\\nomo-async-%lu-%ld",
        (unsigned long)GetCurrentProcessId(),
        (long)sequence
    );
    if (length < 0) {
        return 1;
    }
    DWORD access = (parent_reads ? PIPE_ACCESS_INBOUND : PIPE_ACCESS_OUTBOUND)
        | FILE_FLAG_OVERLAPPED
        | FILE_FLAG_FIRST_PIPE_INSTANCE;
    HANDLE server = CreateNamedPipeW(
        name,
        access,
        PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT,
        1u,
        NOMO_ASYNC_PROCESS_READ_CHUNK,
        NOMO_ASYNC_PROCESS_READ_CHUNK,
        0u,
        NULL
    );
    if (server == INVALID_HANDLE_VALUE) {
        return 1;
    }
    HANDLE connected_event = CreateEventW(NULL, TRUE, FALSE, NULL);
    if (connected_event == NULL) {
        CloseHandle(server);
        return 1;
    }
    OVERLAPPED connected_overlapped;
    memset(&connected_overlapped, 0, sizeof(connected_overlapped));
    connected_overlapped.hEvent = connected_event;
    BOOL connected = ConnectNamedPipe(server, &connected_overlapped);
    DWORD connect_error = connected != FALSE
        ? ERROR_SUCCESS
        : GetLastError();
    if (connected == FALSE
        && connect_error != ERROR_IO_PENDING
        && connect_error != ERROR_PIPE_CONNECTED) {
        CloseHandle(connected_event);
        CloseHandle(server);
        return 1;
    }
    SECURITY_ATTRIBUTES security = {
        .nLength = sizeof(SECURITY_ATTRIBUTES),
        .lpSecurityDescriptor = NULL,
        .bInheritHandle = TRUE
    };
    HANDLE client = CreateFileW(
        name,
        parent_reads ? GENERIC_WRITE : GENERIC_READ,
        0u,
        &security,
        OPEN_EXISTING,
        FILE_ATTRIBUTE_NORMAL,
        NULL
    );
    if (client == INVALID_HANDLE_VALUE) {
        if (connect_error == ERROR_IO_PENDING) {
            CancelIoEx(server, &connected_overlapped);
            WaitForSingleObject(connected_event, INFINITE);
        }
        CloseHandle(connected_event);
        CloseHandle(server);
        return 1;
    }
    if (connect_error == ERROR_IO_PENDING) {
        DWORD wait_status = WaitForSingleObject(connected_event, 5000u);
        DWORD transferred = 0u;
        if (wait_status != WAIT_OBJECT_0
            || !GetOverlappedResult(
                server,
                &connected_overlapped,
                &transferred,
                FALSE
            )) {
            CancelIoEx(server, &connected_overlapped);
            WaitForSingleObject(connected_event, INFINITE);
            CloseHandle(connected_event);
            CloseHandle(server);
            CloseHandle(client);
            return 1;
        }
    }
    CloseHandle(connected_event);
    *parent = server;
    *child = client;
    return 0;
}

static void nomo_async_process_spawn_cleanup(
    HANDLE process,
    HANDLE stdin_write,
    HANDLE stdout_read,
    HANDLE stderr_read
) {
    if (process != NULL) {
        if (WaitForSingleObject(process, 0u) == WAIT_TIMEOUT) {
            TerminateProcess(process, 137u);
        }
        WaitForSingleObject(process, INFINITE);
        CloseHandle(process);
    }
    nomo_async_process_close_handle(&stdin_write);
    nomo_async_process_close_handle(&stdout_read);
    nomo_async_process_close_handle(&stderr_read);
}

static void nomo_async_process_spawn_blocking(
    nomo_async_process_job *job
) {
    HANDLE stdin_read = NULL;
    HANDLE stdin_write = NULL;
    HANDLE stdout_read = NULL;
    HANDLE stdout_write = NULL;
    HANDLE stderr_read = NULL;
    HANDLE stderr_write = NULL;
    STARTUPINFOEXW startup;
    PROCESS_INFORMATION process;
    memset(&startup, 0, sizeof(startup));
    memset(&process, 0, sizeof(process));
    wchar_t *command_line =
        nomo_async_process_command_line(&job->command);
    wchar_t *environment =
        nomo_async_process_environment_block(&job->command);
    wchar_t *cwd = job->command.cwd == NULL
        ? NULL
        : nomo_async_process_utf8_to_wide(job->command.cwd);
    if (command_line == NULL
        || (job->command.cwd != NULL && cwd == NULL)
        || (environment == NULL
            && (job->command.inherit_env == 0u
                || job->command.envc != 0u))) {
        job->spawn_error = ERROR_INVALID_PARAMETER;
        free(command_line);
        free(environment);
        free(cwd);
        return;
    }
    if (nomo_async_process_make_pipe(
            0,
            &stdin_write,
            &stdin_read
        ) != 0
        || nomo_async_process_make_pipe(
            1,
            &stdout_read,
            &stdout_write
        ) != 0
        || nomo_async_process_make_pipe(
            1,
            &stderr_read,
            &stderr_write
        ) != 0) {
        job->spawn_error = GetLastError();
        goto cleanup;
    }
    SIZE_T attribute_bytes = 0u;
    InitializeProcThreadAttributeList(NULL, 1u, 0u, &attribute_bytes);
    startup.lpAttributeList =
        (LPPROC_THREAD_ATTRIBUTE_LIST)malloc(attribute_bytes);
    if (startup.lpAttributeList == NULL
        || !InitializeProcThreadAttributeList(
            startup.lpAttributeList,
            1u,
            0u,
            &attribute_bytes
        )) {
        job->spawn_error = GetLastError();
        goto cleanup;
    }
    HANDLE inherited[3] = {stdin_read, stdout_write, stderr_write};
    if (!UpdateProcThreadAttribute(
            startup.lpAttributeList,
            0u,
            PROC_THREAD_ATTRIBUTE_HANDLE_LIST,
            inherited,
            sizeof(inherited),
            NULL,
            NULL
        )) {
        job->spawn_error = GetLastError();
        goto cleanup;
    }
    startup.StartupInfo.cb = sizeof(startup);
    startup.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
    startup.StartupInfo.hStdInput = stdin_read;
    startup.StartupInfo.hStdOutput = stdout_write;
    startup.StartupInfo.hStdError = stderr_write;
    DWORD flags = EXTENDED_STARTUPINFO_PRESENT | CREATE_UNICODE_ENVIRONMENT;
    BOOL created = CreateProcessW(
        NULL,
        command_line,
        NULL,
        NULL,
        TRUE,
        flags,
        environment,
        cwd,
        &startup.StartupInfo,
        &process
    );
    if (!created) {
        job->spawn_error = GetLastError();
        goto cleanup;
    }
    CloseHandle(process.hThread);
    job->process = process.hProcess;
    job->stdin_write = stdin_write;
    job->stdout_read = stdout_read;
    job->stderr_read = stderr_read;
    stdin_write = NULL;
    stdout_read = NULL;
    stderr_read = NULL;

cleanup:
    if (startup.lpAttributeList != NULL) {
        DeleteProcThreadAttributeList(startup.lpAttributeList);
        free(startup.lpAttributeList);
    }
    nomo_async_process_close_handle(&stdin_read);
    nomo_async_process_close_handle(&stdout_write);
    nomo_async_process_close_handle(&stderr_write);
    nomo_async_process_close_handle(&stdin_write);
    nomo_async_process_close_handle(&stdout_read);
    nomo_async_process_close_handle(&stderr_read);
    free(command_line);
    free(environment);
    free(cwd);
}

static DWORD WINAPI nomo_async_process_worker(void *opaque) {
    nomo_async_process_pool *pool =
        (nomo_async_process_pool *)opaque;
    for (;;) {
        EnterCriticalSection(&pool->lock);
        while (pool->queue_count == 0u && pool->stopping == 0u) {
            SleepConditionVariableCS(
                &pool->available,
                &pool->lock,
                INFINITE
            );
        }
        if (pool->queue_count == 0u && pool->stopping != 0u) {
            LeaveCriticalSection(&pool->lock);
            return 0u;
        }
        uint32_t slot = pool->queue[pool->queue_head];
        pool->queue_head =
            (pool->queue_head + 1u) % NOMO_ASYNC_PROCESS_JOB_CAPACITY;
        pool->queue_count -= 1u;
        nomo_async_process_job *job = &pool->jobs[slot];
        if (job->state == NOMO_ASYNC_PROCESS_JOB_CANCELLED) {
            nomo_async_process_command_release(&job->command);
            memset(job, 0, sizeof(*job));
            if (pool->active_jobs > 0u) {
                pool->active_jobs -= 1u;
            }
            if (pool->active_start_jobs > 0u) {
                pool->active_start_jobs -= 1u;
            }
            LeaveCriticalSection(&pool->lock);
            continue;
        }
        job->state = NOMO_ASYNC_PROCESS_JOB_RUNNING;
        LeaveCriticalSection(&pool->lock);

        nomo_async_process_spawn_blocking(job);
        nomo_async_process_command_release(&job->command);

        EnterCriticalSection(&pool->lock);
        if (job->state == NOMO_ASYNC_PROCESS_JOB_CANCELLED
            || pool->stopping != 0u) {
            HANDLE process = job->process;
            HANDLE stdin_write = job->stdin_write;
            HANDLE stdout_read = job->stdout_read;
            HANDLE stderr_read = job->stderr_read;
            memset(job, 0, sizeof(*job));
            if (pool->active_jobs > 0u) {
                pool->active_jobs -= 1u;
            }
            if (pool->active_start_jobs > 0u) {
                pool->active_start_jobs -= 1u;
            }
            LeaveCriticalSection(&pool->lock);
            nomo_async_process_spawn_cleanup(
                process,
                stdin_write,
                stdout_read,
                stderr_read
            );
            continue;
        }
        job->state = NOMO_ASYNC_PROCESS_JOB_COMPLETED;
        job->notified = 0u;
        LeaveCriticalSection(&pool->lock);
        if (nomo_async_reactor_post(
                &pool->context->reactor,
                &pool->completion_registration,
                NOMO_ASYNC_REACTOR_PROCESS
            ) != 0) {
            InterlockedExchange8(
                (volatile char *)&pool->context->runtime_failed,
                1
            );
        }
    }
}

static void nomo_async_process_pool_completion_wake(
    void *owner,
    uint32_t ready
) {
    nomo_async_process_pool *pool =
        (nomo_async_process_pool *)owner;
    if (pool == NULL
        || (ready & NOMO_ASYNC_REACTOR_PROCESS) == 0u) {
        return;
    }
    for (;;) {
        void *completion_owner = NULL;
        nomo_async_process_completion_fn complete = NULL;
        uint32_t slot = 0u;
        uint32_t generation = 0u;
        EnterCriticalSection(&pool->lock);
        for (uint32_t index = 0u;
             index < NOMO_ASYNC_PROCESS_JOB_CAPACITY;
             index += 1u) {
            nomo_async_process_job *job = &pool->jobs[index];
            if (job->state != NOMO_ASYNC_PROCESS_JOB_COMPLETED
                || job->notified != 0u) {
                continue;
            }
            job->notified = 1u;
            completion_owner = job->owner;
            complete = job->complete;
            slot = index;
            generation = job->generation;
            break;
        }
        LeaveCriticalSection(&pool->lock);
        if (complete == NULL) {
            return;
        }
        complete(completion_owner, slot, generation);
    }
}

static int nomo_async_process_pool_initialize(
    nomo_async_process_pool *pool,
    nomo_async_context *context
) {
    memset(pool, 0, sizeof(*pool));
    pool->context = context;
    InitializeCriticalSection(&pool->lock);
    InitializeConditionVariable(&pool->available);
    pool->completion_registration.owner = pool;
    pool->completion_registration.wake =
        nomo_async_process_pool_completion_wake;
    pool->completion_registration.interests =
        NOMO_ASYNC_REACTOR_PROCESS;
    if (nomo_async_reactor_post_activate(
            &context->reactor,
            &pool->completion_registration
        ) != 0) {
        DeleteCriticalSection(&pool->lock);
        return 1;
    }
    pool->worker = CreateThread(
        NULL,
        0u,
        nomo_async_process_worker,
        pool,
        0u,
        NULL
    );
    if (pool->worker == NULL) {
        nomo_async_reactor_post_deactivate(
            &context->reactor,
            &pool->completion_registration
        );
        DeleteCriticalSection(&pool->lock);
        return 1;
    }
    context->blocking_pool_initializations += 1u;
    context->blocking_threads_started += 1u;
    context->live_blocking_threads += 1u;
    if (context->live_blocking_threads
        > context->peak_live_blocking_threads) {
        context->peak_live_blocking_threads =
            context->live_blocking_threads;
    }
    return 0;
}

static nomo_async_process_runtime *nomo_async_process_runtime_get(
    nomo_async_context *context
) {
    if (context->process_runtime != NULL) {
        return (nomo_async_process_runtime *)context->process_runtime;
    }
    nomo_async_process_runtime *runtime =
        (nomo_async_process_runtime *)calloc(1u, sizeof(*runtime));
    nomo_async_process_pool *pool =
        (nomo_async_process_pool *)calloc(1u, sizeof(*pool));
    if (runtime == NULL || pool == NULL) {
        free(runtime);
        free(pool);
        return NULL;
    }
    runtime->context = context;
    runtime->pool = pool;
    context->process_runtime = runtime;
    if (nomo_async_process_pool_initialize(pool, context) != 0) {
        context->process_runtime = NULL;
        free(pool);
        free(runtime);
        return NULL;
    }
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
    nomo_async_process_pool *pool = runtime->pool;
    EnterCriticalSection(&pool->lock);
    if (pool->stopping != 0u
        || pool->queue_count == NOMO_ASYNC_PROCESS_JOB_CAPACITY
        || pool->active_start_jobs == NOMO_ASYNC_PROCESS_START_CAPACITY) {
        LeaveCriticalSection(&pool->lock);
        return 2;
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
    if (selected == NOMO_ASYNC_PROCESS_JOB_CAPACITY) {
        LeaveCriticalSection(&pool->lock);
        return 2;
    }
    pool->next_generation += 1u;
    if (pool->next_generation == 0u) {
        pool->next_generation = 1u;
    }
    nomo_async_process_job *job = &pool->jobs[selected];
    memset(job, 0, sizeof(*job));
    job->generation = pool->next_generation;
    job->state = NOMO_ASYNC_PROCESS_JOB_QUEUED;
    job->command = *command;
    memset(command, 0, sizeof(*command));
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
    WakeConditionVariable(&pool->available);
    LeaveCriticalSection(&pool->lock);
    runtime->context->blocking_jobs_queued += 1u;
    runtime->context->live_blocking_jobs += 1u;
    if (runtime->context->live_blocking_jobs
        > runtime->context->peak_live_blocking_jobs) {
        runtime->context->peak_live_blocking_jobs =
            runtime->context->live_blocking_jobs;
    }
    return 0;
}

static void nomo_async_process_pool_cancel_start(
    nomo_async_process_runtime *runtime,
    uint32_t slot,
    uint32_t generation
) {
    if (runtime == NULL || slot >= NOMO_ASYNC_PROCESS_JOB_CAPACITY) {
        return;
    }
    nomo_async_process_pool *pool = runtime->pool;
    HANDLE process = NULL;
    HANDLE stdin_write = NULL;
    HANDLE stdout_read = NULL;
    HANDLE stderr_read = NULL;
    int release_now = 0;
    EnterCriticalSection(&pool->lock);
    nomo_async_process_job *job = &pool->jobs[slot];
    if (job->generation == generation
        && job->state != NOMO_ASYNC_PROCESS_JOB_FREE) {
        job->owner = NULL;
        job->complete = NULL;
        if (job->state == NOMO_ASYNC_PROCESS_JOB_COMPLETED) {
            process = job->process;
            stdin_write = job->stdin_write;
            stdout_read = job->stdout_read;
            stderr_read = job->stderr_read;
            nomo_async_process_command_release(&job->command);
            memset(job, 0, sizeof(*job));
            if (pool->active_jobs > 0u) {
                pool->active_jobs -= 1u;
            }
            if (pool->active_start_jobs > 0u) {
                pool->active_start_jobs -= 1u;
            }
            release_now = 1;
        } else {
            job->state = NOMO_ASYNC_PROCESS_JOB_CANCELLED;
        }
    }
    LeaveCriticalSection(&pool->lock);
    if (release_now != 0) {
        nomo_async_process_spawn_cleanup(
            process,
            stdin_write,
            stdout_read,
            stderr_read
        );
        runtime->context->blocking_jobs_cancelled += 1u;
        if (runtime->context->live_blocking_jobs > 0u) {
            runtime->context->live_blocking_jobs -= 1u;
        }
    }
}

static int nomo_async_process_pool_take_start(
    nomo_async_process_runtime *runtime,
    uint32_t slot,
    uint32_t generation,
    HANDLE *process,
    HANDLE *stdin_write,
    HANDLE *stdout_read,
    HANDLE *stderr_read,
    DWORD *spawn_error
) {
    if (runtime == NULL || slot >= NOMO_ASYNC_PROCESS_JOB_CAPACITY) {
        return 1;
    }
    nomo_async_process_pool *pool = runtime->pool;
    EnterCriticalSection(&pool->lock);
    nomo_async_process_job *job = &pool->jobs[slot];
    if (job->generation != generation
        || job->state != NOMO_ASYNC_PROCESS_JOB_COMPLETED) {
        LeaveCriticalSection(&pool->lock);
        return 1;
    }
    *process = job->process;
    *stdin_write = job->stdin_write;
    *stdout_read = job->stdout_read;
    *stderr_read = job->stderr_read;
    *spawn_error = job->spawn_error;
    memset(job, 0, sizeof(*job));
    if (pool->active_jobs > 0u) {
        pool->active_jobs -= 1u;
    }
    if (pool->active_start_jobs > 0u) {
        pool->active_start_jobs -= 1u;
    }
    LeaveCriticalSection(&pool->lock);
    runtime->context->blocking_jobs_started += 1u;
    runtime->context->blocking_jobs_completed += 1u;
    if (runtime->context->live_blocking_jobs > 0u) {
        runtime->context->live_blocking_jobs -= 1u;
    }
    return 0;
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
            if (capacity > maximum / 2u) {
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
        memmove(
            buffer->data,
            buffer->data + consumed,
            buffer->len - consumed
        );
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
        size_t width = 0u;
        int status = nomo_async_process_utf8_width(
            (const unsigned char *)data + index,
            length - index,
            &width
        );
        if (status < 0 || (status == 0 && eof != 0)) {
            return 1;
        }
        if (status == 0 || index + width > maximum) {
            break;
        }
        index += width;
    }
    *prefix = index;
    return 0;
}

static void nomo_async_process_event_release(
    nomo_async_process_registration *registration
);

static void nomo_async_process_cancel(
    nomo_async_process_registration *registration,
    nomo_async_context *context
);

static void nomo_async_process_handle_storage_release(
    nomo_async_process_runtime *runtime,
    nomo_async_process_handle_state *state
) {
    if (state->event_registration != NULL) {
        nomo_async_process_cancel(
            (nomo_async_process_registration *)
                state->event_registration,
            runtime->context
        );
    }
    if (state->stdin_registration.active != 0u) {
        if (state->stdin_registration.operation != NULL
            && state->stdin_data != NULL) {
            nomo_async_reactor_detach_buffer(
                &state->stdin_registration,
                state->stdin_data
            );
            state->stdin_data = NULL;
        }
        nomo_async_reactor_deregister(
            &runtime->context->reactor,
            &state->stdin_registration
        );
    }
    if (state->exit_registration.active != 0u) {
        nomo_async_reactor_post_deactivate(
            &runtime->context->reactor,
            &state->exit_registration
        );
    }
    if (state->wait_handle != NULL) {
        UnregisterWaitEx(state->wait_handle, INVALID_HANDLE_VALUE);
        state->wait_handle = NULL;
    }
    nomo_async_process_close_handle(&state->stdin_write);
    nomo_async_process_close_handle(&state->stdout_read);
    nomo_async_process_close_handle(&state->stderr_read);
    nomo_async_process_close_handle(&state->process);
    if (state->stdin_data != NULL) {
        nomo_async_process_retained_remove(
            runtime->context,
            state->stdin_len
        );
    }
    free(state->stdin_data);
    nomo_async_process_buffer_release(
        runtime->context,
        &state->stdout_buffer
    );
    nomo_async_process_buffer_release(
        runtime->context,
        &state->stderr_buffer
    );
    uint32_t generation = state->generation;
    memset(state, 0, sizeof(*state));
    state->generation = generation;
}

static int nomo_async_process_handle_reserve(
    nomo_async_process_runtime *runtime,
    uint32_t *slot_out,
    uint32_t *generation_out
) {
    for (uint32_t index = 0u;
         index < NOMO_ASYNC_PROCESS_HANDLE_CAPACITY;
         index += 1u) {
        if (runtime->handles[index].occupied != 0u) {
            continue;
        }
        runtime->next_handle_generation += 1u;
        if (runtime->next_handle_generation == 0u) {
            runtime->next_handle_generation = 1u;
        }
        nomo_async_process_handle_state *state =
            &runtime->handles[index];
        memset(state, 0, sizeof(*state));
        state->context = runtime->context;
        state->generation = runtime->next_handle_generation;
        state->occupied = 1u;
        *slot_out = index;
        *generation_out = state->generation;
        return 0;
    }
    return 1;
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
        || state->closing != 0u
        || state->generation != child.@GENERATION_MEMBER@
        || state->process == NULL) {
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
    for (uint32_t index = 0u; index < 2u; index += 1u) {
        if (registration->io[index].operation != NULL
            && registration->read_buffers[index] != NULL) {
            nomo_async_reactor_detach_buffer(
                &registration->io[index],
                registration->read_buffers[index]
            );
            registration->read_buffers[index] = NULL;
        }
        nomo_async_reactor_deregister(
            &registration->context->reactor,
            &registration->io[index]
        );
        free(registration->read_buffers[index]);
        registration->read_buffers[index] = NULL;
    }
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
        && state->process == NULL) {
        nomo_async_process_handle_storage_release(runtime, state);
    }
}

static VOID CALLBACK nomo_async_process_wait_callback(
    PVOID opaque,
    BOOLEAN timed_out
) {
    (void)timed_out;
    nomo_async_process_wait_context *wait_context =
        (nomo_async_process_wait_context *)opaque;
    nomo_async_process_handle_state *state =
        wait_context == NULL ? NULL : wait_context->state;
    if (state == NULL
        || state->context == NULL
        || state->generation != wait_context->generation) {
        return;
    }
    PostQueuedCompletionStatus(
        state->context->reactor.handle,
        0u,
        (ULONG_PTR)&state->exit_registration,
        NULL
    );
}

static void nomo_async_process_event_signal(
    nomo_async_process_handle_state *state
) {
    nomo_async_process_registration *registration =
        (nomo_async_process_registration *)state->event_registration;
    if (registration == NULL
        || registration->kind != NOMO_ASYNC_PROCESS_REGISTRATION_EVENT
        || registration->active == 0u
        || registration->ready != 0u) {
        return;
    }
    registration->ready = 1u;
    if (nomo_async_ready_enqueue(
            registration->context,
            registration->frame,
            registration->poll
        ) != 0) {
        registration->context->runtime_failed = 1u;
    }
}

static void nomo_async_process_exit_wake(void *owner, uint32_t ready) {
    nomo_async_process_handle_state *state =
        (nomo_async_process_handle_state *)owner;
    if (state == NULL
        || state->occupied == 0u
        || (ready & NOMO_ASYNC_REACTOR_PROCESS) == 0u) {
        return;
    }
    DWORD code = STILL_ACTIVE;
    if (state->process != NULL
        && GetExitCodeProcess(state->process, &code)
        && code != STILL_ACTIVE) {
        state->exited = 1u;
        state->exit_info = nomo_async_process_exit_value(code);
    }
    nomo_async_reactor_post_deactivate(
        &state->context->reactor,
        &state->exit_registration
    );
    if (state->wait_handle != NULL) {
        UnregisterWaitEx(state->wait_handle, INVALID_HANDLE_VALUE);
        state->wait_handle = NULL;
    }
    if (state->closing != 0u
        && state->context->process_runtime != NULL) {
        nomo_async_process_runtime *runtime =
            (nomo_async_process_runtime *)
                state->context->process_runtime;
        nomo_async_process_handle_storage_release(runtime, state);
        return;
    }
    nomo_async_process_event_signal(state);
}

static int nomo_async_process_associate_handle(
    nomo_async_context *context,
    HANDLE handle
) {
    if (nomo_async_reactor_init(&context->reactor) != 0) {
        return 1;
    }
    HANDLE associated = CreateIoCompletionPort(
        handle,
        context->reactor.handle,
        0u,
        0u
    );
    if (associated != context->reactor.handle) {
        context->reactor.errors += 1u;
        return 1;
    }
    return 0;
}

static int nomo_async_process_activate_handle(
    nomo_async_process_runtime *runtime,
    nomo_async_process_registration *registration,
    HANDLE process,
    HANDLE stdin_write,
    HANDLE stdout_read,
    HANDLE stderr_read
) {
    nomo_async_process_handle_state *state =
        &runtime->handles[registration->handle_slot];
    state->process = process;
    state->stdin_write = stdin_write;
    state->stdout_read = stdout_read;
    state->stderr_read = stderr_read;
    if (nomo_async_process_associate_handle(
            runtime->context,
            stdin_write
        ) != 0
        || nomo_async_process_associate_handle(
            runtime->context,
            stdout_read
        ) != 0
        || nomo_async_process_associate_handle(
            runtime->context,
            stderr_read
        ) != 0) {
        return 1;
    }
    state->exit_registration.owner = state;
    state->exit_registration.wake = nomo_async_process_exit_wake;
    state->exit_registration.interests = NOMO_ASYNC_REACTOR_PROCESS;
    if (nomo_async_reactor_post_activate(
            &runtime->context->reactor,
            &state->exit_registration
        ) != 0) {
        return 1;
    }
    state->wait_context.state = state;
    state->wait_context.generation = state->generation;
    if (!RegisterWaitForSingleObject(
            &state->wait_handle,
            process,
            nomo_async_process_wait_callback,
            &state->wait_context,
            INFINITE,
            WT_EXECUTEONLYONCE
        )) {
        nomo_async_reactor_post_deactivate(
            &runtime->context->reactor,
            &state->exit_registration
        );
        return 1;
    }
    return 0;
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
            "limit",
            "bounded process start queue is full"
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
    HANDLE process = NULL;
    HANDLE stdin_write = NULL;
    HANDLE stdout_read = NULL;
    HANDLE stderr_read = NULL;
    DWORD spawn_error = ERROR_SUCCESS;
    if (nomo_async_process_pool_take_start(
            runtime,
            registration->job_slot,
            registration->job_generation,
            &process,
            &stdin_write,
            &stdout_read,
            &stderr_read,
            &spawn_error
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
    if (spawn_error != ERROR_SUCCESS || process == NULL) {
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
    if (nomo_async_process_activate_handle(
            runtime,
            registration,
            process,
            stdin_write,
            stdout_read,
            stderr_read
        ) != 0) {
        nomo_async_process_handle_state *state =
            &runtime->handles[registration->handle_slot];
        if (WaitForSingleObject(process, 0u) == WAIT_TIMEOUT) {
            TerminateProcess(process, 137u);
        }
        nomo_async_process_handle_storage_release(runtime, state);
        nomo_async_process_start_error(
            result,
            "reactor",
            "failed to attach process pipes to owner IOCP"
        );
        context->process_errors += 1u;
        registration->kind = NOMO_ASYNC_PROCESS_REGISTRATION_NONE;
        return NOMO_ASYNC_POLL_READY;
    }
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
    if (state->exited != 0u || state->process == NULL) {
        return;
    }
    DWORD code = STILL_ACTIVE;
    if (GetExitCodeProcess(state->process, &code)
        && code != STILL_ACTIVE) {
        state->exited = 1u;
        state->exit_info = nomo_async_process_exit_value(code);
    }
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

static void nomo_async_process_handle_close(
    nomo_async_process_runtime *runtime,
    nomo_async_process_handle_state *state,
    uint8_t cancel_registration
) {
    if (state->closing != 0u) {
        return;
    }
    if (cancel_registration != 0u
        && state->event_registration != NULL) {
        nomo_async_process_cancel(
            (nomo_async_process_registration *)
                state->event_registration,
            runtime->context
        );
    }
    state->closing = 1u;
    if (state->stdin_registration.operation != NULL
        && state->stdin_data != NULL) {
        nomo_async_process_retained_remove(
            runtime->context,
            state->stdin_len
        );
        nomo_async_reactor_detach_buffer(
            &state->stdin_registration,
            state->stdin_data
        );
        state->stdin_data = NULL;
        state->stdin_len = 0u;
        state->stdin_offset = 0u;
    }
    nomo_async_reactor_deregister(
        &runtime->context->reactor,
        &state->stdin_registration
    );
    nomo_async_process_close_handle(&state->stdin_write);
    nomo_async_process_close_handle(&state->stdout_read);
    nomo_async_process_close_handle(&state->stderr_read);
    nomo_async_process_buffer_release(
        runtime->context,
        &state->stdout_buffer
    );
    nomo_async_process_buffer_release(
        runtime->context,
        &state->stderr_buffer
    );
    nomo_async_process_update_exit(state);
    if (runtime->context->live_process_handles > 0u) {
        runtime->context->live_process_handles -= 1u;
    }
    if (state->exited != 0u) {
        nomo_async_process_handle_storage_release(runtime, state);
    }
}

static int nomo_async_process_event_progress(
    nomo_async_process_runtime *runtime,
    nomo_async_process_handle_state *state,
    uint64_t max_chunk_bytes,
    @EVENT_RESULT@ *result
) {
    nomo_async_process_update_exit(state);
    if (state->stdin_error != 0u) {
        state->stdin_error = 0u;
        nomo_async_process_event_error(
            result,
            "io",
            "process stdin write failed"
        );
        runtime->context->process_errors += 1u;
        return 0;
    }
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
        && state->stderr_eof != 0u) {
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
    if (state->occupied != 0u
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
        || ready == 0u
        || registration->ready != 0u) {
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

static int nomo_async_process_event_issue_read(
    nomo_async_process_registration *registration,
    uint32_t index,
    HANDLE handle
) {
    char *buffer =
        (char *)malloc(NOMO_ASYNC_PROCESS_READ_CHUNK);
    if (buffer == NULL) {
        return 2;
    }
    registration->read_buffers[index] = buffer;
    nomo_async_reactor_registration *io = &registration->io[index];
    io->owner = registration;
    io->wake = nomo_async_process_event_wake;
    if (nomo_async_reactor_register(
            &registration->context->reactor,
            io,
            (nomo_socket)handle,
            NOMO_ASYNC_REACTOR_READ
        ) != 0) {
        free(buffer);
        registration->read_buffers[index] = NULL;
        return 2;
    }
    DWORD transferred = 0u;
    BOOL started = ReadFile(
        handle,
        buffer,
        NOMO_ASYNC_PROCESS_READ_CHUNK,
        &transferred,
        nomo_async_reactor_overlapped(io)
    );
    DWORD error = started != FALSE ? ERROR_SUCCESS : GetLastError();
    if (started == FALSE && error != ERROR_IO_PENDING) {
        nomo_async_reactor_deregister(
            &registration->context->reactor,
            io
        );
        free(buffer);
        registration->read_buffers[index] = NULL;
        if (error == ERROR_BROKEN_PIPE || error == ERROR_HANDLE_EOF) {
            return 1;
        }
        return 3;
    }
    nomo_async_reactor_mark_submitted(io);
    return 0;
}

static int nomo_async_process_event_collect_read(
    nomo_async_process_registration *registration,
    nomo_async_process_handle_state *state,
    uint32_t index
) {
    nomo_async_reactor_registration *io = &registration->io[index];
    if (io->active == 0u || io->operation != NULL) {
        return 0;
    }
    DWORD error = io->error;
    DWORD transferred = io->transferred;
    nomo_async_reactor_deregister(
        &registration->context->reactor,
        io
    );
    char *buffer = registration->read_buffers[index];
    registration->read_buffers[index] = NULL;
    if (error == ERROR_BROKEN_PIPE || error == ERROR_HANDLE_EOF
        || (error == ERROR_SUCCESS && transferred == 0u)) {
        if (index == 0u) {
            state->stdout_eof = 1u;
        } else {
            state->stderr_eof = 1u;
        }
        free(buffer);
        return 0;
    }
    if (error != ERROR_SUCCESS
        || nomo_async_process_buffer_append(
            registration->context,
            index == 0u
                ? &state->stdout_buffer
                : &state->stderr_buffer,
            buffer,
            (size_t)transferred
        ) != 0) {
        free(buffer);
        return 1;
    }
    free(buffer);
    return 0;
}

static int nomo_async_process_event_arm(
    nomo_async_process_registration *registration,
    nomo_async_process_handle_state *state,
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
        return 2;
    }
    registration->deadline_millis = registration->timer.deadline_millis;
    int stdout_status = state->stdout_eof != 0u
        ? 1
        : nomo_async_process_event_issue_read(
            registration,
            0u,
            state->stdout_read
        );
    int stderr_status = state->stderr_eof != 0u
        ? 1
        : nomo_async_process_event_issue_read(
            registration,
            1u,
            state->stderr_read
        );
    if (stdout_status == 1) {
        state->stdout_eof = 1u;
    }
    if (stderr_status == 1) {
        state->stderr_eof = 1u;
    }
    if (stdout_status > 1 || stderr_status > 1) {
        nomo_async_process_registration_finish(registration);
        return stdout_status == 2 || stderr_status == 2 ? 2 : 3;
    }
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
    if (max_chunk_bytes == 0u
        || max_chunk_bytes > NOMO_ASYNC_PROCESS_MAX_PAYLOAD_BYTES
        || timeout_millis == 0u
        || timeout_millis > NOMO_ASYNC_PROCESS_MAX_TIMEOUT_MILLIS) {
        nomo_async_process_event_error(
            result,
            "invalid_request",
            "process event limits are invalid"
        );
        context->process_errors += 1u;
        return NOMO_ASYNC_POLL_READY;
    }
    nomo_async_process_runtime *runtime = NULL;
    nomo_async_process_handle_state *state =
        nomo_async_process_handle_find(child, &runtime);
    if (state == NULL || runtime->context != context) {
        nomo_async_process_event_error(
            result,
            "closed",
            "process child is closed or belongs to another executor"
        );
        return NOMO_ASYNC_POLL_READY;
    }
    if (state->event_busy != 0u) {
        nomo_async_process_event_error(
            result,
            "busy",
            "process child already has a pending event pull"
        );
        context->process_errors += 1u;
        return NOMO_ASYNC_POLL_READY;
    }
    if (nomo_async_process_event_progress(
            runtime,
            state,
            max_chunk_bytes,
            result
        ) == 0) {
        return NOMO_ASYNC_POLL_READY;
    }
    registration->kind = NOMO_ASYNC_PROCESS_REGISTRATION_EVENT;
    registration->context = context;
    registration->frame = context->current_frame;
    registration->poll = context->current_poll;
    registration->handle_slot = child.@SLOT_MEMBER@;
    registration->handle_generation = child.@GENERATION_MEMBER@;
    registration->max_chunk_bytes = max_chunk_bytes;
    state->event_busy = 1u;
    state->event_registration = registration;
    int armed =
        nomo_async_process_event_arm(registration, state, timeout_millis);
    if (armed != 0) {
        nomo_async_process_event_release(registration);
        nomo_async_process_event_error(
            result,
            armed == 2 ? "limit" : "reactor",
            armed == 2
                ? "owner executor process event capacity is exhausted"
                : "process IOCP read submission failed"
        );
        context->process_errors += 1u;
        registration->kind = NOMO_ASYNC_PROCESS_REGISTRATION_NONE;
        return NOMO_ASYNC_POLL_READY;
    }
    if (nomo_async_process_event_progress(
            runtime,
            state,
            max_chunk_bytes,
            result
        ) == 0) {
        nomo_async_process_registration_finish(registration);
        nomo_async_process_event_release(registration);
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
    if (registration->timer.expired != 0u) {
        registration->timer.expired = 0u;
        nomo_async_process_registration_finish(registration);
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
    if (registration->ready == 0u) {
        context->pending_reason = NOMO_ASYNC_PENDING_IO;
        return NOMO_ASYNC_POLL_PENDING;
    }
    if (runtime == NULL
        || registration->handle_slot
            >= NOMO_ASYNC_PROCESS_HANDLE_CAPACITY) {
        nomo_async_process_registration_finish(registration);
        nomo_async_process_event_release(registration);
        nomo_async_process_event_error(
            result,
            "closed",
            "process child is closed"
        );
        registration->kind = NOMO_ASYNC_PROCESS_REGISTRATION_NONE;
        return NOMO_ASYNC_POLL_READY;
    }
    nomo_async_process_handle_state *state =
        &runtime->handles[registration->handle_slot];
    if (state->occupied == 0u
        || state->generation != registration->handle_generation) {
        nomo_async_process_registration_finish(registration);
        nomo_async_process_event_release(registration);
        nomo_async_process_event_error(
            result,
            "closed",
            "process child is closed"
        );
        registration->kind = NOMO_ASYNC_PROCESS_REGISTRATION_NONE;
        return NOMO_ASYNC_POLL_READY;
    }
    int read_error =
        nomo_async_process_event_collect_read(registration, state, 0u)
        || nomo_async_process_event_collect_read(registration, state, 1u);
    nomo_async_process_registration_finish(registration);
    if (read_error != 0) {
        nomo_async_process_event_release(registration);
        nomo_async_process_event_error(
            result,
            "io",
            "process output read failed"
        );
        context->process_errors += 1u;
        registration->kind = NOMO_ASYNC_PROCESS_REGISTRATION_NONE;
        return NOMO_ASYNC_POLL_READY;
    }
    registration->ready = 0u;
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
    uint64_t remaining = registration->deadline_millis > now
        ? (uint64_t)(registration->deadline_millis - now)
        : 0u;
    if (remaining == 0u) {
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
    if (nomo_async_process_event_arm(
            registration,
            state,
            remaining
        ) != 0) {
        nomo_async_process_event_release(registration);
        nomo_async_process_event_error(
            result,
            "reactor",
            "process IOCP read resubmission failed"
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
        nomo_async_process_registration_finish(registration);
        nomo_async_process_event_release(registration);
        registration->kind = NOMO_ASYNC_PROCESS_REGISTRATION_NONE;
        return NOMO_ASYNC_POLL_READY;
    }
    return NOMO_ASYNC_POLL_PENDING;
}

static int nomo_async_process_stdin_issue(
    nomo_async_process_handle_state *state
);

static void nomo_async_process_stdin_wake(void *owner, uint32_t ready) {
    nomo_async_process_handle_state *state =
        (nomo_async_process_handle_state *)owner;
    if (state == NULL
        || state->occupied == 0u
        || state->stdin_pending == 0u
        || (ready & NOMO_ASYNC_REACTOR_WRITE) == 0u) {
        return;
    }
    DWORD error = state->stdin_registration.error;
    DWORD transferred = state->stdin_registration.transferred;
    nomo_async_reactor_deregister(
        &state->context->reactor,
        &state->stdin_registration
    );
    if (error != ERROR_SUCCESS || transferred == 0u) {
        nomo_async_process_retained_remove(
            state->context,
            state->stdin_len
        );
        free(state->stdin_data);
        state->stdin_data = NULL;
        state->stdin_len = 0u;
        state->stdin_offset = 0u;
        state->stdin_pending = 0u;
        state->stdin_error = 1u;
        state->stdin_closed = 1u;
        nomo_async_process_close_handle(&state->stdin_write);
        nomo_async_process_event_signal(state);
        return;
    }
    state->stdin_offset += (size_t)transferred;
    if (state->stdin_offset < state->stdin_len) {
        if (nomo_async_process_stdin_issue(state) == 0) {
            return;
        }
        nomo_async_process_retained_remove(
            state->context,
            state->stdin_len
        );
        free(state->stdin_data);
        state->stdin_data = NULL;
        state->stdin_len = 0u;
        state->stdin_offset = 0u;
        state->stdin_pending = 0u;
        state->stdin_error = 1u;
        state->stdin_closed = 1u;
        nomo_async_process_close_handle(&state->stdin_write);
        nomo_async_process_event_signal(state);
        return;
    }
    nomo_async_process_retained_remove(
        state->context,
        state->stdin_len
    );
    free(state->stdin_data);
    state->stdin_data = NULL;
    state->stdin_len = 0u;
    state->stdin_offset = 0u;
    state->stdin_pending = 0u;
    state->stdin_flushed = 1u;
    nomo_async_process_event_signal(state);
}

static int nomo_async_process_stdin_issue(
    nomo_async_process_handle_state *state
) {
    nomo_async_reactor_registration *io = &state->stdin_registration;
    io->owner = state;
    io->wake = nomo_async_process_stdin_wake;
    int registered = io->active == 0u
        ? nomo_async_reactor_register(
            &state->context->reactor,
            io,
            (nomo_socket)state->stdin_write,
            NOMO_ASYNC_REACTOR_WRITE
        )
        : nomo_async_reactor_reregister(
            &state->context->reactor,
            io,
            NOMO_ASYNC_REACTOR_WRITE
        );
    if (registered != 0) {
        return 1;
    }
    size_t remaining = state->stdin_len - state->stdin_offset;
    DWORD transferred = 0u;
    BOOL started = WriteFile(
        state->stdin_write,
        state->stdin_data + state->stdin_offset,
        (DWORD)remaining,
        &transferred,
        nomo_async_reactor_overlapped(io)
    );
    DWORD error = started != FALSE ? ERROR_SUCCESS : GetLastError();
    if (started == FALSE && error != ERROR_IO_PENDING) {
        nomo_async_reactor_deregister(&state->context->reactor, io);
        return 1;
    }
    nomo_async_reactor_mark_submitted(io);
    return 0;
}

static void nomo_async_process_cancel(
    nomo_async_process_registration *registration,
    nomo_async_context *context
) {
    if (registration == NULL
        || registration->kind == NOMO_ASYNC_PROCESS_REGISTRATION_NONE) {
        return;
    }
    nomo_async_process_runtime *runtime =
        (nomo_async_process_runtime *)context->process_runtime;
    if (registration->kind == NOMO_ASYNC_PROCESS_REGISTRATION_START) {
        nomo_async_process_registration_finish(registration);
        nomo_async_process_pool_cancel_start(
            runtime,
            registration->job_slot,
            registration->job_generation
        );
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
        || state->stdin_write == NULL
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
    if (nomo_async_process_stdin_issue(state) != 0) {
        nomo_async_process_retained_remove(runtime->context, length);
        free(state->stdin_data);
        state->stdin_data = NULL;
        state->stdin_len = 0u;
        state->stdin_pending = 0u;
        @VOID_RESULT@ result;
        nomo_async_process_void_error(
            &result,
            "reactor",
            "process stdin IOCP submission failed"
        );
        runtime->context->process_errors += 1u;
        return result;
    }
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
        nomo_async_process_close_handle(&state->stdin_write);
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
        && !TerminateProcess(state->process, 137u)) {
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
    EnterCriticalSection(&pool->lock);
    pool->stopping = 1u;
    WakeAllConditionVariable(&pool->available);
    LeaveCriticalSection(&pool->lock);
    WaitForSingleObject(pool->worker, INFINITE);
    CloseHandle(pool->worker);

    for (uint32_t index = 0u;
         index < NOMO_ASYNC_PROCESS_HANDLE_CAPACITY;
         index += 1u) {
        nomo_async_process_handle_state *state = &runtime->handles[index];
        if (state->occupied == 0u) {
            continue;
        }
        if (state->process != NULL
            && WaitForSingleObject(state->process, 0u) == WAIT_TIMEOUT) {
            TerminateProcess(state->process, 137u);
            WaitForSingleObject(state->process, INFINITE);
        }
        if (state->wait_handle != NULL) {
            UnregisterWaitEx(state->wait_handle, INVALID_HANDLE_VALUE);
            state->wait_handle = NULL;
        }
        nomo_async_process_handle_storage_release(runtime, state);
    }
    for (uint32_t index = 0u;
         index < NOMO_ASYNC_PROCESS_JOB_CAPACITY;
         index += 1u) {
        nomo_async_process_job *job = &pool->jobs[index];
        if (job->state == NOMO_ASYNC_PROCESS_JOB_FREE) {
            continue;
        }
        nomo_async_process_spawn_cleanup(
            job->process,
            job->stdin_write,
            job->stdout_read,
            job->stderr_read
        );
        nomo_async_process_command_release(&job->command);
    }
    nomo_async_reactor_post_deactivate(
        &context->reactor,
        &pool->completion_registration
    );
    DeleteCriticalSection(&pool->lock);
    context->blocking_threads_retired += 1u;
    if (context->live_blocking_threads > 0u) {
        context->live_blocking_threads -= 1u;
    }
    context->live_blocking_jobs = 0u;
    context->live_process_handles = 0u;
    context->live_process_operations = 0u;
    context->retained_process_bytes = 0u;
    free(pool);
    free(runtime);
    context->process_runtime = NULL;
}
