use crate::diagnostic::Diagnostic;
use crate::project::{Project, WorkspaceGraph};
pub use nomo_lsp_bridge::{
    SemanticLocation, SemanticMemberOwner, SemanticSymbol, SemanticSymbolKind, TextPosition,
    TextRange, identifier_at_position, local_definition_for_text, local_references_for_text,
    references_for_symbol_in_text, symbols_for_text, token_range,
};
use nomo_lsp_bridge::{
    definition_for_text as bridge_definition_for_text, references_for_symbol_in_text_with_owners,
    resolve_symbol_at_position_with_owner,
};
use std::path::{Path, PathBuf};

mod member_resolution;
mod project_scope;

use member_resolution::{member_owners_for_document, member_owners_for_text};
use project_scope::{
    accessible_symbols_for_document, is_project_nomo_source, overrides_with_current,
    project_sources,
};
pub use project_scope::{
    dependency_symbols_for_project_with_overrides, symbols_for_project_with_overrides,
};

pub fn symbol_at_position(
    path: &Path,
    source: &str,
    position: TextPosition,
) -> Result<Option<SemanticSymbol>, Diagnostic> {
    let symbols = symbols_for_text(path, source)?;
    let member_owners = member_owners_for_text(path, source).unwrap_or_default();
    let owner = member_owners
        .iter()
        .find(|member| range_contains(member.range, position))
        .map(|member| member.owner.as_str());
    resolve_symbol_at_position_with_owner(path, source, position, symbols, owner)
}

pub fn definition_for_text(
    path: &Path,
    source: &str,
    position: TextPosition,
) -> Result<Option<TextRange>, Diagnostic> {
    if let Some(definition) = local_definition_for_text(path, source, position)? {
        return Ok(Some(definition));
    }
    if let Some(symbol) = symbol_at_position(path, source, position)? {
        return Ok(Some(symbol.selection_range));
    }
    bridge_definition_for_text(path, source, position)
}

pub fn references_for_text(
    path: &Path,
    source: &str,
    position: TextPosition,
    include_declaration: bool,
) -> Result<Option<Vec<TextRange>>, Diagnostic> {
    if let Some(references) =
        local_references_for_text(path, source, position, include_declaration)?
    {
        return Ok(Some(references));
    }
    let Some(symbol) = symbol_at_position(path, source, position)? else {
        return Ok(None);
    };
    let symbols = symbols_for_text(path, source)?;
    let member_owners = member_owners_for_text(path, source).unwrap_or_default();
    Ok(Some(references_for_symbol_in_text_with_owners(
        path,
        source,
        &symbol,
        &symbols,
        include_declaration,
        &member_owners,
    )?))
}

pub fn symbol_at_project_position(
    project: &Project,
    path: &Path,
    source: &str,
    position: TextPosition,
    source_overrides: &[(PathBuf, String)],
) -> Result<Option<SemanticSymbol>, Diagnostic> {
    let overrides = overrides_with_current(path, source, source_overrides);
    let symbols = accessible_symbols_for_document(project, path, source, &overrides)?;
    let member_owners =
        member_owners_for_document(project, path, source, &overrides).unwrap_or_default();
    let owner = member_owners
        .iter()
        .find(|member| range_contains(member.range, position))
        .map(|member| member.owner.as_str());
    resolve_symbol_at_position_with_owner(path, source, position, symbols, owner)
}

pub fn definition_for_project_text(
    project: &Project,
    path: &Path,
    source: &str,
    position: TextPosition,
    source_overrides: &[(PathBuf, String)],
) -> Result<Option<SemanticLocation>, Diagnostic> {
    if let Some(range) = local_definition_for_text(path, source, position)? {
        return Ok(Some(SemanticLocation {
            path: path.to_path_buf(),
            range,
        }));
    }
    Ok(
        symbol_at_project_position(project, path, source, position, source_overrides)?.map(
            |symbol| SemanticLocation {
                path: symbol.source_path,
                range: symbol.selection_range,
            },
        ),
    )
}

pub fn references_for_project_text(
    project: &Project,
    path: &Path,
    source: &str,
    position: TextPosition,
    include_declaration: bool,
    source_overrides: &[(PathBuf, String)],
) -> Result<Option<Vec<SemanticLocation>>, Diagnostic> {
    references_for_projects_text(
        std::slice::from_ref(project),
        project,
        path,
        source,
        position,
        include_declaration,
        source_overrides,
    )
}

pub fn references_for_workspace_text(
    workspace: &WorkspaceGraph,
    project: &Project,
    path: &Path,
    source: &str,
    position: TextPosition,
    include_declaration: bool,
    source_overrides: &[(PathBuf, String)],
) -> Result<Option<Vec<SemanticLocation>>, Diagnostic> {
    references_for_projects_text(
        &workspace.members,
        project,
        path,
        source,
        position,
        include_declaration,
        source_overrides,
    )
}

fn references_for_projects_text(
    editable_projects: &[Project],
    project: &Project,
    path: &Path,
    source: &str,
    position: TextPosition,
    include_declaration: bool,
    source_overrides: &[(PathBuf, String)],
) -> Result<Option<Vec<SemanticLocation>>, Diagnostic> {
    if let Some(ranges) = local_references_for_text(path, source, position, include_declaration)? {
        return Ok(Some(
            ranges
                .into_iter()
                .map(|range| SemanticLocation {
                    path: path.to_path_buf(),
                    range,
                })
                .collect(),
        ));
    }
    let Some(mut symbol) =
        symbol_at_project_position(project, path, source, position, source_overrides)?
    else {
        return Ok(None);
    };
    let Some(source_path) = editable_source_path(editable_projects, &symbol.source_path) else {
        return Ok(None);
    };
    symbol.source_path = source_path;
    let overrides = overrides_with_current(path, source, source_overrides);
    let mut locations = Vec::new();
    for editable_project in editable_projects {
        for (source_path, source) in project_sources(editable_project, &overrides)? {
            let Ok(mut symbols) = accessible_symbols_for_document(
                editable_project,
                &source_path,
                &source,
                &overrides,
            ) else {
                continue;
            };
            for candidate in &mut symbols {
                if let Some(path) = editable_source_path(editable_projects, &candidate.source_path)
                {
                    candidate.source_path = path;
                }
            }
            let member_owners =
                member_owners_for_document(editable_project, &source_path, &source, &overrides)
                    .unwrap_or_default();
            let Ok(ranges) = references_for_symbol_in_text_with_owners(
                &source_path,
                &source,
                &symbol,
                &symbols,
                include_declaration,
                &member_owners,
            ) else {
                continue;
            };
            for range in ranges {
                let location = SemanticLocation {
                    path: source_path.clone(),
                    range,
                };
                if !locations.contains(&location) {
                    locations.push(location);
                }
            }
        }
    }
    Ok(Some(locations))
}

fn editable_source_path(projects: &[Project], path: &Path) -> Option<PathBuf> {
    for project in projects {
        let source_root = project.root.join("src");
        if is_project_nomo_source(&source_root, path) {
            return Some(path.to_path_buf());
        }
        let Ok(canonical_root) = std::fs::canonicalize(&source_root) else {
            continue;
        };
        let Ok(canonical_path) = std::fs::canonicalize(path) else {
            continue;
        };
        let Ok(relative) = canonical_path.strip_prefix(canonical_root) else {
            continue;
        };
        if path.extension().and_then(|extension| extension.to_str()) == Some("nomo") {
            return Some(source_root.join(relative));
        }
    }
    None
}

fn range_contains(range: TextRange, position: TextPosition) -> bool {
    (position.line > range.start.line
        || (position.line == range.start.line && position.character >= range.start.character))
        && (position.line < range.end.line
            || (position.line == range.end.line && position.character <= range.end.character))
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
        let source = "package app.main\n\n/// Adds numbers.\npub fn add(a: i64, b: i64) -> i64 {\n    return a + b\n}\n\nstruct User {\n    /// User email address.\n    pub email: string\n}\n\nenum Status {\n    /// Ready state.\n    Ready\n    /// Done state.\n    Done(i32)\n}\n\n/// Displayable values.\npub interface Display {\n    /// Converts to text.\n    fn to_string(self) -> string\n}\n\nextern \"C\" {\n    /// Writes a C string.\n    fn puts(message: CString) -> i32\n}\n\nimpl User {\n    pub fn email(self) -> string {\n        return self.email\n    }\n}\n";

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
        assert_eq!(symbols[1].container_name.as_deref(), Some("User"));
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
        assert_eq!(symbols[3].container_name.as_deref(), Some("Status"));
        assert_eq!(symbols[3].signature, "variant Status.Ready");
        assert_eq!(symbols[3].docs, "Ready state.");
        assert_eq!(symbols[4].signature, "variant Status.Done(i32)");
        assert_eq!(symbols[4].docs, "Done state.");
        assert_eq!(symbols[5].kind, SemanticSymbolKind::Interface);
        assert_eq!(symbols[5].signature, "pub interface Display");
        assert_eq!(symbols[5].docs, "Displayable values.");
        assert_eq!(symbols[6].kind, SemanticSymbolKind::InterfaceMethod);
        assert_eq!(symbols[6].container_name.as_deref(), Some("Display"));
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
            "extern \"C\" fn puts(message: CString) -> i32"
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
        assert_eq!(symbols[9].container_name.as_deref(), Some("User"));
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
    fn symbols_preserve_generic_interface_bounds() {
        let source = "package app.main\n\ninterface Display {\n    fn to_string(self) -> string\n}\n\n/// Renders a display value.\npub fn render<T: Display>(value: T) -> string {\n    return value.to_string()\n}\n";

        let symbols = symbols_for_text(Path::new("main.nomo"), source).unwrap();
        let render = symbols
            .iter()
            .find(|symbol| symbol.name == "render")
            .unwrap();

        assert_eq!(
            render.signature,
            "pub fn render<T: Display>(value: T) -> string"
        );
        assert_eq!(render.docs, "Renders a display value.");
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
    fn standalone_definition_resolves_ambiguous_fields_by_receiver_type() {
        let source = "package app.main\n\nstruct User {\n    name: string\n}\n\nstruct Team {\n    name: string\n}\n\nfn read(user: User) -> string {\n    return user.name\n}\n";

        let definition = definition_for_text(
            Path::new("main.nomo"),
            source,
            TextPosition {
                line: 11,
                character: 17,
            },
        )
        .unwrap()
        .unwrap();

        assert_eq!(definition.start.line, 3);
        assert_eq!(definition.start.character, 4);
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
    fn project_definition_uses_the_current_module_import_graph() {
        let project = test_project("semantic_definition_import_graph");
        let main = project.root.join("src/main.nomo");
        let alpha = project.root.join("src/alpha.nomo");
        let zeta = project.root.join("src/zeta.nomo");
        write_source(
            &main,
            "package app.main\n\nimport app.zeta\n\nfn main() -> void {\n    let total: i64 = calculate()\n}\n",
        );
        write_source(
            &alpha,
            "package app.alpha\n\npub fn calculate() -> i64 {\n    return 1\n}\n",
        );
        write_source(
            &zeta,
            "package app.zeta\n\npub fn calculate() -> i64 {\n    return 2\n}\n",
        );

        let source = fs::read_to_string(&main).unwrap();
        let definition = definition_for_project_text(
            &project,
            &main,
            &source,
            TextPosition {
                line: 5,
                character: 24,
            },
            &[],
        )
        .unwrap()
        .unwrap();

        assert_eq!(definition.path, zeta);
        assert_ne!(definition.path, alpha);
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
    fn project_references_exclude_shadowed_locals_and_other_declarations() {
        let project = test_project("semantic_reference_identity");
        let main = project.root.join("src/main.nomo");
        let math = project.root.join("src/math.nomo");
        let other = project.root.join("src/other.nomo");
        write_source(
            &main,
            "package app.main\n\nimport app.math\n\nfn consume(add: i64) -> i64 {\n    return add\n}\n\nfn main() -> void {\n    let total: i64 = add(1, 2)\n}\n",
        );
        write_source(
            &math,
            "package app.math\n\npub fn add(a: i64, b: i64) -> i64 {\n    return a + b\n}\n",
        );
        write_source(
            &other,
            "package app.other\n\nfn add(a: i64, b: i64) -> i64 {\n    return add(a, b)\n}\n",
        );

        let source = fs::read_to_string(&main).unwrap();
        let references = references_for_project_text(
            &project,
            &main,
            &source,
            TextPosition {
                line: 9,
                character: 23,
            },
            true,
            &[],
        )
        .unwrap()
        .unwrap();

        assert_eq!(references.len(), 2, "{references:?}");
        assert!(references.iter().any(|location| location.path == main));
        assert!(references.iter().any(|location| location.path == math));
        assert!(!references.iter().any(|location| location.path == other));
        assert!(!references.iter().any(|location| {
            location.path == main && matches!(location.range.start.line, 4 | 5)
        }));
    }

    #[test]
    fn project_references_skip_unrelated_invalid_modules() {
        let project = test_project("semantic_references_invalid_module");
        let main = project.root.join("src/main.nomo");
        let math = project.root.join("src/math.nomo");
        let broken = project.root.join("src/broken.nomo");
        write_source(
            &main,
            "package app.main\n\nimport app.math\n\nfn main() -> void {\n    let total: i64 = add(1, 2)\n}\n",
        );
        write_source(
            &math,
            "package app.math\n\npub fn add(a: i64, b: i64) -> i64 {\n    return a + b\n}\n",
        );
        write_source(&broken, "package app.broken\n\nfn broken( {\n");

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

        assert_eq!(references.len(), 2);
        assert!(!references.iter().any(|location| location.path == broken));
    }

    #[test]
    fn project_navigation_resolves_local_binding_identity() {
        let project = test_project("semantic_local_binding_navigation");
        let main = project.root.join("src/main.nomo");
        let source = "package app.main\n\nfn main() -> void {\n    let value: i64 = 1\n    io.println(value)\n}\n";
        write_source(&main, source);

        let definition = definition_for_project_text(
            &project,
            &main,
            source,
            TextPosition {
                line: 4,
                character: 16,
            },
            &[],
        )
        .unwrap()
        .unwrap();
        let references = references_for_project_text(
            &project,
            &main,
            source,
            TextPosition {
                line: 4,
                character: 16,
            },
            true,
            &[],
        )
        .unwrap()
        .unwrap();

        assert_eq!(definition.path, main);
        assert_eq!(definition.range.start.line, 3);
        assert_eq!(references.len(), 2);
        assert!(references.iter().all(|location| location.path == main));
    }

    #[test]
    fn project_navigation_resolves_fields_and_methods_by_receiver_type() {
        let project = test_project("semantic_receiver_type_navigation");
        let main = project.root.join("src/main.nomo");
        let source = "package app.main\n\nstruct User {\n    name: string\n}\n\nstruct Team {\n    name: string\n}\n\nimpl User {\n    fn label(self) -> string {\n        return self.name\n    }\n}\n\nimpl Team {\n    fn label(self) -> string {\n        return self.name\n    }\n}\n\nfn main() -> void {\n    let user = User { name: \"Ada\" }\n    let team: Team = Team { name: \"Core\" }\n    let user_name: string = user.name\n    let team_name: string = team.name\n    let user_label: string = user.label()\n    let team_label: string = team.label()\n}\n";
        write_source(&main, source);

        let user_field = definition_for_project_text(
            &project,
            &main,
            source,
            position_of(source, "user.name", 0, "name"),
            &[],
        )
        .unwrap()
        .unwrap();
        let team_field = definition_for_project_text(
            &project,
            &main,
            source,
            position_of(source, "team.name", 0, "name"),
            &[],
        )
        .unwrap()
        .unwrap();
        let user_method = definition_for_project_text(
            &project,
            &main,
            source,
            position_of(source, "user.label", 0, "label"),
            &[],
        )
        .unwrap()
        .unwrap();
        let team_method = definition_for_project_text(
            &project,
            &main,
            source,
            position_of(source, "team.label", 0, "label"),
            &[],
        )
        .unwrap()
        .unwrap();
        let user_literal_field = definition_for_project_text(
            &project,
            &main,
            source,
            position_of(source, "User { name", 0, "name"),
            &[],
        )
        .unwrap()
        .unwrap();

        assert_eq!(user_field.range.start.line, 3);
        assert_eq!(team_field.range.start.line, 7);
        assert_eq!(user_method.range.start.line, 11);
        assert_eq!(team_method.range.start.line, 17);
        assert_eq!(user_literal_field.range.start.line, 3);

        let user_field_references = references_for_project_text(
            &project,
            &main,
            source,
            position_of(source, "user.name", 0, "name"),
            true,
            &[],
        )
        .unwrap()
        .unwrap();
        let user_method_references = references_for_project_text(
            &project,
            &main,
            source,
            position_of(source, "user.label", 0, "label"),
            true,
            &[],
        )
        .unwrap()
        .unwrap();

        assert_eq!(user_field_references.len(), 4, "{user_field_references:?}");
        assert!(
            user_field_references
                .iter()
                .all(|location| !matches!(location.range.start.line, 7 | 18 | 24 | 26))
        );
        assert_eq!(
            user_method_references.len(),
            2,
            "{user_method_references:?}"
        );
        assert!(
            user_method_references
                .iter()
                .all(|location| !matches!(location.range.start.line, 17 | 28))
        );
    }

    #[test]
    fn project_navigation_uses_receiver_types_across_modules() {
        let project = test_project("semantic_cross_module_receiver_type");
        let main = project.root.join("src/main.nomo");
        let models = project.root.join("src/models.nomo");
        let main_source = "package app.main\n\nimport app.models\n\nstruct Team {\n    name: string\n}\n\nimpl Team {\n    fn label(self) -> string {\n        return self.name\n    }\n}\n\nfn main() -> void {\n    let user = User { name: \"Ada\" }\n    let team = Team { name: \"Core\" }\n    let user_name: string = user.name\n    let team_name: string = team.name\n    let user_label: string = user.label()\n    let team_label: string = team.label()\n}\n";
        let models_source = "package app.models\n\npub struct User {\n    pub name: string\n}\n\nimpl User {\n    pub fn label(self) -> string {\n        return self.name\n    }\n}\n";
        write_source(&main, main_source);
        write_source(&models, models_source);

        let field_definition = definition_for_project_text(
            &project,
            &main,
            main_source,
            position_of(main_source, "user.name", 0, "name"),
            &[],
        )
        .unwrap()
        .unwrap();
        let method_definition = definition_for_project_text(
            &project,
            &main,
            main_source,
            position_of(main_source, "user.label", 0, "label"),
            &[],
        )
        .unwrap()
        .unwrap();
        let references = references_for_project_text(
            &project,
            &main,
            main_source,
            position_of(main_source, "user.name", 0, "name"),
            true,
            &[],
        )
        .unwrap()
        .unwrap();

        assert_eq!(field_definition.path, models);
        assert_eq!(field_definition.range.start.line, 3);
        assert_eq!(method_definition.path, models);
        assert_eq!(method_definition.range.start.line, 7);
        assert_eq!(references.len(), 4, "{references:?}");
        assert_eq!(
            references
                .iter()
                .filter(|location| location.path == models)
                .count(),
            2
        );
        assert!(references.iter().all(|location| {
            location.path != main || !matches!(location.range.start.line, 5 | 10 | 16 | 18)
        }));
    }

    #[test]
    fn project_navigation_resolves_pattern_binding_member_types() {
        let project = test_project("semantic_pattern_receiver_type");
        let main = project.root.join("src/main.nomo");
        let source = "package app.main\n\nimport std.option\n\nstruct User {\n    name: string\n}\n\nstruct Team {\n    name: string\n}\n\nfn read(maybe: Option<User>) -> string {\n    let Some(user) = maybe else {\n        panic(\"missing\")\n    }\n    return user.name\n}\n\nfn main() -> void {\n}\n";
        write_source(&main, source);

        let definition = definition_for_project_text(
            &project,
            &main,
            source,
            position_of(source, "user.name", 0, "name"),
            &[],
        )
        .unwrap()
        .unwrap();
        let references = references_for_project_text(
            &project,
            &main,
            source,
            position_of(source, "user.name", 0, "name"),
            true,
            &[],
        )
        .unwrap()
        .unwrap();

        assert_eq!(definition.range.start.line, 5);
        assert_eq!(references.len(), 2, "{references:?}");
        assert!(
            references
                .iter()
                .all(|location| location.range.start.line != 9)
        );
    }

    #[test]
    fn project_navigation_resolves_constrained_generic_methods_to_the_interface() {
        let project = test_project("semantic_interface_receiver_type");
        let main = project.root.join("src/main.nomo");
        let source = "package app.main\n\ninterface Display {\n    fn label(self) -> string\n}\n\nstruct User {\n    name: string\n}\n\nimpl Display for User {\n    fn label(self) -> string {\n        return self.name\n    }\n}\n\nfn render<T: Display>(value: T) -> string {\n    return value.label()\n}\n\nfn main() -> void {\n}\n";
        write_source(&main, source);

        let definition = definition_for_project_text(
            &project,
            &main,
            source,
            position_of(source, "value.label", 0, "label"),
            &[],
        )
        .unwrap()
        .unwrap();
        let references = references_for_project_text(
            &project,
            &main,
            source,
            position_of(source, "value.label", 0, "label"),
            true,
            &[],
        )
        .unwrap()
        .unwrap();

        assert_eq!(definition.range.start.line, 3);
        assert_eq!(references.len(), 2, "{references:?}");
        assert!(
            references
                .iter()
                .all(|location| location.range.start.line != 11)
        );
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

    #[test]
    fn workspace_references_include_dependent_members_and_canonical_overlays() {
        let root = env::temp_dir().join(format!(
            "nomo_semantic_workspace_references_{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let core = root.join("packages/core");
        let cli = root.join("apps/cli");
        fs::create_dir_all(core.join("src")).unwrap();
        fs::create_dir_all(cli.join("src")).unwrap();
        fs::write(
            root.join("nomo.toml"),
            "[workspace]\nmembers = [\"apps/*\", \"packages/*\"]\n",
        )
        .unwrap();
        fs::write(
            core.join("nomo.toml"),
            "[package]\nnamespace = \"fynn\"\nname = \"core\"\nversion = \"0.1.0\"\nedition = \"2026\"\n",
        )
        .unwrap();
        fs::write(
            cli.join("nomo.toml"),
            "[package]\nnamespace = \"fynn\"\nname = \"cli\"\nversion = \"0.1.0\"\nedition = \"2026\"\n\n[dependencies]\ncore = { package = \"fynn/core\", path = \"../../packages/core\" }\n",
        )
        .unwrap();
        let core_main = core.join("src/main.nomo");
        let cli_main = cli.join("src/main.nomo");
        write_source(
            &core_main,
            "package core.main\n\npub fn sub(a: i64, b: i64) -> i64 {\n    return a - b\n}\n\nfn main() -> void {\n}\n",
        );
        write_source(
            &cli_main,
            "package cli.main\n\nimport core.main\n\nfn main() -> void {\n    let total: i64 = add(1, 2)\n}\n",
        );
        let core_overlay = "package core.main\n\npub fn add(a: i64, b: i64) -> i64 {\n    return a + b\n}\n\nfn main() -> void {\n}\n";
        let project = crate::project::discover_project(&core_main).unwrap();
        let workspace = crate::project::discover_workspace(&core_main).unwrap();

        let references = references_for_workspace_text(
            &workspace,
            &project,
            &core_main,
            core_overlay,
            TextPosition {
                line: 2,
                character: 8,
            },
            true,
            &[(core_main.clone(), core_overlay.to_string())],
        )
        .unwrap()
        .unwrap();

        assert!(references.iter().any(|location| {
            location.path == core_main
                && location.range.start
                    == TextPosition {
                        line: 2,
                        character: 7,
                    }
        }));
        assert!(references.iter().any(|location| {
            location.path == cli_main
                && location.range.start
                    == TextPosition {
                        line: 5,
                        character: 21,
                    }
        }));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn workspace_references_keep_external_dependencies_read_only() {
        let root = env::temp_dir().join(format!(
            "nomo_semantic_workspace_external_references_{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let app = root.join("app");
        let external = root.join("external");
        fs::create_dir_all(app.join("src")).unwrap();
        fs::create_dir_all(external.join("src")).unwrap();
        fs::write(root.join("nomo.toml"), "[workspace]\nmembers = [\"app\"]\n").unwrap();
        fs::write(
            app.join("nomo.toml"),
            "[package]\nnamespace = \"fynn\"\nname = \"app\"\nversion = \"0.1.0\"\nedition = \"2026\"\n\n[dependencies]\nexternal = { package = \"other/external\", path = \"../external\" }\n",
        )
        .unwrap();
        fs::write(
            external.join("nomo.toml"),
            "[package]\nnamespace = \"other\"\nname = \"external\"\nversion = \"0.1.0\"\nedition = \"2026\"\n",
        )
        .unwrap();
        let app_main = app.join("src/main.nomo");
        let external_main = external.join("src/main.nomo");
        let app_source = "package app.main\n\nimport external.main\n\nfn main() -> void {\n    let total: i64 = add(1, 2)\n}\n";
        write_source(&app_main, app_source);
        write_source(
            &external_main,
            "package external.main\n\npub fn add(a: i64, b: i64) -> i64 {\n    return a + b\n}\n",
        );
        let project = crate::project::discover_project(&app_main).unwrap();
        let workspace = crate::project::discover_workspace(&app_main).unwrap();

        let references = references_for_workspace_text(
            &workspace,
            &project,
            &app_main,
            app_source,
            TextPosition {
                line: 5,
                character: 23,
            },
            true,
            &[],
        )
        .unwrap();

        assert!(references.is_none());
        fs::remove_dir_all(root).unwrap();
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

    fn position_of(
        source: &str,
        occurrence: &str,
        occurrence_index: usize,
        identifier: &str,
    ) -> TextPosition {
        let byte_offset = source
            .match_indices(occurrence)
            .nth(occurrence_index)
            .map(|(offset, _)| offset)
            .unwrap();
        let identifier_offset = occurrence.find(identifier).unwrap();
        let absolute = byte_offset + identifier_offset;
        let before = &source[..absolute];
        let line = before.bytes().filter(|byte| *byte == b'\n').count() as u32;
        let line_start = before.rfind('\n').map_or(0, |index| index + 1);
        let character = source[line_start..absolute].encode_utf16().count() as u32;
        TextPosition { line, character }
    }

    fn write_source(path: &Path, source: &str) {
        fs::write(path, source).unwrap();
    }
}
