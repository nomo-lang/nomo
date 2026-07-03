use crate::ast::{
    ConstDef, EnumDef, Function, ImplBlock, Param, SourceFile, Span, StructDef, TypeRef,
};
use crate::diagnostic::Diagnostic;
use crate::lexer::{TokenKind, lex};
use crate::parser::parse;
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextPosition {
    pub line: u32,
    pub character: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextRange {
    pub start: TextPosition,
    pub end: TextPosition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SemanticSymbolKind {
    Struct,
    Enum,
    Const,
    Function,
    Method,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticSymbol {
    pub name: String,
    pub kind: SemanticSymbolKind,
    pub signature: String,
    pub docs: String,
    pub line: usize,
    pub range: TextRange,
    pub selection_range: TextRange,
}

pub fn symbols_for_text(path: &Path, source: &str) -> Result<Vec<SemanticSymbol>, Diagnostic> {
    let tokens = lex(path, source)?;
    let ast = parse(path, &tokens)?;
    let docs = extract_doc_comments(source);
    Ok(symbols_from_ast(&ast, &docs))
}

pub fn identifier_at_position(source: &str, position: TextPosition) -> Option<String> {
    let line = source.lines().nth(position.line as usize)?;
    let byte_index = utf16_character_to_byte_index(line, position.character);
    let bytes = line.as_bytes();
    if byte_index > bytes.len() {
        return None;
    }

    let mut start = byte_index;
    if start == bytes.len() && start > 0 {
        start -= 1;
    }
    if !is_ident_byte(bytes.get(start).copied()?) && start > 0 {
        start -= 1;
    }
    if !is_ident_byte(bytes.get(start).copied()?) {
        return None;
    }

    let mut end = start;
    while start > 0 && is_ident_byte(bytes[start - 1]) {
        start -= 1;
    }
    while end + 1 < bytes.len() && is_ident_byte(bytes[end + 1]) {
        end += 1;
    }
    Some(line[start..=end].to_string())
}

pub fn symbol_at_position(
    path: &Path,
    source: &str,
    position: TextPosition,
) -> Result<Option<SemanticSymbol>, Diagnostic> {
    let Some(name) = identifier_at_position(source, position) else {
        return Ok(None);
    };
    Ok(symbols_for_text(path, source)?
        .into_iter()
        .filter(|symbol| symbol.name == name)
        .min_by_key(|symbol| symbol.line))
}

pub fn definition_for_text(
    path: &Path,
    source: &str,
    position: TextPosition,
) -> Result<Option<TextRange>, Diagnostic> {
    Ok(symbol_at_position(path, source, position)?.map(|symbol| symbol.selection_range))
}

pub fn references_for_text(
    path: &Path,
    source: &str,
    position: TextPosition,
    include_declaration: bool,
) -> Result<Option<Vec<TextRange>>, Diagnostic> {
    let Some(symbol) = symbol_at_position(path, source, position)? else {
        return Ok(None);
    };
    let tokens = lex(path, source)?;
    Ok(Some(
        tokens
            .iter()
            .filter_map(|token| {
                let TokenKind::Ident(name) = &token.kind else {
                    return None;
                };
                if name != &symbol.name {
                    return None;
                }
                let range = token_range(token.line, token.column, name);
                if !include_declaration && range == symbol.selection_range {
                    return None;
                }
                Some(range)
            })
            .collect(),
    ))
}

fn symbols_from_ast(ast: &SourceFile, docs: &DocComments) -> Vec<SemanticSymbol> {
    let mut symbols = Vec::new();
    for item in &ast.structs {
        symbols.push(SemanticSymbol {
            name: item.name.clone(),
            kind: SemanticSymbolKind::Struct,
            signature: struct_signature(item),
            docs: docs
                .item_docs
                .get(&item.span.line)
                .cloned()
                .unwrap_or_default(),
            line: item.span.line,
            range: line_range(&item.span),
            selection_range: name_selection_range(&item.span, &item.name),
        });
    }
    for item in &ast.enums {
        symbols.push(SemanticSymbol {
            name: item.name.clone(),
            kind: SemanticSymbolKind::Enum,
            signature: enum_signature(item),
            docs: docs
                .item_docs
                .get(&item.span.line)
                .cloned()
                .unwrap_or_default(),
            line: item.span.line,
            range: line_range(&item.span),
            selection_range: name_selection_range(&item.span, &item.name),
        });
    }
    for item in &ast.consts {
        symbols.push(SemanticSymbol {
            name: item.name.clone(),
            kind: SemanticSymbolKind::Const,
            signature: const_signature(item),
            docs: docs
                .item_docs
                .get(&item.span.line)
                .cloned()
                .unwrap_or_default(),
            line: item.span.line,
            range: line_range(&item.span),
            selection_range: name_selection_range(&item.span, &item.name),
        });
    }
    for item in &ast.functions {
        symbols.push(SemanticSymbol {
            name: item.name.clone(),
            kind: SemanticSymbolKind::Function,
            signature: function_signature(item),
            docs: docs
                .item_docs
                .get(&item.span.line)
                .cloned()
                .unwrap_or_default(),
            line: item.span.line,
            range: line_range(&item.span),
            selection_range: name_selection_range(&item.span, &item.name),
        });
    }
    for impl_block in &ast.impls {
        symbols.extend(method_symbols(impl_block, docs));
    }
    symbols
}

fn method_symbols(impl_block: &ImplBlock, docs: &DocComments) -> Vec<SemanticSymbol> {
    let receiver = type_ref(&impl_block.type_name);
    impl_block
        .methods
        .iter()
        .map(|method| SemanticSymbol {
            name: method.name.clone(),
            kind: SemanticSymbolKind::Method,
            signature: method_signature(&receiver, method),
            docs: docs
                .item_docs
                .get(&method.span.line)
                .cloned()
                .unwrap_or_default(),
            line: method.span.line,
            range: line_range(&method.span),
            selection_range: name_selection_range(&method.span, &method.name),
        })
        .collect()
}

fn line_range(span: &Span) -> TextRange {
    let line = span.line.saturating_sub(1) as u32;
    TextRange {
        start: TextPosition { line, character: 0 },
        end: TextPosition {
            line,
            character: span.text.chars().map(|ch| ch.len_utf16() as u32).sum(),
        },
    }
}

fn name_selection_range(span: &Span, name: &str) -> TextRange {
    let line = span.line.saturating_sub(1) as u32;
    let fallback_start = span.column.saturating_sub(1) as u32;
    let start = span
        .text
        .find(name)
        .map(|byte_index| span.text[..byte_index].encode_utf16().count() as u32)
        .unwrap_or(fallback_start);
    let end = start + name.encode_utf16().count() as u32;
    TextRange {
        start: TextPosition {
            line,
            character: start,
        },
        end: TextPosition {
            line,
            character: end,
        },
    }
}

fn token_range(line: usize, column: usize, text: &str) -> TextRange {
    let line = line.saturating_sub(1) as u32;
    let start = column.saturating_sub(1) as u32;
    let end = start + text.encode_utf16().count() as u32;
    TextRange {
        start: TextPosition {
            line,
            character: start,
        },
        end: TextPosition {
            line,
            character: end,
        },
    }
}

fn utf16_character_to_byte_index(line: &str, character: u32) -> usize {
    let mut utf16_count = 0u32;
    for (byte_index, ch) in line.char_indices() {
        if utf16_count >= character {
            return byte_index;
        }
        utf16_count += ch.len_utf16() as u32;
    }
    line.len()
}

fn is_ident_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

fn struct_signature(item: &StructDef) -> String {
    format!(
        "{}struct {}{}",
        visibility_prefix(item.public),
        item.name,
        type_params(&item.type_params)
    )
}

fn enum_signature(item: &EnumDef) -> String {
    format!(
        "{}enum {}{}",
        visibility_prefix(item.public),
        item.name,
        type_params(&item.type_params)
    )
}

fn const_signature(item: &ConstDef) -> String {
    format!(
        "{}const {}: {}",
        visibility_prefix(item.public),
        item.name,
        type_ref(&item.type_ref)
    )
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

fn method_signature(receiver: &str, function: &Function) -> String {
    let params = function
        .params
        .iter()
        .map(param)
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "{}fn {receiver}.{}{}({}) -> {}",
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

fn visibility_prefix(public: bool) -> &'static str {
    if public { "pub " } else { "" }
}

#[derive(Debug, Default)]
struct DocComments {
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
        if trimmed.starts_with("/**") {
            let (doc, next_index) = collect_block_doc(&lines, index);
            pending.push(doc);
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn symbols_include_signatures_docs_and_ranges() {
        let source = "package app.main\n\n/// Adds numbers.\npub fn add(a: i64, b: i64) -> i64 {\n    return a + b\n}\n\nstruct User {\n    email: string\n}\n\nimpl User {\n    pub fn email(self) -> string {\n        return self.email\n    }\n}\n";

        let symbols = symbols_for_text(Path::new("main.nomo"), source).unwrap();

        assert_eq!(
            symbols
                .iter()
                .map(|symbol| symbol.name.as_str())
                .collect::<Vec<_>>(),
            vec!["User", "add", "email"]
        );
        assert_eq!(symbols[1].kind, SemanticSymbolKind::Function);
        assert_eq!(symbols[1].signature, "pub fn add(a: i64, b: i64) -> i64");
        assert_eq!(symbols[1].docs, "Adds numbers.");
        assert_eq!(
            symbols[1].selection_range,
            TextRange {
                start: TextPosition {
                    line: 3,
                    character: 7,
                },
                end: TextPosition {
                    line: 3,
                    character: 10,
                },
            }
        );
        assert_eq!(
            symbols[2].signature,
            "pub fn User.email(self: User) -> string"
        );
    }

    #[test]
    fn definition_returns_declaration_range() {
        let source = "package app.main\n\nfn add(a: i64, b: i64) -> i64 {\n    return a + b\n}\n\nfn main() -> void {\n    let total: i64 = add(1, 2)\n}\n";

        let definition = definition_for_text(
            Path::new("main.nomo"),
            source,
            TextPosition {
                line: 7,
                character: 22,
            },
        )
        .unwrap()
        .unwrap();

        assert_eq!(
            definition,
            TextRange {
                start: TextPosition {
                    line: 2,
                    character: 3,
                },
                end: TextPosition {
                    line: 2,
                    character: 6,
                },
            }
        );
    }

    #[test]
    fn references_can_exclude_declaration() {
        let source = "package app.main\n\nstruct User {\n    email: string\n}\n\nfn main() -> void {\n    let user: User = User { email: \"hi\" }\n}\n";

        let references = references_for_text(
            Path::new("main.nomo"),
            source,
            TextPosition {
                line: 7,
                character: 14,
            },
            false,
        )
        .unwrap()
        .unwrap();

        assert_eq!(
            references,
            vec![
                TextRange {
                    start: TextPosition {
                        line: 7,
                        character: 14,
                    },
                    end: TextPosition {
                        line: 7,
                        character: 18,
                    },
                },
                TextRange {
                    start: TextPosition {
                        line: 7,
                        character: 21,
                    },
                    end: TextPosition {
                        line: 7,
                        character: 25,
                    },
                },
            ]
        );
    }
}
