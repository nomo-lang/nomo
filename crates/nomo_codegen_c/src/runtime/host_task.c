#ifdef _WIN32
#include <windows.h>
#else
#include <pthread.h>
#endif

#define NOMO_TASK_MAX_LIVE 64U
#define NOMO_TASK_MAX_MESSAGE_BYTES (UINT64_C(8) * UINT64_C(1024) * UINT64_C(1024))
#define NOMO_TASK_MAX_JOIN_TIMEOUT_MS UINT64_C(900000)

typedef nomo_string (*nomo_task_worker_fn)(@CONTEXT@, nomo_string);

typedef struct nomo_task_state {
    uint64_t handle;
    nomo_task_worker_fn worker;
    char *input;
    size_t input_len;
    char *output;
    size_t output_len;
    int cancel_requested;
    int finished;
    int outcome;
#ifdef _WIN32
    HANDLE thread;
    CONDITION_VARIABLE condition;
#else
    pthread_t thread;
    pthread_cond_t condition;
#endif
    struct nomo_task_state *next;
} nomo_task_state;

#ifdef _WIN32
static SRWLOCK nomo_task_registry_lock = SRWLOCK_INIT;
#define NOMO_TASK_LOCK() AcquireSRWLockExclusive(&nomo_task_registry_lock)
#define NOMO_TASK_UNLOCK() ReleaseSRWLockExclusive(&nomo_task_registry_lock)
#else
static pthread_mutex_t nomo_task_registry_lock = PTHREAD_MUTEX_INITIALIZER;
#define NOMO_TASK_LOCK() ((void)pthread_mutex_lock(&nomo_task_registry_lock))
#define NOMO_TASK_UNLOCK() ((void)pthread_mutex_unlock(&nomo_task_registry_lock))
#endif

static nomo_task_state *nomo_task_registry = NULL;
static uint64_t nomo_task_next_handle = UINT64_C(1);
static size_t nomo_task_live_count = 0;

static @ERROR@ nomo_task_error(const char *code, const char *message) {
    @ERROR@ error;
    error.@CODE_MEMBER@ = nomo_string_literal(code);
    error.@MESSAGE_MEMBER@ = nomo_string_literal(message);
    return error;
}

static @RESULT_TASK@ nomo_task_spawn_error(const char *code, const char *message) {
    @RESULT_TASK@ result;
    memset(&result, 0, sizeof(result));
    result.tag = @RESULT_TASK_ERR@;
    result.payload.@ERR_PAYLOAD@ = nomo_task_error(code, message);
    return result;
}

static @RESULT_JOIN@ nomo_task_join_error(const char *code, const char *message) {
    @RESULT_JOIN@ result;
    memset(&result, 0, sizeof(result));
    result.tag = @RESULT_JOIN_ERR@;
    result.payload.@ERR_PAYLOAD@ = nomo_task_error(code, message);
    return result;
}

static @RESULT_VOID@ nomo_task_void_error(const char *code, const char *message) {
    @RESULT_VOID@ result;
    memset(&result, 0, sizeof(result));
    result.tag = @RESULT_VOID_ERR@;
    result.payload.@ERR_PAYLOAD@ = nomo_task_error(code, message);
    return result;
}

static @RESULT_VOID@ nomo_task_void_ok(void) {
    @RESULT_VOID@ result;
    memset(&result, 0, sizeof(result));
    result.tag = @RESULT_VOID_OK@;
    return result;
}

static nomo_task_state *nomo_task_find_locked(uint64_t handle) {
    nomo_task_state *state = nomo_task_registry;
    while (state != NULL) {
        if (state->handle == handle) {
            return state;
        }
        state = state->next;
    }
    return NULL;
}

static void nomo_task_remove_locked(nomo_task_state *target) {
    nomo_task_state **link = &nomo_task_registry;
    while (*link != NULL) {
        if (*link == target) {
            *link = target->next;
            target->next = NULL;
            if (nomo_task_live_count > 0) {
                nomo_task_live_count -= 1;
            }
            return;
        }
        link = &(*link)->next;
    }
}

#ifdef _WIN32
static DWORD WINAPI nomo_task_thread_main(LPVOID raw_state)
#else
static void *nomo_task_thread_main(void *raw_state)
#endif
{
    nomo_task_state *state = (nomo_task_state *)raw_state;
    @CONTEXT@ context;
    context.@HANDLE_MEMBER@ = state->handle;
    nomo_string input = nomo_string_from_slice(state->input, 0, state->input_len);
    nomo_string output = state->worker(context, input);
    size_t output_len = strlen(output.data);
    char *output_copy = NULL;
    int outcome = 0;
    if ((uint64_t)output_len > NOMO_TASK_MAX_MESSAGE_BYTES) {
        outcome = 2;
    } else {
        output_copy = (char *)malloc(output_len + 1);
        if (output_copy == NULL) {
            outcome = 3;
        } else {
            memcpy(output_copy, output.data, output_len + 1);
        }
    }
    nomo_string_release(output);
    nomo_string_release(input);
    free(state->input);
    state->input = NULL;

    NOMO_TASK_LOCK();
    if (state->cancel_requested) {
        free(output_copy);
        output_copy = NULL;
        outcome = 1;
    }
    state->output = output_copy;
    state->output_len = output_len;
    state->outcome = outcome;
    state->finished = 1;
#ifdef _WIN32
    WakeAllConditionVariable(&state->condition);
#else
    (void)pthread_cond_broadcast(&state->condition);
#endif
    NOMO_TASK_UNLOCK();

#ifdef _WIN32
    return 0;
#else
    return NULL;
#endif
}

static @RESULT_TASK@ @SPAWN_NAME@(nomo_task_worker_fn worker, nomo_string input) {
    if (worker == NULL) {
        return nomo_task_spawn_error("invalid_argument", "task worker is null");
    }
    size_t input_len = strlen(input.data);
    if ((uint64_t)input_len > NOMO_TASK_MAX_MESSAGE_BYTES) {
        return nomo_task_spawn_error("limit", "task input exceeds 8 MiB");
    }

    char *input_copy = (char *)malloc(input_len + 1);
    if (input_copy == NULL) {
        return nomo_task_spawn_error("resource", "could not allocate task input");
    }
    memcpy(input_copy, input.data, input_len + 1);

    nomo_task_state *state = (nomo_task_state *)calloc(1, sizeof(nomo_task_state));
    if (state == NULL) {
        free(input_copy);
        return nomo_task_spawn_error("resource", "could not allocate task state");
    }
    state->worker = worker;
    state->input = input_copy;
    state->input_len = input_len;
#ifdef _WIN32
    InitializeConditionVariable(&state->condition);
#else
    if (pthread_cond_init(&state->condition, NULL) != 0) {
        free(input_copy);
        free(state);
        return nomo_task_spawn_error("spawn", "could not initialize task state");
    }
#endif

    NOMO_TASK_LOCK();
    if (nomo_task_live_count >= NOMO_TASK_MAX_LIVE) {
        NOMO_TASK_UNLOCK();
#ifndef _WIN32
        (void)pthread_cond_destroy(&state->condition);
#endif
        free(input_copy);
        free(state);
        return nomo_task_spawn_error("limit", "at most 64 live tasks are allowed");
    }
    if (nomo_task_next_handle == 0) {
        NOMO_TASK_UNLOCK();
#ifndef _WIN32
        (void)pthread_cond_destroy(&state->condition);
#endif
        free(input_copy);
        free(state);
        return nomo_task_spawn_error("limit", "task handle space is exhausted");
    }
    state->handle = nomo_task_next_handle++;
    state->next = nomo_task_registry;
    nomo_task_registry = state;
    nomo_task_live_count += 1;
    NOMO_TASK_UNLOCK();

@HTTP_PREFLIGHT@

#ifdef _WIN32
    state->thread = CreateThread(NULL, 0, nomo_task_thread_main, state, 0, NULL);
    if (state->thread == NULL) {
#else
    if (pthread_create(&state->thread, NULL, nomo_task_thread_main, state) != 0) {
#endif
        NOMO_TASK_LOCK();
        nomo_task_remove_locked(state);
        NOMO_TASK_UNLOCK();
#ifndef _WIN32
        (void)pthread_cond_destroy(&state->condition);
#endif
        free(input_copy);
        free(state);
        return nomo_task_spawn_error("spawn", "could not start native task");
    }

    @TASK@ task;
    task.@HANDLE_MEMBER@ = state->handle;
    @RESULT_TASK@ result;
    memset(&result, 0, sizeof(result));
    result.tag = @RESULT_TASK_OK@;
    result.payload.@OK_PAYLOAD@ = task;
    return result;
}

static int @IS_CANCELLED_NAME@(@CONTEXT@ context) {
    NOMO_TASK_LOCK();
    nomo_task_state *state = nomo_task_find_locked(context.@HANDLE_MEMBER@);
    int cancelled = state != NULL && state->cancel_requested;
    NOMO_TASK_UNLOCK();
    return cancelled;
}

static @RESULT_JOIN@ nomo_task_join_timeout(void) {
    @JOIN@ join;
    memset(&join, 0, sizeof(join));
    join.tag = @JOIN_TIMEOUT@;
    @RESULT_JOIN@ result;
    memset(&result, 0, sizeof(result));
    result.tag = @RESULT_JOIN_OK@;
    result.payload.@OK_PAYLOAD@ = join;
    return result;
}

static @RESULT_JOIN@ @JOIN_NAME@(@TASK@ task, uint64_t timeout_millis) {
    if (timeout_millis > NOMO_TASK_MAX_JOIN_TIMEOUT_MS) {
        return nomo_task_join_error("invalid_argument", "task join timeout exceeds 900000 ms");
    }
    NOMO_TASK_LOCK();
    nomo_task_state *state = nomo_task_find_locked(task.@HANDLE_MEMBER@);
    if (state == NULL) {
        NOMO_TASK_UNLOCK();
        return nomo_task_join_error("closed", "task handle is closed or invalid");
    }
    if (!state->finished && timeout_millis == 0) {
        NOMO_TASK_UNLOCK();
        return nomo_task_join_timeout();
    }

#ifdef _WIN32
    if (!state->finished) {
        ULONGLONG started_at = GetTickCount64();
        DWORD remaining = (DWORD)timeout_millis;
        while (!state->finished) {
            BOOL waited = SleepConditionVariableSRW(
                &state->condition,
                &nomo_task_registry_lock,
                remaining,
                0
            );
            if (!waited) {
                DWORD wait_error = GetLastError();
                if (wait_error == ERROR_TIMEOUT && !state->finished) {
                    NOMO_TASK_UNLOCK();
                    return nomo_task_join_timeout();
                }
                if (wait_error != ERROR_TIMEOUT) {
                    NOMO_TASK_UNLOCK();
                    return nomo_task_join_error("wait", "could not wait for task");
                }
            }
            ULONGLONG elapsed = GetTickCount64() - started_at;
            if (!state->finished && elapsed >= timeout_millis) {
                NOMO_TASK_UNLOCK();
                return nomo_task_join_timeout();
            }
            remaining = (DWORD)(timeout_millis - elapsed);
        }
    }
#else
    if (!state->finished) {
        struct timespec deadline;
        if (clock_gettime(CLOCK_REALTIME, &deadline) != 0) {
            NOMO_TASK_UNLOCK();
            return nomo_task_join_error("clock", "could not read task join clock");
        }
        deadline.tv_sec += (time_t)(timeout_millis / UINT64_C(1000));
        deadline.tv_nsec += (long)((timeout_millis % UINT64_C(1000)) * UINT64_C(1000000));
        if (deadline.tv_nsec >= 1000000000L) {
            deadline.tv_sec += 1;
            deadline.tv_nsec -= 1000000000L;
        }
        while (!state->finished) {
            int wait_result =
                pthread_cond_timedwait(&state->condition, &nomo_task_registry_lock, &deadline);
            if (wait_result == ETIMEDOUT && !state->finished) {
                NOMO_TASK_UNLOCK();
                return nomo_task_join_timeout();
            }
            if (wait_result != 0 && wait_result != EINTR) {
                NOMO_TASK_UNLOCK();
                return nomo_task_join_error("wait", "could not wait for task");
            }
        }
    }
#endif
    if (!state->finished) {
        NOMO_TASK_UNLOCK();
        return nomo_task_join_timeout();
    }

    int outcome = state->outcome;
    const char *output = state->output;
    size_t output_len = state->output_len;
    @RESULT_JOIN@ result;
    memset(&result, 0, sizeof(result));
    if (outcome == 2) {
        NOMO_TASK_UNLOCK();
        return nomo_task_join_error("limit", "task output exceeds 8 MiB");
    }
    if (outcome == 3) {
        NOMO_TASK_UNLOCK();
        return nomo_task_join_error("resource", "could not copy task output");
    }
    @JOIN@ join;
    memset(&join, 0, sizeof(join));
    if (outcome == 1) {
        join.tag = @JOIN_CANCELLED@;
    } else {
        join.tag = @JOIN_COMPLETED@;
        join.payload.@COMPLETED_PAYLOAD@ = nomo_string_from_slice(output, 0, output_len);
    }
    result.tag = @RESULT_JOIN_OK@;
    result.payload.@OK_PAYLOAD@ = join;
    NOMO_TASK_UNLOCK();
    return result;
}

static @RESULT_VOID@ @CANCEL_NAME@(@TASK@ task) {
    NOMO_TASK_LOCK();
    nomo_task_state *state = nomo_task_find_locked(task.@HANDLE_MEMBER@);
    if (state == NULL) {
        NOMO_TASK_UNLOCK();
        return nomo_task_void_error("closed", "task handle is closed or invalid");
    }
    if (!state->finished) {
        state->cancel_requested = 1;
    }
    NOMO_TASK_UNLOCK();
    return nomo_task_void_ok();
}

static @RESULT_VOID@ @CLOSE_NAME@(@TASK@ task) {
    NOMO_TASK_LOCK();
    nomo_task_state *state = nomo_task_find_locked(task.@HANDLE_MEMBER@);
    if (state == NULL) {
        NOMO_TASK_UNLOCK();
        return nomo_task_void_error("closed", "task handle is closed or invalid");
    }
    if (!state->finished) {
        NOMO_TASK_UNLOCK();
        return nomo_task_void_error("busy", "task is still running");
    }
    nomo_task_remove_locked(state);
    NOMO_TASK_UNLOCK();

#ifdef _WIN32
    (void)WaitForSingleObject(state->thread, INFINITE);
    (void)CloseHandle(state->thread);
#else
    (void)pthread_join(state->thread, NULL);
    (void)pthread_cond_destroy(&state->condition);
#endif
    free(state->input);
    free(state->output);
    free(state);
    return nomo_task_void_ok();
}

static void nomo_task_shutdown(void) {
    size_t live = 0;
    NOMO_TASK_LOCK();
    nomo_task_state *state = nomo_task_registry;
    while (state != NULL) {
        if (!state->finished) {
            state->cancel_requested = 1;
        }
        live += 1;
        state = state->next;
    }
    NOMO_TASK_UNLOCK();
    if (live != 0) {
        fputs("warning: process exiting with live Nomo tasks\n", stderr);
    }
}
