use super::*;

pub(super) fn is_sqlite_builtin_call(callee: &[String]) -> bool {
    matches!(
        callee,
        [module, name]
            if module == "sqlite"
                && matches!(
                    name.as_str(),
                    "open"
                        | "open_memory"
                        | "execute"
                        | "query"
                        | "next"
                        | "reset"
                        | "close_query"
                        | "close"
                )
    )
}

pub(super) fn lower_sqlite_builtin(
    path: &Path,
    callee: &[String],
    args: &[AstExpr],
    scope: &HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    span: &Span,
) -> Result<(ValueType, ValueExpr), Diagnostic> {
    let [module, name] = callee else {
        unreachable!("sqlite builtin dispatcher only passes qualified calls")
    };
    debug_assert_eq!(module, "sqlite");

    let database = ValueType::Struct("SqliteDatabase".to_string(), Vec::new());
    let query = ValueType::Struct("SqliteQuery".to_string(), Vec::new());
    let error = ValueType::Struct("SqliteError".to_string(), Vec::new());
    let open_mode = ValueType::Enum("SqliteOpenMode".to_string(), Vec::new());
    let value = ValueType::Enum("SqliteValue".to_string(), Vec::new());
    let params = ValueType::Array(Box::new(value));
    let execute_result = ValueType::Struct("SqliteExecuteResult".to_string(), Vec::new());
    let row = ValueType::Struct("SqliteRow".to_string(), Vec::new());

    let result = |ok: ValueType| ValueType::Enum("Result".to_string(), vec![ok, error.clone()]);
    let lower =
        |arg: &AstExpr, expected: &ValueType| -> Result<(ValueType, ValueExpr), Diagnostic> {
            lower_value_expr_with_expected(
                path,
                arg,
                scope,
                imports,
                signatures,
                structs,
                enums,
                Some(expected),
                span,
            )
        };
    let checked =
        |label: &str, arg: &AstExpr, expected: &ValueType| -> Result<ValueExpr, Diagnostic> {
            let (actual, lowered) = lower(arg, expected)?;
            if actual == *expected {
                Ok(lowered)
            } else {
                Err(type_mismatch_expected_found(
                    path,
                    span,
                    format!("`sqlite.{name}` expects {label}"),
                    expected,
                    &actual,
                ))
            }
        };

    match name.as_str() {
        "open" => {
            let [path_arg, mode_arg, timeout_arg] = args else {
                return Err(sqlite_arity_diagnostic(
                    path,
                    span,
                    "sqlite.open",
                    3,
                    args.len(),
                ));
            };
            let path_value = checked("a string path", path_arg, &ValueType::String)?;
            let mode_value = checked("a SqliteOpenMode", mode_arg, &open_mode)?;
            let timeout = checked("a u64 busy timeout", timeout_arg, &ValueType::U64)?;
            Ok((
                result(database),
                ValueExpr::Call {
                    name: BUILTIN_SQLITE_OPEN_EXPR.to_string(),
                    args: vec![path_value, mode_value, timeout],
                },
            ))
        }
        "open_memory" => {
            let [timeout_arg] = args else {
                return Err(sqlite_arity_diagnostic(
                    path,
                    span,
                    "sqlite.open_memory",
                    1,
                    args.len(),
                ));
            };
            let timeout = checked("a u64 busy timeout", timeout_arg, &ValueType::U64)?;
            Ok((
                result(database),
                ValueExpr::Call {
                    name: BUILTIN_SQLITE_OPEN_MEMORY_EXPR.to_string(),
                    args: vec![timeout],
                },
            ))
        }
        "execute" | "query" => {
            let [database_arg, sql_arg, params_arg] = args else {
                return Err(sqlite_arity_diagnostic(
                    path,
                    span,
                    &format!("sqlite.{name}"),
                    3,
                    args.len(),
                ));
            };
            let database_value = checked("a SqliteDatabase", database_arg, &database)?;
            let sql = checked("a string SQL statement", sql_arg, &ValueType::String)?;
            let parameters = checked("an Array<SqliteValue>", params_arg, &params)?;
            let (ok_type, intrinsic) = if name == "execute" {
                (execute_result, BUILTIN_SQLITE_EXECUTE_EXPR)
            } else {
                (query, BUILTIN_SQLITE_QUERY_EXPR)
            };
            Ok((
                result(ok_type),
                ValueExpr::Call {
                    name: intrinsic.to_string(),
                    args: vec![database_value, sql, parameters],
                },
            ))
        }
        "next" => {
            let [query_arg, max_row_arg] = args else {
                return Err(sqlite_arity_diagnostic(
                    path,
                    span,
                    "sqlite.next",
                    2,
                    args.len(),
                ));
            };
            let query_value = checked("a SqliteQuery", query_arg, &query)?;
            let max_row = checked("a u64 row-byte limit", max_row_arg, &ValueType::U64)?;
            Ok((
                result(ValueType::Enum("Option".to_string(), vec![row])),
                ValueExpr::Call {
                    name: BUILTIN_SQLITE_NEXT_EXPR.to_string(),
                    args: vec![query_value, max_row],
                },
            ))
        }
        "reset" => {
            let [query_arg, params_arg] = args else {
                return Err(sqlite_arity_diagnostic(
                    path,
                    span,
                    "sqlite.reset",
                    2,
                    args.len(),
                ));
            };
            let query_value = checked("a SqliteQuery", query_arg, &query)?;
            let parameters = checked("an Array<SqliteValue>", params_arg, &params)?;
            Ok((
                result(ValueType::Void),
                ValueExpr::Call {
                    name: BUILTIN_SQLITE_RESET_EXPR.to_string(),
                    args: vec![query_value, parameters],
                },
            ))
        }
        "close_query" => {
            let [query_arg] = args else {
                return Err(sqlite_arity_diagnostic(
                    path,
                    span,
                    "sqlite.close_query",
                    1,
                    args.len(),
                ));
            };
            let query_value = checked("a SqliteQuery", query_arg, &query)?;
            Ok((
                result(ValueType::Void),
                ValueExpr::Call {
                    name: BUILTIN_SQLITE_CLOSE_QUERY_EXPR.to_string(),
                    args: vec![query_value],
                },
            ))
        }
        "close" => {
            let [database_arg] = args else {
                return Err(sqlite_arity_diagnostic(
                    path,
                    span,
                    "sqlite.close",
                    1,
                    args.len(),
                ));
            };
            let database_value = checked("a SqliteDatabase", database_arg, &database)?;
            Ok((
                result(ValueType::Void),
                ValueExpr::Call {
                    name: BUILTIN_SQLITE_CLOSE_EXPR.to_string(),
                    args: vec![database_value],
                },
            ))
        }
        _ => unreachable!("sqlite builtin matcher and lowering must stay aligned"),
    }
}

fn sqlite_arity_diagnostic(
    path: &Path,
    span: &Span,
    name: &str,
    expected: usize,
    actual: usize,
) -> Diagnostic {
    Diagnostic::new(
        "E0830",
        format!("`{name}` expects {expected} argument(s), got {actual}"),
        path,
        span.line,
        span.column,
        span.length,
        &span.text,
    )
}
