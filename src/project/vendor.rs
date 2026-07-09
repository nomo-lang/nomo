use super::{DependencyVendorOptions, git_cache::locked_git_root};
use nomo_lockfile::{
    DependencyGraph, ResolvedDependency, flatten_dependencies, lock_source_string,
};
use nomo_manifest::{DependencySource, parse_manifest_at_root};
use nomo_resolver::{hex_lower, registry_cached_source_root};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

pub(super) fn locked_or_vendor_source_root(
    base_root: &Path,
    dependency: &ResolvedDependency,
) -> Result<Option<PathBuf>, String> {
    if let Some(dep_root) = locked_source_root(base_root, dependency)? {
        return Ok(Some(dep_root));
    }
    vendored_source_root(base_root, dependency)
}

pub(super) fn vendored_source_root(
    base_root: &Path,
    dependency: &ResolvedDependency,
) -> Result<Option<PathBuf>, String> {
    let vendor_root = base_root.join("vendor");
    let manifest = vendor_root.join("nomo-vendor.toml");
    if !manifest.is_file() {
        return Ok(None);
    }
    let document = parse_vendor_document(&fs::read_to_string(&manifest).map_err(|err| {
        format!(
            "failed to read vendor manifest at {}: {err}",
            manifest.display()
        )
    })?)?;
    let source = lock_source_string(dependency);
    let Some(package) = document.package.into_iter().find(|package| {
        package.id == dependency.package
            && package.alias == dependency.alias
            && package.source == source
            && package.path.is_some()
    }) else {
        return Ok(None);
    };
    let path = package.path.expect("checked above");
    let dep_root = vendor_root.join(&path);
    if !dep_root.exists() {
        return Ok(None);
    }
    let dep_root = fs::canonicalize(&dep_root).map_err(|err| {
        format!(
            "failed to resolve vendored dependency `{}` at {}: {err}",
            dependency.alias,
            dep_root.display()
        )
    })?;
    let dep_manifest = parse_manifest_at_root(&dep_root)?;
    let actual_id = format!(
        "{}/{}",
        dep_manifest.package.namespace, dep_manifest.package.name
    );
    if actual_id != dependency.package {
        return Err(format!(
            "vendored dependency `{}` expected package `{}`, found `{}`",
            dependency.alias, dependency.package, actual_id
        ));
    }
    Ok(Some(dep_root))
}

pub(super) fn write_vendor_directory(
    lock_root: &Path,
    source_base: &Path,
    graphs: &[DependencyGraph],
    options: &DependencyVendorOptions,
) -> Result<PathBuf, String> {
    let vendor_root = if options.dir.is_absolute() {
        options.dir.clone()
    } else {
        lock_root.join(&options.dir)
    };
    if options.sync && vendor_root.exists() {
        fs::remove_dir_all(&vendor_root).map_err(|err| err.to_string())?;
    }
    fs::create_dir_all(&vendor_root).map_err(|err| err.to_string())?;

    let mut entries = BTreeMap::new();
    for graph in graphs {
        for dependency in flatten_dependencies(&graph.dependencies) {
            let entry = vendor_dependency(source_base, &vendor_root, dependency)?;
            entries.insert(
                (entry.id.clone(), entry.alias.clone(), entry.source.clone()),
                entry,
            );
        }
    }

    let document = VendorDocument {
        package: entries.into_values().collect(),
    };
    let manifest_path = vendor_root.join("nomo-vendor.toml");
    fs::write(&manifest_path, render_vendor_document(&document)).map_err(|err| err.to_string())?;
    Ok(vendor_root)
}

fn locked_source_root(
    base_root: &Path,
    dependency: &ResolvedDependency,
) -> Result<Option<PathBuf>, String> {
    let dep_root = match &dependency.source {
        DependencySource::Path { path } => {
            let dep_root = base_root.join(path);
            if !dep_root.exists() {
                return Ok(None);
            }
            fs::canonicalize(&dep_root).map_err(|err| {
                format!(
                    "failed to resolve locked path dependency `{}` at {}: {err}",
                    dependency.alias,
                    base_root.join(path).display()
                )
            })?
        }
        DependencySource::Git { git, .. } => {
            let Some(dep_root) = locked_git_root(base_root, &dependency.package, git)? else {
                return Ok(None);
            };
            dep_root
        }
        DependencySource::Registry { version, registry } => {
            let Some(dep_root) = registry_cached_source_root(
                base_root,
                &dependency.package,
                version,
                registry.as_deref(),
            )?
            else {
                return Ok(None);
            };
            dep_root
        }
    };
    let dep_manifest = parse_manifest_at_root(&dep_root)?;
    let actual_id = format!(
        "{}/{}",
        dep_manifest.package.namespace, dep_manifest.package.name
    );
    if actual_id != dependency.package {
        return Err(format!(
            "locked dependency `{}` expected package `{}`, found `{}`",
            dependency.alias, dependency.package, actual_id
        ));
    }
    Ok(Some(dep_root))
}

fn vendor_dependency(
    source_base: &Path,
    vendor_root: &Path,
    dependency: &ResolvedDependency,
) -> Result<VendorPackage, String> {
    let source = lock_source_string(dependency);
    match &dependency.source {
        DependencySource::Registry { .. } => {
            let Some(source_root) = locked_source_root(source_base, dependency)? else {
                return Ok(VendorPackage {
                    id: dependency.package.clone(),
                    alias: dependency.alias.clone(),
                    source,
                    path: None,
                    checksum: dependency.checksum.clone(),
                    skipped: Some("registry source archive is not cached".to_string()),
                });
            };
            let relative = vendor_relative_path(dependency);
            let target = vendor_root.join(&relative);
            copy_package_source(&source_root, &target)?;
            Ok(VendorPackage {
                id: dependency.package.clone(),
                alias: dependency.alias.clone(),
                source,
                path: Some(relative),
                checksum: dependency.checksum.clone(),
                skipped: None,
            })
        }
        DependencySource::Path { .. } | DependencySource::Git { .. } => {
            let Some(source_root) = locked_source_root(source_base, dependency)? else {
                return Err(format!(
                    "cannot vendor dependency `{}` because its locked source is missing",
                    dependency.alias
                ));
            };
            let relative = vendor_relative_path(dependency);
            let target = vendor_root.join(&relative);
            copy_package_source(&source_root, &target)?;
            Ok(VendorPackage {
                id: dependency.package.clone(),
                alias: dependency.alias.clone(),
                source,
                path: Some(relative),
                checksum: dependency.checksum.clone(),
                skipped: None,
            })
        }
    }
}

fn vendor_relative_path(dependency: &ResolvedDependency) -> String {
    let mut path = PathBuf::new();
    for part in dependency.package.split('/') {
        path.push(part);
    }
    path.push(vendor_source_dir_name(dependency));
    path.to_string_lossy().replace('\\', "/")
}

fn vendor_source_dir_name(dependency: &ResolvedDependency) -> String {
    match &dependency.source {
        DependencySource::Registry { version, .. } => version.clone(),
        DependencySource::Path { .. } => "path".to_string(),
        DependencySource::Git { git, rev, .. } => rev
            .as_deref()
            .map(short_revision)
            .map(|rev| format!("git-{rev}"))
            .unwrap_or_else(|| format!("git-{}", short_hash(git))),
    }
}

fn short_revision(rev: &str) -> String {
    rev.chars().take(12).collect()
}

fn short_hash(value: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(value.as_bytes());
    hex_lower(&hasher.finalize()).chars().take(12).collect()
}

fn copy_package_source(source_root: &Path, target: &Path) -> Result<(), String> {
    if target.exists() {
        fs::remove_dir_all(target).map_err(|err| err.to_string())?;
    }
    fs::create_dir_all(target).map_err(|err| err.to_string())?;
    fs::copy(source_root.join("nomo.toml"), target.join("nomo.toml")).map_err(|err| {
        format!(
            "failed to copy {} to {}: {err}",
            source_root.join("nomo.toml").display(),
            target.join("nomo.toml").display()
        )
    })?;
    let source_src = source_root.join("src");
    if source_src.is_dir() {
        copy_dir_recursive(&source_src, &target.join("src"))?;
    }
    Ok(())
}

fn copy_dir_recursive(source: &Path, target: &Path) -> Result<(), String> {
    fs::create_dir_all(target).map_err(|err| err.to_string())?;
    for entry in fs::read_dir(source).map_err(|err| err.to_string())? {
        let entry = entry.map_err(|err| err.to_string())?;
        let source_path = entry.path();
        let target_path = target.join(entry.file_name());
        if source_path.is_dir() {
            copy_dir_recursive(&source_path, &target_path)?;
        } else if source_path.is_file() {
            fs::copy(&source_path, &target_path).map_err(|err| {
                format!(
                    "failed to copy {} to {}: {err}",
                    source_path.display(),
                    target_path.display()
                )
            })?;
        }
    }
    Ok(())
}

fn render_vendor_document(document: &VendorDocument) -> String {
    let mut out = String::from("# This file is generated by `nomo deps vendor`.\n\n");
    out.push_str(&toml::to_string(document).expect("vendor document should serialize"));
    out
}

fn parse_vendor_document(text: &str) -> Result<VendorDocument, String> {
    toml::from_str(text).map_err(|err| format!("failed to parse nomo-vendor.toml as TOML: {err}"))
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct VendorDocument {
    #[serde(default)]
    package: Vec<VendorPackage>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct VendorPackage {
    id: String,
    alias: String,
    source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    checksum: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    skipped: Option<String>,
}
