use super::*;

pub(super) fn is_cron_builtin_call(callee: &[String]) -> bool {
    matches!(
        callee,
        [module, name]
            if module == "cron"
                && matches!(name.as_str(), "parse" | "matches" | "next_after")
    )
}

#[allow(clippy::too_many_arguments)]
fn checked_cron_arg(
    path: &Path,
    arg: &AstExpr,
    expected: &ValueType,
    description: &str,
    scope: &HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    span: &Span,
) -> Result<ValueExpr, Diagnostic> {
    let (actual_type, value) =
        lower_value_expr(path, arg, scope, imports, signatures, structs, enums, span)?;
    if &actual_type != expected {
        return Err(type_mismatch_expected_found(
            path,
            span,
            format!("cron builtin expects {description}"),
            expected,
            &actual_type,
        ));
    }
    Ok(value)
}

pub(super) fn lower_cron_builtin(
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
        unreachable!("cron builtin dispatcher only passes qualified calls")
    };
    debug_assert_eq!(module, "cron");

    let schedule = ValueType::Struct("CronSchedule".to_string(), Vec::new());
    let error = ValueType::Struct("CronError".to_string(), Vec::new());
    let result = |ok: ValueType| ValueType::Enum("Result".to_string(), vec![ok, error.clone()]);
    let checked = |arg: &AstExpr, expected: &ValueType, description: &str| {
        checked_cron_arg(
            path,
            arg,
            expected,
            description,
            scope,
            imports,
            signatures,
            structs,
            enums,
            span,
        )
    };

    let (operation, lowered, return_type) = match name.as_str() {
        "parse" => {
            let [expression] = args else {
                return Err(cron_arity_error(path, span, name, 1));
            };
            (
                CronOperation::Parse,
                vec![checked(
                    expression,
                    &ValueType::String,
                    "a string expression",
                )?],
                result(schedule),
            )
        }
        "matches" | "next_after" => {
            let [schedule_value, unix_millis] = args else {
                return Err(cron_arity_error(path, span, name, 2));
            };
            (
                if name == "matches" {
                    CronOperation::Matches
                } else {
                    CronOperation::NextAfter
                },
                vec![
                    checked(schedule_value, &schedule, "a CronSchedule")?,
                    checked(unix_millis, &ValueType::Int, "an i64 Unix timestamp")?,
                ],
                result(if name == "matches" {
                    ValueType::Bool
                } else {
                    ValueType::Int
                }),
            )
        }
        _ => unreachable!("cron builtin dispatcher only passes known calls"),
    };

    Ok((
        return_type,
        ValueExpr::Cron {
            operation,
            args: lowered,
        },
    ))
}

fn cron_arity_error(path: &Path, span: &Span, name: &str, expected: usize) -> Diagnostic {
    Diagnostic::new(
        "E0407",
        format!("`cron.{name}` expects exactly {expected} argument(s)"),
        path,
        span.line,
        span.column,
        span.length,
        &span.text,
    )
}
