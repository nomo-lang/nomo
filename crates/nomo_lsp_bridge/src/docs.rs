use std::collections::BTreeMap;

#[derive(Debug, Default)]
pub(super) struct DocComments {
    pub(super) item_docs: BTreeMap<usize, String>,
}

pub(super) fn extract_doc_comments(source: &str) -> DocComments {
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
        if trimmed.starts_with("/**") {
            let (doc, next_index) = collect_block_doc(&lines, index);
            pending.push(doc);
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
    let raw = raw.trim().trim_start_matches("/**").trim_end_matches("*/");
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
