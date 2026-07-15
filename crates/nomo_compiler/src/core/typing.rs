use super::*;

pub(super) fn ensure_supported_array_element(
    path: &Path,
    element_type: &ValueType,
    span: &Span,
) -> Result<(), Diagnostic> {
    if is_supported_array_element(element_type) {
        Ok(())
    } else {
        Err(type_mismatch(
            path,
            span,
            format!(
                "Array elements must be concrete non-void values, got `{}`",
                element_type.name()
            ),
        ))
    }
}

pub(super) fn ensure_supported_value_type(
    path: &Path,
    value_type: &ValueType,
    span: &Span,
) -> Result<(), Diagnostic> {
    match value_type {
        ValueType::Array(element_type) => {
            if matches!(element_type.as_ref(), ValueType::Void | ValueType::Never) {
                return Err(type_mismatch(
                    path,
                    span,
                    format!(
                        "Array elements must be non-void values, got `{}`",
                        element_type.name()
                    ),
                ));
            }
            ensure_supported_value_type(path, element_type, span)
        }
        ValueType::Nullable(inner) => {
            if !is_ffi_handle_type(inner) {
                return Err(type_mismatch(
                    path,
                    span,
                    "Nullable currently supports only extern opaque handle types",
                ));
            }
            ensure_supported_value_type(path, inner, span)
        }
        ValueType::ExternCallback { .. } => Err(type_mismatch(
            path,
            span,
            "extern C callback types may only appear as parameters of extern C functions",
        )),
        ValueType::Struct(_, args) | ValueType::Enum(_, args) => {
            for arg in args {
                ensure_supported_value_type(path, arg, span)?;
            }
            Ok(())
        }
        ValueType::String
        | ValueType::CString
        | ValueType::Opaque
        | ValueType::OpaqueHandle(_)
        | ValueType::OwnedHandle(_)
        | ValueType::BorrowedHandle(_)
        | ValueType::Int
        | ValueType::I32
        | ValueType::U32
        | ValueType::U64
        | ValueType::Float
        | ValueType::Char
        | ValueType::Bool
        | ValueType::Void
        | ValueType::Never
        | ValueType::TypeParam(_) => Ok(()),
    }
}

pub(super) fn synthetic_span() -> Span {
    Span {
        line: 1,
        column: 1,
        length: 1,
        text: String::new(),
    }
}

pub(super) fn is_supported_array_element(element_type: &ValueType) -> bool {
    !matches!(
        element_type,
        ValueType::Void | ValueType::Never | ValueType::TypeParam(_)
    )
}

type LoweredValue = (ValueType, ValueExpr);

#[allow(clippy::too_many_arguments)]
pub(super) fn lower_negate_expr(
    path: &Path,
    expr: &AstExpr,
    scope: &HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    expected: Option<&ValueType>,
    span: &Span,
) -> Result<LoweredValue, Diagnostic> {
    if let AstExpr::Int(value) = expr {
        let value_type = expected
            .filter(|value_type| matches!(value_type, ValueType::Int | ValueType::I32))
            .cloned()
            .unwrap_or(ValueType::Int);
        let negated = value.checked_neg().ok_or_else(|| {
            type_mismatch(
                path,
                span,
                format!("integer literal `-{value}` does not fit in `i64`"),
            )
        })?;
        return match value_type {
            ValueType::Int => Ok((ValueType::Int, ValueExpr::IntLiteral(negated))),
            ValueType::I32 if *value <= i32::MAX as i64 + 1 => {
                Ok((ValueType::I32, ValueExpr::IntLiteral(negated)))
            }
            ValueType::I32 => Err(type_mismatch(
                path,
                span,
                format!("integer literal `-{value}` does not fit in `i32`"),
            )),
            _ => unreachable!("negative integer literal only selects signed integer types"),
        };
    }

    if let AstExpr::Float(value) = expr {
        return Ok((
            ValueType::Float,
            ValueExpr::FloatLiteral(format!("-{value}")),
        ));
    }

    let inner_expected = expected.filter(|value_type| {
        matches!(
            value_type,
            ValueType::Int | ValueType::I32 | ValueType::Float
        )
    });
    let (expr_type, expr) = lower_value_expr_with_expected(
        path,
        expr,
        scope,
        imports,
        signatures,
        structs,
        enums,
        inner_expected,
        span,
    )?;
    match expr_type {
        ValueType::Int | ValueType::I32 => Ok((
            expr_type.clone(),
            ValueExpr::Binary {
                left: Box::new(ValueExpr::IntLiteral(0)),
                op: BinaryOp::Subtract,
                right: Box::new(expr),
                value_type: expr_type,
            },
        )),
        ValueType::Float => Ok((
            ValueType::Float,
            ValueExpr::Binary {
                left: Box::new(ValueExpr::FloatLiteral("0.0".to_string())),
                op: BinaryOp::Subtract,
                right: Box::new(expr),
                value_type: ValueType::Float,
            },
        )),
        _ => Err(type_mismatch(
            path,
            span,
            "`-` expects an i32, i64, or f64 operand".to_string(),
        )),
    }
}

pub(super) fn lower_binary_operands(
    path: &Path,
    left: &AstExpr,
    right: &AstExpr,
    scope: &HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    span: &Span,
) -> Result<(LoweredValue, LoweredValue), Diagnostic> {
    let left_default =
        lower_value_expr(path, left, scope, imports, signatures, structs, enums, span)?;
    let right_with_left = lower_value_expr_with_expected(
        path,
        right,
        scope,
        imports,
        signatures,
        structs,
        enums,
        Some(&left_default.0),
        span,
    )?;
    if numeric_pair_matches(&left_default.0, &right_with_left.0) {
        return Ok((left_default, right_with_left));
    }

    if matches!(left, AstExpr::Int(_)) {
        let right_default = lower_value_expr(
            path, right, scope, imports, signatures, structs, enums, span,
        )?;
        if right_default.0.is_integer() {
            let left_with_right = lower_value_expr_with_expected(
                path,
                left,
                scope,
                imports,
                signatures,
                structs,
                enums,
                Some(&right_default.0),
                span,
            )?;
            return Ok((left_with_right, right_default));
        }
    }

    Ok((left_default, right_with_left))
}

pub(super) fn numeric_pair_matches(left: &ValueType, right: &ValueType) -> bool {
    (left == right && left.is_integer())
        || (left == &ValueType::Float && right == &ValueType::Float)
}

pub(super) fn lower_int_literal(
    path: &Path,
    value: i64,
    expected: Option<&ValueType>,
    span: &Span,
) -> Result<(ValueType, ValueExpr), Diagnostic> {
    let value_type = expected
        .filter(|value_type| value_type.is_integer())
        .cloned()
        .unwrap_or(ValueType::Int);
    if !int_literal_fits(value, &value_type) {
        return Err(type_mismatch(
            path,
            span,
            format!(
                "integer literal `{value}` does not fit in `{}`",
                value_type.name()
            ),
        ));
    }
    Ok((value_type, ValueExpr::IntLiteral(value)))
}

pub(super) fn int_literal_fits(value: i64, value_type: &ValueType) -> bool {
    match value_type {
        ValueType::Int => true,
        ValueType::I32 => i32::try_from(value).is_ok(),
        ValueType::U32 => u32::try_from(value).is_ok(),
        ValueType::U64 => value >= 0,
        _ => false,
    }
}

pub(super) fn coerce_never_expr(expr: ValueExpr, target_type: &ValueType) -> ValueExpr {
    match expr {
        ValueExpr::Panic { message, .. } => ValueExpr::Panic {
            message,
            fallback_type: target_type.clone(),
        },
        other => other,
    }
}

pub(super) fn substitute_type_params(
    value_type: &ValueType,
    type_params: &[String],
    args: &[ValueType],
) -> ValueType {
    match value_type {
        ValueType::TypeParam(name) => type_params
            .iter()
            .position(|param| param == name)
            .and_then(|index| args.get(index).cloned())
            .unwrap_or_else(|| value_type.clone()),
        ValueType::Enum(name, nested_args) => ValueType::Enum(
            name.clone(),
            nested_args
                .iter()
                .map(|arg| substitute_type_params(arg, type_params, args))
                .collect(),
        ),
        ValueType::Struct(name, nested_args) => ValueType::Struct(
            name.clone(),
            nested_args
                .iter()
                .map(|arg| substitute_type_params(arg, type_params, args))
                .collect(),
        ),
        ValueType::Array(element) => {
            ValueType::Array(Box::new(substitute_type_params(element, type_params, args)))
        }
        _ => value_type.clone(),
    }
}

pub(super) fn instantiate_function_signature(
    signature: &FunctionSignature,
    args: &[ValueType],
) -> FunctionSignature {
    FunctionSignature {
        type_params: Vec::new(),
        params: signature
            .params
            .iter()
            .map(|param| ParamSignature {
                value_type: substitute_type_params(&param.value_type, &signature.type_params, args),
                mutable: param.mutable,
            })
            .collect(),
        return_type: substitute_type_params(&signature.return_type, &signature.type_params, args),
        extern_symbol: signature.extern_symbol.clone(),
    }
}

pub(super) fn result_parts(value_type: &ValueType) -> Option<(ValueType, ValueType)> {
    let ValueType::Enum(name, args) = value_type else {
        return None;
    };
    if name != "Result" || args.len() != 2 {
        return None;
    }
    Some((args[0].clone(), args[1].clone()))
}

pub(super) fn option_payload(value_type: &ValueType) -> Option<ValueType> {
    let ValueType::Enum(name, args) = value_type else {
        return None;
    };
    if name != "Option" || args.len() != 1 {
        return None;
    }
    Some(args[0].clone())
}

pub(super) fn question_payload(
    path: &Path,
    span: &Span,
    question_type: &ValueType,
    return_type: &ValueType,
) -> Result<(QuestionCarrier, ValueType), Diagnostic> {
    if let Some((ok_type, err_type)) = result_parts(question_type) {
        let (_, return_err_type) = result_parts(return_type).ok_or_else(|| {
            Diagnostic::new(
                "E0421",
                "`?` on Result<T, E> requires the current function to return Result<U, E>",
                path,
                span.line,
                span.column,
                span.length,
                &span.text,
            )
        })?;
        if err_type != return_err_type {
            return Err(type_mismatch_expected_found(
                path,
                span,
                format!(
                    "`?` error type is `{}` but function returns `{}`",
                    err_type.name(),
                    return_err_type.name()
                ),
                &return_err_type,
                &err_type,
            ));
        }
        return Ok((QuestionCarrier::Result, ok_type));
    }

    if let Some(payload_type) = option_payload(question_type) {
        option_payload(return_type).ok_or_else(|| {
            Diagnostic::new(
                "E0421",
                "`?` on Option<T> requires the current function to return Option<U>",
                path,
                span.line,
                span.column,
                span.length,
                &span.text,
            )
        })?;
        return Ok((QuestionCarrier::Option, payload_type));
    }

    Err(Diagnostic::new(
        "E0420",
        "`?` can only be used with `Result<T, E>` or `Option<T>`",
        path,
        span.line,
        span.column,
        span.length,
        &span.text,
    ))
}

pub(super) fn question_return_payload(
    return_type: &ValueType,
    carrier: QuestionCarrier,
) -> ValueType {
    match carrier {
        QuestionCarrier::Result => result_parts(return_type)
            .map(|(ok_type, _)| ok_type)
            .unwrap_or_else(|| return_type.clone()),
        QuestionCarrier::Option => {
            option_payload(return_type).unwrap_or_else(|| return_type.clone())
        }
    }
}

pub(super) fn core_prelude_variant(name: &str) -> Option<(&'static str, &'static str)> {
    match name {
        "Some" => Some(("Option", "Some")),
        "None" => Some(("Option", "None")),
        "Ok" => Some(("Result", "Ok")),
        "Err" => Some(("Result", "Err")),
        _ => None,
    }
}

pub(super) fn resolve_match_arm_variant(
    pattern: &[String],
    enum_name: &str,
    scope: &HashMap<String, Binding>,
) -> Option<String> {
    match pattern {
        [base, variant] if base == enum_name => Some(variant.clone()),
        [variant]
            if !scope.contains_key(variant)
                && core_prelude_variant(variant)
                    .is_some_and(|(resolved_enum, _)| resolved_enum == enum_name) =>
        {
            Some(variant.clone())
        }
        _ => None,
    }
}

pub(super) fn ast_binary_symbol(op: &AstBinaryOp) -> &'static str {
    match op {
        AstBinaryOp::LogicalOr => "||",
        AstBinaryOp::LogicalAnd => "&&",
        AstBinaryOp::Add => "+",
        AstBinaryOp::Subtract => "-",
        AstBinaryOp::BitOr => "|",
        AstBinaryOp::BitXor => "^",
        AstBinaryOp::Multiply => "*",
        AstBinaryOp::Divide => "/",
        AstBinaryOp::Remainder => "%",
        AstBinaryOp::ShiftLeft => "<<",
        AstBinaryOp::ShiftRight => ">>",
        AstBinaryOp::BitAnd => "&",
        AstBinaryOp::BitAndNot => "&^",
        AstBinaryOp::Equal => "==",
        AstBinaryOp::NotEqual => "!=",
        AstBinaryOp::Less => "<",
        AstBinaryOp::LessEqual => "<=",
        AstBinaryOp::Greater => ">",
        AstBinaryOp::GreaterEqual => ">=",
    }
}

pub(super) fn method_internal_name(owner_name: &str, method_name: &str) -> String {
    format!("{owner_name}_{method_name}")
}

pub(super) fn extern_call_name(symbol: &str) -> String {
    format!("{EXTERN_CALL_PREFIX}{symbol}")
}

pub(super) fn generic_function_instance_name(name: &str, args: &[ValueType]) -> String {
    let suffix = args
        .iter()
        .map(value_type_key_part)
        .collect::<Vec<_>>()
        .join("_");
    format!("{name}_{suffix}")
}

pub(super) fn value_type_key_part(value_type: &ValueType) -> String {
    match value_type {
        ValueType::String => "string".to_string(),
        ValueType::CString => "cstring".to_string(),
        ValueType::Opaque => "opaque".to_string(),
        ValueType::OpaqueHandle(name) => format!("handle_{name}"),
        ValueType::OwnedHandle(name) => format!("owned_handle_{name}"),
        ValueType::BorrowedHandle(name) => format!("borrowed_handle_{name}"),
        ValueType::Nullable(inner) => format!("nullable_{}", value_type_key_part(inner)),
        ValueType::ExternCallback {
            params,
            return_type,
        } => format!(
            "callback_{}_to_{}",
            params
                .iter()
                .map(value_type_key_part)
                .collect::<Vec<_>>()
                .join("_"),
            value_type_key_part(return_type)
        ),
        ValueType::Int => "i64".to_string(),
        ValueType::I32 => "i32".to_string(),
        ValueType::U32 => "u32".to_string(),
        ValueType::U64 => "u64".to_string(),
        ValueType::Float => "f64".to_string(),
        ValueType::Char => "char".to_string(),
        ValueType::Bool => "bool".to_string(),
        ValueType::Array(element) => format!("array_{}", value_type_key_part(element)),
        ValueType::Struct(name, args) => format!("struct_{}{}", name, generic_type_suffix(args)),
        ValueType::Enum(name, args) => format!("enum_{}{}", name, generic_type_suffix(args)),
        ValueType::TypeParam(name) => format!("param_{name}"),
        ValueType::Void => "void".to_string(),
        ValueType::Never => "never".to_string(),
    }
}

pub(super) fn generic_type_suffix(args: &[ValueType]) -> String {
    if args.is_empty() {
        String::new()
    } else {
        format!(
            "_{}",
            args.iter()
                .map(value_type_key_part)
                .collect::<Vec<_>>()
                .join("_")
        )
    }
}

pub(super) fn type_mismatch(path: &Path, span: &Span, message: impl Into<String>) -> Diagnostic {
    Diagnostic::new(
        "E0404",
        message,
        path,
        span.line,
        span.column,
        span.length,
        &span.text,
    )
}

pub(super) fn type_mismatch_expected_found(
    path: &Path,
    span: &Span,
    message: impl Into<String>,
    expected: &ValueType,
    found: &ValueType,
) -> Diagnostic {
    type_mismatch(path, span, message).with_expected_found(expected.name(), found.name())
}
