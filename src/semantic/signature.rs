use crate::ast::{
    ConstDef, EnumDef, EnumVariant, Field, Function, FunctionSignature, InterfaceDef, Param,
    StructDef, TypeRef,
};

pub(super) fn struct_signature(item: &StructDef) -> String {
    format!(
        "{}struct {}{}",
        visibility_prefix(item.public),
        item.name,
        type_params(&item.type_params)
    )
}

pub(super) fn enum_signature(item: &EnumDef) -> String {
    format!(
        "{}enum {}{}",
        visibility_prefix(item.public),
        item.name,
        type_params(&item.type_params)
    )
}

pub(super) fn interface_signature(item: &InterfaceDef) -> String {
    format!("{}interface {}", visibility_prefix(item.public), item.name)
}

pub(super) fn const_signature(item: &ConstDef) -> String {
    format!(
        "{}const {}: {}",
        visibility_prefix(item.public),
        item.name,
        type_ref(&item.type_ref)
    )
}

pub(super) fn function_signature(function: &Function) -> String {
    let params = function
        .params
        .iter()
        .map(param)
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "{}fn {}{}({}) -> {}",
        visibility_prefix(function.public),
        function.name,
        type_params(&function.type_params),
        params,
        type_ref(&function.return_type)
    )
}

pub(super) fn method_signature(receiver: &str, function: &Function) -> String {
    let params = function
        .params
        .iter()
        .map(param)
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "{}fn {receiver}.{}{}({}) -> {}",
        visibility_prefix(function.public),
        function.name,
        type_params(&function.type_params),
        params,
        type_ref(&function.return_type)
    )
}

pub(super) fn extern_function_signature(abi: &str, function: &FunctionSignature) -> String {
    let params = function
        .params
        .iter()
        .map(param)
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "extern \"{}\" fn {}{}({}) -> {}",
        abi,
        function.name,
        type_params(&function.type_params),
        params,
        type_ref(&function.return_type)
    )
}

pub(super) fn interface_method_signature(owner: &str, method: &FunctionSignature) -> String {
    let params = method
        .params
        .iter()
        .map(param)
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "fn {owner}.{}{}({}) -> {}",
        method.name,
        type_params(&method.type_params),
        params,
        type_ref(&method.return_type)
    )
}

pub(super) fn field_signature(owner: &str, field: &Field) -> String {
    format!(
        "{}field {owner}.{}: {}",
        visibility_prefix(field.public),
        field.name,
        type_ref(&field.type_ref)
    )
}

pub(super) fn variant_signature(owner: &str, variant: &EnumVariant) -> String {
    match &variant.payload {
        Some(payload) => format!("variant {owner}.{}({})", variant.name, type_ref(payload)),
        None => format!("variant {owner}.{}", variant.name),
    }
}

fn param(param: &Param) -> String {
    let mutable = if param.mutable { "mut " } else { "" };
    format!("{mutable}{}: {}", param.name, type_ref(&param.type_ref))
}

fn type_params(params: &[String]) -> String {
    if params.is_empty() {
        String::new()
    } else {
        format!("<{}>", params.join(", "))
    }
}

pub(super) fn type_ref(type_ref_value: &TypeRef) -> String {
    let base = type_ref_value.path.join(".");
    if type_ref_value.args.is_empty() {
        base
    } else {
        format!(
            "{base}<{}>",
            type_ref_value
                .args
                .iter()
                .map(type_ref)
                .collect::<Vec<_>>()
                .join(", ")
        )
    }
}

fn visibility_prefix(public: bool) -> &'static str {
    if public { "pub " } else { "" }
}
