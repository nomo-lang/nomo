use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ResultMapErrInstance {
    pub(super) ok_type: ValueType,
    pub(super) source_err_type: ValueType,
    pub(super) target_err_type: ValueType,
    pub(super) converter: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ResultUnwrapOrInstance {
    pub(super) ok_type: ValueType,
    pub(super) err_type: ValueType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ResultMapInstance {
    pub(super) source_ok_type: ValueType,
    pub(super) target_ok_type: ValueType,
    pub(super) err_type: ValueType,
    pub(super) converter: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ResultAndThenInstance {
    pub(super) source_ok_type: ValueType,
    pub(super) target_ok_type: ValueType,
    pub(super) err_type: ValueType,
    pub(super) converter: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct OptionUnwrapOrInstance {
    pub(super) payload_type: ValueType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct OptionMapInstance {
    pub(super) source_type: ValueType,
    pub(super) target_type: ValueType,
    pub(super) converter: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct OptionAndThenInstance {
    pub(super) source_type: ValueType,
    pub(super) target_type: ValueType,
    pub(super) converter: String,
}

pub(super) fn emit_result_map_err_helper(out: &mut String, instance: &ResultMapErrInstance) {
    let source_args = vec![instance.ok_type.clone(), instance.source_err_type.clone()];
    let target_args = vec![instance.ok_type.clone(), instance.target_err_type.clone()];
    let helper_name = c_result_map_err_helper_ident(instance);
    out.push_str("static ");
    out.push_str(&c_enum_ident("Result", &target_args));
    out.push(' ');
    out.push_str(&helper_name);
    out.push('(');
    out.push_str(&c_enum_ident("Result", &source_args));
    out.push_str(" input) {\n");
    out.push_str("    if (input.tag == ");
    out.push_str(&c_enum_variant_ident("Result", &source_args, "Err"));
    out.push_str(") {\n");
    out.push_str("        return (");
    out.push_str(&c_enum_ident("Result", &target_args));
    out.push_str("){.tag = ");
    out.push_str(&c_enum_variant_ident("Result", &target_args, "Err"));
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = ");
    out.push_str(&c_fn_ident(&instance.converter));
    out.push_str("(input.payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(")};\n");
    out.push_str("    }\n");
    out.push_str("    return (");
    out.push_str(&c_enum_ident("Result", &target_args));
    out.push_str("){.tag = ");
    out.push_str(&c_enum_variant_ident("Result", &target_args, "Ok"));
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Ok"));
    out.push_str(" = input.payload.");
    out.push_str(&c_payload_ident("Ok"));
    out.push_str("};\n");
    out.push_str("}\n");
}

pub(super) fn emit_result_unwrap_or_helper(out: &mut String, instance: &ResultUnwrapOrInstance) {
    let result_args = vec![instance.ok_type.clone(), instance.err_type.clone()];
    let helper_name = c_result_unwrap_or_helper_ident(instance);
    out.push_str("static ");
    out.push_str(&c_type(&instance.ok_type));
    out.push(' ');
    out.push_str(&helper_name);
    out.push('(');
    out.push_str(&c_enum_ident("Result", &result_args));
    out.push_str(" input, ");
    out.push_str(&c_type(&instance.ok_type));
    out.push_str(" default_value) {\n");
    out.push_str("    if (input.tag == ");
    out.push_str(&c_enum_variant_ident("Result", &result_args, "Ok"));
    out.push_str(") {\n");
    if value_type_needs_release(&instance.ok_type) {
        emit_value_release_in_place(out, &instance.ok_type, "default_value", 2);
    }
    out.push_str("        return input.payload.");
    out.push_str(&c_payload_ident("Ok"));
    out.push_str(";\n");
    out.push_str("    }\n");
    if value_type_needs_release(&instance.err_type) {
        let value = format!("input.payload.{}", c_payload_ident("Err"));
        emit_value_release_in_place(out, &instance.err_type, &value, 1);
    }
    out.push_str("    return default_value;\n");
    out.push_str("}\n");
}

pub(super) fn emit_result_map_helper(out: &mut String, instance: &ResultMapInstance) {
    let source_args = vec![instance.source_ok_type.clone(), instance.err_type.clone()];
    let target_args = vec![instance.target_ok_type.clone(), instance.err_type.clone()];
    let helper_name = c_result_map_helper_ident(instance);
    out.push_str("static ");
    out.push_str(&c_enum_ident("Result", &target_args));
    out.push(' ');
    out.push_str(&helper_name);
    out.push('(');
    out.push_str(&c_enum_ident("Result", &source_args));
    out.push_str(" input) {\n");
    out.push_str("    if (input.tag == ");
    out.push_str(&c_enum_variant_ident("Result", &source_args, "Ok"));
    out.push_str(") {\n");
    out.push_str("        return (");
    out.push_str(&c_enum_ident("Result", &target_args));
    out.push_str("){.tag = ");
    out.push_str(&c_enum_variant_ident("Result", &target_args, "Ok"));
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Ok"));
    out.push_str(" = ");
    out.push_str(&c_fn_ident(&instance.converter));
    out.push_str("(input.payload.");
    out.push_str(&c_payload_ident("Ok"));
    out.push_str(")};\n");
    out.push_str("    }\n");
    out.push_str("    return (");
    out.push_str(&c_enum_ident("Result", &target_args));
    out.push_str("){.tag = ");
    out.push_str(&c_enum_variant_ident("Result", &target_args, "Err"));
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = input.payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str("};\n");
    out.push_str("}\n");
}

pub(super) fn emit_result_and_then_helper(out: &mut String, instance: &ResultAndThenInstance) {
    let source_args = vec![instance.source_ok_type.clone(), instance.err_type.clone()];
    let target_args = vec![instance.target_ok_type.clone(), instance.err_type.clone()];
    let helper_name = c_result_and_then_helper_ident(instance);
    out.push_str("static ");
    out.push_str(&c_enum_ident("Result", &target_args));
    out.push(' ');
    out.push_str(&helper_name);
    out.push('(');
    out.push_str(&c_enum_ident("Result", &source_args));
    out.push_str(" input) {\n");
    out.push_str("    if (input.tag == ");
    out.push_str(&c_enum_variant_ident("Result", &source_args, "Ok"));
    out.push_str(") {\n");
    out.push_str("        return ");
    out.push_str(&c_fn_ident(&instance.converter));
    out.push_str("(input.payload.");
    out.push_str(&c_payload_ident("Ok"));
    out.push_str(");\n");
    out.push_str("    }\n");
    out.push_str("    return (");
    out.push_str(&c_enum_ident("Result", &target_args));
    out.push_str("){.tag = ");
    out.push_str(&c_enum_variant_ident("Result", &target_args, "Err"));
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = input.payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str("};\n");
    out.push_str("}\n");
}

pub(super) fn emit_option_unwrap_or_helper(out: &mut String, instance: &OptionUnwrapOrInstance) {
    let option_args = vec![instance.payload_type.clone()];
    let helper_name = c_option_unwrap_or_helper_ident(instance);
    out.push_str("static ");
    out.push_str(&c_type(&instance.payload_type));
    out.push(' ');
    out.push_str(&helper_name);
    out.push('(');
    out.push_str(&c_enum_ident("Option", &option_args));
    out.push_str(" input, ");
    out.push_str(&c_type(&instance.payload_type));
    out.push_str(" default_value) {\n");
    out.push_str("    if (input.tag == ");
    out.push_str(&c_enum_variant_ident("Option", &option_args, "Some"));
    out.push_str(") {\n");
    if value_type_needs_release(&instance.payload_type) {
        emit_value_release_in_place(out, &instance.payload_type, "default_value", 2);
    }
    out.push_str("        return input.payload.");
    out.push_str(&c_payload_ident("Some"));
    out.push_str(";\n");
    out.push_str("    }\n");
    out.push_str("    return default_value;\n");
    out.push_str("}\n");
}

pub(super) fn emit_option_map_helper(out: &mut String, instance: &OptionMapInstance) {
    let source_args = vec![instance.source_type.clone()];
    let target_args = vec![instance.target_type.clone()];
    let helper_name = c_option_map_helper_ident(instance);
    out.push_str("static ");
    out.push_str(&c_enum_ident("Option", &target_args));
    out.push(' ');
    out.push_str(&helper_name);
    out.push('(');
    out.push_str(&c_enum_ident("Option", &source_args));
    out.push_str(" input) {\n");
    out.push_str("    if (input.tag == ");
    out.push_str(&c_enum_variant_ident("Option", &source_args, "Some"));
    out.push_str(") {\n");
    out.push_str("        return (");
    out.push_str(&c_enum_ident("Option", &target_args));
    out.push_str("){.tag = ");
    out.push_str(&c_enum_variant_ident("Option", &target_args, "Some"));
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Some"));
    out.push_str(" = ");
    out.push_str(&c_fn_ident(&instance.converter));
    out.push_str("(input.payload.");
    out.push_str(&c_payload_ident("Some"));
    out.push_str(")};\n");
    out.push_str("    }\n");
    out.push_str("    return (");
    out.push_str(&c_enum_ident("Option", &target_args));
    out.push_str("){.tag = ");
    out.push_str(&c_enum_variant_ident("Option", &target_args, "None"));
    out.push_str("};\n");
    out.push_str("}\n");
}

pub(super) fn emit_option_and_then_helper(out: &mut String, instance: &OptionAndThenInstance) {
    let source_args = vec![instance.source_type.clone()];
    let target_args = vec![instance.target_type.clone()];
    let helper_name = c_option_and_then_helper_ident(instance);
    out.push_str("static ");
    out.push_str(&c_enum_ident("Option", &target_args));
    out.push(' ');
    out.push_str(&helper_name);
    out.push('(');
    out.push_str(&c_enum_ident("Option", &source_args));
    out.push_str(" input) {\n");
    out.push_str("    if (input.tag == ");
    out.push_str(&c_enum_variant_ident("Option", &source_args, "Some"));
    out.push_str(") {\n");
    out.push_str("        return ");
    out.push_str(&c_fn_ident(&instance.converter));
    out.push_str("(input.payload.");
    out.push_str(&c_payload_ident("Some"));
    out.push_str(");\n");
    out.push_str("    }\n");
    out.push_str("    return (");
    out.push_str(&c_enum_ident("Option", &target_args));
    out.push_str("){.tag = ");
    out.push_str(&c_enum_variant_ident("Option", &target_args, "None"));
    out.push_str("};\n");
    out.push_str("}\n");
}
