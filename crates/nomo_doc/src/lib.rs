#![allow(clippy::result_large_err)]

use nomo_diagnostics::Diagnostic;
use nomo_syntax::ast::{
    ConstDef, EnumDef, EnumVariant, ExternBlock, Field, Function, FunctionSignature, InterfaceDef,
    Param, SourceFile, StructDef, TypeParamBound, TypeRef,
};
use nomo_syntax::lexer::lex;
use nomo_syntax::parser::parse;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

mod render;

use render::{package_json, render_index_html, render_search_index_json, write_doc_package};

#[derive(Debug)]
pub enum DocError {
    Diagnostic(Diagnostic),
    Message(String),
}

impl DocError {
    pub fn human(&self) -> String {
        match self {
            DocError::Diagnostic(diagnostic) => diagnostic.human(),
            DocError::Message(message) => message.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocPackage {
    pub package: String,
    pub modules: Vec<DocModule>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocModule {
    pub name: String,
    pub source: String,
    pub docs: String,
    pub items: Vec<DocItem>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocItem {
    pub kind: String,
    pub name: String,
    pub signature: String,
    pub visibility: String,
    pub docs: String,
    pub source: String,
    pub line: usize,
    pub children: Vec<DocItem>,
}

pub fn generate_source_docs(
    project_root: &Path,
    source_root: &Path,
    package_id: &str,
    output: &Path,
) -> Result<DocPackage, DocError> {
    let package = collect_source_docs(project_root, source_root, package_id)?;
    write_doc_package(output, &package)?;
    Ok(package)
}

pub fn collect_source_docs(
    project_root: &Path,
    source_root: &Path,
    package_id: &str,
) -> Result<DocPackage, DocError> {
    let mut files = Vec::new();
    collect_nomo_files(source_root, &mut files)?;
    files.sort();

    let mut modules = Vec::new();
    for path in files {
        let source = fs::read_to_string(&path).map_err(|err| {
            DocError::Message(format!("failed to read {}: {err}", path.display()))
        })?;
        let tokens = lex(&path, &source).map_err(DocError::Diagnostic)?;
        let ast = parse(&path, &tokens).map_err(DocError::Diagnostic)?;
        let comments = extract_doc_comments(&source);
        let source_label = path
            .strip_prefix(project_root)
            .unwrap_or(&path)
            .display()
            .to_string();
        modules.push(module_docs(&ast, &comments, source_label));
    }

    Ok(DocPackage {
        package: package_id.to_string(),
        modules,
    })
}

pub fn generate_std_docs(output: &Path) -> Result<DocPackage, DocError> {
    let package = std_doc_package();
    write_doc_package(output, &package)?;
    Ok(package)
}

pub fn std_doc_package() -> DocPackage {
    let source_root = nomo_std::source_root();
    let project_root = source_root
        .parent()
        .and_then(Path::parent)
        .unwrap_or(source_root.as_path());
    let source_modules = collect_source_docs(project_root, &source_root, nomo_std::PACKAGE_ID)
        .ok()
        .map(|package| package.modules)
        .unwrap_or_default();
    let modules = nomo_std::modules()
        .iter()
        .map(|module| {
            source_modules
                .iter()
                .find(|source_module| source_module.name == module.path)
                .filter(|source_module| !source_module.items.is_empty())
                .cloned()
                .unwrap_or_else(|| DocModule {
                    name: module.path.to_string(),
                    source: "toolchain std".to_string(),
                    docs: module.docs.to_string(),
                    items: module
                        .doc_items
                        .iter()
                        .map(|item| DocItem {
                            kind: item.kind.to_string(),
                            name: item.name.to_string(),
                            signature: item.signature.to_string(),
                            visibility: "public".to_string(),
                            docs: item.docs.to_string(),
                            source: Path::new("std/src")
                                .join(nomo_std::module_source_relative_path(module))
                                .display()
                                .to_string(),
                            line: 0,
                            children: Vec::new(),
                        })
                        .collect(),
                })
        })
        .collect();
    DocPackage {
        package: nomo_std::PACKAGE_ID.to_string(),
        modules,
    }
}

pub fn write_doc_index(output: &Path, packages: &[DocPackage]) -> Result<PathBuf, DocError> {
    fs::create_dir_all(output).map_err(|err| DocError::Message(err.to_string()))?;
    fs::write(output.join("index.html"), render_index_html(packages))
        .map_err(|err| DocError::Message(err.to_string()))?;
    fs::write(
        output.join("search-index.json"),
        render_search_index_json(packages),
    )
    .map_err(|err| DocError::Message(err.to_string()))?;
    Ok(output.to_path_buf())
}

pub fn render_packages_json(packages: &[DocPackage]) -> String {
    let mut out = "{\"packages\":[".to_string();
    for (index, package) in packages.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        out.push_str(&package_json(package));
    }
    out.push_str("]}");
    out
}

fn collect_nomo_files(dir: &Path, files: &mut Vec<PathBuf>) -> Result<(), DocError> {
    for entry in fs::read_dir(dir)
        .map_err(|err| DocError::Message(format!("failed to read {}: {err}", dir.display())))?
    {
        let entry = entry.map_err(|err| DocError::Message(err.to_string()))?;
        let path = entry.path();
        if path.is_dir() {
            collect_nomo_files(&path, files)?;
        } else if path.is_file() && path.extension().and_then(|ext| ext.to_str()) == Some("nomo") {
            files.push(path);
        }
    }
    Ok(())
}

fn module_docs(ast: &SourceFile, comments: &DocComments, source: String) -> DocModule {
    let mut items = Vec::new();
    for item in &ast.structs {
        items.push(struct_doc(item, comments, &source));
    }
    for item in &ast.enums {
        items.push(enum_doc(item, comments, &source));
    }
    for item in &ast.interfaces {
        items.push(interface_doc(item, comments, &source));
    }
    for item in &ast.consts {
        items.push(const_doc(item, comments, &source));
    }
    for item in &ast.functions {
        items.push(function_doc("function", item, comments, &source));
    }
    for block in &ast.extern_blocks {
        items.extend(extern_function_docs(block, comments, &source));
    }
    for impl_block in &ast.impls {
        let owner = type_ref(&impl_block.type_name);
        for method in &impl_block.methods {
            let mut item = function_doc("method", method, comments, &source);
            item.name = format!("{owner}.{}", method.name);
            items.push(item);
        }
    }
    items.sort_by(|left, right| {
        left.line
            .cmp(&right.line)
            .then_with(|| left.name.cmp(&right.name))
    });
    DocModule {
        name: ast.package.join("."),
        source,
        docs: comments.module_docs.join("\n\n"),
        items,
    }
}

fn struct_doc(item: &StructDef, comments: &DocComments, source: &str) -> DocItem {
    DocItem {
        kind: "struct".to_string(),
        name: item.name.clone(),
        signature: format!(
            "{}struct {}{}",
            visibility_prefix(item.public),
            item.name,
            type_params(&item.type_params)
        ),
        visibility: visibility(item.public).to_string(),
        docs: comments
            .item_docs
            .get(&item.span.line)
            .cloned()
            .unwrap_or_default(),
        source: source.to_string(),
        line: item.span.line,
        children: item
            .fields
            .iter()
            .map(|field| field_doc(&item.name, field, comments, source))
            .collect(),
    }
}

fn enum_doc(item: &EnumDef, comments: &DocComments, source: &str) -> DocItem {
    DocItem {
        kind: "enum".to_string(),
        name: item.name.clone(),
        signature: format!(
            "{}enum {}{}",
            visibility_prefix(item.public),
            item.name,
            type_params(&item.type_params)
        ),
        visibility: visibility(item.public).to_string(),
        docs: comments
            .item_docs
            .get(&item.span.line)
            .cloned()
            .unwrap_or_default(),
        source: source.to_string(),
        line: item.span.line,
        children: item
            .variants
            .iter()
            .map(|variant| variant_doc(&item.name, variant, comments, source))
            .collect(),
    }
}

fn interface_doc(item: &InterfaceDef, comments: &DocComments, source: &str) -> DocItem {
    DocItem {
        kind: "interface".to_string(),
        name: item.name.clone(),
        signature: format!("{}interface {}", visibility_prefix(item.public), item.name),
        visibility: visibility(item.public).to_string(),
        docs: comments
            .item_docs
            .get(&item.span.line)
            .cloned()
            .unwrap_or_default(),
        source: source.to_string(),
        line: item.span.line,
        children: item
            .methods
            .iter()
            .map(|method| interface_method_doc(&item.name, method, comments, source))
            .collect(),
    }
}

fn const_doc(item: &ConstDef, comments: &DocComments, source: &str) -> DocItem {
    DocItem {
        kind: "const".to_string(),
        name: item.name.clone(),
        signature: format!(
            "{}const {}: {}",
            visibility_prefix(item.public),
            item.name,
            type_ref(&item.type_ref)
        ),
        visibility: visibility(item.public).to_string(),
        docs: comments
            .item_docs
            .get(&item.span.line)
            .cloned()
            .unwrap_or_default(),
        source: source.to_string(),
        line: item.span.line,
        children: Vec::new(),
    }
}

fn function_doc(kind: &str, item: &Function, comments: &DocComments, source: &str) -> DocItem {
    DocItem {
        kind: kind.to_string(),
        name: item.name.clone(),
        signature: function_signature(item),
        visibility: visibility(item.public).to_string(),
        docs: comments
            .item_docs
            .get(&item.span.line)
            .cloned()
            .unwrap_or_default(),
        source: source.to_string(),
        line: item.span.line,
        children: Vec::new(),
    }
}

fn extern_function_docs(block: &ExternBlock, comments: &DocComments, source: &str) -> Vec<DocItem> {
    block
        .functions
        .iter()
        .map(|function| DocItem {
            kind: "extern_function".to_string(),
            name: function.name.clone(),
            signature: extern_function_signature(&block.abi, function),
            visibility: "public".to_string(),
            docs: comments
                .item_docs
                .get(&function.span.line)
                .cloned()
                .unwrap_or_default(),
            source: source.to_string(),
            line: function.span.line,
            children: Vec::new(),
        })
        .collect()
}

fn interface_method_doc(
    owner: &str,
    method: &FunctionSignature,
    comments: &DocComments,
    source: &str,
) -> DocItem {
    DocItem {
        kind: "interface_method".to_string(),
        name: format!("{owner}.{}", method.name),
        signature: interface_method_signature(owner, method),
        visibility: "public".to_string(),
        docs: comments
            .item_docs
            .get(&method.span.line)
            .cloned()
            .unwrap_or_default(),
        source: source.to_string(),
        line: method.span.line,
        children: Vec::new(),
    }
}

fn field_doc(owner: &str, field: &Field, comments: &DocComments, source: &str) -> DocItem {
    DocItem {
        kind: "field".to_string(),
        name: format!("{owner}.{}", field.name),
        signature: format!(
            "{}field {owner}.{}: {}",
            visibility_prefix(field.public),
            field.name,
            type_ref(&field.type_ref)
        ),
        visibility: visibility(field.public).to_string(),
        docs: comments
            .item_docs
            .get(&field.span.line)
            .cloned()
            .unwrap_or_default(),
        source: source.to_string(),
        line: field.span.line,
        children: Vec::new(),
    }
}

fn variant_doc(
    owner: &str,
    variant: &EnumVariant,
    comments: &DocComments,
    source: &str,
) -> DocItem {
    let signature = match &variant.payload {
        Some(payload) => format!("variant {owner}.{}({})", variant.name, type_ref(payload)),
        None => format!("variant {owner}.{}", variant.name),
    };
    DocItem {
        kind: "variant".to_string(),
        name: format!("{owner}.{}", variant.name),
        signature,
        visibility: "public".to_string(),
        docs: comments
            .item_docs
            .get(&variant.span.line)
            .cloned()
            .unwrap_or_default(),
        source: source.to_string(),
        line: variant.span.line,
        children: Vec::new(),
    }
}

fn function_signature(function: &Function) -> String {
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
        type_params_with_bounds(&function.type_params, &function.type_param_bounds),
        params,
        type_ref(&function.return_type)
    )
}

fn extern_function_signature(abi: &str, function: &FunctionSignature) -> String {
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

fn interface_method_signature(owner: &str, method: &FunctionSignature) -> String {
    let params = method
        .params
        .iter()
        .map(param)
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "fn {owner}.{}{}({}) -> {}",
        method.name,
        type_params_with_bounds(&method.type_params, &method.type_param_bounds),
        params,
        type_ref(&method.return_type)
    )
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

fn type_ref(type_ref_value: &TypeRef) -> String {
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

fn visibility(public: bool) -> &'static str {
    if public { "public" } else { "private" }
}

fn visibility_prefix(public: bool) -> &'static str {
    if public { "pub " } else { "" }
}

#[derive(Debug, Default)]
struct DocComments {
    module_docs: Vec<String>,
    item_docs: BTreeMap<usize, String>,
}

fn extract_doc_comments(source: &str) -> DocComments {
    let lines = source.lines().collect::<Vec<_>>();
    let mut comments = DocComments::default();
    let mut pending = Vec::new();
    let mut index = 0usize;
    while index < lines.len() {
        let line = lines[index];
        let trimmed = line.trim_start();
        if let Some(text) = trimmed.strip_prefix("///") {
            pending.push(text.trim_start().to_string());
            index += 1;
            continue;
        }
        if let Some(text) = trimmed.strip_prefix("//!") {
            comments.module_docs.push(text.trim_start().to_string());
            index += 1;
            continue;
        }
        if trimmed.starts_with("/**") || trimmed.starts_with("/*!") {
            let module_doc = trimmed.starts_with("/*!");
            let (doc, next_index) = collect_block_doc(&lines, index);
            if module_doc {
                comments.module_docs.push(doc);
            } else {
                pending.push(doc);
            }
            index = next_index;
            continue;
        }
        if !trimmed.is_empty()
            && !trimmed.starts_with("//")
            && !trimmed.starts_with("/*")
            && !pending.is_empty()
        {
            comments.item_docs.insert(index + 1, pending.join("\n"));
            pending.clear();
        }
        index += 1;
    }
    comments
}

fn collect_block_doc(lines: &[&str], start: usize) -> (String, usize) {
    let mut raw = String::new();
    let mut index = start;
    let mut depth = 0usize;
    while index < lines.len() {
        if !raw.is_empty() {
            raw.push('\n');
        }
        let line = lines[index];
        raw.push_str(line);
        depth = update_block_comment_depth(line, depth);
        if depth == 0 {
            index += 1;
            break;
        }
        index += 1;
    }
    let raw = raw
        .trim()
        .trim_start_matches("/**")
        .trim_start_matches("/*!")
        .trim_end_matches("*/");
    let doc = raw
        .lines()
        .map(|line| line.trim().trim_start_matches('*').trim_start())
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string();
    (doc, index)
}

fn update_block_comment_depth(line: &str, mut depth: usize) -> usize {
    let bytes = line.as_bytes();
    let mut index = 0usize;
    while index + 1 < bytes.len() {
        match (bytes[index], bytes[index + 1]) {
            (b'/', b'*') => {
                depth += 1;
                index += 2;
            }
            (b'*', b'/') => {
                depth = depth.saturating_sub(1);
                index += 2;
            }
            _ => index += 1,
        }
    }
    depth
}

#[cfg(test)]
mod tests {
    use super::std_doc_package;

    #[test]
    fn standard_docs_use_source_items_when_available() {
        let package = std_doc_package();
        let array = package
            .modules
            .iter()
            .find(|module| module.name == "std.array")
            .unwrap();
        let new = array.items.iter().find(|item| item.name == "new").unwrap();
        assert_eq!(new.source, "std/src/array.nomo");
        assert!(new.line > 0);
        assert_eq!(new.signature, "pub fn new<T>() -> Array<T>");

        let string = package
            .modules
            .iter()
            .find(|module| module.name == "std.string")
            .unwrap();
        let split = string
            .items
            .iter()
            .find(|item| item.name == "split")
            .unwrap();
        assert_eq!(split.source, "std/src/string.nomo");
        assert!(split.docs.contains("Splits a string"));

        let io = package
            .modules
            .iter()
            .find(|module| module.name == "std.io")
            .unwrap();
        let println = io.items.iter().find(|item| item.name == "println").unwrap();
        assert_eq!(println.source, "std/src/io.nomo");
        assert_eq!(println.signature, "pub fn println(value: string) -> void");

        let path = package
            .modules
            .iter()
            .find(|module| module.name == "std.path")
            .unwrap();
        let join = path.items.iter().find(|item| item.name == "join").unwrap();
        assert_eq!(join.source, "std/src/path.nomo");
        assert!(join.docs.contains("Joins two path strings"));

        let json = package
            .modules
            .iter()
            .find(|module| module.name == "std.json")
            .unwrap();
        let parse = json.items.iter().find(|item| item.name == "parse").unwrap();
        assert_eq!(parse.source, "std/src/json.nomo");
        assert_eq!(
            parse.signature,
            "pub fn parse(value: string) -> Result<JsonValue, JsonError>"
        );

        let debug = package
            .modules
            .iter()
            .find(|module| module.name == "std.debug")
            .unwrap();
        let panic = debug
            .items
            .iter()
            .find(|item| item.name == "panic")
            .unwrap();
        assert_eq!(panic.source, "std/src/debug.nomo");

        let net = package
            .modules
            .iter()
            .find(|module| module.name == "std.net")
            .unwrap();
        let connect = net
            .items
            .iter()
            .find(|item| item.name == "connect")
            .unwrap();
        assert_eq!(connect.source, "std/src/net.nomo");
        assert_eq!(
            connect.signature,
            "pub fn connect(host: string, port: i64) -> Result<TcpStream, NetError>"
        );

        let http = package
            .modules
            .iter()
            .find(|module| module.name == "std.http")
            .unwrap();
        let get = http.items.iter().find(|item| item.name == "get").unwrap();
        assert_eq!(get.source, "std/src/http.nomo");
        assert!(get.docs.contains("blocking HTTP GET"));
    }
}
