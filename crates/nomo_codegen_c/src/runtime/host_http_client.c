#ifdef _WIN32
#include <windows.h>
typedef HMODULE nomo_http_library;
#define NOMO_HTTP_OPEN_LIBRARY(name) LoadLibraryA(name)
#define NOMO_HTTP_LOAD_SYMBOL(handle, name) GetProcAddress((handle), (name))
#else
#include <dlfcn.h>
typedef void *nomo_http_library;
#define NOMO_HTTP_OPEN_LIBRARY(name) dlopen((name), RTLD_NOW | RTLD_LOCAL)
#define NOMO_HTTP_LOAD_SYMBOL(handle, name) dlsym((handle), (name))
#endif

#define NOMO_HTTP_DEFAULT_TIMEOUT_MS UINT64_C(30000)
#define NOMO_HTTP_DEFAULT_MAX_RESPONSE_BYTES (UINT64_C(8) * UINT64_C(1024) * UINT64_C(1024))
#define NOMO_HTTP_HARD_MAX_RESPONSE_BYTES (UINT64_C(128) * UINT64_C(1024) * UINT64_C(1024))
#define NOMO_HTTP_MAX_TIMEOUT_MS UINT64_C(900000)
#define NOMO_HTTP_MAX_RESPONSE_HEADER_BYTES (UINT64_C(64) * UINT64_C(1024))

typedef void nomo_curl_easy;
typedef void nomo_curl_multi;
typedef struct nomo_curl_slist nomo_curl_slist;
typedef struct nomo_curl_msg {
    int msg;
    nomo_curl_easy *easy_handle;
    union {
        void *whatever;
        int result;
    } data;
} nomo_curl_msg;
typedef int (*nomo_curl_global_init_fn)(long);
typedef nomo_curl_easy *(*nomo_curl_easy_init_fn)(void);
typedef int (*nomo_curl_easy_setopt_fn)(nomo_curl_easy *, int, ...);
typedef int (*nomo_curl_easy_perform_fn)(nomo_curl_easy *);
typedef int (*nomo_curl_easy_getinfo_fn)(nomo_curl_easy *, int, ...);
typedef int (*nomo_curl_easy_pause_fn)(nomo_curl_easy *, int);
typedef void (*nomo_curl_easy_cleanup_fn)(nomo_curl_easy *);
typedef const char *(*nomo_curl_easy_strerror_fn)(int);
typedef nomo_curl_slist *(*nomo_curl_slist_append_fn)(nomo_curl_slist *, const char *);
typedef void (*nomo_curl_slist_free_all_fn)(nomo_curl_slist *);
typedef nomo_curl_multi *(*nomo_curl_multi_init_fn)(void);
typedef int (*nomo_curl_multi_add_handle_fn)(nomo_curl_multi *, nomo_curl_easy *);
typedef int (*nomo_curl_multi_remove_handle_fn)(nomo_curl_multi *, nomo_curl_easy *);
typedef int (*nomo_curl_multi_perform_fn)(nomo_curl_multi *, int *);
typedef int (*nomo_curl_multi_poll_fn)(
    nomo_curl_multi *,
    void *,
    unsigned int,
    int,
    int *
);
typedef int (*nomo_curl_multi_wait_fn)(
    nomo_curl_multi *,
    void *,
    unsigned int,
    int,
    int *
);
typedef nomo_curl_msg *(*nomo_curl_multi_info_read_fn)(nomo_curl_multi *, int *);
typedef int (*nomo_curl_multi_cleanup_fn)(nomo_curl_multi *);

typedef struct nomo_http_curl_api {
    int attempted;
    int available;
    nomo_http_library library;
    nomo_curl_global_init_fn global_init;
    nomo_curl_easy_init_fn easy_init;
    nomo_curl_easy_setopt_fn easy_setopt;
    nomo_curl_easy_perform_fn easy_perform;
    nomo_curl_easy_getinfo_fn easy_getinfo;
    nomo_curl_easy_pause_fn easy_pause;
    nomo_curl_easy_cleanup_fn easy_cleanup;
    nomo_curl_easy_strerror_fn easy_strerror;
    nomo_curl_slist_append_fn slist_append;
    nomo_curl_slist_free_all_fn slist_free_all;
    nomo_curl_multi_init_fn multi_init;
    nomo_curl_multi_add_handle_fn multi_add_handle;
    nomo_curl_multi_remove_handle_fn multi_remove_handle;
    nomo_curl_multi_perform_fn multi_perform;
    nomo_curl_multi_poll_fn multi_poll;
    nomo_curl_multi_wait_fn multi_wait;
    nomo_curl_multi_info_read_fn multi_info_read;
    nomo_curl_multi_cleanup_fn multi_cleanup;
} nomo_http_curl_api;

typedef struct nomo_http_body_buffer {
    char *data;
    size_t len;
    size_t cap;
    size_t limit;
    int too_large;
} nomo_http_body_buffer;

typedef struct nomo_http_header_buffer {
    @HTTP_HEADER_ARRAY@ values;
    size_t bytes;
    int too_large;
} nomo_http_header_buffer;

enum {
    NOMO_CURLOPT_WRITEDATA = 10001,
    NOMO_CURLOPT_URL = 10002,
    NOMO_CURLOPT_WRITEFUNCTION = 20011,
    NOMO_CURLOPT_POSTFIELDS = 10015,
    NOMO_CURLOPT_USERAGENT = 10018,
    NOMO_CURLOPT_HTTPHEADER = 10023,
    NOMO_CURLOPT_HEADERDATA = 10029,
    NOMO_CURLOPT_CUSTOMREQUEST = 10036,
    NOMO_CURLOPT_FOLLOWLOCATION = 52,
    NOMO_CURLOPT_SSL_VERIFYPEER = 64,
    NOMO_CURLOPT_SSL_VERIFYHOST = 81,
    NOMO_CURLOPT_NOSIGNAL = 99,
    NOMO_CURLOPT_POSTFIELDSIZE_LARGE = 30120,
    NOMO_CURLOPT_CAINFO = 10065,
    NOMO_CURLOPT_HEADERFUNCTION = 20079,
    NOMO_CURLOPT_TIMEOUT_MS = 155,
    NOMO_CURLOPT_CONNECTTIMEOUT_MS = 156,
    NOMO_CURLINFO_RESPONSE_CODE = 0x200002
};

static nomo_http_curl_api nomo_http_curl = {0};

static int nomo_http_ascii_case_equal(const char *left, const char *right) {
    while (*left != '\0' && *right != '\0') {
        unsigned char a = (unsigned char)*left;
        unsigned char b = (unsigned char)*right;
        if (a >= 'A' && a <= 'Z') { a = (unsigned char)(a + ('a' - 'A')); }
        if (b >= 'A' && b <= 'Z') { b = (unsigned char)(b + ('a' - 'A')); }
        if (a != b) { return 0; }
        left += 1;
        right += 1;
    }
    return *left == '\0' && *right == '\0';
}

static int nomo_http_load_curl(void) {
    if (nomo_http_curl.attempted) { return nomo_http_curl.available; }
    nomo_http_curl.attempted = 1;
#ifdef _WIN32
    const char *names[] = {"libcurl.dll", "libcurl-x64.dll", "curl.dll", NULL};
#elif defined(__APPLE__)
    const char *names[] = {"libcurl.4.dylib", "/usr/lib/libcurl.4.dylib", "libcurl.dylib", NULL};
#else
    const char *names[] = {"libcurl.so.4", "libcurl.so", NULL};
#endif
    for (size_t index = 0; names[index] != NULL; index += 1) {
        nomo_http_curl.library = NOMO_HTTP_OPEN_LIBRARY(names[index]);
        if (nomo_http_curl.library != NULL) { break; }
    }
    if (nomo_http_curl.library == NULL) { return 0; }
#define NOMO_HTTP_CURL_SYMBOL(field, type, name) \
    nomo_http_curl.field = (type)NOMO_HTTP_LOAD_SYMBOL(nomo_http_curl.library, name); \
    if (nomo_http_curl.field == NULL) { return 0; }
    NOMO_HTTP_CURL_SYMBOL(global_init, nomo_curl_global_init_fn, "curl_global_init")
    NOMO_HTTP_CURL_SYMBOL(easy_init, nomo_curl_easy_init_fn, "curl_easy_init")
    NOMO_HTTP_CURL_SYMBOL(easy_setopt, nomo_curl_easy_setopt_fn, "curl_easy_setopt")
    NOMO_HTTP_CURL_SYMBOL(easy_perform, nomo_curl_easy_perform_fn, "curl_easy_perform")
    NOMO_HTTP_CURL_SYMBOL(easy_getinfo, nomo_curl_easy_getinfo_fn, "curl_easy_getinfo")
    NOMO_HTTP_CURL_SYMBOL(easy_cleanup, nomo_curl_easy_cleanup_fn, "curl_easy_cleanup")
    NOMO_HTTP_CURL_SYMBOL(easy_strerror, nomo_curl_easy_strerror_fn, "curl_easy_strerror")
    NOMO_HTTP_CURL_SYMBOL(slist_append, nomo_curl_slist_append_fn, "curl_slist_append")
    NOMO_HTTP_CURL_SYMBOL(slist_free_all, nomo_curl_slist_free_all_fn, "curl_slist_free_all")
#undef NOMO_HTTP_CURL_SYMBOL
    nomo_http_curl.easy_pause = (nomo_curl_easy_pause_fn)NOMO_HTTP_LOAD_SYMBOL(
        nomo_http_curl.library,
        "curl_easy_pause"
    );
    nomo_http_curl.multi_init = (nomo_curl_multi_init_fn)NOMO_HTTP_LOAD_SYMBOL(
        nomo_http_curl.library,
        "curl_multi_init"
    );
    nomo_http_curl.multi_add_handle =
        (nomo_curl_multi_add_handle_fn)NOMO_HTTP_LOAD_SYMBOL(
            nomo_http_curl.library,
            "curl_multi_add_handle"
        );
    nomo_http_curl.multi_remove_handle =
        (nomo_curl_multi_remove_handle_fn)NOMO_HTTP_LOAD_SYMBOL(
            nomo_http_curl.library,
            "curl_multi_remove_handle"
        );
    nomo_http_curl.multi_perform = (nomo_curl_multi_perform_fn)NOMO_HTTP_LOAD_SYMBOL(
        nomo_http_curl.library,
        "curl_multi_perform"
    );
    nomo_http_curl.multi_poll = (nomo_curl_multi_poll_fn)NOMO_HTTP_LOAD_SYMBOL(
        nomo_http_curl.library,
        "curl_multi_poll"
    );
    nomo_http_curl.multi_wait = (nomo_curl_multi_wait_fn)NOMO_HTTP_LOAD_SYMBOL(
        nomo_http_curl.library,
        "curl_multi_wait"
    );
    nomo_http_curl.multi_info_read =
        (nomo_curl_multi_info_read_fn)NOMO_HTTP_LOAD_SYMBOL(
            nomo_http_curl.library,
            "curl_multi_info_read"
        );
    nomo_http_curl.multi_cleanup = (nomo_curl_multi_cleanup_fn)NOMO_HTTP_LOAD_SYMBOL(
        nomo_http_curl.library,
        "curl_multi_cleanup"
    );
    if (nomo_http_curl.global_init(3L) != 0) { return 0; }
    nomo_http_curl.available = 1;
    return 1;
}

static @RESULT@ nomo_http_error(const char *code, const char *message) {
    return (@RESULT@){
        .tag = @ERR@,
        .payload.@ERR_PAYLOAD@ = (@HTTP_ERROR@){
            .@CODE_MEMBER@ = nomo_string_from_cstr(code),
            .@MESSAGE_MEMBER@ = nomo_string_from_cstr(message)
        }
    };
}

static size_t nomo_http_write_body(char *data, size_t size, size_t count, void *userdata) {
    nomo_http_body_buffer *buffer = (nomo_http_body_buffer *)userdata;
    if (count != 0 && size > SIZE_MAX / count) {
        buffer->too_large = 1;
        return 0;
    }
    size_t incoming = size * count;
    if (incoming > buffer->limit - buffer->len) {
        buffer->too_large = 1;
        return 0;
    }
    size_t needed = buffer->len + incoming + 1;
    if (needed > buffer->cap) {
        size_t cap = buffer->cap == 0 ? 4096 : buffer->cap;
        while (cap < needed) {
            if (cap > (buffer->limit + 1) / 2) {
                cap = buffer->limit + 1;
                break;
            }
            cap *= 2;
        }
        char *next = (char *)realloc(buffer->data, cap);
        if (next == NULL) { nomo_panic("out of memory"); }
        buffer->data = next;
        buffer->cap = cap;
    }
    if (incoming > 0) { memcpy(buffer->data + buffer->len, data, incoming); }
    buffer->len += incoming;
    buffer->data[buffer->len] = '\0';
    return incoming;
}

static size_t nomo_http_write_header(char *data, size_t size, size_t count, void *userdata) {
    nomo_http_header_buffer *buffer = (nomo_http_header_buffer *)userdata;
    if (count != 0 && size > SIZE_MAX / count) {
        buffer->too_large = 1;
        return 0;
    }
    size_t incoming = size * count;
    if (incoming > (size_t)NOMO_HTTP_MAX_RESPONSE_HEADER_BYTES - buffer->bytes) {
        buffer->too_large = 1;
        return 0;
    }
    buffer->bytes += incoming;
    if (incoming >= 5 && memcmp(data, "HTTP/", 5) == 0) {
        @HTTP_HEADER_ARRAY_RELEASE@(buffer->values);
        buffer->values = @HTTP_HEADER_ARRAY_NEW@();
        buffer->bytes = incoming;
        return incoming;
    }
    size_t colon = 0;
    while (colon < incoming && data[colon] != ':') { colon += 1; }
    if (colon == incoming || colon == 0) { return incoming; }
    size_t name_len = colon;
    while (name_len > 0 && (data[name_len - 1] == ' ' || data[name_len - 1] == '\t')) {
        name_len -= 1;
    }
    size_t value_start = colon + 1;
    while (value_start < incoming && (data[value_start] == ' ' || data[value_start] == '\t')) {
        value_start += 1;
    }
    size_t value_end = incoming;
    while (value_end > value_start
        && (data[value_end - 1] == '\r'
            || data[value_end - 1] == '\n'
            || data[value_end - 1] == ' '
            || data[value_end - 1] == '\t')) {
        value_end -= 1;
    }
    @HTTP_HEADER@ header = (@HTTP_HEADER@){
        .@NAME_MEMBER@ = nomo_string_from_slice(data, 0, name_len),
        .@VALUE_MEMBER@ = nomo_string_from_slice(data, value_start, value_end - value_start)
    };
    buffer->values = @HTTP_HEADER_ARRAY_PUSH@(buffer->values, header);
    @HTTP_HEADER_RELEASE@(header);
    return incoming;
}

static int nomo_http_is_token_char(unsigned char value) {
    if ((value >= 'a' && value <= 'z')
        || (value >= 'A' && value <= 'Z')
        || (value >= '0' && value <= '9')) {
        return 1;
    }
    return value == '!' || value == '#' || value == '$' || value == '%'
        || value == '&' || value == '\'' || value == '*' || value == '+'
        || value == '-' || value == '.' || value == '^' || value == '_'
        || value == '`' || value == '|' || value == '~';
}

static int nomo_http_validate_header(@HTTP_HEADER@ header) {
    if (header.@NAME_MEMBER@.data[0] == '\0') { return 0; }
    for (const unsigned char *cursor = (const unsigned char *)header.@NAME_MEMBER@.data;
         *cursor != '\0';
         cursor += 1) {
        if (!nomo_http_is_token_char(*cursor)) { return 0; }
    }
    if (strchr(header.@VALUE_MEMBER@.data, '\r') != NULL
        || strchr(header.@VALUE_MEMBER@.data, '\n') != NULL) {
        return 0;
    }
    const char *name = header.@NAME_MEMBER@.data;
    return !nomo_http_ascii_case_equal(name, "Host")
        && !nomo_http_ascii_case_equal(name, "Connection")
        && !nomo_http_ascii_case_equal(name, "Content-Length")
        && !nomo_http_ascii_case_equal(name, "Transfer-Encoding")
        && !nomo_http_ascii_case_equal(name, "Expect");
}

static int nomo_http_validate_url(const char *url) {
    size_t url_len = 0;
    for (const unsigned char *cursor = (const unsigned char *)url;
         *cursor != '\0';
         cursor += 1) {
        if (*cursor <= 0x20 || *cursor == 0x7f) { return 0; }
        url_len += 1;
        if (url_len > 16384) { return 0; }
    }
    const char *authority = NULL;
    if (strncmp(url, "http://", 7) == 0) {
        authority = url + 7;
    } else if (strncmp(url, "https://", 8) == 0) {
        authority = url + 8;
    } else {
        return 0;
    }
    if (*authority == '\0' || *authority == '/' || *authority == '?' || *authority == '#') {
        return 0;
    }
    const char *authority_end = authority;
    while (*authority_end != '\0'
        && *authority_end != '/'
        && *authority_end != '?'
        && *authority_end != '#') {
        authority_end += 1;
    }
    for (const char *cursor = authority; cursor < authority_end; cursor += 1) {
        if (*cursor == '@') { return 0; }
    }
    return strchr(url, '#') == NULL;
}

static nomo_curl_slist *nomo_http_append_header(
    nomo_curl_slist *list,
    const char *name,
    const char *value
) {
    size_t name_len = strlen(name);
    size_t value_len = strlen(value);
    char *line = (char *)malloc(name_len + value_len + 3);
    if (line == NULL) { nomo_panic("out of memory"); }
    memcpy(line, name, name_len);
    line[name_len] = ':';
    line[name_len + 1] = ' ';
    memcpy(line + name_len + 2, value, value_len + 1);
    nomo_curl_slist *next = nomo_http_curl.slist_append(list, line);
    free(line);
    return next;
}

static const char *nomo_http_error_code_for_curl(int code) {
    if (code == 6) { return "dns"; }
    if (code == 7) { return "connect"; }
    if (code == 28) { return "timeout"; }
    if (code == 35 || code == 51 || code == 53 || code == 54 || code == 58
        || code == 59 || code == 60 || code == 64 || code == 66 || code == 77
        || code == 80 || code == 82 || code == 83 || code == 90 || code == 91) {
        return "tls";
    }
    return "transport";
}

static const char *nomo_http_error_message_for_code(const char *code) {
    if (strcmp(code, "dns") == 0) { return "HTTP host resolution failed"; }
    if (strcmp(code, "connect") == 0) { return "HTTP connection failed"; }
    if (strcmp(code, "tls") == 0) { return "HTTPS certificate or handshake failed"; }
    if (strcmp(code, "timeout") == 0) { return "HTTP request timed out"; }
    return "HTTP transport failed";
}

#ifdef _WIN32
typedef void *nomo_winhttp_handle;
typedef nomo_winhttp_handle (WINAPI *nomo_winhttp_open_fn)(
    const wchar_t *,
    unsigned long,
    const wchar_t *,
    const wchar_t *,
    unsigned long
);
typedef nomo_winhttp_handle (WINAPI *nomo_winhttp_connect_fn)(
    nomo_winhttp_handle,
    const wchar_t *,
    unsigned short,
    unsigned long
);
typedef nomo_winhttp_handle (WINAPI *nomo_winhttp_open_request_fn)(
    nomo_winhttp_handle,
    const wchar_t *,
    const wchar_t *,
    const wchar_t *,
    const wchar_t *,
    const wchar_t *const *,
    unsigned long
);
typedef int (WINAPI *nomo_winhttp_add_headers_fn)(
    nomo_winhttp_handle,
    const wchar_t *,
    unsigned long,
    unsigned long
);
typedef int (WINAPI *nomo_winhttp_send_request_fn)(
    nomo_winhttp_handle,
    const wchar_t *,
    unsigned long,
    void *,
    unsigned long,
    unsigned long,
    uintptr_t
);
typedef int (WINAPI *nomo_winhttp_receive_response_fn)(nomo_winhttp_handle, void *);
typedef int (WINAPI *nomo_winhttp_query_headers_fn)(
    nomo_winhttp_handle,
    unsigned long,
    const wchar_t *,
    void *,
    unsigned long *,
    unsigned long *
);
typedef int (WINAPI *nomo_winhttp_read_data_fn)(
    nomo_winhttp_handle,
    void *,
    unsigned long,
    unsigned long *
);
typedef int (WINAPI *nomo_winhttp_set_timeouts_fn)(
    nomo_winhttp_handle,
    int,
    int,
    int,
    int
);
typedef int (WINAPI *nomo_winhttp_set_option_fn)(
    nomo_winhttp_handle,
    unsigned long,
    void *,
    unsigned long
);
typedef int (WINAPI *nomo_winhttp_close_handle_fn)(nomo_winhttp_handle);

typedef struct nomo_winhttp_api {
    int attempted;
    int available;
    HMODULE library;
    nomo_winhttp_open_fn open;
    nomo_winhttp_connect_fn connect;
    nomo_winhttp_open_request_fn open_request;
    nomo_winhttp_add_headers_fn add_headers;
    nomo_winhttp_send_request_fn send_request;
    nomo_winhttp_receive_response_fn receive_response;
    nomo_winhttp_query_headers_fn query_headers;
    nomo_winhttp_read_data_fn read_data;
    nomo_winhttp_set_timeouts_fn set_timeouts;
    nomo_winhttp_set_option_fn set_option;
    nomo_winhttp_close_handle_fn close_handle;
} nomo_winhttp_api;

typedef struct nomo_winhttp_url {
    wchar_t *host;
    wchar_t *path;
    unsigned short port;
    int secure;
} nomo_winhttp_url;

static nomo_winhttp_api nomo_winhttp = {0};

static int nomo_winhttp_load(void) {
    if (nomo_winhttp.attempted) { return nomo_winhttp.available; }
    nomo_winhttp.attempted = 1;
    nomo_winhttp.library = LoadLibraryA("winhttp.dll");
    if (nomo_winhttp.library == NULL) { return 0; }
#define NOMO_WINHTTP_SYMBOL(field, type, name) \
    nomo_winhttp.field = (type)GetProcAddress(nomo_winhttp.library, name); \
    if (nomo_winhttp.field == NULL) { return 0; }
    NOMO_WINHTTP_SYMBOL(open, nomo_winhttp_open_fn, "WinHttpOpen")
    NOMO_WINHTTP_SYMBOL(connect, nomo_winhttp_connect_fn, "WinHttpConnect")
    NOMO_WINHTTP_SYMBOL(open_request, nomo_winhttp_open_request_fn, "WinHttpOpenRequest")
    NOMO_WINHTTP_SYMBOL(add_headers, nomo_winhttp_add_headers_fn, "WinHttpAddRequestHeaders")
    NOMO_WINHTTP_SYMBOL(send_request, nomo_winhttp_send_request_fn, "WinHttpSendRequest")
    NOMO_WINHTTP_SYMBOL(
        receive_response,
        nomo_winhttp_receive_response_fn,
        "WinHttpReceiveResponse"
    )
    NOMO_WINHTTP_SYMBOL(query_headers, nomo_winhttp_query_headers_fn, "WinHttpQueryHeaders")
    NOMO_WINHTTP_SYMBOL(read_data, nomo_winhttp_read_data_fn, "WinHttpReadData")
    NOMO_WINHTTP_SYMBOL(set_timeouts, nomo_winhttp_set_timeouts_fn, "WinHttpSetTimeouts")
    NOMO_WINHTTP_SYMBOL(set_option, nomo_winhttp_set_option_fn, "WinHttpSetOption")
    NOMO_WINHTTP_SYMBOL(close_handle, nomo_winhttp_close_handle_fn, "WinHttpCloseHandle")
#undef NOMO_WINHTTP_SYMBOL
    nomo_winhttp.available = 1;
    return 1;
}

static wchar_t *nomo_winhttp_wide_from_utf8(const char *value) {
    int count = MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, value, -1, NULL, 0);
    if (count <= 0) { return NULL; }
    wchar_t *wide = (wchar_t *)malloc((size_t)count * sizeof(wchar_t));
    if (wide == NULL) { nomo_panic("out of memory"); }
    if (MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, value, -1, wide, count) <= 0) {
        free(wide);
        return NULL;
    }
    return wide;
}

static char *nomo_winhttp_utf8_from_wide(const wchar_t *value) {
    int count = WideCharToMultiByte(CP_UTF8, 0, value, -1, NULL, 0, NULL, NULL);
    if (count <= 0) { return NULL; }
    char *utf8 = (char *)malloc((size_t)count);
    if (utf8 == NULL) { nomo_panic("out of memory"); }
    if (WideCharToMultiByte(CP_UTF8, 0, value, -1, utf8, count, NULL, NULL) <= 0) {
        free(utf8);
        return NULL;
    }
    return utf8;
}

static char *nomo_winhttp_copy_range(const char *start, size_t len) {
    char *copy = (char *)malloc(len + 1);
    if (copy == NULL) { nomo_panic("out of memory"); }
    memcpy(copy, start, len);
    copy[len] = '\0';
    return copy;
}

static void nomo_winhttp_url_release(nomo_winhttp_url *url) {
    free(url->host);
    free(url->path);
    url->host = NULL;
    url->path = NULL;
}

static int nomo_winhttp_parse_url(const char *value, nomo_winhttp_url *out) {
    size_t scheme_len = strncmp(value, "https://", 8) == 0 ? 8 : 7;
    out->secure = scheme_len == 8;
    out->port = out->secure ? 443 : 80;
    const char *authority = value + scheme_len;
    const char *authority_end = authority;
    while (*authority_end != '\0' && *authority_end != '/' && *authority_end != '?') {
        authority_end += 1;
    }
    const char *host_start = authority;
    const char *host_end = authority_end;
    const char *port_start = NULL;
    if (*host_start == '[') {
        const char *closing = host_start + 1;
        while (closing < authority_end && *closing != ']') { closing += 1; }
        if (closing == authority_end) { return 0; }
        host_start += 1;
        host_end = closing;
        if (closing + 1 < authority_end) {
            if (closing[1] != ':') { return 0; }
            port_start = closing + 2;
        }
    } else {
        const char *colon = NULL;
        for (const char *cursor = authority; cursor < authority_end; cursor += 1) {
            if (*cursor == ':') {
                if (colon != NULL) { return 0; }
                colon = cursor;
            }
        }
        if (colon != NULL) {
            host_end = colon;
            port_start = colon + 1;
        }
    }
    if (host_start == host_end) { return 0; }
    if (port_start != NULL) {
        unsigned long port = 0;
        if (port_start == authority_end) { return 0; }
        for (const char *cursor = port_start; cursor < authority_end; cursor += 1) {
            if (*cursor < '0' || *cursor > '9') { return 0; }
            port = port * 10UL + (unsigned long)(*cursor - '0');
            if (port > 65535UL) { return 0; }
        }
        if (port == 0) { return 0; }
        out->port = (unsigned short)port;
    }
    char *host = nomo_winhttp_copy_range(host_start, (size_t)(host_end - host_start));
    out->host = nomo_winhttp_wide_from_utf8(host);
    free(host);
    if (out->host == NULL) { return 0; }
    char *path = NULL;
    if (*authority_end == '\0') {
        path = nomo_winhttp_copy_range("/", 1);
    } else if (*authority_end == '?') {
        size_t suffix_len = strlen(authority_end);
        path = (char *)malloc(suffix_len + 2);
        if (path == NULL) { nomo_panic("out of memory"); }
        path[0] = '/';
        memcpy(path + 1, authority_end, suffix_len + 1);
    } else {
        path = nomo_winhttp_copy_range(authority_end, strlen(authority_end));
    }
    out->path = nomo_winhttp_wide_from_utf8(path);
    free(path);
    if (out->path == NULL) {
        nomo_winhttp_url_release(out);
        return 0;
    }
    return 1;
}

static const char *nomo_winhttp_error_code(unsigned long error) {
    if (error == 12002UL) { return "timeout"; }
    if (error == 12007UL) { return "dns"; }
    if (error == 12029UL || error == 12030UL) { return "connect"; }
    if ((error >= 12157UL && error <= 12186UL) || error == 12037UL || error == 12045UL) {
        return "tls";
    }
    return "transport";
}

static @RESULT@ nomo_winhttp_error_result(unsigned long error) {
    const char *code = nomo_winhttp_error_code(error);
    return nomo_http_error(code, nomo_http_error_message_for_code(code));
}

static int nomo_winhttp_remaining_timeout(ULONGLONG deadline) {
    ULONGLONG now = GetTickCount64();
    if (now >= deadline) { return 0; }
    ULONGLONG remaining = deadline - now;
    return remaining > (ULONGLONG)INT_MAX ? INT_MAX : (int)remaining;
}

static void nomo_winhttp_close_all(
    nomo_winhttp_handle request,
    nomo_winhttp_handle connection,
    nomo_winhttp_handle session
) {
    if (request != NULL) { nomo_winhttp.close_handle(request); }
    if (connection != NULL) { nomo_winhttp.close_handle(connection); }
    if (session != NULL) { nomo_winhttp.close_handle(session); }
}

static @RESULT@ nomo_http_send_winhttp(@HTTP_REQUEST@ request) {
    if (!nomo_winhttp_load()) {
        return nomo_http_error(
            "runtime_unavailable",
            "the native HTTP runtime is unavailable on this host"
        );
    }
    nomo_winhttp_url url = {0};
    if (!nomo_winhttp_parse_url(request.@URL_MEMBER@.data, &url)) {
        return nomo_http_error("invalid_request", "invalid bounded HTTP request");
    }
    wchar_t *method = nomo_winhttp_wide_from_utf8(request.@METHOD_MEMBER@.data);
    if (method == NULL) {
        nomo_winhttp_url_release(&url);
        return nomo_http_error("invalid_request", "HTTP method is not valid UTF-8");
    }
    ULONGLONG deadline = GetTickCount64() + (ULONGLONG)request.@TIMEOUT_MEMBER@;
    nomo_winhttp_handle session = nomo_winhttp.open(L"nomo/0.1", 0, NULL, NULL, 0);
    if (session == NULL) {
        unsigned long error = GetLastError();
        free(method);
        nomo_winhttp_url_release(&url);
        return nomo_winhttp_error_result(error);
    }
    int timeout = (int)request.@TIMEOUT_MEMBER@;
    if (!nomo_winhttp.set_timeouts(session, timeout, timeout, timeout, timeout)) {
        unsigned long error = GetLastError();
        free(method);
        nomo_winhttp_url_release(&url);
        nomo_winhttp_close_all(NULL, NULL, session);
        return nomo_winhttp_error_result(error);
    }
    nomo_winhttp_handle connection = nomo_winhttp.connect(session, url.host, url.port, 0);
    if (connection == NULL) {
        unsigned long error = GetLastError();
        free(method);
        nomo_winhttp_url_release(&url);
        nomo_winhttp_close_all(NULL, NULL, session);
        return nomo_winhttp_error_result(error);
    }
    int remaining = nomo_winhttp_remaining_timeout(deadline);
    if (remaining == 0) {
        free(method);
        nomo_winhttp_url_release(&url);
        nomo_winhttp_close_all(NULL, connection, session);
        return nomo_http_error("timeout", "HTTP request timed out");
    }
    unsigned long flags = url.secure ? 0x00800000UL : 0;
    nomo_winhttp_handle handle = nomo_winhttp.open_request(
        connection,
        method,
        url.path,
        NULL,
        NULL,
        NULL,
        flags
    );
    free(method);
    nomo_winhttp_url_release(&url);
    if (handle == NULL) {
        unsigned long error = GetLastError();
        nomo_winhttp_close_all(NULL, connection, session);
        return nomo_winhttp_error_result(error);
    }
    if (!nomo_winhttp.set_timeouts(handle, remaining, remaining, remaining, remaining)) {
        unsigned long error = GetLastError();
        nomo_winhttp_close_all(handle, connection, session);
        return nomo_winhttp_error_result(error);
    }
    for (size_t index = 0; index < request.@HEADERS_MEMBER@.len; index += 1) {
        @HTTP_HEADER@ header = request.@HEADERS_MEMBER@.data[index];
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
            || !nomo_winhttp.add_headers(handle, wide, (unsigned long)-1L, 0x20000000UL)) {
            unsigned long error = GetLastError();
            free(wide);
            nomo_winhttp_close_all(handle, connection, session);
            return nomo_winhttp_error_result(error);
        }
        free(wide);
    }
    size_t body_len = strlen(request.@BODY_MEMBER@.data);
    if (body_len > UINT32_MAX) {
        nomo_winhttp_close_all(handle, connection, session);
        return nomo_http_error("invalid_request", "HTTP request body is too large");
    }
    remaining = nomo_winhttp_remaining_timeout(deadline);
    if (remaining == 0) {
        nomo_winhttp_close_all(handle, connection, session);
        return nomo_http_error("timeout", "HTTP request timed out");
    }
    if (!nomo_winhttp.set_timeouts(handle, remaining, remaining, remaining, remaining)
        || !nomo_winhttp.send_request(
            handle,
            NULL,
            0,
            body_len == 0 ? NULL : request.@BODY_MEMBER@.data,
            (unsigned long)body_len,
            (unsigned long)body_len,
            0)) {
        unsigned long error = GetLastError();
        nomo_winhttp_close_all(handle, connection, session);
        return nomo_winhttp_error_result(error);
    }
    remaining = nomo_winhttp_remaining_timeout(deadline);
    if (remaining == 0) {
        nomo_winhttp_close_all(handle, connection, session);
        return nomo_http_error("timeout", "HTTP request timed out");
    }
    if (!nomo_winhttp.set_timeouts(handle, remaining, remaining, remaining, remaining)
        || !nomo_winhttp.receive_response(handle, NULL)) {
        unsigned long error = GetLastError();
        nomo_winhttp_close_all(handle, connection, session);
        return nomo_winhttp_error_result(error);
    }
    unsigned long status = 0;
    unsigned long status_size = (unsigned long)sizeof(status);
    if (!nomo_winhttp.query_headers(
            handle,
            19UL | 0x20000000UL,
            NULL,
            &status,
            &status_size,
            NULL)) {
        unsigned long error = GetLastError();
        nomo_winhttp_close_all(handle, connection, session);
        return nomo_winhttp_error_result(error);
    }
    nomo_http_header_buffer response_headers = {
        .values = @HTTP_HEADER_ARRAY_NEW@(),
        .bytes = 0,
        .too_large = 0
    };
    unsigned long raw_size = 0;
    int raw_query =
        nomo_winhttp.query_headers(handle, 22UL, NULL, NULL, &raw_size, NULL);
    unsigned long raw_error = raw_query ? 0 : GetLastError();
    if (!raw_query && raw_error != 122UL) {
        nomo_winhttp_close_all(handle, connection, session);
        @HTTP_HEADER_ARRAY_RELEASE@(response_headers.values);
        return nomo_winhttp_error_result(raw_error);
    }
    if (raw_size > 0) {
        if ((uint64_t)raw_size > NOMO_HTTP_MAX_RESPONSE_HEADER_BYTES * sizeof(wchar_t)) {
            nomo_winhttp_close_all(handle, connection, session);
            @HTTP_HEADER_ARRAY_RELEASE@(response_headers.values);
            return nomo_http_error(
                "response_too_large",
                "HTTP response exceeded its configured limit"
            );
        }
        wchar_t *raw = (wchar_t *)malloc((size_t)raw_size);
        if (raw == NULL) { nomo_panic("out of memory"); }
        if (!nomo_winhttp.query_headers(handle, 22UL, NULL, raw, &raw_size, NULL)) {
            unsigned long error = GetLastError();
            free(raw);
            nomo_winhttp_close_all(handle, connection, session);
            @HTTP_HEADER_ARRAY_RELEASE@(response_headers.values);
            return nomo_winhttp_error_result(error);
        }
        char *utf8 = nomo_winhttp_utf8_from_wide(raw);
        free(raw);
        if (utf8 == NULL) {
            nomo_winhttp_close_all(handle, connection, session);
            @HTTP_HEADER_ARRAY_RELEASE@(response_headers.values);
            return nomo_http_error("protocol", "HTTP response headers are not valid UTF-8");
        }
        char *line = utf8;
        while (*line != '\0') {
            char *end = strstr(line, "\r\n");
            size_t len = end == NULL ? strlen(line) : (size_t)(end - line) + 2;
            if (nomo_http_write_header(line, 1, len, &response_headers) != len) {
                free(utf8);
                nomo_winhttp_close_all(handle, connection, session);
                @HTTP_HEADER_ARRAY_RELEASE@(response_headers.values);
                return nomo_http_error(
                    "response_too_large",
                    "HTTP response exceeded its configured limit"
                );
            }
            if (end == NULL) { break; }
            line = end + 2;
        }
        free(utf8);
    }
    nomo_http_body_buffer body = {
        .data = NULL,
        .len = 0,
        .cap = 0,
        .limit = (size_t)request.@MAX_RESPONSE_MEMBER@,
        .too_large = 0
    };
    for (;;) {
        remaining = nomo_winhttp_remaining_timeout(deadline);
        if (remaining == 0) {
            free(body.data);
            nomo_winhttp_close_all(handle, connection, session);
            @HTTP_HEADER_ARRAY_RELEASE@(response_headers.values);
            return nomo_http_error("timeout", "HTTP request timed out");
        }
        if (!nomo_winhttp.set_timeouts(handle, remaining, remaining, remaining, remaining)) {
            unsigned long error = GetLastError();
            free(body.data);
            nomo_winhttp_close_all(handle, connection, session);
            @HTTP_HEADER_ARRAY_RELEASE@(response_headers.values);
            return nomo_winhttp_error_result(error);
        }
        unsigned char chunk[4096];
        unsigned long received = 0;
        if (!nomo_winhttp.read_data(handle, chunk, (unsigned long)sizeof(chunk), &received)) {
            unsigned long error = GetLastError();
            free(body.data);
            nomo_winhttp_close_all(handle, connection, session);
            @HTTP_HEADER_ARRAY_RELEASE@(response_headers.values);
            return nomo_winhttp_error_result(error);
        }
        if (received == 0) { break; }
        if (nomo_http_write_body((char *)chunk, 1, received, &body) != received) {
            free(body.data);
            nomo_winhttp_close_all(handle, connection, session);
            @HTTP_HEADER_ARRAY_RELEASE@(response_headers.values);
            return nomo_http_error(
                "response_too_large",
                "HTTP response exceeded its configured limit"
            );
        }
    }
    nomo_winhttp_close_all(handle, connection, session);
    nomo_string response_body = body.data == NULL
        ? nomo_string_literal("")
        : nomo_string_owned(body.data);
    return (@RESULT@){
        .tag = @OK@,
        .payload.@OK_PAYLOAD@ = (@HTTP_RESPONSE@){
            .@STATUS_MEMBER@ = (int64_t)status,
            .@HEADERS_MEMBER@ = response_headers.values,
            .@BODY_MEMBER@ = response_body
        }
    };
}
#endif

static void nomo_http_cleanup(
    nomo_curl_easy *easy,
    nomo_curl_slist *request_headers,
    nomo_http_body_buffer *body,
    nomo_http_header_buffer *headers,
    int release_response
) {
    if (easy != NULL) { nomo_http_curl.easy_cleanup(easy); }
    if (request_headers != NULL) { nomo_http_curl.slist_free_all(request_headers); }
    if (body->data != NULL) { free(body->data); body->data = NULL; }
    if (release_response) { @HTTP_HEADER_ARRAY_RELEASE@(headers->values); }
}

static @RESULT@ @SEND_NAME@(@HTTP_REQUEST@ request) {
    const char *method = request.@METHOD_MEMBER@.data;
    const char *url = request.@URL_MEMBER@.data;
    if ((strcmp(method, "GET") != 0 && strcmp(method, "POST") != 0)
        || !nomo_http_validate_url(url)
        || request.@TIMEOUT_MEMBER@ == 0
        || request.@TIMEOUT_MEMBER@ > NOMO_HTTP_MAX_TIMEOUT_MS
        || request.@MAX_RESPONSE_MEMBER@ == 0
        || request.@MAX_RESPONSE_MEMBER@ > NOMO_HTTP_HARD_MAX_RESPONSE_BYTES
        || (strcmp(method, "GET") == 0 && request.@BODY_MEMBER@.data[0] != '\0')) {
        return nomo_http_error("invalid_request", "invalid bounded HTTP request");
    }
    for (size_t index = 0; index < request.@HEADERS_MEMBER@.len; index += 1) {
        if (!nomo_http_validate_header(request.@HEADERS_MEMBER@.data[index])) {
            return nomo_http_error("invalid_request", "invalid or reserved HTTP header");
        }
    }
#ifdef _WIN32
    return nomo_http_send_winhttp(request);
#else
    if (!nomo_http_load_curl()) {
        return nomo_http_error(
            "runtime_unavailable",
            "the native HTTP runtime is unavailable on this host"
        );
    }
    nomo_curl_easy *easy = nomo_http_curl.easy_init();
    if (easy == NULL) {
        return nomo_http_error("runtime_unavailable", "failed to initialize the HTTP runtime");
    }
    nomo_http_body_buffer body = {
        .data = NULL,
        .len = 0,
        .cap = 0,
        .limit = (size_t)request.@MAX_RESPONSE_MEMBER@,
        .too_large = 0
    };
    nomo_http_header_buffer response_headers = {
        .values = @HTTP_HEADER_ARRAY_NEW@(),
        .bytes = 0,
        .too_large = 0
    };
    nomo_curl_slist *request_headers = NULL;
    request_headers = nomo_http_curl.slist_append(request_headers, "Expect:");
    if (request_headers == NULL) {
        nomo_http_cleanup(easy, NULL, &body, &response_headers, 1);
        return nomo_http_error("transport", "failed to allocate HTTP headers");
    }
    for (size_t index = 0; index < request.@HEADERS_MEMBER@.len; index += 1) {
        @HTTP_HEADER@ header = request.@HEADERS_MEMBER@.data[index];
        nomo_curl_slist *next = nomo_http_append_header(
            request_headers,
            header.@NAME_MEMBER@.data,
            header.@VALUE_MEMBER@.data
        );
        if (next == NULL) {
            nomo_http_cleanup(easy, request_headers, &body, &response_headers, 1);
            return nomo_http_error("transport", "failed to allocate HTTP headers");
        }
        request_headers = next;
    }
#define NOMO_HTTP_SETOPT(option, value) \
    do { \
        if (nomo_http_curl.easy_setopt(easy, (option), (value)) != 0) { \
            nomo_http_cleanup(easy, request_headers, &body, &response_headers, 1); \
            return nomo_http_error("runtime_unavailable", "HTTP runtime option is unavailable"); \
        } \
    } while (0)
    NOMO_HTTP_SETOPT(NOMO_CURLOPT_URL, url);
    NOMO_HTTP_SETOPT(NOMO_CURLOPT_CUSTOMREQUEST, method);
    NOMO_HTTP_SETOPT(NOMO_CURLOPT_USERAGENT, "nomo/0.1");
    NOMO_HTTP_SETOPT(NOMO_CURLOPT_HTTPHEADER, request_headers);
    NOMO_HTTP_SETOPT(NOMO_CURLOPT_TIMEOUT_MS, (long)request.@TIMEOUT_MEMBER@);
    NOMO_HTTP_SETOPT(NOMO_CURLOPT_NOSIGNAL, 1L);
    NOMO_HTTP_SETOPT(NOMO_CURLOPT_FOLLOWLOCATION, 0L);
    NOMO_HTTP_SETOPT(NOMO_CURLOPT_SSL_VERIFYPEER, 1L);
    NOMO_HTTP_SETOPT(NOMO_CURLOPT_SSL_VERIFYHOST, 2L);
    NOMO_HTTP_SETOPT(NOMO_CURLOPT_WRITEFUNCTION, nomo_http_write_body);
    NOMO_HTTP_SETOPT(NOMO_CURLOPT_WRITEDATA, &body);
    NOMO_HTTP_SETOPT(NOMO_CURLOPT_HEADERFUNCTION, nomo_http_write_header);
    NOMO_HTTP_SETOPT(NOMO_CURLOPT_HEADERDATA, &response_headers);
    if (strcmp(method, "POST") == 0) {
        NOMO_HTTP_SETOPT(NOMO_CURLOPT_POSTFIELDS, request.@BODY_MEMBER@.data);
        NOMO_HTTP_SETOPT(
            NOMO_CURLOPT_POSTFIELDSIZE_LARGE,
            (long long)strlen(request.@BODY_MEMBER@.data)
        );
    }
    const char *ca_bundle = getenv("NOMO_HTTP_CA_BUNDLE");
    if (ca_bundle != NULL && ca_bundle[0] != '\0') {
        NOMO_HTTP_SETOPT(NOMO_CURLOPT_CAINFO, ca_bundle);
    }
#undef NOMO_HTTP_SETOPT
    int perform_code = nomo_http_curl.easy_perform(easy);
    if (perform_code != 0) {
        int too_large = body.too_large || response_headers.too_large;
        nomo_http_cleanup(easy, request_headers, &body, &response_headers, 1);
        if (too_large) {
            return nomo_http_error("response_too_large", "HTTP response exceeded its configured limit");
        }
        const char *code = nomo_http_error_code_for_curl(perform_code);
        return nomo_http_error(code, nomo_http_error_message_for_code(code));
    }
    long status = 0;
    if (nomo_http_curl.easy_getinfo(easy, NOMO_CURLINFO_RESPONSE_CODE, &status) != 0) {
        nomo_http_cleanup(easy, request_headers, &body, &response_headers, 1);
        return nomo_http_error("protocol", "HTTP response did not include a valid status");
    }
    nomo_http_curl.easy_cleanup(easy);
    nomo_http_curl.slist_free_all(request_headers);
    nomo_string response_body = body.data == NULL
        ? nomo_string_literal("")
        : nomo_string_owned(body.data);
    return (@RESULT@){
        .tag = @OK@,
        .payload.@OK_PAYLOAD@ = (@HTTP_RESPONSE@){
            .@STATUS_MEMBER@ = (int64_t)status,
            .@HEADERS_MEMBER@ = response_headers.values,
            .@BODY_MEMBER@ = response_body
        }
    };
#endif
}

static @RESULT@ @GET_NAME@(nomo_string url) {
    return @SEND_NAME@((@HTTP_REQUEST@){
        .@METHOD_MEMBER@ = nomo_string_literal("GET"),
        .@URL_MEMBER@ = url,
        .@HEADERS_MEMBER@ = @HTTP_HEADER_ARRAY_NEW@(),
        .@BODY_MEMBER@ = nomo_string_literal(""),
        .@TIMEOUT_MEMBER@ = NOMO_HTTP_DEFAULT_TIMEOUT_MS,
        .@MAX_RESPONSE_MEMBER@ = NOMO_HTTP_DEFAULT_MAX_RESPONSE_BYTES
    });
}

static @RESULT@ @POST_NAME@(nomo_string url, nomo_string body) {
    return @SEND_NAME@((@HTTP_REQUEST@){
        .@METHOD_MEMBER@ = nomo_string_literal("POST"),
        .@URL_MEMBER@ = url,
        .@HEADERS_MEMBER@ = @HTTP_HEADER_ARRAY_NEW@(),
        .@BODY_MEMBER@ = body,
        .@TIMEOUT_MEMBER@ = NOMO_HTTP_DEFAULT_TIMEOUT_MS,
        .@MAX_RESPONSE_MEMBER@ = NOMO_HTTP_DEFAULT_MAX_RESPONSE_BYTES
    });
}
