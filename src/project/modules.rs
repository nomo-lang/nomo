use super::{
    DependencyResolutionOptions, Project,
    dependency_resolution::{
        locked_dependency_graph_and_source_base, validate_project_lock_direct_dependencies,
    },
    git_cache::{resolve_git_source, resolve_git_source_offline},
    registry_http::registry_dependency_authorization,
    vendor::{locked_or_vendor_source_root, vendored_source_root},
};
use crate::compiler::{
    ExternalModule, ModuleGraph, build_module_graph_with_overrides as compiler_module_graph,
};
use crate::diagnostic::Diagnostic;
use nomo_lockfile::ResolvedDependency;
use nomo_manifest::{Dependency, DependencySource, parse_manifest_at_root};
use nomo_resolver::{registry_cached_source_root, resolve_registry_source};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct ProjectModuleContext {
    pub local_source_root: PathBuf,
    pub external_import_roots: Vec<String>,
    pub external_modules: Vec<ExternalModule>,
}

pub fn project_module_context(project: &Project) -> Result<ProjectModuleContext, String> {
    project_module_context_with_options(project, DependencyResolutionOptions::default())
}

pub fn project_module_graph(project: &Project) -> Result<ModuleGraph, Diagnostic> {
    project_module_graph_with_overrides(project, &[])
}

pub fn project_module_graph_with_overrides(
    project: &Project,
    source_overrides: &[(PathBuf, String)],
) -> Result<ModuleGraph, Diagnostic> {
    let context = project_module_context(project).map_err(|message| {
        Diagnostic::new(
            "E0901",
            message,
            &project.root.join("nomo.toml"),
            1,
            1,
            1,
            "",
        )
    })?;
    let source = match source_overrides
        .iter()
        .find(|(path, _)| path == &project.main)
    {
        Some((_, source)) => source.clone(),
        None => fs::read_to_string(&project.main).map_err(|err| {
            Diagnostic::new(
                "E0001",
                format!("failed to read source file: {err}"),
                &project.main,
                1,
                1,
                1,
                "",
            )
        })?,
    };
    compiler_module_graph(
        &project.main,
        &source,
        Some(&context.local_source_root),
        &context.external_modules,
        source_overrides,
    )
}

pub fn resolve_module_source_path(
    context: &ProjectModuleContext,
    local_import_root: &str,
    import: &[String],
) -> Option<PathBuf> {
    let (source_root, module_path) =
        resolve_module_source_root(context, local_import_root, import)?;
    module_source_path(source_root, module_path)
}

fn resolve_module_source_root<'a>(
    context: &'a ProjectModuleContext,
    local_import_root: &str,
    import: &'a [String],
) -> Option<(&'a Path, &'a [String])> {
    let first = import.first()?;
    if first == "std" {
        return None;
    }
    if first == local_import_root {
        return Some((context.local_source_root.as_path(), &import[1..]));
    }
    context
        .external_modules
        .iter()
        .find(|module| module.import_root == *first)
        .map(|module| (module.source_root.as_path(), &import[1..]))
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

pub fn project_module_context_with_options(
    project: &Project,
    options: DependencyResolutionOptions,
) -> Result<ProjectModuleContext, String> {
    if options.locked || (options.offline && project.lock_root().join("nomo.lock").is_file()) {
        let (graph, source_base) = locked_dependency_graph_and_source_base(project)?;
        validate_project_lock_direct_dependencies(project, &graph)?;
        return project_module_context_from_resolved_dependencies(
            project,
            &graph.dependencies,
            &source_base,
        );
    }

    let manifest = parse_manifest_at_root(&project.root)?;
    let mut aliases = Vec::new();
    let mut modules = Vec::new();
    for dependency in manifest.dependencies {
        if dependency.alias == "std" {
            continue;
        }
        if let Some(dep_root) = dependency_module_root(&project.root, &dependency, options.offline)?
        {
            modules.push(ExternalModule {
                import_root: dependency.alias.clone(),
                source_root: dep_root.join("src"),
            });
        }
        aliases.push(dependency.alias);
    }
    Ok(ProjectModuleContext {
        local_source_root: project.root.join("src"),
        external_import_roots: aliases,
        external_modules: modules,
    })
}

fn project_module_context_from_resolved_dependencies(
    project: &Project,
    dependencies: &[ResolvedDependency],
    source_base: &Path,
) -> Result<ProjectModuleContext, String> {
    let mut aliases = Vec::new();
    let mut modules = Vec::new();
    for dependency in dependencies {
        if let Some(dep_root) = resolved_dependency_module_root(source_base, dependency)? {
            modules.push(ExternalModule {
                import_root: dependency.alias.clone(),
                source_root: dep_root.join("src"),
            });
        }
        aliases.push(dependency.alias.clone());
    }
    Ok(ProjectModuleContext {
        local_source_root: project.root.join("src"),
        external_import_roots: aliases,
        external_modules: modules,
    })
}

pub(super) fn dependency_module_root(
    base_root: &Path,
    dependency: &Dependency,
    offline: bool,
) -> Result<Option<PathBuf>, String> {
    let dep_root = match &dependency.source {
        DependencySource::Path { path } => {
            fs::canonicalize(base_root.join(path)).map_err(|err| {
                format!(
                    "failed to resolve path dependency `{}` at {}: {err}",
                    dependency.alias,
                    base_root.join(path).display()
                )
            })?
        }
        DependencySource::Git {
            git,
            branch,
            tag,
            rev,
        } => {
            if offline {
                resolve_git_source_offline(
                    base_root,
                    &dependency.alias,
                    &dependency.package,
                    git,
                    branch.as_deref(),
                    tag.as_deref(),
                    rev.as_deref(),
                )?
            } else {
                resolve_git_source(
                    base_root,
                    &dependency.alias,
                    &dependency.package,
                    git,
                    branch.as_deref(),
                    tag.as_deref(),
                    rev.as_deref(),
                )?
            }
        }
        DependencySource::Registry { version, registry } => {
            let authorization = registry_dependency_authorization(registry.as_deref())?;
            let Some(dep_root) = resolve_registry_source(
                base_root,
                &dependency.alias,
                &dependency.package,
                version,
                registry.as_deref(),
                offline,
                authorization.as_deref(),
            )?
            else {
                return Ok(None);
            };
            dep_root
        }
    };
    validate_dependency_package(&dep_root, dependency)?;
    Ok(Some(dep_root))
}

pub(super) fn resolved_dependency_module_root(
    source_base: &Path,
    dependency: &ResolvedDependency,
) -> Result<Option<PathBuf>, String> {
    let dep_root = match &dependency.source {
        DependencySource::Path { path } => {
            let dep_root = source_base.join(path);
            if !dep_root.exists() {
                let Some(vendored) = vendored_source_root(source_base, dependency)? else {
                    return Ok(None);
                };
                return Ok(Some(vendored));
            }
            fs::canonicalize(&dep_root).map_err(|err| {
                format!(
                    "failed to resolve locked path dependency `{}` at {}: {err}",
                    dependency.alias,
                    source_base.join(path).display()
                )
            })?
        }
        DependencySource::Git { .. } => {
            let Some(dep_root) = locked_or_vendor_source_root(source_base, dependency)? else {
                return Ok(None);
            };
            dep_root
        }
        DependencySource::Registry { version, registry } => {
            let Some(dep_root) = registry_cached_source_root(
                source_base,
                &dependency.package,
                version,
                registry.as_deref(),
            )?
            else {
                let Some(vendored) = vendored_source_root(source_base, dependency)? else {
                    return Ok(None);
                };
                return Ok(Some(vendored));
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

fn validate_dependency_package(dep_root: &Path, dependency: &Dependency) -> Result<(), String> {
    let dep_manifest = parse_manifest_at_root(dep_root)?;
    let actual_id = format!(
        "{}/{}",
        dep_manifest.package.namespace, dep_manifest.package.name
    );
    if actual_id != dependency.package {
        return Err(format!(
            "dependency `{}` expected package `{}`, found `{}`",
            dependency.alias, dependency.package, actual_id
        ));
    }
    Ok(())
}
