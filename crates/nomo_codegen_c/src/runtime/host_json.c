#define NOMO_JSON_MAX_BYTES (8U * 1024U * 1024U)
#define NOMO_JSON_MAX_DEPTH 128U
#define NOMO_JSON_MAX_VALUES 262144U

typedef struct {
    const char *text;
    size_t len;
    size_t index;
    uint32_t depth;
    uint32_t max_depth;
    uint64_t values;
    const char *error_code;
    size_t error_offset;
} nomo_json_cursor;

typedef struct {
    char *data;
    size_t len;
    size_t cap;
} nomo_json_buffer;

static int nomo_json_fail(
    nomo_json_cursor *cursor,
    const char *code,
    size_t offset
) {
    if (cursor->error_code == NULL) {
        cursor->error_code = code;
        cursor->error_offset = offset;
    }
    return 0;
}

static void nomo_json_skip_ws(nomo_json_cursor *cursor) {
    while (cursor->index < cursor->len) {
        char ch = cursor->text[cursor->index];
        if (ch != ' ' && ch != '\n' && ch != '\r' && ch != '\t') {
            break;
        }
        cursor->index += 1U;
    }
}

static int nomo_json_hex_value(unsigned char ch) {
    if (ch >= '0' && ch <= '9') {
        return (int)(ch - '0');
    }
    if (ch >= 'a' && ch <= 'f') {
        return (int)(ch - 'a') + 10;
    }
    if (ch >= 'A' && ch <= 'F') {
        return (int)(ch - 'A') + 10;
    }
    return -1;
}

static int nomo_json_parse_hex4(
    nomo_json_cursor *cursor,
    uint32_t *value
) {
    if (cursor->len - cursor->index < 4U) {
        return nomo_json_fail(cursor, "syntax", cursor->len);
    }
    uint32_t out = 0U;
    for (uint32_t i = 0U; i < 4U; i += 1U) {
        int digit = nomo_json_hex_value(
            (unsigned char)cursor->text[cursor->index]
        );
        if (digit < 0) {
            return nomo_json_fail(cursor, "syntax", cursor->index);
        }
        out = (out << 4U) | (uint32_t)digit;
        cursor->index += 1U;
    }
    *value = out;
    return 1;
}

static int nomo_json_utf8_width(
    nomo_json_cursor *cursor,
    size_t index,
    size_t *width
) {
    unsigned char first = (unsigned char)cursor->text[index];
    if (first < 0x80U) {
        *width = 1U;
        return 1;
    }

    size_t needed;
    uint32_t scalar;
    uint32_t minimum;
    if (first >= 0xc2U && first <= 0xdfU) {
        needed = 2U;
        scalar = (uint32_t)(first & 0x1fU);
        minimum = 0x80U;
    } else if (first >= 0xe0U && first <= 0xefU) {
        needed = 3U;
        scalar = (uint32_t)(first & 0x0fU);
        minimum = 0x800U;
    } else if (first >= 0xf0U && first <= 0xf4U) {
        needed = 4U;
        scalar = (uint32_t)(first & 0x07U);
        minimum = 0x10000U;
    } else {
        return nomo_json_fail(cursor, "unsupported_string", index);
    }

    if (needed > cursor->len - index) {
        return nomo_json_fail(cursor, "unsupported_string", index);
    }
    for (size_t i = 1U; i < needed; i += 1U) {
        unsigned char next = (unsigned char)cursor->text[index + i];
        if ((next & 0xc0U) != 0x80U) {
            return nomo_json_fail(
                cursor,
                "unsupported_string",
                index + i
            );
        }
        scalar = (scalar << 6U) | (uint32_t)(next & 0x3fU);
    }
    if (
        scalar < minimum
        || scalar > 0x10ffffU
        || (scalar >= 0xd800U && scalar <= 0xdfffU)
    ) {
        return nomo_json_fail(cursor, "unsupported_string", index);
    }
    *width = needed;
    return 1;
}

static int nomo_json_scan_string(nomo_json_cursor *cursor) {
    if (
        cursor->index >= cursor->len
        || cursor->text[cursor->index] != '"'
    ) {
        return nomo_json_fail(cursor, "syntax", cursor->index);
    }
    cursor->index += 1U;
    while (cursor->index < cursor->len) {
        unsigned char ch = (unsigned char)cursor->text[cursor->index];
        if (ch == '"') {
            cursor->index += 1U;
            return 1;
        }
        if (ch < 0x20U) {
            return nomo_json_fail(cursor, "syntax", cursor->index);
        }
        if (ch == '\\') {
            size_t escape_offset = cursor->index;
            cursor->index += 1U;
            if (cursor->index >= cursor->len) {
                return nomo_json_fail(cursor, "syntax", cursor->len);
            }
            char escaped = cursor->text[cursor->index];
            if (
                escaped == '"'
                || escaped == '\\'
                || escaped == '/'
                || escaped == 'b'
                || escaped == 'f'
                || escaped == 'n'
                || escaped == 'r'
                || escaped == 't'
            ) {
                cursor->index += 1U;
                continue;
            }
            if (escaped != 'u') {
                return nomo_json_fail(
                    cursor,
                    "syntax",
                    cursor->index
                );
            }
            cursor->index += 1U;
            uint32_t first = 0U;
            if (!nomo_json_parse_hex4(cursor, &first)) {
                return 0;
            }
            if (first == 0U) {
                return nomo_json_fail(
                    cursor,
                    "unsupported_string",
                    escape_offset
                );
            }
            if (first >= 0xd800U && first <= 0xdbffU) {
                if (
                    cursor->len - cursor->index < 6U
                    || cursor->text[cursor->index] != '\\'
                    || cursor->text[cursor->index + 1U] != 'u'
                ) {
                    return nomo_json_fail(
                        cursor,
                        "unsupported_string",
                        escape_offset
                    );
                }
                cursor->index += 2U;
                uint32_t second = 0U;
                if (!nomo_json_parse_hex4(cursor, &second)) {
                    return 0;
                }
                if (second < 0xdc00U || second > 0xdfffU) {
                    return nomo_json_fail(
                        cursor,
                        "unsupported_string",
                        escape_offset
                    );
                }
            } else if (first >= 0xdc00U && first <= 0xdfffU) {
                return nomo_json_fail(
                    cursor,
                    "unsupported_string",
                    escape_offset
                );
            }
            continue;
        }
        if (ch >= 0x80U) {
            size_t width = 0U;
            if (!nomo_json_utf8_width(cursor, cursor->index, &width)) {
                return 0;
            }
            cursor->index += width;
        } else {
            cursor->index += 1U;
        }
    }
    return nomo_json_fail(cursor, "syntax", cursor->len);
}

static int nomo_json_scan_number(nomo_json_cursor *cursor) {
    if (
        cursor->index < cursor->len
        && cursor->text[cursor->index] == '-'
    ) {
        cursor->index += 1U;
    }
    if (cursor->index >= cursor->len) {
        return nomo_json_fail(cursor, "syntax", cursor->len);
    }
    if (cursor->text[cursor->index] == '0') {
        cursor->index += 1U;
    } else if (
        cursor->text[cursor->index] >= '1'
        && cursor->text[cursor->index] <= '9'
    ) {
        do {
            cursor->index += 1U;
        } while (
            cursor->index < cursor->len
            && cursor->text[cursor->index] >= '0'
            && cursor->text[cursor->index] <= '9'
        );
    } else {
        return nomo_json_fail(cursor, "syntax", cursor->index);
    }

    if (
        cursor->index < cursor->len
        && cursor->text[cursor->index] == '.'
    ) {
        cursor->index += 1U;
        if (
            cursor->index >= cursor->len
            || cursor->text[cursor->index] < '0'
            || cursor->text[cursor->index] > '9'
        ) {
            return nomo_json_fail(cursor, "syntax", cursor->index);
        }
        do {
            cursor->index += 1U;
        } while (
            cursor->index < cursor->len
            && cursor->text[cursor->index] >= '0'
            && cursor->text[cursor->index] <= '9'
        );
    }

    if (
        cursor->index < cursor->len
        && (
            cursor->text[cursor->index] == 'e'
            || cursor->text[cursor->index] == 'E'
        )
    ) {
        cursor->index += 1U;
        if (
            cursor->index < cursor->len
            && (
                cursor->text[cursor->index] == '+'
                || cursor->text[cursor->index] == '-'
            )
        ) {
            cursor->index += 1U;
        }
        if (
            cursor->index >= cursor->len
            || cursor->text[cursor->index] < '0'
            || cursor->text[cursor->index] > '9'
        ) {
            return nomo_json_fail(cursor, "syntax", cursor->index);
        }
        do {
            cursor->index += 1U;
        } while (
            cursor->index < cursor->len
            && cursor->text[cursor->index] >= '0'
            && cursor->text[cursor->index] <= '9'
        );
    }
    return 1;
}

static int nomo_json_scan_literal(
    nomo_json_cursor *cursor,
    const char *literal
) {
    size_t len = strlen(literal);
    if (
        len > cursor->len - cursor->index
        || memcmp(cursor->text + cursor->index, literal, len) != 0
    ) {
        return nomo_json_fail(cursor, "syntax", cursor->index);
    }
    cursor->index += len;
    return 1;
}

static int nomo_json_scan_value(
    nomo_json_cursor *cursor,
    uint32_t parent_depth,
    size_t *start,
    size_t *end
);

static int nomo_json_scan_array(
    nomo_json_cursor *cursor,
    uint32_t depth
) {
    cursor->index += 1U;
    nomo_json_skip_ws(cursor);
    if (
        cursor->index < cursor->len
        && cursor->text[cursor->index] == ']'
    ) {
        cursor->index += 1U;
        return 1;
    }
    for (;;) {
        if (!nomo_json_scan_value(cursor, depth, NULL, NULL)) {
            return 0;
        }
        nomo_json_skip_ws(cursor);
        if (
            cursor->index < cursor->len
            && cursor->text[cursor->index] == ']'
        ) {
            cursor->index += 1U;
            return 1;
        }
        if (
            cursor->index >= cursor->len
            || cursor->text[cursor->index] != ','
        ) {
            return nomo_json_fail(cursor, "syntax", cursor->index);
        }
        cursor->index += 1U;
        nomo_json_skip_ws(cursor);
    }
}

static int nomo_json_scan_object(
    nomo_json_cursor *cursor,
    uint32_t depth
) {
    cursor->index += 1U;
    nomo_json_skip_ws(cursor);
    if (
        cursor->index < cursor->len
        && cursor->text[cursor->index] == '}'
    ) {
        cursor->index += 1U;
        return 1;
    }
    for (;;) {
        if (!nomo_json_scan_string(cursor)) {
            return 0;
        }
        nomo_json_skip_ws(cursor);
        if (
            cursor->index >= cursor->len
            || cursor->text[cursor->index] != ':'
        ) {
            return nomo_json_fail(cursor, "syntax", cursor->index);
        }
        cursor->index += 1U;
        if (!nomo_json_scan_value(cursor, depth, NULL, NULL)) {
            return 0;
        }
        nomo_json_skip_ws(cursor);
        if (
            cursor->index < cursor->len
            && cursor->text[cursor->index] == '}'
        ) {
            cursor->index += 1U;
            return 1;
        }
        if (
            cursor->index >= cursor->len
            || cursor->text[cursor->index] != ','
        ) {
            return nomo_json_fail(cursor, "syntax", cursor->index);
        }
        cursor->index += 1U;
        nomo_json_skip_ws(cursor);
    }
}

static int nomo_json_scan_value(
    nomo_json_cursor *cursor,
    uint32_t parent_depth,
    size_t *start,
    size_t *end
) {
    nomo_json_skip_ws(cursor);
    if (start != NULL) {
        *start = cursor->index;
    }
    if (cursor->index >= cursor->len) {
        return nomo_json_fail(cursor, "syntax", cursor->len);
    }
    if (cursor->values >= NOMO_JSON_MAX_VALUES) {
        return nomo_json_fail(cursor, "limit", cursor->index);
    }
    cursor->values += 1U;

    char ch = cursor->text[cursor->index];
    int ok;
    if (ch == '{' || ch == '[') {
        uint32_t depth = parent_depth + 1U;
        if (depth > NOMO_JSON_MAX_DEPTH) {
            return nomo_json_fail(cursor, "limit", cursor->index);
        }
        if (depth > cursor->max_depth) {
            cursor->max_depth = depth;
        }
        ok = ch == '{'
            ? nomo_json_scan_object(cursor, depth)
            : nomo_json_scan_array(cursor, depth);
    } else if (ch == '"') {
        ok = nomo_json_scan_string(cursor);
    } else if (ch == '-' || (ch >= '0' && ch <= '9')) {
        ok = nomo_json_scan_number(cursor);
    } else if (ch == 't') {
        ok = nomo_json_scan_literal(cursor, "true");
    } else if (ch == 'f') {
        ok = nomo_json_scan_literal(cursor, "false");
    } else if (ch == 'n') {
        ok = nomo_json_scan_literal(cursor, "null");
    } else {
        ok = nomo_json_fail(cursor, "syntax", cursor->index);
    }
    if (ok && end != NULL) {
        *end = cursor->index;
    }
    return ok;
}

static int nomo_json_validate(
    const char *text,
    size_t len,
    nomo_json_cursor *result
) {
    nomo_json_cursor cursor = {
        .text = text,
        .len = len,
        .index = 0U,
        .depth = 0U,
        .max_depth = 0U,
        .values = 0U,
        .error_code = NULL,
        .error_offset = 0U
    };
    if (len > NOMO_JSON_MAX_BYTES) {
        nomo_json_fail(&cursor, "limit", NOMO_JSON_MAX_BYTES);
        *result = cursor;
        return 0;
    }
    if (!nomo_json_scan_value(&cursor, 0U, NULL, NULL)) {
        *result = cursor;
        return 0;
    }
    nomo_json_skip_ws(&cursor);
    if (cursor.index != cursor.len) {
        nomo_json_fail(&cursor, "syntax", cursor.index);
        *result = cursor;
        return 0;
    }
    *result = cursor;
    return 1;
}

static const char *nomo_json_error_message(const char *code) {
    if (strcmp(code, "limit") == 0) {
        return "json limit exceeded";
    }
    if (strcmp(code, "unsupported_string") == 0) {
        return "json string is not representable";
    }
    if (strcmp(code, "invalid_number") == 0) {
        return "invalid json number";
    }
    return "invalid json syntax";
}

static @JSON_ERROR@ nomo_json_error(
    const char *code,
    size_t offset
) {
    return (@JSON_ERROR@){
        .@CODE_MEMBER@ = nomo_string_from_cstr(code),
        .@MESSAGE_MEMBER@ = nomo_string_from_cstr(
            nomo_json_error_message(code)
        ),
        .@OFFSET_MEMBER@ = (uint64_t)offset
    };
}

static @RESULT@ nomo_json_err(const char *code, size_t offset) {
    return (@RESULT@){
        .tag = @ERR_TAG@,
        .payload.@ERR_PAYLOAD@ = nomo_json_error(code, offset)
    };
}

static @RESULT@ nomo_json_ok(nomo_string raw) {
    return (@RESULT@){
        .tag = @OK_TAG@,
        .payload.@OK_PAYLOAD@ = (@JSON_VALUE@){
            .@RAW_MEMBER@ = raw
        }
    };
}

static void nomo_json_trim(
    const char *text,
    size_t len,
    size_t *start,
    size_t *end
) {
    size_t left = 0U;
    size_t right = len;
    while (
        left < right
        && (
            text[left] == ' '
            || text[left] == '\n'
            || text[left] == '\r'
            || text[left] == '\t'
        )
    ) {
        left += 1U;
    }
    while (
        right > left
        && (
            text[right - 1U] == ' '
            || text[right - 1U] == '\n'
            || text[right - 1U] == '\r'
            || text[right - 1U] == '\t'
        )
    ) {
        right -= 1U;
    }
    *start = left;
    *end = right;
}

static @RESULT@ nomo_json_parse(nomo_string text) {
    size_t len = strlen(text.data);
    nomo_json_cursor cursor;
    if (!nomo_json_validate(text.data, len, &cursor)) {
        return nomo_json_err(
            cursor.error_code == NULL ? "syntax" : cursor.error_code,
            cursor.error_offset
        );
    }
    return nomo_json_ok(nomo_string_retain(text));
}

static nomo_string nomo_json_stringify(@JSON_VALUE@ value) {
    return nomo_string_retain(value.@RAW_MEMBER@);
}

/* NOMO_STRUCTURED_JSON_BEGIN */

static @JSON_KIND@ nomo_json_kind(@JSON_VALUE@ value) {
    size_t start = 0U;
    size_t end = 0U;
    nomo_json_trim(
        value.@RAW_MEMBER@.data,
        strlen(value.@RAW_MEMBER@.data),
        &start,
        &end
    );
    char ch = value.@RAW_MEMBER@.data[start];
    int tag;
    if (ch == 'n') {
        tag = @KIND_NULL@;
    } else if (ch == 't' || ch == 'f') {
        tag = @KIND_BOOLEAN@;
    } else if (ch == '"') {
        tag = @KIND_STRING@;
    } else if (ch == '[') {
        tag = @KIND_ARRAY@;
    } else if (ch == '{') {
        tag = @KIND_OBJECT@;
    } else {
        tag = @KIND_NUMBER@;
    }
    return (@JSON_KIND@){.tag = tag};
}

static int nomo_json_is_null(@JSON_VALUE@ value) {
    size_t start = 0U;
    size_t end = 0U;
    nomo_json_trim(
        value.@RAW_MEMBER@.data,
        strlen(value.@RAW_MEMBER@.data),
        &start,
        &end
    );
    return end - start == 4U
        && memcmp(value.@RAW_MEMBER@.data + start, "null", 4U) == 0;
}

static @OPTION_BOOL@ nomo_json_as_bool(@JSON_VALUE@ value) {
    size_t start = 0U;
    size_t end = 0U;
    nomo_json_trim(
        value.@RAW_MEMBER@.data,
        strlen(value.@RAW_MEMBER@.data),
        &start,
        &end
    );
    if (
        end - start == 4U
        && memcmp(value.@RAW_MEMBER@.data + start, "true", 4U) == 0
    ) {
        return (@OPTION_BOOL@){
            .tag = @SOME_BOOL_TAG@,
            .payload.@SOME_PAYLOAD@ = 1
        };
    }
    if (
        end - start == 5U
        && memcmp(value.@RAW_MEMBER@.data + start, "false", 5U) == 0
    ) {
        return (@OPTION_BOOL@){
            .tag = @SOME_BOOL_TAG@,
            .payload.@SOME_PAYLOAD@ = 0
        };
    }
    return (@OPTION_BOOL@){.tag = @NONE_BOOL_TAG@};
}

static @OPTION_STRING@ nomo_json_number_text(@JSON_VALUE@ value) {
    size_t start = 0U;
    size_t end = 0U;
    nomo_json_trim(
        value.@RAW_MEMBER@.data,
        strlen(value.@RAW_MEMBER@.data),
        &start,
        &end
    );
    char ch = value.@RAW_MEMBER@.data[start];
    if (!(ch == '-' || (ch >= '0' && ch <= '9'))) {
        return (@OPTION_STRING@){.tag = @NONE_STRING_TAG@};
    }
    return (@OPTION_STRING@){
        .tag = @SOME_STRING_TAG@,
        .payload.@SOME_PAYLOAD@ = nomo_string_from_slice(
            value.@RAW_MEMBER@.data,
            start,
            end - start
        )
    };
}

static size_t nomo_json_encode_utf8(uint32_t scalar, char *out) {
    if (scalar <= 0x7fU) {
        out[0] = (char)scalar;
        return 1U;
    }
    if (scalar <= 0x7ffU) {
        out[0] = (char)(0xc0U | (scalar >> 6U));
        out[1] = (char)(0x80U | (scalar & 0x3fU));
        return 2U;
    }
    if (scalar <= 0xffffU) {
        out[0] = (char)(0xe0U | (scalar >> 12U));
        out[1] = (char)(0x80U | ((scalar >> 6U) & 0x3fU));
        out[2] = (char)(0x80U | (scalar & 0x3fU));
        return 3U;
    }
    out[0] = (char)(0xf0U | (scalar >> 18U));
    out[1] = (char)(0x80U | ((scalar >> 12U) & 0x3fU));
    out[2] = (char)(0x80U | ((scalar >> 6U) & 0x3fU));
    out[3] = (char)(0x80U | (scalar & 0x3fU));
    return 4U;
}

static nomo_string nomo_json_decode_string_range(
    const char *text,
    size_t start,
    size_t end
) {
    size_t capacity = end - start;
    char *out = (char *)malloc(capacity + 1U);
    if (out == NULL) {
        nomo_panic("out of memory");
    }
    size_t input = start + 1U;
    size_t output = 0U;
    while (input + 1U < end) {
        unsigned char ch = (unsigned char)text[input];
        if (ch != '\\') {
            out[output++] = (char)ch;
            input += 1U;
            continue;
        }
        input += 1U;
        char escaped = text[input++];
        if (escaped == '"' || escaped == '\\' || escaped == '/') {
            out[output++] = escaped;
        } else if (escaped == 'b') {
            out[output++] = '\b';
        } else if (escaped == 'f') {
            out[output++] = '\f';
        } else if (escaped == 'n') {
            out[output++] = '\n';
        } else if (escaped == 'r') {
            out[output++] = '\r';
        } else if (escaped == 't') {
            out[output++] = '\t';
        } else {
            uint32_t first = 0U;
            for (uint32_t i = 0U; i < 4U; i += 1U) {
                first = (first << 4U)
                    | (uint32_t)nomo_json_hex_value(
                        (unsigned char)text[input++]
                    );
            }
            uint32_t scalar = first;
            if (first >= 0xd800U && first <= 0xdbffU) {
                input += 2U;
                uint32_t second = 0U;
                for (uint32_t i = 0U; i < 4U; i += 1U) {
                    second = (second << 4U)
                        | (uint32_t)nomo_json_hex_value(
                            (unsigned char)text[input++]
                        );
                }
                scalar = 0x10000U
                    + ((first - 0xd800U) << 10U)
                    + (second - 0xdc00U);
            }
            output += nomo_json_encode_utf8(scalar, out + output);
        }
    }
    out[output] = '\0';
    return nomo_string_owned(out);
}

static @OPTION_STRING@ nomo_json_as_string(@JSON_VALUE@ value) {
    size_t start = 0U;
    size_t end = 0U;
    nomo_json_trim(
        value.@RAW_MEMBER@.data,
        strlen(value.@RAW_MEMBER@.data),
        &start,
        &end
    );
    if (value.@RAW_MEMBER@.data[start] != '"') {
        return (@OPTION_STRING@){.tag = @NONE_STRING_TAG@};
    }
    return (@OPTION_STRING@){
        .tag = @SOME_STRING_TAG@,
        .payload.@SOME_PAYLOAD@ = nomo_json_decode_string_range(
            value.@RAW_MEMBER@.data,
            start,
            end
        )
    };
}

static nomo_json_cursor nomo_json_cursor_for_value(@JSON_VALUE@ value) {
    nomo_json_cursor cursor = {
        .text = value.@RAW_MEMBER@.data,
        .len = strlen(value.@RAW_MEMBER@.data),
        .index = 0U,
        .depth = 0U,
        .max_depth = 0U,
        .values = 0U,
        .error_code = NULL,
        .error_offset = 0U
    };
    nomo_json_skip_ws(&cursor);
    return cursor;
}

static @OPTION_VALUE_ARRAY@ nomo_json_array_items(@JSON_VALUE@ value) {
    nomo_json_cursor cursor = nomo_json_cursor_for_value(value);
    if (
        cursor.index >= cursor.len
        || cursor.text[cursor.index] != '['
    ) {
        return (@OPTION_VALUE_ARRAY@){
            .tag = @NONE_VALUE_ARRAY_TAG@
        };
    }
    cursor.index += 1U;
    nomo_json_skip_ws(&cursor);
    @VALUE_ARRAY@ items = @VALUE_ARRAY@_new();
    if (
        cursor.index < cursor.len
        && cursor.text[cursor.index] == ']'
    ) {
        return (@OPTION_VALUE_ARRAY@){
            .tag = @SOME_VALUE_ARRAY_TAG@,
            .payload.@SOME_PAYLOAD@ = items
        };
    }
    for (;;) {
        size_t start = 0U;
        size_t end = 0U;
        if (!nomo_json_scan_value(&cursor, 1U, &start, &end)) {
            nomo_panic("validated json array became invalid");
        }
        @JSON_VALUE@ item = {
            .@RAW_MEMBER@ = nomo_string_from_slice(
                cursor.text,
                start,
                end - start
            )
        };
        items = @VALUE_ARRAY@_push(items, item);
        @JSON_VALUE@_release(item);
        nomo_json_skip_ws(&cursor);
        if (cursor.text[cursor.index] == ']') {
            break;
        }
        cursor.index += 1U;
        nomo_json_skip_ws(&cursor);
    }
    return (@OPTION_VALUE_ARRAY@){
        .tag = @SOME_VALUE_ARRAY_TAG@,
        .payload.@SOME_PAYLOAD@ = items
    };
}

static @OPTION_MEMBER_ARRAY@ nomo_json_object_members(
    @JSON_VALUE@ value
) {
    nomo_json_cursor cursor = nomo_json_cursor_for_value(value);
    if (
        cursor.index >= cursor.len
        || cursor.text[cursor.index] != '{'
    ) {
        return (@OPTION_MEMBER_ARRAY@){
            .tag = @NONE_MEMBER_ARRAY_TAG@
        };
    }
    cursor.index += 1U;
    nomo_json_skip_ws(&cursor);
    @MEMBER_ARRAY@ members = @MEMBER_ARRAY@_new();
    if (
        cursor.index < cursor.len
        && cursor.text[cursor.index] == '}'
    ) {
        return (@OPTION_MEMBER_ARRAY@){
            .tag = @SOME_MEMBER_ARRAY_TAG@,
            .payload.@SOME_PAYLOAD@ = members
        };
    }
    for (;;) {
        size_t key_start = cursor.index;
        if (!nomo_json_scan_string(&cursor)) {
            nomo_panic("validated json object became invalid");
        }
        size_t key_end = cursor.index;
        nomo_string key = nomo_json_decode_string_range(
            cursor.text,
            key_start,
            key_end
        );
        nomo_json_skip_ws(&cursor);
        cursor.index += 1U;
        size_t value_start = 0U;
        size_t value_end = 0U;
        if (
            !nomo_json_scan_value(
                &cursor,
                1U,
                &value_start,
                &value_end
            )
        ) {
            nomo_panic("validated json object became invalid");
        }
        @JSON_MEMBER@ member = {
            .@KEY_MEMBER@ = key,
            .@VALUE_MEMBER@ = {
                .@RAW_MEMBER@ = nomo_string_from_slice(
                    cursor.text,
                    value_start,
                    value_end - value_start
                )
            }
        };
        members = @MEMBER_ARRAY@_push(members, member);
        @JSON_MEMBER@_release(member);
        nomo_json_skip_ws(&cursor);
        if (cursor.text[cursor.index] == '}') {
            break;
        }
        cursor.index += 1U;
        nomo_json_skip_ws(&cursor);
    }
    return (@OPTION_MEMBER_ARRAY@){
        .tag = @SOME_MEMBER_ARRAY_TAG@,
        .payload.@SOME_PAYLOAD@ = members
    };
}

static @OPTION_VALUE@ nomo_json_get(
    @JSON_VALUE@ value,
    nomo_string requested
) {
    nomo_json_cursor cursor = nomo_json_cursor_for_value(value);
    if (
        cursor.index >= cursor.len
        || cursor.text[cursor.index] != '{'
    ) {
        return (@OPTION_VALUE@){.tag = @NONE_VALUE_TAG@};
    }
    cursor.index += 1U;
    nomo_json_skip_ws(&cursor);
    if (
        cursor.index < cursor.len
        && cursor.text[cursor.index] == '}'
    ) {
        return (@OPTION_VALUE@){.tag = @NONE_VALUE_TAG@};
    }
    int found = 0;
    @JSON_VALUE@ selected;
    for (;;) {
        size_t key_start = cursor.index;
        if (!nomo_json_scan_string(&cursor)) {
            nomo_panic("validated json object became invalid");
        }
        size_t key_end = cursor.index;
        nomo_string key = nomo_json_decode_string_range(
            cursor.text,
            key_start,
            key_end
        );
        nomo_json_skip_ws(&cursor);
        cursor.index += 1U;
        size_t value_start = 0U;
        size_t value_end = 0U;
        if (
            !nomo_json_scan_value(
                &cursor,
                1U,
                &value_start,
                &value_end
            )
        ) {
            nomo_panic("validated json object became invalid");
        }
        if (strcmp(key.data, requested.data) == 0) {
            if (found) {
                @JSON_VALUE@_release(selected);
            }
            selected = (@JSON_VALUE@){
                .@RAW_MEMBER@ = nomo_string_from_slice(
                    cursor.text,
                    value_start,
                    value_end - value_start
                )
            };
            found = 1;
        }
        nomo_string_release(key);
        nomo_json_skip_ws(&cursor);
        if (cursor.text[cursor.index] == '}') {
            break;
        }
        cursor.index += 1U;
        nomo_json_skip_ws(&cursor);
    }
    if (!found) {
        return (@OPTION_VALUE@){.tag = @NONE_VALUE_TAG@};
    }
    return (@OPTION_VALUE@){
        .tag = @SOME_VALUE_TAG@,
        .payload.@SOME_PAYLOAD@ = selected
    };
}

static @JSON_VALUE@ nomo_json_from_null(void) {
    return (@JSON_VALUE@){
        .@RAW_MEMBER@ = nomo_string_literal("null")
    };
}

static @JSON_VALUE@ nomo_json_from_bool(int value) {
    return (@JSON_VALUE@){
        .@RAW_MEMBER@ = value
            ? nomo_string_literal("true")
            : nomo_string_literal("false")
    };
}

static @RESULT@ nomo_json_from_number_text(nomo_string value) {
    size_t len = strlen(value.data);
    if (len > NOMO_JSON_MAX_BYTES) {
        return nomo_json_err("limit", 0U);
    }
    nomo_json_cursor cursor = {
        .text = value.data,
        .len = len,
        .index = 0U,
        .depth = 0U,
        .max_depth = 0U,
        .values = 0U,
        .error_code = NULL,
        .error_offset = 0U
    };
    if (
        len == 0U
        || !nomo_json_scan_number(&cursor)
        || cursor.index != cursor.len
    ) {
        size_t offset = cursor.error_code == NULL
            ? cursor.index
            : cursor.error_offset;
        return nomo_json_err("invalid_number", offset);
    }
    return nomo_json_ok(nomo_string_retain(value));
}

static @JSON_VALUE@ nomo_json_from_i64(int64_t value) {
    char text[32];
    snprintf(text, sizeof(text), "%" PRId64, value);
    return (@JSON_VALUE@){
        .@RAW_MEMBER@ = nomo_string_from_cstr(text)
    };
}

static @JSON_VALUE@ nomo_json_from_u64(uint64_t value) {
    char text[32];
    snprintf(text, sizeof(text), "%" PRIu64, value);
    return (@JSON_VALUE@){
        .@RAW_MEMBER@ = nomo_string_from_cstr(text)
    };
}

static int nomo_json_validate_nomo_string(
    nomo_string value,
    size_t *bad_offset
) {
    nomo_json_cursor cursor = {
        .text = value.data,
        .len = strlen(value.data),
        .index = 0U,
        .depth = 0U,
        .max_depth = 0U,
        .values = 0U,
        .error_code = NULL,
        .error_offset = 0U
    };
    while (cursor.index < cursor.len) {
        unsigned char ch = (unsigned char)cursor.text[cursor.index];
        if (ch < 0x80U) {
            cursor.index += 1U;
            continue;
        }
        size_t width = 0U;
        if (!nomo_json_utf8_width(&cursor, cursor.index, &width)) {
            *bad_offset = cursor.error_offset;
            return 0;
        }
        cursor.index += width;
    }
    return 1;
}

static int nomo_json_escaped_size(
    nomo_string value,
    size_t *size
) {
    size_t total = 2U;
    const unsigned char *bytes = (const unsigned char *)value.data;
    size_t len = strlen(value.data);
    for (size_t i = 0U; i < len; i += 1U) {
        unsigned char ch = bytes[i];
        size_t add = 1U;
        if (ch == '"' || ch == '\\') {
            add = 2U;
        } else if (ch < 0x20U) {
            add = (
                ch == '\b'
                || ch == '\f'
                || ch == '\n'
                || ch == '\r'
                || ch == '\t'
            ) ? 2U : 6U;
        }
        if (total > NOMO_JSON_MAX_BYTES - add) {
            return 0;
        }
        total += add;
    }
    *size = total;
    return 1;
}

static void nomo_json_buffer_init(
    nomo_json_buffer *buffer,
    size_t capacity
) {
    buffer->data = (char *)malloc(capacity + 1U);
    if (buffer->data == NULL) {
        nomo_panic("out of memory");
    }
    buffer->len = 0U;
    buffer->cap = capacity;
}

static void nomo_json_buffer_append(
    nomo_json_buffer *buffer,
    const char *data,
    size_t len
) {
    if (len > buffer->cap - buffer->len) {
        nomo_panic("json buffer size mismatch");
    }
    memcpy(buffer->data + buffer->len, data, len);
    buffer->len += len;
}

static void nomo_json_buffer_char(
    nomo_json_buffer *buffer,
    char ch
) {
    nomo_json_buffer_append(buffer, &ch, 1U);
}

static void nomo_json_buffer_string(
    nomo_json_buffer *buffer,
    nomo_string value
) {
    static const char hex[] = "0123456789abcdef";
    const unsigned char *bytes = (const unsigned char *)value.data;
    size_t len = strlen(value.data);
    nomo_json_buffer_char(buffer, '"');
    for (size_t i = 0U; i < len; i += 1U) {
        unsigned char ch = bytes[i];
        if (ch == '"' || ch == '\\') {
            nomo_json_buffer_char(buffer, '\\');
            nomo_json_buffer_char(buffer, (char)ch);
        } else if (ch == '\b') {
            nomo_json_buffer_append(buffer, "\\b", 2U);
        } else if (ch == '\f') {
            nomo_json_buffer_append(buffer, "\\f", 2U);
        } else if (ch == '\n') {
            nomo_json_buffer_append(buffer, "\\n", 2U);
        } else if (ch == '\r') {
            nomo_json_buffer_append(buffer, "\\r", 2U);
        } else if (ch == '\t') {
            nomo_json_buffer_append(buffer, "\\t", 2U);
        } else if (ch < 0x20U) {
            char escaped[6] = {
                '\\',
                'u',
                '0',
                '0',
                hex[(ch >> 4U) & 0x0fU],
                hex[ch & 0x0fU]
            };
            nomo_json_buffer_append(buffer, escaped, sizeof(escaped));
        } else {
            nomo_json_buffer_char(buffer, (char)ch);
        }
    }
    nomo_json_buffer_char(buffer, '"');
}

static nomo_string nomo_json_buffer_finish(nomo_json_buffer *buffer) {
    if (buffer->len != buffer->cap) {
        nomo_panic("json buffer size mismatch");
    }
    buffer->data[buffer->len] = '\0';
    char *data = buffer->data;
    buffer->data = NULL;
    buffer->len = 0U;
    buffer->cap = 0U;
    return nomo_string_owned(data);
}

static @RESULT@ nomo_json_from_string(nomo_string value) {
    size_t bad_offset = 0U;
    if (!nomo_json_validate_nomo_string(value, &bad_offset)) {
        return nomo_json_err("unsupported_string", bad_offset);
    }
    size_t size = 0U;
    if (!nomo_json_escaped_size(value, &size)) {
        return nomo_json_err("limit", 0U);
    }
    nomo_json_buffer buffer;
    nomo_json_buffer_init(&buffer, size);
    nomo_json_buffer_string(&buffer, value);
    return nomo_json_ok(nomo_json_buffer_finish(&buffer));
}

static int nomo_json_add_size(size_t *total, size_t add) {
    if (add > NOMO_JSON_MAX_BYTES - *total) {
        return 0;
    }
    *total += add;
    return 1;
}

static int nomo_json_add_values(uint64_t *total, uint64_t add) {
    if (add > NOMO_JSON_MAX_VALUES - *total) {
        return 0;
    }
    *total += add;
    return 1;
}

static @RESULT@ nomo_json_from_array(@VALUE_ARRAY@ values) {
    size_t total_size = 2U;
    uint64_t total_values = 1U;
    uint32_t max_child_depth = 0U;
    for (size_t i = 0U; i < values.len; i += 1U) {
        nomo_string raw = values.data[i].@RAW_MEMBER@;
        size_t raw_len = strlen(raw.data);
        size_t start = 0U;
        size_t end = 0U;
        nomo_json_trim(raw.data, raw_len, &start, &end);
        nomo_json_cursor metadata;
        if (!nomo_json_validate(raw.data, raw_len, &metadata)) {
            nomo_panic("opaque json value became invalid");
        }
        if (
            !nomo_json_add_size(&total_size, end - start)
            || (i > 0U && !nomo_json_add_size(&total_size, 1U))
            || !nomo_json_add_values(&total_values, metadata.values)
        ) {
            return nomo_json_err("limit", 0U);
        }
        if (metadata.max_depth > max_child_depth) {
            max_child_depth = metadata.max_depth;
        }
    }
    if (max_child_depth + 1U > NOMO_JSON_MAX_DEPTH) {
        return nomo_json_err("limit", 0U);
    }
    nomo_json_buffer buffer;
    nomo_json_buffer_init(&buffer, total_size);
    nomo_json_buffer_char(&buffer, '[');
    for (size_t i = 0U; i < values.len; i += 1U) {
        if (i > 0U) {
            nomo_json_buffer_char(&buffer, ',');
        }
        nomo_string raw = values.data[i].@RAW_MEMBER@;
        size_t start = 0U;
        size_t end = 0U;
        nomo_json_trim(raw.data, strlen(raw.data), &start, &end);
        nomo_json_buffer_append(
            &buffer,
            raw.data + start,
            end - start
        );
    }
    nomo_json_buffer_char(&buffer, ']');
    return nomo_json_ok(nomo_json_buffer_finish(&buffer));
}

static @RESULT@ nomo_json_from_object(@MEMBER_ARRAY@ members) {
    size_t total_size = 2U;
    uint64_t total_values = 1U;
    uint32_t max_child_depth = 0U;
    for (size_t i = 0U; i < members.len; i += 1U) {
        @JSON_MEMBER@ member = members.data[i];
        size_t bad_offset = 0U;
        if (!nomo_json_validate_nomo_string(
            member.@KEY_MEMBER@,
            &bad_offset
        )) {
            return nomo_json_err("unsupported_string", bad_offset);
        }
        size_t key_size = 0U;
        if (!nomo_json_escaped_size(member.@KEY_MEMBER@, &key_size)) {
            return nomo_json_err("limit", 0U);
        }
        nomo_string raw = member.@VALUE_MEMBER@.@RAW_MEMBER@;
        size_t raw_len = strlen(raw.data);
        size_t start = 0U;
        size_t end = 0U;
        nomo_json_trim(raw.data, raw_len, &start, &end);
        nomo_json_cursor metadata;
        if (!nomo_json_validate(raw.data, raw_len, &metadata)) {
            nomo_panic("opaque json value became invalid");
        }
        if (
            !nomo_json_add_size(&total_size, key_size)
            || !nomo_json_add_size(&total_size, 1U)
            || !nomo_json_add_size(&total_size, end - start)
            || (i > 0U && !nomo_json_add_size(&total_size, 1U))
            || !nomo_json_add_values(&total_values, metadata.values)
        ) {
            return nomo_json_err("limit", 0U);
        }
        if (metadata.max_depth > max_child_depth) {
            max_child_depth = metadata.max_depth;
        }
    }
    if (max_child_depth + 1U > NOMO_JSON_MAX_DEPTH) {
        return nomo_json_err("limit", 0U);
    }
    nomo_json_buffer buffer;
    nomo_json_buffer_init(&buffer, total_size);
    nomo_json_buffer_char(&buffer, '{');
    for (size_t i = 0U; i < members.len; i += 1U) {
        if (i > 0U) {
            nomo_json_buffer_char(&buffer, ',');
        }
        @JSON_MEMBER@ member = members.data[i];
        nomo_json_buffer_string(&buffer, member.@KEY_MEMBER@);
        nomo_json_buffer_char(&buffer, ':');
        nomo_string raw = member.@VALUE_MEMBER@.@RAW_MEMBER@;
        size_t start = 0U;
        size_t end = 0U;
        nomo_json_trim(raw.data, strlen(raw.data), &start, &end);
        nomo_json_buffer_append(
            &buffer,
            raw.data + start,
            end - start
        );
    }
    nomo_json_buffer_char(&buffer, '}');
    return nomo_json_ok(nomo_json_buffer_finish(&buffer));
}
