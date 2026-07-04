use crate::ast::{ConstDef, EnumDef, Function, Param, SourceFile, StructDef, TypeRef};
use crate::diagnostic::Diagnostic;
use crate::lexer::lex;
use crate::parser::parse;
use crate::project::{Project, project_package_id};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

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
}

pub fn generate_project_docs(project: &Project, output: &Path) -> Result<DocPackage, DocError> {
    let package_id = project_package_id(project).map_err(DocError::Message)?;
    let package = collect_project_docs(project, &package_id)?;
    write_doc_package(output, &package)?;
    Ok(package)
}

pub fn collect_project_docs(project: &Project, package_id: &str) -> Result<DocPackage, DocError> {
    let src = project.root.join("src");
    let mut files = Vec::new();
    collect_nomo_files(&src, &mut files)?;
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
            .strip_prefix(&project.root)
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
    let modules = [
        ("std.io", "printing and terminal I/O"),
        ("std.fs", "filesystem helpers"),
        ("std.env", "process environment helpers"),
        ("std.path", "path manipulation helpers"),
        ("std.math", "numeric helpers"),
        ("std.num", "numeric parsing and conversion helpers"),
        ("std.hash", "stable non-cryptographic hashing helpers"),
        ("std.crypto", "cryptographic digest helpers"),
        ("std.regex", "regular expression helpers"),
        ("std.json", "JSON parse and stringify helpers"),
        ("std.net", "blocking TCP and UDP helpers"),
        ("std.collections", "string map and string set helpers"),
        ("std.char", "character helpers"),
        ("std.os", "target OS helpers"),
        ("std.time", "clock and sleep helpers"),
        ("std.process", "process helpers"),
        ("std.testing", "test assertion helpers"),
        ("std.debug", "debug print and panic helpers"),
        ("std.log", "leveled logging helpers"),
        ("std.option", "Option carrier helpers"),
        ("std.result", "Result carrier helpers"),
        ("std.array", "Array helpers"),
        ("std.string", "string helpers"),
    ]
    .into_iter()
    .map(|(name, docs)| DocModule {
        name: name.to_string(),
        source: "toolchain std".to_string(),
        docs: docs.to_string(),
        items: Vec::new(),
    })
    .collect();
    DocPackage {
        package: "nomo-lang/std".to_string(),
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
    for item in &ast.consts {
        items.push(const_doc(item, comments, &source));
    }
    for item in &ast.functions {
        items.push(function_doc("function", item, comments, &source));
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
        type_params(&function.type_params),
        params,
        type_ref(&function.return_type)
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
        if !trimmed.is_empty() && !trimmed.starts_with("//") && !trimmed.starts_with("/*") {
            if !pending.is_empty() {
                comments.item_docs.insert(index + 1, pending.join("\n"));
                pending.clear();
            }
        }
        index += 1;
    }
    comments
}

fn collect_block_doc(lines: &[&str], start: usize) -> (String, usize) {
    let mut raw = String::new();
    let mut index = start;
    while index < lines.len() {
        if !raw.is_empty() {
            raw.push('\n');
        }
        raw.push_str(lines[index]);
        if lines[index].contains("*/") {
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

fn write_doc_package(output: &Path, package: &DocPackage) -> Result<(), DocError> {
    let package_dir = output.join(package.package.replace('/', "/"));
    fs::create_dir_all(&package_dir).map_err(|err| DocError::Message(err.to_string()))?;
    fs::write(package_dir.join("index.html"), render_package_html(package))
        .map_err(|err| DocError::Message(err.to_string()))?;
    for module in &package.modules {
        fs::write(
            package_dir.join(format!("{}.html", safe_file_name(&module.name))),
            render_module_html(package, module),
        )
        .map_err(|err| DocError::Message(err.to_string()))?;
    }
    Ok(())
}

fn render_index_html(packages: &[DocPackage]) -> String {
    let mut body = String::from(
        "<!doctype html><meta charset=\"utf-8\"><title>Nomo Docs</title><h1>Nomo Docs</h1><ul>",
    );
    for package in packages {
        body.push_str(&format!(
            "<li><a href=\"{}/index.html\">{}</a></li>",
            html_escape(&package.package),
            html_escape(&package.package)
        ));
    }
    body.push_str("</ul>");
    body
}

fn render_package_html(package: &DocPackage) -> String {
    let mut body = format!(
        "<!doctype html><meta charset=\"utf-8\"><title>{}</title><h1>{}</h1><h2>Modules</h2><ul>",
        html_escape(&package.package),
        html_escape(&package.package)
    );
    for module in &package.modules {
        body.push_str(&format!(
            "<li><a href=\"{}.html\">{}</a></li>",
            safe_file_name(&module.name),
            html_escape(&module.name)
        ));
    }
    body.push_str("</ul>");
    body
}

fn render_module_html(package: &DocPackage, module: &DocModule) -> String {
    let mut body = format!(
        "<!doctype html><meta charset=\"utf-8\"><title>{}</title><h1>{}</h1><p><a href=\"index.html\">{}</a></p>",
        html_escape(&module.name),
        html_escape(&module.name),
        html_escape(&package.package)
    );
    if !module.docs.is_empty() {
        body.push_str(&format!("<pre>{}</pre>", html_escape(&module.docs)));
    }
    body.push_str(&format!(
        "<p>Source: {}</p><h2>Items</h2>",
        html_escape(&module.source)
    ));
    for item in &module.items {
        body.push_str(&format!(
            "<section><h3 id=\"{}\">{} {}</h3><code>{}</code><p>{}</p><p>{}:{} · {}</p></section>",
            html_escape(&item.name),
            html_escape(&item.kind),
            html_escape(&item.name),
            html_escape(&item.signature),
            html_escape(&item.docs),
            html_escape(&item.source),
            item.line,
            html_escape(&item.visibility)
        ));
    }
    body
}

fn render_search_index_json(packages: &[DocPackage]) -> String {
    let mut out = "{\"items\":[".to_string();
    let mut first = true;
    for package in packages {
        for module in &package.modules {
            if !first {
                out.push(',');
            }
            first = false;
            out.push_str(&format!(
                "{{\"kind\":\"module\",\"package\":\"{}\",\"module\":\"{}\",\"name\":\"{}\",\"url\":\"{}/{}.html\"}}",
                json_escape(&package.package),
                json_escape(&module.name),
                json_escape(&module.name),
                json_escape(&package.package),
                json_escape(&safe_file_name(&module.name))
            ));
            for item in &module.items {
                out.push(',');
                out.push_str(&format!(
                    "{{\"kind\":\"{}\",\"package\":\"{}\",\"module\":\"{}\",\"name\":\"{}\",\"signature\":\"{}\",\"url\":\"{}/{}.html#{}\"}}",
                    json_escape(&item.kind),
                    json_escape(&package.package),
                    json_escape(&module.name),
                    json_escape(&item.name),
                    json_escape(&item.signature),
                    json_escape(&package.package),
                    json_escape(&safe_file_name(&module.name)),
                    json_escape(&item.name)
                ));
            }
        }
    }
    out.push_str("]}");
    out
}

fn package_json(package: &DocPackage) -> String {
    let mut out = format!(
        "{{\"package\":\"{}\",\"modules\":[",
        json_escape(&package.package)
    );
    for (index, module) in package.modules.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        out.push_str(&module_json(module));
    }
    out.push_str("]}");
    out
}

fn module_json(module: &DocModule) -> String {
    let mut out = format!(
        "{{\"name\":\"{}\",\"source\":\"{}\",\"docs\":\"{}\",\"items\":[",
        json_escape(&module.name),
        json_escape(&module.source),
        json_escape(&module.docs)
    );
    for (index, item) in module.items.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        out.push_str(&format!(
            "{{\"kind\":\"{}\",\"name\":\"{}\",\"signature\":\"{}\",\"visibility\":\"{}\",\"docs\":\"{}\",\"source\":\"{}\",\"line\":{}}}",
            json_escape(&item.kind),
            json_escape(&item.name),
            json_escape(&item.signature),
            json_escape(&item.visibility),
            json_escape(&item.docs),
            json_escape(&item.source),
            item.line
        ));
    }
    out.push_str("]}");
    out
}

fn safe_file_name(name: &str) -> String {
    name.chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn json_escape(value: &str) -> String {
    value
        .chars()
        .flat_map(|ch| match ch {
            '"' => "\\\"".chars().collect::<Vec<_>>(),
            '\\' => "\\\\".chars().collect::<Vec<_>>(),
            '\n' => "\\n".chars().collect::<Vec<_>>(),
            '\r' => "\\r".chars().collect::<Vec<_>>(),
            '\t' => "\\t".chars().collect::<Vec<_>>(),
            ch if ch.is_control() => format!("\\u{:04x}", ch as u32).chars().collect(),
            ch => vec![ch],
        })
        .collect()
}
