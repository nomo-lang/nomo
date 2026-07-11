use super::*;

pub(super) fn is_collections_builtin_call(callee: &[String]) -> bool {
    matches!(
        callee,
        [module, name]
            if module == "collections"
                && matches!(
                    name.as_str(),
                    "map_new"
                        | "map_len"
                        | "map_get"
                        | "map_contains"
                        | "map_set"
                        | "map_remove"
                        | "set_new"
                        | "set_len"
                        | "set_contains"
                        | "set_insert"
                        | "set_remove"
                )
    )
}

pub(super) fn lower_collections_builtin(
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
        unreachable!("collections builtin dispatcher only passes qualified calls")
    };
    debug_assert_eq!(module, "collections");
    let map_type = ValueType::Struct("StringMap".to_string(), Vec::new());
    let set_type = ValueType::Struct("StringSet".to_string(), Vec::new());
    match name.as_str() {
        "map_new" => {
            expect_no_args(path, span, "collections.map_new", args)?;
            Ok((map_type, ValueExpr::CollectionsStringMapNew))
        }
        "map_len" => {
            let map = lower_collections_unary_arg(
                path,
                span,
                "collections.map_len",
                args,
                &map_type,
                scope,
                imports,
                signatures,
                structs,
                enums,
            )?;
            Ok((
                ValueType::U64,
                ValueExpr::CollectionsStringMapLen { map: Box::new(map) },
            ))
        }
        "map_get" => {
            let (map, key) = lower_collections_map_key_args(
                path,
                span,
                "collections.map_get",
                args,
                scope,
                imports,
                signatures,
                structs,
                enums,
            )?;
            Ok((
                ValueType::Enum("Option".to_string(), vec![ValueType::String]),
                ValueExpr::CollectionsStringMapGet {
                    map: Box::new(map),
                    key: Box::new(key),
                },
            ))
        }
        "map_contains" => {
            let (map, key) = lower_collections_map_key_args(
                path,
                span,
                "collections.map_contains",
                args,
                scope,
                imports,
                signatures,
                structs,
                enums,
            )?;
            Ok((
                ValueType::Bool,
                ValueExpr::CollectionsStringMapContains {
                    map: Box::new(map),
                    key: Box::new(key),
                },
            ))
        }
        "map_set" => {
            let [map_arg, key_arg, value_arg] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`collections.map_set` expects a StringMap, string key, and string value",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let map = lower_collections_arg(
                path,
                span,
                "collections.map_set",
                map_arg,
                &map_type,
                scope,
                imports,
                signatures,
                structs,
                enums,
            )?;
            let key = lower_collections_arg(
                path,
                span,
                "collections.map_set",
                key_arg,
                &ValueType::String,
                scope,
                imports,
                signatures,
                structs,
                enums,
            )?;
            let value = lower_collections_arg(
                path,
                span,
                "collections.map_set",
                value_arg,
                &ValueType::String,
                scope,
                imports,
                signatures,
                structs,
                enums,
            )?;
            Ok((
                map_type,
                ValueExpr::CollectionsStringMapSet {
                    map: Box::new(map),
                    key: Box::new(key),
                    value: Box::new(value),
                },
            ))
        }
        "map_remove" => {
            let (map, key) = lower_collections_map_key_args(
                path,
                span,
                "collections.map_remove",
                args,
                scope,
                imports,
                signatures,
                structs,
                enums,
            )?;
            Ok((
                map_type,
                ValueExpr::CollectionsStringMapRemove {
                    map: Box::new(map),
                    key: Box::new(key),
                },
            ))
        }
        "set_new" => {
            expect_no_args(path, span, "collections.set_new", args)?;
            Ok((set_type, ValueExpr::CollectionsStringSetNew))
        }
        "set_len" => {
            let set = lower_collections_unary_arg(
                path,
                span,
                "collections.set_len",
                args,
                &set_type,
                scope,
                imports,
                signatures,
                structs,
                enums,
            )?;
            Ok((
                ValueType::U64,
                ValueExpr::CollectionsStringSetLen { set: Box::new(set) },
            ))
        }
        "set_contains" => {
            let (set, value) = lower_collections_set_value_args(
                path,
                span,
                "collections.set_contains",
                args,
                scope,
                imports,
                signatures,
                structs,
                enums,
            )?;
            Ok((
                ValueType::Bool,
                ValueExpr::CollectionsStringSetContains {
                    set: Box::new(set),
                    value: Box::new(value),
                },
            ))
        }
        "set_insert" => {
            let (set, value) = lower_collections_set_value_args(
                path,
                span,
                "collections.set_insert",
                args,
                scope,
                imports,
                signatures,
                structs,
                enums,
            )?;
            Ok((
                set_type,
                ValueExpr::CollectionsStringSetInsert {
                    set: Box::new(set),
                    value: Box::new(value),
                },
            ))
        }
        "set_remove" => {
            let (set, value) = lower_collections_set_value_args(
                path,
                span,
                "collections.set_remove",
                args,
                scope,
                imports,
                signatures,
                structs,
                enums,
            )?;
            Ok((
                set_type,
                ValueExpr::CollectionsStringSetRemove {
                    set: Box::new(set),
                    value: Box::new(value),
                },
            ))
        }
        _ => unreachable!("collections builtin dispatcher only passes known calls"),
    }
}

fn expect_no_args(
    path: &Path,
    span: &Span,
    callable: &str,
    args: &[AstExpr],
) -> Result<(), Diagnostic> {
    if args.is_empty() {
        return Ok(());
    }
    Err(Diagnostic::new(
        "E0407",
        format!("`{callable}` does not accept arguments"),
        path,
        span.line,
        span.column,
        span.length,
        &span.text,
    ))
}

#[allow(clippy::too_many_arguments)]
fn lower_collections_unary_arg(
    path: &Path,
    span: &Span,
    callable: &str,
    args: &[AstExpr],
    expected: &ValueType,
    scope: &HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
) -> Result<ValueExpr, Diagnostic> {
    let [arg] = args else {
        return Err(Diagnostic::new(
            "E0407",
            format!("`{callable}` expects exactly one argument"),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    };
    lower_collections_arg(
        path, span, callable, arg, expected, scope, imports, signatures, structs, enums,
    )
}

#[allow(clippy::too_many_arguments)]
fn lower_collections_map_key_args(
    path: &Path,
    span: &Span,
    callable: &str,
    args: &[AstExpr],
    scope: &HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
) -> Result<(ValueExpr, ValueExpr), Diagnostic> {
    let [map_arg, key_arg] = args else {
        return Err(Diagnostic::new(
            "E0407",
            format!("`{callable}` expects a StringMap and string key"),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    };
    let map_type = ValueType::Struct("StringMap".to_string(), Vec::new());
    let map = lower_collections_arg(
        path, span, callable, map_arg, &map_type, scope, imports, signatures, structs, enums,
    )?;
    let key = lower_collections_arg(
        path,
        span,
        callable,
        key_arg,
        &ValueType::String,
        scope,
        imports,
        signatures,
        structs,
        enums,
    )?;
    Ok((map, key))
}

#[allow(clippy::too_many_arguments)]
fn lower_collections_set_value_args(
    path: &Path,
    span: &Span,
    callable: &str,
    args: &[AstExpr],
    scope: &HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
) -> Result<(ValueExpr, ValueExpr), Diagnostic> {
    let [set_arg, value_arg] = args else {
        return Err(Diagnostic::new(
            "E0407",
            format!("`{callable}` expects a StringSet and string value"),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    };
    let set_type = ValueType::Struct("StringSet".to_string(), Vec::new());
    let set = lower_collections_arg(
        path, span, callable, set_arg, &set_type, scope, imports, signatures, structs, enums,
    )?;
    let value = lower_collections_arg(
        path,
        span,
        callable,
        value_arg,
        &ValueType::String,
        scope,
        imports,
        signatures,
        structs,
        enums,
    )?;
    Ok((set, value))
}

#[allow(clippy::too_many_arguments)]
fn lower_collections_arg(
    path: &Path,
    span: &Span,
    callable: &str,
    arg: &AstExpr,
    expected: &ValueType,
    scope: &HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
) -> Result<ValueExpr, Diagnostic> {
    let (actual, lowered) = lower_value_expr_with_expected(
        path,
        arg,
        scope,
        imports,
        signatures,
        structs,
        enums,
        Some(expected),
        span,
    )?;
    if &actual != expected {
        return Err(type_mismatch_expected_found(
            path,
            span,
            format!(
                "`{callable}` argument is `{}` but expected `{}`",
                actual.name(),
                expected.name()
            ),
            expected,
            &actual,
        ));
    }
    Ok(lowered)
}
