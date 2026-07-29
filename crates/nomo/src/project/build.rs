use super::build_metadata::{
    BuildMetadata, FileRecord, PASS_PIPELINE_VERSION, ProducerExecutableIdentity,
    ReleaseBackendProvenance, atomic_write_canonical_json, canonical_json_bytes,
    codegen_cache_configuration, inspect_compiler, producer_executable_identity, release_c_flags,
    release_driver_config_flags_for_compiler, remove_stale_file, resolve_executable,
    run_recorded_command, write_build_metadata, write_release_provenance,
};
use super::{
    BuildError, BuildProfile, DependencyResolutionOptions, Project, WorkspaceGraph,
    discover_workspace_for_target, project_ffi_link_metadata_for_target_with_options,
    project_module_context_for_target_with_options, project_package_id,
};
use crate::compiler::compile_source_to_c_with_module_identity_for_target;
use crate::incremental::{ContentFingerprint, PersistentQueryCache, QueryKey, project_query_key};
use nomo_manifest::FfiLinkMetadata;
use nomo_target::{CToolchain, TargetTriple};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, OpenOptions};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone)]
pub struct CachedStandaloneSource {
    generated_source: String,
    query_key: QueryKey,
    producer_executable: ProducerExecutableIdentity,
}

const WORKSPACE_EVIDENCE_SCOPE_SCHEMA: u32 = 1;
const WORKSPACE_EVIDENCE_CATALOG_SCHEMA: u32 = 1;
const WORKSPACE_OWNERSHIP_RECEIPT_SCHEMA: u32 = 1;
const WORKSPACE_EVIDENCE_STATE_COMPONENTS: &[&str] = &[".nomo", "state", "release-evidence", "v1"];
const WORKSPACE_OWNERSHIP_RECEIPT_FILE: &str = ".nomo-release-owner-v1.json";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkspaceEvidenceSelection {
    AllMembers,
    DefaultMembers,
    Package(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WorkspaceEvidenceMember {
    package_id: String,
    package_name: String,
    relative_root: String,
    member_key: String,
    default_member: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WorkspaceEvidenceScopeMarker {
    schema: u32,
    stable_scope_id: String,
    workspace_root: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WorkspaceEvidenceCatalog {
    schema: u32,
    stable_scope_id: String,
    catalog_generation: String,
    workspace_root: String,
    members: Vec<WorkspaceEvidenceMember>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct WorkspaceCatalogGeneration {
    domain: &'static str,
    members: Vec<WorkspaceEvidenceMember>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct WorkspaceMemberKeyInput {
    domain: &'static str,
    stable_scope_id: String,
    package_id: String,
    relative_root: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WorkspaceOwnershipReceipt {
    schema: u32,
    stable_scope_id: String,
    workspace_root: String,
    member_key: String,
    member_root: String,
    package_id: String,
    package_name: String,
    selected_profile: BuildProfile,
    target_triple: String,
    target_scoped_artifacts: bool,
    metadata_sha256: String,
    provenance_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct WorkspaceReleaseOwnerContext {
    stable_scope_id: String,
    workspace_root: PathBuf,
    member_key: String,
    member_root: PathBuf,
    package_id: String,
    package_name: String,
}

impl CachedStandaloneSource {
    pub fn generated_source(&self) -> &str {
        &self.generated_source
    }
}

struct BuildEvidenceLock {
    _file: File,
}

impl BuildEvidenceLock {
    fn acquire(target_dir: &Path, create_target: bool) -> Result<Option<Self>, BuildError> {
        reject_existing_symlink_or_reparse_components(target_dir)?;
        match fs::symlink_metadata(target_dir) {
            Ok(metadata)
                if metadata.file_type().is_dir() && !metadata_is_symlink_or_reparse(&metadata) => {}
            Ok(_) => {
                return Err(BuildError::Message(format!(
                    "build output path is not a safe directory: {}",
                    target_dir.display()
                )));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound && !create_target => {
                return Ok(None);
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                fs::create_dir_all(target_dir).map_err(|error| {
                    BuildError::Message(format!(
                        "failed to create build output directory {}: {error}",
                        target_dir.display()
                    ))
                })?;
                reject_existing_symlink_or_reparse_components(target_dir)?;
            }
            Err(error) => {
                return Err(BuildError::Message(format!(
                    "failed to inspect build output directory {}: {error}",
                    target_dir.display()
                )));
            }
        }
        let lock_path = target_dir.join(".nomo-build.lock");
        reject_existing_symlink_or_reparse_components(&lock_path)?;
        if let Ok(metadata) = fs::symlink_metadata(&lock_path)
            && (!metadata.file_type().is_file() || metadata_is_symlink_or_reparse(&metadata))
        {
            return Err(BuildError::Message(format!(
                "build lock is not a safe regular file: {}",
                lock_path.display()
            )));
        }
        let lock = open_build_lock(&lock_path)?;
        let lock_metadata = lock.metadata().map_err(|error| {
            BuildError::Message(format!(
                "failed to inspect opened build lock {}: {error}",
                lock_path.display()
            ))
        })?;
        if !lock_metadata.file_type().is_file() || metadata_is_symlink_or_reparse(&lock_metadata) {
            return Err(BuildError::Message(format!(
                "opened build lock is not a safe regular file: {}",
                lock_path.display()
            )));
        }
        lock.lock().map_err(|error| {
            BuildError::Message(format!(
                "failed to acquire build lock {}: {error}",
                lock_path.display()
            ))
        })?;
        Ok(Some(Self { _file: lock }))
    }
}

struct WorkspaceCatalogLock {
    _file: File,
}

impl WorkspaceCatalogLock {
    fn acquire(workspace_root: &Path, create_state: bool) -> Result<Option<Self>, BuildError> {
        let state_dir = workspace_evidence_state_dir(workspace_root);
        if create_state {
            ensure_workspace_evidence_state_dir(workspace_root)?;
        } else {
            match fs::symlink_metadata(&state_dir) {
                Ok(metadata)
                    if metadata.file_type().is_dir()
                        && !metadata_is_symlink_or_reparse(&metadata) => {}
                Ok(_) => {
                    return Err(BuildError::Message(format!(
                        "workspace evidence state path is not a safe directory: {}",
                        state_dir.display()
                    )));
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
                Err(error) => {
                    return Err(BuildError::Message(format!(
                        "failed to inspect workspace evidence state {}: {error}",
                        state_dir.display()
                    )));
                }
            }
        }
        reject_existing_symlink_or_reparse_components(&state_dir)?;
        let lock_path = state_dir.join(".catalog.lock");
        reject_existing_symlink_or_reparse_components(&lock_path)?;
        if let Ok(metadata) = fs::symlink_metadata(&lock_path)
            && (!metadata.file_type().is_file() || metadata_is_symlink_or_reparse(&metadata))
        {
            return Err(BuildError::Message(format!(
                "workspace catalog lock is not a safe regular file: {}",
                lock_path.display()
            )));
        }
        let lock = open_build_lock(&lock_path)?;
        lock.lock().map_err(|error| {
            BuildError::Message(format!(
                "failed to acquire workspace catalog lock {}: {error}",
                lock_path.display()
            ))
        })?;
        Ok(Some(Self { _file: lock }))
    }
}

struct BuildEvidenceTransaction {
    target_dir: PathBuf,
    _lock: BuildEvidenceLock,
    committed: bool,
}

impl BuildEvidenceTransaction {
    fn acquire(target_dir: &Path) -> Result<Self, BuildError> {
        let target_dir = canonical_build_target_dir(target_dir)?;
        let lock = BuildEvidenceLock::acquire(&target_dir, true)?
            .expect("creating the target directory must produce a build lock");
        let transaction = Self {
            target_dir,
            _lock: lock,
            committed: false,
        };
        transaction.clear_evidence()?;
        Ok(transaction)
    }

    fn clear_evidence(&self) -> Result<(), BuildError> {
        clear_release_evidence_paths(
            &self.target_dir.join("release-provenance.json"),
            &self.target_dir.join(WORKSPACE_OWNERSHIP_RECEIPT_FILE),
            &self.target_dir.join("nomo-build-metadata.json"),
        )
    }

    fn commit(mut self) {
        self.committed = true;
    }
}

impl Drop for BuildEvidenceTransaction {
    fn drop(&mut self) {
        if !self.committed {
            let _ = self.clear_evidence();
        }
    }
}

fn publish_release_evidence(
    metadata_path: &Path,
    metadata: &BuildMetadata,
    owner_receipt: Option<&WorkspaceOwnershipReceipt>,
    provenance_path: &Path,
    provenance: &ReleaseBackendProvenance,
    expected_provenance: &FileRecord,
) -> Result<(), BuildError> {
    let receipt_path = metadata_path
        .parent()
        .ok_or_else(|| {
            BuildError::Message(format!(
                "build metadata path has no target directory: {}",
                metadata_path.display()
            ))
        })?
        .join(WORKSPACE_OWNERSHIP_RECEIPT_FILE);
    publish_release_evidence_with(
        metadata_path,
        &receipt_path,
        provenance_path,
        expected_provenance,
        || write_build_metadata(metadata_path, metadata).map(|_| ()),
        || match owner_receipt {
            Some(receipt) => atomic_write_canonical_json(&receipt_path, receipt),
            None => Ok(()),
        },
        || write_release_provenance(provenance_path, provenance),
    )
}

fn publish_release_evidence_with(
    metadata_path: &Path,
    receipt_path: &Path,
    provenance_path: &Path,
    expected_provenance: &FileRecord,
    write_metadata: impl FnOnce() -> Result<(), BuildError>,
    write_receipt: impl FnOnce() -> Result<(), BuildError>,
    write_provenance: impl FnOnce() -> Result<FileRecord, BuildError>,
) -> Result<(), BuildError> {
    let result = (|| {
        write_metadata()?;
        write_receipt()?;
        let written_provenance = write_provenance()?;
        if &written_provenance != expected_provenance {
            return Err(BuildError::Message(
                "published release provenance does not match build metadata".to_string(),
            ));
        }
        Ok(())
    })();
    if let Err(error) = result {
        return match clear_release_evidence_paths(provenance_path, receipt_path, metadata_path) {
            Ok(()) => Err(error),
            Err(cleanup_error) => Err(BuildError::Message(format!(
                "{}; release evidence cleanup failed: {}",
                error.human(),
                cleanup_error.human()
            ))),
        };
    }
    Ok(())
}

fn clear_release_evidence_paths(
    provenance_path: &Path,
    receipt_path: &Path,
    metadata_path: &Path,
) -> Result<(), BuildError> {
    clear_release_evidence_paths_with(
        provenance_path,
        receipt_path,
        metadata_path,
        remove_stale_file,
    )
}

fn clear_release_evidence_paths_with(
    provenance_path: &Path,
    receipt_path: &Path,
    metadata_path: &Path,
    mut remove: impl FnMut(&Path) -> Result<(), BuildError>,
) -> Result<(), BuildError> {
    let mut first_error = None;
    for path in [provenance_path, receipt_path, metadata_path] {
        if let Err(error) = remove(path)
            && first_error.is_none()
        {
            first_error = Some(error);
        }
    }
    match first_error {
        Some(error) => Err(error),
        None => Ok(()),
    }
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
    build_project_with_profile_options(project, emit_c_only, options, BuildProfile::Debug)
}

pub fn build_project_with_profile_options(
    project: &Project,
    emit_c_only: bool,
    options: DependencyResolutionOptions,
    profile: BuildProfile,
) -> Result<PathBuf, BuildError> {
    let target = TargetTriple::host().map_err(BuildError::Message)?;
    build_project_impl(project, emit_c_only, options, &target, false, profile)
}

pub fn build_project_for_target_with_options(
    project: &Project,
    emit_c_only: bool,
    options: DependencyResolutionOptions,
    target: &TargetTriple,
) -> Result<PathBuf, BuildError> {
    build_project_for_target_with_profile_options(
        project,
        emit_c_only,
        options,
        target,
        BuildProfile::Debug,
    )
}

pub fn build_project_for_target_with_profile_options(
    project: &Project,
    emit_c_only: bool,
    options: DependencyResolutionOptions,
    target: &TargetTriple,
    profile: BuildProfile,
) -> Result<PathBuf, BuildError> {
    build_project_impl(project, emit_c_only, options, target, true, profile)
}

pub fn build_standalone_release_c(
    source: &Path,
    generated: &CachedStandaloneSource,
    output: Option<&Path>,
    target: &TargetTriple,
) -> Result<PathBuf, BuildError> {
    let host = TargetTriple::host().map_err(BuildError::Message)?;
    let source_parent = source.parent().unwrap_or_else(|| Path::new("."));
    let target_dir = if target == &host {
        source_parent.join("build")
    } else {
        source_parent.join("build").join(target.to_string())
    };
    let evidence = BuildEvidenceTransaction::acquire(&target_dir)?;
    let c_dir = target_dir.join("c");
    let object_dir = target_dir.join("obj");
    let bin_dir = target_dir.join("bin");
    fs::create_dir_all(&c_dir).map_err(|error| BuildError::Message(error.to_string()))?;
    fs::create_dir_all(&object_dir).map_err(|error| BuildError::Message(error.to_string()))?;
    fs::create_dir_all(&bin_dir).map_err(|error| BuildError::Message(error.to_string()))?;
    let c_path = c_dir.join("main.c");
    let stem = source
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("program");
    let default_name = if target.operating_system().as_str() == "windows" {
        format!("{stem}.exe")
    } else {
        stem.to_string()
    };
    let bin_path = output
        .map(Path::to_path_buf)
        .unwrap_or_else(|| bin_dir.join(default_name));
    if let Some(parent) = bin_path.parent() {
        fs::create_dir_all(parent).map_err(|error| BuildError::Message(error.to_string()))?;
    }
    let provenance_path = target_dir.join("release-provenance.json");
    let metadata_path = target_dir.join("nomo-build-metadata.json");
    fs::write(&c_path, &generated.generated_source)
        .map_err(|error| BuildError::Message(error.to_string()))?;
    let generated_c = FileRecord::from_path(&c_path)?;
    let toolchain = c_toolchain_for_profile(target, &host, BuildProfile::Release)?;
    let compiler_path = resolve_executable(&toolchain.program, BuildProfile::Release)?;
    let compiler = inspect_compiler(&compiler_path, &toolchain.args, true)?;
    let ffi = FfiLinkMetadata::default();
    let (objects, compile_commands) = compile_release_translation_units(
        &compiler_path,
        &toolchain,
        &c_path,
        &c_dir,
        &object_dir,
        &ffi,
        false,
    )?;
    remove_stale_file(&bin_path)?;
    let link_argv = release_link_argv(
        &compiler_path,
        &toolchain,
        &objects,
        &bin_path,
        &ffi,
        target,
        generated_c_uses_native_tasks(&generated.generated_source),
        false,
        generated_c_uses_math(&generated.generated_source),
        generated_c_uses_dynamic_loader(&generated.generated_source),
        generated_c_uses_winsock(&generated.generated_source),
    )?;
    let link_command = run_recorded_command(link_argv, BuildProfile::Release)?;
    let object_records = objects
        .iter()
        .map(|path| FileRecord::from_path(path))
        .collect::<Result<Vec<_>, _>>()?;
    let binary = FileRecord::from_path(&bin_path)?;
    let provenance = ReleaseBackendProvenance::new(
        compiler.clone(),
        object_records,
        compile_commands.clone(),
        link_command.clone(),
        generated_c.clone(),
        binary.clone(),
    );
    let provenance_record = FileRecord::for_canonical_json(&provenance_path, &provenance)?;
    let metadata = BuildMetadata::new(
        BuildProfile::Release,
        target.to_string(),
        generated.query_key.clone(),
        generated.producer_executable.clone(),
        Some(compiler),
        compile_commands,
        Some(link_command),
        None,
        generated_c,
        Some(binary),
        Some(provenance_record.clone()),
    )?;
    publish_release_evidence(
        &metadata_path,
        &metadata,
        None,
        &provenance_path,
        &provenance,
        &provenance_record,
    )?;
    evidence.commit();
    Ok(bin_path)
}

pub fn record_standalone_c_build_metadata(
    source: &Path,
    generated: &CachedStandaloneSource,
    output: Option<&Path>,
    target: &TargetTriple,
    profile: BuildProfile,
) -> Result<PathBuf, BuildError> {
    let host = TargetTriple::host().map_err(BuildError::Message)?;
    let source_parent = source.parent().unwrap_or_else(|| Path::new("."));
    let target_dir = if target == &host {
        source_parent.join("build")
    } else {
        source_parent.join("build").join(target.to_string())
    };
    let evidence = BuildEvidenceTransaction::acquire(&target_dir)?;
    let c_path = match output {
        Some(path) => path.to_path_buf(),
        None => target_dir.join("c").join("main.c"),
    };
    if output.is_none() {
        let parent = c_path.parent().ok_or_else(|| {
            BuildError::Message(format!(
                "generated-C output has no parent: {}",
                c_path.display()
            ))
        })?;
        fs::create_dir_all(parent).map_err(|error| BuildError::Message(error.to_string()))?;
        fs::write(&c_path, &generated.generated_source)
            .map_err(|error| BuildError::Message(error.to_string()))?;
    }
    let metadata_path = target_dir.join("nomo-build-metadata.json");
    let metadata = BuildMetadata::new(
        profile,
        target.to_string(),
        generated.query_key.clone(),
        generated.producer_executable.clone(),
        None,
        Vec::new(),
        None,
        None,
        FileRecord::from_path(&c_path)?,
        None,
        None,
    )?;
    write_build_metadata(&metadata_path, &metadata)?;
    evidence.commit();
    Ok(metadata_path)
}

pub fn compile_standalone_source_with_profile_cache(
    source: &Path,
    target: &TargetTriple,
    profile: BuildProfile,
) -> Result<CachedStandaloneSource, BuildError> {
    let producer_executable = producer_executable_identity()?;
    let cache_key =
        standalone_codegen_query_key(source, target, profile, "nomoc", &producer_executable)?;
    let cache_root = source.parent().unwrap_or_else(|| Path::new("."));
    let cache = PersistentQueryCache::at_root(cache_root);
    if let Some(cached) = cache.get::<String>(&cache_key) {
        return Ok(CachedStandaloneSource {
            generated_source: cached,
            query_key: cache_key,
            producer_executable,
        });
    }
    let generated = crate::compiler::compile_source_to_c_for_target(source, target)
        .map_err(BuildError::Diagnostic)?;
    let _ = cache.insert(&cache_key, &generated);
    Ok(CachedStandaloneSource {
        generated_source: generated,
        query_key: cache_key,
        producer_executable,
    })
}

pub fn compile_standalone_script_with_profile_cache(
    source: &Path,
    target: &TargetTriple,
    profile: BuildProfile,
) -> Result<CachedStandaloneSource, BuildError> {
    let producer_executable = producer_executable_identity()?;
    let cache_key =
        standalone_codegen_query_key(source, target, profile, "script", &producer_executable)?;
    let cache_root = source.parent().unwrap_or_else(|| Path::new("."));
    let cache = PersistentQueryCache::at_root(cache_root);
    if let Some(cached) = cache.get::<String>(&cache_key) {
        return Ok(CachedStandaloneSource {
            generated_source: cached,
            query_key: cache_key,
            producer_executable,
        });
    }
    let generated = crate::compiler::compile_script_source_to_c_for_target(source, target)
        .map_err(BuildError::Diagnostic)?;
    let _ = cache.insert(&cache_key, &generated);
    Ok(CachedStandaloneSource {
        generated_source: generated,
        query_key: cache_key,
        producer_executable,
    })
}

pub fn clear_project_build_metadata(
    project: &Project,
    target: &TargetTriple,
    target_scoped_artifacts: bool,
) -> Result<(), BuildError> {
    let target_dir = if target_scoped_artifacts {
        project.root.join("build").join(target.to_string())
    } else {
        project.root.join("build")
    };
    clear_build_metadata_at(&target_dir)
}

pub fn clear_standalone_build_metadata(
    source: &Path,
    target: &TargetTriple,
) -> Result<(), BuildError> {
    let host = TargetTriple::host().map_err(BuildError::Message)?;
    let source_parent = source.parent().unwrap_or_else(|| Path::new("."));
    let target_dir = if target == &host {
        source_parent.join("build")
    } else {
        source_parent.join("build").join(target.to_string())
    };
    clear_build_metadata_at(&target_dir)
}

pub fn clear_requested_build_metadata(
    requested_path: &Path,
    target: &TargetTriple,
    target_scoped_artifacts: bool,
) -> Result<(), BuildError> {
    let root = requested_build_root(requested_path)?;
    let target_dir = if target_scoped_artifacts {
        root.join("build").join(target.to_string())
    } else {
        root.join("build")
    };
    clear_build_metadata_at(&target_dir)
}

pub fn clear_workspace_project_build_metadata(
    projects: &[Project],
    target: &TargetTriple,
    target_scoped_artifacts: bool,
) -> Result<(), BuildError> {
    let mut roots = projects
        .iter()
        .map(|project| canonical_safe_directory(&project.root, "selected workspace member"))
        .collect::<Result<Vec<_>, _>>()?;
    roots.sort();
    roots.dedup();
    for root in roots {
        clear_project_root_build_metadata(&root, target, target_scoped_artifacts)?;
    }
    Ok(())
}

pub fn clear_failed_workspace_build_metadata(
    requested_path: &Path,
    target: &TargetTriple,
    target_scoped_artifacts: bool,
    profile: BuildProfile,
    selection: &WorkspaceEvidenceSelection,
) -> Result<(), BuildError> {
    clear_failed_workspace_build_metadata_from_catalog(
        requested_path,
        target,
        target_scoped_artifacts,
        profile,
        selection,
    )
    .map_err(|error| {
        let message = error.human();
        if message.starts_with(
            "workspace discovery failed and release evidence could not be safely cleared:",
        ) {
            error
        } else {
            BuildError::Message(format!(
                "workspace discovery failed and release evidence could not be safely cleared: {message}"
            ))
        }
    })
}

pub fn refresh_workspace_build_evidence_catalog(
    workspace: &WorkspaceGraph,
) -> Result<(), BuildError> {
    refresh_workspace_evidence_catalog(workspace).map(|_| ())
}

fn clear_build_metadata_at(target_dir: &Path) -> Result<(), BuildError> {
    match fs::symlink_metadata(target_dir) {
        Ok(metadata) if metadata.file_type().is_dir() => {}
        Ok(_) => {
            return Err(BuildError::Message(format!(
                "build output path is not a directory: {}",
                target_dir.display()
            )));
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(BuildError::Message(format!(
                "failed to inspect build output directory {}: {error}",
                target_dir.display()
            )));
        }
    }
    let evidence = BuildEvidenceTransaction::acquire(target_dir)?;
    evidence.commit();
    Ok(())
}

fn clear_project_root_build_metadata(
    root: &Path,
    target: &TargetTriple,
    target_scoped_artifacts: bool,
) -> Result<(), BuildError> {
    let target_dir = if target_scoped_artifacts {
        root.join("build").join(target.to_string())
    } else {
        root.join("build")
    };
    clear_build_metadata_at(&target_dir)
}

fn clear_failed_workspace_build_metadata_from_catalog(
    requested_path: &Path,
    target: &TargetTriple,
    target_scoped_artifacts: bool,
    _profile: BuildProfile,
    selection: &WorkspaceEvidenceSelection,
) -> Result<(), BuildError> {
    let Some(workspace_root) = find_workspace_evidence_catalog_root(requested_path)? else {
        return Err(BuildError::Message(
            "workspace discovery failed and release evidence could not be safely cleared: no trusted workspace evidence catalog was found"
                .to_string(),
        ));
    };
    let (catalog, member_roots) = {
        let Some(_catalog_lock) = WorkspaceCatalogLock::acquire(&workspace_root, false)? else {
            return Err(BuildError::Message(
                "workspace discovery failed and release evidence could not be safely cleared: the workspace catalog lock is unavailable"
                    .to_string(),
            ));
        };
        read_validated_workspace_catalog_locked(&workspace_root)?
    };
    let selected_members = selected_catalog_members(&catalog, selection);
    let mut unsafe_members = Vec::new();
    for member in selected_members {
        let Some(member_root) = member_roots.get(&member.member_key).cloned() else {
            unsafe_members.push(format!(
                "{}: catalog member root is unavailable",
                member.member_key
            ));
            continue;
        };
        let target_dir = project_target_dir(&member_root, target, target_scoped_artifacts);
        let Some(_target_lock) = BuildEvidenceLock::acquire(&target_dir, false)? else {
            continue;
        };
        validate_member_target_dir(&member_root, &target_dir, target, target_scoped_artifacts)?;
        {
            let Some(_catalog_lock) = WorkspaceCatalogLock::acquire(&workspace_root, false)? else {
                unsafe_members.push(format!(
                    "{}: catalog disappeared while the target was locked",
                    member.member_key
                ));
                continue;
            };
            let (current_catalog, _) = read_validated_workspace_catalog_locked(&workspace_root)?;
            let current_member = current_catalog
                .members
                .iter()
                .find(|candidate| candidate.member_key == member.member_key);
            if current_catalog.catalog_generation != catalog.catalog_generation
                || current_member != Some(member)
            {
                unsafe_members.push(format!(
                    "{}: catalog generation changed during cleanup",
                    member.member_key
                ));
                continue;
            }
        }
        let receipt_path = target_dir.join(WORKSPACE_OWNERSHIP_RECEIPT_FILE);
        let Some(receipt) = read_workspace_ownership_receipt(&receipt_path)? else {
            if target_has_release_evidence(&target_dir)? {
                unsafe_members.push(format!(
                    "{}: release evidence has no matching workspace owner receipt",
                    member.member_key
                ));
            }
            continue;
        };
        if !workspace_ownership_receipt_matches(
            &receipt,
            &catalog,
            &workspace_root,
            member,
            &member_root,
            &target_dir,
            target,
            target_scoped_artifacts,
        )? {
            unsafe_members.push(format!(
                "{}: workspace owner receipt or evidence hash does not match",
                member.member_key
            ));
            continue;
        }
        clear_locked_workspace_evidence(&target_dir)?;
    }
    if unsafe_members.is_empty() {
        Ok(())
    } else {
        Err(BuildError::Message(format!(
            "workspace discovery failed and release evidence could not be safely cleared: {}",
            unsafe_members.join("; ")
        )))
    }
}

fn refresh_workspace_evidence_catalog(
    workspace: &WorkspaceGraph,
) -> Result<(WorkspaceEvidenceCatalog, PathBuf), BuildError> {
    let workspace_root = canonical_safe_directory(&workspace.root, "workspace root")?;
    let _catalog_lock = WorkspaceCatalogLock::acquire(&workspace_root, true)?
        .expect("creating workspace evidence state must produce a catalog lock");
    let scope = read_or_create_workspace_scope_locked(&workspace_root)?;
    let default_roots = workspace
        .default_members
        .iter()
        .map(|project| canonical_safe_directory(&project.root, "default workspace member"))
        .collect::<Result<BTreeSet<_>, _>>()?;
    let mut members = Vec::with_capacity(workspace.members.len());
    let mut member_keys = BTreeSet::new();
    for project in &workspace.members {
        let member_root = canonical_safe_directory(&project.root, "workspace member")?;
        if !member_root.starts_with(&workspace_root)
            || crosses_nested_repository_boundary(&workspace_root, &member_root)
        {
            return Err(BuildError::Message(format!(
                "refusing to catalog workspace member outside the canonical workspace repository: {}",
                project.root.display()
            )));
        }
        let relative_root = root_relative_member_path(&workspace_root, &member_root)?;
        if !member_keys.insert(relative_root.clone()) {
            return Err(BuildError::Message(format!(
                "workspace evidence catalog contains duplicate relative root `{relative_root}`"
            )));
        }
        let package_id = project_package_id(project).map_err(BuildError::Message)?;
        let member_key = workspace_member_key(&scope.stable_scope_id, &package_id, &relative_root)?;
        members.push(WorkspaceEvidenceMember {
            package_id,
            package_name: project.name.clone(),
            relative_root,
            member_key,
            default_member: default_roots.contains(&member_root),
        });
    }
    members.sort_by(|left, right| {
        left.relative_root
            .cmp(&right.relative_root)
            .then_with(|| left.package_id.cmp(&right.package_id))
    });
    let catalog_generation = sha256_canonical_json(&WorkspaceCatalogGeneration {
        domain: "nomo-workspace-evidence-catalog-generation-v1",
        members: members.clone(),
    })?;
    let catalog = WorkspaceEvidenceCatalog {
        schema: WORKSPACE_EVIDENCE_CATALOG_SCHEMA,
        stable_scope_id: scope.stable_scope_id,
        catalog_generation,
        workspace_root: workspace_root.to_string_lossy().into_owned(),
        members,
    };
    atomic_write_canonical_json(&workspace_evidence_catalog_path(&workspace_root), &catalog)?;
    Ok((catalog, workspace_root))
}

fn root_relative_member_path(
    workspace_root: &Path,
    member_root: &Path,
) -> Result<String, BuildError> {
    let relative = member_root.strip_prefix(workspace_root).map_err(|_| {
        BuildError::Message(format!(
            "workspace member {} is outside {}",
            member_root.display(),
            workspace_root.display()
        ))
    })?;
    if relative.as_os_str().is_empty() {
        return Ok(".".to_string());
    }
    if relative
        .components()
        .any(|component| !matches!(component, std::path::Component::Normal(_)))
    {
        return Err(BuildError::Message(format!(
            "workspace member has a non-canonical relative path: {}",
            member_root.display()
        )));
    }
    Ok(relative.to_string_lossy().replace('\\', "/"))
}

fn workspace_member_key(
    stable_scope_id: &str,
    package_id: &str,
    relative_root: &str,
) -> Result<String, BuildError> {
    sha256_canonical_json(&WorkspaceMemberKeyInput {
        domain: "nomo-workspace-member-key-v1",
        stable_scope_id: stable_scope_id.to_string(),
        package_id: package_id.to_string(),
        relative_root: relative_root.to_string(),
    })
}

fn selected_catalog_members<'a>(
    catalog: &'a WorkspaceEvidenceCatalog,
    selection: &WorkspaceEvidenceSelection,
) -> Vec<&'a WorkspaceEvidenceMember> {
    let default_members_are_implicit = !catalog.members.iter().any(|member| member.default_member);
    catalog
        .members
        .iter()
        .filter(|member| match selection {
            WorkspaceEvidenceSelection::AllMembers => true,
            WorkspaceEvidenceSelection::DefaultMembers => {
                default_members_are_implicit || member.default_member
            }
            WorkspaceEvidenceSelection::Package(package) => {
                member.package_id == *package || member.package_name == *package
            }
        })
        .collect()
}

fn sha256_canonical_json(value: &impl Serialize) -> Result<String, BuildError> {
    Ok(format!(
        "{:x}",
        Sha256::digest(canonical_json_bytes(value)?)
    ))
}

fn workspace_evidence_state_dir(workspace_root: &Path) -> PathBuf {
    WORKSPACE_EVIDENCE_STATE_COMPONENTS
        .iter()
        .fold(workspace_root.to_path_buf(), |path, component| {
            path.join(component)
        })
}

fn ensure_workspace_evidence_state_dir(workspace_root: &Path) -> Result<PathBuf, BuildError> {
    let mut path = workspace_root.to_path_buf();
    for component in WORKSPACE_EVIDENCE_STATE_COMPONENTS {
        path = ensure_safe_child_directory(&path, component)?;
    }
    Ok(path)
}

fn workspace_scope_marker_path(workspace_root: &Path) -> PathBuf {
    workspace_evidence_state_dir(workspace_root).join("scope-id")
}

fn workspace_evidence_catalog_path(workspace_root: &Path) -> PathBuf {
    workspace_evidence_state_dir(workspace_root).join("catalog.json")
}

fn find_workspace_evidence_catalog_root(
    requested_path: &Path,
) -> Result<Option<PathBuf>, BuildError> {
    let requested_root = requested_workspace_cleanup_root(requested_path)?;
    let mut cursor = canonical_safe_directory(&requested_root, "workspace cleanup root")?;
    loop {
        let catalog_path = workspace_evidence_catalog_path(&cursor);
        match fs::symlink_metadata(&catalog_path) {
            Ok(metadata)
                if metadata.file_type().is_file() && !metadata_is_symlink_or_reparse(&metadata) =>
            {
                return canonical_safe_directory(&cursor, "workspace catalog root").map(Some);
            }
            Ok(_) => {
                return Err(BuildError::Message(format!(
                    "workspace evidence catalog is not a safe regular file: {}",
                    catalog_path.display()
                )));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(BuildError::Message(format!(
                    "failed to inspect workspace evidence catalog {}: {error}",
                    catalog_path.display()
                )));
            }
        }
        if cursor.join(".git").exists() {
            break;
        }
        let Some(parent) = cursor.parent() else {
            break;
        };
        if parent == cursor {
            break;
        }
        cursor = parent.to_path_buf();
    }
    Ok(None)
}

fn read_or_create_workspace_scope_locked(
    workspace_root: &Path,
) -> Result<WorkspaceEvidenceScopeMarker, BuildError> {
    let path = workspace_scope_marker_path(workspace_root);
    match fs::symlink_metadata(&path) {
        Ok(_) => {
            let marker = read_canonical_workspace_state_file::<WorkspaceEvidenceScopeMarker>(
                &path,
                "workspace scope marker",
            )?;
            if marker.schema != WORKSPACE_EVIDENCE_SCOPE_SCHEMA
                || marker.workspace_root != workspace_root.to_string_lossy()
                || !is_sha256_hex(&marker.stable_scope_id)
            {
                return Err(BuildError::Message(format!(
                    "workspace scope marker is invalid: {}",
                    path.display()
                )));
            }
            Ok(marker)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let marker = WorkspaceEvidenceScopeMarker {
                schema: WORKSPACE_EVIDENCE_SCOPE_SCHEMA,
                stable_scope_id: new_workspace_scope_id(workspace_root),
                workspace_root: workspace_root.to_string_lossy().into_owned(),
            };
            atomic_write_canonical_json(&path, &marker)?;
            Ok(marker)
        }
        Err(error) => Err(BuildError::Message(format!(
            "failed to inspect workspace scope marker {}: {error}",
            path.display()
        ))),
    }
}

fn read_validated_workspace_catalog_locked(
    workspace_root: &Path,
) -> Result<(WorkspaceEvidenceCatalog, BTreeMap<String, PathBuf>), BuildError> {
    let scope = read_canonical_workspace_state_file::<WorkspaceEvidenceScopeMarker>(
        &workspace_scope_marker_path(workspace_root),
        "workspace scope marker",
    )?;
    if scope.schema != WORKSPACE_EVIDENCE_SCOPE_SCHEMA
        || scope.workspace_root != workspace_root.to_string_lossy()
        || !is_sha256_hex(&scope.stable_scope_id)
    {
        return Err(BuildError::Message(
            "workspace evidence scope marker is invalid".to_string(),
        ));
    }
    let catalog = read_canonical_workspace_state_file::<WorkspaceEvidenceCatalog>(
        &workspace_evidence_catalog_path(workspace_root),
        "workspace evidence catalog",
    )?;
    if catalog.schema != WORKSPACE_EVIDENCE_CATALOG_SCHEMA
        || catalog.stable_scope_id != scope.stable_scope_id
        || catalog.workspace_root != workspace_root.to_string_lossy()
        || catalog.members.is_empty()
    {
        return Err(BuildError::Message(
            "workspace evidence catalog identity is invalid".to_string(),
        ));
    }
    let mut sorted_members = catalog.members.clone();
    sorted_members.sort_by(|left, right| {
        left.relative_root
            .cmp(&right.relative_root)
            .then_with(|| left.package_id.cmp(&right.package_id))
    });
    if sorted_members != catalog.members {
        return Err(BuildError::Message(
            "workspace evidence catalog members are not canonical".to_string(),
        ));
    }
    let expected_generation = sha256_canonical_json(&WorkspaceCatalogGeneration {
        domain: "nomo-workspace-evidence-catalog-generation-v1",
        members: catalog.members.clone(),
    })?;
    if catalog.catalog_generation != expected_generation {
        return Err(BuildError::Message(
            "workspace evidence catalog generation hash does not match".to_string(),
        ));
    }
    let mut member_roots = BTreeMap::new();
    for member in &catalog.members {
        if member.package_id.is_empty()
            || member.package_name.is_empty()
            || member_roots.contains_key(&member.member_key)
        {
            return Err(BuildError::Message(
                "workspace evidence catalog contains an invalid member".to_string(),
            ));
        }
        let expected_member_key = workspace_member_key(
            &catalog.stable_scope_id,
            &member.package_id,
            &member.relative_root,
        )?;
        if member.member_key != expected_member_key {
            return Err(BuildError::Message(format!(
                "workspace member key does not match `{}`",
                member.relative_root
            )));
        }
        let root = catalog_member_root(workspace_root, &member.relative_root)?;
        member_roots.insert(member.member_key.clone(), root);
    }
    Ok((catalog, member_roots))
}

fn catalog_member_root(workspace_root: &Path, relative_root: &str) -> Result<PathBuf, BuildError> {
    let relative = Path::new(relative_root);
    let member_root = if relative_root == "." {
        workspace_root.to_path_buf()
    } else {
        if relative.is_absolute()
            || relative
                .components()
                .any(|component| !matches!(component, std::path::Component::Normal(_)))
        {
            return Err(BuildError::Message(format!(
                "workspace catalog path is not a canonical relative path: {relative_root}"
            )));
        }
        workspace_root.join(relative)
    };
    let canonical_member = canonical_safe_directory(&member_root, "workspace catalog member")?;
    if !canonical_member.starts_with(workspace_root)
        || root_relative_member_path(workspace_root, &canonical_member)? != relative_root
        || crosses_nested_repository_boundary(workspace_root, &canonical_member)
    {
        return Err(BuildError::Message(format!(
            "workspace catalog member escapes its canonical repository boundary: {relative_root}"
        )));
    }
    Ok(canonical_member)
}

fn read_canonical_workspace_state_file<T: serde::de::DeserializeOwned + Serialize>(
    path: &Path,
    label: &str,
) -> Result<T, BuildError> {
    reject_existing_symlink_or_reparse_components(path)?;
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        BuildError::Message(format!(
            "failed to inspect {label} {}: {error}",
            path.display()
        ))
    })?;
    if !metadata.file_type().is_file() || metadata_is_symlink_or_reparse(&metadata) {
        return Err(BuildError::Message(format!(
            "{label} is not a safe regular file: {}",
            path.display()
        )));
    }
    let bytes = fs::read(path).map_err(|error| {
        BuildError::Message(format!(
            "failed to read {label} {}: {error}",
            path.display()
        ))
    })?;
    let value = serde_json::from_slice::<T>(&bytes).map_err(|error| {
        BuildError::Message(format!(
            "{label} has an unknown or truncated schema at {}: {error}",
            path.display()
        ))
    })?;
    if canonical_json_bytes(&value)? != bytes {
        return Err(BuildError::Message(format!(
            "{label} is not canonical JSON: {}",
            path.display()
        )));
    }
    Ok(value)
}

fn new_workspace_scope_id(workspace_root: &Path) -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static SCOPE_SEQUENCE: AtomicU64 = AtomicU64::new(0);
    let mut hasher = Sha256::new();
    hasher.update(b"nomo-workspace-stable-scope-v1");
    hasher.update(workspace_root.as_os_str().as_encoded_bytes());
    hasher.update(std::process::id().to_be_bytes());
    hasher.update(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
            .to_be_bytes(),
    );
    hasher.update(SCOPE_SEQUENCE.fetch_add(1, Ordering::Relaxed).to_be_bytes());
    format!("{:x}", hasher.finalize())
}

fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn target_has_release_evidence(target_dir: &Path) -> Result<bool, BuildError> {
    for name in ["nomo-build-metadata.json", "release-provenance.json"] {
        let path = target_dir.join(name);
        match fs::symlink_metadata(&path) {
            Ok(metadata)
                if metadata.file_type().is_file() && !metadata_is_symlink_or_reparse(&metadata) =>
            {
                reject_existing_symlink_or_reparse_components(&path)?;
                return Ok(true);
            }
            Ok(_) => {
                return Err(BuildError::Message(format!(
                    "release evidence is not a safe regular file: {}",
                    path.display()
                )));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(BuildError::Message(format!(
                    "failed to inspect release evidence {}: {error}",
                    path.display()
                )));
            }
        }
    }
    Ok(false)
}

fn safe_required_file_record(path: &Path, label: &str) -> Result<FileRecord, BuildError> {
    reject_existing_symlink_or_reparse_components(path)?;
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        BuildError::Message(format!(
            "failed to inspect {label} {}: {error}",
            path.display()
        ))
    })?;
    if !metadata.file_type().is_file() || metadata_is_symlink_or_reparse(&metadata) {
        return Err(BuildError::Message(format!(
            "{label} is not a safe regular file: {}",
            path.display()
        )));
    }
    FileRecord::from_path(path)
}

fn read_workspace_ownership_receipt(
    path: &Path,
) -> Result<Option<WorkspaceOwnershipReceipt>, BuildError> {
    match fs::symlink_metadata(path) {
        Ok(metadata)
            if metadata.file_type().is_file() && !metadata_is_symlink_or_reparse(&metadata) => {}
        Ok(_) => {
            return Err(BuildError::Message(format!(
                "workspace ownership receipt is not a safe regular file: {}",
                path.display()
            )));
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(BuildError::Message(format!(
                "failed to inspect workspace ownership receipt {}: {error}",
                path.display()
            )));
        }
    }
    reject_existing_symlink_or_reparse_components(path)?;
    let bytes = fs::read(path).map_err(|error| {
        BuildError::Message(format!(
            "failed to read workspace ownership receipt {}: {error}",
            path.display()
        ))
    })?;
    let receipt = serde_json::from_slice::<WorkspaceOwnershipReceipt>(&bytes).map_err(|error| {
        BuildError::Message(format!(
            "workspace ownership receipt has an unknown or truncated schema at {}: {error}",
            path.display()
        ))
    })?;
    if canonical_json_bytes(&receipt)? != bytes {
        return Err(BuildError::Message(format!(
            "workspace ownership receipt is not canonical: {}",
            path.display()
        )));
    }
    Ok(Some(receipt))
}

#[allow(clippy::too_many_arguments)]
fn workspace_ownership_receipt_matches(
    receipt: &WorkspaceOwnershipReceipt,
    catalog: &WorkspaceEvidenceCatalog,
    workspace_root: &Path,
    member: &WorkspaceEvidenceMember,
    member_root: &Path,
    target_dir: &Path,
    target: &TargetTriple,
    target_scoped_artifacts: bool,
) -> Result<bool, BuildError> {
    let metadata = safe_required_file_record(
        &target_dir.join("nomo-build-metadata.json"),
        "build metadata",
    )?;
    let provenance = safe_required_file_record(
        &target_dir.join("release-provenance.json"),
        "release provenance",
    )?;
    if receipt.schema != WORKSPACE_OWNERSHIP_RECEIPT_SCHEMA
        || receipt.stable_scope_id != catalog.stable_scope_id
        || receipt.workspace_root != workspace_root.to_string_lossy()
        || receipt.member_key != member.member_key
        || receipt.member_root != member_root.to_string_lossy()
        || receipt.package_id != member.package_id
        || receipt.package_name != member.package_name
        || receipt.selected_profile != BuildProfile::Release
        || receipt.target_triple != target.to_string()
        || receipt.target_scoped_artifacts != target_scoped_artifacts
        || receipt.metadata_sha256 != metadata.sha256
        || receipt.provenance_sha256 != provenance.sha256
    {
        return Ok(false);
    }
    Ok(true)
}

fn clear_locked_workspace_evidence(target_dir: &Path) -> Result<(), BuildError> {
    clear_release_evidence_paths(
        &target_dir.join("release-provenance.json"),
        &target_dir.join(WORKSPACE_OWNERSHIP_RECEIPT_FILE),
        &target_dir.join("nomo-build-metadata.json"),
    )
}

fn project_target_dir(
    member_root: &Path,
    target: &TargetTriple,
    target_scoped_artifacts: bool,
) -> PathBuf {
    if target_scoped_artifacts {
        member_root.join("build").join(target.to_string())
    } else {
        member_root.join("build")
    }
}

fn validate_member_target_dir(
    member_root: &Path,
    target_dir: &Path,
    target: &TargetTriple,
    target_scoped_artifacts: bool,
) -> Result<(), BuildError> {
    let expected = project_target_dir(member_root, target, target_scoped_artifacts);
    if target_dir != expected {
        return Err(BuildError::Message(format!(
            "workspace member target path does not match its canonical build layout: {}",
            target_dir.display()
        )));
    }
    reject_existing_symlink_or_reparse_components(target_dir)?;
    let canonical_target = fs::canonicalize(target_dir).map_err(|error| {
        BuildError::Message(format!(
            "failed to resolve workspace member target {}: {error}",
            target_dir.display()
        ))
    })?;
    let canonical_build = fs::canonicalize(member_root.join("build")).map_err(|error| {
        BuildError::Message(format!(
            "failed to resolve workspace member build directory {}: {error}",
            member_root.join("build").display()
        ))
    })?;
    if !canonical_target.starts_with(&canonical_build) {
        return Err(BuildError::Message(format!(
            "workspace member target escapes its canonical build directory: {}",
            target_dir.display()
        )));
    }
    Ok(())
}

fn workspace_release_owner_context(
    project: &Project,
    target: &TargetTriple,
) -> Result<Option<WorkspaceReleaseOwnerContext>, BuildError> {
    let Some(workspace_root) = project.workspace_root.as_deref() else {
        return Ok(None);
    };
    let workspace = match discover_workspace_for_target(workspace_root, target) {
        Ok(workspace) => workspace,
        Err(_) => return Ok(None),
    };
    let (catalog, canonical_workspace_root) = refresh_workspace_evidence_catalog(&workspace)?;
    let canonical_member_root = canonical_safe_directory(&project.root, "workspace project")?;
    let relative_root =
        root_relative_member_path(&canonical_workspace_root, &canonical_member_root)?;
    let package_id = project_package_id(project).map_err(BuildError::Message)?;
    let Some(member) = catalog.members.iter().find(|member| {
        member.relative_root == relative_root
            && member.package_id == package_id
            && member.package_name == project.name
    }) else {
        return Ok(None);
    };
    Ok(Some(WorkspaceReleaseOwnerContext {
        stable_scope_id: catalog.stable_scope_id,
        workspace_root: canonical_workspace_root,
        member_key: member.member_key.clone(),
        member_root: canonical_member_root,
        package_id,
        package_name: project.name.clone(),
    }))
}

fn workspace_ownership_receipt(
    owner: &WorkspaceReleaseOwnerContext,
    target: &TargetTriple,
    target_scoped_artifacts: bool,
    metadata: &FileRecord,
    provenance: &FileRecord,
) -> WorkspaceOwnershipReceipt {
    WorkspaceOwnershipReceipt {
        schema: WORKSPACE_OWNERSHIP_RECEIPT_SCHEMA,
        stable_scope_id: owner.stable_scope_id.clone(),
        workspace_root: owner.workspace_root.to_string_lossy().into_owned(),
        member_key: owner.member_key.clone(),
        member_root: owner.member_root.to_string_lossy().into_owned(),
        package_id: owner.package_id.clone(),
        package_name: owner.package_name.clone(),
        selected_profile: BuildProfile::Release,
        target_triple: target.to_string(),
        target_scoped_artifacts,
        metadata_sha256: metadata.sha256.clone(),
        provenance_sha256: provenance.sha256.clone(),
    }
}

fn canonical_safe_directory(path: &Path, label: &str) -> Result<PathBuf, BuildError> {
    let absolute = absolute_build_path(path)?;
    let metadata = fs::symlink_metadata(&absolute).map_err(|error| {
        BuildError::Message(format!(
            "failed to inspect {label} {}: {error}",
            absolute.display()
        ))
    })?;
    if !metadata.file_type().is_dir() || metadata_is_symlink_or_reparse(&metadata) {
        return Err(BuildError::Message(format!(
            "{label} is not a safe directory: {}",
            absolute.display()
        )));
    }
    let canonical = fs::canonicalize(&absolute).map_err(|error| {
        BuildError::Message(format!(
            "failed to resolve {label} {}: {error}",
            absolute.display()
        ))
    })?;
    reject_existing_symlink_or_reparse_components(&canonical)?;
    Ok(canonical)
}

fn canonical_build_target_dir(target_dir: &Path) -> Result<PathBuf, BuildError> {
    let absolute = absolute_build_path(target_dir)?;
    let (project_root, target_suffix) =
        if absolute.file_name() == Some(std::ffi::OsStr::new("build")) {
            let project_root = absolute.parent().ok_or_else(|| {
                BuildError::Message(format!(
                    "build target has no project root: {}",
                    absolute.display()
                ))
            })?;
            (project_root, PathBuf::from("build"))
        } else {
            let build_dir = absolute.parent().ok_or_else(|| {
                BuildError::Message(format!(
                    "target-scoped build output has no build directory: {}",
                    absolute.display()
                ))
            })?;
            if build_dir.file_name() != Some(std::ffi::OsStr::new("build")) {
                return Err(BuildError::Message(format!(
                    "build target does not use the canonical build layout: {}",
                    absolute.display()
                )));
            }
            let project_root = build_dir.parent().ok_or_else(|| {
                BuildError::Message(format!(
                    "target-scoped build output has no project root: {}",
                    absolute.display()
                ))
            })?;
            let target_name = absolute.file_name().ok_or_else(|| {
                BuildError::Message(format!("build target has no name: {}", absolute.display()))
            })?;
            (project_root, Path::new("build").join(target_name))
        };
    let canonical_root = canonical_safe_directory(project_root, "build project root")?;
    let canonical_target = canonical_root.join(target_suffix);
    reject_existing_symlink_or_reparse_components(&canonical_target)?;
    Ok(canonical_target)
}

fn ensure_safe_child_directory(parent: &Path, name: &str) -> Result<PathBuf, BuildError> {
    reject_existing_symlink_or_reparse_components(parent)?;
    let parent_metadata = fs::symlink_metadata(parent).map_err(|error| {
        BuildError::Message(format!(
            "failed to inspect directory {}: {error}",
            parent.display()
        ))
    })?;
    if !parent_metadata.file_type().is_dir() || metadata_is_symlink_or_reparse(&parent_metadata) {
        return Err(BuildError::Message(format!(
            "parent path is not a safe directory: {}",
            parent.display()
        )));
    }
    let child = parent.join(name);
    match fs::symlink_metadata(&child) {
        Ok(metadata)
            if metadata.file_type().is_dir() && !metadata_is_symlink_or_reparse(&metadata) => {}
        Ok(_) => {
            return Err(BuildError::Message(format!(
                "state path is not a safe directory: {}",
                child.display()
            )));
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            fs::create_dir(&child).map_err(|error| {
                BuildError::Message(format!(
                    "failed to create state directory {}: {error}",
                    child.display()
                ))
            })?;
        }
        Err(error) => {
            return Err(BuildError::Message(format!(
                "failed to inspect state directory {}: {error}",
                child.display()
            )));
        }
    }
    reject_existing_symlink_or_reparse_components(&child)?;
    Ok(child)
}

fn reject_existing_symlink_or_reparse_components(path: &Path) -> Result<(), BuildError> {
    visit_complete_path_components(path, |cursor| {
        match fs::symlink_metadata(cursor) {
            Ok(metadata) if metadata_is_symlink_or_reparse(&metadata) => {
                return Err(BuildError::Message(format!(
                    "refusing symlink or reparse-point path component: {}",
                    cursor.display()
                )));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(BuildError::Message(format!(
                    "failed to inspect path component {}: {error}",
                    cursor.display()
                )));
            }
        }
        Ok(())
    })
}

fn visit_complete_path_components(
    path: &Path,
    mut visit: impl FnMut(&Path) -> Result<(), BuildError>,
) -> Result<(), BuildError> {
    let mut cursor = PathBuf::new();
    for component in path.components() {
        cursor.push(component.as_os_str());
        // A Windows verbatim/drive/UNC prefix is not a complete path that can
        // be opened on its own. The following root component completes the
        // anchor, after which every existing component is inspected.
        if matches!(component, std::path::Component::Prefix(_)) {
            continue;
        }
        visit(&cursor)?;
    }
    Ok(())
}

#[cfg(not(windows))]
fn metadata_is_symlink_or_reparse(metadata: &fs::Metadata) -> bool {
    metadata.file_type().is_symlink()
}

#[cfg(windows)]
fn metadata_is_symlink_or_reparse(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;

    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    metadata.file_type().is_symlink()
        || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

fn open_build_lock(path: &Path) -> Result<File, BuildError> {
    let mut options = OpenOptions::new();
    options.create(true).read(true).write(true).truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;

        #[cfg(any(target_os = "linux", target_os = "android"))]
        const O_NOFOLLOW: i32 = 0x20000;
        #[cfg(not(any(target_os = "linux", target_os = "android")))]
        const O_NOFOLLOW: i32 = 0x100;
        options.custom_flags(O_NOFOLLOW);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;

        const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
        options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    }
    let file = options.open(path).map_err(|error| {
        BuildError::Message(format!(
            "failed to safely open lock {}: {error}",
            path.display()
        ))
    })?;
    let metadata = file.metadata().map_err(|error| {
        BuildError::Message(format!(
            "failed to inspect safely opened lock {}: {error}",
            path.display()
        ))
    })?;
    if !metadata.file_type().is_file() || metadata_is_symlink_or_reparse(&metadata) {
        return Err(BuildError::Message(format!(
            "lock is not a safe regular file: {}",
            path.display()
        )));
    }
    Ok(file)
}

fn crosses_nested_repository_boundary(workspace_root: &Path, member_root: &Path) -> bool {
    let Ok(relative) = member_root.strip_prefix(workspace_root) else {
        return true;
    };
    let mut cursor = workspace_root.to_path_buf();
    for component in relative.components() {
        cursor.push(component);
        if cursor.join(".git").exists() {
            return true;
        }
    }
    false
}

fn requested_workspace_cleanup_root(requested_path: &Path) -> Result<PathBuf, BuildError> {
    let requested_path = absolute_build_path(requested_path)?;
    if requested_path
        .extension()
        .and_then(|extension| extension.to_str())
        == Some("nomo")
    {
        requested_path
            .parent()
            .map(Path::to_path_buf)
            .ok_or_else(|| {
                BuildError::Message(format!(
                    "workspace source has no parent: {}",
                    requested_path.display()
                ))
            })
    } else {
        Ok(requested_path)
    }
}

fn requested_build_root(requested_path: &Path) -> Result<PathBuf, BuildError> {
    let requested_path = absolute_build_path(requested_path)?;
    let mut cursor = if requested_path.is_dir() {
        requested_path.clone()
    } else {
        requested_path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| requested_path.clone())
    };
    loop {
        if cursor.join("nomo.toml").exists() {
            return Ok(cursor);
        }
        let Some(parent) = cursor.parent() else {
            break;
        };
        if parent == cursor {
            break;
        }
        cursor = parent.to_path_buf();
    }
    if requested_path
        .extension()
        .and_then(|extension| extension.to_str())
        == Some("nomo")
    {
        return requested_path
            .parent()
            .map(Path::to_path_buf)
            .ok_or_else(|| {
                BuildError::Message(format!(
                    "standalone source has no parent: {}",
                    requested_path.display()
                ))
            });
    }
    Ok(requested_path)
}

fn build_project_impl(
    project: &Project,
    emit_c_only: bool,
    options: DependencyResolutionOptions,
    target: &TargetTriple,
    target_scoped_artifacts: bool,
    profile: BuildProfile,
) -> Result<PathBuf, BuildError> {
    let workspace_owner = if profile == BuildProfile::Release {
        workspace_release_owner_context(project, target)?
    } else {
        None
    };
    let target_dir = if target_scoped_artifacts {
        project.root.join("build").join(target.to_string())
    } else {
        project.root.join("build")
    };
    let provenance_path = target_dir.join("release-provenance.json");
    let metadata_path = target_dir.join("nomo-build-metadata.json");
    let producer_executable = producer_executable_identity()?;
    let context = project_module_context_for_target_with_options(project, options, target)
        .map_err(BuildError::Message)?;
    let ffi_link_metadata =
        project_ffi_link_metadata_for_target_with_options(project, options, target)
            .map_err(BuildError::Message)?;
    let cache_root = project
        .workspace_root
        .as_deref()
        .unwrap_or(project.root.as_path());
    let cache = PersistentQueryCache::at_root(cache_root);
    let cache_configuration =
        codegen_cache_configuration(profile, PASS_PIPELINE_VERSION, &producer_executable);
    let cache_key = project_query_key(
        project,
        &context.external_modules,
        &[],
        target,
        "codegen-c",
        format!(
            "{}:{}:{}",
            project.name,
            project.main.display(),
            cache_configuration,
        ),
    );
    let c = match cache.get::<String>(&cache_key) {
        Some(cached) => cached,
        None => {
            let generated = compile_source_to_c_with_module_identity_for_target(
                &project.main,
                &context.local_source_root,
                &context.local_identity,
                &context.external_import_roots,
                &context.external_modules,
                target,
            )
            .map_err(BuildError::Diagnostic)?;
            let _ = cache.insert(&cache_key, &generated);
            generated
        }
    };
    let evidence = BuildEvidenceTransaction::acquire(&target_dir)?;
    let c_dir = target_dir.join("c");
    let object_dir = target_dir.join("obj");
    let bin_dir = target_dir.join("bin");
    fs::create_dir_all(&c_dir).map_err(|err| BuildError::Message(err.to_string()))?;
    fs::create_dir_all(&object_dir).map_err(|err| BuildError::Message(err.to_string()))?;
    fs::create_dir_all(&bin_dir).map_err(|err| BuildError::Message(err.to_string()))?;

    let c_path = c_dir.join("main.c");
    let uses_native_tasks = generated_c_uses_native_tasks(&c);
    let uses_bundled_sqlite = generated_c_uses_bundled_sqlite(&c);
    let uses_math = generated_c_uses_math(&c);
    let uses_dynamic_loader = generated_c_uses_dynamic_loader(&c);
    let uses_winsock = generated_c_uses_winsock(&c);
    fs::write(&c_path, c).map_err(|err| BuildError::Message(err.to_string()))?;
    materialize_bundled_sqlite(&c_dir, uses_bundled_sqlite)
        .map_err(|err| BuildError::Message(err.to_string()))?;
    let generated_c = FileRecord::from_path(&c_path)?;
    if emit_c_only {
        let metadata = BuildMetadata::new(
            profile,
            target.to_string(),
            cache_key.clone(),
            producer_executable.clone(),
            None,
            Vec::new(),
            None,
            None,
            generated_c,
            None,
            None,
        )?;
        write_build_metadata(&metadata_path, &metadata)?;
        evidence.commit();
        return Ok(c_path);
    }

    let host = TargetTriple::host().map_err(BuildError::Message)?;
    let toolchain = c_toolchain_for_profile(target, &host, profile)?;
    let bin_name = if target.operating_system().as_str() == "windows" {
        format!("{}.exe", project.name)
    } else {
        project.name.clone()
    };
    let bin_path = bin_dir.join(bin_name);
    let compiler_path = resolve_executable(&toolchain.program, profile)?;
    let compiler = inspect_compiler(
        &compiler_path,
        &toolchain.args,
        profile == BuildProfile::Release,
    )?;
    if profile == BuildProfile::Release {
        let (objects, compile_commands) = compile_release_translation_units(
            &compiler_path,
            &toolchain,
            &c_path,
            &c_dir,
            &object_dir,
            &ffi_link_metadata,
            uses_bundled_sqlite,
        )?;
        remove_stale_file(&bin_path)?;
        let link_argv = release_link_argv(
            &compiler_path,
            &toolchain,
            &objects,
            &bin_path,
            &ffi_link_metadata,
            target,
            uses_native_tasks,
            uses_bundled_sqlite,
            uses_math,
            uses_dynamic_loader,
            uses_winsock,
        )?;
        let link_command = run_recorded_command(link_argv, BuildProfile::Release)?;
        let object_records = objects
            .iter()
            .map(|path| FileRecord::from_path(path))
            .collect::<Result<Vec<_>, _>>()?;
        let binary = FileRecord::from_path(&bin_path)?;
        let provenance = ReleaseBackendProvenance::new(
            compiler.clone(),
            object_records,
            compile_commands.clone(),
            link_command.clone(),
            generated_c.clone(),
            binary.clone(),
        );
        let provenance_record = FileRecord::for_canonical_json(&provenance_path, &provenance)?;
        let metadata = BuildMetadata::new(
            profile,
            target.to_string(),
            cache_key.clone(),
            producer_executable.clone(),
            Some(compiler),
            compile_commands,
            Some(link_command),
            None,
            generated_c,
            Some(binary),
            Some(provenance_record.clone()),
        )?;
        let metadata_record = FileRecord::for_canonical_json(&metadata_path, &metadata)?;
        let owner_receipt = workspace_owner.as_ref().map(|owner| {
            workspace_ownership_receipt(
                owner,
                target,
                target_scoped_artifacts,
                &metadata_record,
                &provenance_record,
            )
        });
        publish_release_evidence(
            &metadata_path,
            &metadata,
            owner_receipt.as_ref(),
            &provenance_path,
            &provenance,
            &provenance_record,
        )?;
    } else {
        let mut command = Command::new(&compiler_path);
        command.args(&toolchain.args);
        configure_c_compile_command(
            &mut command,
            &c_path,
            &bin_path,
            &ffi_link_metadata,
            target,
            uses_native_tasks,
            uses_bundled_sqlite,
        );
        let combined_command = run_recorded_command(command_argv(&command), BuildProfile::Debug)?;
        let binary = FileRecord::from_path(&bin_path)?;
        let metadata = BuildMetadata::new(
            profile,
            target.to_string(),
            cache_key,
            producer_executable,
            Some(compiler),
            Vec::new(),
            None,
            Some(combined_command),
            generated_c,
            Some(binary),
            None,
        )?;
        write_build_metadata(&metadata_path, &metadata)?;
    }
    evidence.commit();
    Ok(bin_path)
}

fn standalone_codegen_query_key(
    source: &Path,
    target: &TargetTriple,
    profile: BuildProfile,
    mode: &str,
    producer_executable: &ProducerExecutableIdentity,
) -> Result<QueryKey, BuildError> {
    let source_path = absolute_build_path(source)?;
    let source_bytes = fs::read(&source_path).map_err(|error| {
        BuildError::Message(format!(
            "failed to fingerprint standalone source {}: {error}",
            source_path.display()
        ))
    })?;
    let configuration =
        codegen_cache_configuration(profile, PASS_PIPELINE_VERSION, producer_executable);
    Ok(QueryKey::new(
        target.to_string(),
        "codegen-c",
        format!(
            "standalone-{mode}:{}:{configuration}",
            source_path.display()
        ),
        ContentFingerprint::of_bytes(&source_bytes),
    ))
}

fn c_toolchain_for_profile(
    target: &TargetTriple,
    host: &TargetTriple,
    profile: BuildProfile,
) -> Result<CToolchain, BuildError> {
    let mut toolchain = target.c_toolchain_from(host).map_err(BuildError::Message)?;
    if profile == BuildProfile::Release
        && (target == host
            || (target.operating_system().as_str() == "darwin"
                && host.operating_system().as_str() == "darwin"))
    {
        toolchain.program = "clang".to_string();
    }
    Ok(toolchain)
}

pub(super) fn compile_release_translation_units(
    compiler_path: &Path,
    toolchain: &CToolchain,
    main_c_path: &Path,
    c_dir: &Path,
    object_dir: &Path,
    ffi_link_metadata: &FfiLinkMetadata,
    uses_bundled_sqlite: bool,
) -> Result<(Vec<PathBuf>, Vec<super::build_metadata::CommandRecord>), BuildError> {
    let mut sources = vec![main_c_path.to_path_buf()];
    if uses_bundled_sqlite {
        sources.push(c_dir.join("sqlite3.c"));
    }
    sources.extend(ffi_link_metadata.sources.iter().cloned());
    let mut objects = Vec::with_capacity(sources.len());
    let mut commands = Vec::with_capacity(sources.len());
    for (index, source) in sources.iter().enumerate() {
        let stem = source
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("translation-unit");
        let object = object_dir.join(format!("{index}-{stem}.o"));
        remove_stale_file(&object)?;
        let argv = release_compile_argv(
            compiler_path,
            toolchain,
            source,
            &object,
            uses_bundled_sqlite,
        )?;
        commands.push(run_recorded_command(argv, BuildProfile::Release)?);
        objects.push(object);
    }
    Ok((objects, commands))
}

pub(super) fn release_compile_argv(
    compiler_path: &Path,
    toolchain: &CToolchain,
    source: &Path,
    object: &Path,
    uses_bundled_sqlite: bool,
) -> Result<Vec<String>, BuildError> {
    let mut argv = vec![compiler_path.to_string_lossy().into_owned()];
    argv.extend(toolchain.args.iter().cloned());
    argv.extend(
        release_driver_config_flags_for_compiler(compiler_path)
            .iter()
            .map(|flag| flag.to_string()),
    );
    argv.extend(release_c_flags().iter().map(|flag| flag.to_string()));
    if uses_bundled_sqlite {
        argv.extend(
            nomo_codegen_c::BUNDLED_SQLITE_COMPILE_OPTIONS
                .iter()
                .map(|option| format!("-D{option}")),
        );
    }
    argv.extend([
        "-c".to_string(),
        absolute_build_path(source)?.to_string_lossy().into_owned(),
        "-o".to_string(),
        absolute_build_path(object)?.to_string_lossy().into_owned(),
    ]);
    Ok(argv)
}

pub(super) fn release_link_argv(
    compiler_path: &Path,
    toolchain: &CToolchain,
    objects: &[PathBuf],
    bin_path: &Path,
    ffi_link_metadata: &FfiLinkMetadata,
    target: &TargetTriple,
    uses_native_tasks: bool,
    uses_bundled_sqlite: bool,
    uses_math: bool,
    uses_dynamic_loader: bool,
    uses_winsock: bool,
) -> Result<Vec<String>, BuildError> {
    let mut argv = vec![compiler_path.to_string_lossy().into_owned()];
    argv.extend(toolchain.args.iter().cloned());
    argv.extend(
        release_driver_config_flags_for_compiler(compiler_path)
            .iter()
            .map(|flag| flag.to_string()),
    );
    for path in objects {
        argv.push(absolute_build_path(path)?.to_string_lossy().into_owned());
    }
    argv.extend([
        "-o".to_string(),
        absolute_build_path(bin_path)?
            .to_string_lossy()
            .into_owned(),
    ]);
    for path in &ffi_link_metadata.library_paths {
        argv.push(format!("-L{}", path.display()));
    }
    for library in &ffi_link_metadata.libraries {
        argv.push(format!("-l{library}"));
    }
    for framework in &ffi_link_metadata.frameworks {
        argv.extend(["-framework".to_string(), framework.clone()]);
    }
    argv.extend(ffi_link_metadata.link_args.iter().cloned());
    if uses_dynamic_loader && target.operating_system().as_str() == "linux" {
        argv.push("-ldl".to_string());
    }
    if uses_winsock && target.operating_system().as_str() == "windows" {
        argv.push("-lws2_32".to_string());
    }
    if (uses_native_tasks || uses_bundled_sqlite) && target.operating_system().as_str() != "windows"
    {
        argv.push("-pthread".to_string());
    }
    if uses_math && target.operating_system().as_str() != "windows" {
        argv.push("-lm".to_string());
    }
    Ok(argv)
}

fn command_argv(command: &Command) -> Vec<String> {
    std::iter::once(command.get_program())
        .chain(command.get_args())
        .map(|part| part.to_string_lossy().into_owned())
        .collect()
}

fn absolute_build_path(path: &Path) -> Result<PathBuf, BuildError> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        Ok(std::env::current_dir()
            .map_err(|error| BuildError::Message(error.to_string()))?
            .join(path))
    }
}

pub fn clean_project(project: &Project) -> Result<PathBuf, String> {
    let build_dir = project.root.join("build");
    if build_dir.exists() {
        fs::remove_dir_all(&build_dir).map_err(|err| err.to_string())?;
    }
    Ok(build_dir)
}

pub(super) fn configure_c_compile_command(
    command: &mut Command,
    c_path: &Path,
    bin_path: &Path,
    ffi_link_metadata: &FfiLinkMetadata,
    target: &TargetTriple,
    uses_native_tasks: bool,
    uses_bundled_sqlite: bool,
) {
    command.arg("-std=c99");
    if uses_bundled_sqlite {
        for option in nomo_codegen_c::BUNDLED_SQLITE_COMPILE_OPTIONS {
            command.arg(format!("-D{option}"));
        }
    }
    command.arg(c_path);
    if uses_bundled_sqlite {
        command.arg(c_path.with_file_name("sqlite3.c"));
    }
    for source in &ffi_link_metadata.sources {
        command.arg(source);
    }
    for path in &ffi_link_metadata.library_paths {
        command.arg(format!("-L{}", path.display()));
    }
    for library in &ffi_link_metadata.libraries {
        command.arg(format!("-l{library}"));
    }
    for framework in &ffi_link_metadata.frameworks {
        command.arg("-framework").arg(framework);
    }
    for arg in &ffi_link_metadata.link_args {
        command.arg(arg);
    }
    if target.operating_system().as_str() == "linux" {
        command.arg("-ldl");
    }
    if target.operating_system().as_str() == "windows" {
        command.arg("-lws2_32");
    }
    if (uses_native_tasks || uses_bundled_sqlite) && target.operating_system().as_str() != "windows"
    {
        command.arg("-pthread");
    }
    if target.operating_system().as_str() != "windows" {
        command.arg("-lm");
    }
    command.arg("-o").arg(bin_path);
}

pub(super) fn generated_c_uses_native_tasks(source: &str) -> bool {
    source.contains("#define NOMO_TASK_MAX_LIVE")
        || source.contains("#define NOMO_ASYNC_BLOCKING_POOL_MAX_THREADS")
}

pub(super) fn generated_c_uses_bundled_sqlite(source: &str) -> bool {
    source.contains("#define NOMO_SQLITE_MAX_DATABASES")
}

pub(super) fn generated_c_uses_math(source: &str) -> bool {
    c_calls_any_symbol(
        source,
        &[
            "sqrt", "sqrtf", "pow", "powf", "fmod", "fmodf", "floor", "floorf", "ceil", "ceilf",
            "round", "roundf", "sin", "sinf", "cos", "cosf", "tan", "tanf", "exp", "expf", "log",
            "logf",
        ],
    )
}

fn c_calls_any_symbol(source: &str, symbols: &[&str]) -> bool {
    #[derive(Clone, Copy)]
    enum State {
        Code,
        LineComment,
        BlockComment,
        String,
        Character,
    }

    let bytes = source.as_bytes();
    let mut state = State::Code;
    let mut index = 0;
    while index < bytes.len() {
        match state {
            State::Code if bytes[index..].starts_with(b"//") => {
                state = State::LineComment;
                index += 2;
            }
            State::Code if bytes[index..].starts_with(b"/*") => {
                state = State::BlockComment;
                index += 2;
            }
            State::Code if bytes[index] == b'"' => {
                state = State::String;
                index += 1;
            }
            State::Code if bytes[index] == b'\'' => {
                state = State::Character;
                index += 1;
            }
            State::Code if bytes[index].is_ascii_alphabetic() || bytes[index] == b'_' => {
                let start = index;
                index += 1;
                while index < bytes.len()
                    && (bytes[index].is_ascii_alphanumeric() || bytes[index] == b'_')
                {
                    index += 1;
                }
                let identifier = &source[start..index];
                index = skip_c_whitespace_and_comments(bytes, index);
                if bytes.get(index) == Some(&b'(') && symbols.contains(&identifier) {
                    return true;
                }
            }
            State::LineComment if bytes[index] == b'\n' => {
                state = State::Code;
                index += 1;
            }
            State::BlockComment if bytes[index..].starts_with(b"*/") => {
                state = State::Code;
                index += 2;
            }
            State::String | State::Character if bytes[index] == b'\\' => {
                index = (index + 2).min(bytes.len());
            }
            State::String if bytes[index] == b'"' => {
                state = State::Code;
                index += 1;
            }
            State::Character if bytes[index] == b'\'' => {
                state = State::Code;
                index += 1;
            }
            _ => index += 1,
        }
    }
    false
}

fn skip_c_whitespace_and_comments(bytes: &[u8], mut index: usize) -> usize {
    loop {
        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        if bytes[index..].starts_with(b"//") {
            index += 2;
            while index < bytes.len() && bytes[index] != b'\n' {
                index += 1;
            }
            continue;
        }
        if bytes[index..].starts_with(b"/*") {
            index += 2;
            while index < bytes.len() && !bytes[index..].starts_with(b"*/") {
                index += 1;
            }
            index = (index + 2).min(bytes.len());
            continue;
        }
        return index;
    }
}

pub(super) fn generated_c_uses_dynamic_loader(source: &str) -> bool {
    source.contains("dlopen(") || source.contains("dlsym(") || source.contains("dlclose(")
}

pub(super) fn generated_c_uses_winsock(source: &str) -> bool {
    source.contains("WSAStartup(")
        || source.contains("socket(")
        || source.contains("getaddrinfo(")
        || source.contains("WSAPoll(")
}

pub(super) fn materialize_bundled_sqlite(
    c_dir: &Path,
    enabled: bool,
) -> Result<(), std::io::Error> {
    if !enabled {
        return Ok(());
    }
    verify_bundled_sqlite_source(
        nomo_codegen_c::BUNDLED_SQLITE3_C,
        nomo_codegen_c::BUNDLED_SQLITE3_C_SHA256,
        "sqlite3.c",
    )?;
    verify_bundled_sqlite_source(
        nomo_codegen_c::BUNDLED_SQLITE3_H,
        nomo_codegen_c::BUNDLED_SQLITE3_H_SHA256,
        "sqlite3.h",
    )?;
    let sqlite_c_path = c_dir.join("sqlite3.c");
    let sqlite_h_path = c_dir.join("sqlite3.h");
    fs::write(&sqlite_c_path, nomo_codegen_c::BUNDLED_SQLITE3_C)?;
    fs::write(&sqlite_h_path, nomo_codegen_c::BUNDLED_SQLITE3_H)?;
    fs::write(
        c_dir.join("sqlite3-SOURCE.md"),
        nomo_codegen_c::BUNDLED_SQLITE_SOURCE,
    )?;
    verify_bundled_sqlite_source(
        &fs::read(sqlite_c_path)?,
        nomo_codegen_c::BUNDLED_SQLITE3_C_SHA256,
        "materialized sqlite3.c",
    )?;
    verify_bundled_sqlite_source(
        &fs::read(sqlite_h_path)?,
        nomo_codegen_c::BUNDLED_SQLITE3_H_SHA256,
        "materialized sqlite3.h",
    )?;
    Ok(())
}

fn verify_bundled_sqlite_source(
    source: &[u8],
    expected: &str,
    name: &str,
) -> Result<(), std::io::Error> {
    let actual = format!("{:x}", Sha256::digest(source));
    if actual == expected {
        return Ok(());
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        format!("bundled {name} digest mismatch"),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_c_capabilities_select_libm_without_workload_names() {
        assert!(generated_c_uses_math("double value = sqrt(input);"));
        assert!(generated_c_uses_math(
            "float value = powf /* intrinsic call */ (left, right);"
        ));
        assert!(generated_c_uses_math("double value = round(input);"));
        assert!(generated_c_uses_math("float value = roundf(input);"));
        assert!(!generated_c_uses_math("int main(void) { return 0; }"));
        assert!(!generated_c_uses_math(
            "const char *name = \"sqrt(\"; /* pow(left, right) */"
        ));
        assert!(!generated_c_uses_math("double my_sqrt(double value);"));
        assert!(!generated_c_uses_math(
            "double round_trip(double value); // round(value)"
        ));
        assert!(!generated_c_uses_math(
            "const char *literal = \"roundf(value)\";"
        ));
    }

    #[test]
    fn release_argv_keeps_clang_formal_flags_and_uses_gnu_cross_driver_semantics() {
        let root = std::env::temp_dir().join(format!("nomo-release-driver-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let source = root.join("main.c");
        let object = root.join("main.o");
        fs::write(&source, "int main(void) { return 0; }\n").unwrap();
        let clang = root.join("clang");
        let clang_toolchain = CToolchain {
            program: "clang".to_string(),
            args: Vec::new(),
        };
        let clang_argv =
            release_compile_argv(&clang, &clang_toolchain, &source, &object, false).unwrap();
        assert_eq!(
            &clang_argv[1..6],
            [
                "--no-default-config",
                "-std=c99",
                "-O3",
                "-DNDEBUG",
                "-fomit-frame-pointer",
            ]
        );

        let gcc = root.join("aarch64-linux-gnu-gcc");
        let gcc_toolchain = CToolchain {
            program: "aarch64-linux-gnu-gcc".to_string(),
            args: Vec::new(),
        };
        let gcc_argv = release_compile_argv(&gcc, &gcc_toolchain, &source, &object, false).unwrap();
        assert_eq!(
            &gcc_argv[1..5],
            ["-std=c99", "-O3", "-DNDEBUG", "-fomit-frame-pointer"]
        );
        assert!(
            !gcc_argv
                .iter()
                .any(|argument| argument == "--no-default-config")
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn release_gnu_compiler_identity_uses_dumpmachine_without_clang_flags() {
        use std::os::unix::fs::PermissionsExt;

        let root =
            std::env::temp_dir().join(format!("nomo-release-gcc-probe-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let gcc = root.join("aarch64-linux-gnu-gcc");
        fs::write(
            &gcc,
            "#!/bin/sh\ncase \"$1\" in\n  --version) echo 'mock gcc 1.0' ;;\n  -dumpmachine) echo 'aarch64-unknown-linux-gnu' ;;\n  *) exit 23 ;;\nesac\n",
        )
        .unwrap();
        let mut permissions = fs::metadata(&gcc).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&gcc, permissions).unwrap();
        let identity = inspect_compiler(&gcc, &[], true).unwrap();
        assert_eq!(identity.target_triple, "aarch64-unknown-linux-gnu");
        assert_eq!(identity.version_output, "mock gcc 1.0");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn release_test_ffi_link_args_never_enter_translation_unit_compile_argv() {
        let root =
            std::env::temp_dir().join(format!("nomo-release-ffi-argv-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let compiler = root.join("clang");
        let source = root.join("main.c");
        let object = root.join("main.o");
        let binary = root.join("program");
        fs::write(&source, "int main(void) { return 0; }\n").unwrap();
        let toolchain = CToolchain {
            program: "clang".to_string(),
            args: Vec::new(),
        };
        let compile = release_compile_argv(&compiler, &toolchain, &source, &object, false).unwrap();
        let ffi = FfiLinkMetadata {
            link_args: vec!["-O0".to_string()],
            ..FfiLinkMetadata::default()
        };
        let target = "x86_64-unknown-linux-gnu".parse::<TargetTriple>().unwrap();
        let link = release_link_argv(
            &compiler,
            &toolchain,
            std::slice::from_ref(&object),
            &binary,
            &ffi,
            &target,
            false,
            false,
            false,
            false,
            false,
        )
        .unwrap();
        assert!(compile.iter().any(|argument| argument == "-O3"));
        assert!(!compile.iter().any(|argument| argument == "-O0"));
        assert!(link.iter().any(|argument| argument == "-O0"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn failed_metadata_receipt_or_sidecar_publication_clears_every_evidence_file() {
        let root = std::env::temp_dir().join(format!(
            "nomo-release-evidence-failure-{}",
            std::process::id()
        ));
        fs::create_dir_all(&root).unwrap();
        let metadata_path = root.join("nomo-build-metadata.json");
        let receipt_path = root.join(WORKSPACE_OWNERSHIP_RECEIPT_FILE);
        let provenance_path = root.join("release-provenance.json");
        let expected = FileRecord {
            path: provenance_path.to_string_lossy().into_owned(),
            sha256: "expected".to_string(),
        };

        fs::write(&metadata_path, "old metadata").unwrap();
        fs::write(&receipt_path, "old receipt").unwrap();
        fs::write(&provenance_path, "old provenance").unwrap();
        let metadata_failure = publish_release_evidence_with(
            &metadata_path,
            &receipt_path,
            &provenance_path,
            &expected,
            || {
                fs::write(&metadata_path, "partial metadata").unwrap();
                Err(BuildError::Message(
                    "injected metadata write failure".to_string(),
                ))
            },
            || unreachable!(),
            || unreachable!(),
        );
        assert!(metadata_failure.is_err());
        assert!(!metadata_path.exists());
        assert!(!receipt_path.exists());
        assert!(!provenance_path.exists());

        let receipt_failure = publish_release_evidence_with(
            &metadata_path,
            &receipt_path,
            &provenance_path,
            &expected,
            || {
                fs::write(&metadata_path, "new metadata")
                    .map_err(|error| BuildError::Message(error.to_string()))
            },
            || {
                fs::write(&receipt_path, "partial receipt").unwrap();
                Err(BuildError::Message(
                    "injected owner receipt write failure".to_string(),
                ))
            },
            || unreachable!(),
        );
        assert!(receipt_failure.is_err());
        assert!(!metadata_path.exists());
        assert!(!receipt_path.exists());
        assert!(!provenance_path.exists());

        let sidecar_failure = publish_release_evidence_with(
            &metadata_path,
            &receipt_path,
            &provenance_path,
            &expected,
            || {
                fs::write(&metadata_path, "new metadata")
                    .map_err(|error| BuildError::Message(error.to_string()))
            },
            || {
                fs::write(&receipt_path, "new receipt")
                    .map_err(|error| BuildError::Message(error.to_string()))
            },
            || {
                fs::write(&provenance_path, "partial sidecar").unwrap();
                Err(BuildError::Message(
                    "injected sidecar write failure".to_string(),
                ))
            },
        );
        assert!(sidecar_failure.is_err());
        assert!(!metadata_path.exists());
        assert!(!receipt_path.exists());
        assert!(!provenance_path.exists());

        let mut removal_order = Vec::new();
        clear_release_evidence_paths_with(
            &provenance_path,
            &receipt_path,
            &metadata_path,
            |path| {
                removal_order.push(path.to_path_buf());
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(
            removal_order,
            vec![
                provenance_path.clone(),
                receipt_path.clone(),
                metadata_path.clone()
            ]
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn pre_discovery_cleanup_waits_for_metadata_then_sidecar_transaction() {
        use std::sync::mpsc;
        use std::thread;
        use std::time::Duration;

        let project_root = std::env::temp_dir().join(format!(
            "nomo-release-evidence-lock-window-{}",
            std::process::id()
        ));
        if project_root.exists() {
            fs::remove_dir_all(&project_root).unwrap();
        }
        fs::create_dir_all(project_root.join("build")).unwrap();
        let project_root = fs::canonicalize(project_root).unwrap();
        let root = project_root.join("build");
        let metadata_path = root.join("nomo-build-metadata.json");
        let receipt_path = root.join(WORKSPACE_OWNERSHIP_RECEIPT_FILE);
        let provenance_path = root.join("release-provenance.json");
        let (metadata_written_tx, metadata_written_rx) = mpsc::channel();
        let (publish_tx, publish_rx) = mpsc::channel();
        let publisher_root = root.clone();
        let publisher = thread::spawn(move || {
            let transaction = BuildEvidenceTransaction::acquire(&publisher_root).unwrap();
            fs::write(
                publisher_root.join("nomo-build-metadata.json"),
                "complete metadata",
            )
            .unwrap();
            fs::write(
                publisher_root.join(WORKSPACE_OWNERSHIP_RECEIPT_FILE),
                "complete owner receipt",
            )
            .unwrap();
            metadata_written_tx.send(()).unwrap();
            publish_rx.recv().unwrap();
            fs::write(
                publisher_root.join("release-provenance.json"),
                "commit marker",
            )
            .unwrap();
            transaction.commit();
        });

        metadata_written_rx.recv().unwrap();
        assert!(metadata_path.is_file());
        assert!(receipt_path.is_file());
        assert!(!provenance_path.exists());
        let (cleanup_started_tx, cleanup_started_rx) = mpsc::channel();
        let (cleanup_finished_tx, cleanup_finished_rx) = mpsc::channel();
        let cleanup_root = root.clone();
        let cleanup = thread::spawn(move || {
            cleanup_started_tx.send(()).unwrap();
            clear_build_metadata_at(&cleanup_root).unwrap();
            cleanup_finished_tx.send(()).unwrap();
        });
        cleanup_started_rx.recv().unwrap();
        thread::sleep(Duration::from_millis(50));
        assert!(
            cleanup_finished_rx.try_recv().is_err(),
            "cleanup must block on the publisher's target-directory lock"
        );
        assert!(metadata_path.is_file());
        assert!(receipt_path.is_file());
        assert!(!provenance_path.exists());

        publish_tx.send(()).unwrap();
        publisher.join().unwrap();
        cleanup_finished_rx
            .recv_timeout(Duration::from_secs(5))
            .unwrap();
        cleanup.join().unwrap();
        assert!(!metadata_path.exists());
        assert!(!receipt_path.exists());
        assert!(!provenance_path.exists());
        fs::remove_dir_all(project_root).unwrap();
    }

    #[test]
    fn release_evidence_paths_reject_symlink_or_reparse_components_and_lock_files() {
        let root = std::env::temp_dir().join(format!(
            "nomo-release-evidence-symlink-{}",
            std::process::id()
        ));
        if root.exists() {
            fs::remove_dir_all(&root).unwrap();
        }
        fs::create_dir_all(&root).unwrap();
        let root = fs::canonicalize(root).unwrap();
        let external = root.join("external");
        fs::create_dir_all(&external).unwrap();
        let external_file = external.join("sentinel");
        fs::write(&external_file, "untouched").unwrap();

        let target = root.join("target");
        fs::create_dir_all(&target).unwrap();
        reject_existing_symlink_or_reparse_components(&target).unwrap();
        let lock_path = target.join(".nomo-build.lock");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&external_file, &lock_path).unwrap();
        #[cfg(windows)]
        if std::os::windows::fs::symlink_file(&external_file, &lock_path).is_err() {
            fs::remove_dir_all(root).unwrap();
            return;
        }
        assert!(BuildEvidenceLock::acquire(&target, false).is_err());
        assert_eq!(fs::read_to_string(&external_file).unwrap(), "untouched");
        fs::remove_file(&lock_path).unwrap();

        let receipt_path = target.join(WORKSPACE_OWNERSHIP_RECEIPT_FILE);
        #[cfg(unix)]
        std::os::unix::fs::symlink(&external_file, &receipt_path).unwrap();
        #[cfg(windows)]
        std::os::windows::fs::symlink_file(&external_file, &receipt_path).unwrap();
        assert!(read_workspace_ownership_receipt(&receipt_path).is_err());
        assert_eq!(fs::read_to_string(&external_file).unwrap(), "untouched");

        let linked_parent = root.join("linked-parent");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&external, &linked_parent).unwrap();
        #[cfg(windows)]
        std::os::windows::fs::symlink_dir(&external, &linked_parent).unwrap();
        assert!(
            reject_existing_symlink_or_reparse_components(&linked_parent.join("sentinel")).is_err()
        );
        assert_eq!(fs::read_to_string(&external_file).unwrap(), "untouched");
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn windows_path_component_walker_completes_drive_and_unc_anchors_before_inspection() {
        fn visited(path: &str) -> Vec<PathBuf> {
            let mut visited = Vec::new();
            visit_complete_path_components(Path::new(path), |component| {
                visited.push(component.to_path_buf());
                Ok(())
            })
            .unwrap();
            visited
        }

        let drive = visited(r"D:\a\nomo");
        assert_eq!(drive.first(), Some(&PathBuf::from(r"D:\")));
        assert!(!drive.contains(&PathBuf::from("D:")));

        let verbatim_drive = visited(r"\\?\D:\a\nomo");
        assert_eq!(verbatim_drive.first(), Some(&PathBuf::from(r"\\?\D:\")));
        assert!(!verbatim_drive.contains(&PathBuf::from(r"\\?\D:")));

        let unc = visited(r"\\server\share\a\nomo");
        assert_eq!(unc.first(), Some(&PathBuf::from(r"\\server\share\")));

        let verbatim_unc = visited(r"\\?\UNC\server\share\a\nomo");
        assert_eq!(
            verbatim_unc.first(),
            Some(&PathBuf::from(r"\\?\UNC\server\share\"))
        );
    }

    #[test]
    fn codegen_cache_separates_profile_and_misses_after_pipeline_change() {
        let producer = producer_executable_identity().unwrap();
        let debug =
            codegen_cache_configuration(BuildProfile::Debug, PASS_PIPELINE_VERSION, &producer);
        let release =
            codegen_cache_configuration(BuildProfile::Release, PASS_PIPELINE_VERSION, &producer);
        let next_pipeline = codegen_cache_configuration(
            BuildProfile::Release,
            PASS_PIPELINE_VERSION + 1,
            &producer,
        );
        assert_ne!(debug, release);
        assert_ne!(release, next_pipeline);
        assert!(release.contains(&format!("exe-sha256:{}", producer.sha256)));

        let root = std::env::temp_dir().join(format!(
            "nomo-codegen-cache-profile-pipeline-{}",
            std::process::id()
        ));
        if root.exists() {
            fs::remove_dir_all(&root).unwrap();
        }
        let cache = PersistentQueryCache::at_root(&root);
        let fingerprint = crate::incremental::ContentFingerprint::of_text("unchanged sources");
        let current_key = crate::incremental::QueryKey::new(
            "test-target",
            "codegen-c",
            release,
            fingerprint.clone(),
        );
        let next_key = crate::incremental::QueryKey::new(
            "test-target",
            "codegen-c",
            next_pipeline,
            fingerprint,
        );
        assert_eq!(cache.get::<String>(&current_key), None);
        cache
            .insert(&current_key, &"generated C".to_string())
            .unwrap();
        assert_eq!(
            cache.get::<String>(&current_key).as_deref(),
            Some("generated C")
        );
        assert_eq!(
            cache.get::<String>(&next_key),
            None,
            "a pass-pipeline version change must miss even with identical sources"
        );
        fs::remove_dir_all(&root).unwrap();
    }
}
