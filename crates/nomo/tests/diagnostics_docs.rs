use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

#[test]
fn diagnostics_index_links_existing_docs_with_required_sections() {
    let docs_dir = diagnostics_dir();
    let index = fs::read_to_string(docs_dir.join("index.md")).unwrap();
    let codes = documented_codes(&index);
    let registered_codes = registered_codes();

    assert!(!codes.is_empty(), "diagnostics index should list codes");
    assert_eq!(
        codes, registered_codes,
        "diagnostics index must match the compiler registry"
    );
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

#[test]
fn emitted_diagnostic_codes_are_registered() {
    let registered = registered_codes().into_iter().collect::<BTreeSet<String>>();
    let emitted = emitted_codes();
    let missing = emitted
        .iter()
        .filter(|code| code.as_str() != "E9999" && !registered.contains(*code))
        .cloned()
        .collect::<Vec<_>>();

    assert!(
        missing.is_empty(),
        "emitted diagnostic codes must be documented: {missing:?}"
    );
}

fn diagnostics_dir() -> PathBuf {
    workspace_root().join("docs/diagnostics")
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn registered_codes() -> Vec<String> {
    nomo::diagnostic::documented_diagnostic_codes()
        .iter()
        .map(|code| (*code).to_string())
        .collect()
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

fn emitted_codes() -> BTreeSet<String> {
    let root = workspace_root();
    let mut files = Vec::new();
    collect_rs_files(&root.join("crates"), &mut files);
    collect_rs_files(&root.join("libs"), &mut files);

    let mut codes = BTreeSet::new();
    for file in files {
        let text = fs::read_to_string(file).unwrap();
        for quoted in text.split('"').skip(1).step_by(2) {
            if is_diagnostic_code(quoted) {
                codes.insert(quoted.to_string());
            }
        }
    }
    codes
}

fn collect_rs_files(dir: &Path, files: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            collect_rs_files(&path, files);
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
            files.push(path);
        }
    }
}

fn is_diagnostic_code(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 5 && bytes[0] == b'E' && bytes[1..].iter().all(|byte| byte.is_ascii_digit())
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
