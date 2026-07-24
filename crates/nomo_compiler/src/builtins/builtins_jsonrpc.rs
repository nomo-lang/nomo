use super::*;

pub(super) fn is_jsonrpc_builtin_call(callee: &[String]) -> bool {
    matches!(
        callee,
        [module, name]
            if module == "jsonrpc"
                && matches!(
                    name.as_str(),
                    "decoder"
                        | "feed"
                        | "finish"
                        | "parse"
                        | "encode"
                        | "value"
                        | "kind"
                        | "request"
                        | "notification"
                        | "success"
                        | "failure"
                )
    )
}

#[allow(clippy::too_many_arguments)]
fn checked_jsonrpc_arg(
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
            format!("JSON-RPC builtin expects {description}"),
            expected,
            &actual_type,
        ));
    }
    Ok(value)
}

pub(super) fn lower_jsonrpc_builtin(
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
        unreachable!("jsonrpc builtin dispatcher only passes qualified calls")
    };
    debug_assert_eq!(module, "jsonrpc");

    let json_value = ValueType::Struct("JsonValue".to_string(), Vec::new());
    let message = ValueType::Struct("JsonRpcMessage".to_string(), Vec::new());
    let decoder = ValueType::Struct("JsonRpcDecoder".to_string(), Vec::new());
    let batch = ValueType::Struct("JsonRpcDecodeBatch".to_string(), Vec::new());
    let error = ValueType::Struct("JsonRpcProtocolError".to_string(), Vec::new());
    let kind = ValueType::Enum("JsonRpcMessageKind".to_string(), Vec::new());
    let option_value = ValueType::Enum("Option".to_string(), vec![json_value.clone()]);
    let result = |ok: ValueType| ValueType::Enum("Result".to_string(), vec![ok, error.clone()]);

    let checked = |arg: &AstExpr, expected: &ValueType, description: &str| {
        checked_jsonrpc_arg(
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
        "decoder" => {
            let [limit] = args else {
                return Err(jsonrpc_arity_error(path, span, name, 1));
            };
            (
                JsonRpcOperation::Decoder,
                vec![checked(limit, &ValueType::U64, "a u64 message limit")?],
                result(decoder),
            )
        }
        "feed" => {
            let [state, chunk] = args else {
                return Err(jsonrpc_arity_error(path, span, name, 2));
            };
            (
                JsonRpcOperation::Feed,
                vec![
                    checked(state, &decoder, "a JsonRpcDecoder")?,
                    checked(chunk, &ValueType::String, "a string chunk")?,
                ],
                result(batch),
            )
        }
        "finish" => {
            let [state] = args else {
                return Err(jsonrpc_arity_error(path, span, name, 1));
            };
            (
                JsonRpcOperation::Finish,
                vec![checked(state, &decoder, "a JsonRpcDecoder")?],
                result(ValueType::Void),
            )
        }
        "parse" => {
            let [value, limit] = args else {
                return Err(jsonrpc_arity_error(path, span, name, 2));
            };
            (
                JsonRpcOperation::Parse,
                vec![
                    checked(value, &json_value, "a JsonValue")?,
                    checked(limit, &ValueType::U64, "a u64 message limit")?,
                ],
                result(message),
            )
        }
        "encode" => {
            let [value, limit] = args else {
                return Err(jsonrpc_arity_error(path, span, name, 2));
            };
            (
                JsonRpcOperation::Encode,
                vec![
                    checked(value, &message, "a JsonRpcMessage")?,
                    checked(limit, &ValueType::U64, "a u64 message limit")?,
                ],
                result(ValueType::String),
            )
        }
        "value" | "kind" => {
            let [value] = args else {
                return Err(jsonrpc_arity_error(path, span, name, 1));
            };
            (
                if name == "value" {
                    JsonRpcOperation::Value
                } else {
                    JsonRpcOperation::Kind
                },
                vec![checked(value, &message, "a JsonRpcMessage")?],
                if name == "value" { json_value } else { kind },
            )
        }
        "request" => {
            let [id, method, params] = args else {
                return Err(jsonrpc_arity_error(path, span, name, 3));
            };
            (
                JsonRpcOperation::Request,
                vec![
                    checked(id, &json_value, "a JsonValue id")?,
                    checked(method, &ValueType::String, "a string method")?,
                    checked(params, &option_value, "an Option<JsonValue> params value")?,
                ],
                result(message),
            )
        }
        "notification" => {
            let [method, params] = args else {
                return Err(jsonrpc_arity_error(path, span, name, 2));
            };
            (
                JsonRpcOperation::Notification,
                vec![
                    checked(method, &ValueType::String, "a string method")?,
                    checked(params, &option_value, "an Option<JsonValue> params value")?,
                ],
                result(message),
            )
        }
        "success" => {
            let [id, value] = args else {
                return Err(jsonrpc_arity_error(path, span, name, 2));
            };
            (
                JsonRpcOperation::Success,
                vec![
                    checked(id, &json_value, "a JsonValue id")?,
                    checked(value, &json_value, "a JsonValue result")?,
                ],
                result(message),
            )
        }
        "failure" => {
            let [id, code, message_text, data] = args else {
                return Err(jsonrpc_arity_error(path, span, name, 4));
            };
            (
                JsonRpcOperation::Failure,
                vec![
                    checked(id, &json_value, "a JsonValue id")?,
                    checked(code, &ValueType::Int, "an i64 error code")?,
                    checked(message_text, &ValueType::String, "a string error message")?,
                    checked(data, &option_value, "an Option<JsonValue> error data value")?,
                ],
                result(message),
            )
        }
        _ => unreachable!("jsonrpc builtin dispatcher only passes known calls"),
    };

    Ok((
        return_type,
        ValueExpr::JsonRpc {
            operation,
            args: lowered,
        },
    ))
}

fn jsonrpc_arity_error(path: &Path, span: &Span, name: &str, expected: usize) -> Diagnostic {
    Diagnostic::new(
        "E0407",
        format!("`jsonrpc.{name}` expects exactly {expected} argument(s)"),
        path,
        span.line,
        span.column,
        span.length,
        &span.text,
    )
}
