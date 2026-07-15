use crate::{StructType, ValueType};
use nomo_target::TargetTriple;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CFieldLayout {
    pub name: String,
    pub offset: u64,
    pub size: u64,
    pub align: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CStructLayout {
    pub name: String,
    pub target: String,
    pub size: u64,
    pub align: u64,
    pub fields: Vec<CFieldLayout>,
}

pub fn compute_repr_c_layout(
    name: &str,
    structs: &[StructType],
    repr_c_structs: &HashSet<String>,
    target: &TargetTriple,
) -> Result<CStructLayout, String> {
    let structs = structs
        .iter()
        .map(|item| (item.name.as_str(), item))
        .collect::<HashMap<_, _>>();
    let mut cache = HashMap::new();
    let mut visiting = HashSet::new();
    compute_struct_layout(
        name,
        &structs,
        repr_c_structs,
        target,
        &mut cache,
        &mut visiting,
    )
}

fn compute_struct_layout(
    name: &str,
    structs: &HashMap<&str, &StructType>,
    repr_c_structs: &HashSet<String>,
    target: &TargetTriple,
    cache: &mut HashMap<String, CStructLayout>,
    visiting: &mut HashSet<String>,
) -> Result<CStructLayout, String> {
    if let Some(layout) = cache.get(name) {
        return Ok(layout.clone());
    }
    if !repr_c_structs.contains(name) {
        return Err(format!("struct `{name}` is not declared `#[repr(C)]`"));
    }
    let item = structs
        .get(name)
        .ok_or_else(|| format!("unknown struct `{name}`"))?;
    if !item.type_params.is_empty() {
        return Err(format!("repr(C) struct `{name}` cannot be generic"));
    }
    if item.fields.is_empty() {
        return Err(format!("repr(C) struct `{name}` cannot be empty"));
    }
    if !visiting.insert(name.to_string()) {
        return Err(format!("recursive repr(C) layout involving `{name}`"));
    }

    let mut offset = 0;
    let mut struct_align = 1;
    let mut fields = Vec::with_capacity(item.fields.len());
    for field in &item.fields {
        let (size, align) = c_type_layout(
            &field.value_type,
            structs,
            repr_c_structs,
            target,
            cache,
            visiting,
        )?;
        offset = align_up(offset, align);
        fields.push(CFieldLayout {
            name: field.name.clone(),
            offset,
            size,
            align,
        });
        offset += size;
        struct_align = struct_align.max(align);
    }
    visiting.remove(name);
    let layout = CStructLayout {
        name: name.to_string(),
        target: target.to_string(),
        size: align_up(offset, struct_align),
        align: struct_align,
        fields,
    };
    cache.insert(name.to_string(), layout.clone());
    Ok(layout)
}

fn c_type_layout(
    value_type: &ValueType,
    structs: &HashMap<&str, &StructType>,
    repr_c_structs: &HashSet<String>,
    target: &TargetTriple,
    cache: &mut HashMap<String, CStructLayout>,
    visiting: &mut HashSet<String>,
) -> Result<(u64, u64), String> {
    let fixed = match value_type {
        ValueType::Int | ValueType::U64 | ValueType::Float => Some((8, 8)),
        ValueType::I32 | ValueType::U32 | ValueType::Char | ValueType::Bool => Some((4, 4)),
        ValueType::Opaque
        | ValueType::OpaqueHandle(_)
        | ValueType::BorrowedHandle(_)
        | ValueType::Nullable(_) => {
            let bytes = u64::from(target.abi().pointer_width / 8);
            Some((bytes, bytes))
        }
        _ => None,
    };
    if let Some(layout) = fixed {
        return Ok(layout);
    }
    if let ValueType::Struct(name, args) = value_type {
        if !args.is_empty() {
            return Err(format!("generic struct `{name}` is not repr(C)-safe"));
        }
        let layout = compute_struct_layout(name, structs, repr_c_structs, target, cache, visiting)?;
        return Ok((layout.size, layout.align));
    }
    Err(format!(
        "type `{}` is not supported in repr(C) layout",
        value_type.name()
    ))
}

fn align_up(value: u64, alignment: u64) -> u64 {
    let remainder = value % alignment;
    if remainder == 0 {
        value
    } else {
        value + alignment - remainder
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::StructField;

    fn packet() -> StructType {
        StructType {
            package: "app.main".to_string(),
            name: "Packet".to_string(),
            type_params: Vec::new(),
            fields: vec![
                StructField {
                    name: "tag".to_string(),
                    value_type: ValueType::I32,
                },
                StructField {
                    name: "value".to_string(),
                    value_type: ValueType::U64,
                },
                StructField {
                    name: "ready".to_string(),
                    value_type: ValueType::Bool,
                },
            ],
        }
    }

    #[test]
    fn validates_layout_fixture_on_linux_and_windows_targets() {
        let structs = vec![packet()];
        let repr = HashSet::from(["Packet".to_string()]);
        for target in ["x86_64-unknown-linux-gnu", "x86_64-pc-windows-msvc"] {
            let target = target.parse::<TargetTriple>().unwrap();
            let layout = compute_repr_c_layout("Packet", &structs, &repr, &target).unwrap();
            assert_eq!(layout.size, 24);
            assert_eq!(layout.align, 8);
            assert_eq!(
                layout
                    .fields
                    .iter()
                    .map(|field| field.offset)
                    .collect::<Vec<_>>(),
                [0, 8, 16]
            );
        }
    }
}
