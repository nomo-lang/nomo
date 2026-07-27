pub(super) fn emit_async_resolver_pool_helpers(
    out: &mut String,
    target: &nomo_target::TargetTriple,
) {
    if target.operating_system() == nomo_target::OperatingSystem::Windows {
        emit_async_resolver_pool_windows_helpers(out);
    } else {
        emit_async_resolver_pool_unix_helpers(out);
    }
}

fn emit_async_resolver_pool_unix_helpers(out: &mut String) {
    out.push_str(
        r#"#include <pthread.h>

#define NOMO_ASYNC_BLOCKING_POOL_QUEUE_CAPACITY 16u
#define NOMO_ASYNC_BLOCKING_POOL_MAX_THREADS 1u
#define NOMO_ASYNC_RESOLVER_MAX_ADDRESSES 16u
#define NOMO_ASYNC_RESOLVER_HOST_CAPACITY 254u

typedef void (*nomo_async_blocking_completion_fn)(
    void *,
    uint32_t,
    uint32_t
);

typedef enum {
    NOMO_ASYNC_BLOCKING_JOB_FREE = 0,
    NOMO_ASYNC_BLOCKING_JOB_QUEUED = 1,
    NOMO_ASYNC_BLOCKING_JOB_RUNNING = 2,
    NOMO_ASYNC_BLOCKING_JOB_COMPLETED_QUEUED = 3,
    NOMO_ASYNC_BLOCKING_JOB_COMPLETED_DELIVERED = 4,
    NOMO_ASYNC_BLOCKING_JOB_CANCELLED_RUNNING = 5,
    NOMO_ASYNC_BLOCKING_JOB_CANCELLED_QUEUED = 6
} nomo_async_blocking_job_state;

typedef struct {
    uint32_t generation;
    nomo_async_blocking_job_state state;
    char host[NOMO_ASYNC_RESOLVER_HOST_CAPACITY];
    char port[16];
    struct sockaddr_storage addresses[NOMO_ASYNC_RESOLVER_MAX_ADDRESSES];
    socklen_t address_lengths[NOMO_ASYNC_RESOLVER_MAX_ADDRESSES];
    uint32_t address_count;
    int resolver_status;
    void *owner;
    nomo_async_blocking_completion_fn complete;
} nomo_async_resolver_job;

typedef struct {
    nomo_async_context *context;
    pthread_mutex_t mutex;
    pthread_cond_t available;
    pthread_t worker;
    int wake_read;
    int wake_write;
    nomo_async_reactor_registration wake_registration;
    nomo_async_resolver_job jobs[NOMO_ASYNC_BLOCKING_POOL_QUEUE_CAPACITY];
    uint32_t queue[NOMO_ASYNC_BLOCKING_POOL_QUEUE_CAPACITY];
    uint32_t completions[NOMO_ASYNC_BLOCKING_POOL_QUEUE_CAPACITY];
    uint32_t queue_head;
    uint32_t queue_tail;
    uint32_t queue_count;
    uint32_t completion_head;
    uint32_t completion_tail;
    uint32_t completion_count;
    uint32_t next_generation;
    uint32_t active_jobs;
    uint8_t stopping;
} nomo_async_resolver_pool;

static void nomo_async_resolver_pool_maybe_idle(
    nomo_async_resolver_pool *pool
) {
    uint32_t active_jobs = 0u;
    if (pthread_mutex_lock(&pool->mutex) != 0) {
        pool->context->runtime_failed = 1u;
        return;
    }
    active_jobs = pool->active_jobs;
    pthread_mutex_unlock(&pool->mutex);
    if (active_jobs == 0u && pool->wake_registration.active != 0u) {
        nomo_async_reactor_deregister(
            &pool->context->reactor,
            &pool->wake_registration
        );
    }
}

static void nomo_async_resolver_pool_release_job_locked(
    nomo_async_resolver_pool *pool,
    nomo_async_resolver_job *job
) {
    memset(job->host, 0, sizeof(job->host));
    memset(job->port, 0, sizeof(job->port));
    memset(job->addresses, 0, sizeof(job->addresses));
    memset(job->address_lengths, 0, sizeof(job->address_lengths));
    job->address_count = 0u;
    job->resolver_status = 0;
    job->owner = NULL;
    job->complete = NULL;
    job->state = NOMO_ASYNC_BLOCKING_JOB_FREE;
    if (pool->active_jobs > 0u) {
        pool->active_jobs -= 1u;
    }
    if (pool->context->live_blocking_jobs > 0u) {
        pool->context->live_blocking_jobs -= 1u;
    }
}

static int nomo_async_resolver_pool_remove_queued_locked(
    nomo_async_resolver_pool *pool,
    uint32_t selected
) {
    uint32_t kept[NOMO_ASYNC_BLOCKING_POOL_QUEUE_CAPACITY];
    uint32_t kept_count = 0u;
    uint8_t removed = 0u;
    while (pool->queue_count > 0u) {
        uint32_t slot = pool->queue[pool->queue_head];
        pool->queue_head =
            (pool->queue_head + 1u) % NOMO_ASYNC_BLOCKING_POOL_QUEUE_CAPACITY;
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
            (pool->queue_tail + 1u) % NOMO_ASYNC_BLOCKING_POOL_QUEUE_CAPACITY;
        pool->queue_count += 1u;
    }
    return removed == 0u;
}

static void *nomo_async_resolver_worker(void *raw_pool) {
    nomo_async_resolver_pool *pool = (nomo_async_resolver_pool *)raw_pool;
    while (1) {
        if (pthread_mutex_lock(&pool->mutex) != 0) {
            return NULL;
        }
        while (pool->queue_count == 0u && pool->stopping == 0u) {
            if (pthread_cond_wait(&pool->available, &pool->mutex) != 0) {
                pthread_mutex_unlock(&pool->mutex);
                return NULL;
            }
        }
        if (pool->stopping != 0u && pool->queue_count == 0u) {
            pthread_mutex_unlock(&pool->mutex);
            return NULL;
        }
        uint32_t slot = pool->queue[pool->queue_head];
        pool->queue_head =
            (pool->queue_head + 1u) % NOMO_ASYNC_BLOCKING_POOL_QUEUE_CAPACITY;
        pool->queue_count -= 1u;
        nomo_async_resolver_job *job = &pool->jobs[slot];
        if (job->state != NOMO_ASYNC_BLOCKING_JOB_QUEUED) {
            pthread_mutex_unlock(&pool->mutex);
            continue;
        }
        job->state = NOMO_ASYNC_BLOCKING_JOB_RUNNING;
        char host[NOMO_ASYNC_RESOLVER_HOST_CAPACITY];
        char port[16];
        memcpy(host, job->host, sizeof(host));
        memcpy(port, job->port, sizeof(port));
        pthread_mutex_unlock(&pool->mutex);

#ifdef NOMO_ASYNC_RESOLVER_TEST_DELAY_MILLIS
        struct timespec resolver_test_delay = {
            .tv_sec = NOMO_ASYNC_RESOLVER_TEST_DELAY_MILLIS / 1000,
            .tv_nsec =
                (NOMO_ASYNC_RESOLVER_TEST_DELAY_MILLIS % 1000) * 1000000L
        };
        nanosleep(&resolver_test_delay, NULL);
#endif
        struct addrinfo hints;
        memset(&hints, 0, sizeof(hints));
        hints.ai_family = AF_UNSPEC;
        hints.ai_socktype = SOCK_STREAM;
        struct addrinfo *addresses = NULL;
        int resolver_status = getaddrinfo(host, port, &hints, &addresses);
        struct sockaddr_storage resolved[NOMO_ASYNC_RESOLVER_MAX_ADDRESSES];
        socklen_t resolved_lengths[NOMO_ASYNC_RESOLVER_MAX_ADDRESSES];
        memset(resolved, 0, sizeof(resolved));
        memset(resolved_lengths, 0, sizeof(resolved_lengths));
        uint32_t resolved_count = 0u;
        if (resolver_status == 0) {
            for (struct addrinfo *address = addresses;
                 address != NULL
                    && resolved_count < NOMO_ASYNC_RESOLVER_MAX_ADDRESSES;
                 address = address->ai_next) {
                if ((address->ai_family != AF_INET
                        && address->ai_family != AF_INET6)
                    || address->ai_addr == NULL
                    || address->ai_addrlen > sizeof(struct sockaddr_storage)) {
                    continue;
                }
                memcpy(
                    &resolved[resolved_count],
                    address->ai_addr,
                    address->ai_addrlen
                );
                resolved_lengths[resolved_count] =
                    (socklen_t)address->ai_addrlen;
                resolved_count += 1u;
            }
        }
        if (addresses != NULL) {
            freeaddrinfo(addresses);
        }

        if (pthread_mutex_lock(&pool->mutex) != 0) {
            return NULL;
        }
        job = &pool->jobs[slot];
        if (job->state == NOMO_ASYNC_BLOCKING_JOB_RUNNING) {
            memcpy(job->addresses, resolved, sizeof(resolved));
            memcpy(
                job->address_lengths,
                resolved_lengths,
                sizeof(resolved_lengths)
            );
            job->address_count = resolved_count;
            job->resolver_status = resolver_status;
            job->state = NOMO_ASYNC_BLOCKING_JOB_COMPLETED_QUEUED;
        } else if (job->state == NOMO_ASYNC_BLOCKING_JOB_CANCELLED_RUNNING) {
            job->state = NOMO_ASYNC_BLOCKING_JOB_CANCELLED_QUEUED;
        }
        if (pool->completion_count
            == NOMO_ASYNC_BLOCKING_POOL_QUEUE_CAPACITY) {
            pthread_mutex_unlock(&pool->mutex);
            return NULL;
        }
        pool->completions[pool->completion_tail] = slot;
        pool->completion_tail =
            (pool->completion_tail + 1u)
            % NOMO_ASYNC_BLOCKING_POOL_QUEUE_CAPACITY;
        pool->completion_count += 1u;
        pthread_mutex_unlock(&pool->mutex);

        unsigned char signal = 1u;
        ssize_t ignored = write(pool->wake_write, &signal, sizeof(signal));
        (void)ignored;
    }
}

static void nomo_async_resolver_pool_completion_wake(
    void *raw_pool,
    uint32_t ready
) {
    nomo_async_resolver_pool *pool = (nomo_async_resolver_pool *)raw_pool;
    if (pool == NULL || (ready & NOMO_ASYNC_REACTOR_READ) == 0u) {
        return;
    }
    unsigned char signals[32];
    while (read(pool->wake_read, signals, sizeof(signals)) > 0) {
    }
    while (1) {
        void *owner = NULL;
        nomo_async_blocking_completion_fn complete = NULL;
        uint32_t slot = 0u;
        uint32_t generation = 0u;
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
            % NOMO_ASYNC_BLOCKING_POOL_QUEUE_CAPACITY;
        pool->completion_count -= 1u;
        nomo_async_resolver_job *job = &pool->jobs[slot];
        pool->context->blocking_jobs_started += 1u;
        pool->context->blocking_jobs_completed += 1u;
        if (job->state == NOMO_ASYNC_BLOCKING_JOB_COMPLETED_QUEUED) {
            job->state = NOMO_ASYNC_BLOCKING_JOB_COMPLETED_DELIVERED;
            owner = job->owner;
            complete = job->complete;
            generation = job->generation;
        } else if (job->state == NOMO_ASYNC_BLOCKING_JOB_CANCELLED_QUEUED) {
            nomo_async_resolver_pool_release_job_locked(pool, job);
        }
        pthread_mutex_unlock(&pool->mutex);
        if (complete != NULL) {
            complete(owner, slot, generation);
        }
    }
    uint32_t active_jobs = 0u;
    if (pthread_mutex_lock(&pool->mutex) != 0) {
        pool->context->runtime_failed = 1u;
        return;
    }
    active_jobs = pool->active_jobs;
    pthread_mutex_unlock(&pool->mutex);
    if (active_jobs == 0u) {
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

static int nomo_async_resolver_pool_initialize(nomo_async_context *context) {
    if (context->blocking_pool != NULL) {
        return 0;
    }
    nomo_async_resolver_pool *pool =
        (nomo_async_resolver_pool *)calloc(1u, sizeof(*pool));
    if (pool == NULL) {
        return 1;
    }
    pool->context = context;
    pool->wake_read = -1;
    pool->wake_write = -1;
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
    if (pipe(wake_pipe) != 0
        || fcntl(wake_pipe[0], F_SETFL, O_NONBLOCK) != 0
        || fcntl(wake_pipe[1], F_SETFL, O_NONBLOCK) != 0
        || fcntl(wake_pipe[0], F_SETFD, FD_CLOEXEC) != 0
        || fcntl(wake_pipe[1], F_SETFD, FD_CLOEXEC) != 0) {
        if (wake_pipe[0] >= 0) {
            close(wake_pipe[0]);
        }
        if (wake_pipe[1] >= 0) {
            close(wake_pipe[1]);
        }
        pthread_cond_destroy(&pool->available);
        pthread_mutex_destroy(&pool->mutex);
        free(pool);
        return 1;
    }
    pool->wake_read = wake_pipe[0];
    pool->wake_write = wake_pipe[1];
    pool->wake_registration.owner = pool;
    pool->wake_registration.wake = nomo_async_resolver_pool_completion_wake;
    if (pthread_create(
            &pool->worker,
            NULL,
            nomo_async_resolver_worker,
            pool
        ) != 0) {
        close(pool->wake_read);
        close(pool->wake_write);
        pthread_cond_destroy(&pool->available);
        pthread_mutex_destroy(&pool->mutex);
        free(pool);
        return 1;
    }
    context->blocking_pool = pool;
    context->blocking_pool_initializations += 1u;
    context->blocking_threads_started += 1u;
    context->live_blocking_threads = 1u;
    context->peak_live_blocking_threads = 1u;
    return 0;
}

static int nomo_async_resolver_pool_ensure_wake_registration(
    nomo_async_resolver_pool *pool
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

"#,
    );
    emit_async_resolver_pool_unix_tail(out);
}

fn emit_async_resolver_pool_windows_helpers(out: &mut String) {
    out.push_str(
        r#"#define NOMO_ASYNC_BLOCKING_POOL_QUEUE_CAPACITY 16u
#define NOMO_ASYNC_BLOCKING_POOL_MAX_THREADS 1u
#define NOMO_ASYNC_RESOLVER_MAX_ADDRESSES 16u
#define NOMO_ASYNC_RESOLVER_HOST_CAPACITY 254u

typedef void (*nomo_async_blocking_completion_fn)(
    void *,
    uint32_t,
    uint32_t
);

typedef enum {
    NOMO_ASYNC_BLOCKING_JOB_FREE = 0,
    NOMO_ASYNC_BLOCKING_JOB_QUEUED = 1,
    NOMO_ASYNC_BLOCKING_JOB_RUNNING = 2,
    NOMO_ASYNC_BLOCKING_JOB_COMPLETED_QUEUED = 3,
    NOMO_ASYNC_BLOCKING_JOB_COMPLETED_DELIVERED = 4,
    NOMO_ASYNC_BLOCKING_JOB_CANCELLED_RUNNING = 5,
    NOMO_ASYNC_BLOCKING_JOB_CANCELLED_QUEUED = 6
} nomo_async_blocking_job_state;

typedef struct {
    uint32_t generation;
    nomo_async_blocking_job_state state;
    char host[NOMO_ASYNC_RESOLVER_HOST_CAPACITY];
    char port[16];
    struct sockaddr_storage addresses[NOMO_ASYNC_RESOLVER_MAX_ADDRESSES];
    int address_lengths[NOMO_ASYNC_RESOLVER_MAX_ADDRESSES];
    uint32_t address_count;
    int resolver_status;
    void *owner;
    nomo_async_blocking_completion_fn complete;
} nomo_async_resolver_job;

typedef struct {
    nomo_async_context *context;
    CRITICAL_SECTION mutex;
    CONDITION_VARIABLE available;
    HANDLE worker;
    nomo_async_reactor_registration wake_registration;
    nomo_async_resolver_job jobs[NOMO_ASYNC_BLOCKING_POOL_QUEUE_CAPACITY];
    uint32_t queue[NOMO_ASYNC_BLOCKING_POOL_QUEUE_CAPACITY];
    uint32_t completions[NOMO_ASYNC_BLOCKING_POOL_QUEUE_CAPACITY];
    uint32_t queue_head;
    uint32_t queue_tail;
    uint32_t queue_count;
    uint32_t completion_head;
    uint32_t completion_tail;
    uint32_t completion_count;
    uint32_t pending_posts;
    uint32_t next_generation;
    uint32_t active_jobs;
    uint8_t stopping;
} nomo_async_resolver_pool;

static void nomo_async_resolver_pool_maybe_idle(
    nomo_async_resolver_pool *pool
) {
    uint32_t active_jobs = 0u;
    EnterCriticalSection(&pool->mutex);
    active_jobs = pool->active_jobs;
    LeaveCriticalSection(&pool->mutex);
    if (active_jobs == 0u) {
        nomo_async_reactor_post_deactivate(
            &pool->context->reactor,
            &pool->wake_registration
        );
    }
}

static void nomo_async_resolver_pool_release_job_locked(
    nomo_async_resolver_pool *pool,
    nomo_async_resolver_job *job
) {
    memset(job->host, 0, sizeof(job->host));
    memset(job->port, 0, sizeof(job->port));
    memset(job->addresses, 0, sizeof(job->addresses));
    memset(job->address_lengths, 0, sizeof(job->address_lengths));
    job->address_count = 0u;
    job->resolver_status = 0;
    job->owner = NULL;
    job->complete = NULL;
    job->state = NOMO_ASYNC_BLOCKING_JOB_FREE;
    if (pool->active_jobs > 0u) {
        pool->active_jobs -= 1u;
    }
    if (pool->context->live_blocking_jobs > 0u) {
        pool->context->live_blocking_jobs -= 1u;
    }
}

static int nomo_async_resolver_pool_remove_queued_locked(
    nomo_async_resolver_pool *pool,
    uint32_t selected
) {
    uint32_t kept[NOMO_ASYNC_BLOCKING_POOL_QUEUE_CAPACITY];
    uint32_t kept_count = 0u;
    uint8_t removed = 0u;
    while (pool->queue_count > 0u) {
        uint32_t slot = pool->queue[pool->queue_head];
        pool->queue_head =
            (pool->queue_head + 1u) % NOMO_ASYNC_BLOCKING_POOL_QUEUE_CAPACITY;
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
            (pool->queue_tail + 1u) % NOMO_ASYNC_BLOCKING_POOL_QUEUE_CAPACITY;
        pool->queue_count += 1u;
    }
    return removed == 0u;
}

static DWORD WINAPI nomo_async_resolver_worker(LPVOID raw_pool) {
    nomo_async_resolver_pool *pool = (nomo_async_resolver_pool *)raw_pool;
    while (1) {
        EnterCriticalSection(&pool->mutex);
        while (pool->queue_count == 0u && pool->stopping == 0u) {
            if (SleepConditionVariableCS(
                    &pool->available,
                    &pool->mutex,
                    INFINITE
                ) == FALSE) {
                LeaveCriticalSection(&pool->mutex);
                return 1u;
            }
        }
        if (pool->stopping != 0u && pool->queue_count == 0u) {
            LeaveCriticalSection(&pool->mutex);
            return 0u;
        }
        uint32_t slot = pool->queue[pool->queue_head];
        pool->queue_head =
            (pool->queue_head + 1u) % NOMO_ASYNC_BLOCKING_POOL_QUEUE_CAPACITY;
        pool->queue_count -= 1u;
        nomo_async_resolver_job *job = &pool->jobs[slot];
        if (job->state != NOMO_ASYNC_BLOCKING_JOB_QUEUED) {
            LeaveCriticalSection(&pool->mutex);
            continue;
        }
        job->state = NOMO_ASYNC_BLOCKING_JOB_RUNNING;
        char host[NOMO_ASYNC_RESOLVER_HOST_CAPACITY];
        char port[16];
        memcpy(host, job->host, sizeof(host));
        memcpy(port, job->port, sizeof(port));
        LeaveCriticalSection(&pool->mutex);

#ifdef NOMO_ASYNC_RESOLVER_TEST_DELAY_MILLIS
        Sleep(NOMO_ASYNC_RESOLVER_TEST_DELAY_MILLIS);
#endif
        struct addrinfo hints;
        memset(&hints, 0, sizeof(hints));
        hints.ai_family = AF_UNSPEC;
        hints.ai_socktype = SOCK_STREAM;
        struct addrinfo *addresses = NULL;
        int resolver_status = getaddrinfo(host, port, &hints, &addresses);
        struct sockaddr_storage resolved[NOMO_ASYNC_RESOLVER_MAX_ADDRESSES];
        int resolved_lengths[NOMO_ASYNC_RESOLVER_MAX_ADDRESSES];
        memset(resolved, 0, sizeof(resolved));
        memset(resolved_lengths, 0, sizeof(resolved_lengths));
        uint32_t resolved_count = 0u;
        if (resolver_status == 0) {
            for (struct addrinfo *address = addresses;
                 address != NULL
                    && resolved_count < NOMO_ASYNC_RESOLVER_MAX_ADDRESSES;
                 address = address->ai_next) {
                if ((address->ai_family != AF_INET
                        && address->ai_family != AF_INET6)
                    || address->ai_addr == NULL
                    || address->ai_addrlen > sizeof(struct sockaddr_storage)) {
                    continue;
                }
                memcpy(
                    &resolved[resolved_count],
                    address->ai_addr,
                    address->ai_addrlen
                );
                resolved_lengths[resolved_count] = (int)address->ai_addrlen;
                resolved_count += 1u;
            }
        }
        if (addresses != NULL) {
            freeaddrinfo(addresses);
        }

        EnterCriticalSection(&pool->mutex);
        job = &pool->jobs[slot];
        if (job->state == NOMO_ASYNC_BLOCKING_JOB_RUNNING) {
            memcpy(job->addresses, resolved, sizeof(resolved));
            memcpy(
                job->address_lengths,
                resolved_lengths,
                sizeof(resolved_lengths)
            );
            job->address_count = resolved_count;
            job->resolver_status = resolver_status;
            job->state = NOMO_ASYNC_BLOCKING_JOB_COMPLETED_QUEUED;
        } else if (job->state == NOMO_ASYNC_BLOCKING_JOB_CANCELLED_RUNNING) {
            job->state = NOMO_ASYNC_BLOCKING_JOB_CANCELLED_QUEUED;
        }
        if (pool->completion_count
            == NOMO_ASYNC_BLOCKING_POOL_QUEUE_CAPACITY) {
            LeaveCriticalSection(&pool->mutex);
            return 1u;
        }
        pool->completions[pool->completion_tail] = slot;
        pool->completion_tail =
            (pool->completion_tail + 1u)
            % NOMO_ASYNC_BLOCKING_POOL_QUEUE_CAPACITY;
        pool->completion_count += 1u;
        pool->pending_posts += 1u;
        LeaveCriticalSection(&pool->mutex);

        if (nomo_async_reactor_post(
                &pool->context->reactor,
                &pool->wake_registration,
                NOMO_ASYNC_REACTOR_READ
            ) != 0) {
            EnterCriticalSection(&pool->mutex);
            if (pool->pending_posts > 0u) {
                pool->pending_posts -= 1u;
            }
            LeaveCriticalSection(&pool->mutex);
            pool->context->runtime_failed = 1u;
            return 1u;
        }
    }
}

static void nomo_async_resolver_pool_completion_wake(
    void *raw_pool,
    uint32_t ready
) {
    nomo_async_resolver_pool *pool = (nomo_async_resolver_pool *)raw_pool;
    if (pool == NULL || (ready & NOMO_ASYNC_REACTOR_READ) == 0u) {
        return;
    }
    EnterCriticalSection(&pool->mutex);
    if (pool->pending_posts > 0u) {
        pool->pending_posts -= 1u;
    }
    LeaveCriticalSection(&pool->mutex);
    while (1) {
        void *owner = NULL;
        nomo_async_blocking_completion_fn complete = NULL;
        uint32_t slot = 0u;
        uint32_t generation = 0u;
        EnterCriticalSection(&pool->mutex);
        if (pool->completion_count == 0u) {
            LeaveCriticalSection(&pool->mutex);
            break;
        }
        slot = pool->completions[pool->completion_head];
        pool->completion_head =
            (pool->completion_head + 1u)
            % NOMO_ASYNC_BLOCKING_POOL_QUEUE_CAPACITY;
        pool->completion_count -= 1u;
        nomo_async_resolver_job *job = &pool->jobs[slot];
        pool->context->blocking_jobs_started += 1u;
        pool->context->blocking_jobs_completed += 1u;
        if (job->state == NOMO_ASYNC_BLOCKING_JOB_COMPLETED_QUEUED) {
            job->state = NOMO_ASYNC_BLOCKING_JOB_COMPLETED_DELIVERED;
            owner = job->owner;
            complete = job->complete;
            generation = job->generation;
        } else if (job->state == NOMO_ASYNC_BLOCKING_JOB_CANCELLED_QUEUED) {
            nomo_async_resolver_pool_release_job_locked(pool, job);
        }
        LeaveCriticalSection(&pool->mutex);
        if (complete != NULL) {
            complete(owner, slot, generation);
        }
    }
    nomo_async_resolver_pool_maybe_idle(pool);
}

static int nomo_async_resolver_pool_initialize(nomo_async_context *context) {
    if (context->blocking_pool != NULL) {
        return 0;
    }
    if (nomo_async_reactor_init(&context->reactor) != 0) {
        return 1;
    }
    nomo_async_resolver_pool *pool =
        (nomo_async_resolver_pool *)calloc(1u, sizeof(*pool));
    if (pool == NULL) {
        return 1;
    }
    pool->context = context;
    InitializeCriticalSection(&pool->mutex);
    InitializeConditionVariable(&pool->available);
    pool->wake_registration.owner = pool;
    pool->wake_registration.wake = nomo_async_resolver_pool_completion_wake;
    pool->worker = CreateThread(
        NULL,
        0u,
        nomo_async_resolver_worker,
        pool,
        0u,
        NULL
    );
    if (pool->worker == NULL) {
        DeleteCriticalSection(&pool->mutex);
        free(pool);
        return 1;
    }
    context->blocking_pool = pool;
    context->blocking_pool_initializations += 1u;
    context->blocking_threads_started += 1u;
    context->live_blocking_threads = 1u;
    context->peak_live_blocking_threads = 1u;
    return 0;
}

static int nomo_async_resolver_submit(
    nomo_async_context *context,
    const char *host,
    int64_t port,
    void *owner,
    nomo_async_blocking_completion_fn complete,
    uint32_t *slot_out,
    uint32_t *generation_out
) {
    if (nomo_async_resolver_pool_initialize(context) != 0) {
        return 1;
    }
    nomo_async_resolver_pool *pool =
        (nomo_async_resolver_pool *)context->blocking_pool;
    if (nomo_async_reactor_post_activate(
            &context->reactor,
            &pool->wake_registration
        ) != 0) {
        return 1;
    }
    EnterCriticalSection(&pool->mutex);
    uint32_t selected = NOMO_ASYNC_BLOCKING_POOL_QUEUE_CAPACITY;
    for (uint32_t index = 0u;
         index < NOMO_ASYNC_BLOCKING_POOL_QUEUE_CAPACITY;
         index += 1u) {
        if (pool->jobs[index].state == NOMO_ASYNC_BLOCKING_JOB_FREE) {
            selected = index;
            break;
        }
    }
    if (selected == NOMO_ASYNC_BLOCKING_POOL_QUEUE_CAPACITY
        || pool->queue_count == NOMO_ASYNC_BLOCKING_POOL_QUEUE_CAPACITY) {
        LeaveCriticalSection(&pool->mutex);
        context->blocking_queue_saturations += 1u;
        return 2;
    }
    pool->next_generation += 1u;
    if (pool->next_generation == 0u) {
        pool->next_generation = 1u;
    }
    nomo_async_resolver_job *job = &pool->jobs[selected];
    memset(job, 0, sizeof(*job));
    job->generation = pool->next_generation;
    job->state = NOMO_ASYNC_BLOCKING_JOB_QUEUED;
    memcpy(job->host, host, strlen(host) + 1u);
    snprintf(job->port, sizeof(job->port), "%" PRId64, port);
    job->owner = owner;
    job->complete = complete;
    pool->queue[pool->queue_tail] = selected;
    pool->queue_tail =
        (pool->queue_tail + 1u) % NOMO_ASYNC_BLOCKING_POOL_QUEUE_CAPACITY;
    pool->queue_count += 1u;
    pool->active_jobs += 1u;
    *slot_out = selected;
    *generation_out = job->generation;
    WakeConditionVariable(&pool->available);
    LeaveCriticalSection(&pool->mutex);
    context->blocking_jobs_queued += 1u;
    context->live_blocking_jobs += 1u;
    if (context->live_blocking_jobs > context->peak_live_blocking_jobs) {
        context->peak_live_blocking_jobs = context->live_blocking_jobs;
    }
    return 0;
}

static int nomo_async_resolver_take(
    nomo_async_context *context,
    uint32_t slot,
    uint32_t generation,
    struct sockaddr_storage *addresses,
    int *address_lengths,
    uint32_t *address_count,
    int *resolver_status
) {
    nomo_async_resolver_pool *pool =
        (nomo_async_resolver_pool *)context->blocking_pool;
    if (pool == NULL || slot >= NOMO_ASYNC_BLOCKING_POOL_QUEUE_CAPACITY) {
        return 1;
    }
    EnterCriticalSection(&pool->mutex);
    nomo_async_resolver_job *job = &pool->jobs[slot];
    if (job->generation != generation
        || job->state != NOMO_ASYNC_BLOCKING_JOB_COMPLETED_DELIVERED) {
        LeaveCriticalSection(&pool->mutex);
        return 1;
    }
    memcpy(addresses, job->addresses, sizeof(job->addresses));
    memcpy(
        address_lengths,
        job->address_lengths,
        sizeof(job->address_lengths)
    );
    *address_count = job->address_count;
    *resolver_status = job->resolver_status;
    nomo_async_resolver_pool_release_job_locked(pool, job);
    LeaveCriticalSection(&pool->mutex);
    nomo_async_resolver_pool_maybe_idle(pool);
    return 0;
}

static void nomo_async_resolver_cancel(
    nomo_async_context *context,
    uint32_t slot,
    uint32_t generation
) {
    nomo_async_resolver_pool *pool =
        (nomo_async_resolver_pool *)context->blocking_pool;
    if (pool == NULL || slot >= NOMO_ASYNC_BLOCKING_POOL_QUEUE_CAPACITY) {
        return;
    }
    EnterCriticalSection(&pool->mutex);
    nomo_async_resolver_job *job = &pool->jobs[slot];
    if (job->generation != generation) {
        LeaveCriticalSection(&pool->mutex);
        return;
    }
    uint8_t cancelled = 1u;
    if (job->state == NOMO_ASYNC_BLOCKING_JOB_QUEUED) {
        if (nomo_async_resolver_pool_remove_queued_locked(pool, slot) == 0) {
            nomo_async_resolver_pool_release_job_locked(pool, job);
        }
    } else if (job->state == NOMO_ASYNC_BLOCKING_JOB_RUNNING) {
        job->state = NOMO_ASYNC_BLOCKING_JOB_CANCELLED_RUNNING;
        job->owner = NULL;
        job->complete = NULL;
    } else if (job->state == NOMO_ASYNC_BLOCKING_JOB_COMPLETED_QUEUED) {
        job->state = NOMO_ASYNC_BLOCKING_JOB_CANCELLED_QUEUED;
        job->owner = NULL;
        job->complete = NULL;
    } else if (job->state == NOMO_ASYNC_BLOCKING_JOB_COMPLETED_DELIVERED) {
        nomo_async_resolver_pool_release_job_locked(pool, job);
    } else {
        cancelled = 0u;
    }
    LeaveCriticalSection(&pool->mutex);
    if (cancelled != 0u) {
        context->blocking_jobs_cancelled += 1u;
    }
    nomo_async_resolver_pool_maybe_idle(pool);
}

static void nomo_async_blocking_pool_shutdown(nomo_async_context *context) {
    nomo_async_resolver_pool *pool =
        (nomo_async_resolver_pool *)context->blocking_pool;
    if (pool == NULL) {
        return;
    }
    EnterCriticalSection(&pool->mutex);
    pool->stopping = 1u;
    WakeAllConditionVariable(&pool->available);
    LeaveCriticalSection(&pool->mutex);
    WaitForSingleObject(pool->worker, INFINITE);
    while (1) {
        uint32_t pending_posts = 0u;
        EnterCriticalSection(&pool->mutex);
        pending_posts = pool->pending_posts;
        LeaveCriticalSection(&pool->mutex);
        if (pending_posts == 0u) {
            break;
        }
        uint8_t had_completion = 0u;
        if (nomo_async_reactor_wait(
                &context->reactor,
                -1,
                &had_completion
            ) != 0) {
            context->runtime_failed = 1u;
            break;
        }
    }
    nomo_async_reactor_post_deactivate(
        &context->reactor,
        &pool->wake_registration
    );
    CloseHandle(pool->worker);
    DeleteCriticalSection(&pool->mutex);
    context->blocking_threads_retired += 1u;
    context->live_blocking_threads = 0u;
    context->live_blocking_jobs = 0u;
    free(pool);
    context->blocking_pool = NULL;
}

"#,
    );
}

fn emit_async_resolver_pool_unix_tail(out: &mut String) {
    out.push_str(
        r#"static int nomo_async_resolver_submit(
    nomo_async_context *context,
    const char *host,
    int64_t port,
    void *owner,
    nomo_async_blocking_completion_fn complete,
    uint32_t *slot_out,
    uint32_t *generation_out
) {
    if (nomo_async_resolver_pool_initialize(context) != 0) {
        return 1;
    }
    nomo_async_resolver_pool *pool =
        (nomo_async_resolver_pool *)context->blocking_pool;
    if (nomo_async_resolver_pool_ensure_wake_registration(pool) != 0) {
        return 1;
    }
    if (pthread_mutex_lock(&pool->mutex) != 0) {
        return 1;
    }
    uint32_t selected = NOMO_ASYNC_BLOCKING_POOL_QUEUE_CAPACITY;
    for (uint32_t index = 0u;
         index < NOMO_ASYNC_BLOCKING_POOL_QUEUE_CAPACITY;
         index += 1u) {
        if (pool->jobs[index].state == NOMO_ASYNC_BLOCKING_JOB_FREE) {
            selected = index;
            break;
        }
    }
    if (selected == NOMO_ASYNC_BLOCKING_POOL_QUEUE_CAPACITY
        || pool->queue_count == NOMO_ASYNC_BLOCKING_POOL_QUEUE_CAPACITY) {
        pthread_mutex_unlock(&pool->mutex);
        context->blocking_queue_saturations += 1u;
        return 2;
    }
    pool->next_generation += 1u;
    if (pool->next_generation == 0u) {
        pool->next_generation = 1u;
    }
    nomo_async_resolver_job *job = &pool->jobs[selected];
    memset(job, 0, sizeof(*job));
    job->generation = pool->next_generation;
    job->state = NOMO_ASYNC_BLOCKING_JOB_QUEUED;
    memcpy(job->host, host, strlen(host) + 1u);
    snprintf(job->port, sizeof(job->port), "%" PRId64, port);
    job->owner = owner;
    job->complete = complete;
    pool->queue[pool->queue_tail] = selected;
    pool->queue_tail =
        (pool->queue_tail + 1u) % NOMO_ASYNC_BLOCKING_POOL_QUEUE_CAPACITY;
    pool->queue_count += 1u;
    pool->active_jobs += 1u;
    *slot_out = selected;
    *generation_out = job->generation;
    pthread_cond_signal(&pool->available);
    pthread_mutex_unlock(&pool->mutex);
    context->blocking_jobs_queued += 1u;
    context->live_blocking_jobs += 1u;
    if (context->live_blocking_jobs > context->peak_live_blocking_jobs) {
        context->peak_live_blocking_jobs = context->live_blocking_jobs;
    }
    return 0;
}

static int nomo_async_resolver_take(
    nomo_async_context *context,
    uint32_t slot,
    uint32_t generation,
    struct sockaddr_storage *addresses,
    socklen_t *address_lengths,
    uint32_t *address_count,
    int *resolver_status
) {
    nomo_async_resolver_pool *pool =
        (nomo_async_resolver_pool *)context->blocking_pool;
    if (pool == NULL
        || slot >= NOMO_ASYNC_BLOCKING_POOL_QUEUE_CAPACITY
        || pthread_mutex_lock(&pool->mutex) != 0) {
        return 1;
    }
    nomo_async_resolver_job *job = &pool->jobs[slot];
    if (job->generation != generation
        || job->state != NOMO_ASYNC_BLOCKING_JOB_COMPLETED_DELIVERED) {
        pthread_mutex_unlock(&pool->mutex);
        return 1;
    }
    memcpy(
        addresses,
        job->addresses,
        sizeof(job->addresses)
    );
    memcpy(
        address_lengths,
        job->address_lengths,
        sizeof(job->address_lengths)
    );
    *address_count = job->address_count;
    *resolver_status = job->resolver_status;
    nomo_async_resolver_pool_release_job_locked(pool, job);
    pthread_mutex_unlock(&pool->mutex);
    nomo_async_resolver_pool_maybe_idle(pool);
    return 0;
}

static void nomo_async_resolver_cancel(
    nomo_async_context *context,
    uint32_t slot,
    uint32_t generation
) {
    nomo_async_resolver_pool *pool =
        (nomo_async_resolver_pool *)context->blocking_pool;
    if (pool == NULL
        || slot >= NOMO_ASYNC_BLOCKING_POOL_QUEUE_CAPACITY
        || pthread_mutex_lock(&pool->mutex) != 0) {
        return;
    }
    nomo_async_resolver_job *job = &pool->jobs[slot];
    if (job->generation != generation) {
        pthread_mutex_unlock(&pool->mutex);
        return;
    }
    uint8_t cancelled = 1u;
    if (job->state == NOMO_ASYNC_BLOCKING_JOB_QUEUED) {
        if (nomo_async_resolver_pool_remove_queued_locked(pool, slot) == 0) {
            nomo_async_resolver_pool_release_job_locked(pool, job);
        }
    } else if (job->state == NOMO_ASYNC_BLOCKING_JOB_RUNNING) {
        job->state = NOMO_ASYNC_BLOCKING_JOB_CANCELLED_RUNNING;
        job->owner = NULL;
        job->complete = NULL;
    } else if (job->state == NOMO_ASYNC_BLOCKING_JOB_COMPLETED_QUEUED) {
        job->state = NOMO_ASYNC_BLOCKING_JOB_CANCELLED_QUEUED;
        job->owner = NULL;
        job->complete = NULL;
    } else if (job->state == NOMO_ASYNC_BLOCKING_JOB_COMPLETED_DELIVERED) {
        nomo_async_resolver_pool_release_job_locked(pool, job);
    } else {
        cancelled = 0u;
    }
    pthread_mutex_unlock(&pool->mutex);
    if (cancelled != 0u) {
        context->blocking_jobs_cancelled += 1u;
    }
    nomo_async_resolver_pool_maybe_idle(pool);
}

static void nomo_async_blocking_pool_shutdown(nomo_async_context *context) {
    nomo_async_resolver_pool *pool =
        (nomo_async_resolver_pool *)context->blocking_pool;
    if (pool == NULL) {
        return;
    }
    if (pthread_mutex_lock(&pool->mutex) == 0) {
        pool->stopping = 1u;
        pthread_cond_broadcast(&pool->available);
        pthread_mutex_unlock(&pool->mutex);
    }
    pthread_join(pool->worker, NULL);
    if (pool->wake_registration.active != 0u) {
        nomo_async_reactor_deregister(
            &context->reactor,
            &pool->wake_registration
        );
    }
    close(pool->wake_read);
    close(pool->wake_write);
    pthread_cond_destroy(&pool->available);
    pthread_mutex_destroy(&pool->mutex);
    context->blocking_threads_retired += 1u;
    context->live_blocking_threads = 0u;
    context->live_blocking_jobs = 0u;
    free(pool);
    context->blocking_pool = NULL;
}

"#,
    );
}
