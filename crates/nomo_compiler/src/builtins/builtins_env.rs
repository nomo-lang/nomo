use super::*;
pub(super) fn lower_env_builtin(
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
    match callee {
        [module, name] if module == "env" && name == "get" => {
            let [name_arg] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`env.get` expects exactly one environment variable name",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (name_type, lowered_name) = lower_value_expr(
                path, name_arg, scope, imports, signatures, structs, enums, span,
            )?;
            if name_type != ValueType::String {
                return Err(type_mismatch(
                    path,
                    span,
                    "`env.get` expects a string variable name",
                ));
            }
            Ok((
                ValueType::Enum("Option".to_string(), vec![ValueType::String]),
                ValueExpr::EnvGet {
                    name: Box::new(lowered_name),
                },
            ))
        }
        [module, name] if module == "env" && name == "set" => {
            let [name_arg, value_arg] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`env.set` expects exactly a name and value string",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (name_type, lowered_name) = lower_value_expr(
                path, name_arg, scope, imports, signatures, structs, enums, span,
            )?;
            let (value_type, lowered_value) = lower_value_expr(
                path, value_arg, scope, imports, signatures, structs, enums, span,
            )?;
            if name_type != ValueType::String || value_type != ValueType::String {
                return Err(type_mismatch(
                    path,
                    span,
                    "`env.set` expects two string arguments",
                ));
            }
            Ok((
                ValueType::Void,
                ValueExpr::EnvSet {
                    name: Box::new(lowered_name),
                    value: Box::new(lowered_value),
                },
            ))
        }
        [module, name] if module == "env" && name == "cwd" => {
            if !args.is_empty() {
                return Err(Diagnostic::new(
                    "E0407",
                    "`env.cwd` does not accept arguments",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            }
            Ok((ValueType::String, ValueExpr::EnvCwd))
        }
        [module, name] if module == "env" && name == "home_dir" => {
            if !args.is_empty() {
                return Err(Diagnostic::new(
                    "E0407",
                    "`env.home_dir` does not accept arguments",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            }
            Ok((
                ValueType::Enum("Option".to_string(), vec![ValueType::String]),
                ValueExpr::EnvHomeDir,
            ))
        }
        [module, name] if module == "env" && name == "temp_dir" => {
            if !args.is_empty() {
                return Err(Diagnostic::new(
                    "E0407",
                    "`env.temp_dir` does not accept arguments",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            }
            Ok((ValueType::String, ValueExpr::EnvTempDir))
        }
        [module, name] if module == "env" && name == "args" => {
            if !args.is_empty() {
                return Err(Diagnostic::new(
                    "E0407",
                    "`env.args` does not accept arguments",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            }
            Ok((
                ValueType::Array(Box::new(ValueType::String)),
                ValueExpr::EnvArgs,
            ))
        }
        _ => unreachable!("env builtin dispatcher only passes known calls"),
    }
}
pub(super) fn is_env_builtin_call(callee: &[String]) -> bool {
    matches!(
        callee,
        [module, name]
            if module == "env"
                && matches!(
                    name.as_str(),
                    "args" | "get" | "set" | "cwd" | "home_dir" | "temp_dir"
                )
    )
}
