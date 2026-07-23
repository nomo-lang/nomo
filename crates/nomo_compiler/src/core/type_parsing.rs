use super::*;

pub(super) const OPAQUE_HANDLE_ARITY: usize = usize::MAX;
const OPAQUE_HANDLE_STRUCT_PACKAGE: &str = "__nomo_ffi_opaque";

pub(super) fn struct_type_names(structs: &HashMap<String, StructType>) -> Vec<(String, usize)> {
    structs
        .values()
        .map(|item| {
            let arity = if is_opaque_handle_struct(item) {
                OPAQUE_HANDLE_ARITY
            } else {
                item.type_params.len()
            };
            (item.name.clone(), arity)
        })
        .collect()
}

pub(super) fn opaque_handle_struct(name: &str) -> StructType {
    StructType {
        package: OPAQUE_HANDLE_STRUCT_PACKAGE.to_string(),
        name: name.to_string(),
        type_params: Vec::new(),
        fields: Vec::new(),
    }
}

pub(super) fn is_opaque_handle_struct(item: &StructType) -> bool {
    item.package == OPAQUE_HANDLE_STRUCT_PACKAGE
}

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
    let struct_names = struct_type_names(structs);
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
        [name] if name == crate::ast::EXTERN_C_CALLBACK_TYPE_PATH => {
            let (return_ref, param_refs) = type_ref.args.split_last()?;
            let params = param_refs
                .iter()
                .map(|param| {
                    parse_value_type_with_names(param, struct_names, enum_names, type_params)
                })
                .collect::<Option<Vec<_>>>()?;
            let return_type =
                parse_value_type_with_names(return_ref, struct_names, enum_names, type_params)?;
            Some(ValueType::ExternCallback {
                params,
                return_type: Box::new(return_type),
            })
        }
        [name] if name == "string" && type_ref.args.is_empty() => Some(ValueType::String),
        [name] if name == "CString" && type_ref.args.is_empty() => Some(ValueType::CString),
        [name] if name == "Opaque" && type_ref.args.is_empty() => Some(ValueType::Opaque),
        [name] if name == "Owned" || name == "Borrowed" => {
            let [inner] = type_ref.args.as_slice() else {
                return None;
            };
            let ValueType::OpaqueHandle(handle) =
                parse_value_type_with_names(inner, struct_names, enum_names, type_params)?
            else {
                return None;
            };
            Some(if name == "Owned" {
                ValueType::OwnedHandle(handle)
            } else {
                ValueType::BorrowedHandle(handle)
            })
        }
        [name] if name == "Nullable" => {
            let [inner] = type_ref.args.as_slice() else {
                return None;
            };
            let inner = parse_value_type_with_names(inner, struct_names, enum_names, type_params)?;
            is_ffi_handle_type(&inner).then(|| ValueType::Nullable(Box::new(inner)))
        }
        [name] if name == "i64" && type_ref.args.is_empty() => Some(ValueType::Int),
        [name] if name == "i32" && type_ref.args.is_empty() => Some(ValueType::I32),
        [name] if name == "u32" && type_ref.args.is_empty() => Some(ValueType::U32),
        [name] if matches!(name.as_str(), "u64" | "ui64") && type_ref.args.is_empty() => {
            Some(ValueType::U64)
        }
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
            if arity == OPAQUE_HANDLE_ARITY {
                return type_ref
                    .args
                    .is_empty()
                    .then(|| ValueType::OpaqueHandle(name.to_string()));
            }
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

pub(super) fn is_ffi_handle_type(value_type: &ValueType) -> bool {
    matches!(
        value_type,
        ValueType::OpaqueHandle(_) | ValueType::OwnedHandle(_) | ValueType::BorrowedHandle(_)
    )
}
