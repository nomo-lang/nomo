use nomo_syntax::ast::{
    ConstDef, EnumDef, EnumVariant, Field, Function, FunctionSignature, InterfaceDef, Param,
    StructDef, TypeParamBound, TypeRef,
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
        "{}{}fn {}{}({}) -> {}",
        visibility_prefix(function.public),
        suspend_prefix(function.is_suspend),
        function.name,
        type_params_with_bounds(&function.type_params, &function.type_param_bounds),
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
        "{}{}fn {receiver}.{}{}({}) -> {}",
        visibility_prefix(function.public),
        suspend_prefix(function.is_suspend),
        function.name,
        type_params_with_bounds(&function.type_params, &function.type_param_bounds),
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
        type_params_with_bounds(&function.type_params, &function.type_param_bounds),
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
        "{}fn {owner}.{}{}({}) -> {}",
        suspend_prefix(method.is_suspend),
        method.name,
        type_params_with_bounds(&method.type_params, &method.type_param_bounds),
        params,
        type_ref(&method.return_type)
    )
}

fn suspend_prefix(is_suspend: bool) -> &'static str {
    if is_suspend { "suspend " } else { "" }
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

fn type_params_with_bounds(params: &[String], bounds: &[TypeParamBound]) -> String {
    if params.is_empty() {
        return String::new();
    }
    let params = params
        .iter()
        .map(|parameter| {
            bounds
                .iter()
                .find(|bound| &bound.parameter == parameter)
                .map(|bound| format!("{parameter}: {}", type_ref(&bound.interface)))
                .unwrap_or_else(|| parameter.clone())
        })
        .collect::<Vec<_>>();
    format!("<{}>", params.join(", "))
}

pub(super) fn type_ref(type_ref_value: &TypeRef) -> String {
    if type_ref_value.path.as_slice() == [nomo_syntax::ast::TASK_CALLBACK_TYPE_PATH] {
        let Some((return_type, params)) = type_ref_value.args.split_last() else {
            return "task fn() -> void".to_string();
        };
        return format!(
            "task fn({}) -> {}",
            params.iter().map(type_ref).collect::<Vec<_>>().join(", "),
            type_ref(return_type)
        );
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn renders_task_worker_type_syntax() {
        let worker = TypeRef {
            path: vec![nomo_syntax::ast::TASK_CALLBACK_TYPE_PATH.to_string()],
            args: vec![
                TypeRef {
                    path: vec!["TaskContext".to_string()],
                    args: Vec::new(),
                },
                TypeRef {
                    path: vec!["string".to_string()],
                    args: Vec::new(),
                },
                TypeRef {
                    path: vec!["string".to_string()],
                    args: Vec::new(),
                },
            ],
        };

        assert_eq!(type_ref(&worker), "task fn(TaskContext, string) -> string");
    }

    #[test]
    fn renders_suspend_effect_in_semantic_signatures() {
        let source = "package app.main\n\npub interface Loader {\n    suspend fn load(self) -> string\n}\n\npub suspend fn run() -> string {\n    return \"ready\"\n}\n";
        let symbols = crate::symbols_for_text(Path::new("main.nomo"), source).unwrap();

        let method = symbols.iter().find(|symbol| symbol.name == "load").unwrap();
        assert_eq!(
            method.signature,
            "suspend fn Loader.load(self: Self) -> string"
        );

        let function = symbols.iter().find(|symbol| symbol.name == "run").unwrap();
        assert_eq!(function.signature, "pub suspend fn run() -> string");
    }
}
