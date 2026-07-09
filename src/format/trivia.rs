use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct FormatTrivia {
    pub(super) leading: BTreeMap<usize, Vec<String>>,
    pub(super) trailing: BTreeMap<usize, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RawComment {
    end_line: usize,
    lines: Vec<String>,
}

pub(super) fn collect_trivia(source: &str) -> FormatTrivia {
    let mut full_line_comments = Vec::new();
    let mut trailing = BTreeMap::new();
    let mut code_lines = BTreeSet::new();
    let lines = source.lines().collect::<Vec<_>>();
    let mut index = 0usize;

    while index < lines.len() {
        let line_no = index + 1;
        let line = lines[index];
        let Some(comment) = find_comment_start(line) else {
            if !line.trim().is_empty() {
                code_lines.insert(line_no);
            }
            index += 1;
            continue;
        };

        let before = &line[..comment.index];
        if comment.kind == CommentKind::Line {
            if before.trim().is_empty() {
                full_line_comments.push(RawComment {
                    end_line: line_no,
                    lines: vec![line[comment.index..].trim_end().to_string()],
                });
            } else {
                code_lines.insert(line_no);
                trailing.insert(line_no, line[comment.index..].trim().to_string());
            }
            index += 1;
            continue;
        }

        if let Some(end) = find_block_comment_end(line, comment.index) {
            let after = &line[end..];
            if before.trim().is_empty() && after.trim().is_empty() {
                full_line_comments.push(RawComment {
                    end_line: line_no,
                    lines: vec![line[comment.index..end].trim_end().to_string()],
                });
            } else {
                code_lines.insert(line_no);
                trailing.insert(line_no, line[comment.index..end].trim().to_string());
            }
            index += 1;
            continue;
        }

        let mut comment_lines = vec![line[comment.index..].trim_end().to_string()];
        let full_line = before.trim().is_empty();
        if !full_line {
            code_lines.insert(line_no);
        }
        index += 1;
        while index < lines.len() {
            let current_line_no = index + 1;
            let current = lines[index];
            if let Some(end) = find_block_comment_end_in_comment(current) {
                comment_lines.push(current[..end].trim_end().to_string());
                if !current[end..].trim().is_empty() {
                    code_lines.insert(current_line_no);
                }
                break;
            }
            comment_lines.push(current.trim_end().to_string());
            index += 1;
        }
        full_line_comments.push(RawComment {
            end_line: index.saturating_add(1),
            lines: comment_lines,
        });
        index += 1;
    }

    let mut leading = BTreeMap::<usize, Vec<String>>::new();
    for comment in full_line_comments {
        let target = code_lines
            .range((comment.end_line + 1)..)
            .next()
            .copied()
            .unwrap_or(usize::MAX);
        leading.entry(target).or_default().extend(comment.lines);
    }

    FormatTrivia { leading, trailing }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CommentStart {
    index: usize,
    kind: CommentKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CommentKind {
    Line,
    Block,
}

fn find_comment_start(line: &str) -> Option<CommentStart> {
    let mut chars = line.char_indices().peekable();
    while let Some((index, ch)) = chars.next() {
        match ch {
            '"' => skip_quoted(&mut chars, '"'),
            '\'' => skip_quoted(&mut chars, '\''),
            '/' => match chars.peek() {
                Some((_, '/')) => {
                    return Some(CommentStart {
                        index,
                        kind: CommentKind::Line,
                    });
                }
                Some((_, '*')) => {
                    return Some(CommentStart {
                        index,
                        kind: CommentKind::Block,
                    });
                }
                _ => {}
            },
            _ => {}
        }
    }
    None
}

fn find_block_comment_end(line: &str, start: usize) -> Option<usize> {
    find_block_comment_end_with_depth(line, start, 0)
}

fn find_block_comment_end_in_comment(line: &str) -> Option<usize> {
    find_block_comment_end_with_depth(line, 0, 1)
}

fn find_block_comment_end_with_depth(
    line: &str,
    start: usize,
    initial_depth: usize,
) -> Option<usize> {
    let mut depth = initial_depth;
    let mut chars = line[start..].char_indices().peekable();
    while let Some((offset, ch)) = chars.next() {
        match ch {
            '/' if matches!(chars.peek(), Some((_, '*'))) => {
                chars.next();
                depth += 1;
            }
            '*' if depth > 0 && matches!(chars.peek(), Some((_, '/'))) => {
                let (_, slash) = chars.next().expect("peeked slash");
                depth -= 1;
                if depth == 0 {
                    return Some(start + offset + ch.len_utf8() + slash.len_utf8());
                }
            }
            _ => {}
        }
    }
    None
}

fn skip_quoted(chars: &mut std::iter::Peekable<std::str::CharIndices<'_>>, quote: char) {
    while let Some((_, ch)) = chars.next() {
        if ch == '\\' {
            chars.next();
        } else if ch == quote {
            break;
        }
    }
}
