use super::*;

struct ResolvedImportedModule<'a> {
    source_root: &'a Path,
    module_path: Vec<String>,
    expected_package: Vec<String>,
    canonical_package: Option<&'a str>,
}

pub(super) fn merge_imported_public_api(
    importer_path: &Path,
    ast: &mut SourceFile,
    local_source_root: Option<&Path>,
    local_import_root: Option<&str>,
    local_identity: Option<&ModulePackageIdentity>,
    current_canonical_package: Option<&str>,
    external_modules: &[ExternalModule],
    module_source_overrides: &[(PathBuf, String)],
    module_graph: &mut ModuleGraph,
) -> Result<(), Diagnostic> {
    let imports = ast.imports.clone();
    for import in imports {
        if import.first().is_some_and(|root| root == "std") {
            continue;
        }
        let Some(ResolvedImportedModule {
            source_root,
            module_path,
            expected_package,
            canonical_package,
        }) = resolve_imported_module(
            importer_path,
            &import,
            local_source_root,
            local_import_root,
            local_identity,
            current_canonical_package,
            external_modules,
        )?
        else {
            continue;
        };
        let Some(source_path) = module_source_path(source_root, &module_path) else {
            return Err(Diagnostic::new(
                "E0903",
                format!("could not find module `{}`", import.join(".")),
                importer_path,
                1,
                1,
                import.join(".").len().max(1),
                import.join("."),
            ));
        };
        let source_override = module_source_overrides
            .iter()
            .find(|(path, _)| path == &source_path)
            .map(|(_, source)| source.as_str());
        let source = match source_override {
            Some(source) => source.to_string(),
            None => fs::read_to_string(&source_path).map_err(|err| {
                Diagnostic::new(
                    "E0902",
                    format!("failed to read module `{}`: {err}", source_path.display()),
                    importer_path,
                    1,
                    1,
                    1,
                    "",
                )
            })?,
        };
        let tokens = lexer::lex(&source_path, &source)?;
        let mut module_ast = parser::parse(&source_path, &tokens)?;
        reject_script_body(
            &source_path,
            &module_ast,
            "imported modules cannot contain top-level script statements",
        )?;
        validate_package_declaration(
            &source_path,
            &source,
            &module_ast.package,
            &expected_package,
            expected_package
                .first()
                .map(String::as_str)
                .unwrap_or_default(),
        )?;
        let importer_id = module_id(current_canonical_package, ast.package.clone());
        let imported_id = module_id(canonical_package, module_ast.package.clone());
        let already_loaded = module_graph.contains(&imported_id);
        module_graph.add_module(ModuleNode::new(
            imported_id.clone(),
            source_path.clone(),
            module_ast
                .imports
                .iter()
                .cloned()
                .map(ModuleId::from)
                .collect(),
        ));
        if let Some(cycle) = module_graph.add_dependency(importer_id, imported_id) {
            return Err(Diagnostic::new(
                "E0607",
                format!("cyclic module import: {cycle}"),
                importer_path,
                1,
                1,
                import.join(".").len().max(1),
                import.join("."),
            ));
        }
        if already_loaded {
            continue;
        }
        merge_imported_public_api(
            &source_path,
            &mut module_ast,
            local_source_root,
            local_import_root,
            local_identity,
            canonical_package,
            external_modules,
            module_source_overrides,
            module_graph,
        )?;
        merge_public_items(ast, module_ast);
    }
    Ok(())
}

fn resolve_imported_module<'a>(
    importer_path: &Path,
    import: &[String],
    local_source_root: Option<&'a Path>,
    local_import_root: Option<&str>,
    local_identity: Option<&'a ModulePackageIdentity>,
    current_canonical_package: Option<&str>,
    external_modules: &'a [ExternalModule],
) -> Result<Option<ResolvedImportedModule<'a>>, Diagnostic> {
    let Some(import_root) = import.first() else {
        return Ok(None);
    };
    let legacy_local_import = import_root == "app"
        && local_identity.is_some()
        && !external_modules
            .iter()
            .any(|module| module.import_root == *import_root);
    if local_import_root.is_some_and(|root| root == import_root) || legacy_local_import {
        let Some(source_root) = local_source_root else {
            return Ok(None);
        };
        let mut expected_package = vec![
            local_identity
                .map(|identity| identity.module_root.clone())
                .unwrap_or_else(|| import_root.clone()),
        ];
        expected_package.extend(import[1..].iter().cloned());
        return Ok(Some(ResolvedImportedModule {
            source_root,
            module_path: import[1..].to_vec(),
            expected_package,
            canonical_package: local_identity.map(|identity| identity.canonical_package.as_str()),
        }));
    }
    let module = external_modules
        .iter()
        .find(|module| module.import_root == *import_root)
        .or_else(|| {
            external_modules.iter().find(|module| {
                module.source_import_root == *import_root
                    && current_canonical_package == Some(module.canonical_package.as_str())
            })
        });
    if let Some(module) = module {
        let mut expected_package = vec![module.source_import_root.clone()];
        expected_package.extend(import[1..].iter().cloned());
        return Ok(Some(ResolvedImportedModule {
            source_root: module.source_root.as_path(),
            module_path: import[1..].to_vec(),
            expected_package,
            canonical_package: Some(module.canonical_package.as_str()),
        }));
    }
    if external_modules.iter().any(|module| {
        module.import_root == *import_root || module.source_import_root == *import_root
    }) {
        return Ok(None);
    }
    let _ = importer_path;
    Ok(None)
}

pub fn expected_module_package(
    source_root: &Path,
    module_root: &str,
    source_path: &Path,
) -> Result<Vec<String>, String> {
    let mut relative = match source_path.strip_prefix(source_root) {
        Ok(relative) => relative.to_path_buf(),
        Err(_) => {
            let canonical_root = fs::canonicalize(source_root).map_err(|error| {
                format!(
                    "failed to resolve module root {}: {error}",
                    source_root.display()
                )
            })?;
            let canonical_path = fs::canonicalize(source_path).map_err(|error| {
                format!(
                    "failed to resolve source file {}: {error}",
                    source_path.display()
                )
            })?;
            canonical_path
                .strip_prefix(&canonical_root)
                .map(Path::to_path_buf)
                .map_err(|_| {
                    format!(
                        "source file {} is outside module root {}",
                        source_path.display(),
                        source_root.display()
                    )
                })?
        }
    };
    if relative
        .extension()
        .and_then(|extension| extension.to_str())
        != Some("nomo")
    {
        return Err(format!(
            "source file {} must use the .nomo extension",
            source_path.display()
        ));
    }
    relative.set_extension("");
    let mut segments = relative
        .components()
        .map(|component| {
            component
                .as_os_str()
                .to_str()
                .map(str::to_string)
                .ok_or_else(|| {
                    format!(
                        "source path {} contains a non-UTF-8 module segment",
                        source_path.display()
                    )
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    if segments.last().is_some_and(|segment| segment == "main") {
        segments.pop();
    }
    let mut package = vec![module_root.to_string()];
    package.extend(segments);
    Ok(package)
}

pub(super) fn validate_package_declaration(
    source_path: &Path,
    source: &str,
    actual: &[String],
    expected: &[String],
    module_root: &str,
) -> Result<(), Diagnostic> {
    if actual == expected || is_legacy_module_package(actual, expected, module_root) {
        return Ok(());
    }
    let actual_text = actual.join(".");
    let expected_text = expected.join(".");
    let (line, column, line_text) = package_location(source, &actual_text);
    let mut diagnostic = Diagnostic::new(
        "E0904",
        format!(
            "module package mismatch: {} must declare `package {expected_text}`, found `package {actual_text}`",
            source_path.display()
        ),
        source_path,
        line,
        column,
        actual_text.len().max(1),
        line_text,
    )
    .with_expected_found(expected_text.clone(), actual_text);
    diagnostic.suggestions.push(Suggestion {
        line,
        column,
        length: actual.join(".").len().max(1),
        text: expected_text.clone(),
        description: format!("replace the declaration with `package {expected_text}`"),
    });
    Err(diagnostic)
}

pub(super) fn is_legacy_module_package(
    actual: &[String],
    expected: &[String],
    module_root: &str,
) -> bool {
    if expected.len() == 1 {
        return actual == ["app", "main"] || actual == [module_root, "main"];
    }
    let mut app = vec!["app".to_string()];
    app.extend(expected[1..].iter().cloned());
    actual == app
}

fn package_location(source: &str, package: &str) -> (usize, usize, String) {
    for (line_index, line) in source.lines().enumerate() {
        let Some(package_index) = line.find("package") else {
            continue;
        };
        let remainder = &line[package_index + "package".len()..];
        let Some(value_index) = remainder.find(package) else {
            continue;
        };
        let column = package_index + "package".len() + value_index + 1;
        return (line_index + 1, column, line.to_string());
    }
    (1, 1, source.lines().next().unwrap_or_default().to_string())
}

fn module_id(canonical_package: Option<&str>, package: Vec<String>) -> ModuleId {
    match canonical_package {
        Some(canonical_package) => ModuleId::with_canonical_package(canonical_package, package),
        None => ModuleId::from(package),
    }
}

fn module_source_path(source_root: &Path, module_path: &[String]) -> Option<PathBuf> {
    if module_path.is_empty() || (module_path.len() == 1 && module_path[0] == "main") {
        let main = source_root.join("main.nomo");
        return main.is_file().then_some(main);
    }
    let mut flat = source_root.to_path_buf();
    for segment in module_path {
        flat.push(segment);
    }
    flat.set_extension("nomo");
    if flat.is_file() {
        return Some(flat);
    }
    let mut dir_main = source_root.to_path_buf();
    for segment in module_path {
        dir_main.push(segment);
    }
    dir_main.push("main.nomo");
    dir_main.is_file().then_some(dir_main)
}

fn merge_public_items(ast: &mut SourceFile, module_ast: SourceFile) {
    let public_structs = module_ast
        .structs
        .iter()
        .filter(|item| item.public)
        .map(|item| item.name.clone())
        .collect::<HashSet<_>>();

    ast.imports.extend(module_ast.imports);
    ast.structs
        .extend(module_ast.structs.into_iter().filter(|item| item.public));
    ast.enums
        .extend(module_ast.enums.into_iter().filter(|item| item.public));
    ast.interfaces
        .extend(module_ast.interfaces.into_iter().filter(|item| item.public));
    ast.extern_opaque_types
        .extend(module_ast.extern_opaque_types);
    ast.consts
        .extend(module_ast.consts.into_iter().filter(|item| item.public));
    ast.extern_blocks.extend(module_ast.extern_blocks);
    ast.functions.extend(
        module_ast
            .functions
            .into_iter()
            .filter(|item| item.public && item.name != "main"),
    );
    ast.impls
        .extend(module_ast.impls.into_iter().filter_map(|mut item| {
            let target = item.type_name.path.first()?;
            if !public_structs.contains(target) {
                return None;
            }
            item.methods.retain(|method| method.public);
            if item.methods.is_empty() {
                None
            } else {
                Some(item)
            }
        }));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ModulePackageIdentity, build_module_graph_with_module_identity_and_overrides,
        check_source_text_with_module_identity_and_overrides,
    };
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_root() -> PathBuf {
        std::env::temp_dir().join(format!(
            "nomo-module-roots-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    fn identity() -> ModulePackageIdentity {
        ModulePackageIdentity {
            module_root: "hello_world".to_string(),
            canonical_package: "acme/hello-world".to_string(),
        }
    }

    #[test]
    fn derives_entry_flat_and_directory_main_packages() {
        let root = Path::new("/project/src");
        assert_eq!(
            expected_module_package(root, "hello_world", Path::new("/project/src/main.nomo"))
                .unwrap(),
            ["hello_world"]
        );
        assert_eq!(
            expected_module_package(root, "hello_world", Path::new("/project/src/math.nomo"))
                .unwrap(),
            ["hello_world", "math"]
        );
        assert_eq!(
            expected_module_package(
                root,
                "hello_world",
                Path::new("/project/src/http/main.nomo")
            )
            .unwrap(),
            ["hello_world", "http"]
        );
    }

    #[test]
    fn entry_validation_uses_manifest_root_instead_of_source_prefix() {
        let root = Path::new("/project/src");
        let path = root.join("main.nomo");
        let error = check_source_text_with_module_identity_and_overrides(
            &path,
            "package internally_consistent\n\nfn main() {\n}\n",
            root,
            &identity(),
            &[],
            &[],
            &[],
        )
        .unwrap_err();
        assert_eq!(error.code, "E0904");
        assert_eq!(error.expected.as_deref(), Some("hello_world"));
        assert_eq!(error.found.as_deref(), Some("internally_consistent"));
    }

    #[test]
    fn module_graph_validates_nested_sources_and_keeps_canonical_identity() {
        let root = temp_root();
        let source_root = root.join("src");
        fs::create_dir_all(source_root.join("http")).unwrap();
        let main = source_root.join("main.nomo");
        fs::write(
            &main,
            "package hello_world\n\nimport hello_world.math\nimport hello_world.http\n\nfn main() {\n}\n",
        )
        .unwrap();
        fs::write(
            source_root.join("math.nomo"),
            "package hello_world.math\n\npub fn value() -> i64 {\n    return 1\n}\n",
        )
        .unwrap();
        fs::write(
            source_root.join("http/main.nomo"),
            "package hello_world.http\n\npub fn status() -> i64 {\n    return 200\n}\n",
        )
        .unwrap();
        let source = fs::read_to_string(&main).unwrap();
        let graph = build_module_graph_with_module_identity_and_overrides(
            &main,
            &source,
            &source_root,
            &identity(),
            &[],
            &[],
        )
        .unwrap();
        assert_eq!(graph.modules().len(), 3);
        assert!(
            graph
                .modules()
                .all(|module| module.id.canonical_package() == Some("acme/hello-world"))
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn canonical_package_distinguishes_equal_source_paths() {
        let left = ModuleId::with_canonical_package(
            "acme/first",
            vec!["shared".to_string(), "math".to_string()],
        );
        let right = ModuleId::with_canonical_package(
            "acme/second",
            vec!["shared".to_string(), "math".to_string()],
        );
        assert_ne!(left, right);
    }

    #[test]
    fn consumer_alias_does_not_change_dependency_source_package() {
        let root = temp_root();
        let source_root = root.join("consumer/src");
        let dependency_root = root.join("utils/src");
        fs::create_dir_all(&source_root).unwrap();
        fs::create_dir_all(&dependency_root).unwrap();
        let main = source_root.join("main.nomo");
        fs::write(
            &main,
            "package consumer\n\nimport local_utils.math\n\nfn main() {\n    let result: i64 = value()\n}\n",
        )
        .unwrap();
        fs::write(
            dependency_root.join("math.nomo"),
            "package utils.math\n\npub fn value() -> i64 {\n    return 1\n}\n",
        )
        .unwrap();
        let source = fs::read_to_string(&main).unwrap();
        let external = ExternalModule {
            import_root: "local_utils".to_string(),
            source_import_root: "utils".to_string(),
            canonical_package: "acme/utils".to_string(),
            source_root: dependency_root,
        };
        let program = check_source_text_with_module_identity_and_overrides(
            &main,
            &source,
            &source_root,
            &ModulePackageIdentity {
                module_root: "consumer".to_string(),
                canonical_package: "acme/consumer".to_string(),
            },
            &["local_utils".to_string()],
            &[external],
            &[],
        )
        .unwrap();
        assert!(
            program
                .functions
                .iter()
                .any(|function| function.name == "value")
        );
        fs::remove_dir_all(root).unwrap();
    }
}
