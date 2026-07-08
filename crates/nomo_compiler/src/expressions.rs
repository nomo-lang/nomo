use super::*;

pub(super) fn lower_value_expr(
    path: &Path,
    expr: &AstExpr,
    scope: &HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    span: &Span,
) -> Result<(ValueType, ValueExpr), Diagnostic> {
    lower_value_expr_with_expected(
        path, expr, scope, imports, signatures, structs, enums, None, span,
    )
}

pub(super) fn lower_value_expr_with_expected(
    path: &Path,
    expr: &AstExpr,
    scope: &HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    expected: Option<&ValueType>,
    span: &Span,
) -> Result<(ValueType, ValueExpr), Diagnostic> {
    match expr {
        AstExpr::String(value) => Ok((ValueType::String, ValueExpr::StringLiteral(value.clone()))),
        AstExpr::Int(value) => lower_int_literal(path, *value, expected, span),
        AstExpr::Float(value) => Ok((ValueType::Float, ValueExpr::FloatLiteral(value.clone()))),
        AstExpr::Char(value) => Ok((ValueType::Char, ValueExpr::CharLiteral(*value))),
        AstExpr::Bool(value) => Ok((ValueType::Bool, ValueExpr::BoolLiteral(*value))),
        AstExpr::Void => Ok((ValueType::Void, ValueExpr::VoidLiteral)),
        AstExpr::MutArg { .. } => Err(Diagnostic::new(
            "E0505",
            "`mut` is only valid in function call arguments",
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        )),
        AstExpr::Name(name) if name.len() == 1 => {
            let name = &name[0];
            let Some(binding) = scope.get(name) else {
                if let Some((enum_name, variant)) = core_prelude_variant(name) {
                    return lower_enum_variant_without_payload(
                        path, enum_name, variant, enums, expected, span,
                    );
                }
                return Err(Diagnostic::new(
                    "E0303",
                    format!("unknown variable `{name}`"),
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let _ = binding.mutable;
            if let BindingSource::EnumPayload { value, variant } = &binding.source {
                return Ok((
                    binding.value_type.clone(),
                    ValueExpr::EnumPayload {
                        value: Box::new(value.clone()),
                        variant: variant.clone(),
                    },
                ));
            }
            Ok((
                binding.value_type.clone(),
                ValueExpr::Variable(name.clone()),
            ))
        }
        AstExpr::Name(name) if name.len() == 2 => {
            let base = &name[0];
            let field = &name[1];
            if let Some(enum_type) = enums.get(base) {
                let Some(variant_type) = enum_type
                    .variants
                    .iter()
                    .find(|variant| variant.name == *field)
                else {
                    return Err(Diagnostic::new(
                        "E0315",
                        format!("enum `{base}` has no variant `{field}`"),
                        path,
                        span.line,
                        span.column,
                        span.length,
                        &span.text,
                    ));
                };
                if variant_type.payload.is_some() {
                    return Err(Diagnostic::new(
                        "E0320",
                        format!("enum variant `{base}.{field}` requires a payload"),
                        path,
                        span.line,
                        span.column,
                        span.length,
                        &span.text,
                    ));
                }
                let enum_args = match expected {
                    Some(ValueType::Enum(expected_name, expected_args))
                        if expected_name == base =>
                    {
                        expected_args.clone()
                    }
                    _ if enum_type.type_params.is_empty() => Vec::new(),
                    _ => {
                        return Err(Diagnostic::new(
                            "E0324",
                            format!(
                                "generic enum constructor `{base}.{field}` needs a type annotation"
                            ),
                            path,
                            span.line,
                            span.column,
                            span.length,
                            &span.text,
                        ));
                    }
                };
                return Ok((
                    ValueType::Enum(base.clone(), enum_args.clone()),
                    ValueExpr::EnumVariant {
                        enum_name: base.clone(),
                        enum_args,
                        variant: field.clone(),
                        payload: None,
                    },
                ));
            }
            let Some(binding) = scope.get(base) else {
                return Err(Diagnostic::new(
                    "E0303",
                    format!("unknown variable `{base}`"),
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let ValueType::Struct(type_name, struct_args) = &binding.value_type else {
                return Err(type_mismatch(
                    path,
                    span,
                    format!("`{base}` is not a struct value"),
                ));
            };
            let struct_type = structs
                .get(type_name)
                .expect("struct binding must refer to a known struct");
            let Some(field_type) = struct_type
                .fields
                .iter()
                .find(|item| item.name == *field)
                .map(|item| {
                    substitute_type_params(&item.value_type, &struct_type.type_params, struct_args)
                })
            else {
                return Err(Diagnostic::new(
                    "E0308",
                    format!("struct `{type_name}` has no field `{field}`"),
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let value = match &binding.source {
                BindingSource::EnumPayload { value, variant } => {
                    ValueExpr::EnumPayloadFieldAccess {
                        value: Box::new(value.clone()),
                        variant: variant.clone(),
                        field: field.clone(),
                    }
                }
                BindingSource::Local | BindingSource::Param => ValueExpr::FieldAccess {
                    base: base.clone(),
                    field: field.clone(),
                },
            };
            Ok((field_type, value))
        }
        AstExpr::Match { value, arms } => lower_match_value_expr(
            path, value, arms, scope, imports, signatures, structs, enums, expected, span,
        ),
        AstExpr::If {
            condition,
            then_branch,
            else_branch,
        } => {
            let (condition_type, lowered_condition) = lower_value_expr(
                path, condition, scope, imports, signatures, structs, enums, span,
            )?;
            if condition_type != ValueType::Bool {
                return Err(type_mismatch(path, span, "`if` condition must be `bool`"));
            }

            let (then_type, mut lowered_then) = lower_value_expr_with_expected(
                path,
                then_branch,
                scope,
                imports,
                signatures,
                structs,
                enums,
                expected,
                span,
            )?;
            let else_expected = if then_type == ValueType::Never {
                expected
            } else {
                Some(&then_type)
            };
            let (else_type, mut lowered_else) = lower_value_expr_with_expected(
                path,
                else_branch,
                scope,
                imports,
                signatures,
                structs,
                enums,
                else_expected,
                span,
            )?;
            let result_type = if then_type == ValueType::Never && else_type == ValueType::Never {
                ValueType::Never
            } else if then_type == ValueType::Never {
                lowered_then = coerce_never_expr(lowered_then, &else_type);
                else_type
            } else if else_type == ValueType::Never {
                lowered_else = coerce_never_expr(lowered_else, &then_type);
                then_type
            } else if else_type == then_type {
                then_type
            } else {
                return Err(type_mismatch(
                    path,
                    span,
                    format!(
                        "`if` branches return `{}` and `{}`",
                        then_type.name(),
                        else_type.name()
                    ),
                ));
            };

            Ok((
                result_type,
                ValueExpr::If {
                    condition: Box::new(lowered_condition),
                    then_branch: Box::new(lowered_then),
                    else_branch: Box::new(lowered_else),
                },
            ))
        }
        AstExpr::Panic { message } => {
            let message = lower_panic_message(
                path, message, scope, imports, signatures, structs, enums, span,
            )?;
            let fallback_type = expected.cloned().unwrap_or(ValueType::Never);
            Ok((
                fallback_type.clone(),
                ValueExpr::Panic {
                    message: Box::new(message),
                    fallback_type,
                },
            ))
        }
        AstExpr::Question { .. } => Err(Diagnostic::new(
            "E0422",
            "`?` is currently supported only in statement-level expressions with unconditional evaluation",
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        )),
        AstExpr::Cast { expr, target } => {
            let Some(target_type) = parse_value_type(target, structs, enums) else {
                return Err(unsupported_type_diagnostic_from_maps(
                    path,
                    span,
                    target,
                    "unknown cast target type",
                    structs,
                    enums,
                ));
            };
            let (source_type, lowered) =
                lower_value_expr(path, expr, scope, imports, signatures, structs, enums, span)?;
            match (&source_type, &target_type) {
                (source, ValueType::Float) if source.is_integer() => Ok((
                    target_type.clone(),
                    ValueExpr::Cast {
                        expr: Box::new(lowered),
                        target_type,
                    },
                )),
                (ValueType::Float, ValueType::Float) => Ok((
                    target_type.clone(),
                    ValueExpr::Cast {
                        expr: Box::new(lowered),
                        target_type,
                    },
                )),
                (source, target) if source.is_integer() && target.is_integer() => Ok((
                    target_type.clone(),
                    ValueExpr::Cast {
                        expr: Box::new(lowered),
                        target_type,
                    },
                )),
                _ => Err(type_mismatch(
                    path,
                    span,
                    format!(
                        "cannot cast `{}` to `{}`",
                        source_type.name(),
                        target_type.name()
                    ),
                )),
            }
        }
        AstExpr::Unary { op, expr } => {
            let lowered_op = match op {
                AstUnaryOp::Not => UnaryOp::Not,
                AstUnaryOp::Negate => UnaryOp::Negate,
            };
            if matches!(lowered_op, UnaryOp::Negate) {
                return lower_negate_expr(
                    path, expr, scope, imports, signatures, structs, enums, expected, span,
                );
            }
            let (expr_type, expr) =
                lower_value_expr(path, expr, scope, imports, signatures, structs, enums, span)?;
            match (lowered_op, &expr_type) {
                (UnaryOp::Not, ValueType::Bool) => Ok((
                    ValueType::Bool,
                    ValueExpr::Unary {
                        op: lowered_op,
                        expr: Box::new(expr),
                    },
                )),
                (UnaryOp::Not, _) => Err(type_mismatch(
                    path,
                    span,
                    "`!` expects a bool operand".to_string(),
                )),
                (UnaryOp::Negate, _) => unreachable!("negation is lowered before this match"),
            }
        }
        AstExpr::Binary { left, op, right } => {
            let ((left_type, left), (right_type, right)) = lower_binary_operands(
                path, left, right, scope, imports, signatures, structs, enums, span,
            )?;
            let lowered_op = match op {
                AstBinaryOp::LogicalOr => BinaryOp::LogicalOr,
                AstBinaryOp::LogicalAnd => BinaryOp::LogicalAnd,
                AstBinaryOp::Add => BinaryOp::Add,
                AstBinaryOp::Subtract => BinaryOp::Subtract,
                AstBinaryOp::BitOr => BinaryOp::BitOr,
                AstBinaryOp::BitXor => BinaryOp::BitXor,
                AstBinaryOp::Multiply => BinaryOp::Multiply,
                AstBinaryOp::Divide => BinaryOp::Divide,
                AstBinaryOp::Remainder => BinaryOp::Remainder,
                AstBinaryOp::ShiftLeft => BinaryOp::ShiftLeft,
                AstBinaryOp::ShiftRight => BinaryOp::ShiftRight,
                AstBinaryOp::BitAnd => BinaryOp::BitAnd,
                AstBinaryOp::BitAndNot => BinaryOp::BitAndNot,
                AstBinaryOp::Equal => BinaryOp::Equal,
                AstBinaryOp::NotEqual => BinaryOp::NotEqual,
                AstBinaryOp::Less => BinaryOp::Less,
                AstBinaryOp::LessEqual => BinaryOp::LessEqual,
                AstBinaryOp::Greater => BinaryOp::Greater,
                AstBinaryOp::GreaterEqual => BinaryOp::GreaterEqual,
            };
            let value_type = match (lowered_op, &left_type, &right_type) {
                (BinaryOp::LogicalOr | BinaryOp::LogicalAnd, ValueType::Bool, ValueType::Bool) => {
                    ValueType::Bool
                }
                (
                    BinaryOp::Add | BinaryOp::Subtract | BinaryOp::Multiply | BinaryOp::Divide,
                    left_type,
                    right_type,
                ) if numeric_pair_matches(left_type, right_type) => left_type.clone(),
                (BinaryOp::Remainder, left_type, right_type)
                    if left_type == right_type && left_type.is_integer() =>
                {
                    left_type.clone()
                }
                (
                    BinaryOp::BitOr | BinaryOp::BitXor | BinaryOp::BitAnd | BinaryOp::BitAndNot,
                    left_type,
                    right_type,
                ) if left_type == right_type && left_type.is_integer() => left_type.clone(),
                (BinaryOp::ShiftLeft | BinaryOp::ShiftRight, left_type, right_type)
                    if left_type.is_integer() && right_type.is_integer() =>
                {
                    left_type.clone()
                }
                (BinaryOp::Equal | BinaryOp::NotEqual, ValueType::String, ValueType::String)
                | (BinaryOp::Equal | BinaryOp::NotEqual, ValueType::Char, ValueType::Char)
                | (BinaryOp::Equal | BinaryOp::NotEqual, ValueType::Bool, ValueType::Bool) => {
                    ValueType::Bool
                }
                (
                    BinaryOp::Equal
                    | BinaryOp::NotEqual
                    | BinaryOp::Less
                    | BinaryOp::LessEqual
                    | BinaryOp::Greater
                    | BinaryOp::GreaterEqual,
                    ValueType::Int,
                    ValueType::Int,
                )
                | (
                    BinaryOp::Equal
                    | BinaryOp::NotEqual
                    | BinaryOp::Less
                    | BinaryOp::LessEqual
                    | BinaryOp::Greater
                    | BinaryOp::GreaterEqual,
                    ValueType::I32,
                    ValueType::I32,
                )
                | (
                    BinaryOp::Equal
                    | BinaryOp::NotEqual
                    | BinaryOp::Less
                    | BinaryOp::LessEqual
                    | BinaryOp::Greater
                    | BinaryOp::GreaterEqual,
                    ValueType::U32,
                    ValueType::U32,
                )
                | (
                    BinaryOp::Equal
                    | BinaryOp::NotEqual
                    | BinaryOp::Less
                    | BinaryOp::LessEqual
                    | BinaryOp::Greater
                    | BinaryOp::GreaterEqual,
                    ValueType::U64,
                    ValueType::U64,
                )
                | (
                    BinaryOp::Equal
                    | BinaryOp::NotEqual
                    | BinaryOp::Less
                    | BinaryOp::LessEqual
                    | BinaryOp::Greater
                    | BinaryOp::GreaterEqual,
                    ValueType::Float,
                    ValueType::Float,
                ) => ValueType::Bool,
                _ => {
                    let operand_kind = if matches!(
                        lowered_op,
                        BinaryOp::Add
                            | BinaryOp::Subtract
                            | BinaryOp::Multiply
                            | BinaryOp::Divide
                            | BinaryOp::Remainder
                    ) {
                        "numeric"
                    } else if matches!(lowered_op, BinaryOp::LogicalOr | BinaryOp::LogicalAnd) {
                        "bool"
                    } else if matches!(
                        lowered_op,
                        BinaryOp::BitOr
                            | BinaryOp::BitXor
                            | BinaryOp::BitAnd
                            | BinaryOp::BitAndNot
                            | BinaryOp::ShiftLeft
                            | BinaryOp::ShiftRight
                    ) {
                        "integer"
                    } else {
                        "comparable"
                    };
                    return Err(type_mismatch(
                        path,
                        span,
                        format!(
                            "`{}` expects two matching {operand_kind} operands",
                            ast_binary_symbol(op),
                        ),
                    ));
                }
            };
            let value = if left_type == ValueType::String
                && right_type == ValueType::String
                && matches!(lowered_op, BinaryOp::Equal | BinaryOp::NotEqual)
            {
                ValueExpr::StringCompare {
                    left: Box::new(left),
                    op: lowered_op,
                    right: Box::new(right),
                }
            } else {
                ValueExpr::Binary {
                    left: Box::new(left),
                    op: lowered_op,
                    right: Box::new(right),
                    value_type: value_type.clone(),
                }
            };
            Ok((value_type, value))
        }
        AstExpr::Call {
            callee,
            args,
            type_args,
        } if callee.len() == 1 => {
            let name = &callee[0];
            if let Some(qualified) = resolve_specific_value_builtin(name, imports) {
                if qualified == ["Array", "new"] {
                    return lower_array_new(path, type_args, args, structs, enums, span);
                }
                if !type_args.is_empty() {
                    return Err(type_mismatch(
                        path,
                        span,
                        format!(
                            "standard library function `{name}` does not accept type arguments"
                        ),
                    ));
                }
                if qualified[0] == "string" {
                    return lower_string_builtin(
                        path, &qualified, args, scope, imports, signatures, structs, enums, span,
                    );
                }
                if qualified[0] == "fs" {
                    return lower_fs_builtin(
                        path, &qualified, args, scope, imports, signatures, structs, enums, span,
                    );
                }
                if qualified[0] == "io" {
                    return lower_io_builtin(path, &qualified, args, span);
                }
                if qualified[0] == "debug" {
                    return lower_debug_builtin(
                        path, &qualified, args, scope, imports, signatures, structs, enums, span,
                    );
                }
                if qualified[0] == "log" {
                    return lower_log_builtin(
                        path, &qualified, args, scope, imports, signatures, structs, enums, span,
                    );
                }
                if qualified[0] == "hash" {
                    return lower_hash_builtin(
                        path, &qualified, args, scope, imports, signatures, structs, enums, span,
                    );
                }
                if qualified[0] == "crypto" {
                    return lower_crypto_builtin(
                        path, &qualified, args, scope, imports, signatures, structs, enums, span,
                    );
                }
                if qualified[0] == "json" {
                    return lower_json_builtin(
                        path, &qualified, args, scope, imports, signatures, structs, enums, span,
                    );
                }
                if qualified[0] == "http" {
                    return lower_http_builtin(
                        path, &qualified, args, scope, imports, signatures, structs, enums, span,
                    );
                }
                if qualified[0] == "net" {
                    return lower_net_builtin(
                        path, &qualified, args, scope, imports, signatures, structs, enums, span,
                    );
                }
                if qualified[0] == "regex" {
                    return lower_regex_builtin(
                        path, &qualified, args, scope, imports, signatures, structs, enums, span,
                    );
                }
                if qualified[0] == "collections" {
                    return lower_collections_builtin(
                        path, &qualified, args, scope, imports, signatures, structs, enums, span,
                    );
                }
                if qualified[0] == "env" {
                    return lower_env_builtin(
                        path, &qualified, args, scope, imports, signatures, structs, enums, span,
                    );
                }
                if qualified[0] == "process" {
                    return lower_process_builtin(
                        path, &qualified, args, scope, imports, signatures, structs, enums, span,
                    );
                }
                if qualified[0] == "path" {
                    return lower_path_builtin(
                        path, &qualified, args, scope, imports, signatures, structs, enums, span,
                    );
                }
                if qualified[0] == "math" {
                    return lower_math_builtin(
                        path, &qualified, args, scope, imports, signatures, structs, enums, span,
                    );
                }
                if qualified[0] == "char" {
                    return lower_char_builtin(
                        path, &qualified, args, scope, imports, signatures, structs, enums, span,
                    );
                }
                if qualified[0] == "os" {
                    return lower_os_builtin(path, &qualified, args, span);
                }
                if qualified[0] == "time" {
                    return lower_time_builtin(
                        path, &qualified, args, scope, imports, signatures, structs, enums, span,
                    );
                }
                if qualified[0] == "testing" {
                    return lower_testing_builtin(
                        path, &qualified, args, scope, imports, signatures, structs, enums, span,
                    );
                }
                if qualified[0] == "num" {
                    return lower_num_builtin(
                        path, &qualified, args, scope, imports, signatures, structs, enums, span,
                    );
                }
                if qualified[0] == "option" {
                    return lower_option_builtin(
                        path, &qualified, args, scope, imports, signatures, structs, enums, span,
                    );
                }
                if qualified[0] == "result" {
                    return lower_result_builtin(
                        path, &qualified, args, scope, imports, signatures, structs, enums, span,
                    );
                }
            }
            let Some(template_signature) = signatures.get(name) else {
                if name == "puts" {
                    if !type_args.is_empty() {
                        return Err(type_mismatch(
                            path,
                            span,
                            "extern function `puts` does not accept type arguments",
                        ));
                    }
                    let [arg] = args.as_slice() else {
                        return Err(Diagnostic::new(
                            "E1519",
                            "extern function `puts` expects 1 argument",
                            path,
                            span.line,
                            span.column,
                            span.length,
                            &span.text,
                        ));
                    };
                    let (arg_type, lowered) = lower_value_expr_with_expected(
                        path,
                        arg,
                        scope,
                        imports,
                        signatures,
                        structs,
                        enums,
                        Some(&ValueType::String),
                        span,
                    )?;
                    if arg_type != ValueType::String {
                        return Err(type_mismatch(
                            path,
                            span,
                            "extern function `puts` expects a `string` argument",
                        ));
                    }
                    let return_type = if matches!(expected, Some(ValueType::Void)) {
                        ValueType::Void
                    } else {
                        ValueType::I32
                    };
                    return Ok((
                        return_type,
                        ValueExpr::Call {
                            name: BUILTIN_FFI_PUTS_EXPR.to_string(),
                            args: vec![lowered],
                        },
                    ));
                }
                if scope.contains_key(name) {
                    return Err(Diagnostic::new(
                        "E0305",
                        format!("local variable `{name}` is not callable"),
                        path,
                        span.line,
                        span.column,
                        span.length,
                        &span.text,
                    ));
                }
                if let Some((enum_name, variant)) = core_prelude_variant(name) {
                    if !type_args.is_empty() {
                        return Err(type_mismatch(
                            path,
                            span,
                            format!(
                                "enum variant `{enum_name}.{variant}` does not accept type arguments"
                            ),
                        ));
                    }
                    return lower_enum_variant_with_payload(
                        path, enum_name, variant, args, scope, imports, signatures, structs, enums,
                        expected, span,
                    );
                }
                return Err(Diagnostic::new(
                    "E0305",
                    format!("unknown function `{name}`"),
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (call_name, signature) = if type_args.is_empty() {
                if !template_signature.type_params.is_empty() {
                    return Err(Diagnostic::new(
                        "E0407",
                        format!("generic function `{name}` requires explicit type arguments"),
                        path,
                        span.line,
                        span.column,
                        span.length,
                        &span.text,
                    ));
                }
                (name.clone(), template_signature.clone())
            } else {
                if template_signature.type_params.is_empty() {
                    return Err(type_mismatch(
                        path,
                        span,
                        format!("function `{name}` does not accept type arguments"),
                    ));
                }
                if type_args.len() != template_signature.type_params.len() {
                    return Err(Diagnostic::new(
                        "E0407",
                        format!(
                            "function `{name}` expects {} type argument(s), got {}",
                            template_signature.type_params.len(),
                            type_args.len()
                        ),
                        path,
                        span.line,
                        span.column,
                        span.length,
                        &span.text,
                    ));
                }
                let instance_args = type_args
                    .iter()
                    .map(|arg| parse_non_void_type(arg, structs, enums))
                    .collect::<Option<Vec<_>>>()
                    .ok_or_else(|| {
                        let type_arg = type_args
                            .iter()
                            .find(|arg| parse_non_void_type(arg, structs, enums).is_none())
                            .expect("at least one type argument failed to lower");
                        unsupported_type_diagnostic_from_maps(
                            path,
                            span,
                            type_arg,
                            format!("unsupported type argument for `{name}`"),
                            structs,
                            enums,
                        )
                    })?;
                (
                    generic_function_instance_name(name, &instance_args),
                    instantiate_function_signature(template_signature, &instance_args),
                )
            };
            if signature.return_type == ValueType::Void
                && !matches!(expected, Some(ValueType::Void))
            {
                return Err(type_mismatch(
                    path,
                    span,
                    format!("function `{call_name}` returns `void` and cannot be used as a value"),
                ));
            }
            if args.len() != signature.params.len() {
                return Err(Diagnostic::new(
                    "E0407",
                    format!(
                        "function `{call_name}` expects {} argument(s), got {}",
                        signature.params.len(),
                        args.len()
                    ),
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            }

            let mut lowered_args = Vec::new();
            let mut mutable_borrows = Vec::new();
            for (index, (arg, expected)) in args.iter().zip(signature.params.iter()).enumerate() {
                lowered_args.push(lower_call_arg_for_param(
                    path,
                    arg,
                    expected,
                    scope,
                    imports,
                    signatures,
                    structs,
                    enums,
                    span,
                    &call_name,
                    index + 1,
                    &mut mutable_borrows,
                )?);
            }

            Ok((
                signature.return_type.clone(),
                ValueExpr::Call {
                    name: signature
                        .extern_symbol
                        .as_ref()
                        .map(|symbol| extern_call_name(symbol))
                        .unwrap_or(call_name),
                    args: lowered_args,
                },
            ))
        }
        AstExpr::Call {
            callee,
            args,
            type_args,
        } if callee.len() == 2 => {
            if is_io_print_call(callee) {
                if !type_args.is_empty() {
                    return Err(type_mismatch(
                        path,
                        span,
                        format!(
                            "standard library function `{}` does not accept type arguments",
                            callee.join(".")
                        ),
                    ));
                }
                let Some(function_name) = resolve_io_print_function(callee, imports) else {
                    return Err(missing_io_import_diagnostic(path, span, callee));
                };
                if !matches!(expected, Some(ValueType::Void)) {
                    return Err(type_mismatch(
                        path,
                        span,
                        format!(
                            "function `{}` returns `void` and cannot be used as a value",
                            callee.join(".")
                        ),
                    ));
                }
                let [arg] = args.as_slice() else {
                    return Err(println_type_error(path, span, function_name));
                };
                let (arg_type, lowered) =
                    lower_value_expr(path, arg, scope, imports, signatures, structs, enums, span)?;
                if arg_type != ValueType::String {
                    return Err(println_type_error(path, span, function_name));
                }
                let name = io_print_builtin_expr_name(function_name);
                return Ok((
                    ValueType::Void,
                    ValueExpr::Call {
                        name,
                        args: vec![lowered],
                    },
                ));
            }
            if callee == &["Array", "new"] {
                require_import(path, imports, span, "std.array", "Array.new")?;
                return lower_array_new(path, type_args, args, structs, enums, span);
            }
            if is_string_builtin_call(callee) {
                require_import(path, imports, span, "std.string", &callee.join("."))?;
                if !type_args.is_empty() {
                    return Err(type_mismatch(
                        path,
                        span,
                        "string builtins do not accept type arguments",
                    ));
                }
                return lower_string_builtin(
                    path, callee, args, scope, imports, signatures, structs, enums, span,
                );
            }
            if callee == &["fs", "read_to_string"]
                || callee == &["fs", "write_string"]
                || callee == &["fs", "read_bytes"]
                || callee == &["fs", "write_bytes"]
                || callee == &["fs", "exists"]
                || callee == &["fs", "metadata"]
                || callee == &["fs", "create_dir"]
                || callee == &["fs", "remove_dir"]
                || callee == &["fs", "read_dir"]
                || callee == &["fs", "open"]
            {
                require_import(path, imports, span, "std.fs", &callee.join("."))?;
                if !type_args.is_empty() {
                    return Err(type_mismatch(
                        path,
                        span,
                        "fs builtins do not accept type arguments",
                    ));
                }
                return lower_fs_builtin(
                    path, callee, args, scope, imports, signatures, structs, enums, span,
                );
            }
            if is_io_value_builtin_call(callee) {
                require_import(path, imports, span, "std.io", &callee.join("."))?;
                if !type_args.is_empty() {
                    return Err(type_mismatch(
                        path,
                        span,
                        "io builtins do not accept type arguments",
                    ));
                }
                return lower_io_builtin(path, callee, args, span);
            }
            if is_debug_builtin_call(callee) {
                require_import(path, imports, span, "std.debug", &callee.join("."))?;
                if !type_args.is_empty() {
                    return Err(type_mismatch(
                        path,
                        span,
                        "debug builtins do not accept type arguments",
                    ));
                }
                return lower_debug_builtin(
                    path, callee, args, scope, imports, signatures, structs, enums, span,
                );
            }
            if is_log_builtin_call(callee) {
                require_import(path, imports, span, "std.log", &callee.join("."))?;
                if !type_args.is_empty() {
                    return Err(type_mismatch(
                        path,
                        span,
                        "log builtins do not accept type arguments",
                    ));
                }
                return lower_log_builtin(
                    path, callee, args, scope, imports, signatures, structs, enums, span,
                );
            }
            if is_hash_builtin_call(callee) {
                require_import(path, imports, span, "std.hash", &callee.join("."))?;
                if !type_args.is_empty() {
                    return Err(type_mismatch(
                        path,
                        span,
                        "hash builtins do not accept type arguments",
                    ));
                }
                return lower_hash_builtin(
                    path, callee, args, scope, imports, signatures, structs, enums, span,
                );
            }
            if is_crypto_builtin_call(callee) {
                require_import(path, imports, span, "std.crypto", &callee.join("."))?;
                if !type_args.is_empty() {
                    return Err(type_mismatch(
                        path,
                        span,
                        "crypto builtins do not accept type arguments",
                    ));
                }
                return lower_crypto_builtin(
                    path, callee, args, scope, imports, signatures, structs, enums, span,
                );
            }
            if is_json_builtin_call(callee) {
                require_import(path, imports, span, "std.json", &callee.join("."))?;
                if !type_args.is_empty() {
                    return Err(type_mismatch(
                        path,
                        span,
                        "json builtins do not accept type arguments",
                    ));
                }
                return lower_json_builtin(
                    path, callee, args, scope, imports, signatures, structs, enums, span,
                );
            }
            if is_http_builtin_call(callee) {
                require_import(path, imports, span, "std.http", &callee.join("."))?;
                if !type_args.is_empty() {
                    return Err(type_mismatch(
                        path,
                        span,
                        "http builtins do not accept type arguments",
                    ));
                }
                return lower_http_builtin(
                    path, callee, args, scope, imports, signatures, structs, enums, span,
                );
            }
            if is_net_builtin_call(callee) {
                require_import(path, imports, span, "std.net", &callee.join("."))?;
                if !type_args.is_empty() {
                    return Err(type_mismatch(
                        path,
                        span,
                        "net builtins do not accept type arguments",
                    ));
                }
                return lower_net_builtin(
                    path, callee, args, scope, imports, signatures, structs, enums, span,
                );
            }
            if is_regex_builtin_call(callee) {
                require_import(path, imports, span, "std.regex", &callee.join("."))?;
                if !type_args.is_empty() {
                    return Err(type_mismatch(
                        path,
                        span,
                        "regex builtins do not accept type arguments",
                    ));
                }
                return lower_regex_builtin(
                    path, callee, args, scope, imports, signatures, structs, enums, span,
                );
            }
            if is_collections_builtin_call(callee) {
                require_import(path, imports, span, "std.collections", &callee.join("."))?;
                if !type_args.is_empty() {
                    return Err(type_mismatch(
                        path,
                        span,
                        "collections builtins do not accept type arguments",
                    ));
                }
                return lower_collections_builtin(
                    path, callee, args, scope, imports, signatures, structs, enums, span,
                );
            }
            if is_env_builtin_call(callee) {
                require_import(path, imports, span, "std.env", &callee.join("."))?;
                if !type_args.is_empty() {
                    return Err(type_mismatch(
                        path,
                        span,
                        "env builtins do not accept type arguments",
                    ));
                }
                return lower_env_builtin(
                    path, callee, args, scope, imports, signatures, structs, enums, span,
                );
            }
            if is_process_builtin_call(callee) {
                require_import(path, imports, span, "std.process", &callee.join("."))?;
                if !type_args.is_empty() {
                    return Err(type_mismatch(
                        path,
                        span,
                        "process builtins do not accept type arguments",
                    ));
                }
                return lower_process_builtin(
                    path, callee, args, scope, imports, signatures, structs, enums, span,
                );
            }
            if is_path_builtin_call(callee) {
                require_import(path, imports, span, "std.path", &callee.join("."))?;
                if !type_args.is_empty() {
                    return Err(type_mismatch(
                        path,
                        span,
                        "path builtins do not accept type arguments",
                    ));
                }
                return lower_path_builtin(
                    path, callee, args, scope, imports, signatures, structs, enums, span,
                );
            }
            if is_math_builtin_call(callee) {
                require_import(path, imports, span, "std.math", &callee.join("."))?;
                if !type_args.is_empty() {
                    return Err(type_mismatch(
                        path,
                        span,
                        "math builtins do not accept type arguments",
                    ));
                }
                return lower_math_builtin(
                    path, callee, args, scope, imports, signatures, structs, enums, span,
                );
            }
            if is_char_builtin_call(callee) {
                require_import(path, imports, span, "std.char", &callee.join("."))?;
                if !type_args.is_empty() {
                    return Err(type_mismatch(
                        path,
                        span,
                        "char builtins do not accept type arguments",
                    ));
                }
                return lower_char_builtin(
                    path, callee, args, scope, imports, signatures, structs, enums, span,
                );
            }
            if is_os_builtin_call(callee) {
                require_import(path, imports, span, "std.os", &callee.join("."))?;
                if !type_args.is_empty() {
                    return Err(type_mismatch(
                        path,
                        span,
                        "os builtins do not accept type arguments",
                    ));
                }
                return lower_os_builtin(path, callee, args, span);
            }
            if is_time_builtin_call(callee) {
                require_import(path, imports, span, "std.time", &callee.join("."))?;
                if !type_args.is_empty() {
                    return Err(type_mismatch(
                        path,
                        span,
                        "time builtins do not accept type arguments",
                    ));
                }
                return lower_time_builtin(
                    path, callee, args, scope, imports, signatures, structs, enums, span,
                );
            }
            if is_testing_builtin_call(callee) {
                require_import(path, imports, span, "std.testing", &callee.join("."))?;
                if !type_args.is_empty() {
                    return Err(type_mismatch(
                        path,
                        span,
                        "testing builtins do not accept type arguments",
                    ));
                }
                return lower_testing_builtin(
                    path, callee, args, scope, imports, signatures, structs, enums, span,
                );
            }
            if is_num_builtin_call(callee) {
                require_import(path, imports, span, "std.num", &callee.join("."))?;
                if !type_args.is_empty() {
                    return Err(type_mismatch(
                        path,
                        span,
                        "num builtins do not accept type arguments",
                    ));
                }
                return lower_num_builtin(
                    path, callee, args, scope, imports, signatures, structs, enums, span,
                );
            }
            if is_option_builtin_call(callee) {
                require_option_method_import(path, imports, span, &callee[1])?;
                if !type_args.is_empty() {
                    return Err(type_mismatch(
                        path,
                        span,
                        "option builtins do not accept type arguments",
                    ));
                }
                return lower_option_builtin(
                    path, callee, args, scope, imports, signatures, structs, enums, span,
                );
            }
            if is_result_builtin_call(callee) {
                require_result_method_import(path, imports, span, &callee[1])?;
                if !type_args.is_empty() {
                    return Err(type_mismatch(
                        path,
                        span,
                        "result builtins do not accept type arguments",
                    ));
                }
                return lower_result_builtin(
                    path, callee, args, scope, imports, signatures, structs, enums, span,
                );
            }
            if type_args.is_empty() && is_array_value_method(callee, scope) {
                return lower_array_value_method(
                    path, callee, args, scope, imports, signatures, structs, enums, span,
                );
            }
            if type_args.is_empty() && is_string_value_method(callee, scope) {
                return lower_string_value_method(
                    path, callee, args, scope, imports, signatures, structs, enums, span,
                );
            }
            if type_args.is_empty() && is_file_value_method(callee, scope) {
                return lower_file_value_method(
                    path, callee, args, scope, imports, signatures, structs, enums, span,
                );
            }
            if type_args.is_empty() && is_tcp_stream_value_method(callee, scope) {
                return lower_tcp_stream_value_method(
                    path, callee, args, scope, imports, signatures, structs, enums, span,
                );
            }
            if type_args.is_empty() && is_tcp_listener_value_method(callee, scope) {
                return lower_tcp_listener_value_method(path, callee, args, scope, span);
            }
            if type_args.is_empty() && is_udp_socket_value_method(callee, scope) {
                return lower_udp_socket_value_method(
                    path, callee, args, scope, imports, signatures, structs, enums, span,
                );
            }
            if type_args.is_empty() {
                if let Some(lowered) = lower_option_value_method(
                    path, callee, args, scope, imports, signatures, structs, enums, span,
                )? {
                    return Ok(lowered);
                }
                if let Some(lowered) = lower_result_value_method(
                    path, callee, args, scope, imports, signatures, structs, enums, span,
                )? {
                    return Ok(lowered);
                }
            }
            if type_args.is_empty() {
                if let Some(lowered) = lower_struct_value_method(
                    path,
                    callee,
                    args,
                    scope,
                    imports,
                    signatures,
                    structs,
                    enums,
                    span,
                    matches!(expected, Some(ValueType::Void)),
                )? {
                    return Ok(lowered);
                }
            }
            if !type_args.is_empty() {
                return Err(type_mismatch(
                    path,
                    span,
                    format!(
                        "function `{}` does not accept type arguments",
                        callee.join(".")
                    ),
                ));
            }
            let enum_name = &callee[0];
            let variant = &callee[1];
            let Some(enum_type) = enums.get(enum_name) else {
                return Err(Diagnostic::new(
                    "E0305",
                    format!("unknown function `{}`", callee.join(".")),
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let Some(variant_type) = enum_type.variants.iter().find(|item| item.name == *variant)
            else {
                return Err(Diagnostic::new(
                    "E0315",
                    format!("enum `{enum_name}` has no variant `{variant}`"),
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let Some(raw_payload_type) = &variant_type.payload else {
                return Err(Diagnostic::new(
                    "E0323",
                    format!("enum variant `{enum_name}.{variant}` does not accept a payload"),
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let [arg] = args.as_slice() else {
                return Err(Diagnostic::new(
                    "E0407",
                    format!("enum variant `{enum_name}.{variant}` expects exactly one payload"),
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let enum_args = match expected {
                Some(ValueType::Enum(expected_name, expected_args))
                    if expected_name == enum_name =>
                {
                    expected_args.clone()
                }
                _ if enum_type.type_params.is_empty() => Vec::new(),
                _ => {
                    return Err(Diagnostic::new(
                        "E0324",
                        format!(
                            "generic enum constructor `{enum_name}.{variant}` needs a type annotation"
                        ),
                        path,
                        span.line,
                        span.column,
                        span.length,
                        &span.text,
                    ));
                }
            };
            let payload_type =
                substitute_type_params(raw_payload_type, &enum_type.type_params, &enum_args);
            let (actual_type, payload) = lower_value_expr_with_expected(
                path,
                arg,
                scope,
                imports,
                signatures,
                structs,
                enums,
                Some(&payload_type),
                span,
            )?;
            if actual_type != payload_type {
                return Err(type_mismatch(
                    path,
                    span,
                    format!(
                        "payload for `{enum_name}.{variant}` is `{}` but expected `{}`",
                        actual_type.name(),
                        payload_type.name()
                    ),
                ));
            }
            Ok((
                ValueType::Enum(enum_name.clone(), enum_args.clone()),
                ValueExpr::EnumVariant {
                    enum_name: enum_name.clone(),
                    enum_args,
                    variant: variant.clone(),
                    payload: Some(Box::new(payload)),
                },
            ))
        }
        AstExpr::StructLiteral { type_name, fields } if type_name.len() == 1 => {
            let type_name = &type_name[0];
            let Some(struct_type) = structs.get(type_name) else {
                return Err(Diagnostic::new(
                    "E0309",
                    format!("unknown struct `{type_name}`"),
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let struct_args = match expected {
                Some(ValueType::Struct(expected_name, expected_args))
                    if expected_name == type_name =>
                {
                    expected_args.clone()
                }
                _ if struct_type.type_params.is_empty() => Vec::new(),
                _ => {
                    return Err(Diagnostic::new(
                        "E0317",
                        format!("generic struct literal `{type_name}` needs a type annotation"),
                        path,
                        span.line,
                        span.column,
                        span.length,
                        &span.text,
                    ));
                }
            };
            let mut seen = HashMap::new();
            for (field_name, _) in fields {
                if seen.insert(field_name.clone(), ()).is_some() {
                    return Err(Diagnostic::new(
                        "E0311",
                        format!("field `{field_name}` is specified more than once"),
                        path,
                        span.line,
                        span.column,
                        span.length,
                        &span.text,
                    ));
                }
            }
            let mut lowered_fields = Vec::new();
            for expected_field in &struct_type.fields {
                let expected_field_type = substitute_type_params(
                    &expected_field.value_type,
                    &struct_type.type_params,
                    &struct_args,
                );
                let Some((_, value)) = fields
                    .iter()
                    .find(|(field_name, _)| field_name == &expected_field.name)
                else {
                    return Err(Diagnostic::new(
                        "E0310",
                        format!(
                            "missing field `{}` for struct `{type_name}`",
                            expected_field.name
                        ),
                        path,
                        span.line,
                        span.column,
                        span.length,
                        &span.text,
                    ));
                };
                let (actual_type, lowered) = lower_value_expr_with_expected(
                    path,
                    value,
                    scope,
                    imports,
                    signatures,
                    structs,
                    enums,
                    Some(&expected_field_type),
                    span,
                )?;
                if actual_type != expected_field_type {
                    return Err(type_mismatch(
                        path,
                        span,
                        format!(
                            "field `{}` is `{}` but expected `{}`",
                            expected_field.name,
                            actual_type.name(),
                            expected_field_type.name()
                        ),
                    ));
                }
                lowered_fields.push((expected_field.name.clone(), lowered));
            }
            for (field_name, _) in fields {
                if !struct_type
                    .fields
                    .iter()
                    .any(|field| field.name == *field_name)
                {
                    return Err(Diagnostic::new(
                        "E0312",
                        format!("struct `{type_name}` has no field `{field_name}`"),
                        path,
                        span.line,
                        span.column,
                        span.length,
                        &span.text,
                    ));
                }
            }
            Ok((
                ValueType::Struct(type_name.clone(), struct_args.clone()),
                ValueExpr::StructLiteral {
                    type_name: type_name.clone(),
                    struct_args,
                    fields: lowered_fields,
                },
            ))
        }
        _ => Err(Diagnostic::new(
            "E0405",
            "expression is not supported as a value in v0.1 current implementation",
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        )),
    }
}
