use super::*;
pub(super) fn is_math_builtin_call(callee: &[String]) -> bool {
    matches!(
        callee,
        [module, name]
            if module == "math"
                && matches!(
                    name.as_str(),
                    "abs"
                        | "min"
                        | "max"
                        | "floor"
                        | "ceil"
                        | "round"
                        | "sqrt"
                        | "pow"
                        | "sin"
                        | "cos"
                )
    )
}
pub(super) fn lower_math_builtin(
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
        unreachable!("math builtin dispatcher only passes qualified calls")
    };
    debug_assert_eq!(module, "math");
    match name.as_str() {
        "abs" => {
            let [value] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`math.abs` expects exactly one numeric argument",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (value_type, lowered) = lower_value_expr(
                path, value, scope, imports, signatures, structs, enums, span,
            )?;
            if !value_type.is_numeric() {
                return Err(type_mismatch(
                    path,
                    span,
                    "`math.abs` expects a numeric value",
                ));
            }
            Ok((
                value_type.clone(),
                ValueExpr::MathUnary {
                    function: MathUnaryFunction::Abs,
                    value: Box::new(lowered),
                    value_type,
                },
            ))
        }
        "floor" | "ceil" | "round" | "sqrt" | "sin" | "cos" => {
            let [value] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    format!("`math.{name}` expects exactly one f64 argument"),
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (value_type, lowered) = lower_value_expr(
                path, value, scope, imports, signatures, structs, enums, span,
            )?;
            if value_type != ValueType::Float {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    format!("`math.{name}` expects an f64 value"),
                    &ValueType::Float,
                    &value_type,
                ));
            }
            let function = match name.as_str() {
                "floor" => MathUnaryFunction::Floor,
                "ceil" => MathUnaryFunction::Ceil,
                "round" => MathUnaryFunction::Round,
                "sqrt" => MathUnaryFunction::Sqrt,
                "sin" => MathUnaryFunction::Sin,
                "cos" => MathUnaryFunction::Cos,
                _ => unreachable!("math builtin dispatcher only passes known calls"),
            };
            Ok((
                ValueType::Float,
                ValueExpr::MathUnary {
                    function,
                    value: Box::new(lowered),
                    value_type: ValueType::Float,
                },
            ))
        }
        "min" | "max" => {
            let [left, right] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    format!("`math.{name}` expects exactly two matching numeric arguments"),
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (left_type, lowered_left) =
                lower_value_expr(path, left, scope, imports, signatures, structs, enums, span)?;
            let (right_type, lowered_right) = lower_value_expr(
                path, right, scope, imports, signatures, structs, enums, span,
            )?;
            if left_type != right_type || !left_type.is_numeric() {
                return Err(type_mismatch(
                    path,
                    span,
                    format!("`math.{name}` expects two matching numeric values"),
                ));
            }
            let function = if name == "min" {
                MathBinaryFunction::Min
            } else {
                MathBinaryFunction::Max
            };
            Ok((
                left_type.clone(),
                ValueExpr::MathBinary {
                    function,
                    left: Box::new(lowered_left),
                    right: Box::new(lowered_right),
                    value_type: left_type,
                },
            ))
        }
        "pow" => {
            let [left, right] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`math.pow` expects exactly two f64 arguments",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (left_type, lowered_left) =
                lower_value_expr(path, left, scope, imports, signatures, structs, enums, span)?;
            let (right_type, lowered_right) = lower_value_expr(
                path, right, scope, imports, signatures, structs, enums, span,
            )?;
            if left_type != ValueType::Float || right_type != ValueType::Float {
                return Err(type_mismatch(
                    path,
                    span,
                    "`math.pow` expects two f64 values",
                ));
            }
            Ok((
                ValueType::Float,
                ValueExpr::MathBinary {
                    function: MathBinaryFunction::Pow,
                    left: Box::new(lowered_left),
                    right: Box::new(lowered_right),
                    value_type: ValueType::Float,
                },
            ))
        }
        _ => unreachable!("math builtin dispatcher only passes known calls"),
    }
}
