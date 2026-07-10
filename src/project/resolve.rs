use super::{
    DependencyResolutionOptions, Project, WorkspaceGraph,
    dependency_resolution::{resolve_dependency_graph_for_lock, validate_project_lock},
};
use nomo_lockfile::{render_lockfile, render_workspace_lockfile};
use std::fs;
use std::path::PathBuf;

pub fn resolve_project_dependencies(project: &Project) -> Result<PathBuf, String> {
    resolve_project_dependencies_with_options(project, DependencyResolutionOptions::default())
}

pub fn resolve_project_dependencies_with_options(
    project: &Project,
    options: DependencyResolutionOptions,
) -> Result<PathBuf, String> {
    let lock_path = project.lock_root().join("nomo.lock");
    if options.locked {
        validate_project_lock(project)?;
        return Ok(lock_path);
    }
    let graph = resolve_dependency_graph_for_lock(
        &project.root,
        Some(&project.root),
        Some(&project.root),
        options.offline,
    )?;
    let lock = render_lockfile(&graph);
    fs::write(&lock_path, lock).map_err(|err| err.to_string())?;
    Ok(lock_path)
}

pub fn resolve_workspace_dependencies(workspace: &WorkspaceGraph) -> Result<PathBuf, String> {
    resolve_workspace_dependencies_with_options(workspace, DependencyResolutionOptions::default())
}

pub fn resolve_workspace_dependencies_with_options(
    workspace: &WorkspaceGraph,
    options: DependencyResolutionOptions,
) -> Result<PathBuf, String> {
    let lock_path = workspace.root.join("nomo.lock");
    if options.locked {
        for project in &workspace.members {
            validate_project_lock(project)?;
        }
        return Ok(lock_path);
    }
    let mut graphs = Vec::new();
    for project in &workspace.members {
        graphs.push(resolve_dependency_graph_for_lock(
            &project.root,
            Some(&workspace.root),
            Some(&workspace.root),
            options.offline,
        )?);
    }
    let lock = render_workspace_lockfile(&graphs)?;
    fs::write(&lock_path, lock).map_err(|err| err.to_string())?;
    Ok(lock_path)
}
