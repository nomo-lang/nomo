use super::*;

pub(super) fn validate_extern_opaque_type_namespace(
    path: &Path,
    opaque_types: &[AstExternOpaqueType],
    structs: &[StructType],
    enums: &[EnumType],
) -> Result<(), Diagnostic> {
    const BUILTIN_TYPES: &[&str] = &[
        "string", "CString", "Opaque", "i64", "i32", "u32", "u64", "f64", "char", "bool", "void",
        "Array", "Nullable", "Owned", "Borrowed",
    ];
    let concrete_types = structs
        .iter()
        .map(|item| item.name.as_str())
        .chain(enums.iter().map(|item| item.name.as_str()))
        .collect::<HashSet<_>>();
    let mut seen = HashSet::new();
    for item in opaque_types {
        if BUILTIN_TYPES.contains(&item.name.as_str())
            || concrete_types.contains(item.name.as_str())
        {
            return Err(Diagnostic::new(
                "E1521",
                format!(
                    "opaque handle type `{}` conflicts with an existing type",
                    item.name
                ),
                path,
                item.span.line,
                item.span.column,
                item.span.length,
                &item.span.text,
            ));
        }
        if !seen.insert(item.name.as_str()) {
            return Err(Diagnostic::new(
                "E1521",
                format!("opaque handle type `{}` is already defined", item.name),
                path,
                item.span.line,
                item.span.column,
                item.span.length,
                &item.span.text,
            ));
        }
    }
    Ok(())
}

pub(super) fn validate_type_namespace(
    path: &Path,
    structs: &[StructType],
    enums: &[EnumType],
) -> Result<(), Diagnostic> {
    let struct_names = structs
        .iter()
        .map(|item| item.name.as_str())
        .collect::<HashSet<_>>();
    for enum_type in enums {
        if struct_names.contains(enum_type.name.as_str()) {
            return Err(Diagnostic::new(
                "E0312",
                format!("type `{}` is already defined", enum_type.name),
                path,
                1,
                1,
                1,
                "",
            ));
        }
    }
    Ok(())
}

pub(super) fn validate_repr_c_structs(
    path: &Path,
    structs: &[StructType],
    repr_c_structs: &HashSet<String>,
) -> Result<(), Diagnostic> {
    let struct_map = structs
        .iter()
        .map(|item| (item.name.as_str(), item))
        .collect::<HashMap<_, _>>();
    for name in repr_c_structs {
        let Some(struct_type) = struct_map.get(name.as_str()) else {
            continue;
        };
        if !struct_type.type_params.is_empty() {
            return Err(repr_c_diagnostic(
                path,
                format!("`#[repr(C)]` struct `{name}` cannot be generic"),
            ));
        }
        if struct_type.fields.is_empty() {
            return Err(repr_c_diagnostic(
                path,
                format!("`#[repr(C)]` struct `{name}` cannot be empty"),
            ));
        }
        for field in &struct_type.fields {
            if !is_repr_c_field_type(&field.value_type, repr_c_structs) {
                return Err(repr_c_diagnostic(
                    path,
                    format!(
                        "field `{}.{}` has non-ABI-safe type `{}`; repr(C) fields must be fixed-width scalars, handles, nullable handles, or another non-generic repr(C) struct",
                        name,
                        field.name,
                        field.value_type.name()
                    ),
                ));
            }
        }
    }
    Ok(())
}

fn is_repr_c_field_type(value_type: &ValueType, repr_c_structs: &HashSet<String>) -> bool {
    matches!(
        value_type,
        ValueType::Int
            | ValueType::I32
            | ValueType::U32
            | ValueType::U64
            | ValueType::Float
            | ValueType::Char
            | ValueType::Bool
            | ValueType::Opaque
            | ValueType::OpaqueHandle(_)
            | ValueType::BorrowedHandle(_)
            | ValueType::Nullable(_)
    ) || matches!(value_type, ValueType::Struct(name, args) if args.is_empty() && repr_c_structs.contains(name))
}

fn repr_c_diagnostic(path: &Path, message: String) -> Diagnostic {
    Diagnostic::new("E1530", message, path, 1, 1, 1, "")
}

pub(super) fn validate_no_recursive_value_types(
    path: &Path,
    structs: &[StructType],
    enums: &[EnumType],
) -> Result<(), Diagnostic> {
    let mut graph = HashMap::<String, Vec<String>>::new();
    let nominal_names = structs
        .iter()
        .map(|item| item.name.as_str())
        .chain(enums.iter().map(|item| item.name.as_str()))
        .collect::<HashSet<_>>();

    for struct_type in structs {
        let mut deps = Vec::new();
        for field in &struct_type.fields {
            collect_value_type_dependencies(&field.value_type, &nominal_names, &mut deps);
        }
        graph.insert(struct_type.name.clone(), deps);
    }
    for enum_type in enums {
        let mut deps = Vec::new();
        for variant in &enum_type.variants {
            if let Some(payload) = &variant.payload {
                collect_value_type_dependencies(payload, &nominal_names, &mut deps);
            }
        }
        graph.insert(enum_type.name.clone(), deps);
    }

    for name in graph.keys() {
        let mut visiting = Vec::new();
        let mut visited = HashSet::new();
        if type_dependency_reaches(name, name, &graph, &mut visiting, &mut visited) {
            return Err(Diagnostic::new(
                "E0410",
                format!("type `{name}` is recursively embedded by value"),
                path,
                1,
                1,
                1,
                "",
            ));
        }
    }
    Ok(())
}

pub(super) fn collect_value_type_dependencies(
    value_type: &ValueType,
    nominal_names: &HashSet<&str>,
    out: &mut Vec<String>,
) {
    match value_type {
        ValueType::Struct(name, args) | ValueType::Enum(name, args) => {
            if nominal_names.contains(name.as_str()) {
                out.push(name.clone());
            }
            for arg in args {
                collect_value_type_dependencies(arg, nominal_names, out);
            }
        }
        ValueType::Nullable(inner) => {
            collect_value_type_dependencies(inner, nominal_names, out);
        }
        ValueType::ExternCallback {
            params,
            return_type,
        } => {
            for param in params {
                collect_value_type_dependencies(param, nominal_names, out);
            }
            collect_value_type_dependencies(return_type, nominal_names, out);
        }
        ValueType::Array(_) => {}
        ValueType::String
        | ValueType::CString
        | ValueType::Opaque
        | ValueType::OpaqueHandle(_)
        | ValueType::OwnedHandle(_)
        | ValueType::BorrowedHandle(_)
        | ValueType::Int
        | ValueType::I32
        | ValueType::U32
        | ValueType::U64
        | ValueType::Float
        | ValueType::Char
        | ValueType::Bool
        | ValueType::TypeParam(_)
        | ValueType::Void
        | ValueType::Never => {}
    }
}

pub(super) fn type_dependency_reaches(
    start: &str,
    current: &str,
    graph: &HashMap<String, Vec<String>>,
    visiting: &mut Vec<String>,
    visited: &mut HashSet<String>,
) -> bool {
    if !visited.insert(current.to_string()) {
        return false;
    }
    visiting.push(current.to_string());
    for dep in graph.get(current).into_iter().flatten() {
        if dep == start {
            return true;
        }
        if !visiting.iter().any(|item| item == dep)
            && type_dependency_reaches(start, dep, graph, visiting, visited)
        {
            return true;
        }
    }
    visiting.pop();
    false
}

pub(super) fn validate_standard_type_conflicts(
    path: &Path,
    needs: StandardTypeNeeds,
    structs: &[AstStructDef],
    enums: &[AstEnumDef],
) -> Result<(), Diagnostic> {
    if needs.io {
        reject_user_std_struct(path, structs, "IoError")?;
    }
    if needs.fs {
        reject_user_std_struct(path, structs, "FsError")?;
        reject_user_std_struct(path, structs, "File")?;
    }
    if needs.net {
        reject_user_std_struct(path, structs, "NetError")?;
        reject_user_std_struct(path, structs, "TcpListener")?;
        reject_user_std_struct(path, structs, "TcpStream")?;
        reject_user_std_struct(path, structs, "UdpDatagram")?;
        reject_user_std_struct(path, structs, "UdpSocket")?;
    }
    if needs.http {
        reject_user_std_struct(path, structs, "HttpExchange")?;
        reject_user_std_struct(path, structs, "HttpError")?;
        reject_user_std_struct(path, structs, "HttpResponse")?;
        reject_user_std_struct(path, structs, "HttpServer")?;
    }
    if needs.num {
        reject_user_std_struct(path, structs, "NumError")?;
    }
    if needs.process {
        reject_user_std_struct(path, structs, "ProcessError")?;
        reject_user_std_struct(path, structs, "ProcessOutput")?;
    }
    if needs.hash {
        reject_user_std_struct(path, structs, "HashState")?;
    }
    if needs.ffi {
        reject_user_std_struct(path, structs, "CString")?;
        reject_user_std_struct(path, structs, "Opaque")?;
        reject_user_std_enum(path, enums, "CString")?;
        reject_user_std_enum(path, enums, "Opaque")?;
    }
    if needs.io || needs.fs || needs.net || needs.http || needs.num || needs.process || needs.result
    {
        reject_user_std_enum(path, enums, "Result")?;
    }
    if needs.env || needs.num || needs.option || needs.array {
        reject_user_std_enum(path, enums, "Option")?;
    }
    Ok(())
}

pub(super) fn reject_user_std_struct(
    path: &Path,
    structs: &[AstStructDef],
    name: &str,
) -> Result<(), Diagnostic> {
    if structs.iter().any(|item| item.name == name) {
        return Err(Diagnostic::new(
            "E0312",
            format!("type `{name}` conflicts with a required standard library type"),
            path,
            1,
            1,
            1,
            "",
        ));
    }
    Ok(())
}

pub(super) fn reject_user_std_enum(
    path: &Path,
    enums: &[AstEnumDef],
    name: &str,
) -> Result<(), Diagnostic> {
    if enums.iter().any(|item| item.name == name) {
        return Err(Diagnostic::new(
            "E0312",
            format!("type `{name}` conflicts with a required standard library type"),
            path,
            1,
            1,
            1,
            "",
        ));
    }
    Ok(())
}
