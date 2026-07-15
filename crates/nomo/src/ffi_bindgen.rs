use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedBindings {
    pub source: String,
    pub provenance_json: String,
}

#[derive(Debug, Serialize)]
struct BindingProvenance<'a> {
    schema: u32,
    generator: &'static str,
    generator_version: &'static str,
    source: &'a str,
    source_sha256: String,
    package: &'a str,
    opaque_types: usize,
    repr_c_structs: usize,
    functions: usize,
    limitations: [&'static str; 4],
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CStruct {
    name: String,
    fields: Vec<CDeclaration>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CFunction {
    name: String,
    return_type: CType,
    params: Vec<CDeclaration>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CDeclaration {
    name: String,
    value_type: CType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CType {
    base: String,
    pointer: bool,
    is_const: bool,
    callback: Option<Box<CCallback>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CCallback {
    return_type: CType,
    params: Vec<CType>,
}

pub fn generate_bindings_from_header(
    header: &str,
    package: &str,
    source_name: &str,
) -> Result<GeneratedBindings, String> {
    validate_package_path(package)?;
    let cleaned = strip_comments_and_directives(header)?;
    let statements = split_c_statements(&cleaned)?;
    let mut opaque_types = BTreeSet::new();
    let mut structs = Vec::new();
    let mut functions = Vec::new();
    for statement in statements {
        let statement = statement.trim();
        if statement.is_empty() {
            continue;
        }
        if statement.starts_with("typedef struct ") {
            if statement.contains('{') {
                structs.push(parse_struct_typedef(statement)?);
            } else {
                opaque_types.insert(parse_opaque_typedef(statement)?);
            }
        } else {
            functions.push(parse_function(statement)?);
        }
    }
    structs.sort_by(|left, right| left.name.cmp(&right.name));
    functions.sort_by(|left, right| left.name.cmp(&right.name));

    let releases = infer_release_functions(&functions, &opaque_types);
    let source = render_nomo_bindings(package, &opaque_types, &structs, &functions, &releases)?;
    let provenance = BindingProvenance {
        schema: 1,
        generator: "nomo ffi bindgen",
        generator_version: env!("CARGO_PKG_VERSION"),
        source: source_name,
        source_sha256: format!("{:x}", Sha256::digest(header.as_bytes())),
        package,
        opaque_types: opaque_types.len(),
        repr_c_structs: structs.len(),
        functions: functions.len(),
        limitations: [
            "controlled declarations only",
            "no unions or bitfields",
            "no variadic functions",
            "pointer ownership uses deterministic naming heuristics",
        ],
    };
    let mut provenance_json =
        serde_json::to_string_pretty(&provenance).map_err(|err| err.to_string())?;
    provenance_json.push('\n');
    Ok(GeneratedBindings {
        source,
        provenance_json,
    })
}

pub fn write_bindings_from_header(
    header_path: &Path,
    package: &str,
    output_path: &Path,
    provenance_path: Option<&Path>,
) -> Result<(PathBuf, PathBuf), String> {
    let header = fs::read_to_string(header_path)
        .map_err(|err| format!("failed to read {}: {err}", header_path.display()))?;
    let generated =
        generate_bindings_from_header(&header, package, &header_path.display().to_string())?;
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("failed to create {}: {err}", parent.display()))?;
    }
    fs::write(output_path, generated.source)
        .map_err(|err| format!("failed to write {}: {err}", output_path.display()))?;
    let provenance_path = provenance_path
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from(format!("{}.provenance.json", output_path.display())));
    if let Some(parent) = provenance_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("failed to create {}: {err}", parent.display()))?;
    }
    fs::write(&provenance_path, generated.provenance_json)
        .map_err(|err| format!("failed to write {}: {err}", provenance_path.display()))?;
    Ok((output_path.to_path_buf(), provenance_path))
}

fn validate_package_path(package: &str) -> Result<(), String> {
    if package.is_empty()
        || package.split('.').any(|segment| {
            segment.is_empty()
                || !segment
                    .chars()
                    .all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
                || segment.chars().next().is_some_and(|ch| ch.is_ascii_digit())
        })
    {
        return Err(format!("invalid Nomo package path `{package}`"));
    }
    Ok(())
}

fn strip_comments_and_directives(header: &str) -> Result<String, String> {
    let mut out = String::with_capacity(header.len());
    let bytes = header.as_bytes();
    let mut index = 0;
    let mut block_depth = 0usize;
    while index < bytes.len() {
        if block_depth > 0 {
            if bytes[index..].starts_with(b"/*") {
                block_depth += 1;
                index += 2;
            } else if bytes[index..].starts_with(b"*/") {
                block_depth -= 1;
                index += 2;
            } else {
                if bytes[index] == b'\n' {
                    out.push('\n');
                }
                index += 1;
            }
            continue;
        }
        if bytes[index..].starts_with(b"/*") {
            block_depth = 1;
            index += 2;
        } else if bytes[index..].starts_with(b"//") {
            while index < bytes.len() && bytes[index] != b'\n' {
                index += 1;
            }
        } else {
            out.push(bytes[index] as char);
            index += 1;
        }
    }
    if block_depth != 0 {
        return Err("unterminated block comment in C header".to_string());
    }
    Ok(out
        .lines()
        .filter(|line| !line.trim_start().starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n"))
}

fn split_c_statements(source: &str) -> Result<Vec<String>, String> {
    let mut statements = Vec::new();
    let mut start = 0;
    let mut brace_depth = 0usize;
    for (index, ch) in source.char_indices() {
        match ch {
            '{' => brace_depth += 1,
            '}' => {
                brace_depth = brace_depth
                    .checked_sub(1)
                    .ok_or_else(|| "unmatched `}` in C header".to_string())?;
            }
            ';' if brace_depth == 0 => {
                statements.push(source[start..index].trim().to_string());
                start = index + 1;
            }
            _ => {}
        }
    }
    if brace_depth != 0 {
        return Err("unterminated struct declaration in C header".to_string());
    }
    if !source[start..].trim().is_empty() {
        return Err("C declarations must end with `;`".to_string());
    }
    Ok(statements)
}

fn parse_opaque_typedef(statement: &str) -> Result<String, String> {
    let tokens = declaration_tokens(statement);
    let [typedef, structure, tag, alias] = tokens.as_slice() else {
        return Err(format!("unsupported opaque typedef `{statement}`"));
    };
    if typedef != "typedef" || structure != "struct" || tag != alias {
        return Err(format!(
            "opaque typedef must use `typedef struct Name Name`, got `{statement}`"
        ));
    }
    Ok(alias.clone())
}

fn parse_struct_typedef(statement: &str) -> Result<CStruct, String> {
    let open = statement
        .find('{')
        .ok_or_else(|| format!("missing `{{` in `{statement}`"))?;
    let close = statement
        .rfind('}')
        .ok_or_else(|| format!("missing `}}` in `{statement}`"))?;
    let prefix = declaration_tokens(&statement[..open]);
    let suffix = declaration_tokens(&statement[close + 1..]);
    let [typedef, structure, tag] = prefix.as_slice() else {
        return Err(format!("unsupported struct typedef `{statement}`"));
    };
    let [alias] = suffix.as_slice() else {
        return Err(format!(
            "struct typedef requires one alias in `{statement}`"
        ));
    };
    if typedef != "typedef" || structure != "struct" || tag != alias {
        return Err(format!(
            "repr(C) typedef must use matching tag and alias in `{statement}`"
        ));
    }
    let fields = statement[open + 1..close]
        .split(';')
        .map(str::trim)
        .filter(|field| !field.is_empty())
        .map(parse_regular_declaration)
        .collect::<Result<Vec<_>, _>>()?;
    if fields.is_empty() {
        return Err(format!("struct `{alias}` cannot be empty"));
    }
    Ok(CStruct {
        name: alias.clone(),
        fields,
    })
}

fn parse_function(statement: &str) -> Result<CFunction, String> {
    if statement.contains("...") {
        return Err(format!("variadic functions are unsupported: `{statement}`"));
    }
    let open = statement
        .find('(')
        .ok_or_else(|| format!("expected C function declaration, got `{statement}`"))?;
    let close = matching_paren(statement, open)?;
    if !statement[close + 1..].trim().is_empty() {
        return Err(format!(
            "unexpected tokens after function declaration `{statement}`"
        ));
    }
    let declaration = parse_regular_declaration(statement[..open].trim())?;
    let params_source = &statement[open + 1..close];
    let params = if params_source.trim() == "void" || params_source.trim().is_empty() {
        Vec::new()
    } else {
        split_top_level(params_source, ',')
            .into_iter()
            .map(|param| parse_parameter(&param))
            .collect::<Result<Vec<_>, _>>()?
    };
    Ok(CFunction {
        name: declaration.name,
        return_type: declaration.value_type,
        params,
    })
}

fn parse_parameter(source: &str) -> Result<CDeclaration, String> {
    if source.contains("(*") {
        return parse_callback_declaration(source);
    }
    parse_regular_declaration(source)
}

fn parse_callback_declaration(source: &str) -> Result<CDeclaration, String> {
    let marker = source
        .find("(*")
        .ok_or_else(|| format!("invalid callback declaration `{source}`"))?;
    let name_end = source[marker + 2..]
        .find(')')
        .map(|offset| marker + 2 + offset)
        .ok_or_else(|| format!("invalid callback declaration `{source}`"))?;
    let name = source[marker + 2..name_end].trim();
    if !is_identifier(name) {
        return Err(format!("invalid callback name in `{source}`"));
    }
    let params_open = source[name_end + 1..]
        .find('(')
        .map(|offset| name_end + 1 + offset)
        .ok_or_else(|| format!("callback parameters are missing in `{source}`"))?;
    let params_close = matching_paren(source, params_open)?;
    if !source[params_close + 1..].trim().is_empty() {
        return Err(format!("unexpected callback tokens in `{source}`"));
    }
    let return_type = parse_abstract_type(source[..marker].trim())?;
    let params_source = &source[params_open + 1..params_close];
    let params = if params_source.trim() == "void" || params_source.trim().is_empty() {
        Vec::new()
    } else {
        split_top_level(params_source, ',')
            .into_iter()
            .map(|param| parse_abstract_type(param.trim()))
            .collect::<Result<Vec<_>, _>>()?
    };
    Ok(CDeclaration {
        name: name.to_string(),
        value_type: CType {
            base: "callback".to_string(),
            pointer: false,
            is_const: false,
            callback: Some(Box::new(CCallback {
                return_type,
                params,
            })),
        },
    })
}

fn parse_regular_declaration(source: &str) -> Result<CDeclaration, String> {
    if source.contains('[') || source.contains(']') || source.contains(':') {
        return Err(format!(
            "arrays and bitfields are unsupported in `{source}`"
        ));
    }
    let mut tokens = declaration_tokens(source);
    let name = tokens
        .pop()
        .ok_or_else(|| format!("missing declaration name in `{source}`"))?;
    if !is_identifier(&name) {
        return Err(format!("invalid declaration name in `{source}`"));
    }
    let value_type = parse_type_tokens(&tokens, source)?;
    Ok(CDeclaration { name, value_type })
}

fn parse_abstract_type(source: &str) -> Result<CType, String> {
    parse_type_tokens(&declaration_tokens(source), source)
}

fn parse_type_tokens(tokens: &[String], source: &str) -> Result<CType, String> {
    let pointer_count = tokens.iter().filter(|token| token.as_str() == "*").count();
    if pointer_count > 1 {
        return Err(format!(
            "multiple pointer indirection is unsupported in `{source}`"
        ));
    }
    let is_const = tokens.iter().any(|token| token == "const");
    let base_tokens = tokens
        .iter()
        .filter(|token| token.as_str() != "*" && token.as_str() != "const")
        .cloned()
        .collect::<Vec<_>>();
    if base_tokens.len() != 1 {
        return Err(format!("unsupported C type spelling `{source}`"));
    }
    Ok(CType {
        base: base_tokens[0].clone(),
        pointer: pointer_count == 1,
        is_const,
        callback: None,
    })
}

fn declaration_tokens(source: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    for ch in source.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            current.push(ch);
        } else {
            if !current.is_empty() {
                tokens.push(std::mem::take(&mut current));
            }
            if ch == '*' {
                tokens.push("*".to_string());
            }
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

fn matching_paren(source: &str, open: usize) -> Result<usize, String> {
    let mut depth = 0usize;
    for (offset, ch) in source[open..].char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Ok(open + offset);
                }
            }
            _ => {}
        }
    }
    Err(format!("unmatched `(` in `{source}`"))
}

fn split_top_level(source: &str, delimiter: char) -> Vec<String> {
    let mut items = Vec::new();
    let mut depth = 0usize;
    let mut start = 0;
    for (index, ch) in source.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => depth -= 1,
            _ if ch == delimiter && depth == 0 => {
                items.push(source[start..index].trim().to_string());
                start = index + ch.len_utf8();
            }
            _ => {}
        }
    }
    items.push(source[start..].trim().to_string());
    items
}

fn infer_release_functions(
    functions: &[CFunction],
    opaque_types: &BTreeSet<String>,
) -> BTreeMap<String, String> {
    let mut releases = BTreeMap::new();
    for function in functions {
        let release_name = ["_close", "_free", "_destroy", "_release"]
            .iter()
            .any(|suffix| function.name.ends_with(suffix));
        let [param] = function.params.as_slice() else {
            continue;
        };
        if release_name
            && function.return_type.base == "void"
            && !function.return_type.pointer
            && param.value_type.pointer
            && opaque_types.contains(&param.value_type.base)
        {
            releases
                .entry(param.value_type.base.clone())
                .or_insert_with(|| function.name.clone());
        }
    }
    releases
}

fn render_nomo_bindings(
    package: &str,
    opaque_types: &BTreeSet<String>,
    structs: &[CStruct],
    functions: &[CFunction],
    releases: &BTreeMap<String, String>,
) -> Result<String, String> {
    let struct_names = structs
        .iter()
        .map(|item| item.name.clone())
        .collect::<BTreeSet<_>>();
    let mut out = format!("package {package}\n\n");
    for name in opaque_types {
        out.push_str("extern opaque type ");
        out.push_str(name);
        if let Some(release) = releases.get(name) {
            out.push_str(" release ");
            out.push_str(release);
        }
        out.push('\n');
    }
    if !opaque_types.is_empty() {
        out.push('\n');
    }
    for item in structs {
        out.push_str("#[repr(C)]\npub struct ");
        out.push_str(&item.name);
        out.push_str(" {\n");
        for field in &item.fields {
            out.push_str("    pub ");
            out.push_str(&field.name);
            out.push_str(": ");
            out.push_str(&render_field_type(
                &field.value_type,
                opaque_types,
                &struct_names,
            )?);
            out.push('\n');
        }
        out.push_str("}\n\n");
    }
    if !functions.is_empty() {
        out.push_str("extern \"C\" {\n");
        for function in functions {
            out.push_str("    fn ");
            out.push_str(&function.name);
            out.push('(');
            for (index, param) in function.params.iter().enumerate() {
                if index > 0 {
                    out.push_str(", ");
                }
                out.push_str(&param.name);
                out.push_str(": ");
                out.push_str(&render_function_type(
                    &param.value_type,
                    TypePosition::Parameter {
                        function: &function.name,
                    },
                    opaque_types,
                    &struct_names,
                    releases,
                )?);
            }
            out.push_str(") -> ");
            out.push_str(&render_function_type(
                &function.return_type,
                TypePosition::Return,
                opaque_types,
                &struct_names,
                releases,
            )?);
            out.push('\n');
        }
        out.push_str("}\n");
    }
    Ok(out)
}

fn render_field_type(
    value_type: &CType,
    opaque_types: &BTreeSet<String>,
    struct_names: &BTreeSet<String>,
) -> Result<String, String> {
    if value_type.callback.is_some() {
        return Err("callback fields are unsupported".to_string());
    }
    if value_type.pointer {
        if opaque_types.contains(&value_type.base) {
            return Ok(format!("Borrowed<{}>", value_type.base));
        }
        return Err(format!(
            "pointer field `{}` is unsupported unless it targets an opaque typedef",
            value_type.base
        ));
    }
    render_scalar_type(&value_type.base, struct_names)
}

#[derive(Clone, Copy)]
enum TypePosition<'a> {
    Parameter { function: &'a str },
    Return,
    CallbackParameter,
    CallbackReturn,
}

fn render_function_type(
    value_type: &CType,
    position: TypePosition<'_>,
    opaque_types: &BTreeSet<String>,
    struct_names: &BTreeSet<String>,
    releases: &BTreeMap<String, String>,
) -> Result<String, String> {
    if let Some(callback) = &value_type.callback {
        let mut out = "extern \"C\" fn(".to_string();
        for (index, param) in callback.params.iter().enumerate() {
            if index > 0 {
                out.push_str(", ");
            }
            out.push_str(&render_function_type(
                param,
                TypePosition::CallbackParameter,
                opaque_types,
                struct_names,
                releases,
            )?);
        }
        out.push_str(") -> ");
        out.push_str(&render_function_type(
            &callback.return_type,
            TypePosition::CallbackReturn,
            opaque_types,
            struct_names,
            releases,
        )?);
        return Ok(out);
    }
    if value_type.pointer {
        if value_type.base == "char"
            && value_type.is_const
            && matches!(position, TypePosition::Parameter { .. })
        {
            return Ok("CString".to_string());
        }
        if value_type.base == "void" {
            return Ok("Opaque".to_string());
        }
        if opaque_types.contains(&value_type.base) {
            return match position {
                TypePosition::Parameter { function }
                    if releases.get(&value_type.base).is_some_and(|release| release == function) =>
                {
                    Ok(format!("Owned<{}>", value_type.base))
                }
                TypePosition::Parameter { .. } | TypePosition::CallbackParameter => {
                    Ok(format!("Borrowed<{}>", value_type.base))
                }
                TypePosition::Return => {
                    if releases.contains_key(&value_type.base) {
                        Ok(format!("Nullable<Owned<{}>>", value_type.base))
                    } else {
                        Ok(format!("Nullable<{}>", value_type.base))
                    }
                }
                TypePosition::CallbackReturn => Err(
                    "opaque pointer callback returns are unsupported; use an integer status and context handle"
                        .to_string(),
                ),
            };
        }
        return Err(format!("unsupported pointer type `{}`", value_type.base));
    }
    render_scalar_type(&value_type.base, struct_names)
}

fn render_scalar_type(base: &str, struct_names: &BTreeSet<String>) -> Result<String, String> {
    let mapped = match base {
        "void" => "void",
        "int" | "int32_t" => "i32",
        "uint32_t" => "u32",
        "int64_t" => "i64",
        "uint64_t" | "size_t" => "u64",
        "double" => "f64",
        "bool" | "_Bool" => "bool",
        "char32_t" => "char",
        other if struct_names.contains(other) => other,
        other => return Err(format!("unsupported C scalar type `{other}`")),
    };
    Ok(mapped.to_string())
}

fn is_identifier(value: &str) -> bool {
    !value.is_empty()
        && !value.chars().next().is_some_and(|ch| ch.is_ascii_digit())
        && value
            .chars()
            .all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
}

#[cfg(test)]
mod tests {
    use super::*;

    const HEADER: &str = r#"
#include <stdint.h>

typedef struct FileHandle FileHandle;
typedef struct Point {
    int32_t x;
    int32_t y;
} Point;

FileHandle *file_open(void);
void file_close(FileHandle *handle);
int32_t point_sum(Point point);
int32_t apply(int32_t value, int32_t (*callback)(int32_t));
"#;

    #[test]
    fn generates_deterministic_typed_nomo_bindings_and_provenance() {
        let first = generate_bindings_from_header(HEADER, "app.bindings", "fixture.h").unwrap();
        let second = generate_bindings_from_header(HEADER, "app.bindings", "fixture.h").unwrap();
        assert_eq!(first, second);
        assert!(
            first
                .source
                .contains("extern opaque type FileHandle release file_close")
        );
        assert!(first.source.contains("#[repr(C)]\npub struct Point"));
        assert!(
            first
                .source
                .contains("fn file_open() -> Nullable<Owned<FileHandle>>")
        );
        assert!(
            first
                .source
                .contains("fn file_close(handle: Owned<FileHandle>) -> void")
        );
        assert!(
            first
                .source
                .contains("callback: extern \"C\" fn(i32) -> i32")
        );
        assert!(first.provenance_json.contains("source_sha256"));
    }

    #[test]
    fn generated_source_passes_parser_and_module_typecheck() {
        let generated = generate_bindings_from_header(HEADER, "app.bindings", "fixture.h").unwrap();
        let path = Path::new("bindings.nomo");
        let tokens = crate::lexer::lex(path, &generated.source).unwrap();
        crate::parser::parse(path, &tokens).unwrap();
        crate::compiler::check_module_source_text_with_project_modules_and_overrides(
            path,
            &generated.source,
            None,
            &[],
            &[],
            &[],
        )
        .unwrap();
    }
}
