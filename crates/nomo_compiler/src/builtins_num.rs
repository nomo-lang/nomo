use super::*;
pub(super) fn is_num_builtin_call(callee: &[String]) -> bool {
    matches!(
        callee,
        [module, name]
            if module == "num"
                && matches!(
                    name.as_str(),
                    "parse_i64"
                        | "parse_u64"
                        | "parse_f64"
                        | "to_string"
                        | "checked_add"
                        | "checked_sub"
                        | "checked_mul"
                        | "wrapping_add"
                        | "wrapping_sub"
                        | "wrapping_mul"
                )
    )
}
pub(super) fn lower_num_builtin(
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
        unreachable!("num builtin dispatcher only passes qualified calls")
    };
    debug_assert_eq!(module, "num");
    let num_error = ValueType::Struct("NumError".to_string(), Vec::new());
    match name.as_str() {
        "parse_i64" | "parse_u64" | "parse_f64" => {
            let [value] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    format!("`num.{name}` expects exactly one argument"),
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (value_type, lowered_value) = lower_value_expr(
                path, value, scope, imports, signatures, structs, enums, span,
            )?;
            if value_type != ValueType::String {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    format!("`num.{name}` expects a string argument"),
                    &ValueType::String,
                    &value_type,
                ));
            }
            let (ok_type, expr) = match name.as_str() {
                "parse_i64" => (
                    ValueType::Int,
                    ValueExpr::NumParseI64 {
                        value: Box::new(lowered_value),
                    },
                ),
                "parse_u64" => (
                    ValueType::U64,
                    ValueExpr::NumParseU64 {
                        value: Box::new(lowered_value),
                    },
                ),
                "parse_f64" => (
                    ValueType::Float,
                    ValueExpr::NumParseF64 {
                        value: Box::new(lowered_value),
                    },
                ),
                _ => unreachable!("num parse dispatcher only passes known calls"),
            };
            Ok((
                ValueType::Enum("Result".to_string(), vec![ok_type, num_error]),
                expr,
            ))
        }
        "to_string" => {
            let [value] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`num.to_string` expects exactly one argument",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (value_type, lowered_value) = lower_value_expr(
                path, value, scope, imports, signatures, structs, enums, span,
            )?;
            if !matches!(
                value_type,
                ValueType::Int
                    | ValueType::I32
                    | ValueType::U32
                    | ValueType::U64
                    | ValueType::Float
            ) {
                return Err(type_mismatch(
                    path,
                    span,
                    "`num.to_string` expects an i64, i32, u32, u64, or f64 value",
                ));
            }
            Ok((
                ValueType::String,
                ValueExpr::NumToString {
                    value: Box::new(lowered_value),
                    value_type,
                },
            ))
        }
        "checked_add" | "checked_sub" | "checked_mul" | "wrapping_add" | "wrapping_sub"
        | "wrapping_mul" => {
            let [left, right] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    format!("`num.{name}` expects exactly two integer arguments"),
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let ((left_type, lowered_left), (right_type, lowered_right)) = lower_binary_operands(
                path, left, right, scope, imports, signatures, structs, enums, span,
            )?;
            if left_type != right_type || !left_type.is_integer() {
                return Err(type_mismatch(
                    path,
                    span,
                    format!("`num.{name}` expects two matching integer operands"),
                ));
            }
            let op = match name.as_str() {
                "checked_add" | "wrapping_add" => BinaryOp::Add,
                "checked_sub" | "wrapping_sub" => BinaryOp::Subtract,
                "checked_mul" | "wrapping_mul" => BinaryOp::Multiply,
                _ => unreachable!("num binary dispatcher only passes known calls"),
            };
            let function = if name.starts_with("checked_") {
                NumBinaryFunction::Checked
            } else {
                NumBinaryFunction::Wrapping
            };
            let result_type = if function == NumBinaryFunction::Checked {
                ValueType::Enum("Option".to_string(), vec![left_type.clone()])
            } else {
                left_type.clone()
            };
            Ok((
                result_type,
                ValueExpr::NumBinary {
                    function,
                    op,
                    left: Box::new(lowered_left),
                    right: Box::new(lowered_right),
                    value_type: left_type,
                },
            ))
        }
        _ => unreachable!("num builtin dispatcher only passes known calls"),
    }
}
