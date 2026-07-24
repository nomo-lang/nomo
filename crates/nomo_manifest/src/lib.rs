use nomo_target::TargetTriple;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::path::{Component, Path, PathBuf};

mod version;

pub use version::{PackageVersion, VersionConstraint};

pub const MANIFEST_VERSION_V2: i64 = 2;
pub const PROJECT_CONFIG_VERSION: i64 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManifestSchema {
    V1,
    V2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManifestDocumentKind {
    Package,
    Workspace,
    Combined,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PackageDetails {
    pub description: Option<String>,
    pub license: Option<String>,
    pub repository: Option<String>,
    pub publish: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectConfig {
    pub version: i64,
    pub trust: RegistryTrustPolicy,
    pub transparency: TransparencyTrustConfig,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestMigration {
    pub manifest: String,
    pub project_config: Option<String>,
    pub changed: bool,
}

impl Default for ProjectConfig {
    fn default() -> Self {
        Self {
            version: PROJECT_CONFIG_VERSION,
            trust: RegistryTrustPolicy::default(),
            transparency: TransparencyTrustConfig::default(),
        }
    }
}

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

    pub fn is_empty(&self) -> bool {
        self.libraries.is_empty()
            && self.library_paths.is_empty()
            && self.sources.is_empty()
            && self.frameworks.is_empty()
            && self.link_args.is_empty()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TargetCondition {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    arch: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    os: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    env: Vec<String>,
}

impl TargetCondition {
    pub fn is_unconditional(&self) -> bool {
        self.arch.is_empty() && self.os.is_empty() && self.env.is_empty()
    }

    pub fn matches(&self, target: &TargetTriple) -> bool {
        target_component_matches(&self.arch, target.architecture().as_str())
            && target_component_matches(&self.os, target.operating_system().as_str())
            && target_component_matches(&self.env, target.environment().as_str())
    }

    pub fn intersection(&self, other: &Self) -> Option<Self> {
        let condition = Self {
            arch: intersect_target_values(&self.arch, &other.arch)?,
            os: intersect_target_values(&self.os, &other.os)?,
            env: intersect_target_values(&self.env, &other.env)?,
        };
        condition.is_satisfiable().then_some(condition)
    }

    pub fn architectures(&self) -> &[String] {
        &self.arch
    }

    pub fn operating_systems(&self) -> &[String] {
        &self.os
    }

    pub fn environments(&self) -> &[String] {
        &self.env
    }

    pub fn validate_canonical(&self) -> Result<(), String> {
        validate_canonical_target_values("arch", &self.arch, &["aarch64", "x86_64"])?;
        validate_canonical_target_values("os", &self.os, &["darwin", "linux", "windows"])?;
        validate_canonical_target_values("env", &self.env, &["gnu", "msvc", "none"])?;
        if !self.is_unconditional() && !self.is_satisfiable() {
            return Err(format!(
                "target condition `{self}` does not match any supported target"
            ));
        }
        Ok(())
    }

    fn is_satisfiable(&self) -> bool {
        TargetTriple::supported()
            .iter()
            .any(|target| self.matches(target))
    }
}

impl fmt::Display for TargetCondition {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut predicates = Vec::new();
        push_target_predicate(&mut predicates, "arch", &self.arch);
        push_target_predicate(&mut predicates, "os", &self.os);
        push_target_predicate(&mut predicates, "env", &self.env);
        if predicates.is_empty() {
            formatter.write_str("all targets")
        } else {
            formatter.write_str(&predicates.join(" and "))
        }
    }
}

fn target_component_matches(values: &[String], actual: &str) -> bool {
    values.is_empty() || values.iter().any(|value| value == actual)
}

fn validate_canonical_target_values(
    field: &str,
    values: &[String],
    allowed: &[&str],
) -> Result<(), String> {
    if let Some(value) = values
        .iter()
        .find(|value| !allowed.contains(&value.as_str()))
    {
        return Err(format!(
            "target condition field `{field}` has non-canonical value `{value}`"
        ));
    }
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(format!(
            "target condition field `{field}` values must be sorted and unique"
        ));
    }
    Ok(())
}

fn intersect_target_values(left: &[String], right: &[String]) -> Option<Vec<String>> {
    if left.is_empty() {
        return Some(right.to_vec());
    }
    if right.is_empty() {
        return Some(left.to_vec());
    }
    let values = left
        .iter()
        .filter(|value| right.contains(value))
        .cloned()
        .collect::<Vec<_>>();
    (!values.is_empty()).then_some(values)
}

fn push_target_predicate(out: &mut Vec<String>, field: &str, values: &[String]) {
    match values {
        [] => {}
        [value] => out.push(format!("target.{field} = {value}")),
        _ => out.push(format!("target.{field} in [{}]", values.join(", "))),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConditionalFfiLinkMetadata {
    pub condition: TargetCondition,
    pub metadata: FfiLinkMetadata,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    pub schema: ManifestSchema,
    pub kind: ManifestDocumentKind,
    pub package: PackageMetadata,
    pub details: PackageDetails,
    pub dependencies: Vec<Dependency>,
    pub ffi: FfiLinkMetadata,
    pub target_ffi: Vec<ConditionalFfiLinkMetadata>,
    pub trust: RegistryTrustPolicy,
    pub transparency: TransparencyTrustConfig,
}

impl Manifest {
    pub fn ffi_for_target(&self, target: &TargetTriple) -> FfiLinkMetadata {
        let mut metadata = self.ffi.clone();
        for conditional in &self.target_ffi {
            if conditional.condition.matches(target) {
                metadata.extend(conditional.metadata.clone());
            }
        }
        metadata
    }
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

pub const DEFAULT_TRANSPARENCY_PROOF_MAX_AGE_SECONDS: u64 = 24 * 60 * 60;
pub const DEFAULT_TRANSPARENCY_OFFLINE_PROOF_MAX_AGE_SECONDS: u64 = 7 * 24 * 60 * 60;
pub const DEFAULT_TRANSPARENCY_MAX_FUTURE_SKEW_SECONDS: u64 = 5 * 60;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransparencyTrustConfig {
    pub keys: Vec<String>,
    pub proof_max_age_seconds: u64,
    pub offline_proof_max_age_seconds: u64,
    pub max_future_skew_seconds: u64,
    pub gossip_checkpoints: Vec<PathBuf>,
}

impl Default for TransparencyTrustConfig {
    fn default() -> Self {
        Self {
            keys: Vec::new(),
            proof_max_age_seconds: DEFAULT_TRANSPARENCY_PROOF_MAX_AGE_SECONDS,
            offline_proof_max_age_seconds: DEFAULT_TRANSPARENCY_OFFLINE_PROOF_MAX_AGE_SECONDS,
            max_future_skew_seconds: DEFAULT_TRANSPARENCY_MAX_FUTURE_SKEW_SECONDS,
            gossip_checkpoints: Vec::new(),
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

/// Returns the stable Nomo module root derived from a package name.
///
/// Package names are normally lowercase kebab-case, but this also supports
/// legacy CamelCase names so migration can be deterministic.
pub fn package_name_to_module_root(name: &str) -> Result<String, String> {
    let mut root = String::new();
    let mut previous_was_lower_or_digit = false;
    for ch in name.chars() {
        if ch.is_ascii_uppercase() {
            if previous_was_lower_or_digit && !root.ends_with('_') {
                root.push('_');
            }
            root.push(ch.to_ascii_lowercase());
            previous_was_lower_or_digit = false;
        } else if ch.is_ascii_lowercase() || ch.is_ascii_digit() {
            root.push(ch);
            previous_was_lower_or_digit = true;
        } else if ch == '-' || ch == '_' {
            if !root.is_empty() && !root.ends_with('_') {
                root.push('_');
            }
            previous_was_lower_or_digit = false;
        } else {
            return Err(format!(
                "package name `{name}` cannot derive a Nomo module root"
            ));
        }
    }
    let root = root.trim_matches('_');
    if root.is_empty() || validate_dependency_alias(root).is_err() {
        return Err(format!(
            "package name `{name}` cannot derive a Nomo module root"
        ));
    }
    Ok(root.to_string())
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WorkspacePackageDefaults {
    pub namespace: Option<String>,
    pub name: Option<String>,
    pub version: Option<String>,
    pub edition: Option<String>,
    pub details: PackageDetails,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceContext {
    pub schema: ManifestSchema,
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
    pub target: TargetCondition,
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

pub fn manifest_schema(document: &toml::Value) -> Result<ManifestSchema, String> {
    let Some(value) = document.get("manifest-version") else {
        return Ok(ManifestSchema::V1);
    };
    let version = value
        .as_integer()
        .ok_or_else(|| "manifest `manifest-version` must be an integer".to_string())?;
    match version {
        MANIFEST_VERSION_V2 => Ok(ManifestSchema::V2),
        other => Err(format!(
            "unsupported manifest version `{other}`; this Nomo build supports version {MANIFEST_VERSION_V2}"
        )),
    }
}

pub fn manifest_document_kind(document: &toml::Value) -> Result<ManifestDocumentKind, String> {
    match (
        optional_table(document, "package")?.is_some(),
        optional_table(document, "workspace")?.is_some(),
    ) {
        (true, false) => Ok(ManifestDocumentKind::Package),
        (false, true) => Ok(ManifestDocumentKind::Workspace),
        (true, true) => Ok(ManifestDocumentKind::Combined),
        (false, false) if manifest_schema(document)? == ManifestSchema::V1 => {
            Ok(ManifestDocumentKind::Package)
        }
        (false, false) => Err(
            "manifest v2 must define a `[package]` table, a `[workspace]` table, or both"
                .to_string(),
        ),
    }
}

pub fn manifest_document_has_workspace(document: &toml::Value) -> Result<bool, String> {
    Ok(optional_table(document, "workspace")?.is_some())
}

pub fn parse_workspace_context(
    root: &Path,
    document: &toml::Value,
) -> Result<WorkspaceContext, String> {
    parse_workspace_context_impl(root, document, true)
}

fn parse_workspace_context_impl(
    root: &Path,
    document: &toml::Value,
    include_dependencies: bool,
) -> Result<WorkspaceContext, String> {
    let schema = manifest_schema(document)?;
    if schema == ManifestSchema::V2 {
        validate_v2_top_level(document)?;
    }
    let workspace_table = optional_table(document, "workspace")?
        .ok_or_else(|| "manifest does not define a [workspace] table".to_string())?;
    if schema == ManifestSchema::V2 {
        validate_supported_fields(
            workspace_table,
            "workspace",
            &[
                "members",
                "default-members",
                "exclude",
                "resolver",
                "package",
                "dependencies",
            ],
        )?;
    }
    let members = optional_string_array_field(workspace_table, "workspace", "members")?;
    let default_members =
        optional_string_array_field(workspace_table, "workspace", "default-members")?;
    let exclude = optional_string_array_field(workspace_table, "workspace", "exclude")?;
    let resolver = optional_string_field(workspace_table, "workspace", "resolver")?;
    if schema == ManifestSchema::V2 {
        if members.is_empty() {
            return Err("manifest v2 `workspace.members` must not be empty".to_string());
        }
        validate_workspace_patterns("workspace.members", &members)?;
        validate_workspace_patterns("workspace.default-members", &default_members)?;
        validate_workspace_patterns("workspace.exclude", &exclude)?;
        if optional_table(document, "package")?.is_some()
            && !members.iter().any(|member| member == ".")
        {
            return Err(
                "a manifest v2 combined workspace root must include `.` in `workspace.members`"
                    .to_string(),
            );
        }
    }
    let package = match workspace_table.get("package") {
        Some(value) => parse_workspace_package_defaults(value, schema)?,
        None => WorkspacePackageDefaults::default(),
    };
    if schema == ManifestSchema::V2 {
        if let Some(namespace) = &package.namespace {
            validate_package_namespace("workspace package namespace", namespace)?;
        }
        if let Some(version) = &package.version {
            validate_version_like("workspace package version", version)?;
        }
        if package.edition.as_deref() == Some("") {
            return Err("workspace package edition must not be empty".to_string());
        }
    }
    let dependencies = if include_dependencies {
        match workspace_table.get("dependencies") {
            Some(value) => parse_workspace_dependencies(value, schema, root)?,
            None => BTreeMap::new(),
        }
    } else {
        BTreeMap::new()
    };
    Ok(WorkspaceContext {
        schema,
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
            if manifest_schema(&document)? == ManifestSchema::V2 {
                let context = parse_workspace_context_impl(candidate, &document, false)?;
                if !workspace_context_includes_root(&context, root)? {
                    continue;
                }
            }
            return Ok(Some(candidate.to_path_buf()));
        }
    }
    Ok(None)
}

pub fn upsert_registry_dependency(
    document: &mut toml::Value,
    spec: &DependencyAddSpec,
) -> Result<(), String> {
    let schema = manifest_schema(document)?;
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
    parse_dependency_value(&spec.alias, &value, None, schema, None)?;
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

pub fn render_project_config(config: &ProjectConfig, root: &Path) -> Result<String, String> {
    let mut registry = toml::map::Map::new();
    registry.insert(
        "policy".to_string(),
        toml::Value::String(config.trust.as_str().to_string()),
    );
    if !config.transparency.keys.is_empty() {
        registry.insert(
            "transparency-keys".to_string(),
            toml::Value::Array(
                config
                    .transparency
                    .keys
                    .iter()
                    .cloned()
                    .map(toml::Value::String)
                    .collect(),
            ),
        );
    }
    registry.insert(
        "proof-max-age-seconds".to_string(),
        toml::Value::Integer(
            i64::try_from(config.transparency.proof_max_age_seconds)
                .map_err(|_| "proof-max-age-seconds is too large to render".to_string())?,
        ),
    );
    registry.insert(
        "offline-proof-max-age-seconds".to_string(),
        toml::Value::Integer(
            i64::try_from(config.transparency.offline_proof_max_age_seconds)
                .map_err(|_| "offline-proof-max-age-seconds is too large to render".to_string())?,
        ),
    );
    registry.insert(
        "max-future-skew-seconds".to_string(),
        toml::Value::Integer(
            i64::try_from(config.transparency.max_future_skew_seconds)
                .map_err(|_| "max-future-skew-seconds is too large to render".to_string())?,
        ),
    );
    if !config.transparency.gossip_checkpoints.is_empty() {
        let mut checkpoints = Vec::new();
        for checkpoint in &config.transparency.gossip_checkpoints {
            let relative = relative_path(root, checkpoint).ok_or_else(|| {
                format!(
                    "cannot express gossip checkpoint {} relative to project root {}",
                    checkpoint.display(),
                    root.display()
                )
            })?;
            if relative.components().any(|component| {
                matches!(
                    component,
                    Component::ParentDir | Component::RootDir | Component::Prefix(_)
                )
            }) {
                return Err(format!(
                    "gossip checkpoint {} is outside project root {}",
                    checkpoint.display(),
                    root.display()
                ));
            }
            checkpoints.push(toml::Value::String(
                relative.to_string_lossy().replace('\\', "/"),
            ));
        }
        registry.insert(
            "gossip-checkpoints".to_string(),
            toml::Value::Array(checkpoints),
        );
    }

    let mut document = toml::map::Map::new();
    document.insert(
        "config-version".to_string(),
        toml::Value::Integer(PROJECT_CONFIG_VERSION),
    );
    document.insert("registry".to_string(), toml::Value::Table(registry));
    let document = toml::Value::Table(document);
    parse_project_config_document(root, &document, &root.join(".nomo/config.toml"))?;
    render_manifest_document(&document)
}

pub fn migrate_manifest_at_root(root: &Path) -> Result<ManifestMigration, String> {
    let manifest_path = root.join("nomo.toml");
    let original = fs::read_to_string(&manifest_path)
        .map_err(|err| format!("failed to read {}: {err}", manifest_path.display()))?;
    let document = parse_manifest_document(&original)?;
    let schema = manifest_schema(&document)?;
    let kind = manifest_document_kind(&document)?;

    if schema == ManifestSchema::V2 {
        if kind == ManifestDocumentKind::Workspace {
            parse_workspace_context(root, &document)?;
        } else {
            let workspace = workspace_context_for_manifest(root, &document)?;
            parse_manifest_document_at_root(&document, root, workspace.as_ref())?;
        }
        return Ok(ManifestMigration {
            manifest: original,
            project_config: None,
            changed: false,
        });
    }

    let workspace = workspace_context_for_manifest(root, &document)?;
    let parsed = if kind == ManifestDocumentKind::Workspace {
        None
    } else {
        Some(parse_manifest_document_at_root(
            &document,
            root,
            workspace.as_ref(),
        )?)
    };
    let mut migrated = document.clone();
    let root_table = migrated
        .as_table_mut()
        .ok_or_else(|| "manifest root must be a TOML table".to_string())?;
    root_table.insert(
        "manifest-version".to_string(),
        toml::Value::Integer(MANIFEST_VERSION_V2),
    );

    let trust = root_table.remove("trust");
    let project_config = trust
        .map(|trust| migrate_trust_to_project_config(root, trust))
        .transpose()?;

    if let Some(parsed) = &parsed {
        migrate_package_table(root_table, &document, parsed)?;
    }
    if let Some(workspace) = root_table
        .get_mut("workspace")
        .and_then(toml::Value::as_table_mut)
    {
        migrate_workspace_table(workspace, kind)?;
    }

    let migrated_workspace =
        if kind == ManifestDocumentKind::Workspace || kind == ManifestDocumentKind::Combined {
            Some(parse_workspace_context(root, &migrated)?)
        } else {
            workspace
        };
    if kind != ManifestDocumentKind::Workspace {
        parse_manifest_document_at_root(&migrated, root, migrated_workspace.as_ref())?;
    }

    Ok(ManifestMigration {
        manifest: render_manifest_document(&migrated)?,
        project_config,
        changed: true,
    })
}

fn migrate_package_table(
    root: &mut toml::map::Map<String, toml::Value>,
    original: &toml::Value,
    parsed: &Manifest,
) -> Result<(), String> {
    let original_package = optional_table(original, "package")?;
    let inherited = ["namespace", "version", "edition"]
        .into_iter()
        .filter(|key| {
            original_package
                .and_then(|table| table.get(*key))
                .is_some_and(is_workspace_inheritance)
        })
        .collect::<Vec<_>>();
    let legacy_fields = if original_package.is_none() {
        [
            "namespace",
            "name",
            "version",
            "edition",
            "description",
            "license",
            "repository",
            "publish",
        ]
        .into_iter()
        .filter_map(|key| root.remove(key).map(|value| (key.to_string(), value)))
        .collect::<Vec<_>>()
    } else {
        Vec::new()
    };

    let package = root
        .entry("package".to_string())
        .or_insert_with(|| toml::Value::Table(toml::map::Map::new()))
        .as_table_mut()
        .ok_or_else(|| "manifest `package` must be a TOML table".to_string())?;

    for (key, value) in legacy_fields {
        package.insert(key, value);
    }

    package.insert(
        "name".to_string(),
        toml::Value::String(parsed.package.name.clone()),
    );
    for (key, value) in [
        ("namespace", &parsed.package.namespace),
        ("version", &parsed.package.version),
        ("edition", &parsed.package.edition),
    ] {
        if inherited.contains(&key) {
            package.remove(key);
        } else if !package.contains_key(key) {
            package.insert(key.to_string(), toml::Value::String(value.clone()));
        }
    }
    if inherited.is_empty() {
        package.remove("inherit");
    } else {
        package.insert(
            "inherit".to_string(),
            toml::Value::String("workspace".to_string()),
        );
    }
    Ok(())
}

fn migrate_workspace_table(
    workspace: &mut toml::map::Map<String, toml::Value>,
    kind: ManifestDocumentKind,
) -> Result<(), String> {
    if let Some(package) = workspace
        .get_mut("package")
        .and_then(toml::Value::as_table_mut)
    {
        package.remove("name");
    }
    if kind == ManifestDocumentKind::Combined {
        let members = workspace
            .entry("members".to_string())
            .or_insert_with(|| toml::Value::Array(Vec::new()))
            .as_array_mut()
            .ok_or_else(|| "manifest `workspace.members` must be an array".to_string())?;
        if !members.iter().any(|member| member.as_str() == Some(".")) {
            members.insert(0, toml::Value::String(".".to_string()));
        }
    }
    Ok(())
}

fn migrate_trust_to_project_config(root: &Path, trust: toml::Value) -> Result<String, String> {
    let trust_table = trust
        .as_table()
        .ok_or_else(|| "manifest `trust` must be a TOML table".to_string())?;
    parse_registry_trust_policy_table(root, Some(trust_table), "trust")?;
    let mut document = toml::map::Map::new();
    document.insert(
        "config-version".to_string(),
        toml::Value::Integer(PROJECT_CONFIG_VERSION),
    );
    document.insert("registry".to_string(), trust);
    let document = toml::Value::Table(document);
    parse_project_config_document(root, &document, &root.join(".nomo/config.toml"))?;
    render_manifest_document(&document)
}

pub fn parse_project_config_at_root(root: &Path) -> Result<ProjectConfig, String> {
    let path = root.join(".nomo/config.toml");
    if !path.is_file() {
        return Ok(ProjectConfig::default());
    }
    let text = fs::read_to_string(&path)
        .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
    let document = text
        .parse::<toml::Value>()
        .map_err(|err| format!("failed to parse {} as TOML: {err}", path.display()))?;
    parse_project_config_document(root, &document, &path)
}

pub fn parse_project_config_text(text: &str, root: &Path) -> Result<ProjectConfig, String> {
    let path = root.join(".nomo/config.toml");
    let document = text
        .parse::<toml::Value>()
        .map_err(|err| format!("failed to parse {} as TOML: {err}", path.display()))?;
    parse_project_config_document(root, &document, &path)
}

pub fn parse_manifest_document_with_workspace(
    document: &toml::Value,
    root: &Path,
    workspace: Option<&WorkspaceContext>,
) -> Result<Manifest, String> {
    parse_manifest_document_at_root(document, root, workspace)
}

fn parse_project_config_document(
    root: &Path,
    document: &toml::Value,
    path: &Path,
) -> Result<ProjectConfig, String> {
    let table = document
        .as_table()
        .ok_or_else(|| format!("project config {} must be a TOML table", path.display()))?;
    validate_supported_fields(table, "project config", &["config-version", "registry"])?;
    let version = table
        .get("config-version")
        .and_then(toml::Value::as_integer)
        .ok_or_else(|| {
            format!(
                "project config {} must define integer `config-version = {PROJECT_CONFIG_VERSION}`",
                path.display()
            )
        })?;
    if version != PROJECT_CONFIG_VERSION {
        return Err(format!(
            "unsupported project config version `{version}` in {}; this Nomo build supports version {PROJECT_CONFIG_VERSION}",
            path.display()
        ));
    }
    let registry = match table.get("registry") {
        Some(value) => Some(value.as_table().ok_or_else(|| {
            format!(
                "project config {} field `registry` must be a table",
                path.display()
            )
        })?),
        None => None,
    };
    let (trust, transparency) =
        parse_registry_trust_policy_table(root, registry, "registry config")?;
    Ok(ProjectConfig {
        version,
        trust,
        transparency,
    })
}

fn parse_manifest_document_at_root(
    document: &toml::Value,
    root: &Path,
    workspace: Option<&WorkspaceContext>,
) -> Result<Manifest, String> {
    let schema = manifest_schema(document)?;
    let kind = manifest_document_kind(document)?;
    if schema == ManifestSchema::V2 {
        validate_v2_top_level(document)?;
    }
    if kind == ManifestDocumentKind::Workspace {
        return Err(format!(
            "{} is a workspace manifest and does not define a package",
            root.join("nomo.toml").display()
        ));
    }

    let (package, details, namespace_explicit) =
        parse_package_header(document, root, workspace, schema)?;
    let mut dependencies = Vec::new();
    let ffi_table = optional_table(document, "ffi")?;
    let ffi = parse_ffi_link_metadata(root, ffi_table)?;
    let target_ffi = parse_target_ffi_link_metadata(root, ffi_table)?;
    let (trust, transparency) = if schema == ManifestSchema::V2 {
        let config_root = workspace
            .map(|workspace| workspace.root.as_path())
            .unwrap_or(root);
        let config = parse_project_config_at_root(config_root)?;
        (config.trust, config.transparency)
    } else {
        parse_registry_trust_policy(root, optional_table(document, "trust")?)?
    };

    if let Some(table) = optional_table(document, "dependencies")? {
        let inheritance = workspace.map(|workspace| WorkspaceDependencyInheritance {
            workspace,
            package_root: root,
        });
        for (alias, value) in table {
            if let Some(dependency) =
                parse_dependency_value(alias, value, inheritance.as_ref(), schema, Some(root))?
            {
                dependencies.push(dependency);
            }
        }
    }

    validate_package_namespace("package namespace", &package.namespace)?;
    if schema == ManifestSchema::V2 || namespace_explicit {
        validate_package_segment("package name", &package.name)?;
    } else if !is_legacy_package_name(&package.name) {
        return Err(format!("invalid legacy package name `{}`", package.name));
    }
    validate_version_like("package version", &package.version)?;
    if package.edition.is_empty() {
        return Err("package edition must not be empty".to_string());
    }

    Ok(Manifest {
        schema,
        kind,
        package,
        details,
        dependencies,
        ffi,
        target_ffi,
        trust,
        transparency,
    })
}

fn parse_package_header(
    document: &toml::Value,
    root: &Path,
    workspace: Option<&WorkspaceContext>,
    schema: ManifestSchema,
) -> Result<(PackageMetadata, PackageDetails, bool), String> {
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
    let package_table = match optional_table(document, "package")? {
        Some(table) => table,
        None if schema == ManifestSchema::V1 => document
            .as_table()
            .ok_or_else(|| "manifest root must be a TOML table".to_string())?,
        None => return Err("manifest v2 does not define a `[package]` table".to_string()),
    };
    let namespace_explicit = package_table.contains_key("namespace");
    let mut details = parse_package_details(package_table)?;
    if schema == ManifestSchema::V2 {
        validate_supported_fields(
            package_table,
            "package",
            &[
                "namespace",
                "name",
                "version",
                "edition",
                "inherit",
                "description",
                "license",
                "repository",
                "publish",
            ],
        )?;
        let inherit = match optional_string_field(package_table, "package", "inherit")? {
            Some(value) if value == "workspace" => true,
            Some(value) => {
                return Err(format!(
                    "manifest `package.inherit` must be `workspace`, found `{value}`"
                ));
            }
            None => false,
        };
        if inherit && workspace.is_none() {
            return Err(
                "manifest `package.inherit = \"workspace\"` is only valid for a verified workspace member"
                    .to_string(),
            );
        }
        package.name = required_package_string(package_table, "name")?;
        package.namespace = v2_package_string(package_table, workspace, inherit, "namespace")?;
        package.version = v2_package_string(package_table, workspace, inherit, "version")?;
        package.edition = v2_package_string(package_table, workspace, inherit, "edition")?;
        if inherit {
            inherit_package_details(&mut details, workspace.expect("inherit requires workspace"));
        }
    } else {
        if let Some(value) = optional_package_string_field(package_table, "namespace", workspace)? {
            package.namespace = value;
        }
        if let Some(value) = optional_package_string_field(package_table, "name", workspace)? {
            package.name = value;
        }
        if let Some(value) = optional_package_string_field(package_table, "version", workspace)? {
            package.version = value;
        }
        if let Some(value) = optional_package_string_field(package_table, "edition", workspace)? {
            package.edition = value;
        }
    }
    Ok((package, details, namespace_explicit))
}

fn parse_registry_trust_policy(
    root: &Path,
    table: Option<&toml::map::Map<String, toml::Value>>,
) -> Result<(RegistryTrustPolicy, TransparencyTrustConfig), String> {
    parse_registry_trust_policy_table(root, table, "trust")
}

fn parse_registry_trust_policy_table(
    root: &Path,
    table: Option<&toml::map::Map<String, toml::Value>>,
    section: &str,
) -> Result<(RegistryTrustPolicy, TransparencyTrustConfig), String> {
    let Some(table) = table else {
        return Ok((
            RegistryTrustPolicy::default(),
            TransparencyTrustConfig::default(),
        ));
    };
    if let Some(field) = table.keys().find(|field| {
        !matches!(
            field.as_str(),
            "policy"
                | "transparency-keys"
                | "proof-max-age-seconds"
                | "offline-proof-max-age-seconds"
                | "max-future-skew-seconds"
                | "gossip-checkpoints"
        )
    }) {
        return Err(format!("`{section}` contains unsupported field `{field}`"));
    }
    let policy = optional_string_field(table, section, "policy")?
        .unwrap_or_else(|| "checksum-only".to_string());
    let policy = RegistryTrustPolicy::parse(&policy)?;
    let mut transparency = TransparencyTrustConfig {
        keys: optional_string_array_field(table, section, "transparency-keys")?,
        ..TransparencyTrustConfig::default()
    };
    for key in &mut transparency.keys {
        if key.len() != 64 || !key.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(format!(
                "`{section}.transparency-keys` entries must be 32-byte hexadecimal Ed25519 public keys"
            ));
        }
        *key = key.to_ascii_lowercase();
    }
    transparency.keys.sort();
    transparency.keys.dedup();
    transparency.proof_max_age_seconds =
        optional_positive_u64_field(table, section, "proof-max-age-seconds")?
            .unwrap_or(DEFAULT_TRANSPARENCY_PROOF_MAX_AGE_SECONDS);
    transparency.offline_proof_max_age_seconds =
        optional_positive_u64_field(table, section, "offline-proof-max-age-seconds")?
            .unwrap_or(DEFAULT_TRANSPARENCY_OFFLINE_PROOF_MAX_AGE_SECONDS);
    transparency.max_future_skew_seconds =
        optional_u64_field(table, section, "max-future-skew-seconds")?
            .unwrap_or(DEFAULT_TRANSPARENCY_MAX_FUTURE_SKEW_SECONDS);
    if transparency.offline_proof_max_age_seconds < transparency.proof_max_age_seconds {
        return Err(format!(
            "`{section}.offline-proof-max-age-seconds` must be at least `proof-max-age-seconds`"
        ));
    }
    transparency.gossip_checkpoints =
        optional_string_array_field(table, section, "gossip-checkpoints")?
            .into_iter()
            .map(|path| rebase_trust_path(root, &path, section))
            .collect::<Result<Vec<_>, _>>()?;
    transparency.gossip_checkpoints.sort();
    transparency.gossip_checkpoints.dedup();
    if policy == RegistryTrustPolicy::SignedTransparent && transparency.keys.is_empty() {
        return Err(format!(
            "`{section}.policy = \"signed+transparent\"` requires at least one `transparency-keys` entry"
        ));
    }
    Ok((policy, transparency))
}

fn optional_u64_field(
    table: &toml::map::Map<String, toml::Value>,
    section: &str,
    key: &str,
) -> Result<Option<u64>, String> {
    let Some(value) = table.get(key) else {
        return Ok(None);
    };
    let integer = value
        .as_integer()
        .ok_or_else(|| format!("manifest `{section}.{key}` must be a non-negative integer"))?;
    u64::try_from(integer)
        .map(Some)
        .map_err(|_| format!("manifest `{section}.{key}` must be a non-negative integer"))
}

fn optional_positive_u64_field(
    table: &toml::map::Map<String, toml::Value>,
    section: &str,
    key: &str,
) -> Result<Option<u64>, String> {
    let value = optional_u64_field(table, section, key)?;
    if value == Some(0) {
        return Err(format!("manifest `{section}.{key}` must be positive"));
    }
    Ok(value)
}

fn rebase_trust_path(root: &Path, path: &str, section: &str) -> Result<PathBuf, String> {
    let path = Path::new(path);
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(format!(
            "`{section}.gossip-checkpoints` entry `{}` must stay inside the project root",
            path.display()
        ));
    }
    Ok(root.join(path))
}

fn parse_ffi_link_metadata(
    root: &Path,
    table: Option<&toml::map::Map<String, toml::Value>>,
) -> Result<FfiLinkMetadata, String> {
    let Some(table) = table else {
        return Ok(FfiLinkMetadata::default());
    };
    parse_ffi_link_metadata_table(root, table, "ffi")
}

fn parse_target_ffi_link_metadata(
    root: &Path,
    table: Option<&toml::map::Map<String, toml::Value>>,
) -> Result<Vec<ConditionalFfiLinkMetadata>, String> {
    let Some(value) = table.and_then(|table| table.get("target")) else {
        return Ok(Vec::new());
    };
    let entries = value
        .as_array()
        .ok_or_else(|| "manifest `ffi.target` must be an array of tables".to_string())?;
    let mut conditional = Vec::with_capacity(entries.len());
    for (index, entry) in entries.iter().enumerate() {
        let section = format!("ffi.target[{index}]");
        let table = entry
            .as_table()
            .ok_or_else(|| format!("manifest `{section}` must be a table"))?;
        if let Some(field) = table.keys().find(|field| {
            !matches!(
                field.as_str(),
                "arch"
                    | "os"
                    | "env"
                    | "libraries"
                    | "library_paths"
                    | "sources"
                    | "frameworks"
                    | "link_args"
            )
        }) {
            return Err(format!(
                "manifest `{section}` contains unsupported field `{field}`"
            ));
        }
        let condition = parse_target_condition_fields(table, &section)?;
        if condition.is_unconditional() {
            return Err(format!(
                "manifest `{section}` must select at least one target arch, os, or env"
            ));
        }
        let metadata = parse_ffi_link_metadata_table(root, table, &section)?;
        if metadata.is_empty() {
            return Err(format!(
                "manifest `{section}` must define at least one FFI source, library, search path, framework, or link argument"
            ));
        }
        conditional.push(ConditionalFfiLinkMetadata {
            condition,
            metadata,
        });
    }
    Ok(conditional)
}

fn parse_ffi_link_metadata_table(
    root: &Path,
    table: &toml::map::Map<String, toml::Value>,
    section: &str,
) -> Result<FfiLinkMetadata, String> {
    let libraries = optional_string_array_field(table, section, "libraries")?;
    let raw_library_paths = optional_string_array_field(table, section, "library_paths")?;
    validate_non_empty_ffi_entries(&format!("{section}.library_paths"), &raw_library_paths)?;
    let library_paths = raw_library_paths
        .into_iter()
        .map(|path| rebase_ffi_library_path(root, &path))
        .collect();
    let raw_sources = optional_string_array_field(table, section, "sources")?;
    validate_non_empty_ffi_entries(&format!("{section}.sources"), &raw_sources)?;
    let sources = raw_sources
        .into_iter()
        .map(|path| rebase_ffi_source_path(root, &path))
        .collect::<Result<Vec<_>, _>>()?;
    let frameworks = optional_string_array_field(table, section, "frameworks")?;
    let link_args = optional_string_array_field(table, section, "link_args")?;
    validate_non_empty_ffi_entries(&format!("{section}.libraries"), &libraries)?;
    validate_non_empty_ffi_entries(&format!("{section}.frameworks"), &frameworks)?;
    validate_non_empty_ffi_entries(&format!("{section}.link_args"), &link_args)?;
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

fn workspace_context_for_package_identity(
    root: &Path,
    document: &toml::Value,
) -> Result<Option<WorkspaceContext>, String> {
    if optional_table(document, "workspace")?.is_some() {
        return parse_workspace_context_impl(root, document, false).map(Some);
    }
    let Some(workspace_root) = workspace_root_for_package(root)? else {
        return Ok(None);
    };
    let text =
        fs::read_to_string(workspace_root.join("nomo.toml")).map_err(|err| err.to_string())?;
    let document = parse_manifest_document(&text)?;
    parse_workspace_context_impl(&workspace_root, &document, false).map(Some)
}

fn parse_package_identity_only_at_root(root: &Path) -> Result<String, String> {
    let manifest_path = root.join("nomo.toml");
    let text = fs::read_to_string(&manifest_path).map_err(|err| {
        format!(
            "failed to read path dependency manifest {}: {err}",
            manifest_path.display()
        )
    })?;
    let document = parse_manifest_document(&text)?;
    let schema = manifest_schema(&document)?;
    let kind = manifest_document_kind(&document)?;
    if kind == ManifestDocumentKind::Workspace {
        return Err(format!(
            "path dependency {} is a virtual workspace, not a package",
            root.display()
        ));
    }
    if schema == ManifestSchema::V2 {
        validate_v2_top_level(&document)?;
    }
    let workspace = workspace_context_for_package_identity(root, &document)?;
    let (package, _, namespace_explicit) =
        parse_package_header(&document, root, workspace.as_ref(), schema)?;
    validate_package_namespace("path dependency package namespace", &package.namespace)?;
    if schema == ManifestSchema::V2 || namespace_explicit {
        validate_package_segment("path dependency package name", &package.name)?;
    } else if !is_legacy_package_name(&package.name) {
        return Err(format!("invalid legacy package name `{}`", package.name));
    }
    validate_version_like("path dependency package version", &package.version)?;
    if package.edition.is_empty() {
        return Err("path dependency package edition must not be empty".to_string());
    }
    Ok(format!("{}/{}", package.namespace, package.name))
}

fn parse_workspace_package_defaults(
    value: &toml::Value,
    schema: ManifestSchema,
) -> Result<WorkspacePackageDefaults, String> {
    let table = value
        .as_table()
        .ok_or_else(|| "manifest `workspace.package` must be a TOML table".to_string())?;
    if schema == ManifestSchema::V2 {
        validate_supported_fields(
            table,
            "workspace.package",
            &[
                "namespace",
                "version",
                "edition",
                "description",
                "license",
                "repository",
                "publish",
            ],
        )?;
        if table.contains_key("name") {
            return Err(
                "manifest v2 `workspace.package.name` is not inheritable; define `package.name` in each member"
                    .to_string(),
            );
        }
    }
    Ok(WorkspacePackageDefaults {
        namespace: optional_string_field(table, "workspace.package", "namespace")?,
        name: if schema == ManifestSchema::V1 {
            optional_string_field(table, "workspace.package", "name")?
        } else {
            None
        },
        version: optional_string_field(table, "workspace.package", "version")?,
        edition: optional_string_field(table, "workspace.package", "edition")?,
        details: parse_package_details(table)?,
    })
}

fn parse_workspace_dependencies(
    value: &toml::Value,
    schema: ManifestSchema,
    root: &Path,
) -> Result<BTreeMap<String, Dependency>, String> {
    let table = value
        .as_table()
        .ok_or_else(|| "manifest `workspace.dependencies` must be a TOML table".to_string())?;
    let mut dependencies = BTreeMap::new();
    for (alias, value) in table {
        if let Some(dependency) = parse_dependency_value(alias, value, None, schema, Some(root))? {
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

fn validate_supported_fields(
    table: &toml::map::Map<String, toml::Value>,
    section: &str,
    supported: &[&str],
) -> Result<(), String> {
    if let Some(field) = table
        .keys()
        .find(|field| !supported.contains(&field.as_str()))
    {
        return Err(format!("`{section}` contains unsupported field `{field}`"));
    }
    Ok(())
}

fn validate_v2_top_level(document: &toml::Value) -> Result<(), String> {
    let table = document
        .as_table()
        .ok_or_else(|| "manifest root must be a TOML table".to_string())?;
    if table.contains_key("trust") {
        return Err(
            "manifest v2 does not allow `[trust]`; move registry trust policy to `.nomo/config.toml`"
                .to_string(),
        );
    }
    validate_supported_fields(
        table,
        "manifest v2 root",
        &[
            "manifest-version",
            "package",
            "workspace",
            "dependencies",
            "ffi",
        ],
    )
}

fn optional_bool_field(
    table: &toml::map::Map<String, toml::Value>,
    section: &str,
    key: &str,
) -> Result<Option<bool>, String> {
    match table.get(key) {
        Some(value) => value
            .as_bool()
            .map(Some)
            .ok_or_else(|| format!("manifest `{section}.{key}` must be a boolean")),
        None => Ok(None),
    }
}

fn parse_package_details(
    table: &toml::map::Map<String, toml::Value>,
) -> Result<PackageDetails, String> {
    Ok(PackageDetails {
        description: optional_string_field(table, "package", "description")?,
        license: optional_string_field(table, "package", "license")?,
        repository: optional_string_field(table, "package", "repository")?,
        publish: optional_bool_field(table, "package", "publish")?,
    })
}

fn required_package_string(
    table: &toml::map::Map<String, toml::Value>,
    key: &str,
) -> Result<String, String> {
    optional_string_field(table, "package", key)?
        .ok_or_else(|| format!("manifest v2 `package.{key}` must be explicitly defined"))
}

fn v2_package_string(
    table: &toml::map::Map<String, toml::Value>,
    workspace: Option<&WorkspaceContext>,
    inherit: bool,
    key: &str,
) -> Result<String, String> {
    if let Some(value) = optional_string_field(table, "package", key)? {
        return Ok(value);
    }
    if inherit {
        return workspace_package_default(workspace, key);
    }
    Err(format!(
        "manifest v2 `package.{key}` must be explicitly defined or inherited with `package.inherit = \"workspace\"`"
    ))
}

fn inherit_package_details(details: &mut PackageDetails, workspace: &WorkspaceContext) {
    if details.description.is_none() {
        details.description = workspace.package.details.description.clone();
    }
    if details.license.is_none() {
        details.license = workspace.package.details.license.clone();
    }
    if details.repository.is_none() {
        details.repository = workspace.package.details.repository.clone();
    }
    if details.publish.is_none() {
        details.publish = workspace.package.details.publish;
    }
}

fn validate_workspace_patterns(section: &str, patterns: &[String]) -> Result<(), String> {
    for pattern in patterns {
        let path = Path::new(pattern);
        if pattern.is_empty()
            || path.is_absolute()
            || path.components().any(|component| {
                matches!(
                    component,
                    Component::ParentDir | Component::RootDir | Component::Prefix(_)
                )
            })
        {
            return Err(format!(
                "manifest `{section}` entry `{pattern}` must be a relative path inside the workspace"
            ));
        }
        if pattern.contains("**") {
            return Err(format!(
                "manifest `{section}` entry `{pattern}` uses unsupported recursive wildcard `**`"
            ));
        }
    }
    Ok(())
}

fn workspace_context_includes_root(
    context: &WorkspaceContext,
    package_root: &Path,
) -> Result<bool, String> {
    let workspace_root = fs::canonicalize(&context.root).map_err(|err| {
        format!(
            "failed to resolve workspace root {}: {err}",
            context.root.display()
        )
    })?;
    let package_root = fs::canonicalize(package_root).map_err(|err| {
        format!(
            "failed to resolve package root {}: {err}",
            package_root.display()
        )
    })?;
    let Ok(relative) = package_root.strip_prefix(&workspace_root) else {
        return Ok(false);
    };
    let relative = if relative.as_os_str().is_empty() {
        ".".to_string()
    } else {
        relative.to_string_lossy().replace('\\', "/")
    };
    let included = context
        .members
        .iter()
        .any(|pattern| workspace_pattern_matches(pattern, &relative));
    let excluded = context.exclude.iter().any(|pattern| {
        workspace_pattern_matches(pattern, &relative)
            || (!pattern.contains('*')
                && relative
                    .strip_prefix(pattern.trim_matches('/'))
                    .is_some_and(|suffix| suffix.starts_with('/')))
    });
    Ok(included && !excluded)
}

fn workspace_pattern_matches(pattern: &str, relative: &str) -> bool {
    if pattern == "." || relative == "." {
        return pattern == relative;
    }
    let pattern = pattern.trim_matches('/').split('/').collect::<Vec<_>>();
    let relative = relative.trim_matches('/').split('/').collect::<Vec<_>>();
    pattern.len() == relative.len()
        && pattern
            .iter()
            .zip(relative)
            .all(|(pattern, value)| wildcard_segment_matches(pattern, value))
}

fn wildcard_segment_matches(pattern: &str, value: &str) -> bool {
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
        if index == 0 && !pattern.starts_with('*') {
            let Some(stripped) = remaining.strip_prefix(part) else {
                return false;
            };
            remaining = stripped;
        } else if index == parts.len() - 1 && !pattern.ends_with('*') {
            return remaining.ends_with(part);
        } else {
            let Some(position) = remaining.find(part) else {
                return false;
            };
            remaining = &remaining[position + part.len()..];
        }
    }
    true
}

fn parse_target_condition(value: &toml::Value, section: &str) -> Result<TargetCondition, String> {
    let table = value
        .as_table()
        .ok_or_else(|| format!("manifest `{section}` must be an inline table"))?;
    if let Some(field) = table
        .keys()
        .find(|field| !matches!(field.as_str(), "arch" | "os" | "env"))
    {
        return Err(format!(
            "manifest `{section}` contains unsupported target field `{field}`; expected arch, os, or env"
        ));
    }
    let condition = parse_target_condition_fields(table, section)?;
    if condition.is_unconditional() {
        return Err(format!(
            "manifest `{section}` must select at least one target arch, os, or env"
        ));
    }
    Ok(condition)
}

fn parse_target_condition_fields(
    table: &toml::map::Map<String, toml::Value>,
    section: &str,
) -> Result<TargetCondition, String> {
    let condition = TargetCondition {
        arch: parse_target_values(table.get("arch"), section, "arch", canonical_arch)?,
        os: parse_target_values(table.get("os"), section, "os", canonical_os)?,
        env: parse_target_values(table.get("env"), section, "env", canonical_env)?,
    };
    if !condition.is_unconditional() && !condition.is_satisfiable() {
        return Err(format!(
            "manifest `{section}` condition `{condition}` does not match any supported target"
        ));
    }
    Ok(condition)
}

fn parse_target_values(
    value: Option<&toml::Value>,
    section: &str,
    field: &str,
    canonicalize: fn(&str) -> Option<&'static str>,
) -> Result<Vec<String>, String> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let raw = if let Some(value) = value.as_str() {
        vec![value]
    } else if let Some(values) = value.as_array() {
        if values.is_empty() {
            return Err(format!(
                "manifest `{section}.{field}` must not be an empty array"
            ));
        }
        values
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .ok_or_else(|| format!("manifest `{section}.{field}` entries must be strings"))
            })
            .collect::<Result<Vec<_>, _>>()?
    } else {
        return Err(format!(
            "manifest `{section}.{field}` must be a string or string array"
        ));
    };
    let mut values = raw
        .into_iter()
        .map(|value| {
            canonicalize(value).map(str::to_string).ok_or_else(|| {
                format!("manifest `{section}.{field}` has unsupported value `{value}`")
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    values.sort();
    values.dedup();
    Ok(values)
}

fn canonical_arch(value: &str) -> Option<&'static str> {
    match value {
        "x86_64" | "amd64" => Some("x86_64"),
        "aarch64" | "arm64" => Some("aarch64"),
        _ => None,
    }
}

fn canonical_os(value: &str) -> Option<&'static str> {
    match value {
        "linux" => Some("linux"),
        "darwin" | "macos" => Some("darwin"),
        "windows" => Some("windows"),
        _ => None,
    }
}

fn canonical_env(value: &str) -> Option<&'static str> {
    match value {
        "gnu" => Some("gnu"),
        "msvc" => Some("msvc"),
        "none" => Some("none"),
        _ => None,
    }
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
    schema: ManifestSchema,
    package_root: Option<&Path>,
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
    if schema == ManifestSchema::V2 {
        validate_supported_fields(
            fields,
            &format!("dependencies.{alias}"),
            &[
                "package",
                "version",
                "registry",
                "path",
                "git",
                "branch",
                "tag",
                "rev",
                "workspace",
                "target",
            ],
        )?;
    }

    let target = match fields.get("target") {
        Some(value) => parse_target_condition(value, &format!("dependencies.{alias}.target"))?,
        None => TargetCondition::default(),
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

    let package = match optional_dependency_string(alias, fields, "package")? {
        Some(package) => package,
        None if schema == ManifestSchema::V2 && fields.contains_key("path") => {
            let package_root = package_root.ok_or_else(|| {
                format!(
                    "dependency `{alias}` cannot derive path package identity without a manifest root"
                )
            })?;
            let path = required_dependency_string(alias, fields, "path")?;
            parse_package_identity_only_at_root(&normalize_logical_path(&package_root.join(path)))?
        }
        None => {
            return Err(format!(
                "dependency `{alias}` field `package` must be a non-empty string"
            ));
        }
    };
    validate_package_id(&package)?;
    if schema == ManifestSchema::V2 && fields.contains_key("path") {
        let package_root = package_root.ok_or_else(|| {
            format!(
                "dependency `{alias}` cannot validate path package identity without a manifest root"
            )
        })?;
        let path = required_dependency_string(alias, fields, "path")?;
        let derived =
            parse_package_identity_only_at_root(&normalize_logical_path(&package_root.join(path)))?;
        if derived != package {
            return Err(format!(
                "dependency `{alias}` asserts package `{package}`, but path manifest declares `{derived}`"
            ));
        }
    }
    if alias == "std" {
        if package == "nomo-lang/std" {
            return Ok(None);
        }
        return Err(
            "dependency alias `std` is reserved for the built-in standard library".to_string(),
        );
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
        target,
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

#[cfg(test)]
mod module_root_tests {
    use super::package_name_to_module_root;

    #[test]
    fn derives_stable_module_roots_from_current_and_legacy_names() {
        assert_eq!(package_name_to_module_root("hello").unwrap(), "hello");
        assert_eq!(
            package_name_to_module_root("hello-world").unwrap(),
            "hello_world"
        );
        assert_eq!(
            package_name_to_module_root("HelloWorld").unwrap(),
            "hello_world"
        );
        assert!(package_name_to_module_root("hello world").is_err());
    }
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
mod manifest_v2_tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_root(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "nomo-manifest-v2-{label}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    fn write_package(root: &Path, body: &str) {
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("nomo.toml"), body).unwrap();
        fs::write(root.join("src/main.nomo"), "package app.main\n").unwrap();
    }

    #[test]
    fn parses_strict_standalone_manifest_v2_and_rejects_v1_policy_fields() {
        let manifest = parse_manifest_text(
            "manifest-version = 2\n\n[package]\nnamespace = \"acme\"\nname = \"demo\"\nversion = \"1.2.3\"\nedition = \"2026\"\ndescription = \"Demo\"\npublish = false\n",
            Path::new("demo"),
        )
        .unwrap();
        assert_eq!(manifest.schema, ManifestSchema::V2);
        assert_eq!(manifest.kind, ManifestDocumentKind::Package);
        assert_eq!(manifest.package.namespace, "acme");
        assert_eq!(manifest.details.description.as_deref(), Some("Demo"));
        assert_eq!(manifest.details.publish, Some(false));

        let missing = parse_manifest_text(
            "manifest-version = 2\n\n[package]\nname = \"demo\"\nversion = \"1.2.3\"\nedition = \"2026\"\n",
            Path::new("demo"),
        )
        .unwrap_err();
        assert!(missing.contains("package.namespace"), "{missing}");

        let trust = parse_manifest_text(
            "manifest-version = 2\n\n[package]\nnamespace = \"acme\"\nname = \"demo\"\nversion = \"1.2.3\"\nedition = \"2026\"\n\n[trust]\npolicy = \"signed\"\n",
            Path::new("demo"),
        )
        .unwrap_err();
        assert!(trust.contains(".nomo/config.toml"), "{trust}");
    }

    #[test]
    fn verifies_membership_before_inheritance_and_derives_path_identity() {
        let root = temp_root("workspace");
        let app = root.join("apps/cli");
        let core = root.join("packages/core");
        let outsider = root.join("scratch/tool");
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join("nomo.toml"),
            "manifest-version = 2\n\n[workspace]\nmembers = [\"apps/*\", \"packages/*\"]\ndefault-members = [\"apps/cli\"]\nresolver = \"2\"\n\n[workspace.package]\nnamespace = \"acme\"\nversion = \"0.1.0\"\nedition = \"2026\"\nlicense = \"MIT\"\n\n[workspace.dependencies]\ncore = { path = \"packages/core\" }\n",
        )
        .unwrap();
        write_package(
            &app,
            "manifest-version = 2\n\n[package]\nname = \"cli\"\ninherit = \"workspace\"\n\n[dependencies]\ncore = { workspace = true }\n",
        );
        write_package(
            &core,
            "manifest-version = 2\n\n[package]\nname = \"core\"\ninherit = \"workspace\"\n",
        );
        write_package(
            &outsider,
            "manifest-version = 2\n\n[package]\nname = \"tool\"\ninherit = \"workspace\"\n",
        );

        let parsed = parse_manifest_at_root(&app).unwrap();
        assert_eq!(parsed.package.namespace, "acme");
        assert_eq!(parsed.details.license.as_deref(), Some("MIT"));
        assert_eq!(parsed.dependencies[0].package, "acme/core");
        assert_eq!(
            workspace_root_for_package(&app).unwrap(),
            Some(root.clone())
        );
        assert_eq!(workspace_root_for_package(&outsider).unwrap(), None);
        let error = parse_manifest_at_root(&outsider).unwrap_err();
        assert!(error.contains("verified workspace member"), "{error}");

        let mismatch = parse_manifest_text(
            "manifest-version = 2\n\n[package]\nnamespace = \"acme\"\nname = \"standalone\"\nversion = \"0.1.0\"\nedition = \"2026\"\n\n[dependencies]\ncore = { package = \"acme/not-core\", path = \"../../packages/core\" }\n",
            &app,
        )
        .unwrap_err();
        assert!(
            mismatch.contains("path manifest declares `acme/core`"),
            "{mismatch}"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn loads_registry_policy_from_workspace_project_config() {
        let root = temp_root("config");
        let app = root.join("apps/cli");
        fs::create_dir_all(root.join(".nomo")).unwrap();
        fs::write(
            root.join("nomo.toml"),
            "manifest-version = 2\n\n[workspace]\nmembers = [\"apps/*\"]\n\n[workspace.package]\nnamespace = \"acme\"\nversion = \"0.1.0\"\nedition = \"2026\"\n",
        )
        .unwrap();
        write_package(
            &app,
            "manifest-version = 2\n\n[package]\nname = \"cli\"\ninherit = \"workspace\"\n",
        );
        fs::write(
            root.join(".nomo/config.toml"),
            format!(
                "config-version = 1\n\n[registry]\npolicy = \"signed+transparent\"\ntransparency-keys = [\"{}\"]\nproof-max-age-seconds = 60\noffline-proof-max-age-seconds = 600\ngossip-checkpoints = [\"trust/peer.json\"]\n",
                "a".repeat(64)
            ),
        )
        .unwrap();

        let manifest = parse_manifest_at_root(&app).unwrap();
        assert_eq!(manifest.trust, RegistryTrustPolicy::SignedTransparent);
        assert_eq!(manifest.transparency.proof_max_age_seconds, 60);
        assert_eq!(
            manifest.transparency.gossip_checkpoints,
            vec![root.join("trust/peer.json")]
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn migrates_legacy_defaults_and_trust_without_rewriting_v2() {
        let root = temp_root("migration-standalone");
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join("nomo.toml"),
            "[package]\nname = \"legacy-demo\"\n\n[trust]\npolicy = \"signed\"\n",
        )
        .unwrap();

        let migration = migrate_manifest_at_root(&root).unwrap();
        assert!(migration.changed);
        let document = parse_manifest_document(&migration.manifest).unwrap();
        assert_eq!(manifest_schema(&document).unwrap(), ManifestSchema::V2);
        let package = document.get("package").unwrap().as_table().unwrap();
        assert_eq!(package.get("namespace").unwrap().as_str(), Some("local"));
        assert_eq!(package.get("version").unwrap().as_str(), Some("0.1.0"));
        assert_eq!(package.get("edition").unwrap().as_str(), Some("2026"));
        assert!(document.get("trust").is_none());
        let config =
            parse_project_config_text(migration.project_config.as_deref().unwrap(), &root).unwrap();
        assert_eq!(config.trust, RegistryTrustPolicy::Signed);

        fs::write(root.join("nomo.toml"), migration.manifest).unwrap();
        let second = migrate_manifest_at_root(&root).unwrap();
        assert!(!second.changed);
        assert!(second.project_config.is_none());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn migrates_workspace_field_inheritance_to_one_explicit_switch() {
        let root = temp_root("migration-workspace");
        let app = root.join("apps/cli");
        fs::create_dir_all(&app).unwrap();
        fs::write(
            root.join("nomo.toml"),
            "[workspace]\nmembers = [\"apps/*\"]\n\n[workspace.package]\nnamespace = \"acme\"\nname = \"invalid-default\"\nversion = \"1.2.3\"\nedition = \"2026\"\n",
        )
        .unwrap();
        fs::write(
            app.join("nomo.toml"),
            "[package]\nname = \"cli\"\nnamespace.workspace = true\nversion.workspace = true\nedition.workspace = true\n",
        )
        .unwrap();

        let member = migrate_manifest_at_root(&app).unwrap();
        let member = parse_manifest_document(&member.manifest).unwrap();
        let package = member.get("package").unwrap().as_table().unwrap();
        assert_eq!(package.get("name").unwrap().as_str(), Some("cli"));
        assert_eq!(package.get("inherit").unwrap().as_str(), Some("workspace"));
        assert!(!package.contains_key("namespace"));
        assert!(!package.contains_key("version"));
        assert!(!package.contains_key("edition"));

        let workspace = migrate_manifest_at_root(&root).unwrap();
        let workspace = parse_manifest_document(&workspace.manifest).unwrap();
        let defaults = workspace
            .get("workspace")
            .unwrap()
            .get("package")
            .unwrap()
            .as_table()
            .unwrap();
        assert!(!defaults.contains_key("name"));
        fs::remove_dir_all(root).unwrap();
    }
}

#[cfg(test)]
mod trust_tests {
    use super::*;

    #[test]
    fn parses_explicit_registry_trust_policies_and_defaults_to_checksum_only() {
        let base = "[package]\nnamespace = \"app\"\nname = \"demo\"\nversion = \"1.0.0\"\nedition = \"2026\"\n";
        let default = parse_manifest_text(base, Path::new("demo")).unwrap();
        assert_eq!(default.trust, RegistryTrustPolicy::ChecksumOnly);
        assert!(default.transparency.keys.is_empty());
        assert_eq!(
            default.transparency.proof_max_age_seconds,
            DEFAULT_TRANSPARENCY_PROOF_MAX_AGE_SECONDS
        );

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
        let configured = format!(
            "{base}\n[trust]\npolicy = \"signed+transparent\"\ntransparency-keys = [\"{}\"]\nproof-max-age-seconds = 60\noffline-proof-max-age-seconds = 600\nmax-future-skew-seconds = 10\ngossip-checkpoints = [\"trust/peer.json\"]\n",
            "a".repeat(64)
        );
        let configured = parse_manifest_text(&configured, Path::new("demo")).unwrap();
        assert_eq!(configured.transparency.proof_max_age_seconds, 60);
        assert_eq!(configured.transparency.offline_proof_max_age_seconds, 600);
        assert_eq!(configured.transparency.max_future_skew_seconds, 10);
        assert_eq!(
            configured.transparency.gossip_checkpoints,
            vec![PathBuf::from("demo/trust/peer.json")]
        );
        let invalid = format!("{base}\n[trust]\npolicy = \"trust-me\"\n");
        assert!(
            parse_manifest_text(&invalid, Path::new("demo"))
                .unwrap_err()
                .contains("unknown registry trust policy")
        );
    }
}

#[cfg(test)]
mod target_tests {
    use super::*;

    const PACKAGE: &str = "[package]\nnamespace = \"app\"\nname = \"demo\"\nversion = \"1.0.0\"\nedition = \"2026\"\n";

    #[test]
    fn parses_and_canonicalizes_target_conditioned_dependencies() {
        let manifest = parse_manifest_text(
            &format!(
                "{PACKAGE}\n[dependencies]\nplatform = {{ package = \"app/platform\", path = \"../platform\", target = {{ arch = [\"amd64\", \"arm64\"], os = \"macos\" }} }}\n"
            ),
            Path::new("demo"),
        )
        .unwrap();
        let condition = &manifest.dependencies[0].target;
        assert_eq!(condition.architectures(), ["aarch64", "x86_64"]);
        assert_eq!(condition.operating_systems(), ["darwin"]);
        assert!(condition.matches(&"aarch64-apple-darwin".parse::<TargetTriple>().unwrap()));
        assert!(!condition.matches(&"aarch64-unknown-linux-gnu".parse::<TargetTriple>().unwrap()));
    }

    #[test]
    fn rejects_target_conditions_that_match_no_supported_target() {
        let error = parse_manifest_text(
            &format!(
                "{PACKAGE}\n[dependencies]\nbad = {{ package = \"app/bad\", path = \"../bad\", target = {{ os = \"linux\", env = \"msvc\" }} }}\n"
            ),
            Path::new("demo"),
        )
        .unwrap_err();
        assert!(
            error.contains("does not match any supported target"),
            "{error}"
        );
    }

    #[test]
    fn selects_target_conditioned_ffi_metadata() {
        let manifest = parse_manifest_text(
            &format!(
                "{PACKAGE}\n[ffi]\nlibraries = [\"common\"]\n\n[[ffi.target]]\nos = [\"linux\"]\nlibraries = [\"pthread\"]\nsources = [\"native/linux.c\"]\n\n[[ffi.target]]\nos = \"macos\"\nframeworks = [\"Security\"]\n"
            ),
            Path::new("demo"),
        )
        .unwrap();
        let linux =
            manifest.ffi_for_target(&"x86_64-unknown-linux-gnu".parse::<TargetTriple>().unwrap());
        assert_eq!(linux.libraries, ["common", "pthread"]);
        assert_eq!(linux.sources, [PathBuf::from("demo/native/linux.c")]);
        assert!(linux.frameworks.is_empty());

        let macos =
            manifest.ffi_for_target(&"aarch64-apple-darwin".parse::<TargetTriple>().unwrap());
        assert_eq!(macos.libraries, ["common"]);
        assert_eq!(macos.frameworks, ["Security"]);
        assert!(macos.sources.is_empty());
    }
}
