use super::discover_project;
use crate::compiler::expected_module_package;
use crate::{Diagnostic, Suggestion};
use crate::{lexer, parser};
use nomo_manifest::parse_manifest_at_root;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleRootMigrationResult {
    pub root: PathBuf,
    pub updated_files: Vec<PathBuf>,
}

#[derive(Debug)]
struct FileUpdate {
    path: PathBuf,
    content: String,
    original: String,
}

pub fn migrate_project_module_roots(
    path: &Path,
    check: bool,
) -> Result<ModuleRootMigrationResult, String> {
    let project = discover_project(path)?;
    let manifest = parse_manifest_at_root(&project.root)?;
    let dependency_aliases = manifest
        .dependencies
        .iter()
        .map(|dependency| dependency.alias.as_str())
        .collect::<BTreeSet<_>>();
    let mut sources = Vec::new();
    collect_sources(&project.root.join("src"), &mut sources)?;
    sources.sort();

    let mut updates = Vec::new();
    for source_path in sources {
        let source = fs::read_to_string(&source_path)
            .map_err(|err| format!("failed to read {}: {err}", source_path.display()))?;
        let tokens = lexer::lex(&source_path, &source).map_err(|diagnostic| diagnostic.human())?;
        let ast = parser::parse(&source_path, &tokens).map_err(|diagnostic| diagnostic.human())?;
        let expected = expected_module_package(
            &project.root.join("src"),
            &project.module_root,
            &source_path,
        )?;
        let migrated = migrate_source(
            &source_path,
            &source,
            &ast.package,
            &ast.imports,
            &expected,
            &project.module_root,
            &dependency_aliases,
        )?;
        if migrated == source {
            continue;
        }
        let migrated_tokens =
            lexer::lex(&source_path, &migrated).map_err(|diagnostic| diagnostic.human())?;
        let migrated_ast = parser::parse(&source_path, &migrated_tokens)
            .map_err(|diagnostic| diagnostic.human())?;
        if migrated_ast.package != expected {
            return Err(format!(
                "module-root migration did not produce `package {}` in {}",
                expected.join("."),
                source_path.display()
            ));
        }
        updates.push(FileUpdate {
            path: source_path,
            content: migrated,
            original: source,
        });
    }

    if check && !updates.is_empty() {
        return Err(format!(
            "warning[W0904]: module-root migration required for: {}",
            updates
                .iter()
                .map(|update| update.path.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    if !check {
        replace_files_atomically(&updates)?;
    }
    Ok(ModuleRootMigrationResult {
        root: project.root,
        updated_files: updates.into_iter().map(|update| update.path).collect(),
    })
}

pub fn module_root_migration_diagnostics(
    project: &super::Project,
) -> Result<Vec<Diagnostic>, String> {
    let mut sources = Vec::new();
    collect_sources(&project.root.join("src"), &mut sources)?;
    sources.sort();
    let mut diagnostics = Vec::new();
    for path in sources {
        let source = fs::read_to_string(&path)
            .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
        let Ok(tokens) = lexer::lex(&path, &source) else {
            continue;
        };
        let Ok(ast) = parser::parse(&path, &tokens) else {
            continue;
        };
        let expected =
            expected_module_package(&project.root.join("src"), &project.module_root, &path)?;
        if !is_legacy_package(&ast.package, &expected, &project.module_root) {
            continue;
        }
        let actual_text = ast.package.join(".");
        let expected_text = expected.join(".");
        let (line, column, line_text) = declaration_location(&source, "package", &actual_text);
        let mut diagnostic = Diagnostic::warning(
            "W0904",
            format!(
                "legacy module declaration `package {actual_text}` is accepted for one development snapshot; use `package {expected_text}`"
            ),
            &path,
            line,
            column,
            actual_text.len(),
            line_text,
        )
        .with_expected_found(expected_text.clone(), actual_text);
        diagnostic.suggestions.push(Suggestion {
            line,
            column,
            length: ast.package.join(".").len(),
            text: expected_text.clone(),
            description: format!("replace the declaration with `package {expected_text}`"),
        });
        diagnostics.push(diagnostic);
    }
    Ok(diagnostics)
}

fn is_legacy_package(actual: &[String], expected: &[String], module_root: &str) -> bool {
    if expected.len() == 1 {
        return actual == ["app", "main"] || actual == [module_root, "main"];
    }
    let mut legacy = vec!["app".to_string()];
    legacy.extend(expected[1..].iter().cloned());
    actual == legacy
}

fn declaration_location(source: &str, keyword: &str, value: &str) -> (usize, usize, String) {
    for (index, line) in source.lines().enumerate() {
        if declaration_value(line, keyword) != Some(value) {
            continue;
        }
        if let Some(column) = line.find(value) {
            return (index + 1, column + 1, line.to_string());
        }
    }
    (1, 1, source.lines().next().unwrap_or_default().to_string())
}

fn collect_sources(root: &Path, sources: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries = fs::read_dir(root)
        .map_err(|err| format!("failed to read source directory {}: {err}", root.display()))?;
    for entry in entries {
        let entry =
            entry.map_err(|err| format!("failed to read entry in {}: {err}", root.display()))?;
        let path = entry.path();
        if path.is_dir() {
            collect_sources(&path, sources)?;
        } else if path.extension().and_then(|extension| extension.to_str()) == Some("nomo") {
            sources.push(path);
        }
    }
    Ok(())
}

fn migrate_source(
    path: &Path,
    source: &str,
    actual_package: &[String],
    imports: &[Vec<String>],
    expected_package: &[String],
    module_root: &str,
    dependency_aliases: &BTreeSet<&str>,
) -> Result<String, String> {
    let mut replacements = vec![(
        "package",
        actual_package.join("."),
        expected_package.join("."),
    )];
    for import in imports {
        let Some(first) = import.first().map(String::as_str) else {
            continue;
        };
        if dependency_aliases.contains(first) {
            continue;
        }
        let replacement = if first == "app" {
            let suffix = if import.get(1).is_some_and(|segment| segment == "main") {
                &import[2..]
            } else {
                &import[1..]
            };
            let mut canonical = vec![module_root.to_string()];
            canonical.extend_from_slice(suffix);
            Some(canonical)
        } else if first == module_root && import.get(1).is_some_and(|segment| segment == "main") {
            let mut canonical = vec![module_root.to_string()];
            canonical.extend_from_slice(&import[2..]);
            Some(canonical)
        } else {
            None
        };
        if let Some(replacement) = replacement {
            replacements.push(("import", import.join("."), replacement.join(".")));
        }
    }

    let newline = if source.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    let mut lines = source.lines().map(str::to_string).collect::<Vec<_>>();
    let trailing_newline = source.ends_with('\n');
    for (keyword, before, after) in replacements {
        if before == after {
            continue;
        }
        let line = lines
            .iter_mut()
            .find(|line| declaration_value(line, keyword).is_some_and(|value| value == before))
            .ok_or_else(|| {
                format!(
                    "cannot safely rewrite multiline or non-canonical `{keyword} {before}` in {}",
                    path.display()
                )
            })?;
        let value_start = line.find(&before).ok_or_else(|| {
            format!(
                "cannot locate `{keyword} {before}` text in {}",
                path.display()
            )
        })?;
        line.replace_range(value_start..value_start + before.len(), &after);
    }
    let mut migrated = lines.join(newline);
    if trailing_newline {
        migrated.push_str(newline);
    }
    Ok(migrated)
}

fn declaration_value<'a>(line: &'a str, keyword: &str) -> Option<&'a str> {
    let trimmed = line.trim_start();
    let rest = trimmed.strip_prefix(keyword)?;
    if !rest.starts_with(char::is_whitespace) {
        return None;
    }
    rest.split_whitespace().next()
}

fn replace_files_atomically(updates: &[FileUpdate]) -> Result<(), String> {
    if updates.is_empty() {
        return Ok(());
    }
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| err.to_string())?
        .as_nanos();
    let mut temporary = Vec::new();
    let mut backups = Vec::new();
    for (index, update) in updates.iter().enumerate() {
        let file_name = update
            .path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("source");
        let path = update.path.with_file_name(format!(
            ".{file_name}.module-roots-{}-{nonce}-{index}.tmp",
            std::process::id()
        ));
        let backup = update.path.with_file_name(format!(
            ".{file_name}.module-roots-{}-{nonce}-{index}.bak",
            std::process::id()
        ));
        if let Err(err) = fs::write(&path, &update.content) {
            for temporary in &temporary {
                let _ = fs::remove_file(temporary);
            }
            return Err(format!(
                "failed to prepare module-root migration output {}: {err}",
                path.display()
            ));
        }
        temporary.push(path);
        backups.push(backup);
    }
    for (index, ((update, temporary_path), backup_path)) in
        updates.iter().zip(&temporary).zip(&backups).enumerate()
    {
        if let Err(err) = fs::rename(&update.path, backup_path) {
            let rollback = rollback_updates(&updates[..index], &backups[..index]);
            for temporary in &temporary[index..] {
                let _ = fs::remove_file(temporary);
            }
            return Err(format!(
                "failed to prepare {} for module-root migration: {err}{}",
                update.path.display(),
                rollback
                    .err()
                    .map(|message| format!("; rollback also failed: {message}"))
                    .unwrap_or_default()
            ));
        }
        if let Err(err) = fs::rename(temporary_path, &update.path) {
            let current_restore = fs::rename(backup_path, &update.path).map_err(|restore| {
                format!(
                    "failed to restore {} after replacement error: {restore}",
                    update.path.display()
                )
            });
            let rollback = rollback_updates(&updates[..index], &backups[..index]);
            for temporary in &temporary[index + 1..] {
                let _ = fs::remove_file(temporary);
            }
            return Err(format!(
                "failed to replace {} during module-root migration: {err}{}{}",
                update.path.display(),
                current_restore
                    .err()
                    .map(|message| format!("; {message}"))
                    .unwrap_or_default(),
                rollback
                    .err()
                    .map(|message| format!("; rollback also failed: {message}"))
                    .unwrap_or_default()
            ));
        }
    }
    for backup in backups {
        let _ = fs::remove_file(backup);
    }
    Ok(())
}

fn rollback_updates(updates: &[FileUpdate], backups: &[PathBuf]) -> Result<(), String> {
    let mut failures = Vec::new();
    for (update, backup) in updates.iter().zip(backups).rev() {
        if let Err(err) = fs::remove_file(&update.path) {
            failures.push(format!("failed to remove {}: {err}", update.path.display()));
            continue;
        }
        if let Err(err) = fs::rename(backup, &update.path) {
            failures.push(format!(
                "failed to restore {} from {}: {err}",
                update.path.display(),
                backup.display()
            ));
            if let Err(write_error) = fs::write(&update.path, &update.original) {
                failures.push(format!(
                    "failed to rewrite {} from memory: {write_error}",
                    update.path.display()
                ));
            }
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures.join("; "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "nomo-module-root-migration-{name}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    fn write_project(root: &Path, main: &str, math: &str) {
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(
            root.join("nomo.toml"),
            "manifest-version = 2\n\n[package]\nnamespace = \"acme\"\nname = \"hello-world\"\nversion = \"0.1.0\"\nedition = \"2026\"\npublish = false\n\n[dependencies.app]\npackage = \"vendor/app\"\nversion = \"1.0.0\"\n",
        )
        .unwrap();
        fs::write(root.join("src/main.nomo"), main).unwrap();
        fs::write(root.join("src/math.nomo"), math).unwrap();
    }

    #[test]
    fn check_write_and_idempotence_preserve_dependency_aliases() {
        let root = temp_root("idempotent");
        write_project(
            &root,
            "package app.main\n\nimport app.remote\n\nfn main() {\n}\n",
            "package app.math\n\nfn value() -> i64 {\n    return 1\n}\n",
        );
        let required = migrate_project_module_roots(&root, true).unwrap_err();
        assert!(
            required.contains("warning[W0904]: module-root migration required"),
            "{required}"
        );
        assert!(
            fs::read_to_string(root.join("src/main.nomo"))
                .unwrap()
                .starts_with("package app.main")
        );
        let project = discover_project(&root).unwrap();
        let warnings = module_root_migration_diagnostics(&project).unwrap();
        assert_eq!(warnings.len(), 2);
        assert!(warnings.iter().all(|warning| warning.code == "W0904"));

        let migrated = migrate_project_module_roots(&root, false).unwrap();
        assert_eq!(migrated.updated_files.len(), 2);
        let main = fs::read_to_string(root.join("src/main.nomo")).unwrap();
        assert!(main.starts_with("package hello_world"));
        assert!(main.contains("import app.remote"));
        assert!(
            fs::read_to_string(root.join("src/math.nomo"))
                .unwrap()
                .starts_with("package hello_world.math")
        );
        assert!(
            migrate_project_module_roots(&root, true)
                .unwrap()
                .updated_files
                .is_empty()
        );
        assert!(
            module_root_migration_diagnostics(&project)
                .unwrap()
                .is_empty()
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn invalid_source_aborts_before_any_write() {
        let root = temp_root("atomic");
        write_project(
            &root,
            "package app.main\n\nfn main() {\n}\n",
            "package app.math\n\nfn broken(\n",
        );
        let before = fs::read_to_string(root.join("src/main.nomo")).unwrap();
        assert!(migrate_project_module_roots(&root, false).is_err());
        assert_eq!(
            fs::read_to_string(root.join("src/main.nomo")).unwrap(),
            before
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn migration_preserves_crlf_line_endings() {
        let root = temp_root("crlf");
        write_project(
            &root,
            "package app.main\r\n\r\nimport app.remote\r\n\r\nfn main() {\r\n}\r\n",
            "package app.math\r\n\r\nfn value() -> i64 {\r\n    return 1\r\n}\r\n",
        );
        migrate_project_module_roots(&root, false).unwrap();
        for source in [
            fs::read_to_string(root.join("src/main.nomo")).unwrap(),
            fs::read_to_string(root.join("src/math.nomo")).unwrap(),
        ] {
            assert!(source.contains("\r\n"));
            assert!(!source.replace("\r\n", "").contains('\n'));
        }
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn migration_never_rewrites_path_dependency_sources() {
        let root = temp_root("dependency-boundary");
        let consumer = root.join("consumer");
        let dependency = root.join("dependency");
        fs::create_dir_all(consumer.join("src")).unwrap();
        fs::create_dir_all(dependency.join("src")).unwrap();
        fs::write(
            consumer.join("nomo.toml"),
            "manifest-version = 2\n\n[package]\nnamespace = \"acme\"\nname = \"consumer\"\nversion = \"0.1.0\"\nedition = \"2026\"\npublish = false\n\n[dependencies.utils]\npackage = \"vendor/utils\"\npath = \"../dependency\"\n",
        )
        .unwrap();
        fs::write(
            consumer.join("src/main.nomo"),
            "package app.main\n\nimport utils.math\n\nfn main() {\n}\n",
        )
        .unwrap();
        fs::write(
            dependency.join("nomo.toml"),
            "manifest-version = 2\n\n[package]\nnamespace = \"vendor\"\nname = \"utils\"\nversion = \"1.0.0\"\nedition = \"2026\"\npublish = false\n",
        )
        .unwrap();
        let dependency_source = "package app.main\n\npub fn value() -> i64 {\n    return 1\n}\n";
        fs::write(dependency.join("src/main.nomo"), dependency_source).unwrap();

        migrate_project_module_roots(&consumer, false).unwrap();
        assert!(
            fs::read_to_string(consumer.join("src/main.nomo"))
                .unwrap()
                .starts_with("package consumer")
        );
        assert_eq!(
            fs::read_to_string(dependency.join("src/main.nomo")).unwrap(),
            dependency_source
        );
        fs::remove_dir_all(root).unwrap();
    }
}
