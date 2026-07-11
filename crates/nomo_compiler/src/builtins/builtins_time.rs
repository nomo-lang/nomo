use super::*;
pub(super) fn is_time_builtin_call(callee: &[String]) -> bool {
    matches!(
        callee,
        [module, name]
            if module == "time"
                && matches!(
                    name.as_str(),
                    "now_millis"
                        | "monotonic_millis"
                        | "duration_millis"
                        | "duration_seconds"
                        | "duration_as_millis"
                        | "format_duration"
                        | "sleep"
                        | "sleep_millis"
                )
    )
}
pub(super) fn lower_time_builtin(
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
        unreachable!("time builtin dispatcher only passes qualified calls")
    };
    debug_assert_eq!(module, "time");
    match name.as_str() {
        "now_millis" => {
            if !args.is_empty() {
                return Err(Diagnostic::new(
                    "E0407",
                    "`time.now_millis` does not accept arguments",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            }
            Ok((ValueType::Int, ValueExpr::TimeNowMillis))
        }
        "monotonic_millis" => {
            if !args.is_empty() {
                return Err(Diagnostic::new(
                    "E0407",
                    "`time.monotonic_millis` does not accept arguments",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            }
            Ok((ValueType::Int, ValueExpr::TimeMonotonicMillis))
        }
        "duration_millis" => {
            let [millis] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`time.duration_millis` expects exactly one i64 millisecond value",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (millis_type, lowered_millis) = lower_value_expr(
                path, millis, scope, imports, signatures, structs, enums, span,
            )?;
            if millis_type != ValueType::Int {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    "`time.duration_millis` expects an i64 millisecond value",
                    &ValueType::Int,
                    &millis_type,
                ));
            }
            Ok((
                ValueType::Struct("Duration".to_string(), Vec::new()),
                ValueExpr::TimeDurationMillis {
                    millis: Box::new(lowered_millis),
                },
            ))
        }
        "duration_seconds" => {
            let [seconds] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`time.duration_seconds` expects exactly one i64 second value",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (seconds_type, lowered_seconds) = lower_value_expr(
                path, seconds, scope, imports, signatures, structs, enums, span,
            )?;
            if seconds_type != ValueType::Int {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    "`time.duration_seconds` expects an i64 second value",
                    &ValueType::Int,
                    &seconds_type,
                ));
            }
            Ok((
                ValueType::Struct("Duration".to_string(), Vec::new()),
                ValueExpr::TimeDurationSeconds {
                    seconds: Box::new(lowered_seconds),
                },
            ))
        }
        "duration_as_millis" => {
            let [duration] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`time.duration_as_millis` expects exactly one Duration value",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (duration_type, lowered_duration) = lower_value_expr(
                path, duration, scope, imports, signatures, structs, enums, span,
            )?;
            let expected = ValueType::Struct("Duration".to_string(), Vec::new());
            if duration_type != expected {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    "`time.duration_as_millis` expects a Duration value",
                    &expected,
                    &duration_type,
                ));
            }
            Ok((
                ValueType::Int,
                ValueExpr::TimeDurationAsMillis {
                    duration: Box::new(lowered_duration),
                },
            ))
        }
        "format_duration" => {
            let [duration] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`time.format_duration` expects exactly one Duration value",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (duration_type, lowered_duration) = lower_value_expr(
                path, duration, scope, imports, signatures, structs, enums, span,
            )?;
            let expected = ValueType::Struct("Duration".to_string(), Vec::new());
            if duration_type != expected {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    "`time.format_duration` expects a Duration value",
                    &expected,
                    &duration_type,
                ));
            }
            Ok((
                ValueType::String,
                ValueExpr::TimeFormatDuration {
                    duration: Box::new(lowered_duration),
                },
            ))
        }
        "sleep" => {
            let [duration] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`time.sleep` expects exactly one Duration value",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (duration_type, lowered_duration) = lower_value_expr(
                path, duration, scope, imports, signatures, structs, enums, span,
            )?;
            let expected = ValueType::Struct("Duration".to_string(), Vec::new());
            if duration_type != expected {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    "`time.sleep` expects a Duration value",
                    &expected,
                    &duration_type,
                ));
            }
            Ok((
                ValueType::Void,
                ValueExpr::TimeSleep {
                    duration: Box::new(lowered_duration),
                },
            ))
        }
        "sleep_millis" => {
            let [duration] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`time.sleep_millis` expects exactly one i64 duration in milliseconds",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (duration_type, lowered_duration) = lower_value_expr(
                path, duration, scope, imports, signatures, structs, enums, span,
            )?;
            if duration_type != ValueType::Int {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    "`time.sleep_millis` expects an i64 duration in milliseconds",
                    &ValueType::Int,
                    &duration_type,
                ));
            }
            Ok((
                ValueType::Void,
                ValueExpr::TimeSleepMillis {
                    duration: Box::new(lowered_duration),
                },
            ))
        }
        _ => unreachable!("time builtin dispatcher only passes known calls"),
    }
}
