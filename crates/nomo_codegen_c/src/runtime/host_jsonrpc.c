#define NOMO_JSONRPC_MAX_MESSAGE_BYTES 1048575U
#define NOMO_JSONRPC_MAX_CHUNK_BYTES 1048576U
#define NOMO_JSONRPC_MAX_COMBINED_BYTES 2097151U
#define NOMO_JSONRPC_MAX_BATCH_MESSAGES 4096U

static const char *nomo_jsonrpc_error_message(const char *code) {
    if (strcmp(code, "limit") == 0) {
        return "JSON-RPC limit exceeded";
    }
    if (strcmp(code, "framing") == 0) {
        return "invalid JSON-RPC newline framing";
    }
    if (strcmp(code, "json") == 0) {
        return "invalid bounded JSON input";
    }
    if (strcmp(code, "protocol") == 0) {
        return "invalid JSON-RPC 2.0 envelope";
    }
    return "invalid JSON-RPC argument";
}

static @PROTOCOL_ERROR@ nomo_jsonrpc_error(const char *code) {
    return (@PROTOCOL_ERROR@){
        .@CODE_MEMBER@ = nomo_string_from_cstr(code),
        .@MESSAGE_MEMBER@ = nomo_string_from_cstr(
            nomo_jsonrpc_error_message(code)
        )
    };
}

static int nomo_jsonrpc_limit_valid(uint64_t limit) {
    return limit >= 1U && limit <= NOMO_JSONRPC_MAX_MESSAGE_BYTES;
}

static int nomo_jsonrpc_contains_line_break(
    const char *text,
    size_t len
) {
    for (size_t i = 0U; i < len; i += 1U) {
        if (text[i] == '\n' || text[i] == '\r') {
            return 1;
        }
    }
    return 0;
}

static char nomo_jsonrpc_kind_char(@JSON_VALUE@ value) {
    size_t start = 0U;
    size_t end = 0U;
    nomo_json_trim(
        value.@RAW_MEMBER@.data,
        strlen(value.@RAW_MEMBER@.data),
        &start,
        &end
    );
    return start < end ? value.@RAW_MEMBER@.data[start] : '\0';
}

static int nomo_jsonrpc_string_equals(
    @JSON_VALUE@ value,
    const char *expected
) {
    @OPTION_STRING@ decoded = nomo_json_as_string(value);
    if (decoded.tag != @OPTION_STRING_SOME@) {
        return 0;
    }
    int equal = strcmp(
        decoded.payload.@SOME_PAYLOAD@.data,
        expected
    ) == 0;
    nomo_string_release(decoded.payload.@SOME_PAYLOAD@);
    return equal;
}

static int nomo_jsonrpc_integer_i64(@JSON_VALUE@ value) {
    size_t start = 0U;
    size_t end = 0U;
    nomo_string raw = value.@RAW_MEMBER@;
    nomo_json_trim(raw.data, strlen(raw.data), &start, &end);
    size_t len = end - start;
    if (len == 0U || len >= 32U) {
        return 0;
    }
    for (size_t i = start; i < end; i += 1U) {
        if (raw.data[i] == '.' || raw.data[i] == 'e' || raw.data[i] == 'E') {
            return 0;
        }
    }
    char text[32];
    memcpy(text, raw.data + start, len);
    text[len] = '\0';
    errno = 0;
    char *parsed_end = NULL;
    intmax_t parsed = strtoimax(text, &parsed_end, 10);
    return errno != ERANGE
        && parsed_end == text + len
        && parsed >= INT64_MIN
        && parsed <= INT64_MAX;
}

static int nomo_jsonrpc_validate_error_object(@JSON_VALUE@ value) {
    @OPTION_MEMBER_ARRAY@ selected = nomo_json_object_members(value);
    if (selected.tag != @OPTION_MEMBER_ARRAY_SOME@) {
        return 0;
    }
    @MEMBER_ARRAY@ members = selected.payload.@SOME_PAYLOAD@;
    int code_count = 0;
    int message_count = 0;
    int data_count = 0;
    int valid = 1;
    for (size_t i = 0U; i < members.len; i += 1U) {
        @JSON_MEMBER@ member = members.data[i];
        const char *key = member.@KEY_MEMBER@.data;
        if (strcmp(key, "code") == 0) {
            code_count += 1;
            if (
                code_count != 1
                || !nomo_jsonrpc_integer_i64(member.@VALUE_MEMBER@)
            ) {
                valid = 0;
                break;
            }
        } else if (strcmp(key, "message") == 0) {
            message_count += 1;
            if (
                message_count != 1
                || nomo_jsonrpc_kind_char(member.@VALUE_MEMBER@) != '"'
            ) {
                valid = 0;
                break;
            }
        } else if (strcmp(key, "data") == 0) {
            data_count += 1;
            if (data_count != 1) {
                valid = 0;
                break;
            }
        }
    }
    if (code_count != 1 || message_count != 1) {
        valid = 0;
    }
    @MEMBER_ARRAY_RELEASE@(members);
    return valid;
}

static int nomo_jsonrpc_validate_envelope(
    @JSON_VALUE@ value,
    int *kind
) {
    @OPTION_MEMBER_ARRAY@ selected = nomo_json_object_members(value);
    if (selected.tag != @OPTION_MEMBER_ARRAY_SOME@) {
        return 0;
    }
    @MEMBER_ARRAY@ members = selected.payload.@SOME_PAYLOAD@;
    int version_count = 0;
    int method_count = 0;
    int id_count = 0;
    int params_count = 0;
    int result_count = 0;
    int error_count = 0;
    char id_kind = '\0';
    int valid = 1;
    for (size_t i = 0U; i < members.len; i += 1U) {
        @JSON_MEMBER@ member = members.data[i];
        const char *key = member.@KEY_MEMBER@.data;
        if (strcmp(key, "jsonrpc") == 0) {
            version_count += 1;
            if (
                version_count != 1
                || !nomo_jsonrpc_string_equals(
                    member.@VALUE_MEMBER@,
                    "2.0"
                )
            ) {
                valid = 0;
                break;
            }
        } else if (strcmp(key, "method") == 0) {
            method_count += 1;
            if (
                method_count != 1
                || nomo_jsonrpc_kind_char(member.@VALUE_MEMBER@) != '"'
            ) {
                valid = 0;
                break;
            }
        } else if (strcmp(key, "id") == 0) {
            id_count += 1;
            if (id_count != 1) {
                valid = 0;
                break;
            }
            id_kind = nomo_jsonrpc_kind_char(member.@VALUE_MEMBER@);
        } else if (strcmp(key, "params") == 0) {
            params_count += 1;
            char params_kind = nomo_jsonrpc_kind_char(
                member.@VALUE_MEMBER@
            );
            if (
                params_count != 1
                || (params_kind != '{' && params_kind != '[')
            ) {
                valid = 0;
                break;
            }
        } else if (strcmp(key, "result") == 0) {
            result_count += 1;
            if (result_count != 1) {
                valid = 0;
                break;
            }
        } else if (strcmp(key, "error") == 0) {
            error_count += 1;
            if (
                error_count != 1
                || !nomo_jsonrpc_validate_error_object(
                    member.@VALUE_MEMBER@
                )
            ) {
                valid = 0;
                break;
            }
        }
    }

    if (version_count != 1) {
        valid = 0;
    } else if (method_count == 1) {
        if (
            result_count != 0
            || error_count != 0
            || (id_count == 1
                && id_kind != '"'
                && id_kind != '-'
                && (id_kind < '0' || id_kind > '9'))
        ) {
            valid = 0;
        } else {
            *kind = id_count == 0
                ? @KIND_NOTIFICATION@
                : @KIND_REQUEST@;
        }
    } else if (
        method_count != 0
        || params_count != 0
        || id_count != 1
        || (result_count + error_count) != 1
        || (
            id_kind != '"'
            && id_kind != '-'
            && id_kind != 'n'
            && (id_kind < '0' || id_kind > '9')
        )
    ) {
        valid = 0;
    } else {
        *kind = result_count == 1 ? @KIND_SUCCESS@ : @KIND_ERROR@;
    }
    @MEMBER_ARRAY_RELEASE@(members);
    return valid;
}

static int nomo_jsonrpc_validate_raw(
    const char *text,
    size_t len,
    uint64_t limit,
    int *kind,
    const char **error_code
) {
    if (!nomo_jsonrpc_limit_valid(limit)) {
        *error_code = "invalid_request";
        return 0;
    }
    if (len > (size_t)limit) {
        *error_code = "limit";
        return 0;
    }
    if (nomo_jsonrpc_contains_line_break(text, len)) {
        *error_code = "framing";
        return 0;
    }
    nomo_json_cursor cursor;
    if (!nomo_json_validate(text, len, &cursor)) {
        *error_code = cursor.error_code != NULL
            && strcmp(cursor.error_code, "limit") == 0
            ? "limit"
            : "json";
        return 0;
    }
    @JSON_VALUE@ value = {
        .@RAW_MEMBER@ = nomo_string_from_slice(text, 0U, len)
    };
    int valid = nomo_jsonrpc_validate_envelope(value, kind);
    nomo_string_release(value.@RAW_MEMBER@);
    if (!valid) {
        *error_code = "protocol";
        return 0;
    }
    return 1;
}

static @RESULT_DECODER@ nomo_jsonrpc_decoder(uint64_t limit) {
    if (!nomo_jsonrpc_limit_valid(limit)) {
        return (@RESULT_DECODER@){
            .tag = @RESULT_DECODER_ERR@,
            .payload.@ERR_PAYLOAD@ = nomo_jsonrpc_error(
                "invalid_request"
            )
        };
    }
    return (@RESULT_DECODER@){
        .tag = @RESULT_DECODER_OK@,
        .payload.@OK_PAYLOAD@ = (@DECODER@){
            .@PENDING_MEMBER@ = nomo_string_literal(""),
            .@MAX_MESSAGE_BYTES_MEMBER@ = limit
        }
    };
}

static @RESULT_BATCH@ nomo_jsonrpc_feed(
    @DECODER@ decoder,
    nomo_string chunk
) {
    uint64_t limit = decoder.@MAX_MESSAGE_BYTES_MEMBER@;
    size_t pending_len = strlen(decoder.@PENDING_MEMBER@.data);
    size_t chunk_len = strlen(chunk.data);
    if (
        !nomo_jsonrpc_limit_valid(limit)
        || pending_len > (size_t)limit
        || memchr(decoder.@PENDING_MEMBER@.data, '\n', pending_len) != NULL
    ) {
        return (@RESULT_BATCH@){
            .tag = @RESULT_BATCH_ERR@,
            .payload.@ERR_PAYLOAD@ = nomo_jsonrpc_error(
                "invalid_request"
            )
        };
    }
    if (
        chunk_len > NOMO_JSONRPC_MAX_CHUNK_BYTES
        || pending_len > NOMO_JSONRPC_MAX_COMBINED_BYTES - chunk_len
    ) {
        return (@RESULT_BATCH@){
            .tag = @RESULT_BATCH_ERR@,
            .payload.@ERR_PAYLOAD@ = nomo_jsonrpc_error("limit")
        };
    }
    size_t combined_len = pending_len + chunk_len;
    char *combined = (char *)malloc(combined_len + 1U);
    if (combined == NULL) {
        nomo_panic("out of memory");
    }
    memcpy(combined, decoder.@PENDING_MEMBER@.data, pending_len);
    memcpy(combined + pending_len, chunk.data, chunk_len);
    combined[combined_len] = '\0';

    @MESSAGE_ARRAY@ messages = @MESSAGE_ARRAY@_new();
    size_t line_start = 0U;
    size_t count = 0U;
    const char *failure = NULL;
    for (size_t i = 0U; i < combined_len; i += 1U) {
        if (combined[i] != '\n') {
            continue;
        }
        size_t line_end = i;
        if (line_end > line_start && combined[line_end - 1U] == '\r') {
            line_end -= 1U;
        }
        size_t line_len = line_end - line_start;
        if (line_len == 0U) {
            failure = "framing";
            break;
        }
        if (count >= NOMO_JSONRPC_MAX_BATCH_MESSAGES) {
            failure = "limit";
            break;
        }
        int kind = 0;
        if (
            !nomo_jsonrpc_validate_raw(
                combined + line_start,
                line_len,
                limit,
                &kind,
                &failure
            )
        ) {
            break;
        }
        @MESSAGE@ message = {
            .@RAW_MEMBER@ = nomo_string_from_slice(
                combined,
                line_start,
                line_len
            )
        };
        messages = @MESSAGE_ARRAY@_push(messages, message);
        @MESSAGE_RELEASE@(message);
        count += 1U;
        line_start = i + 1U;
    }
    size_t suffix_len = combined_len - line_start;
    if (failure == NULL && suffix_len > (size_t)limit) {
        failure = "limit";
    }
    if (failure != NULL) {
        @MESSAGE_ARRAY_RELEASE@(messages);
        free(combined);
        return (@RESULT_BATCH@){
            .tag = @RESULT_BATCH_ERR@,
            .payload.@ERR_PAYLOAD@ = nomo_jsonrpc_error(failure)
        };
    }

    nomo_string pending = nomo_string_from_slice(
        combined,
        line_start,
        suffix_len
    );
    free(combined);
    return (@RESULT_BATCH@){
        .tag = @RESULT_BATCH_OK@,
        .payload.@OK_PAYLOAD@ = (@BATCH@){
            .@DECODER_MEMBER@ = (@DECODER@){
                .@PENDING_MEMBER@ = pending,
                .@MAX_MESSAGE_BYTES_MEMBER@ = limit
            },
            .@MESSAGES_MEMBER@ = messages
        }
    };
}

static @RESULT_VOID@ nomo_jsonrpc_finish(@DECODER@ decoder) {
    uint64_t limit = decoder.@MAX_MESSAGE_BYTES_MEMBER@;
    size_t pending_len = strlen(decoder.@PENDING_MEMBER@.data);
    if (
        !nomo_jsonrpc_limit_valid(limit)
        || pending_len > (size_t)limit
        || memchr(decoder.@PENDING_MEMBER@.data, '\n', pending_len) != NULL
    ) {
        return (@RESULT_VOID@){
            .tag = @RESULT_VOID_ERR@,
            .payload.@ERR_PAYLOAD@ = nomo_jsonrpc_error(
                "invalid_request"
            )
        };
    }
    if (pending_len != 0U) {
        return (@RESULT_VOID@){
            .tag = @RESULT_VOID_ERR@,
            .payload.@ERR_PAYLOAD@ = nomo_jsonrpc_error("framing")
        };
    }
    return (@RESULT_VOID@){
        .tag = @RESULT_VOID_OK@,
        .payload.@OK_PAYLOAD@ = 0
    };
}

static @RESULT_MESSAGE@ nomo_jsonrpc_parse(
    @JSON_VALUE@ value,
    uint64_t limit
) {
    nomo_string raw = value.@RAW_MEMBER@;
    size_t len = strlen(raw.data);
    int kind = 0;
    const char *failure = NULL;
    if (
        !nomo_jsonrpc_validate_raw(
            raw.data,
            len,
            limit,
            &kind,
            &failure
        )
    ) {
        return (@RESULT_MESSAGE@){
            .tag = @RESULT_MESSAGE_ERR@,
            .payload.@ERR_PAYLOAD@ = nomo_jsonrpc_error(failure)
        };
    }
    return (@RESULT_MESSAGE@){
        .tag = @RESULT_MESSAGE_OK@,
        .payload.@OK_PAYLOAD@ = (@MESSAGE@){
            .@RAW_MEMBER@ = nomo_string_retain(raw)
        }
    };
}

static @RESULT_STRING@ nomo_jsonrpc_encode(
    @MESSAGE@ message,
    uint64_t limit
) {
    nomo_string raw = message.@RAW_MEMBER@;
    size_t len = strlen(raw.data);
    int kind = 0;
    const char *failure = NULL;
    if (
        !nomo_jsonrpc_validate_raw(
            raw.data,
            len,
            limit,
            &kind,
            &failure
        )
    ) {
        return (@RESULT_STRING@){
            .tag = @RESULT_STRING_ERR@,
            .payload.@ERR_PAYLOAD@ = nomo_jsonrpc_error(failure)
        };
    }
    char *encoded = (char *)malloc(len + 2U);
    if (encoded == NULL) {
        nomo_panic("out of memory");
    }
    memcpy(encoded, raw.data, len);
    encoded[len] = '\n';
    encoded[len + 1U] = '\0';
    return (@RESULT_STRING@){
        .tag = @RESULT_STRING_OK@,
        .payload.@OK_PAYLOAD@ = nomo_string_owned(encoded)
    };
}

static @JSON_VALUE@ nomo_jsonrpc_value(@MESSAGE@ message) {
    return (@JSON_VALUE@){
        .@RAW_MEMBER@ = nomo_string_retain(message.@RAW_MEMBER@)
    };
}

static @MESSAGE_KIND@ nomo_jsonrpc_kind(@MESSAGE@ message) {
    int kind = 0;
    const char *failure = NULL;
    size_t len = strlen(message.@RAW_MEMBER@.data);
    if (
        !nomo_jsonrpc_validate_raw(
            message.@RAW_MEMBER@.data,
            len,
            NOMO_JSONRPC_MAX_MESSAGE_BYTES,
            &kind,
            &failure
        )
    ) {
        nomo_panic("opaque JSON-RPC message became invalid");
    }
    return (@MESSAGE_KIND@){.tag = kind};
}

static int nomo_jsonrpc_add_size(size_t *total, size_t add) {
    if (add > NOMO_JSONRPC_MAX_MESSAGE_BYTES - *total) {
        return 0;
    }
    *total += add;
    return 1;
}

static void nomo_jsonrpc_trimmed_raw(
    @JSON_VALUE@ value,
    const char **text,
    size_t *len
) {
    size_t start = 0U;
    size_t end = 0U;
    nomo_string raw = value.@RAW_MEMBER@;
    nomo_json_trim(raw.data, strlen(raw.data), &start, &end);
    *text = raw.data + start;
    *len = end - start;
}

static int nomo_jsonrpc_id_valid(@JSON_VALUE@ id, int allow_null) {
    char kind = nomo_jsonrpc_kind_char(id);
    return kind == '"'
        || kind == '-'
        || (kind >= '0' && kind <= '9')
        || (allow_null && kind == 'n');
}

static int nomo_jsonrpc_params_valid(@OPTION_VALUE@ params) {
    if (params.tag != @OPTION_VALUE_SOME@) {
        return 1;
    }
    char kind = nomo_jsonrpc_kind_char(
        params.payload.@SOME_PAYLOAD@
    );
    return kind == '{' || kind == '[';
}

static @RESULT_MESSAGE@ nomo_jsonrpc_construct(
    const char *first,
    size_t first_len,
    const char *second,
    size_t second_len,
    const char *third,
    size_t third_len,
    const char *fourth,
    size_t fourth_len,
    const char *fifth,
    size_t fifth_len
) {
    size_t total = 0U;
    if (
        !nomo_jsonrpc_add_size(&total, first_len)
        || !nomo_jsonrpc_add_size(&total, second_len)
        || !nomo_jsonrpc_add_size(&total, third_len)
        || !nomo_jsonrpc_add_size(&total, fourth_len)
        || !nomo_jsonrpc_add_size(&total, fifth_len)
    ) {
        return (@RESULT_MESSAGE@){
            .tag = @RESULT_MESSAGE_ERR@,
            .payload.@ERR_PAYLOAD@ = nomo_jsonrpc_error("limit")
        };
    }
    nomo_json_buffer buffer;
    nomo_json_buffer_init(&buffer, total);
    nomo_json_buffer_append(&buffer, first, first_len);
    nomo_json_buffer_append(&buffer, second, second_len);
    nomo_json_buffer_append(&buffer, third, third_len);
    nomo_json_buffer_append(&buffer, fourth, fourth_len);
    nomo_json_buffer_append(&buffer, fifth, fifth_len);
    nomo_string raw = nomo_json_buffer_finish(&buffer);
    int kind = 0;
    const char *failure = NULL;
    if (
        !nomo_jsonrpc_validate_raw(
            raw.data,
            total,
            NOMO_JSONRPC_MAX_MESSAGE_BYTES,
            &kind,
            &failure
        )
    ) {
        nomo_string_release(raw);
        return (@RESULT_MESSAGE@){
            .tag = @RESULT_MESSAGE_ERR@,
            .payload.@ERR_PAYLOAD@ = nomo_jsonrpc_error(failure)
        };
    }
    return (@RESULT_MESSAGE@){
        .tag = @RESULT_MESSAGE_OK@,
        .payload.@OK_PAYLOAD@ = (@MESSAGE@){
            .@RAW_MEMBER@ = raw
        }
    };
}

static @RESULT_MESSAGE@ nomo_jsonrpc_request(
    @JSON_VALUE@ id,
    nomo_string method,
    @OPTION_VALUE@ params
) {
    if (
        !nomo_jsonrpc_id_valid(id, 0)
        || !nomo_jsonrpc_params_valid(params)
    ) {
        return (@RESULT_MESSAGE@){
            .tag = @RESULT_MESSAGE_ERR@,
            .payload.@ERR_PAYLOAD@ = nomo_jsonrpc_error("protocol")
        };
    }
    size_t method_size = 0U;
    if (!nomo_json_escaped_size(method, &method_size)) {
        return (@RESULT_MESSAGE@){
            .tag = @RESULT_MESSAGE_ERR@,
            .payload.@ERR_PAYLOAD@ = nomo_jsonrpc_error("limit")
        };
    }
    nomo_json_buffer method_buffer;
    nomo_json_buffer_init(&method_buffer, method_size);
    nomo_json_buffer_string(&method_buffer, method);
    nomo_string method_json = nomo_json_buffer_finish(&method_buffer);
    const char *id_text = NULL;
    size_t id_len = 0U;
    nomo_jsonrpc_trimmed_raw(id, &id_text, &id_len);
    const char *params_text = "";
    size_t params_len = 0U;
    const char *params_prefix = "";
    size_t params_prefix_len = 0U;
    if (params.tag == @OPTION_VALUE_SOME@) {
        params_prefix = ",\"params\":";
        params_prefix_len = sizeof(",\"params\":") - 1U;
        nomo_jsonrpc_trimmed_raw(
            params.payload.@SOME_PAYLOAD@,
            &params_text,
            &params_len
        );
    }
    static const char first[] = "{\"jsonrpc\":\"2.0\",\"id\":";
    static const char middle[] = ",\"method\":";
    size_t fixed = sizeof(first) - 1U
        + id_len
        + sizeof(middle) - 1U
        + method_size
        + params_prefix_len
        + params_len
        + 1U;
    if (fixed > NOMO_JSONRPC_MAX_MESSAGE_BYTES) {
        nomo_string_release(method_json);
        return (@RESULT_MESSAGE@){
            .tag = @RESULT_MESSAGE_ERR@,
            .payload.@ERR_PAYLOAD@ = nomo_jsonrpc_error("limit")
        };
    }
    nomo_json_buffer buffer;
    nomo_json_buffer_init(&buffer, fixed);
    nomo_json_buffer_append(&buffer, first, sizeof(first) - 1U);
    nomo_json_buffer_append(&buffer, id_text, id_len);
    nomo_json_buffer_append(&buffer, middle, sizeof(middle) - 1U);
    nomo_json_buffer_append(&buffer, method_json.data, method_size);
    nomo_json_buffer_append(&buffer, params_prefix, params_prefix_len);
    nomo_json_buffer_append(&buffer, params_text, params_len);
    nomo_json_buffer_char(&buffer, '}');
    nomo_string_release(method_json);
    nomo_string raw = nomo_json_buffer_finish(&buffer);
    @RESULT_MESSAGE@ parsed = nomo_jsonrpc_construct(
        raw.data,
        fixed,
        "",
        0U,
        "",
        0U,
        "",
        0U,
        "",
        0U
    );
    nomo_string_release(raw);
    return parsed;
}

static @RESULT_MESSAGE@ nomo_jsonrpc_notification(
    nomo_string method,
    @OPTION_VALUE@ params
) {
    if (!nomo_jsonrpc_params_valid(params)) {
        return (@RESULT_MESSAGE@){
            .tag = @RESULT_MESSAGE_ERR@,
            .payload.@ERR_PAYLOAD@ = nomo_jsonrpc_error("protocol")
        };
    }
    size_t method_size = 0U;
    if (!nomo_json_escaped_size(method, &method_size)) {
        return (@RESULT_MESSAGE@){
            .tag = @RESULT_MESSAGE_ERR@,
            .payload.@ERR_PAYLOAD@ = nomo_jsonrpc_error("limit")
        };
    }
    nomo_json_buffer method_buffer;
    nomo_json_buffer_init(&method_buffer, method_size);
    nomo_json_buffer_string(&method_buffer, method);
    nomo_string method_json = nomo_json_buffer_finish(&method_buffer);
    const char *params_text = "";
    size_t params_len = 0U;
    const char *params_prefix = "";
    size_t params_prefix_len = 0U;
    if (params.tag == @OPTION_VALUE_SOME@) {
        params_prefix = ",\"params\":";
        params_prefix_len = sizeof(",\"params\":") - 1U;
        nomo_jsonrpc_trimmed_raw(
            params.payload.@SOME_PAYLOAD@,
            &params_text,
            &params_len
        );
    }
    static const char first[] = "{\"jsonrpc\":\"2.0\",\"method\":";
    size_t total = sizeof(first) - 1U
        + method_size
        + params_prefix_len
        + params_len
        + 1U;
    if (total > NOMO_JSONRPC_MAX_MESSAGE_BYTES) {
        nomo_string_release(method_json);
        return (@RESULT_MESSAGE@){
            .tag = @RESULT_MESSAGE_ERR@,
            .payload.@ERR_PAYLOAD@ = nomo_jsonrpc_error("limit")
        };
    }
    nomo_json_buffer buffer;
    nomo_json_buffer_init(&buffer, total);
    nomo_json_buffer_append(&buffer, first, sizeof(first) - 1U);
    nomo_json_buffer_append(&buffer, method_json.data, method_size);
    nomo_json_buffer_append(&buffer, params_prefix, params_prefix_len);
    nomo_json_buffer_append(&buffer, params_text, params_len);
    nomo_json_buffer_char(&buffer, '}');
    nomo_string_release(method_json);
    nomo_string raw = nomo_json_buffer_finish(&buffer);
    @RESULT_MESSAGE@ parsed = nomo_jsonrpc_construct(
        raw.data, total, "", 0U, "", 0U, "", 0U, "", 0U
    );
    nomo_string_release(raw);
    return parsed;
}

static @RESULT_MESSAGE@ nomo_jsonrpc_success(
    @JSON_VALUE@ id,
    @JSON_VALUE@ result
) {
    if (!nomo_jsonrpc_id_valid(id, 1)) {
        return (@RESULT_MESSAGE@){
            .tag = @RESULT_MESSAGE_ERR@,
            .payload.@ERR_PAYLOAD@ = nomo_jsonrpc_error("protocol")
        };
    }
    const char *id_text = NULL;
    size_t id_len = 0U;
    const char *result_text = NULL;
    size_t result_len = 0U;
    nomo_jsonrpc_trimmed_raw(id, &id_text, &id_len);
    nomo_jsonrpc_trimmed_raw(result, &result_text, &result_len);
    static const char first[] = "{\"jsonrpc\":\"2.0\",\"id\":";
    static const char middle[] = ",\"result\":";
    static const char last[] = "}";
    return nomo_jsonrpc_construct(
        first,
        sizeof(first) - 1U,
        id_text,
        id_len,
        middle,
        sizeof(middle) - 1U,
        result_text,
        result_len,
        last,
        sizeof(last) - 1U
    );
}

static @RESULT_MESSAGE@ nomo_jsonrpc_failure(
    @JSON_VALUE@ id,
    int64_t code,
    nomo_string message,
    @OPTION_VALUE@ data
) {
    if (!nomo_jsonrpc_id_valid(id, 1)) {
        return (@RESULT_MESSAGE@){
            .tag = @RESULT_MESSAGE_ERR@,
            .payload.@ERR_PAYLOAD@ = nomo_jsonrpc_error("protocol")
        };
    }
    size_t message_size = 0U;
    if (!nomo_json_escaped_size(message, &message_size)) {
        return (@RESULT_MESSAGE@){
            .tag = @RESULT_MESSAGE_ERR@,
            .payload.@ERR_PAYLOAD@ = nomo_jsonrpc_error("limit")
        };
    }
    nomo_json_buffer message_buffer;
    nomo_json_buffer_init(&message_buffer, message_size);
    nomo_json_buffer_string(&message_buffer, message);
    nomo_string message_json = nomo_json_buffer_finish(&message_buffer);
    char code_text[32];
    int code_len = snprintf(
        code_text,
        sizeof(code_text),
        "%" PRId64,
        code
    );
    const char *id_text = NULL;
    size_t id_len = 0U;
    nomo_jsonrpc_trimmed_raw(id, &id_text, &id_len);
    const char *data_text = "";
    size_t data_len = 0U;
    const char *data_prefix = "";
    size_t data_prefix_len = 0U;
    if (data.tag == @OPTION_VALUE_SOME@) {
        data_prefix = ",\"data\":";
        data_prefix_len = sizeof(",\"data\":") - 1U;
        nomo_jsonrpc_trimmed_raw(
            data.payload.@SOME_PAYLOAD@,
            &data_text,
            &data_len
        );
    }
    static const char first[] = "{\"jsonrpc\":\"2.0\",\"id\":";
    static const char middle[] = ",\"error\":{\"code\":";
    static const char message_prefix[] = ",\"message\":";
    size_t total = sizeof(first) - 1U
        + id_len
        + sizeof(middle) - 1U
        + (size_t)code_len
        + sizeof(message_prefix) - 1U
        + message_size
        + data_prefix_len
        + data_len
        + 2U;
    if (total > NOMO_JSONRPC_MAX_MESSAGE_BYTES) {
        nomo_string_release(message_json);
        return (@RESULT_MESSAGE@){
            .tag = @RESULT_MESSAGE_ERR@,
            .payload.@ERR_PAYLOAD@ = nomo_jsonrpc_error("limit")
        };
    }
    nomo_json_buffer buffer;
    nomo_json_buffer_init(&buffer, total);
    nomo_json_buffer_append(&buffer, first, sizeof(first) - 1U);
    nomo_json_buffer_append(&buffer, id_text, id_len);
    nomo_json_buffer_append(&buffer, middle, sizeof(middle) - 1U);
    nomo_json_buffer_append(&buffer, code_text, (size_t)code_len);
    nomo_json_buffer_append(
        &buffer,
        message_prefix,
        sizeof(message_prefix) - 1U
    );
    nomo_json_buffer_append(&buffer, message_json.data, message_size);
    nomo_json_buffer_append(&buffer, data_prefix, data_prefix_len);
    nomo_json_buffer_append(&buffer, data_text, data_len);
    nomo_json_buffer_append(&buffer, "}}", 2U);
    nomo_string_release(message_json);
    nomo_string raw = nomo_json_buffer_finish(&buffer);
    @RESULT_MESSAGE@ parsed = nomo_jsonrpc_construct(
        raw.data, total, "", 0U, "", 0U, "", 0U, "", 0U
    );
    nomo_string_release(raw);
    return parsed;
}
