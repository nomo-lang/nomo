use super::*;

pub(super) fn emit_num_checked_binary_helper(
    out: &mut String,
    instance: &NumCheckedBinaryInstance,
) {
    let option = c_enum_ident("Option", std::slice::from_ref(&instance.value_type));
    let some = c_enum_variant_ident("Option", std::slice::from_ref(&instance.value_type), "Some");
    let none = c_enum_variant_ident("Option", std::slice::from_ref(&instance.value_type), "None");
    let c_type = c_type(&instance.value_type);
    let helper = num_checked_binary_helper_name(&instance.op, &instance.value_type);
    out.push_str("static ");
    out.push_str(&option);
    out.push(' ');
    out.push_str(helper);
    out.push('(');
    out.push_str(&c_type);
    out.push_str(" left, ");
    out.push_str(&c_type);
    out.push_str(" right) {\n");
    emit_num_checked_overflow_guard(out, instance, &option, &none);
    out.push_str("    return (");
    out.push_str(&option);
    out.push_str("){.tag = ");
    out.push_str(&some);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Some"));
    out.push_str(" = ");
    out.push_str(num_wrapping_binary_helper_name(
        &instance.op,
        &instance.value_type,
    ));
    out.push_str("(left, right)};\n");
    out.push_str("}\n");
}

fn emit_num_checked_overflow_guard(
    out: &mut String,
    instance: &NumCheckedBinaryInstance,
    option: &str,
    none: &str,
) {
    let condition = num_checked_overflow_condition(&instance.op, &instance.value_type);
    out.push_str("    if (");
    out.push_str(condition);
    out.push_str(") { return (");
    out.push_str(option);
    out.push_str("){.tag = ");
    out.push_str(none);
    out.push_str("}; }\n");
}

fn num_checked_overflow_condition(op: &BinaryOp, value_type: &ValueType) -> &'static str {
    match (op, value_type) {
        (BinaryOp::Add, ValueType::Int) => {
            "(right > 0 && left > LLONG_MAX - right) || (right < 0 && left < LLONG_MIN - right)"
        }
        (BinaryOp::Subtract, ValueType::Int) => {
            "(right < 0 && left > LLONG_MAX + right) || (right > 0 && left < LLONG_MIN + right)"
        }
        (BinaryOp::Multiply, ValueType::Int) => {
            "left != 0 && right != 0 && ((left == -1 && right == LLONG_MIN) || (right == -1 && left == LLONG_MIN) || (left > 0 ? (right > 0 ? left > LLONG_MAX / right : right < LLONG_MIN / left) : (right > 0 ? left < LLONG_MIN / right : left < LLONG_MAX / right)))"
        }
        (BinaryOp::Add, ValueType::I32) => {
            "(right > 0 && left > INT32_MAX - right) || (right < 0 && left < INT32_MIN - right)"
        }
        (BinaryOp::Subtract, ValueType::I32) => {
            "(right < 0 && left > INT32_MAX + right) || (right > 0 && left < INT32_MIN + right)"
        }
        (BinaryOp::Multiply, ValueType::I32) => {
            "left != 0 && right != 0 && ((left == -1 && right == INT32_MIN) || (right == -1 && left == INT32_MIN) || (left > 0 ? (right > 0 ? left > INT32_MAX / right : right < INT32_MIN / left) : (right > 0 ? left < INT32_MIN / right : left < INT32_MAX / right)))"
        }
        (BinaryOp::Add, ValueType::U32) => "left > UINT32_MAX - right",
        (BinaryOp::Subtract, ValueType::U32) => "left < right",
        (BinaryOp::Multiply, ValueType::U32) => "right != 0 && left > UINT32_MAX / right",
        (BinaryOp::Add, ValueType::U64) => "left > UINT64_MAX - right",
        (BinaryOp::Subtract, ValueType::U64) => "left < right",
        (BinaryOp::Multiply, ValueType::U64) => "right != 0 && left > UINT64_MAX / right",
        _ => unreachable!("num checked helpers only support integer add/sub/mul"),
    }
}
