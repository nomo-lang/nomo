use super::*;

pub(super) fn emit_operator_runtime(out: &mut String) {
    out.push_str("static long long nomo_add_i64(long long left, long long right) {\n");
    out.push_str("    if ((right > 0 && left > LLONG_MAX - right) || (right < 0 && left < LLONG_MIN - right)) { nomo_panic(\"signed integer overflow\"); }\n");
    out.push_str("    return left + right;\n");
    out.push_str("}\n\n");
    out.push_str("static long long nomo_sub_i64(long long left, long long right) {\n");
    out.push_str("    if ((right < 0 && left > LLONG_MAX + right) || (right > 0 && left < LLONG_MIN + right)) { nomo_panic(\"signed integer overflow\"); }\n");
    out.push_str("    return left - right;\n");
    out.push_str("}\n\n");
    out.push_str("static long long nomo_mul_i64(long long left, long long right) {\n");
    out.push_str("    if (left == 0 || right == 0) { return 0; }\n");
    out.push_str("    if ((left == -1 && right == LLONG_MIN) || (right == -1 && left == LLONG_MIN)) { nomo_panic(\"signed integer overflow\"); }\n");
    out.push_str("    if (left > 0) {\n");
    out.push_str("        if (right > 0) { if (left > LLONG_MAX / right) { nomo_panic(\"signed integer overflow\"); } }\n");
    out.push_str("        else { if (right < LLONG_MIN / left) { nomo_panic(\"signed integer overflow\"); } }\n");
    out.push_str("    } else {\n");
    out.push_str("        if (right > 0) { if (left < LLONG_MIN / right) { nomo_panic(\"signed integer overflow\"); } }\n");
    out.push_str("        else { if (left < LLONG_MAX / right) { nomo_panic(\"signed integer overflow\"); } }\n");
    out.push_str("    }\n");
    out.push_str("    return left * right;\n");
    out.push_str("}\n\n");
    out.push_str("static long long nomo_div_i64(long long left, long long right) {\n");
    out.push_str("    if (right == 0) { nomo_panic(\"division by zero\"); }\n");
    out.push_str(
        "    if (left == LLONG_MIN && right == -1) { nomo_panic(\"signed integer overflow\"); }\n",
    );
    out.push_str("    return left / right;\n");
    out.push_str("}\n\n");
    out.push_str("static long long nomo_rem_i64(long long left, long long right) {\n");
    out.push_str("    if (right == 0) { nomo_panic(\"division by zero\"); }\n");
    out.push_str(
        "    if (left == LLONG_MIN && right == -1) { nomo_panic(\"signed integer overflow\"); }\n",
    );
    out.push_str("    return left % right;\n");
    out.push_str("}\n\n");
    out.push_str("static int32_t nomo_add_i32(int32_t left, int32_t right) {\n");
    out.push_str("    if ((right > 0 && left > INT32_MAX - right) || (right < 0 && left < INT32_MIN - right)) { nomo_panic(\"signed integer overflow\"); }\n");
    out.push_str("    return left + right;\n");
    out.push_str("}\n\n");
    out.push_str("static int32_t nomo_sub_i32(int32_t left, int32_t right) {\n");
    out.push_str("    if ((right < 0 && left > INT32_MAX + right) || (right > 0 && left < INT32_MIN + right)) { nomo_panic(\"signed integer overflow\"); }\n");
    out.push_str("    return left - right;\n");
    out.push_str("}\n\n");
    out.push_str("static int32_t nomo_mul_i32(int32_t left, int32_t right) {\n");
    out.push_str("    if (left == 0 || right == 0) { return 0; }\n");
    out.push_str("    if ((left == -1 && right == INT32_MIN) || (right == -1 && left == INT32_MIN)) { nomo_panic(\"signed integer overflow\"); }\n");
    out.push_str("    if (left > 0) {\n");
    out.push_str("        if (right > 0) { if (left > INT32_MAX / right) { nomo_panic(\"signed integer overflow\"); } }\n");
    out.push_str("        else { if (right < INT32_MIN / left) { nomo_panic(\"signed integer overflow\"); } }\n");
    out.push_str("    } else {\n");
    out.push_str("        if (right > 0) { if (left < INT32_MIN / right) { nomo_panic(\"signed integer overflow\"); } }\n");
    out.push_str("        else { if (left < INT32_MAX / right) { nomo_panic(\"signed integer overflow\"); } }\n");
    out.push_str("    }\n");
    out.push_str("    return left * right;\n");
    out.push_str("}\n\n");
    out.push_str("static int32_t nomo_div_i32(int32_t left, int32_t right) {\n");
    out.push_str("    if (right == 0) { nomo_panic(\"division by zero\"); }\n");
    out.push_str(
        "    if (left == INT32_MIN && right == -1) { nomo_panic(\"signed integer overflow\"); }\n",
    );
    out.push_str("    return left / right;\n");
    out.push_str("}\n\n");
    out.push_str("static int32_t nomo_rem_i32(int32_t left, int32_t right) {\n");
    out.push_str("    if (right == 0) { nomo_panic(\"division by zero\"); }\n");
    out.push_str(
        "    if (left == INT32_MIN && right == -1) { nomo_panic(\"signed integer overflow\"); }\n",
    );
    out.push_str("    return left % right;\n");
    out.push_str("}\n\n");
    out.push_str("static uint32_t nomo_div_u32(uint32_t left, uint32_t right) {\n");
    out.push_str("    if (right == 0) { nomo_panic(\"division by zero\"); }\n");
    out.push_str("    return left / right;\n");
    out.push_str("}\n\n");
    out.push_str("static uint32_t nomo_rem_u32(uint32_t left, uint32_t right) {\n");
    out.push_str("    if (right == 0) { nomo_panic(\"division by zero\"); }\n");
    out.push_str("    return left % right;\n");
    out.push_str("}\n\n");
    out.push_str("static uint64_t nomo_div_u64(uint64_t left, uint64_t right) {\n");
    out.push_str("    if (right == 0) { nomo_panic(\"division by zero\"); }\n");
    out.push_str("    return left / right;\n");
    out.push_str("}\n\n");
    out.push_str("static uint64_t nomo_rem_u64(uint64_t left, uint64_t right) {\n");
    out.push_str("    if (right == 0) { nomo_panic(\"division by zero\"); }\n");
    out.push_str("    return left % right;\n");
    out.push_str("}\n\n");
    out.push_str("static double nomo_div_f64(double left, double right) {\n");
    out.push_str("    if (right == 0.0) { nomo_panic(\"division by zero\"); }\n");
    out.push_str("    return left / right;\n");
    out.push_str("}\n\n");
    out.push_str("static long long nomo_shl_i64(long long left, uint64_t right) {\n");
    out.push_str(
        "    if (right >= sizeof(left) * CHAR_BIT) { nomo_panic(\"invalid shift amount\"); }\n",
    );
    out.push_str("    return left << right;\n");
    out.push_str("}\n\n");
    out.push_str("static long long nomo_shr_i64(long long left, uint64_t right) {\n");
    out.push_str(
        "    if (right >= sizeof(left) * CHAR_BIT) { nomo_panic(\"invalid shift amount\"); }\n",
    );
    out.push_str("    uint64_t bits = (uint64_t)left;\n");
    out.push_str("    if (right == 0) { return left; }\n");
    out.push_str("    if (left >= 0) { return (long long)(bits >> right); }\n");
    out.push_str("    uint64_t shifted = (bits >> right) | (~UINT64_C(0) << (64U - right));\n");
    out.push_str("    return -1 - (long long)(UINT64_MAX - shifted);\n");
    out.push_str("}\n\n");
    out.push_str("static int32_t nomo_shl_i32(int32_t left, uint64_t right) {\n");
    out.push_str(
        "    if (right >= sizeof(left) * CHAR_BIT) { nomo_panic(\"invalid shift amount\"); }\n",
    );
    out.push_str("    return left << right;\n");
    out.push_str("}\n\n");
    out.push_str("static int32_t nomo_shr_i32(int32_t left, uint64_t right) {\n");
    out.push_str(
        "    if (right >= sizeof(left) * CHAR_BIT) { nomo_panic(\"invalid shift amount\"); }\n",
    );
    out.push_str("    uint32_t bits = (uint32_t)left;\n");
    out.push_str("    if (right == 0) { return left; }\n");
    out.push_str("    if (left >= 0) { return (int32_t)(bits >> right); }\n");
    out.push_str("    uint32_t shifted = (bits >> right) | (~UINT32_C(0) << (32U - right));\n");
    out.push_str("    return -1 - (int32_t)(UINT32_MAX - shifted);\n");
    out.push_str("}\n\n");
    out.push_str("static uint32_t nomo_shl_u32(uint32_t left, uint64_t right) {\n");
    out.push_str(
        "    if (right >= sizeof(left) * CHAR_BIT) { nomo_panic(\"invalid shift amount\"); }\n",
    );
    out.push_str("    return left << right;\n");
    out.push_str("}\n\n");
    out.push_str("static uint32_t nomo_shr_u32(uint32_t left, uint64_t right) {\n");
    out.push_str(
        "    if (right >= sizeof(left) * CHAR_BIT) { nomo_panic(\"invalid shift amount\"); }\n",
    );
    out.push_str("    return left >> right;\n");
    out.push_str("}\n\n");
    out.push_str("static uint64_t nomo_shl_u64(uint64_t left, uint64_t right) {\n");
    out.push_str(
        "    if (right >= sizeof(left) * CHAR_BIT) { nomo_panic(\"invalid shift amount\"); }\n",
    );
    out.push_str("    return left << right;\n");
    out.push_str("}\n\n");
    out.push_str("static uint64_t nomo_shr_u64(uint64_t left, uint64_t right) {\n");
    out.push_str(
        "    if (right >= sizeof(left) * CHAR_BIT) { nomo_panic(\"invalid shift amount\"); }\n",
    );
    out.push_str("    return left >> right;\n");
    out.push_str("}\n\n");
    out.push_str("static long long nomo_wrap_i64(uint64_t bits) {\n");
    out.push_str("    if (bits <= (uint64_t)LLONG_MAX) { return (long long)bits; }\n");
    out.push_str("    return -1 - (long long)(UINT64_MAX - bits);\n");
    out.push_str("}\n\n");
    out.push_str("static int32_t nomo_wrap_i32(uint32_t bits) {\n");
    out.push_str("    if (bits <= (uint32_t)INT32_MAX) { return (int32_t)bits; }\n");
    out.push_str("    return (int32_t)(-1 - (int32_t)(UINT32_MAX - bits));\n");
    out.push_str("}\n\n");
    out.push_str("static long long nomo_num_wrapping_add_i64(long long left, long long right) {\n");
    out.push_str("    return nomo_wrap_i64((uint64_t)left + (uint64_t)right);\n");
    out.push_str("}\n\n");
    out.push_str("static long long nomo_num_wrapping_sub_i64(long long left, long long right) {\n");
    out.push_str("    return nomo_wrap_i64((uint64_t)left - (uint64_t)right);\n");
    out.push_str("}\n\n");
    out.push_str("static long long nomo_num_wrapping_mul_i64(long long left, long long right) {\n");
    out.push_str("    return nomo_wrap_i64((uint64_t)left * (uint64_t)right);\n");
    out.push_str("}\n\n");
    out.push_str("static int32_t nomo_num_wrapping_add_i32(int32_t left, int32_t right) {\n");
    out.push_str("    return nomo_wrap_i32((uint32_t)left + (uint32_t)right);\n");
    out.push_str("}\n\n");
    out.push_str("static int32_t nomo_num_wrapping_sub_i32(int32_t left, int32_t right) {\n");
    out.push_str("    return nomo_wrap_i32((uint32_t)left - (uint32_t)right);\n");
    out.push_str("}\n\n");
    out.push_str("static int32_t nomo_num_wrapping_mul_i32(int32_t left, int32_t right) {\n");
    out.push_str("    return nomo_wrap_i32((uint32_t)left * (uint32_t)right);\n");
    out.push_str("}\n\n");
    out.push_str("static uint32_t nomo_num_wrapping_add_u32(uint32_t left, uint32_t right) {\n");
    out.push_str("    return left + right;\n");
    out.push_str("}\n\n");
    out.push_str("static uint32_t nomo_num_wrapping_sub_u32(uint32_t left, uint32_t right) {\n");
    out.push_str("    return left - right;\n");
    out.push_str("}\n\n");
    out.push_str("static uint32_t nomo_num_wrapping_mul_u32(uint32_t left, uint32_t right) {\n");
    out.push_str("    return left * right;\n");
    out.push_str("}\n\n");
    out.push_str("static uint64_t nomo_num_wrapping_add_u64(uint64_t left, uint64_t right) {\n");
    out.push_str("    return left + right;\n");
    out.push_str("}\n\n");
    out.push_str("static uint64_t nomo_num_wrapping_sub_u64(uint64_t left, uint64_t right) {\n");
    out.push_str("    return left - right;\n");
    out.push_str("}\n\n");
    out.push_str("static uint64_t nomo_num_wrapping_mul_u64(uint64_t left, uint64_t right) {\n");
    out.push_str("    return left * right;\n");
    out.push_str("}\n");
}

pub(super) fn emit_math_runtime(out: &mut String) {
    out.push_str("static int64_t nomo_math_abs_i64(int64_t value) {\n");
    out.push_str("    if (value == INT64_MIN) { nomo_panic(\"integer overflow\"); }\n");
    out.push_str("    return value < 0 ? -value : value;\n");
    out.push_str("}\n\n");
    out.push_str("static int32_t nomo_math_abs_i32(int32_t value) {\n");
    out.push_str("    if (value == INT32_MIN) { nomo_panic(\"integer overflow\"); }\n");
    out.push_str("    return value < 0 ? -value : value;\n");
    out.push_str("}\n\n");
    out.push_str("static uint32_t nomo_math_abs_u32(uint32_t value) {\n");
    out.push_str("    return value;\n");
    out.push_str("}\n\n");
    out.push_str("static uint64_t nomo_math_abs_u64(uint64_t value) {\n");
    out.push_str("    return value;\n");
    out.push_str("}\n\n");
    out.push_str("static double nomo_math_abs_f64(double value) {\n");
    out.push_str("    return fabs(value);\n");
    out.push_str("}\n\n");
    out.push_str("static int64_t nomo_math_min_i64(int64_t left, int64_t right) {\n");
    out.push_str("    return left < right ? left : right;\n");
    out.push_str("}\n\n");
    out.push_str("static int32_t nomo_math_min_i32(int32_t left, int32_t right) {\n");
    out.push_str("    return left < right ? left : right;\n");
    out.push_str("}\n\n");
    out.push_str("static uint32_t nomo_math_min_u32(uint32_t left, uint32_t right) {\n");
    out.push_str("    return left < right ? left : right;\n");
    out.push_str("}\n\n");
    out.push_str("static uint64_t nomo_math_min_u64(uint64_t left, uint64_t right) {\n");
    out.push_str("    return left < right ? left : right;\n");
    out.push_str("}\n\n");
    out.push_str("static double nomo_math_min_f64(double left, double right) {\n");
    out.push_str("    return left < right ? left : right;\n");
    out.push_str("}\n\n");
    out.push_str("static int64_t nomo_math_max_i64(int64_t left, int64_t right) {\n");
    out.push_str("    return left > right ? left : right;\n");
    out.push_str("}\n\n");
    out.push_str("static int32_t nomo_math_max_i32(int32_t left, int32_t right) {\n");
    out.push_str("    return left > right ? left : right;\n");
    out.push_str("}\n\n");
    out.push_str("static uint32_t nomo_math_max_u32(uint32_t left, uint32_t right) {\n");
    out.push_str("    return left > right ? left : right;\n");
    out.push_str("}\n\n");
    out.push_str("static uint64_t nomo_math_max_u64(uint64_t left, uint64_t right) {\n");
    out.push_str("    return left > right ? left : right;\n");
    out.push_str("}\n\n");
    out.push_str("static double nomo_math_max_f64(double left, double right) {\n");
    out.push_str("    return left > right ? left : right;\n");
    out.push_str("}\n");
}

pub(super) fn emit_string_runtime(out: &mut String) {
    out.push_str("typedef struct nomo_string {\n");
    out.push_str("    const char *data;\n");
    out.push_str("    size_t *refcount;\n");
    out.push_str("} nomo_string;\n\n");
    out.push_str("static nomo_string nomo_string_literal(const char *data) {\n");
    out.push_str("    return (nomo_string){.data = data, .refcount = NULL};\n");
    out.push_str("}\n\n");
    out.push_str("static nomo_string nomo_string_owned(char *data) {\n");
    out.push_str("    size_t *refcount = (size_t *)malloc(sizeof(size_t));\n");
    out.push_str("    if (refcount == NULL) {\n");
    out.push_str("        free(data);\n");
    out.push_str("        nomo_panic(\"out of memory\");\n");
    out.push_str("    }\n");
    out.push_str("    *refcount = 1;\n");
    out.push_str("    return (nomo_string){.data = data, .refcount = refcount};\n");
    out.push_str("}\n\n");
    out.push_str("static nomo_string nomo_string_from_cstr(const char *value) {\n");
    out.push_str("    size_t len = strlen(value);\n");
    out.push_str("    char *data = (char *)malloc(len + 1);\n");
    out.push_str("    if (data == NULL) { nomo_panic(\"out of memory\"); }\n");
    out.push_str("    memcpy(data, value, len + 1);\n");
    out.push_str("    return nomo_string_owned(data);\n");
    out.push_str("}\n\n");
    out.push_str(
        "static nomo_string nomo_string_from_slice(const char *data, size_t start, size_t len) {\n",
    );
    out.push_str("    char *out = (char *)malloc(len + 1);\n");
    out.push_str("    if (out == NULL) { nomo_panic(\"out of memory\"); }\n");
    out.push_str("    memcpy(out, data + start, len);\n");
    out.push_str("    out[len] = '\\0';\n");
    out.push_str("    return nomo_string_owned(out);\n");
    out.push_str("}\n\n");
    out.push_str("static nomo_string nomo_string_retain(nomo_string value) {\n");
    out.push_str("    if (value.refcount != NULL) { *value.refcount += 1; }\n");
    out.push_str("    return value;\n");
    out.push_str("}\n\n");
    out.push_str("static void nomo_string_release(nomo_string value) {\n");
    out.push_str("    if (value.refcount == NULL) { return; }\n");
    out.push_str("    *value.refcount -= 1;\n");
    out.push_str("    if (*value.refcount != 0) { return; }\n");
    out.push_str("    free((char *)value.data);\n");
    out.push_str("    free(value.refcount);\n");
    out.push_str("}\n\n");
    out.push_str("static nomo_string nomo_string_concat(nomo_string left, nomo_string right) {\n");
    out.push_str("    size_t left_len = strlen(left.data);\n");
    out.push_str("    size_t right_len = strlen(right.data);\n");
    out.push_str("    char *out = (char *)malloc(left_len + right_len + 1);\n");
    out.push_str("    if (out == NULL) {\n");
    out.push_str("        nomo_panic(\"out of memory\");\n");
    out.push_str("    }\n");
    out.push_str("    memcpy(out, left.data, left_len);\n");
    out.push_str("    memcpy(out + left_len, right.data, right_len + 1);\n");
    out.push_str("    return nomo_string_owned(out);\n");
    out.push_str("}\n");
    out.push_str("\nstatic int nomo_string_equal(nomo_string left, nomo_string right) {\n");
    out.push_str("    return strcmp(left.data, right.data) == 0;\n");
    out.push_str("}\n");
    out.push_str("\nstatic int nomo_string_is_empty(nomo_string value) {\n");
    out.push_str("    return value.data[0] == '\\0';\n");
    out.push_str("}\n");
    out.push_str("\nstatic int nomo_string_contains(nomo_string value, nomo_string needle) {\n");
    out.push_str("    return strstr(value.data, needle.data) != NULL;\n");
    out.push_str("}\n");
    out.push_str("\nstatic int nomo_string_starts_with(nomo_string value, nomo_string prefix) {\n");
    out.push_str("    size_t prefix_len = strlen(prefix.data);\n");
    out.push_str("    return strncmp(value.data, prefix.data, prefix_len) == 0;\n");
    out.push_str("}\n");
    out.push_str("\nstatic int nomo_string_ends_with(nomo_string value, nomo_string suffix) {\n");
    out.push_str("    size_t value_len = strlen(value.data);\n");
    out.push_str("    size_t suffix_len = strlen(suffix.data);\n");
    out.push_str("    if (suffix_len > value_len) { return 0; }\n");
    out.push_str(
        "    return memcmp(value.data + value_len - suffix_len, suffix.data, suffix_len) == 0;\n",
    );
    out.push_str("}\n");
    out.push_str("\nstatic nomo_string nomo_string_trim(nomo_string value) {\n");
    out.push_str("    size_t start = 0;\n");
    out.push_str("    size_t end = strlen(value.data);\n");
    out.push_str(
        "    while (start < end && isspace((unsigned char)value.data[start])) { start += 1; }\n",
    );
    out.push_str(
        "    while (end > start && isspace((unsigned char)value.data[end - 1])) { end -= 1; }\n",
    );
    out.push_str("    return nomo_string_from_slice(value.data, start, end - start);\n");
    out.push_str("}\n");
    out.push_str("\nstatic nomo_string nomo_string_to_lower(nomo_string value) {\n");
    out.push_str("    size_t len = strlen(value.data);\n");
    out.push_str("    char *out = (char *)malloc(len + 1);\n");
    out.push_str("    if (out == NULL) { nomo_panic(\"out of memory\"); }\n");
    out.push_str("    for (size_t i = 0; i < len; i += 1) { out[i] = (char)tolower((unsigned char)value.data[i]); }\n");
    out.push_str("    out[len] = '\\0';\n");
    out.push_str("    return nomo_string_owned(out);\n");
    out.push_str("}\n");
    out.push_str("\nstatic nomo_string nomo_string_to_upper(nomo_string value) {\n");
    out.push_str("    size_t len = strlen(value.data);\n");
    out.push_str("    char *out = (char *)malloc(len + 1);\n");
    out.push_str("    if (out == NULL) { nomo_panic(\"out of memory\"); }\n");
    out.push_str("    for (size_t i = 0; i < len; i += 1) { out[i] = (char)toupper((unsigned char)value.data[i]); }\n");
    out.push_str("    out[len] = '\\0';\n");
    out.push_str("    return nomo_string_owned(out);\n");
    out.push_str("}\n");
    out.push_str("\nstatic int nomo_char_is_digit(uint32_t value) {\n");
    out.push_str("    return value <= 127 && isdigit((unsigned char)value) != 0;\n");
    out.push_str("}\n");
    out.push_str("\nstatic int nomo_char_is_alpha(uint32_t value) {\n");
    out.push_str("    return value <= 127 && isalpha((unsigned char)value) != 0;\n");
    out.push_str("}\n");
    out.push_str("\nstatic int nomo_char_is_whitespace(uint32_t value) {\n");
    out.push_str("    return value <= 127 && isspace((unsigned char)value) != 0;\n");
    out.push_str("}\n");
    out.push_str("\nstatic nomo_string nomo_char_to_string(uint32_t value) {\n");
    out.push_str("    char *out = (char *)malloc(5);\n");
    out.push_str("    if (out == NULL) { nomo_panic(\"out of memory\"); }\n");
    out.push_str("    if (value <= 0x7F) {\n");
    out.push_str("        out[0] = (char)value;\n");
    out.push_str("        out[1] = '\\0';\n");
    out.push_str("    } else if (value <= 0x7FF) {\n");
    out.push_str("        out[0] = (char)(0xC0 | (value >> 6));\n");
    out.push_str("        out[1] = (char)(0x80 | (value & 0x3F));\n");
    out.push_str("        out[2] = '\\0';\n");
    out.push_str("    } else if (value <= 0xFFFF) {\n");
    out.push_str("        out[0] = (char)(0xE0 | (value >> 12));\n");
    out.push_str("        out[1] = (char)(0x80 | ((value >> 6) & 0x3F));\n");
    out.push_str("        out[2] = (char)(0x80 | (value & 0x3F));\n");
    out.push_str("        out[3] = '\\0';\n");
    out.push_str("    } else if (value <= 0x10FFFF) {\n");
    out.push_str("        out[0] = (char)(0xF0 | (value >> 18));\n");
    out.push_str("        out[1] = (char)(0x80 | ((value >> 12) & 0x3F));\n");
    out.push_str("        out[2] = (char)(0x80 | ((value >> 6) & 0x3F));\n");
    out.push_str("        out[3] = (char)(0x80 | (value & 0x3F));\n");
    out.push_str("        out[4] = '\\0';\n");
    out.push_str("    } else {\n");
    out.push_str("        out[0] = '?';\n");
    out.push_str("        out[1] = '\\0';\n");
    out.push_str("    }\n");
    out.push_str("    return nomo_string_owned(out);\n");
    out.push_str("}\n");
    out.push_str("\nstatic nomo_string nomo_os_platform(void) {\n");
    out.push_str("#if defined(_WIN32)\n");
    out.push_str("    return nomo_string_literal(\"windows\");\n");
    out.push_str("#elif defined(__APPLE__)\n");
    out.push_str("    return nomo_string_literal(\"macos\");\n");
    out.push_str("#elif defined(__linux__)\n");
    out.push_str("    return nomo_string_literal(\"linux\");\n");
    out.push_str("#elif defined(__FreeBSD__)\n");
    out.push_str("    return nomo_string_literal(\"freebsd\");\n");
    out.push_str("#else\n");
    out.push_str("    return nomo_string_literal(\"unknown\");\n");
    out.push_str("#endif\n");
    out.push_str("}\n");
    out.push_str("\nstatic nomo_string nomo_os_arch(void) {\n");
    out.push_str("#if defined(__aarch64__) || defined(_M_ARM64)\n");
    out.push_str("    return nomo_string_literal(\"aarch64\");\n");
    out.push_str("#elif defined(__x86_64__) || defined(_M_X64)\n");
    out.push_str("    return nomo_string_literal(\"x86_64\");\n");
    out.push_str("#elif defined(__i386__) || defined(_M_IX86)\n");
    out.push_str("    return nomo_string_literal(\"x86\");\n");
    out.push_str("#elif defined(__arm__) || defined(_M_ARM)\n");
    out.push_str("    return nomo_string_literal(\"arm\");\n");
    out.push_str("#else\n");
    out.push_str("    return nomo_string_literal(\"unknown\");\n");
    out.push_str("#endif\n");
    out.push_str("}\n");
    out.push_str("\nstatic nomo_string nomo_os_path_separator(void) {\n");
    out.push_str("#if defined(_WIN32)\n");
    out.push_str("    return nomo_string_literal(\"\\\\\");\n");
    out.push_str("#else\n");
    out.push_str("    return nomo_string_literal(\"/\");\n");
    out.push_str("#endif\n");
    out.push_str("}\n");
    out.push_str("\nstatic nomo_string nomo_os_line_ending(void) {\n");
    out.push_str("#if defined(_WIN32)\n");
    out.push_str("    return nomo_string_literal(\"\\r\\n\");\n");
    out.push_str("#else\n");
    out.push_str("    return nomo_string_literal(\"\\n\");\n");
    out.push_str("#endif\n");
    out.push_str("}\n");
    out.push_str("\nstatic int64_t nomo_time_now_millis(void) {\n");
    out.push_str("#if defined(_WIN32)\n");
    out.push_str("    FILETIME ft;\n");
    out.push_str("    ULARGE_INTEGER value;\n");
    out.push_str("    GetSystemTimeAsFileTime(&ft);\n");
    out.push_str("    value.LowPart = ft.dwLowDateTime;\n");
    out.push_str("    value.HighPart = ft.dwHighDateTime;\n");
    out.push_str("    return (int64_t)((value.QuadPart - 116444736000000000ULL) / 10000ULL);\n");
    out.push_str("#else\n");
    out.push_str("    struct timeval tv;\n");
    out.push_str(
        "    if (gettimeofday(&tv, NULL) != 0) { nomo_panic(\"time.now_millis failed\"); }\n",
    );
    out.push_str("    return ((int64_t)tv.tv_sec * 1000) + ((int64_t)tv.tv_usec / 1000);\n");
    out.push_str("#endif\n");
    out.push_str("}\n");
    out.push_str("\nstatic int64_t nomo_time_monotonic_millis(void) {\n");
    out.push_str("#if defined(_WIN32)\n");
    out.push_str("    return (int64_t)GetTickCount64();\n");
    out.push_str("#else\n");
    out.push_str("    struct timespec ts;\n");
    out.push_str("    if (clock_gettime(CLOCK_MONOTONIC, &ts) != 0) { nomo_panic(\"time.monotonic_millis failed\"); }\n");
    out.push_str("    return ((int64_t)ts.tv_sec * 1000) + ((int64_t)ts.tv_nsec / 1000000);\n");
    out.push_str("#endif\n");
    out.push_str("}\n");
    out.push_str("\nstatic void nomo_time_sleep_millis(int64_t duration) {\n");
    out.push_str("    if (duration < 0) { nomo_panic(\"time.sleep_millis duration must be non-negative\"); }\n");
    out.push_str("#if defined(_WIN32)\n");
    out.push_str("    Sleep((DWORD)duration);\n");
    out.push_str("#else\n");
    out.push_str("    struct timespec request;\n");
    out.push_str("    request.tv_sec = (time_t)(duration / 1000);\n");
    out.push_str("    request.tv_nsec = (long)((duration % 1000) * 1000000);\n");
    out.push_str("    while (nanosleep(&request, &request) != 0) {\n");
    out.push_str("        if (errno != EINTR) { nomo_panic(\"time.sleep_millis failed\"); }\n");
    out.push_str("    }\n");
    out.push_str("#endif\n");
    out.push_str("}\n");
    out.push_str("\nstatic int64_t nomo_time_duration_seconds_to_millis(int64_t seconds) {\n");
    out.push_str("    if (seconds > INT64_MAX / 1000 || seconds < INT64_MIN / 1000) { nomo_panic(\"time.duration_seconds overflow\"); }\n");
    out.push_str("    return seconds * 1000;\n");
    out.push_str("}\n");
    out.push_str("\nstatic nomo_string nomo_time_format_duration_millis(int64_t millis) {\n");
    out.push_str("    char buffer[64];\n");
    out.push_str("    snprintf(buffer, sizeof(buffer), \"%\" PRId64 \"ms\", millis);\n");
    out.push_str("    return nomo_string_from_cstr(buffer);\n");
    out.push_str("}\n");
    out.push_str("\nstatic nomo_string nomo_num_i64_to_string(int64_t value) {\n");
    out.push_str("    char buffer[64];\n");
    out.push_str("    snprintf(buffer, sizeof(buffer), \"%\" PRId64, value);\n");
    out.push_str("    return nomo_string_from_cstr(buffer);\n");
    out.push_str("}\n");
    out.push_str("\nstatic nomo_string nomo_num_i32_to_string(int32_t value) {\n");
    out.push_str("    char buffer[32];\n");
    out.push_str("    snprintf(buffer, sizeof(buffer), \"%\" PRId32, value);\n");
    out.push_str("    return nomo_string_from_cstr(buffer);\n");
    out.push_str("}\n");
    out.push_str("\nstatic nomo_string nomo_num_u32_to_string(uint32_t value) {\n");
    out.push_str("    char buffer[32];\n");
    out.push_str("    snprintf(buffer, sizeof(buffer), \"%\" PRIu32, value);\n");
    out.push_str("    return nomo_string_from_cstr(buffer);\n");
    out.push_str("}\n");
    out.push_str("\nstatic nomo_string nomo_num_u64_to_string(uint64_t value) {\n");
    out.push_str("    char buffer[64];\n");
    out.push_str("    snprintf(buffer, sizeof(buffer), \"%\" PRIu64, value);\n");
    out.push_str("    return nomo_string_from_cstr(buffer);\n");
    out.push_str("}\n");
    out.push_str("\nstatic nomo_string nomo_num_f64_to_string(double value) {\n");
    out.push_str("    char buffer[128];\n");
    out.push_str("    snprintf(buffer, sizeof(buffer), \"%.17g\", value);\n");
    out.push_str("    return nomo_string_from_cstr(buffer);\n");
    out.push_str("}\n");
    out.push_str("\nstatic nomo_string nomo_path_string_from_slice(const char *data, size_t start, size_t len) {\n");
    out.push_str("    return nomo_string_from_slice(data, start, len);\n");
    out.push_str("}\n\n");
    out.push_str("static size_t nomo_path_trim_trailing_slashes(const char *data) {\n");
    out.push_str("    size_t len = strlen(data);\n");
    out.push_str("    while (len > 1 && data[len - 1] == '/') { len -= 1; }\n");
    out.push_str("    return len;\n");
    out.push_str("}\n\n");
    out.push_str("static nomo_string nomo_path_join(nomo_string left, nomo_string right) {\n");
    out.push_str("    if (right.data[0] == '/' || left.data[0] == '\\0') {\n");
    out.push_str("        return nomo_string_from_cstr(right.data);\n");
    out.push_str("    }\n");
    out.push_str("    if (right.data[0] == '\\0') {\n");
    out.push_str("        return nomo_string_from_cstr(left.data);\n");
    out.push_str("    }\n");
    out.push_str("    size_t left_len = strlen(left.data);\n");
    out.push_str("    size_t right_len = strlen(right.data);\n");
    out.push_str("    int needs_sep = left.data[left_len - 1] != '/';\n");
    out.push_str("    char *out = (char *)malloc(left_len + (size_t)needs_sep + right_len + 1);\n");
    out.push_str("    if (out == NULL) { nomo_panic(\"out of memory\"); }\n");
    out.push_str("    memcpy(out, left.data, left_len);\n");
    out.push_str("    size_t offset = left_len;\n");
    out.push_str("    if (needs_sep) { out[offset] = '/'; offset += 1; }\n");
    out.push_str("    memcpy(out + offset, right.data, right_len + 1);\n");
    out.push_str("    return nomo_string_owned(out);\n");
    out.push_str("}\n\n");
    out.push_str("static nomo_string nomo_path_basename(nomo_string path) {\n");
    out.push_str("    size_t len = nomo_path_trim_trailing_slashes(path.data);\n");
    out.push_str("    if (len == 0) { return nomo_string_literal(\"\"); }\n");
    out.push_str(
        "    if (len == 1 && path.data[0] == '/') { return nomo_string_literal(\"/\"); }\n",
    );
    out.push_str("    size_t start = len;\n");
    out.push_str("    while (start > 0 && path.data[start - 1] != '/') { start -= 1; }\n");
    out.push_str("    return nomo_path_string_from_slice(path.data, start, len - start);\n");
    out.push_str("}\n\n");
    out.push_str("static nomo_string nomo_path_dirname(nomo_string path) {\n");
    out.push_str("    size_t len = nomo_path_trim_trailing_slashes(path.data);\n");
    out.push_str("    if (len == 0) { return nomo_string_literal(\".\"); }\n");
    out.push_str(
        "    if (len == 1 && path.data[0] == '/') { return nomo_string_literal(\"/\"); }\n",
    );
    out.push_str("    size_t slash = len;\n");
    out.push_str("    while (slash > 0 && path.data[slash - 1] != '/') { slash -= 1; }\n");
    out.push_str("    if (slash == 0) { return nomo_string_literal(\".\"); }\n");
    out.push_str("    while (slash > 1 && path.data[slash - 1] == '/') { slash -= 1; }\n");
    out.push_str("    return nomo_path_string_from_slice(path.data, 0, slash);\n");
    out.push_str("}\n\n");
    out.push_str("static nomo_string nomo_path_extension(nomo_string path) {\n");
    out.push_str("    size_t len = nomo_path_trim_trailing_slashes(path.data);\n");
    out.push_str("    size_t start = len;\n");
    out.push_str("    while (start > 0 && path.data[start - 1] != '/') { start -= 1; }\n");
    out.push_str("    size_t dot = len;\n");
    out.push_str("    while (dot > start && path.data[dot - 1] != '.') { dot -= 1; }\n");
    out.push_str("    if (dot == start || dot == len) { return nomo_string_literal(\"\"); }\n");
    out.push_str("    return nomo_path_string_from_slice(path.data, dot, len - dot);\n");
    out.push_str("}\n\n");
    out.push_str("static int nomo_path_is_absolute(nomo_string path) {\n");
    out.push_str("    return path.data[0] == '/';\n");
    out.push_str("}\n\n");
    out.push_str("static int nomo_path_prev_segment_is_dotdot(char *out, size_t *starts, size_t *lens, size_t count) {\n");
    out.push_str("    if (count == 0) { return 0; }\n");
    out.push_str("    size_t start = starts[count - 1];\n");
    out.push_str("    if (out[start] == '/') { start += 1; }\n");
    out.push_str(
        "    return lens[count - 1] == 2 && out[start] == '.' && out[start + 1] == '.';\n",
    );
    out.push_str("}\n\n");
    out.push_str("static void nomo_path_append_segment(char *out, size_t *out_len, size_t *starts, size_t *lens, size_t *count, const char *segment, size_t segment_len) {\n");
    out.push_str("    size_t restore = *out_len;\n");
    out.push_str("    if (*out_len > 0 && out[*out_len - 1] != '/') { out[*out_len] = '/'; *out_len += 1; }\n");
    out.push_str("    starts[*count] = restore;\n");
    out.push_str("    lens[*count] = segment_len;\n");
    out.push_str("    *count += 1;\n");
    out.push_str("    memcpy(out + *out_len, segment, segment_len);\n");
    out.push_str("    *out_len += segment_len;\n");
    out.push_str("    (void)restore;\n");
    out.push_str("}\n\n");
    out.push_str("static nomo_string nomo_path_normalize(nomo_string path) {\n");
    out.push_str("    const char *data = path.data;\n");
    out.push_str("    size_t len = strlen(data);\n");
    out.push_str("    if (len == 0) { return nomo_string_literal(\".\"); }\n");
    out.push_str("    int absolute = data[0] == '/';\n");
    out.push_str("    char *out = (char *)malloc(len + 2);\n");
    out.push_str("    size_t *starts = (size_t *)malloc((len + 1) * sizeof(size_t));\n");
    out.push_str("    size_t *lens = (size_t *)malloc((len + 1) * sizeof(size_t));\n");
    out.push_str("    if (out == NULL || starts == NULL || lens == NULL) {\n");
    out.push_str("        free(out); free(starts); free(lens);\n");
    out.push_str("        nomo_panic(\"out of memory\");\n");
    out.push_str("    }\n");
    out.push_str("    size_t out_len = 0;\n");
    out.push_str("    size_t count = 0;\n");
    out.push_str("    if (absolute) { out[out_len] = '/'; out_len += 1; }\n");
    out.push_str("    size_t index = 0;\n");
    out.push_str("    while (index < len) {\n");
    out.push_str("        while (index < len && data[index] == '/') { index += 1; }\n");
    out.push_str("        size_t start = index;\n");
    out.push_str("        while (index < len && data[index] != '/') { index += 1; }\n");
    out.push_str("        size_t segment_len = index - start;\n");
    out.push_str(
        "        if (segment_len == 0 || (segment_len == 1 && data[start] == '.')) { continue; }\n",
    );
    out.push_str(
        "        if (segment_len == 2 && data[start] == '.' && data[start + 1] == '.') {\n",
    );
    out.push_str("            if (count > 0 && !nomo_path_prev_segment_is_dotdot(out, starts, lens, count)) {\n");
    out.push_str("                count -= 1;\n");
    out.push_str("                out_len = starts[count];\n");
    out.push_str(
        "                if (absolute && out_len == 0) { out[out_len] = '/'; out_len += 1; }\n",
    );
    out.push_str("            } else if (!absolute) {\n");
    out.push_str("                nomo_path_append_segment(out, &out_len, starts, lens, &count, data + start, segment_len);\n");
    out.push_str("            }\n");
    out.push_str("        } else {\n");
    out.push_str("            nomo_path_append_segment(out, &out_len, starts, lens, &count, data + start, segment_len);\n");
    out.push_str("        }\n");
    out.push_str("    }\n");
    out.push_str("    if (out_len == 0) { out[out_len] = '.'; out_len += 1; }\n");
    out.push_str("    out[out_len] = '\\0';\n");
    out.push_str("    free(starts); free(lens);\n");
    out.push_str("    return nomo_string_owned(out);\n");
    out.push_str("}\n");
}

pub(super) fn emit_log_enabled_helper(out: &mut String) {
    out.push_str("static int32_t nomo_log_level_value(const char *level) {\n");
    out.push_str("    if (strcmp(level, \"debug\") == 0) { return 0; }\n");
    out.push_str("    if (strcmp(level, \"info\") == 0) { return 1; }\n");
    out.push_str(
        "    if (strcmp(level, \"warn\") == 0 || strcmp(level, \"warning\") == 0) { return 2; }\n",
    );
    out.push_str("    if (strcmp(level, \"error\") == 0) { return 3; }\n");
    out.push_str("    if (strcmp(level, \"off\") == 0) { return 4; }\n");
    out.push_str("    return 1;\n");
    out.push_str("}\n\n");
    out.push_str("static int32_t nomo_log_enabled(nomo_string level) {\n");
    out.push_str("    const char *filter = getenv(\"NOMO_LOG\");\n");
    out.push_str("    int32_t threshold = filter == NULL ? 1 : nomo_log_level_value(filter);\n");
    out.push_str("    int32_t current = nomo_log_level_value(level.data);\n");
    out.push_str("    return threshold < 4 && current >= threshold;\n");
    out.push_str("}\n");
}

pub(super) fn emit_hash_helpers(out: &mut String) {
    let hash_state = c_type(&ValueType::Struct("HashState".to_string(), Vec::new()));
    let value_field = c_member_ident("value");
    out.push_str("static const uint64_t NOMO_HASH_OFFSET = UINT64_C(14695981039346656037);\n");
    out.push_str("static const uint64_t NOMO_HASH_PRIME = UINT64_C(1099511628211);\n\n");
    out.push_str("static uint64_t nomo_hash_write_cstr(uint64_t state, const char *data) {\n");
    out.push_str("    const unsigned char *bytes = (const unsigned char *)data;\n");
    out.push_str("    while (*bytes != '\\0') {\n");
    out.push_str("        state ^= (uint64_t)(*bytes);\n");
    out.push_str("        state *= NOMO_HASH_PRIME;\n");
    out.push_str("        bytes += 1;\n");
    out.push_str("    }\n");
    out.push_str("    return state;\n");
    out.push_str("}\n\n");
    out.push_str(
        "static uint64_t nomo_hash_write_array_u32(uint64_t state, nomo_array_u32 data) {\n",
    );
    out.push_str("    for (size_t i = 0; i < data.len; i += 1) {\n");
    out.push_str("        state ^= (uint64_t)(data.data[i] & 0xffU);\n");
    out.push_str("        state *= NOMO_HASH_PRIME;\n");
    out.push_str("    }\n");
    out.push_str("    return state;\n");
    out.push_str("}\n\n");
    out.push_str("static ");
    out.push_str(&hash_state);
    out.push_str(" nomo_hash_new(void) {\n");
    out.push_str("    return (");
    out.push_str(&hash_state);
    out.push_str("){.");
    out.push_str(&value_field);
    out.push_str(" = NOMO_HASH_OFFSET};\n");
    out.push_str("}\n\n");
    out.push_str("static uint64_t nomo_hash_string(nomo_string value) {\n");
    out.push_str("    return nomo_hash_write_cstr(NOMO_HASH_OFFSET, value.data);\n");
    out.push_str("}\n\n");
    out.push_str("static uint64_t nomo_hash_bytes(nomo_array_u32 value) {\n");
    out.push_str("    return nomo_hash_write_array_u32(NOMO_HASH_OFFSET, value);\n");
    out.push_str("}\n\n");
    out.push_str("static ");
    out.push_str(&hash_state);
    out.push_str(" nomo_hash_write_string(");
    out.push_str(&hash_state);
    out.push_str(" state, nomo_string value) {\n");
    out.push_str("    return (");
    out.push_str(&hash_state);
    out.push_str("){.");
    out.push_str(&value_field);
    out.push_str(" = nomo_hash_write_cstr(state.");
    out.push_str(&value_field);
    out.push_str(", value.data)};\n");
    out.push_str("}\n\n");
    out.push_str("static ");
    out.push_str(&hash_state);
    out.push_str(" nomo_hash_write_bytes(");
    out.push_str(&hash_state);
    out.push_str(" state, nomo_array_u32 value) {\n");
    out.push_str("    return (");
    out.push_str(&hash_state);
    out.push_str("){.");
    out.push_str(&value_field);
    out.push_str(" = nomo_hash_write_array_u32(state.");
    out.push_str(&value_field);
    out.push_str(", value)};\n");
    out.push_str("}\n\n");
    out.push_str("static uint64_t nomo_hash_finish(");
    out.push_str(&hash_state);
    out.push_str(" state) {\n");
    out.push_str("    return state.");
    out.push_str(&value_field);
    out.push_str(";\n");
    out.push_str("}\n");
}

pub(super) fn emit_crypto_helpers(out: &mut String) {
    out.push_str(
        r#"static uint32_t nomo_crypto_rotr32(uint32_t value, uint32_t amount) {
    return (value >> amount) | (value << (32U - amount));
}

static uint64_t nomo_crypto_rotr64(uint64_t value, uint64_t amount) {
    return (value >> amount) | (value << (64U - amount));
}

static nomo_string nomo_crypto_hex_string(const unsigned char *digest, size_t len) {
    static const char hex[] = "0123456789abcdef";
    char *out = (char *)malloc(len * 2 + 1);
    if (out == NULL) { nomo_panic("out of memory"); }
    for (size_t i = 0; i < len; i += 1) {
        out[i * 2] = hex[(digest[i] >> 4) & 0x0f];
        out[i * 2 + 1] = hex[digest[i] & 0x0f];
    }
    out[len * 2] = '\0';
    return nomo_string_owned(out);
}

static nomo_string nomo_crypto_sha256(nomo_string value) {
    static const uint32_t k[64] = {
        UINT32_C(0x428a2f98), UINT32_C(0x71374491), UINT32_C(0xb5c0fbcf), UINT32_C(0xe9b5dba5),
        UINT32_C(0x3956c25b), UINT32_C(0x59f111f1), UINT32_C(0x923f82a4), UINT32_C(0xab1c5ed5),
        UINT32_C(0xd807aa98), UINT32_C(0x12835b01), UINT32_C(0x243185be), UINT32_C(0x550c7dc3),
        UINT32_C(0x72be5d74), UINT32_C(0x80deb1fe), UINT32_C(0x9bdc06a7), UINT32_C(0xc19bf174),
        UINT32_C(0xe49b69c1), UINT32_C(0xefbe4786), UINT32_C(0x0fc19dc6), UINT32_C(0x240ca1cc),
        UINT32_C(0x2de92c6f), UINT32_C(0x4a7484aa), UINT32_C(0x5cb0a9dc), UINT32_C(0x76f988da),
        UINT32_C(0x983e5152), UINT32_C(0xa831c66d), UINT32_C(0xb00327c8), UINT32_C(0xbf597fc7),
        UINT32_C(0xc6e00bf3), UINT32_C(0xd5a79147), UINT32_C(0x06ca6351), UINT32_C(0x14292967),
        UINT32_C(0x27b70a85), UINT32_C(0x2e1b2138), UINT32_C(0x4d2c6dfc), UINT32_C(0x53380d13),
        UINT32_C(0x650a7354), UINT32_C(0x766a0abb), UINT32_C(0x81c2c92e), UINT32_C(0x92722c85),
        UINT32_C(0xa2bfe8a1), UINT32_C(0xa81a664b), UINT32_C(0xc24b8b70), UINT32_C(0xc76c51a3),
        UINT32_C(0xd192e819), UINT32_C(0xd6990624), UINT32_C(0xf40e3585), UINT32_C(0x106aa070),
        UINT32_C(0x19a4c116), UINT32_C(0x1e376c08), UINT32_C(0x2748774c), UINT32_C(0x34b0bcb5),
        UINT32_C(0x391c0cb3), UINT32_C(0x4ed8aa4a), UINT32_C(0x5b9cca4f), UINT32_C(0x682e6ff3),
        UINT32_C(0x748f82ee), UINT32_C(0x78a5636f), UINT32_C(0x84c87814), UINT32_C(0x8cc70208),
        UINT32_C(0x90befffa), UINT32_C(0xa4506ceb), UINT32_C(0xbef9a3f7), UINT32_C(0xc67178f2),
    };
    uint32_t h[8] = {
        UINT32_C(0x6a09e667), UINT32_C(0xbb67ae85), UINT32_C(0x3c6ef372), UINT32_C(0xa54ff53a),
        UINT32_C(0x510e527f), UINT32_C(0x9b05688c), UINT32_C(0x1f83d9ab), UINT32_C(0x5be0cd19),
    };
    size_t len = strlen(value.data);
    size_t padded_len = len + 1;
    while ((padded_len % 64) != 56) { padded_len += 1; }
    unsigned char *msg = (unsigned char *)calloc(padded_len + 8, 1);
    if (msg == NULL) { nomo_panic("out of memory"); }
    memcpy(msg, value.data, len);
    msg[len] = 0x80;
    uint64_t bit_len = (uint64_t)len * UINT64_C(8);
    for (size_t i = 0; i < 8; i += 1) {
        msg[padded_len + i] = (unsigned char)((bit_len >> (56 - 8 * i)) & 0xff);
    }
    for (size_t offset = 0; offset < padded_len + 8; offset += 64) {
        uint32_t w[64];
        for (size_t i = 0; i < 16; i += 1) {
            size_t j = offset + i * 4;
            w[i] = ((uint32_t)msg[j] << 24) | ((uint32_t)msg[j + 1] << 16) |
                ((uint32_t)msg[j + 2] << 8) | (uint32_t)msg[j + 3];
        }
        for (size_t i = 16; i < 64; i += 1) {
            uint32_t s0 = nomo_crypto_rotr32(w[i - 15], 7) ^ nomo_crypto_rotr32(w[i - 15], 18) ^ (w[i - 15] >> 3);
            uint32_t s1 = nomo_crypto_rotr32(w[i - 2], 17) ^ nomo_crypto_rotr32(w[i - 2], 19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16] + s0 + w[i - 7] + s1;
        }
        uint32_t a = h[0], b = h[1], c = h[2], d = h[3], e = h[4], f = h[5], g = h[6], hh = h[7];
        for (size_t i = 0; i < 64; i += 1) {
            uint32_t s1 = nomo_crypto_rotr32(e, 6) ^ nomo_crypto_rotr32(e, 11) ^ nomo_crypto_rotr32(e, 25);
            uint32_t ch = (e & f) ^ ((~e) & g);
            uint32_t temp1 = hh + s1 + ch + k[i] + w[i];
            uint32_t s0 = nomo_crypto_rotr32(a, 2) ^ nomo_crypto_rotr32(a, 13) ^ nomo_crypto_rotr32(a, 22);
            uint32_t maj = (a & b) ^ (a & c) ^ (b & c);
            uint32_t temp2 = s0 + maj;
            hh = g; g = f; f = e; e = d + temp1; d = c; c = b; b = a; a = temp1 + temp2;
        }
        h[0] += a; h[1] += b; h[2] += c; h[3] += d; h[4] += e; h[5] += f; h[6] += g; h[7] += hh;
    }
    free(msg);
    unsigned char digest[32];
    for (size_t i = 0; i < 8; i += 1) {
        digest[i * 4] = (unsigned char)((h[i] >> 24) & 0xff);
        digest[i * 4 + 1] = (unsigned char)((h[i] >> 16) & 0xff);
        digest[i * 4 + 2] = (unsigned char)((h[i] >> 8) & 0xff);
        digest[i * 4 + 3] = (unsigned char)(h[i] & 0xff);
    }
    return nomo_crypto_hex_string(digest, 32);
}

static nomo_string nomo_crypto_sha512(nomo_string value) {
    static const uint64_t k[80] = {
        UINT64_C(0x428a2f98d728ae22), UINT64_C(0x7137449123ef65cd), UINT64_C(0xb5c0fbcfec4d3b2f), UINT64_C(0xe9b5dba58189dbbc),
        UINT64_C(0x3956c25bf348b538), UINT64_C(0x59f111f1b605d019), UINT64_C(0x923f82a4af194f9b), UINT64_C(0xab1c5ed5da6d8118),
        UINT64_C(0xd807aa98a3030242), UINT64_C(0x12835b0145706fbe), UINT64_C(0x243185be4ee4b28c), UINT64_C(0x550c7dc3d5ffb4e2),
        UINT64_C(0x72be5d74f27b896f), UINT64_C(0x80deb1fe3b1696b1), UINT64_C(0x9bdc06a725c71235), UINT64_C(0xc19bf174cf692694),
        UINT64_C(0xe49b69c19ef14ad2), UINT64_C(0xefbe4786384f25e3), UINT64_C(0x0fc19dc68b8cd5b5), UINT64_C(0x240ca1cc77ac9c65),
        UINT64_C(0x2de92c6f592b0275), UINT64_C(0x4a7484aa6ea6e483), UINT64_C(0x5cb0a9dcbd41fbd4), UINT64_C(0x76f988da831153b5),
        UINT64_C(0x983e5152ee66dfab), UINT64_C(0xa831c66d2db43210), UINT64_C(0xb00327c898fb213f), UINT64_C(0xbf597fc7beef0ee4),
        UINT64_C(0xc6e00bf33da88fc2), UINT64_C(0xd5a79147930aa725), UINT64_C(0x06ca6351e003826f), UINT64_C(0x142929670a0e6e70),
        UINT64_C(0x27b70a8546d22ffc), UINT64_C(0x2e1b21385c26c926), UINT64_C(0x4d2c6dfc5ac42aed), UINT64_C(0x53380d139d95b3df),
        UINT64_C(0x650a73548baf63de), UINT64_C(0x766a0abb3c77b2a8), UINT64_C(0x81c2c92e47edaee6), UINT64_C(0x92722c851482353b),
        UINT64_C(0xa2bfe8a14cf10364), UINT64_C(0xa81a664bbc423001), UINT64_C(0xc24b8b70d0f89791), UINT64_C(0xc76c51a30654be30),
        UINT64_C(0xd192e819d6ef5218), UINT64_C(0xd69906245565a910), UINT64_C(0xf40e35855771202a), UINT64_C(0x106aa07032bbd1b8),
        UINT64_C(0x19a4c116b8d2d0c8), UINT64_C(0x1e376c085141ab53), UINT64_C(0x2748774cdf8eeb99), UINT64_C(0x34b0bcb5e19b48a8),
        UINT64_C(0x391c0cb3c5c95a63), UINT64_C(0x4ed8aa4ae3418acb), UINT64_C(0x5b9cca4f7763e373), UINT64_C(0x682e6ff3d6b2b8a3),
        UINT64_C(0x748f82ee5defb2fc), UINT64_C(0x78a5636f43172f60), UINT64_C(0x84c87814a1f0ab72), UINT64_C(0x8cc702081a6439ec),
        UINT64_C(0x90befffa23631e28), UINT64_C(0xa4506cebde82bde9), UINT64_C(0xbef9a3f7b2c67915), UINT64_C(0xc67178f2e372532b),
        UINT64_C(0xca273eceea26619c), UINT64_C(0xd186b8c721c0c207), UINT64_C(0xeada7dd6cde0eb1e), UINT64_C(0xf57d4f7fee6ed178),
        UINT64_C(0x06f067aa72176fba), UINT64_C(0x0a637dc5a2c898a6), UINT64_C(0x113f9804bef90dae), UINT64_C(0x1b710b35131c471b),
        UINT64_C(0x28db77f523047d84), UINT64_C(0x32caab7b40c72493), UINT64_C(0x3c9ebe0a15c9bebc), UINT64_C(0x431d67c49c100d4c),
        UINT64_C(0x4cc5d4becb3e42b6), UINT64_C(0x597f299cfc657e2a), UINT64_C(0x5fcb6fab3ad6faec), UINT64_C(0x6c44198c4a475817),
    };
    uint64_t h[8] = {
        UINT64_C(0x6a09e667f3bcc908), UINT64_C(0xbb67ae8584caa73b), UINT64_C(0x3c6ef372fe94f82b), UINT64_C(0xa54ff53a5f1d36f1),
        UINT64_C(0x510e527fade682d1), UINT64_C(0x9b05688c2b3e6c1f), UINT64_C(0x1f83d9abfb41bd6b), UINT64_C(0x5be0cd19137e2179),
    };
    size_t len = strlen(value.data);
    size_t padded_len = len + 1;
    while ((padded_len % 128) != 112) { padded_len += 1; }
    unsigned char *msg = (unsigned char *)calloc(padded_len + 16, 1);
    if (msg == NULL) { nomo_panic("out of memory"); }
    memcpy(msg, value.data, len);
    msg[len] = 0x80;
    uint64_t bit_low = (uint64_t)len * UINT64_C(8);
    uint64_t bit_high = (uint64_t)len >> 61;
    for (size_t i = 0; i < 8; i += 1) {
        msg[padded_len + i] = (unsigned char)((bit_high >> (56 - 8 * i)) & 0xff);
        msg[padded_len + 8 + i] = (unsigned char)((bit_low >> (56 - 8 * i)) & 0xff);
    }
    for (size_t offset = 0; offset < padded_len + 16; offset += 128) {
        uint64_t w[80];
        for (size_t i = 0; i < 16; i += 1) {
            size_t j = offset + i * 8;
            w[i] = ((uint64_t)msg[j] << 56) | ((uint64_t)msg[j + 1] << 48) |
                ((uint64_t)msg[j + 2] << 40) | ((uint64_t)msg[j + 3] << 32) |
                ((uint64_t)msg[j + 4] << 24) | ((uint64_t)msg[j + 5] << 16) |
                ((uint64_t)msg[j + 6] << 8) | (uint64_t)msg[j + 7];
        }
        for (size_t i = 16; i < 80; i += 1) {
            uint64_t s0 = nomo_crypto_rotr64(w[i - 15], 1) ^ nomo_crypto_rotr64(w[i - 15], 8) ^ (w[i - 15] >> 7);
            uint64_t s1 = nomo_crypto_rotr64(w[i - 2], 19) ^ nomo_crypto_rotr64(w[i - 2], 61) ^ (w[i - 2] >> 6);
            w[i] = w[i - 16] + s0 + w[i - 7] + s1;
        }
        uint64_t a = h[0], b = h[1], c = h[2], d = h[3], e = h[4], f = h[5], g = h[6], hh = h[7];
        for (size_t i = 0; i < 80; i += 1) {
            uint64_t s1 = nomo_crypto_rotr64(e, 14) ^ nomo_crypto_rotr64(e, 18) ^ nomo_crypto_rotr64(e, 41);
            uint64_t ch = (e & f) ^ ((~e) & g);
            uint64_t temp1 = hh + s1 + ch + k[i] + w[i];
            uint64_t s0 = nomo_crypto_rotr64(a, 28) ^ nomo_crypto_rotr64(a, 34) ^ nomo_crypto_rotr64(a, 39);
            uint64_t maj = (a & b) ^ (a & c) ^ (b & c);
            uint64_t temp2 = s0 + maj;
            hh = g; g = f; f = e; e = d + temp1; d = c; c = b; b = a; a = temp1 + temp2;
        }
        h[0] += a; h[1] += b; h[2] += c; h[3] += d; h[4] += e; h[5] += f; h[6] += g; h[7] += hh;
    }
    free(msg);
    unsigned char digest[64];
    for (size_t i = 0; i < 8; i += 1) {
        digest[i * 8] = (unsigned char)((h[i] >> 56) & 0xff);
        digest[i * 8 + 1] = (unsigned char)((h[i] >> 48) & 0xff);
        digest[i * 8 + 2] = (unsigned char)((h[i] >> 40) & 0xff);
        digest[i * 8 + 3] = (unsigned char)((h[i] >> 32) & 0xff);
        digest[i * 8 + 4] = (unsigned char)((h[i] >> 24) & 0xff);
        digest[i * 8 + 5] = (unsigned char)((h[i] >> 16) & 0xff);
        digest[i * 8 + 6] = (unsigned char)((h[i] >> 8) & 0xff);
        digest[i * 8 + 7] = (unsigned char)(h[i] & 0xff);
    }
    return nomo_crypto_hex_string(digest, 64);
}

static nomo_array_u32 nomo_crypto_random_bytes(uint64_t count) {
    if (count > (uint64_t)SIZE_MAX) { nomo_panic("crypto.random_bytes count is too large"); }
    nomo_array_u32 out = nomo_array_u32_new();
#if defined(_WIN32)
    for (uint64_t i = 0; i < count; i += 1) {
        unsigned int value = 0;
        if (rand_s(&value) != 0) { nomo_panic("crypto.random_bytes failed"); }
        out = nomo_array_u32_push(out, (uint32_t)(value & 0xffU));
    }
#else
    FILE *file = fopen("/dev/urandom", "rb");
    if (file == NULL) { nomo_panic("crypto.random_bytes failed"); }
    for (uint64_t i = 0; i < count; i += 1) {
        unsigned char value = 0;
        if (fread(&value, 1, 1, file) != 1) {
            fclose(file);
            nomo_panic("crypto.random_bytes failed");
        }
        out = nomo_array_u32_push(out, (uint32_t)value);
    }
    fclose(file);
#endif
    return out;
}
"#,
    );
}
