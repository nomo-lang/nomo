use super::{
    DependencyResolutionOptions, Project,
    dependency_resolution::{
        dependency_graph_from_lockfile, resolve_dependency_graph, resolve_dependency_graph_for_lock,
    },
};
use nomo_lockfile::{DependencyGraph, ResolvedDependency};
use nomo_manifest::DependencySource;

pub fn dependency_tree(project: &Project) -> Result<String, String> {
    dependency_tree_with_options(project, DependencyResolutionOptions::default())
}

pub fn dependency_tree_with_options(
    project: &Project,
    options: DependencyResolutionOptions,
) -> Result<String, String> {
    let lock_root = project.lock_root();
    let graph = if lock_root.join("nomo.lock").is_file() {
        dependency_graph_from_lockfile(&project.root, &lock_root)?
    } else if options.locked {
        return Err(format!(
            "nomo.lock is required for locked mode at {}",
            lock_root.join("nomo.lock").display()
        ));
    } else {
        resolve_dependency_graph_for_lock(&project.root, None, None, options.offline)?
    };
    Ok(render_dependency_tree(&graph))
}

pub fn dependency_tree_current_sources(project: &Project) -> Result<String, String> {
    Ok(render_dependency_tree(&resolve_dependency_graph(
        &project.root,
    )?))
}

pub(super) fn render_dependency_tree(graph: &DependencyGraph) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "{}/{} {}\n",
        graph.root.namespace, graph.root.name, graph.root.version
    ));
    if graph.dependencies.is_empty() {
        out.push_str("(no dependencies)\n");
        return out;
    }
    render_dependency_tree_entries(&mut out, &graph.dependencies, "");
    out
}

fn render_dependency_tree_entries(
    out: &mut String,
    dependencies: &[ResolvedDependency],
    indent: &str,
) {
    for dependency in dependencies {
        out.push_str(&format!(
            "{indent}+-- {} -> {}{}\n",
            dependency.alias,
            dependency.package,
            source_suffix(&dependency.source)
        ));
        let next_indent = format!("{indent}    ");
        render_dependency_tree_entries(out, &dependency.dependencies, &next_indent);
    }
}

fn source_suffix(source: &DependencySource) -> String {
    match source {
        DependencySource::Registry { version, registry } => {
            if let Some(registry) = registry {
                format!(" {version} (registry {registry})")
            } else {
                format!(" {version} (registry)")
            }
        }
        DependencySource::Path { path } => format!(" (path {path})"),
        DependencySource::Git {
            git,
            branch,
            tag,
            rev,
        } => git_suffix(git, branch.as_deref(), tag.as_deref(), rev.as_deref()),
    }
}

fn git_suffix(git: &str, branch: Option<&str>, tag: Option<&str>, rev: Option<&str>) -> String {
    format!(" ({})", git_description(git, branch, tag, rev))
}

pub(super) fn source_description(source: &DependencySource) -> String {
    match source {
        DependencySource::Registry { version, registry } => {
            if let Some(registry) = registry {
                format!("registry {registry} version {version}")
            } else {
                format!("registry version {version}")
            }
        }
        DependencySource::Path { path } => format!("path {path}"),
        DependencySource::Git {
            git,
            branch,
            tag,
            rev,
        } => git_description(git, branch.as_deref(), tag.as_deref(), rev.as_deref()),
    }
}

fn git_description(
    git: &str,
    branch: Option<&str>,
    tag: Option<&str>,
    rev: Option<&str>,
) -> String {
    match (branch, tag, rev) {
        (Some(branch), None, Some(rev)) => format!("git {git}@{branch}#{rev}"),
        (Some(branch), None, None) => format!("git {git}@{branch}"),
        (None, Some(tag), Some(rev)) => format!("git {git}@{tag}#{rev}"),
        (None, Some(tag), None) => format!("git {git}@{tag}"),
        (None, None, Some(rev)) => format!("git {git}#{rev}"),
        (None, None, None) => format!("git {git}"),
        _ => format!("git {git}"),
    }
}
