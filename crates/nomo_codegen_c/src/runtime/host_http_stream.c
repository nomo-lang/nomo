#define NOMO_HTTP_STREAM_MAX_CHUNK_BYTES (UINT64_C(1024) * UINT64_C(1024))
#define NOMO_HTTP_STREAM_MAX_EVENT_BYTES (UINT64_C(1024) * UINT64_C(1024))
#define NOMO_CURL_WRITEFUNC_PAUSE ((size_t)0x10000001)
#define NOMO_CURLPAUSE_CONT 0
#define NOMO_CURLMSG_DONE 1

typedef struct nomo_http_stream_buffer {
    char *data;
    size_t len;
    size_t cap;
} nomo_http_stream_buffer;

typedef struct nomo_http_stream_state {
    uint64_t id;
    uint64_t idle_timeout_millis;
    uint64_t max_response_bytes;
    uint64_t response_bytes;
    int mode;
    int eof;
    int too_large;
    const char *failure_code;
    nomo_http_stream_buffer pending;
    nomo_http_stream_buffer sse_data;
    nomo_http_stream_buffer sse_event;
    nomo_http_stream_buffer sse_id;
    int sse_first_line;
    int sse_retry_present;
    uint64_t sse_retry_millis;
    uint64_t sse_event_bytes;
#ifdef _WIN32
    nomo_winhttp_handle session;
    nomo_winhttp_handle connection;
    nomo_winhttp_handle request;
#else
    nomo_curl_easy *easy;
    nomo_curl_multi *multi;
    nomo_curl_slist *request_headers;
    char *method;
    char *url;
    char *body;
    nomo_http_header_buffer response_headers;
    int current_status;
    int headers_complete;
    int accept_body;
    int paused;
    int curl_done;
    int curl_result;
#endif
    struct nomo_http_stream_state *next;
} nomo_http_stream_state;

static nomo_http_stream_state *nomo_http_streams = NULL;
static uint64_t nomo_http_next_stream_id = UINT64_C(1);

static uint64_t nomo_http_stream_now_millis(void) {
#ifdef _WIN32
    return (uint64_t)GetTickCount64();
#else
    struct timespec now;
    if (clock_gettime(CLOCK_MONOTONIC, &now) != 0) {
        nomo_panic("HTTP stream monotonic clock failed");
    }
    return (uint64_t)now.tv_sec * UINT64_C(1000)
        + (uint64_t)now.tv_nsec / UINT64_C(1000000);
#endif
}

static char *nomo_http_stream_copy_cstr(const char *value) {
    size_t len = strlen(value);
    char *copy = (char *)malloc(len + 1);
    if (copy == NULL) { nomo_panic("out of memory"); }
    memcpy(copy, value, len + 1);
    return copy;
}

static void nomo_http_stream_buffer_reserve(
    nomo_http_stream_buffer *buffer,
    size_t needed
) {
    if (needed <= buffer->cap) { return; }
    size_t cap = buffer->cap == 0 ? 4096 : buffer->cap;
    while (cap < needed) {
        if (cap > SIZE_MAX / 2) { nomo_panic("out of memory"); }
        cap *= 2;
    }
    char *next = (char *)realloc(buffer->data, cap);
    if (next == NULL) { nomo_panic("out of memory"); }
    buffer->data = next;
    buffer->cap = cap;
}

static void nomo_http_stream_buffer_append(
    nomo_http_stream_buffer *buffer,
    const char *data,
    size_t len
) {
    if (len == 0) { return; }
    if (len > SIZE_MAX - buffer->len) { nomo_panic("out of memory"); }
    nomo_http_stream_buffer_reserve(buffer, buffer->len + len);
    memcpy(buffer->data + buffer->len, data, len);
    buffer->len += len;
}

static void nomo_http_stream_buffer_assign(
    nomo_http_stream_buffer *buffer,
    const char *data,
    size_t len
) {
    buffer->len = 0;
    nomo_http_stream_buffer_append(buffer, data, len);
}

static void nomo_http_stream_buffer_consume(
    nomo_http_stream_buffer *buffer,
    size_t len
) {
    if (len >= buffer->len) {
        buffer->len = 0;
        return;
    }
    memmove(buffer->data, buffer->data + len, buffer->len - len);
    buffer->len -= len;
}

static void nomo_http_stream_buffer_release(nomo_http_stream_buffer *buffer) {
    free(buffer->data);
    buffer->data = NULL;
    buffer->len = 0;
    buffer->cap = 0;
}

static int nomo_http_stream_utf8_width(
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

static int nomo_http_stream_utf8_prefix(
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
        int status = nomo_http_stream_utf8_width(
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

static @OPEN_RESULT@ nomo_http_stream_open_error(
    const char *code,
    const char *message
) {
    return (@OPEN_RESULT@){
        .tag = @OPEN_ERR@,
        .payload.@ERR_PAYLOAD@ = (@HTTP_ERROR@){
            .@CODE_MEMBER@ = nomo_string_from_cstr(code),
            .@MESSAGE_MEMBER@ = nomo_string_from_cstr(message)
        }
    };
}

static @READ_RESULT@ nomo_http_stream_read_error(
    const char *code,
    const char *message
) {
    return (@READ_RESULT@){
        .tag = @READ_ERR@,
        .payload.@ERR_PAYLOAD@ = (@HTTP_ERROR@){
            .@CODE_MEMBER@ = nomo_string_from_cstr(code),
            .@MESSAGE_MEMBER@ = nomo_string_from_cstr(message)
        }
    };
}

static @SSE_RESULT@ nomo_http_stream_sse_error(
    const char *code,
    const char *message
) {
    return (@SSE_RESULT@){
        .tag = @SSE_ERR@,
        .payload.@ERR_PAYLOAD@ = (@HTTP_ERROR@){
            .@CODE_MEMBER@ = nomo_string_from_cstr(code),
            .@MESSAGE_MEMBER@ = nomo_string_from_cstr(message)
        }
    };
}

static nomo_http_stream_state *nomo_http_stream_find(uint64_t id) {
    for (nomo_http_stream_state *state = nomo_http_streams;
         state != NULL;
         state = state->next) {
        if (state->id == id) { return state; }
    }
    return NULL;
}

static void nomo_http_stream_destroy(nomo_http_stream_state *state) {
    if (state == NULL) { return; }
#ifdef _WIN32
    nomo_winhttp_close_all(state->request, state->connection, state->session);
#else
    if (state->multi != NULL && state->easy != NULL
        && nomo_http_curl.multi_remove_handle != NULL) {
        nomo_http_curl.multi_remove_handle(state->multi, state->easy);
    }
    if (state->easy != NULL) { nomo_http_curl.easy_cleanup(state->easy); }
    if (state->multi != NULL && nomo_http_curl.multi_cleanup != NULL) {
        nomo_http_curl.multi_cleanup(state->multi);
    }
    if (state->request_headers != NULL) {
        nomo_http_curl.slist_free_all(state->request_headers);
    }
    free(state->method);
    free(state->url);
    free(state->body);
    @HTTP_HEADER_ARRAY_RELEASE@(state->response_headers.values);
#endif
    nomo_http_stream_buffer_release(&state->pending);
    nomo_http_stream_buffer_release(&state->sse_data);
    nomo_http_stream_buffer_release(&state->sse_event);
    nomo_http_stream_buffer_release(&state->sse_id);
    free(state);
}

static void nomo_http_stream_detach(nomo_http_stream_state *state) {
    nomo_http_stream_state **cursor = &nomo_http_streams;
    while (*cursor != NULL) {
        if (*cursor == state) {
            *cursor = state->next;
            nomo_http_stream_destroy(state);
            return;
        }
        cursor = &(*cursor)->next;
    }
}

static int nomo_http_stream_validate_request(
    @HTTP_REQUEST@ request,
    uint64_t idle_timeout_millis
) {
    const char *method = request.@METHOD_MEMBER@.data;
    if ((strcmp(method, "GET") != 0 && strcmp(method, "POST") != 0)
        || !nomo_http_validate_url(request.@URL_MEMBER@.data)
        || request.@TIMEOUT_MEMBER@ == 0
        || request.@TIMEOUT_MEMBER@ > NOMO_HTTP_MAX_TIMEOUT_MS
        || idle_timeout_millis == 0
        || idle_timeout_millis > NOMO_HTTP_MAX_TIMEOUT_MS
        || request.@MAX_RESPONSE_MEMBER@ == 0
        || request.@MAX_RESPONSE_MEMBER@ > NOMO_HTTP_HARD_MAX_RESPONSE_BYTES
        || (strcmp(method, "GET") == 0 && request.@BODY_MEMBER@.data[0] != '\0')) {
        return 0;
    }
    for (size_t index = 0; index < request.@HEADERS_REQUEST_MEMBER@.len; index += 1) {
        if (!nomo_http_validate_header(request.@HEADERS_REQUEST_MEMBER@.data[index])) {
            return 0;
        }
    }
    return 1;
}

#ifndef _WIN32
static int nomo_http_stream_has_curl_api(void) {
    return nomo_http_load_curl()
        && nomo_http_curl.easy_pause != NULL
        && nomo_http_curl.multi_init != NULL
        && nomo_http_curl.multi_add_handle != NULL
        && nomo_http_curl.multi_remove_handle != NULL
        && nomo_http_curl.multi_perform != NULL
        && (nomo_http_curl.multi_poll != NULL || nomo_http_curl.multi_wait != NULL)
        && nomo_http_curl.multi_info_read != NULL
        && nomo_http_curl.multi_cleanup != NULL;
}

static size_t nomo_http_stream_curl_header(
    char *data,
    size_t size,
    size_t count,
    void *userdata
) {
    nomo_http_stream_state *state = (nomo_http_stream_state *)userdata;
    if (count != 0 && size > SIZE_MAX / count) {
        state->response_headers.too_large = 1;
        return 0;
    }
    size_t incoming = size * count;
    if (incoming >= 5 && memcmp(data, "HTTP/", 5) == 0) {
        state->headers_complete = 0;
        const char *space = memchr(data, ' ', incoming);
        state->current_status = space == NULL ? 0 : atoi(space + 1);
    }
    size_t written = nomo_http_write_header(data, size, count, &state->response_headers);
    if (written == incoming
        && (state->current_status < 100 || state->current_status >= 200)
        && ((incoming == 2 && data[0] == '\r' && data[1] == '\n')
            || (incoming == 1 && data[0] == '\n'))) {
        state->headers_complete = 1;
    }
    return written;
}

static size_t nomo_http_stream_curl_body(
    char *data,
    size_t size,
    size_t count,
    void *userdata
) {
    nomo_http_stream_state *state = (nomo_http_stream_state *)userdata;
    if (count != 0 && size > SIZE_MAX / count) {
        state->too_large = 1;
        return 0;
    }
    size_t incoming = size * count;
    if (!state->accept_body) {
        state->paused = 1;
        return NOMO_CURL_WRITEFUNC_PAUSE;
    }
    state->accept_body = 0;
    if ((uint64_t)incoming > state->max_response_bytes - state->response_bytes) {
        state->too_large = 1;
        return 0;
    }
    state->response_bytes += (uint64_t)incoming;
    nomo_http_stream_buffer_append(&state->pending, data, incoming);
    return incoming;
}

static int nomo_http_stream_curl_perform(nomo_http_stream_state *state) {
    int running = 0;
    int code = 0;
    do {
        code = nomo_http_curl.multi_perform(state->multi, &running);
    } while (code == -1);
    if (code != 0) {
        state->failure_code = "transport";
        return 0;
    }
    int queued = 0;
    nomo_curl_msg *message = NULL;
    while ((message = nomo_http_curl.multi_info_read(state->multi, &queued)) != NULL) {
        if (message->msg == NOMO_CURLMSG_DONE && message->easy_handle == state->easy) {
            state->curl_done = 1;
            state->curl_result = message->data.result;
            if (state->curl_result == 0) {
                state->eof = 1;
            } else if (!state->too_large && !state->response_headers.too_large) {
                state->failure_code = nomo_http_error_code_for_curl(state->curl_result);
            }
        }
    }
    return 1;
}

static int nomo_http_stream_curl_wait(
    nomo_http_stream_state *state,
    int timeout_millis
) {
    int ready = 0;
    int code = nomo_http_curl.multi_poll != NULL
        ? nomo_http_curl.multi_poll(state->multi, NULL, 0, timeout_millis, &ready)
        : nomo_http_curl.multi_wait(state->multi, NULL, 0, timeout_millis, &ready);
    if (code != 0) {
        state->failure_code = "transport";
        return 0;
    }
    return 1;
}

static int nomo_http_stream_curl_drive_headers(
    nomo_http_stream_state *state,
    uint64_t deadline
) {
    for (;;) {
        if (!nomo_http_stream_curl_perform(state)) { return 0; }
        if (state->response_headers.too_large) {
            state->too_large = 1;
            return 0;
        }
        if (state->headers_complete) { return 1; }
        if (state->curl_done) {
            if (state->failure_code == NULL) { state->failure_code = "protocol"; }
            return 0;
        }
        uint64_t now = nomo_http_stream_now_millis();
        if (now >= deadline) {
            state->failure_code = "timeout";
            return 0;
        }
        uint64_t remaining = deadline - now;
        int wait = remaining > (uint64_t)INT_MAX ? INT_MAX : (int)remaining;
        if (!nomo_http_stream_curl_wait(state, wait)) { return 0; }
    }
}

static int nomo_http_stream_curl_read_more(nomo_http_stream_state *state) {
    if (state->eof || state->failure_code != NULL || state->too_large) { return 0; }
    size_t before = state->pending.len;
    if (state->paused) {
        state->paused = 0;
        state->accept_body = 1;
        if (nomo_http_curl.easy_pause(state->easy, NOMO_CURLPAUSE_CONT) != 0) {
            state->failure_code = "transport";
            return 0;
        }
    } else {
        state->accept_body = 1;
    }
    uint64_t deadline =
        nomo_http_stream_now_millis() + state->idle_timeout_millis;
    while (state->pending.len == before && !state->eof
        && state->failure_code == NULL && !state->too_large) {
        if (!nomo_http_stream_curl_perform(state)) { break; }
        if (state->pending.len != before || state->eof
            || state->failure_code != NULL || state->too_large) {
            break;
        }
        uint64_t now = nomo_http_stream_now_millis();
        if (now >= deadline) {
            state->failure_code = "timeout";
            break;
        }
        uint64_t remaining = deadline - now;
        int wait = remaining > (uint64_t)INT_MAX ? INT_MAX : (int)remaining;
        if (!nomo_http_stream_curl_wait(state, wait)) { break; }
    }
    return state->pending.len > before;
}

static int nomo_http_stream_open_curl(
    nomo_http_stream_state *state,
    @HTTP_REQUEST@ request,
    uint64_t deadline
) {
    if (!nomo_http_stream_has_curl_api()) {
        state->failure_code = "runtime_unavailable";
        return 0;
    }
    state->method = nomo_http_stream_copy_cstr(request.@METHOD_MEMBER@.data);
    state->url = nomo_http_stream_copy_cstr(request.@URL_MEMBER@.data);
    state->body = nomo_http_stream_copy_cstr(request.@BODY_MEMBER@.data);
    state->response_headers = (nomo_http_header_buffer){
        .values = @HTTP_HEADER_ARRAY_NEW@(),
        .bytes = 0,
        .too_large = 0
    };
    state->easy = nomo_http_curl.easy_init();
    state->multi = nomo_http_curl.multi_init();
    if (state->easy == NULL || state->multi == NULL) {
        state->failure_code = "runtime_unavailable";
        return 0;
    }
    state->request_headers = nomo_http_curl.slist_append(NULL, "Expect:");
    if (state->request_headers == NULL) {
        state->failure_code = "transport";
        return 0;
    }
    for (size_t index = 0; index < request.@HEADERS_REQUEST_MEMBER@.len; index += 1) {
        @HTTP_HEADER@ header = request.@HEADERS_REQUEST_MEMBER@.data[index];
        nomo_curl_slist *next = nomo_http_append_header(
            state->request_headers,
            header.@NAME_MEMBER@.data,
            header.@VALUE_MEMBER@.data
        );
        if (next == NULL) {
            state->failure_code = "transport";
            return 0;
        }
        state->request_headers = next;
    }
#define NOMO_STREAM_SETOPT(option, value) \
    do { \
        if (nomo_http_curl.easy_setopt(state->easy, (option), (value)) != 0) { \
            state->failure_code = "runtime_unavailable"; \
            return 0; \
        } \
    } while (0)
    NOMO_STREAM_SETOPT(NOMO_CURLOPT_URL, state->url);
    NOMO_STREAM_SETOPT(NOMO_CURLOPT_CUSTOMREQUEST, state->method);
    NOMO_STREAM_SETOPT(NOMO_CURLOPT_USERAGENT, "nomo/0.1");
    NOMO_STREAM_SETOPT(NOMO_CURLOPT_HTTPHEADER, state->request_headers);
    NOMO_STREAM_SETOPT(NOMO_CURLOPT_CONNECTTIMEOUT_MS, (long)request.@TIMEOUT_MEMBER@);
    NOMO_STREAM_SETOPT(NOMO_CURLOPT_NOSIGNAL, 1L);
    NOMO_STREAM_SETOPT(NOMO_CURLOPT_FOLLOWLOCATION, 0L);
    NOMO_STREAM_SETOPT(NOMO_CURLOPT_SSL_VERIFYPEER, 1L);
    NOMO_STREAM_SETOPT(NOMO_CURLOPT_SSL_VERIFYHOST, 2L);
    NOMO_STREAM_SETOPT(NOMO_CURLOPT_WRITEFUNCTION, nomo_http_stream_curl_body);
    NOMO_STREAM_SETOPT(NOMO_CURLOPT_WRITEDATA, state);
    NOMO_STREAM_SETOPT(NOMO_CURLOPT_HEADERFUNCTION, nomo_http_stream_curl_header);
    NOMO_STREAM_SETOPT(NOMO_CURLOPT_HEADERDATA, state);
    if (strcmp(state->method, "POST") == 0) {
        NOMO_STREAM_SETOPT(NOMO_CURLOPT_POSTFIELDS, state->body);
        NOMO_STREAM_SETOPT(
            NOMO_CURLOPT_POSTFIELDSIZE_LARGE,
            (long long)strlen(state->body)
        );
    }
    const char *ca_bundle = getenv("NOMO_HTTP_CA_BUNDLE");
    if (ca_bundle != NULL && ca_bundle[0] != '\0') {
        NOMO_STREAM_SETOPT(NOMO_CURLOPT_CAINFO, ca_bundle);
    }
#undef NOMO_STREAM_SETOPT
    if (nomo_http_curl.multi_add_handle(state->multi, state->easy) != 0) {
        state->failure_code = "runtime_unavailable";
        return 0;
    }
    if (!nomo_http_stream_curl_drive_headers(state, deadline)) { return 0; }
    long status = 0;
    if (nomo_http_curl.easy_getinfo(
            state->easy,
            NOMO_CURLINFO_RESPONSE_CODE,
            &status) != 0
        || status <= 0) {
        state->failure_code = "protocol";
        return 0;
    }
    state->current_status = (int)status;
    return 1;
}
#endif

#ifdef _WIN32
static int nomo_http_stream_open_winhttp(
    nomo_http_stream_state *state,
    @HTTP_REQUEST@ request,
    uint64_t deadline,
    long *status,
    @HTTP_HEADER_ARRAY@ *headers
) {
    if (!nomo_winhttp_load()) {
        state->failure_code = "runtime_unavailable";
        return 0;
    }
    nomo_winhttp_url url = {0};
    if (!nomo_winhttp_parse_url(request.@URL_MEMBER@.data, &url)) {
        state->failure_code = "invalid_request";
        return 0;
    }
    wchar_t *method = nomo_winhttp_wide_from_utf8(request.@METHOD_MEMBER@.data);
    if (method == NULL) {
        nomo_winhttp_url_release(&url);
        state->failure_code = "invalid_request";
        return 0;
    }
    state->session = nomo_winhttp.open(L"nomo/0.1", 0, NULL, NULL, 0);
    if (state->session == NULL) {
        state->failure_code = nomo_winhttp_error_code(GetLastError());
        free(method);
        nomo_winhttp_url_release(&url);
        return 0;
    }
    int timeout = (int)request.@TIMEOUT_MEMBER@;
    if (!nomo_winhttp.set_timeouts(
            state->session,
            timeout,
            timeout,
            timeout,
            timeout)) {
        state->failure_code = nomo_winhttp_error_code(GetLastError());
        free(method);
        nomo_winhttp_url_release(&url);
        return 0;
    }
    state->connection = nomo_winhttp.connect(
        state->session,
        url.host,
        url.port,
        0
    );
    if (state->connection == NULL) {
        state->failure_code = nomo_winhttp_error_code(GetLastError());
        free(method);
        nomo_winhttp_url_release(&url);
        return 0;
    }
    int remaining = nomo_winhttp_remaining_timeout((ULONGLONG)deadline);
    if (remaining == 0) {
        state->failure_code = "timeout";
        free(method);
        nomo_winhttp_url_release(&url);
        return 0;
    }
    state->request = nomo_winhttp.open_request(
        state->connection,
        method,
        url.path,
        NULL,
        NULL,
        NULL,
        url.secure ? 0x00800000UL : 0
    );
    free(method);
    nomo_winhttp_url_release(&url);
    if (state->request == NULL) {
        state->failure_code = nomo_winhttp_error_code(GetLastError());
        return 0;
    }
    if (!nomo_winhttp.set_timeouts(
            state->request,
            remaining,
            remaining,
            remaining,
            remaining)) {
        state->failure_code = nomo_winhttp_error_code(GetLastError());
        return 0;
    }
    for (size_t index = 0; index < request.@HEADERS_REQUEST_MEMBER@.len; index += 1) {
        @HTTP_HEADER@ header = request.@HEADERS_REQUEST_MEMBER@.data[index];
        size_t name_len = strlen(header.@NAME_MEMBER@.data);
        size_t value_len = strlen(header.@VALUE_MEMBER@.data);
        char *line = (char *)malloc(name_len + value_len + 3);
        if (line == NULL) { nomo_panic("out of memory"); }
        memcpy(line, header.@NAME_MEMBER@.data, name_len);
        line[name_len] = ':';
        line[name_len + 1] = ' ';
        memcpy(line + name_len + 2, header.@VALUE_MEMBER@.data, value_len + 1);
        wchar_t *wide = nomo_winhttp_wide_from_utf8(line);
        free(line);
        if (wide == NULL
            || !nomo_winhttp.add_headers(
                state->request,
                wide,
                (unsigned long)-1L,
                0x20000000UL)) {
            state->failure_code = nomo_winhttp_error_code(GetLastError());
            free(wide);
            return 0;
        }
        free(wide);
    }
    size_t body_len = strlen(request.@BODY_MEMBER@.data);
    if (body_len > UINT32_MAX) {
        state->failure_code = "invalid_request";
        return 0;
    }
    remaining = nomo_winhttp_remaining_timeout((ULONGLONG)deadline);
    if (remaining == 0) {
        state->failure_code = "timeout";
        return 0;
    }
    if (!nomo_winhttp.set_timeouts(
            state->request,
            remaining,
            remaining,
            remaining,
            remaining)
        || !nomo_winhttp.send_request(
            state->request,
            NULL,
            0,
            body_len == 0 ? NULL : request.@BODY_MEMBER@.data,
            (unsigned long)body_len,
            (unsigned long)body_len,
            0)) {
        state->failure_code = nomo_winhttp_error_code(GetLastError());
        return 0;
    }
    remaining = nomo_winhttp_remaining_timeout((ULONGLONG)deadline);
    if (remaining == 0) {
        state->failure_code = "timeout";
        return 0;
    }
    if (!nomo_winhttp.set_timeouts(
            state->request,
            remaining,
            remaining,
            remaining,
            remaining)
        || !nomo_winhttp.receive_response(state->request, NULL)) {
        state->failure_code = nomo_winhttp_error_code(GetLastError());
        return 0;
    }
    unsigned long raw_status = 0;
    unsigned long status_size = (unsigned long)sizeof(raw_status);
    if (!nomo_winhttp.query_headers(
            state->request,
            19UL | 0x20000000UL,
            NULL,
            &raw_status,
            &status_size,
            NULL)) {
        state->failure_code = nomo_winhttp_error_code(GetLastError());
        return 0;
    }
    nomo_http_header_buffer response_headers = {
        .values = @HTTP_HEADER_ARRAY_NEW@(),
        .bytes = 0,
        .too_large = 0
    };
    unsigned long raw_size = 0;
    int raw_query =
        nomo_winhttp.query_headers(state->request, 22UL, NULL, NULL, &raw_size, NULL);
    unsigned long raw_error = raw_query ? 0 : GetLastError();
    if (!raw_query && raw_error != 122UL) {
        @HTTP_HEADER_ARRAY_RELEASE@(response_headers.values);
        state->failure_code = nomo_winhttp_error_code(raw_error);
        return 0;
    }
    if ((uint64_t)raw_size
        > NOMO_HTTP_MAX_RESPONSE_HEADER_BYTES * sizeof(wchar_t)) {
        @HTTP_HEADER_ARRAY_RELEASE@(response_headers.values);
        state->too_large = 1;
        return 0;
    }
    if (raw_size > 0) {
        wchar_t *raw = (wchar_t *)malloc((size_t)raw_size);
        if (raw == NULL) { nomo_panic("out of memory"); }
        if (!nomo_winhttp.query_headers(
                state->request,
                22UL,
                NULL,
                raw,
                &raw_size,
                NULL)) {
            state->failure_code = nomo_winhttp_error_code(GetLastError());
            free(raw);
            @HTTP_HEADER_ARRAY_RELEASE@(response_headers.values);
            return 0;
        }
        char *utf8 = nomo_winhttp_utf8_from_wide(raw);
        free(raw);
        if (utf8 == NULL) {
            state->failure_code = "protocol";
            @HTTP_HEADER_ARRAY_RELEASE@(response_headers.values);
            return 0;
        }
        char *line = utf8;
        while (*line != '\0') {
            char *end = strstr(line, "\r\n");
            size_t len = end == NULL ? strlen(line) : (size_t)(end - line) + 2;
            if (nomo_http_write_header(line, 1, len, &response_headers) != len) {
                state->too_large = 1;
                free(utf8);
                @HTTP_HEADER_ARRAY_RELEASE@(response_headers.values);
                return 0;
            }
            if (end == NULL) { break; }
            line = end + 2;
        }
        free(utf8);
    }
    *status = (long)raw_status;
    *headers = response_headers.values;
    return 1;
}

static int nomo_http_stream_winhttp_read_more(nomo_http_stream_state *state) {
    if (state->eof || state->failure_code != NULL || state->too_large) { return 0; }
    int timeout = (int)state->idle_timeout_millis;
    if (!nomo_winhttp.set_timeouts(
            state->request,
            timeout,
            timeout,
            timeout,
            timeout)) {
        state->failure_code = nomo_winhttp_error_code(GetLastError());
        return 0;
    }
    unsigned char chunk[16384];
    unsigned long received = 0;
    if (!nomo_winhttp.read_data(
            state->request,
            chunk,
            (unsigned long)sizeof(chunk),
            &received)) {
        state->failure_code = nomo_winhttp_error_code(GetLastError());
        return 0;
    }
    if (received == 0) {
        state->eof = 1;
        return 0;
    }
    if ((uint64_t)received > state->max_response_bytes - state->response_bytes) {
        state->too_large = 1;
        return 0;
    }
    state->response_bytes += (uint64_t)received;
    nomo_http_stream_buffer_append(
        &state->pending,
        (const char *)chunk,
        (size_t)received
    );
    return 1;
}
#endif

static int nomo_http_stream_read_more(nomo_http_stream_state *state) {
#ifdef _WIN32
    return nomo_http_stream_winhttp_read_more(state);
#else
    return nomo_http_stream_curl_read_more(state);
#endif
}

static const char *nomo_http_stream_failure_message(const char *code) {
    if (code == NULL) { return "HTTP stream failed"; }
    if (strcmp(code, "invalid_request") == 0) { return "invalid bounded HTTP stream request"; }
    if (strcmp(code, "runtime_unavailable") == 0) {
        return "the native HTTP streaming runtime is unavailable on this host";
    }
    if (strcmp(code, "response_too_large") == 0) {
        return "HTTP response exceeded its configured limit";
    }
    if (strcmp(code, "protocol") == 0) {
        return "HTTP response stream was not valid UTF-8 text";
    }
    return nomo_http_error_message_for_code(code);
}

static @OPEN_RESULT@ @OPEN_NAME@(
    @HTTP_REQUEST@ request,
    uint64_t idle_timeout_millis
) {
    if (!nomo_http_stream_validate_request(request, idle_timeout_millis)) {
        return nomo_http_stream_open_error(
            "invalid_request",
            "invalid bounded HTTP stream request"
        );
    }
    nomo_http_stream_state *state =
        (nomo_http_stream_state *)calloc(1, sizeof(nomo_http_stream_state));
    if (state == NULL) { nomo_panic("out of memory"); }
    state->idle_timeout_millis = idle_timeout_millis;
    state->max_response_bytes = request.@MAX_RESPONSE_MEMBER@;
    state->sse_first_line = 1;
    uint64_t deadline =
        nomo_http_stream_now_millis() + request.@TIMEOUT_MEMBER@;
    long status = 0;
    @HTTP_HEADER_ARRAY@ headers = @HTTP_HEADER_ARRAY_NEW@();
#ifdef _WIN32
    if (!nomo_http_stream_open_winhttp(state, request, deadline, &status, &headers)) {
#else
    if (!nomo_http_stream_open_curl(state, request, deadline)) {
#endif
        const char *code = state->too_large
            ? "response_too_large"
            : (state->failure_code == NULL ? "transport" : state->failure_code);
        const char *message = nomo_http_stream_failure_message(code);
        nomo_http_stream_destroy(state);
        return nomo_http_stream_open_error(code, message);
    }
#ifndef _WIN32
    status = (long)state->current_status;
    headers = state->response_headers.values;
    state->response_headers.values = @HTTP_HEADER_ARRAY_NEW@();
#endif
    state->id = nomo_http_next_stream_id++;
    if (state->id == 0) { state->id = nomo_http_next_stream_id++; }
    state->next = nomo_http_streams;
    nomo_http_streams = state;
    return (@OPEN_RESULT@){
        .tag = @OPEN_OK@,
        .payload.@OK_PAYLOAD@ = (@HTTP_STREAM@){
            .@HANDLE_MEMBER@ = state->id,
            .@STATUS_MEMBER@ = (int64_t)status,
            .@HEADERS_MEMBER@ = headers
        }
    };
}

static @READ_RESULT@ @READ_NAME@(
    @HTTP_STREAM@ stream,
    uint64_t max_chunk_bytes
) {
    if (max_chunk_bytes < 4
        || max_chunk_bytes > NOMO_HTTP_STREAM_MAX_CHUNK_BYTES) {
        return nomo_http_stream_read_error(
            "invalid_request",
            "invalid HTTP stream chunk limit"
        );
    }
    nomo_http_stream_state *state = nomo_http_stream_find(stream.@HANDLE_MEMBER@);
    if (state == NULL || (state->mode != 0 && state->mode != 1)) {
        return nomo_http_stream_read_error(
            "invalid_request",
            "invalid or closed HTTP stream"
        );
    }
    state->mode = 1;
    for (;;) {
        size_t prefix = 0;
        if (!nomo_http_stream_utf8_prefix(
                state->pending.data,
                state->pending.len,
                (size_t)max_chunk_bytes,
                state->eof,
                &prefix)) {
            nomo_http_stream_detach(state);
            return nomo_http_stream_read_error(
                "protocol",
                "HTTP response stream was not valid UTF-8 text"
            );
        }
        if (prefix > 0) {
            nomo_string data =
                nomo_string_from_slice(state->pending.data, 0, prefix);
            nomo_http_stream_buffer_consume(&state->pending, prefix);
            return (@READ_RESULT@){
                .tag = @READ_OK@,
                .payload.@OK_PAYLOAD@ = (@HTTP_STREAM_CHUNK@){
                    .@DATA_MEMBER@ = data,
                    .@DONE_MEMBER@ = 0
                }
            };
        }
        if (state->too_large) {
            nomo_http_stream_detach(state);
            return nomo_http_stream_read_error(
                "response_too_large",
                "HTTP response exceeded its configured limit"
            );
        }
        if (state->failure_code != NULL) {
            const char *code = state->failure_code;
            const char *message = nomo_http_stream_failure_message(code);
            nomo_http_stream_detach(state);
            return nomo_http_stream_read_error(code, message);
        }
        if (state->eof) {
            return (@READ_RESULT@){
                .tag = @READ_OK@,
                .payload.@OK_PAYLOAD@ = (@HTTP_STREAM_CHUNK@){
                    .@DATA_MEMBER@ = nomo_string_literal(""),
                    .@DONE_MEMBER@ = 1
                }
            };
        }
        nomo_http_stream_read_more(state);
    }
}

static int nomo_http_stream_sse_next_line(
    nomo_http_stream_state *state,
    const char **line,
    size_t *line_len,
    size_t *consumed
) {
    for (size_t index = 0; index < state->pending.len; index += 1) {
        char ch = state->pending.data[index];
        if (ch == '\n') {
            size_t len = index;
            if (len > 0 && state->pending.data[len - 1] == '\r') { len -= 1; }
            *line = state->pending.data;
            *line_len = len;
            *consumed = index + 1;
            return 1;
        }
        if (ch == '\r') {
            if (index + 1 == state->pending.len && !state->eof) { return 0; }
            *line = state->pending.data;
            *line_len = index;
            *consumed = index + 1;
            if (index + 1 < state->pending.len
                && state->pending.data[index + 1] == '\n') {
                *consumed += 1;
            }
            return 1;
        }
    }
    if (state->eof && state->pending.len > 0) {
        *line = state->pending.data;
        *line_len = state->pending.len;
        *consumed = state->pending.len;
        return 1;
    }
    return 0;
}

static int nomo_http_stream_sse_valid_line(const char *line, size_t len) {
    size_t index = 0;
    while (index < len) {
        size_t width = 0;
        int status = nomo_http_stream_utf8_width(
            (const unsigned char *)line + index,
            len - index,
            &width
        );
        if (status != 1) { return 0; }
        index += width;
    }
    return 1;
}

static int nomo_http_stream_sse_pending_within(
    nomo_http_stream_state *state,
    uint64_t limit
) {
    uint64_t total = (uint64_t)state->sse_data.len;
    if ((uint64_t)state->sse_event.len > UINT64_MAX - total) { return 0; }
    total += (uint64_t)state->sse_event.len;
    if ((uint64_t)state->sse_id.len > UINT64_MAX - total) { return 0; }
    total += (uint64_t)state->sse_id.len;
    return total <= limit;
}

static int nomo_http_stream_sse_parse_retry(
    const char *value,
    size_t len,
    uint64_t *retry
) {
    if (len == 0) { return 0; }
    uint64_t out = 0;
    for (size_t index = 0; index < len; index += 1) {
        if (value[index] < '0' || value[index] > '9') { return 0; }
        uint64_t digit = (uint64_t)(value[index] - '0');
        if (out > (UINT64_MAX - digit) / UINT64_C(10)) { return 0; }
        out = out * UINT64_C(10) + digit;
    }
    *retry = out;
    return 1;
}

static @EVENT_OPTION@ nomo_http_stream_sse_dispatch(
    nomo_http_stream_state *state
) {
    size_t data_len = state->sse_data.len;
    if (data_len > 0 && state->sse_data.data[data_len - 1] == '\n') {
        data_len -= 1;
    }
    nomo_string event_name = state->sse_event.len == 0
        ? nomo_string_literal("message")
        : nomo_string_from_slice(state->sse_event.data, 0, state->sse_event.len);
    nomo_string data =
        nomo_string_from_slice(state->sse_data.data, 0, data_len);
    nomo_string id =
        nomo_string_from_slice(state->sse_id.data, 0, state->sse_id.len);
    @RETRY_OPTION@ retry = state->sse_retry_present
        ? (@RETRY_OPTION@){
            .tag = @RETRY_SOME@,
            .payload.@SOME_PAYLOAD@ = state->sse_retry_millis
        }
        : (@RETRY_OPTION@){.tag = @RETRY_NONE@};
    state->sse_data.len = 0;
    state->sse_event.len = 0;
    state->sse_retry_present = 0;
    state->sse_event_bytes = 0;
    return (@EVENT_OPTION@){
        .tag = @EVENT_SOME@,
        .payload.@SOME_PAYLOAD@ = (@SSE_EVENT@){
            .@EVENT_MEMBER@ = event_name,
            .@DATA_MEMBER@ = data,
            .@ID_MEMBER@ = id,
            .@RETRY_MEMBER@ = retry
        }
    };
}

static @SSE_RESULT@ @SSE_NAME@(
    @HTTP_STREAM@ stream,
    uint64_t max_event_bytes
) {
    if (max_event_bytes == 0
        || max_event_bytes > NOMO_HTTP_STREAM_MAX_EVENT_BYTES) {
        return nomo_http_stream_sse_error(
            "invalid_request",
            "invalid SSE event limit"
        );
    }
    nomo_http_stream_state *state = nomo_http_stream_find(stream.@HANDLE_MEMBER@);
    if (state == NULL || (state->mode != 0 && state->mode != 2)) {
        return nomo_http_stream_sse_error(
            "invalid_request",
            "invalid or closed HTTP stream"
        );
    }
    state->mode = 2;
    for (;;) {
        if (!nomo_http_stream_sse_pending_within(state, max_event_bytes)) {
            nomo_http_stream_detach(state);
            return nomo_http_stream_sse_error(
                "response_too_large",
                "SSE event exceeded its configured limit"
            );
        }
        const char *line = NULL;
        size_t line_len = 0;
        size_t consumed = 0;
        if (nomo_http_stream_sse_next_line(
                state,
                &line,
                &line_len,
                &consumed)) {
            if (!nomo_http_stream_sse_valid_line(line, line_len)) {
                nomo_http_stream_detach(state);
                return nomo_http_stream_sse_error(
                    "protocol",
                    "HTTP response stream was not valid UTF-8 text"
                );
            }
            if (state->sse_first_line) {
                state->sse_first_line = 0;
                if (line_len >= 3
                    && (unsigned char)line[0] == 0xef
                    && (unsigned char)line[1] == 0xbb
                    && (unsigned char)line[2] == 0xbf) {
                    line += 3;
                    line_len -= 3;
                }
            }
            if (line_len == 0) {
                nomo_http_stream_buffer_consume(&state->pending, consumed);
                if (state->sse_data.len > 0) {
                    @EVENT_OPTION@ event = nomo_http_stream_sse_dispatch(state);
                    return (@SSE_RESULT@){
                        .tag = @SSE_OK@,
                        .payload.@OK_PAYLOAD@ = event
                    };
                }
                state->sse_event.len = 0;
                state->sse_retry_present = 0;
                state->sse_event_bytes = 0;
                continue;
            }
            if ((uint64_t)line_len
                > max_event_bytes - state->sse_event_bytes) {
                nomo_http_stream_detach(state);
                return nomo_http_stream_sse_error(
                    "response_too_large",
                    "SSE event exceeded its configured limit"
                );
            }
            state->sse_event_bytes += (uint64_t)line_len;
            if (line[0] != ':') {
                size_t colon = 0;
                while (colon < line_len && line[colon] != ':') { colon += 1; }
                const char *value = colon < line_len ? line + colon + 1 : line + line_len;
                size_t value_len = colon < line_len ? line_len - colon - 1 : 0;
                if (value_len > 0 && value[0] == ' ') {
                    value += 1;
                    value_len -= 1;
                }
                if (colon == 4 && memcmp(line, "data", 4) == 0) {
                    nomo_http_stream_buffer_append(&state->sse_data, value, value_len);
                    nomo_http_stream_buffer_append(&state->sse_data, "\n", 1);
                } else if (colon == 5 && memcmp(line, "event", 5) == 0) {
                    nomo_http_stream_buffer_assign(&state->sse_event, value, value_len);
                } else if (colon == 2 && memcmp(line, "id", 2) == 0) {
                    nomo_http_stream_buffer_assign(&state->sse_id, value, value_len);
                } else if (colon == 5 && memcmp(line, "retry", 5) == 0) {
                    uint64_t retry = 0;
                    if (nomo_http_stream_sse_parse_retry(value, value_len, &retry)) {
                        state->sse_retry_present = 1;
                        state->sse_retry_millis = retry;
                    }
                }
            }
            nomo_http_stream_buffer_consume(&state->pending, consumed);
            continue;
        }
        if (state->too_large) {
            nomo_http_stream_detach(state);
            return nomo_http_stream_sse_error(
                "response_too_large",
                "HTTP response exceeded its configured limit"
            );
        }
        if (state->failure_code != NULL) {
            const char *code = state->failure_code;
            const char *message = nomo_http_stream_failure_message(code);
            nomo_http_stream_detach(state);
            return nomo_http_stream_sse_error(code, message);
        }
        if (state->eof) {
            if (state->sse_data.len > 0) {
                @EVENT_OPTION@ event = nomo_http_stream_sse_dispatch(state);
                return (@SSE_RESULT@){
                    .tag = @SSE_OK@,
                    .payload.@OK_PAYLOAD@ = event
                };
            }
            return (@SSE_RESULT@){
                .tag = @SSE_OK@,
                .payload.@OK_PAYLOAD@ = (@EVENT_OPTION@){.tag = @EVENT_NONE@}
            };
        }
        if ((uint64_t)state->pending.len
            > max_event_bytes - state->sse_event_bytes) {
            nomo_http_stream_detach(state);
            return nomo_http_stream_sse_error(
                "response_too_large",
                "SSE event exceeded its configured limit"
            );
        }
        nomo_http_stream_read_more(state);
    }
}

static void @CANCEL_NAME@(@HTTP_STREAM@ stream) {
    nomo_http_stream_state *state = nomo_http_stream_find(stream.@HANDLE_MEMBER@);
    if (state != NULL) { nomo_http_stream_detach(state); }
}

static void @CLOSE_NAME@(@HTTP_STREAM@ stream) {
    nomo_http_stream_state *state = nomo_http_stream_find(stream.@HANDLE_MEMBER@);
    if (state != NULL) { nomo_http_stream_detach(state); }
}
