use super::*;

#[test]
fn lowers_bounded_sqlite_lifecycle_calls() {
    let source = r#"package app.main

import std.array.Array
import std.sqlite

fn run() -> Result<void, SqliteError> {
    let persistent: SqliteDatabase = sqlite.open("state.db", SqliteOpenMode.ReadWriteCreate, 0)?
    sqlite.close(persistent)?
    let database: SqliteDatabase = sqlite.open_memory(1000)?
    let empty: Array<SqliteValue> = Array.new<SqliteValue>()
    let created: SqliteExecuteResult = sqlite.execute(database, "CREATE TABLE item(value TEXT)", empty)?
    let query_value: SqliteQuery = sqlite.query(database, "SELECT value FROM item", empty)?
    let row: Option<SqliteRow> = sqlite.next(query_value, 4096)?
    sqlite.reset(query_value, empty)?
    sqlite.close_query(query_value)?
    sqlite.close(database)?
    return Ok(void)
}

fn main() -> void {
    let result: Result<void, SqliteError> = run()
}
"#;

    let program = parse_inline(source).unwrap();
    for expected in [
        "SqliteColumn",
        "SqliteDatabase",
        "SqliteError",
        "SqliteExecuteResult",
        "SqliteQuery",
        "SqliteRow",
    ] {
        assert!(program.structs.iter().any(|item| item.name == expected));
    }
    for expected in ["SqliteOpenMode", "SqliteValue"] {
        assert!(program.enums.iter().any(|item| item.name == expected));
    }
    let debug = format!("{:?}", program.functions);
    for intrinsic in [
        BUILTIN_SQLITE_OPEN_EXPR,
        BUILTIN_SQLITE_OPEN_MEMORY_EXPR,
        BUILTIN_SQLITE_EXECUTE_EXPR,
        BUILTIN_SQLITE_QUERY_EXPR,
        BUILTIN_SQLITE_NEXT_EXPR,
        BUILTIN_SQLITE_RESET_EXPR,
        BUILTIN_SQLITE_CLOSE_QUERY_EXPR,
        BUILTIN_SQLITE_CLOSE_EXPR,
    ] {
        assert!(debug.contains(intrinsic), "missing intrinsic {intrinsic}");
    }
}

#[test]
fn validates_every_sqlite_operation_argument_and_result_type() {
    let cases = [
        (
            r#"let value: Result<SqliteDatabase, SqliteError> =
        sqlite.open(1, SqliteOpenMode.ReadOnly, 0)"#,
            "string",
        ),
        (
            r#"let value: Result<SqliteDatabase, SqliteError> =
        sqlite.open("state.db", 1, 0)"#,
            "SqliteOpenMode",
        ),
        (
            r#"let value: Result<SqliteDatabase, SqliteError> =
        sqlite.open("state.db", SqliteOpenMode.ReadOnly, "slow")"#,
            "u64",
        ),
        (
            r#"let value: Result<SqliteDatabase, SqliteError> =
        sqlite.open_memory("slow")"#,
            "u64",
        ),
        (
            r#"let database: SqliteDatabase = sqlite.open_memory(0)?
    let values: Array<string> = Array.new<string>()
    let value: Result<SqliteExecuteResult, SqliteError> =
        sqlite.execute(database, "SELECT 1", values)"#,
            "Array<SqliteValue>",
        ),
        (
            r#"let database: SqliteDatabase = sqlite.open_memory(0)?
    let values: Array<SqliteValue> = Array.new<SqliteValue>()
    let value: Result<SqliteExecuteResult, SqliteError> =
        sqlite.execute(database, 1, values)"#,
            "string",
        ),
        (
            r#"let values: Array<SqliteValue> = Array.new<SqliteValue>()
    let value: Result<SqliteQuery, SqliteError> =
        sqlite.query("not-a-database", "SELECT 1", values)"#,
            "SqliteDatabase",
        ),
        (
            r#"let database: SqliteDatabase = sqlite.open_memory(0)?
    let values: Array<SqliteValue> = Array.new<SqliteValue>()
    let query_value: SqliteQuery = sqlite.query(database, "SELECT 1", values)?
    let row: Result<Option<SqliteRow>, SqliteError> =
        sqlite.next(query_value, "large")"#,
            "u64",
        ),
        (
            r#"let database: SqliteDatabase = sqlite.open_memory(0)?
    let values: Array<SqliteValue> = Array.new<SqliteValue>()
    let query_value: SqliteQuery = sqlite.query(database, "SELECT 1", values)?
    let replacements: Array<string> = Array.new<string>()
    let value: Result<void, SqliteError> = sqlite.reset(query_value, replacements)"#,
            "Array<SqliteValue>",
        ),
        (
            r#"let database: SqliteDatabase = sqlite.open_memory(0)?
    let value: Result<void, SqliteError> = sqlite.close_query(database)"#,
            "SqliteQuery",
        ),
        (
            r#"let database: SqliteDatabase = sqlite.open_memory(0)?
    let values: Array<SqliteValue> = Array.new<SqliteValue>()
    let query_value: SqliteQuery = sqlite.query(database, "SELECT 1", values)?
    let value: Result<void, SqliteError> = sqlite.close(query_value)"#,
            "SqliteDatabase",
        ),
        (
            r#"let value: Result<SqliteQuery, SqliteError> = sqlite.open_memory(0)"#,
            "cannot initialize",
        ),
    ];

    for (body, expected) in cases {
        let body = body.replace("=\n        ", "= ");
        let source = format!(
            r#"package app.main

import std.array.Array
import std.sqlite

fn exercise() -> Result<void, SqliteError> {{
    {body}
    return Ok(void)
}}

fn main() -> void {{
    let result: Result<void, SqliteError> = exercise()
}}
"#
        );
        let error = parse_inline(&source).unwrap_err();
        assert!(
            error.message.contains(expected),
            "expected `{expected}` in diagnostic, got {}",
            error.message
        );
    }
}

#[test]
fn lowers_specifically_imported_sqlite_function() {
    let source = r#"package app.main

import std.sqlite.open_memory

fn main() -> void {
    let opened: Result<SqliteDatabase, SqliteError> = open_memory(0)
}
"#;

    let program = parse_inline(source).unwrap();
    assert!(format!("{:?}", program.functions).contains(BUILTIN_SQLITE_OPEN_MEMORY_EXPR));
}

#[test]
fn diagnoses_missing_sqlite_type_import() {
    let source = r#"package app.main

fn keep(database: SqliteDatabase) -> void {
}

fn main() -> void {
}
"#;
    let error = parse_inline(source).unwrap_err();
    assert_eq!(error.code, "E0301");
    assert_eq!(
        error.message,
        "`SqliteDatabase` requires `import std.sqlite`"
    );
}

#[test]
fn rejects_forged_sqlite_handles_and_field_access() {
    let forged = r#"package app.main

import std.sqlite

fn main() -> void {
    let database: SqliteDatabase = SqliteDatabase { handle: 1 }
}
"#;
    let error = parse_inline(forged).unwrap_err();
    assert_eq!(error.code, "E0830");
    assert!(error.message.contains("cannot be constructed"));

    let exposed = r#"package app.main

import std.io
import std.sqlite

fn main() -> Result<void, SqliteError> {
    let database: SqliteDatabase = sqlite.open_memory(0)?
    io.println(database.handle)
    return Ok(void)
}
"#;
    let error = parse_inline(exposed).unwrap_err();
    assert_eq!(error.code, "E0830");
    assert!(error.message.contains("does not expose its fields"));
}

#[test]
fn rejects_sqlite_operations_inside_isolated_tasks_without_leaking_values() {
    let source = r#"package app.main

import std.sqlite
import std.task

fn worker(context: TaskContext, input: string) -> string {
    let opened: Result<SqliteDatabase, SqliteError> = sqlite.open("secret-agent.db", SqliteOpenMode.ReadWriteCreate, 0)
    return input
}

fn main() -> void {
    let started: Result<Task, TaskError> = task.spawn(worker, "secret-token")
}
"#;

    let error = parse_inline(source).unwrap_err();
    assert_eq!(error.code, "E0821");
    assert!(error.message.contains("sqlite.open"));
    assert!(!error.message.contains("secret-agent.db"));
    assert!(!error.message.contains("secret-token"));
}

#[test]
fn rejects_transitive_sqlite_use_from_isolated_tasks() {
    let source = r#"package app.main

import std.sqlite
import std.task

fn load_state(input: string) -> string {
    let opened: Result<SqliteDatabase, SqliteError> = sqlite.open("transitive-secret.db", SqliteOpenMode.ReadOnly, 0)
    return input
}

fn worker(context: TaskContext, input: string) -> string {
    return load_state(input)
}

fn main() -> void {
    let started: Result<Task, TaskError> = task.spawn(worker, "transitive-token")
}
"#;

    let error = parse_inline(source).unwrap_err();
    assert_eq!(error.code, "E0821");
    assert!(error.message.contains("worker -> load_state"));
    assert!(error.message.contains("sqlite.open"));
    assert!(!error.message.contains("transitive-secret.db"));
    assert!(!error.message.contains("transitive-token"));
}

#[test]
fn emits_sqlite_runtime_and_shutdown_only_when_used() {
    let source = r#"package app.main

import std.sqlite

fn main() -> void {
    let opened: Result<SqliteDatabase, SqliteError> = sqlite.open_memory(0)
    return
}
"#;
    let generated = nomo_codegen_c::emit_c(&parse_inline(source).unwrap());
    assert!(generated.contains("#define NOMO_SQLITE_MAX_DATABASES 32"));
    assert!(generated.contains("nomo_fn___nomo_sqlite_open_memory"));
    assert!(generated.contains("nomo_fn_main();\n    nomo_sqlite_shutdown();"));
    assert!(!generated.contains("secret-token"));

    let plain = r#"package app.main

fn main() -> void {
}
"#;
    let generated = nomo_codegen_c::emit_c(&parse_inline(plain).unwrap());
    assert!(!generated.contains("NOMO_SQLITE_MAX_DATABASES"));
    assert!(!generated.contains("sqlite3.h"));
}
