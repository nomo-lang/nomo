use crate::compiler::{ExternalModule, check_source_text_with_project_modules};
use crate::diagnostic::Diagnostic;
#[cfg(test)]
use nomo_lockfile::parse_lockfile_text;
pub use nomo_lockfile::{DependencyGraph, ResolvedDependency};
use nomo_lockfile::{
    flatten_dependencies, lock_source_string, parse_lockfile_root, render_lockfile,
    render_workspace_lockfile, validate_locked_source_matches_manifest,
};
#[cfg(test)]
use nomo_manifest::parse_manifest_text;
pub use nomo_manifest::{
    Dependency, DependencyAddSpec, DependencySource, FfiLinkMetadata, Manifest, PackageMetadata,
    parse_manifest_at_root,
};
use nomo_manifest::{
    is_package_name, parse_manifest_document, relative_path, remove_dependency_from_manifest,
    render_manifest_document, upsert_registry_dependency, validate_dependency_alias,
    validate_package_id, validate_version_like, workspace_root_for_package,
};
use nomo_resolver::{
    archive_checksum, build_package_archive, hex_lower, package_archive_filename, package_checksum,
    registry_cached_source_root, resolve_registry_source,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

mod build;
mod dependency_tree;
mod ffi;
mod registry_http;
mod running;
mod testing;
mod workspace;

use build::configure_c_compile_command;
pub use build::{
    build_project, build_project_with_diagnostics, build_project_with_options, clean_project,
};
use dependency_tree::{render_dependency_tree, source_description};
use ffi::project_ffi_link_metadata_with_options;
use registry_http::registry_dependency_authorization;
pub use registry_http::{
    RegistryLogin, RegistrySearchResult, add_registry_package_owner, login_registry,
    publish_package_archive, remove_registry_package_owner, search_registry_packages,
    yank_registry_package,
};
pub use running::{
    run_project, run_project_with_args, run_project_with_args_and_diagnostics,
    run_standalone_script_with_args_and_diagnostics,
};
pub use testing::{
    ProjectTestCaseResult, ProjectTestOptions, ProjectTestReport, ProjectTestStatus,
    run_project_tests_with_options,
};
use workspace::validate_workspace_update_target;
pub use workspace::{WorkspaceGraph, discover_workspace};

#[derive(Debug, Clone)]
pub struct Project {
    pub root: PathBuf,
    pub name: String,
    pub main: PathBuf,
    pub workspace_root: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct ProjectModuleContext {
    pub local_source_root: PathBuf,
    pub external_import_roots: Vec<String>,
    pub external_modules: Vec<ExternalModule>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DependencyResolutionOptions {
    pub locked: bool,
    pub offline: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DependencyUpdateOptions {
    pub resolution: DependencyResolutionOptions,
    pub precise: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublishPackage {
    pub package: String,
    pub version: String,
    pub archive_path: PathBuf,
    pub checksum: String,
    pub size: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DependencyVendorOptions {
    pub dir: PathBuf,
    pub sync: bool,
}

impl Default for DependencyVendorOptions {
    fn default() -> Self {
        Self {
            dir: PathBuf::from("vendor"),
            sync: false,
        }
    }
}

#[derive(Debug)]
pub enum BuildError {
    Diagnostic(Diagnostic),
    Message(String),
}

impl BuildError {
    pub fn human(&self) -> String {
        match self {
            BuildError::Diagnostic(diagnostic) => diagnostic.human(),
            BuildError::Message(message) => message.clone(),
        }
    }
}

pub fn create_project(root: &Path, name: &str) -> Result<Project, String> {
    if !is_package_name(name) {
        return Err(format!("invalid project name `{name}`"));
    }
    let project_root = root.join(name);
    if project_root.exists() {
        return Err(format!(
            "destination already exists: {}",
            project_root.display()
        ));
    }
    fs::create_dir_all(project_root.join("src")).map_err(|err| err.to_string())?;
    fs::write(
        project_root.join("nomo.toml"),
        format!(
            "[package]\nnamespace = \"local\"\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2026\"\n"
        ),
    )
    .map_err(|err| err.to_string())?;
    fs::write(
        project_root.join("src/main.nomo"),
        "package app.main\n\nimport std.io\n\nfn greeting() -> string {\n    return \"Hello, Nomo\"\n}\n\nfn main() -> void {\n    let message: string = greeting()\n    io.println(message)\n}\n",
    )
    .map_err(|err| err.to_string())?;
    discover_project(&project_root)
}

pub fn discover_project(path: &Path) -> Result<Project, String> {
    let source_file = path.extension().and_then(|ext| ext.to_str()) == Some("nomo");
    let search_root = if source_file {
        path.parent()
            .ok_or_else(|| format!("source file has no parent: {}", path.display()))?
    } else {
        path
    };
    let root = find_manifest_root(search_root).ok_or_else(|| {
        format!(
            "could not find nomo.toml for {}; use `nomoc` for standalone source files",
            path.display()
        )
    })?;
    let main = if source_file {
        path.to_path_buf()
    } else {
        root.join("src/main.nomo")
    };
    let manifest = parse_manifest_at_root(&root)?;
    let workspace_root = workspace_root_for_package(&root)?;
    Ok(Project {
        root,
        name: manifest.package.name,
        main,
        workspace_root,
    })
}

fn find_manifest_root(start: &Path) -> Option<PathBuf> {
    for candidate in start.ancestors() {
        if candidate.join("nomo.toml").exists() {
            return Some(candidate.to_path_buf());
        }
    }
    None
}

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
    let graph = resolve_dependency_graph_for_lock(&project.root, None, None, options.offline)?;
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

pub fn add_registry_dependency(
    project: &Project,
    spec: DependencyAddSpec,
) -> Result<PathBuf, String> {
    validate_dependency_alias(&spec.alias)?;
    if spec.alias == "std" {
        return Err(
            "dependency alias `std` is reserved for the built-in standard library".to_string(),
        );
    }
    validate_package_id(&spec.package)?;
    validate_version_like(
        &format!("dependency `{}` version", spec.alias),
        &spec.version,
    )?;
    if spec.registry.as_deref().is_some_and(str::is_empty) {
        return Err("--registry requires a non-empty registry endpoint".to_string());
    }

    let manifest_path = project.root.join("nomo.toml");
    let text = fs::read_to_string(&manifest_path).map_err(|err| err.to_string())?;
    let mut document = parse_manifest_document(&text)?;
    upsert_registry_dependency(&mut document, &spec)?;
    let rendered = render_manifest_document(&document)?;
    fs::write(&manifest_path, rendered).map_err(|err| err.to_string())?;

    parse_manifest_at_root(&project.root)?;
    Ok(manifest_path)
}

pub fn remove_dependency(project: &Project, alias: &str) -> Result<PathBuf, String> {
    validate_dependency_alias(alias)?;
    if alias == "std" {
        return Err(
            "dependency alias `std` is reserved for the built-in standard library".to_string(),
        );
    }

    let manifest_path = project.root.join("nomo.toml");
    let text = fs::read_to_string(&manifest_path).map_err(|err| err.to_string())?;
    let mut document = parse_manifest_document(&text)?;
    remove_dependency_from_manifest(&mut document, alias)?;
    let rendered = render_manifest_document(&document)?;
    fs::write(&manifest_path, rendered).map_err(|err| err.to_string())?;

    parse_manifest_at_root(&project.root)?;
    Ok(manifest_path)
}

pub fn prepare_publish_package(
    project: &Project,
    output_dir: Option<&Path>,
) -> Result<PublishPackage, BuildError> {
    let manifest_path = project.root.join("nomo.toml");
    if !manifest_path.is_file() {
        return Err(BuildError::Message(format!(
            "package is missing {}",
            manifest_path.display()
        )));
    }
    let src = project.root.join("src");
    if !src.is_dir() {
        return Err(BuildError::Message(format!(
            "package is missing {}",
            src.display()
        )));
    }

    check_project(project).map_err(BuildError::Diagnostic)?;
    let manifest = parse_manifest_at_root(&project.root).map_err(BuildError::Message)?;
    let archive =
        build_package_archive(&project.root, &manifest.package).map_err(BuildError::Message)?;
    let checksum = archive_checksum(&archive);
    let package = format!("{}/{}", manifest.package.namespace, manifest.package.name);
    let version = manifest.package.version;
    let output_dir = output_dir
        .map(Path::to_path_buf)
        .unwrap_or_else(|| project.root.join("build/package"));
    fs::create_dir_all(&output_dir).map_err(|err| BuildError::Message(err.to_string()))?;
    let archive_path = output_dir.join(package_archive_filename(&package, &version));
    fs::write(&archive_path, &archive).map_err(|err| {
        BuildError::Message(format!("failed to write {}: {err}", archive_path.display()))
    })?;
    Ok(PublishPackage {
        package,
        version,
        archive_path,
        checksum,
        size: archive.len(),
    })
}

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
        None,
        None,
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

    let mut graphs = Vec::new();
    for (project, manifest) in manifests {
        graphs.push(resolve_dependency_graph_for_manifest(
            &project.root,
            manifest,
            Some(&workspace.root),
            Some(&workspace.root),
            options.resolution.offline,
        )?);
    }

    let lock = render_workspace_lockfile(&graphs)?;
    fs::write(&lock_path, lock).map_err(|err| err.to_string())?;
    Ok(lock_path)
}

pub fn vendor_project_dependencies(
    project: &Project,
    options: DependencyVendorOptions,
) -> Result<PathBuf, String> {
    let lock_root = project.lock_root();
    if !lock_root.join("nomo.lock").is_file() {
        resolve_project_dependencies_with_options(project, DependencyResolutionOptions::default())?;
    }
    let (graph, source_base) = locked_dependency_graph_and_source_base(project)?;
    write_vendor_directory(&lock_root, &source_base, &[graph], &options)
}

pub fn vendor_workspace_dependencies(
    workspace: &WorkspaceGraph,
    options: DependencyVendorOptions,
) -> Result<PathBuf, String> {
    if !workspace.root.join("nomo.lock").is_file() {
        resolve_workspace_dependencies_with_options(
            workspace,
            DependencyResolutionOptions::default(),
        )?;
    }
    let mut graphs = Vec::new();
    let mut source_base = workspace.root.clone();
    for project in &workspace.members {
        let (graph, graph_source_base) = locked_dependency_graph_and_source_base(project)?;
        source_base = graph_source_base;
        graphs.push(graph);
    }
    write_vendor_directory(&workspace.root, &source_base, &graphs, &options)
}

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

impl Project {
    fn lock_root(&self) -> PathBuf {
        self.workspace_root
            .clone()
            .unwrap_or_else(|| self.root.clone())
    }
}

pub fn project_module_context(project: &Project) -> Result<ProjectModuleContext, String> {
    project_module_context_with_options(project, DependencyResolutionOptions::default())
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

fn dependency_module_root(
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

fn resolved_dependency_module_root(
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
        DependencySource::Git { git, .. } => {
            let Some(dep_root) = locked_git_root(source_base, dependency, git)? else {
                let Some(vendored) = vendored_source_root(source_base, dependency)? else {
                    return Ok(None);
                };
                return Ok(Some(vendored));
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

pub fn check_project(project: &Project) -> Result<(), Diagnostic> {
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
    let source = fs::read_to_string(&project.main).map_err(|err| {
        Diagnostic::new(
            "E0001",
            format!("failed to read source file: {err}"),
            &project.main,
            1,
            1,
            1,
            "",
        )
    })?;
    check_source_text_with_project_modules(
        &project.main,
        &source,
        Some(&context.local_source_root),
        &context.external_import_roots,
        &context.external_modules,
    )
    .map(|_| ())
}

pub fn project_package_id(project: &Project) -> Result<String, String> {
    let manifest = parse_manifest_at_root(&project.root)?;
    Ok(package_id(&manifest.package))
}

fn package_id(package: &PackageMetadata) -> String {
    format!("{}/{}", package.namespace, package.name)
}

pub fn clean_dependency_cache(path: &Path) -> Result<PathBuf, String> {
    let root = match discover_project(path) {
        Ok(project) => project.lock_root(),
        Err(project_err) => match discover_workspace(path) {
            Ok(workspace) => workspace.root,
            Err(_) => return Err(project_err),
        },
    };
    let cache_dir = root.join(".nomo/deps/git");
    if cache_dir.exists() {
        fs::remove_dir_all(&cache_dir).map_err(|err| err.to_string())?;
    }
    Ok(cache_dir)
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
        DependencySource::Registry { registry, .. } => {
            validate_version_like(
                &format!("dependency `{}` precise version", dependency.alias),
                precise,
            )?;
            Ok(DependencySource::Registry {
                version: precise.to_string(),
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

fn resolve_dependency_graph(root: &Path) -> Result<DependencyGraph, String> {
    resolve_dependency_graph_for_lock(root, None, None, false)
}

fn resolve_dependency_graph_for_lock(
    root: &Path,
    lock_source_base: Option<&Path>,
    git_cache_base: Option<&Path>,
    offline: bool,
) -> Result<DependencyGraph, String> {
    let manifest = parse_manifest_at_root(&root)?;
    resolve_dependency_graph_for_manifest(
        &root,
        manifest,
        lock_source_base.as_deref(),
        git_cache_base.as_deref(),
        offline,
    )
}

fn resolve_dependency_graph_for_manifest(
    root: &Path,
    manifest: Manifest,
    lock_source_base: Option<&Path>,
    git_cache_base: Option<&Path>,
    offline: bool,
) -> Result<DependencyGraph, String> {
    let root = fs::canonicalize(root).map_err(|err| err.to_string())?;
    let lock_source_base = lock_source_base
        .map(fs::canonicalize)
        .transpose()
        .map_err(|err| err.to_string())?;
    let git_cache_base = git_cache_base
        .map(fs::canonicalize)
        .transpose()
        .map_err(|err| err.to_string())?;
    let mut package_sources = BTreeMap::new();
    let mut path_stack = vec![root.clone()];
    let dependencies = resolve_dependencies(
        &manifest.dependencies,
        &root,
        &mut path_stack,
        &mut package_sources,
        lock_source_base.as_deref(),
        git_cache_base.as_deref(),
        offline,
    )?;
    Ok(DependencyGraph {
        root: manifest.package,
        dependencies,
    })
}

fn resolve_dependencies(
    dependencies: &[Dependency],
    base_root: &Path,
    path_stack: &mut Vec<PathBuf>,
    package_sources: &mut BTreeMap<String, DependencySource>,
    lock_source_base: Option<&Path>,
    git_cache_base: Option<&Path>,
    offline: bool,
) -> Result<Vec<ResolvedDependency>, String> {
    let mut resolved = Vec::new();
    for dependency in dependencies {
        let (resolved_source, checksum, child_dependencies) = match &dependency.source {
            DependencySource::Path { path } => {
                let dep_root = fs::canonicalize(base_root.join(path)).map_err(|err| {
                    format!(
                        "failed to resolve path dependency `{}` at {}: {err}",
                        dependency.alias,
                        base_root.join(path).display()
                    )
                })?;
                if path_stack.contains(&dep_root) {
                    return Err(format!(
                        "cyclic path dependency involving `{}` at {}",
                        dependency.package,
                        dep_root.display()
                    ));
                }

                path_stack.push(dep_root.clone());
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
                    path_stack,
                    package_sources,
                    lock_source_base,
                    git_cache_base,
                    offline,
                )?;
                let checksum = package_checksum(&dep_root)?;
                path_stack.pop();
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
                        git_cache_base.unwrap_or(base_root),
                        &dependency.alias,
                        &dependency.package,
                        git,
                        branch.as_deref(),
                        tag.as_deref(),
                        rev.as_deref(),
                    )?
                } else {
                    resolve_git_source(
                        git_cache_base.unwrap_or(base_root),
                        &dependency.alias,
                        &dependency.package,
                        git,
                        branch.as_deref(),
                        tag.as_deref(),
                        rev.as_deref(),
                    )?
                };
                if path_stack.contains(&dep_root) {
                    return Err(format!(
                        "cyclic git dependency involving `{}` at {}",
                        dependency.package,
                        dep_root.display()
                    ));
                }

                path_stack.push(dep_root.clone());
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
                    path_stack,
                    package_sources,
                    lock_source_base,
                    git_cache_base,
                    offline,
                )?;
                let checksum = package_checksum(&dep_root)?;
                path_stack.pop();
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
                    base_root,
                    &dependency.alias,
                    &dependency.package,
                    version,
                    registry.as_deref(),
                    offline,
                    authorization.as_deref(),
                )? {
                    Some(dep_root) => {
                        if path_stack.contains(&dep_root) {
                            return Err(format!(
                                "cyclic registry dependency involving `{}` at {}",
                                dependency.package,
                                dep_root.display()
                            ));
                        }

                        path_stack.push(dep_root.clone());
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
                            path_stack,
                            package_sources,
                            lock_source_base,
                            git_cache_base,
                            offline,
                        )?;
                        let checksum = package_checksum(&dep_root)?;
                        path_stack.pop();
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

fn resolve_git_source(
    base_root: &Path,
    alias: &str,
    package: &str,
    git: &str,
    branch: Option<&str>,
    tag: Option<&str>,
    rev: Option<&str>,
) -> Result<PathBuf, String> {
    let cache_root = base_root.join(".nomo/deps/git");
    fs::create_dir_all(&cache_root).map_err(|err| err.to_string())?;
    let checkout = cache_root.join(git_cache_key(package, git));
    if checkout.exists() {
        run_git_fetch(&checkout, alias)?;
    } else {
        let clone_source = git_clone_source(base_root, git);
        let output = Command::new("git")
            .arg("clone")
            .arg("--quiet")
            .arg(&clone_source)
            .arg(&checkout)
            .output()
            .map_err(|err| format!("failed to run git clone for dependency `{alias}`: {err}"))?;
        if !output.status.success() {
            return Err(format!(
                "failed to clone git dependency `{alias}` from {git}:\n{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            ));
        }
    }

    if let Some(branch) = branch {
        git_checkout(&checkout, alias, "branch", branch)?;
        git_pull_ff_only(&checkout, alias, branch)?;
    } else if let Some(tag) = tag {
        git_checkout(&checkout, alias, "tag", &format!("refs/tags/{tag}"))?;
    } else if let Some(rev) = rev {
        git_checkout(&checkout, alias, "rev", rev)?;
    } else if checkout.exists() {
        checkout_default_branch(&checkout, alias)?;
    }

    fs::canonicalize(&checkout).map_err(|err| err.to_string())
}

fn git_clone_source(base_root: &Path, git: &str) -> PathBuf {
    let path = Path::new(git);
    if path.is_absolute() || git.contains("://") || git.contains(':') {
        path.to_path_buf()
    } else {
        base_root.join(path)
    }
}

fn resolve_git_source_offline(
    base_root: &Path,
    alias: &str,
    package: &str,
    git: &str,
    _branch: Option<&str>,
    _tag: Option<&str>,
    _rev: Option<&str>,
) -> Result<PathBuf, String> {
    let checkout = base_root
        .join(".nomo/deps/git")
        .join(git_cache_key(package, git));
    if checkout.exists() {
        fs::canonicalize(&checkout).map_err(|err| err.to_string())
    } else {
        Err(format!(
            "offline mode cannot fetch git dependency `{alias}` from {git}; missing cached checkout at {}",
            checkout.display()
        ))
    }
}

fn run_git_fetch(checkout: &Path, alias: &str) -> Result<(), String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(checkout)
        .arg("fetch")
        .arg("--tags")
        .arg("--prune")
        .arg("origin")
        .output()
        .map_err(|err| format!("failed to run git fetch for dependency `{alias}`: {err}"))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "failed to fetch git dependency `{alias}`:\n{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ))
    }
}

fn git_checkout(
    checkout: &Path,
    alias: &str,
    selector_name: &str,
    selector: &str,
) -> Result<(), String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(checkout)
        .arg("checkout")
        .arg("--quiet")
        .arg(selector)
        .output()
        .map_err(|err| {
            format!(
                "failed to run git checkout for dependency `{alias}` at {selector_name} `{selector}`: {err}"
            )
        })?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "failed to checkout git dependency `{alias}` at {selector_name} `{selector}`:\n{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ))
    }
}

fn git_pull_ff_only(checkout: &Path, alias: &str, branch: &str) -> Result<(), String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(checkout)
        .arg("pull")
        .arg("--ff-only")
        .arg("--quiet")
        .output()
        .map_err(|err| {
            format!("failed to run git pull for dependency `{alias}` at branch `{branch}`: {err}")
        })?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "failed to pull git dependency `{alias}` at branch `{branch}`:\n{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ))
    }
}

fn checkout_default_branch(checkout: &Path, alias: &str) -> Result<(), String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(checkout)
        .arg("symbolic-ref")
        .arg("--short")
        .arg("refs/remotes/origin/HEAD")
        .output()
        .map_err(|err| {
            format!("failed to resolve default branch for git dependency `{alias}`: {err}")
        })?;
    if !output.status.success() {
        return Ok(());
    }
    let remote_branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let branch = remote_branch
        .strip_prefix("origin/")
        .unwrap_or(&remote_branch)
        .to_string();
    if branch.is_empty() {
        return Ok(());
    }
    git_checkout(checkout, alias, "branch", &branch)?;
    git_pull_ff_only(checkout, alias, &branch)
}

fn git_head_rev(root: &Path) -> Result<String, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .arg("rev-parse")
        .arg("HEAD")
        .output()
        .map_err(|err| format!("failed to resolve git HEAD at {}: {err}", root.display()))?;
    if !output.status.success() {
        return Err(format!(
            "failed to resolve git HEAD at {}:\n{}{}",
            root.display(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn git_cache_key(package: &str, git: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(package.as_bytes());
    hasher.update(b"\0");
    hasher.update(git.as_bytes());
    format!("git-{}", hex_lower(&hasher.finalize()))
}

fn dependency_graph_from_lockfile(
    root: &Path,
    lock_root: &Path,
) -> Result<DependencyGraph, String> {
    let (graph, _) = dependency_graph_and_source_base_from_lockfile(root, lock_root)?;
    Ok(graph)
}

fn locked_dependency_graph_and_source_base(
    project: &Project,
) -> Result<(DependencyGraph, PathBuf), String> {
    dependency_graph_and_source_base_from_lockfile(&project.root, &project.lock_root())
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

fn validate_project_lock(project: &Project) -> Result<(), String> {
    let (graph, _) = locked_dependency_graph_and_source_base(project)?;
    validate_project_lock_direct_dependencies(project, &graph)
}

fn validate_project_lock_direct_dependencies(
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

fn locked_or_vendor_source_root(
    base_root: &Path,
    dependency: &ResolvedDependency,
) -> Result<Option<PathBuf>, String> {
    if let Some(dep_root) = locked_source_root(base_root, dependency)? {
        return Ok(Some(dep_root));
    }
    vendored_source_root(base_root, dependency)
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
            let Some(dep_root) = locked_git_root(base_root, dependency, git)? else {
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

fn vendored_source_root(
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

fn locked_git_root(
    base_root: &Path,
    dependency: &ResolvedDependency,
    git: &str,
) -> Result<Option<PathBuf>, String> {
    let cache_root = base_root.join(".nomo/deps/git");
    if !cache_root.is_dir() {
        return Ok(None);
    }
    let path = cache_root.join(git_cache_key(&dependency.package, git));
    if !path.is_dir() {
        return Ok(None);
    }
    let Ok(manifest) = parse_manifest_at_root(&path) else {
        return Ok(None);
    };
    let actual_id = format!("{}/{}", manifest.package.namespace, manifest.package.name);
    if actual_id != dependency.package {
        return Ok(None);
    }
    let Some(remote_url) = git_remote_url(&path) else {
        return Ok(None);
    };
    let clone_source = git_clone_source(base_root, git)
        .to_string_lossy()
        .replace('\\', "/");
    if remote_url != git && remote_url.replace('\\', "/") != clone_source {
        return Ok(None);
    }
    fs::canonicalize(&path)
        .map(Some)
        .map_err(|err| err.to_string())
}

fn write_vendor_directory(
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

fn git_remote_url(root: &Path) -> Option<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .arg("remote")
        .arg("get-url")
        .arg("origin")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_namespace_first_manifest() {
        let manifest = "[package]\nnamespace = \"fynn\"\nname = \"demo\"\nversion = \"0.1.0\"\nedition = \"2026\"\n\n[dependencies]\nstd = { package = \"nomo-lang/std\", version = \"0.1.0\" }\nutils = { package = \"fynn/utils\", path = \"../utils\" }\n";
        let parsed = parse_manifest_text(manifest, Path::new("demo")).unwrap();

        assert_eq!(parsed.package.namespace, "fynn");
        assert_eq!(parsed.package.name, "demo");
        assert_eq!(parsed.dependencies.len(), 1);
        assert_eq!(parsed.dependencies[0].alias, "utils");
        assert_eq!(parsed.dependencies[0].package, "fynn/utils");
    }

    #[test]
    fn parses_legacy_std_dependency_as_implicit_builtin() {
        let manifest =
            "[package]\nname = \"demo\"\nversion = \"0.1.0\"\n\n[dependencies]\nstd = \"0.1.0\"\n";
        let parsed = parse_manifest_text(manifest, Path::new("demo")).unwrap();

        assert_eq!(parsed.package.namespace, "local");
        assert_eq!(parsed.package.name, "demo");
        assert!(parsed.dependencies.is_empty());
    }

    #[test]
    fn parses_standard_toml_comments_and_escaped_strings() {
        let manifest = r#"# package comment
[package]
namespace = "fynn"
name = "escaped-demo"
version = "0.1.0"
edition = "2026"

[dependencies]
json = { package = "nomo-lang/json", version = "0.1.0", registry = "https://packages.example.com/v1?token=\"dev\"" }

[dependencies.local_utils]
package = "fynn/utils"
path = "../utils"
"#;
        let parsed = parse_manifest_text(manifest, Path::new("demo")).unwrap();

        assert_eq!(parsed.dependencies.len(), 2);
        assert_eq!(parsed.dependencies[0].alias, "json");
        assert_eq!(
            parsed.dependencies[0].source,
            DependencySource::Registry {
                version: "0.1.0".to_string(),
                registry: Some("https://packages.example.com/v1?token=\"dev\"".to_string()),
            }
        );
        assert_eq!(parsed.dependencies[1].alias, "local_utils");
        assert_eq!(
            parsed.dependencies[1].source,
            DependencySource::Path {
                path: "../utils".to_string(),
            }
        );
    }

    #[test]
    fn parses_ffi_link_metadata_and_rebases_library_paths() {
        let root = Path::new("/tmp/nomo-ffi-demo");
        let manifest = r#"[package]
namespace = "local"
name = "ffi-demo"
version = "0.1.0"
edition = "2026"

[ffi]
libraries = ["z"]
library_paths = ["native/lib"]
frameworks = ["Security"]
link_args = ["-Wl,-rpath,@loader_path"]
"#;
        let parsed = parse_manifest_text(manifest, root).unwrap();

        assert_eq!(parsed.ffi.libraries, vec!["z"]);
        assert_eq!(parsed.ffi.library_paths, vec![root.join("native/lib")]);
        assert_eq!(parsed.ffi.frameworks, vec!["Security"]);
        assert_eq!(parsed.ffi.link_args, vec!["-Wl,-rpath,@loader_path"]);
    }

    #[test]
    fn parses_lockfile_as_standard_toml() {
        let packages = parse_lockfile_text(
            r#"# This file is generated by `nomo deps resolve`.

[[package]]
id = "fynn/utils"
alias = "local_utils"
source = "path+../utils"
dependencies = [
  "cli -> nomo-lang/cli",
]

[[package]]
id = "nomo-lang/cli"
alias = "cli"
version = "0.2.1"
source = "registry+nomo-lang/cli"
"#,
        )
        .unwrap();

        assert_eq!(packages.len(), 2);
        assert_eq!(packages[0].package, "fynn/utils");
        assert_eq!(packages[0].dependencies[0].alias, "cli");
        assert_eq!(packages[0].dependencies[0].package, "nomo-lang/cli");
        assert_eq!(
            packages[1].source,
            DependencySource::Registry {
                version: "0.2.1".to_string(),
                registry: None,
            }
        );
    }

    #[test]
    fn rejects_unknown_lockfile_fields() {
        let err = parse_lockfile_text(
            "[[package]]\nid = \"nomo-lang/json\"\nalias = \"json\"\nversion = \"0.1.0\"\nsource = \"registry+nomo-lang/json\"\nextra = \"nope\"\n",
        )
        .unwrap_err();

        assert!(err.contains("unknown field"), "{err}");
    }

    #[test]
    fn rejects_dependency_alias_that_overrides_std() {
        let manifest = "[package]\nname = \"demo\"\n\n[dependencies]\nstd = { package = \"other/std\", version = \"0.1.0\" }\n";
        let err = parse_manifest_text(manifest, Path::new("demo")).unwrap_err();

        assert!(err.contains("alias `std` is reserved"), "{err}");
    }

    #[test]
    fn parses_workspace_member_package_and_dependency_inheritance() {
        let root = temp_test_root("workspace-inheritance");
        if root.exists() {
            fs::remove_dir_all(&root).unwrap();
        }
        let app = root.join("apps/cli");
        let core = root.join("packages/core");
        fs::create_dir_all(app.join("src")).unwrap();
        fs::create_dir_all(core.join("src")).unwrap();
        fs::write(
            root.join("nomo.toml"),
            "[workspace]\nmembers = [\"apps/*\", \"packages/*\"]\n\n[workspace.package]\nnamespace = \"fynn\"\nedition = \"2026\"\n\n[workspace.dependencies]\njson = { package = \"nomo-lang/json\", version = \"0.1.0\" }\ncore = { package = \"fynn/core\", path = \"packages/core\" }\n",
        )
        .unwrap();
        fs::write(
            app.join("nomo.toml"),
            "[package]\nname = \"cli\"\nversion = \"0.1.0\"\nnamespace.workspace = true\nedition.workspace = true\n\n[dependencies]\njson.workspace = true\ncore.workspace = true\n",
        )
        .unwrap();
        fs::write(
            core.join("nomo.toml"),
            "[package]\nname = \"core\"\nversion = \"0.1.0\"\nnamespace.workspace = true\nedition.workspace = true\n",
        )
        .unwrap();

        let manifest = parse_manifest_at_root(&app).unwrap();

        assert_eq!(manifest.package.namespace, "fynn");
        assert_eq!(manifest.package.name, "cli");
        assert_eq!(manifest.package.edition, "2026");
        assert_eq!(manifest.dependencies.len(), 2);
        assert_eq!(manifest.dependencies[0].alias, "core");
        assert_eq!(manifest.dependencies[0].package, "fynn/core");
        assert_eq!(
            manifest.dependencies[0].source,
            DependencySource::Path {
                path: "../../packages/core".to_string(),
            }
        );
        assert_eq!(manifest.dependencies[1].alias, "json");
        assert_eq!(manifest.dependencies[1].package, "nomo-lang/json");
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn rejects_workspace_root_without_package_as_project() {
        let root = temp_test_root("workspace-root-without-package");
        if root.exists() {
            fs::remove_dir_all(&root).unwrap();
        }
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join("nomo.toml"),
            "[workspace]\nmembers = [\"apps/*\"]\n",
        )
        .unwrap();

        let err = discover_project(&root).unwrap_err();

        assert!(err.contains("workspace manifest"), "{err}");
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn discovers_workspace_members_defaults_and_excludes() {
        let root = temp_test_root("workspace-discovery");
        if root.exists() {
            fs::remove_dir_all(&root).unwrap();
        }
        let app = root.join("apps/cli");
        let core = root.join("packages/core");
        let skipped = root.join("target/generated");
        fs::create_dir_all(app.join("src")).unwrap();
        fs::create_dir_all(core.join("src")).unwrap();
        fs::create_dir_all(skipped.join("src")).unwrap();
        fs::write(
            root.join("nomo.toml"),
            "[workspace]\nmembers = [\"apps/*\", \"packages/*\", \"target/*\"]\ndefault-members = [\"apps/cli\"]\nexclude = [\"target\"]\n\n[workspace.package]\nnamespace = \"fynn\"\nedition = \"2026\"\n",
        )
        .unwrap();
        for (dir, name) in [(&app, "cli"), (&core, "core"), (&skipped, "generated")] {
            fs::write(
                dir.join("nomo.toml"),
                format!(
                    "[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nnamespace.workspace = true\nedition.workspace = true\n"
                ),
            )
            .unwrap();
            fs::write(dir.join("src/main.nomo"), "package app.main\n").unwrap();
        }

        let workspace = discover_workspace(&app.join("src/main.nomo")).unwrap();

        assert_eq!(workspace.root, root);
        assert_eq!(
            workspace
                .members
                .iter()
                .map(|project| project.name.as_str())
                .collect::<Vec<_>>(),
            vec!["cli", "core"]
        );
        assert_eq!(
            workspace
                .default_members
                .iter()
                .map(|project| project.name.as_str())
                .collect::<Vec<_>>(),
            vec!["cli"]
        );
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn falls_back_to_directory_name_when_package_name_is_missing() {
        let root = temp_test_root("manifest-fallback");
        if root.exists() {
            fs::remove_dir_all(&root).unwrap();
        }
        let project_root = root.join("fallback-demo");
        fs::create_dir_all(project_root.join("src")).unwrap();
        fs::write(project_root.join("nomo.toml"), "[dependencies]\n").unwrap();
        fs::write(project_root.join("src/main.nomo"), "package app.main\n").unwrap();

        let project = discover_project(&project_root).unwrap();

        assert_eq!(project.name, "fallback-demo");
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn discovers_project_root_from_source_file_path() {
        let root = temp_test_root("discover-source-file");
        if root.exists() {
            fs::remove_dir_all(&root).unwrap();
        }
        fs::create_dir_all(&root).unwrap();
        let project = create_project(&root, "source-demo").unwrap();

        let discovered = discover_project(&project.root.join("src/main.nomo")).unwrap();

        assert_eq!(discovered.root, project.root);
        assert_eq!(discovered.name, "source-demo");
        assert_eq!(discovered.main, project.root.join("src/main.nomo"));
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn builds_source_file_path_under_project_root() {
        let root = temp_test_root("source-file-build");
        if root.exists() {
            fs::remove_dir_all(&root).unwrap();
        }
        fs::create_dir_all(&root).unwrap();
        let project = create_project(&root, "source-build-demo").unwrap();
        let discovered = discover_project(&project.root.join("src/main.nomo")).unwrap();

        let artifact = build_project(&discovered, true).unwrap();

        assert_eq!(artifact, project.root.join("build/c/main.c"));
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn cleans_project_build_directory() {
        let root = temp_test_root("clean-project");
        if root.exists() {
            fs::remove_dir_all(&root).unwrap();
        }
        fs::create_dir_all(&root).unwrap();
        let project = create_project(&root, "clean-demo").unwrap();
        let artifact = build_project(&project, true).unwrap();
        assert!(artifact.exists());

        let cleaned = clean_project(&project).unwrap();

        assert_eq!(cleaned, project.root.join("build"));
        assert!(!cleaned.exists());
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn rejects_directory_without_manifest() {
        let root = temp_test_root("missing-manifest-dir");
        if root.exists() {
            fs::remove_dir_all(&root).unwrap();
        }
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("src/main.nomo"), "package app.main\n").unwrap();

        let err = discover_project(&root).unwrap_err();

        assert!(err.contains("nomo.toml"));
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn rejects_standalone_source_file_without_manifest() {
        let root = temp_test_root("standalone-source");
        if root.exists() {
            fs::remove_dir_all(&root).unwrap();
        }
        fs::create_dir_all(&root).unwrap();
        let source = root.join("main.nomo");
        fs::write(&source, "package app.main\n").unwrap();

        let err = discover_project(&source).unwrap_err();

        assert!(err.contains("nomo.toml"));
        assert!(err.contains("nomoc"));
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn runs_project_with_forwarded_args() {
        let root = temp_test_root("forwarded-args");
        if root.exists() {
            fs::remove_dir_all(&root).unwrap();
        }
        fs::create_dir_all(&root).unwrap();
        let project = create_project(&root, "args-demo").unwrap();
        fs::write(
            project.root.join("src/main.nomo"),
            r#"package app.main

import std.env
import std.io
import std.array

fn main() -> void {
    let args: Array<string> = env.args()
    let size: u64 = args.len()
    let status: string = if size == 2 {
        "ok"
    } else {
        panic("expected one forwarded arg")
    }
    io.println(status)
}
"#,
        )
        .unwrap();

        let status = run_project_with_args(&project, &["hello".to_string()]).unwrap();
        assert_eq!(status, 0);
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn resolves_module_source_paths_for_local_and_dependency_imports() {
        let root = temp_test_root("module-source-resolution");
        if root.exists() {
            fs::remove_dir_all(&root).unwrap();
        }
        fs::create_dir_all(&root).unwrap();
        let project = create_project(&root, "module-demo").unwrap();
        fs::write(
            project.root.join("src/math.nomo"),
            "package app.math\n\npub fn add() -> i64 {\n    return 1\n}\n",
        )
        .unwrap();

        let dependency = root.join("local-utils");
        fs::create_dir_all(dependency.join("src/path")).unwrap();
        fs::write(
            dependency.join("nomo.toml"),
            "[package]\nnamespace = \"local\"\nname = \"utils\"\nversion = \"0.1.0\"\nedition = \"2026\"\n",
        )
        .unwrap();
        fs::write(
            dependency.join("src/path/main.nomo"),
            "package local_utils.path\n\npub fn join() -> i64 {\n    return 1\n}\n",
        )
        .unwrap();

        let context = ProjectModuleContext {
            local_source_root: project.root.join("src"),
            external_import_roots: vec!["local_utils".to_string()],
            external_modules: vec![ExternalModule {
                import_root: "local_utils".to_string(),
                source_root: dependency.join("src"),
            }],
        };

        assert_eq!(
            resolve_module_source_path(&context, "app", &["app".to_string(), "math".to_string()]),
            Some(project.root.join("src/math.nomo"))
        );
        assert_eq!(
            resolve_module_source_path(
                &context,
                "app",
                &["local_utils".to_string(), "path".to_string()]
            ),
            Some(dependency.join("src/path/main.nomo"))
        );
        assert_eq!(
            resolve_module_source_path(&context, "app", &["std".to_string(), "io".to_string()]),
            None
        );
        fs::remove_dir_all(&root).unwrap();
    }

    fn temp_test_root(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("nomo-project-test-{name}-{}", std::process::id()))
    }
}
