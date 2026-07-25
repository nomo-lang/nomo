#define NOMO_CRON_MAX_EXPRESSION_BYTES 256U
#define NOMO_CRON_SEARCH_MINUTES 4208400ULL
#define NOMO_CRON_MAX_TIMESTAMP_MILLIS INT64_C(253402300799999)

typedef struct {
    uint64_t minute;
    uint64_t hour;
    uint64_t day_of_month;
    uint64_t month;
    uint64_t day_of_week;
} nomo_cron_parsed;

static const char *nomo_cron_error_message(const char *code) {
    if (strcmp(code, "range") == 0) {
        return "cron field value is outside its range";
    }
    if (strcmp(code, "limit") == 0) {
        return "cron limit exceeded";
    }
    if (strcmp(code, "timestamp_range") == 0) {
        return "cron timestamp is outside the supported range";
    }
    if (strcmp(code, "no_match") == 0) {
        return "cron schedule has no later matching minute";
    }
    return "invalid cron expression syntax";
}

static @CRON_ERROR@ nomo_cron_error(
    const char *code,
    uint64_t field
) {
    return (@CRON_ERROR@){
        .@CODE_MEMBER@ = nomo_string_from_cstr(code),
        .@MESSAGE_MEMBER@ = nomo_string_from_cstr(
            nomo_cron_error_message(code)
        ),
        .@FIELD_MEMBER@ = field
    };
}

static int nomo_cron_ascii_space(unsigned char value) {
    return value == ' '
        || value == '\t'
        || value == '\n'
        || value == '\r'
        || value == '\v'
        || value == '\f';
}

static int nomo_cron_parse_uint(
    const char *text,
    size_t len,
    uint64_t *value,
    const char **failure
) {
    if (len == 0U) {
        *failure = "syntax";
        return 0;
    }
    uint64_t parsed = 0U;
    for (size_t i = 0U; i < len; i += 1U) {
        unsigned char byte = (unsigned char)text[i];
        if (byte < '0' || byte > '9') {
            *failure = "syntax";
            return 0;
        }
        uint64_t digit = (uint64_t)(byte - '0');
        if (parsed > (UINT64_MAX - digit) / 10U) {
            *failure = "range";
            return 0;
        }
        parsed = parsed * 10U + digit;
    }
    *value = parsed;
    return 1;
}

static uint64_t nomo_cron_full_mask(uint64_t minimum, uint64_t maximum) {
    uint64_t mask = 0U;
    for (uint64_t value = minimum; value <= maximum; value += 1U) {
        mask |= UINT64_C(1) << value;
    }
    return mask;
}

static int nomo_cron_parse_member(
    const char *text,
    size_t len,
    uint64_t minimum,
    uint64_t maximum,
    uint64_t *mask,
    const char **failure
) {
    size_t slash = SIZE_MAX;
    for (size_t i = 0U; i < len; i += 1U) {
        if (text[i] == '/') {
            if (slash != SIZE_MAX) {
                *failure = "syntax";
                return 0;
            }
            slash = i;
        }
    }

    size_t base_len = slash == SIZE_MAX ? len : slash;
    uint64_t step = 1U;
    if (slash != SIZE_MAX) {
        if (
            !nomo_cron_parse_uint(
                text + slash + 1U,
                len - slash - 1U,
                &step,
                failure
            )
        ) {
            return 0;
        }
        if (step == 0U || step > maximum - minimum + 1U) {
            *failure = "range";
            return 0;
        }
    }

    uint64_t start = 0U;
    uint64_t end = 0U;
    if (base_len == 1U && text[0] == '*') {
        start = minimum;
        end = maximum;
    } else {
        size_t dash = SIZE_MAX;
        for (size_t i = 0U; i < base_len; i += 1U) {
            if (text[i] == '-') {
                if (dash != SIZE_MAX) {
                    *failure = "syntax";
                    return 0;
                }
                dash = i;
            }
        }
        if (dash == SIZE_MAX) {
            if (slash != SIZE_MAX) {
                *failure = "syntax";
                return 0;
            }
            if (!nomo_cron_parse_uint(text, base_len, &start, failure)) {
                return 0;
            }
            end = start;
        } else {
            if (
                !nomo_cron_parse_uint(text, dash, &start, failure)
                || !nomo_cron_parse_uint(
                    text + dash + 1U,
                    base_len - dash - 1U,
                    &end,
                    failure
                )
            ) {
                return 0;
            }
        }
    }

    if (
        start < minimum
        || start > maximum
        || end < minimum
        || end > maximum
        || start > end
    ) {
        *failure = "range";
        return 0;
    }
    for (uint64_t value = start; value <= end;) {
        *mask |= UINT64_C(1) << value;
        if (end - value < step) {
            break;
        }
        value += step;
    }
    return 1;
}

static int nomo_cron_parse_field(
    const char *text,
    size_t len,
    uint64_t minimum,
    uint64_t maximum,
    uint64_t *mask,
    const char **failure
) {
    *mask = 0U;
    size_t start = 0U;
    while (start <= len) {
        size_t end = start;
        while (end < len && text[end] != ',') {
            end += 1U;
        }
        if (
            end == start
            || !nomo_cron_parse_member(
                text + start,
                end - start,
                minimum,
                maximum,
                mask,
                failure
            )
        ) {
            if (end == start) {
                *failure = "syntax";
            }
            return 0;
        }
        if (end == len) {
            break;
        }
        start = end + 1U;
    }
    return *mask != 0U;
}

static int nomo_cron_parse_expression(
    const char *expression,
    nomo_cron_parsed *parsed,
    const char **failure,
    uint64_t *failure_field
) {
    size_t len = strlen(expression);
    if (len > NOMO_CRON_MAX_EXPRESSION_BYTES) {
        *failure = "limit";
        *failure_field = 5U;
        return 0;
    }
    for (size_t i = 0U; i < len; i += 1U) {
        if ((unsigned char)expression[i] > 127U) {
            *failure = "syntax";
            *failure_field = 5U;
            return 0;
        }
    }
    size_t scan_cursor = 0U;
    uint64_t token_count = 0U;
    while (1) {
        while (
            scan_cursor < len
            && nomo_cron_ascii_space((unsigned char)expression[scan_cursor])
        ) {
            scan_cursor += 1U;
        }
        if (scan_cursor == len) {
            break;
        }
        token_count += 1U;
        while (
            scan_cursor < len
            && !nomo_cron_ascii_space((unsigned char)expression[scan_cursor])
        ) {
            scan_cursor += 1U;
        }
    }
    if (token_count != 5U) {
        *failure = "syntax";
        *failure_field = 5U;
        return 0;
    }

    static const uint64_t minimums[5] = {0U, 0U, 1U, 1U, 0U};
    static const uint64_t maximums[5] = {59U, 23U, 31U, 12U, 6U};
    uint64_t *masks[5] = {
        &parsed->minute,
        &parsed->hour,
        &parsed->day_of_month,
        &parsed->month,
        &parsed->day_of_week
    };

    size_t cursor = 0U;
    uint64_t field = 0U;
    while (1) {
        while (
            cursor < len
            && nomo_cron_ascii_space((unsigned char)expression[cursor])
        ) {
            cursor += 1U;
        }
        if (cursor == len) {
            break;
        }
        if (field == 5U) {
            *failure = "syntax";
            *failure_field = 5U;
            return 0;
        }
        size_t end = cursor;
        while (
            end < len
            && !nomo_cron_ascii_space((unsigned char)expression[end])
        ) {
            end += 1U;
        }
        if (
            !nomo_cron_parse_field(
                expression + cursor,
                end - cursor,
                minimums[field],
                maximums[field],
                masks[field],
                failure
            )
        ) {
            *failure_field = field;
            return 0;
        }
        field += 1U;
        cursor = end;
    }
    if (field != 5U) {
        *failure = "syntax";
        *failure_field = 5U;
        return 0;
    }
    return 1;
}

static void nomo_cron_civil_from_days(
    int64_t days,
    int64_t *year,
    uint64_t *month,
    uint64_t *day
) {
    int64_t adjusted = days + INT64_C(719468);
    int64_t era = adjusted / INT64_C(146097);
    uint64_t day_of_era = (uint64_t)(
        adjusted - era * INT64_C(146097)
    );
    uint64_t year_of_era = (
        day_of_era
        - day_of_era / 1460U
        + day_of_era / 36524U
        - day_of_era / 146096U
    ) / 365U;
    int64_t selected_year = (int64_t)year_of_era + era * 400;
    uint64_t day_of_year = day_of_era - (
        365U * year_of_era
        + year_of_era / 4U
        - year_of_era / 100U
    );
    uint64_t month_prime = (5U * day_of_year + 2U) / 153U;
    uint64_t selected_day = day_of_year
        - (153U * month_prime + 2U) / 5U
        + 1U;
    uint64_t selected_month = month_prime < 10U
        ? month_prime + 3U
        : month_prime - 9U;
    selected_year += selected_month <= 2U ? 1 : 0;
    *year = selected_year;
    *month = selected_month;
    *day = selected_day;
}

static int nomo_cron_mask_contains(uint64_t mask, uint64_t value) {
    return (mask & (UINT64_C(1) << value)) != 0U;
}

static int nomo_cron_minute_matches(
    const nomo_cron_parsed *parsed,
    int64_t minute_index
) {
    int64_t days = minute_index / 1440;
    uint64_t minute = (uint64_t)(minute_index % 60);
    uint64_t hour = (uint64_t)((minute_index / 60) % 24);
    int64_t year = 0;
    uint64_t month = 0U;
    uint64_t day = 0U;
    nomo_cron_civil_from_days(days, &year, &month, &day);
    (void)year;
    uint64_t day_of_week = (uint64_t)((days + 4) % 7);

    if (
        !nomo_cron_mask_contains(parsed->minute, minute)
        || !nomo_cron_mask_contains(parsed->hour, hour)
        || !nomo_cron_mask_contains(parsed->month, month)
    ) {
        return 0;
    }
    int day_of_month_matches = nomo_cron_mask_contains(
        parsed->day_of_month,
        day
    );
    int day_of_week_matches = nomo_cron_mask_contains(
        parsed->day_of_week,
        day_of_week
    );
    int day_of_month_unrestricted = parsed->day_of_month
        == nomo_cron_full_mask(1U, 31U);
    int day_of_week_unrestricted = parsed->day_of_week
        == nomo_cron_full_mask(0U, 6U);
    if (day_of_month_unrestricted && day_of_week_unrestricted) {
        return 1;
    }
    if (day_of_month_unrestricted) {
        return day_of_week_matches;
    }
    if (day_of_week_unrestricted) {
        return day_of_month_matches;
    }
    return day_of_month_matches || day_of_week_matches;
}

static int nomo_cron_timestamp_valid(int64_t unix_millis) {
    return unix_millis >= 0
        && unix_millis <= NOMO_CRON_MAX_TIMESTAMP_MILLIS;
}

static @RESULT_SCHEDULE@ nomo_cron_parse(nomo_string expression) {
    nomo_cron_parsed parsed;
    const char *failure = NULL;
    uint64_t field = 5U;
    if (
        !nomo_cron_parse_expression(
            expression.data,
            &parsed,
            &failure,
            &field
        )
    ) {
        return (@RESULT_SCHEDULE@){
            .tag = @RESULT_SCHEDULE_ERR@,
            .payload.@ERR_PAYLOAD@ = nomo_cron_error(failure, field)
        };
    }
    return (@RESULT_SCHEDULE@){
        .tag = @RESULT_SCHEDULE_OK@,
        .payload.@OK_PAYLOAD@ = (@SCHEDULE@){
            .@EXPRESSION_MEMBER@ = nomo_string_retain(expression)
        }
    };
}

static @RESULT_BOOL@ nomo_cron_matches(
    @SCHEDULE@ schedule,
    int64_t unix_millis
) {
    if (!nomo_cron_timestamp_valid(unix_millis)) {
        return (@RESULT_BOOL@){
            .tag = @RESULT_BOOL_ERR@,
            .payload.@ERR_PAYLOAD@ = nomo_cron_error(
                "timestamp_range",
                5U
            )
        };
    }
    nomo_cron_parsed parsed;
    const char *failure = NULL;
    uint64_t field = 5U;
    if (
        !nomo_cron_parse_expression(
            schedule.@EXPRESSION_MEMBER@.data,
            &parsed,
            &failure,
            &field
        )
    ) {
        return (@RESULT_BOOL@){
            .tag = @RESULT_BOOL_ERR@,
            .payload.@ERR_PAYLOAD@ = nomo_cron_error(failure, field)
        };
    }
    return (@RESULT_BOOL@){
        .tag = @RESULT_BOOL_OK@,
        .payload.@OK_PAYLOAD@ = nomo_cron_minute_matches(
            &parsed,
            unix_millis / INT64_C(60000)
        )
    };
}

static @RESULT_INT@ nomo_cron_next_after(
    @SCHEDULE@ schedule,
    int64_t unix_millis
) {
    if (!nomo_cron_timestamp_valid(unix_millis)) {
        return (@RESULT_INT@){
            .tag = @RESULT_INT_ERR@,
            .payload.@ERR_PAYLOAD@ = nomo_cron_error(
                "timestamp_range",
                5U
            )
        };
    }
    nomo_cron_parsed parsed;
    const char *failure = NULL;
    uint64_t field = 5U;
    if (
        !nomo_cron_parse_expression(
            schedule.@EXPRESSION_MEMBER@.data,
            &parsed,
            &failure,
            &field
        )
    ) {
        return (@RESULT_INT@){
            .tag = @RESULT_INT_ERR@,
            .payload.@ERR_PAYLOAD@ = nomo_cron_error(failure, field)
        };
    }

    int64_t candidate = unix_millis / INT64_C(60000) + 1;
    int64_t maximum_minute = NOMO_CRON_MAX_TIMESTAMP_MILLIS
        / INT64_C(60000);
    for (
        uint64_t checked = 0U;
        checked < NOMO_CRON_SEARCH_MINUTES && candidate <= maximum_minute;
        checked += 1U, candidate += 1
    ) {
        if (nomo_cron_minute_matches(&parsed, candidate)) {
            return (@RESULT_INT@){
                .tag = @RESULT_INT_OK@,
                .payload.@OK_PAYLOAD@ = candidate * INT64_C(60000)
            };
        }
    }
    return (@RESULT_INT@){
        .tag = @RESULT_INT_ERR@,
        .payload.@ERR_PAYLOAD@ = nomo_cron_error("no_match", 5U)
    };
}
