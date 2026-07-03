use crate::compiler::{
    BinaryOp, DeferredCall, EnumType, Function, LoopKind, MatchStatementArm, Program, Statement,
    StructType, UnaryOp, ValueExpr, ValueType,
};
use std::collections::BTreeSet;

const BUILTIN_PRINTLN_EXPR: &str = "__nomo_builtin_println";
const BUILTIN_EPRINTLN_EXPR: &str = "__nomo_builtin_eprintln";

#[derive(Debug, Clone, PartialEq, Eq)]
struct ResultMapErrInstance {
    ok_type: ValueType,
    source_err_type: ValueType,
    target_err_type: ValueType,
    converter: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LocalArray {
    name: String,
    value_type: ValueType,
    c_value: Option<String>,
}

pub fn emit_c(program: &Program) -> String {
    let mut out = String::new();
    out.push_str(
        "#include <errno.h>\n#include <stdint.h>\n#include <stdio.h>\n#include <stdlib.h>\n#include <string.h>\n\n",
    );
    out.push_str("static void nomo_panic(const char *message) {\n");
    out.push_str("    fputs(\"panic: \", stderr);\n");
    out.push_str("    fputs(message, stderr);\n");
    out.push_str("    fputc('\\n', stderr);\n");
    out.push_str("    exit(1);\n");
    out.push_str("}\n\n");
    emit_string_runtime(&mut out);
    out.push('\n');

    for const_def in &program.consts {
        out.push_str("#define ");
        out.push_str(&c_var_ident(&const_def.name));
        out.push(' ');
        emit_expr(&mut out, &const_def.initializer);
        out.push('\n');
    }
    if !program.consts.is_empty() {
        out.push('\n');
    }

    emit_function_name_macros(&mut out, program);
    emit_type_name_macros(&mut out, program);

    let array_element_types = collect_array_element_types(program);
    emit_type_forward_declarations(&mut out, program, &array_element_types);
    emit_lifecycle_helper_prototypes(&mut out, program, &array_element_types);

    for element_type in &array_element_types {
        emit_array_type(&mut out, element_type);
        out.push('\n');
    }
    emit_struct_and_enum_types(&mut out, program);
    emit_nominal_lifecycle_helpers(&mut out, program);
    for element_type in &array_element_types {
        emit_array_helpers(&mut out, element_type);
        out.push('\n');
    }
    if uses_env_args(program) {
        out.push_str("static int nomo_argc = 0;\n");
        out.push_str("static char **nomo_argv = NULL;\n\n");
        emit_env_args_helper(&mut out);
        out.push('\n');
    }
    if uses_fs_read_to_string(program) {
        emit_fs_read_to_string_helper(&mut out);
        out.push('\n');
    }
    if uses_fs_write_string(program) {
        emit_fs_write_string_helper(&mut out);
        out.push('\n');
    }
    if uses_fs_open(program) {
        emit_fs_open_helper(&mut out);
        out.push('\n');
        emit_file_close_helper(&mut out);
        out.push('\n');
    }
    if uses_env_get(program) {
        emit_env_get_helper(&mut out);
        out.push('\n');
    }

    let main = program
        .functions
        .iter()
        .find(|function| function.name == "main")
        .expect("checked programs always contain main");
    let main_returns_result = result_void_error(&main.return_type).is_some();

    for function in program
        .functions
        .iter()
        .filter(|function| function.name != "main" || main_returns_result)
    {
        emit_prototype(&mut out, function);
    }
    if program
        .functions
        .iter()
        .any(|function| function.name != "main" || main_returns_result)
    {
        out.push('\n');
    }

    let result_map_err_instances = collect_result_map_err_instances(program);
    for instance in &result_map_err_instances {
        emit_result_map_err_helper(&mut out, instance);
        out.push('\n');
    }

    for function in program
        .functions
        .iter()
        .filter(|function| function.name != "main" || main_returns_result)
    {
        emit_function(&mut out, function);
        out.push('\n');
    }

    if uses_env_args(program) {
        out.push_str("int main(int argc, char **argv) {\n");
    } else {
        out.push_str("int main(void) {\n");
    }
    if uses_env_args(program) {
        out.push_str("    nomo_argc = argc;\n");
        out.push_str("    nomo_argv = argv;\n");
    }
    if let Some(result_args) = result_void_error(&main.return_type) {
        let result_type = c_enum_ident("Result", &result_args);
        out.push_str("    ");
        out.push_str(&result_type);
        out.push_str(" nomo__result = ");
        out.push_str(&c_fn_ident("main"));
        out.push_str("();\n");
        out.push_str("    return nomo__result.tag == ");
        out.push_str(&c_enum_variant_ident("Result", &result_args, "Ok"));
        out.push_str(" ? 0 : 1;\n");
    } else {
        emit_body(&mut out, main);
        out.push_str("    return 0;\n");
    }
    out.push_str("}\n");
    out
}

fn emit_string_runtime(out: &mut String) {
    out.push_str("typedef struct nomo_string {\n");
    out.push_str("    const char *data;\n");
    out.push_str("    size_t *refcount;\n");
    out.push_str("} nomo_string;\n\n");
    out.push_str("static nomo_string nomo_string_literal(const char *data) {\n");
    out.push_str("    return (nomo_string){.data = data, .refcount = NULL};\n");
    out.push_str("}\n\n");
    out.push_str("static nomo_string nomo_string_owned(char *data) {\n");
    out.push_str("    size_t *refcount = (size_t *)malloc(sizeof(size_t));\n");
    out.push_str("    if (refcount == NULL) {\n");
    out.push_str("        free(data);\n");
    out.push_str("        nomo_panic(\"out of memory\");\n");
    out.push_str("    }\n");
    out.push_str("    *refcount = 1;\n");
    out.push_str("    return (nomo_string){.data = data, .refcount = refcount};\n");
    out.push_str("}\n\n");
    out.push_str("static nomo_string nomo_string_from_cstr(const char *value) {\n");
    out.push_str("    size_t len = strlen(value);\n");
    out.push_str("    char *data = (char *)malloc(len + 1);\n");
    out.push_str("    if (data == NULL) { nomo_panic(\"out of memory\"); }\n");
    out.push_str("    memcpy(data, value, len + 1);\n");
    out.push_str("    return nomo_string_owned(data);\n");
    out.push_str("}\n\n");
    out.push_str("static nomo_string nomo_string_retain(nomo_string value) {\n");
    out.push_str("    if (value.refcount != NULL) { *value.refcount += 1; }\n");
    out.push_str("    return value;\n");
    out.push_str("}\n\n");
    out.push_str("static void nomo_string_release(nomo_string value) {\n");
    out.push_str("    if (value.refcount == NULL) { return; }\n");
    out.push_str("    *value.refcount -= 1;\n");
    out.push_str("    if (*value.refcount != 0) { return; }\n");
    out.push_str("    free((char *)value.data);\n");
    out.push_str("    free(value.refcount);\n");
    out.push_str("}\n\n");
    out.push_str("static nomo_string nomo_string_concat(nomo_string left, nomo_string right) {\n");
    out.push_str("    size_t left_len = strlen(left.data);\n");
    out.push_str("    size_t right_len = strlen(right.data);\n");
    out.push_str("    char *out = (char *)malloc(left_len + right_len + 1);\n");
    out.push_str("    if (out == NULL) {\n");
    out.push_str("        nomo_panic(\"out of memory\");\n");
    out.push_str("    }\n");
    out.push_str("    memcpy(out, left.data, left_len);\n");
    out.push_str("    memcpy(out + left_len, right.data, right_len + 1);\n");
    out.push_str("    return nomo_string_owned(out);\n");
    out.push_str("}\n");
    out.push_str("\nstatic int nomo_string_equal(nomo_string left, nomo_string right) {\n");
    out.push_str("    return strcmp(left.data, right.data) == 0;\n");
    out.push_str("}\n");
}

fn emit_function_name_macros(out: &mut String, program: &Program) {
    for function in &program.functions {
        let package = c_package_ident(&function.package);
        out.push_str("#define ");
        out.push_str(&c_fn_ident(&function.name));
        out.push(' ');
        out.push_str("nomo_pkg_");
        out.push_str(&package);
        out.push_str("_fn_");
        out.push_str(&function.name);
        out.push('\n');
    }
    if !program.functions.is_empty() {
        out.push('\n');
    }
}

fn emit_type_name_macros(out: &mut String, program: &Program) {
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

fn emit_type_forward_declarations(
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

fn emit_lifecycle_helper_prototypes(
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

fn emit_prototype(out: &mut String, function: &Function) {
    emit_signature(out, function);
    out.push_str(";\n");
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum TypeInstance {
    Struct(String, Vec<ValueType>),
    Enum(String, Vec<ValueType>),
}

fn emit_struct_and_enum_types(out: &mut String, program: &Program) {
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

fn emit_nominal_lifecycle_helpers(out: &mut String, program: &Program) {
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

fn emit_fs_read_to_string_helper(out: &mut String) {
    let fs_error = c_struct_ident("FsError", &[]);
    let result = c_enum_ident(
        "Result",
        &[
            ValueType::String,
            ValueType::Struct("FsError".to_string(), Vec::new()),
        ],
    );
    let ok = c_enum_variant_ident(
        "Result",
        &[
            ValueType::String,
            ValueType::Struct("FsError".to_string(), Vec::new()),
        ],
        "Ok",
    );
    let err = c_enum_variant_ident(
        "Result",
        &[
            ValueType::String,
            ValueType::Struct("FsError".to_string(), Vec::new()),
        ],
        "Err",
    );
    out.push_str("static ");
    out.push_str(&result);
    out.push_str(" nomo_fs_read_to_string(nomo_string path) {\n");
    out.push_str("    FILE *file = fopen(path.data, \"rb\");\n");
    out.push_str("    if (file == NULL) {\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&fs_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(strerror(errno))}};\n");
    out.push_str("    }\n");
    out.push_str("    if (fseek(file, 0, SEEK_END) != 0) {\n");
    out.push_str("        const char *message = strerror(errno);\n");
    out.push_str("        fclose(file);\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&fs_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(message)}};\n");
    out.push_str("    }\n");
    out.push_str("    long size = ftell(file);\n");
    out.push_str("    if (size < 0 || fseek(file, 0, SEEK_SET) != 0) {\n");
    out.push_str("        const char *message = strerror(errno);\n");
    out.push_str("        fclose(file);\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&fs_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(message)}};\n");
    out.push_str("    }\n");
    out.push_str("    char *buffer = (char *)malloc((size_t)size + 1);\n");
    out.push_str("    if (buffer == NULL) {\n");
    out.push_str("        fclose(file);\n");
    out.push_str("        nomo_panic(\"out of memory\");\n");
    out.push_str("    }\n");
    out.push_str("    size_t read = fread(buffer, 1, (size_t)size, file);\n");
    out.push_str("    if (read != (size_t)size) {\n");
    out.push_str(
        "        const char *message = ferror(file) ? strerror(errno) : \"short read\";\n",
    );
    out.push_str("        free(buffer);\n");
    out.push_str("        fclose(file);\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&fs_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(message)}};\n");
    out.push_str("    }\n");
    out.push_str("    buffer[size] = '\\0';\n");
    out.push_str("    fclose(file);\n");
    out.push_str("    return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&ok);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Ok"));
    out.push_str(" = nomo_string_owned(buffer)};\n");
    out.push_str("}\n");
}

fn emit_fs_write_string_helper(out: &mut String) {
    let fs_error = c_struct_ident("FsError", &[]);
    let result = c_enum_ident(
        "Result",
        &[
            ValueType::Void,
            ValueType::Struct("FsError".to_string(), Vec::new()),
        ],
    );
    let ok = c_enum_variant_ident(
        "Result",
        &[
            ValueType::Void,
            ValueType::Struct("FsError".to_string(), Vec::new()),
        ],
        "Ok",
    );
    let err = c_enum_variant_ident(
        "Result",
        &[
            ValueType::Void,
            ValueType::Struct("FsError".to_string(), Vec::new()),
        ],
        "Err",
    );
    out.push_str("static ");
    out.push_str(&result);
    out.push_str(" nomo_fs_write_string(nomo_string path, nomo_string content) {\n");
    out.push_str("    FILE *file = fopen(path.data, \"wb\");\n");
    out.push_str("    if (file == NULL) {\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&fs_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(strerror(errno))}};\n");
    out.push_str("    }\n");
    out.push_str("    size_t len = strlen(content.data);\n");
    out.push_str("    if (fwrite(content.data, 1, len, file) != len) {\n");
    out.push_str("        const char *message = strerror(errno);\n");
    out.push_str("        fclose(file);\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&fs_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(message)}};\n");
    out.push_str("    }\n");
    out.push_str("    if (fclose(file) != 0) {\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&fs_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(strerror(errno))}};\n");
    out.push_str("    }\n");
    out.push_str("    return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&ok);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Ok"));
    out.push_str(" = 0};\n");
    out.push_str("}\n");
}

fn emit_fs_open_helper(out: &mut String) {
    let file_type = c_struct_ident("File", &[]);
    let fs_error = c_struct_ident("FsError", &[]);
    let result = c_enum_ident(
        "Result",
        &[
            ValueType::Struct("File".to_string(), Vec::new()),
            ValueType::Struct("FsError".to_string(), Vec::new()),
        ],
    );
    let ok = c_enum_variant_ident(
        "Result",
        &[
            ValueType::Struct("File".to_string(), Vec::new()),
            ValueType::Struct("FsError".to_string(), Vec::new()),
        ],
        "Ok",
    );
    let err = c_enum_variant_ident(
        "Result",
        &[
            ValueType::Struct("File".to_string(), Vec::new()),
            ValueType::Struct("FsError".to_string(), Vec::new()),
        ],
        "Err",
    );
    out.push_str("static ");
    out.push_str(&result);
    out.push_str(" nomo_fs_open(nomo_string path) {\n");
    out.push_str("    FILE *file = fopen(path.data, \"rb\");\n");
    out.push_str("    if (file == NULL) {\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = (");
    out.push_str(&fs_error);
    out.push_str("){.");
    out.push_str(&c_member_ident("message"));
    out.push_str(" = nomo_string_from_cstr(strerror(errno))}};\n");
    out.push_str("    }\n");
    out.push_str("    return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&ok);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Ok"));
    out.push_str(" = (");
    out.push_str(&file_type);
    out.push_str("){.");
    out.push_str(&c_member_ident("handle"));
    out.push_str(" = file}};\n");
    out.push_str("}\n");
}

fn emit_file_close_helper(out: &mut String) {
    let file_type = c_struct_ident("File", &[]);
    out.push_str("static void nomo_file_close(");
    out.push_str(&file_type);
    out.push_str(" file) {\n");
    out.push_str("    if (file.");
    out.push_str(&c_member_ident("handle"));
    out.push_str(" != NULL) {\n");
    out.push_str("        fclose(file.");
    out.push_str(&c_member_ident("handle"));
    out.push_str(");\n");
    out.push_str("    }\n");
    out.push_str("}\n");
}

fn emit_env_get_helper(out: &mut String) {
    let result = c_enum_ident("Option", &[ValueType::String]);
    let some = c_enum_variant_ident("Option", &[ValueType::String], "Some");
    let none = c_enum_variant_ident("Option", &[ValueType::String], "None");
    out.push_str("static ");
    out.push_str(&result);
    out.push_str(" nomo_env_get(nomo_string name) {\n");
    out.push_str("    const char *value = getenv(name.data);\n");
    out.push_str("    if (value == NULL) {\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&none);
    out.push_str("};\n");
    out.push_str("    }\n");
    out.push_str("    return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&some);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Some"));
    out.push_str(" = nomo_string_from_cstr(value)};\n");
    out.push_str("}\n");
}

fn emit_env_args_helper(out: &mut String) {
    out.push_str("static nomo_array_string nomo_env_args(int argc, char **argv) {\n");
    out.push_str("    nomo_array_string args = nomo_array_string_new();\n");
    out.push_str("    for (int i = 0; i < argc; i += 1) {\n");
    out.push_str("        nomo_string arg = nomo_string_from_cstr(argv[i]);\n");
    out.push_str("        args = nomo_array_string_push(args, arg);\n");
    out.push_str("        nomo_string_release(arg);\n");
    out.push_str("    }\n");
    out.push_str("    return args;\n");
    out.push_str("}\n");
}

fn emit_array_type(out: &mut String, element_type: &ValueType) {
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

fn emit_array_helpers(out: &mut String, element_type: &ValueType) {
    let array = c_array_ident(element_type);
    let option = c_enum_ident("Option", &[element_type.clone()]);
    let some = c_enum_variant_ident("Option", &[element_type.clone()], "Some");
    let none = c_enum_variant_ident("Option", &[element_type.clone()], "None");
    out.push_str("static ");
    out.push_str(&array);
    out.push(' ');
    out.push_str(&array);
    out.push_str("_new(void) {\n");
    out.push_str("    return (");
    out.push_str(&array);
    out.push_str("){.len = 0, .cap = 0, .data = NULL, .refcount = NULL};\n");
    out.push_str("}\n\n");
    out.push_str("static ");
    out.push_str(&array);
    out.push(' ');
    out.push_str(&array);
    out.push_str("_retain(");
    out.push_str(&array);
    out.push_str(" array) {\n");
    out.push_str("    if (array.refcount != NULL) { *array.refcount += 1; }\n");
    out.push_str("    return array;\n");
    out.push_str("}\n\n");
    out.push_str("static ");
    out.push_str("void ");
    out.push_str(&array);
    out.push_str("_release(");
    out.push_str(&array);
    out.push_str(" array) {\n");
    out.push_str("    if (array.refcount == NULL) { return; }\n");
    out.push_str("    *array.refcount -= 1;\n");
    out.push_str("    if (*array.refcount != 0) { return; }\n");
    emit_array_element_release_loop(out, element_type);
    out.push_str("    free(array.data);\n");
    out.push_str("    free(array.refcount);\n");
    out.push_str("}\n\n");
    out.push_str("static ");
    out.push_str(&array);
    out.push(' ');
    out.push_str(&array);
    out.push_str("_make_unique(");
    out.push_str(&array);
    out.push_str(" array, size_t needed) {\n");
    out.push_str("    size_t cap = array.cap;\n");
    out.push_str("    if (cap < needed) { cap = cap == 0 ? 4 : cap; }\n");
    out.push_str("    while (cap < needed) { cap *= 2; }\n");
    out.push_str("    if (cap == 0) { return array; }\n");
    out.push_str("    if (array.refcount != NULL && *array.refcount == 1 && array.cap >= needed) { return array; }\n");
    out.push_str("    ");
    out.push_str(&c_type(element_type));
    out.push_str(" *data = (");
    out.push_str(&c_type(element_type));
    out.push_str(" *)malloc(cap * sizeof(");
    out.push_str(&c_type(element_type));
    out.push_str("));\n");
    out.push_str("    if (data == NULL) { nomo_panic(\"out of memory\"); }\n");
    out.push_str("    for (size_t i = 0; i < array.len; i += 1) { data[i] = ");
    emit_array_element_retain_expr(out, element_type, "array.data[i]");
    out.push_str("; }\n");
    out.push_str("    size_t *refcount = (size_t *)malloc(sizeof(size_t));\n");
    out.push_str("    if (refcount == NULL) { nomo_panic(\"out of memory\"); }\n");
    out.push_str("    *refcount = 1;\n");
    out.push_str("    ");
    out.push_str(&array);
    out.push_str("_release(array);\n");
    out.push_str("    array.data = data;\n");
    out.push_str("    array.cap = cap;\n");
    out.push_str("    array.refcount = refcount;\n");
    out.push_str("    return array;\n");
    out.push_str("}\n\n");
    out.push_str("static ");
    out.push_str(&array);
    out.push(' ');
    out.push_str(&array);
    out.push_str("_push(");
    out.push_str(&array);
    out.push_str(" array, ");
    out.push_str(&c_type(element_type));
    out.push_str(" value) {\n");
    out.push_str("    array = ");
    out.push_str(&array);
    out.push_str("_make_unique(array, array.len + 1);\n");
    out.push_str("    array.data[array.len] = ");
    emit_array_element_retain_expr(out, element_type, "value");
    out.push_str(";\n");
    out.push_str("    array.len += 1;\n");
    out.push_str("    return array;\n");
    out.push_str("}\n\n");
    out.push_str("static ");
    out.push_str(&option);
    out.push(' ');
    out.push_str(&array);
    out.push_str("_get(");
    out.push_str(&array);
    out.push_str(" array, uint64_t index) {\n");
    out.push_str("    if (index >= array.len) {\n");
    out.push_str("        return (");
    out.push_str(&option);
    out.push_str("){.tag = ");
    out.push_str(&none);
    out.push_str("};\n");
    out.push_str("    }\n");
    out.push_str("    return (");
    out.push_str(&option);
    out.push_str("){.tag = ");
    out.push_str(&some);
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Some"));
    out.push_str(" = ");
    emit_array_element_retain_expr(out, element_type, "array.data[index]");
    out.push_str("};\n");
    out.push_str("}\n\n");
    out.push_str("static ");
    out.push_str(&array);
    out.push(' ');
    out.push_str(&array);
    out.push_str("_set(");
    out.push_str(&array);
    out.push_str(" array, uint64_t index, ");
    out.push_str(&c_type(element_type));
    out.push_str(" value) {\n");
    out.push_str(
        "    if (index >= array.len) { nomo_panic(\"Array.set index out of bounds\"); }\n",
    );
    out.push_str("    array = ");
    out.push_str(&array);
    out.push_str("_make_unique(array, array.len);\n");
    emit_array_element_release_stmt(out, element_type, "array.data[index]");
    out.push_str("    array.data[index] = ");
    emit_array_element_retain_expr(out, element_type, "value");
    out.push_str(";\n");
    out.push_str("    return array;\n");
    out.push_str("}\n");
}

fn emit_array_element_release_loop(out: &mut String, element_type: &ValueType) {
    if value_type_needs_release(element_type) {
        out.push_str("    for (size_t i = 0; i < array.len; i += 1) { ");
        emit_array_element_release_expr(out, element_type, "array.data[i]");
        out.push_str("; }\n");
    }
}

fn emit_array_element_release_stmt(out: &mut String, element_type: &ValueType, value: &str) {
    if value_type_needs_release(element_type) {
        out.push_str("    ");
        emit_array_element_release_expr(out, element_type, value);
        out.push_str(";\n");
    }
}

fn emit_array_element_release_expr(out: &mut String, element_type: &ValueType, value: &str) {
    if value_type_needs_release(element_type) {
        out.push_str(&c_release_ident(element_type));
        out.push('(');
        out.push_str(value);
        out.push(')');
    }
}

fn emit_array_element_retain_expr(out: &mut String, element_type: &ValueType, value: &str) {
    if value_type_needs_release(element_type) {
        out.push_str(&c_retain_ident(element_type));
        out.push('(');
        out.push_str(value);
        out.push(')');
    } else {
        out.push_str(value);
    }
}

fn emit_function(out: &mut String, function: &Function) {
    emit_signature(out, function);
    out.push_str(" {\n");
    emit_mut_param_macros(out, function);
    emit_body(out, function);
    if function.return_type == ValueType::Void {
        out.push_str("    return;\n");
    }
    emit_mut_param_undefs(out, function);
    out.push_str("}\n");
}

fn emit_signature(out: &mut String, function: &Function) {
    out.push_str(&c_type(&function.return_type));
    out.push(' ');
    out.push_str(&c_fn_ident(&function.name));
    out.push('(');
    if function.params.is_empty() {
        out.push_str("void");
    } else {
        for (index, param) in function.params.iter().enumerate() {
            if index > 0 {
                out.push_str(", ");
            }
            out.push_str(&c_type(&param.value_type));
            if param.mutable {
                out.push_str(" *");
            }
            out.push(' ');
            out.push_str(&c_var_ident(&param.name));
        }
    }
    out.push(')');
}

fn emit_mut_param_macros(out: &mut String, function: &Function) {
    for param in &function.params {
        if param.mutable {
            let name = c_var_ident(&param.name);
            out.push_str("#define ");
            out.push_str(&name);
            out.push_str(" (*");
            out.push_str(&name);
            out.push_str(")\n");
        }
    }
}

fn emit_mut_param_undefs(out: &mut String, function: &Function) {
    for param in &function.params {
        if param.mutable {
            out.push_str("#undef ");
            out.push_str(&c_var_ident(&param.name));
            out.push('\n');
        }
    }
}

fn emit_body(out: &mut String, function: &Function) {
    let mut deferred: Vec<DeferredCall> = Vec::new();
    let mut active_arrays = array_params(function);
    for local in &active_arrays {
        emit_array_retain_binding(out, &local.name, &local.value_type, 1);
    }
    let mut last_statement_exits = false;
    for statement in &function.body {
        if let Statement::Defer { call } = statement {
            deferred.push(call.clone());
        } else {
            emit_stmt(
                out,
                statement,
                1,
                &deferred,
                &function.return_type,
                &active_arrays,
                0,
                0,
                0,
                0,
            );
            if let Some(local) = local_array_from_statement(statement) {
                active_arrays.push(local);
            }
            last_statement_exits = statement_exits_function(statement);
        }
    }
    if !last_statement_exits {
        emit_deferred(out, 1, &deferred);
        emit_array_releases(out, 1, &active_arrays);
    }
}

fn write_indent(out: &mut String, indent: usize) {
    for _ in 0..indent {
        out.push_str("    ");
    }
}

fn emit_deferred(out: &mut String, indent: usize, deferred: &[DeferredCall]) {
    for call in deferred.iter().rev() {
        emit_deferred_call(out, indent, call);
    }
}

fn emit_deferred_call(out: &mut String, indent: usize, call: &DeferredCall) {
    match call {
        DeferredCall::Expr(expr) => {
            write_indent(out, indent);
            emit_expr(out, expr);
            out.push_str(";\n");
        }
        DeferredCall::Println(arg) => {
            write_indent(out, indent);
            out.push_str("puts(");
            emit_string_data_expr(out, arg);
            out.push_str(");\n");
        }
        DeferredCall::Eprintln(arg) => {
            write_indent(out, indent);
            out.push_str("fputs(");
            emit_string_data_expr(out, arg);
            out.push_str(", stderr);\n");
            write_indent(out, indent);
            out.push_str("fputc('\\n', stderr);\n");
        }
    }
}

fn statement_exits_function(statement: &Statement) -> bool {
    match statement {
        Statement::Return(_) | Statement::TryReturnOk { .. } | Statement::Panic(_) => true,
        Statement::Match { arms, .. } => arms.iter().all(|arm| statements_exit_function(&arm.body)),
        Statement::If {
            body, else_body, ..
        } => statements_exit_function(body) && statements_exit_function(else_body),
        Statement::IfLet {
            body,
            else_body: Some(else_body),
            ..
        } => statements_exit_function(body) && statements_exit_function(else_body),
        _ => false,
    }
}

fn statements_exit_function(statements: &[Statement]) -> bool {
    statements.last().is_some_and(statement_exits_function)
}

fn statement_exits_block(statement: &Statement) -> bool {
    match statement {
        Statement::Return(_)
        | Statement::TryReturnOk { .. }
        | Statement::Panic(_)
        | Statement::Break
        | Statement::Continue => true,
        Statement::Match { arms, .. } => arms.iter().all(|arm| statements_exit_block(&arm.body)),
        Statement::If {
            body, else_body, ..
        } => statements_exit_block(body) && statements_exit_block(else_body),
        Statement::IfLet {
            body,
            else_body: Some(else_body),
            ..
        } => statements_exit_block(body) && statements_exit_block(else_body),
        _ => false,
    }
}

fn statements_exit_block(statements: &[Statement]) -> bool {
    statements.last().is_some_and(statement_exits_block)
}

fn emit_stmt(
    out: &mut String,
    statement: &Statement,
    indent: usize,
    deferred: &[DeferredCall],
    function_return_type: &ValueType,
    active_arrays: &[LocalArray],
    break_deferred_start: usize,
    continue_deferred_start: usize,
    break_cleanup_start: usize,
    continue_cleanup_start: usize,
) {
    match statement {
        Statement::Let {
            name,
            value_type,
            initializer,
        } => emit_let(out, name, value_type, initializer, indent),
        Statement::LetIf {
            name,
            value_type,
            condition,
            body,
            else_body,
        } => emit_let_if(
            out,
            name,
            value_type,
            condition,
            body,
            else_body,
            indent,
            deferred,
            function_return_type,
            active_arrays,
            break_deferred_start,
            continue_deferred_start,
            break_cleanup_start,
            continue_cleanup_start,
        ),
        Statement::LetMatch {
            name,
            value_type,
            value,
            enum_name,
            enum_args,
            arms,
        } => emit_let_match(
            out,
            name,
            value_type,
            value,
            enum_name,
            enum_args,
            arms,
            indent,
            deferred,
            function_return_type,
            active_arrays,
            break_deferred_start,
            continue_deferred_start,
            break_cleanup_start,
            continue_cleanup_start,
        ),
        Statement::LetElse {
            binding,
            value_type,
            value,
            enum_name,
            enum_args,
            variant,
            else_body,
        } => emit_let_else(
            out,
            binding,
            value_type,
            value,
            enum_name,
            enum_args,
            variant,
            else_body,
            indent,
            deferred,
            function_return_type,
            active_arrays,
            break_deferred_start,
            continue_deferred_start,
            break_cleanup_start,
            continue_cleanup_start,
        ),
        Statement::IfLet {
            binding,
            value_type,
            value,
            enum_name,
            enum_args,
            variant,
            body,
            else_body,
        } => emit_if_let(
            out,
            binding.as_deref(),
            value_type.as_ref(),
            value,
            enum_name,
            enum_args,
            variant,
            body,
            else_body.as_deref(),
            indent,
            deferred,
            function_return_type,
            active_arrays,
            break_deferred_start,
            continue_deferred_start,
            break_cleanup_start,
            continue_cleanup_start,
        ),
        Statement::If {
            condition,
            body,
            else_body,
        } => emit_if_statement(
            out,
            condition,
            body,
            else_body,
            indent,
            deferred,
            function_return_type,
            active_arrays,
            break_deferred_start,
            continue_deferred_start,
            break_cleanup_start,
            continue_cleanup_start,
        ),
        Statement::TryLet {
            name,
            value_type,
            result_type,
            return_type,
            result_expr,
        } => emit_try_let(
            out,
            name,
            value_type,
            result_type,
            return_type,
            result_expr,
            indent,
            deferred,
            active_arrays,
        ),
        Statement::TryReturnOk {
            ok_type,
            result_type,
            return_type,
            result_expr,
        } => emit_try_return_ok(
            out,
            ok_type,
            result_type,
            return_type,
            result_expr,
            indent,
            deferred,
            active_arrays,
        ),
        Statement::Assign { name, value } => emit_assign(out, name, value, indent, active_arrays),
        Statement::AssignField {
            base,
            field,
            value_type,
            value,
        } => emit_assign_field(out, base, field, value_type, value, indent),
        Statement::Println(arg) => {
            write_indent(out, indent);
            out.push_str("puts(");
            emit_string_data_expr(out, arg);
            out.push_str(");\n");
        }
        Statement::Eprintln(arg) => {
            write_indent(out, indent);
            out.push_str("fputs(");
            emit_string_data_expr(out, arg);
            out.push_str(", stderr);\n");
            write_indent(out, indent);
            out.push_str("fputc('\\n', stderr);\n");
        }
        Statement::Panic(message) => {
            emit_deferred(out, indent, deferred);
            emit_array_releases(out, indent, active_arrays);
            write_indent(out, indent);
            out.push_str("nomo_panic(");
            emit_string_data_expr(out, message);
            out.push_str(");\n");
        }
        Statement::Return(Some(value)) => emit_return_value(
            out,
            value,
            indent,
            deferred,
            function_return_type,
            active_arrays,
        ),
        Statement::Return(None) => {
            emit_deferred(out, indent, deferred);
            emit_array_releases(out, indent, active_arrays);
            write_indent(out, indent);
            out.push_str("return;\n");
        }
        Statement::Expr(value) => {
            write_indent(out, indent);
            emit_expr(out, value);
            out.push_str(";\n");
        }
        Statement::Match {
            value,
            enum_name,
            enum_args,
            arms,
        } => emit_match_statement(
            out,
            value,
            enum_name,
            enum_args,
            arms,
            indent,
            deferred,
            function_return_type,
            active_arrays,
            break_deferred_start,
            continue_deferred_start,
            break_cleanup_start,
            continue_cleanup_start,
        ),
        Statement::Loop { kind, body } => emit_loop(
            out,
            kind,
            body,
            indent,
            deferred,
            function_return_type,
            active_arrays,
            break_deferred_start,
            continue_deferred_start,
            break_cleanup_start,
            continue_cleanup_start,
        ),
        Statement::Break => {
            emit_deferred(out, indent, &deferred[break_deferred_start..]);
            emit_array_releases(out, indent, &active_arrays[break_cleanup_start..]);
            write_indent(out, indent);
            out.push_str("break;\n");
        }
        Statement::Continue => {
            emit_deferred(out, indent, &deferred[continue_deferred_start..]);
            emit_array_releases(out, indent, &active_arrays[continue_cleanup_start..]);
            write_indent(out, indent);
            out.push_str("continue;\n");
        }
        Statement::Defer { .. } => {
            // Deferred calls are collected by emit_body and emitted at exit points.
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn emit_match_statement(
    out: &mut String,
    value: &ValueExpr,
    enum_name: &str,
    enum_args: &[ValueType],
    arms: &[MatchStatementArm],
    indent: usize,
    deferred: &[DeferredCall],
    function_return_type: &ValueType,
    active_arrays: &[LocalArray],
    break_deferred_start: usize,
    continue_deferred_start: usize,
    break_cleanup_start: usize,
    continue_cleanup_start: usize,
) {
    for (index, arm) in arms.iter().enumerate() {
        write_indent(out, indent);
        if index == 0 {
            out.push_str("if (");
        } else {
            out.push_str("else if (");
        }
        emit_expr(out, value);
        out.push_str(".tag == ");
        out.push_str(&c_enum_variant_ident(enum_name, enum_args, &arm.variant));
        out.push_str(") {\n");
        emit_block(
            out,
            &arm.body,
            indent + 1,
            deferred,
            function_return_type,
            active_arrays,
            break_deferred_start,
            continue_deferred_start,
            break_cleanup_start,
            continue_cleanup_start,
        );
        write_indent(out, indent);
        out.push_str("}\n");
    }
}

#[allow(clippy::too_many_arguments)]
fn emit_if_statement(
    out: &mut String,
    condition: &ValueExpr,
    body: &[Statement],
    else_body: &[Statement],
    indent: usize,
    deferred: &[DeferredCall],
    function_return_type: &ValueType,
    active_arrays: &[LocalArray],
    break_deferred_start: usize,
    continue_deferred_start: usize,
    break_cleanup_start: usize,
    continue_cleanup_start: usize,
) {
    write_indent(out, indent);
    out.push_str("if (");
    emit_expr(out, condition);
    out.push_str(") {\n");
    emit_block(
        out,
        body,
        indent + 1,
        deferred,
        function_return_type,
        active_arrays,
        break_deferred_start,
        continue_deferred_start,
        break_cleanup_start,
        continue_cleanup_start,
    );
    write_indent(out, indent);
    out.push_str("} else {\n");
    emit_block(
        out,
        else_body,
        indent + 1,
        deferred,
        function_return_type,
        active_arrays,
        break_deferred_start,
        continue_deferred_start,
        break_cleanup_start,
        continue_cleanup_start,
    );
    write_indent(out, indent);
    out.push_str("}\n");
}

#[allow(clippy::too_many_arguments)]
fn emit_if_let(
    out: &mut String,
    binding: Option<&str>,
    value_type: Option<&ValueType>,
    value: &ValueExpr,
    enum_name: &str,
    enum_args: &[ValueType],
    variant: &str,
    body: &[Statement],
    else_body: Option<&[Statement]>,
    indent: usize,
    deferred: &[DeferredCall],
    function_return_type: &ValueType,
    active_arrays: &[LocalArray],
    break_deferred_start: usize,
    continue_deferred_start: usize,
    break_cleanup_start: usize,
    continue_cleanup_start: usize,
) {
    let temp = format!(
        "nomo__if_let_{}",
        c_enum_variant_ident(enum_name, enum_args, variant)
    );
    write_indent(out, indent);
    out.push_str("{\n");
    write_indent(out, indent + 1);
    out.push_str(&c_enum_ident(enum_name, enum_args));
    out.push(' ');
    out.push_str(&temp);
    out.push_str(" = ");
    emit_expr(out, value);
    out.push_str(";\n");
    write_indent(out, indent + 1);
    out.push_str("if (");
    out.push_str(&temp);
    out.push_str(".tag == ");
    out.push_str(&c_enum_variant_ident(enum_name, enum_args, variant));
    out.push_str(") {\n");
    if let (Some(binding), Some(value_type)) = (binding, value_type) {
        write_indent(out, indent + 2);
        out.push_str(&c_type(value_type));
        out.push(' ');
        out.push_str(&c_var_ident(binding));
        out.push_str(" = ");
        out.push_str(&temp);
        out.push_str(".payload.");
        out.push_str(&c_payload_ident(variant));
        out.push_str(";\n");
        emit_array_retain_binding(out, binding, value_type, indent + 2);
    }
    emit_enum_temp_release_if_owned(out, &temp, enum_name, enum_args, value, indent + 2);
    let mut then_active_arrays = active_arrays.to_vec();
    if let (Some(binding), Some(value_type)) = (binding, value_type) {
        if let Some(local) = local_array(binding, value_type) {
            then_active_arrays.push(local);
        }
    }
    emit_block(
        out,
        body,
        indent + 2,
        deferred,
        function_return_type,
        &then_active_arrays,
        break_deferred_start,
        continue_deferred_start,
        break_cleanup_start,
        continue_cleanup_start,
    );
    if let (Some(binding), Some(value_type)) = (binding, value_type) {
        emit_value_release_binding(out, binding, value_type, indent + 2);
    }
    write_indent(out, indent + 1);
    out.push_str("}");
    if let Some(else_body) = else_body {
        out.push_str(" else {\n");
        emit_enum_temp_release_if_owned(out, &temp, enum_name, enum_args, value, indent + 2);
        emit_block(
            out,
            else_body,
            indent + 2,
            deferred,
            function_return_type,
            active_arrays,
            break_deferred_start,
            continue_deferred_start,
            break_cleanup_start,
            continue_cleanup_start,
        );
        write_indent(out, indent + 1);
        out.push('}');
    } else {
        out.push_str(" else {\n");
        emit_enum_temp_release_if_owned(out, &temp, enum_name, enum_args, value, indent + 2);
        write_indent(out, indent + 1);
        out.push('}');
    }
    out.push('\n');
    write_indent(out, indent);
    out.push_str("}\n");
}

fn emit_loop(
    out: &mut String,
    kind: &LoopKind,
    body: &[Statement],
    indent: usize,
    deferred: &[DeferredCall],
    function_return_type: &ValueType,
    active_arrays: &[LocalArray],
    _break_deferred_start: usize,
    _continue_deferred_start: usize,
    _break_cleanup_start: usize,
    _continue_cleanup_start: usize,
) {
    match kind {
        LoopKind::Infinite => {
            write_indent(out, indent);
            out.push_str("for (;;) {\n");
            emit_block(
                out,
                body,
                indent + 1,
                deferred,
                function_return_type,
                active_arrays,
                deferred.len(),
                deferred.len(),
                active_arrays.len(),
                active_arrays.len(),
            );
            write_indent(out, indent);
            out.push_str("}\n");
        }
        LoopKind::While(condition) => {
            write_indent(out, indent);
            out.push_str("while (");
            emit_expr(out, condition);
            out.push_str(") {\n");
            emit_block(
                out,
                body,
                indent + 1,
                deferred,
                function_return_type,
                active_arrays,
                deferred.len(),
                deferred.len(),
                active_arrays.len(),
                active_arrays.len(),
            );
            write_indent(out, indent);
            out.push_str("}\n");
        }
        LoopKind::Iterate {
            binding,
            element_type,
            iterable,
        } => {
            let array_type = ValueType::Array(Box::new(element_type.clone()));
            let owned_iterable = !expr_may_share_array_storage(iterable);
            write_indent(out, indent);
            out.push_str("{\n");
            write_indent(out, indent + 1);
            out.push_str(&c_type(&array_type));
            out.push_str(" nomo__seq = ");
            emit_expr(out, iterable);
            out.push_str(";\n");
            write_indent(out, indent + 1);
            out.push_str("for (uint64_t nomo_i = 0; nomo_i < nomo__seq.len; nomo_i++) {\n");
            write_indent(out, indent + 2);
            out.push_str(&c_type(element_type));
            out.push(' ');
            out.push_str(&c_var_ident(binding));
            out.push_str(" = nomo__seq.data[nomo_i];\n");
            emit_array_retain_binding(out, binding, element_type, indent + 2);
            let mut body_active_arrays = active_arrays.to_vec();
            if owned_iterable {
                if let Some(local) = local_c_value("nomo__seq", &array_type) {
                    body_active_arrays.push(local);
                }
            }
            let loop_binding_cleanup_start = body_active_arrays.len();
            if let Some(local) = local_array(binding, element_type) {
                body_active_arrays.push(local);
            }
            emit_block(
                out,
                body,
                indent + 2,
                deferred,
                function_return_type,
                &body_active_arrays,
                deferred.len(),
                deferred.len(),
                loop_binding_cleanup_start,
                loop_binding_cleanup_start,
            );
            emit_value_release_binding(out, binding, element_type, indent + 2);
            write_indent(out, indent + 1);
            out.push_str("}\n");
            if owned_iterable {
                emit_value_release_in_place(out, &array_type, "nomo__seq", indent + 1);
            }
            write_indent(out, indent);
            out.push_str("}\n");
        }
    }
}

fn emit_block(
    out: &mut String,
    body: &[Statement],
    indent: usize,
    deferred: &[DeferredCall],
    function_return_type: &ValueType,
    active_arrays: &[LocalArray],
    break_deferred_start: usize,
    continue_deferred_start: usize,
    break_cleanup_start: usize,
    continue_cleanup_start: usize,
) {
    let inherited_len = active_arrays.len();
    let mut scope_arrays = active_arrays.to_vec();
    let mut block_deferred: Vec<DeferredCall> = Vec::new();
    let mut last_statement_exits = false;
    for statement in body {
        if let Statement::Defer { call } = statement {
            block_deferred.push(call.clone());
            last_statement_exits = false;
            continue;
        }
        let mut active_deferred = deferred.to_vec();
        active_deferred.extend(block_deferred.iter().cloned());
        emit_stmt(
            out,
            statement,
            indent,
            &active_deferred,
            function_return_type,
            &scope_arrays,
            break_deferred_start,
            continue_deferred_start,
            break_cleanup_start,
            continue_cleanup_start,
        );
        if let Some(local) = local_array_from_statement(statement) {
            scope_arrays.push(local);
        }
        last_statement_exits = statement_exits_block(statement);
        if last_statement_exits {
            break;
        }
    }
    if !last_statement_exits {
        emit_deferred(out, indent, &block_deferred);
        if scope_arrays.len() > inherited_len {
            emit_array_releases(out, indent, &scope_arrays[inherited_len..]);
        }
    }
}

fn emit_return_value(
    out: &mut String,
    value: &ValueExpr,
    indent: usize,
    deferred: &[DeferredCall],
    return_type: &ValueType,
    active_arrays: &[LocalArray],
) {
    if deferred.is_empty() {
        if !active_arrays.is_empty() {
            write_indent(out, indent);
            out.push_str("{\n");
            write_indent(out, indent + 1);
            out.push_str(&c_type(return_type));
            out.push_str(" nomo__return = ");
            emit_expr(out, value);
            out.push_str(";\n");
            emit_array_retain_return_if_needed(out, value, return_type, indent + 1);
            emit_array_releases(out, indent + 1, active_arrays);
            write_indent(out, indent + 1);
            out.push_str("return nomo__return;\n");
            write_indent(out, indent);
            out.push_str("}\n");
            return;
        }
        write_indent(out, indent);
        out.push_str("return ");
        emit_expr(out, value);
        out.push_str(";\n");
        return;
    }

    write_indent(out, indent);
    out.push_str("{\n");
    write_indent(out, indent + 1);
    out.push_str(&c_type(return_type));
    out.push_str(" nomo__return = ");
    emit_expr(out, value);
    out.push_str(";\n");
    emit_array_retain_return_if_needed(out, value, return_type, indent + 1);
    emit_deferred(out, indent + 1, deferred);
    emit_array_releases(out, indent + 1, active_arrays);
    write_indent(out, indent + 1);
    out.push_str("return nomo__return;\n");
    write_indent(out, indent);
    out.push_str("}\n");
}

fn emit_let(
    out: &mut String,
    name: &str,
    value_type: &ValueType,
    initializer: &ValueExpr,
    indent: usize,
) {
    write_indent(out, indent);
    out.push_str(&c_type(value_type));
    out.push(' ');
    out.push_str(&c_var_ident(name));
    out.push_str(" = ");
    emit_expr(out, initializer);
    out.push_str(";\n");
    emit_array_retain_after_binding(out, name, value_type, initializer, indent);
}

#[allow(clippy::too_many_arguments)]
fn emit_let_if(
    out: &mut String,
    name: &str,
    value_type: &ValueType,
    condition: &ValueExpr,
    body: &[Statement],
    else_body: &[Statement],
    indent: usize,
    deferred: &[DeferredCall],
    function_return_type: &ValueType,
    active_arrays: &[LocalArray],
    break_deferred_start: usize,
    continue_deferred_start: usize,
    break_cleanup_start: usize,
    continue_cleanup_start: usize,
) {
    write_indent(out, indent);
    out.push_str(&c_type(value_type));
    out.push(' ');
    out.push_str(&c_var_ident(name));
    out.push_str(";\n");
    emit_if_statement(
        out,
        condition,
        body,
        else_body,
        indent,
        deferred,
        function_return_type,
        active_arrays,
        break_deferred_start,
        continue_deferred_start,
        break_cleanup_start,
        continue_cleanup_start,
    );
}

#[allow(clippy::too_many_arguments)]
fn emit_let_match(
    out: &mut String,
    name: &str,
    value_type: &ValueType,
    value: &ValueExpr,
    enum_name: &str,
    enum_args: &[ValueType],
    arms: &[MatchStatementArm],
    indent: usize,
    deferred: &[DeferredCall],
    function_return_type: &ValueType,
    active_arrays: &[LocalArray],
    break_deferred_start: usize,
    continue_deferred_start: usize,
    break_cleanup_start: usize,
    continue_cleanup_start: usize,
) {
    write_indent(out, indent);
    out.push_str(&c_type(value_type));
    out.push(' ');
    out.push_str(&c_var_ident(name));
    out.push_str(";\n");
    emit_match_statement(
        out,
        value,
        enum_name,
        enum_args,
        arms,
        indent,
        deferred,
        function_return_type,
        active_arrays,
        break_deferred_start,
        continue_deferred_start,
        break_cleanup_start,
        continue_cleanup_start,
    );
}

fn emit_assign(
    out: &mut String,
    name: &str,
    value: &ValueExpr,
    indent: usize,
    active_arrays: &[LocalArray],
) {
    let Some(value_type) = active_array_type(active_arrays, name) else {
        write_indent(out, indent);
        out.push_str(&c_var_ident(name));
        out.push_str(" = ");
        emit_expr(out, value);
        out.push_str(";\n");
        return;
    };
    if is_array_mutating_assignment(value) {
        write_indent(out, indent);
        out.push_str(&c_var_ident(name));
        out.push_str(" = ");
        emit_expr(out, value);
        out.push_str(";\n");
        return;
    }

    let temp = format!("nomo__assign_{}", c_var_ident(name));
    write_indent(out, indent);
    out.push_str("{\n");
    write_indent(out, indent + 1);
    out.push_str(&c_type(value_type));
    out.push(' ');
    out.push_str(&temp);
    out.push_str(" = ");
    emit_expr(out, value);
    out.push_str(";\n");
    emit_value_retain_value_if_needed(out, &temp, value_type, value, indent + 1);
    emit_value_release_binding(out, name, value_type, indent + 1);
    write_indent(out, indent + 1);
    out.push_str(&c_var_ident(name));
    out.push_str(" = ");
    out.push_str(&temp);
    out.push_str(";\n");
    write_indent(out, indent);
    out.push_str("}\n");
}

fn emit_assign_field(
    out: &mut String,
    base: &str,
    field: &str,
    value_type: &ValueType,
    value: &ValueExpr,
    indent: usize,
) {
    let field_access = format!("{}.{}", c_var_ident(base), c_member_ident(field));
    if !value_type_needs_release(value_type) {
        write_indent(out, indent);
        out.push_str(&field_access);
        out.push_str(" = ");
        emit_expr(out, value);
        out.push_str(";\n");
        return;
    }

    let temp = format!(
        "nomo__assign_{}_{}",
        c_var_ident(base),
        c_member_ident(field)
    );
    write_indent(out, indent);
    out.push_str("{\n");
    write_indent(out, indent + 1);
    out.push_str(&c_type(value_type));
    out.push(' ');
    out.push_str(&temp);
    out.push_str(" = ");
    emit_expr(out, value);
    out.push_str(";\n");
    emit_value_retain_value_if_needed(out, &temp, value_type, value, indent + 1);
    emit_value_release_in_place(out, value_type, &field_access, indent + 1);
    write_indent(out, indent + 1);
    out.push_str(&field_access);
    out.push_str(" = ");
    out.push_str(&temp);
    out.push_str(";\n");
    write_indent(out, indent);
    out.push_str("}\n");
}

fn is_array_mutating_assignment(value: &ValueExpr) -> bool {
    matches!(
        value,
        ValueExpr::ArrayPush { .. } | ValueExpr::ArraySet { .. }
    )
}

#[allow(clippy::too_many_arguments)]
fn emit_let_else(
    out: &mut String,
    binding: &str,
    value_type: &ValueType,
    value: &ValueExpr,
    enum_name: &str,
    enum_args: &[ValueType],
    variant: &str,
    else_body: &[Statement],
    indent: usize,
    deferred: &[DeferredCall],
    function_return_type: &ValueType,
    active_arrays: &[LocalArray],
    break_deferred_start: usize,
    continue_deferred_start: usize,
    break_cleanup_start: usize,
    continue_cleanup_start: usize,
) {
    let temp = format!("nomo__let_else_{}", c_var_ident(binding));
    write_indent(out, indent);
    out.push_str(&c_enum_ident(enum_name, enum_args));
    out.push(' ');
    out.push_str(&temp);
    out.push_str(" = ");
    emit_expr(out, value);
    out.push_str(";\n");
    write_indent(out, indent);
    out.push_str("if (");
    out.push_str(&temp);
    out.push_str(".tag != ");
    out.push_str(&c_enum_variant_ident(enum_name, enum_args, variant));
    out.push_str(") {\n");
    emit_enum_temp_release_if_owned(out, &temp, enum_name, enum_args, value, indent + 1);
    emit_block(
        out,
        else_body,
        indent + 1,
        deferred,
        function_return_type,
        active_arrays,
        break_deferred_start,
        continue_deferred_start,
        break_cleanup_start,
        continue_cleanup_start,
    );
    write_indent(out, indent);
    out.push_str("}\n");
    write_indent(out, indent);
    out.push_str(&c_type(value_type));
    out.push(' ');
    out.push_str(&c_var_ident(binding));
    out.push_str(" = ");
    out.push_str(&temp);
    out.push_str(".payload.");
    out.push_str(&c_payload_ident(variant));
    out.push_str(";\n");
    emit_array_retain_binding(out, binding, value_type, indent);
    emit_enum_temp_release_if_owned(out, &temp, enum_name, enum_args, value, indent);
}

fn emit_enum_temp_release_if_owned(
    out: &mut String,
    temp: &str,
    enum_name: &str,
    enum_args: &[ValueType],
    value: &ValueExpr,
    indent: usize,
) {
    let enum_type = ValueType::Enum(enum_name.to_string(), enum_args.to_vec());
    if expr_may_share_array_storage(value) || !value_type_needs_release(&enum_type) {
        return;
    }
    emit_value_release_in_place(out, &enum_type, temp, indent);
}

fn emit_array_retain_after_binding(
    out: &mut String,
    name: &str,
    value_type: &ValueType,
    initializer: &ValueExpr,
    indent: usize,
) {
    if !value_type_needs_release(value_type) || !expr_may_share_array_storage(initializer) {
        return;
    }
    emit_value_retain_in_place(out, value_type, &c_var_ident(name), indent);
}

fn emit_array_retain_binding(out: &mut String, name: &str, value_type: &ValueType, indent: usize) {
    if !value_type_needs_release(value_type) {
        return;
    }
    emit_value_retain_in_place(out, value_type, &c_var_ident(name), indent);
}

fn emit_value_retain_value_if_needed(
    out: &mut String,
    c_value: &str,
    value_type: &ValueType,
    initializer: &ValueExpr,
    indent: usize,
) {
    if !expr_may_share_array_storage(initializer) || !value_type_needs_release(value_type) {
        return;
    }
    emit_value_retain_in_place(out, value_type, c_value, indent);
}

fn local_array_from_statement(statement: &Statement) -> Option<LocalArray> {
    match statement {
        Statement::Let {
            name, value_type, ..
        }
        | Statement::LetIf {
            name, value_type, ..
        }
        | Statement::LetMatch {
            name, value_type, ..
        }
        | Statement::TryLet {
            name, value_type, ..
        } => local_array(name, value_type),
        Statement::LetElse {
            binding,
            value_type,
            ..
        } => local_array(binding, value_type),
        _ => None,
    }
}

fn array_params(function: &Function) -> Vec<LocalArray> {
    function
        .params
        .iter()
        .filter(|param| !param.mutable)
        .filter_map(|param| local_array(&param.name, &param.value_type))
        .collect()
}

fn local_array(name: &str, value_type: &ValueType) -> Option<LocalArray> {
    if value_type_needs_release(value_type) {
        Some(LocalArray {
            name: name.to_string(),
            value_type: value_type.clone(),
            c_value: None,
        })
    } else {
        None
    }
}

fn local_c_value(c_value: &str, value_type: &ValueType) -> Option<LocalArray> {
    if value_type_needs_release(value_type) {
        Some(LocalArray {
            name: c_value.to_string(),
            value_type: value_type.clone(),
            c_value: Some(c_value.to_string()),
        })
    } else {
        None
    }
}

fn emit_array_releases(out: &mut String, indent: usize, active_arrays: &[LocalArray]) {
    for local in active_arrays.iter().rev() {
        if let Some(c_value) = &local.c_value {
            emit_value_release_in_place(out, &local.value_type, c_value, indent);
        } else {
            emit_value_release_binding(out, &local.name, &local.value_type, indent);
        }
    }
}

fn value_type_needs_release(value_type: &ValueType) -> bool {
    match value_type {
        ValueType::String => true,
        ValueType::Array(element_type) => is_supported_array_element(element_type),
        ValueType::Struct(_, _) | ValueType::Enum(_, _) => true,
        _ => false,
    }
}

fn emit_value_release_binding(out: &mut String, name: &str, value_type: &ValueType, indent: usize) {
    emit_value_release_in_place(out, value_type, &c_var_ident(name), indent);
}

fn emit_value_release_in_place(
    out: &mut String,
    value_type: &ValueType,
    c_value: &str,
    indent: usize,
) {
    match value_type {
        ValueType::Array(element_type) if is_supported_array_element(element_type) => {
            write_indent(out, indent);
            emit_array_release_expr(out, element_type, c_value);
            out.push_str(";\n");
        }
        ValueType::String => {
            write_indent(out, indent);
            out.push_str("nomo_string_release(");
            out.push_str(c_value);
            out.push_str(");\n");
        }
        ValueType::Struct(_, _) | ValueType::Enum(_, _) => {
            write_indent(out, indent);
            out.push_str(&c_release_ident(value_type));
            out.push('(');
            out.push_str(c_value);
            out.push_str(");\n");
        }
        _ => {}
    }
}

fn emit_value_retain_in_place(
    out: &mut String,
    value_type: &ValueType,
    c_value: &str,
    indent: usize,
) {
    match value_type {
        ValueType::Array(element_type) if is_supported_array_element(element_type) => {
            write_indent(out, indent);
            out.push_str(c_value);
            out.push_str(" = ");
            out.push_str(&c_array_ident(element_type));
            out.push_str("_retain(");
            out.push_str(c_value);
            out.push_str(");\n");
        }
        ValueType::String => {
            write_indent(out, indent);
            out.push_str(c_value);
            out.push_str(" = nomo_string_retain(");
            out.push_str(c_value);
            out.push_str(");\n");
        }
        ValueType::Struct(_, _) | ValueType::Enum(_, _) => {
            write_indent(out, indent);
            out.push_str(c_value);
            out.push_str(" = ");
            out.push_str(&c_retain_ident(value_type));
            out.push('(');
            out.push_str(c_value);
            out.push_str(");\n");
        }
        _ => {}
    }
}

fn emit_array_release_expr(out: &mut String, element_type: &ValueType, c_value: &str) {
    out.push_str(&c_array_ident(element_type));
    out.push_str("_release(");
    out.push_str(c_value);
    out.push(')');
}

fn active_array_type<'a>(active_arrays: &'a [LocalArray], name: &str) -> Option<&'a ValueType> {
    active_arrays
        .iter()
        .find(|local| local.name == name)
        .map(|local| &local.value_type)
}

fn emit_array_retain_return_if_needed(
    out: &mut String,
    value: &ValueExpr,
    return_type: &ValueType,
    indent: usize,
) {
    if !value_type_needs_release(return_type) || !expr_may_share_array_storage(value) {
        return;
    }
    emit_value_retain_in_place(out, return_type, "nomo__return", indent);
}

fn expr_may_share_array_storage(value: &ValueExpr) -> bool {
    match value {
        ValueExpr::Variable(_)
        | ValueExpr::FieldAccess { .. }
        | ValueExpr::EnumPayload { .. }
        | ValueExpr::EnumPayloadFieldAccess { .. } => true,
        ValueExpr::Cast { expr, .. }
        | ValueExpr::Unary { expr, .. }
        | ValueExpr::StringLen { value: expr }
        | ValueExpr::FsReadToString { path: expr }
        | ValueExpr::FsOpen { path: expr }
        | ValueExpr::FileClose { file: expr }
        | ValueExpr::EnvGet { name: expr }
        | ValueExpr::ArrayLen { array: expr }
        | ValueExpr::EnumVariant {
            payload: Some(expr),
            ..
        } => expr_may_share_array_storage(expr),
        ValueExpr::Binary { left, right, .. }
        | ValueExpr::StringCompare { left, right, .. }
        | ValueExpr::FsWriteString {
            path: left,
            content: right,
        } => expr_may_share_array_storage(left) || expr_may_share_array_storage(right),
        ValueExpr::StringConcat { .. } => false,
        ValueExpr::ArrayPush { value, .. } | ValueExpr::ArraySet { value, .. } => {
            expr_may_share_array_storage(value)
        }
        ValueExpr::ResultMapErr { result, .. } => expr_may_share_array_storage(result),
        ValueExpr::StructLiteral { fields, .. } => fields
            .iter()
            .any(|(_, field)| expr_may_share_array_storage(field)),
        ValueExpr::Match { value, arms } => {
            expr_may_share_array_storage(value)
                || arms
                    .iter()
                    .any(|arm| expr_may_share_array_storage(&arm.value))
        }
        ValueExpr::If {
            condition,
            then_branch,
            else_branch,
        } => {
            expr_may_share_array_storage(condition)
                || expr_may_share_array_storage(then_branch)
                || expr_may_share_array_storage(else_branch)
        }
        ValueExpr::StringLiteral(_)
        | ValueExpr::IntLiteral(_)
        | ValueExpr::FloatLiteral(_)
        | ValueExpr::CharLiteral(_)
        | ValueExpr::BoolLiteral(_)
        | ValueExpr::VoidLiteral
        | ValueExpr::Panic { .. }
        | ValueExpr::MutBorrow(_)
        | ValueExpr::Call { .. }
        | ValueExpr::EnvArgs
        | ValueExpr::ArrayNew { .. }
        | ValueExpr::ArrayGet { .. }
        | ValueExpr::EnumVariant { payload: None, .. } => false,
    }
}

fn emit_try_let(
    out: &mut String,
    name: &str,
    value_type: &ValueType,
    result_type: &ValueType,
    return_type: &ValueType,
    result_expr: &ValueExpr,
    indent: usize,
    deferred: &[DeferredCall],
    active_arrays: &[LocalArray],
) {
    let temp = format!("{}_result", c_var_ident(name));
    write_indent(out, indent);
    out.push_str(&c_type(result_type));
    out.push(' ');
    out.push_str(&temp);
    out.push_str(" = ");
    emit_expr(out, result_expr);
    out.push_str(";\n");
    let ValueType::Enum(result_name, result_args) = result_type else {
        return;
    };
    write_indent(out, indent);
    out.push_str("if (");
    out.push_str(&temp);
    out.push_str(".tag == ");
    out.push_str(&c_enum_variant_ident(result_name, result_args, "Err"));
    out.push_str(") {\n");
    write_indent(out, indent + 1);
    let (return_name, return_args) = match return_type {
        ValueType::Enum(name, args) => (name, args),
        _ => (result_name, result_args),
    };
    out.push_str(&c_enum_ident(return_name, return_args));
    out.push_str(" nomo__try_return = (");
    out.push_str(&c_enum_ident(return_name, return_args));
    out.push_str("){.tag = ");
    out.push_str(&c_enum_variant_ident(return_name, return_args, "Err"));
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = ");
    out.push_str(&temp);
    out.push_str(".payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str("};\n");
    if expr_may_share_array_storage(result_expr) && value_type_needs_release(return_type) {
        emit_value_retain_in_place(out, return_type, "nomo__try_return", indent + 1);
    }
    emit_deferred(out, indent + 1, deferred);
    emit_array_releases(out, indent + 1, active_arrays);
    write_indent(out, indent + 1);
    out.push_str("return nomo__try_return;\n");
    write_indent(out, indent);
    out.push_str("}\n");
    write_indent(out, indent);
    out.push_str(&c_type(value_type));
    out.push(' ');
    out.push_str(&c_var_ident(name));
    out.push_str(" = ");
    out.push_str(&temp);
    out.push_str(".payload.");
    out.push_str(&c_payload_ident("Ok"));
    out.push_str(";\n");
    if expr_may_share_array_storage(result_expr) && value_type_needs_release(value_type) {
        emit_value_retain_in_place(out, value_type, &c_var_ident(name), indent);
    }
}

fn emit_try_return_ok(
    out: &mut String,
    ok_type: &ValueType,
    result_type: &ValueType,
    return_type: &ValueType,
    result_expr: &ValueExpr,
    indent: usize,
    deferred: &[DeferredCall],
    active_arrays: &[LocalArray],
) {
    write_indent(out, indent);
    out.push_str("{\n");
    write_indent(out, indent + 1);
    out.push_str(&c_type(result_type));
    out.push_str(" nomo__try_result = ");
    emit_expr(out, result_expr);
    out.push_str(";\n");
    let ValueType::Enum(result_name, result_args) = result_type else {
        write_indent(out, indent);
        out.push_str("}\n");
        return;
    };
    let (return_name, return_args) = match return_type {
        ValueType::Enum(name, args) => (name, args),
        _ => (result_name, result_args),
    };
    write_indent(out, indent + 1);
    out.push_str("if (nomo__try_result.tag == ");
    out.push_str(&c_enum_variant_ident(result_name, result_args, "Err"));
    out.push_str(") {\n");
    write_indent(out, indent + 2);
    out.push_str(&c_enum_ident(return_name, return_args));
    out.push_str(" nomo__try_return = (");
    out.push_str(&c_enum_ident(return_name, return_args));
    out.push_str("){.tag = ");
    out.push_str(&c_enum_variant_ident(return_name, return_args, "Err"));
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str(" = nomo__try_result.payload.");
    out.push_str(&c_payload_ident("Err"));
    out.push_str("};\n");
    if expr_may_share_array_storage(result_expr) && value_type_needs_release(return_type) {
        emit_value_retain_in_place(out, return_type, "nomo__try_return", indent + 2);
    }
    emit_deferred(out, indent + 2, deferred);
    emit_array_releases(out, indent + 2, active_arrays);
    write_indent(out, indent + 2);
    out.push_str("return nomo__try_return;\n");
    write_indent(out, indent + 1);
    out.push_str("}\n");
    write_indent(out, indent + 1);
    out.push_str(&c_type(ok_type));
    out.push_str(" nomo__try_ok = nomo__try_result.payload.");
    out.push_str(&c_payload_ident("Ok"));
    out.push_str(";\n");
    write_indent(out, indent + 1);
    out.push_str(&c_enum_ident(return_name, return_args));
    out.push_str(" nomo__return = (");
    out.push_str(&c_enum_ident(return_name, return_args));
    out.push_str("){.tag = ");
    out.push_str(&c_enum_variant_ident(return_name, return_args, "Ok"));
    out.push_str(", .payload.");
    out.push_str(&c_payload_ident("Ok"));
    out.push_str(" = nomo__try_ok};\n");
    if expr_may_share_array_storage(result_expr) && value_type_needs_release(return_type) {
        emit_value_retain_in_place(out, return_type, "nomo__return", indent + 1);
    }
    emit_deferred(out, indent + 1, deferred);
    emit_array_releases(out, indent + 1, active_arrays);
    write_indent(out, indent + 1);
    out.push_str("return nomo__return;\n");
    write_indent(out, indent);
    out.push_str("}\n");
}

fn emit_result_map_err_helper(out: &mut String, instance: &ResultMapErrInstance) {
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

fn emit_expr(out: &mut String, expr: &ValueExpr) {
    match expr {
        ValueExpr::StringLiteral(value) => {
            out.push_str("nomo_string_literal(\"");
            out.push_str(&escape_c_string(value));
            out.push_str("\")");
        }
        ValueExpr::IntLiteral(value) => out.push_str(&value.to_string()),
        ValueExpr::FloatLiteral(value) => out.push_str(value),
        ValueExpr::CharLiteral(value) => out.push_str(&(*value as u32).to_string()),
        ValueExpr::BoolLiteral(value) => out.push_str(if *value { "1" } else { "0" }),
        ValueExpr::VoidLiteral => out.push('0'),
        ValueExpr::Variable(name) => out.push_str(&c_var_ident(name)),
        ValueExpr::MutBorrow(path) => {
            out.push('&');
            emit_lvalue_path(out, path);
        }
        ValueExpr::Cast { expr, target_type } => {
            out.push_str("((");
            out.push_str(&c_type(target_type));
            out.push(')');
            emit_expr(out, expr);
            out.push(')');
        }
        ValueExpr::StructLiteral {
            type_name,
            struct_args,
            fields,
        } => {
            out.push('(');
            out.push_str(&c_struct_ident(type_name, struct_args));
            out.push_str("){");
            for (index, (field_name, value)) in fields.iter().enumerate() {
                if index > 0 {
                    out.push_str(", ");
                }
                out.push('.');
                out.push_str(&c_member_ident(field_name));
                out.push_str(" = ");
                emit_expr(out, value);
            }
            out.push('}');
        }
        ValueExpr::FieldAccess { base, field } => {
            out.push_str(&c_var_ident(base));
            out.push('.');
            out.push_str(&c_member_ident(field));
        }
        ValueExpr::EnumPayloadFieldAccess {
            value,
            variant,
            field,
        } => {
            emit_expr(out, value);
            out.push_str(".payload.");
            out.push_str(&c_payload_ident(variant));
            out.push('.');
            out.push_str(&c_member_ident(field));
        }
        ValueExpr::EnumVariant {
            enum_name,
            enum_args,
            variant,
            payload,
        } => {
            out.push('(');
            out.push_str(&c_enum_ident(enum_name, enum_args));
            out.push_str("){.tag = ");
            out.push_str(&c_enum_variant_ident(enum_name, enum_args, variant));
            if let Some(payload) = payload {
                out.push_str(", .payload.");
                out.push_str(&c_payload_ident(variant));
                out.push_str(" = ");
                emit_expr(out, payload);
            }
            out.push('}');
        }
        ValueExpr::EnumPayload { value, variant } => {
            emit_expr(out, value);
            out.push_str(".payload.");
            out.push_str(&c_payload_ident(variant));
        }
        ValueExpr::If {
            condition,
            then_branch,
            else_branch,
        } => {
            out.push('(');
            emit_expr(out, condition);
            out.push_str(" ? ");
            emit_expr(out, then_branch);
            out.push_str(" : ");
            emit_expr(out, else_branch);
            out.push(')');
        }
        ValueExpr::Panic {
            message,
            fallback_type,
        } => {
            out.push_str("(nomo_panic(");
            emit_string_data_expr(out, message);
            out.push_str("), ");
            out.push_str(&c_zero_value(fallback_type));
            out.push(')');
        }
        ValueExpr::Match { value, arms } => emit_match_expr(out, value, arms),
        ValueExpr::Binary { left, op, right } => {
            out.push('(');
            emit_expr(out, left);
            out.push(' ');
            out.push_str(c_binary_op(op));
            out.push(' ');
            emit_expr(out, right);
            out.push(')');
        }
        ValueExpr::Unary { op, expr } => {
            out.push('(');
            out.push_str(c_unary_op(op));
            emit_expr(out, expr);
            out.push(')');
        }
        ValueExpr::StringCompare { left, op, right } => {
            out.push('(');
            if matches!(op, BinaryOp::NotEqual) {
                out.push('!');
            }
            out.push_str("nomo_string_equal(");
            emit_expr(out, left);
            out.push_str(", ");
            emit_expr(out, right);
            out.push_str("))");
        }
        ValueExpr::Call { name, args } => {
            if name == BUILTIN_PRINTLN_EXPR {
                out.push_str("(puts(");
                emit_string_data_expr(out, &args[0]);
                out.push_str("), 0)");
            } else if name == BUILTIN_EPRINTLN_EXPR {
                out.push_str("(fputs(");
                emit_string_data_expr(out, &args[0]);
                out.push_str(", stderr), fputc('\\n', stderr), 0)");
            } else {
                out.push_str(&c_fn_ident(name));
                out.push('(');
                for (index, arg) in args.iter().enumerate() {
                    if index > 0 {
                        out.push_str(", ");
                    }
                    emit_expr(out, arg);
                }
                out.push(')');
            }
        }
        ValueExpr::StringLen { value } => {
            out.push_str("((uint64_t)strlen(");
            emit_string_data_expr(out, value);
            out.push_str("))");
        }
        ValueExpr::StringConcat { left, right } => {
            out.push_str("nomo_string_concat(");
            emit_expr(out, left);
            out.push_str(", ");
            emit_expr(out, right);
            out.push(')');
        }
        ValueExpr::FsReadToString { path } => {
            out.push_str("nomo_fs_read_to_string(");
            emit_expr(out, path);
            out.push(')');
        }
        ValueExpr::FsWriteString { path, content } => {
            out.push_str("nomo_fs_write_string(");
            emit_expr(out, path);
            out.push_str(", ");
            emit_expr(out, content);
            out.push(')');
        }
        ValueExpr::FsOpen { path } => {
            out.push_str("nomo_fs_open(");
            emit_expr(out, path);
            out.push(')');
        }
        ValueExpr::FileClose { file } => {
            out.push_str("nomo_file_close(");
            emit_expr(out, file);
            out.push(')');
        }
        ValueExpr::ResultMapErr {
            result,
            ok_type,
            source_err_type,
            target_err_type,
            converter,
        } => {
            out.push_str(&c_result_map_err_helper_ident(&ResultMapErrInstance {
                ok_type: ok_type.clone(),
                source_err_type: source_err_type.clone(),
                target_err_type: target_err_type.clone(),
                converter: converter.clone(),
            }));
            out.push('(');
            emit_expr(out, result);
            out.push(')');
        }
        ValueExpr::EnvGet { name } => {
            out.push_str("nomo_env_get(");
            emit_expr(out, name);
            out.push(')');
        }
        ValueExpr::EnvArgs => out.push_str("nomo_env_args(nomo_argc, nomo_argv)"),
        ValueExpr::ArrayNew { element_type } if is_supported_array_element(element_type) => {
            out.push_str(&c_array_ident(element_type));
            out.push_str("_new()");
        }
        ValueExpr::ArrayLen { array } => {
            out.push_str("((uint64_t)");
            emit_expr(out, array);
            out.push_str(".len)");
        }
        ValueExpr::ArrayGet {
            array,
            index,
            element_type,
        } if is_supported_array_element(element_type) => {
            out.push_str(&c_array_ident(element_type));
            out.push_str("_get(");
            emit_expr(out, array);
            out.push_str(", ");
            emit_expr(out, index);
            out.push(')');
        }
        ValueExpr::ArrayPush {
            array,
            value,
            element_type,
        } if is_supported_array_element(element_type) => {
            out.push_str(&c_array_ident(element_type));
            out.push_str("_push(");
            out.push_str(&c_var_ident(array));
            out.push_str(", ");
            emit_expr(out, value);
            out.push(')');
        }
        ValueExpr::ArraySet {
            array,
            index,
            value,
            element_type,
        } if is_supported_array_element(element_type) => {
            out.push_str(&c_array_ident(element_type));
            out.push_str("_set(");
            out.push_str(&c_var_ident(array));
            out.push_str(", ");
            emit_expr(out, index);
            out.push_str(", ");
            emit_expr(out, value);
            out.push(')');
        }
        ValueExpr::ArrayNew { element_type }
        | ValueExpr::ArrayGet { element_type, .. }
        | ValueExpr::ArrayPush { element_type, .. }
        | ValueExpr::ArraySet { element_type, .. } => {
            panic!(
                "unsupported Array element type reached C codegen: {}",
                element_type.name()
            );
        }
    }
}

fn emit_lvalue_path(out: &mut String, path: &[String]) {
    let Some((root, fields)) = path.split_first() else {
        return;
    };
    out.push_str(&c_var_ident(root));
    for field in fields {
        out.push('.');
        out.push_str(&c_member_ident(field));
    }
}

fn emit_match_expr(out: &mut String, value: &ValueExpr, arms: &[crate::compiler::MatchValueArm]) {
    emit_match_arm(out, value, arms, 0);
}

fn emit_match_arm(
    out: &mut String,
    value: &ValueExpr,
    arms: &[crate::compiler::MatchValueArm],
    index: usize,
) {
    let arm = &arms[index];
    if index + 1 == arms.len() {
        emit_expr(out, &arm.value);
        return;
    }
    out.push('(');
    emit_expr(out, value);
    out.push_str(".tag == ");
    out.push_str(&c_enum_variant_ident(
        &arm.enum_name,
        &arm.enum_args,
        &arm.variant,
    ));
    out.push_str(" ? ");
    emit_expr(out, &arm.value);
    out.push_str(" : ");
    emit_match_arm(out, value, arms, index + 1);
    out.push(')');
}

fn collect_result_map_err_instances(program: &Program) -> Vec<ResultMapErrInstance> {
    let mut out = Vec::new();
    for function in &program.functions {
        for statement in &function.body {
            collect_stmt_result_map_err(statement, &mut out);
        }
    }
    out
}

fn collect_stmt_result_map_err(statement: &Statement, out: &mut Vec<ResultMapErrInstance>) {
    match statement {
        Statement::Let { initializer, .. }
        | Statement::TryLet {
            result_expr: initializer,
            ..
        }
        | Statement::TryReturnOk {
            result_expr: initializer,
            ..
        }
        | Statement::Assign {
            value: initializer, ..
        }
        | Statement::AssignField {
            value: initializer, ..
        }
        | Statement::Println(initializer)
        | Statement::Eprintln(initializer)
        | Statement::Panic(initializer)
        | Statement::Return(Some(initializer))
        | Statement::Expr(initializer) => collect_expr_result_map_err(initializer, out),
        Statement::LetElse {
            value, else_body, ..
        } => {
            collect_expr_result_map_err(value, out);
            for statement in else_body {
                collect_stmt_result_map_err(statement, out);
            }
        }
        Statement::IfLet {
            value,
            body,
            else_body,
            ..
        } => {
            collect_expr_result_map_err(value, out);
            for statement in body {
                collect_stmt_result_map_err(statement, out);
            }
            if let Some(else_body) = else_body {
                for statement in else_body {
                    collect_stmt_result_map_err(statement, out);
                }
            }
        }
        Statement::If {
            condition,
            body,
            else_body,
        } => {
            collect_expr_result_map_err(condition, out);
            for statement in body {
                collect_stmt_result_map_err(statement, out);
            }
            for statement in else_body {
                collect_stmt_result_map_err(statement, out);
            }
        }
        Statement::LetIf {
            condition,
            body,
            else_body,
            ..
        } => {
            collect_expr_result_map_err(condition, out);
            for statement in body {
                collect_stmt_result_map_err(statement, out);
            }
            for statement in else_body {
                collect_stmt_result_map_err(statement, out);
            }
        }
        Statement::LetMatch { value, arms, .. } => {
            collect_expr_result_map_err(value, out);
            for arm in arms {
                for statement in &arm.body {
                    collect_stmt_result_map_err(statement, out);
                }
            }
        }
        Statement::Loop { kind, body } => {
            match kind {
                LoopKind::Infinite => {}
                LoopKind::While(condition) => collect_expr_result_map_err(condition, out),
                LoopKind::Iterate { iterable, .. } => collect_expr_result_map_err(iterable, out),
            }
            for statement in body {
                collect_stmt_result_map_err(statement, out);
            }
        }
        Statement::Match { value, arms, .. } => {
            collect_expr_result_map_err(value, out);
            for arm in arms {
                for statement in &arm.body {
                    collect_stmt_result_map_err(statement, out);
                }
            }
        }
        Statement::Defer { call } => collect_deferred_result_map_err(call, out),
        Statement::Return(None) | Statement::Break | Statement::Continue => {}
    }
}

fn collect_deferred_result_map_err(call: &DeferredCall, out: &mut Vec<ResultMapErrInstance>) {
    match call {
        DeferredCall::Expr(expr) | DeferredCall::Println(expr) | DeferredCall::Eprintln(expr) => {
            collect_expr_result_map_err(expr, out);
        }
    }
}

fn collect_expr_result_map_err(expr: &ValueExpr, out: &mut Vec<ResultMapErrInstance>) {
    match expr {
        ValueExpr::ResultMapErr {
            result,
            ok_type,
            source_err_type,
            target_err_type,
            converter,
        } => {
            collect_expr_result_map_err(result, out);
            let instance = ResultMapErrInstance {
                ok_type: ok_type.clone(),
                source_err_type: source_err_type.clone(),
                target_err_type: target_err_type.clone(),
                converter: converter.clone(),
            };
            if !out.contains(&instance) {
                out.push(instance);
            }
        }
        ValueExpr::Binary { left, right, .. }
        | ValueExpr::StringCompare { left, right, .. }
        | ValueExpr::StringConcat { left, right } => {
            collect_expr_result_map_err(left, out);
            collect_expr_result_map_err(right, out);
        }
        ValueExpr::FsReadToString { path }
        | ValueExpr::FsOpen { path }
        | ValueExpr::FileClose { file: path }
        | ValueExpr::EnvGet { name: path }
        | ValueExpr::StringLen { value: path }
        | ValueExpr::Unary { expr: path, .. }
        | ValueExpr::Cast { expr: path, .. }
        | ValueExpr::EnumPayload { value: path, .. }
        | ValueExpr::EnumPayloadFieldAccess { value: path, .. }
        | ValueExpr::ArrayLen { array: path } => collect_expr_result_map_err(path, out),
        ValueExpr::FsWriteString { path, content } => {
            collect_expr_result_map_err(path, out);
            collect_expr_result_map_err(content, out);
        }
        ValueExpr::Call { args, .. } => {
            for arg in args {
                collect_expr_result_map_err(arg, out);
            }
        }
        ValueExpr::ArrayGet { array, index, .. } => {
            collect_expr_result_map_err(array, out);
            collect_expr_result_map_err(index, out);
        }
        ValueExpr::ArrayPush { value, .. } => collect_expr_result_map_err(value, out),
        ValueExpr::ArraySet { index, value, .. } => {
            collect_expr_result_map_err(index, out);
            collect_expr_result_map_err(value, out);
        }
        ValueExpr::StructLiteral { fields, .. } => {
            for (_, value) in fields {
                collect_expr_result_map_err(value, out);
            }
        }
        ValueExpr::EnumVariant { payload, .. } => {
            if let Some(payload) = payload {
                collect_expr_result_map_err(payload, out);
            }
        }
        ValueExpr::If {
            condition,
            then_branch,
            else_branch,
        } => {
            collect_expr_result_map_err(condition, out);
            collect_expr_result_map_err(then_branch, out);
            collect_expr_result_map_err(else_branch, out);
        }
        ValueExpr::Panic { message, .. } => collect_expr_result_map_err(message, out),
        ValueExpr::Match { value, arms } => {
            collect_expr_result_map_err(value, out);
            for arm in arms {
                collect_expr_result_map_err(&arm.value, out);
            }
        }
        ValueExpr::StringLiteral(_)
        | ValueExpr::IntLiteral(_)
        | ValueExpr::FloatLiteral(_)
        | ValueExpr::CharLiteral(_)
        | ValueExpr::BoolLiteral(_)
        | ValueExpr::VoidLiteral
        | ValueExpr::Variable(_)
        | ValueExpr::MutBorrow(_)
        | ValueExpr::EnvArgs
        | ValueExpr::ArrayNew { .. }
        | ValueExpr::FieldAccess { .. } => {}
    }
}

fn collect_struct_instances(program: &Program) -> Vec<(String, Vec<ValueType>)> {
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for struct_type in &program.structs {
        if struct_type.type_params.is_empty() {
            push_struct_instance(&mut seen, &mut out, &struct_type.name, &[]);
        }
    }
    for function in &program.functions {
        collect_type_struct(&function.return_type, &mut seen, &mut out);
        for param in &function.params {
            collect_type_struct(&param.value_type, &mut seen, &mut out);
        }
        for statement in &function.body {
            collect_stmt_struct(statement, &mut seen, &mut out);
        }
    }
    out
}

fn collect_stmt_struct(
    statement: &Statement,
    seen: &mut BTreeSet<String>,
    out: &mut Vec<(String, Vec<ValueType>)>,
) {
    match statement {
        Statement::Let {
            value_type,
            initializer,
            ..
        } => {
            collect_type_struct(value_type, seen, out);
            collect_expr_struct(initializer, seen, out);
        }
        Statement::LetIf {
            value_type,
            condition,
            body,
            else_body,
            ..
        } => {
            collect_type_struct(value_type, seen, out);
            collect_expr_struct(condition, seen, out);
            for stmt in body {
                collect_stmt_struct(stmt, seen, out);
            }
            for stmt in else_body {
                collect_stmt_struct(stmt, seen, out);
            }
        }
        Statement::LetMatch {
            value_type,
            value,
            enum_args,
            arms,
            ..
        } => {
            collect_type_struct(value_type, seen, out);
            for arg in enum_args {
                collect_type_struct(arg, seen, out);
            }
            collect_expr_struct(value, seen, out);
            for arm in arms {
                for stmt in &arm.body {
                    collect_stmt_struct(stmt, seen, out);
                }
            }
        }
        Statement::TryLet {
            value_type,
            result_type,
            return_type,
            result_expr,
            ..
        } => {
            collect_type_struct(value_type, seen, out);
            collect_type_struct(result_type, seen, out);
            collect_type_struct(return_type, seen, out);
            collect_expr_struct(result_expr, seen, out);
        }
        Statement::TryReturnOk {
            ok_type,
            result_type,
            return_type,
            result_expr,
        } => {
            collect_type_struct(ok_type, seen, out);
            collect_type_struct(result_type, seen, out);
            collect_type_struct(return_type, seen, out);
            collect_expr_struct(result_expr, seen, out);
        }
        Statement::LetElse {
            value_type,
            value,
            enum_args,
            else_body,
            ..
        } => {
            collect_type_struct(value_type, seen, out);
            for arg in enum_args {
                collect_type_struct(arg, seen, out);
            }
            collect_expr_struct(value, seen, out);
            for stmt in else_body {
                collect_stmt_struct(stmt, seen, out);
            }
        }
        Statement::IfLet {
            value_type,
            value,
            enum_args,
            body,
            else_body,
            ..
        } => {
            if let Some(value_type) = value_type {
                collect_type_struct(value_type, seen, out);
            }
            for arg in enum_args {
                collect_type_struct(arg, seen, out);
            }
            collect_expr_struct(value, seen, out);
            for stmt in body {
                collect_stmt_struct(stmt, seen, out);
            }
            if let Some(else_body) = else_body {
                for stmt in else_body {
                    collect_stmt_struct(stmt, seen, out);
                }
            }
        }
        Statement::If {
            condition,
            body,
            else_body,
        } => {
            collect_expr_struct(condition, seen, out);
            for stmt in body {
                collect_stmt_struct(stmt, seen, out);
            }
            for stmt in else_body {
                collect_stmt_struct(stmt, seen, out);
            }
        }
        Statement::Assign { value, .. }
        | Statement::AssignField { value, .. }
        | Statement::Println(value)
        | Statement::Eprintln(value)
        | Statement::Panic(value)
        | Statement::Expr(value)
        | Statement::Return(Some(value)) => {
            collect_expr_struct(value, seen, out);
        }
        Statement::Loop { kind, body } => {
            match kind {
                LoopKind::Infinite => {}
                LoopKind::While(condition) => collect_expr_struct(condition, seen, out),
                LoopKind::Iterate {
                    element_type,
                    iterable,
                    ..
                } => {
                    collect_type_struct(element_type, seen, out);
                    collect_expr_struct(iterable, seen, out);
                }
            }
            for stmt in body {
                collect_stmt_struct(stmt, seen, out);
            }
        }
        Statement::Match { value, arms, .. } => {
            collect_expr_struct(value, seen, out);
            for arm in arms {
                for stmt in &arm.body {
                    collect_stmt_struct(stmt, seen, out);
                }
            }
        }
        Statement::Defer { call } => collect_deferred_struct(call, seen, out),
        Statement::Break | Statement::Continue => {}
        Statement::Return(None) => {}
    }
}

fn collect_deferred_struct(
    call: &DeferredCall,
    seen: &mut BTreeSet<String>,
    out: &mut Vec<(String, Vec<ValueType>)>,
) {
    match call {
        DeferredCall::Expr(expr) | DeferredCall::Println(expr) | DeferredCall::Eprintln(expr) => {
            collect_expr_struct(expr, seen, out);
        }
    }
}

fn collect_type_struct(
    value_type: &ValueType,
    seen: &mut BTreeSet<String>,
    out: &mut Vec<(String, Vec<ValueType>)>,
) {
    match value_type {
        ValueType::Struct(name, args) => {
            push_struct_instance(seen, out, name, args);
            for arg in args {
                collect_type_struct(arg, seen, out);
            }
        }
        ValueType::Enum(_, args) => {
            for arg in args {
                collect_type_struct(arg, seen, out);
            }
        }
        ValueType::Array(element) => collect_type_struct(element, seen, out),
        _ => {}
    }
}

fn collect_expr_struct(
    expr: &ValueExpr,
    seen: &mut BTreeSet<String>,
    out: &mut Vec<(String, Vec<ValueType>)>,
) {
    match expr {
        ValueExpr::Binary { left, right, .. } | ValueExpr::StringCompare { left, right, .. } => {
            collect_expr_struct(left, seen, out);
            collect_expr_struct(right, seen, out);
        }
        ValueExpr::Call { args, .. } => {
            for arg in args {
                collect_expr_struct(arg, seen, out);
            }
        }
        ValueExpr::StringLen { value } | ValueExpr::Unary { expr: value, .. } => {
            collect_expr_struct(value, seen, out)
        }
        ValueExpr::StringConcat { left, right } => {
            collect_expr_struct(left, seen, out);
            collect_expr_struct(right, seen, out);
        }
        ValueExpr::FsReadToString { path } => {
            push_struct_instance(seen, out, "FsError", &[]);
            collect_expr_struct(path, seen, out);
        }
        ValueExpr::FsWriteString { path, content } => {
            push_struct_instance(seen, out, "FsError", &[]);
            collect_expr_struct(path, seen, out);
            collect_expr_struct(content, seen, out);
        }
        ValueExpr::FsOpen { path } => {
            push_struct_instance(seen, out, "FsError", &[]);
            push_struct_instance(seen, out, "File", &[]);
            collect_expr_struct(path, seen, out);
        }
        ValueExpr::FileClose { file } => collect_expr_struct(file, seen, out),
        ValueExpr::ResultMapErr {
            result,
            ok_type,
            source_err_type,
            target_err_type,
            ..
        } => {
            collect_type_struct(ok_type, seen, out);
            collect_type_struct(source_err_type, seen, out);
            collect_type_struct(target_err_type, seen, out);
            collect_expr_struct(result, seen, out);
        }
        ValueExpr::EnvGet { name } => collect_expr_struct(name, seen, out),
        ValueExpr::EnvArgs => {}
        ValueExpr::ArrayNew { element_type } => collect_type_struct(element_type, seen, out),
        ValueExpr::ArrayLen { array } => collect_expr_struct(array, seen, out),
        ValueExpr::ArrayGet {
            array,
            index,
            element_type,
        } => {
            collect_type_struct(element_type, seen, out);
            collect_expr_struct(array, seen, out);
            collect_expr_struct(index, seen, out);
        }
        ValueExpr::ArrayPush {
            value,
            element_type,
            ..
        } => {
            collect_type_struct(element_type, seen, out);
            collect_expr_struct(value, seen, out);
        }
        ValueExpr::ArraySet {
            index,
            value,
            element_type,
            ..
        } => {
            collect_type_struct(element_type, seen, out);
            collect_expr_struct(index, seen, out);
            collect_expr_struct(value, seen, out);
        }
        ValueExpr::Cast { expr, target_type } => {
            collect_type_struct(target_type, seen, out);
            collect_expr_struct(expr, seen, out);
        }
        ValueExpr::StructLiteral {
            type_name,
            struct_args,
            fields,
        } => {
            push_struct_instance(seen, out, type_name, struct_args);
            for arg in struct_args {
                collect_type_struct(arg, seen, out);
            }
            for (_, value) in fields {
                collect_expr_struct(value, seen, out);
            }
        }
        ValueExpr::EnumVariant {
            enum_args, payload, ..
        } => {
            for arg in enum_args {
                collect_type_struct(arg, seen, out);
            }
            if let Some(payload) = payload {
                collect_expr_struct(payload, seen, out);
            }
        }
        ValueExpr::EnumPayload { value, .. } => collect_expr_struct(value, seen, out),
        ValueExpr::EnumPayloadFieldAccess { value, .. } => collect_expr_struct(value, seen, out),
        ValueExpr::If {
            condition,
            then_branch,
            else_branch,
        } => {
            collect_expr_struct(condition, seen, out);
            collect_expr_struct(then_branch, seen, out);
            collect_expr_struct(else_branch, seen, out);
        }
        ValueExpr::Panic {
            message,
            fallback_type,
        } => {
            collect_type_struct(fallback_type, seen, out);
            collect_expr_struct(message, seen, out);
        }
        ValueExpr::Match { value, arms } => {
            collect_expr_struct(value, seen, out);
            for arm in arms {
                for arg in &arm.enum_args {
                    collect_type_struct(arg, seen, out);
                }
                collect_expr_struct(&arm.value, seen, out);
            }
        }
        ValueExpr::StringLiteral(_)
        | ValueExpr::IntLiteral(_)
        | ValueExpr::FloatLiteral(_)
        | ValueExpr::CharLiteral(_)
        | ValueExpr::BoolLiteral(_)
        | ValueExpr::VoidLiteral
        | ValueExpr::Variable(_)
        | ValueExpr::MutBorrow(_)
        | ValueExpr::FieldAccess { .. } => {}
    }
}

fn collect_enum_instances(program: &Program) -> Vec<(String, Vec<ValueType>)> {
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for enum_type in &program.enums {
        if enum_type.type_params.is_empty() {
            push_enum_instance(&mut seen, &mut out, &enum_type.name, &[]);
        }
    }
    for function in &program.functions {
        collect_type_enum(&function.return_type, &mut seen, &mut out);
        for param in &function.params {
            collect_type_enum(&param.value_type, &mut seen, &mut out);
        }
        for statement in &function.body {
            collect_stmt_enum(statement, &mut seen, &mut out);
        }
    }
    for element_type in collect_array_element_types(program) {
        push_enum_instance(&mut seen, &mut out, "Option", &[element_type]);
    }
    out
}

fn collect_stmt_enum(
    statement: &Statement,
    seen: &mut BTreeSet<String>,
    out: &mut Vec<(String, Vec<ValueType>)>,
) {
    match statement {
        Statement::Let {
            value_type,
            initializer,
            ..
        } => {
            collect_type_enum(value_type, seen, out);
            collect_expr_enum(initializer, seen, out);
        }
        Statement::LetIf {
            value_type,
            condition,
            body,
            else_body,
            ..
        } => {
            collect_type_enum(value_type, seen, out);
            collect_expr_enum(condition, seen, out);
            for stmt in body {
                collect_stmt_enum(stmt, seen, out);
            }
            for stmt in else_body {
                collect_stmt_enum(stmt, seen, out);
            }
        }
        Statement::LetMatch {
            value_type,
            value,
            enum_name,
            enum_args,
            arms,
            ..
        } => {
            push_enum_instance(seen, out, enum_name, enum_args);
            collect_type_enum(value_type, seen, out);
            for arg in enum_args {
                collect_type_enum(arg, seen, out);
            }
            collect_expr_enum(value, seen, out);
            for arm in arms {
                for stmt in &arm.body {
                    collect_stmt_enum(stmt, seen, out);
                }
            }
        }
        Statement::TryLet {
            value_type,
            result_type,
            return_type,
            result_expr,
            ..
        } => {
            collect_type_enum(value_type, seen, out);
            collect_type_enum(result_type, seen, out);
            collect_type_enum(return_type, seen, out);
            collect_expr_enum(result_expr, seen, out);
        }
        Statement::TryReturnOk {
            ok_type,
            result_type,
            return_type,
            result_expr,
        } => {
            collect_type_enum(ok_type, seen, out);
            collect_type_enum(result_type, seen, out);
            collect_type_enum(return_type, seen, out);
            collect_expr_enum(result_expr, seen, out);
        }
        Statement::LetElse {
            value_type,
            value,
            enum_name,
            enum_args,
            else_body,
            ..
        } => {
            push_enum_instance(seen, out, enum_name, enum_args);
            collect_type_enum(value_type, seen, out);
            for arg in enum_args {
                collect_type_enum(arg, seen, out);
            }
            collect_expr_enum(value, seen, out);
            for stmt in else_body {
                collect_stmt_enum(stmt, seen, out);
            }
        }
        Statement::IfLet {
            value_type,
            value,
            enum_name,
            enum_args,
            body,
            else_body,
            ..
        } => {
            push_enum_instance(seen, out, enum_name, enum_args);
            if let Some(value_type) = value_type {
                collect_type_enum(value_type, seen, out);
            }
            for arg in enum_args {
                collect_type_enum(arg, seen, out);
            }
            collect_expr_enum(value, seen, out);
            for stmt in body {
                collect_stmt_enum(stmt, seen, out);
            }
            if let Some(else_body) = else_body {
                for stmt in else_body {
                    collect_stmt_enum(stmt, seen, out);
                }
            }
        }
        Statement::If {
            condition,
            body,
            else_body,
        } => {
            collect_expr_enum(condition, seen, out);
            for stmt in body {
                collect_stmt_enum(stmt, seen, out);
            }
            for stmt in else_body {
                collect_stmt_enum(stmt, seen, out);
            }
        }
        Statement::Assign { value, .. }
        | Statement::AssignField { value, .. }
        | Statement::Println(value)
        | Statement::Eprintln(value)
        | Statement::Panic(value)
        | Statement::Expr(value)
        | Statement::Return(Some(value)) => {
            collect_expr_enum(value, seen, out);
        }
        Statement::Loop { kind, body } => {
            match kind {
                LoopKind::Infinite => {}
                LoopKind::While(condition) => collect_expr_enum(condition, seen, out),
                LoopKind::Iterate {
                    element_type,
                    iterable,
                    ..
                } => {
                    collect_type_enum(element_type, seen, out);
                    collect_expr_enum(iterable, seen, out);
                }
            }
            for stmt in body {
                collect_stmt_enum(stmt, seen, out);
            }
        }
        Statement::Match {
            value,
            enum_name,
            enum_args,
            arms,
        } => {
            push_enum_instance(seen, out, enum_name, enum_args);
            collect_expr_enum(value, seen, out);
            for arm in arms {
                for stmt in &arm.body {
                    collect_stmt_enum(stmt, seen, out);
                }
            }
        }
        Statement::Defer { call } => collect_deferred_enum(call, seen, out),
        Statement::Break | Statement::Continue => {}
        Statement::Return(None) => {}
    }
}

fn collect_deferred_enum(
    call: &DeferredCall,
    seen: &mut BTreeSet<String>,
    out: &mut Vec<(String, Vec<ValueType>)>,
) {
    match call {
        DeferredCall::Expr(expr) | DeferredCall::Println(expr) | DeferredCall::Eprintln(expr) => {
            collect_expr_enum(expr, seen, out);
        }
    }
}

fn collect_type_enum(
    value_type: &ValueType,
    seen: &mut BTreeSet<String>,
    out: &mut Vec<(String, Vec<ValueType>)>,
) {
    match value_type {
        ValueType::Enum(name, args) => {
            push_enum_instance(seen, out, name, args);
            for arg in args {
                collect_type_enum(arg, seen, out);
            }
        }
        ValueType::Array(element) => collect_type_enum(element, seen, out),
        ValueType::Never => {}
        _ => {}
    }
}

fn collect_expr_enum(
    expr: &ValueExpr,
    seen: &mut BTreeSet<String>,
    out: &mut Vec<(String, Vec<ValueType>)>,
) {
    match expr {
        ValueExpr::Binary { left, right, .. } | ValueExpr::StringCompare { left, right, .. } => {
            collect_expr_enum(left, seen, out);
            collect_expr_enum(right, seen, out);
        }
        ValueExpr::Call { args, .. } => {
            for arg in args {
                collect_expr_enum(arg, seen, out);
            }
        }
        ValueExpr::StringLen { value } | ValueExpr::Unary { expr: value, .. } => {
            collect_expr_enum(value, seen, out)
        }
        ValueExpr::StringConcat { left, right } => {
            collect_expr_enum(left, seen, out);
            collect_expr_enum(right, seen, out);
        }
        ValueExpr::FsReadToString { path } => {
            push_enum_instance(
                seen,
                out,
                "Result",
                &[
                    ValueType::String,
                    ValueType::Struct("FsError".to_string(), Vec::new()),
                ],
            );
            collect_expr_enum(path, seen, out);
        }
        ValueExpr::FsWriteString { path, content } => {
            push_enum_instance(
                seen,
                out,
                "Result",
                &[
                    ValueType::Void,
                    ValueType::Struct("FsError".to_string(), Vec::new()),
                ],
            );
            collect_expr_enum(path, seen, out);
            collect_expr_enum(content, seen, out);
        }
        ValueExpr::FsOpen { path } => {
            push_enum_instance(
                seen,
                out,
                "Result",
                &[
                    ValueType::Struct("File".to_string(), Vec::new()),
                    ValueType::Struct("FsError".to_string(), Vec::new()),
                ],
            );
            collect_expr_enum(path, seen, out);
        }
        ValueExpr::FileClose { file } => collect_expr_enum(file, seen, out),
        ValueExpr::ResultMapErr {
            result,
            ok_type,
            source_err_type,
            target_err_type,
            ..
        } => {
            push_enum_instance(
                seen,
                out,
                "Result",
                &[ok_type.clone(), source_err_type.clone()],
            );
            push_enum_instance(
                seen,
                out,
                "Result",
                &[ok_type.clone(), target_err_type.clone()],
            );
            collect_type_enum(ok_type, seen, out);
            collect_type_enum(source_err_type, seen, out);
            collect_type_enum(target_err_type, seen, out);
            collect_expr_enum(result, seen, out);
        }
        ValueExpr::EnvGet { name } => {
            push_enum_instance(seen, out, "Option", &[ValueType::String]);
            collect_expr_enum(name, seen, out);
        }
        ValueExpr::EnvArgs => {}
        ValueExpr::ArrayNew { .. } => {}
        ValueExpr::ArrayLen { array } => collect_expr_enum(array, seen, out),
        ValueExpr::ArrayGet {
            array,
            index,
            element_type,
        } => {
            push_enum_instance(seen, out, "Option", &[element_type.clone()]);
            collect_expr_enum(array, seen, out);
            collect_expr_enum(index, seen, out);
        }
        ValueExpr::ArrayPush { value, .. } => collect_expr_enum(value, seen, out),
        ValueExpr::ArraySet { index, value, .. } => {
            collect_expr_enum(index, seen, out);
            collect_expr_enum(value, seen, out);
        }
        ValueExpr::Cast { expr, .. } => collect_expr_enum(expr, seen, out),
        ValueExpr::StructLiteral { fields, .. } => {
            for (_, value) in fields {
                collect_expr_enum(value, seen, out);
            }
        }
        ValueExpr::EnumVariant {
            enum_name,
            enum_args,
            payload,
            ..
        } => {
            push_enum_instance(seen, out, enum_name, enum_args);
            if let Some(payload) = payload {
                collect_expr_enum(payload, seen, out);
            }
        }
        ValueExpr::EnumPayload { value, .. } => collect_expr_enum(value, seen, out),
        ValueExpr::EnumPayloadFieldAccess { value, .. } => collect_expr_enum(value, seen, out),
        ValueExpr::If {
            condition,
            then_branch,
            else_branch,
        } => {
            collect_expr_enum(condition, seen, out);
            collect_expr_enum(then_branch, seen, out);
            collect_expr_enum(else_branch, seen, out);
        }
        ValueExpr::Panic { message, .. } => collect_expr_enum(message, seen, out),
        ValueExpr::Match { value, arms } => {
            collect_expr_enum(value, seen, out);
            for arm in arms {
                push_enum_instance(seen, out, &arm.enum_name, &arm.enum_args);
                collect_expr_enum(&arm.value, seen, out);
            }
        }
        ValueExpr::StringLiteral(_)
        | ValueExpr::IntLiteral(_)
        | ValueExpr::FloatLiteral(_)
        | ValueExpr::CharLiteral(_)
        | ValueExpr::BoolLiteral(_)
        | ValueExpr::VoidLiteral
        | ValueExpr::Variable(_)
        | ValueExpr::MutBorrow(_)
        | ValueExpr::FieldAccess { .. } => {}
    }
}

fn uses_fs_read_to_string(program: &Program) -> bool {
    program.functions.iter().any(|function| {
        function
            .body
            .iter()
            .any(|statement| statement_uses_fs_read_to_string(statement))
    })
}

fn uses_fs_write_string(program: &Program) -> bool {
    program.functions.iter().any(|function| {
        function
            .body
            .iter()
            .any(|statement| statement_uses_fs_write_string(statement))
    })
}

fn uses_fs_open(program: &Program) -> bool {
    program.functions.iter().any(|function| {
        function
            .body
            .iter()
            .any(|statement| statement_uses_fs_open(statement))
    })
}

fn uses_env_get(program: &Program) -> bool {
    program.functions.iter().any(|function| {
        function
            .body
            .iter()
            .any(|statement| statement_uses_env_get(statement))
    })
}

fn uses_env_args(program: &Program) -> bool {
    program.functions.iter().any(|function| {
        function
            .body
            .iter()
            .any(|statement| statement_uses_env_args(statement))
    })
}

fn collect_array_element_types(program: &Program) -> Vec<ValueType> {
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for function in &program.functions {
        collect_type_array_elements(&function.return_type, &mut seen, &mut out);
        for param in &function.params {
            collect_type_array_elements(&param.value_type, &mut seen, &mut out);
        }
        for statement in &function.body {
            collect_statement_array_elements(statement, &mut seen, &mut out);
        }
    }
    out
}

fn collect_type_array_elements(
    value_type: &ValueType,
    seen: &mut BTreeSet<String>,
    out: &mut Vec<ValueType>,
) {
    match value_type {
        ValueType::Array(element) => {
            push_array_element_type(seen, out, element);
            collect_type_array_elements(element, seen, out);
        }
        ValueType::Enum(_, args) => {
            for arg in args {
                collect_type_array_elements(arg, seen, out);
            }
        }
        _ => {}
    }
}

fn push_array_element_type(
    seen: &mut BTreeSet<String>,
    out: &mut Vec<ValueType>,
    element_type: &ValueType,
) {
    if is_supported_array_element(element_type) {
        let key = c_type_name_part(element_type);
        if seen.insert(key) {
            out.push(element_type.clone());
        }
    }
}

fn statement_uses_fs_read_to_string(statement: &Statement) -> bool {
    match statement {
        Statement::Let { initializer, .. } => expr_uses_fs_read_to_string(initializer),
        Statement::LetIf {
            condition,
            body,
            else_body,
            ..
        } => {
            expr_uses_fs_read_to_string(condition)
                || body.iter().any(statement_uses_fs_read_to_string)
                || else_body.iter().any(statement_uses_fs_read_to_string)
        }
        Statement::LetMatch { value, arms, .. } => {
            expr_uses_fs_read_to_string(value)
                || arms
                    .iter()
                    .any(|arm| arm.body.iter().any(statement_uses_fs_read_to_string))
        }
        Statement::TryLet { result_expr, .. } => expr_uses_fs_read_to_string(result_expr),
        Statement::TryReturnOk { result_expr, .. } => expr_uses_fs_read_to_string(result_expr),
        Statement::LetElse {
            value, else_body, ..
        } => {
            expr_uses_fs_read_to_string(value)
                || else_body.iter().any(statement_uses_fs_read_to_string)
        }
        Statement::IfLet {
            value,
            body,
            else_body,
            ..
        } => {
            expr_uses_fs_read_to_string(value)
                || body.iter().any(statement_uses_fs_read_to_string)
                || else_body
                    .as_ref()
                    .is_some_and(|else_body| else_body.iter().any(statement_uses_fs_read_to_string))
        }
        Statement::If {
            condition,
            body,
            else_body,
        } => {
            expr_uses_fs_read_to_string(condition)
                || body.iter().any(statement_uses_fs_read_to_string)
                || else_body.iter().any(statement_uses_fs_read_to_string)
        }
        Statement::Assign { value, .. }
        | Statement::AssignField { value, .. }
        | Statement::Println(value)
        | Statement::Eprintln(value)
        | Statement::Panic(value)
        | Statement::Expr(value)
        | Statement::Return(Some(value)) => expr_uses_fs_read_to_string(value),
        Statement::Loop { kind, body } => match kind {
            LoopKind::Infinite => body.iter().any(statement_uses_fs_read_to_string),
            LoopKind::While(condition) => {
                expr_uses_fs_read_to_string(condition)
                    || body.iter().any(statement_uses_fs_read_to_string)
            }
            LoopKind::Iterate { iterable, .. } => {
                expr_uses_fs_read_to_string(iterable)
                    || body.iter().any(statement_uses_fs_read_to_string)
            }
        },
        Statement::Match { value, arms, .. } => {
            expr_uses_fs_read_to_string(value)
                || arms
                    .iter()
                    .any(|arm| arm.body.iter().any(statement_uses_fs_read_to_string))
        }
        Statement::Defer { call } => deferred_uses_fs_read_to_string(call),
        Statement::Break | Statement::Continue => false,
        Statement::Return(None) => false,
    }
}

fn statement_uses_fs_write_string(statement: &Statement) -> bool {
    match statement {
        Statement::Let { initializer, .. } => expr_uses_fs_write_string(initializer),
        Statement::LetIf {
            condition,
            body,
            else_body,
            ..
        } => {
            expr_uses_fs_write_string(condition)
                || body.iter().any(statement_uses_fs_write_string)
                || else_body.iter().any(statement_uses_fs_write_string)
        }
        Statement::LetMatch { value, arms, .. } => {
            expr_uses_fs_write_string(value)
                || arms
                    .iter()
                    .any(|arm| arm.body.iter().any(statement_uses_fs_write_string))
        }
        Statement::TryLet { result_expr, .. } => expr_uses_fs_write_string(result_expr),
        Statement::TryReturnOk { result_expr, .. } => expr_uses_fs_write_string(result_expr),
        Statement::LetElse {
            value, else_body, ..
        } => {
            expr_uses_fs_write_string(value) || else_body.iter().any(statement_uses_fs_write_string)
        }
        Statement::IfLet {
            value,
            body,
            else_body,
            ..
        } => {
            expr_uses_fs_write_string(value)
                || body.iter().any(statement_uses_fs_write_string)
                || else_body
                    .as_ref()
                    .is_some_and(|else_body| else_body.iter().any(statement_uses_fs_write_string))
        }
        Statement::If {
            condition,
            body,
            else_body,
        } => {
            expr_uses_fs_write_string(condition)
                || body.iter().any(statement_uses_fs_write_string)
                || else_body.iter().any(statement_uses_fs_write_string)
        }
        Statement::Assign { value, .. }
        | Statement::AssignField { value, .. }
        | Statement::Println(value)
        | Statement::Eprintln(value)
        | Statement::Panic(value)
        | Statement::Expr(value)
        | Statement::Return(Some(value)) => expr_uses_fs_write_string(value),
        Statement::Loop { kind, body } => match kind {
            LoopKind::Infinite => body.iter().any(statement_uses_fs_write_string),
            LoopKind::While(condition) => {
                expr_uses_fs_write_string(condition)
                    || body.iter().any(statement_uses_fs_write_string)
            }
            LoopKind::Iterate { iterable, .. } => {
                expr_uses_fs_write_string(iterable)
                    || body.iter().any(statement_uses_fs_write_string)
            }
        },
        Statement::Match { value, arms, .. } => {
            expr_uses_fs_write_string(value)
                || arms
                    .iter()
                    .any(|arm| arm.body.iter().any(statement_uses_fs_write_string))
        }
        Statement::Defer { call } => deferred_uses_fs_write_string(call),
        Statement::Break | Statement::Continue => false,
        Statement::Return(None) => false,
    }
}

fn statement_uses_fs_open(statement: &Statement) -> bool {
    match statement {
        Statement::Let { initializer, .. } => expr_uses_fs_open(initializer),
        Statement::LetIf {
            condition,
            body,
            else_body,
            ..
        } => {
            expr_uses_fs_open(condition)
                || body.iter().any(statement_uses_fs_open)
                || else_body.iter().any(statement_uses_fs_open)
        }
        Statement::LetMatch { value, arms, .. } => {
            expr_uses_fs_open(value)
                || arms
                    .iter()
                    .any(|arm| arm.body.iter().any(statement_uses_fs_open))
        }
        Statement::TryLet { result_expr, .. } => expr_uses_fs_open(result_expr),
        Statement::TryReturnOk { result_expr, .. } => expr_uses_fs_open(result_expr),
        Statement::LetElse {
            value, else_body, ..
        } => expr_uses_fs_open(value) || else_body.iter().any(statement_uses_fs_open),
        Statement::IfLet {
            value,
            body,
            else_body,
            ..
        } => {
            expr_uses_fs_open(value)
                || body.iter().any(statement_uses_fs_open)
                || else_body
                    .as_ref()
                    .is_some_and(|else_body| else_body.iter().any(statement_uses_fs_open))
        }
        Statement::If {
            condition,
            body,
            else_body,
        } => {
            expr_uses_fs_open(condition)
                || body.iter().any(statement_uses_fs_open)
                || else_body.iter().any(statement_uses_fs_open)
        }
        Statement::Assign { value, .. }
        | Statement::AssignField { value, .. }
        | Statement::Println(value)
        | Statement::Eprintln(value)
        | Statement::Panic(value)
        | Statement::Expr(value)
        | Statement::Return(Some(value)) => expr_uses_fs_open(value),
        Statement::Loop { kind, body } => match kind {
            LoopKind::Infinite => body.iter().any(statement_uses_fs_open),
            LoopKind::While(condition) => {
                expr_uses_fs_open(condition) || body.iter().any(statement_uses_fs_open)
            }
            LoopKind::Iterate { iterable, .. } => {
                expr_uses_fs_open(iterable) || body.iter().any(statement_uses_fs_open)
            }
        },
        Statement::Match { value, arms, .. } => {
            expr_uses_fs_open(value)
                || arms
                    .iter()
                    .any(|arm| arm.body.iter().any(statement_uses_fs_open))
        }
        Statement::Defer { call } => deferred_uses_fs_open(call),
        Statement::Break | Statement::Continue => false,
        Statement::Return(None) => false,
    }
}

fn statement_uses_env_get(statement: &Statement) -> bool {
    match statement {
        Statement::Let { initializer, .. } => expr_uses_env_get(initializer),
        Statement::LetIf {
            condition,
            body,
            else_body,
            ..
        } => {
            expr_uses_env_get(condition)
                || body.iter().any(statement_uses_env_get)
                || else_body.iter().any(statement_uses_env_get)
        }
        Statement::LetMatch { value, arms, .. } => {
            expr_uses_env_get(value)
                || arms
                    .iter()
                    .any(|arm| arm.body.iter().any(statement_uses_env_get))
        }
        Statement::TryLet { result_expr, .. } => expr_uses_env_get(result_expr),
        Statement::TryReturnOk { result_expr, .. } => expr_uses_env_get(result_expr),
        Statement::LetElse {
            value, else_body, ..
        } => expr_uses_env_get(value) || else_body.iter().any(statement_uses_env_get),
        Statement::IfLet {
            value,
            body,
            else_body,
            ..
        } => {
            expr_uses_env_get(value)
                || body.iter().any(statement_uses_env_get)
                || else_body
                    .as_ref()
                    .is_some_and(|else_body| else_body.iter().any(statement_uses_env_get))
        }
        Statement::If {
            condition,
            body,
            else_body,
        } => {
            expr_uses_env_get(condition)
                || body.iter().any(statement_uses_env_get)
                || else_body.iter().any(statement_uses_env_get)
        }
        Statement::Assign { value, .. }
        | Statement::AssignField { value, .. }
        | Statement::Println(value)
        | Statement::Eprintln(value)
        | Statement::Panic(value)
        | Statement::Expr(value)
        | Statement::Return(Some(value)) => expr_uses_env_get(value),
        Statement::Loop { kind, body } => match kind {
            LoopKind::Infinite => body.iter().any(statement_uses_env_get),
            LoopKind::While(condition) => {
                expr_uses_env_get(condition) || body.iter().any(statement_uses_env_get)
            }
            LoopKind::Iterate { iterable, .. } => {
                expr_uses_env_get(iterable) || body.iter().any(statement_uses_env_get)
            }
        },
        Statement::Match { value, arms, .. } => {
            expr_uses_env_get(value)
                || arms
                    .iter()
                    .any(|arm| arm.body.iter().any(statement_uses_env_get))
        }
        Statement::Defer { call } => deferred_uses_env_get(call),
        Statement::Break | Statement::Continue => false,
        Statement::Return(None) => false,
    }
}

fn statement_uses_env_args(statement: &Statement) -> bool {
    match statement {
        Statement::Let { initializer, .. } => expr_uses_env_args(initializer),
        Statement::LetIf {
            condition,
            body,
            else_body,
            ..
        } => {
            expr_uses_env_args(condition)
                || body.iter().any(statement_uses_env_args)
                || else_body.iter().any(statement_uses_env_args)
        }
        Statement::LetMatch { value, arms, .. } => {
            expr_uses_env_args(value)
                || arms
                    .iter()
                    .any(|arm| arm.body.iter().any(statement_uses_env_args))
        }
        Statement::TryLet { result_expr, .. } => expr_uses_env_args(result_expr),
        Statement::TryReturnOk { result_expr, .. } => expr_uses_env_args(result_expr),
        Statement::LetElse {
            value, else_body, ..
        } => expr_uses_env_args(value) || else_body.iter().any(statement_uses_env_args),
        Statement::IfLet {
            value,
            body,
            else_body,
            ..
        } => {
            expr_uses_env_args(value)
                || body.iter().any(statement_uses_env_args)
                || else_body
                    .as_ref()
                    .is_some_and(|else_body| else_body.iter().any(statement_uses_env_args))
        }
        Statement::If {
            condition,
            body,
            else_body,
        } => {
            expr_uses_env_args(condition)
                || body.iter().any(statement_uses_env_args)
                || else_body.iter().any(statement_uses_env_args)
        }
        Statement::Assign { value, .. }
        | Statement::AssignField { value, .. }
        | Statement::Println(value)
        | Statement::Eprintln(value)
        | Statement::Panic(value)
        | Statement::Expr(value)
        | Statement::Return(Some(value)) => expr_uses_env_args(value),
        Statement::Loop { kind, body } => match kind {
            LoopKind::Infinite => body.iter().any(statement_uses_env_args),
            LoopKind::While(condition) => {
                expr_uses_env_args(condition) || body.iter().any(statement_uses_env_args)
            }
            LoopKind::Iterate { iterable, .. } => {
                expr_uses_env_args(iterable) || body.iter().any(statement_uses_env_args)
            }
        },
        Statement::Match { value, arms, .. } => {
            expr_uses_env_args(value)
                || arms
                    .iter()
                    .any(|arm| arm.body.iter().any(statement_uses_env_args))
        }
        Statement::Defer { call } => deferred_uses_env_args(call),
        Statement::Break | Statement::Continue => false,
        Statement::Return(None) => false,
    }
}

fn collect_statement_array_elements(
    statement: &Statement,
    seen: &mut BTreeSet<String>,
    out: &mut Vec<ValueType>,
) {
    match statement {
        Statement::Let {
            value_type,
            initializer,
            ..
        } => {
            collect_type_array_elements(value_type, seen, out);
            collect_expr_array_elements(initializer, seen, out);
        }
        Statement::LetIf {
            value_type,
            condition,
            body,
            else_body,
            ..
        } => {
            collect_type_array_elements(value_type, seen, out);
            collect_expr_array_elements(condition, seen, out);
            for stmt in body {
                collect_statement_array_elements(stmt, seen, out);
            }
            for stmt in else_body {
                collect_statement_array_elements(stmt, seen, out);
            }
        }
        Statement::LetMatch {
            value_type,
            value,
            enum_args,
            arms,
            ..
        } => {
            collect_type_array_elements(value_type, seen, out);
            for arg in enum_args {
                collect_type_array_elements(arg, seen, out);
            }
            collect_expr_array_elements(value, seen, out);
            for arm in arms {
                for stmt in &arm.body {
                    collect_statement_array_elements(stmt, seen, out);
                }
            }
        }
        Statement::TryLet {
            value_type,
            result_type,
            return_type,
            result_expr,
            ..
        } => {
            collect_type_array_elements(value_type, seen, out);
            collect_type_array_elements(result_type, seen, out);
            collect_type_array_elements(return_type, seen, out);
            collect_expr_array_elements(result_expr, seen, out);
        }
        Statement::TryReturnOk {
            ok_type,
            result_type,
            return_type,
            result_expr,
        } => {
            collect_type_array_elements(ok_type, seen, out);
            collect_type_array_elements(result_type, seen, out);
            collect_type_array_elements(return_type, seen, out);
            collect_expr_array_elements(result_expr, seen, out);
        }
        Statement::LetElse {
            value_type,
            value,
            enum_args,
            else_body,
            ..
        } => {
            collect_type_array_elements(value_type, seen, out);
            for arg in enum_args {
                collect_type_array_elements(arg, seen, out);
            }
            collect_expr_array_elements(value, seen, out);
            for stmt in else_body {
                collect_statement_array_elements(stmt, seen, out);
            }
        }
        Statement::IfLet {
            value_type,
            value,
            enum_args,
            body,
            else_body,
            ..
        } => {
            if let Some(value_type) = value_type {
                collect_type_array_elements(value_type, seen, out);
            }
            for arg in enum_args {
                collect_type_array_elements(arg, seen, out);
            }
            collect_expr_array_elements(value, seen, out);
            for stmt in body {
                collect_statement_array_elements(stmt, seen, out);
            }
            if let Some(else_body) = else_body {
                for stmt in else_body {
                    collect_statement_array_elements(stmt, seen, out);
                }
            }
        }
        Statement::If {
            condition,
            body,
            else_body,
        } => {
            collect_expr_array_elements(condition, seen, out);
            for stmt in body {
                collect_statement_array_elements(stmt, seen, out);
            }
            for stmt in else_body {
                collect_statement_array_elements(stmt, seen, out);
            }
        }
        Statement::Assign { value, .. }
        | Statement::AssignField { value, .. }
        | Statement::Println(value)
        | Statement::Eprintln(value)
        | Statement::Panic(value)
        | Statement::Expr(value)
        | Statement::Return(Some(value)) => collect_expr_array_elements(value, seen, out),
        Statement::Loop { kind, body } => match kind {
            LoopKind::Infinite => {
                for stmt in body {
                    collect_statement_array_elements(stmt, seen, out);
                }
            }
            LoopKind::While(condition) => {
                collect_expr_array_elements(condition, seen, out);
                for stmt in body {
                    collect_statement_array_elements(stmt, seen, out);
                }
            }
            LoopKind::Iterate {
                element_type,
                iterable,
                ..
            } => {
                collect_type_array_elements(element_type, seen, out);
                collect_expr_array_elements(iterable, seen, out);
                for stmt in body {
                    collect_statement_array_elements(stmt, seen, out);
                }
            }
        },
        Statement::Match { value, arms, .. } => {
            collect_expr_array_elements(value, seen, out);
            for arm in arms {
                for stmt in &arm.body {
                    collect_statement_array_elements(stmt, seen, out);
                }
            }
        }
        Statement::Defer { call } => collect_deferred_array_elements(call, seen, out),
        Statement::Break | Statement::Continue => {}
        Statement::Return(None) => {}
    }
}

fn deferred_uses_fs_read_to_string(call: &DeferredCall) -> bool {
    match call {
        DeferredCall::Expr(expr) | DeferredCall::Println(expr) | DeferredCall::Eprintln(expr) => {
            expr_uses_fs_read_to_string(expr)
        }
    }
}

fn deferred_uses_fs_write_string(call: &DeferredCall) -> bool {
    match call {
        DeferredCall::Expr(expr) | DeferredCall::Println(expr) | DeferredCall::Eprintln(expr) => {
            expr_uses_fs_write_string(expr)
        }
    }
}

fn deferred_uses_fs_open(call: &DeferredCall) -> bool {
    match call {
        DeferredCall::Expr(expr) | DeferredCall::Println(expr) | DeferredCall::Eprintln(expr) => {
            expr_uses_fs_open(expr)
        }
    }
}

fn deferred_uses_env_get(call: &DeferredCall) -> bool {
    match call {
        DeferredCall::Expr(expr) | DeferredCall::Println(expr) | DeferredCall::Eprintln(expr) => {
            expr_uses_env_get(expr)
        }
    }
}

fn deferred_uses_env_args(call: &DeferredCall) -> bool {
    match call {
        DeferredCall::Expr(expr) | DeferredCall::Println(expr) | DeferredCall::Eprintln(expr) => {
            expr_uses_env_args(expr)
        }
    }
}

fn collect_deferred_array_elements(
    call: &DeferredCall,
    seen: &mut BTreeSet<String>,
    out: &mut Vec<ValueType>,
) {
    match call {
        DeferredCall::Expr(expr) | DeferredCall::Println(expr) | DeferredCall::Eprintln(expr) => {
            collect_expr_array_elements(expr, seen, out);
        }
    }
}

fn expr_uses_fs_read_to_string(expr: &ValueExpr) -> bool {
    match expr {
        ValueExpr::FsReadToString { .. } => true,
        ValueExpr::Binary { left, right, .. }
        | ValueExpr::StringCompare { left, right, .. }
        | ValueExpr::StringConcat { left, right } => {
            expr_uses_fs_read_to_string(left) || expr_uses_fs_read_to_string(right)
        }
        ValueExpr::FsWriteString { path, content } => {
            expr_uses_fs_read_to_string(path) || expr_uses_fs_read_to_string(content)
        }
        ValueExpr::FsOpen { path } | ValueExpr::FileClose { file: path } => {
            expr_uses_fs_read_to_string(path)
        }
        ValueExpr::ResultMapErr { result, .. } => expr_uses_fs_read_to_string(result),
        ValueExpr::EnvGet { name } => expr_uses_fs_read_to_string(name),
        ValueExpr::EnvArgs => false,
        ValueExpr::ArrayNew { .. } => false,
        ValueExpr::ArrayLen { array } => expr_uses_fs_read_to_string(array),
        ValueExpr::ArrayGet { array, index, .. } => {
            expr_uses_fs_read_to_string(array) || expr_uses_fs_read_to_string(index)
        }
        ValueExpr::ArrayPush { value, .. } => expr_uses_fs_read_to_string(value),
        ValueExpr::ArraySet { index, value, .. } => {
            expr_uses_fs_read_to_string(index) || expr_uses_fs_read_to_string(value)
        }
        ValueExpr::Call { args, .. } => args.iter().any(expr_uses_fs_read_to_string),
        ValueExpr::StringLen { value }
        | ValueExpr::Unary { expr: value, .. }
        | ValueExpr::Cast { expr: value, .. }
        | ValueExpr::EnumPayload { value, .. } => expr_uses_fs_read_to_string(value),
        ValueExpr::StructLiteral { fields, .. } => fields
            .iter()
            .any(|(_, value)| expr_uses_fs_read_to_string(value)),
        ValueExpr::EnumVariant { payload, .. } => {
            payload.as_deref().is_some_and(expr_uses_fs_read_to_string)
        }
        ValueExpr::If {
            condition,
            then_branch,
            else_branch,
        } => {
            expr_uses_fs_read_to_string(condition)
                || expr_uses_fs_read_to_string(then_branch)
                || expr_uses_fs_read_to_string(else_branch)
        }
        ValueExpr::Panic { message, .. } => expr_uses_fs_read_to_string(message),
        ValueExpr::Match { value, arms } => {
            expr_uses_fs_read_to_string(value)
                || arms
                    .iter()
                    .any(|arm| expr_uses_fs_read_to_string(&arm.value))
        }
        ValueExpr::StringLiteral(_)
        | ValueExpr::IntLiteral(_)
        | ValueExpr::FloatLiteral(_)
        | ValueExpr::CharLiteral(_)
        | ValueExpr::BoolLiteral(_)
        | ValueExpr::VoidLiteral
        | ValueExpr::Variable(_)
        | ValueExpr::MutBorrow(_)
        | ValueExpr::FieldAccess { .. } => false,
        ValueExpr::EnumPayloadFieldAccess { value, .. } => expr_uses_fs_read_to_string(value),
    }
}

fn expr_uses_fs_write_string(expr: &ValueExpr) -> bool {
    match expr {
        ValueExpr::FsWriteString { .. } => true,
        ValueExpr::Binary { left, right, .. }
        | ValueExpr::StringCompare { left, right, .. }
        | ValueExpr::StringConcat { left, right } => {
            expr_uses_fs_write_string(left) || expr_uses_fs_write_string(right)
        }
        ValueExpr::FsReadToString { path } => expr_uses_fs_write_string(path),
        ValueExpr::FsOpen { path } | ValueExpr::FileClose { file: path } => {
            expr_uses_fs_write_string(path)
        }
        ValueExpr::ResultMapErr { result, .. } => expr_uses_fs_write_string(result),
        ValueExpr::EnvGet { name } => expr_uses_fs_write_string(name),
        ValueExpr::EnvArgs => false,
        ValueExpr::ArrayNew { .. } => false,
        ValueExpr::ArrayLen { array } => expr_uses_fs_write_string(array),
        ValueExpr::ArrayGet { array, index, .. } => {
            expr_uses_fs_write_string(array) || expr_uses_fs_write_string(index)
        }
        ValueExpr::ArrayPush { value, .. } => expr_uses_fs_write_string(value),
        ValueExpr::ArraySet { index, value, .. } => {
            expr_uses_fs_write_string(index) || expr_uses_fs_write_string(value)
        }
        ValueExpr::Call { args, .. } => args.iter().any(expr_uses_fs_write_string),
        ValueExpr::StringLen { value }
        | ValueExpr::Unary { expr: value, .. }
        | ValueExpr::Cast { expr: value, .. }
        | ValueExpr::EnumPayload { value, .. } => expr_uses_fs_write_string(value),
        ValueExpr::StructLiteral { fields, .. } => fields
            .iter()
            .any(|(_, value)| expr_uses_fs_write_string(value)),
        ValueExpr::EnumVariant { payload, .. } => {
            payload.as_deref().is_some_and(expr_uses_fs_write_string)
        }
        ValueExpr::If {
            condition,
            then_branch,
            else_branch,
        } => {
            expr_uses_fs_write_string(condition)
                || expr_uses_fs_write_string(then_branch)
                || expr_uses_fs_write_string(else_branch)
        }
        ValueExpr::Panic { message, .. } => expr_uses_fs_write_string(message),
        ValueExpr::Match { value, arms } => {
            expr_uses_fs_write_string(value)
                || arms.iter().any(|arm| expr_uses_fs_write_string(&arm.value))
        }
        ValueExpr::StringLiteral(_)
        | ValueExpr::IntLiteral(_)
        | ValueExpr::FloatLiteral(_)
        | ValueExpr::CharLiteral(_)
        | ValueExpr::BoolLiteral(_)
        | ValueExpr::VoidLiteral
        | ValueExpr::Variable(_)
        | ValueExpr::MutBorrow(_)
        | ValueExpr::FieldAccess { .. } => false,
        ValueExpr::EnumPayloadFieldAccess { value, .. } => expr_uses_fs_write_string(value),
    }
}

fn expr_uses_fs_open(expr: &ValueExpr) -> bool {
    match expr {
        ValueExpr::FsOpen { .. } | ValueExpr::FileClose { .. } => true,
        ValueExpr::Binary { left, right, .. }
        | ValueExpr::StringCompare { left, right, .. }
        | ValueExpr::StringConcat { left, right } => {
            expr_uses_fs_open(left) || expr_uses_fs_open(right)
        }
        ValueExpr::FsReadToString { path }
        | ValueExpr::EnvGet { name: path }
        | ValueExpr::ArrayLen { array: path } => expr_uses_fs_open(path),
        ValueExpr::ResultMapErr { result, .. } => expr_uses_fs_open(result),
        ValueExpr::FsWriteString { path, content } => {
            expr_uses_fs_open(path) || expr_uses_fs_open(content)
        }
        ValueExpr::EnvArgs => false,
        ValueExpr::ArrayNew { .. } => false,
        ValueExpr::ArrayGet { array, index, .. } => {
            expr_uses_fs_open(array) || expr_uses_fs_open(index)
        }
        ValueExpr::ArrayPush { value, .. } => expr_uses_fs_open(value),
        ValueExpr::ArraySet { index, value, .. } => {
            expr_uses_fs_open(index) || expr_uses_fs_open(value)
        }
        ValueExpr::Call { args, .. } => args.iter().any(expr_uses_fs_open),
        ValueExpr::StringLen { value }
        | ValueExpr::Unary { expr: value, .. }
        | ValueExpr::Cast { expr: value, .. }
        | ValueExpr::EnumPayload { value, .. }
        | ValueExpr::EnumPayloadFieldAccess { value, .. } => expr_uses_fs_open(value),
        ValueExpr::StructLiteral { fields, .. } => {
            fields.iter().any(|(_, value)| expr_uses_fs_open(value))
        }
        ValueExpr::EnumVariant { payload, .. } => payload.as_deref().is_some_and(expr_uses_fs_open),
        ValueExpr::If {
            condition,
            then_branch,
            else_branch,
        } => {
            expr_uses_fs_open(condition)
                || expr_uses_fs_open(then_branch)
                || expr_uses_fs_open(else_branch)
        }
        ValueExpr::Panic { message, .. } => expr_uses_fs_open(message),
        ValueExpr::Match { value, arms } => {
            expr_uses_fs_open(value) || arms.iter().any(|arm| expr_uses_fs_open(&arm.value))
        }
        ValueExpr::StringLiteral(_)
        | ValueExpr::IntLiteral(_)
        | ValueExpr::FloatLiteral(_)
        | ValueExpr::CharLiteral(_)
        | ValueExpr::BoolLiteral(_)
        | ValueExpr::VoidLiteral
        | ValueExpr::Variable(_)
        | ValueExpr::MutBorrow(_)
        | ValueExpr::FieldAccess { .. } => false,
    }
}

fn expr_uses_env_get(expr: &ValueExpr) -> bool {
    match expr {
        ValueExpr::EnvGet { .. } => true,
        ValueExpr::Binary { left, right, .. }
        | ValueExpr::StringCompare { left, right, .. }
        | ValueExpr::StringConcat { left, right } => {
            expr_uses_env_get(left) || expr_uses_env_get(right)
        }
        ValueExpr::FsReadToString { path } => expr_uses_env_get(path),
        ValueExpr::FsWriteString { path, content } => {
            expr_uses_env_get(path) || expr_uses_env_get(content)
        }
        ValueExpr::FsOpen { path } | ValueExpr::FileClose { file: path } => expr_uses_env_get(path),
        ValueExpr::ResultMapErr { result, .. } => expr_uses_env_get(result),
        ValueExpr::EnvArgs => false,
        ValueExpr::ArrayNew { .. } => false,
        ValueExpr::ArrayLen { array } => expr_uses_env_get(array),
        ValueExpr::ArrayGet { array, index, .. } => {
            expr_uses_env_get(array) || expr_uses_env_get(index)
        }
        ValueExpr::ArrayPush { value, .. } => expr_uses_env_get(value),
        ValueExpr::ArraySet { index, value, .. } => {
            expr_uses_env_get(index) || expr_uses_env_get(value)
        }
        ValueExpr::Call { args, .. } => args.iter().any(expr_uses_env_get),
        ValueExpr::StringLen { value }
        | ValueExpr::Unary { expr: value, .. }
        | ValueExpr::Cast { expr: value, .. }
        | ValueExpr::EnumPayload { value, .. }
        | ValueExpr::EnumPayloadFieldAccess { value, .. } => expr_uses_env_get(value),
        ValueExpr::StructLiteral { fields, .. } => {
            fields.iter().any(|(_, value)| expr_uses_env_get(value))
        }
        ValueExpr::EnumVariant { payload, .. } => payload.as_deref().is_some_and(expr_uses_env_get),
        ValueExpr::If {
            condition,
            then_branch,
            else_branch,
        } => {
            expr_uses_env_get(condition)
                || expr_uses_env_get(then_branch)
                || expr_uses_env_get(else_branch)
        }
        ValueExpr::Panic { message, .. } => expr_uses_env_get(message),
        ValueExpr::Match { value, arms } => {
            expr_uses_env_get(value) || arms.iter().any(|arm| expr_uses_env_get(&arm.value))
        }
        ValueExpr::StringLiteral(_)
        | ValueExpr::IntLiteral(_)
        | ValueExpr::FloatLiteral(_)
        | ValueExpr::CharLiteral(_)
        | ValueExpr::BoolLiteral(_)
        | ValueExpr::VoidLiteral
        | ValueExpr::Variable(_)
        | ValueExpr::MutBorrow(_)
        | ValueExpr::FieldAccess { .. } => false,
    }
}

fn expr_uses_env_args(expr: &ValueExpr) -> bool {
    match expr {
        ValueExpr::EnvArgs => true,
        ValueExpr::Binary { left, right, .. }
        | ValueExpr::StringCompare { left, right, .. }
        | ValueExpr::StringConcat { left, right } => {
            expr_uses_env_args(left) || expr_uses_env_args(right)
        }
        ValueExpr::FsReadToString { path }
        | ValueExpr::FsOpen { path }
        | ValueExpr::FileClose { file: path }
        | ValueExpr::EnvGet { name: path }
        | ValueExpr::ArrayLen { array: path } => expr_uses_env_args(path),
        ValueExpr::ResultMapErr { result, .. } => expr_uses_env_args(result),
        ValueExpr::FsWriteString { path, content } => {
            expr_uses_env_args(path) || expr_uses_env_args(content)
        }
        ValueExpr::ArrayGet { array, index, .. } => {
            expr_uses_env_args(array) || expr_uses_env_args(index)
        }
        ValueExpr::ArrayPush { value, .. } => expr_uses_env_args(value),
        ValueExpr::ArraySet { index, value, .. } => {
            expr_uses_env_args(index) || expr_uses_env_args(value)
        }
        ValueExpr::Call { args, .. } => args.iter().any(expr_uses_env_args),
        ValueExpr::StringLen { value }
        | ValueExpr::Unary { expr: value, .. }
        | ValueExpr::Cast { expr: value, .. }
        | ValueExpr::EnumPayload { value, .. }
        | ValueExpr::EnumPayloadFieldAccess { value, .. } => expr_uses_env_args(value),
        ValueExpr::StructLiteral { fields, .. } => {
            fields.iter().any(|(_, value)| expr_uses_env_args(value))
        }
        ValueExpr::EnumVariant { payload, .. } => {
            payload.as_deref().is_some_and(expr_uses_env_args)
        }
        ValueExpr::If {
            condition,
            then_branch,
            else_branch,
        } => {
            expr_uses_env_args(condition)
                || expr_uses_env_args(then_branch)
                || expr_uses_env_args(else_branch)
        }
        ValueExpr::Panic { message, .. } => expr_uses_env_args(message),
        ValueExpr::Match { value, arms } => {
            expr_uses_env_args(value) || arms.iter().any(|arm| expr_uses_env_args(&arm.value))
        }
        ValueExpr::ArrayNew { .. }
        | ValueExpr::StringLiteral(_)
        | ValueExpr::IntLiteral(_)
        | ValueExpr::FloatLiteral(_)
        | ValueExpr::CharLiteral(_)
        | ValueExpr::BoolLiteral(_)
        | ValueExpr::VoidLiteral
        | ValueExpr::Variable(_)
        | ValueExpr::MutBorrow(_)
        | ValueExpr::FieldAccess { .. } => false,
    }
}

fn collect_expr_array_elements(
    expr: &ValueExpr,
    seen: &mut BTreeSet<String>,
    out: &mut Vec<ValueType>,
) {
    match expr {
        ValueExpr::EnvArgs => push_array_element_type(seen, out, &ValueType::String),
        ValueExpr::ArrayNew { element_type }
        | ValueExpr::ArrayGet { element_type, .. }
        | ValueExpr::ArrayPush { element_type, .. }
        | ValueExpr::ArraySet { element_type, .. } => {
            push_array_element_type(seen, out, element_type);
        }
        ValueExpr::ArrayLen { array } => collect_expr_array_elements(array, seen, out),
        ValueExpr::Binary { left, right, .. }
        | ValueExpr::StringCompare { left, right, .. }
        | ValueExpr::StringConcat { left, right } => {
            collect_expr_array_elements(left, seen, out);
            collect_expr_array_elements(right, seen, out);
        }
        ValueExpr::FsReadToString { path } | ValueExpr::EnvGet { name: path } => {
            collect_expr_array_elements(path, seen, out);
        }
        ValueExpr::FsOpen { path } | ValueExpr::FileClose { file: path } => {
            collect_expr_array_elements(path, seen, out);
        }
        ValueExpr::ResultMapErr {
            result,
            ok_type,
            source_err_type,
            target_err_type,
            ..
        } => {
            collect_type_array_elements(ok_type, seen, out);
            collect_type_array_elements(source_err_type, seen, out);
            collect_type_array_elements(target_err_type, seen, out);
            collect_expr_array_elements(result, seen, out);
        }
        ValueExpr::FsWriteString { path, content } => {
            collect_expr_array_elements(path, seen, out);
            collect_expr_array_elements(content, seen, out);
        }
        ValueExpr::Call { args, .. } => {
            for arg in args {
                collect_expr_array_elements(arg, seen, out);
            }
        }
        ValueExpr::StringLen { value }
        | ValueExpr::Unary { expr: value, .. }
        | ValueExpr::Cast { expr: value, .. }
        | ValueExpr::EnumPayload { value, .. }
        | ValueExpr::EnumPayloadFieldAccess { value, .. } => {
            collect_expr_array_elements(value, seen, out);
        }
        ValueExpr::StructLiteral { fields, .. } => {
            for (_, value) in fields {
                collect_expr_array_elements(value, seen, out);
            }
        }
        ValueExpr::EnumVariant { payload, .. } => {
            if let Some(payload) = payload {
                collect_expr_array_elements(payload, seen, out);
            }
        }
        ValueExpr::If {
            condition,
            then_branch,
            else_branch,
        } => {
            collect_expr_array_elements(condition, seen, out);
            collect_expr_array_elements(then_branch, seen, out);
            collect_expr_array_elements(else_branch, seen, out);
        }
        ValueExpr::Panic { message, .. } => collect_expr_array_elements(message, seen, out),
        ValueExpr::Match { value, arms } => {
            collect_expr_array_elements(value, seen, out);
            for arm in arms {
                collect_expr_array_elements(&arm.value, seen, out);
            }
        }
        ValueExpr::StringLiteral(_)
        | ValueExpr::IntLiteral(_)
        | ValueExpr::FloatLiteral(_)
        | ValueExpr::CharLiteral(_)
        | ValueExpr::BoolLiteral(_)
        | ValueExpr::VoidLiteral
        | ValueExpr::Variable(_)
        | ValueExpr::MutBorrow(_)
        | ValueExpr::FieldAccess { .. } => {}
    }
}

fn push_enum_instance(
    seen: &mut BTreeSet<String>,
    out: &mut Vec<(String, Vec<ValueType>)>,
    name: &str,
    args: &[ValueType],
) {
    let key = format!("{name}{}", c_type_suffix(args));
    if seen.insert(key) {
        out.push((name.to_string(), args.to_vec()));
    }
}

fn push_struct_instance(
    seen: &mut BTreeSet<String>,
    out: &mut Vec<(String, Vec<ValueType>)>,
    name: &str,
    args: &[ValueType],
) {
    let key = format!("{name}{}", c_type_suffix(args));
    if seen.insert(key) {
        out.push((name.to_string(), args.to_vec()));
    }
}

fn subst_type(value_type: &ValueType, type_params: &[String], args: &[ValueType]) -> ValueType {
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

fn emit_string_data_expr(out: &mut String, expr: &ValueExpr) {
    out.push('(');
    emit_expr(out, expr);
    out.push_str(").data");
}

fn c_binary_op(op: &BinaryOp) -> &'static str {
    match op {
        BinaryOp::LogicalOr => "||",
        BinaryOp::LogicalAnd => "&&",
        BinaryOp::Add => "+",
        BinaryOp::Subtract => "-",
        BinaryOp::Multiply => "*",
        BinaryOp::Divide => "/",
        BinaryOp::Remainder => "%",
        BinaryOp::Equal => "==",
        BinaryOp::NotEqual => "!=",
        BinaryOp::Less => "<",
        BinaryOp::LessEqual => "<=",
        BinaryOp::Greater => ">",
        BinaryOp::GreaterEqual => ">=",
    }
}

fn c_unary_op(op: &UnaryOp) -> &'static str {
    match op {
        UnaryOp::Not => "!",
    }
}

fn c_type(value_type: &ValueType) -> String {
    match value_type {
        ValueType::String => "nomo_string".to_string(),
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

fn result_void_error(value_type: &ValueType) -> Option<Vec<ValueType>> {
    let ValueType::Enum(name, args) = value_type else {
        return None;
    };
    if name == "Result" && args.len() == 2 && args[0] == ValueType::Void {
        Some(args.clone())
    } else {
        None
    }
}

fn c_payload_type(value_type: &ValueType) -> String {
    if value_type == &ValueType::Void {
        "char".to_string()
    } else {
        c_type(value_type)
    }
}

fn c_zero_value(value_type: &ValueType) -> String {
    match value_type {
        ValueType::String => "nomo_string_literal(\"\")".to_string(),
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

fn c_type_suffix(args: &[ValueType]) -> String {
    if args.is_empty() {
        return String::new();
    }
    let parts = args.iter().map(c_type_name_part).collect::<Vec<_>>();
    format!("_{}", parts.join("_"))
}

fn c_type_name_part(value_type: &ValueType) -> String {
    match value_type {
        ValueType::String => "string".to_string(),
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

fn c_var_ident(name: &str) -> String {
    format!("nomo_{name}")
}

fn c_member_ident(name: &str) -> String {
    format!("nomo_member_{name}")
}

fn c_payload_ident(variant: &str) -> String {
    format!("nomo_payload_{variant}")
}

fn c_fn_ident(name: &str) -> String {
    format!("nomo_fn_{name}")
}

fn c_package_ident(package: &str) -> String {
    package
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
        .collect()
}

fn c_struct_ident(name: &str, args: &[ValueType]) -> String {
    format!("nomo_struct_{}{}", name, c_type_suffix(args))
}

fn c_array_ident(element_type: &ValueType) -> String {
    format!("nomo_array_{}", c_type_name_part(element_type))
}

fn c_enum_ident(name: &str, args: &[ValueType]) -> String {
    format!("nomo_enum_{}{}", name, c_type_suffix(args))
}

fn c_enum_tag_ident(name: &str, args: &[ValueType]) -> String {
    format!("nomo_enum_{}{}_tag", name, c_type_suffix(args))
}

fn c_enum_variant_ident(enum_name: &str, args: &[ValueType], variant: &str) -> String {
    format!("nomo_enum_{}{}_{}", enum_name, c_type_suffix(args), variant)
}

fn c_result_map_err_helper_ident(instance: &ResultMapErrInstance) -> String {
    format!(
        "nomo_result_map_err_{}_{}_{}_{}",
        c_type_name_part(&instance.ok_type),
        c_type_name_part(&instance.source_err_type),
        c_type_name_part(&instance.target_err_type),
        instance.converter
    )
}

fn c_retain_ident(value_type: &ValueType) -> String {
    match value_type {
        ValueType::String => "nomo_string_retain".to_string(),
        ValueType::Array(element_type) => format!("{}_retain", c_array_ident(element_type)),
        ValueType::Struct(name, args) => format!("{}_retain", c_struct_ident(name, args)),
        ValueType::Enum(name, args) => format!("{}_retain", c_enum_ident(name, args)),
        _ => panic!(
            "unsupported retain helper requested for C type: {}",
            value_type.name()
        ),
    }
}

fn c_release_ident(value_type: &ValueType) -> String {
    match value_type {
        ValueType::String => "nomo_string_release".to_string(),
        ValueType::Array(element_type) => format!("{}_release", c_array_ident(element_type)),
        ValueType::Struct(name, args) => format!("{}_release", c_struct_ident(name, args)),
        ValueType::Enum(name, args) => format!("{}_release", c_enum_ident(name, args)),
        _ => panic!(
            "unsupported release helper requested for C type: {}",
            value_type.name()
        ),
    }
}

fn escape_c_string(value: &str) -> String {
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

fn is_supported_array_element(value_type: &ValueType) -> bool {
    !matches!(
        value_type,
        ValueType::Void | ValueType::Never | ValueType::TypeParam(_)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler::{EnumVariantType, MatchValueArm, Parameter, StructField, ValueExpr};

    fn string_literal(value: &str) -> String {
        format!("nomo_string_literal(\"{value}\")")
    }

    fn puts_literal(value: &str) -> String {
        format!("puts(({}).data);", string_literal(value))
    }

    fn fputs_literal(value: &str) -> String {
        format!("fputs(({}).data, stderr);", string_literal(value))
    }

    fn panic_literal(value: &str) -> String {
        format!("nomo_panic(({}).data);", string_literal(value))
    }

    #[test]
    fn emits_puts_for_println() {
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: vec!["std.io".to_string()],
            structs: Vec::new(),
            enums: Vec::new(),
            functions: vec![Function {
                package: "app.main".to_string(),
                name: "main".to_string(),
                params: Vec::new(),
                return_type: ValueType::Void,
                body: vec![Statement::Println(ValueExpr::StringLiteral(
                    "Hello".to_string(),
                ))],
            }],
        };

        let c = emit_c(&program);
        assert!(c.contains("#include <stdio.h>"));
        assert!(c.contains(&puts_literal("Hello")));
    }

    #[test]
    fn emits_package_prefixed_function_symbol_macros() {
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: Vec::new(),
            structs: Vec::new(),
            enums: Vec::new(),
            functions: vec![
                Function {
                    package: "app.main".to_string(),
                    name: "add".to_string(),
                    params: vec![
                        Parameter {
                            name: "a".to_string(),
                            value_type: ValueType::I32,
                            mutable: false,
                        },
                        Parameter {
                            name: "b".to_string(),
                            value_type: ValueType::I32,
                            mutable: false,
                        },
                    ],
                    return_type: ValueType::I32,
                    body: vec![Statement::Return(Some(ValueExpr::Binary {
                        op: BinaryOp::Add,
                        left: Box::new(ValueExpr::Variable("a".to_string())),
                        right: Box::new(ValueExpr::Variable("b".to_string())),
                    }))],
                },
                Function {
                    package: "app.main".to_string(),
                    name: "main".to_string(),
                    params: Vec::new(),
                    return_type: ValueType::Void,
                    body: vec![Statement::Expr(ValueExpr::Call {
                        name: "add".to_string(),
                        args: vec![ValueExpr::IntLiteral(1), ValueExpr::IntLiteral(2)],
                    })],
                },
            ],
        };

        let c = emit_c(&program);

        assert!(c.contains("#define nomo_fn_add nomo_pkg_app_main_fn_add"));
        assert!(c.contains("#define nomo_fn_main nomo_pkg_app_main_fn_main"));
        assert!(c.contains("int32_t nomo_fn_add(int32_t nomo_a, int32_t nomo_b);"));
        assert!(c.contains("nomo_fn_add(1, 2);"));
    }

    #[test]
    fn emits_package_prefixed_type_symbol_macros() {
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: Vec::new(),
            structs: vec![StructType {
                package: "app.main".to_string(),
                name: "Point".to_string(),
                type_params: Vec::new(),
                fields: vec![StructField {
                    name: "x".to_string(),
                    value_type: ValueType::I32,
                }],
            }],
            enums: vec![EnumType {
                package: "app.main".to_string(),
                name: "Color".to_string(),
                type_params: Vec::new(),
                variants: vec![
                    EnumVariantType {
                        name: "Red".to_string(),
                        payload: None,
                    },
                    EnumVariantType {
                        name: "Blue".to_string(),
                        payload: None,
                    },
                ],
            }],
            functions: vec![Function {
                package: "app.main".to_string(),
                name: "main".to_string(),
                params: Vec::new(),
                return_type: ValueType::Void,
                body: vec![
                    Statement::Let {
                        name: "point".to_string(),
                        value_type: ValueType::Struct("Point".to_string(), Vec::new()),
                        initializer: ValueExpr::StructLiteral {
                            type_name: "Point".to_string(),
                            struct_args: Vec::new(),
                            fields: vec![("x".to_string(), ValueExpr::IntLiteral(1))],
                        },
                    },
                    Statement::Let {
                        name: "color".to_string(),
                        value_type: ValueType::Enum("Color".to_string(), Vec::new()),
                        initializer: ValueExpr::EnumVariant {
                            enum_name: "Color".to_string(),
                            enum_args: Vec::new(),
                            variant: "Red".to_string(),
                            payload: None,
                        },
                    },
                ],
            }],
        };

        let c = emit_c(&program);

        assert!(c.contains("#define nomo_struct_Point nomo_pkg_app_main_struct_Point"));
        assert!(c.contains("#define nomo_enum_Color_tag nomo_pkg_app_main_enum_Color_tag"));
        assert!(c.contains("#define nomo_enum_Color nomo_pkg_app_main_enum_Color"));
        assert!(c.contains("#define nomo_enum_Color_Red nomo_pkg_app_main_enum_Color_Red"));
        assert!(c.contains("#define nomo_enum_Color_Blue nomo_pkg_app_main_enum_Color_Blue"));
    }

    #[test]
    fn emits_fputs_for_eprintln() {
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: vec!["std.io".to_string()],
            structs: Vec::new(),
            enums: Vec::new(),
            functions: vec![Function {
                package: "app.main".to_string(),
                name: "main".to_string(),
                params: Vec::new(),
                return_type: ValueType::Void,
                body: vec![Statement::Eprintln(ValueExpr::StringLiteral(
                    "error".to_string(),
                ))],
            }],
        };

        let c = emit_c(&program);
        assert!(c.contains(&fputs_literal("error")));
        assert!(c.contains("fputc('\\n', stderr);"));
    }

    #[test]
    fn emits_function_and_call() {
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: vec!["std.io".to_string()],
            structs: Vec::new(),
            enums: Vec::new(),
            functions: vec![
                Function {
                    package: "app.main".to_string(),
                    name: "add".to_string(),
                    params: vec![
                        Parameter {
                            name: "a".to_string(),
                            mutable: false,
                            value_type: ValueType::Int,
                        },
                        Parameter {
                            name: "b".to_string(),
                            mutable: false,
                            value_type: ValueType::Int,
                        },
                    ],
                    return_type: ValueType::Int,
                    body: vec![Statement::Return(Some(ValueExpr::Binary {
                        left: Box::new(ValueExpr::Variable("a".to_string())),
                        op: BinaryOp::Add,
                        right: Box::new(ValueExpr::Variable("b".to_string())),
                    }))],
                },
                Function {
                    package: "app.main".to_string(),
                    name: "main".to_string(),
                    params: Vec::new(),
                    return_type: ValueType::Void,
                    body: vec![
                        Statement::Let {
                            name: "answer".to_string(),
                            value_type: ValueType::Int,
                            initializer: ValueExpr::Call {
                                name: "add".to_string(),
                                args: vec![ValueExpr::IntLiteral(40), ValueExpr::IntLiteral(2)],
                            },
                        },
                        Statement::Println(ValueExpr::StringLiteral("done".to_string())),
                    ],
                },
            ],
        };

        let c = emit_c(&program);
        assert!(c.contains("long long nomo_fn_add(long long nomo_a, long long nomo_b);"));
        assert!(c.contains("long long nomo_fn_add(long long nomo_a, long long nomo_b)"));
        assert!(c.contains("return (nomo_a + nomo_b);"));
        assert!(c.contains("long long nomo_answer = nomo_fn_add(40, 2);"));
    }

    #[test]
    fn emits_mut_parameter_as_pointer_borrow() {
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: Vec::new(),
            structs: Vec::new(),
            enums: Vec::new(),
            functions: vec![
                Function {
                    package: "app.main".to_string(),
                    name: "bump".to_string(),
                    params: vec![Parameter {
                        name: "value".to_string(),
                        mutable: true,
                        value_type: ValueType::Int,
                    }],
                    return_type: ValueType::Void,
                    body: vec![Statement::Assign {
                        name: "value".to_string(),
                        value: ValueExpr::Binary {
                            left: Box::new(ValueExpr::Variable("value".to_string())),
                            op: BinaryOp::Add,
                            right: Box::new(ValueExpr::IntLiteral(1)),
                        },
                    }],
                },
                Function {
                    package: "app.main".to_string(),
                    name: "main".to_string(),
                    params: Vec::new(),
                    return_type: ValueType::Void,
                    body: vec![
                        Statement::Let {
                            name: "count".to_string(),
                            value_type: ValueType::Int,
                            initializer: ValueExpr::IntLiteral(1),
                        },
                        Statement::Expr(ValueExpr::Call {
                            name: "bump".to_string(),
                            args: vec![ValueExpr::MutBorrow(vec!["count".to_string()])],
                        }),
                    ],
                },
            ],
        };

        let c = emit_c(&program);
        assert!(c.contains("void nomo_fn_bump(long long * nomo_value);"));
        assert!(c.contains("#define nomo_value (*nomo_value)"));
        assert!(c.contains("nomo_value = (nomo_value + 1);"));
        assert!(c.contains("#undef nomo_value"));
        assert!(c.contains("nomo_fn_bump(&nomo_count);"));
    }

    #[test]
    fn emits_mut_field_path_as_pointer_borrow() {
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: Vec::new(),
            structs: vec![StructType {
                package: "app.main".to_string(),
                name: "Point".to_string(),
                type_params: Vec::new(),
                fields: vec![
                    StructField {
                        name: "x".to_string(),
                        value_type: ValueType::I32,
                    },
                    StructField {
                        name: "y".to_string(),
                        value_type: ValueType::I32,
                    },
                ],
            }],
            enums: Vec::new(),
            functions: vec![
                Function {
                    package: "app.main".to_string(),
                    name: "bump".to_string(),
                    params: vec![Parameter {
                        name: "value".to_string(),
                        mutable: true,
                        value_type: ValueType::I32,
                    }],
                    return_type: ValueType::Void,
                    body: Vec::new(),
                },
                Function {
                    package: "app.main".to_string(),
                    name: "main".to_string(),
                    params: Vec::new(),
                    return_type: ValueType::Void,
                    body: vec![
                        Statement::Let {
                            name: "point".to_string(),
                            value_type: ValueType::Struct("Point".to_string(), Vec::new()),
                            initializer: ValueExpr::StructLiteral {
                                type_name: "Point".to_string(),
                                struct_args: Vec::new(),
                                fields: vec![
                                    ("x".to_string(), ValueExpr::IntLiteral(1)),
                                    ("y".to_string(), ValueExpr::IntLiteral(2)),
                                ],
                            },
                        },
                        Statement::Expr(ValueExpr::Call {
                            name: "bump".to_string(),
                            args: vec![ValueExpr::MutBorrow(vec![
                                "point".to_string(),
                                "x".to_string(),
                            ])],
                        }),
                    ],
                },
            ],
        };

        let c = emit_c(&program);
        assert!(c.contains("nomo_fn_bump(&nomo_point.nomo_member_x);"));
    }

    #[test]
    fn emits_float_literal_and_cast() {
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: vec!["std.io".to_string()],
            structs: Vec::new(),
            enums: Vec::new(),
            functions: vec![
                Function {
                    package: "app.main".to_string(),
                    name: "ratio".to_string(),
                    params: vec![Parameter {
                        name: "age".to_string(),
                        mutable: false,
                        value_type: ValueType::Int,
                    }],
                    return_type: ValueType::Float,
                    body: vec![Statement::Return(Some(ValueExpr::Cast {
                        expr: Box::new(ValueExpr::Variable("age".to_string())),
                        target_type: ValueType::Float,
                    }))],
                },
                Function {
                    package: "app.main".to_string(),
                    name: "main".to_string(),
                    params: Vec::new(),
                    return_type: ValueType::Void,
                    body: vec![
                        Statement::Let {
                            name: "pi".to_string(),
                            value_type: ValueType::Float,
                            initializer: ValueExpr::FloatLiteral("3.14".to_string()),
                        },
                        Statement::Println(ValueExpr::StringLiteral("done".to_string())),
                    ],
                },
            ],
        };

        let c = emit_c(&program);
        assert!(c.contains("double nomo_fn_ratio(long long nomo_age);"));
        assert!(c.contains("return ((double)nomo_age);"));
        assert!(c.contains("double nomo_pi = 3.14;"));
    }

    #[test]
    fn emits_char_literal() {
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: vec!["std.io".to_string()],
            structs: Vec::new(),
            enums: Vec::new(),
            functions: vec![
                Function {
                    package: "app.main".to_string(),
                    name: "initial".to_string(),
                    params: Vec::new(),
                    return_type: ValueType::Char,
                    body: vec![Statement::Return(Some(ValueExpr::CharLiteral('語')))],
                },
                Function {
                    package: "app.main".to_string(),
                    name: "main".to_string(),
                    params: Vec::new(),
                    return_type: ValueType::Void,
                    body: vec![
                        Statement::Let {
                            name: "letter".to_string(),
                            value_type: ValueType::Char,
                            initializer: ValueExpr::Call {
                                name: "initial".to_string(),
                                args: Vec::new(),
                            },
                        },
                        Statement::Println(ValueExpr::StringLiteral("done".to_string())),
                    ],
                },
            ],
        };

        let c = emit_c(&program);
        assert!(c.contains("uint32_t nomo_fn_initial(void);"));
        assert!(c.contains("return 35486;"));
        assert!(c.contains("uint32_t nomo_letter = nomo_fn_initial();"));
    }

    #[test]
    fn emits_fixed_width_integer_types() {
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: vec!["std.io".to_string()],
            structs: Vec::new(),
            enums: Vec::new(),
            functions: vec![
                Function {
                    package: "app.main".to_string(),
                    name: "add32".to_string(),
                    params: vec![
                        Parameter {
                            name: "a".to_string(),
                            mutable: false,
                            value_type: ValueType::I32,
                        },
                        Parameter {
                            name: "b".to_string(),
                            mutable: false,
                            value_type: ValueType::I32,
                        },
                    ],
                    return_type: ValueType::I32,
                    body: vec![Statement::Return(Some(ValueExpr::Binary {
                        left: Box::new(ValueExpr::Variable("a".to_string())),
                        op: BinaryOp::Add,
                        right: Box::new(ValueExpr::Variable("b".to_string())),
                    }))],
                },
                Function {
                    package: "app.main".to_string(),
                    name: "main".to_string(),
                    params: Vec::new(),
                    return_type: ValueType::Void,
                    body: vec![
                        Statement::Let {
                            name: "signed".to_string(),
                            value_type: ValueType::I32,
                            initializer: ValueExpr::IntLiteral(1),
                        },
                        Statement::Let {
                            name: "word".to_string(),
                            value_type: ValueType::U32,
                            initializer: ValueExpr::IntLiteral(2),
                        },
                        Statement::Let {
                            name: "wide".to_string(),
                            value_type: ValueType::U64,
                            initializer: ValueExpr::IntLiteral(3),
                        },
                        Statement::Println(ValueExpr::StringLiteral("done".to_string())),
                    ],
                },
            ],
        };

        let c = emit_c(&program);
        assert!(c.contains("#include <stdint.h>"));
        assert!(c.contains("int32_t nomo_fn_add32(int32_t nomo_a, int32_t nomo_b);"));
        assert!(c.contains("int32_t nomo_signed = 1;"));
        assert!(c.contains("uint32_t nomo_word = 2;"));
        assert!(c.contains("uint64_t nomo_wide = 3;"));
    }

    #[test]
    fn emits_string_len_and_concat() {
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: vec!["std.io".to_string(), "std.string".to_string()],
            structs: Vec::new(),
            enums: Vec::new(),
            functions: vec![Function {
                package: "app.main".to_string(),
                name: "main".to_string(),
                params: Vec::new(),
                return_type: ValueType::Void,
                body: vec![
                    Statement::Let {
                        name: "message".to_string(),
                        value_type: ValueType::String,
                        initializer: ValueExpr::StringConcat {
                            left: Box::new(ValueExpr::StringLiteral("No".to_string())),
                            right: Box::new(ValueExpr::StringLiteral("mo".to_string())),
                        },
                    },
                    Statement::Let {
                        name: "count".to_string(),
                        value_type: ValueType::U64,
                        initializer: ValueExpr::StringLen {
                            value: Box::new(ValueExpr::Variable("message".to_string())),
                        },
                    },
                ],
            }],
        };

        let c = emit_c(&program);
        assert!(c.contains("#include <string.h>"));
        assert!(c.contains("static nomo_string nomo_string_concat"));
        assert!(c.contains(
            "nomo_string nomo_message = nomo_string_concat(nomo_string_literal(\"No\"), nomo_string_literal(\"mo\"));"
        ));
        assert!(c.contains("uint64_t nomo_count = ((uint64_t)strlen((nomo_message).data));"));
        assert!(c.contains("nomo_string_release(nomo_message);"));
    }

    #[test]
    fn emits_string_retain_and_release_for_shared_bindings() {
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: vec!["std.io".to_string(), "std.string".to_string()],
            structs: Vec::new(),
            enums: Vec::new(),
            functions: vec![Function {
                package: "app.main".to_string(),
                name: "main".to_string(),
                params: Vec::new(),
                return_type: ValueType::Void,
                body: vec![
                    Statement::Let {
                        name: "first".to_string(),
                        value_type: ValueType::String,
                        initializer: ValueExpr::StringConcat {
                            left: Box::new(ValueExpr::StringLiteral("No".to_string())),
                            right: Box::new(ValueExpr::StringLiteral("mo".to_string())),
                        },
                    },
                    Statement::Let {
                        name: "second".to_string(),
                        value_type: ValueType::String,
                        initializer: ValueExpr::Variable("first".to_string()),
                    },
                ],
            }],
        };

        let c = emit_c(&program);
        assert!(c.contains("typedef struct nomo_string"));
        assert!(c.contains("static nomo_string nomo_string_retain(nomo_string value)"));
        assert!(c.contains("static void nomo_string_release(nomo_string value)"));
        assert!(c.contains("nomo_second = nomo_string_retain(nomo_second);"));
        let retain = c
            .find("nomo_second = nomo_string_retain(nomo_second);")
            .unwrap();
        let release_second = c[retain..]
            .find("nomo_string_release(nomo_second);")
            .unwrap()
            + retain;
        let release_first = c[release_second..]
            .find("nomo_string_release(nomo_first);")
            .unwrap()
            + release_second;
        assert!(retain < release_second);
        assert!(release_second < release_first);
    }

    #[test]
    fn emits_string_parameter_retain_before_return_release() {
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: Vec::new(),
            structs: Vec::new(),
            enums: Vec::new(),
            functions: vec![
                Function {
                    package: "app.main".to_string(),
                    name: "echo".to_string(),
                    params: vec![Parameter {
                        name: "value".to_string(),
                        mutable: false,
                        value_type: ValueType::String,
                    }],
                    return_type: ValueType::String,
                    body: vec![Statement::Return(Some(ValueExpr::Variable(
                        "value".to_string(),
                    )))],
                },
                Function {
                    package: "app.main".to_string(),
                    name: "main".to_string(),
                    params: Vec::new(),
                    return_type: ValueType::Void,
                    body: Vec::new(),
                },
            ],
        };

        let c = emit_c(&program);
        let fn_start = c
            .find("nomo_string nomo_fn_echo(nomo_string nomo_value)")
            .unwrap();
        let param_retain = c[fn_start..]
            .find("nomo_value = nomo_string_retain(nomo_value);")
            .unwrap()
            + fn_start;
        let return_retain = c[param_retain..]
            .find("nomo__return = nomo_string_retain(nomo__return);")
            .unwrap()
            + param_retain;
        let param_release = c[return_retain..]
            .find("nomo_string_release(nomo_value);")
            .unwrap()
            + return_retain;
        let return_stmt = c[param_release..].find("return nomo__return;").unwrap() + param_release;
        assert!(fn_start < param_retain);
        assert!(param_retain < return_retain);
        assert!(return_retain < param_release);
        assert!(param_release < return_stmt);
    }

    #[test]
    fn emits_fs_read_and_write_helpers() {
        let fs_error = ValueType::Struct("FsError".to_string(), Vec::new());
        let result_string_error = ValueType::Enum(
            "Result".to_string(),
            vec![ValueType::String, fs_error.clone()],
        );
        let result_void_error =
            ValueType::Enum("Result".to_string(), vec![ValueType::Void, fs_error]);
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: vec!["std.fs".to_string()],
            structs: vec![StructType {
                package: "app.main".to_string(),
                name: "FsError".to_string(),
                type_params: Vec::new(),
                fields: vec![StructField {
                    name: "message".to_string(),
                    value_type: ValueType::String,
                }],
            }],
            enums: vec![EnumType {
                package: "app.main".to_string(),
                name: "Result".to_string(),
                type_params: vec!["T".to_string(), "E".to_string()],
                variants: vec![
                    EnumVariantType {
                        name: "Ok".to_string(),
                        payload: Some(ValueType::TypeParam("T".to_string())),
                    },
                    EnumVariantType {
                        name: "Err".to_string(),
                        payload: Some(ValueType::TypeParam("E".to_string())),
                    },
                ],
            }],
            functions: vec![Function {
                package: "app.main".to_string(),
                name: "main".to_string(),
                params: Vec::new(),
                return_type: ValueType::Void,
                body: vec![
                    Statement::Let {
                        name: "read_result".to_string(),
                        value_type: result_string_error,
                        initializer: ValueExpr::FsReadToString {
                            path: Box::new(ValueExpr::StringLiteral("input.txt".to_string())),
                        },
                    },
                    Statement::Let {
                        name: "write_result".to_string(),
                        value_type: result_void_error,
                        initializer: ValueExpr::FsWriteString {
                            path: Box::new(ValueExpr::StringLiteral("output.txt".to_string())),
                            content: Box::new(ValueExpr::StringLiteral("hello".to_string())),
                        },
                    },
                ],
            }],
        };

        let c = emit_c(&program);
        assert!(c.contains("#include <errno.h>"));
        assert!(c.contains("typedef struct nomo_struct_FsError"));
        assert!(c.contains("static nomo_enum_Result_string_struct_FsError nomo_fs_read_to_string"));
        assert!(c.contains("static nomo_enum_Result_void_struct_FsError nomo_fs_write_string"));
        assert!(c.contains("nomo_fs_read_to_string(nomo_string_literal(\"input.txt\"))"));
        assert!(c.contains(
            "nomo_fs_write_string(nomo_string_literal(\"output.txt\"), nomo_string_literal(\"hello\"))"
        ));
    }

    #[test]
    fn emits_env_get_helper() {
        let option_string = ValueType::Enum("Option".to_string(), vec![ValueType::String]);
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: vec!["std.env".to_string()],
            structs: Vec::new(),
            enums: vec![EnumType {
                package: "app.main".to_string(),
                name: "Option".to_string(),
                type_params: vec!["T".to_string()],
                variants: vec![
                    EnumVariantType {
                        name: "Some".to_string(),
                        payload: Some(ValueType::TypeParam("T".to_string())),
                    },
                    EnumVariantType {
                        name: "None".to_string(),
                        payload: None,
                    },
                ],
            }],
            functions: vec![Function {
                package: "app.main".to_string(),
                name: "main".to_string(),
                params: Vec::new(),
                return_type: ValueType::Void,
                body: vec![Statement::Let {
                    name: "value".to_string(),
                    value_type: option_string,
                    initializer: ValueExpr::EnvGet {
                        name: Box::new(ValueExpr::StringLiteral("NOMO_TEST_ENV".to_string())),
                    },
                }],
            }],
        };

        let c = emit_c(&program);
        assert!(c.contains("static nomo_enum_Option_string nomo_env_get"));
        assert!(c.contains("getenv(name.data)"));
        assert!(c.contains("nomo_env_get(nomo_string_literal(\"NOMO_TEST_ENV\"))"));
    }

    #[test]
    #[should_panic(expected = "unsupported Array element type reached C type lowering")]
    fn panics_instead_of_emitting_unsupported_array_placeholders() {
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: Vec::new(),
            structs: Vec::new(),
            enums: Vec::new(),
            functions: vec![Function {
                package: "app.main".to_string(),
                name: "main".to_string(),
                params: Vec::new(),
                return_type: ValueType::Void,
                body: vec![Statement::Let {
                    name: "bad".to_string(),
                    value_type: ValueType::Array(Box::new(ValueType::Void)),
                    initializer: ValueExpr::ArrayNew {
                        element_type: ValueType::Void,
                    },
                }],
            }],
        };

        let _ = emit_c(&program);
    }

    #[test]
    fn emits_env_args_helper_and_main_arguments() {
        let array_string = ValueType::Array(Box::new(ValueType::String));
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: vec!["std.env".to_string()],
            structs: Vec::new(),
            enums: vec![EnumType {
                package: "app.main".to_string(),
                name: "Option".to_string(),
                type_params: vec!["T".to_string()],
                variants: vec![
                    EnumVariantType {
                        name: "Some".to_string(),
                        payload: Some(ValueType::TypeParam("T".to_string())),
                    },
                    EnumVariantType {
                        name: "None".to_string(),
                        payload: None,
                    },
                ],
            }],
            functions: vec![Function {
                package: "app.main".to_string(),
                name: "main".to_string(),
                params: Vec::new(),
                return_type: ValueType::Void,
                body: vec![Statement::Let {
                    name: "args".to_string(),
                    value_type: array_string,
                    initializer: ValueExpr::EnvArgs,
                }],
            }],
        };

        let c = emit_c(&program);
        assert!(c.contains("int main(int argc, char **argv)"));
        assert!(c.contains("static int nomo_argc = 0;"));
        assert!(c.contains("static char **nomo_argv = NULL;"));
        assert!(c.contains("static nomo_array_string nomo_env_args(int argc, char **argv)"));
        assert!(c.contains("nomo_argc = argc;"));
        assert!(c.contains("nomo_argv = argv;"));
        assert!(c.contains("nomo_array_string nomo_args = nomo_env_args(nomo_argc, nomo_argv);"));
    }

    #[test]
    fn emits_string_array_helpers() {
        let array_string = ValueType::Array(Box::new(ValueType::String));
        let option_string = ValueType::Enum("Option".to_string(), vec![ValueType::String]);
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: vec!["std.array".to_string()],
            structs: Vec::new(),
            enums: vec![EnumType {
                package: "app.main".to_string(),
                name: "Option".to_string(),
                type_params: vec!["T".to_string()],
                variants: vec![
                    EnumVariantType {
                        name: "Some".to_string(),
                        payload: Some(ValueType::TypeParam("T".to_string())),
                    },
                    EnumVariantType {
                        name: "None".to_string(),
                        payload: None,
                    },
                ],
            }],
            functions: vec![Function {
                package: "app.main".to_string(),
                name: "main".to_string(),
                params: Vec::new(),
                return_type: ValueType::Void,
                body: vec![
                    Statement::Let {
                        name: "items".to_string(),
                        value_type: array_string.clone(),
                        initializer: ValueExpr::ArrayNew {
                            element_type: ValueType::String,
                        },
                    },
                    Statement::Assign {
                        name: "items".to_string(),
                        value: ValueExpr::ArrayPush {
                            array: "items".to_string(),
                            value: Box::new(ValueExpr::StringLiteral("first".to_string())),
                            element_type: ValueType::String,
                        },
                    },
                    Statement::Let {
                        name: "size".to_string(),
                        value_type: ValueType::U64,
                        initializer: ValueExpr::ArrayLen {
                            array: Box::new(ValueExpr::Variable("items".to_string())),
                        },
                    },
                    Statement::Let {
                        name: "first".to_string(),
                        value_type: option_string,
                        initializer: ValueExpr::ArrayGet {
                            array: Box::new(ValueExpr::Variable("items".to_string())),
                            index: Box::new(ValueExpr::IntLiteral(0)),
                            element_type: ValueType::String,
                        },
                    },
                ],
            }],
        };

        let c = emit_c(&program);
        assert!(c.contains("typedef struct nomo_array_string"));
        assert!(c.contains("nomo_array_string nomo_items = nomo_array_string_new();"));
        assert!(c.contains(
            "nomo_items = nomo_array_string_push(nomo_items, nomo_string_literal(\"first\"));"
        ));
        assert!(c.contains("uint64_t nomo_size = ((uint64_t)nomo_items.len);"));
        assert!(c.contains("nomo_array_string_get(nomo_items, 0)"));
    }

    #[test]
    fn emits_i32_array_helpers() {
        let array_i32 = ValueType::Array(Box::new(ValueType::I32));
        let option_i32 = ValueType::Enum("Option".to_string(), vec![ValueType::I32]);
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: vec!["std.array".to_string()],
            structs: Vec::new(),
            enums: vec![EnumType {
                package: "app.main".to_string(),
                name: "Option".to_string(),
                type_params: vec!["T".to_string()],
                variants: vec![
                    EnumVariantType {
                        name: "Some".to_string(),
                        payload: Some(ValueType::TypeParam("T".to_string())),
                    },
                    EnumVariantType {
                        name: "None".to_string(),
                        payload: None,
                    },
                ],
            }],
            functions: vec![Function {
                package: "app.main".to_string(),
                name: "main".to_string(),
                params: Vec::new(),
                return_type: ValueType::Void,
                body: vec![
                    Statement::Let {
                        name: "items".to_string(),
                        value_type: array_i32.clone(),
                        initializer: ValueExpr::ArrayNew {
                            element_type: ValueType::I32,
                        },
                    },
                    Statement::Assign {
                        name: "items".to_string(),
                        value: ValueExpr::ArrayPush {
                            array: "items".to_string(),
                            value: Box::new(ValueExpr::IntLiteral(7)),
                            element_type: ValueType::I32,
                        },
                    },
                    Statement::Let {
                        name: "first".to_string(),
                        value_type: option_i32,
                        initializer: ValueExpr::ArrayGet {
                            array: Box::new(ValueExpr::Variable("items".to_string())),
                            index: Box::new(ValueExpr::IntLiteral(0)),
                            element_type: ValueType::I32,
                        },
                    },
                ],
            }],
        };

        let c = emit_c(&program);
        assert!(c.contains("typedef struct nomo_array_i32"));
        assert!(c.contains("int32_t *data;"));
        assert!(c.contains("size_t *refcount;"));
        assert!(c.contains("static nomo_array_i32 nomo_array_i32_retain(nomo_array_i32 array)"));
        assert!(c.contains("static void nomo_array_i32_release(nomo_array_i32 array)"));
        assert!(c.contains("if (*array.refcount != 0) { return; }"));
        assert!(c.contains("free(array.data);"));
        assert!(c.contains("free(array.refcount);"));
        assert!(c.contains(
            "static nomo_array_i32 nomo_array_i32_make_unique(nomo_array_i32 array, size_t needed)"
        ));
        assert!(c.contains("array = nomo_array_i32_make_unique(array, array.len + 1);"));
        assert!(c.contains("array = nomo_array_i32_make_unique(array, array.len);"));
        assert!(c.contains("nomo_array_i32 nomo_items = nomo_array_i32_new();"));
        assert!(c.contains("nomo_items = nomo_array_i32_push(nomo_items, 7);"));
        assert!(c.contains("nomo_array_i32_get(nomo_items, 0)"));
    }

    #[test]
    fn emits_array_retain_for_shared_array_bindings_and_nested_elements() {
        let array_i32 = ValueType::Array(Box::new(ValueType::I32));
        let array_array_i32 = ValueType::Array(Box::new(array_i32.clone()));
        let option_array_i32 = ValueType::Enum("Option".to_string(), vec![array_i32.clone()]);
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: vec!["std.array".to_string()],
            structs: Vec::new(),
            enums: vec![EnumType {
                package: "app.main".to_string(),
                name: "Option".to_string(),
                type_params: vec!["T".to_string()],
                variants: vec![
                    EnumVariantType {
                        name: "Some".to_string(),
                        payload: Some(ValueType::TypeParam("T".to_string())),
                    },
                    EnumVariantType {
                        name: "None".to_string(),
                        payload: None,
                    },
                ],
            }],
            functions: vec![Function {
                package: "app.main".to_string(),
                name: "main".to_string(),
                params: Vec::new(),
                return_type: ValueType::Void,
                body: vec![
                    Statement::Let {
                        name: "inner".to_string(),
                        value_type: array_i32.clone(),
                        initializer: ValueExpr::ArrayNew {
                            element_type: ValueType::I32,
                        },
                    },
                    Statement::Let {
                        name: "snapshot".to_string(),
                        value_type: array_i32.clone(),
                        initializer: ValueExpr::Variable("inner".to_string()),
                    },
                    Statement::Let {
                        name: "outer".to_string(),
                        value_type: array_array_i32,
                        initializer: ValueExpr::ArrayNew {
                            element_type: array_i32.clone(),
                        },
                    },
                    Statement::Assign {
                        name: "outer".to_string(),
                        value: ValueExpr::ArrayPush {
                            array: "outer".to_string(),
                            value: Box::new(ValueExpr::Variable("inner".to_string())),
                            element_type: array_i32.clone(),
                        },
                    },
                    Statement::Let {
                        name: "first".to_string(),
                        value_type: option_array_i32,
                        initializer: ValueExpr::ArrayGet {
                            array: Box::new(ValueExpr::Variable("outer".to_string())),
                            index: Box::new(ValueExpr::IntLiteral(0)),
                            element_type: array_i32,
                        },
                    },
                ],
            }],
        };

        let c = emit_c(&program);
        assert!(c.contains("nomo_snapshot = nomo_array_i32_retain(nomo_snapshot);"));
        assert!(c.contains("array.data[array.len] = nomo_array_i32_retain(value);"));
        assert!(c.contains("nomo_array_i32_retain(array.data[index])"));
        assert!(c.contains("nomo_array_i32_release(nomo_snapshot);"));
        assert!(c.contains("nomo_array_i32_release(nomo_inner);"));
        assert!(c.contains("nomo_array_array_i32_release(nomo_outer);"));
        assert!(c.contains("nomo_array_i32_release(array.data[i]);"));
        assert!(c.contains("nomo_enum_Option_array_i32_release(nomo_first);"));
        assert!(c.contains("if (value.tag == nomo_enum_Option_array_i32_Some) {"));
        assert!(c.contains("nomo_array_i32_release(value.payload.nomo_payload_Some);"));
    }

    #[test]
    fn emits_array_releases_before_return_and_try_error_exit() {
        let array_i32 = ValueType::Array(Box::new(ValueType::I32));
        let result_i32_string = ValueType::Enum(
            "Result".to_string(),
            vec![ValueType::I32, ValueType::String],
        );
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: vec!["std.array".to_string()],
            structs: Vec::new(),
            enums: vec![
                EnumType {
                    package: "app.main".to_string(),
                    name: "Option".to_string(),
                    type_params: vec!["T".to_string()],
                    variants: vec![
                        EnumVariantType {
                            name: "Some".to_string(),
                            payload: Some(ValueType::TypeParam("T".to_string())),
                        },
                        EnumVariantType {
                            name: "None".to_string(),
                            payload: None,
                        },
                    ],
                },
                EnumType {
                    package: "app.main".to_string(),
                    name: "Result".to_string(),
                    type_params: vec!["T".to_string(), "E".to_string()],
                    variants: vec![
                        EnumVariantType {
                            name: "Ok".to_string(),
                            payload: Some(ValueType::TypeParam("T".to_string())),
                        },
                        EnumVariantType {
                            name: "Err".to_string(),
                            payload: Some(ValueType::TypeParam("E".to_string())),
                        },
                    ],
                },
            ],
            functions: vec![
                Function {
                    package: "app.main".to_string(),
                    name: "parse".to_string(),
                    params: Vec::new(),
                    return_type: result_i32_string.clone(),
                    body: vec![Statement::Return(Some(ValueExpr::EnumVariant {
                        enum_name: "Result".to_string(),
                        enum_args: vec![ValueType::I32, ValueType::String],
                        variant: "Ok".to_string(),
                        payload: Some(Box::new(ValueExpr::IntLiteral(7))),
                    }))],
                },
                Function {
                    package: "app.main".to_string(),
                    name: "compute".to_string(),
                    params: Vec::new(),
                    return_type: result_i32_string,
                    body: vec![
                        Statement::Let {
                            name: "items".to_string(),
                            value_type: array_i32.clone(),
                            initializer: ValueExpr::ArrayNew {
                                element_type: ValueType::I32,
                            },
                        },
                        Statement::TryLet {
                            name: "value".to_string(),
                            value_type: ValueType::I32,
                            result_type: ValueType::Enum(
                                "Result".to_string(),
                                vec![ValueType::I32, ValueType::String],
                            ),
                            return_type: ValueType::Enum(
                                "Result".to_string(),
                                vec![ValueType::I32, ValueType::String],
                            ),
                            result_expr: ValueExpr::Call {
                                name: "parse".to_string(),
                                args: Vec::new(),
                            },
                        },
                        Statement::Return(Some(ValueExpr::EnumVariant {
                            enum_name: "Result".to_string(),
                            enum_args: vec![ValueType::I32, ValueType::String],
                            variant: "Ok".to_string(),
                            payload: Some(Box::new(ValueExpr::Variable("value".to_string()))),
                        })),
                    ],
                },
                Function {
                    package: "app.main".to_string(),
                    name: "main".to_string(),
                    params: Vec::new(),
                    return_type: ValueType::Void,
                    body: Vec::new(),
                },
            ],
        };

        let c = emit_c(&program);
        let try_error = c.find("if (nomo_value_result.tag").unwrap();
        let try_temp = c[try_error..]
            .find("nomo_enum_Result_i32_string nomo__try_return =")
            .unwrap();
        let release_in_error = c[try_error..]
            .find("nomo_array_i32_release(nomo_items);")
            .unwrap();
        let try_return = c[try_error..].find("return nomo__try_return;").unwrap();
        assert!(try_temp < release_in_error);
        assert!(release_in_error < try_return);
        let ok_return = c.rfind("return nomo__return;").unwrap();
        let release_before_ok = c[..ok_return]
            .rfind("nomo_array_i32_release(nomo_items);")
            .unwrap();
        assert!(release_before_ok < ok_return);
    }

    #[test]
    fn emits_try_return_ok_with_cleanup_on_error_and_success() {
        let array_i32 = ValueType::Array(Box::new(ValueType::I32));
        let result_i32_string = ValueType::Enum(
            "Result".to_string(),
            vec![ValueType::I32, ValueType::String],
        );
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: vec!["std.array".to_string()],
            structs: Vec::new(),
            enums: vec![
                EnumType {
                    package: "app.main".to_string(),
                    name: "Option".to_string(),
                    type_params: vec!["T".to_string()],
                    variants: vec![
                        EnumVariantType {
                            name: "Some".to_string(),
                            payload: Some(ValueType::TypeParam("T".to_string())),
                        },
                        EnumVariantType {
                            name: "None".to_string(),
                            payload: None,
                        },
                    ],
                },
                EnumType {
                    package: "app.main".to_string(),
                    name: "Result".to_string(),
                    type_params: vec!["T".to_string(), "E".to_string()],
                    variants: vec![
                        EnumVariantType {
                            name: "Ok".to_string(),
                            payload: Some(ValueType::TypeParam("T".to_string())),
                        },
                        EnumVariantType {
                            name: "Err".to_string(),
                            payload: Some(ValueType::TypeParam("E".to_string())),
                        },
                    ],
                },
            ],
            functions: vec![
                Function {
                    package: "app.main".to_string(),
                    name: "parse".to_string(),
                    params: Vec::new(),
                    return_type: result_i32_string.clone(),
                    body: vec![Statement::Return(Some(ValueExpr::EnumVariant {
                        enum_name: "Result".to_string(),
                        enum_args: vec![ValueType::I32, ValueType::String],
                        variant: "Ok".to_string(),
                        payload: Some(Box::new(ValueExpr::IntLiteral(7))),
                    }))],
                },
                Function {
                    package: "app.main".to_string(),
                    name: "compute".to_string(),
                    params: Vec::new(),
                    return_type: result_i32_string.clone(),
                    body: vec![
                        Statement::Let {
                            name: "items".to_string(),
                            value_type: array_i32.clone(),
                            initializer: ValueExpr::ArrayNew {
                                element_type: ValueType::I32,
                            },
                        },
                        Statement::TryReturnOk {
                            ok_type: ValueType::I32,
                            result_type: result_i32_string.clone(),
                            return_type: result_i32_string,
                            result_expr: ValueExpr::Call {
                                name: "parse".to_string(),
                                args: Vec::new(),
                            },
                        },
                    ],
                },
                Function {
                    package: "app.main".to_string(),
                    name: "main".to_string(),
                    params: Vec::new(),
                    return_type: ValueType::Void,
                    body: Vec::new(),
                },
            ],
        };

        let c = emit_c(&program);
        let try_result = c.find("nomo__try_result = nomo_fn_parse();").unwrap();
        let error_branch = c[try_result..]
            .find("if (nomo__try_result.tag == nomo_enum_Result_i32_string_Err)")
            .unwrap();
        let try_return = c[try_result..].find("return nomo__try_return;").unwrap();
        let error_release = c[try_result..try_result + try_return]
            .find("nomo_array_i32_release(nomo_items);")
            .unwrap();
        assert!(error_branch < error_release);

        let ok_temp = c[try_result..]
            .find("int32_t nomo__try_ok = nomo__try_result.payload.nomo_payload_Ok;")
            .unwrap();
        let ok_return = c[try_result..].find("return nomo__return;").unwrap();
        let success_release = c[try_result + ok_temp..try_result + ok_return]
            .find("nomo_array_i32_release(nomo_items);")
            .unwrap();
        assert!(success_release < ok_return - ok_temp);
    }

    #[test]
    fn try_let_retains_managed_payloads_when_result_expr_is_shared() {
        let array_i32 = ValueType::Array(Box::new(ValueType::I32));
        let result_array_array = ValueType::Enum(
            "Result".to_string(),
            vec![array_i32.clone(), array_i32.clone()],
        );
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: vec!["std.array".to_string()],
            structs: Vec::new(),
            enums: vec![
                EnumType {
                    package: "app.main".to_string(),
                    name: "Option".to_string(),
                    type_params: vec!["T".to_string()],
                    variants: vec![
                        EnumVariantType {
                            name: "Some".to_string(),
                            payload: Some(ValueType::TypeParam("T".to_string())),
                        },
                        EnumVariantType {
                            name: "None".to_string(),
                            payload: None,
                        },
                    ],
                },
                EnumType {
                    package: "app.main".to_string(),
                    name: "Result".to_string(),
                    type_params: vec!["T".to_string(), "E".to_string()],
                    variants: vec![
                        EnumVariantType {
                            name: "Ok".to_string(),
                            payload: Some(ValueType::TypeParam("T".to_string())),
                        },
                        EnumVariantType {
                            name: "Err".to_string(),
                            payload: Some(ValueType::TypeParam("E".to_string())),
                        },
                    ],
                },
            ],
            functions: vec![
                Function {
                    package: "app.main".to_string(),
                    name: "compute".to_string(),
                    params: Vec::new(),
                    return_type: result_array_array.clone(),
                    body: vec![
                        Statement::Let {
                            name: "items".to_string(),
                            value_type: array_i32.clone(),
                            initializer: ValueExpr::ArrayNew {
                                element_type: ValueType::I32,
                            },
                        },
                        Statement::Let {
                            name: "raw".to_string(),
                            value_type: result_array_array.clone(),
                            initializer: ValueExpr::EnumVariant {
                                enum_name: "Result".to_string(),
                                enum_args: vec![array_i32.clone(), array_i32.clone()],
                                variant: "Ok".to_string(),
                                payload: Some(Box::new(ValueExpr::Variable("items".to_string()))),
                            },
                        },
                        Statement::TryLet {
                            name: "value".to_string(),
                            value_type: array_i32.clone(),
                            result_type: result_array_array.clone(),
                            return_type: result_array_array.clone(),
                            result_expr: ValueExpr::Variable("raw".to_string()),
                        },
                        Statement::Return(Some(ValueExpr::EnumVariant {
                            enum_name: "Result".to_string(),
                            enum_args: vec![array_i32.clone(), array_i32],
                            variant: "Ok".to_string(),
                            payload: Some(Box::new(ValueExpr::Variable("value".to_string()))),
                        })),
                    ],
                },
                Function {
                    package: "app.main".to_string(),
                    name: "main".to_string(),
                    params: Vec::new(),
                    return_type: ValueType::Void,
                    body: Vec::new(),
                },
            ],
        };

        let c = emit_c(&program);
        assert!(c.contains("nomo_enum_Result_array_i32_array_i32 nomo_value_result = nomo_raw;"));
        let try_error = c.find("if (nomo_value_result.tag").unwrap();
        let try_return_retain = c[try_error..]
            .find(
                "nomo__try_return = nomo_enum_Result_array_i32_array_i32_retain(nomo__try_return);",
            )
            .unwrap();
        let raw_release = c[try_error..]
            .find("nomo_enum_Result_array_i32_array_i32_release(nomo_raw);")
            .unwrap();
        let try_return = c[try_error..].find("return nomo__try_return;").unwrap();
        assert!(try_return_retain < raw_release);
        assert!(raw_release < try_return);
        assert!(c.contains("nomo_value = nomo_array_i32_retain(nomo_value);"));
    }

    #[test]
    fn break_releases_only_loop_body_array_locals() {
        let array_i32 = ValueType::Array(Box::new(ValueType::I32));
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: vec!["std.array".to_string()],
            structs: Vec::new(),
            enums: vec![EnumType {
                package: "app.main".to_string(),
                name: "Option".to_string(),
                type_params: vec!["T".to_string()],
                variants: vec![
                    EnumVariantType {
                        name: "Some".to_string(),
                        payload: Some(ValueType::TypeParam("T".to_string())),
                    },
                    EnumVariantType {
                        name: "None".to_string(),
                        payload: None,
                    },
                ],
            }],
            functions: vec![Function {
                package: "app.main".to_string(),
                name: "main".to_string(),
                params: Vec::new(),
                return_type: ValueType::Void,
                body: vec![
                    Statement::Let {
                        name: "items".to_string(),
                        value_type: array_i32.clone(),
                        initializer: ValueExpr::ArrayNew {
                            element_type: ValueType::I32,
                        },
                    },
                    Statement::Loop {
                        kind: LoopKind::Infinite,
                        body: vec![
                            Statement::Let {
                                name: "temp".to_string(),
                                value_type: array_i32,
                                initializer: ValueExpr::ArrayNew {
                                    element_type: ValueType::I32,
                                },
                            },
                            Statement::Break,
                        ],
                    },
                    Statement::Let {
                        name: "size".to_string(),
                        value_type: ValueType::U64,
                        initializer: ValueExpr::ArrayLen {
                            array: Box::new(ValueExpr::Variable("items".to_string())),
                        },
                    },
                ],
            }],
        };

        let c = emit_c(&program);
        let break_index = c.find("break;").unwrap();
        let temp_release = c.find("nomo_array_i32_release(nomo_temp);").unwrap();
        assert!(temp_release < break_index);
        assert!(!c[..break_index].contains("nomo_array_i32_release(nomo_items);"));
        let size_index = c
            .find("uint64_t nomo_size = ((uint64_t)nomo_items.len);")
            .unwrap();
        let items_release = c.rfind("nomo_array_i32_release(nomo_items);").unwrap();
        assert!(break_index < size_index);
        assert!(size_index < items_release);
    }

    #[test]
    fn for_in_releases_owned_iterable_temp_but_not_shared_iterable() {
        let array_string = ValueType::Array(Box::new(ValueType::String));
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: vec!["std.env".to_string(), "std.array".to_string()],
            structs: Vec::new(),
            enums: vec![EnumType {
                package: "app.main".to_string(),
                name: "Option".to_string(),
                type_params: vec!["T".to_string()],
                variants: vec![
                    EnumVariantType {
                        name: "Some".to_string(),
                        payload: Some(ValueType::TypeParam("T".to_string())),
                    },
                    EnumVariantType {
                        name: "None".to_string(),
                        payload: None,
                    },
                ],
            }],
            functions: vec![Function {
                package: "app.main".to_string(),
                name: "main".to_string(),
                params: Vec::new(),
                return_type: ValueType::Void,
                body: vec![
                    Statement::Loop {
                        kind: LoopKind::Iterate {
                            binding: "arg".to_string(),
                            element_type: ValueType::String,
                            iterable: ValueExpr::EnvArgs,
                        },
                        body: vec![Statement::Println(ValueExpr::Variable("arg".to_string()))],
                    },
                    Statement::Let {
                        name: "words".to_string(),
                        value_type: array_string,
                        initializer: ValueExpr::EnvArgs,
                    },
                    Statement::Loop {
                        kind: LoopKind::Iterate {
                            binding: "word".to_string(),
                            element_type: ValueType::String,
                            iterable: ValueExpr::Variable("words".to_string()),
                        },
                        body: vec![Statement::Println(ValueExpr::Variable("word".to_string()))],
                    },
                ],
            }],
        };

        let c = emit_c(&program);
        let owned_seq = "nomo_array_string nomo__seq = nomo_env_args(nomo_argc, nomo_argv);";
        let owned_release = "nomo_array_string_release(nomo__seq);";
        let shared_seq = "nomo_array_string nomo__seq = nomo_words;";
        let owned_seq_index = c.find(owned_seq).unwrap();
        let owned_release_index =
            c[owned_seq_index..].find(owned_release).unwrap() + owned_seq_index;
        let shared_seq_index = c.find(shared_seq).unwrap();
        assert!(owned_seq_index < owned_release_index);
        assert!(owned_release_index < shared_seq_index);
        assert!(!c[shared_seq_index..].contains(owned_release));
    }

    #[test]
    fn for_in_releases_managed_binding_after_each_iteration() {
        let array_i32 = ValueType::Array(Box::new(ValueType::I32));
        let array_array_i32 = ValueType::Array(Box::new(array_i32.clone()));
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: vec!["std.array".to_string()],
            structs: Vec::new(),
            enums: vec![EnumType {
                package: "app.main".to_string(),
                name: "Option".to_string(),
                type_params: vec!["T".to_string()],
                variants: vec![
                    EnumVariantType {
                        name: "Some".to_string(),
                        payload: Some(ValueType::TypeParam("T".to_string())),
                    },
                    EnumVariantType {
                        name: "None".to_string(),
                        payload: None,
                    },
                ],
            }],
            functions: vec![Function {
                package: "app.main".to_string(),
                name: "main".to_string(),
                params: Vec::new(),
                return_type: ValueType::Void,
                body: vec![
                    Statement::Let {
                        name: "items".to_string(),
                        value_type: array_array_i32,
                        initializer: ValueExpr::ArrayNew {
                            element_type: array_i32.clone(),
                        },
                    },
                    Statement::Loop {
                        kind: LoopKind::Iterate {
                            binding: "item".to_string(),
                            element_type: array_i32,
                            iterable: ValueExpr::Variable("items".to_string()),
                        },
                        body: vec![Statement::Println(ValueExpr::StringLiteral(
                            "tick".to_string(),
                        ))],
                    },
                ],
            }],
        };

        let c = emit_c(&program);
        let binding = "nomo_array_i32 nomo_item = nomo__seq.data[nomo_i];";
        let retain = "nomo_item = nomo_array_i32_retain(nomo_item);";
        let body = puts_literal("tick");
        let release = "nomo_array_i32_release(nomo_item);";
        let binding_index = c.find(binding).unwrap();
        let retain_index = c[binding_index..].find(retain).unwrap() + binding_index;
        let body_index = c[retain_index..].find(&body).unwrap() + retain_index;
        let release_index = c[body_index..].find(release).unwrap() + body_index;
        assert!(binding_index < retain_index);
        assert!(retain_index < body_index);
        assert!(body_index < release_index);
    }

    #[test]
    fn for_in_return_releases_owned_iterable_temp_and_managed_binding() {
        let array_i32 = ValueType::Array(Box::new(ValueType::I32));
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: vec!["std.array".to_string()],
            structs: Vec::new(),
            enums: vec![EnumType {
                package: "app.main".to_string(),
                name: "Option".to_string(),
                type_params: vec!["T".to_string()],
                variants: vec![
                    EnumVariantType {
                        name: "Some".to_string(),
                        payload: Some(ValueType::TypeParam("T".to_string())),
                    },
                    EnumVariantType {
                        name: "None".to_string(),
                        payload: None,
                    },
                ],
            }],
            functions: vec![
                Function {
                    package: "app.main".to_string(),
                    name: "take".to_string(),
                    params: Vec::new(),
                    return_type: array_i32.clone(),
                    body: vec![Statement::Loop {
                        kind: LoopKind::Iterate {
                            binding: "item".to_string(),
                            element_type: array_i32.clone(),
                            iterable: ValueExpr::ArrayNew {
                                element_type: array_i32.clone(),
                            },
                        },
                        body: vec![Statement::Return(Some(ValueExpr::Variable(
                            "item".to_string(),
                        )))],
                    }],
                },
                Function {
                    package: "app.main".to_string(),
                    name: "main".to_string(),
                    params: Vec::new(),
                    return_type: ValueType::Void,
                    body: Vec::new(),
                },
            ],
        };

        let c = emit_c(&program);
        let return_temp = "nomo_array_i32 nomo__return = nomo_item;";
        let retain_return = "nomo__return = nomo_array_i32_retain(nomo__return);";
        let release_binding = "nomo_array_i32_release(nomo_item);";
        let release_seq = "nomo_array_array_i32_release(nomo__seq);";
        let return_stmt = "return nomo__return;";
        let return_temp_index = c.find(return_temp).unwrap();
        let retain_index = c[return_temp_index..].find(retain_return).unwrap() + return_temp_index;
        let binding_release_index = c[retain_index..].find(release_binding).unwrap() + retain_index;
        let seq_release_index =
            c[binding_release_index..].find(release_seq).unwrap() + binding_release_index;
        let return_index = c[seq_release_index..].find(return_stmt).unwrap() + seq_release_index;
        assert!(return_temp_index < retain_index);
        assert!(retain_index < binding_release_index);
        assert!(binding_release_index < seq_release_index);
        assert!(seq_release_index < return_index);
    }

    #[test]
    fn array_reassignment_releases_old_storage_and_retains_shared_rhs() {
        let array_i32 = ValueType::Array(Box::new(ValueType::I32));
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: vec!["std.array".to_string()],
            structs: Vec::new(),
            enums: vec![EnumType {
                package: "app.main".to_string(),
                name: "Option".to_string(),
                type_params: vec!["T".to_string()],
                variants: vec![
                    EnumVariantType {
                        name: "Some".to_string(),
                        payload: Some(ValueType::TypeParam("T".to_string())),
                    },
                    EnumVariantType {
                        name: "None".to_string(),
                        payload: None,
                    },
                ],
            }],
            functions: vec![Function {
                package: "app.main".to_string(),
                name: "main".to_string(),
                params: Vec::new(),
                return_type: ValueType::Void,
                body: vec![
                    Statement::Let {
                        name: "left".to_string(),
                        value_type: array_i32.clone(),
                        initializer: ValueExpr::ArrayNew {
                            element_type: ValueType::I32,
                        },
                    },
                    Statement::Let {
                        name: "right".to_string(),
                        value_type: array_i32,
                        initializer: ValueExpr::ArrayNew {
                            element_type: ValueType::I32,
                        },
                    },
                    Statement::Assign {
                        name: "left".to_string(),
                        value: ValueExpr::Variable("right".to_string()),
                    },
                    Statement::Assign {
                        name: "left".to_string(),
                        value: ValueExpr::Variable("left".to_string()),
                    },
                ],
            }],
        };

        let c = emit_c(&program);
        let temp = "nomo_array_i32 nomo__assign_nomo_left = nomo_right;";
        let retain = "nomo__assign_nomo_left = nomo_array_i32_retain(nomo__assign_nomo_left);";
        let release = "nomo_array_i32_release(nomo_left);";
        let assign = "nomo_left = nomo__assign_nomo_left;";
        let temp_index = c.find(temp).unwrap();
        let retain_index = c[temp_index..].find(retain).unwrap() + temp_index;
        let release_index = c[retain_index..].find(release).unwrap() + retain_index;
        let assign_index = c[release_index..].find(assign).unwrap() + release_index;
        assert!(temp_index < retain_index);
        assert!(retain_index < release_index);
        assert!(release_index < assign_index);
        assert!(c.contains("nomo_array_i32 nomo__assign_nomo_left = nomo_left;"));
    }

    #[test]
    fn option_array_reassignment_retains_and_releases_payload() {
        let array_i32 = ValueType::Array(Box::new(ValueType::I32));
        let option_array_i32 = ValueType::Enum("Option".to_string(), vec![array_i32.clone()]);
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: vec!["std.array".to_string()],
            structs: Vec::new(),
            enums: vec![EnumType {
                package: "app.main".to_string(),
                name: "Option".to_string(),
                type_params: vec!["T".to_string()],
                variants: vec![
                    EnumVariantType {
                        name: "Some".to_string(),
                        payload: Some(ValueType::TypeParam("T".to_string())),
                    },
                    EnumVariantType {
                        name: "None".to_string(),
                        payload: None,
                    },
                ],
            }],
            functions: vec![Function {
                package: "app.main".to_string(),
                name: "main".to_string(),
                params: Vec::new(),
                return_type: ValueType::Void,
                body: vec![
                    Statement::Let {
                        name: "values".to_string(),
                        value_type: array_i32,
                        initializer: ValueExpr::ArrayNew {
                            element_type: ValueType::I32,
                        },
                    },
                    Statement::Let {
                        name: "maybe".to_string(),
                        value_type: option_array_i32.clone(),
                        initializer: ValueExpr::EnumVariant {
                            enum_name: "Option".to_string(),
                            enum_args: vec![ValueType::Array(Box::new(ValueType::I32))],
                            variant: "Some".to_string(),
                            payload: Some(Box::new(ValueExpr::Variable("values".to_string()))),
                        },
                    },
                    Statement::Let {
                        name: "snapshot".to_string(),
                        value_type: option_array_i32,
                        initializer: ValueExpr::Variable("maybe".to_string()),
                    },
                    Statement::Assign {
                        name: "maybe".to_string(),
                        value: ValueExpr::Variable("maybe".to_string()),
                    },
                ],
            }],
        };

        let c = emit_c(&program);
        assert!(c.contains("nomo_maybe = nomo_enum_Option_array_i32_retain(nomo_maybe);"));
        assert!(c.contains("nomo_snapshot = nomo_enum_Option_array_i32_retain(nomo_snapshot);"));
        assert!(c.contains(
            "nomo__assign_nomo_maybe = nomo_enum_Option_array_i32_retain(nomo__assign_nomo_maybe);"
        ));
        assert!(c.contains("nomo_enum_Option_array_i32_release(nomo_maybe);"));
        assert!(c.contains("if (value.tag == nomo_enum_Option_array_i32_Some) {"));
        assert!(c.contains("value.payload.nomo_payload_Some = nomo_array_i32_retain(value.payload.nomo_payload_Some);"));
        assert!(c.contains("nomo_array_i32_release(value.payload.nomo_payload_Some);"));
    }

    #[test]
    fn array_get_returns_owned_option_payload_without_extra_binding_retain() {
        let array_i32 = ValueType::Array(Box::new(ValueType::I32));
        let array_array_i32 = ValueType::Array(Box::new(array_i32.clone()));
        let option_array_i32 = ValueType::Enum("Option".to_string(), vec![array_i32.clone()]);
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: vec!["std.array".to_string()],
            structs: Vec::new(),
            enums: vec![EnumType {
                package: "app.main".to_string(),
                name: "Option".to_string(),
                type_params: vec!["T".to_string()],
                variants: vec![
                    EnumVariantType {
                        name: "Some".to_string(),
                        payload: Some(ValueType::TypeParam("T".to_string())),
                    },
                    EnumVariantType {
                        name: "None".to_string(),
                        payload: None,
                    },
                ],
            }],
            functions: vec![Function {
                package: "app.main".to_string(),
                name: "main".to_string(),
                params: Vec::new(),
                return_type: ValueType::Void,
                body: vec![
                    Statement::Let {
                        name: "inner".to_string(),
                        value_type: array_i32.clone(),
                        initializer: ValueExpr::ArrayNew {
                            element_type: ValueType::I32,
                        },
                    },
                    Statement::Let {
                        name: "outer".to_string(),
                        value_type: array_array_i32,
                        initializer: ValueExpr::ArrayNew {
                            element_type: array_i32.clone(),
                        },
                    },
                    Statement::Assign {
                        name: "outer".to_string(),
                        value: ValueExpr::ArrayPush {
                            array: "outer".to_string(),
                            value: Box::new(ValueExpr::Variable("inner".to_string())),
                            element_type: array_i32.clone(),
                        },
                    },
                    Statement::Let {
                        name: "maybe".to_string(),
                        value_type: option_array_i32.clone(),
                        initializer: ValueExpr::ArrayGet {
                            array: Box::new(ValueExpr::Variable("outer".to_string())),
                            index: Box::new(ValueExpr::IntLiteral(0)),
                            element_type: array_i32,
                        },
                    },
                    Statement::Let {
                        name: "snapshot".to_string(),
                        value_type: option_array_i32,
                        initializer: ValueExpr::Variable("maybe".to_string()),
                    },
                ],
            }],
        };

        let c = emit_c(&program);
        assert!(c.contains(
            "nomo_enum_Option_array_i32 nomo_maybe = nomo_array_array_i32_get(nomo_outer, 0);"
        ));
        assert!(!c.contains("nomo_maybe = nomo_enum_Option_array_i32_retain(nomo_maybe);"));
        assert!(c.contains("nomo_snapshot = nomo_enum_Option_array_i32_retain(nomo_snapshot);"));
    }

    #[test]
    fn if_let_releases_owned_enum_temp_after_retaining_payload_binding() {
        let array_i32 = ValueType::Array(Box::new(ValueType::I32));
        let array_array_i32 = ValueType::Array(Box::new(array_i32.clone()));
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: vec!["std.array".to_string()],
            structs: Vec::new(),
            enums: vec![EnumType {
                package: "app.main".to_string(),
                name: "Option".to_string(),
                type_params: vec!["T".to_string()],
                variants: vec![
                    EnumVariantType {
                        name: "Some".to_string(),
                        payload: Some(ValueType::TypeParam("T".to_string())),
                    },
                    EnumVariantType {
                        name: "None".to_string(),
                        payload: None,
                    },
                ],
            }],
            functions: vec![Function {
                package: "app.main".to_string(),
                name: "main".to_string(),
                params: Vec::new(),
                return_type: ValueType::Void,
                body: vec![
                    Statement::Let {
                        name: "outer".to_string(),
                        value_type: array_array_i32,
                        initializer: ValueExpr::ArrayNew {
                            element_type: array_i32.clone(),
                        },
                    },
                    Statement::IfLet {
                        binding: Some("values".to_string()),
                        value_type: Some(array_i32.clone()),
                        value: ValueExpr::ArrayGet {
                            array: Box::new(ValueExpr::Variable("outer".to_string())),
                            index: Box::new(ValueExpr::IntLiteral(0)),
                            element_type: array_i32,
                        },
                        enum_name: "Option".to_string(),
                        enum_args: vec![ValueType::Array(Box::new(ValueType::I32))],
                        variant: "Some".to_string(),
                        body: vec![Statement::Println(ValueExpr::StringLiteral(
                            "some".to_string(),
                        ))],
                        else_body: Some(vec![Statement::Println(ValueExpr::StringLiteral(
                            "none".to_string(),
                        ))]),
                    },
                ],
            }],
        };

        let c = emit_c(&program);
        let retain = "nomo_values = nomo_array_i32_retain(nomo_values);";
        let temp_release =
            "nomo_enum_Option_array_i32_release(nomo__if_let_nomo_enum_Option_array_i32_Some);";
        let body = puts_literal("some");
        let binding_release = "nomo_array_i32_release(nomo_values);";
        let retain_index = c.find(retain).unwrap();
        let release_index = c[retain_index..].find(temp_release).unwrap() + retain_index;
        let body_index = c[release_index..].find(&body).unwrap() + release_index;
        let binding_release_index = c[body_index..].find(binding_release).unwrap() + body_index;
        assert!(retain_index < release_index);
        assert!(release_index < body_index);
        assert!(body_index < binding_release_index);
        let else_index = c.find(" else {").unwrap();
        let else_release = c[else_index..].find(temp_release).unwrap() + else_index;
        let else_body = c[else_release..].find(&puts_literal("none")).unwrap() + else_release;
        assert!(else_index < else_release);
        assert!(else_release < else_body);
    }

    #[test]
    fn let_else_releases_owned_enum_temp_after_retaining_payload_binding() {
        let array_i32 = ValueType::Array(Box::new(ValueType::I32));
        let array_array_i32 = ValueType::Array(Box::new(array_i32.clone()));
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: vec!["std.array".to_string()],
            structs: Vec::new(),
            enums: vec![EnumType {
                package: "app.main".to_string(),
                name: "Option".to_string(),
                type_params: vec!["T".to_string()],
                variants: vec![
                    EnumVariantType {
                        name: "Some".to_string(),
                        payload: Some(ValueType::TypeParam("T".to_string())),
                    },
                    EnumVariantType {
                        name: "None".to_string(),
                        payload: None,
                    },
                ],
            }],
            functions: vec![Function {
                package: "app.main".to_string(),
                name: "main".to_string(),
                params: Vec::new(),
                return_type: ValueType::Void,
                body: vec![
                    Statement::Let {
                        name: "outer".to_string(),
                        value_type: array_array_i32,
                        initializer: ValueExpr::ArrayNew {
                            element_type: array_i32.clone(),
                        },
                    },
                    Statement::LetElse {
                        binding: "values".to_string(),
                        value_type: array_i32.clone(),
                        value: ValueExpr::ArrayGet {
                            array: Box::new(ValueExpr::Variable("outer".to_string())),
                            index: Box::new(ValueExpr::IntLiteral(0)),
                            element_type: array_i32,
                        },
                        enum_name: "Option".to_string(),
                        enum_args: vec![ValueType::Array(Box::new(ValueType::I32))],
                        variant: "Some".to_string(),
                        else_body: vec![Statement::Panic(ValueExpr::StringLiteral(
                            "missing".to_string(),
                        ))],
                    },
                    Statement::Println(ValueExpr::StringLiteral("ok".to_string())),
                ],
            }],
        };

        let c = emit_c(&program);
        let else_release = "nomo_enum_Option_array_i32_release(nomo__let_else_nomo_values);";
        let else_panic = panic_literal("missing");
        let binding_retain = "nomo_values = nomo_array_i32_retain(nomo_values);";
        let binding_release = "nomo_enum_Option_array_i32_release(nomo__let_else_nomo_values);";
        let else_index = c.find(else_release).unwrap();
        let panic_index = c[else_index..].find(&else_panic).unwrap() + else_index;
        assert!(else_index < panic_index);
        let retain_index = c.rfind(binding_retain).unwrap();
        let release_index = c[retain_index..].find(binding_release).unwrap() + retain_index;
        assert!(retain_index < release_index);
    }

    #[test]
    fn struct_and_custom_enum_lifecycle_helpers_manage_array_payloads() {
        let array_i32 = ValueType::Array(Box::new(ValueType::I32));
        let bag = ValueType::Struct("Bag".to_string(), Vec::new());
        let slot = ValueType::Enum("Slot".to_string(), Vec::new());
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: vec!["std.array".to_string()],
            structs: vec![StructType {
                package: "app.main".to_string(),
                name: "Bag".to_string(),
                type_params: Vec::new(),
                fields: vec![StructField {
                    name: "items".to_string(),
                    value_type: array_i32.clone(),
                }],
            }],
            enums: vec![
                EnumType {
                    package: "app.main".to_string(),
                    name: "Option".to_string(),
                    type_params: vec!["T".to_string()],
                    variants: vec![
                        EnumVariantType {
                            name: "Some".to_string(),
                            payload: Some(ValueType::TypeParam("T".to_string())),
                        },
                        EnumVariantType {
                            name: "None".to_string(),
                            payload: None,
                        },
                    ],
                },
                EnumType {
                    package: "app.main".to_string(),
                    name: "Slot".to_string(),
                    type_params: Vec::new(),
                    variants: vec![
                        EnumVariantType {
                            name: "Full".to_string(),
                            payload: Some(bag.clone()),
                        },
                        EnumVariantType {
                            name: "Empty".to_string(),
                            payload: None,
                        },
                    ],
                },
            ],
            functions: vec![
                Function {
                    package: "app.main".to_string(),
                    name: "label".to_string(),
                    params: vec![Parameter {
                        name: "bag".to_string(),
                        mutable: false,
                        value_type: bag.clone(),
                    }],
                    return_type: ValueType::Void,
                    body: Vec::new(),
                },
                Function {
                    package: "app.main".to_string(),
                    name: "main".to_string(),
                    params: Vec::new(),
                    return_type: ValueType::Void,
                    body: vec![
                        Statement::Let {
                            name: "items".to_string(),
                            value_type: array_i32.clone(),
                            initializer: ValueExpr::ArrayNew {
                                element_type: ValueType::I32,
                            },
                        },
                        Statement::Let {
                            name: "bag".to_string(),
                            value_type: bag.clone(),
                            initializer: ValueExpr::StructLiteral {
                                type_name: "Bag".to_string(),
                                struct_args: Vec::new(),
                                fields: vec![(
                                    "items".to_string(),
                                    ValueExpr::Variable("items".to_string()),
                                )],
                            },
                        },
                        Statement::Let {
                            name: "replacement".to_string(),
                            value_type: array_i32.clone(),
                            initializer: ValueExpr::ArrayNew {
                                element_type: ValueType::I32,
                            },
                        },
                        Statement::AssignField {
                            base: "bag".to_string(),
                            field: "items".to_string(),
                            value_type: array_i32,
                            value: ValueExpr::Variable("replacement".to_string()),
                        },
                        Statement::Let {
                            name: "slot".to_string(),
                            value_type: slot,
                            initializer: ValueExpr::EnumVariant {
                                enum_name: "Slot".to_string(),
                                enum_args: Vec::new(),
                                variant: "Full".to_string(),
                                payload: Some(Box::new(ValueExpr::Variable("bag".to_string()))),
                            },
                        },
                    ],
                },
            ],
        };

        let c = emit_c(&program);
        assert!(c.contains("static nomo_struct_Bag nomo_struct_Bag_retain(nomo_struct_Bag value)"));
        assert!(
            c.contains("value.nomo_member_items = nomo_array_i32_retain(value.nomo_member_items);")
        );
        assert!(c.contains("static void nomo_struct_Bag_release(nomo_struct_Bag value)"));
        assert!(c.contains("nomo_array_i32_release(value.nomo_member_items);"));
        assert!(c.contains("static nomo_enum_Slot nomo_enum_Slot_retain(nomo_enum_Slot value)"));
        assert!(c.contains("value.payload.nomo_payload_Full = nomo_struct_Bag_retain(value.payload.nomo_payload_Full);"));
        assert!(c.contains("nomo_struct_Bag_release(value.payload.nomo_payload_Full);"));
        assert!(c.contains("nomo_bag = nomo_struct_Bag_retain(nomo_bag);"));
        assert!(c.contains("nomo_slot = nomo_enum_Slot_retain(nomo_slot);"));
        assert!(c.contains("nomo_enum_Slot_release(nomo_slot);"));
        let field_temp =
            "nomo_array_i32 nomo__assign_nomo_bag_nomo_member_items = nomo_replacement;";
        let field_retain = "nomo__assign_nomo_bag_nomo_member_items = nomo_array_i32_retain(nomo__assign_nomo_bag_nomo_member_items);";
        let field_release = "nomo_array_i32_release(nomo_bag.nomo_member_items);";
        let field_assign = "nomo_bag.nomo_member_items = nomo__assign_nomo_bag_nomo_member_items;";
        let temp_index = c.find(field_temp).unwrap();
        let retain_index = c[temp_index..].find(field_retain).unwrap() + temp_index;
        let release_index = c[retain_index..].find(field_release).unwrap() + retain_index;
        let assign_index = c[release_index..].find(field_assign).unwrap() + release_index;
        assert!(temp_index < retain_index);
        assert!(retain_index < release_index);
        assert!(release_index < assign_index);
    }

    #[test]
    fn array_parameters_are_retained_and_released_by_value_but_not_mut_borrows() {
        let array_i32 = ValueType::Array(Box::new(ValueType::I32));
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: vec!["std.array".to_string()],
            structs: Vec::new(),
            enums: vec![EnumType {
                package: "app.main".to_string(),
                name: "Option".to_string(),
                type_params: vec!["T".to_string()],
                variants: vec![
                    EnumVariantType {
                        name: "Some".to_string(),
                        payload: Some(ValueType::TypeParam("T".to_string())),
                    },
                    EnumVariantType {
                        name: "None".to_string(),
                        payload: None,
                    },
                ],
            }],
            functions: vec![
                Function {
                    package: "app.main".to_string(),
                    name: "id".to_string(),
                    params: vec![Parameter {
                        name: "values".to_string(),
                        mutable: false,
                        value_type: array_i32.clone(),
                    }],
                    return_type: array_i32.clone(),
                    body: vec![Statement::Return(Some(ValueExpr::Variable(
                        "values".to_string(),
                    )))],
                },
                Function {
                    package: "app.main".to_string(),
                    name: "borrow".to_string(),
                    params: vec![Parameter {
                        name: "values".to_string(),
                        mutable: true,
                        value_type: array_i32,
                    }],
                    return_type: ValueType::Void,
                    body: Vec::new(),
                },
                Function {
                    package: "app.main".to_string(),
                    name: "main".to_string(),
                    params: Vec::new(),
                    return_type: ValueType::Void,
                    body: Vec::new(),
                },
            ],
        };

        let c = emit_c(&program);
        let id_start = c
            .find("nomo_array_i32 nomo_fn_id(nomo_array_i32 nomo_values)")
            .unwrap();
        let id_body = &c[id_start
            ..c[id_start..]
                .find("#undef")
                .map_or(c.len(), |end| id_start + end)];
        assert!(id_body.contains("nomo_values = nomo_array_i32_retain(nomo_values);"));
        assert!(id_body.contains("nomo__return = nomo_array_i32_retain(nomo__return);"));
        assert!(id_body.contains("nomo_array_i32_release(nomo_values);"));

        let borrow_start = c
            .rfind("void nomo_fn_borrow(nomo_array_i32 * nomo_values)")
            .unwrap();
        let main_start = c[borrow_start..]
            .find("int main")
            .map(|offset| borrow_start + offset)
            .unwrap_or(c.len());
        let borrow_body = &c[borrow_start..main_start];
        assert!(!borrow_body.contains("nomo_values = nomo_array_i32_retain(nomo_values);"));
        assert!(!borrow_body.contains("nomo_array_i32_release(nomo_values);"));
    }

    #[test]
    fn emits_array_helpers_for_all_v0_1_primitive_elements() {
        let elements = vec![
            ValueType::String,
            ValueType::Int,
            ValueType::I32,
            ValueType::U32,
            ValueType::U64,
            ValueType::Float,
            ValueType::Char,
            ValueType::Bool,
        ];
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: vec!["std.array".to_string()],
            structs: Vec::new(),
            enums: vec![EnumType {
                package: "app.main".to_string(),
                name: "Option".to_string(),
                type_params: vec!["T".to_string()],
                variants: vec![
                    EnumVariantType {
                        name: "Some".to_string(),
                        payload: Some(ValueType::TypeParam("T".to_string())),
                    },
                    EnumVariantType {
                        name: "None".to_string(),
                        payload: None,
                    },
                ],
            }],
            functions: vec![Function {
                package: "app.main".to_string(),
                name: "main".to_string(),
                params: Vec::new(),
                return_type: ValueType::Void,
                body: elements
                    .iter()
                    .map(|element_type| Statement::Let {
                        name: format!("items_{}", c_type_name_part(element_type)),
                        value_type: ValueType::Array(Box::new(element_type.clone())),
                        initializer: ValueExpr::ArrayNew {
                            element_type: element_type.clone(),
                        },
                    })
                    .collect(),
            }],
        };

        let c = emit_c(&program);
        for (element_type, c_data_type) in [
            (ValueType::String, "nomo_string"),
            (ValueType::Int, "long long"),
            (ValueType::I32, "int32_t"),
            (ValueType::U32, "uint32_t"),
            (ValueType::U64, "uint64_t"),
            (ValueType::Float, "double"),
            (ValueType::Char, "uint32_t"),
            (ValueType::Bool, "int"),
        ] {
            let array = c_array_ident(&element_type);
            assert!(c.contains(&format!("typedef struct {array}")));
            assert!(c.contains(&format!("{c_data_type} *data;")));
            assert!(c.contains(&format!("static {array} {array}_new(void)")));
        }
    }

    #[test]
    fn emits_if_expression_and_comparison() {
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: vec!["std.io".to_string()],
            structs: Vec::new(),
            enums: Vec::new(),
            functions: vec![
                Function {
                    package: "app.main".to_string(),
                    name: "label".to_string(),
                    params: vec![Parameter {
                        name: "score".to_string(),
                        mutable: false,
                        value_type: ValueType::Int,
                    }],
                    return_type: ValueType::String,
                    body: vec![Statement::Return(Some(ValueExpr::If {
                        condition: Box::new(ValueExpr::Binary {
                            left: Box::new(ValueExpr::Variable("score".to_string())),
                            op: BinaryOp::GreaterEqual,
                            right: Box::new(ValueExpr::IntLiteral(60)),
                        }),
                        then_branch: Box::new(ValueExpr::StringLiteral("pass".to_string())),
                        else_branch: Box::new(ValueExpr::StringLiteral("fail".to_string())),
                    }))],
                },
                Function {
                    package: "app.main".to_string(),
                    name: "main".to_string(),
                    params: Vec::new(),
                    return_type: ValueType::Void,
                    body: vec![Statement::Println(ValueExpr::StringLiteral(
                        "done".to_string(),
                    ))],
                },
            ],
        };

        let c = emit_c(&program);
        assert!(c.contains(
            "return ((nomo_score >= 60) ? nomo_string_literal(\"pass\") : nomo_string_literal(\"fail\"));"
        ));
    }

    #[test]
    fn emits_string_equality_with_runtime_compare() {
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: vec!["std.io".to_string()],
            structs: Vec::new(),
            enums: Vec::new(),
            functions: vec![Function {
                package: "app.main".to_string(),
                name: "main".to_string(),
                params: Vec::new(),
                return_type: ValueType::Void,
                body: vec![Statement::Let {
                    name: "same".to_string(),
                    value_type: ValueType::Bool,
                    initializer: ValueExpr::StringCompare {
                        left: Box::new(ValueExpr::StringLiteral("nomo".to_string())),
                        op: BinaryOp::Equal,
                        right: Box::new(ValueExpr::StringLiteral("nomo".to_string())),
                    },
                }],
            }],
        };

        let c = emit_c(&program);
        assert!(c.contains("static int nomo_string_equal(nomo_string left, nomo_string right)"));
        assert!(c.contains(
            "int nomo_same = (nomo_string_equal(nomo_string_literal(\"nomo\"), nomo_string_literal(\"nomo\")));"
        ));
    }

    #[test]
    fn emits_panic_statement_and_expression() {
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: vec!["std.io".to_string()],
            structs: Vec::new(),
            enums: Vec::new(),
            functions: vec![
                Function {
                    package: "app.main".to_string(),
                    name: "label".to_string(),
                    params: vec![Parameter {
                        name: "ok".to_string(),
                        mutable: false,
                        value_type: ValueType::Bool,
                    }],
                    return_type: ValueType::String,
                    body: vec![Statement::Return(Some(ValueExpr::If {
                        condition: Box::new(ValueExpr::Variable("ok".to_string())),
                        then_branch: Box::new(ValueExpr::StringLiteral("yes".to_string())),
                        else_branch: Box::new(ValueExpr::Panic {
                            message: Box::new(ValueExpr::StringLiteral("no".to_string())),
                            fallback_type: ValueType::String,
                        }),
                    }))],
                },
                Function {
                    package: "app.main".to_string(),
                    name: "main".to_string(),
                    params: Vec::new(),
                    return_type: ValueType::Void,
                    body: vec![Statement::Panic(ValueExpr::StringLiteral(
                        "boom".to_string(),
                    ))],
                },
            ],
        };

        let c = emit_c(&program);
        assert!(c.contains("static void nomo_panic"));
        assert!(c.contains(&panic_literal("boom")));
        assert!(c.contains(
            "(nomo_panic((nomo_string_literal(\"no\")).data), nomo_string_literal(\"\"))"
        ));
    }

    #[test]
    fn emits_binary_arithmetic_operators() {
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: Vec::new(),
            structs: Vec::new(),
            enums: Vec::new(),
            functions: vec![
                Function {
                    package: "app.main".to_string(),
                    name: "calc".to_string(),
                    params: vec![
                        Parameter {
                            name: "a".to_string(),
                            mutable: false,
                            value_type: ValueType::Int,
                        },
                        Parameter {
                            name: "b".to_string(),
                            mutable: false,
                            value_type: ValueType::Int,
                        },
                        Parameter {
                            name: "c".to_string(),
                            mutable: false,
                            value_type: ValueType::Int,
                        },
                    ],
                    return_type: ValueType::Int,
                    body: vec![Statement::Return(Some(ValueExpr::Binary {
                        left: Box::new(ValueExpr::Binary {
                            left: Box::new(ValueExpr::Variable("a".to_string())),
                            op: BinaryOp::Subtract,
                            right: Box::new(ValueExpr::Variable("b".to_string())),
                        }),
                        op: BinaryOp::Remainder,
                        right: Box::new(ValueExpr::Binary {
                            left: Box::new(ValueExpr::Binary {
                                left: Box::new(ValueExpr::Variable("c".to_string())),
                                op: BinaryOp::Multiply,
                                right: Box::new(ValueExpr::IntLiteral(4)),
                            }),
                            op: BinaryOp::Divide,
                            right: Box::new(ValueExpr::IntLiteral(2)),
                        }),
                    }))],
                },
                Function {
                    package: "app.main".to_string(),
                    name: "main".to_string(),
                    params: Vec::new(),
                    return_type: ValueType::Void,
                    body: Vec::new(),
                },
            ],
        };

        let c = emit_c(&program);

        assert!(c.contains(" - "));
        assert!(c.contains(" * "));
        assert!(c.contains(" / "));
        assert!(c.contains(" % "));
    }

    #[test]
    fn emits_logical_operators() {
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: Vec::new(),
            structs: Vec::new(),
            enums: Vec::new(),
            functions: vec![
                Function {
                    package: "app.main".to_string(),
                    name: "check".to_string(),
                    params: vec![
                        Parameter {
                            name: "a".to_string(),
                            mutable: false,
                            value_type: ValueType::Bool,
                        },
                        Parameter {
                            name: "b".to_string(),
                            mutable: false,
                            value_type: ValueType::Bool,
                        },
                    ],
                    return_type: ValueType::Bool,
                    body: vec![Statement::Return(Some(ValueExpr::Binary {
                        left: Box::new(ValueExpr::Unary {
                            op: UnaryOp::Not,
                            expr: Box::new(ValueExpr::Variable("a".to_string())),
                        }),
                        op: BinaryOp::LogicalOr,
                        right: Box::new(ValueExpr::Binary {
                            left: Box::new(ValueExpr::Variable("a".to_string())),
                            op: BinaryOp::LogicalAnd,
                            right: Box::new(ValueExpr::Variable("b".to_string())),
                        }),
                    }))],
                },
                Function {
                    package: "app.main".to_string(),
                    name: "main".to_string(),
                    params: Vec::new(),
                    return_type: ValueType::Void,
                    body: Vec::new(),
                },
            ],
        };

        let c = emit_c(&program);

        assert!(c.contains("!"));
        assert!(c.contains(" || "));
        assert!(c.contains(" && "));
    }

    #[test]
    fn emits_defer_before_panic_statement() {
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: vec!["std.io".to_string()],
            structs: Vec::new(),
            enums: Vec::new(),
            functions: vec![
                Function {
                    package: "app.main".to_string(),
                    name: "cleanup".to_string(),
                    params: Vec::new(),
                    return_type: ValueType::Void,
                    body: vec![Statement::Println(ValueExpr::StringLiteral(
                        "cleanup".to_string(),
                    ))],
                },
                Function {
                    package: "app.main".to_string(),
                    name: "main".to_string(),
                    params: Vec::new(),
                    return_type: ValueType::Void,
                    body: vec![
                        Statement::Defer {
                            call: DeferredCall::Expr(ValueExpr::Call {
                                name: "cleanup".to_string(),
                                args: Vec::new(),
                            }),
                        },
                        Statement::Panic(ValueExpr::StringLiteral("boom".to_string())),
                    ],
                },
            ],
        };

        let c = emit_c(&program);
        let cleanup = c.find("nomo_fn_cleanup();").unwrap();
        let panic = c.find(&panic_literal("boom")).unwrap();
        assert!(cleanup < panic);
        assert_eq!(c.matches("nomo_fn_cleanup();").count(), 1);
    }

    #[test]
    fn emits_defer_at_fallthrough_function_exit() {
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: vec!["std.io".to_string()],
            structs: Vec::new(),
            enums: Vec::new(),
            functions: vec![
                Function {
                    package: "app.main".to_string(),
                    name: "cleanup".to_string(),
                    params: Vec::new(),
                    return_type: ValueType::Void,
                    body: vec![Statement::Println(ValueExpr::StringLiteral(
                        "cleanup".to_string(),
                    ))],
                },
                Function {
                    package: "app.main".to_string(),
                    name: "main".to_string(),
                    params: Vec::new(),
                    return_type: ValueType::Void,
                    body: vec![
                        Statement::Defer {
                            call: DeferredCall::Expr(ValueExpr::Call {
                                name: "cleanup".to_string(),
                                args: Vec::new(),
                            }),
                        },
                        Statement::Println(ValueExpr::StringLiteral("working".to_string())),
                    ],
                },
            ],
        };

        let c = emit_c(&program);
        let working = c.find(&puts_literal("working")).unwrap();
        let cleanup = c.find("nomo_fn_cleanup();").unwrap();
        assert!(working < cleanup);
        assert_eq!(c.matches("nomo_fn_cleanup();").count(), 1);
    }

    #[test]
    fn emits_deferred_println_at_fallthrough_exit() {
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: vec!["std.io".to_string()],
            structs: Vec::new(),
            enums: Vec::new(),
            functions: vec![Function {
                package: "app.main".to_string(),
                name: "main".to_string(),
                params: Vec::new(),
                return_type: ValueType::Void,
                body: vec![
                    Statement::Defer {
                        call: DeferredCall::Println(ValueExpr::StringLiteral(
                            "cleanup".to_string(),
                        )),
                    },
                    Statement::Println(ValueExpr::StringLiteral("working".to_string())),
                ],
            }],
        };

        let c = emit_c(&program);
        let working = c.find(&puts_literal("working")).unwrap();
        let cleanup = c.find(&puts_literal("cleanup")).unwrap();
        assert!(working < cleanup);
    }

    #[test]
    fn emits_nested_block_defer_at_block_fallthrough_exit() {
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: vec!["std.io".to_string()],
            structs: Vec::new(),
            enums: vec![EnumType {
                package: "app.main".to_string(),
                name: "Color".to_string(),
                type_params: Vec::new(),
                variants: vec![
                    EnumVariantType {
                        name: "Red".to_string(),
                        payload: None,
                    },
                    EnumVariantType {
                        name: "Blue".to_string(),
                        payload: None,
                    },
                ],
            }],
            functions: vec![Function {
                package: "app.main".to_string(),
                name: "main".to_string(),
                params: Vec::new(),
                return_type: ValueType::Void,
                body: vec![
                    Statement::Defer {
                        call: DeferredCall::Println(ValueExpr::StringLiteral("outer".to_string())),
                    },
                    Statement::Match {
                        value: ValueExpr::EnumVariant {
                            enum_name: "Color".to_string(),
                            enum_args: Vec::new(),
                            variant: "Red".to_string(),
                            payload: None,
                        },
                        enum_name: "Color".to_string(),
                        enum_args: Vec::new(),
                        arms: vec![
                            MatchStatementArm {
                                variant: "Red".to_string(),
                                binding: None,
                                body: vec![
                                    Statement::Defer {
                                        call: DeferredCall::Println(ValueExpr::StringLiteral(
                                            "inner".to_string(),
                                        )),
                                    },
                                    Statement::Println(ValueExpr::StringLiteral("red".to_string())),
                                ],
                            },
                            MatchStatementArm {
                                variant: "Blue".to_string(),
                                binding: None,
                                body: vec![Statement::Println(ValueExpr::StringLiteral(
                                    "blue".to_string(),
                                ))],
                            },
                        ],
                    },
                    Statement::Println(ValueExpr::StringLiteral("after".to_string())),
                ],
            }],
        };

        let c = emit_c(&program);
        let red = c.find(&puts_literal("red")).unwrap();
        let inner = c[red..].find(&puts_literal("inner")).unwrap() + red;
        let after = c[inner..].find(&puts_literal("after")).unwrap() + inner;
        let outer = c[after..].find(&puts_literal("outer")).unwrap() + after;
        assert!(red < inner);
        assert!(inner < after);
        assert!(after < outer);
        assert_eq!(c.matches(&puts_literal("inner")).count(), 1);
        assert_eq!(c.matches(&puts_literal("outer")).count(), 1);
    }

    #[test]
    fn emits_nested_block_defer_before_return_and_outer_defer() {
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: vec!["std.io".to_string()],
            structs: Vec::new(),
            enums: vec![EnumType {
                package: "app.main".to_string(),
                name: "Color".to_string(),
                type_params: Vec::new(),
                variants: vec![
                    EnumVariantType {
                        name: "Red".to_string(),
                        payload: None,
                    },
                    EnumVariantType {
                        name: "Blue".to_string(),
                        payload: None,
                    },
                ],
            }],
            functions: vec![Function {
                package: "app.main".to_string(),
                name: "main".to_string(),
                params: Vec::new(),
                return_type: ValueType::Void,
                body: vec![
                    Statement::Defer {
                        call: DeferredCall::Println(ValueExpr::StringLiteral("outer".to_string())),
                    },
                    Statement::Match {
                        value: ValueExpr::EnumVariant {
                            enum_name: "Color".to_string(),
                            enum_args: Vec::new(),
                            variant: "Red".to_string(),
                            payload: None,
                        },
                        enum_name: "Color".to_string(),
                        enum_args: Vec::new(),
                        arms: vec![
                            MatchStatementArm {
                                variant: "Red".to_string(),
                                binding: None,
                                body: vec![
                                    Statement::Defer {
                                        call: DeferredCall::Println(ValueExpr::StringLiteral(
                                            "inner".to_string(),
                                        )),
                                    },
                                    Statement::Return(None),
                                ],
                            },
                            MatchStatementArm {
                                variant: "Blue".to_string(),
                                binding: None,
                                body: Vec::new(),
                            },
                        ],
                    },
                ],
            }],
        };

        let c = emit_c(&program);
        let inner = c.find(&puts_literal("inner")).unwrap();
        let outer = c[inner..].find(&puts_literal("outer")).unwrap() + inner;
        let return_stmt = c[outer..].find("return;").unwrap() + outer;
        assert!(inner < outer);
        assert!(outer < return_stmt);
        assert_eq!(c.matches(&puts_literal("inner")).count(), 1);
        assert_eq!(c.matches(&puts_literal("outer")).count(), 2);
    }

    #[test]
    fn emits_loop_defer_before_break_without_function_defer() {
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: vec!["std.io".to_string()],
            structs: Vec::new(),
            enums: Vec::new(),
            functions: vec![Function {
                package: "app.main".to_string(),
                name: "main".to_string(),
                params: Vec::new(),
                return_type: ValueType::Void,
                body: vec![
                    Statement::Defer {
                        call: DeferredCall::Println(ValueExpr::StringLiteral("outer".to_string())),
                    },
                    Statement::Loop {
                        kind: LoopKind::Infinite,
                        body: vec![
                            Statement::Defer {
                                call: DeferredCall::Println(ValueExpr::StringLiteral(
                                    "inner".to_string(),
                                )),
                            },
                            Statement::Break,
                        ],
                    },
                ],
            }],
        };

        let c = emit_c(&program);
        let inner = c.find(&puts_literal("inner")).unwrap();
        let break_stmt = c[inner..].find("break;").unwrap() + inner;
        let outer = c[break_stmt..].find(&puts_literal("outer")).unwrap() + break_stmt;
        assert!(inner < break_stmt);
        assert!(break_stmt < outer);
        assert_eq!(c.matches(&puts_literal("inner")).count(), 1);
        assert_eq!(c.matches(&puts_literal("outer")).count(), 1);
    }

    #[test]
    fn emits_loop_defer_before_continue_without_function_defer() {
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: vec!["std.io".to_string()],
            structs: Vec::new(),
            enums: Vec::new(),
            functions: vec![Function {
                package: "app.main".to_string(),
                name: "main".to_string(),
                params: Vec::new(),
                return_type: ValueType::Void,
                body: vec![
                    Statement::Defer {
                        call: DeferredCall::Println(ValueExpr::StringLiteral("outer".to_string())),
                    },
                    Statement::Loop {
                        kind: LoopKind::Infinite,
                        body: vec![
                            Statement::Defer {
                                call: DeferredCall::Println(ValueExpr::StringLiteral(
                                    "inner".to_string(),
                                )),
                            },
                            Statement::Continue,
                        ],
                    },
                ],
            }],
        };

        let c = emit_c(&program);
        let inner = c.find(&puts_literal("inner")).unwrap();
        let continue_stmt = c[inner..].find("continue;").unwrap() + inner;
        let outer = c[continue_stmt..].find(&puts_literal("outer")).unwrap() + continue_stmt;
        assert!(inner < continue_stmt);
        assert!(continue_stmt < outer);
        assert_eq!(c.matches(&puts_literal("inner")).count(), 1);
        assert_eq!(c.matches(&puts_literal("outer")).count(), 1);
    }

    #[test]
    fn inner_loop_break_only_runs_inner_loop_defer() {
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: vec!["std.io".to_string()],
            structs: Vec::new(),
            enums: Vec::new(),
            functions: vec![Function {
                package: "app.main".to_string(),
                name: "main".to_string(),
                params: Vec::new(),
                return_type: ValueType::Void,
                body: vec![Statement::Loop {
                    kind: LoopKind::Infinite,
                    body: vec![
                        Statement::Defer {
                            call: DeferredCall::Println(ValueExpr::StringLiteral(
                                "outer loop".to_string(),
                            )),
                        },
                        Statement::Loop {
                            kind: LoopKind::Infinite,
                            body: vec![
                                Statement::Defer {
                                    call: DeferredCall::Println(ValueExpr::StringLiteral(
                                        "inner loop".to_string(),
                                    )),
                                },
                                Statement::Break,
                            ],
                        },
                        Statement::Break,
                    ],
                }],
            }],
        };

        let c = emit_c(&program);
        let inner = c.find(&puts_literal("inner loop")).unwrap();
        let inner_break = c[inner..].find("break;").unwrap() + inner;
        let outer = c[inner_break..].find(&puts_literal("outer loop")).unwrap() + inner_break;
        let outer_break = c[outer..].find("break;").unwrap() + outer;
        assert!(inner < inner_break);
        assert!(inner_break < outer);
        assert!(outer < outer_break);
        assert_eq!(c.matches(&puts_literal("inner loop")).count(), 1);
        assert_eq!(c.matches(&puts_literal("outer loop")).count(), 1);
    }

    #[test]
    fn emits_return_value_before_deferred_calls() {
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: vec!["std.io".to_string()],
            structs: Vec::new(),
            enums: Vec::new(),
            functions: vec![
                Function {
                    package: "app.main".to_string(),
                    name: "cleanup".to_string(),
                    params: Vec::new(),
                    return_type: ValueType::Void,
                    body: vec![Statement::Println(ValueExpr::StringLiteral(
                        "cleanup".to_string(),
                    ))],
                },
                Function {
                    package: "app.main".to_string(),
                    name: "value".to_string(),
                    params: Vec::new(),
                    return_type: ValueType::Int,
                    body: vec![Statement::Return(Some(ValueExpr::IntLiteral(7)))],
                },
                Function {
                    package: "app.main".to_string(),
                    name: "compute".to_string(),
                    params: Vec::new(),
                    return_type: ValueType::Int,
                    body: vec![
                        Statement::Defer {
                            call: DeferredCall::Expr(ValueExpr::Call {
                                name: "cleanup".to_string(),
                                args: Vec::new(),
                            }),
                        },
                        Statement::Return(Some(ValueExpr::Call {
                            name: "value".to_string(),
                            args: Vec::new(),
                        })),
                    ],
                },
                Function {
                    package: "app.main".to_string(),
                    name: "main".to_string(),
                    params: Vec::new(),
                    return_type: ValueType::Void,
                    body: vec![Statement::Println(ValueExpr::StringLiteral(
                        "done".to_string(),
                    ))],
                },
            ],
        };

        let c = emit_c(&program);
        let value = c.find("long long nomo__return = nomo_fn_value();").unwrap();
        let cleanup = c.find("nomo_fn_cleanup();").unwrap();
        let return_value = c.find("return nomo__return;").unwrap();
        assert!(value < cleanup);
        assert!(cleanup < return_value);
    }

    #[test]
    fn emits_assignment() {
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: vec!["std.io".to_string()],
            structs: Vec::new(),
            enums: Vec::new(),
            functions: vec![Function {
                package: "app.main".to_string(),
                name: "main".to_string(),
                params: Vec::new(),
                return_type: ValueType::Void,
                body: vec![
                    Statement::Let {
                        name: "count".to_string(),
                        value_type: ValueType::Int,
                        initializer: ValueExpr::IntLiteral(1),
                    },
                    Statement::Assign {
                        name: "count".to_string(),
                        value: ValueExpr::Binary {
                            left: Box::new(ValueExpr::Variable("count".to_string())),
                            op: BinaryOp::Add,
                            right: Box::new(ValueExpr::IntLiteral(1)),
                        },
                    },
                ],
            }],
        };

        let c = emit_c(&program);
        assert!(c.contains("long long nomo_count = 1;"));
        assert!(c.contains("nomo_count = (nomo_count + 1);"));
    }

    #[test]
    fn emits_field_assignment() {
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: vec!["std.io".to_string()],
            structs: vec![StructType {
                package: "app.main".to_string(),
                name: "Counter".to_string(),
                type_params: Vec::new(),
                fields: vec![StructField {
                    name: "value".to_string(),
                    value_type: ValueType::Int,
                }],
            }],
            enums: Vec::new(),
            functions: vec![Function {
                package: "app.main".to_string(),
                name: "main".to_string(),
                params: Vec::new(),
                return_type: ValueType::Void,
                body: vec![
                    Statement::Let {
                        name: "counter".to_string(),
                        value_type: ValueType::Struct("Counter".to_string(), Vec::new()),
                        initializer: ValueExpr::StructLiteral {
                            type_name: "Counter".to_string(),
                            struct_args: Vec::new(),
                            fields: vec![("value".to_string(), ValueExpr::IntLiteral(1))],
                        },
                    },
                    Statement::AssignField {
                        base: "counter".to_string(),
                        field: "value".to_string(),
                        value_type: ValueType::Int,
                        value: ValueExpr::Binary {
                            left: Box::new(ValueExpr::FieldAccess {
                                base: "counter".to_string(),
                                field: "value".to_string(),
                            }),
                            op: BinaryOp::Add,
                            right: Box::new(ValueExpr::IntLiteral(1)),
                        },
                    },
                ],
            }],
        };

        let c = emit_c(&program);
        assert!(
            c.contains("nomo_counter.nomo_member_value = (nomo_counter.nomo_member_value + 1);")
        );
    }

    #[test]
    fn emits_struct_type_literal_and_field_access() {
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: vec!["std.io".to_string()],
            structs: vec![StructType {
                package: "app.main".to_string(),
                name: "Point".to_string(),
                type_params: Vec::new(),
                fields: vec![
                    StructField {
                        name: "x".to_string(),
                        value_type: ValueType::Int,
                    },
                    StructField {
                        name: "y".to_string(),
                        value_type: ValueType::Int,
                    },
                ],
            }],
            enums: Vec::new(),
            functions: vec![Function {
                package: "app.main".to_string(),
                name: "main".to_string(),
                params: Vec::new(),
                return_type: ValueType::Void,
                body: vec![
                    Statement::Let {
                        name: "point".to_string(),
                        value_type: ValueType::Struct("Point".to_string(), Vec::new()),
                        initializer: ValueExpr::StructLiteral {
                            type_name: "Point".to_string(),
                            struct_args: Vec::new(),
                            fields: vec![
                                ("x".to_string(), ValueExpr::IntLiteral(1)),
                                ("y".to_string(), ValueExpr::IntLiteral(2)),
                            ],
                        },
                    },
                    Statement::Let {
                        name: "x".to_string(),
                        value_type: ValueType::Int,
                        initializer: ValueExpr::FieldAccess {
                            base: "point".to_string(),
                            field: "x".to_string(),
                        },
                    },
                ],
            }],
        };

        let c = emit_c(&program);
        assert!(c.contains("typedef struct nomo_struct_Point"));
        assert!(c.contains(
            "nomo_struct_Point nomo_point = (nomo_struct_Point){.nomo_member_x = 1, .nomo_member_y = 2};"
        ));
        assert!(c.contains("long long nomo_x = nomo_point.nomo_member_x;"));
    }

    #[test]
    fn emits_generic_struct_instance() {
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: vec!["std.io".to_string()],
            structs: vec![StructType {
                package: "app.main".to_string(),
                name: "Box".to_string(),
                type_params: vec!["T".to_string()],
                fields: vec![StructField {
                    name: "value".to_string(),
                    value_type: ValueType::TypeParam("T".to_string()),
                }],
            }],
            enums: Vec::new(),
            functions: vec![Function {
                package: "app.main".to_string(),
                name: "main".to_string(),
                params: Vec::new(),
                return_type: ValueType::Void,
                body: vec![Statement::Let {
                    name: "item".to_string(),
                    value_type: ValueType::Struct("Box".to_string(), vec![ValueType::I32]),
                    initializer: ValueExpr::StructLiteral {
                        type_name: "Box".to_string(),
                        struct_args: vec![ValueType::I32],
                        fields: vec![("value".to_string(), ValueExpr::IntLiteral(7))],
                    },
                }],
            }],
        };

        let c = emit_c(&program);
        assert!(c.contains("typedef struct nomo_struct_Box_i32"));
        assert!(c.contains("int32_t nomo_member_value;"));
        assert!(c.contains(
            "nomo_struct_Box_i32 nomo_item = (nomo_struct_Box_i32){.nomo_member_value = 7};"
        ));
    }

    #[test]
    fn emits_enum_variant_and_match_expression() {
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: vec!["std.io".to_string()],
            structs: Vec::new(),
            enums: vec![EnumType {
                package: "app.main".to_string(),
                name: "Color".to_string(),
                type_params: Vec::new(),
                variants: vec![
                    EnumVariantType {
                        name: "Red".to_string(),
                        payload: None,
                    },
                    EnumVariantType {
                        name: "Blue".to_string(),
                        payload: None,
                    },
                ],
            }],
            functions: vec![Function {
                package: "app.main".to_string(),
                name: "main".to_string(),
                params: Vec::new(),
                return_type: ValueType::Void,
                body: vec![
                    Statement::Let {
                        name: "color".to_string(),
                        value_type: ValueType::Enum("Color".to_string(), Vec::new()),
                        initializer: ValueExpr::EnumVariant {
                            enum_name: "Color".to_string(),
                            enum_args: Vec::new(),
                            variant: "Red".to_string(),
                            payload: None,
                        },
                    },
                    Statement::Let {
                        name: "label".to_string(),
                        value_type: ValueType::String,
                        initializer: ValueExpr::Match {
                            value: Box::new(ValueExpr::Variable("color".to_string())),
                            arms: vec![
                                MatchValueArm {
                                    enum_name: "Color".to_string(),
                                    enum_args: Vec::new(),
                                    variant: "Red".to_string(),
                                    binding: None,
                                    value: ValueExpr::StringLiteral("red".to_string()),
                                },
                                MatchValueArm {
                                    enum_name: "Color".to_string(),
                                    enum_args: Vec::new(),
                                    variant: "Blue".to_string(),
                                    binding: None,
                                    value: ValueExpr::StringLiteral("blue".to_string()),
                                },
                            ],
                        },
                    },
                ],
            }],
        };

        let c = emit_c(&program);
        assert!(c.contains("typedef enum nomo_enum_Color_tag"));
        assert!(c.contains(
            "nomo_enum_Color nomo_color = (nomo_enum_Color){.tag = nomo_enum_Color_Red};"
        ));
        assert!(c.contains(
            "nomo_color.tag == nomo_enum_Color_Red ? nomo_string_literal(\"red\") : nomo_string_literal(\"blue\")"
        ));
    }

    #[test]
    fn emits_payload_enum_and_match_binding_access() {
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: vec!["std.io".to_string()],
            structs: Vec::new(),
            enums: vec![EnumType {
                package: "app.main".to_string(),
                name: "MaybeInt".to_string(),
                type_params: Vec::new(),
                variants: vec![
                    EnumVariantType {
                        name: "Some".to_string(),
                        payload: Some(ValueType::Int),
                    },
                    EnumVariantType {
                        name: "None".to_string(),
                        payload: None,
                    },
                ],
            }],
            functions: vec![Function {
                package: "app.main".to_string(),
                name: "main".to_string(),
                params: Vec::new(),
                return_type: ValueType::Void,
                body: vec![
                    Statement::Let {
                        name: "value".to_string(),
                        value_type: ValueType::Enum("MaybeInt".to_string(), Vec::new()),
                        initializer: ValueExpr::EnumVariant {
                            enum_name: "MaybeInt".to_string(),
                            enum_args: Vec::new(),
                            variant: "Some".to_string(),
                            payload: Some(Box::new(ValueExpr::IntLiteral(41))),
                        },
                    },
                    Statement::Let {
                        name: "answer".to_string(),
                        value_type: ValueType::Int,
                        initializer: ValueExpr::Match {
                            value: Box::new(ValueExpr::Variable("value".to_string())),
                            arms: vec![
                                MatchValueArm {
                                    enum_name: "MaybeInt".to_string(),
                                    enum_args: Vec::new(),
                                    variant: "Some".to_string(),
                                    binding: Some("n".to_string()),
                                    value: ValueExpr::EnumPayload {
                                        value: Box::new(ValueExpr::Variable("value".to_string())),
                                        variant: "Some".to_string(),
                                    },
                                },
                                MatchValueArm {
                                    enum_name: "MaybeInt".to_string(),
                                    enum_args: Vec::new(),
                                    variant: "None".to_string(),
                                    binding: None,
                                    value: ValueExpr::IntLiteral(0),
                                },
                            ],
                        },
                    },
                ],
            }],
        };

        let c = emit_c(&program);
        assert!(c.contains("union"));
        assert!(c.contains("long long nomo_payload_Some;"));
        assert!(c.contains(".payload.nomo_payload_Some = 41"));
        assert!(c.contains("nomo_value.payload.nomo_payload_Some"));
    }

    #[test]
    fn emits_void_enum_payload_as_unit_storage() {
        let result_void_string = ValueType::Enum(
            "Result".to_string(),
            vec![ValueType::Void, ValueType::String],
        );
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: vec!["std.io".to_string()],
            structs: Vec::new(),
            enums: vec![EnumType {
                package: "app.main".to_string(),
                name: "Result".to_string(),
                type_params: vec!["T".to_string(), "E".to_string()],
                variants: vec![
                    EnumVariantType {
                        name: "Ok".to_string(),
                        payload: Some(ValueType::TypeParam("T".to_string())),
                    },
                    EnumVariantType {
                        name: "Err".to_string(),
                        payload: Some(ValueType::TypeParam("E".to_string())),
                    },
                ],
            }],
            functions: vec![
                Function {
                    package: "app.main".to_string(),
                    name: "write".to_string(),
                    params: Vec::new(),
                    return_type: result_void_string.clone(),
                    body: vec![Statement::Return(Some(ValueExpr::EnumVariant {
                        enum_name: "Result".to_string(),
                        enum_args: vec![ValueType::Void, ValueType::String],
                        variant: "Ok".to_string(),
                        payload: Some(Box::new(ValueExpr::VoidLiteral)),
                    }))],
                },
                Function {
                    package: "app.main".to_string(),
                    name: "main".to_string(),
                    params: Vec::new(),
                    return_type: ValueType::Void,
                    body: vec![Statement::Println(ValueExpr::StringLiteral(
                        "done".to_string(),
                    ))],
                },
            ],
        };

        let c = emit_c(&program);
        assert!(c.contains("char nomo_payload_Ok;"));
        assert!(!c.contains("void nomo_payload_Ok;"));
        assert!(c.contains(".payload.nomo_payload_Ok = 0"));
    }

    #[test]
    fn emits_result_try_let_early_return() {
        let result_i64_string = ValueType::Enum(
            "Result".to_string(),
            vec![ValueType::Int, ValueType::String],
        );
        let program = Program {
            consts: Vec::new(),
            package: "app.main".to_string(),
            imports: vec!["std.io".to_string()],
            structs: Vec::new(),
            enums: vec![EnumType {
                package: "app.main".to_string(),
                name: "Result".to_string(),
                type_params: vec!["T".to_string(), "E".to_string()],
                variants: vec![
                    EnumVariantType {
                        name: "Ok".to_string(),
                        payload: Some(ValueType::TypeParam("T".to_string())),
                    },
                    EnumVariantType {
                        name: "Err".to_string(),
                        payload: Some(ValueType::TypeParam("E".to_string())),
                    },
                ],
            }],
            functions: vec![
                Function {
                    package: "app.main".to_string(),
                    name: "parse".to_string(),
                    params: Vec::new(),
                    return_type: result_i64_string.clone(),
                    body: vec![Statement::Return(Some(ValueExpr::EnumVariant {
                        enum_name: "Result".to_string(),
                        enum_args: vec![ValueType::Int, ValueType::String],
                        variant: "Ok".to_string(),
                        payload: Some(Box::new(ValueExpr::IntLiteral(41))),
                    }))],
                },
                Function {
                    package: "app.main".to_string(),
                    name: "compute".to_string(),
                    params: Vec::new(),
                    return_type: result_i64_string.clone(),
                    body: vec![
                        Statement::TryLet {
                            name: "value".to_string(),
                            value_type: ValueType::Int,
                            result_type: result_i64_string.clone(),
                            return_type: result_i64_string,
                            result_expr: ValueExpr::Call {
                                name: "parse".to_string(),
                                args: Vec::new(),
                            },
                        },
                        Statement::Return(Some(ValueExpr::EnumVariant {
                            enum_name: "Result".to_string(),
                            enum_args: vec![ValueType::Int, ValueType::String],
                            variant: "Ok".to_string(),
                            payload: Some(Box::new(ValueExpr::Binary {
                                left: Box::new(ValueExpr::Variable("value".to_string())),
                                op: BinaryOp::Add,
                                right: Box::new(ValueExpr::IntLiteral(1)),
                            })),
                        })),
                    ],
                },
                Function {
                    package: "app.main".to_string(),
                    name: "main".to_string(),
                    params: Vec::new(),
                    return_type: ValueType::Void,
                    body: vec![Statement::Println(ValueExpr::StringLiteral(
                        "done".to_string(),
                    ))],
                },
            ],
        };

        let c = emit_c(&program);
        assert!(c.contains("nomo_enum_Result_i64_string nomo_value_result = nomo_fn_parse();"));
        assert!(c.contains("if (nomo_value_result.tag == nomo_enum_Result_i64_string_Err) {"));
        assert!(c.contains(
            "nomo_enum_Result_i64_string nomo__try_return = (nomo_enum_Result_i64_string){.tag = nomo_enum_Result_i64_string_Err, .payload.nomo_payload_Err = nomo_value_result.payload.nomo_payload_Err};"
        ));
        assert!(c.contains("return nomo__try_return;"));
        assert!(c.contains("long long nomo_value = nomo_value_result.payload.nomo_payload_Ok;"));
    }
}
