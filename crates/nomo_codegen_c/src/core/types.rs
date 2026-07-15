use super::*;
use std::collections::BTreeSet;

pub(super) fn emit_type_name_macros(out: &mut String, program: &Program) {
    for (struct_name, struct_args) in collect_struct_instances(program) {
        let struct_type = program
            .structs
            .iter()
            .find(|item| item.name == struct_name)
            .expect("checked programs only use known structs");
        let package = c_package_ident(&struct_type.package);
        let local = c_struct_ident(&struct_name, &struct_args);
        let suffix = c_type_suffix(&struct_args);
        out.push_str("#define ");
        out.push_str(&local);
        out.push_str(" nomo_pkg_");
        out.push_str(&package);
        out.push_str("_struct_");
        out.push_str(&struct_name);
        out.push_str(&suffix);
        out.push('\n');
    }
    for (enum_name, enum_args) in collect_enum_instances(program) {
        let enum_type = program
            .enums
            .iter()
            .find(|item| item.name == enum_name)
            .expect("checked programs only use known enums");
        let package = c_package_ident(&enum_type.package);
        let suffix = c_type_suffix(&enum_args);
        out.push_str("#define ");
        out.push_str(&c_enum_tag_ident(&enum_name, &enum_args));
        out.push_str(" nomo_pkg_");
        out.push_str(&package);
        out.push_str("_enum_");
        out.push_str(&enum_name);
        out.push_str(&suffix);
        out.push_str("_tag\n");
        out.push_str("#define ");
        out.push_str(&c_enum_ident(&enum_name, &enum_args));
        out.push_str(" nomo_pkg_");
        out.push_str(&package);
        out.push_str("_enum_");
        out.push_str(&enum_name);
        out.push_str(&suffix);
        out.push('\n');
        for variant in &enum_type.variants {
            out.push_str("#define ");
            out.push_str(&c_enum_variant_ident(&enum_name, &enum_args, &variant.name));
            out.push_str(" nomo_pkg_");
            out.push_str(&package);
            out.push_str("_enum_");
            out.push_str(&enum_name);
            out.push_str(&suffix);
            out.push('_');
            out.push_str(&variant.name);
            out.push('\n');
        }
    }
    if !program.structs.is_empty() || !program.enums.is_empty() {
        out.push('\n');
    }
}

pub(super) fn emit_type_forward_declarations(
    out: &mut String,
    program: &Program,
    array_element_types: &[ValueType],
) {
    let mut emitted = false;
    for (struct_name, struct_args) in collect_struct_instances(program) {
        out.push_str("typedef struct ");
        out.push_str(&c_struct_ident(&struct_name, &struct_args));
        out.push(' ');
        out.push_str(&c_struct_ident(&struct_name, &struct_args));
        out.push_str(";\n");
        emitted = true;
    }
    for (enum_name, enum_args) in collect_enum_instances(program) {
        out.push_str("typedef struct ");
        out.push_str(&c_enum_ident(&enum_name, &enum_args));
        out.push(' ');
        out.push_str(&c_enum_ident(&enum_name, &enum_args));
        out.push_str(";\n");
        emitted = true;
    }
    for element_type in array_element_types {
        out.push_str("typedef struct ");
        out.push_str(&c_array_ident(element_type));
        out.push(' ');
        out.push_str(&c_array_ident(element_type));
        out.push_str(";\n");
        emitted = true;
    }
    if emitted {
        out.push('\n');
    }
}

pub(super) fn emit_lifecycle_helper_prototypes(
    out: &mut String,
    program: &Program,
    array_element_types: &[ValueType],
) {
    let mut emitted = false;
    for element_type in array_element_types {
        let array_type = ValueType::Array(Box::new(element_type.clone()));
        emit_retain_prototype(out, &array_type);
        emit_release_prototype(out, &array_type);
        emitted = true;
    }
    for (name, args) in collect_struct_instances(program) {
        let value_type = ValueType::Struct(name, args);
        emit_retain_prototype(out, &value_type);
        emit_release_prototype(out, &value_type);
        emitted = true;
    }
    for (name, args) in collect_enum_instances(program) {
        let value_type = ValueType::Enum(name, args);
        emit_retain_prototype(out, &value_type);
        emit_release_prototype(out, &value_type);
        emitted = true;
    }
    if emitted {
        out.push('\n');
    }
}

fn emit_retain_prototype(out: &mut String, value_type: &ValueType) {
    out.push_str("static ");
    out.push_str(&c_type(value_type));
    out.push(' ');
    out.push_str(&c_retain_ident(value_type));
    out.push('(');
    out.push_str(&c_type(value_type));
    out.push_str(" value);\n");
}

fn emit_release_prototype(out: &mut String, value_type: &ValueType) {
    out.push_str("static void ");
    out.push_str(&c_release_ident(value_type));
    out.push('(');
    out.push_str(&c_type(value_type));
    out.push_str(" value);\n");
}
#[derive(Debug, Clone, PartialEq, Eq)]
enum TypeInstance {
    Struct(String, Vec<ValueType>),
    Enum(String, Vec<ValueType>),
}

pub(super) fn emit_struct_and_enum_types(out: &mut String, program: &Program) {
    let mut remaining = collect_struct_instances(program)
        .into_iter()
        .map(|(name, args)| TypeInstance::Struct(name, args))
        .chain(
            collect_enum_instances(program)
                .into_iter()
                .map(|(name, args)| TypeInstance::Enum(name, args)),
        )
        .collect::<Vec<_>>();
    let mut defined = BTreeSet::new();

    while !remaining.is_empty() {
        let mut index = 0;
        let mut emitted_any = false;
        while index < remaining.len() {
            if type_instance_dependencies_satisfied(program, &remaining[index], &defined) {
                let item = remaining.remove(index);
                emit_type_instance(out, program, &item);
                out.push('\n');
                defined.insert(type_instance_key(&item));
                emitted_any = true;
            } else {
                index += 1;
            }
        }
        if !emitted_any {
            for item in remaining.drain(..) {
                emit_type_instance(out, program, &item);
                out.push('\n');
            }
        }
    }
}

fn emit_type_instance(out: &mut String, program: &Program, item: &TypeInstance) {
    match item {
        TypeInstance::Struct(name, args) => {
            let struct_type = program
                .structs
                .iter()
                .find(|item| item.name == *name)
                .expect("checked programs only use known structs");
            emit_struct_type(out, struct_type, args);
        }
        TypeInstance::Enum(name, args) => {
            let enum_type = program
                .enums
                .iter()
                .find(|item| item.name == *name)
                .expect("checked programs only use known enums");
            emit_enum_type(out, enum_type, args);
        }
    }
}

fn type_instance_dependencies_satisfied(
    program: &Program,
    item: &TypeInstance,
    defined: &BTreeSet<String>,
) -> bool {
    let deps = match item {
        TypeInstance::Struct(name, args) => {
            let struct_type = program
                .structs
                .iter()
                .find(|item| item.name == *name)
                .expect("checked programs only use known structs");
            let mut deps = BTreeSet::new();
            for field in &struct_type.fields {
                let field_type = subst_type(&field.value_type, &struct_type.type_params, args);
                collect_complete_type_dependencies(&field_type, &mut deps);
            }
            deps
        }
        TypeInstance::Enum(name, args) => {
            let enum_type = program
                .enums
                .iter()
                .find(|item| item.name == *name)
                .expect("checked programs only use known enums");
            let mut deps = BTreeSet::new();
            for variant in &enum_type.variants {
                if let Some(payload) = &variant.payload {
                    let payload_type = subst_type(payload, &enum_type.type_params, args);
                    collect_complete_type_dependencies(&payload_type, &mut deps);
                }
            }
            deps
        }
    };
    let self_key = type_instance_key(item);
    deps.iter()
        .filter(|dep| dep.as_str() != self_key)
        .all(|dep| defined.contains(dep))
}

fn collect_complete_type_dependencies(value_type: &ValueType, out: &mut BTreeSet<String>) {
    match value_type {
        ValueType::Struct(name, args) => {
            out.insert(type_instance_key(&TypeInstance::Struct(
                name.clone(),
                args.clone(),
            )));
            for arg in args {
                collect_complete_type_dependencies(arg, out);
            }
        }
        ValueType::Enum(name, args) => {
            out.insert(type_instance_key(&TypeInstance::Enum(
                name.clone(),
                args.clone(),
            )));
            for arg in args {
                collect_complete_type_dependencies(arg, out);
            }
        }
        ValueType::Array(_) => {}
        ValueType::String
        | ValueType::CString
        | ValueType::Opaque
        | ValueType::OpaqueHandle(_)
        | ValueType::OwnedHandle(_)
        | ValueType::BorrowedHandle(_)
        | ValueType::Nullable(_)
        | ValueType::ExternCallback { .. }
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

fn type_instance_key(item: &TypeInstance) -> String {
    match item {
        TypeInstance::Struct(name, args) => c_struct_ident(name, args),
        TypeInstance::Enum(name, args) => c_enum_ident(name, args),
    }
}

fn emit_struct_type(out: &mut String, struct_type: &StructType, struct_args: &[ValueType]) {
    if struct_type.name == "File" && struct_args.is_empty() {
        out.push_str("struct ");
        out.push_str(&c_struct_ident(&struct_type.name, struct_args));
        out.push_str(" {\n");
        out.push_str("    FILE *");
        out.push_str(&c_member_ident("handle"));
        out.push_str(";\n");
        out.push_str("};\n");
        return;
    }
    if struct_type.name == "TcpStream" && struct_args.is_empty() {
        out.push_str("struct ");
        out.push_str(&c_struct_ident(&struct_type.name, struct_args));
        out.push_str(" {\n");
        out.push_str("    nomo_socket ");
        out.push_str(&c_member_ident("handle"));
        out.push_str(";\n");
        out.push_str("};\n");
        return;
    }
    if struct_type.name == "TcpListener" && struct_args.is_empty() {
        out.push_str("struct ");
        out.push_str(&c_struct_ident(&struct_type.name, struct_args));
        out.push_str(" {\n");
        out.push_str("    nomo_socket ");
        out.push_str(&c_member_ident("handle"));
        out.push_str(";\n");
        out.push_str("};\n");
        return;
    }
    if struct_type.name == "UdpSocket" && struct_args.is_empty() {
        out.push_str("struct ");
        out.push_str(&c_struct_ident(&struct_type.name, struct_args));
        out.push_str(" {\n");
        out.push_str("    nomo_socket ");
        out.push_str(&c_member_ident("handle"));
        out.push_str(";\n");
        out.push_str("};\n");
        return;
    }
    if struct_type.name == "HttpServer" && struct_args.is_empty() {
        out.push_str("struct ");
        out.push_str(&c_struct_ident(&struct_type.name, struct_args));
        out.push_str(" {\n");
        out.push_str("    nomo_socket ");
        out.push_str(&c_member_ident("handle"));
        out.push_str(";\n");
        out.push_str("};\n");
        return;
    }
    if struct_type.name == "HttpExchange" && struct_args.is_empty() {
        out.push_str("struct ");
        out.push_str(&c_struct_ident(&struct_type.name, struct_args));
        out.push_str(" {\n");
        out.push_str("    nomo_socket ");
        out.push_str(&c_member_ident("handle"));
        out.push_str(";\n");
        for field in &struct_type.fields {
            out.push_str("    ");
            out.push_str(&c_type(&field.value_type));
            out.push(' ');
            out.push_str(&c_member_ident(&field.name));
            out.push_str(";\n");
        }
        out.push_str("};\n");
        return;
    }
    out.push_str("struct ");
    out.push_str(&c_struct_ident(&struct_type.name, struct_args));
    out.push_str(" {\n");
    for field in &struct_type.fields {
        out.push_str("    ");
        out.push_str(&c_type(&subst_type(
            &field.value_type,
            &struct_type.type_params,
            struct_args,
        )));
        out.push(' ');
        out.push_str(&c_member_ident(&field.name));
        out.push_str(";\n");
    }
    out.push_str("};\n");
}

fn emit_enum_type(out: &mut String, enum_type: &EnumType, enum_args: &[ValueType]) {
    out.push_str("typedef enum ");
    out.push_str(&c_enum_tag_ident(&enum_type.name, enum_args));
    out.push_str(" {\n");
    for (index, variant) in enum_type.variants.iter().enumerate() {
        out.push_str("    ");
        out.push_str(&c_enum_variant_ident(
            &enum_type.name,
            enum_args,
            &variant.name,
        ));
        if index + 1 != enum_type.variants.len() {
            out.push(',');
        }
        out.push('\n');
    }
    out.push_str("} ");
    out.push_str(&c_enum_tag_ident(&enum_type.name, enum_args));
    out.push_str(";\n\n");
    out.push_str("struct ");
    out.push_str(&c_enum_ident(&enum_type.name, enum_args));
    out.push_str(" {\n");
    out.push_str("    ");
    out.push_str(&c_enum_tag_ident(&enum_type.name, enum_args));
    out.push_str(" tag;\n");
    if enum_type
        .variants
        .iter()
        .any(|variant| variant.payload.is_some())
    {
        out.push_str("    union {\n");
        for variant in enum_type
            .variants
            .iter()
            .filter(|variant| variant.payload.is_some())
        {
            out.push_str("        ");
            out.push_str(&c_payload_type(&subst_type(
                variant.payload.as_ref().unwrap(),
                &enum_type.type_params,
                enum_args,
            )));
            out.push(' ');
            out.push_str(&c_payload_ident(&variant.name));
            out.push_str(";\n");
        }
        out.push_str("    } payload;\n");
    }
    out.push_str("};\n");
}

pub(super) fn emit_nominal_lifecycle_helpers(out: &mut String, program: &Program) {
    for (name, args) in collect_struct_instances(program) {
        let struct_type = program
            .structs
            .iter()
            .find(|item| item.name == name)
            .expect("checked programs only use known structs");
        emit_struct_lifecycle_helpers(out, struct_type, &args);
        out.push('\n');
    }
    for (name, args) in collect_enum_instances(program) {
        let enum_type = program
            .enums
            .iter()
            .find(|item| item.name == name)
            .expect("checked programs only use known enums");
        emit_enum_lifecycle_helpers(out, enum_type, &args);
        out.push('\n');
    }
}

fn emit_struct_lifecycle_helpers(
    out: &mut String,
    struct_type: &StructType,
    struct_args: &[ValueType],
) {
    let value_type = ValueType::Struct(struct_type.name.clone(), struct_args.to_vec());
    let c_type_name = c_type(&value_type);
    out.push_str("static ");
    out.push_str(&c_type_name);
    out.push(' ');
    out.push_str(&c_retain_ident(&value_type));
    out.push('(');
    out.push_str(&c_type_name);
    out.push_str(" value) {\n");
    for field in &struct_type.fields {
        let field_type = subst_type(&field.value_type, &struct_type.type_params, struct_args);
        if value_type_needs_release(&field_type) {
            let field = format!("value.{}", c_member_ident(&field.name));
            emit_value_retain_in_place(out, &field_type, &field, 1);
        }
    }
    out.push_str("    return value;\n");
    out.push_str("}\n\n");

    out.push_str("static void ");
    out.push_str(&c_release_ident(&value_type));
    out.push('(');
    out.push_str(&c_type_name);
    out.push_str(" value) {\n");
    for field in &struct_type.fields {
        let field_type = subst_type(&field.value_type, &struct_type.type_params, struct_args);
        if value_type_needs_release(&field_type) {
            let field = format!("value.{}", c_member_ident(&field.name));
            emit_value_release_in_place(out, &field_type, &field, 1);
        }
    }
    out.push_str("}\n");
}

fn emit_enum_lifecycle_helpers(out: &mut String, enum_type: &EnumType, enum_args: &[ValueType]) {
    let value_type = ValueType::Enum(enum_type.name.clone(), enum_args.to_vec());
    let c_type_name = c_type(&value_type);
    out.push_str("static ");
    out.push_str(&c_type_name);
    out.push(' ');
    out.push_str(&c_retain_ident(&value_type));
    out.push('(');
    out.push_str(&c_type_name);
    out.push_str(" value) {\n");
    for variant in &enum_type.variants {
        let Some(payload_type) = &variant.payload else {
            continue;
        };
        let payload_type = subst_type(payload_type, &enum_type.type_params, enum_args);
        if value_type_needs_release(&payload_type) {
            write_indent(out, 1);
            out.push_str("if (value.tag == ");
            out.push_str(&c_enum_variant_ident(
                &enum_type.name,
                enum_args,
                &variant.name,
            ));
            out.push_str(") {\n");
            let payload = format!("value.payload.{}", c_payload_ident(&variant.name));
            emit_value_retain_in_place(out, &payload_type, &payload, 2);
            out.push_str("    }\n");
        }
    }
    out.push_str("    return value;\n");
    out.push_str("}\n\n");

    out.push_str("static void ");
    out.push_str(&c_release_ident(&value_type));
    out.push('(');
    out.push_str(&c_type_name);
    out.push_str(" value) {\n");
    for variant in &enum_type.variants {
        let Some(payload_type) = &variant.payload else {
            continue;
        };
        let payload_type = subst_type(payload_type, &enum_type.type_params, enum_args);
        if value_type_needs_release(&payload_type) {
            write_indent(out, 1);
            out.push_str("if (value.tag == ");
            out.push_str(&c_enum_variant_ident(
                &enum_type.name,
                enum_args,
                &variant.name,
            ));
            out.push_str(") {\n");
            let payload = format!("value.payload.{}", c_payload_ident(&variant.name));
            emit_value_release_in_place(out, &payload_type, &payload, 2);
            out.push_str("    }\n");
        }
    }
    out.push_str("}\n");
}

pub(super) fn emit_array_type(out: &mut String, element_type: &ValueType) {
    let array = c_array_ident(element_type);
    out.push_str("struct ");
    out.push_str(&array);
    out.push_str(" {\n");
    out.push_str("    size_t len;\n");
    out.push_str("    size_t cap;\n");
    out.push_str("    ");
    out.push_str(&c_type(element_type));
    out.push_str(" *data;\n");
    out.push_str("    size_t *refcount;\n");
    out.push_str("};\n");
}
