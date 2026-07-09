use super::{Project, discover_project};
use nomo_manifest::{
    WorkspaceContext, manifest_document_has_workspace, parse_manifest_at_root,
    parse_manifest_document, parse_workspace_context,
};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct WorkspaceGraph {
    pub root: PathBuf,
    pub members: Vec<Project>,
    pub default_members: Vec<Project>,
}

pub fn discover_workspace(path: &Path) -> Result<WorkspaceGraph, String> {
    let source_file = path.extension().and_then(|ext| ext.to_str()) == Some("nomo");
    let search_root = if source_file {
        path.parent()
            .ok_or_else(|| format!("source file has no parent: {}", path.display()))?
    } else {
        path
    };
    let root = find_workspace_root(search_root)
        .ok_or_else(|| format!("could not find workspace nomo.toml for {}", path.display()))?;
    let text = fs::read_to_string(root.join("nomo.toml")).map_err(|err| err.to_string())?;
    let document = parse_manifest_document(&text)?;
    let context = parse_workspace_context(&root, &document)?;
    let members = workspace_projects_from_patterns(&context, &context.members)?;
    let default_members = if context.default_members.is_empty() {
        Vec::new()
    } else {
        workspace_projects_from_patterns(&context, &context.default_members)?
    };
    Ok(WorkspaceGraph {
        root,
        members,
        default_members,
    })
}

fn find_workspace_root(start: &Path) -> Option<PathBuf> {
    for candidate in start.ancestors() {
        let manifest = candidate.join("nomo.toml");
        if !manifest.is_file() {
            continue;
        }
        let text = fs::read_to_string(&manifest).ok()?;
        let document = parse_manifest_document(&text).ok()?;
        if manifest_document_has_workspace(&document).ok()? {
            return Some(candidate.to_path_buf());
        }
    }
    None
}

fn workspace_projects_from_patterns(
    context: &WorkspaceContext,
    patterns: &[String],
) -> Result<Vec<Project>, String> {
    if patterns.is_empty() {
        return Ok(Vec::new());
    }

    let mut member_roots = BTreeSet::new();
    for pattern in patterns {
        let mut expanded = expand_workspace_pattern(&context.root, pattern)?;
        expanded.sort();
        if expanded.is_empty() {
            return Err(format!(
                "workspace member pattern `{pattern}` did not match any package"
            ));
        }
        for root in expanded {
            let relative = root
                .strip_prefix(&context.root)
                .unwrap_or(&root)
                .to_string_lossy()
                .replace('\\', "/");
            if workspace_path_is_excluded(&relative, &context.exclude) {
                continue;
            }
            if !root.join("nomo.toml").is_file() {
                return Err(format!(
                    "workspace member `{relative}` is missing nomo.toml"
                ));
            }
            member_roots.insert(root);
        }
    }

    member_roots
        .into_iter()
        .map(|root| discover_project(&root))
        .collect()
}

fn expand_workspace_pattern(root: &Path, pattern: &str) -> Result<Vec<PathBuf>, String> {
    let normalized = pattern.replace('\\', "/");
    let parts = normalized
        .split('/')
        .filter(|part| !part.is_empty() && *part != ".")
        .collect::<Vec<_>>();
    let mut out = Vec::new();
    expand_workspace_pattern_parts(root, &parts, &mut out)?;
    Ok(out)
}

fn expand_workspace_pattern_parts(
    base: &Path,
    parts: &[&str],
    out: &mut Vec<PathBuf>,
) -> Result<(), String> {
    let Some((part, rest)) = parts.split_first() else {
        if base.is_dir() {
            out.push(base.to_path_buf());
        }
        return Ok(());
    };

    if part.contains('*') {
        if !base.is_dir() {
            return Ok(());
        }
        for entry in fs::read_dir(base).map_err(|err| err.to_string())? {
            let entry = entry.map_err(|err| err.to_string())?;
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            if wildcard_match(part, name) {
                expand_workspace_pattern_parts(&path, rest, out)?;
            }
        }
    } else {
        expand_workspace_pattern_parts(&base.join(part), rest, out)?;
    }
    Ok(())
}

fn wildcard_match(pattern: &str, value: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    let parts = pattern.split('*').collect::<Vec<_>>();
    if parts.len() == 1 {
        return pattern == value;
    }
    let mut remaining = value;
    for (index, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }
        if index == 0 {
            let Some(stripped) = remaining.strip_prefix(part) else {
                return false;
            };
            remaining = stripped;
        } else if index == parts.len() - 1 && !pattern.ends_with('*') {
            return remaining.ends_with(part);
        } else {
            let Some(pos) = remaining.find(part) else {
                return false;
            };
            remaining = &remaining[pos + part.len()..];
        }
    }
    true
}

fn workspace_path_is_excluded(relative: &str, exclude: &[String]) -> bool {
    exclude.iter().any(|pattern| {
        let pattern = pattern.trim_matches('/');
        relative == pattern || relative.starts_with(&format!("{pattern}/"))
    })
}

pub(super) fn validate_workspace_update_target(
    workspace: &WorkspaceGraph,
    target: &str,
) -> Result<(), String> {
    let mut package_ids = Vec::new();
    for project in &workspace.members {
        let manifest = parse_manifest_at_root(&project.root)?;
        if manifest
            .dependencies
            .iter()
            .any(|dependency| dependency.alias == target || dependency.package == target)
        {
            return Ok(());
        }
        package_ids.push(format!(
            "{}/{}",
            manifest.package.namespace, manifest.package.name
        ));
    }
    Err(format!(
        "dependency update target `{target}` is not a direct dependency of workspace members: {}",
        package_ids.join(", ")
    ))
}
