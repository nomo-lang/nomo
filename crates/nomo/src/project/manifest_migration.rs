use super::{discover_workspace, find_manifest_root};
use nomo_manifest::{
    ManifestDocumentKind, ManifestMigration, ProjectConfig, WorkspaceContext,
    manifest_document_has_workspace, manifest_document_kind, manifest_schema,
    migrate_manifest_at_root, parse_manifest_document, parse_manifest_document_with_workspace,
    parse_project_config_at_root, parse_project_config_text, parse_workspace_context,
    render_project_config, workspace_root_for_package,
};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestMigrationResult {
    pub root: PathBuf,
    pub updated_files: Vec<PathBuf>,
}

struct PlannedManifest {
    root: PathBuf,
    migration: ManifestMigration,
    document: toml::Value,
}

struct FileUpdate {
    path: PathBuf,
    content: String,
    original: Option<String>,
}

pub fn migrate_project_manifests(
    path: &Path,
    check: bool,
) -> Result<ManifestMigrationResult, String> {
    let selected_root = selected_manifest_root(path)?;
    let selected_document = read_manifest_document(&selected_root)?;
    let project_root = if manifest_document_has_workspace(&selected_document)? {
        selected_root.clone()
    } else {
        workspace_root_for_package(&selected_root)?.unwrap_or_else(|| selected_root.clone())
    };

    let mut roots = BTreeSet::from([project_root.clone()]);
    let project_document = read_manifest_document(&project_root)?;
    if manifest_document_has_workspace(&project_document)? {
        let workspace = discover_workspace(&project_root)?;
        roots.extend(workspace.members.into_iter().map(|member| member.root));
    }

    let mut planned = Vec::new();
    for root in roots {
        let migration = migrate_manifest_at_root(&root)?;
        let document = parse_manifest_document(&migration.manifest)?;
        planned.push(PlannedManifest {
            root,
            migration,
            document,
        });
    }
    validate_migrated_graph(&project_root, &planned)?;

    let config_update = plan_project_config_update(&project_root, &planned)?;
    let mut updates = planned
        .iter()
        .filter(|planned| planned.migration.changed)
        .map(|planned| file_update(planned.root.join("nomo.toml"), &planned.migration.manifest))
        .collect::<Result<Vec<_>, _>>()?;
    if let Some(update) = config_update {
        updates.push(update);
    }
    updates.sort_by(|left, right| left.path.cmp(&right.path));

    if check && !updates.is_empty() {
        return Err(format!(
            "manifest migration required for: {}",
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

    Ok(ManifestMigrationResult {
        root: project_root,
        updated_files: updates.into_iter().map(|update| update.path).collect(),
    })
}

fn selected_manifest_root(path: &Path) -> Result<PathBuf, String> {
    let root = if path.is_file() {
        if path.file_name().and_then(|name| name.to_str()) == Some("nomo.toml") {
            path.parent()
                .ok_or_else(|| format!("manifest has no parent: {}", path.display()))?
                .to_path_buf()
        } else {
            path.parent()
                .and_then(find_manifest_root)
                .ok_or_else(|| format!("could not find nomo.toml for {}", path.display()))?
        }
    } else {
        find_manifest_root(path)
            .ok_or_else(|| format!("could not find nomo.toml for {}", path.display()))?
    };
    fs::canonicalize(&root)
        .map_err(|err| format!("failed to resolve project root {}: {err}", root.display()))
}

fn read_manifest_document(root: &Path) -> Result<toml::Value, String> {
    let path = root.join("nomo.toml");
    let text = fs::read_to_string(&path)
        .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
    parse_manifest_document(&text)
}

fn validate_migrated_graph(project_root: &Path, planned: &[PlannedManifest]) -> Result<(), String> {
    let root = planned
        .iter()
        .find(|planned| planned.root == project_root)
        .ok_or_else(|| "migration plan does not contain its project root".to_string())?;
    let workspace = if manifest_document_has_workspace(&root.document)? {
        Some(parse_workspace_context(project_root, &root.document)?)
    } else {
        None
    };

    let mut identities = BTreeMap::new();
    for planned in planned {
        if manifest_schema(&planned.document)? != nomo_manifest::ManifestSchema::V2 {
            return Err(format!(
                "migration did not produce manifest v2 for {}",
                planned.root.join("nomo.toml").display()
            ));
        }
        let kind = manifest_document_kind(&planned.document)?;
        if kind == ManifestDocumentKind::Workspace {
            parse_workspace_context(&planned.root, &planned.document)?;
            continue;
        }
        let context = migration_workspace_context(project_root, &planned.root, workspace.as_ref());
        let manifest =
            parse_manifest_document_with_workspace(&planned.document, &planned.root, context)?;
        let identity = format!("{}/{}", manifest.package.namespace, manifest.package.name);
        if let Some(existing) = identities.insert(identity.clone(), planned.root.clone()) {
            return Err(format!(
                "migration would create duplicate package identity `{identity}` at {} and {}",
                existing.display(),
                planned.root.display()
            ));
        }
    }
    Ok(())
}

fn migration_workspace_context<'a>(
    project_root: &Path,
    package_root: &Path,
    workspace: Option<&'a WorkspaceContext>,
) -> Option<&'a WorkspaceContext> {
    if workspace.is_some()
        && (package_root == project_root || package_root.starts_with(project_root))
    {
        workspace
    } else {
        None
    }
}

fn plan_project_config_update(
    project_root: &Path,
    planned: &[PlannedManifest],
) -> Result<Option<FileUpdate>, String> {
    let mut migrated_config: Option<ProjectConfig> = None;
    for planned in planned {
        let Some(text) = &planned.migration.project_config else {
            continue;
        };
        let config = parse_project_config_text(text, &planned.root)?;
        match &migrated_config {
            Some(existing) if existing != &config => {
                return Err(format!(
                    "workspace manifests contain conflicting `[trust]` policies; consolidate them before migration (conflict at {})",
                    planned.root.join("nomo.toml").display()
                ));
            }
            Some(_) => {}
            None => migrated_config = Some(config),
        }
    }

    let Some(migrated_config) = migrated_config else {
        return Ok(None);
    };
    let path = project_root.join(".nomo/config.toml");
    if path.is_file() {
        let existing = parse_project_config_at_root(project_root)?;
        if existing != migrated_config {
            return Err(format!(
                "migrated `[trust]` policy conflicts with existing {}",
                path.display()
            ));
        }
        return Ok(None);
    }
    let rendered = render_project_config(&migrated_config, project_root)?;
    file_update(path, &rendered).map(Some)
}

fn file_update(path: PathBuf, content: &str) -> Result<FileUpdate, String> {
    let original = if path.exists() {
        Some(
            fs::read_to_string(&path)
                .map_err(|err| format!("failed to read {}: {err}", path.display()))?,
        )
    } else {
        None
    };
    Ok(FileUpdate {
        path,
        content: content.to_string(),
        original,
    })
}

fn replace_files_atomically(updates: &[FileUpdate]) -> Result<(), String> {
    if updates.is_empty() {
        return Ok(());
    }
    for update in updates {
        if let Some(parent) = update.path.parent() {
            fs::create_dir_all(parent)
                .map_err(|err| format!("failed to create {}: {err}", parent.display()))?;
        }
    }

    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| err.to_string())?
        .as_nanos();
    let mut temporary = Vec::new();
    for (index, update) in updates.iter().enumerate() {
        let file_name = update
            .path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("manifest");
        let path = update.path.with_file_name(format!(
            ".{file_name}.nomo-migrate-{}-{nonce}-{index}.tmp",
            std::process::id()
        ));
        if let Err(err) = fs::write(&path, &update.content) {
            for temporary in &temporary {
                let _ = fs::remove_file(temporary);
            }
            return Err(format!(
                "failed to prepare migration output {}: {err}",
                path.display()
            ));
        }
        temporary.push(path);
    }

    for (index, (update, temporary_path)) in updates.iter().zip(&temporary).enumerate() {
        if let Err(err) = fs::rename(temporary_path, &update.path) {
            rollback_updates(&updates[..index]);
            for temporary in &temporary[index..] {
                let _ = fs::remove_file(temporary);
            }
            return Err(format!(
                "failed to replace {} during manifest migration: {err}",
                update.path.display()
            ));
        }
    }
    Ok(())
}

fn rollback_updates(updates: &[FileUpdate]) {
    for update in updates.iter().rev() {
        match &update.original {
            Some(original) => {
                let _ = fs::write(&update.path, original);
            }
            None => {
                let _ = fs::remove_file(&update.path);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root() -> PathBuf {
        std::env::temp_dir().join(format!(
            "nomo-manifest-migration-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    #[test]
    fn workspace_migration_check_write_and_idempotence_are_transactional() {
        let root = temp_root();
        let app = root.join("apps/cli");
        fs::create_dir_all(app.join("src")).unwrap();
        fs::write(
            root.join("nomo.toml"),
            format!(
                "[workspace]\nmembers = [\"apps/*\"]\ndefault-members = [\"apps/cli\"]\n\n[workspace.package]\nnamespace = \"acme\"\nversion = \"0.1.0\"\nedition = \"2026\"\n\n[trust]\npolicy = \"signed+transparent\"\ntransparency-keys = [\"{}\"]\n",
                "a".repeat(64)
            ),
        )
        .unwrap();
        fs::write(
            app.join("nomo.toml"),
            "[package]\nname = \"cli\"\nnamespace.workspace = true\nversion.workspace = true\nedition.workspace = true\n",
        )
        .unwrap();
        fs::write(app.join("src/main.nomo"), "package app.main\n").unwrap();

        let check = migrate_project_manifests(&root, true).unwrap_err();
        assert!(check.contains("migration required"), "{check}");
        assert!(
            !fs::read_to_string(root.join("nomo.toml"))
                .unwrap()
                .contains("manifest-version")
        );

        let migrated = migrate_project_manifests(&root, false).unwrap();
        assert_eq!(migrated.updated_files.len(), 3);
        assert!(root.join(".nomo/config.toml").is_file());
        assert_eq!(
            parse_manifest_at_root_for_test(&app).schema,
            nomo_manifest::ManifestSchema::V2
        );

        let second = migrate_project_manifests(&root, true).unwrap();
        assert!(second.updated_files.is_empty());
        fs::remove_dir_all(root).unwrap();
    }

    fn parse_manifest_at_root_for_test(root: &Path) -> nomo_manifest::Manifest {
        nomo_manifest::parse_manifest_at_root(root).unwrap()
    }
}
