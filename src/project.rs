use crate::compiler::{
    ExternalModule, check_source_text_with_project_modules, compile_script_source_to_c,
    compile_source_to_c_with_project_modules,
};
use crate::diagnostic::Diagnostic;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone)]
pub struct Project {
    pub root: PathBuf,
    pub name: String,
    pub main: PathBuf,
}

#[derive(Debug, Clone)]
pub struct ProjectModuleContext {
    pub local_source_root: PathBuf,
    pub external_import_roots: Vec<String>,
    pub external_modules: Vec<ExternalModule>,
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
            "[package]\nnamespace = \"local\"\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2026\"\n\n[dependencies]\nstd = {{ package = \"nomo-lang/std\", version = \"0.1.0\" }}\n"
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
    let manifest = root.join("nomo.toml");
    let main = if source_file {
        path.to_path_buf()
    } else {
        root.join("src/main.nomo")
    };
    let name = if manifest.exists() {
        parse_manifest_at_root(&root)?.package.name
    } else {
        root.file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string()
    };
    Ok(Project { root, name, main })
}

fn find_manifest_root(start: &Path) -> Option<PathBuf> {
    for candidate in start.ancestors() {
        if candidate.join("nomo.toml").exists() {
            return Some(candidate.to_path_buf());
        }
    }
    None
}

pub fn parse_manifest_at_root(root: &Path) -> Result<Manifest, String> {
    let manifest_path = root.join("nomo.toml");
    let text = fs::read_to_string(&manifest_path).map_err(|err| err.to_string())?;
    parse_manifest_text(&text, root)
}

pub fn resolve_project_dependencies(project: &Project) -> Result<PathBuf, String> {
    let graph = resolve_dependency_graph(&project.root)?;
    let lock = render_lockfile(&graph);
    let lock_path = project.root.join("nomo.lock");
    fs::write(&lock_path, lock).map_err(|err| err.to_string())?;
    Ok(lock_path)
}

pub fn dependency_tree(project: &Project) -> Result<String, String> {
    let graph = if project.root.join("nomo.lock").is_file() {
        dependency_graph_from_lockfile(&project.root)?
    } else {
        resolve_dependency_graph(&project.root)?
    };
    Ok(render_dependency_tree(&graph))
}

pub fn project_module_context(project: &Project) -> Result<ProjectModuleContext, String> {
    let manifest = parse_manifest_at_root(&project.root)?;
    let mut aliases = Vec::new();
    let mut modules = Vec::new();
    for dependency in manifest.dependencies {
        if dependency.alias == "std" {
            continue;
        }
        if let Some(dep_root) = dependency_module_root(&project.root, &dependency)? {
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

fn dependency_module_root(
    base_root: &Path,
    dependency: &Dependency,
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
        } => resolve_git_source(
            base_root,
            &dependency.alias,
            git,
            branch.as_deref(),
            tag.as_deref(),
            rev.as_deref(),
        )?,
        DependencySource::Registry { .. } => return Ok(None),
    };
    validate_dependency_package(&dep_root, dependency)?;
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
            "N0901",
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
            "N0001",
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
    let context = project_module_context(project).map_err(BuildError::Message)?;
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

pub fn clean_project(project: &Project) -> Result<PathBuf, String> {
    let build_dir = project.root.join("build");
    if build_dir.exists() {
        fs::remove_dir_all(&build_dir).map_err(|err| err.to_string())?;
    }
    Ok(build_dir)
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

fn parse_manifest_text(manifest: &str, root: &Path) -> Result<Manifest, String> {
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
    let mut aliases = BTreeSet::new();
    let mut section = "";
    let mut namespace_explicit = false;

    for line in manifest.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            section = &line[1..line.len() - 1];
            continue;
        }
        match section {
            "package" => {
                if split_key_value(line).is_some_and(|(key, _)| key == "namespace") {
                    namespace_explicit = true;
                }
                parse_package_field(line, &mut package)?;
            }
            "dependencies" => {
                let dependency = parse_dependency_line(line)?;
                if !aliases.insert(dependency.alias.clone()) {
                    return Err(format!(
                        "duplicate dependency alias `{}` in nomo.toml",
                        dependency.alias
                    ));
                }
                dependencies.push(dependency);
            }
            _ => {}
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

fn resolve_dependency_graph(root: &Path) -> Result<DependencyGraph, String> {
    let root = fs::canonicalize(root).map_err(|err| err.to_string())?;
    let manifest = parse_manifest_at_root(&root)?;
    let mut package_sources = BTreeMap::new();
    let mut path_stack = vec![root.clone()];
    let dependencies = resolve_dependencies(
        &manifest.dependencies,
        &root,
        &mut path_stack,
        &mut package_sources,
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
                )?;
                let checksum = package_checksum(&dep_root)?;
                path_stack.pop();
                (
                    dependency.source.clone(),
                    Some(checksum),
                    child_dependencies,
                )
            }
            DependencySource::Git {
                git,
                branch,
                tag,
                rev,
            } => {
                let dep_root = resolve_git_source(
                    base_root,
                    &dependency.alias,
                    git,
                    branch.as_deref(),
                    tag.as_deref(),
                    rev.as_deref(),
                )?;
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
            DependencySource::Registry { .. } => (dependency.source.clone(), None, Vec::new()),
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
    git: &str,
    branch: Option<&str>,
    tag: Option<&str>,
    rev: Option<&str>,
) -> Result<PathBuf, String> {
    let cache_root = base_root.join(".nomo/deps/git");
    fs::create_dir_all(&cache_root).map_err(|err| err.to_string())?;
    let checkout = cache_root.join(git_cache_key(alias, git, branch, tag, rev));
    if checkout.exists() {
        fs::remove_dir_all(&checkout).map_err(|err| {
            format!(
                "failed to clear cached git dependency `{alias}` at {}: {err}",
                checkout.display()
            )
        })?;
    }

    let output = Command::new("git")
        .arg("clone")
        .arg("--quiet")
        .arg(git)
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

    if let Some(branch) = branch {
        let output = Command::new("git")
            .arg("-C")
            .arg(&checkout)
            .arg("checkout")
            .arg("--quiet")
            .arg(branch)
            .output()
            .map_err(|err| {
                format!(
                    "failed to run git checkout for dependency `{alias}` at branch `{branch}`: {err}"
                )
            })?;
        if !output.status.success() {
            return Err(format!(
                "failed to checkout git dependency `{alias}` at branch `{branch}`:\n{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            ));
        }
    } else if let Some(tag) = tag {
        let output = Command::new("git")
            .arg("-C")
            .arg(&checkout)
            .arg("checkout")
            .arg("--quiet")
            .arg(tag)
            .output()
            .map_err(|err| {
                format!("failed to run git checkout for dependency `{alias}` at tag `{tag}`: {err}")
            })?;
        if !output.status.success() {
            return Err(format!(
                "failed to checkout git dependency `{alias}` at tag `{tag}`:\n{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            ));
        }
    } else if let Some(rev) = rev {
        let output = Command::new("git")
            .arg("-C")
            .arg(&checkout)
            .arg("checkout")
            .arg("--quiet")
            .arg(rev)
            .output()
            .map_err(|err| {
                format!("failed to run git checkout for dependency `{alias}` at rev `{rev}`: {err}")
            })?;
        if !output.status.success() {
            return Err(format!(
                "failed to checkout git dependency `{alias}` at rev `{rev}`:\n{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            ));
        }
    }

    fs::canonicalize(&checkout).map_err(|err| err.to_string())
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

fn git_cache_key(
    alias: &str,
    git: &str,
    branch: Option<&str>,
    tag: Option<&str>,
    rev: Option<&str>,
) -> String {
    let mut hasher = DefaultHasher::new();
    alias.hash(&mut hasher);
    git.hash(&mut hasher);
    branch.hash(&mut hasher);
    tag.hash(&mut hasher);
    rev.hash(&mut hasher);
    format!("{}-{:016x}", alias, hasher.finish())
}

fn parse_package_field(line: &str, package: &mut PackageMetadata) -> Result<(), String> {
    let Some((key, value)) = split_key_value(line) else {
        return Ok(());
    };
    let Some(value) = parse_quoted_value(value) else {
        return Ok(());
    };
    match key {
        "namespace" => package.namespace = value,
        "name" => package.name = value,
        "version" => package.version = value,
        "edition" => package.edition = value,
        _ => {}
    }
    Ok(())
}

fn parse_dependency_line(line: &str) -> Result<Dependency, String> {
    let Some((alias, value)) = split_key_value(line) else {
        return Err(format!("invalid dependency entry `{line}`"));
    };
    validate_dependency_alias(alias)?;

    if let Some(version) = parse_quoted_value(value) {
        if alias != "std" {
            return Err(format!(
                "dependency `{alias}` must use an inline table with `package = \"owner/name\"`"
            ));
        }
        return Ok(Dependency {
            alias: alias.to_string(),
            package: "nomo-lang/std".to_string(),
            source: DependencySource::Registry {
                version,
                registry: None,
            },
        });
    }

    let fields = parse_inline_table(value)?;
    let package = fields
        .get("package")
        .cloned()
        .ok_or_else(|| format!("dependency `{alias}` is missing `package = \"owner/name\"`"))?;
    validate_package_id(&package)?;
    if alias == "std" && package != "nomo-lang/std" {
        return Err(
            "dependency alias `std` is reserved for the standard library package `nomo-lang/std`"
                .to_string(),
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

    let source = if let Some(path) = fields.get("path") {
        DependencySource::Path { path: path.clone() }
    } else if let Some(git) = fields.get("git") {
        DependencySource::Git {
            git: git.clone(),
            branch: fields.get("branch").cloned(),
            tag: fields.get("tag").cloned(),
            rev: fields.get("rev").cloned(),
        }
    } else if let Some(version) = fields.get("version") {
        validate_version_like(&format!("dependency `{alias}` version"), version)?;
        DependencySource::Registry {
            version: version.clone(),
            registry: fields.get("registry").cloned(),
        }
    } else {
        unreachable!("source key count already validated")
    };

    Ok(Dependency {
        alias: alias.to_string(),
        package,
        source,
    })
}

fn split_key_value(line: &str) -> Option<(&str, &str)> {
    let (key, value) = line.split_once('=')?;
    Some((key.trim(), value.trim()))
}

fn parse_quoted_value(value: &str) -> Option<String> {
    let value = value.trim();
    let value = value.strip_prefix('"')?;
    let end = value.find('"')?;
    Some(value[..end].to_string())
}

fn parse_inline_table(value: &str) -> Result<BTreeMap<String, String>, String> {
    let value = value.trim();
    let Some(value) = value.strip_prefix('{').and_then(|v| v.strip_suffix('}')) else {
        return Err(format!("expected inline dependency table, found `{value}`"));
    };
    let mut fields = BTreeMap::new();
    for part in value.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let Some((key, value)) = split_key_value(part) else {
            return Err(format!("invalid inline dependency field `{part}`"));
        };
        let Some(value) = parse_quoted_value(value) else {
            return Err(format!("dependency field `{key}` must be a quoted string"));
        };
        fields.insert(key.to_string(), value);
    }
    Ok(fields)
}

fn render_lockfile(graph: &DependencyGraph) -> String {
    let mut out = String::from("# This file is generated by `nomo deps resolve`.\n");
    for dependency in &flatten_dependencies(&graph.dependencies) {
        render_lock_package(&mut out, dependency);
    }
    out
}

fn dependency_graph_from_lockfile(root: &Path) -> Result<DependencyGraph, String> {
    let manifest = parse_manifest_at_root(root)?;
    let lock_path = root.join("nomo.lock");
    let text = fs::read_to_string(&lock_path).map_err(|err| err.to_string())?;
    let packages = parse_lockfile_text(&text)?;
    let referenced_packages = packages
        .iter()
        .flat_map(|package| {
            package
                .dependencies
                .iter()
                .map(|dependency| dependency.package.clone())
        })
        .collect::<BTreeSet<_>>();
    let dependencies = packages
        .iter()
        .filter(|package| !referenced_packages.contains(&package.package))
        .map(|package| build_locked_dependency(package, &packages, &mut Vec::new()))
        .collect::<Result<Vec<_>, _>>()?;
    verify_locked_source_checksums(root, &dependencies)?;
    Ok(DependencyGraph {
        root: manifest.package,
        dependencies,
    })
}

fn verify_locked_source_checksums(
    base_root: &Path,
    dependencies: &[ResolvedDependency],
) -> Result<(), String> {
    for dependency in dependencies {
        let Some(dep_root) = locked_source_root(base_root, dependency)? else {
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
        DependencySource::Registry { .. } => return Ok(None),
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

fn locked_git_root(
    base_root: &Path,
    dependency: &ResolvedDependency,
    git: &str,
) -> Result<Option<PathBuf>, String> {
    let cache_root = base_root.join(".nomo/deps/git");
    if !cache_root.is_dir() {
        return Ok(None);
    }
    let prefix = format!("{}-", dependency.alias);
    for entry in fs::read_dir(&cache_root).map_err(|err| err.to_string())? {
        let entry = entry.map_err(|err| err.to_string())?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if !name.starts_with(&prefix) {
            continue;
        }
        let Ok(manifest) = parse_manifest_at_root(&path) else {
            continue;
        };
        let actual_id = format!("{}/{}", manifest.package.namespace, manifest.package.name);
        if actual_id != dependency.package {
            continue;
        }
        if git_remote_url(&path).as_deref() != Some(git) {
            continue;
        }
        return fs::canonicalize(&path)
            .map(Some)
            .map_err(|err| err.to_string());
    }
    Ok(None)
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

fn parse_lockfile_text(lockfile: &str) -> Result<Vec<ResolvedDependency>, String> {
    let mut packages = Vec::new();
    let mut current = None;
    for line in lockfile.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line == "[[package]]" {
            if let Some(package) = current.take() {
                packages.push(finish_locked_package(package)?);
            }
            current = Some(LockedPackageFields::default());
            continue;
        }

        let fields = current
            .as_mut()
            .ok_or_else(|| format!("lockfile field outside [[package]]: `{line}`"))?;
        let Some((key, value)) = split_key_value(line) else {
            return Err(format!("invalid lockfile entry `{line}`"));
        };
        match key {
            "id" => fields.id = parse_quoted_value(value),
            "alias" => fields.alias = parse_quoted_value(value),
            "version" => fields.version = parse_quoted_value(value),
            "source" => fields.source = parse_quoted_value(value),
            "branch" => fields.branch = parse_quoted_value(value),
            "tag" => fields.tag = parse_quoted_value(value),
            "rev" => fields.rev = parse_quoted_value(value),
            "checksum" => fields.checksum = parse_quoted_value(value),
            "dependencies" => fields.dependencies = parse_lock_dependencies(value)?,
            _ => {}
        }
    }
    if let Some(package) = current {
        packages.push(finish_locked_package(package)?);
    }
    Ok(packages)
}

#[derive(Debug, Default)]
struct LockedPackageFields {
    id: Option<String>,
    alias: Option<String>,
    version: Option<String>,
    source: Option<String>,
    branch: Option<String>,
    tag: Option<String>,
    rev: Option<String>,
    checksum: Option<String>,
    dependencies: Vec<DependencyEdge>,
}

#[derive(Debug, Clone)]
struct DependencyEdge {
    alias: String,
    package: String,
}

fn finish_locked_package(fields: LockedPackageFields) -> Result<ResolvedDependency, String> {
    let package = fields
        .id
        .ok_or_else(|| "lockfile package is missing `id`".to_string())?;
    validate_package_id(&package)?;
    let alias = fields
        .alias
        .ok_or_else(|| format!("lockfile package `{package}` is missing `alias`"))?;
    validate_dependency_alias(&alias)?;
    let source = parse_lock_source(
        &package,
        fields.source.as_deref(),
        fields.version,
        fields.branch,
        fields.tag,
        fields.rev,
    )?;
    if let Some(checksum) = fields.checksum.as_deref() {
        validate_checksum(&package, checksum)?;
    }
    let dependencies = fields
        .dependencies
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
        alias,
        package,
        source,
        checksum: fields.checksum,
        dependencies,
    })
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
    source: Option<&str>,
    version: Option<String>,
    branch: Option<String>,
    tag: Option<String>,
    rev: Option<String>,
) -> Result<DependencySource, String> {
    let source =
        source.ok_or_else(|| format!("lockfile package `{package}` is missing `source`"))?;
    if let Some(registry_source) = source.strip_prefix("registry+") {
        let version =
            version.ok_or_else(|| format!("registry package `{package}` is missing `version`"))?;
        validate_version_like(&format!("lockfile package `{package}` version"), &version)?;
        let registry = if registry_source == package {
            None
        } else {
            Some(registry_source.to_string())
        };
        Ok(DependencySource::Registry { version, registry })
    } else if let Some(path) = source.strip_prefix("path+") {
        Ok(DependencySource::Path {
            path: path.to_string(),
        })
    } else if let Some(git) = source.strip_prefix("git+") {
        Ok(DependencySource::Git {
            git: git.to_string(),
            branch,
            tag,
            rev,
        })
    } else {
        Err(format!(
            "lockfile package `{package}` has unsupported source `{source}`"
        ))
    }
}

fn parse_lock_dependencies(value: &str) -> Result<Vec<DependencyEdge>, String> {
    let value = value.trim();
    let Some(value) = value.strip_prefix('[').and_then(|v| v.strip_suffix(']')) else {
        return Err(format!(
            "lockfile dependencies must be an array, found `{value}`"
        ));
    };
    if value.trim().is_empty() {
        return Ok(Vec::new());
    }

    let mut dependencies = Vec::new();
    for entry in value.split(',') {
        let Some(entry) = parse_quoted_value(entry) else {
            return Err(format!(
                "lockfile dependency entry must be quoted, found `{}`",
                entry.trim()
            ));
        };
        let Some((alias, package)) = entry.split_once(" -> ") else {
            return Err(format!(
                "lockfile dependency entry `{entry}` must use `alias -> owner/package`"
            ));
        };
        validate_dependency_alias(alias)?;
        validate_package_id(package)?;
        dependencies.push(DependencyEdge {
            alias: alias.to_string(),
            package: package.to_string(),
        });
    }
    Ok(dependencies)
}

fn render_lock_package(out: &mut String, dependency: &ResolvedDependency) {
    out.push_str("\n[[package]]\n");
    out.push_str(&format!("id = \"{}\"\n", dependency.package));
    out.push_str(&format!("alias = \"{}\"\n", dependency.alias));
    match &dependency.source {
        DependencySource::Registry { version, registry } => {
            out.push_str(&format!("version = \"{version}\"\n"));
            let registry_source = registry.as_deref().unwrap_or(&dependency.package);
            out.push_str(&format!("source = \"registry+{registry_source}\"\n"));
        }
        DependencySource::Path { path } => {
            out.push_str(&format!("source = \"path+{path}\"\n"));
        }
        DependencySource::Git {
            git,
            branch,
            tag,
            rev,
        } => {
            out.push_str(&format!("source = \"git+{git}\"\n"));
            if let Some(branch) = branch {
                out.push_str(&format!("branch = \"{branch}\"\n"));
            }
            if let Some(tag) = tag {
                out.push_str(&format!("tag = \"{tag}\"\n"));
            }
            if let Some(rev) = rev {
                out.push_str(&format!("rev = \"{rev}\"\n"));
            }
        }
    }
    if let Some(checksum) = &dependency.checksum {
        out.push_str(&format!("checksum = \"{checksum}\"\n"));
    }
    if !dependency.dependencies.is_empty() {
        let entries = dependency
            .dependencies
            .iter()
            .map(|child| format!("\"{} -> {}\"", child.alias, child.package))
            .collect::<Vec<_>>()
            .join(", ");
        out.push_str(&format!("dependencies = [{entries}]\n"));
    }
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
        assert_eq!(parsed.dependencies.len(), 2);
        assert_eq!(parsed.dependencies[1].alias, "utils");
        assert_eq!(parsed.dependencies[1].package, "fynn/utils");
    }

    #[test]
    fn parses_legacy_std_dependency() {
        let manifest =
            "[package]\nname = \"demo\"\nversion = \"0.1.0\"\n\n[dependencies]\nstd = \"0.1.0\"\n";
        let parsed = parse_manifest_text(manifest, Path::new("demo")).unwrap();

        assert_eq!(parsed.package.namespace, "local");
        assert_eq!(parsed.package.name, "demo");
        assert_eq!(parsed.dependencies[0].alias, "std");
        assert_eq!(parsed.dependencies[0].package, "nomo-lang/std");
    }

    #[test]
    fn rejects_dependency_alias_that_overrides_std() {
        let manifest = "[package]\nname = \"demo\"\n\n[dependencies]\nstd = { package = \"other/std\", version = \"0.1.0\" }\n";
        let err = parse_manifest_text(manifest, Path::new("demo")).unwrap_err();

        assert!(err.contains("alias `std` is reserved"), "{err}");
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

    fn temp_test_root(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("nomo-project-test-{name}-{}", std::process::id()))
    }
}
