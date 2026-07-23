use super::*;

const DISPLAY_INTERFACE: &str = "Display";
const DEBUG_INTERFACE: &str = "Debug";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FormatMode {
    Display,
    Debug,
}

impl FormatMode {
    fn interface(self) -> &'static str {
        match self {
            Self::Display => DISPLAY_INTERFACE,
            Self::Debug => DEBUG_INTERFACE,
        }
    }

    fn method(self) -> &'static str {
        match self {
            Self::Display => "to_string",
            Self::Debug => "debug_string",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum FormatPart {
    Literal(String),
    Placeholder(FormatMode),
}

pub(super) fn is_fmt_builtin_call(callee: &[String]) -> bool {
    matches!(
        callee,
        [module, name]
            if module == "fmt"
                && matches!(name.as_str(), "to_string" | "debug_string" | "format")
    )
}

pub(super) fn fmt_interface_impl_marker(interface: &str, owner: &str) -> String {
    format!("__nomo_fmt_impl::{interface}::{owner}")
}

#[allow(clippy::too_many_arguments)]
pub(super) fn lower_fmt_builtin(
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
    let function_name = callee
        .get(1)
        .expect("fmt builtin calls contain a function name");
    match function_name.as_str() {
        "to_string" | "debug_string" => {
            if args.len() != 1 {
                return Err(fmt_diagnostic(
                    path,
                    span,
                    format!(
                        "`fmt.{function_name}` expects exactly one value, got {}",
                        args.len()
                    ),
                ));
            }
            let mode = if function_name == "to_string" {
                FormatMode::Display
            } else {
                FormatMode::Debug
            };
            let value = lower_formatted_value(
                path, &args[0], mode, scope, imports, signatures, structs, enums, span,
            )?;
            Ok((ValueType::String, value))
        }
        "format" => {
            lower_format_template(path, args, scope, imports, signatures, structs, enums, span)
        }
        _ => unreachable!("fmt builtin dispatcher only passes known functions"),
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn lower_joined_display_args(
    path: &Path,
    args: &[AstExpr],
    scope: &HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    span: &Span,
) -> Result<ValueExpr, Diagnostic> {
    let mut rendered = Vec::with_capacity(args.len());
    for arg in args {
        rendered.push(lower_formatted_value(
            path,
            arg,
            FormatMode::Display,
            scope,
            imports,
            signatures,
            structs,
            enums,
            span,
        )?);
    }
    Ok(join_string_values(rendered, " "))
}

#[allow(clippy::too_many_arguments)]
fn lower_format_template(
    path: &Path,
    args: &[AstExpr],
    scope: &HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    span: &Span,
) -> Result<(ValueType, ValueExpr), Diagnostic> {
    let Some(AstExpr::String(template)) = args.first() else {
        return Err(fmt_diagnostic(
            path,
            span,
            "`fmt.format` requires a compile-time string literal template",
        ));
    };
    let parts =
        parse_format_template(template).map_err(|message| fmt_diagnostic(path, span, message))?;
    let placeholder_count = parts
        .iter()
        .filter(|part| matches!(part, FormatPart::Placeholder(_)))
        .count();
    let value_count = args.len().saturating_sub(1);
    if placeholder_count != value_count {
        return Err(fmt_diagnostic(
            path,
            span,
            format!("format template expects {placeholder_count} value(s), got {value_count}"),
        ));
    }

    let mut values = args[1..].iter();
    let mut lowered = Vec::with_capacity(parts.len());
    for part in parts {
        match part {
            FormatPart::Literal(value) if !value.is_empty() => {
                lowered.push(ValueExpr::StringLiteral(value));
            }
            FormatPart::Literal(_) => {}
            FormatPart::Placeholder(mode) => {
                let value = values
                    .next()
                    .expect("placeholder count is checked before lowering");
                lowered.push(lower_formatted_value(
                    path, value, mode, scope, imports, signatures, structs, enums, span,
                )?);
            }
        }
    }
    Ok((ValueType::String, join_string_values(lowered, "")))
}

#[allow(clippy::too_many_arguments)]
fn lower_formatted_value(
    path: &Path,
    value: &AstExpr,
    mode: FormatMode,
    scope: &HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    span: &Span,
) -> Result<ValueExpr, Diagnostic> {
    let (value_type, value) = lower_value_expr(
        path, value, scope, imports, signatures, structs, enums, span,
    )?;
    match value_type {
        ValueType::String => Ok(value),
        value_type if value_type.is_numeric() => Ok(ValueExpr::NumToString {
            value: Box::new(value),
            value_type,
        }),
        ValueType::Char => Ok(ValueExpr::CharToString {
            value: Box::new(value),
        }),
        ValueType::Bool => Ok(ValueExpr::If {
            condition: Box::new(value),
            then_branch: Box::new(ValueExpr::StringLiteral("true".to_string())),
            else_branch: Box::new(ValueExpr::StringLiteral("false".to_string())),
        }),
        ValueType::Struct(owner, args) if args.is_empty() => {
            let marker = fmt_interface_impl_marker(mode.interface(), &owner);
            if !signatures.contains_key(&marker) && !owner.starts_with("__nomo_bound_") {
                return Err(unformattable_type_diagnostic(
                    path,
                    span,
                    &ValueType::Struct(owner, args),
                    mode,
                ));
            }
            let method_name = method_internal_name(&owner, mode.method());
            let Some(signature) = signatures.get(&method_name) else {
                return Err(unformattable_type_diagnostic(
                    path,
                    span,
                    &ValueType::Struct(owner, args),
                    mode,
                ));
            };
            if signature.params.len() != 1 || signature.return_type != ValueType::String {
                return Err(unformattable_type_diagnostic(
                    path,
                    span,
                    &ValueType::Struct(owner, args),
                    mode,
                ));
            }
            Ok(ValueExpr::Call {
                name: method_name,
                args: vec![value],
            })
        }
        other => Err(unformattable_type_diagnostic(path, span, &other, mode)),
    }
}

fn parse_format_template(template: &str) -> Result<Vec<FormatPart>, String> {
    let mut parts = Vec::new();
    let mut literal = String::new();
    let mut chars = template.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '{' => match chars.next() {
                Some('{') => literal.push('{'),
                Some('}') => {
                    if !literal.is_empty() {
                        parts.push(FormatPart::Literal(std::mem::take(&mut literal)));
                    }
                    parts.push(FormatPart::Placeholder(FormatMode::Display));
                }
                Some(':') => {
                    if chars.next() != Some('?') || chars.next() != Some('}') {
                        return Err(
                            "unsupported format placeholder; use `{}`, `{:?}`, `{{`, or `}}`"
                                .to_string(),
                        );
                    }
                    if !literal.is_empty() {
                        parts.push(FormatPart::Literal(std::mem::take(&mut literal)));
                    }
                    parts.push(FormatPart::Placeholder(FormatMode::Debug));
                }
                _ => {
                    return Err(
                        "unterminated format placeholder; use `{}`, `{:?}`, `{{`, or `}}`"
                            .to_string(),
                    );
                }
            },
            '}' => {
                if chars.peek() == Some(&'}') {
                    chars.next();
                    literal.push('}');
                } else {
                    return Err(
                        "unmatched `}` in format template; write `}}` for a literal brace"
                            .to_string(),
                    );
                }
            }
            other => literal.push(other),
        }
    }
    if !literal.is_empty() {
        parts.push(FormatPart::Literal(literal));
    }
    Ok(parts)
}

fn join_string_values(values: Vec<ValueExpr>, separator: &str) -> ValueExpr {
    let mut values = values.into_iter();
    let Some(first) = values.next() else {
        return ValueExpr::StringLiteral(String::new());
    };
    values.fold(first, |left, right| {
        let left = if separator.is_empty() {
            left
        } else {
            ValueExpr::StringConcat {
                left: Box::new(left),
                right: Box::new(ValueExpr::StringLiteral(separator.to_string())),
            }
        };
        ValueExpr::StringConcat {
            left: Box::new(left),
            right: Box::new(right),
        }
    })
}

fn fmt_diagnostic(path: &Path, span: &Span, message: impl Into<String>) -> Diagnostic {
    Diagnostic::new(
        "E0408",
        message,
        path,
        span.line,
        span.column,
        span.length,
        &span.text,
    )
}

fn unformattable_type_diagnostic(
    path: &Path,
    span: &Span,
    value_type: &ValueType,
    mode: FormatMode,
) -> Diagnostic {
    Diagnostic::new(
        "E0402",
        format!(
            "type `{}` cannot be formatted with `{}`; implement `std.fmt.{}`",
            value_type.name(),
            mode.method(),
            mode.interface()
        ),
        path,
        span.line,
        span.column,
        span.length,
        &span.text,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_display_debug_and_escaped_braces() {
        assert_eq!(
            parse_format_template("{{ {} {:?} }}").unwrap(),
            vec![
                FormatPart::Literal("{ ".to_string()),
                FormatPart::Placeholder(FormatMode::Display),
                FormatPart::Literal(" ".to_string()),
                FormatPart::Placeholder(FormatMode::Debug),
                FormatPart::Literal(" }".to_string()),
            ]
        );
    }

    #[test]
    fn rejects_invalid_templates() {
        for template in ["{", "{name}", "{:x}", "}"] {
            assert!(parse_format_template(template).is_err(), "{template}");
        }
    }
}
