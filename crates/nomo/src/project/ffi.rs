use super::{
    DependencyResolutionOptions, Project,
    dependency_resolution::{
        locked_dependency_graph_and_source_base, validate_project_lock_direct_dependencies,
    },
    modules::{dependency_module_root, resolved_dependency_module_root},
    package_id,
};
use nomo_lockfile::{ResolvedDependency, filter_dependencies_for_target};
use nomo_manifest::{Dependency, FfiLinkMetadata, parse_manifest_at_root};
use nomo_resolver::package_source_files;
use nomo_target::TargetTriple;
use std::collections::BTreeSet;
use std::path::Path;

pub(super) fn project_ffi_link_metadata_with_options(
    project: &Project,
    options: DependencyResolutionOptions,
) -> Result<FfiLinkMetadata, String> {
    let target = TargetTriple::host()?;
    project_ffi_link_metadata_for_target_with_options(project, options, &target)
}

pub(super) fn project_ffi_link_metadata_for_target_with_options(
    project: &Project,
    options: DependencyResolutionOptions,
    target: &TargetTriple,
) -> Result<FfiLinkMetadata, String> {
    let manifest = parse_manifest_at_root(&project.root)?;
    package_source_files(&project.root)?;
    let mut metadata = FfiLinkMetadata::default();
    let mut seen = BTreeSet::new();
    let root_id = package_id(&manifest.package);
    seen.insert(root_id);
    metadata.extend(manifest.ffi_for_target(target));

    if options.locked || (options.offline && project.lock_root().join("nomo.lock").is_file()) {
        let (graph, source_base) = locked_dependency_graph_and_source_base(project)?;
        validate_project_lock_direct_dependencies(project, &graph)?;
        let dependencies = filter_dependencies_for_target(&graph.dependencies, target);
        collect_locked_dependency_ffi_metadata(
            &dependencies,
            &source_base,
            &mut seen,
            &mut metadata,
            target,
        )?;
    } else {
        collect_current_dependency_ffi_metadata(
            &project.root,
            &manifest.dependencies,
            options.offline,
            &mut seen,
            &mut metadata,
            target,
        )?;
    }
    Ok(metadata)
}

fn collect_current_dependency_ffi_metadata(
    base_root: &Path,
    dependencies: &[Dependency],
    offline: bool,
    seen: &mut BTreeSet<String>,
    metadata: &mut FfiLinkMetadata,
    target: &TargetTriple,
) -> Result<(), String> {
    for dependency in dependencies
        .iter()
        .filter(|dependency| dependency.target.matches(target))
    {
        let Some(dep_root) = dependency_module_root(base_root, dependency, offline)? else {
            continue;
        };
        let manifest = parse_manifest_at_root(&dep_root)?;
        package_source_files(&dep_root)?;
        let package_id = package_id(&manifest.package);
        if !seen.insert(package_id) {
            continue;
        }
        let dependencies = manifest.dependencies.clone();
        metadata.extend(manifest.ffi_for_target(target));
        collect_current_dependency_ffi_metadata(
            &dep_root,
            &dependencies,
            offline,
            seen,
            metadata,
            target,
        )?;
    }
    Ok(())
}

fn collect_locked_dependency_ffi_metadata(
    dependencies: &[ResolvedDependency],
    source_base: &Path,
    seen: &mut BTreeSet<String>,
    metadata: &mut FfiLinkMetadata,
    target: &TargetTriple,
) -> Result<(), String> {
    for dependency in dependencies {
        let Some(dep_root) = resolved_dependency_module_root(source_base, dependency)? else {
            continue;
        };
        let manifest = parse_manifest_at_root(&dep_root)?;
        package_source_files(&dep_root)?;
        let package_id = package_id(&manifest.package);
        if seen.insert(package_id) {
            metadata.extend(manifest.ffi_for_target(target));
        }
        collect_locked_dependency_ffi_metadata(
            &dependency.dependencies,
            source_base,
            seen,
            metadata,
            target,
        )?;
    }
    Ok(())
}
