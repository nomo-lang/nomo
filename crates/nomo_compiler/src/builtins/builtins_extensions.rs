use super::*;

pub(super) fn is_crypto_builtin_call(callee: &[String]) -> bool {
    matches!(
        callee,
        [module, name]
            if module == "crypto" && matches!(name.as_str(), "sha256" | "sha512" | "random_bytes")
    )
}

pub(super) fn is_json_builtin_call(callee: &[String]) -> bool {
    matches!(
        callee,
        [module, name]
            if module == "json"
                && matches!(
                    name.as_str(),
                    "parse"
                        | "stringify"
                        | "kind"
                        | "is_null"
                        | "as_bool"
                        | "number_text"
                        | "as_string"
                        | "array_items"
                        | "object_members"
                        | "get"
                        | "from_null"
                        | "from_bool"
                        | "from_number_text"
                        | "from_i64"
                        | "from_u64"
                        | "from_string"
                        | "from_array"
                        | "from_object"
                )
    )
}

pub(super) fn is_net_builtin_call(callee: &[String]) -> bool {
    matches!(
        callee,
        [module, name]
            if module == "net"
                && matches!(
                    name.as_str(),
                    "connect" | "connect_blocking" | "listen" | "udp_bind"
                )
    )
}

pub(super) fn is_regex_builtin_call(callee: &[String]) -> bool {
    matches!(
        callee,
        [module, name]
            if module == "regex" && matches!(name.as_str(), "compile" | "is_match" | "captures")
    )
}

pub(super) fn lower_crypto_builtin(
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
        unreachable!("crypto builtin dispatcher only passes qualified calls")
    };
    debug_assert_eq!(module, "crypto");
    if name == "random_bytes" {
        let [count_arg] = args else {
            return Err(Diagnostic::new(
                "E0407",
                "`crypto.random_bytes` expects exactly one u64 count",
                path,
                span.line,
                span.column,
                span.length,
                &span.text,
            ));
        };
        let (count_type, count) = lower_value_expr(
            path, count_arg, scope, imports, signatures, structs, enums, span,
        )?;
        if count_type != ValueType::U64 {
            return Err(type_mismatch_expected_found(
                path,
                span,
                "`crypto.random_bytes` expects a u64 count",
                &ValueType::U64,
                &count_type,
            ));
        }
        return Ok((
            ValueType::Array(Box::new(ValueType::U32)),
            ValueExpr::CryptoRandomBytes {
                count: Box::new(count),
            },
        ));
    }
    let [value_arg] = args else {
        return Err(Diagnostic::new(
            "E0407",
            format!("`crypto.{name}` expects exactly one string value"),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    };
    let (value_type, value) = lower_value_expr(
        path, value_arg, scope, imports, signatures, structs, enums, span,
    )?;
    if value_type != ValueType::String {
        return Err(type_mismatch_expected_found(
            path,
            span,
            format!("`crypto.{name}` expects a string value"),
            &ValueType::String,
            &value_type,
        ));
    }
    let expr = match name.as_str() {
        "sha256" => ValueExpr::CryptoSha256 {
            value: Box::new(value),
        },
        "sha512" => ValueExpr::CryptoSha512 {
            value: Box::new(value),
        },
        _ => unreachable!("crypto builtin dispatcher only passes known calls"),
    };
    Ok((ValueType::String, expr))
}

pub(super) fn lower_json_builtin(
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
        unreachable!("json builtin dispatcher only passes qualified calls")
    };
    debug_assert_eq!(module, "json");
    let json_value = ValueType::Struct("JsonValue".to_string(), Vec::new());
    let json_error = ValueType::Struct("JsonError".to_string(), Vec::new());
    let json_member = ValueType::Struct("JsonMember".to_string(), Vec::new());
    let json_result = ValueType::Enum(
        "Result".to_string(),
        vec![json_value.clone(), json_error.clone()],
    );
    let option = |value_type: ValueType| ValueType::Enum("Option".to_string(), vec![value_type]);

    if name == "from_null" {
        if !args.is_empty() {
            return Err(Diagnostic::new(
                "E0407",
                "`json.from_null` expects no arguments",
                path,
                span.line,
                span.column,
                span.length,
                &span.text,
            ));
        }
        return Ok((
            json_value,
            ValueExpr::JsonStructured {
                operation: JsonOperation::FromNull,
                args: Vec::new(),
            },
        ));
    }

    if name == "get" {
        let [value_arg, key_arg] = args else {
            return Err(Diagnostic::new(
                "E0407",
                "`json.get` expects a JsonValue and string key",
                path,
                span.line,
                span.column,
                span.length,
                &span.text,
            ));
        };
        let (value_type, value) = lower_value_expr(
            path, value_arg, scope, imports, signatures, structs, enums, span,
        )?;
        if value_type != json_value {
            return Err(type_mismatch_expected_found(
                path,
                span,
                "`json.get` expects a JsonValue first argument",
                &json_value,
                &value_type,
            ));
        }
        let (key_type, key) = lower_value_expr(
            path, key_arg, scope, imports, signatures, structs, enums, span,
        )?;
        if key_type != ValueType::String {
            return Err(type_mismatch_expected_found(
                path,
                span,
                "`json.get` expects a string key",
                &ValueType::String,
                &key_type,
            ));
        }
        return Ok((
            option(json_value),
            ValueExpr::JsonStructured {
                operation: JsonOperation::Get,
                args: vec![value, key],
            },
        ));
    }

    let [value_arg] = args else {
        return Err(Diagnostic::new(
            "E0407",
            format!("`json.{name}` expects exactly one argument"),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    };
    let (value_type, value) = lower_value_expr(
        path, value_arg, scope, imports, signatures, structs, enums, span,
    )?;
    match name.as_str() {
        "parse" => {
            if value_type != ValueType::String {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    "`json.parse` expects a string value",
                    &ValueType::String,
                    &value_type,
                ));
            }
            Ok((
                json_result,
                ValueExpr::JsonParse {
                    value: Box::new(value),
                },
            ))
        }
        "stringify" => {
            if value_type != json_value {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    "`json.stringify` expects a JsonValue value",
                    &json_value,
                    &value_type,
                ));
            }
            Ok((
                ValueType::String,
                ValueExpr::JsonStringify {
                    value: Box::new(value),
                },
            ))
        }
        "kind" | "is_null" | "as_bool" | "number_text" | "as_string" | "array_items"
        | "object_members" => {
            if value_type != json_value {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    format!("`json.{name}` expects a JsonValue value"),
                    &json_value,
                    &value_type,
                ));
            }
            match name.as_str() {
                "kind" => Ok((
                    ValueType::Enum("JsonKind".to_string(), Vec::new()),
                    ValueExpr::JsonStructured {
                        operation: JsonOperation::Kind,
                        args: vec![value],
                    },
                )),
                "is_null" => Ok((
                    ValueType::Bool,
                    ValueExpr::JsonStructured {
                        operation: JsonOperation::IsNull,
                        args: vec![value],
                    },
                )),
                "as_bool" => Ok((
                    option(ValueType::Bool),
                    ValueExpr::JsonStructured {
                        operation: JsonOperation::AsBool,
                        args: vec![value],
                    },
                )),
                "number_text" => Ok((
                    option(ValueType::String),
                    ValueExpr::JsonStructured {
                        operation: JsonOperation::NumberText,
                        args: vec![value],
                    },
                )),
                "as_string" => Ok((
                    option(ValueType::String),
                    ValueExpr::JsonStructured {
                        operation: JsonOperation::AsString,
                        args: vec![value],
                    },
                )),
                "array_items" => Ok((
                    option(ValueType::Array(Box::new(json_value))),
                    ValueExpr::JsonStructured {
                        operation: JsonOperation::ArrayItems,
                        args: vec![value],
                    },
                )),
                "object_members" => Ok((
                    option(ValueType::Array(Box::new(json_member))),
                    ValueExpr::JsonStructured {
                        operation: JsonOperation::ObjectMembers,
                        args: vec![value],
                    },
                )),
                _ => unreachable!("matched structured JSON accessor"),
            }
        }
        "from_bool" => {
            if value_type != ValueType::Bool {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    "`json.from_bool` expects a bool value",
                    &ValueType::Bool,
                    &value_type,
                ));
            }
            Ok((
                json_value,
                ValueExpr::JsonStructured {
                    operation: JsonOperation::FromBool,
                    args: vec![value],
                },
            ))
        }
        "from_number_text" | "from_string" => {
            if value_type != ValueType::String {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    format!("`json.{name}` expects a string value"),
                    &ValueType::String,
                    &value_type,
                ));
            }
            let expression = if name == "from_number_text" {
                ValueExpr::JsonStructured {
                    operation: JsonOperation::FromNumberText,
                    args: vec![value],
                }
            } else {
                ValueExpr::JsonStructured {
                    operation: JsonOperation::FromString,
                    args: vec![value],
                }
            };
            Ok((json_result, expression))
        }
        "from_i64" => {
            if value_type != ValueType::Int {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    "`json.from_i64` expects an i64 value",
                    &ValueType::Int,
                    &value_type,
                ));
            }
            Ok((
                json_value,
                ValueExpr::JsonStructured {
                    operation: JsonOperation::FromI64,
                    args: vec![value],
                },
            ))
        }
        "from_u64" => {
            if value_type != ValueType::U64 {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    "`json.from_u64` expects a u64 value",
                    &ValueType::U64,
                    &value_type,
                ));
            }
            Ok((
                json_value,
                ValueExpr::JsonStructured {
                    operation: JsonOperation::FromU64,
                    args: vec![value],
                },
            ))
        }
        "from_array" => {
            let expected = ValueType::Array(Box::new(json_value));
            if value_type != expected {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    "`json.from_array` expects Array<JsonValue>",
                    &expected,
                    &value_type,
                ));
            }
            Ok((
                json_result,
                ValueExpr::JsonStructured {
                    operation: JsonOperation::FromArray,
                    args: vec![value],
                },
            ))
        }
        "from_object" => {
            let expected = ValueType::Array(Box::new(json_member));
            if value_type != expected {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    "`json.from_object` expects Array<JsonMember>",
                    &expected,
                    &value_type,
                ));
            }
            Ok((
                json_result,
                ValueExpr::JsonStructured {
                    operation: JsonOperation::FromObject,
                    args: vec![value],
                },
            ))
        }
        _ => unreachable!("json builtin dispatcher only passes known calls"),
    }
}

pub(super) fn lower_net_builtin(
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
        unreachable!("net builtin dispatcher only passes qualified calls")
    };
    debug_assert_eq!(module, "net");
    let net_error = ValueType::Struct("NetError".to_string(), Vec::new());
    match name.as_str() {
        "connect" => {
            if !current_function_is_suspend(scope) {
                return Err(Diagnostic::new(
                    "E0870",
                    "synchronous function cannot call suspend function `net.connect`; mark the caller `suspend`",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            }
            let [host_arg, port_arg, timeout_arg] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`net.connect` expects host, port, and timeout_millis arguments",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (host_type, host) = lower_value_expr_with_expected(
                path,
                host_arg,
                scope,
                imports,
                signatures,
                structs,
                enums,
                Some(&ValueType::String),
                span,
            )?;
            if host_type != ValueType::String {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    "`net.connect` expects a string host",
                    &ValueType::String,
                    &host_type,
                ));
            }
            let (port_type, port) = lower_value_expr_with_expected(
                path,
                port_arg,
                scope,
                imports,
                signatures,
                structs,
                enums,
                Some(&ValueType::Int),
                span,
            )?;
            if port_type != ValueType::Int {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    "`net.connect` expects an i64 port",
                    &ValueType::Int,
                    &port_type,
                ));
            }
            let (timeout_type, timeout) = lower_value_expr_with_expected(
                path,
                timeout_arg,
                scope,
                imports,
                signatures,
                structs,
                enums,
                Some(&ValueType::U64),
                span,
            )?;
            if timeout_type != ValueType::U64 {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    "`net.connect` expects a u64 timeout_millis",
                    &ValueType::U64,
                    &timeout_type,
                ));
            }
            Ok((
                ValueType::Enum(
                    "Result".to_string(),
                    vec![
                        ValueType::Struct("TcpStream".to_string(), Vec::new()),
                        net_error,
                    ],
                ),
                ValueExpr::Call {
                    name: BUILTIN_NET_CONNECT_EXPR.to_string(),
                    args: vec![host, port, timeout],
                },
            ))
        }
        "connect_blocking" | "listen" | "udp_bind" => {
            let [host_arg, port_arg] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    format!("`net.{name}` expects host and port arguments"),
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (host_type, host) = lower_value_expr_with_expected(
                path,
                host_arg,
                scope,
                imports,
                signatures,
                structs,
                enums,
                Some(&ValueType::String),
                span,
            )?;
            if host_type != ValueType::String {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    format!("`net.{name}` expects a string host"),
                    &ValueType::String,
                    &host_type,
                ));
            }
            let (port_type, port) = lower_value_expr_with_expected(
                path,
                port_arg,
                scope,
                imports,
                signatures,
                structs,
                enums,
                Some(&ValueType::Int),
                span,
            )?;
            if port_type != ValueType::Int {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    format!("`net.{name}` expects an i64 port"),
                    &ValueType::Int,
                    &port_type,
                ));
            }
            let ok_type = if name == "connect_blocking" {
                ValueType::Struct("TcpStream".to_string(), Vec::new())
            } else if name == "listen" {
                ValueType::Struct("TcpListener".to_string(), Vec::new())
            } else {
                ValueType::Struct("UdpSocket".to_string(), Vec::new())
            };
            let result_type = ValueType::Enum("Result".to_string(), vec![ok_type, net_error]);
            let expr = if name == "connect_blocking" {
                ValueExpr::NetConnect {
                    host: Box::new(host),
                    port: Box::new(port),
                }
            } else if name == "listen" {
                ValueExpr::NetListen {
                    host: Box::new(host),
                    port: Box::new(port),
                }
            } else {
                ValueExpr::NetUdpBind {
                    host: Box::new(host),
                    port: Box::new(port),
                }
            };
            Ok((result_type, expr))
        }
        _ => unreachable!("net builtin dispatcher only passes known calls"),
    }
}

pub(super) fn lower_regex_builtin(
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
        unreachable!("regex builtin dispatcher only passes qualified calls")
    };
    debug_assert_eq!(module, "regex");
    let regex_type = ValueType::Struct("Regex".to_string(), Vec::new());
    let regex_error = ValueType::Struct("RegexError".to_string(), Vec::new());
    match name.as_str() {
        "compile" => {
            let [pattern_arg] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`regex.compile` expects exactly one string pattern",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (pattern_type, pattern) = lower_value_expr(
                path,
                pattern_arg,
                scope,
                imports,
                signatures,
                structs,
                enums,
                span,
            )?;
            if pattern_type != ValueType::String {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    "`regex.compile` expects a string pattern",
                    &ValueType::String,
                    &pattern_type,
                ));
            }
            Ok((
                ValueType::Enum("Result".to_string(), vec![regex_type.clone(), regex_error]),
                ValueExpr::RegexCompile {
                    pattern: Box::new(pattern),
                },
            ))
        }
        "is_match" | "captures" => {
            let [regex_arg, value_arg] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    format!("`regex.{name}` expects a Regex and string value"),
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (actual_regex_type, regex) = lower_value_expr(
                path, regex_arg, scope, imports, signatures, structs, enums, span,
            )?;
            if actual_regex_type != regex_type {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    format!("`regex.{name}` expects a Regex value"),
                    &regex_type,
                    &actual_regex_type,
                ));
            }
            let (value_type, value) = lower_value_expr(
                path, value_arg, scope, imports, signatures, structs, enums, span,
            )?;
            if value_type != ValueType::String {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    format!("`regex.{name}` expects a string value"),
                    &ValueType::String,
                    &value_type,
                ));
            }
            if name == "is_match" {
                Ok((
                    ValueType::Bool,
                    ValueExpr::RegexIsMatch {
                        regex: Box::new(regex),
                        value: Box::new(value),
                    },
                ))
            } else {
                Ok((
                    ValueType::Enum(
                        "Option".to_string(),
                        vec![ValueType::Array(Box::new(ValueType::String))],
                    ),
                    ValueExpr::RegexCaptures {
                        regex: Box::new(regex),
                        value: Box::new(value),
                    },
                ))
            }
        }
        _ => unreachable!("regex builtin dispatcher only passes known calls"),
    }
}
