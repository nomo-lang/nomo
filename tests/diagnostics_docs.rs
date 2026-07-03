use std::fs;
use std::path::{Path, PathBuf};

#[test]
fn diagnostics_index_links_existing_docs_with_required_sections() {
    let docs_dir = diagnostics_dir();
    let index = fs::read_to_string(docs_dir.join("index.md")).unwrap();
    let codes = documented_codes(&index);

    assert!(!codes.is_empty(), "diagnostics index should list codes");
    for code in codes {
        let path = docs_dir.join(format!("{code}.md"));
        assert!(path.exists(), "missing diagnostics doc for {code}");
        assert_doc_has_required_sections(&code, &path);
    }
}

#[test]
fn every_diagnostics_doc_is_linked_from_index() {
    let docs_dir = diagnostics_dir();
    let index = fs::read_to_string(docs_dir.join("index.md")).unwrap();
    let codes = documented_codes(&index);

    for entry in fs::read_dir(&docs_dir).unwrap() {
        let path = entry.unwrap().path();
        if path.file_name().and_then(|name| name.to_str()) == Some("index.md") {
            continue;
        }
        let Some(code) = path.file_stem().and_then(|name| name.to_str()) else {
            continue;
        };
        assert!(
            codes.iter().any(|documented| documented == code),
            "{code}.md is not linked from diagnostics index"
        );
    }
}

fn diagnostics_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/diagnostics")
}

fn documented_codes(index: &str) -> Vec<String> {
    index
        .lines()
        .filter_map(|line| {
            let line = line.trim_start();
            let rest = line.strip_prefix("- [")?;
            let code = rest.split(']').next()?;
            let link = format!("]({code}.md)");
            rest.contains(&link).then(|| code.to_string())
        })
        .collect()
}

fn assert_doc_has_required_sections(code: &str, path: &Path) {
    let text = fs::read_to_string(path).unwrap();
    assert!(
        text.starts_with(&format!("# {code}:")),
        "{code} doc should start with a matching H1"
    );
    for heading in [
        "## Explanation",
        "## Error Example",
        "## Correct Code",
        "## Fix",
        "## LSP Code Action",
        "## Related Links",
    ] {
        assert!(text.contains(heading), "{code} doc is missing {heading}");
    }
}
