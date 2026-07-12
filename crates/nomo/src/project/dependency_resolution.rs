use super::{
    Project,
    dependency_tree::source_description,
    git_cache::{git_head_rev, resolve_git_source, resolve_git_source_offline},
    registry_http::registry_dependency_authorization,
    vendor::locked_or_vendor_source_root,
};
use nomo_graph::DirectedGraph;
use nomo_lockfile::{
    DependencyGraph, ResolvedDependency, parse_lockfile_root,
    validate_locked_source_matches_manifest,
};
use nomo_manifest::{
    Dependency, DependencySource, Manifest, parse_manifest_at_root, relative_path,
};
use nomo_resolver::{package_checksum, resolve_registry_source};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

pub(super) fn resolve_dependency_graph(root: &Path) -> Result<DependencyGraph, String> {
    resolve_dependency_graph_for_lock(root, None, None, false)
}

pub(super) fn resolve_dependency_graph_for_lock(
    root: &Path,
    lock_source_base: Option<&Path>,
    dependency_cache_base: Option<&Path>,
    offline: bool,
) -> Result<DependencyGraph, String> {
    let manifest = parse_manifest_at_root(root)?;
    resolve_dependency_graph_for_manifest(
        root,
        manifest,
        lock_source_base,
        dependency_cache_base,
        offline,
    )
}

pub(super) fn resolve_dependency_graph_for_manifest(
    root: &Path,
    manifest: Manifest,
    lock_source_base: Option<&Path>,
    dependency_cache_base: Option<&Path>,
    offline: bool,
) -> Result<DependencyGraph, String> {
    let root = fs::canonicalize(root).map_err(|err| err.to_string())?;
    let lock_source_base = lock_source_base
        .map(fs::canonicalize)
        .transpose()
        .map_err(|err| err.to_string())?;
    let dependency_cache_base = dependency_cache_base
        .map(fs::canonicalize)
        .transpose()
        .map_err(|err| err.to_string())?;
    let mut package_sources = BTreeMap::new();
    let root_package = format!("{}/{}", manifest.package.namespace, manifest.package.name);
    let mut package_graph = DirectedGraph::new();
    package_graph.add_node(root_package.clone());
    let dependencies = resolve_dependencies(
        &manifest.dependencies,
        &root,
        &root_package,
        &mut package_graph,
        &mut package_sources,
        lock_source_base.as_deref(),
        dependency_cache_base.as_deref(),
        offline,
    )?;
    Ok(DependencyGraph {
        root: manifest.package,
        dependencies,
    })
}

pub(super) fn dependency_graph_from_lockfile(
    root: &Path,
    lock_root: &Path,
) -> Result<DependencyGraph, String> {
    let (graph, _) = dependency_graph_and_source_base_from_lockfile(root, lock_root)?;
    Ok(graph)
}

pub(super) fn locked_dependency_graph_and_source_base(
    project: &Project,
) -> Result<(DependencyGraph, PathBuf), String> {
    dependency_graph_and_source_base_from_lockfile(&project.root, &project.lock_root())
}

pub(super) fn validate_project_lock(project: &Project) -> Result<(), String> {
    let (graph, _) = locked_dependency_graph_and_source_base(project)?;
    validate_project_lock_direct_dependencies(project, &graph)
}

pub(super) fn validate_project_lock_direct_dependencies(
    project: &Project,
    graph: &DependencyGraph,
) -> Result<(), String> {
    let manifest = parse_manifest_at_root(&project.root)?;
    let locked_by_alias = graph
        .dependencies
        .iter()
        .map(|dependency| (dependency.alias.as_str(), dependency))
        .collect::<BTreeMap<_, _>>();
    for dependency in manifest
        .dependencies
        .iter()
        .filter(|dep| dep.alias != "std")
    {
        let locked = locked_by_alias
            .get(dependency.alias.as_str())
            .ok_or_else(|| {
                format!(
                    "nomo.lock is out of date: missing dependency `{}`",
                    dependency.alias
                )
            })?;
        if locked.package != dependency.package {
            return Err(format!(
                "nomo.lock is out of date: dependency `{}` expected package `{}`, found `{}`",
                dependency.alias, dependency.package, locked.package
            ));
        }
        validate_locked_source_matches_manifest(dependency, locked)?;
    }
    Ok(())
}

fn resolve_dependencies(
    dependencies: &[Dependency],
    base_root: &Path,
    current_package: &str,
    package_graph: &mut DirectedGraph<String>,
    package_sources: &mut BTreeMap<String, DependencySource>,
    lock_source_base: Option<&Path>,
    dependency_cache_base: Option<&Path>,
    offline: bool,
) -> Result<Vec<ResolvedDependency>, String> {
    let mut resolved = Vec::new();
    for dependency in dependencies {
        package_graph.add_edge(current_package.to_string(), dependency.package.clone());
        if let Some(cycle) = package_graph.find_cycle() {
            return Err(format!(
                "cyclic package dependency: {}",
                cycle
                    .path()
                    .iter()
                    .map(String::as_str)
                    .collect::<Vec<_>>()
                    .join(" -> ")
            ));
        }
        let (resolved_source, checksum, child_dependencies) = match &dependency.source {
            DependencySource::Path { path } => {
                let dep_root = fs::canonicalize(base_root.join(path)).map_err(|err| {
                    format!(
                        "failed to resolve path dependency `{}` at {}: {err}",
                        dependency.alias,
                        base_root.join(path).display()
                    )
                })?;
                let dep_manifest = parse_manifest_at_root(&dep_root)?;
                let actual_id = format!(
                    "{}/{}",
                    dep_manifest.package.namespace, dep_manifest.package.name
                );
                if actual_id != dependency.package {
                    return Err(format!(
                        "path dependency `{}` expected package `{}`, found `{}`",
                        dependency.alias, dependency.package, actual_id
                    ));
                }
                let child_dependencies = resolve_dependencies(
                    &dep_manifest.dependencies,
                    &dep_root,
                    &dependency.package,
                    package_graph,
                    package_sources,
                    lock_source_base,
                    dependency_cache_base,
                    offline,
                )?;
                let checksum = package_checksum(&dep_root)?;
                let resolved_source = match lock_source_base {
                    Some(lock_source_base) => DependencySource::Path {
                        path: relative_path(lock_source_base, &dep_root)
                            .unwrap_or_else(|| dep_root.clone())
                            .to_string_lossy()
                            .replace('\\', "/"),
                    },
                    None => dependency.source.clone(),
                };
                (resolved_source, Some(checksum), child_dependencies)
            }
            DependencySource::Git {
                git,
                branch,
                tag,
                rev,
            } => {
                let dep_root = if offline {
                    resolve_git_source_offline(
                        dependency_cache_base.unwrap_or(base_root),
                        &dependency.alias,
                        &dependency.package,
                        git,
                        branch.as_deref(),
                        tag.as_deref(),
                        rev.as_deref(),
                    )?
                } else {
                    resolve_git_source(
                        dependency_cache_base.unwrap_or(base_root),
                        &dependency.alias,
                        &dependency.package,
                        git,
                        branch.as_deref(),
                        tag.as_deref(),
                        rev.as_deref(),
                    )?
                };
                let dep_manifest = parse_manifest_at_root(&dep_root)?;
                let actual_id = format!(
                    "{}/{}",
                    dep_manifest.package.namespace, dep_manifest.package.name
                );
                if actual_id != dependency.package {
                    return Err(format!(
                        "git dependency `{}` expected package `{}`, found `{}`",
                        dependency.alias, dependency.package, actual_id
                    ));
                }
                let actual_rev = git_head_rev(&dep_root)?;
                let child_dependencies = resolve_dependencies(
                    &dep_manifest.dependencies,
                    &dep_root,
                    &dependency.package,
                    package_graph,
                    package_sources,
                    lock_source_base,
                    dependency_cache_base,
                    offline,
                )?;
                let checksum = package_checksum(&dep_root)?;
                (
                    DependencySource::Git {
                        git: git.clone(),
                        branch: branch.clone(),
                        tag: tag.clone(),
                        rev: Some(actual_rev),
                    },
                    Some(checksum),
                    child_dependencies,
                )
            }
            DependencySource::Registry { version, registry } => {
                let authorization = registry_dependency_authorization(registry.as_deref())?;
                match resolve_registry_source(
                    dependency_cache_base.unwrap_or(base_root),
                    &dependency.alias,
                    &dependency.package,
                    version,
                    registry.as_deref(),
                    offline,
                    authorization.as_deref(),
                )? {
                    Some(dep_root) => {
                        let dep_manifest = parse_manifest_at_root(&dep_root)?;
                        let actual_id = format!(
                            "{}/{}",
                            dep_manifest.package.namespace, dep_manifest.package.name
                        );
                        if actual_id != dependency.package {
                            return Err(format!(
                                "registry dependency `{}` expected package `{}`, found `{}`",
                                dependency.alias, dependency.package, actual_id
                            ));
                        }
                        let child_dependencies = resolve_dependencies(
                            &dep_manifest.dependencies,
                            &dep_root,
                            &dependency.package,
                            package_graph,
                            package_sources,
                            lock_source_base,
                            dependency_cache_base,
                            offline,
                        )?;
                        let checksum = package_checksum(&dep_root)?;
                        (
                            dependency.source.clone(),
                            Some(checksum),
                            child_dependencies,
                        )
                    }
                    None => (dependency.source.clone(), None, Vec::new()),
                }
            }
        };
        remember_package_source(package_sources, &dependency.package, &resolved_source)?;

        resolved.push(ResolvedDependency {
            alias: dependency.alias.clone(),
            package: dependency.package.clone(),
            source: resolved_source,
            checksum,
            dependencies: child_dependencies,
        });
    }
    Ok(resolved)
}

fn remember_package_source(
    package_sources: &mut BTreeMap<String, DependencySource>,
    package: &str,
    source: &DependencySource,
) -> Result<(), String> {
    if let Some(existing) = package_sources.get(package) {
        if existing != source {
            return Err(format!(
                "package `{}` is required with conflicting sources: {} and {}",
                package,
                source_description(existing),
                source_description(source)
            ));
        }
    } else {
        package_sources.insert(package.to_string(), source.clone());
    }
    Ok(())
}

fn dependency_graph_and_source_base_from_lockfile(
    root: &Path,
    lock_root: &Path,
) -> Result<(DependencyGraph, PathBuf), String> {
    let manifest = parse_manifest_at_root(root)?;
    let lock_path = lock_root.join("nomo.lock");
    if !lock_path.is_file() {
        return Err(format!(
            "nomo.lock is required for locked mode at {}",
            lock_path.display()
        ));
    }
    let text = fs::read_to_string(&lock_path).map_err(|err| err.to_string())?;
    let root_id = format!("{}/{}", manifest.package.namespace, manifest.package.name);
    let parsed = parse_lockfile_root(&text, &root_id)?;
    let checksum_base = if parsed.has_workspace_roots {
        lock_root
    } else {
        root
    };
    let dependencies = parsed.dependencies;
    verify_locked_source_checksums(checksum_base, &dependencies)?;
    Ok((
        DependencyGraph {
            root: manifest.package,
            dependencies,
        },
        checksum_base.to_path_buf(),
    ))
}

fn verify_locked_source_checksums(
    base_root: &Path,
    dependencies: &[ResolvedDependency],
) -> Result<(), String> {
    for dependency in dependencies {
        let Some(dep_root) = locked_or_vendor_source_root(base_root, dependency)? else {
            continue;
        };
        if let Some(expected) = &dependency.checksum {
            let actual = package_checksum(&dep_root)?;
            if &actual != expected {
                return Err(format!(
                    "checksum mismatch for locked package `{}`: expected {}, found {}",
                    dependency.package, expected, actual
                ));
            }
        }
        verify_locked_source_checksums(&dep_root, &dependency.dependencies)?;
    }
    Ok(())
}
