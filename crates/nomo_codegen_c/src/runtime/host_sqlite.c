#include "sqlite3.h"

#define NOMO_SQLITE_MAX_DATABASES 32
#define NOMO_SQLITE_MAX_QUERIES 256
#define NOMO_SQLITE_MAX_PATH_BYTES 4096
#define NOMO_SQLITE_MAX_SQL_BYTES 1048576
#define NOMO_SQLITE_MAX_PARAMETERS 1024
#define NOMO_SQLITE_MAX_VALUE_BYTES 8388608
#define NOMO_SQLITE_MAX_PARAMETER_BYTES 16777216
#define NOMO_SQLITE_MAX_ROW_BYTES 16777216
#define NOMO_SQLITE_MAX_BUSY_TIMEOUT 300000

typedef struct nomo_sqlite_database_state {
    uint64_t handle;
    sqlite3 *database;
    size_t live_queries;
    struct nomo_sqlite_database_state *next;
} nomo_sqlite_database_state;

typedef struct nomo_sqlite_query_state {
    uint64_t handle;
    nomo_sqlite_database_state *owner;
    sqlite3_stmt *statement;
    int done;
    int failed;
    struct nomo_sqlite_query_state *next;
} nomo_sqlite_query_state;

typedef struct nomo_sqlite_prepared {
    sqlite3_stmt *statement;
    int ok;
    @ERROR@ error;
} nomo_sqlite_prepared;

typedef struct nomo_sqlite_bound {
    int ok;
    @ERROR@ error;
} nomo_sqlite_bound;

static nomo_sqlite_database_state *nomo_sqlite_databases = NULL;
static nomo_sqlite_query_state *nomo_sqlite_queries = NULL;
static uint64_t nomo_sqlite_next_database_handle = 1;
static uint64_t nomo_sqlite_next_query_handle = 1;
static size_t nomo_sqlite_database_count = 0;
static size_t nomo_sqlite_query_count = 0;

static @ERROR@ nomo_sqlite_error_value(
    const char *code,
    const char *message,
    int native_code
) {
    return (@ERROR@){
        .@CODE_MEMBER@ = nomo_string_from_cstr(code),
        .@MESSAGE_MEMBER@ = nomo_string_from_cstr(message),
        .@NATIVE_CODE_MEMBER@ = (int64_t)native_code
    };
}

static const char *nomo_sqlite_classified_code(int result, const char *fallback) {
    switch (result & 0xff) {
        case SQLITE_BUSY:
        case SQLITE_LOCKED:
            return "busy";
        case SQLITE_CONSTRAINT:
            return "constraint";
        case SQLITE_READONLY:
            return "read_only";
        case SQLITE_CORRUPT:
        case SQLITE_NOTADB:
            return "corrupt";
        case SQLITE_FULL:
            return "full";
        case SQLITE_TOOBIG:
            return "limit";
        default:
            return fallback;
    }
}

static const char *nomo_sqlite_error_message(const char *code) {
    if (strcmp(code, "invalid_request") == 0) {
        return "invalid SQLite request";
    }
    if (strcmp(code, "limit") == 0) {
        return "SQLite resource limit exceeded";
    }
    if (strcmp(code, "open") == 0) {
        return "database open failed";
    }
    if (strcmp(code, "prepare") == 0) {
        return "statement preparation failed";
    }
    if (strcmp(code, "bind") == 0) {
        return "parameter binding failed";
    }
    if (strcmp(code, "step") == 0) {
        return "statement execution failed";
    }
    if (strcmp(code, "busy") == 0) {
        return "database is busy";
    }
    if (strcmp(code, "constraint") == 0) {
        return "database constraint rejected the operation";
    }
    if (strcmp(code, "read_only") == 0) {
        return "database is read-only";
    }
    if (strcmp(code, "corrupt") == 0) {
        return "database content is corrupt";
    }
    if (strcmp(code, "full") == 0) {
        return "database storage is full";
    }
    if (strcmp(code, "encoding") == 0) {
        return "database text is not valid UTF-8";
    }
    if (strcmp(code, "unexpected_row") == 0) {
        return "execute cannot return rows";
    }
    if (strcmp(code, "busy_handle") == 0) {
        return "database has live queries";
    }
    if (strcmp(code, "closed") == 0) {
        return "SQLite handle is closed";
    }
    if (strcmp(code, "runtime_unavailable") == 0) {
        return "SQLite runtime is unavailable";
    }
    return "internal SQLite runtime failure";
}

static @ERROR@ nomo_sqlite_native_error(int result, const char *fallback) {
    const char *code = nomo_sqlite_classified_code(result, fallback);
    return nomo_sqlite_error_value(
        code,
        nomo_sqlite_error_message(code),
        result
    );
}

static @RESULT_DATABASE@ nomo_sqlite_database_error(@ERROR@ error) {
    return (@RESULT_DATABASE@){
        .tag = @RESULT_DATABASE_ERR@,
        .payload.@ERR_PAYLOAD@ = error
    };
}

static @RESULT_QUERY@ nomo_sqlite_query_error(@ERROR@ error) {
    return (@RESULT_QUERY@){
        .tag = @RESULT_QUERY_ERR@,
        .payload.@ERR_PAYLOAD@ = error
    };
}

static @RESULT_EXECUTE@ nomo_sqlite_execute_error(@ERROR@ error) {
    return (@RESULT_EXECUTE@){
        .tag = @RESULT_EXECUTE_ERR@,
        .payload.@ERR_PAYLOAD@ = error
    };
}

static @RESULT_ROW@ nomo_sqlite_row_error(@ERROR@ error) {
    return (@RESULT_ROW@){
        .tag = @RESULT_ROW_ERR@,
        .payload.@ERR_PAYLOAD@ = error
    };
}

static @RESULT_VOID@ nomo_sqlite_void_error(@ERROR@ error) {
    return (@RESULT_VOID@){
        .tag = @RESULT_VOID_ERR@,
        .payload.@ERR_PAYLOAD@ = error
    };
}

static @ERROR@ nomo_sqlite_validation_error(const char *code) {
    return nomo_sqlite_error_value(code, nomo_sqlite_error_message(code), 0);
}

static int nomo_sqlite_has_embedded_nul(nomo_string value) {
    (void)value;
    /* Nomo's v0.1 string representation is NUL-terminated and NUL-free. */
    return 0;
}

static int nomo_sqlite_valid_utf8(const unsigned char *data, size_t length) {
    size_t index = 0;
    while (index < length) {
        unsigned char first = data[index++];
        if (first <= 0x7f) {
            continue;
        }
        size_t needed = 0;
        uint32_t scalar = 0;
        if ((first & 0xe0) == 0xc0) {
            needed = 1;
            scalar = (uint32_t)(first & 0x1f);
            if (scalar < 2) {
                return 0;
            }
        } else if ((first & 0xf0) == 0xe0) {
            needed = 2;
            scalar = (uint32_t)(first & 0x0f);
        } else if ((first & 0xf8) == 0xf0) {
            needed = 3;
            scalar = (uint32_t)(first & 0x07);
            if (scalar > 4) {
                return 0;
            }
        } else {
            return 0;
        }
        if (needed > length - index) {
            return 0;
        }
        for (size_t offset = 0; offset < needed; offset += 1) {
            unsigned char next = data[index++];
            if ((next & 0xc0) != 0x80) {
                return 0;
            }
            scalar = (scalar << 6) | (uint32_t)(next & 0x3f);
        }
        if ((needed == 2 && scalar < 0x800)
            || (needed == 3 && scalar < 0x10000)
            || scalar > 0x10ffff
            || (scalar >= 0xd800 && scalar <= 0xdfff)) {
            return 0;
        }
    }
    return 1;
}

static nomo_sqlite_database_state *nomo_sqlite_find_database(uint64_t handle) {
    nomo_sqlite_database_state *current = nomo_sqlite_databases;
    while (current != NULL) {
        if (current->handle == handle) {
            return current;
        }
        current = current->next;
    }
    return NULL;
}

static nomo_sqlite_query_state *nomo_sqlite_find_query(uint64_t handle) {
    nomo_sqlite_query_state *current = nomo_sqlite_queries;
    while (current != NULL) {
        if (current->handle == handle) {
            return current;
        }
        current = current->next;
    }
    return NULL;
}

static int nomo_sqlite_tail_is_trivia(const char *cursor, const char *end) {
    while (cursor < end) {
        if (isspace((unsigned char)*cursor)) {
            cursor += 1;
            continue;
        }
        if (end - cursor >= 2 && cursor[0] == '-' && cursor[1] == '-') {
            cursor += 2;
            while (cursor < end && *cursor != '\n') {
                cursor += 1;
            }
            continue;
        }
        if (end - cursor >= 2 && cursor[0] == '/' && cursor[1] == '*') {
            cursor += 2;
            while (end - cursor >= 2 && !(cursor[0] == '*' && cursor[1] == '/')) {
                cursor += 1;
            }
            if (end - cursor >= 2) {
                cursor += 2;
            }
            continue;
        }
        return 0;
    }
    return 1;
}

static nomo_sqlite_prepared nomo_sqlite_prepare_one(
    nomo_sqlite_database_state *owner,
    nomo_string sql
) {
    nomo_sqlite_prepared prepared = {0};
    size_t sql_length = strlen(sql.data);
    if (sql_length == 0
        || sql_length > NOMO_SQLITE_MAX_SQL_BYTES
        || nomo_sqlite_has_embedded_nul(sql)) {
        prepared.error = nomo_sqlite_validation_error("invalid_request");
        return prepared;
    }
    const char *tail = NULL;
    int result = sqlite3_prepare_v3(
        owner->database,
        sql.data,
        (int)sql_length,
        SQLITE_PREPARE_PERSISTENT,
        &prepared.statement,
        &tail
    );
    if (result != SQLITE_OK) {
        prepared.error = nomo_sqlite_native_error(result, "prepare");
        if (prepared.statement != NULL) {
            sqlite3_finalize(prepared.statement);
            prepared.statement = NULL;
        }
        return prepared;
    }
    if (prepared.statement == NULL
        || tail == NULL
        || !nomo_sqlite_tail_is_trivia(tail, sql.data + sql_length)) {
        if (prepared.statement != NULL) {
            sqlite3_finalize(prepared.statement);
            prepared.statement = NULL;
        }
        prepared.error = nomo_sqlite_validation_error("invalid_request");
        return prepared;
    }
    int parameter_count = sqlite3_bind_parameter_count(prepared.statement);
    if (parameter_count < 0 || parameter_count > NOMO_SQLITE_MAX_PARAMETERS) {
        sqlite3_finalize(prepared.statement);
        prepared.statement = NULL;
        prepared.error = nomo_sqlite_validation_error("limit");
        return prepared;
    }
    prepared.ok = 1;
    return prepared;
}

static nomo_sqlite_bound nomo_sqlite_bind_values(
    sqlite3_stmt *statement,
    @VALUE_ARRAY@ parameters
) {
    nomo_sqlite_bound bound = {0};
    int expected = sqlite3_bind_parameter_count(statement);
    if (parameters.len > NOMO_SQLITE_MAX_PARAMETERS
        || parameters.len != (size_t)expected) {
        bound.error = nomo_sqlite_validation_error("invalid_request");
        return bound;
    }

    size_t total_bytes = 0;
    for (size_t index = 0; index < parameters.len; index += 1) {
        @VALUE@ value = parameters.data[index];
        if (value.tag == @VALUE_TEXT@) {
            nomo_string text = value.payload.@TEXT_PAYLOAD@;
            size_t text_length = strlen(text.data);
            if (text_length > NOMO_SQLITE_MAX_VALUE_BYTES
                || text_length > NOMO_SQLITE_MAX_PARAMETER_BYTES - total_bytes) {
                bound.error = nomo_sqlite_validation_error("limit");
                return bound;
            }
            total_bytes += text_length;
        } else if (value.tag == @VALUE_BLOB@) {
            @BYTE_ARRAY@ blob = value.payload.@BLOB_PAYLOAD@;
            if (blob.len > NOMO_SQLITE_MAX_VALUE_BYTES
                || blob.len > NOMO_SQLITE_MAX_PARAMETER_BYTES - total_bytes) {
                bound.error = nomo_sqlite_validation_error("limit");
                return bound;
            }
            for (size_t byte_index = 0; byte_index < blob.len; byte_index += 1) {
                if (blob.data[byte_index] > 255) {
                    bound.error = nomo_sqlite_validation_error("invalid_request");
                    return bound;
                }
            }
            total_bytes += blob.len;
        } else if (value.tag != @VALUE_NULL@
            && value.tag != @VALUE_INTEGER@
            && value.tag != @VALUE_REAL@) {
            bound.error = nomo_sqlite_validation_error("invalid_request");
            return bound;
        }
    }

    for (size_t index = 0; index < parameters.len; index += 1) {
        @VALUE@ value = parameters.data[index];
        int parameter = (int)index + 1;
        int result = SQLITE_MISUSE;
        if (value.tag == @VALUE_NULL@) {
            result = sqlite3_bind_null(statement, parameter);
        } else if (value.tag == @VALUE_INTEGER@) {
            result = sqlite3_bind_int64(
                statement,
                parameter,
                (sqlite3_int64)value.payload.@INTEGER_PAYLOAD@
            );
        } else if (value.tag == @VALUE_REAL@) {
            result = sqlite3_bind_double(
                statement,
                parameter,
                value.payload.@REAL_PAYLOAD@
            );
        } else if (value.tag == @VALUE_TEXT@) {
            nomo_string text = value.payload.@TEXT_PAYLOAD@;
            size_t text_length = strlen(text.data);
            result = sqlite3_bind_text64(
                statement,
                parameter,
                text.data,
                (sqlite3_uint64)text_length,
                SQLITE_TRANSIENT,
                SQLITE_UTF8
            );
        } else if (value.tag == @VALUE_BLOB@) {
            @BYTE_ARRAY@ blob = value.payload.@BLOB_PAYLOAD@;
            unsigned char *copy = NULL;
            if (blob.len > 0) {
                copy = (unsigned char *)malloc(blob.len);
                if (copy == NULL) {
                    bound.error = nomo_sqlite_validation_error("internal");
                    return bound;
                }
                for (size_t byte_index = 0; byte_index < blob.len; byte_index += 1) {
                    copy[byte_index] = (unsigned char)blob.data[byte_index];
                }
            }
            result = sqlite3_bind_blob64(
                statement,
                parameter,
                blob.len == 0 ? (const void *)"" : (const void *)copy,
                (sqlite3_uint64)blob.len,
                SQLITE_TRANSIENT
            );
            free(copy);
        }
        if (result != SQLITE_OK) {
            bound.error = nomo_sqlite_native_error(result, "bind");
            return bound;
        }
    }
    bound.ok = 1;
    return bound;
}

static int nomo_sqlite_initialize_database(
    sqlite3 *database,
    uint64_t busy_timeout_millis
) {
    if (sqlite3_extended_result_codes(database, 1) != SQLITE_OK) {
        return SQLITE_ERROR;
    }
    if (sqlite3_busy_timeout(database, (int)busy_timeout_millis) != SQLITE_OK) {
        return SQLITE_ERROR;
    }
    sqlite3_limit(database, SQLITE_LIMIT_LENGTH, NOMO_SQLITE_MAX_PARAMETER_BYTES);
    sqlite3_limit(database, SQLITE_LIMIT_SQL_LENGTH, NOMO_SQLITE_MAX_SQL_BYTES);
    sqlite3_limit(database, SQLITE_LIMIT_COLUMN, 256);
    sqlite3_limit(database, SQLITE_LIMIT_EXPR_DEPTH, 100);
    sqlite3_limit(database, SQLITE_LIMIT_VARIABLE_NUMBER, NOMO_SQLITE_MAX_PARAMETERS);
    sqlite3_limit(database, SQLITE_LIMIT_FUNCTION_ARG, 100);
    sqlite3_limit(database, SQLITE_LIMIT_COMPOUND_SELECT, 64);
    sqlite3_limit(database, SQLITE_LIMIT_LIKE_PATTERN_LENGTH, 1024);

    int previous = 0;
    int result = sqlite3_db_config(
        database,
        SQLITE_DBCONFIG_DEFENSIVE,
        1,
        &previous
    );
    if (result != SQLITE_OK) {
        return result;
    }
    result = sqlite3_db_config(
        database,
        SQLITE_DBCONFIG_TRUSTED_SCHEMA,
        0,
        &previous
    );
    if (result != SQLITE_OK) {
        return result;
    }
    return sqlite3_exec(database, "PRAGMA foreign_keys = ON", NULL, NULL, NULL);
}

static @RESULT_DATABASE@ nomo_sqlite_open_internal(
    const char *path,
    int flags,
    uint64_t busy_timeout_millis
) {
    if (busy_timeout_millis > NOMO_SQLITE_MAX_BUSY_TIMEOUT) {
        return nomo_sqlite_database_error(
            nomo_sqlite_validation_error("invalid_request")
        );
    }
    if (nomo_sqlite_database_count >= NOMO_SQLITE_MAX_DATABASES
        || nomo_sqlite_next_database_handle == 0) {
        return nomo_sqlite_database_error(
            nomo_sqlite_validation_error("limit")
        );
    }

    sqlite3 *database = NULL;
    int result = sqlite3_open_v2(
        path,
        &database,
        flags | SQLITE_OPEN_FULLMUTEX | SQLITE_OPEN_PRIVATECACHE,
        NULL
    );
    if (result != SQLITE_OK) {
        if (database != NULL) {
            sqlite3_close_v2(database);
        }
        return nomo_sqlite_database_error(
            nomo_sqlite_native_error(result, "open")
        );
    }
    result = nomo_sqlite_initialize_database(database, busy_timeout_millis);
    if (result != SQLITE_OK) {
        sqlite3_close_v2(database);
        return nomo_sqlite_database_error(
            nomo_sqlite_native_error(result, "open")
        );
    }

    nomo_sqlite_database_state *state =
        (nomo_sqlite_database_state *)calloc(1, sizeof(nomo_sqlite_database_state));
    if (state == NULL) {
        sqlite3_close_v2(database);
        return nomo_sqlite_database_error(
            nomo_sqlite_validation_error("internal")
        );
    }
    state->handle = nomo_sqlite_next_database_handle++;
    state->database = database;
    state->next = nomo_sqlite_databases;
    nomo_sqlite_databases = state;
    nomo_sqlite_database_count += 1;

    return (@RESULT_DATABASE@){
        .tag = @RESULT_DATABASE_OK@,
        .payload.@OK_PAYLOAD@ = (@DATABASE@){
            .@HANDLE_MEMBER@ = state->handle
        }
    };
}

static @RESULT_DATABASE@ @OPEN_NAME@(
    nomo_string path,
    @OPEN_MODE@ mode,
    uint64_t busy_timeout_millis
) {
    size_t path_length = strlen(path.data);
    if (path_length == 0
        || path_length > NOMO_SQLITE_MAX_PATH_BYTES
        || nomo_sqlite_has_embedded_nul(path)
        || (path_length == 8 && memcmp(path.data, ":memory:", 8) == 0)) {
        return nomo_sqlite_database_error(
            nomo_sqlite_validation_error("invalid_request")
        );
    }

    int flags = 0;
    if (mode.tag == @OPEN_READ_ONLY@) {
        flags = SQLITE_OPEN_READONLY;
    } else if (mode.tag == @OPEN_READ_WRITE@) {
        flags = SQLITE_OPEN_READWRITE;
    } else if (mode.tag == @OPEN_READ_WRITE_CREATE@) {
        flags = SQLITE_OPEN_READWRITE | SQLITE_OPEN_CREATE;
    } else {
        return nomo_sqlite_database_error(
            nomo_sqlite_validation_error("invalid_request")
        );
    }
    return nomo_sqlite_open_internal(path.data, flags, busy_timeout_millis);
}

static @RESULT_DATABASE@ @OPEN_MEMORY_NAME@(uint64_t busy_timeout_millis) {
    return nomo_sqlite_open_internal(
        ":memory:",
        SQLITE_OPEN_READWRITE | SQLITE_OPEN_CREATE | SQLITE_OPEN_MEMORY,
        busy_timeout_millis
    );
}

static @RESULT_EXECUTE@ @EXECUTE_NAME@(
    @DATABASE@ database_value,
    nomo_string sql,
    @VALUE_ARRAY@ parameters
) {
    nomo_sqlite_database_state *owner =
        nomo_sqlite_find_database(database_value.@HANDLE_MEMBER@);
    if (owner == NULL) {
        return nomo_sqlite_execute_error(
            nomo_sqlite_validation_error("closed")
        );
    }
    nomo_sqlite_prepared prepared = nomo_sqlite_prepare_one(owner, sql);
    if (!prepared.ok) {
        return nomo_sqlite_execute_error(prepared.error);
    }
    nomo_sqlite_bound bound =
        nomo_sqlite_bind_values(prepared.statement, parameters);
    if (!bound.ok) {
        sqlite3_finalize(prepared.statement);
        return nomo_sqlite_execute_error(bound.error);
    }

    int result = sqlite3_step(prepared.statement);
    if (result == SQLITE_ROW) {
        sqlite3_finalize(prepared.statement);
        return nomo_sqlite_execute_error(
            nomo_sqlite_validation_error("unexpected_row")
        );
    }
    if (result != SQLITE_DONE) {
        sqlite3_finalize(prepared.statement);
        return nomo_sqlite_execute_error(
            nomo_sqlite_native_error(result, "step")
        );
    }
    sqlite3_int64 changes = sqlite3_changes64(owner->database);
    sqlite3_int64 row_id = sqlite3_last_insert_rowid(owner->database);
    result = sqlite3_finalize(prepared.statement);
    if (result != SQLITE_OK) {
        return nomo_sqlite_execute_error(
            nomo_sqlite_native_error(result, "step")
        );
    }
    return (@RESULT_EXECUTE@){
        .tag = @RESULT_EXECUTE_OK@,
        .payload.@OK_PAYLOAD@ = (@EXECUTE_RESULT@){
            .@CHANGES_MEMBER@ = changes < 0 ? 0 : (uint64_t)changes,
            .@LAST_INSERT_ROWID_MEMBER@ = (int64_t)row_id
        }
    };
}

static @RESULT_QUERY@ @QUERY_NAME@(
    @DATABASE@ database_value,
    nomo_string sql,
    @VALUE_ARRAY@ parameters
) {
    nomo_sqlite_database_state *owner =
        nomo_sqlite_find_database(database_value.@HANDLE_MEMBER@);
    if (owner == NULL) {
        return nomo_sqlite_query_error(
            nomo_sqlite_validation_error("closed")
        );
    }
    if (nomo_sqlite_query_count >= NOMO_SQLITE_MAX_QUERIES
        || nomo_sqlite_next_query_handle == 0) {
        return nomo_sqlite_query_error(
            nomo_sqlite_validation_error("limit")
        );
    }
    nomo_sqlite_prepared prepared = nomo_sqlite_prepare_one(owner, sql);
    if (!prepared.ok) {
        return nomo_sqlite_query_error(prepared.error);
    }
    nomo_sqlite_bound bound =
        nomo_sqlite_bind_values(prepared.statement, parameters);
    if (!bound.ok) {
        sqlite3_finalize(prepared.statement);
        return nomo_sqlite_query_error(bound.error);
    }

    nomo_sqlite_query_state *state =
        (nomo_sqlite_query_state *)calloc(1, sizeof(nomo_sqlite_query_state));
    if (state == NULL) {
        sqlite3_finalize(prepared.statement);
        return nomo_sqlite_query_error(
            nomo_sqlite_validation_error("internal")
        );
    }
    state->handle = nomo_sqlite_next_query_handle++;
    state->owner = owner;
    state->statement = prepared.statement;
    state->next = nomo_sqlite_queries;
    nomo_sqlite_queries = state;
    nomo_sqlite_query_count += 1;
    owner->live_queries += 1;

    return (@RESULT_QUERY@){
        .tag = @RESULT_QUERY_OK@,
        .payload.@OK_PAYLOAD@ = (@QUERY@){
            .@HANDLE_MEMBER@ = state->handle
        }
    };
}

static int nomo_sqlite_measure_row(
    sqlite3_stmt *statement,
    uint64_t max_row_bytes,
    @ERROR@ *error
) {
    int column_count = sqlite3_column_count(statement);
    if (column_count < 0 || column_count > 256) {
        *error = nomo_sqlite_validation_error("limit");
        return 0;
    }
    uint64_t total = 0;
    for (int index = 0; index < column_count; index += 1) {
        const char *name = sqlite3_column_name(statement, index);
        if (name == NULL) {
            *error = nomo_sqlite_validation_error("internal");
            return 0;
        }
        size_t name_length = strlen(name);
        if (!nomo_sqlite_valid_utf8(
                (const unsigned char *)name,
                name_length
            )
            || name_length > max_row_bytes - total) {
            *error = nomo_sqlite_validation_error(
                name_length > max_row_bytes - total ? "limit" : "encoding"
            );
            return 0;
        }
        total += (uint64_t)name_length;

        int value_type = sqlite3_column_type(statement, index);
        uint64_t value_bytes =
            value_type == SQLITE_NULL ? 1 : 8;
        if (value_type == SQLITE_TEXT || value_type == SQLITE_BLOB) {
            int length = sqlite3_column_bytes(statement, index);
            if (length < 0 || length > NOMO_SQLITE_MAX_VALUE_BYTES) {
                *error = nomo_sqlite_validation_error("limit");
                return 0;
            }
            const void *data = value_type == SQLITE_TEXT
                ? (const void *)sqlite3_column_text(statement, index)
                : sqlite3_column_blob(statement, index);
            if (length > 0 && data == NULL) {
                *error = nomo_sqlite_validation_error("internal");
                return 0;
            }
            if (value_type == SQLITE_TEXT
                && !nomo_sqlite_valid_utf8(
                    (const unsigned char *)data,
                    (size_t)length
                )) {
                *error = nomo_sqlite_validation_error("encoding");
                return 0;
            }
            value_bytes = (uint64_t)length;
        }
        if (value_bytes > max_row_bytes - total) {
            *error = nomo_sqlite_validation_error("limit");
            return 0;
        }
        total += value_bytes;
    }
    return 1;
}

static @RESULT_ROW@ @NEXT_NAME@(
    @QUERY@ query_value,
    uint64_t max_row_bytes
) {
    if (max_row_bytes == 0 || max_row_bytes > NOMO_SQLITE_MAX_ROW_BYTES) {
        return nomo_sqlite_row_error(
            nomo_sqlite_validation_error("invalid_request")
        );
    }
    nomo_sqlite_query_state *state =
        nomo_sqlite_find_query(query_value.@HANDLE_MEMBER@);
    if (state == NULL) {
        return nomo_sqlite_row_error(
            nomo_sqlite_validation_error("closed")
        );
    }
    if (state->done) {
        return (@RESULT_ROW@){
            .tag = @RESULT_ROW_OK@,
            .payload.@OK_PAYLOAD@ = (@OPTION_ROW@){
                .tag = @OPTION_ROW_NONE@
            }
        };
    }
    if (state->failed) {
        return nomo_sqlite_row_error(
            nomo_sqlite_validation_error("step")
        );
    }

    int result = sqlite3_step(state->statement);
    if (result == SQLITE_DONE) {
        state->done = 1;
        return (@RESULT_ROW@){
            .tag = @RESULT_ROW_OK@,
            .payload.@OK_PAYLOAD@ = (@OPTION_ROW@){
                .tag = @OPTION_ROW_NONE@
            }
        };
    }
    if (result != SQLITE_ROW) {
        state->failed = 1;
        return nomo_sqlite_row_error(
            nomo_sqlite_native_error(result, "step")
        );
    }

    @ERROR@ row_error = {0};
    if (!nomo_sqlite_measure_row(
            state->statement,
            max_row_bytes,
            &row_error
        )) {
        state->failed = 1;
        return nomo_sqlite_row_error(row_error);
    }

    @COLUMN_ARRAY@ columns = @COLUMN_ARRAY@_new();
    int column_count = sqlite3_column_count(state->statement);
    for (int index = 0; index < column_count; index += 1) {
        const char *name = sqlite3_column_name(state->statement, index);
        @VALUE@ value = {0};
        int value_type = sqlite3_column_type(state->statement, index);
        if (value_type == SQLITE_NULL) {
            value.tag = @VALUE_NULL@;
        } else if (value_type == SQLITE_INTEGER) {
            value.tag = @VALUE_INTEGER@;
            value.payload.@INTEGER_PAYLOAD@ =
                (int64_t)sqlite3_column_int64(state->statement, index);
        } else if (value_type == SQLITE_FLOAT) {
            value.tag = @VALUE_REAL@;
            value.payload.@REAL_PAYLOAD@ =
                sqlite3_column_double(state->statement, index);
        } else if (value_type == SQLITE_TEXT) {
            const unsigned char *text =
                sqlite3_column_text(state->statement, index);
            int length = sqlite3_column_bytes(state->statement, index);
            value.tag = @VALUE_TEXT@;
            value.payload.@TEXT_PAYLOAD@ = nomo_string_from_slice(
                (const char *)text,
                0,
                (size_t)length
            );
        } else {
            const unsigned char *blob =
                (const unsigned char *)sqlite3_column_blob(
                    state->statement,
                    index
                );
            int length = sqlite3_column_bytes(state->statement, index);
            @BYTE_ARRAY@ bytes = @BYTE_ARRAY@_new();
            for (int byte_index = 0; byte_index < length; byte_index += 1) {
                bytes = @BYTE_ARRAY@_push(
                    bytes,
                    (uint32_t)blob[byte_index]
                );
            }
            value.tag = @VALUE_BLOB@;
            value.payload.@BLOB_PAYLOAD@ = bytes;
        }

        @COLUMN@ column = {
            .@NAME_MEMBER@ = nomo_string_from_cstr(name),
            .@VALUE_MEMBER@ = value
        };
        columns = @COLUMN_ARRAY@_push(columns, column);
        @COLUMN_RELEASE@(column);
    }

    return (@RESULT_ROW@){
        .tag = @RESULT_ROW_OK@,
        .payload.@OK_PAYLOAD@ = (@OPTION_ROW@){
            .tag = @OPTION_ROW_SOME@,
            .payload.@SOME_PAYLOAD@ = (@ROW@){
                .@COLUMNS_MEMBER@ = columns
            }
        }
    };
}

static @RESULT_VOID@ @RESET_NAME@(
    @QUERY@ query_value,
    @VALUE_ARRAY@ parameters
) {
    nomo_sqlite_query_state *state =
        nomo_sqlite_find_query(query_value.@HANDLE_MEMBER@);
    if (state == NULL) {
        return nomo_sqlite_void_error(
            nomo_sqlite_validation_error("closed")
        );
    }
    int result = sqlite3_reset(state->statement);
    if (result != SQLITE_OK) {
        state->failed = 1;
        return nomo_sqlite_void_error(
            nomo_sqlite_native_error(result, "step")
        );
    }
    result = sqlite3_clear_bindings(state->statement);
    if (result != SQLITE_OK) {
        state->failed = 1;
        return nomo_sqlite_void_error(
            nomo_sqlite_native_error(result, "bind")
        );
    }
    nomo_sqlite_bound bound =
        nomo_sqlite_bind_values(state->statement, parameters);
    if (!bound.ok) {
        sqlite3_clear_bindings(state->statement);
        state->failed = 1;
        return nomo_sqlite_void_error(bound.error);
    }
    state->done = 0;
    state->failed = 0;
    return (@RESULT_VOID@){
        .tag = @RESULT_VOID_OK@,
        .payload.@OK_PAYLOAD@ = 0
    };
}

static @RESULT_VOID@ @CLOSE_QUERY_NAME@(@QUERY@ query_value) {
    nomo_sqlite_query_state *previous = NULL;
    nomo_sqlite_query_state *state = nomo_sqlite_queries;
    while (state != NULL && state->handle != query_value.@HANDLE_MEMBER@) {
        previous = state;
        state = state->next;
    }
    if (state == NULL) {
        return nomo_sqlite_void_error(
            nomo_sqlite_validation_error("closed")
        );
    }
    if (previous == NULL) {
        nomo_sqlite_queries = state->next;
    } else {
        previous->next = state->next;
    }
    if (state->owner != NULL && state->owner->live_queries > 0) {
        state->owner->live_queries -= 1;
    }
    int result = sqlite3_finalize(state->statement);
    free(state);
    nomo_sqlite_query_count -= 1;
    if (result != SQLITE_OK) {
        return nomo_sqlite_void_error(
            nomo_sqlite_native_error(result, "step")
        );
    }
    return (@RESULT_VOID@){
        .tag = @RESULT_VOID_OK@,
        .payload.@OK_PAYLOAD@ = 0
    };
}

static @RESULT_VOID@ @CLOSE_NAME@(@DATABASE@ database_value) {
    nomo_sqlite_database_state *previous = NULL;
    nomo_sqlite_database_state *state = nomo_sqlite_databases;
    while (state != NULL && state->handle != database_value.@HANDLE_MEMBER@) {
        previous = state;
        state = state->next;
    }
    if (state == NULL) {
        return nomo_sqlite_void_error(
            nomo_sqlite_validation_error("closed")
        );
    }
    if (state->live_queries != 0) {
        return nomo_sqlite_void_error(
            nomo_sqlite_validation_error("busy_handle")
        );
    }
    int result = sqlite3_close_v2(state->database);
    if (result != SQLITE_OK) {
        return nomo_sqlite_void_error(
            nomo_sqlite_native_error(result, "internal")
        );
    }
    if (previous == NULL) {
        nomo_sqlite_databases = state->next;
    } else {
        previous->next = state->next;
    }
    free(state);
    nomo_sqlite_database_count -= 1;
    return (@RESULT_VOID@){
        .tag = @RESULT_VOID_OK@,
        .payload.@OK_PAYLOAD@ = 0
    };
}

static void nomo_sqlite_shutdown(void) {
    size_t query_count = nomo_sqlite_query_count;
    size_t database_count = nomo_sqlite_database_count;
    nomo_sqlite_query_state *query = nomo_sqlite_queries;
    while (query != NULL) {
        nomo_sqlite_query_state *next = query->next;
        sqlite3_finalize(query->statement);
        free(query);
        query = next;
    }
    nomo_sqlite_queries = NULL;
    nomo_sqlite_query_count = 0;

    nomo_sqlite_database_state *database = nomo_sqlite_databases;
    while (database != NULL) {
        nomo_sqlite_database_state *next = database->next;
        sqlite3_close_v2(database->database);
        free(database);
        database = next;
    }
    nomo_sqlite_databases = NULL;
    nomo_sqlite_database_count = 0;

    if (query_count != 0 || database_count != 0) {
        fprintf(
            stderr,
            "nomo: closed %zu SQLite query handle(s) and %zu database handle(s) at shutdown\n",
            query_count,
            database_count
        );
    }
}
