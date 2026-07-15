use std::collections::BTreeMap;
use std::fs;
use std::path::{Component, Path, PathBuf};

mod version;

pub use version::{PackageVersion, VersionConstraint};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DependencyAddSpec {
    pub alias: String,
    pub package: String,
    pub version: String,
    pub registry: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FfiLinkMetadata {
    pub libraries: Vec<String>,
    pub library_paths: Vec<PathBuf>,
    pub sources: Vec<PathBuf>,
    pub frameworks: Vec<String>,
    pub link_args: Vec<String>,
}

impl FfiLinkMetadata {
    pub fn extend(&mut self, other: FfiLinkMetadata) {
        self.libraries.extend(other.libraries);
        self.library_paths.extend(other.library_paths);
        self.sources.extend(other.sources);
        self.frameworks.extend(other.frameworks);
        self.link_args.extend(other.link_args);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    pub package: PackageMetadata,
    pub dependencies: Vec<Dependency>,
    pub ffi: FfiLinkMetadata,
    pub trust: RegistryTrustPolicy,
    pub transparency_keys: Vec<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum RegistryTrustPolicy {
    #[default]
    ChecksumOnly,
    Signed,
    SignedTransparent,
}

impl RegistryTrustPolicy {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "checksum-only" => Ok(Self::ChecksumOnly),
            "signed" => Ok(Self::Signed),
            "signed+transparent" => Ok(Self::SignedTransparent),
            other => Err(format!(
                "unknown registry trust policy `{other}`; expected checksum-only, signed, or signed+transparent"
            )),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::ChecksumOnly => "checksum-only",
            Self::Signed => "signed",
            Self::SignedTransparent => "signed+transparent",
        }
    }
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

pub fn parse_manifest_at_root(root: &Path) -> Result<Manifest, String> {
    let manifest_path = root.join("nomo.toml");
    let text = fs::read_to_string(&manifest_path).map_err(|err| err.to_string())?;
    let document = parse_manifest_document(&text)?;
    let workspace = workspace_context_for_manifest(root, &document)?;
    parse_manifest_document_at_root(&document, root, workspace.as_ref())
}

pub fn parse_manifest_text(manifest: &str, root: &Path) -> Result<Manifest, String> {
    let document = parse_manifest_document(manifest)?;
    parse_manifest_document_at_root(&document, root, None)
}

pub fn parse_manifest_document(manifest: &str) -> Result<toml::Value, String> {
    manifest
        .parse::<toml::Value>()
        .map_err(|err| format!("failed to parse nomo.toml as TOML: {err}"))
}

pub fn manifest_document_has_workspace(document: &toml::Value) -> Result<bool, String> {
    Ok(optional_table(document, "workspace")?.is_some())
}

pub fn parse_workspace_context(
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

pub fn workspace_root_for_package(root: &Path) -> Result<Option<PathBuf>, String> {
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

pub fn upsert_registry_dependency(
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

pub fn remove_dependency_from_manifest(
    document: &mut toml::Value,
    alias: &str,
) -> Result<(), String> {
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

pub fn render_manifest_document(document: &toml::Value) -> Result<String, String> {
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
    let ffi = parse_ffi_link_metadata(root, optional_table(document, "ffi")?)?;
    let (trust, transparency_keys) =
        parse_registry_trust_policy(optional_table(document, "trust")?)?;

    let package_table = optional_table(document, "package")?;
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

    if let Some(table) = optional_table(document, "dependencies")? {
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
        ffi,
        trust,
        transparency_keys,
    })
}

fn parse_registry_trust_policy(
    table: Option<&toml::map::Map<String, toml::Value>>,
) -> Result<(RegistryTrustPolicy, Vec<String>), String> {
    let Some(table) = table else {
        return Ok((RegistryTrustPolicy::default(), Vec::new()));
    };
    let policy = optional_string_field(table, "trust", "policy")?
        .unwrap_or_else(|| "checksum-only".to_string());
    let policy = RegistryTrustPolicy::parse(&policy)?;
    let transparency_keys = optional_string_array_field(table, "trust", "transparency-keys")?;
    for key in &transparency_keys {
        if key.len() != 64 || !key.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(
                "manifest `trust.transparency-keys` entries must be 32-byte hexadecimal Ed25519 public keys"
                    .to_string(),
            );
        }
    }
    if policy == RegistryTrustPolicy::SignedTransparent && transparency_keys.is_empty() {
        return Err(
            "manifest `trust.policy = \"signed+transparent\"` requires at least one `transparency-keys` entry"
                .to_string(),
        );
    }
    Ok((policy, transparency_keys))
}

fn parse_ffi_link_metadata(
    root: &Path,
    table: Option<&toml::map::Map<String, toml::Value>>,
) -> Result<FfiLinkMetadata, String> {
    let Some(table) = table else {
        return Ok(FfiLinkMetadata::default());
    };
    let libraries = optional_string_array_field(table, "ffi", "libraries")?;
    let raw_library_paths = optional_string_array_field(table, "ffi", "library_paths")?;
    validate_non_empty_ffi_entries("ffi.library_paths", &raw_library_paths)?;
    let library_paths = raw_library_paths
        .into_iter()
        .map(|path| rebase_ffi_library_path(root, &path))
        .collect();
    let raw_sources = optional_string_array_field(table, "ffi", "sources")?;
    validate_non_empty_ffi_entries("ffi.sources", &raw_sources)?;
    let sources = raw_sources
        .into_iter()
        .map(|path| rebase_ffi_source_path(root, &path))
        .collect::<Result<Vec<_>, _>>()?;
    let frameworks = optional_string_array_field(table, "ffi", "frameworks")?;
    let link_args = optional_string_array_field(table, "ffi", "link_args")?;
    validate_non_empty_ffi_entries("ffi.libraries", &libraries)?;
    validate_non_empty_ffi_entries("ffi.frameworks", &frameworks)?;
    validate_non_empty_ffi_entries("ffi.link_args", &link_args)?;
    Ok(FfiLinkMetadata {
        libraries,
        library_paths,
        sources,
        frameworks,
        link_args,
    })
}

fn validate_non_empty_ffi_entries(section: &str, values: &[String]) -> Result<(), String> {
    if values.iter().any(|value| value.is_empty()) {
        return Err(format!("manifest `{section}` entries must not be empty"));
    }
    Ok(())
}

fn rebase_ffi_library_path(root: &Path, path: &str) -> PathBuf {
    let path = PathBuf::from(path);
    if path.is_absolute() {
        path
    } else {
        root.join(path)
    }
}

fn rebase_ffi_source_path(root: &Path, path: &str) -> Result<PathBuf, String> {
    let path = Path::new(path);
    if path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(format!(
            "manifest `ffi.sources` entry `{}` must stay inside the package root",
            path.display()
        ));
    }
    Ok(root.join(path))
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
        validate_version_constraint(&format!("dependency `{alias}` version"), &version)?;
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

pub fn relative_path(base: &Path, target: &Path) -> Option<PathBuf> {
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

pub fn validate_package_id(value: &str) -> Result<(), String> {
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

pub fn validate_package_namespace(label: &str, value: &str) -> Result<(), String> {
    validate_package_segment(label, value)?;
    if matches!(value, "std" | "nomo" | "core") {
        Err(format!(
            "{label} `{value}` is reserved; use an organization or user namespace such as `nomo-lang`"
        ))
    } else {
        Ok(())
    }
}

pub fn validate_package_segment(label: &str, value: &str) -> Result<(), String> {
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

pub fn validate_dependency_alias(value: &str) -> Result<(), String> {
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

pub fn validate_version_like(label: &str, value: &str) -> Result<(), String> {
    PackageVersion::parse(value)
        .map(|_| ())
        .map_err(|err| format!("{label} `{value}` is invalid: {err}"))
}

pub fn validate_version_constraint(label: &str, value: &str) -> Result<(), String> {
    VersionConstraint::parse(value)
        .map(|_| ())
        .map_err(|err| format!("{label} `{value}` is invalid: {err}"))
}

pub fn is_package_name(value: &str) -> bool {
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
mod trust_tests {
    use super::*;

    #[test]
    fn parses_explicit_registry_trust_policies_and_defaults_to_checksum_only() {
        let base = "[package]\nnamespace = \"app\"\nname = \"demo\"\nversion = \"1.0.0\"\nedition = \"2026\"\n";
        let default = parse_manifest_text(base, Path::new("demo")).unwrap();
        assert_eq!(default.trust, RegistryTrustPolicy::ChecksumOnly);
        assert!(default.transparency_keys.is_empty());

        for (value, expected) in [
            ("signed", RegistryTrustPolicy::Signed),
            ("signed+transparent", RegistryTrustPolicy::SignedTransparent),
        ] {
            let keys = if value == "signed+transparent" {
                format!("transparency-keys = [\"{}\"]\n", "1".repeat(64))
            } else {
                String::new()
            };
            let manifest = format!("{base}\n[trust]\npolicy = \"{value}\"\n{keys}");
            assert_eq!(
                parse_manifest_text(&manifest, Path::new("demo"))
                    .unwrap()
                    .trust,
                expected
            );
        }
        let invalid = format!("{base}\n[trust]\npolicy = \"trust-me\"\n");
        assert!(
            parse_manifest_text(&invalid, Path::new("demo"))
                .unwrap_err()
                .contains("unknown registry trust policy")
        );
    }
}
