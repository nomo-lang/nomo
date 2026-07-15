use super::{
    DependencyUpdateOptions, Project, WorkspaceGraph,
    dependency_resolution::{
        resolve_dependency_graph_for_manifest, resolve_dependency_graphs_for_manifests,
    },
    resolve_project_dependencies_with_options, resolve_workspace_dependencies_with_options,
    workspace::validate_workspace_update_target,
};
use nomo_lockfile::{render_lockfile, render_workspace_lockfile};
use nomo_manifest::{
    Dependency, DependencySource, PackageVersion, VersionConstraint, parse_manifest_at_root,
};
use std::fs;
use std::path::PathBuf;

pub fn update_project_dependencies(
    project: &Project,
    target: Option<&str>,
    options: DependencyUpdateOptions,
) -> Result<PathBuf, String> {
    let lock_path = project.lock_root().join("nomo.lock");
    let Some(precise) = options.precise.as_deref() else {
        if let Some(target) = target {
            validate_project_update_target(project, target)?;
        }
        return resolve_project_dependencies_with_options(project, options.resolution);
    };
    let target = target.ok_or_else(|| {
        "nomo deps update --precise requires an alias-or-package target".to_string()
    })?;
    let mut manifest = parse_manifest_at_root(&project.root)?;
    if !apply_precise_update(&mut manifest.dependencies, target, precise)? {
        return Err(format!(
            "dependency update target `{target}` is not a direct dependency of {}/{}",
            manifest.package.namespace, manifest.package.name
        ));
    }
    let graph = resolve_dependency_graph_for_manifest(
        &project.root,
        manifest,
        Some(&project.root),
        Some(&project.root),
        options.resolution.offline,
    )?;
    let lock = render_lockfile(&graph);
    fs::write(&lock_path, lock).map_err(|err| err.to_string())?;
    Ok(lock_path)
}

pub fn update_workspace_dependencies(
    workspace: &WorkspaceGraph,
    target: Option<&str>,
    options: DependencyUpdateOptions,
) -> Result<PathBuf, String> {
    let lock_path = workspace.root.join("nomo.lock");
    let Some(precise) = options.precise.as_deref() else {
        if let Some(target) = target {
            validate_workspace_update_target(workspace, target)?;
        }
        return resolve_workspace_dependencies_with_options(workspace, options.resolution);
    };
    let target = target.ok_or_else(|| {
        "nomo deps update --precise requires an alias-or-package target".to_string()
    })?;

    let mut found = false;
    let mut package_ids = Vec::new();
    let mut manifests = Vec::new();
    for project in &workspace.members {
        let mut manifest = parse_manifest_at_root(&project.root)?;
        package_ids.push(format!(
            "{}/{}",
            manifest.package.namespace, manifest.package.name
        ));
        found |= apply_precise_update(&mut manifest.dependencies, target, precise)?;
        manifests.push((project, manifest));
    }
    if !found {
        return Err(format!(
            "dependency update target `{target}` is not a direct dependency of workspace members: {}",
            package_ids.join(", ")
        ));
    }

    let graphs = resolve_dependency_graphs_for_manifests(
        manifests
            .iter()
            .map(|(project, manifest)| (&*project.root, manifest.clone()))
            .collect(),
        Some(&workspace.root),
        Some(&workspace.root),
        options.resolution.offline,
    )?;

    let lock = render_workspace_lockfile(&graphs)?;
    fs::write(&lock_path, lock).map_err(|err| err.to_string())?;
    Ok(lock_path)
}

fn validate_project_update_target(project: &Project, target: &str) -> Result<(), String> {
    let manifest = parse_manifest_at_root(&project.root)?;
    if manifest
        .dependencies
        .iter()
        .any(|dependency| dependency.alias == target || dependency.package == target)
    {
        Ok(())
    } else {
        Err(format!(
            "dependency update target `{target}` is not a direct dependency of {}/{}",
            manifest.package.namespace, manifest.package.name
        ))
    }
}

fn apply_precise_update(
    dependencies: &mut [Dependency],
    target: &str,
    precise: &str,
) -> Result<bool, String> {
    let mut updated = false;
    for dependency in dependencies {
        if dependency.alias != target && dependency.package != target {
            continue;
        }

        dependency.source = precise_dependency_source(dependency, precise)?;
        updated = true;
    }
    Ok(updated)
}

fn precise_dependency_source(
    dependency: &Dependency,
    precise: &str,
) -> Result<DependencySource, String> {
    match &dependency.source {
        DependencySource::Registry { version, registry } => {
            let requirement = VersionConstraint::parse(version)?;
            let precise_version = PackageVersion::parse(precise).map_err(|error| {
                format!(
                    "dependency `{}` precise version `{precise}` is invalid: {error}",
                    dependency.alias
                )
            })?;
            if !requirement.matches(&precise_version) {
                return Err(format!(
                    "dependency `{}` precise version `{precise}` does not satisfy manifest requirement `{version}`",
                    dependency.alias
                ));
            }
            Ok(DependencySource::Registry {
                version: precise_version.to_string(),
                registry: registry.clone(),
            })
        }
        DependencySource::Git { git, .. } => Ok(DependencySource::Git {
            git: git.clone(),
            branch: None,
            tag: None,
            rev: Some(precise.to_string()),
        }),
        DependencySource::Path { .. } => Err(format!(
            "dependency `{}` uses a path source and cannot be updated with --precise",
            dependency.alias
        )),
    }
}
