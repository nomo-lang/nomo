use super::*;

pub(super) fn subst_type(
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
                .map(|arg| subst_type(arg, type_params, args))
                .collect(),
        ),
        ValueType::Struct(name, nested_args) => ValueType::Struct(
            name.clone(),
            nested_args
                .iter()
                .map(|arg| subst_type(arg, type_params, args))
                .collect(),
        ),
        ValueType::Array(element) => {
            ValueType::Array(Box::new(subst_type(element, type_params, args)))
        }
        _ => value_type.clone(),
    }
}

pub(super) fn emit_string_data_expr(out: &mut String, expr: &ValueExpr) {
    out.push('(');
    emit_expr(out, expr);
    out.push_str(").data");
}

pub(super) fn c_binary_op(op: &BinaryOp) -> &'static str {
    match op {
        BinaryOp::LogicalOr => "||",
        BinaryOp::LogicalAnd => "&&",
        BinaryOp::Add => "+",
        BinaryOp::Subtract => "-",
        BinaryOp::BitOr => "|",
        BinaryOp::BitXor => "^",
        BinaryOp::Multiply => "*",
        BinaryOp::Divide => "/",
        BinaryOp::Remainder => "%",
        BinaryOp::ShiftLeft => "<<",
        BinaryOp::ShiftRight => ">>",
        BinaryOp::BitAnd => "&",
        BinaryOp::BitAndNot => "&^",
        BinaryOp::Equal => "==",
        BinaryOp::NotEqual => "!=",
        BinaryOp::Less => "<",
        BinaryOp::LessEqual => "<=",
        BinaryOp::Greater => ">",
        BinaryOp::GreaterEqual => ">=",
    }
}

pub(super) fn math_unary_function_name(
    function: MathUnaryFunction,
    value_type: &ValueType,
) -> &'static str {
    match (function, value_type) {
        (MathUnaryFunction::Abs, ValueType::Int) => "nomo_math_abs_i64",
        (MathUnaryFunction::Abs, ValueType::I32) => "nomo_math_abs_i32",
        (MathUnaryFunction::Abs, ValueType::U32) => "nomo_math_abs_u32",
        (MathUnaryFunction::Abs, ValueType::U64) => "nomo_math_abs_u64",
        (MathUnaryFunction::Abs, ValueType::Float) => "nomo_math_abs_f64",
        (MathUnaryFunction::Floor, ValueType::Float) => "floor",
        (MathUnaryFunction::Ceil, ValueType::Float) => "ceil",
        (MathUnaryFunction::Round, ValueType::Float) => "round",
        (MathUnaryFunction::Sqrt, ValueType::Float) => "sqrt",
        (MathUnaryFunction::Sin, ValueType::Float) => "sin",
        (MathUnaryFunction::Cos, ValueType::Float) => "cos",
        _ => unreachable!("compiler only emits well-typed math unary calls"),
    }
}

pub(super) fn math_binary_function_name(
    function: MathBinaryFunction,
    value_type: &ValueType,
) -> &'static str {
    match (function, value_type) {
        (MathBinaryFunction::Min, ValueType::Int) => "nomo_math_min_i64",
        (MathBinaryFunction::Min, ValueType::I32) => "nomo_math_min_i32",
        (MathBinaryFunction::Min, ValueType::U32) => "nomo_math_min_u32",
        (MathBinaryFunction::Min, ValueType::U64) => "nomo_math_min_u64",
        (MathBinaryFunction::Min, ValueType::Float) => "nomo_math_min_f64",
        (MathBinaryFunction::Max, ValueType::Int) => "nomo_math_max_i64",
        (MathBinaryFunction::Max, ValueType::I32) => "nomo_math_max_i32",
        (MathBinaryFunction::Max, ValueType::U32) => "nomo_math_max_u32",
        (MathBinaryFunction::Max, ValueType::U64) => "nomo_math_max_u64",
        (MathBinaryFunction::Max, ValueType::Float) => "nomo_math_max_f64",
        (MathBinaryFunction::Pow, ValueType::Float) => "pow",
        _ => unreachable!("compiler only emits well-typed math binary calls"),
    }
}

pub(super) fn checked_binary_helper(op: &BinaryOp, value_type: &ValueType) -> Option<&'static str> {
    match (op, value_type) {
        (BinaryOp::Add, ValueType::Int) => Some("nomo_add_i64"),
        (BinaryOp::Subtract, ValueType::Int) => Some("nomo_sub_i64"),
        (BinaryOp::Multiply, ValueType::Int) => Some("nomo_mul_i64"),
        (BinaryOp::Divide, ValueType::Int) => Some("nomo_div_i64"),
        (BinaryOp::Remainder, ValueType::Int) => Some("nomo_rem_i64"),
        (BinaryOp::Add, ValueType::I32) => Some("nomo_add_i32"),
        (BinaryOp::Subtract, ValueType::I32) => Some("nomo_sub_i32"),
        (BinaryOp::Multiply, ValueType::I32) => Some("nomo_mul_i32"),
        (BinaryOp::Divide, ValueType::I32) => Some("nomo_div_i32"),
        (BinaryOp::Remainder, ValueType::I32) => Some("nomo_rem_i32"),
        (BinaryOp::Divide, ValueType::U32) => Some("nomo_div_u32"),
        (BinaryOp::Remainder, ValueType::U32) => Some("nomo_rem_u32"),
        (BinaryOp::Divide, ValueType::U64) => Some("nomo_div_u64"),
        (BinaryOp::Remainder, ValueType::U64) => Some("nomo_rem_u64"),
        (BinaryOp::Divide, ValueType::Float) => Some("nomo_div_f64"),
        (BinaryOp::ShiftLeft, ValueType::Int) => Some("nomo_shl_i64"),
        (BinaryOp::ShiftRight, ValueType::Int) => Some("nomo_shr_i64"),
        (BinaryOp::ShiftLeft, ValueType::I32) => Some("nomo_shl_i32"),
        (BinaryOp::ShiftRight, ValueType::I32) => Some("nomo_shr_i32"),
        (BinaryOp::ShiftLeft, ValueType::U32) => Some("nomo_shl_u32"),
        (BinaryOp::ShiftRight, ValueType::U32) => Some("nomo_shr_u32"),
        (BinaryOp::ShiftLeft, ValueType::U64) => Some("nomo_shl_u64"),
        (BinaryOp::ShiftRight, ValueType::U64) => Some("nomo_shr_u64"),
        _ => None,
    }
}

pub(super) fn c_unary_op(op: &UnaryOp) -> &'static str {
    match op {
        UnaryOp::Not => "!",
        UnaryOp::Negate => "-",
    }
}

pub(super) fn num_to_string_helper_name(value_type: &ValueType) -> &'static str {
    match value_type {
        ValueType::Int => "nomo_num_i64_to_string",
        ValueType::I32 => "nomo_num_i32_to_string",
        ValueType::U32 => "nomo_num_u32_to_string",
        ValueType::U64 => "nomo_num_u64_to_string",
        ValueType::Float => "nomo_num_f64_to_string",
        _ => unreachable!("num.to_string only lowers supported numeric types"),
    }
}

pub(super) fn num_checked_binary_helper_name(
    op: &BinaryOp,
    value_type: &ValueType,
) -> &'static str {
    match (op, value_type) {
        (BinaryOp::Add, ValueType::Int) => "nomo_num_checked_add_i64",
        (BinaryOp::Subtract, ValueType::Int) => "nomo_num_checked_sub_i64",
        (BinaryOp::Multiply, ValueType::Int) => "nomo_num_checked_mul_i64",
        (BinaryOp::Add, ValueType::I32) => "nomo_num_checked_add_i32",
        (BinaryOp::Subtract, ValueType::I32) => "nomo_num_checked_sub_i32",
        (BinaryOp::Multiply, ValueType::I32) => "nomo_num_checked_mul_i32",
        (BinaryOp::Add, ValueType::U32) => "nomo_num_checked_add_u32",
        (BinaryOp::Subtract, ValueType::U32) => "nomo_num_checked_sub_u32",
        (BinaryOp::Multiply, ValueType::U32) => "nomo_num_checked_mul_u32",
        (BinaryOp::Add, ValueType::U64) => "nomo_num_checked_add_u64",
        (BinaryOp::Subtract, ValueType::U64) => "nomo_num_checked_sub_u64",
        (BinaryOp::Multiply, ValueType::U64) => "nomo_num_checked_mul_u64",
        _ => unreachable!("num checked helpers only lower integer add/sub/mul"),
    }
}

pub(super) fn num_wrapping_binary_helper_name(
    op: &BinaryOp,
    value_type: &ValueType,
) -> &'static str {
    match (op, value_type) {
        (BinaryOp::Add, ValueType::Int) => "nomo_num_wrapping_add_i64",
        (BinaryOp::Subtract, ValueType::Int) => "nomo_num_wrapping_sub_i64",
        (BinaryOp::Multiply, ValueType::Int) => "nomo_num_wrapping_mul_i64",
        (BinaryOp::Add, ValueType::I32) => "nomo_num_wrapping_add_i32",
        (BinaryOp::Subtract, ValueType::I32) => "nomo_num_wrapping_sub_i32",
        (BinaryOp::Multiply, ValueType::I32) => "nomo_num_wrapping_mul_i32",
        (BinaryOp::Add, ValueType::U32) => "nomo_num_wrapping_add_u32",
        (BinaryOp::Subtract, ValueType::U32) => "nomo_num_wrapping_sub_u32",
        (BinaryOp::Multiply, ValueType::U32) => "nomo_num_wrapping_mul_u32",
        (BinaryOp::Add, ValueType::U64) => "nomo_num_wrapping_add_u64",
        (BinaryOp::Subtract, ValueType::U64) => "nomo_num_wrapping_sub_u64",
        (BinaryOp::Multiply, ValueType::U64) => "nomo_num_wrapping_mul_u64",
        _ => unreachable!("num wrapping helpers only lower integer add/sub/mul"),
    }
}

pub(super) fn c_type(value_type: &ValueType) -> String {
    match value_type {
        ValueType::String => "nomo_string".to_string(),
        ValueType::CString => "nomo_string".to_string(),
        ValueType::Opaque => "void *".to_string(),
        ValueType::OpaqueHandle(_) => "void *".to_string(),
        ValueType::OwnedHandle(_) => "void *".to_string(),
        ValueType::BorrowedHandle(_) => "void *".to_string(),
        ValueType::Nullable(_) => "void *".to_string(),
        ValueType::ExternCallback {
            params,
            return_type,
        } => c_callback_type(params, return_type),
        ValueType::Int => "long long".to_string(),
        ValueType::I32 => "int32_t".to_string(),
        ValueType::U32 => "uint32_t".to_string(),
        ValueType::U64 => "uint64_t".to_string(),
        ValueType::Float => "double".to_string(),
        ValueType::Char => "uint32_t".to_string(),
        ValueType::Bool => "int".to_string(),
        ValueType::Array(element) if is_supported_array_element(element) => c_array_ident(element),
        ValueType::Array(element) => panic!(
            "unsupported Array element type reached C type lowering: {}",
            element.name()
        ),
        ValueType::Struct(name, args) => c_struct_ident(name, args),
        ValueType::Enum(name, args) => c_enum_ident(name, args),
        ValueType::TypeParam(name) => {
            panic!("unsubstituted type parameter reached C codegen: {name}")
        }
        ValueType::Void => "void".to_string(),
        ValueType::Never => "void".to_string(),
    }
}

pub(super) fn c_extern_type(value_type: &ValueType) -> String {
    match value_type {
        ValueType::CString => "const char *".to_string(),
        ValueType::Opaque => "void *".to_string(),
        ValueType::OpaqueHandle(_) => "void *".to_string(),
        ValueType::OwnedHandle(_) => "void *".to_string(),
        ValueType::BorrowedHandle(_) => "void *".to_string(),
        ValueType::Nullable(_) => "void *".to_string(),
        ValueType::ExternCallback {
            params,
            return_type,
        } => c_callback_type(params, return_type),
        _ => c_type(value_type),
    }
}

pub(super) fn result_void_error(value_type: &ValueType) -> Option<Vec<ValueType>> {
    let ValueType::Enum(name, args) = value_type else {
        return None;
    };
    if name == "Result" && args.len() == 2 && args[0] == ValueType::Void {
        Some(args.clone())
    } else {
        None
    }
}

pub(super) fn c_payload_type(value_type: &ValueType) -> String {
    if value_type == &ValueType::Void {
        "char".to_string()
    } else {
        c_type(value_type)
    }
}

pub(super) fn c_zero_value(value_type: &ValueType) -> String {
    match value_type {
        ValueType::String | ValueType::CString => "nomo_string_literal(\"\")".to_string(),
        ValueType::Opaque => "NULL".to_string(),
        ValueType::OpaqueHandle(_) => "NULL".to_string(),
        ValueType::OwnedHandle(_) => "NULL".to_string(),
        ValueType::BorrowedHandle(_) => "NULL".to_string(),
        ValueType::Nullable(_) => "NULL".to_string(),
        ValueType::ExternCallback { .. } => "NULL".to_string(),
        ValueType::Int => "0".to_string(),
        ValueType::I32 | ValueType::U32 | ValueType::U64 => "0".to_string(),
        ValueType::Float => "0.0".to_string(),
        ValueType::Char => "0".to_string(),
        ValueType::Bool => "0".to_string(),
        ValueType::Array(element) if is_supported_array_element(element) => {
            format!("{}_new()", c_array_ident(element))
        }
        ValueType::Array(_) => "0".to_string(),
        ValueType::Struct(name, args) => format!("({}){{0}}", c_struct_ident(name, args)),
        ValueType::Enum(name, args) => format!("({}){{0}}", c_enum_ident(name, args)),
        ValueType::TypeParam(_) => "0".to_string(),
        ValueType::Void | ValueType::Never => "(void)0".to_string(),
    }
}

pub(super) fn c_type_suffix(args: &[ValueType]) -> String {
    if args.is_empty() {
        return String::new();
    }
    let parts = args.iter().map(c_type_name_part).collect::<Vec<_>>();
    format!("_{}", parts.join("_"))
}

pub(super) fn c_type_name_part(value_type: &ValueType) -> String {
    match value_type {
        ValueType::String => "string".to_string(),
        ValueType::CString => "cstring".to_string(),
        ValueType::Opaque => "opaque".to_string(),
        ValueType::OpaqueHandle(name) => format!("handle_{name}"),
        ValueType::OwnedHandle(name) => format!("owned_handle_{name}"),
        ValueType::BorrowedHandle(name) => format!("borrowed_handle_{name}"),
        ValueType::Nullable(inner) => format!("nullable_{}", c_type_name_part(inner)),
        ValueType::ExternCallback {
            params,
            return_type,
        } => format!(
            "callback_{}_to_{}",
            params
                .iter()
                .map(c_type_name_part)
                .collect::<Vec<_>>()
                .join("_"),
            c_type_name_part(return_type)
        ),
        ValueType::Int => "i64".to_string(),
        ValueType::I32 => "i32".to_string(),
        ValueType::U32 => "u32".to_string(),
        ValueType::U64 => "u64".to_string(),
        ValueType::Float => "f64".to_string(),
        ValueType::Char => "char".to_string(),
        ValueType::Bool => "bool".to_string(),
        ValueType::Array(element) => format!("array_{}", c_type_name_part(element)),
        ValueType::Struct(name, args) => format!("struct_{}{}", name, c_type_suffix(args)),
        ValueType::Enum(name, args) => format!("enum_{}{}", name, c_type_suffix(args)),
        ValueType::TypeParam(name) => format!("param_{name}"),
        ValueType::Void => "void".to_string(),
        ValueType::Never => "never".to_string(),
    }
}

fn c_callback_type(params: &[ValueType], return_type: &ValueType) -> String {
    let params = if params.is_empty() {
        "void".to_string()
    } else {
        params
            .iter()
            .map(c_extern_type)
            .collect::<Vec<_>>()
            .join(", ")
    };
    format!("{} (*)({params})", c_extern_type(return_type))
}

pub(super) fn c_var_ident(name: &str) -> String {
    format!("nomo_{name}")
}

pub(super) fn c_member_ident(name: &str) -> String {
    format!("nomo_member_{name}")
}

pub(super) fn c_payload_ident(variant: &str) -> String {
    format!("nomo_payload_{variant}")
}

pub(super) fn c_fn_ident(name: &str) -> String {
    format!("nomo_fn_{name}")
}

pub(super) fn c_package_ident(package: &str) -> String {
    package
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
        .collect()
}

pub(super) fn c_struct_ident(name: &str, args: &[ValueType]) -> String {
    format!("nomo_struct_{}{}", name, c_type_suffix(args))
}

pub(super) fn c_array_ident(element_type: &ValueType) -> String {
    format!("nomo_array_{}", c_type_name_part(element_type))
}

pub(super) fn c_enum_ident(name: &str, args: &[ValueType]) -> String {
    format!("nomo_enum_{}{}", name, c_type_suffix(args))
}

pub(super) fn c_enum_tag_ident(name: &str, args: &[ValueType]) -> String {
    format!("nomo_enum_{}{}_tag", name, c_type_suffix(args))
}

pub(super) fn c_enum_variant_ident(enum_name: &str, args: &[ValueType], variant: &str) -> String {
    format!("nomo_enum_{}{}_{}", enum_name, c_type_suffix(args), variant)
}

pub(super) fn c_result_map_err_helper_ident(instance: &ResultMapErrInstance) -> String {
    format!(
        "nomo_result_map_err_{}_{}_{}_{}",
        c_type_name_part(&instance.ok_type),
        c_type_name_part(&instance.source_err_type),
        c_type_name_part(&instance.target_err_type),
        instance.converter
    )
}

pub(super) fn c_result_unwrap_or_helper_ident(instance: &ResultUnwrapOrInstance) -> String {
    format!(
        "nomo_result_unwrap_or_{}_{}",
        c_type_name_part(&instance.ok_type),
        c_type_name_part(&instance.err_type)
    )
}

pub(super) fn c_result_map_helper_ident(instance: &ResultMapInstance) -> String {
    format!(
        "nomo_result_map_{}_{}_{}_{}",
        c_type_name_part(&instance.source_ok_type),
        c_type_name_part(&instance.target_ok_type),
        c_type_name_part(&instance.err_type),
        instance.converter
    )
}

pub(super) fn c_result_and_then_helper_ident(instance: &ResultAndThenInstance) -> String {
    format!(
        "nomo_result_and_then_{}_{}_{}_{}",
        c_type_name_part(&instance.source_ok_type),
        c_type_name_part(&instance.target_ok_type),
        c_type_name_part(&instance.err_type),
        instance.converter
    )
}

pub(super) fn c_option_unwrap_or_helper_ident(instance: &OptionUnwrapOrInstance) -> String {
    format!(
        "nomo_option_unwrap_or_{}",
        c_type_name_part(&instance.payload_type)
    )
}

pub(super) fn c_option_map_helper_ident(instance: &OptionMapInstance) -> String {
    format!(
        "nomo_option_map_{}_{}_{}",
        c_type_name_part(&instance.source_type),
        c_type_name_part(&instance.target_type),
        instance.converter
    )
}

pub(super) fn c_option_and_then_helper_ident(instance: &OptionAndThenInstance) -> String {
    format!(
        "nomo_option_and_then_{}_{}_{}",
        c_type_name_part(&instance.source_type),
        c_type_name_part(&instance.target_type),
        instance.converter
    )
}

pub(super) fn c_retain_ident(value_type: &ValueType) -> String {
    match value_type {
        ValueType::String | ValueType::CString => "nomo_string_retain".to_string(),
        ValueType::Array(element_type) => format!("{}_retain", c_array_ident(element_type)),
        ValueType::Struct(name, args) => format!("{}_retain", c_struct_ident(name, args)),
        ValueType::Enum(name, args) => format!("{}_retain", c_enum_ident(name, args)),
        _ => panic!(
            "unsupported retain helper requested for C type: {}",
            value_type.name()
        ),
    }
}

pub(super) fn c_release_ident(value_type: &ValueType) -> String {
    match value_type {
        ValueType::String | ValueType::CString => "nomo_string_release".to_string(),
        ValueType::Array(element_type) => format!("{}_release", c_array_ident(element_type)),
        ValueType::Struct(name, args) => format!("{}_release", c_struct_ident(name, args)),
        ValueType::Enum(name, args) => format!("{}_release", c_enum_ident(name, args)),
        _ => panic!(
            "unsupported release helper requested for C type: {}",
            value_type.name()
        ),
    }
}

pub(super) fn escape_c_string(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            c => escaped.push(c),
        }
    }
    escaped
}

pub(super) fn is_supported_array_element(value_type: &ValueType) -> bool {
    !matches!(
        value_type,
        ValueType::Void | ValueType::Never | ValueType::TypeParam(_)
    )
}
