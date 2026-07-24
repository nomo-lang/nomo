use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn sqlite_persists_between_native_cli_processes() {
    let root = temp_test_root("sqlite-persistence");
    reset_dir(&root);
    write_project(
        &root,
        r#"package sqlite_fixture.main

import std.array.Array
import std.sqlite

fn write_state() -> Result<void, SqliteError> {
    let database: SqliteDatabase = sqlite.open("agent.db", SqliteOpenMode.ReadWriteCreate, 0)?
    let empty: Array<SqliteValue> = Array.new<SqliteValue>()
    let created: SqliteExecuteResult = sqlite.execute(
        database,
        "CREATE TABLE IF NOT EXISTS agent_state(value TEXT NOT NULL)",
        empty
    )?
    let cleared: SqliteExecuteResult = sqlite.execute(database, "DELETE FROM agent_state", empty)?
    let mut values: Array<SqliteValue> = Array.new<SqliteValue>()
    values.push(SqliteValue.Text("persisted-state"))
    let inserted: SqliteExecuteResult = sqlite.execute(
        database,
        "INSERT INTO agent_state(value) VALUES (?)",
        values
    )?
    sqlite.close(database)?
    return Ok(void)
}

fn main() -> Result<void, SqliteError> {
    return write_state()
}
"#,
    );
    let first = run_nomo(&root);
    assert_success(&first);
    assert!(root.join("agent.db").is_file());
    assert!(first.stderr.is_empty(), "{}", output_text(&first));

    fs::write(
        root.join("src/main.nomo"),
        r#"package sqlite_fixture.main

import std.array.Array
import std.io
import std.sqlite

fn read_state() -> Result<void, SqliteError> {
    let database: SqliteDatabase = sqlite.open("agent.db", SqliteOpenMode.ReadOnly, 0)?
    let empty: Array<SqliteValue> = Array.new<SqliteValue>()
    let query_value: SqliteQuery = sqlite.query(
        database,
        "SELECT value FROM agent_state LIMIT 1",
        empty
    )?
    let row: Option<SqliteRow> = sqlite.next(query_value, 4096)?
    match row {
        None => {
            panic("missing persisted row")
        }
        Some(value) => {
            let columns: Array<SqliteColumn> = value.columns
            let first: Option<SqliteColumn> = columns.get(0)
            match first {
                None => {
                    panic("missing persisted column")
                }
                Some(column) => {
                    match column.value {
                        SqliteValue.Text(text) => {
                            io.println(text)
                        }
                        SqliteValue.Null => {
                            panic("unexpected null")
                        }
                        SqliteValue.Integer(number) => {
                            panic("unexpected integer")
                        }
                        SqliteValue.Real(number) => {
                            panic("unexpected real")
                        }
                        SqliteValue.Blob(bytes) => {
                            panic("unexpected blob")
                        }
                    }
                }
            }
        }
    }
    sqlite.close_query(query_value)?
    sqlite.close(database)?
    return Ok(void)
}

fn main() -> Result<void, SqliteError> {
    return read_state()
}
"#,
    )
    .unwrap();
    let second = run_nomo(&root);
    assert_success(&second);
    assert_eq!(normalized_text(&second.stdout), "persisted-state\n");
    assert!(second.stderr.is_empty(), "{}", output_text(&second));

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn sqlite_round_trips_all_values_resets_queries_and_commits_transactions() {
    let root = temp_test_root("sqlite-values");
    reset_dir(&root);
    write_project(
        &root,
        r#"package sqlite_fixture.main

import std.array.Array
import std.io
import std.sqlite

fn print_row(row: SqliteRow) -> void {
    for column in row.columns {
        match column.value {
            SqliteValue.Null => {
                io.println(column.name, "null")
            }
            SqliteValue.Integer(value) => {
                io.println(column.name, "integer", value)
            }
            SqliteValue.Real(value) => {
                io.println(column.name, "real", value)
            }
            SqliteValue.Text(value) => {
                io.println(column.name, "text", value)
            }
            SqliteValue.Blob(value) => {
                io.println(column.name, "blob", value.len())
                for byte in value {
                    io.println("byte", byte)
                }
            }
        }
    }
}

fn print_next(query_value: SqliteQuery, label: string) -> Result<void, SqliteError> {
    let next_row: Option<SqliteRow> = sqlite.next(query_value, 16777216)?
    match next_row {
        None => {
            io.println(label, "none")
        }
        Some(row) => {
            io.println(label, "row")
            print_row(row)
        }
    }
    return Ok(void)
}

fn exercise() -> Result<void, SqliteError> {
    let database: SqliteDatabase = sqlite.open_memory(300000)?
    let empty: Array<SqliteValue> = Array.new<SqliteValue>()
    let created: SqliteExecuteResult = sqlite.execute(
        database,
        "CREATE TABLE item (id INTEGER PRIMARY KEY, nullable, integer_value, real_value, text_value, blob_value)",
        empty
    )?
    let begun: SqliteExecuteResult = sqlite.execute(database, "BEGIN IMMEDIATE", empty)?

    let mut values: Array<SqliteValue> = Array.new<SqliteValue>()
    values.push(SqliteValue.Null)
    values.push(SqliteValue.Integer(-42))
    values.push(SqliteValue.Real(3.25))
    values.push(SqliteValue.Text("bound-text"))
    let blob: Array<u32> = Array.new<u32>()
    values.push(SqliteValue.Blob(blob))
    let inserted: SqliteExecuteResult = sqlite.execute(
        database,
        "INSERT INTO item(nullable, integer_value, real_value, text_value, blob_value) VALUES (?, ?, ?, ?, ?)",
        values
    )?
    io.println("inserted", inserted.changes, inserted.last_insert_rowid)
    let committed: SqliteExecuteResult = sqlite.execute(database, "COMMIT", empty)?

    let mut id: Array<SqliteValue> = Array.new<SqliteValue>()
    id.push(SqliteValue.Integer(inserted.last_insert_rowid))
    let query_value: SqliteQuery = sqlite.query(
        database,
        "SELECT nullable AS duplicate, integer_value AS duplicate, real_value AS real_value, text_value AS text_value, blob_value AS blob_value FROM item WHERE id = ?",
        id
    )?
    print_next(query_value, "stored")?
    print_next(query_value, "done")?
    print_next(query_value, "done-again")?

    let mut missing: Array<SqliteValue> = Array.new<SqliteValue>()
    missing.push(SqliteValue.Integer(999))
    sqlite.reset(query_value, missing)?
    print_next(query_value, "reset-missing")?

    let mut existing: Array<SqliteValue> = Array.new<SqliteValue>()
    existing.push(SqliteValue.Integer(inserted.last_insert_rowid))
    sqlite.reset(query_value, existing)?
    print_next(query_value, "reset-existing")?
    sqlite.close_query(query_value)?

    let mut first_blob: Array<u32> = Array.new<u32>()
    first_blob.push(0)
    first_blob.push(1)
    first_blob.push(255)
    let mut first_params: Array<SqliteValue> = Array.new<SqliteValue>()
    first_params.push(SqliteValue.Blob(first_blob))
    let reusable: SqliteQuery = sqlite.query(database, "SELECT ? AS selected", first_params)?
    print_next(reusable, "bound-first")?

    let empty_blob: Array<u32> = Array.new<u32>()
    let mut second_params: Array<SqliteValue> = Array.new<SqliteValue>()
    second_params.push(SqliteValue.Blob(empty_blob))
    sqlite.reset(reusable, second_params)?
    print_next(reusable, "bound-second")?
    let copied_query: SqliteQuery = reusable
    sqlite.close_query(reusable)?
    let stale_query: Result<Option<SqliteRow>, SqliteError> = sqlite.next(copied_query, 4096)
    match stale_query {
        Ok(value) => {
            panic("copied query stayed live after close")
        }
        Err(error) => {
            io.println("copied-query", error.code)
        }
    }

    let isolated: SqliteDatabase = sqlite.open_memory(0)?
    let missing_table: Result<SqliteQuery, SqliteError> = sqlite.query(
        isolated,
        "SELECT id FROM item",
        empty
    )
    match missing_table {
        Ok(unexpected) => {
            panic("in-memory database state leaked across handles")
        }
        Err(error) => {
            io.println("isolated", error.code)
        }
    }
    sqlite.close(isolated)?

    let unique_table: SqliteExecuteResult = sqlite.execute(
        database,
        "CREATE TABLE unique_value(value TEXT UNIQUE)",
        empty
    )?
    let mut seed: Array<SqliteValue> = Array.new<SqliteValue>()
    seed.push(SqliteValue.Text("seed"))
    let seeded: SqliteExecuteResult = sqlite.execute(
        database,
        "INSERT INTO unique_value(value) VALUES (?)",
        seed
    )?
    let failing_query: SqliteQuery = sqlite.query(
        database,
        "INSERT INTO unique_value(value) VALUES (?)",
        seed
    )?
    let failed_step: Result<Option<SqliteRow>, SqliteError> = sqlite.next(failing_query, 4096)
    match failed_step {
        Ok(unexpected) => {
            panic("query step accepted a duplicate unique value")
        }
        Err(error) => {
            io.println("step-error", error.code)
        }
    }
    let mut replacement: Array<SqliteValue> = Array.new<SqliteValue>()
    replacement.push(SqliteValue.Text("replacement"))
    let first_reset: Result<void, SqliteError> = sqlite.reset(failing_query, replacement)
    match first_reset {
        Ok(unexpected) => {
            panic("first reset hid the prior step error")
        }
        Err(error) => {
            io.println("reset-prior-error", error.code)
        }
    }
    sqlite.reset(failing_query, replacement)?
    let recovered_step: Option<SqliteRow> = sqlite.next(failing_query, 4096)?
    match recovered_step {
        None => {
            io.println("reset-recovered")
        }
        Some(unexpected) => {
            panic("non-row statement returned a row")
        }
    }
    sqlite.close_query(failing_query)?

    let copied_database: SqliteDatabase = database
    sqlite.close(database)?
    let stale_database: Result<void, SqliteError> = sqlite.close(copied_database)
    match stale_database {
        Ok(value) => {
            panic("copied database stayed live after close")
        }
        Err(error) => {
            io.println("copied-database", error.code)
        }
    }
    return Ok(void)
}

fn main() -> Result<void, SqliteError> {
    return exercise()
}
"#,
    );

    let output = run_nomo(&root);
    assert_success(&output);
    assert_eq!(
        normalized_text(&output.stdout),
        concat!(
            "inserted 1 1\n",
            "stored row\n",
            "duplicate null\n",
            "duplicate integer -42\n",
            "real_value real 3.25\n",
            "text_value text bound-text\n",
            "blob_value blob 0\n",
            "done none\n",
            "done-again none\n",
            "reset-missing none\n",
            "reset-existing row\n",
            "duplicate null\n",
            "duplicate integer -42\n",
            "real_value real 3.25\n",
            "text_value text bound-text\n",
            "blob_value blob 0\n",
            "bound-first row\n",
            "selected blob 3\n",
            "byte 0\n",
            "byte 1\n",
            "byte 255\n",
            "bound-second row\n",
            "selected blob 0\n",
            "copied-query closed\n",
            "isolated prepare\n",
            "step-error constraint\n",
            "reset-prior-error constraint\n",
            "reset-recovered\n",
            "copied-database closed\n"
        )
    );
    assert!(output.stderr.is_empty(), "{}", output_text(&output));

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn sqlite_enforces_request_row_binding_and_lifecycle_limits() {
    let root = temp_test_root("sqlite-boundaries");
    reset_dir(&root);
    write_project(
        &root,
        r#"package sqlite_fixture.main

import std.array.Array
import std.io
import std.sqlite

fn print_error(label: string, result: Result<void, SqliteError>) -> void {
    match result {
        Ok(value) => {
            panic("expected SQLite error")
        }
        Err(error) => {
            io.println(label, error.code)
        }
    }
}

fn exercise() -> Result<void, SqliteError> {
    let empty: Array<SqliteValue> = Array.new<SqliteValue>()
    let invalid_timeout: Result<SqliteDatabase, SqliteError> = sqlite.open_memory(300001)
    match invalid_timeout {
        Ok(database) => {
            panic("accepted oversized busy timeout")
        }
        Err(error) => {
            io.println("timeout", error.code)
        }
    }
    let invalid_path: Result<SqliteDatabase, SqliteError> = sqlite.open(
        ":memory:",
        SqliteOpenMode.ReadWriteCreate,
        0
    )
    match invalid_path {
        Ok(database) => {
            panic("accepted reserved memory path")
        }
        Err(error) => {
            io.println("path", error.code)
        }
    }

    let database: SqliteDatabase = sqlite.open_memory(0)?
    let multi: Result<SqliteExecuteResult, SqliteError> = sqlite.execute(
        database,
        "CREATE TABLE first(value TEXT); CREATE TABLE second(value TEXT)",
        empty
    )
    match multi {
        Ok(value) => {
            panic("accepted multiple statements")
        }
        Err(error) => {
            io.println("multi", error.code)
        }
    }
    let row_execute: Result<SqliteExecuteResult, SqliteError> = sqlite.execute(
        database,
        "SELECT 'do-not-log-this-value'",
        empty
    )
    match row_execute {
        Ok(value) => {
            panic("execute returned a row")
        }
        Err(error) => {
            io.println("execute-row", error.code)
        }
    }
    let count_created: SqliteExecuteResult = sqlite.execute(
        database,
        "CREATE TABLE count_check(value TEXT)",
        empty
    )?
    let wrong_count: Result<SqliteExecuteResult, SqliteError> = sqlite.execute(
        database,
        "INSERT INTO count_check(value) VALUES (?)",
        empty
    )
    match wrong_count {
        Ok(value) => {
            panic("accepted wrong parameter count")
        }
        Err(error) => {
            io.println("parameters", error.code)
        }
    }
    let mut bytes: Array<u32> = Array.new<u32>()
    bytes.push(256)
    let mut blob_values: Array<SqliteValue> = Array.new<SqliteValue>()
    blob_values.push(SqliteValue.Blob(bytes))
    let bad_blob: Result<SqliteExecuteResult, SqliteError> = sqlite.execute(
        database,
        "CREATE TABLE blob_check AS SELECT ? AS value",
        blob_values
    )
    match bad_blob {
        Ok(value) => {
            panic("accepted an out-of-range blob byte")
        }
        Err(error) => {
            io.println("blob", error.code)
        }
    }

    let query_value: SqliteQuery = sqlite.query(database, "SELECT 'abcdef'", empty)?
    let busy: Result<void, SqliteError> = sqlite.close(database)
    print_error("busy", busy)
    let zero_limit: Result<Option<SqliteRow>, SqliteError> = sqlite.next(query_value, 0)
    match zero_limit {
        Ok(value) => {
            panic("accepted a zero row limit")
        }
        Err(error) => {
            io.println("row-zero", error.code)
        }
    }
    let short_row: Result<Option<SqliteRow>, SqliteError> = sqlite.next(query_value, 1)
    match short_row {
        Ok(value) => {
            panic("accepted an oversized row")
        }
        Err(error) => {
            io.println("row-limit", error.code)
        }
    }
    sqlite.close_query(query_value)?
    let stale_query: Result<Option<SqliteRow>, SqliteError> = sqlite.next(query_value, 4096)
    match stale_query {
        Ok(value) => {
            panic("accepted a stale query")
        }
        Err(error) => {
            io.println("stale-query", error.code)
        }
    }
    sqlite.close(database)?
    let stale_database: Result<void, SqliteError> = sqlite.close(database)
    print_error("stale-database", stale_database)
    return Ok(void)
}

fn main() -> Result<void, SqliteError> {
    return exercise()
}
"#,
    );

    let output = run_nomo(&root);
    assert_success(&output);
    assert_eq!(
        normalized_text(&output.stdout),
        concat!(
            "timeout invalid_request\n",
            "path invalid_request\n",
            "multi invalid_request\n",
            "execute-row unexpected_row\n",
            "parameters invalid_request\n",
            "blob invalid_request\n",
            "busy busy_handle\n",
            "row-zero invalid_request\n",
            "row-limit limit\n",
            "stale-query closed\n",
            "stale-database closed\n"
        )
    );
    let combined = output_text(&output);
    assert!(!combined.contains("do-not-log-this-value"), "{combined}");
    assert!(output.stderr.is_empty(), "{combined}");

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn sqlite_enforces_exact_live_handle_limits_and_recovers_after_overflow() {
    let root = temp_test_root("sqlite-live-limits");
    reset_dir(&root);
    write_project(
        &root,
        r#"package sqlite_fixture.main

import std.array.Array
import std.io
import std.sqlite

fn exercise() -> Result<void, SqliteError> {
    let mut databases: Array<SqliteDatabase> = Array.new<SqliteDatabase>()
    for let i: u64 = 0; i < 32; i++ {
        databases.push(sqlite.open_memory(0)?)
    }
    let database_overflow: Result<SqliteDatabase, SqliteError> = sqlite.open_memory(0)
    match database_overflow {
        Ok(unexpected) => {
            panic("accepted a thirty-third live database")
        }
        Err(error) => {
            io.println("database-limit", error.code)
        }
    }
    for database in databases {
        sqlite.close(database)?
    }

    let database: SqliteDatabase = sqlite.open_memory(0)?
    let empty: Array<SqliteValue> = Array.new<SqliteValue>()
    let mut queries: Array<SqliteQuery> = Array.new<SqliteQuery>()
    for let i: u64 = 0; i < 256; i++ {
        queries.push(sqlite.query(database, "SELECT 1", empty)?)
    }
    let query_overflow: Result<SqliteQuery, SqliteError> = sqlite.query(
        database,
        "SELECT 1",
        empty
    )
    match query_overflow {
        Ok(unexpected) => {
            panic("accepted a two-hundred-fifty-seventh live query")
        }
        Err(error) => {
            io.println("query-limit", error.code)
        }
    }
    for query_value in queries {
        sqlite.close_query(query_value)?
    }
    sqlite.close(database)?

    let recovered: SqliteDatabase = sqlite.open_memory(0)?
    sqlite.close(recovered)?
    io.println("recovered")
    return Ok(void)
}

fn main() -> Result<void, SqliteError> {
    return exercise()
}
"#,
    );

    let output = run_nomo(&root);
    assert_success(&output);
    assert_eq!(
        normalized_text(&output.stdout),
        concat!(
            "database-limit limit\n",
            "query-limit limit\n",
            "recovered\n"
        )
    );
    assert!(output.stderr.is_empty(), "{}", output_text(&output));

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn sqlite_classifies_native_failures_and_process_exit_cleanup_is_secret_safe() {
    let root = temp_test_root("sqlite-native-errors");
    reset_dir(&root);
    write_project(
        &root,
        r#"package sqlite_fixture.main

import std.array.Array
import std.io
import std.sqlite

fn print_execute_error(label: string, result: Result<SqliteExecuteResult, SqliteError>) -> void {
    match result {
        Ok(unexpected) => {
            panic("expected SQLite execution failure")
        }
        Err(error) => {
            io.println(label, error.code)
        }
    }
}

fn exercise() -> Result<void, SqliteError> {
    let empty: Array<SqliteValue> = Array.new<SqliteValue>()
    let first: SqliteDatabase = sqlite.open(
        "sqlite-secret-path.db",
        SqliteOpenMode.ReadWriteCreate,
        0
    )?
    let created: SqliteExecuteResult = sqlite.execute(
        first,
        "CREATE TABLE private_schema(value TEXT UNIQUE)",
        empty
    )?
    let mut secret: Array<SqliteValue> = Array.new<SqliteValue>()
    secret.push(SqliteValue.Text("secret-bound-token"))
    let inserted: SqliteExecuteResult = sqlite.execute(
        first,
        "INSERT INTO private_schema(value) VALUES (?)",
        secret
    )?
    print_execute_error(
        "constraint",
        sqlite.execute(first, "INSERT INTO private_schema(value) VALUES (?)", secret)
    )

    let second: SqliteDatabase = sqlite.open(
        "sqlite-secret-path.db",
        SqliteOpenMode.ReadWrite,
        1
    )?
    let begun: SqliteExecuteResult = sqlite.execute(first, "BEGIN IMMEDIATE", empty)?
    print_execute_error(
        "busy",
        sqlite.execute(second, "INSERT INTO private_schema(value) VALUES ('busy-secret')", empty)
    )
    let rolled_back: SqliteExecuteResult = sqlite.execute(first, "ROLLBACK", empty)?
    sqlite.close(second)?
    sqlite.close(first)?

    let read_only: SqliteDatabase = sqlite.open(
        "sqlite-secret-path.db",
        SqliteOpenMode.ReadOnly,
        0
    )?
    print_execute_error(
        "read-only",
        sqlite.execute(read_only, "INSERT INTO private_schema(value) VALUES ('read-only-secret')", empty)
    )
    let invalid_text: SqliteQuery = sqlite.query(
        read_only,
        "SELECT CAST(X'80' AS TEXT) AS invalid_secret_text",
        empty
    )?
    let invalid_row: Result<Option<SqliteRow>, SqliteError> = sqlite.next(invalid_text, 4096)
    match invalid_row {
        Ok(unexpected) => {
            panic("accepted invalid UTF-8 text")
        }
        Err(error) => {
            io.println("encoding", error.code)
        }
    }
    sqlite.close_query(invalid_text)?
    sqlite.close(read_only)?

    let corrupt: SqliteDatabase = sqlite.open(
        "corrupt-secret.db",
        SqliteOpenMode.ReadOnly,
        0
    )?
    let corrupt_query: Result<SqliteQuery, SqliteError> = sqlite.query(
        corrupt,
        "SELECT * FROM corrupt_secret_schema",
        empty
    )
    match corrupt_query {
        Ok(unexpected) => {
            panic("accepted corrupt database input")
        }
        Err(error) => {
            io.println("corrupt", error.code)
        }
    }
    sqlite.close(corrupt)?

    let full: SqliteDatabase = sqlite.open(
        "full-secret.db",
        SqliteOpenMode.ReadWriteCreate,
        0
    )?
    let page_size: SqliteExecuteResult = sqlite.execute(
        full,
        "PRAGMA page_size = 512",
        empty
    )?
    let page_limit: SqliteQuery = sqlite.query(full, "PRAGMA max_page_count = 3", empty)?
    let applied_limit: Option<SqliteRow> = sqlite.next(page_limit, 4096)?
    sqlite.close_query(page_limit)?
    let full_table: SqliteExecuteResult = sqlite.execute(
        full,
        "CREATE TABLE full_secret_schema(value BLOB)",
        empty
    )?
    print_execute_error(
        "full",
        sqlite.execute(full, "INSERT INTO full_secret_schema(value) VALUES (zeroblob(100000))", empty)
    )
    sqlite.close(full)?
    return Ok(void)
}

fn main() -> Result<void, SqliteError> {
    return exercise()
}
"#,
    );
    fs::write(
        root.join("corrupt-secret.db"),
        b"corrupt-secret-file-content-not-a-sqlite-database",
    )
    .unwrap();

    let output = run_nomo(&root);
    assert_success(&output);
    assert_eq!(
        normalized_text(&output.stdout),
        concat!(
            "constraint constraint\n",
            "busy busy\n",
            "read-only read_only\n",
            "encoding encoding\n",
            "corrupt corrupt\n",
            "full full\n"
        )
    );
    let combined = output_text(&output);
    for secret in [
        "sqlite-secret-path",
        "secret-bound-token",
        "private_schema",
        "busy-secret",
        "read-only-secret",
        "invalid_secret_text",
        "corrupt-secret",
        "full-secret",
        "full_secret_schema",
    ] {
        assert!(!combined.contains(secret), "leaked `{secret}`:\n{combined}");
    }
    assert!(output.stderr.is_empty(), "{combined}");

    fs::write(
        root.join("src/main.nomo"),
        r#"package sqlite_fixture.main

import std.array.Array
import std.sqlite

fn main() -> Result<void, SqliteError> {
    let database: SqliteDatabase = sqlite.open(
        "cleanup-secret-path.db",
        SqliteOpenMode.ReadWriteCreate,
        0
    )?
    let empty: Array<SqliteValue> = Array.new<SqliteValue>()
    let query_value: SqliteQuery = sqlite.query(
        database,
        "SELECT 'cleanup-secret-row' AS cleanup_secret_column",
        empty
    )?
    return Ok(void)
}
"#,
    )
    .unwrap();
    let cleanup = run_nomo(&root);
    assert_success(&cleanup);
    assert!(cleanup.stdout.is_empty(), "{}", output_text(&cleanup));
    assert_eq!(
        normalized_text(&cleanup.stderr),
        "nomo: closed 1 SQLite query handle(s) and 1 database handle(s) at shutdown\n"
    );
    let cleanup_text = output_text(&cleanup);
    for secret in [
        "cleanup-secret-path",
        "cleanup-secret-row",
        "cleanup_secret_column",
    ] {
        assert!(
            !cleanup_text.contains(secret),
            "leaked `{secret}`:\n{cleanup_text}"
        );
    }

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn sqlite_enforces_exact_path_sql_parameter_value_column_row_and_timeout_limits() {
    const MAX_PATH_BYTES: usize = 4096;
    const MAX_SQL_BYTES: usize = 1024 * 1024;
    let root = temp_test_root("sqlite-exact-limits");
    reset_dir(&root);

    let exact_path = "p".repeat(MAX_PATH_BYTES);
    let oversized_path = "p".repeat(MAX_PATH_BYTES + 1);
    let sql_prefix = "SELECT 1 --";
    let exact_sql = format!(
        "{sql_prefix}{}",
        "s".repeat(MAX_SQL_BYTES - sql_prefix.len())
    );
    let oversized_sql = format!("{exact_sql}s");
    let exact_parameter_sql = format!(
        "INSERT INTO parameter_limit(value) VALUES {}",
        vec!["(?)"; 1024].join(",")
    );
    let oversized_parameter_sql = format!(
        "INSERT INTO parameter_limit(value) VALUES {}",
        vec!["(?)"; 1025].join(",")
    );
    let exact_column_sql = format!(
        "SELECT {}",
        (0..256)
            .map(|index| format!("1 AS c{index}"))
            .collect::<Vec<_>>()
            .join(",")
    );
    let oversized_column_sql = format!(
        "SELECT {}",
        (0..257)
            .map(|index| format!("1 AS c{index}"))
            .collect::<Vec<_>>()
            .join(",")
    );

    let source = r#"package sqlite_fixture.main

import std.array.Array
import std.io
import std.sqlite
import std.string

fn exercise() -> Result<void, SqliteError> {
    let empty: Array<SqliteValue> = Array.new<SqliteValue>()

    let exact_timeout: SqliteDatabase = sqlite.open_memory(300000)?
    sqlite.close(exact_timeout)?
    let oversized_timeout: Result<SqliteDatabase, SqliteError> = sqlite.open_memory(300001)
    match oversized_timeout {
        Ok(unexpected) => {
            panic("accepted oversized timeout")
        }
        Err(error) => {
            io.println("timeout-over", error.code)
        }
    }

    let exact_path: Result<SqliteDatabase, SqliteError> = sqlite.open(
        "__EXACT_PATH__",
        SqliteOpenMode.ReadOnly,
        0
    )
    match exact_path {
        Ok(unexpected) => {
            sqlite.close(unexpected)?
            io.println("path-exact", "opened")
        }
        Err(error) => {
            io.println("path-exact", error.code)
        }
    }
    let oversized_path: Result<SqliteDatabase, SqliteError> = sqlite.open(
        "__OVERSIZED_PATH__",
        SqliteOpenMode.ReadOnly,
        0
    )
    match oversized_path {
        Ok(unexpected) => {
            panic("accepted oversized path")
        }
        Err(error) => {
            io.println("path-over", error.code)
        }
    }

    let database: SqliteDatabase = sqlite.open_memory(0)?
    let exact_sql: SqliteQuery = sqlite.query(database, "__EXACT_SQL__", empty)?
    sqlite.close_query(exact_sql)?
    io.println("sql-exact")
    let oversized_sql: Result<SqliteQuery, SqliteError> = sqlite.query(
        database,
        "__OVERSIZED_SQL__",
        empty
    )
    match oversized_sql {
        Ok(unexpected) => {
            panic("accepted oversized SQL")
        }
        Err(error) => {
            io.println("sql-over", error.code)
        }
    }

    let parameter_table: SqliteExecuteResult = sqlite.execute(
        database,
        "CREATE TABLE parameter_limit(value INTEGER)",
        empty
    )?
    let mut exact_parameters: Array<SqliteValue> = Array.new<SqliteValue>()
    for let i: u64 = 0; i < 1024; i++ {
        exact_parameters.push(SqliteValue.Integer(1))
    }
    let inserted: SqliteExecuteResult = sqlite.execute(
        database,
        "__EXACT_PARAMETER_SQL__",
        exact_parameters
    )?
    io.println("parameters-exact", inserted.changes)
    let mut oversized_parameters: Array<SqliteValue> = Array.new<SqliteValue>()
    for let i: u64 = 0; i < 1025; i++ {
        oversized_parameters.push(SqliteValue.Integer(1))
    }
    let parameter_overflow: Result<SqliteExecuteResult, SqliteError> = sqlite.execute(
        database,
        "__OVERSIZED_PARAMETER_SQL__",
        oversized_parameters
    )
    match parameter_overflow {
        Ok(unexpected) => {
            panic("accepted oversized parameter array")
        }
        Err(error) => {
            io.println("parameters-over", error.code)
        }
    }

    let mut maximum_text: string = "x"
    for let i: u64 = 0; i < 23; i++ {
        maximum_text = string.concat(maximum_text, maximum_text)
    }
    let mut maximum_value: Array<SqliteValue> = Array.new<SqliteValue>()
    maximum_value.push(SqliteValue.Text(maximum_text))
    let maximum_query: SqliteQuery = sqlite.query(database, "SELECT ?", maximum_value)?
    sqlite.close_query(maximum_query)?
    io.println("value-exact", maximum_text.len())

    let oversized_text: string = string.concat(maximum_text, "x")
    let mut oversized_value: Array<SqliteValue> = Array.new<SqliteValue>()
    oversized_value.push(SqliteValue.Text(oversized_text))
    let value_overflow: Result<SqliteQuery, SqliteError> = sqlite.query(
        database,
        "SELECT ?",
        oversized_value
    )
    match value_overflow {
        Ok(unexpected) => {
            panic("accepted oversized SQLite value")
        }
        Err(error) => {
            io.println("value-over", error.code)
        }
    }

    let mut total_exact: Array<SqliteValue> = Array.new<SqliteValue>()
    total_exact.push(SqliteValue.Text(maximum_text))
    total_exact.push(SqliteValue.Text(maximum_text))
    let total_exact_query: SqliteQuery = sqlite.query(database, "SELECT ?, ?", total_exact)?
    sqlite.close_query(total_exact_query)?
    io.println("total-exact", 16777216)

    let mut total_over: Array<SqliteValue> = Array.new<SqliteValue>()
    total_over.push(SqliteValue.Text(maximum_text))
    total_over.push(SqliteValue.Text(maximum_text))
    total_over.push(SqliteValue.Text("x"))
    let total_overflow: Result<SqliteQuery, SqliteError> = sqlite.query(
        database,
        "SELECT ?, ?, ?",
        total_over
    )
    match total_overflow {
        Ok(unexpected) => {
            panic("accepted oversized total parameter bytes")
        }
        Err(error) => {
            io.println("total-over", error.code)
        }
    }

    let exact_columns: SqliteQuery = sqlite.query(database, "__EXACT_COLUMN_SQL__", empty)?
    let exact_column_row: Option<SqliteRow> = sqlite.next(exact_columns, 16777216)?
    match exact_column_row {
        None => {
            panic("missing exact-limit column row")
        }
        Some(row) => {
            let columns: Array<SqliteColumn> = row.columns
            let column_count: u64 = columns.len()
            io.println("columns-exact", column_count)
        }
    }
    sqlite.close_query(exact_columns)?
    let column_overflow: Result<SqliteQuery, SqliteError> = sqlite.query(
        database,
        "__OVERSIZED_COLUMN_SQL__",
        empty
    )
    match column_overflow {
        Ok(unexpected) => {
            panic("accepted oversized result-column count")
        }
        Err(error) => {
            io.println("columns-over", error.code)
        }
    }

    let row_limit_query: SqliteQuery = sqlite.query(database, "SELECT 'row'", empty)?
    let exact_row_limit: Option<SqliteRow> = sqlite.next(row_limit_query, 16777216)?
    let oversized_row_limit: Result<Option<SqliteRow>, SqliteError> = sqlite.next(
        row_limit_query,
        16777217
    )
    match oversized_row_limit {
        Ok(unexpected) => {
            panic("accepted oversized caller row limit")
        }
        Err(error) => {
            io.println("row-limit-over", error.code)
        }
    }
    sqlite.close_query(row_limit_query)?
    sqlite.close(database)?
    return Ok(void)
}

fn main() -> Result<void, SqliteError> {
    return exercise()
}
"#
    .replace("__EXACT_PATH__", &exact_path)
    .replace("__OVERSIZED_PATH__", &oversized_path)
    .replace("__EXACT_SQL__", &exact_sql)
    .replace("__OVERSIZED_SQL__", &oversized_sql)
    .replace("__EXACT_PARAMETER_SQL__", &exact_parameter_sql)
    .replace("__OVERSIZED_PARAMETER_SQL__", &oversized_parameter_sql)
    .replace("__EXACT_COLUMN_SQL__", &exact_column_sql)
    .replace("__OVERSIZED_COLUMN_SQL__", &oversized_column_sql);
    write_project(&root, &source);

    let output = run_nomo(&root);
    assert_success(&output);
    assert_eq!(
        normalized_text(&output.stdout),
        concat!(
            "timeout-over invalid_request\n",
            "path-exact open\n",
            "path-over invalid_request\n",
            "sql-exact\n",
            "sql-over invalid_request\n",
            "parameters-exact 1024\n",
            "parameters-over prepare\n",
            "value-exact 8388608\n",
            "value-over limit\n",
            "total-exact 16777216\n",
            "total-over limit\n",
            "columns-exact 256\n",
            "columns-over prepare\n",
            "row-limit-over invalid_request\n"
        )
    );
    assert!(output.stderr.is_empty(), "{}", output_text(&output));

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn sqlite_wrapper_stress_passes_address_and_leak_sanitizers_when_available() {
    let root = temp_test_root("sqlite-sanitizer");
    reset_dir(&root);
    if !cc_supports_address_sanitizer(&root) {
        fs::remove_dir_all(root).unwrap();
        return;
    }

    write_project(
        &root,
        r#"package sqlite_fixture.main

import std.array.Array
import std.sqlite

fn exercise() -> Result<void, SqliteError> {
    for let database_index: u64 = 0; database_index < 64; database_index++ {
        let database: SqliteDatabase = sqlite.open_memory(0)?
        let mut initial: Array<SqliteValue> = Array.new<SqliteValue>()
        initial.push(SqliteValue.Integer(0))
        let query_value: SqliteQuery = sqlite.query(database, "SELECT ? AS value", initial)?
        for let query_index: i64 = 0; query_index < 64; query_index++ {
            let mut params: Array<SqliteValue> = Array.new<SqliteValue>()
            params.push(SqliteValue.Integer(query_index))
            sqlite.reset(query_value, params)?
            let row: Option<SqliteRow> = sqlite.next(query_value, 4096)?
        }
        sqlite.close_query(query_value)?
        sqlite.close(database)?
    }
    return Ok(void)
}

fn main() -> Result<void, SqliteError> {
    return exercise()
}
"#,
    );
    let build = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("build")
        .arg(&root)
        .arg("--emit-c")
        .output()
        .unwrap();
    assert_success(&build);

    let c_dir = root.join("build/c");
    let binary = root.join("sqlite-sanitizer");
    let mut compiler = Command::new("cc");
    compiler
        .arg("-std=c99")
        .arg("-fsanitize=address")
        .arg("-fno-omit-frame-pointer")
        .arg("-g");
    for option in nomo_codegen_c::BUNDLED_SQLITE_COMPILE_OPTIONS {
        compiler.arg(format!("-D{option}"));
    }
    compiler
        .arg(c_dir.join("main.c"))
        .arg(c_dir.join("sqlite3.c"))
        .arg("-pthread");
    if cfg!(target_os = "linux") {
        compiler.arg("-ldl");
    }
    if !cfg!(target_os = "windows") {
        compiler.arg("-lm");
    }
    let compiled = compiler.arg("-o").arg(&binary).output().unwrap();
    assert!(compiled.status.success(), "{}", output_text(&compiled));

    let sanitizer_options = if cfg!(target_os = "macos") {
        "detect_leaks=0:abort_on_error=1"
    } else {
        "detect_leaks=1:abort_on_error=1"
    };
    let run = Command::new(binary)
        .env("ASAN_OPTIONS", sanitizer_options)
        .output()
        .unwrap();
    assert_success(&run);
    assert!(run.stdout.is_empty(), "{}", output_text(&run));
    assert!(run.stderr.is_empty(), "{}", output_text(&run));

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn sqlite_agent_memory_example_persists_checkpoint_for_a_second_process() {
    let root = temp_test_root("sqlite-agent-memory-example");
    reset_dir(&root);
    let database = root.join("agent-checkpoint.db");
    let example = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("examples/sqlite_agent_memory");
    let manifest = fs::read_to_string(example.join("nomo.toml")).unwrap();
    assert!(!manifest.contains("[ffi]"));
    assert!(!manifest.contains("linker"));
    assert!(!manifest.contains("sources"));

    let write = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(&example)
        .arg("--")
        .arg("write")
        .arg(&database)
        .output()
        .unwrap();
    assert_success(&write);
    assert_eq!(normalized_text(&write.stdout), "checkpoint-written 1\n");
    assert!(write.stderr.is_empty(), "{}", output_text(&write));
    assert!(database.is_file());

    let read = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(&example)
        .arg("--")
        .arg("read")
        .arg(&database)
        .output()
        .unwrap();
    assert_success(&read);
    assert_eq!(
        normalized_text(&read.stdout),
        concat!(
            "request {\"messages\":[{\"role\":\"user\",\"content\":\"hello\"}]}\n",
            "checkpoint {\"next_tool\":\"search\",\"attempt\":2}\n"
        )
    );
    assert!(read.stderr.is_empty(), "{}", output_text(&read));

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn sqlite_works_through_nomo_test_and_standard_documentation() {
    let root = temp_test_root("sqlite-test-command");
    reset_dir(&root);
    write_project(
        &root,
        r#"package sqlite_fixture.main

import std.array.Array
import std.sqlite

fn exercise() -> Result<void, SqliteError> {
    let database: SqliteDatabase = sqlite.open_memory(0)?
    let empty: Array<SqliteValue> = Array.new<SqliteValue>()
    let created: SqliteExecuteResult = sqlite.execute(
        database,
        "CREATE TABLE test_value(value INTEGER)",
        empty
    )?
    sqlite.close(database)?
    return Ok(void)
}

#[test]
fn sqlite_runtime_is_available() -> void {
    let result: Result<void, SqliteError> = exercise()
    match result {
        Ok(value) => {
        }
        Err(error) => {
            panic(error.message)
        }
    }
}

fn main() -> void {
}
"#,
    );

    let tested = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("test")
        .arg(&root)
        .output()
        .unwrap();
    assert_success(&tested);
    assert!(
        normalized_text(&tested.stdout)
            .contains("ok sqlite_fixture.main.sqlite_runtime_is_available"),
        "{}",
        output_text(&tested)
    );
    assert!(tested.stderr.is_empty(), "{}", output_text(&tested));

    let documented = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("doc")
        .arg(&root)
        .arg("--std")
        .arg("--json")
        .output()
        .unwrap();
    assert_success(&documented);
    let docs = normalized_text(&documented.stdout);
    assert!(docs.contains("\"name\":\"std.sqlite\""), "{docs}");
    assert!(docs.contains("\"name\":\"SqliteDatabase\""), "{docs}");
    assert!(docs.contains("\"name\":\"open_memory\""), "{docs}");
    assert!(
        docs.contains("\"source\":\"std/src/sqlite.nomo\""),
        "{docs}"
    );
    assert!(documented.stderr.is_empty(), "{}", output_text(&documented));

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn emit_c_materializes_verified_sqlite_sources_only_when_used() {
    let root = temp_test_root("sqlite-emit-c");
    reset_dir(&root);
    let sqlite_project = root.join("sqlite");
    let plain_project = root.join("plain");
    write_project(
        &sqlite_project,
        r#"package sqlite_fixture.main

import std.sqlite

fn main() -> void {
    let opened: Result<SqliteDatabase, SqliteError> = sqlite.open_memory(0)
}
"#,
    );
    write_project(
        &plain_project,
        r#"package sqlite_fixture.main

fn main() -> void {
}
"#,
    );

    for project in [&sqlite_project, &plain_project] {
        let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
            .arg("build")
            .arg(project)
            .arg("--emit-c")
            .output()
            .unwrap();
        assert_success(&output);
    }

    let sqlite_c_dir = sqlite_project.join("build/c");
    assert!(sqlite_c_dir.join("main.c").is_file());
    assert!(sqlite_c_dir.join("sqlite3.c").is_file());
    assert!(sqlite_c_dir.join("sqlite3.h").is_file());
    assert!(sqlite_c_dir.join("sqlite3-SOURCE.md").is_file());
    assert_eq!(
        sha256_file(&sqlite_c_dir.join("sqlite3.c")),
        nomo_codegen_c::BUNDLED_SQLITE3_C_SHA256
    );
    assert_eq!(
        sha256_file(&sqlite_c_dir.join("sqlite3.h")),
        nomo_codegen_c::BUNDLED_SQLITE3_H_SHA256
    );
    let provenance = fs::read_to_string(sqlite_c_dir.join("sqlite3-SOURCE.md")).unwrap();
    assert!(provenance.contains("Version: 3.53.3"));
    assert!(provenance.contains("Archive SHA3-256"));
    assert!(provenance.contains("Public-domain notice"));
    for option in nomo_codegen_c::BUNDLED_SQLITE_COMPILE_OPTIONS {
        assert!(provenance.contains(option), "missing `{option}`");
    }
    let generated = fs::read_to_string(sqlite_c_dir.join("main.c")).unwrap();
    assert!(generated.contains("#include \"sqlite3.h\""));
    assert!(generated.contains("#define NOMO_SQLITE_MAX_DATABASES 32"));

    let plain_c_dir = plain_project.join("build/c");
    assert!(plain_c_dir.join("main.c").is_file());
    assert!(!plain_c_dir.join("sqlite3.c").exists());
    assert!(!plain_c_dir.join("sqlite3.h").exists());
    assert!(!plain_c_dir.join("sqlite3-SOURCE.md").exists());
    let generated = fs::read_to_string(plain_c_dir.join("main.c")).unwrap();
    assert!(!generated.contains("NOMO_SQLITE_MAX_DATABASES"));

    fs::remove_dir_all(root).unwrap();
}

fn write_project(root: &Path, source: &str) {
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("nomo.toml"),
        "[package]\nname = \"sqlite_fixture\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(root.join("src/main.nomo"), source).unwrap();
}

fn run_nomo(root: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(root)
        .output()
        .unwrap()
}

fn assert_success(output: &Output) {
    assert!(output.status.success(), "{}", output_text(output));
}

fn output_text(output: &Output) -> String {
    format!(
        "stdout:\n{}\nstderr:\n{}",
        normalized_text(&output.stdout),
        normalized_text(&output.stderr)
    )
}

fn normalized_text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).replace("\r\n", "\n")
}

fn sha256_file(path: &Path) -> String {
    format!("{:x}", Sha256::digest(fs::read(path).unwrap()))
}

fn temp_test_root(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("nomo-{name}-{}-{nanos}", std::process::id()))
}

fn reset_dir(path: &Path) {
    if path.exists() {
        fs::remove_dir_all(path).unwrap();
    }
    fs::create_dir_all(path).unwrap();
}

fn cc_supports_address_sanitizer(root: &Path) -> bool {
    let source = root.join("asan-probe.c");
    let binary = root.join("asan-probe");
    fs::write(&source, "int main(void) { return 0; }\n").unwrap();
    let Ok(compiled) = Command::new("cc")
        .arg("-fsanitize=address")
        .arg(&source)
        .arg("-o")
        .arg(&binary)
        .output()
    else {
        return false;
    };
    if !compiled.status.success() {
        return false;
    }
    let Ok(run) = Command::new(binary)
        .env("ASAN_OPTIONS", "detect_leaks=0:abort_on_error=1")
        .output()
    else {
        return false;
    };
    run.status.success()
}
