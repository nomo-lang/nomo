use super::*;

pub(super) fn parse_non_void_type(
    type_ref: &crate::ast::TypeRef,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
) -> Option<ValueType> {
    parse_value_type(type_ref, structs, enums).filter(|value_type| value_type != &ValueType::Void)
}

pub(super) fn parse_value_type(
    type_ref: &crate::ast::TypeRef,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
) -> Option<ValueType> {
    let struct_names = structs
        .values()
        .map(|item| (item.name.clone(), item.type_params.len()))
        .collect::<Vec<_>>();
    let enum_names = enums
        .values()
        .map(|item| (item.name.clone(), item.type_params.len()))
        .collect::<Vec<_>>();
    parse_value_type_with_names(type_ref, &struct_names, &enum_names, &[])
}

pub(super) fn parse_value_type_with_names(
    type_ref: &crate::ast::TypeRef,
    struct_names: &[(String, usize)],
    enum_names: &[(String, usize)],
    type_params: &[String],
) -> Option<ValueType> {
    match type_ref.path.as_slice() {
        [name] if name == "string" && type_ref.args.is_empty() => Some(ValueType::String),
        [name] if name == "i64" && type_ref.args.is_empty() => Some(ValueType::Int),
        [name] if name == "i32" && type_ref.args.is_empty() => Some(ValueType::I32),
        [name] if name == "u32" && type_ref.args.is_empty() => Some(ValueType::U32),
        [name] if name == "u64" && type_ref.args.is_empty() => Some(ValueType::U64),
        [name] if name == "f64" && type_ref.args.is_empty() => Some(ValueType::Float),
        [name] if name == "char" && type_ref.args.is_empty() => Some(ValueType::Char),
        [name] if name == "bool" && type_ref.args.is_empty() => Some(ValueType::Bool),
        [name] if name == "void" && type_ref.args.is_empty() => Some(ValueType::Void),
        [name] if name == "Array" => {
            let [element] = type_ref.args.as_slice() else {
                return None;
            };
            let element_type =
                parse_value_type_with_names(element, struct_names, enum_names, type_params)?;
            Some(ValueType::Array(Box::new(element_type)))
        }
        [name] if struct_names.iter().any(|(item, _)| item == name) => {
            let arity = struct_names
                .iter()
                .find(|(item, _)| item == name)
                .map(|(_, arity)| *arity)?;
            if type_ref.args.len() != arity {
                return None;
            }
            let args = type_ref
                .args
                .iter()
                .map(|arg| parse_value_type_with_names(arg, struct_names, enum_names, type_params))
                .collect::<Option<Vec<_>>>()?;
            Some(ValueType::Struct(name.to_string(), args))
        }
        [name] if enum_names.iter().any(|(item, _)| item == name) => {
            let arity = enum_names
                .iter()
                .find(|(item, _)| item == name)
                .map(|(_, arity)| *arity)?;
            if type_ref.args.len() != arity {
                return None;
            }
            let args = type_ref
                .args
                .iter()
                .map(|arg| parse_value_type_with_names(arg, struct_names, enum_names, type_params))
                .collect::<Option<Vec<_>>>()?;
            Some(ValueType::Enum(name.to_string(), args))
        }
        [name] if type_params.iter().any(|item| item == name) => {
            if !type_ref.args.is_empty() {
                return None;
            }
            Some(ValueType::TypeParam(name.to_string()))
        }
        _ => None,
    }
}
