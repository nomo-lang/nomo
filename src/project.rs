use crate::compiler::{
    ExternalModule, check_source_text_with_project_modules, compile_script_source_to_c,
    compile_source_text_to_c_with_project_modules, compile_source_to_c_with_project_modules,
};
use crate::diagnostic::Diagnostic;
use crate::{lexer, parser};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::time::Instant;

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
pub struct DependencyAddSpec {
    pub alias: String,
    pub package: String,
    pub version: String,
    pub registry: Option<String>,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    pub package: PackageMetadata,
    pub dependencies: Vec<Dependency>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageMetadata {
    pub namespace: String,
    pub name: String,
    pub version: String,
    pub edition: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WorkspacePackageDefaults {
    pub namespace: Option<String>,
    pub name: Option<String>,
    pub version: Option<String>,
    pub edition: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceContext {
    pub root: PathBuf,
    pub members: Vec<String>,
    pub default_members: Vec<String>,
    pub exclude: Vec<String>,
    pub resolver: Option<String>,
    pub package: WorkspacePackageDefaults,
    pub dependencies: BTreeMap<String, Dependency>,
}

#[derive(Debug, Clone)]
pub struct WorkspaceGraph {
    pub root: PathBuf,
    pub members: Vec<Project>,
    pub default_members: Vec<Project>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dependency {
    pub alias: String,
    pub package: String,
    pub source: DependencySource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DependencySource {
    Registry {
        version: String,
        registry: Option<String>,
    },
    Path {
        path: String,
    },
    Git {
        git: String,
        branch: Option<String>,
        tag: Option<String>,
        rev: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DependencyGraph {
    pub root: PackageMetadata,
    pub dependencies: Vec<ResolvedDependency>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedDependency {
    pub alias: String,
    pub package: String,
    pub source: DependencySource,
    pub checksum: Option<String>,
    pub dependencies: Vec<ResolvedDependency>,
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

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProjectTestOptions {
    pub filter: Option<String>,
    pub resolution: DependencyResolutionOptions,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectTestReport {
    pub project: String,
    pub tests: Vec<ProjectTestCaseResult>,
}

impl ProjectTestReport {
    pub fn has_failures(&self) -> bool {
        self.tests
            .iter()
            .any(|test| test.status == ProjectTestStatus::Failed)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectTestCaseResult {
    pub name: String,
    pub status: ProjectTestStatus,
    pub duration_ms: u128,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectTestStatus {
    Ok,
    Failed,
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
        if optional_table(&document, "workspace")
            .ok()
            .flatten()
            .is_some()
        {
            return Some(candidate.to_path_buf());
        }
    }
    None
}

pub fn parse_manifest_at_root(root: &Path) -> Result<Manifest, String> {
    let manifest_path = root.join("nomo.toml");
    let text = fs::read_to_string(&manifest_path).map_err(|err| err.to_string())?;
    let document = parse_manifest_document(&text)?;
    let workspace = workspace_context_for_manifest(root, &document)?;
    parse_manifest_document_at_root(&document, root, workspace.as_ref())
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
    let archive = build_package_archive(&project.root, &manifest).map_err(BuildError::Message)?;
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

pub fn publish_package_archive(
    project: &Project,
    registry: &str,
    output_dir: Option<&Path>,
) -> Result<PublishPackage, BuildError> {
    let package = prepare_publish_package(project, output_dir)?;
    let archive = fs::read(&package.archive_path).map_err(|err| {
        BuildError::Message(format!(
            "failed to read {} for registry upload: {err}",
            package.archive_path.display()
        ))
    })?;
    upload_http_registry_archive(registry, &package.package, &package.version, &archive)
        .map_err(BuildError::Message)?;
    Ok(package)
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
            let Some(dep_root) = resolve_registry_source(
                base_root,
                &dependency.alias,
                &dependency.package,
                version,
                registry.as_deref(),
                offline,
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

pub fn build_project(project: &Project, emit_c_only: bool) -> Result<PathBuf, String> {
    build_project_with_diagnostics(project, emit_c_only).map_err(|err| err.human())
}

pub fn build_project_with_diagnostics(
    project: &Project,
    emit_c_only: bool,
) -> Result<PathBuf, BuildError> {
    build_project_with_options(project, emit_c_only, DependencyResolutionOptions::default())
}

pub fn build_project_with_options(
    project: &Project,
    emit_c_only: bool,
    options: DependencyResolutionOptions,
) -> Result<PathBuf, BuildError> {
    let context =
        project_module_context_with_options(project, options).map_err(BuildError::Message)?;
    let c = compile_source_to_c_with_project_modules(
        &project.main,
        Some(&context.local_source_root),
        &context.external_import_roots,
        &context.external_modules,
    )
    .map_err(BuildError::Diagnostic)?;
    let c_dir = project.root.join("build/c");
    let bin_dir = project.root.join("build/bin");
    fs::create_dir_all(&c_dir).map_err(|err| BuildError::Message(err.to_string()))?;
    fs::create_dir_all(&bin_dir).map_err(|err| BuildError::Message(err.to_string()))?;

    let c_path = c_dir.join("main.c");
    fs::write(&c_path, c).map_err(|err| BuildError::Message(err.to_string()))?;
    if emit_c_only {
        return Ok(c_path);
    }

    let bin_path = bin_dir.join(&project.name);
    let output = Command::new("cc")
        .arg("-std=c99")
        .arg(&c_path)
        .arg("-lm")
        .arg("-o")
        .arg(&bin_path)
        .output()
        .map_err(|err| BuildError::Message(format!("failed to run cc: {err}")))?;
    if !output.status.success() {
        return Err(BuildError::Message(format!(
            "cc failed:\n{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    Ok(bin_path)
}

pub fn run_project_tests_with_options(
    project: &Project,
    options: ProjectTestOptions,
) -> Result<ProjectTestReport, BuildError> {
    let manifest = parse_manifest_at_root(&project.root).map_err(BuildError::Message)?;
    let project_id = package_id(&manifest.package);
    let context = project_module_context_with_options(project, options.resolution)
        .map_err(BuildError::Message)?;
    let mut test_sources = discover_project_tests(project)?;
    test_sources.sort_by(|left, right| left.name.cmp(&right.name));
    if let Some(filter) = options.filter.as_deref() {
        test_sources.retain(|test| test.name.contains(filter));
    }

    let c_dir = project.root.join("build/test/c");
    let bin_dir = project.root.join("build/test/bin");
    fs::create_dir_all(&c_dir).map_err(|err| BuildError::Message(err.to_string()))?;
    fs::create_dir_all(&bin_dir).map_err(|err| BuildError::Message(err.to_string()))?;

    let mut results = Vec::new();
    for test in test_sources {
        let started = Instant::now();
        let result = run_single_project_test(project, &context, &test, &c_dir, &bin_dir);
        let duration_ms = started.elapsed().as_millis();
        match result {
            Ok(()) => results.push(ProjectTestCaseResult {
                name: test.name,
                status: ProjectTestStatus::Ok,
                duration_ms,
                message: None,
            }),
            Err(message) => results.push(ProjectTestCaseResult {
                name: test.name,
                status: ProjectTestStatus::Failed,
                duration_ms,
                message: Some(message),
            }),
        }
    }

    Ok(ProjectTestReport {
        project: project_id,
        tests: results,
    })
}

pub fn clean_project(project: &Project) -> Result<PathBuf, String> {
    let build_dir = project.root.join("build");
    if build_dir.exists() {
        fs::remove_dir_all(&build_dir).map_err(|err| err.to_string())?;
    }
    Ok(build_dir)
}

pub fn project_package_id(project: &Project) -> Result<String, String> {
    let manifest = parse_manifest_at_root(&project.root)?;
    Ok(package_id(&manifest.package))
}

#[derive(Debug, Clone)]
struct DiscoveredTest {
    name: String,
    function_name: String,
    source_path: PathBuf,
    source: String,
}

fn discover_project_tests(project: &Project) -> Result<Vec<DiscoveredTest>, BuildError> {
    let src = project.root.join("src");
    let mut files = Vec::new();
    collect_nomo_source_files(&src, &mut files).map_err(BuildError::Message)?;
    let mut tests = Vec::new();
    for source_path in files {
        let source = fs::read_to_string(&source_path).map_err(|err| {
            BuildError::Message(format!("failed to read {}: {err}", source_path.display()))
        })?;
        let tokens = lexer::lex(&source_path, &source).map_err(BuildError::Diagnostic)?;
        let ast = parser::parse(&source_path, &tokens).map_err(BuildError::Diagnostic)?;
        for function in ast.functions.iter().filter(|function| function.is_test) {
            validate_test_function(&source_path, function)?;
            let mut name = function.package.join(".");
            name.push('.');
            name.push_str(&function.name);
            tests.push(DiscoveredTest {
                name,
                function_name: function.name.clone(),
                source_path: source_path.clone(),
                source: source.clone(),
            });
        }
    }
    Ok(tests)
}

fn collect_nomo_source_files(dir: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    for entry in fs::read_dir(dir)
        .map_err(|err| format!("failed to read source directory {}: {err}", dir.display()))?
    {
        let entry = entry.map_err(|err| err.to_string())?;
        let path = entry.path();
        if path.is_dir() {
            collect_nomo_source_files(&path, files)?;
        } else if path.is_file() && path.extension().and_then(|ext| ext.to_str()) == Some("nomo") {
            files.push(path);
        }
    }
    Ok(())
}

fn validate_test_function(
    source_path: &Path,
    function: &crate::ast::Function,
) -> Result<(), BuildError> {
    let error = |message: &str| {
        BuildError::Diagnostic(Diagnostic::new(
            "E1101",
            message,
            source_path,
            function.span.line,
            function.span.column,
            function.span.length,
            &function.span.text,
        ))
    };
    if function.name == "main" {
        return Err(error("`#[test]` function cannot be named `main`"));
    }
    if !function.type_params.is_empty() {
        return Err(error(
            "`#[test]` functions must not declare type parameters",
        ));
    }
    if !function.params.is_empty() {
        return Err(error("`#[test]` functions must not take parameters"));
    }
    if !is_void_type(&function.return_type) {
        return Err(error("`#[test]` functions must return `void`"));
    }
    Ok(())
}

fn is_void_type(type_ref: &crate::ast::TypeRef) -> bool {
    type_ref.path == ["void"] && type_ref.args.is_empty()
}

fn run_single_project_test(
    project: &Project,
    context: &ProjectModuleContext,
    test: &DiscoveredTest,
    c_dir: &Path,
    bin_dir: &Path,
) -> Result<(), String> {
    let runner_source = test_runner_source(&test.source, &test.function_name);
    let c = compile_source_text_to_c_with_project_modules(
        &test.source_path,
        &runner_source,
        Some(&context.local_source_root),
        &context.external_import_roots,
        &context.external_modules,
    )
    .map_err(|diag| diag.human())?;
    let file_stem = safe_test_artifact_name(&test.name);
    let c_path = c_dir.join(format!("{file_stem}.c"));
    let bin_path = bin_dir.join(file_stem);
    fs::write(&c_path, c).map_err(|err| format!("failed to write {}: {err}", c_path.display()))?;
    let output = Command::new("cc")
        .arg("-std=c99")
        .arg(&c_path)
        .arg("-lm")
        .arg("-o")
        .arg(&bin_path)
        .output()
        .map_err(|err| format!("failed to run cc: {err}"))?;
    if !output.status.success() {
        return Err(format!(
            "cc failed:\n{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let run_path = fs::canonicalize(&bin_path)
        .map_err(|err| format!("failed to resolve {}: {err}", bin_path.display()))?;
    let output = Command::new(&run_path)
        .current_dir(&project.root)
        .output()
        .map_err(|err| format!("failed to run {}: {err}", run_path.display()))?;
    if output.status.success() {
        return Ok(());
    }
    let status = output.status.code().unwrap_or(1);
    let message = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    if message.trim().is_empty() {
        Err(format!("test exited with status {status}"))
    } else {
        Err(message.trim().to_string())
    }
}

fn test_runner_source(source: &str, function_name: &str) -> String {
    let mut runner = rename_existing_main(source);
    if !runner.ends_with('\n') {
        runner.push('\n');
    }
    runner.push_str("\nfn main() -> void {\n    ");
    runner.push_str(function_name);
    runner.push_str("()\n}\n");
    runner
}

fn rename_existing_main(source: &str) -> String {
    let mut output = String::new();
    for line in source.lines() {
        let trimmed = line.trim_start();
        if is_main_declaration_start(trimmed) {
            output.push_str(&line.replacen("fn main", "fn __nomo_original_main", 1));
        } else {
            output.push_str(line);
        }
        output.push('\n');
    }
    output
}

fn is_main_declaration_start(trimmed: &str) -> bool {
    let rest = trimmed
        .strip_prefix("fn main")
        .or_else(|| trimmed.strip_prefix("pub fn main"));
    rest.is_some_and(|rest| {
        rest.starts_with('(')
            || rest
                .chars()
                .next()
                .is_some_and(|ch| ch.is_ascii_whitespace())
    })
}

fn safe_test_artifact_name(name: &str) -> String {
    name.chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect()
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

pub fn run_project(project: &Project) -> Result<i32, String> {
    run_project_with_args(project, &[])
}

pub fn run_project_with_args(project: &Project, args: &[String]) -> Result<i32, String> {
    run_project_with_args_and_diagnostics(project, args).map_err(|err| err.human())
}

pub fn run_project_with_args_and_diagnostics(
    project: &Project,
    args: &[String],
) -> Result<i32, BuildError> {
    let bin = build_project_with_diagnostics(project, false)?;
    let bin = if bin.is_absolute() {
        bin
    } else {
        std::env::current_dir()
            .map_err(|err| BuildError::Message(err.to_string()))?
            .join(bin)
    };
    let status = Command::new(&bin)
        .current_dir(&project.root)
        .args(args)
        .status()
        .map_err(|err| BuildError::Message(format!("failed to run {}: {err}", bin.display())))?;
    Ok(status.code().unwrap_or(1))
}

pub fn run_standalone_script_with_args_and_diagnostics(
    source: &Path,
    args: &[String],
) -> Result<i32, BuildError> {
    let c = compile_script_source_to_c(source).map_err(BuildError::Diagnostic)?;
    let stem = source
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("script");
    let mut hasher = DefaultHasher::new();
    source.hash(&mut hasher);
    c.hash(&mut hasher);
    let build_dir = std::env::temp_dir().join(format!("nomo-script-{:016x}", hasher.finish()));
    let c_dir = build_dir.join("c");
    let bin_dir = build_dir.join("bin");
    fs::create_dir_all(&c_dir).map_err(|err| BuildError::Message(err.to_string()))?;
    fs::create_dir_all(&bin_dir).map_err(|err| BuildError::Message(err.to_string()))?;

    let c_path = c_dir.join("main.c");
    fs::write(&c_path, c).map_err(|err| BuildError::Message(err.to_string()))?;
    let bin_path = bin_dir.join(stem);
    let output = Command::new("cc")
        .arg("-std=c99")
        .arg(&c_path)
        .arg("-lm")
        .arg("-o")
        .arg(&bin_path)
        .output()
        .map_err(|err| BuildError::Message(format!("failed to run cc: {err}")))?;
    if !output.status.success() {
        return Err(BuildError::Message(format!(
            "cc failed:\n{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )));
    }

    let current_dir = source.parent().unwrap_or_else(|| Path::new("."));
    let status = Command::new(&bin_path)
        .current_dir(current_dir)
        .args(args)
        .status()
        .map_err(|err| {
            BuildError::Message(format!("failed to run {}: {err}", bin_path.display()))
        })?;
    Ok(status.code().unwrap_or(1))
}

#[cfg(test)]
fn parse_manifest_text(manifest: &str, root: &Path) -> Result<Manifest, String> {
    let document = parse_manifest_document(manifest)?;
    parse_manifest_document_at_root(&document, root, None)
}

fn parse_manifest_document(manifest: &str) -> Result<toml::Value, String> {
    manifest
        .parse::<toml::Value>()
        .map_err(|err| format!("failed to parse nomo.toml as TOML: {err}"))
}

fn upsert_registry_dependency(
    document: &mut toml::Value,
    spec: &DependencyAddSpec,
) -> Result<(), String> {
    let root = document
        .as_table_mut()
        .ok_or_else(|| "manifest root must be a TOML table".to_string())?;
    let dependencies = root
        .entry("dependencies".to_string())
        .or_insert_with(|| toml::Value::Table(toml::map::Map::new()));
    let dependencies = dependencies
        .as_table_mut()
        .ok_or_else(|| "manifest `dependencies` must be a TOML table".to_string())?;
    if dependencies.contains_key(&spec.alias) {
        return Err(format!("dependency `{}` already exists", spec.alias));
    }

    let mut fields = toml::map::Map::new();
    fields.insert(
        "package".to_string(),
        toml::Value::String(spec.package.clone()),
    );
    fields.insert(
        "version".to_string(),
        toml::Value::String(spec.version.clone()),
    );
    if let Some(registry) = &spec.registry {
        fields.insert(
            "registry".to_string(),
            toml::Value::String(registry.clone()),
        );
    }
    let value = toml::Value::Table(fields);
    parse_dependency_value(&spec.alias, &value, None)?;
    dependencies.insert(spec.alias.clone(), value);
    Ok(())
}

fn remove_dependency_from_manifest(document: &mut toml::Value, alias: &str) -> Result<(), String> {
    let root = document
        .as_table_mut()
        .ok_or_else(|| "manifest root must be a TOML table".to_string())?;
    let Some(dependencies) = root.get_mut("dependencies") else {
        return Err(format!("dependency `{alias}` was not found"));
    };
    let dependencies = dependencies
        .as_table_mut()
        .ok_or_else(|| "manifest `dependencies` must be a TOML table".to_string())?;
    if dependencies.remove(alias).is_none() {
        return Err(format!("dependency `{alias}` was not found"));
    }
    if dependencies.is_empty() {
        root.remove("dependencies");
    }
    Ok(())
}

fn render_manifest_document(document: &toml::Value) -> Result<String, String> {
    let mut rendered = toml::to_string_pretty(document)
        .map_err(|err| format!("failed to render nomo.toml as TOML: {err}"))?;
    if !rendered.ends_with('\n') {
        rendered.push('\n');
    }
    Ok(rendered)
}

fn parse_manifest_document_at_root(
    document: &toml::Value,
    root: &Path,
    workspace: Option<&WorkspaceContext>,
) -> Result<Manifest, String> {
    let root_name = root
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let mut package = PackageMetadata {
        namespace: "local".to_string(),
        name: root_name,
        version: "0.1.0".to_string(),
        edition: "2026".to_string(),
    };
    let mut dependencies = Vec::new();

    let package_table = optional_table(&document, "package")?;
    if package_table.is_none() && optional_table(document, "workspace")?.is_some() {
        return Err(format!(
            "{} is a workspace manifest and does not define a package",
            root.join("nomo.toml").display()
        ));
    }
    let namespace_explicit = package_table.is_some_and(|table| table.contains_key("namespace"));
    if let Some(table) = package_table {
        if let Some(value) = optional_package_string_field(table, "namespace", workspace)? {
            package.namespace = value;
        }
        if let Some(value) = optional_package_string_field(table, "name", workspace)? {
            package.name = value;
        }
        if let Some(value) = optional_package_string_field(table, "version", workspace)? {
            package.version = value;
        }
        if let Some(value) = optional_package_string_field(table, "edition", workspace)? {
            package.edition = value;
        }
    }

    if let Some(table) = optional_table(&document, "dependencies")? {
        let inheritance = workspace.map(|workspace| WorkspaceDependencyInheritance {
            workspace,
            package_root: root,
        });
        for (alias, value) in table {
            if let Some(dependency) = parse_dependency_value(alias, value, inheritance.as_ref())? {
                dependencies.push(dependency);
            }
        }
    }

    validate_package_namespace("package namespace", &package.namespace)?;
    if namespace_explicit {
        validate_package_segment("package name", &package.name)?;
    } else if !is_legacy_package_name(&package.name) {
        return Err(format!("invalid legacy package name `{}`", package.name));
    }
    validate_version_like("package version", &package.version)?;
    if package.edition.is_empty() {
        return Err("package edition must not be empty".to_string());
    }

    Ok(Manifest {
        package,
        dependencies,
    })
}

fn workspace_context_for_manifest(
    root: &Path,
    document: &toml::Value,
) -> Result<Option<WorkspaceContext>, String> {
    if optional_table(document, "workspace")?.is_some() {
        return parse_workspace_context(root, document).map(Some);
    }

    let Some(workspace_root) = workspace_root_for_package(root)? else {
        return Ok(None);
    };
    let text =
        fs::read_to_string(workspace_root.join("nomo.toml")).map_err(|err| err.to_string())?;
    let document = parse_manifest_document(&text)?;
    parse_workspace_context(&workspace_root, &document).map(Some)
}

fn workspace_root_for_package(root: &Path) -> Result<Option<PathBuf>, String> {
    for candidate in root.ancestors().skip(1) {
        let manifest = candidate.join("nomo.toml");
        if !manifest.is_file() {
            continue;
        }
        let text = fs::read_to_string(&manifest).map_err(|err| err.to_string())?;
        let document = parse_manifest_document(&text)?;
        if optional_table(&document, "workspace")?.is_some() {
            return Ok(Some(candidate.to_path_buf()));
        }
    }
    Ok(None)
}

fn parse_workspace_context(
    root: &Path,
    document: &toml::Value,
) -> Result<WorkspaceContext, String> {
    let workspace_table = optional_table(document, "workspace")?
        .ok_or_else(|| "manifest does not define a [workspace] table".to_string())?;
    let members = optional_string_array_field(workspace_table, "workspace", "members")?;
    let default_members =
        optional_string_array_field(workspace_table, "workspace", "default-members")?;
    let exclude = optional_string_array_field(workspace_table, "workspace", "exclude")?;
    let resolver = optional_string_field(workspace_table, "workspace", "resolver")?;
    let package = match workspace_table.get("package") {
        Some(value) => parse_workspace_package_defaults(value)?,
        None => WorkspacePackageDefaults::default(),
    };
    let dependencies = match workspace_table.get("dependencies") {
        Some(value) => parse_workspace_dependencies(value)?,
        None => BTreeMap::new(),
    };
    Ok(WorkspaceContext {
        root: root.to_path_buf(),
        members,
        default_members,
        exclude,
        resolver,
        package,
        dependencies,
    })
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

fn parse_workspace_package_defaults(
    value: &toml::Value,
) -> Result<WorkspacePackageDefaults, String> {
    let table = value
        .as_table()
        .ok_or_else(|| "manifest `workspace.package` must be a TOML table".to_string())?;
    Ok(WorkspacePackageDefaults {
        namespace: optional_string_field(table, "workspace.package", "namespace")?,
        name: optional_string_field(table, "workspace.package", "name")?,
        version: optional_string_field(table, "workspace.package", "version")?,
        edition: optional_string_field(table, "workspace.package", "edition")?,
    })
}

fn parse_workspace_dependencies(
    value: &toml::Value,
) -> Result<BTreeMap<String, Dependency>, String> {
    let table = value
        .as_table()
        .ok_or_else(|| "manifest `workspace.dependencies` must be a TOML table".to_string())?;
    let mut dependencies = BTreeMap::new();
    for (alias, value) in table {
        if let Some(dependency) = parse_dependency_value(alias, value, None)? {
            dependencies.insert(alias.clone(), dependency);
        }
    }
    Ok(dependencies)
}

fn optional_table<'a>(
    document: &'a toml::Value,
    key: &str,
) -> Result<Option<&'a toml::map::Map<String, toml::Value>>, String> {
    match document.get(key) {
        Some(value) => value
            .as_table()
            .map(Some)
            .ok_or_else(|| format!("manifest `{key}` must be a TOML table")),
        None => Ok(None),
    }
}

fn optional_string_field(
    table: &toml::map::Map<String, toml::Value>,
    section: &str,
    key: &str,
) -> Result<Option<String>, String> {
    match table.get(key) {
        Some(value) => value
            .as_str()
            .map(|value| Some(value.to_string()))
            .ok_or_else(|| format!("manifest `{section}.{key}` must be a string")),
        None => Ok(None),
    }
}

fn optional_string_array_field(
    table: &toml::map::Map<String, toml::Value>,
    section: &str,
    key: &str,
) -> Result<Vec<String>, String> {
    let Some(value) = table.get(key) else {
        return Ok(Vec::new());
    };
    let Some(values) = value.as_array() else {
        return Err(format!("manifest `{section}.{key}` must be an array"));
    };
    values
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(|value| value.to_string())
                .ok_or_else(|| format!("manifest `{section}.{key}` entries must be strings"))
        })
        .collect()
}

fn optional_package_string_field(
    table: &toml::map::Map<String, toml::Value>,
    key: &str,
    workspace: Option<&WorkspaceContext>,
) -> Result<Option<String>, String> {
    match table.get(key) {
        Some(value) => {
            if let Some(value) = value.as_str() {
                return Ok(Some(value.to_string()));
            }
            if is_workspace_inheritance(value) {
                return workspace_package_default(workspace, key).map(Some);
            }
            Err(format!(
                "manifest `package.{key}` must be a string or `{{ workspace = true }}`"
            ))
        }
        None => Ok(None),
    }
}

fn workspace_package_default(
    workspace: Option<&WorkspaceContext>,
    key: &str,
) -> Result<String, String> {
    let workspace = workspace.ok_or_else(|| {
        format!("manifest `package.{key}` uses workspace inheritance outside a workspace")
    })?;
    let value = match key {
        "namespace" => &workspace.package.namespace,
        "name" => &workspace.package.name,
        "version" => &workspace.package.version,
        "edition" => &workspace.package.edition,
        _ => unreachable!("package inheritance key is validated by caller"),
    };
    value.clone().ok_or_else(|| {
        format!(
            "manifest `package.{key}` inherits from workspace.package.{key}, but it is not defined"
        )
    })
}

#[derive(Clone, Copy)]
struct WorkspaceDependencyInheritance<'a> {
    workspace: &'a WorkspaceContext,
    package_root: &'a Path,
}

fn parse_dependency_value(
    alias: &str,
    value: &toml::Value,
    inheritance: Option<&WorkspaceDependencyInheritance<'_>>,
) -> Result<Option<Dependency>, String> {
    validate_dependency_alias(alias)?;

    if let Some(version) = value.as_str() {
        if alias == "std" {
            validate_version_like("dependency `std` version", version)?;
            return Ok(None);
        }
        return Err(format!(
            "dependency `{alias}` must use an inline table with `package = \"owner/name\"`"
        ));
    }

    let Some(fields) = value.as_table() else {
        return Err(format!(
            "dependency `{alias}` must be a TOML string or table"
        ));
    };

    if is_workspace_inheritance(value) {
        let inheritance = inheritance.ok_or_else(|| {
            format!("dependency `{alias}` uses workspace inheritance outside a workspace")
        })?;
        let dependency = inheritance
            .workspace
            .dependencies
            .get(alias)
            .ok_or_else(|| {
                format!("dependency `{alias}` inherits from workspace.dependencies.{alias}, but it is not defined")
            })?;
        return Ok(Some(rebase_workspace_dependency(
            dependency,
            inheritance.workspace,
            inheritance.package_root,
        )));
    }
    if fields.contains_key("workspace") {
        return Err(format!(
            "dependency `{alias}` field `workspace` must be `true` and cannot be combined with source fields"
        ));
    }

    let package = required_dependency_string(alias, fields, "package")?;
    validate_package_id(&package)?;
    if alias == "std" {
        if package == "nomo-lang/std" {
            return Ok(None);
        }
        return Err(
            "dependency alias `std` is reserved for the built-in standard library".to_string(),
        );
    }

    let source_keys = ["path", "git", "version"]
        .into_iter()
        .filter(|key| fields.contains_key(*key))
        .collect::<Vec<_>>();
    if source_keys.len() != 1 {
        return Err(format!(
            "dependency `{alias}` must specify exactly one source: `path`, `git`, or `version`"
        ));
    }
    if fields.contains_key("registry") && !fields.contains_key("version") {
        return Err(format!(
            "dependency `{alias}` can only specify `registry` together with `version`"
        ));
    }
    if fields.contains_key("rev") && !fields.contains_key("git") {
        return Err(format!(
            "dependency `{alias}` can only specify `rev` together with `git`"
        ));
    }
    if fields.contains_key("branch") && !fields.contains_key("git") {
        return Err(format!(
            "dependency `{alias}` can only specify `branch` together with `git`"
        ));
    }
    if fields.contains_key("tag") && !fields.contains_key("git") {
        return Err(format!(
            "dependency `{alias}` can only specify `tag` together with `git`"
        ));
    }
    let git_selectors = ["branch", "tag", "rev"]
        .into_iter()
        .filter(|key| fields.contains_key(*key))
        .collect::<Vec<_>>();
    if git_selectors.len() > 1 {
        return Err(format!(
            "dependency `{alias}` must specify only one git checkout selector: `branch`, `tag`, or `rev`"
        ));
    }

    let source = if fields.contains_key("path") {
        DependencySource::Path {
            path: required_dependency_string(alias, fields, "path")?,
        }
    } else if fields.contains_key("git") {
        DependencySource::Git {
            git: required_dependency_string(alias, fields, "git")?,
            branch: optional_dependency_string(alias, fields, "branch")?,
            tag: optional_dependency_string(alias, fields, "tag")?,
            rev: optional_dependency_string(alias, fields, "rev")?,
        }
    } else if fields.contains_key("version") {
        let version = required_dependency_string(alias, fields, "version")?;
        validate_version_like(&format!("dependency `{alias}` version"), &version)?;
        DependencySource::Registry {
            version,
            registry: optional_dependency_string(alias, fields, "registry")?,
        }
    } else {
        unreachable!("source key count already validated")
    };

    Ok(Some(Dependency {
        alias: alias.to_string(),
        package,
        source,
    }))
}

fn is_workspace_inheritance(value: &toml::Value) -> bool {
    let Some(table) = value.as_table() else {
        return false;
    };
    table.len() == 1 && table.get("workspace").and_then(|value| value.as_bool()) == Some(true)
}

fn rebase_workspace_dependency(
    dependency: &Dependency,
    workspace: &WorkspaceContext,
    package_root: &Path,
) -> Dependency {
    let mut dependency = dependency.clone();
    if let DependencySource::Path { path } = &dependency.source {
        dependency.source = DependencySource::Path {
            path: rebase_workspace_path(&workspace.root, package_root, path),
        };
    }
    dependency
}

fn rebase_workspace_path(workspace_root: &Path, package_root: &Path, path: &str) -> String {
    let path = Path::new(path);
    if path.is_absolute() {
        return path.to_string_lossy().replace('\\', "/");
    }
    let target = normalize_logical_path(&workspace_root.join(path));
    let package_root = normalize_logical_path(package_root);
    relative_path(&package_root, &target)
        .unwrap_or(target)
        .to_string_lossy()
        .replace('\\', "/")
}

fn normalize_logical_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                if !normalized.pop() {
                    normalized.push("..");
                }
            }
            _ => normalized.push(component.as_os_str()),
        }
    }
    normalized
}

fn relative_path(base: &Path, target: &Path) -> Option<PathBuf> {
    let base_components = base.components().collect::<Vec<_>>();
    let target_components = target.components().collect::<Vec<_>>();

    let mut common = 0;
    while common < base_components.len()
        && common < target_components.len()
        && base_components[common] == target_components[common]
    {
        common += 1;
    }

    if common == 0
        && base_components
            .first()
            .is_some_and(|component| matches!(component, std::path::Component::Prefix(_)))
    {
        return None;
    }

    let mut relative = PathBuf::new();
    for _ in common..base_components.len() {
        relative.push("..");
    }
    for component in &target_components[common..] {
        relative.push(component.as_os_str());
    }
    if relative.as_os_str().is_empty() {
        relative.push(".");
    }
    Some(relative)
}

fn required_dependency_string(
    alias: &str,
    fields: &toml::map::Map<String, toml::Value>,
    key: &str,
) -> Result<String, String> {
    optional_dependency_string(alias, fields, key)?
        .ok_or_else(|| format!("dependency `{alias}` is missing `{key}`"))
}

fn optional_dependency_string(
    alias: &str,
    fields: &toml::map::Map<String, toml::Value>,
    key: &str,
) -> Result<Option<String>, String> {
    match fields.get(key) {
        Some(value) => value
            .as_str()
            .map(|value| Some(value.to_string()))
            .ok_or_else(|| format!("dependency `{alias}` field `{key}` must be a string")),
        None => Ok(None),
    }
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

fn validate_workspace_update_target(
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
            DependencySource::Registry { version, registry } => match resolve_registry_source(
                base_root,
                &dependency.alias,
                &dependency.package,
                version,
                registry.as_deref(),
                offline,
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
            },
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

fn package_checksum(root: &Path) -> Result<String, String> {
    let mut files = Vec::new();
    let manifest = root.join("nomo.toml");
    if manifest.is_file() {
        files.push(manifest);
    }
    let src = root.join("src");
    if src.is_dir() {
        collect_source_files(&src, &mut files)?;
    }
    files.sort();

    let mut hasher = Sha256::new();
    for file in files {
        let relative = file
            .strip_prefix(root)
            .map_err(|err| err.to_string())?
            .to_string_lossy()
            .replace('\\', "/");
        hasher.update(relative.as_bytes());
        hasher.update(b"\0");
        let bytes = fs::read(&file)
            .map_err(|err| format!("failed to read {} for checksum: {err}", file.display()))?;
        hasher.update(bytes);
        hasher.update(b"\0");
    }
    Ok(format!("sha256:{}", hex_lower(&hasher.finalize())))
}

fn build_package_archive(root: &Path, manifest: &Manifest) -> Result<Vec<u8>, String> {
    let mut files = package_source_files(root)?;
    files.sort();

    let mut archive = Vec::new();
    writeln!(&mut archive, "nomo-package-v1").expect("write to Vec cannot fail");
    writeln!(
        &mut archive,
        "package {}/{}",
        manifest.package.namespace, manifest.package.name
    )
    .expect("write to Vec cannot fail");
    writeln!(&mut archive, "version {}", manifest.package.version)
        .expect("write to Vec cannot fail");
    writeln!(&mut archive, "files {}", files.len()).expect("write to Vec cannot fail");

    for file in files {
        let relative = file
            .strip_prefix(root)
            .map_err(|err| err.to_string())?
            .to_string_lossy()
            .replace('\\', "/");
        if relative.contains('\n') || relative.contains('\0') {
            return Err(format!("package file path `{relative}` is not publishable"));
        }
        let bytes = fs::read(&file)
            .map_err(|err| format!("failed to read {} for archive: {err}", file.display()))?;
        let file_checksum = archive_checksum(&bytes);
        writeln!(
            &mut archive,
            "file {} {} {}",
            relative,
            bytes.len(),
            file_checksum
        )
        .expect("write to Vec cannot fail");
        archive.extend_from_slice(&bytes);
        archive.push(b'\n');
    }

    Ok(archive)
}

fn package_source_files(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    let manifest = root.join("nomo.toml");
    if !manifest.is_file() {
        return Err(format!("package is missing {}", manifest.display()));
    }
    files.push(manifest);
    let src = root.join("src");
    if !src.is_dir() {
        return Err(format!("package is missing {}", src.display()));
    }
    collect_source_files(&src, &mut files)?;
    if files.len() == 1 {
        return Err(format!(
            "package source directory is empty: {}",
            src.display()
        ));
    }
    Ok(files)
}

fn archive_checksum(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("sha256:{}", hex_lower(&hasher.finalize()))
}

fn package_archive_filename(package: &str, version: &str) -> String {
    format!("{}-{}.nomo-package", package.replace('/', "-"), version)
}

fn resolve_registry_source(
    base_root: &Path,
    alias: &str,
    package: &str,
    version: &str,
    registry: Option<&str>,
    offline: bool,
) -> Result<Option<PathBuf>, String> {
    if let Some(source_root) = registry_cached_source_root(base_root, package, version, registry)? {
        return Ok(Some(source_root));
    }
    if offline {
        return Ok(None);
    }
    let Some(registry) = registry else {
        return Ok(None);
    };
    let Some(archive) = read_registry_archive(alias, registry, package, version)? else {
        return Ok(None);
    };
    let cache_root = registry_cache_root(base_root, package, version, Some(registry));
    fs::create_dir_all(&cache_root).map_err(|err| err.to_string())?;
    let archive_path = cache_root.join("package.nomo-package");
    fs::write(&archive_path, &archive).map_err(|err| {
        format!(
            "failed to cache registry dependency `{alias}` archive at {}: {err}",
            archive_path.display()
        )
    })?;
    let source_root = cache_root.join("source");
    unpack_package_archive(&archive, package, version, &source_root)?;
    Ok(Some(source_root))
}

fn registry_cached_source_root(
    base_root: &Path,
    package: &str,
    version: &str,
    _registry: Option<&str>,
) -> Result<Option<PathBuf>, String> {
    let source_root = registry_cache_root(base_root, package, version, _registry).join("source");
    if !source_root.exists() {
        return Ok(None);
    }
    fs::canonicalize(&source_root).map(Some).map_err(|err| {
        format!(
            "failed to resolve cached registry package `{package}` at {}: {err}",
            source_root.display()
        )
    })
}

fn registry_cache_root(
    base_root: &Path,
    package: &str,
    version: &str,
    registry: Option<&str>,
) -> PathBuf {
    let mut root = base_root.join(".nomo/cache/registry");
    for segment in package.split('/') {
        root.push(segment);
    }
    root.push(version);
    root.push(registry_cache_key(registry));
    root
}

fn registry_cache_key(registry: Option<&str>) -> String {
    let Some(registry) = registry else {
        return "default".to_string();
    };
    let mut hasher = Sha256::new();
    hasher.update(registry.as_bytes());
    hex_lower(&hasher.finalize())
}

fn registry_file_download_path(
    registry: &str,
    package: &str,
    version: &str,
) -> Result<Option<PathBuf>, String> {
    let Some(root) = registry.strip_prefix("file://") else {
        return Ok(None);
    };
    let mut path = PathBuf::from(root);
    path.push("api/v1/packages");
    let (owner, name) = package.split_once('/').ok_or_else(|| {
        format!("registry package `{package}` must use canonical owner/package form")
    })?;
    path.push(owner);
    path.push(name);
    path.push(version);
    path.push("download");
    Ok(Some(path))
}

fn read_registry_archive(
    alias: &str,
    registry: &str,
    package: &str,
    version: &str,
) -> Result<Option<Vec<u8>>, String> {
    if let Some(download_path) = registry_file_download_path(registry, package, version)? {
        if !download_path.is_file() {
            return Err(format!(
                "registry dependency `{alias}` archive is missing at {}",
                download_path.display()
            ));
        }
        return fs::read(&download_path).map(Some).map_err(|err| {
            format!(
                "failed to read registry dependency `{alias}` archive at {}: {err}",
                download_path.display()
            )
        });
    }
    if registry.starts_with("http://") {
        return fetch_http_registry_archive(alias, registry, package, version).map(Some);
    }
    Ok(None)
}

fn fetch_http_registry_archive(
    alias: &str,
    registry: &str,
    package: &str,
    version: &str,
) -> Result<Vec<u8>, String> {
    let request = http_registry_request(registry, package, version)?;
    let mut stream = TcpStream::connect((&*request.host, request.port)).map_err(|err| {
        format!(
            "failed to connect to registry `{}` for dependency `{alias}`: {err}",
            request.authority
        )
    })?;
    let request_text = format!(
        "GET {} HTTP/1.1\r\nHost: {}\r\nUser-Agent: nomo/0.1\r\nAccept: application/octet-stream\r\nConnection: close\r\n\r\n",
        request.path, request.authority
    );
    stream
        .write_all(request_text.as_bytes())
        .map_err(|err| format!("failed to request registry dependency `{alias}`: {err}"))?;
    let mut response = Vec::new();
    stream
        .read_to_end(&mut response)
        .map_err(|err| format!("failed to read registry response for `{alias}`: {err}"))?;
    parse_http_registry_response(alias, registry, &response)
}

fn upload_http_registry_archive(
    registry: &str,
    package: &str,
    version: &str,
    archive: &[u8],
) -> Result<(), String> {
    if !registry.starts_with("http://") {
        return Err(format!(
            "registry upload currently supports only http:// endpoints, got `{registry}`"
        ));
    }
    let request = http_registry_upload_request(registry, package, version)?;
    let mut stream = TcpStream::connect((&*request.host, request.port)).map_err(|err| {
        format!(
            "failed to connect to registry `{}` for package `{package}`: {err}",
            request.authority
        )
    })?;
    let request_text = format!(
        "PUT {} HTTP/1.1\r\nHost: {}\r\nUser-Agent: nomo/0.1\r\nContent-Type: application/octet-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        request.path,
        request.authority,
        archive.len()
    );
    stream
        .write_all(request_text.as_bytes())
        .and_then(|_| stream.write_all(archive))
        .map_err(|err| format!("failed to upload package `{package}`: {err}"))?;
    let mut response = Vec::new();
    stream
        .read_to_end(&mut response)
        .map_err(|err| format!("failed to read registry upload response for `{package}`: {err}"))?;
    parse_http_registry_upload_response(registry, package, version, &response)
}

struct HttpRegistryRequest {
    host: String,
    port: u16,
    authority: String,
    path: String,
}

fn http_registry_request(
    registry: &str,
    package: &str,
    version: &str,
) -> Result<HttpRegistryRequest, String> {
    http_registry_api_request(registry, package, version, "/download")
}

fn http_registry_upload_request(
    registry: &str,
    package: &str,
    version: &str,
) -> Result<HttpRegistryRequest, String> {
    http_registry_api_request(registry, package, version, "")
}

fn http_registry_api_request(
    registry: &str,
    package: &str,
    version: &str,
    suffix: &str,
) -> Result<HttpRegistryRequest, String> {
    let rest = registry
        .strip_prefix("http://")
        .ok_or_else(|| format!("registry endpoint `{registry}` must start with http://"))?;
    let (authority, base_path) = rest
        .split_once('/')
        .map(|(authority, path)| (authority, format!("/{path}")))
        .unwrap_or((rest, String::new()));
    if authority.is_empty() {
        return Err("registry endpoint is missing a host".to_string());
    }
    let (host, port) = parse_http_authority(authority)?;
    let base_path = base_path.trim_end_matches('/');
    let (owner, name) = package.split_once('/').ok_or_else(|| {
        format!("registry package `{package}` must use canonical owner/package form")
    })?;
    Ok(HttpRegistryRequest {
        host,
        port,
        authority: authority.to_string(),
        path: format!("{base_path}/api/v1/packages/{owner}/{name}/{version}{suffix}"),
    })
}

fn parse_http_authority(authority: &str) -> Result<(String, u16), String> {
    let Some((host, port)) = authority.rsplit_once(':') else {
        return Ok((authority.to_string(), 80));
    };
    if host.is_empty() || port.is_empty() || host.contains(']') {
        return Ok((authority.to_string(), 80));
    }
    let port = port
        .parse::<u16>()
        .map_err(|_| format!("registry endpoint `{authority}` has invalid port"))?;
    Ok((host.to_string(), port))
}

fn parse_http_registry_response(
    alias: &str,
    registry: &str,
    response: &[u8],
) -> Result<Vec<u8>, String> {
    let Some(header_end) = response.windows(4).position(|window| window == b"\r\n\r\n") else {
        return Err(format!(
            "registry `{registry}` returned a malformed response for dependency `{alias}`"
        ));
    };
    let headers = String::from_utf8(response[..header_end].to_vec()).map_err(|_| {
        format!("registry `{registry}` returned non-UTF-8 headers for dependency `{alias}`")
    })?;
    let status = headers
        .lines()
        .next()
        .ok_or_else(|| format!("registry `{registry}` returned an empty response"))?;
    if !status.starts_with("HTTP/1.1 200 ") && !status.starts_with("HTTP/1.0 200 ") {
        return Err(format!(
            "registry `{registry}` failed to fetch dependency `{alias}`: {status}"
        ));
    }
    Ok(response[header_end + 4..].to_vec())
}

fn parse_http_registry_upload_response(
    registry: &str,
    package: &str,
    version: &str,
    response: &[u8],
) -> Result<(), String> {
    let Some(header_end) = response.windows(4).position(|window| window == b"\r\n\r\n") else {
        return Err(format!(
            "registry `{registry}` returned a malformed upload response for `{package}` {version}"
        ));
    };
    let headers = String::from_utf8(response[..header_end].to_vec()).map_err(|_| {
        format!("registry `{registry}` returned non-UTF-8 upload headers for `{package}` {version}")
    })?;
    let status = headers
        .lines()
        .next()
        .ok_or_else(|| format!("registry `{registry}` returned an empty upload response"))?;
    if !status.starts_with("HTTP/1.1 200 ")
        && !status.starts_with("HTTP/1.1 201 ")
        && !status.starts_with("HTTP/1.1 204 ")
        && !status.starts_with("HTTP/1.0 200 ")
        && !status.starts_with("HTTP/1.0 201 ")
        && !status.starts_with("HTTP/1.0 204 ")
    {
        return Err(format!(
            "registry `{registry}` failed to publish `{package}` {version}: {status}"
        ));
    }
    Ok(())
}

fn unpack_package_archive(
    archive: &[u8],
    expected_package: &str,
    expected_version: &str,
    target: &Path,
) -> Result<(), String> {
    let mut cursor = 0usize;
    expect_archive_line(archive, &mut cursor, "nomo-package-v1")?;
    expect_archive_line(archive, &mut cursor, &format!("package {expected_package}"))?;
    expect_archive_line(archive, &mut cursor, &format!("version {expected_version}"))?;
    let files_line = read_archive_line(archive, &mut cursor)?;
    let Some(files_count) = files_line
        .strip_prefix("files ")
        .and_then(|count| count.parse::<usize>().ok())
    else {
        return Err("package archive is missing file count".to_string());
    };

    if target.exists() {
        fs::remove_dir_all(target).map_err(|err| {
            format!(
                "failed to replace cached registry package at {}: {err}",
                target.display()
            )
        })?;
    }
    fs::create_dir_all(target).map_err(|err| err.to_string())?;

    for _ in 0..files_count {
        let file_line = read_archive_line(archive, &mut cursor)?;
        let (relative, length, expected_checksum) = parse_archive_file_header(&file_line)?;
        let end = cursor
            .checked_add(length)
            .ok_or_else(|| "package archive file length overflowed".to_string())?;
        if end > archive.len() {
            return Err(format!(
                "package archive ended before file `{relative}` was complete"
            ));
        }
        let bytes = &archive[cursor..end];
        let actual_checksum = archive_checksum(bytes);
        if actual_checksum != expected_checksum {
            return Err(format!(
                "checksum mismatch for package archive file `{relative}`: expected {expected_checksum}, found {actual_checksum}"
            ));
        }
        let output = archive_output_path(target, &relative)?;
        if let Some(parent) = output.parent() {
            fs::create_dir_all(parent).map_err(|err| err.to_string())?;
        }
        fs::write(&output, bytes)
            .map_err(|err| format!("failed to write {}: {err}", output.display()))?;
        cursor = end;
        if archive.get(cursor) != Some(&b'\n') {
            return Err(format!(
                "package archive file `{relative}` is missing trailing newline"
            ));
        }
        cursor += 1;
    }
    if cursor != archive.len() {
        return Err("package archive contains trailing data".to_string());
    }
    if !target.join("nomo.toml").is_file() || !target.join("src").is_dir() {
        return Err("package archive must contain nomo.toml and src/".to_string());
    }
    Ok(())
}

fn expect_archive_line(archive: &[u8], cursor: &mut usize, expected: &str) -> Result<(), String> {
    let actual = read_archive_line(archive, cursor)?;
    if actual != expected {
        return Err(format!(
            "package archive expected `{expected}`, found `{actual}`"
        ));
    }
    Ok(())
}

fn read_archive_line(archive: &[u8], cursor: &mut usize) -> Result<String, String> {
    let start = *cursor;
    let Some(offset) = archive[start..].iter().position(|byte| *byte == b'\n') else {
        return Err("package archive is truncated".to_string());
    };
    let end = start + offset;
    *cursor = end + 1;
    String::from_utf8(archive[start..end].to_vec())
        .map_err(|_| "package archive header is not UTF-8".to_string())
}

fn parse_archive_file_header(header: &str) -> Result<(String, usize, String), String> {
    let mut parts = header.split(' ');
    if parts.next() != Some("file") {
        return Err(format!(
            "package archive expected file header, found `{header}`"
        ));
    }
    let relative = parts
        .next()
        .ok_or_else(|| "package archive file header is missing path".to_string())?
        .to_string();
    let length = parts
        .next()
        .and_then(|length| length.parse::<usize>().ok())
        .ok_or_else(|| format!("package archive file `{relative}` has invalid length"))?;
    let checksum = parts
        .next()
        .ok_or_else(|| format!("package archive file `{relative}` is missing checksum"))?
        .to_string();
    if parts.next().is_some() {
        return Err(format!(
            "package archive file `{relative}` has malformed header"
        ));
    }
    validate_checksum(&relative, &checksum)?;
    Ok((relative, length, checksum))
}

fn archive_output_path(root: &Path, relative: &str) -> Result<PathBuf, String> {
    let relative_path = Path::new(relative);
    if relative_path.is_absolute() {
        return Err(format!(
            "package archive path `{relative}` must be relative"
        ));
    }
    let mut output = root.to_path_buf();
    for component in relative_path.components() {
        match component {
            Component::Normal(segment) => output.push(segment),
            _ => return Err(format!("package archive path `{relative}` is not safe")),
        }
    }
    Ok(output)
}

fn collect_source_files(dir: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    for entry in fs::read_dir(dir).map_err(|err| err.to_string())? {
        let entry = entry.map_err(|err| err.to_string())?;
        let path = entry.path();
        if path.is_dir() {
            collect_source_files(&path, files)?;
        } else if path.is_file() {
            files.push(path);
        }
    }
    Ok(())
}

fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
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

fn render_lockfile(graph: &DependencyGraph) -> String {
    let document = LockfileDocument {
        root: Vec::new(),
        package: flatten_dependencies(&graph.dependencies)
            .into_iter()
            .map(LockPackage::from_resolved)
            .collect(),
    };
    render_lockfile_document(&document)
}

fn render_workspace_lockfile(graphs: &[DependencyGraph]) -> Result<String, String> {
    let mut root_ids = BTreeSet::new();
    let mut packages = BTreeMap::new();
    let mut package_sources = BTreeMap::new();
    let mut roots = Vec::new();

    for graph in graphs {
        let root_id = format!("{}/{}", graph.root.namespace, graph.root.name);
        if !root_ids.insert(root_id.clone()) {
            return Err(format!(
                "workspace lockfile has duplicate root package `{root_id}`"
            ));
        }
        roots.push(LockRoot::from_graph(graph));
        for dependency in flatten_dependencies(&graph.dependencies) {
            remember_package_source(
                &mut package_sources,
                &dependency.package,
                &dependency.source,
            )?;
            let package = LockPackage::from_resolved(dependency);
            let key = (package.alias.clone(), package.id.clone());
            match packages.get(&key) {
                Some(existing) if existing != &package => {
                    return Err(format!(
                        "workspace lockfile has conflicting entries for `{} -> {}`",
                        package.alias, package.id
                    ));
                }
                Some(_) => {}
                None => {
                    packages.insert(key, package);
                }
            }
        }
    }

    let document = LockfileDocument {
        root: roots,
        package: packages.into_values().collect(),
    };
    Ok(render_lockfile_document(&document))
}

fn render_lockfile_document(document: &LockfileDocument) -> String {
    let mut out = String::from("# This file is generated by `nomo deps resolve`.\n\n");
    out.push_str(&toml::to_string(&document).expect("lockfile document should serialize"));
    out
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
    let document = parse_lockfile_document(&text)?;
    let root_id = format!("{}/{}", manifest.package.namespace, manifest.package.name);
    let root_edges = document
        .root
        .iter()
        .find(|root| root.id == root_id)
        .map(LockRoot::dependency_edges)
        .transpose()?;
    let has_workspace_roots = !document.root.is_empty();
    let packages = document
        .package
        .into_iter()
        .map(LockPackage::into_resolved)
        .collect::<Result<Vec<_>, _>>()?;
    let dependencies = match root_edges {
        Some(edges) => build_locked_dependencies_from_edges(&edges, &packages)?,
        None if has_workspace_roots => {
            return Err(format!(
                "nomo.lock does not contain workspace root `{root_id}`"
            ));
        }
        None => {
            let referenced_packages = packages
                .iter()
                .flat_map(|package| {
                    package
                        .dependencies
                        .iter()
                        .map(|dependency| dependency.package.clone())
                })
                .collect::<BTreeSet<_>>();
            packages
                .iter()
                .filter(|package| !referenced_packages.contains(&package.package))
                .map(|package| build_locked_dependency(package, &packages, &mut Vec::new()))
                .collect::<Result<Vec<_>, _>>()?
        }
    };
    let checksum_base = if has_workspace_roots { lock_root } else { root };
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

fn validate_locked_source_matches_manifest(
    manifest: &Dependency,
    locked: &ResolvedDependency,
) -> Result<(), String> {
    match (&manifest.source, &locked.source) {
        (
            DependencySource::Registry { version, registry },
            DependencySource::Registry {
                version: locked_version,
                registry: locked_registry,
            },
        ) if version == locked_version && registry == locked_registry => Ok(()),
        (DependencySource::Path { .. }, DependencySource::Path { .. }) => Ok(()),
        (
            DependencySource::Git {
                git,
                branch,
                tag,
                rev,
            },
            DependencySource::Git {
                git: locked_git,
                branch: locked_branch,
                tag: locked_tag,
                rev: locked_rev,
            },
        ) if git == locked_git
            && branch == locked_branch
            && tag == locked_tag
            && rev
                .as_ref()
                .is_none_or(|rev| Some(rev) == locked_rev.as_ref()) =>
        {
            Ok(())
        }
        _ => Err(format!(
            "nomo.lock is out of date: dependency `{}` source no longer matches manifest",
            manifest.alias
        )),
    }
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

fn lock_source_string(dependency: &ResolvedDependency) -> String {
    LockPackage::from_resolved(dependency).source
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

fn build_locked_dependency(
    dependency: &ResolvedDependency,
    packages: &[ResolvedDependency],
    stack: &mut Vec<String>,
) -> Result<ResolvedDependency, String> {
    if stack.contains(&dependency.package) {
        return Err(format!(
            "cyclic dependency in nomo.lock involving `{}`",
            dependency.package
        ));
    }

    stack.push(dependency.package.clone());
    let mut children = Vec::new();
    for child in &dependency.dependencies {
        let locked_child = packages
            .iter()
            .find(|package| package.package == child.package && package.alias == child.alias)
            .or_else(|| {
                packages
                    .iter()
                    .find(|package| package.package == child.package)
            })
            .ok_or_else(|| {
                format!(
                    "nomo.lock references missing dependency `{} -> {}`",
                    child.alias, child.package
                )
            })?;
        children.push(build_locked_dependency(locked_child, packages, stack)?);
    }
    stack.pop();

    Ok(ResolvedDependency {
        alias: dependency.alias.clone(),
        package: dependency.package.clone(),
        source: dependency.source.clone(),
        checksum: dependency.checksum.clone(),
        dependencies: children,
    })
}

fn build_locked_dependencies_from_edges(
    edges: &[DependencyEdge],
    packages: &[ResolvedDependency],
) -> Result<Vec<ResolvedDependency>, String> {
    edges
        .iter()
        .map(|edge| {
            let locked_child = packages
                .iter()
                .find(|package| package.package == edge.package && package.alias == edge.alias)
                .or_else(|| {
                    packages
                        .iter()
                        .find(|package| package.package == edge.package)
                })
                .ok_or_else(|| {
                    format!(
                        "nomo.lock references missing dependency `{} -> {}`",
                        edge.alias, edge.package
                    )
                })?;
            build_locked_dependency(locked_child, packages, &mut Vec::new())
        })
        .collect()
}

#[cfg(test)]
fn parse_lockfile_text(lockfile: &str) -> Result<Vec<ResolvedDependency>, String> {
    parse_lockfile_document(lockfile)?
        .package
        .into_iter()
        .map(LockPackage::into_resolved)
        .collect()
}

fn parse_lockfile_document(lockfile: &str) -> Result<LockfileDocument, String> {
    toml::from_str(lockfile).map_err(|err| format!("failed to parse nomo.lock as TOML: {err}"))
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct LockfileDocument {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    root: Vec<LockRoot>,
    #[serde(default)]
    package: Vec<LockPackage>,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct LockRoot {
    id: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    dependencies: Vec<String>,
}

impl LockRoot {
    fn from_graph(graph: &DependencyGraph) -> Self {
        Self {
            id: format!("{}/{}", graph.root.namespace, graph.root.name),
            dependencies: graph
                .dependencies
                .iter()
                .map(|dependency| format!("{} -> {}", dependency.alias, dependency.package))
                .collect(),
        }
    }

    fn dependency_edges(&self) -> Result<Vec<DependencyEdge>, String> {
        validate_package_id(&self.id)?;
        self.dependencies
            .iter()
            .map(|entry| parse_lock_dependency_entry(entry))
            .collect()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct LockPackage {
    id: String,
    alias: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    version: Option<String>,
    source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    branch: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tag: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    rev: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    checksum: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    dependencies: Vec<String>,
}

impl LockPackage {
    fn from_resolved(dependency: &ResolvedDependency) -> Self {
        let (version, source, branch, tag, rev) = match &dependency.source {
            DependencySource::Registry { version, registry } => {
                let registry_source = registry.as_deref().unwrap_or(&dependency.package);
                (
                    Some(version.clone()),
                    format!("registry+{registry_source}"),
                    None,
                    None,
                    None,
                )
            }
            DependencySource::Path { path } => (None, format!("path+{path}"), None, None, None),
            DependencySource::Git {
                git,
                branch,
                tag,
                rev,
            } => (
                None,
                format!("git+{git}"),
                branch.clone(),
                tag.clone(),
                rev.clone(),
            ),
        };
        Self {
            id: dependency.package.clone(),
            alias: dependency.alias.clone(),
            version,
            source,
            branch,
            tag,
            rev,
            checksum: dependency.checksum.clone(),
            dependencies: dependency
                .dependencies
                .iter()
                .map(|child| format!("{} -> {}", child.alias, child.package))
                .collect(),
        }
    }

    fn into_resolved(self) -> Result<ResolvedDependency, String> {
        validate_package_id(&self.id)?;
        validate_dependency_alias(&self.alias)?;
        let source = parse_lock_source(
            &self.id,
            &self.source,
            self.version,
            self.branch,
            self.tag,
            self.rev,
        )?;
        if let Some(checksum) = self.checksum.as_deref() {
            validate_checksum(&self.id, checksum)?;
        }
        let dependencies = self
            .dependencies
            .into_iter()
            .map(|entry| parse_lock_dependency_entry(&entry))
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .map(|edge| ResolvedDependency {
                alias: edge.alias,
                package: edge.package,
                source: DependencySource::Registry {
                    version: "locked".to_string(),
                    registry: None,
                },
                checksum: None,
                dependencies: Vec::new(),
            })
            .collect();
        Ok(ResolvedDependency {
            alias: self.alias,
            package: self.id,
            source,
            checksum: self.checksum,
            dependencies,
        })
    }
}

#[derive(Debug, Clone)]
struct DependencyEdge {
    alias: String,
    package: String,
}

fn parse_lock_dependency_entry(entry: &str) -> Result<DependencyEdge, String> {
    let Some((alias, package)) = entry.split_once(" -> ") else {
        return Err(format!(
            "lockfile dependency entry `{entry}` must use `alias -> owner/package`"
        ));
    };
    validate_dependency_alias(alias)?;
    validate_package_id(package)?;
    Ok(DependencyEdge {
        alias: alias.to_string(),
        package: package.to_string(),
    })
}

fn validate_lock_source_selectors(package: &str, source: &DependencySource) -> Result<(), String> {
    if let DependencySource::Git {
        branch, tag, rev, ..
    } = source
    {
        let selector_count = [branch, tag, rev]
            .iter()
            .filter(|selector| selector.is_some())
            .count();
        if selector_count > 2 || (selector_count == 2 && rev.is_none()) {
            return Err(format!(
                "lockfile package `{package}` git source can only combine one selector with resolved `rev`"
            ));
        }
    }
    Ok(())
}

fn validate_lock_version_shape(package: &str, source: &DependencySource) -> Result<(), String> {
    if let DependencySource::Registry { version, .. } = source {
        validate_version_like(&format!("lockfile package `{package}` version"), version)?;
    }
    Ok(())
}

fn validate_checksum(package: &str, checksum: &str) -> Result<(), String> {
    let Some(hex) = checksum.strip_prefix("sha256:") else {
        return Err(format!(
            "lockfile package `{package}` checksum must use `sha256:<hex>`"
        ));
    };
    let valid = hex.len() == 64 && hex.chars().all(|ch| ch.is_ascii_hexdigit());
    if valid {
        Ok(())
    } else {
        Err(format!(
            "lockfile package `{package}` checksum must contain 64 hexadecimal digits"
        ))
    }
}

fn parse_lock_source(
    package: &str,
    source: &str,
    version: Option<String>,
    branch: Option<String>,
    tag: Option<String>,
    rev: Option<String>,
) -> Result<DependencySource, String> {
    let source = if let Some(registry_source) = source.strip_prefix("registry+") {
        if branch.is_some() || tag.is_some() || rev.is_some() {
            return Err(format!(
                "lockfile package `{package}` registry source must not declare git selectors"
            ));
        }
        let version =
            version.ok_or_else(|| format!("registry package `{package}` is missing `version`"))?;
        let registry = if registry_source == package {
            None
        } else {
            Some(registry_source.to_string())
        };
        DependencySource::Registry { version, registry }
    } else if let Some(path) = source.strip_prefix("path+") {
        if version.is_some() {
            return Err(format!(
                "lockfile package `{package}` path source must not declare `version`"
            ));
        }
        if branch.is_some() || tag.is_some() || rev.is_some() {
            return Err(format!(
                "lockfile package `{package}` path source must not declare git selectors"
            ));
        }
        DependencySource::Path {
            path: path.to_string(),
        }
    } else if let Some(git) = source.strip_prefix("git+") {
        if version.is_some() {
            return Err(format!(
                "lockfile package `{package}` git source must not declare `version`"
            ));
        }
        DependencySource::Git {
            git: git.to_string(),
            branch,
            tag,
            rev,
        }
    } else {
        return Err(format!(
            "lockfile package `{package}` has unsupported source `{source}`"
        ));
    };
    validate_lock_version_shape(package, &source)?;
    validate_lock_source_selectors(package, &source)?;
    Ok(source)
}

fn flatten_dependencies(dependencies: &[ResolvedDependency]) -> Vec<&ResolvedDependency> {
    let mut flattened = Vec::new();
    for dependency in dependencies {
        flattened.push(dependency);
        flattened.extend(flatten_dependencies(&dependency.dependencies));
    }
    flattened
}

fn render_dependency_tree(graph: &DependencyGraph) -> String {
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

fn source_description(source: &DependencySource) -> String {
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

fn validate_package_id(value: &str) -> Result<(), String> {
    let Some((owner, package)) = value.split_once('/') else {
        return Err(format!(
            "canonical package id `{value}` must use `owner/package`"
        ));
    };
    if package.contains('/') {
        return Err(format!(
            "canonical package id `{value}` must contain exactly one `/`"
        ));
    }
    validate_package_namespace("package namespace", owner)?;
    validate_package_segment("package name", package)
}

fn validate_package_namespace(label: &str, value: &str) -> Result<(), String> {
    validate_package_segment(label, value)?;
    if matches!(value, "std" | "nomo" | "core") {
        Err(format!(
            "{label} `{value}` is reserved; use an organization or user namespace such as `nomo-lang`"
        ))
    } else {
        Ok(())
    }
}

fn validate_package_segment(label: &str, value: &str) -> Result<(), String> {
    let valid = !value.is_empty()
        && value
            .chars()
            .all(|ch| ch == '-' || ch.is_ascii_lowercase() || ch.is_ascii_digit())
        && value
            .chars()
            .next()
            .is_some_and(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit())
        && value
            .chars()
            .last()
            .is_some_and(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit());
    if valid {
        Ok(())
    } else {
        Err(format!(
            "{label} `{value}` must use lowercase letters, digits, or hyphens, and cannot start or end with `-`"
        ))
    }
}

fn validate_dependency_alias(value: &str) -> Result<(), String> {
    let valid = !value.is_empty()
        && value
            .chars()
            .all(|ch| ch == '_' || ch.is_ascii_lowercase() || ch.is_ascii_digit())
        && value
            .chars()
            .next()
            .is_some_and(|ch| ch == '_' || ch.is_ascii_lowercase());
    if valid {
        Ok(())
    } else {
        Err(format!(
            "dependency alias `{value}` must be a lowercase Nomo identifier"
        ))
    }
}

fn validate_version_like(label: &str, value: &str) -> Result<(), String> {
    let valid = !value.is_empty()
        && value
            .chars()
            .all(|ch| ch == '.' || ch == '-' || ch.is_ascii_alphanumeric());
    if valid {
        Ok(())
    } else {
        Err(format!("{label} `{value}` contains unsupported characters"))
    }
}

fn is_package_name(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|ch| ch == '-' || ch.is_ascii_lowercase() || ch.is_ascii_digit())
        && value
            .chars()
            .next()
            .is_some_and(|ch| ch.is_ascii_lowercase())
        && value
            .chars()
            .last()
            .is_some_and(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit())
}

fn is_legacy_package_name(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|ch| ch == '-' || ch == '_' || ch.is_ascii_alphanumeric())
        && value
            .chars()
            .next()
            .is_some_and(|ch| ch == '_' || ch.is_ascii_alphabetic())
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
