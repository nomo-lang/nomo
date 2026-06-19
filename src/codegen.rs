use crate::compiler::{
    BinaryOp, EnumType, Function, Program, Statement, StructType, ValueExpr, ValueType,
};
use std::collections::BTreeSet;

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
    out.push_str("static char *nomo_string_concat(const char *left, const char *right) {\n");
    out.push_str("    size_t left_len = strlen(left);\n");
    out.push_str("    size_t right_len = strlen(right);\n");
    out.push_str("    char *out = (char *)malloc(left_len + right_len + 1);\n");
    out.push_str("    if (out == NULL) {\n");
    out.push_str("        nomo_panic(\"out of memory\");\n");
    out.push_str("    }\n");
    out.push_str("    memcpy(out, left, left_len);\n");
    out.push_str("    memcpy(out + left_len, right, right_len + 1);\n");
    out.push_str("    return out;\n");
    out.push_str("}\n\n");

    for (struct_name, struct_args) in collect_struct_instances(program) {
        let struct_type = program
            .structs
            .iter()
            .find(|item| item.name == struct_name)
            .expect("checked programs only use known structs");
        emit_struct_type(&mut out, struct_type, &struct_args);
        out.push('\n');
    }
    for (enum_name, enum_args) in collect_enum_instances(program) {
        let enum_type = program
            .enums
            .iter()
            .find(|item| item.name == enum_name)
            .expect("checked programs only use known enums");
        emit_enum_type(&mut out, enum_type, &enum_args);
        out.push('\n');
    }
    let array_element_types = collect_array_element_types(program);
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
    if uses_env_get(program) {
        emit_env_get_helper(&mut out);
        out.push('\n');
    }

    for function in program
        .functions
        .iter()
        .filter(|function| function.name != "main")
    {
        emit_prototype(&mut out, function);
    }
    if program
        .functions
        .iter()
        .any(|function| function.name != "main")
    {
        out.push('\n');
    }

    for function in program
        .functions
        .iter()
        .filter(|function| function.name != "main")
    {
        emit_function(&mut out, function);
        out.push('\n');
    }

    let main = program
        .functions
        .iter()
        .find(|function| function.name == "main")
        .expect("checked programs always contain main");
    if uses_env_args(program) {
        out.push_str("int main(int argc, char **argv) {\n");
    } else {
        out.push_str("int main(void) {\n");
    }
    if uses_env_args(program) {
        out.push_str("    nomo_argc = argc;\n");
        out.push_str("    nomo_argv = argv;\n");
    }
    emit_body(&mut out, main);
    out.push_str("    return 0;\n");
    out.push_str("}\n");
    out
}

fn emit_prototype(out: &mut String, function: &Function) {
    emit_signature(out, function);
    out.push_str(";\n");
}

fn emit_struct_type(out: &mut String, struct_type: &StructType, struct_args: &[ValueType]) {
    out.push_str("typedef struct ");
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
        out.push_str(&field.name);
        out.push_str(";\n");
    }
    out.push_str("} ");
    out.push_str(&c_struct_ident(&struct_type.name, struct_args));
    out.push_str(";\n");
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
    out.push_str("typedef struct ");
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
            out.push_str(&variant.name);
            out.push_str(";\n");
        }
        out.push_str("    } payload;\n");
    }
    out.push_str("} ");
    out.push_str(&c_enum_ident(&enum_type.name, enum_args));
    out.push_str(";\n");
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
    out.push_str(" nomo_fs_read_to_string(const char *path) {\n");
    out.push_str("    FILE *file = fopen(path, \"rb\");\n");
    out.push_str("    if (file == NULL) {\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.Err = (");
    out.push_str(&fs_error);
    out.push_str("){.message = strerror(errno)}};\n");
    out.push_str("    }\n");
    out.push_str("    if (fseek(file, 0, SEEK_END) != 0) {\n");
    out.push_str("        const char *message = strerror(errno);\n");
    out.push_str("        fclose(file);\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.Err = (");
    out.push_str(&fs_error);
    out.push_str("){.message = message}};\n");
    out.push_str("    }\n");
    out.push_str("    long size = ftell(file);\n");
    out.push_str("    if (size < 0 || fseek(file, 0, SEEK_SET) != 0) {\n");
    out.push_str("        const char *message = strerror(errno);\n");
    out.push_str("        fclose(file);\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.Err = (");
    out.push_str(&fs_error);
    out.push_str("){.message = message}};\n");
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
    out.push_str(", .payload.Err = (");
    out.push_str(&fs_error);
    out.push_str("){.message = message}};\n");
    out.push_str("    }\n");
    out.push_str("    buffer[size] = '\\0';\n");
    out.push_str("    fclose(file);\n");
    out.push_str("    return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&ok);
    out.push_str(", .payload.Ok = buffer};\n");
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
    out.push_str(" nomo_fs_write_string(const char *path, const char *content) {\n");
    out.push_str("    FILE *file = fopen(path, \"wb\");\n");
    out.push_str("    if (file == NULL) {\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.Err = (");
    out.push_str(&fs_error);
    out.push_str("){.message = strerror(errno)}};\n");
    out.push_str("    }\n");
    out.push_str("    size_t len = strlen(content);\n");
    out.push_str("    if (fwrite(content, 1, len, file) != len) {\n");
    out.push_str("        const char *message = strerror(errno);\n");
    out.push_str("        fclose(file);\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.Err = (");
    out.push_str(&fs_error);
    out.push_str("){.message = message}};\n");
    out.push_str("    }\n");
    out.push_str("    if (fclose(file) != 0) {\n");
    out.push_str("        return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&err);
    out.push_str(", .payload.Err = (");
    out.push_str(&fs_error);
    out.push_str("){.message = strerror(errno)}};\n");
    out.push_str("    }\n");
    out.push_str("    return (");
    out.push_str(&result);
    out.push_str("){.tag = ");
    out.push_str(&ok);
    out.push_str(", .payload.Ok = 0};\n");
    out.push_str("}\n");
}

fn emit_env_get_helper(out: &mut String) {
    let result = c_enum_ident("Option", &[ValueType::String]);
    let some = c_enum_variant_ident("Option", &[ValueType::String], "Some");
    let none = c_enum_variant_ident("Option", &[ValueType::String], "None");
    out.push_str("static ");
    out.push_str(&result);
    out.push_str(" nomo_env_get(const char *name) {\n");
    out.push_str("    const char *value = getenv(name);\n");
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
    out.push_str(", .payload.Some = value};\n");
    out.push_str("}\n");
}

fn emit_env_args_helper(out: &mut String) {
    out.push_str("static nomo_array_string nomo_env_args(int argc, char **argv) {\n");
    out.push_str("    nomo_array_string args = nomo_array_string_new();\n");
    out.push_str("    for (int i = 0; i < argc; i += 1) {\n");
    out.push_str("        args = nomo_array_string_push(args, argv[i]);\n");
    out.push_str("    }\n");
    out.push_str("    return args;\n");
    out.push_str("}\n");
}

fn emit_array_helpers(out: &mut String, element_type: &ValueType) {
    let array = c_array_ident(element_type);
    let option = c_enum_ident("Option", &[element_type.clone()]);
    let some = c_enum_variant_ident("Option", &[element_type.clone()], "Some");
    let none = c_enum_variant_ident("Option", &[element_type.clone()], "None");
    out.push_str("typedef struct ");
    out.push_str(&array);
    out.push_str(" {\n");
    out.push_str("    size_t len;\n");
    out.push_str("    size_t cap;\n");
    out.push_str("    ");
    out.push_str(&c_type(element_type));
    out.push_str(" *data;\n");
    out.push_str("} ");
    out.push_str(&array);
    out.push_str(";\n\n");
    out.push_str("static ");
    out.push_str(&array);
    out.push(' ');
    out.push_str(&array);
    out.push_str("_new(void) {\n");
    out.push_str("    return (");
    out.push_str(&array);
    out.push_str("){.len = 0, .cap = 0, .data = NULL};\n");
    out.push_str("}\n\n");
    out.push_str("static ");
    out.push_str(&array);
    out.push(' ');
    out.push_str(&array);
    out.push_str("_reserve(");
    out.push_str(&array);
    out.push_str(" array, size_t needed) {\n");
    out.push_str("    if (array.cap >= needed) { return array; }\n");
    out.push_str("    size_t cap = array.cap == 0 ? 4 : array.cap;\n");
    out.push_str("    while (cap < needed) { cap *= 2; }\n");
    out.push_str("    ");
    out.push_str(&c_type(element_type));
    out.push_str(" *data = (");
    out.push_str(&c_type(element_type));
    out.push_str(" *)realloc(array.data, cap * sizeof(");
    out.push_str(&c_type(element_type));
    out.push_str("));\n");
    out.push_str("    if (data == NULL) { nomo_panic(\"out of memory\"); }\n");
    out.push_str("    array.data = data;\n");
    out.push_str("    array.cap = cap;\n");
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
    out.push_str("_reserve(array, array.len + 1);\n");
    out.push_str("    array.data[array.len] = value;\n");
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
    out.push_str(", .payload.Some = array.data[index]};\n");
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
    out.push_str("    array.data[index] = value;\n");
    out.push_str("    return array;\n");
    out.push_str("}\n");
}

fn emit_function(out: &mut String, function: &Function) {
    emit_signature(out, function);
    out.push_str(" {\n");
    emit_body(out, function);
    if function.return_type == ValueType::Void {
        out.push_str("    return;\n");
    }
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
            out.push(' ');
            out.push_str(&c_var_ident(&param.name));
        }
    }
    out.push(')');
}

fn emit_body(out: &mut String, function: &Function) {
    for statement in &function.body {
        match statement {
            Statement::Let {
                name,
                value_type,
                initializer,
            } => emit_let(out, name, value_type, initializer),
            Statement::TryLet {
                name,
                value_type,
                result_type,
                return_type,
                result_expr,
            } => emit_try_let(out, name, value_type, result_type, return_type, result_expr),
            Statement::Assign { name, value } => {
                out.push_str("    ");
                out.push_str(&c_var_ident(name));
                out.push_str(" = ");
                emit_expr(out, value);
                out.push_str(";\n");
            }
            Statement::AssignField { base, field, value } => {
                out.push_str("    ");
                out.push_str(&c_var_ident(base));
                out.push('.');
                out.push_str(field);
                out.push_str(" = ");
                emit_expr(out, value);
                out.push_str(";\n");
            }
            Statement::Println(arg) => {
                out.push_str("    puts(");
                emit_expr(out, arg);
                out.push_str(");\n");
            }
            Statement::Eprintln(arg) => {
                out.push_str("    fputs(");
                emit_expr(out, arg);
                out.push_str(", stderr);\n");
                out.push_str("    fputc('\\n', stderr);\n");
            }
            Statement::Panic(message) => {
                out.push_str("    nomo_panic(");
                emit_expr(out, message);
                out.push_str(");\n");
            }
            Statement::Return(Some(value)) => {
                out.push_str("    return ");
                emit_expr(out, value);
                out.push_str(";\n");
            }
            Statement::Return(None) => out.push_str("    return;\n"),
        }
    }
}

fn emit_let(out: &mut String, name: &str, value_type: &ValueType, initializer: &ValueExpr) {
    out.push_str("    ");
    out.push_str(&c_type(value_type));
    out.push(' ');
    out.push_str(&c_var_ident(name));
    out.push_str(" = ");
    emit_expr(out, initializer);
    out.push_str(";\n");
}

fn emit_try_let(
    out: &mut String,
    name: &str,
    value_type: &ValueType,
    result_type: &ValueType,
    return_type: &ValueType,
    result_expr: &ValueExpr,
) {
    let temp = format!("{}_result", c_var_ident(name));
    out.push_str("    ");
    out.push_str(&c_type(result_type));
    out.push(' ');
    out.push_str(&temp);
    out.push_str(" = ");
    emit_expr(out, result_expr);
    out.push_str(";\n");
    let ValueType::Enum(result_name, result_args) = result_type else {
        return;
    };
    out.push_str("    if (");
    out.push_str(&temp);
    out.push_str(".tag == ");
    out.push_str(&c_enum_variant_ident(result_name, result_args, "Err"));
    out.push_str(") {\n");
    out.push_str("        return (");
    let (return_name, return_args) = match return_type {
        ValueType::Enum(name, args) => (name, args),
        _ => (result_name, result_args),
    };
    out.push_str(&c_enum_ident(return_name, return_args));
    out.push_str("){.tag = ");
    out.push_str(&c_enum_variant_ident(return_name, return_args, "Err"));
    out.push_str(", .payload.Err = ");
    out.push_str(&temp);
    out.push_str(".payload.Err};\n");
    out.push_str("    }\n");
    out.push_str("    ");
    out.push_str(&c_type(value_type));
    out.push(' ');
    out.push_str(&c_var_ident(name));
    out.push_str(" = ");
    out.push_str(&temp);
    out.push_str(".payload.Ok;\n");
}

fn emit_expr(out: &mut String, expr: &ValueExpr) {
    match expr {
        ValueExpr::StringLiteral(value) => {
            out.push('"');
            out.push_str(&escape_c_string(value));
            out.push('"');
        }
        ValueExpr::IntLiteral(value) => out.push_str(&value.to_string()),
        ValueExpr::FloatLiteral(value) => out.push_str(value),
        ValueExpr::CharLiteral(value) => out.push_str(&(*value as u32).to_string()),
        ValueExpr::BoolLiteral(value) => out.push_str(if *value { "1" } else { "0" }),
        ValueExpr::VoidLiteral => out.push('0'),
        ValueExpr::Variable(name) => out.push_str(&c_var_ident(name)),
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
                out.push_str(field_name);
                out.push_str(" = ");
                emit_expr(out, value);
            }
            out.push('}');
        }
        ValueExpr::FieldAccess { base, field } => {
            out.push_str(&c_var_ident(base));
            out.push('.');
            out.push_str(field);
        }
        ValueExpr::EnumPayloadFieldAccess {
            value,
            variant,
            field,
        } => {
            emit_expr(out, value);
            out.push_str(".payload.");
            out.push_str(variant);
            out.push('.');
            out.push_str(field);
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
                out.push_str(variant);
                out.push_str(" = ");
                emit_expr(out, payload);
            }
            out.push('}');
        }
        ValueExpr::EnumPayload { value, variant } => {
            emit_expr(out, value);
            out.push_str(".payload.");
            out.push_str(variant);
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
            emit_expr(out, message);
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
        ValueExpr::Call { name, args } => {
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
        ValueExpr::StringLen { value } => {
            out.push_str("((uint64_t)strlen(");
            emit_expr(out, value);
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
            out.push_str("/* unsupported Array<");
            out.push_str(element_type.name());
            out.push_str("> */");
        }
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
            match statement {
                Statement::Let {
                    value_type,
                    initializer,
                    ..
                } => {
                    collect_type_struct(value_type, &mut seen, &mut out);
                    collect_expr_struct(initializer, &mut seen, &mut out);
                }
                Statement::TryLet {
                    value_type,
                    result_type,
                    return_type,
                    result_expr,
                    ..
                } => {
                    collect_type_struct(value_type, &mut seen, &mut out);
                    collect_type_struct(result_type, &mut seen, &mut out);
                    collect_type_struct(return_type, &mut seen, &mut out);
                    collect_expr_struct(result_expr, &mut seen, &mut out);
                }
                Statement::Assign { value, .. }
                | Statement::AssignField { value, .. }
                | Statement::Println(value)
                | Statement::Eprintln(value)
                | Statement::Panic(value)
                | Statement::Return(Some(value)) => {
                    collect_expr_struct(value, &mut seen, &mut out);
                }
                Statement::Return(None) => {}
            }
        }
    }
    out
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
        ValueExpr::Binary { left, right, .. } => {
            collect_expr_struct(left, seen, out);
            collect_expr_struct(right, seen, out);
        }
        ValueExpr::Call { args, .. } => {
            for arg in args {
                collect_expr_struct(arg, seen, out);
            }
        }
        ValueExpr::StringLen { value } => collect_expr_struct(value, seen, out),
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
            match statement {
                Statement::Let {
                    value_type,
                    initializer,
                    ..
                } => {
                    collect_type_enum(value_type, &mut seen, &mut out);
                    collect_expr_enum(initializer, &mut seen, &mut out);
                }
                Statement::TryLet {
                    value_type,
                    result_type,
                    return_type,
                    result_expr,
                    ..
                } => {
                    collect_type_enum(value_type, &mut seen, &mut out);
                    collect_type_enum(result_type, &mut seen, &mut out);
                    collect_type_enum(return_type, &mut seen, &mut out);
                    collect_expr_enum(result_expr, &mut seen, &mut out);
                }
                Statement::Assign { value, .. }
                | Statement::AssignField { value, .. }
                | Statement::Println(value)
                | Statement::Eprintln(value)
                | Statement::Panic(value)
                | Statement::Return(Some(value)) => {
                    collect_expr_enum(value, &mut seen, &mut out);
                }
                Statement::Return(None) => {}
            }
        }
    }
    for element_type in collect_array_element_types(program) {
        push_enum_instance(&mut seen, &mut out, "Option", &[element_type]);
    }
    out
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
        ValueExpr::Binary { left, right, .. } => {
            collect_expr_enum(left, seen, out);
            collect_expr_enum(right, seen, out);
        }
        ValueExpr::Call { args, .. } => {
            for arg in args {
                collect_expr_enum(arg, seen, out);
            }
        }
        ValueExpr::StringLen { value } => collect_expr_enum(value, seen, out),
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
        Statement::TryLet { result_expr, .. } => expr_uses_fs_read_to_string(result_expr),
        Statement::Assign { value, .. }
        | Statement::AssignField { value, .. }
        | Statement::Println(value)
        | Statement::Eprintln(value)
        | Statement::Panic(value)
        | Statement::Return(Some(value)) => expr_uses_fs_read_to_string(value),
        Statement::Return(None) => false,
    }
}

fn statement_uses_fs_write_string(statement: &Statement) -> bool {
    match statement {
        Statement::Let { initializer, .. } => expr_uses_fs_write_string(initializer),
        Statement::TryLet { result_expr, .. } => expr_uses_fs_write_string(result_expr),
        Statement::Assign { value, .. }
        | Statement::AssignField { value, .. }
        | Statement::Println(value)
        | Statement::Eprintln(value)
        | Statement::Panic(value)
        | Statement::Return(Some(value)) => expr_uses_fs_write_string(value),
        Statement::Return(None) => false,
    }
}

fn statement_uses_env_get(statement: &Statement) -> bool {
    match statement {
        Statement::Let { initializer, .. } => expr_uses_env_get(initializer),
        Statement::TryLet { result_expr, .. } => expr_uses_env_get(result_expr),
        Statement::Assign { value, .. }
        | Statement::AssignField { value, .. }
        | Statement::Println(value)
        | Statement::Eprintln(value)
        | Statement::Panic(value)
        | Statement::Return(Some(value)) => expr_uses_env_get(value),
        Statement::Return(None) => false,
    }
}

fn statement_uses_env_args(statement: &Statement) -> bool {
    match statement {
        Statement::Let { initializer, .. } => expr_uses_env_args(initializer),
        Statement::TryLet { result_expr, .. } => expr_uses_env_args(result_expr),
        Statement::Assign { value, .. }
        | Statement::AssignField { value, .. }
        | Statement::Println(value)
        | Statement::Eprintln(value)
        | Statement::Panic(value)
        | Statement::Return(Some(value)) => expr_uses_env_args(value),
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
        Statement::Assign { value, .. }
        | Statement::AssignField { value, .. }
        | Statement::Println(value)
        | Statement::Eprintln(value)
        | Statement::Panic(value)
        | Statement::Return(Some(value)) => collect_expr_array_elements(value, seen, out),
        Statement::Return(None) => {}
    }
}

fn expr_uses_fs_read_to_string(expr: &ValueExpr) -> bool {
    match expr {
        ValueExpr::FsReadToString { .. } => true,
        ValueExpr::Binary { left, right, .. } | ValueExpr::StringConcat { left, right } => {
            expr_uses_fs_read_to_string(left) || expr_uses_fs_read_to_string(right)
        }
        ValueExpr::FsWriteString { path, content } => {
            expr_uses_fs_read_to_string(path) || expr_uses_fs_read_to_string(content)
        }
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
        | ValueExpr::FieldAccess { .. } => false,
        ValueExpr::EnumPayloadFieldAccess { value, .. } => expr_uses_fs_read_to_string(value),
    }
}

fn expr_uses_fs_write_string(expr: &ValueExpr) -> bool {
    match expr {
        ValueExpr::FsWriteString { .. } => true,
        ValueExpr::Binary { left, right, .. } | ValueExpr::StringConcat { left, right } => {
            expr_uses_fs_write_string(left) || expr_uses_fs_write_string(right)
        }
        ValueExpr::FsReadToString { path } => expr_uses_fs_write_string(path),
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
        | ValueExpr::FieldAccess { .. } => false,
        ValueExpr::EnumPayloadFieldAccess { value, .. } => expr_uses_fs_write_string(value),
    }
}

fn expr_uses_env_get(expr: &ValueExpr) -> bool {
    match expr {
        ValueExpr::EnvGet { .. } => true,
        ValueExpr::Binary { left, right, .. } | ValueExpr::StringConcat { left, right } => {
            expr_uses_env_get(left) || expr_uses_env_get(right)
        }
        ValueExpr::FsReadToString { path } => expr_uses_env_get(path),
        ValueExpr::FsWriteString { path, content } => {
            expr_uses_env_get(path) || expr_uses_env_get(content)
        }
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
        | ValueExpr::FieldAccess { .. } => false,
    }
}

fn expr_uses_env_args(expr: &ValueExpr) -> bool {
    match expr {
        ValueExpr::EnvArgs => true,
        ValueExpr::Binary { left, right, .. } | ValueExpr::StringConcat { left, right } => {
            expr_uses_env_args(left) || expr_uses_env_args(right)
        }
        ValueExpr::FsReadToString { path }
        | ValueExpr::EnvGet { name: path }
        | ValueExpr::ArrayLen { array: path } => expr_uses_env_args(path),
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
        ValueExpr::Binary { left, right, .. } | ValueExpr::StringConcat { left, right } => {
            collect_expr_array_elements(left, seen, out);
            collect_expr_array_elements(right, seen, out);
        }
        ValueExpr::FsReadToString { path } | ValueExpr::EnvGet { name: path } => {
            collect_expr_array_elements(path, seen, out);
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

fn c_binary_op(op: &BinaryOp) -> &'static str {
    match op {
        BinaryOp::Add => "+",
        BinaryOp::Equal => "==",
        BinaryOp::NotEqual => "!=",
        BinaryOp::Less => "<",
        BinaryOp::LessEqual => "<=",
        BinaryOp::Greater => ">",
        BinaryOp::GreaterEqual => ">=",
    }
}

fn c_type(value_type: &ValueType) -> String {
    match value_type {
        ValueType::String => "const char *".to_string(),
        ValueType::Int => "long long".to_string(),
        ValueType::I32 => "int32_t".to_string(),
        ValueType::U32 => "uint32_t".to_string(),
        ValueType::U64 => "uint64_t".to_string(),
        ValueType::Float => "double".to_string(),
        ValueType::Char => "unsigned int".to_string(),
        ValueType::Bool => "int".to_string(),
        ValueType::Array(element) if is_supported_array_element(element) => c_array_ident(element),
        ValueType::Array(element) => format!("/*unsupported Array<{}>*/ void", element.name()),
        ValueType::Struct(name, args) => c_struct_ident(name, args),
        ValueType::Enum(name, args) => c_enum_ident(name, args),
        ValueType::TypeParam(name) => format!("/*unsubstituted {name}*/ void"),
        ValueType::Void => "void".to_string(),
        ValueType::Never => "void".to_string(),
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
        ValueType::String => "NULL".to_string(),
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

fn c_fn_ident(name: &str) -> String {
    format!("nomo_fn_{name}")
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
    matches!(
        value_type,
        ValueType::String
            | ValueType::Int
            | ValueType::I32
            | ValueType::U32
            | ValueType::U64
            | ValueType::Float
            | ValueType::Char
            | ValueType::Bool
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler::{EnumVariantType, MatchValueArm, Parameter, StructField, ValueExpr};

    #[test]
    fn emits_puts_for_println() {
        let program = Program {
            package: "app.main".to_string(),
            imports: vec!["std.io".to_string()],
            structs: Vec::new(),
            enums: Vec::new(),
            functions: vec![Function {
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
        assert!(c.contains("puts(\"Hello\");"));
    }

    #[test]
    fn emits_fputs_for_eprintln() {
        let program = Program {
            package: "app.main".to_string(),
            imports: vec!["std.io".to_string()],
            structs: Vec::new(),
            enums: Vec::new(),
            functions: vec![Function {
                name: "main".to_string(),
                params: Vec::new(),
                return_type: ValueType::Void,
                body: vec![Statement::Eprintln(ValueExpr::StringLiteral(
                    "error".to_string(),
                ))],
            }],
        };

        let c = emit_c(&program);
        assert!(c.contains("fputs(\"error\", stderr);"));
        assert!(c.contains("fputc('\\n', stderr);"));
    }

    #[test]
    fn emits_function_and_call() {
        let program = Program {
            package: "app.main".to_string(),
            imports: vec!["std.io".to_string()],
            structs: Vec::new(),
            enums: Vec::new(),
            functions: vec![
                Function {
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
    fn emits_float_literal_and_cast() {
        let program = Program {
            package: "app.main".to_string(),
            imports: vec!["std.io".to_string()],
            structs: Vec::new(),
            enums: Vec::new(),
            functions: vec![
                Function {
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
            package: "app.main".to_string(),
            imports: vec!["std.io".to_string()],
            structs: Vec::new(),
            enums: Vec::new(),
            functions: vec![
                Function {
                    name: "initial".to_string(),
                    params: Vec::new(),
                    return_type: ValueType::Char,
                    body: vec![Statement::Return(Some(ValueExpr::CharLiteral('N')))],
                },
                Function {
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
        assert!(c.contains("unsigned int nomo_fn_initial(void);"));
        assert!(c.contains("return 78;"));
        assert!(c.contains("unsigned int nomo_letter = nomo_fn_initial();"));
    }

    #[test]
    fn emits_fixed_width_integer_types() {
        let program = Program {
            package: "app.main".to_string(),
            imports: vec!["std.io".to_string()],
            structs: Vec::new(),
            enums: Vec::new(),
            functions: vec![
                Function {
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
            package: "app.main".to_string(),
            imports: vec!["std.io".to_string(), "std.string".to_string()],
            structs: Vec::new(),
            enums: Vec::new(),
            functions: vec![Function {
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
        assert!(c.contains("static char *nomo_string_concat"));
        assert!(c.contains("const char * nomo_message = nomo_string_concat(\"No\", \"mo\");"));
        assert!(c.contains("uint64_t nomo_count = ((uint64_t)strlen(nomo_message));"));
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
            package: "app.main".to_string(),
            imports: vec!["std.fs".to_string()],
            structs: vec![StructType {
                name: "FsError".to_string(),
                type_params: Vec::new(),
                fields: vec![StructField {
                    name: "message".to_string(),
                    value_type: ValueType::String,
                }],
            }],
            enums: vec![EnumType {
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
        assert!(c.contains("nomo_fs_read_to_string(\"input.txt\")"));
        assert!(c.contains("nomo_fs_write_string(\"output.txt\", \"hello\")"));
    }

    #[test]
    fn emits_env_get_helper() {
        let option_string = ValueType::Enum("Option".to_string(), vec![ValueType::String]);
        let program = Program {
            package: "app.main".to_string(),
            imports: vec!["std.env".to_string()],
            structs: Vec::new(),
            enums: vec![EnumType {
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
        assert!(c.contains("getenv(name)"));
        assert!(c.contains("nomo_env_get(\"NOMO_TEST_ENV\")"));
    }

    #[test]
    fn emits_env_args_helper_and_main_arguments() {
        let array_string = ValueType::Array(Box::new(ValueType::String));
        let program = Program {
            package: "app.main".to_string(),
            imports: vec!["std.env".to_string()],
            structs: Vec::new(),
            enums: vec![EnumType {
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
            package: "app.main".to_string(),
            imports: vec!["std.array".to_string()],
            structs: Vec::new(),
            enums: vec![EnumType {
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
        assert!(c.contains("nomo_items = nomo_array_string_push(nomo_items, \"first\");"));
        assert!(c.contains("uint64_t nomo_size = ((uint64_t)nomo_items.len);"));
        assert!(c.contains("nomo_array_string_get(nomo_items, 0)"));
    }

    #[test]
    fn emits_i32_array_helpers() {
        let array_i32 = ValueType::Array(Box::new(ValueType::I32));
        let option_i32 = ValueType::Enum("Option".to_string(), vec![ValueType::I32]);
        let program = Program {
            package: "app.main".to_string(),
            imports: vec!["std.array".to_string()],
            structs: Vec::new(),
            enums: vec![EnumType {
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
        assert!(c.contains("nomo_array_i32 nomo_items = nomo_array_i32_new();"));
        assert!(c.contains("nomo_items = nomo_array_i32_push(nomo_items, 7);"));
        assert!(c.contains("nomo_array_i32_get(nomo_items, 0)"));
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
            package: "app.main".to_string(),
            imports: vec!["std.array".to_string()],
            structs: Vec::new(),
            enums: vec![EnumType {
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
            (ValueType::String, "const char *"),
            (ValueType::Int, "long long"),
            (ValueType::I32, "int32_t"),
            (ValueType::U32, "uint32_t"),
            (ValueType::U64, "uint64_t"),
            (ValueType::Float, "double"),
            (ValueType::Char, "unsigned int"),
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
            package: "app.main".to_string(),
            imports: vec!["std.io".to_string()],
            structs: Vec::new(),
            enums: Vec::new(),
            functions: vec![
                Function {
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
        assert!(c.contains("return ((nomo_score >= 60) ? \"pass\" : \"fail\");"));
    }

    #[test]
    fn emits_panic_statement_and_expression() {
        let program = Program {
            package: "app.main".to_string(),
            imports: vec!["std.io".to_string()],
            structs: Vec::new(),
            enums: Vec::new(),
            functions: vec![
                Function {
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
        assert!(c.contains("nomo_panic(\"boom\");"));
        assert!(c.contains("(nomo_panic(\"no\"), NULL)"));
    }

    #[test]
    fn emits_assignment() {
        let program = Program {
            package: "app.main".to_string(),
            imports: vec!["std.io".to_string()],
            structs: Vec::new(),
            enums: Vec::new(),
            functions: vec![Function {
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
            package: "app.main".to_string(),
            imports: vec!["std.io".to_string()],
            structs: vec![StructType {
                name: "Counter".to_string(),
                type_params: Vec::new(),
                fields: vec![StructField {
                    name: "value".to_string(),
                    value_type: ValueType::Int,
                }],
            }],
            enums: Vec::new(),
            functions: vec![Function {
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
        assert!(c.contains("nomo_counter.value = (nomo_counter.value + 1);"));
    }

    #[test]
    fn emits_struct_type_literal_and_field_access() {
        let program = Program {
            package: "app.main".to_string(),
            imports: vec!["std.io".to_string()],
            structs: vec![StructType {
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
        assert!(c.contains("nomo_struct_Point nomo_point = (nomo_struct_Point){.x = 1, .y = 2};"));
        assert!(c.contains("long long nomo_x = nomo_point.x;"));
    }

    #[test]
    fn emits_generic_struct_instance() {
        let program = Program {
            package: "app.main".to_string(),
            imports: vec!["std.io".to_string()],
            structs: vec![StructType {
                name: "Box".to_string(),
                type_params: vec!["T".to_string()],
                fields: vec![StructField {
                    name: "value".to_string(),
                    value_type: ValueType::TypeParam("T".to_string()),
                }],
            }],
            enums: Vec::new(),
            functions: vec![Function {
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
        assert!(c.contains("int32_t value;"));
        assert!(c.contains("nomo_struct_Box_i32 nomo_item = (nomo_struct_Box_i32){.value = 7};"));
    }

    #[test]
    fn emits_enum_variant_and_match_expression() {
        let program = Program {
            package: "app.main".to_string(),
            imports: vec!["std.io".to_string()],
            structs: Vec::new(),
            enums: vec![EnumType {
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
        assert!(c.contains("nomo_color.tag == nomo_enum_Color_Red ? \"red\" : \"blue\""));
    }

    #[test]
    fn emits_payload_enum_and_match_binding_access() {
        let program = Program {
            package: "app.main".to_string(),
            imports: vec!["std.io".to_string()],
            structs: Vec::new(),
            enums: vec![EnumType {
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
        assert!(c.contains("long long Some;"));
        assert!(c.contains(".payload.Some = 41"));
        assert!(c.contains("nomo_value.payload.Some"));
    }

    #[test]
    fn emits_void_enum_payload_as_unit_storage() {
        let result_void_string = ValueType::Enum(
            "Result".to_string(),
            vec![ValueType::Void, ValueType::String],
        );
        let program = Program {
            package: "app.main".to_string(),
            imports: vec!["std.io".to_string()],
            structs: Vec::new(),
            enums: vec![EnumType {
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
        assert!(c.contains("char Ok;"));
        assert!(!c.contains("void Ok;"));
        assert!(c.contains(".payload.Ok = 0"));
    }

    #[test]
    fn emits_result_try_let_early_return() {
        let result_i64_string = ValueType::Enum(
            "Result".to_string(),
            vec![ValueType::Int, ValueType::String],
        );
        let program = Program {
            package: "app.main".to_string(),
            imports: vec!["std.io".to_string()],
            structs: Vec::new(),
            enums: vec![EnumType {
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
            "return (nomo_enum_Result_i64_string){.tag = nomo_enum_Result_i64_string_Err, .payload.Err = nomo_value_result.payload.Err};"
        ));
        assert!(c.contains("long long nomo_value = nomo_value_result.payload.Ok;"));
    }
}
