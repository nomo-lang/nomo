use crate::diagnostic::Diagnostic;
use crate::lexer::{TokenKind, lex};
use crate::parser::parse;
use crate::project::Project;
use std::path::{Path, PathBuf};

mod docs;
mod project_scope;
mod signature;
mod symbols;

use docs::extract_doc_comments;
use project_scope::{
    accessible_symbols_for_document, is_project_nomo_source, overrides_with_current,
    project_sources,
};
pub use project_scope::{
    dependency_symbols_for_project_with_overrides, symbols_for_project_with_overrides,
};
use symbols::symbols_from_ast;

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticLocation {
    pub path: PathBuf,
    pub range: TextRange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SemanticSymbolKind {
    Struct,
    Enum,
    Field,
    Variant,
    Interface,
    InterfaceMethod,
    Const,
    Function,
    ExternFunction,
    Method,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticSymbol {
    pub source_path: PathBuf,
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
    Ok(symbols_from_ast(path, &ast, &docs))
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
    let preference = symbol_lookup_preference(path, source, position)?;
    Ok(resolve_symbol(
        path,
        position,
        &name,
        symbols_for_text(path, source)?,
        &preference,
    ))
}

pub fn symbol_at_project_position(
    project: &Project,
    path: &Path,
    source: &str,
    position: TextPosition,
    source_overrides: &[(PathBuf, String)],
) -> Result<Option<SemanticSymbol>, Diagnostic> {
    let Some(name) = identifier_at_position(source, position) else {
        return Ok(None);
    };
    let overrides = overrides_with_current(path, source, source_overrides);
    let symbols = accessible_symbols_for_document(project, source, &overrides)?;
    let preference = symbol_lookup_preference(path, source, position)?;
    Ok(resolve_symbol(path, position, &name, symbols, &preference))
}

pub fn definition_for_text(
    path: &Path,
    source: &str,
    position: TextPosition,
) -> Result<Option<TextRange>, Diagnostic> {
    Ok(symbol_at_position(path, source, position)?.map(|symbol| symbol.selection_range))
}

pub fn definition_for_project_text(
    project: &Project,
    path: &Path,
    source: &str,
    position: TextPosition,
    source_overrides: &[(PathBuf, String)],
) -> Result<Option<SemanticLocation>, Diagnostic> {
    Ok(
        symbol_at_project_position(project, path, source, position, source_overrides)?.map(
            |symbol| SemanticLocation {
                path: symbol.source_path,
                range: symbol.selection_range,
            },
        ),
    )
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

pub fn references_for_project_text(
    project: &Project,
    path: &Path,
    source: &str,
    position: TextPosition,
    include_declaration: bool,
    source_overrides: &[(PathBuf, String)],
) -> Result<Option<Vec<SemanticLocation>>, Diagnostic> {
    let Some(symbol) =
        symbol_at_project_position(project, path, source, position, source_overrides)?
    else {
        return Ok(None);
    };
    let local_source_root = project.root.join("src");
    if !is_project_nomo_source(&local_source_root, &symbol.source_path) {
        return Ok(None);
    }
    let overrides = overrides_with_current(path, source, source_overrides);
    let mut locations = Vec::new();
    for (source_path, source) in project_sources(project, &overrides)? {
        let tokens = lex(&source_path, &source)?;
        for token in &tokens {
            let TokenKind::Ident(name) = &token.kind else {
                continue;
            };
            if name != &symbol.name {
                continue;
            }
            let range = token_range(token.line, token.column, name);
            if !include_declaration
                && source_path == symbol.source_path
                && range == symbol.selection_range
            {
                continue;
            }
            locations.push(SemanticLocation {
                path: source_path.clone(),
                range,
            });
        }
    }
    Ok(Some(locations))
}

fn resolve_symbol(
    path: &Path,
    position: TextPosition,
    name: &str,
    symbols: Vec<SemanticSymbol>,
    preference: &[SemanticSymbolKind],
) -> Option<SemanticSymbol> {
    let mut matches = symbols
        .into_iter()
        .filter(|symbol| symbol.name == name)
        .collect::<Vec<_>>();
    matches.sort_by(|left, right| {
        left.source_path
            .cmp(&right.source_path)
            .then(left.line.cmp(&right.line))
            .then(
                left.selection_range
                    .start
                    .line
                    .cmp(&right.selection_range.start.line),
            )
            .then(
                left.selection_range
                    .start
                    .character
                    .cmp(&right.selection_range.start.character),
            )
    });
    let fallback = matches
        .iter()
        .find(|symbol| {
            symbol.source_path == path && range_contains(symbol.selection_range, position)
        })
        .cloned()
        .or_else(|| {
            matches
                .iter()
                .find(|symbol| symbol.source_path == path)
                .cloned()
        })
        .or_else(|| matches.first().cloned());
    prefer_symbol_kind(fallback, &matches, preference)
}

fn prefer_symbol_kind(
    fallback: Option<SemanticSymbol>,
    matches: &[SemanticSymbol],
    preference: &[SemanticSymbolKind],
) -> Option<SemanticSymbol> {
    for kind in preference {
        if let Some(symbol) = matches.iter().find(|symbol| symbol.kind == *kind) {
            return Some(symbol.clone());
        }
    }
    fallback
}

fn symbol_lookup_preference(
    path: &Path,
    source: &str,
    position: TextPosition,
) -> Result<Vec<SemanticSymbolKind>, Diagnostic> {
    let tokens = lex(path, source)?;
    let Some(index) = ident_token_at_position(&tokens, position) else {
        return Ok(Vec::new());
    };
    let TokenKind::Ident(name) = &tokens[index].kind else {
        return Ok(Vec::new());
    };
    let previous = previous_significant_token(&tokens, index);
    let next = next_significant_index(&tokens, index, tokens.len()).map(|index| &tokens[index]);
    let starts_upper = name
        .chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_uppercase());

    if previous.is_some_and(|token| matches!(token.kind, TokenKind::Fn)) {
        return Ok(vec![
            SemanticSymbolKind::InterfaceMethod,
            SemanticSymbolKind::Method,
            SemanticSymbolKind::Function,
        ]);
    }
    if previous.is_some_and(|token| matches!(token.kind, TokenKind::Interface)) {
        return Ok(vec![SemanticSymbolKind::Interface]);
    }
    if previous.is_some_and(|token| matches!(token.kind, TokenKind::Dot)) && starts_upper {
        return Ok(vec![SemanticSymbolKind::Variant, SemanticSymbolKind::Field]);
    }
    if previous.is_some_and(|token| matches!(token.kind, TokenKind::Dot))
        && next.is_some_and(|token| matches!(token.kind, TokenKind::LParen))
    {
        return Ok(vec![
            SemanticSymbolKind::Method,
            SemanticSymbolKind::Function,
        ]);
    }
    if previous.is_some_and(|token| matches!(token.kind, TokenKind::Dot)) {
        return Ok(vec![SemanticSymbolKind::Field, SemanticSymbolKind::Method]);
    }
    if next.is_some_and(|token| matches!(token.kind, TokenKind::Colon)) {
        return Ok(vec![SemanticSymbolKind::Field]);
    }
    Ok(Vec::new())
}

fn ident_token_at_position(
    tokens: &[crate::lexer::Token],
    position: TextPosition,
) -> Option<usize> {
    tokens.iter().position(|token| {
        matches!(token.kind, TokenKind::Ident(_))
            && range_contains(token_range_for_lookup(token), position)
    })
}

fn token_range_for_lookup(token: &crate::lexer::Token) -> TextRange {
    match &token.kind {
        TokenKind::Ident(name) => token_range(token.line, token.column, name),
        _ => source_line_range(token.line, &token.text),
    }
}

fn previous_significant_token(
    tokens: &[crate::lexer::Token],
    index: usize,
) -> Option<&crate::lexer::Token> {
    (0..index)
        .rev()
        .map(|candidate| &tokens[candidate])
        .find(|token| !matches!(token.kind, TokenKind::Newline | TokenKind::Eof))
}

fn next_significant_index(
    tokens: &[crate::lexer::Token],
    index: usize,
    end: usize,
) -> Option<usize> {
    (index + 1..end).find(|next| !matches!(tokens[*next].kind, TokenKind::Newline))
}

fn range_contains(range: TextRange, position: TextPosition) -> bool {
    if position.line < range.start.line || position.line > range.end.line {
        return false;
    }
    if position.line == range.start.line && position.character < range.start.character {
        return false;
    }
    if position.line == range.end.line && position.character > range.end.character {
        return false;
    }
    true
}

fn source_line_range(line: usize, text: &str) -> TextRange {
    let line = line.saturating_sub(1) as u32;
    TextRange {
        start: TextPosition { line, character: 0 },
        end: TextPosition {
            line,
            character: text.chars().map(|ch| ch.len_utf16() as u32).sum(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::fs;
    use std::path::Path;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn symbols_include_signatures_docs_and_ranges() {
        let source = "package app.main\n\n/// Adds numbers.\npub fn add(a: i64, b: i64) -> i64 {\n    return a + b\n}\n\nstruct User {\n    /// User email address.\n    pub email: string\n}\n\nenum Status {\n    /// Ready state.\n    Ready\n    /// Done state.\n    Done(i32)\n}\n\n/// Displayable values.\npub interface Display {\n    /// Converts to text.\n    fn to_string(self) -> string\n}\n\nextern \"C\" {\n    /// Writes a C string.\n    fn puts(message: string) -> i32\n}\n\nimpl User {\n    pub fn email(self) -> string {\n        return self.email\n    }\n}\n";

        let symbols = symbols_for_text(Path::new("main.nomo"), source).unwrap();

        assert_eq!(
            symbols
                .iter()
                .map(|symbol| symbol.name.as_str())
                .collect::<Vec<_>>(),
            vec![
                "User",
                "email",
                "Status",
                "Ready",
                "Done",
                "Display",
                "to_string",
                "add",
                "puts",
                "email"
            ]
        );
        assert_eq!(symbols[1].kind, SemanticSymbolKind::Field);
        assert_eq!(symbols[1].signature, "pub field User.email: string");
        assert_eq!(symbols[1].docs, "User email address.");
        assert_eq!(
            symbols[1].selection_range,
            TextRange {
                start: TextPosition {
                    line: 9,
                    character: 8,
                },
                end: TextPosition {
                    line: 9,
                    character: 13,
                },
            }
        );
        assert_eq!(symbols[3].kind, SemanticSymbolKind::Variant);
        assert_eq!(symbols[3].signature, "variant Status.Ready");
        assert_eq!(symbols[3].docs, "Ready state.");
        assert_eq!(symbols[4].signature, "variant Status.Done(i32)");
        assert_eq!(symbols[4].docs, "Done state.");
        assert_eq!(symbols[5].kind, SemanticSymbolKind::Interface);
        assert_eq!(symbols[5].signature, "pub interface Display");
        assert_eq!(symbols[5].docs, "Displayable values.");
        assert_eq!(symbols[6].kind, SemanticSymbolKind::InterfaceMethod);
        assert_eq!(
            symbols[6].signature,
            "fn Display.to_string(self: Self) -> string"
        );
        assert_eq!(symbols[6].docs, "Converts to text.");
        assert_eq!(
            symbols[6].selection_range,
            TextRange {
                start: TextPosition {
                    line: 22,
                    character: 7,
                },
                end: TextPosition {
                    line: 22,
                    character: 16,
                },
            }
        );
        assert_eq!(symbols[7].kind, SemanticSymbolKind::Function);
        assert_eq!(symbols[7].signature, "pub fn add(a: i64, b: i64) -> i64");
        assert_eq!(symbols[7].docs, "Adds numbers.");
        assert_eq!(
            symbols[7].selection_range,
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
        assert_eq!(symbols[8].kind, SemanticSymbolKind::ExternFunction);
        assert_eq!(
            symbols[8].signature,
            "extern \"C\" fn puts(message: string) -> i32"
        );
        assert_eq!(symbols[8].docs, "Writes a C string.");
        assert_eq!(
            symbols[8].selection_range,
            TextRange {
                start: TextPosition {
                    line: 27,
                    character: 7,
                },
                end: TextPosition {
                    line: 27,
                    character: 11,
                },
            }
        );
        assert_eq!(
            symbols[9].signature,
            "pub fn User.email(self: User) -> string"
        );
    }

    #[test]
    fn symbols_keep_nested_block_doc_comments() {
        let source = "package app.main\n\n/**\n * Outer docs.\n * /* Nested docs. */\n * Still outer.\n */\npub fn nested() -> void {\n}\n";

        let symbols = symbols_for_text(Path::new("main.nomo"), source).unwrap();

        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "nested");
        assert_eq!(
            symbols[0].docs,
            "Outer docs.\n/* Nested docs. */\nStill outer."
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
    fn definition_returns_field_declaration_range() {
        let source = "package app.main\n\nstruct User {\n    email: string\n}\n\nfn main() -> void {\n    let user: User = User { email: \"hi\" }\n}\n";

        let definition = definition_for_text(
            Path::new("main.nomo"),
            source,
            TextPosition {
                line: 7,
                character: 30,
            },
        )
        .unwrap()
        .unwrap();

        assert_eq!(
            definition,
            TextRange {
                start: TextPosition {
                    line: 3,
                    character: 4,
                },
                end: TextPosition {
                    line: 3,
                    character: 9,
                },
            }
        );
    }

    #[test]
    fn definition_returns_enum_variant_declaration_range() {
        let source = "package app.main\n\nenum Status {\n    Ok\n    Err(string)\n}\n\nfn main() -> void {\n    let status: Status = Status.Err(\"bad\")\n}\n";

        let definition = definition_for_text(
            Path::new("main.nomo"),
            source,
            TextPosition {
                line: 8,
                character: 33,
            },
        )
        .unwrap()
        .unwrap();

        assert_eq!(
            definition,
            TextRange {
                start: TextPosition {
                    line: 4,
                    character: 4,
                },
                end: TextPosition {
                    line: 4,
                    character: 7,
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

    #[test]
    fn project_definition_returns_cross_file_location() {
        let project = test_project("semantic_definition");
        let main = project.root.join("src/main.nomo");
        let math = project.root.join("src/math.nomo");
        write_source(
            &main,
            "package app.main\n\nimport app.math\n\nfn main() -> void {\n    let total: i64 = add(1, 2)\n}\n",
        );
        write_source(
            &math,
            "package app.math\n\n/// Adds numbers.\npub fn add(a: i64, b: i64) -> i64 {\n    return a + b\n}\n",
        );

        let source = fs::read_to_string(&main).unwrap();
        let definition = definition_for_project_text(
            &project,
            &main,
            &source,
            TextPosition {
                line: 5,
                character: 23,
            },
            &[],
        )
        .unwrap()
        .unwrap();

        assert_eq!(definition.path, math);
        assert_eq!(
            definition.range,
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
    }

    #[test]
    fn project_definition_resolves_imported_dependency_public_symbol() {
        let root = env::temp_dir().join(format!(
            "nomo_semantic_dependency_definition_{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let project_root = root.join("hello");
        let dependency_root = root.join("utils");
        fs::create_dir_all(project_root.join("src")).unwrap();
        fs::create_dir_all(dependency_root.join("src")).unwrap();
        fs::write(
            project_root.join("nomo.toml"),
            "[package]\nnamespace = \"fynn\"\nname = \"hello\"\nversion = \"0.1.0\"\nedition = \"2026\"\n\n[dependencies]\nlocal_utils = { package = \"fynn/utils\", path = \"../utils\" }\n",
        )
        .unwrap();
        fs::write(
            dependency_root.join("nomo.toml"),
            "[package]\nnamespace = \"fynn\"\nname = \"utils\"\nversion = \"0.1.0\"\nedition = \"2026\"\n",
        )
        .unwrap();
        let main = project_root.join("src/main.nomo");
        let dep_module = dependency_root.join("src/path.nomo");
        let main_source = "package app.main\n\nimport local_utils.path\n\nfn main() -> void {\n    let total: i64 = join(1, 2)\n}\n";
        write_source(&main, main_source);
        write_source(
            &dep_module,
            "package local_utils.path\n\n/// Joins values.\npub fn join(a: i64, b: i64) -> i64 {\n    return a + b\n}\n\nfn hidden() -> i64 {\n    return 1\n}\n",
        );
        let project = Project {
            main: main.clone(),
            root: project_root,
            name: "hello".to_string(),
            workspace_root: None,
        };

        let definition = definition_for_project_text(
            &project,
            &main,
            main_source,
            TextPosition {
                line: 5,
                character: 23,
            },
            &[],
        )
        .unwrap()
        .unwrap();

        assert_eq!(
            fs::canonicalize(&definition.path).unwrap(),
            fs::canonicalize(&dep_module).unwrap()
        );
        assert_eq!(
            definition.range,
            TextRange {
                start: TextPosition {
                    line: 3,
                    character: 7,
                },
                end: TextPosition {
                    line: 3,
                    character: 11,
                },
            }
        );
        let missing_private = symbol_at_project_position(
            &project,
            &main,
            "package app.main\n\nimport local_utils.path\n\nfn main() -> void {\n    let total: i64 = hidden()\n}\n",
            TextPosition {
                line: 5,
                character: 23,
            },
            &[],
        )
        .unwrap();
        assert!(missing_private.is_none());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn dependency_symbols_for_project_include_public_dependency_api_only() {
        let root = env::temp_dir().join(format!(
            "nomo_semantic_dependency_symbols_{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let project_root = root.join("hello");
        let dependency_root = root.join("utils");
        fs::create_dir_all(project_root.join("src")).unwrap();
        fs::create_dir_all(dependency_root.join("src")).unwrap();
        fs::write(
            project_root.join("nomo.toml"),
            "[package]\nnamespace = \"fynn\"\nname = \"hello\"\nversion = \"0.1.0\"\nedition = \"2026\"\n\n[dependencies]\nlocal_utils = { package = \"fynn/utils\", path = \"../utils\" }\n",
        )
        .unwrap();
        fs::write(
            dependency_root.join("nomo.toml"),
            "[package]\nnamespace = \"fynn\"\nname = \"utils\"\nversion = \"0.1.0\"\nedition = \"2026\"\n",
        )
        .unwrap();
        write_source(&project_root.join("src/main.nomo"), "package app.main\n");
        write_source(
            &dependency_root.join("src/path.nomo"),
            "package local_utils.path\n\npub struct PathInfo {\n    pub name: string\n    hidden: string\n}\n\npub fn join(a: string, b: string) -> string {\n    return a\n}\n\nfn hidden() -> string {\n    return \"hidden\"\n}\n",
        );
        let project = Project {
            main: project_root.join("src/main.nomo"),
            root: project_root,
            name: "hello".to_string(),
            workspace_root: None,
        };

        let symbols = dependency_symbols_for_project_with_overrides(&project, &[]).unwrap();

        let names = symbols
            .iter()
            .map(|symbol| symbol.name.as_str())
            .collect::<Vec<_>>();
        assert!(names.contains(&"PathInfo"), "{names:?}");
        assert!(names.contains(&"name"), "{names:?}");
        assert!(names.contains(&"join"), "{names:?}");
        assert!(!names.contains(&"hidden"), "{names:?}");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn project_references_include_cross_file_identifier_locations() {
        let project = test_project("semantic_references");
        let main = project.root.join("src/main.nomo");
        let math = project.root.join("src/math.nomo");
        write_source(
            &main,
            "package app.main\n\nimport app.math\n\nfn main() -> void {\n    let total: i64 = add(1, 2)\n}\n",
        );
        write_source(
            &math,
            "package app.math\n\npub fn add(a: i64, b: i64) -> i64 {\n    return a + b\n}\n",
        );

        let source = fs::read_to_string(&main).unwrap();
        let references = references_for_project_text(
            &project,
            &main,
            &source,
            TextPosition {
                line: 5,
                character: 23,
            },
            true,
            &[],
        )
        .unwrap()
        .unwrap();

        assert!(references.iter().any(|location| {
            location.path == main
                && location.range
                    == TextRange {
                        start: TextPosition {
                            line: 5,
                            character: 21,
                        },
                        end: TextPosition {
                            line: 5,
                            character: 24,
                        },
                    }
        }));
        assert!(references.iter().any(|location| {
            location.path == math
                && location.range
                    == TextRange {
                        start: TextPosition {
                            line: 2,
                            character: 7,
                        },
                        end: TextPosition {
                            line: 2,
                            character: 10,
                        },
                    }
        }));
    }

    #[test]
    fn project_symbols_use_source_overlays() {
        let project = test_project("semantic_overlays");
        let main = project.root.join("src/main.nomo");
        let math = project.root.join("src/math.nomo");
        write_source(
            &main,
            "package app.main\n\nimport app.math\n\nfn main() -> void {\n    let total: i64 = add(1, 2)\n}\n",
        );
        write_source(
            &math,
            "package app.math\n\npub fn sub(a: i64, b: i64) -> i64 {\n    return a - b\n}\n",
        );

        let source = fs::read_to_string(&main).unwrap();
        let overlay =
            "package app.math\n\npub fn add(a: i64, b: i64) -> i64 {\n    return a + b\n}\n";
        let definition = definition_for_project_text(
            &project,
            &main,
            &source,
            TextPosition {
                line: 5,
                character: 23,
            },
            &[(math.clone(), overlay.to_string())],
        )
        .unwrap()
        .unwrap();

        assert_eq!(definition.path, math);
        assert_eq!(definition.range.start.line, 2);
    }

    fn test_project(name: &str) -> Project {
        let root = env::temp_dir().join(format!(
            "nomo_{name}_{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(
            root.join("nomo.toml"),
            "[package]\nnamespace = \"app\"\nname = \"main\"\nversion = \"0.1.0\"\nedition = \"2026\"\n",
        )
        .unwrap();
        Project {
            main: root.join("src/main.nomo"),
            root,
            name: "main".to_string(),
            workspace_root: None,
        }
    }

    fn write_source(path: &Path, source: &str) {
        fs::write(path, source).unwrap();
    }
}
