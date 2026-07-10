use super::{DocError, DocItem, DocModule, DocPackage};
use std::fs;
use std::path::Path;

pub(super) fn write_doc_package(output: &Path, package: &DocPackage) -> Result<(), DocError> {
    let package_dir = output.join(&package.package);
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

pub(super) fn render_index_html(packages: &[DocPackage]) -> String {
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

pub(super) fn render_search_index_json(packages: &[DocPackage]) -> String {
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
                append_search_item(&mut out, package, module, item);
            }
        }
    }
    out.push_str("]}");
    out
}

pub(super) fn package_json(package: &DocPackage) -> String {
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
        body.push_str(&render_item_html(item));
    }
    body
}

fn render_item_html(item: &DocItem) -> String {
    let mut body = format!(
        "<section><h3 id=\"{}\">{} {}</h3><code>{}</code><p>{}</p><p>{}:{} · {}</p>",
        html_escape(&item.name),
        html_escape(&item.kind),
        html_escape(&item.name),
        html_escape(&item.signature),
        html_escape(&item.docs),
        html_escape(&item.source),
        item.line,
        html_escape(&item.visibility)
    );
    if !item.children.is_empty() {
        body.push_str("<h4>Members</h4>");
        for child in &item.children {
            body.push_str(&render_item_html(child));
        }
    }
    body.push_str("</section>");
    body
}

fn append_search_item(out: &mut String, package: &DocPackage, module: &DocModule, item: &DocItem) {
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
    for child in &item.children {
        append_search_item(out, package, module, child);
    }
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
        out.push_str(&item_json(item));
    }
    out.push_str("]}");
    out
}

fn item_json(item: &DocItem) -> String {
    let mut out = format!(
        "{{\"kind\":\"{}\",\"name\":\"{}\",\"signature\":\"{}\",\"visibility\":\"{}\",\"docs\":\"{}\",\"source\":\"{}\",\"line\":{},\"children\":[",
        json_escape(&item.kind),
        json_escape(&item.name),
        json_escape(&item.signature),
        json_escape(&item.visibility),
        json_escape(&item.docs),
        json_escape(&item.source),
        item.line
    );
    for (index, child) in item.children.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        out.push_str(&item_json(child));
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
