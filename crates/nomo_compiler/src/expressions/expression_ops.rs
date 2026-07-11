use super::*;

pub(super) fn lower_cast_value_expr(
    path: &Path,
    expr: &AstExpr,
    target: &AstTypeRef,
    scope: &HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    span: &Span,
) -> Result<(ValueType, ValueExpr), Diagnostic> {
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

pub(super) fn lower_unary_value_expr(
    path: &Path,
    op: &AstUnaryOp,
    expr: &AstExpr,
    scope: &HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    expected: Option<&ValueType>,
    span: &Span,
) -> Result<(ValueType, ValueExpr), Diagnostic> {
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

pub(super) fn lower_binary_value_expr(
    path: &Path,
    left: &AstExpr,
    op: &AstBinaryOp,
    right: &AstExpr,
    scope: &HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    span: &Span,
) -> Result<(ValueType, ValueExpr), Diagnostic> {
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
