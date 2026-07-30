use super::{BuildError, BuildProfile};
use crate::incremental::QueryKey;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::env;
use std::ffi::OsStr;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::OnceLock;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

#[cfg(windows)]
#[path = "build_metadata_windows.rs"]
mod windows_authority;

const BUILD_METADATA_SCHEMA: u32 = 1;
const RELEASE_PROVENANCE_SCHEMA: u32 = 1;
const CACHE_IDENTITY_SCHEMA: u32 = 1;
const CONTENT_BINDING_SCHEMA: u32 = 1;
const PRODUCER_EXECUTABLE_SCHEMA: u32 = 1;
const CONTENT_BINDING_DOMAIN: &str = "nomo-build-metadata-content-binding-v1";
pub(super) const PASS_PIPELINE_VERSION: u32 = 2;
const TOOLCHAIN_CONFIG_VERSION: u32 = 1;
const RELEASE_DRIVER_CONFIG_FLAGS: &[&str] = &["--no-default-config"];
const RELEASE_C_FLAGS: &[&str] = &["-std=c99", "-O3", "-DNDEBUG", "-fomit-frame-pointer"];

const COMPILER_AFFECTING_ENVIRONMENT: &[&str] = &[
    "CPATH",
    "C_INCLUDE_PATH",
    "CPLUS_INCLUDE_PATH",
    "OBJC_INCLUDE_PATH",
    "LIBRARY_PATH",
    "SDKROOT",
    "MACOSX_DEPLOYMENT_TARGET",
    "DEVELOPER_DIR",
    "TOOLCHAINS",
    "INCLUDE",
    "LIB",
    "LIBPATH",
    "CL",
    "_CL_",
    "CC",
    "CXX",
    "CFLAGS",
    "CPPFLAGS",
    "CXXFLAGS",
    "LDFLAGS",
    "GOFLAGS",
    "GOENV",
    "GOCACHE",
    "GOMODCACHE",
    "CGO_CFLAGS",
    "CGO_CPPFLAGS",
    "CGO_CXXFLAGS",
    "CGO_FFLAGS",
    "CGO_LDFLAGS",
    "RUSTFLAGS",
    "CARGO_ENCODED_RUSTFLAGS",
    "RUSTC",
    "RUSTDOC",
    "RUSTC_WRAPPER",
    "RUSTC_WORKSPACE_WRAPPER",
    "CARGO_BUILD_RUSTC",
    "CARGO_BUILD_RUSTC_WRAPPER",
    "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER",
    "CARGO_HOME",
];

#[cfg(not(windows))]
const POSIX_RETAINED_ENVIRONMENT: &[&str] = &[
    "PATH",
    "HOME",
    "USERPROFILE",
    "TMPDIR",
    "TEMP",
    "TMP",
    "SystemRoot",
    "WINDIR",
    "RUSTUP_HOME",
    "GOENV",
    "SDKROOT",
    "LC_ALL",
    "LANG",
];

const PATH_ENVIRONMENT: &[&str] = &[
    "HOME",
    "USERPROFILE",
    "TMPDIR",
    "TEMP",
    "TMP",
    "SystemRoot",
    "WINDIR",
    "CARGO_HOME",
    "RUSTUP_HOME",
    "CARGO_TARGET_DIR",
    "RUSTC",
    "GOCACHE",
    "GOMODCACHE",
];

const CACHE_INPUT_ORDER: &[&str] = &[
    "profile",
    "target_triple",
    "producer_executable_sha256",
    "compiler_revision",
    "runtime_revision",
    "pass_pipeline_version",
    "toolchain_config_version",
    "toolchain_config",
    "toolchain_config_sha256",
    "query_schema",
    "query_toolchain",
    "query_target",
    "query_namespace",
    "query_identity",
    "query_fingerprint",
];

const CONTENT_BINDING_INPUT_ORDER: &[&str] = &[
    "profile",
    "target_triple",
    "cache_key",
    "producer_identity_sha256",
    "compiler_identity_sha256",
    "commands_sha256",
    "generated_c_path",
    "generated_c_sha256",
    "binary_path",
    "binary_sha256",
    "release_provenance_path",
    "release_provenance_sha256",
];

#[derive(Clone)]
struct CanonicalBuildEnvironment {
    values: BTreeMap<String, String>,
    projection: Value,
}

static CANONICAL_BUILD_ENVIRONMENT: OnceLock<Result<CanonicalBuildEnvironment, String>> =
    OnceLock::new();
static PRODUCER_EXECUTABLE: OnceLock<Result<ProducerExecutableIdentity, String>> = OnceLock::new();

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(super) struct FileRecord {
    pub path: String,
    pub sha256: String,
}

impl FileRecord {
    pub(super) fn from_path(path: &Path) -> Result<Self, BuildError> {
        let absolute = absolute_path(path)?;
        let metadata = fs::symlink_metadata(&absolute).map_err(|error| {
            BuildError::Message(format!(
                "failed to inspect build artifact {}: {error}",
                absolute.display()
            ))
        })?;
        if !metadata.file_type().is_file() {
            return Err(BuildError::Message(format!(
                "build artifact is not a regular file: {}",
                absolute.display()
            )));
        }
        Ok(Self {
            path: absolute.to_string_lossy().into_owned(),
            sha256: sha256_file(&absolute)?,
        })
    }

    pub(super) fn for_canonical_json<T: Serialize>(
        path: &Path,
        value: &T,
    ) -> Result<Self, BuildError> {
        let absolute = absolute_path(path)?;
        Ok(Self {
            path: absolute.to_string_lossy().into_owned(),
            sha256: sha256_bytes(&canonical_json_bytes(value)?),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(super) struct ProducerExecutableIdentity {
    pub schema: u32,
    pub path: String,
    pub realpath: String,
    pub sha256: String,
    pub size_bytes: u64,
    pub package_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(super) struct CompilerIdentity {
    pub path: String,
    pub realpath: String,
    pub sha256: String,
    pub version_output: String,
    pub target_triple: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(super) struct CommandRecord {
    pub argv: Vec<String>,
    pub command: String,
    pub cwd: Option<String>,
    pub duration_ns: u64,
    pub exit_code: i32,
    pub environment: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(super) struct ReleaseBackendProvenance {
    pub schema: u32,
    pub complete_argv: bool,
    pub compiler: CompilerIdentity,
    pub objects: Vec<FileRecord>,
    pub compile_commands: Vec<CommandRecord>,
    pub link_command: CommandRecord,
    pub generated_c: FileRecord,
    pub binary: FileRecord,
}

impl ReleaseBackendProvenance {
    pub(super) fn new(
        compiler: CompilerIdentity,
        objects: Vec<FileRecord>,
        compile_commands: Vec<CommandRecord>,
        link_command: CommandRecord,
        generated_c: FileRecord,
        binary: FileRecord,
    ) -> Self {
        Self {
            schema: RELEASE_PROVENANCE_SCHEMA,
            complete_argv: true,
            compiler,
            objects,
            compile_commands,
            link_command,
            generated_c,
            binary,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct CacheIdentity {
    schema: u32,
    algorithm: String,
    formula: String,
    input_order: Vec<String>,
    inputs: BTreeMap<String, String>,
    cache_key: String,
    query_key: QueryKey,
    query_key_json: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct ContentBinding {
    schema: u32,
    algorithm: String,
    domain: String,
    formula: String,
    input_order: Vec<String>,
    inputs: BTreeMap<String, String>,
    canonical_subdocuments: BTreeMap<String, String>,
    sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(super) struct BuildMetadata {
    schema: u32,
    selected_profile: BuildProfile,
    target_triple: String,
    producer_executable: ProducerExecutableIdentity,
    cache_identity: CacheIdentity,
    content_binding: ContentBinding,
    compiler: Option<CompilerIdentity>,
    complete_argv: bool,
    compile_commands: Vec<CommandRecord>,
    link_command: Option<CommandRecord>,
    combined_compile_link_command: Option<CommandRecord>,
    generated_c: FileRecord,
    binary: Option<FileRecord>,
    release_provenance: Option<FileRecord>,
}

impl BuildMetadata {
    pub(super) fn new(
        profile: BuildProfile,
        target_triple: String,
        cache_query_key: QueryKey,
        producer_executable: ProducerExecutableIdentity,
        compiler: Option<CompilerIdentity>,
        compile_commands: Vec<CommandRecord>,
        link_command: Option<CommandRecord>,
        combined_compile_link_command: Option<CommandRecord>,
        generated_c: FileRecord,
        binary: Option<FileRecord>,
        release_provenance: Option<FileRecord>,
    ) -> Result<Self, BuildError> {
        if profile == BuildProfile::Release && binary.is_some() && release_provenance.is_none() {
            return Err(BuildError::Message(
                "release build metadata requires release provenance".to_string(),
            ));
        }
        if profile == BuildProfile::Debug && release_provenance.is_some() {
            return Err(BuildError::Message(
                "debug build metadata cannot claim release provenance".to_string(),
            ));
        }
        let complete_argv = if binary.is_none() {
            compile_commands.is_empty()
                && link_command.is_none()
                && combined_compile_link_command.is_none()
        } else if profile == BuildProfile::Release {
            !compile_commands.is_empty()
                && link_command.is_some()
                && combined_compile_link_command.is_none()
        } else {
            compile_commands.is_empty()
                && link_command.is_none()
                && combined_compile_link_command.is_some()
        };
        if !complete_argv {
            return Err(BuildError::Message(
                "build metadata command shape does not match the selected profile".to_string(),
            ));
        }
        let cache_identity = cache_identity(
            profile,
            &target_triple,
            cache_query_key,
            &producer_executable,
        )?;
        let content_binding = content_binding(
            profile,
            &target_triple,
            &cache_identity.cache_key,
            &producer_executable,
            &generated_c,
            compiler.as_ref(),
            &compile_commands,
            link_command.as_ref(),
            combined_compile_link_command.as_ref(),
            binary.as_ref(),
            release_provenance.as_ref(),
        )?;
        Ok(Self {
            schema: BUILD_METADATA_SCHEMA,
            selected_profile: profile,
            target_triple,
            producer_executable,
            cache_identity,
            content_binding,
            compiler,
            complete_argv,
            compile_commands,
            link_command,
            combined_compile_link_command,
            generated_c,
            binary,
            release_provenance,
        })
    }
}

pub(super) fn release_driver_config_flags() -> &'static [&'static str] {
    RELEASE_DRIVER_CONFIG_FLAGS
}

pub(super) fn release_driver_config_flags_for_compiler(
    compiler_path: &Path,
) -> &'static [&'static str] {
    if is_gnu_compiler(compiler_path) {
        &[]
    } else {
        RELEASE_DRIVER_CONFIG_FLAGS
    }
}

pub(super) fn release_c_flags() -> &'static [&'static str] {
    RELEASE_C_FLAGS
}

pub(super) fn producer_executable_identity() -> Result<ProducerExecutableIdentity, BuildError> {
    PRODUCER_EXECUTABLE
        .get_or_init(|| {
            let path = env::current_exe()
                .map_err(|error| format!("failed to locate producer executable: {error}"))?;
            producer_executable_identity_from_path(&path).map_err(|error| error.human())
        })
        .clone()
        .map_err(BuildError::Message)
}

fn producer_executable_identity_from_path(
    path: &Path,
) -> Result<ProducerExecutableIdentity, BuildError> {
    let path = absolute_path(path)?;
    let realpath = fs::canonicalize(&path).map_err(|error| {
        BuildError::Message(format!(
            "failed to resolve producer executable {}: {error}",
            path.display()
        ))
    })?;
    let metadata = fs::metadata(&realpath).map_err(|error| {
        BuildError::Message(format!(
            "failed to inspect producer executable {}: {error}",
            realpath.display()
        ))
    })?;
    if !metadata.is_file() {
        return Err(BuildError::Message(format!(
            "producer executable is not a regular file: {}",
            realpath.display()
        )));
    }
    Ok(ProducerExecutableIdentity {
        schema: PRODUCER_EXECUTABLE_SCHEMA,
        path: path.to_string_lossy().into_owned(),
        realpath: realpath.to_string_lossy().into_owned(),
        sha256: sha256_file(&realpath)?,
        size_bytes: metadata.len(),
        package_version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

pub(super) fn resolve_executable(
    program: &str,
    profile: BuildProfile,
) -> Result<PathBuf, BuildError> {
    let candidate = Path::new(program);
    if candidate.is_absolute() || candidate.components().count() > 1 {
        let absolute = absolute_path(candidate)?;
        if absolute.is_file() {
            return Ok(absolute);
        }
        return Err(BuildError::Message(format!(
            "C compiler is unavailable: {}",
            absolute.display()
        )));
    }
    #[cfg(target_os = "macos")]
    if program == "clang" {
        let system_clang = PathBuf::from("/usr/bin/clang");
        if system_clang.is_file() {
            return Ok(system_clang);
        }
    }
    let path = if profile == BuildProfile::Release {
        canonical_build_environment()?
            .values
            .get("PATH")
            .cloned()
            .ok_or_else(|| BuildError::Message("controlled build PATH is unavailable".to_string()))?
            .into()
    } else {
        env::var_os("PATH").ok_or_else(|| BuildError::Message("PATH is unavailable".to_string()))?
    };
    let executable_names = executable_names(program, profile)?;
    for directory in env::split_paths(&path) {
        for name in &executable_names {
            let path = directory.join(name);
            if path.is_file() {
                return absolute_path(&path);
            }
        }
    }
    Err(BuildError::Message(format!(
        "C compiler `{program}` was not found in PATH"
    )))
}

fn executable_names(program: &str, profile: BuildProfile) -> Result<Vec<String>, BuildError> {
    #[cfg(windows)]
    {
        if Path::new(program).extension().is_some() {
            return Ok(vec![program.to_string()]);
        }
        let extensions = if profile == BuildProfile::Release {
            canonical_build_environment()?
                .values
                .get("PATHEXT")
                .cloned()
                .ok_or_else(|| {
                    BuildError::Message("controlled build PATHEXT is unavailable".to_string())
                })?
        } else {
            env::var("PATHEXT").unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".to_string())
        };
        let mut names = vec![format!("{program}.exe")];
        names.extend(
            extensions
                .split(';')
                .filter(|extension| !extension.is_empty())
                .map(|extension| format!("{program}{extension}")),
        );
        names.dedup();
        return Ok(names);
    }
    #[cfg(not(windows))]
    {
        let _ = profile;
        Ok(vec![program.to_string()])
    }
}

pub(super) fn inspect_compiler(
    compiler_path: &Path,
    toolchain_args: &[String],
    release: bool,
) -> Result<CompilerIdentity, BuildError> {
    let compiler_path = absolute_path(compiler_path)?;
    let realpath = release_compiler_realpath(&compiler_path, release)?;
    let release_driver_flags = if release {
        release_driver_config_flags_for_compiler(&compiler_path)
    } else {
        &[]
    };
    let mut version_argv = vec![compiler_path.to_string_lossy().into_owned()];
    version_argv.extend(release_driver_flags.iter().map(|flag| flag.to_string()));
    version_argv.push("--version".to_string());
    let version_output = probe_output(&version_argv, "C compiler version", release)?;

    let mut target_argv = vec![compiler_path.to_string_lossy().into_owned()];
    target_argv.extend(release_driver_flags.iter().map(|flag| flag.to_string()));
    target_argv.extend(toolchain_args.iter().cloned());
    target_argv.push(
        if release && is_gnu_compiler(&compiler_path) {
            "-dumpmachine"
        } else {
            "-print-target-triple"
        }
        .to_string(),
    );
    let target_triple = match probe_output(&target_argv, "C compiler target", release) {
        Ok(target) => target,
        Err(_) if !release => {
            let fallback = vec![
                compiler_path.to_string_lossy().into_owned(),
                "-dumpmachine".to_string(),
            ];
            probe_output(&fallback, "C compiler target", false)?
        }
        Err(error) => return Err(error),
    };
    if target_triple.chars().any(char::is_whitespace) {
        return Err(BuildError::Message(format!(
            "C compiler reported an invalid target triple `{target_triple}`"
        )));
    }
    Ok(CompilerIdentity {
        path: compiler_path.to_string_lossy().into_owned(),
        realpath: realpath.to_string_lossy().into_owned(),
        sha256: sha256_file(&realpath)?,
        version_output,
        target_triple,
    })
}

fn is_gnu_compiler(path: &Path) -> bool {
    path.file_name()
        .and_then(OsStr::to_str)
        .map(|name| {
            let name = name.to_ascii_lowercase();
            name == "gcc"
                || name == "gcc.exe"
                || name.ends_with("-gcc")
                || name.ends_with("-gcc.exe")
        })
        .unwrap_or(false)
}

fn release_compiler_realpath(compiler_path: &Path, release: bool) -> Result<PathBuf, BuildError> {
    #[cfg(not(target_os = "macos"))]
    let _ = release;

    #[cfg(target_os = "macos")]
    if release && compiler_path == Path::new("/usr/bin/clang") {
        let environment = canonical_build_environment()?;
        let output = Command::new("/usr/bin/xcrun")
            .args(["--find", "clang"])
            .env_clear()
            .envs(&environment.values)
            .output()
            .map_err(|error| {
                BuildError::Message(format!(
                    "failed to select the Darwin Clang backend with xcrun: {error}"
                ))
            })?;
        if !output.status.success() || !output.stderr.is_empty() {
            return Err(BuildError::Message(format!(
                "xcrun Clang selection failed:\n{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            )));
        }
        let selected = PathBuf::from(String::from_utf8_lossy(&output.stdout).trim().to_string());
        if !selected.is_absolute() || !selected.is_file() {
            return Err(BuildError::Message(format!(
                "xcrun selected an unavailable Clang backend: {}",
                selected.display()
            )));
        }
        return fs::canonicalize(&selected).map_err(|error| {
            BuildError::Message(format!(
                "failed to resolve xcrun-selected Clang {}: {error}",
                selected.display()
            ))
        });
    }
    fs::canonicalize(compiler_path).map_err(|error| {
        BuildError::Message(format!(
            "failed to resolve C compiler {}: {error}",
            compiler_path.display()
        ))
    })
}

pub(super) fn run_recorded_command(
    argv: Vec<String>,
    profile: BuildProfile,
) -> Result<CommandRecord, BuildError> {
    if argv.is_empty() {
        return Err(BuildError::Message(
            "cannot execute an empty compiler command".to_string(),
        ));
    }
    let cwd = absolute_path(&env::current_dir().map_err(|error| {
        BuildError::Message(format!("failed to read current directory: {error}"))
    })?)?;
    let canonical_environment = (profile == BuildProfile::Release)
        .then(canonical_build_environment)
        .transpose()?;
    let environment = canonical_environment
        .as_ref()
        .map(|environment| environment.projection.clone())
        .unwrap_or_else(ambient_build_environment_projection);
    let started = Instant::now();
    let mut command = Command::new(&argv[0]);
    command.args(&argv[1..]);
    if let Some(environment) = &canonical_environment {
        command.env_clear().envs(&environment.values);
    }
    let output = command.output().map_err(|error| {
        BuildError::Message(format!("failed to run C compiler `{}`: {error}", argv[0]))
    })?;
    let duration_ns = u64::try_from(started.elapsed().as_nanos()).unwrap_or(u64::MAX);
    ensure_command_success(&argv, &output)?;
    Ok(CommandRecord {
        command: command_text(&argv),
        argv,
        cwd: Some(cwd.to_string_lossy().into_owned()),
        duration_ns,
        exit_code: output.status.code().unwrap_or(0),
        environment,
    })
}

fn ensure_command_success(argv: &[String], output: &Output) -> Result<(), BuildError> {
    if output.status.success() {
        return Ok(());
    }
    Err(BuildError::Message(format!(
        "C compiler failed while running `{}`:\n{}{}",
        command_text(argv),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )))
}

fn probe_output(argv: &[String], label: &str, release: bool) -> Result<String, BuildError> {
    let mut command = Command::new(&argv[0]);
    command.args(&argv[1..]);
    if release {
        let environment = canonical_build_environment()?;
        command.env_clear().envs(&environment.values);
    }
    let output = command.output().map_err(|error| {
        BuildError::Message(format!(
            "failed to run {label} probe `{}`: {error}",
            command_text(argv)
        ))
    })?;
    if !output.status.success() {
        return Err(BuildError::Message(format!(
            "{label} probe failed:\n{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    let combined = [output.stdout, output.stderr].concat();
    let value = String::from_utf8_lossy(&combined).trim().to_string();
    if value.is_empty() {
        return Err(BuildError::Message(format!(
            "{label} probe returned empty output"
        )));
    }
    Ok(value)
}

pub(super) fn write_release_provenance(
    path: &Path,
    provenance: &ReleaseBackendProvenance,
) -> Result<FileRecord, BuildError> {
    atomic_write_canonical_json(path, provenance)?;
    FileRecord::from_path(path)
}

pub(super) fn write_build_metadata(
    path: &Path,
    metadata: &BuildMetadata,
) -> Result<FileRecord, BuildError> {
    atomic_write_canonical_json(path, metadata)?;
    FileRecord::from_path(path)
}

#[cfg(test)]
pub(super) fn validate_build_metadata(path: &Path) -> Result<BuildMetadata, BuildError> {
    let bytes = fs::read(path).map_err(|error| {
        BuildError::Message(format!(
            "failed to read build metadata {}: {error}",
            path.display()
        ))
    })?;
    let metadata = serde_json::from_slice::<BuildMetadata>(&bytes).map_err(|error| {
        BuildError::Message(format!(
            "invalid build metadata {}: {error}",
            path.display()
        ))
    })?;
    if metadata.schema != BUILD_METADATA_SCHEMA {
        return Err(BuildError::Message(format!(
            "unsupported build metadata schema {}",
            metadata.schema
        )));
    }
    if canonical_json_bytes(&metadata)? != bytes {
        return Err(BuildError::Message(
            "build metadata is not canonical JSON".to_string(),
        ));
    }
    let current_producer =
        producer_executable_identity_from_path(Path::new(&metadata.producer_executable.path))?;
    if current_producer != metadata.producer_executable {
        return Err(BuildError::Message(
            "build metadata producer executable identity is stale".to_string(),
        ));
    }
    validate_file_record(&metadata.generated_c, "generated C")?;
    if let Some(binary) = &metadata.binary {
        validate_file_record(binary, "binary")?;
    }
    if let Some(provenance_record) = &metadata.release_provenance {
        validate_file_record(provenance_record, "release provenance")?;
        let provenance_bytes = fs::read(&provenance_record.path).map_err(|error| {
            BuildError::Message(format!("failed to read release provenance: {error}"))
        })?;
        let provenance = serde_json::from_slice::<ReleaseBackendProvenance>(&provenance_bytes)
            .map_err(|error| BuildError::Message(format!("invalid release provenance: {error}")))?;
        if provenance.schema != RELEASE_PROVENANCE_SCHEMA
            || !provenance.complete_argv
            || canonical_json_bytes(&provenance)? != provenance_bytes
        {
            return Err(BuildError::Message(
                "release provenance is not canonical schema 1".to_string(),
            ));
        }
        if metadata.compiler.as_ref() != Some(&provenance.compiler)
            || metadata.compile_commands != provenance.compile_commands
            || metadata.link_command.as_ref() != Some(&provenance.link_command)
            || metadata.generated_c != provenance.generated_c
            || metadata.binary.as_ref() != Some(&provenance.binary)
        {
            return Err(BuildError::Message(
                "release metadata does not match its provenance sidecar".to_string(),
            ));
        }
        for object in &provenance.objects {
            validate_file_record(object, "release object")?;
        }
    }
    if let Some(compiler) = &metadata.compiler {
        let path = Path::new(&compiler.path);
        let realpath = Path::new(&compiler.realpath);
        if !path.is_absolute() || !realpath.is_absolute() || !realpath.is_file() {
            return Err(BuildError::Message(
                "build metadata compiler identity is invalid".to_string(),
            ));
        }
        if sha256_file(realpath)? != compiler.sha256 {
            return Err(BuildError::Message(
                "build metadata compiler hash is stale".to_string(),
            ));
        }
    }
    let rebuilt = BuildMetadata::new(
        metadata.selected_profile,
        metadata.target_triple.clone(),
        metadata.cache_identity.query_key.clone(),
        metadata.producer_executable.clone(),
        metadata.compiler.clone(),
        metadata.compile_commands.clone(),
        metadata.link_command.clone(),
        metadata.combined_compile_link_command.clone(),
        metadata.generated_c.clone(),
        metadata.binary.clone(),
        metadata.release_provenance.clone(),
    )?;
    if rebuilt.cache_identity != metadata.cache_identity
        || rebuilt.content_binding != metadata.content_binding
        || rebuilt.complete_argv != metadata.complete_argv
    {
        return Err(BuildError::Message(
            "build metadata cache identity, content binding, or command shape was tampered"
                .to_string(),
        ));
    }
    Ok(metadata)
}

#[cfg(test)]
fn validate_file_record(record: &FileRecord, label: &str) -> Result<(), BuildError> {
    let current = FileRecord::from_path(Path::new(&record.path))?;
    if &current == record {
        Ok(())
    } else {
        Err(BuildError::Message(format!(
            "{label} content binding is stale"
        )))
    }
}

pub(super) fn remove_stale_file(path: &Path) -> Result<(), BuildError> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(BuildError::Message(format!(
                "failed to inspect stale build output {}: {error}",
                path.display()
            )));
        }
    };
    if !metadata.file_type().is_file() {
        return Err(BuildError::Message(format!(
            "refusing to replace non-regular build output: {}",
            path.display()
        )));
    }
    fs::remove_file(path).map_err(|error| {
        BuildError::Message(format!(
            "failed to remove stale build output {}: {error}",
            path.display()
        ))
    })
}

pub(super) fn atomic_write_canonical_json<T: Serialize>(
    path: &Path,
    value: &T,
) -> Result<(), BuildError> {
    let parent = path.parent().ok_or_else(|| {
        BuildError::Message(format!(
            "build metadata path has no parent: {}",
            path.display()
        ))
    })?;
    fs::create_dir_all(parent).map_err(|error| {
        BuildError::Message(format!(
            "failed to create build metadata directory {}: {error}",
            parent.display()
        ))
    })?;
    require_replaceable_file(path)?;
    let bytes = canonical_json_bytes(value)?;
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let file_name = path.file_name().and_then(OsStr::to_str).ok_or_else(|| {
        BuildError::Message(format!(
            "build metadata path has an invalid filename: {}",
            path.display()
        ))
    })?;
    let temporary = parent.join(format!(".{file_name}.{}.{}.tmp", std::process::id(), nonce));
    let result = (|| {
        let mut file = open_new_file(&temporary)?;
        file.write_all(&bytes).map_err(|error| {
            BuildError::Message(format!(
                "failed to write temporary build metadata {}: {error}",
                temporary.display()
            ))
        })?;
        file.sync_all().map_err(|error| {
            BuildError::Message(format!(
                "failed to sync temporary build metadata {}: {error}",
                temporary.display()
            ))
        })?;
        atomic_replace(&temporary, path).map_err(|error| {
            BuildError::Message(format!(
                "failed to publish build metadata {}: {error}",
                path.display()
            ))
        })?;
        sync_directory(parent);
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

pub(super) fn canonical_json_bytes<T: Serialize>(value: &T) -> Result<Vec<u8>, BuildError> {
    let canonical = serde_json::to_value(value).map_err(|error| {
        BuildError::Message(format!("failed to encode build metadata: {error}"))
    })?;
    let mut bytes = serde_json::to_vec_pretty(&canonical).map_err(|error| {
        BuildError::Message(format!("failed to encode build metadata: {error}"))
    })?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn canonical_compact_json<T: Serialize>(value: &T) -> Result<String, BuildError> {
    let value = serde_json::to_value(value).map_err(|error| {
        BuildError::Message(format!(
            "failed to encode canonical JSON subdocument: {error}"
        ))
    })?;
    serde_json::to_string(&canonicalize_json_value(value)).map_err(|error| {
        BuildError::Message(format!(
            "failed to serialize canonical JSON subdocument: {error}"
        ))
    })
}

fn canonicalize_json_value(value: Value) -> Value {
    match value {
        Value::Array(values) => {
            Value::Array(values.into_iter().map(canonicalize_json_value).collect())
        }
        Value::Object(values) => {
            let ordered = values
                .into_iter()
                .map(|(name, value)| (name, canonicalize_json_value(value)))
                .collect::<BTreeMap<_, _>>();
            Value::Object(ordered.into_iter().collect())
        }
        value => value,
    }
}

fn require_replaceable_file(path: &Path) -> Result<(), BuildError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_file() => Ok(()),
        Ok(_) => Err(BuildError::Message(format!(
            "refusing to replace non-regular build output: {}",
            path.display()
        ))),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(BuildError::Message(format!(
            "failed to inspect build output {}: {error}",
            path.display()
        ))),
    }
}

#[cfg(not(windows))]
fn atomic_replace(temporary: &Path, path: &Path) -> Result<(), std::io::Error> {
    fs::rename(temporary, path)
}

#[cfg(windows)]
fn atomic_replace(temporary: &Path, path: &Path) -> Result<(), std::io::Error> {
    use std::os::windows::ffi::OsStrExt;

    const MOVEFILE_REPLACE_EXISTING: u32 = 0x1;
    const MOVEFILE_WRITE_THROUGH: u32 = 0x8;
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn MoveFileExW(
            existing_file_name: *const u16,
            new_file_name: *const u16,
            flags: u32,
        ) -> i32;
    }
    let existing = temporary
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let replacement = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let moved = unsafe {
        MoveFileExW(
            existing.as_ptr(),
            replacement.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if moved == 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}

fn sync_directory(path: &Path) {
    if let Ok(directory) = File::open(path) {
        let _ = directory.sync_all();
    }
}

fn open_new_file(path: &Path) -> Result<File, BuildError> {
    OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)
        .map_err(|error| {
            BuildError::Message(format!(
                "failed to create temporary build metadata {}: {error}",
                path.display()
            ))
        })
}

fn cache_identity(
    profile: BuildProfile,
    target_triple: &str,
    query_key: QueryKey,
    producer_executable: &ProducerExecutableIdentity,
) -> Result<CacheIdentity, BuildError> {
    let toolchain_config =
        codegen_cache_configuration(profile, PASS_PIPELINE_VERSION, producer_executable);
    if query_key.target != target_triple {
        return Err(BuildError::Message(
            "cache query target does not match build metadata".to_string(),
        ));
    }
    if !query_key.identity.ends_with(&toolchain_config) {
        return Err(BuildError::Message(
            "cache query identity does not contain the selected profile configuration".to_string(),
        ));
    }
    let query_json = serde_json::to_vec(&query_key).map_err(|error| {
        BuildError::Message(format!(
            "failed to encode persistent cache query key: {error}"
        ))
    })?;
    let mut inputs = BTreeMap::new();
    inputs.insert("profile".to_string(), profile.as_str().to_string());
    inputs.insert("target_triple".to_string(), target_triple.to_string());
    inputs.insert(
        "producer_executable_sha256".to_string(),
        producer_executable.sha256.clone(),
    );
    inputs.insert(
        "compiler_revision".to_string(),
        format!("exe-sha256:{}", producer_executable.sha256),
    );
    inputs.insert(
        "runtime_revision".to_string(),
        format!("exe-sha256:{}", producer_executable.sha256),
    );
    inputs.insert(
        "pass_pipeline_version".to_string(),
        PASS_PIPELINE_VERSION.to_string(),
    );
    inputs.insert(
        "toolchain_config_version".to_string(),
        TOOLCHAIN_CONFIG_VERSION.to_string(),
    );
    inputs.insert("toolchain_config".to_string(), toolchain_config.clone());
    inputs.insert(
        "toolchain_config_sha256".to_string(),
        sha256_bytes(toolchain_config.as_bytes()),
    );
    inputs.insert("query_schema".to_string(), query_key.schema.to_string());
    inputs.insert("query_toolchain".to_string(), query_key.toolchain.clone());
    inputs.insert("query_target".to_string(), query_key.target.clone());
    inputs.insert("query_namespace".to_string(), query_key.namespace.clone());
    inputs.insert("query_identity".to_string(), query_key.identity.clone());
    inputs.insert(
        "query_fingerprint".to_string(),
        query_key.fingerprint.as_str().to_string(),
    );
    Ok(CacheIdentity {
        schema: CACHE_IDENTITY_SCHEMA,
        algorithm: "sha256".to_string(),
        formula: "sha256(UTF-8 bytes of query_key_json)".to_string(),
        input_order: CACHE_INPUT_ORDER
            .iter()
            .map(|input| (*input).to_string())
            .collect(),
        inputs,
        cache_key: sha256_bytes(&query_json),
        query_key,
        query_key_json: String::from_utf8(query_json).map_err(|error| {
            BuildError::Message(format!(
                "persistent cache query key is not UTF-8 JSON: {error}"
            ))
        })?,
    })
}

pub(super) fn codegen_cache_configuration(
    profile: BuildProfile,
    pipeline_version: u32,
    producer_executable: &ProducerExecutableIdentity,
) -> String {
    format!(
        "profile-{}:compiler-exe-sha256:{}:runtime-exe-sha256:{}:pipeline-{}:driver-{:x}:cflags-{:x}:sqlite-{}:{}:{}:{:x}:{:x}",
        profile.as_str(),
        producer_executable.sha256,
        producer_executable.sha256,
        pipeline_version,
        Sha256::digest(release_driver_config_flags().join("\n")),
        Sha256::digest(release_c_flags().join("\n")),
        nomo_codegen_c::BUNDLED_SQLITE_VERSION,
        nomo_codegen_c::BUNDLED_SQLITE3_C_SHA256,
        nomo_codegen_c::BUNDLED_SQLITE3_H_SHA256,
        Sha256::digest(nomo_codegen_c::BUNDLED_SQLITE_COMPILE_OPTIONS.join("\n")),
        Sha256::digest(nomo_codegen_c::BUNDLED_SQLITE_RUNTIME_SOURCE.as_bytes())
    )
}

fn content_binding(
    profile: BuildProfile,
    target_triple: &str,
    cache_key: &str,
    producer_executable: &ProducerExecutableIdentity,
    generated_c: &FileRecord,
    compiler: Option<&CompilerIdentity>,
    compile_commands: &[CommandRecord],
    link_command: Option<&CommandRecord>,
    combined_compile_link_command: Option<&CommandRecord>,
    binary: Option<&FileRecord>,
    release_provenance: Option<&FileRecord>,
) -> Result<ContentBinding, BuildError> {
    let producer_json = canonical_compact_json(producer_executable).map_err(|error| {
        BuildError::Message(format!(
            "failed to encode producer identity for content binding: {}",
            error.human()
        ))
    })?;
    let compiler_json = canonical_compact_json(&compiler).map_err(|error| {
        BuildError::Message(format!(
            "failed to encode compiler identity for content binding: {}",
            error.human()
        ))
    })?;
    let commands_json = canonical_compact_json(&serde_json::json!({
        "compile_commands": compile_commands,
        "link_command": link_command,
        "combined_compile_link_command": combined_compile_link_command,
    }))
    .map_err(|error| {
        BuildError::Message(format!(
            "failed to encode build commands for content binding: {}",
            error.human()
        ))
    })?;
    let canonical_subdocuments = BTreeMap::from([
        ("commands".to_string(), commands_json.clone()),
        ("compiler_identity".to_string(), compiler_json.clone()),
        ("producer_identity".to_string(), producer_json.clone()),
    ]);
    let mut inputs = BTreeMap::new();
    inputs.insert("profile".to_string(), profile.as_str().to_string());
    inputs.insert("target_triple".to_string(), target_triple.to_string());
    inputs.insert("cache_key".to_string(), cache_key.to_string());
    inputs.insert(
        "producer_identity_sha256".to_string(),
        sha256_bytes(producer_json.as_bytes()),
    );
    inputs.insert(
        "compiler_identity_sha256".to_string(),
        sha256_bytes(compiler_json.as_bytes()),
    );
    inputs.insert(
        "commands_sha256".to_string(),
        sha256_bytes(commands_json.as_bytes()),
    );
    inputs.insert("generated_c_path".to_string(), generated_c.path.clone());
    inputs.insert("generated_c_sha256".to_string(), generated_c.sha256.clone());
    inputs.insert(
        "binary_path".to_string(),
        binary
            .map(|binary| binary.path.clone())
            .unwrap_or_else(|| "none".to_string()),
    );
    inputs.insert(
        "binary_sha256".to_string(),
        binary
            .map(|binary| binary.sha256.clone())
            .unwrap_or_else(|| "none".to_string()),
    );
    inputs.insert(
        "release_provenance_path".to_string(),
        release_provenance
            .map(|provenance| provenance.path.clone())
            .unwrap_or_else(|| "none".to_string()),
    );
    inputs.insert(
        "release_provenance_sha256".to_string(),
        release_provenance
            .map(|provenance| provenance.sha256.clone())
            .unwrap_or_else(|| "none".to_string()),
    );
    let mut hasher = Sha256::new();
    hash_framed(&mut hasher, CONTENT_BINDING_DOMAIN.as_bytes());
    for key in CONTENT_BINDING_INPUT_ORDER {
        hash_framed(&mut hasher, key.as_bytes());
        hash_framed(
            &mut hasher,
            inputs
                .get(*key)
                .expect("cache input order must match cache inputs")
                .as_bytes(),
        );
    }
    Ok(ContentBinding {
        schema: CONTENT_BINDING_SCHEMA,
        algorithm: "sha256".to_string(),
        domain: CONTENT_BINDING_DOMAIN.to_string(),
        formula: "sha256(concat(u64be(length(utf8(part))), utf8(part)) for domain, then each ordered input name and value)".to_string(),
        input_order: CONTENT_BINDING_INPUT_ORDER
            .iter()
            .map(|input| (*input).to_string())
            .collect(),
        inputs,
        canonical_subdocuments,
        sha256: format!("{:x}", hasher.finalize()),
    })
}

fn hash_framed(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_be_bytes());
    hasher.update(bytes);
}

fn canonical_build_environment() -> Result<CanonicalBuildEnvironment, BuildError> {
    CANONICAL_BUILD_ENVIRONMENT
        .get_or_init(|| compute_canonical_build_environment().map_err(|error| error.human()))
        .clone()
        .map_err(BuildError::Message)
}

fn compute_canonical_build_environment() -> Result<CanonicalBuildEnvironment, BuildError> {
    #[cfg(windows)]
    let (mut values, platform_authority): (
        BTreeMap<String, String>,
        Option<(&'static str, Value)>,
    ) = {
        let (values, authority) = windows_authority::canonical_windows_build_support()?;
        (values, Some(("windows_toolchain", authority)))
    };
    #[cfg(not(windows))]
    let (mut values, platform_authority): (
        BTreeMap<String, String>,
        Option<(&'static str, Value)>,
    ) = {
        let mut values = BTreeMap::new();
        for name in POSIX_RETAINED_ENVIRONMENT {
            if *name == "PATH" || *name == "GOENV" || *name == "SDKROOT" {
                continue;
            }
            if let Some(value) = env::var_os(name) {
                values.insert((*name).to_string(), value.to_string_lossy().into_owned());
            }
        }
        values.insert("GOENV".to_string(), "off".to_string());
        values.insert("PATH".to_string(), stable_build_path()?);
        values.insert("LC_ALL".to_string(), "C".to_string());
        values.insert("LANG".to_string(), "C".to_string());
        #[cfg(target_os = "macos")]
        {
            let authority = darwin_sdk_authority(&values)?;
            values.insert(
                "SDKROOT".to_string(),
                authority["sdkroot"]
                    .as_str()
                    .expect("Darwin SDK authority must include sdkroot")
                    .to_string(),
            );
            (values, Some(("darwin_sdk", authority)))
        }
        #[cfg(not(target_os = "macos"))]
        {
            (values, None)
        }
    };
    values.insert("GOENV".to_string(), "off".to_string());
    values.insert("LC_ALL".to_string(), "C".to_string());
    values.insert("LANG".to_string(), "C".to_string());
    let mut retained = BTreeMap::new();
    for (name, value) in &values {
        retained.insert(
            name.clone(),
            if PATH_ENVIRONMENT.contains(&name.as_str()) {
                resolved_environment_path(value)?
            } else {
                value.clone()
            },
        );
    }
    let cleared = COMPILER_AFFECTING_ENVIRONMENT
        .iter()
        .filter(|name| !retained.contains_key(**name))
        .map(|name| (*name).to_string())
        .collect::<Vec<_>>();
    let mut projection = serde_json::Map::new();
    projection.insert(
        "retained".to_string(),
        serde_json::to_value(retained).map_err(|error| {
            BuildError::Message(format!(
                "failed to encode retained build environment: {error}"
            ))
        })?,
    );
    projection.insert(
        "cleared".to_string(),
        serde_json::to_value(cleared).map_err(|error| {
            BuildError::Message(format!(
                "failed to encode cleared build environment: {error}"
            ))
        })?,
    );
    projection.insert("cleared_values_recorded".to_string(), Value::Bool(false));
    if let Some((name, authority)) = platform_authority {
        projection.insert(name.to_string(), authority);
    }
    Ok(CanonicalBuildEnvironment {
        values,
        projection: Value::Object(projection),
    })
}

fn ambient_build_environment_projection() -> Value {
    let retained = COMPILER_AFFECTING_ENVIRONMENT
        .iter()
        .filter_map(|name| {
            env::var(name)
                .ok()
                .map(|value| ((*name).to_string(), value))
        })
        .collect::<BTreeMap<_, _>>();
    serde_json::json!({
        "mode": "ambient-debug",
        "compiler_affecting": retained,
    })
}

#[cfg(not(windows))]
fn stable_build_path() -> Result<String, BuildError> {
    let home = env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| BuildError::Message("HOME is unavailable".to_string()))?;
    let candidates = [
        PathBuf::from("/opt/homebrew/bin"),
        PathBuf::from("/usr/local/bin"),
        home.join(".cargo/bin"),
        home.join("go/bin"),
        PathBuf::from("/Applications/Xcode.app/Contents/Developer/usr/bin"),
        PathBuf::from("/Library/Developer/CommandLineTools/usr/bin"),
        PathBuf::from("/usr/bin"),
        PathBuf::from("/bin"),
        PathBuf::from("/usr/sbin"),
        PathBuf::from("/sbin"),
        PathBuf::from("/snap/bin"),
    ];
    let mut entries = Vec::new();
    for candidate in candidates {
        let path = fs::canonicalize(&candidate).unwrap_or(candidate);
        let value = path.to_string_lossy().into_owned();
        if !path.is_dir()
            || value
                .replace('\\', "/")
                .to_ascii_lowercase()
                .contains("/.codex/tmp/arg0/")
            || entries.contains(&value)
        {
            continue;
        }
        entries.push(value);
    }
    if entries.is_empty() {
        return Err(BuildError::Message(
            "controlled build PATH has no stable directories".to_string(),
        ));
    }
    Ok(env::join_paths(entries)
        .map_err(|error| BuildError::Message(error.to_string()))?
        .to_string_lossy()
        .into_owned())
}

fn resolved_environment_path(value: &str) -> Result<String, BuildError> {
    let path = Path::new(value);
    let absolute = absolute_path(path)?;
    let resolved = fs::canonicalize(&absolute)
        .unwrap_or(absolute)
        .to_string_lossy()
        .into_owned();
    #[cfg(windows)]
    {
        if let Some(rest) = resolved.strip_prefix(r"\\?\UNC\") {
            return Ok(format!(r"\\{rest}"));
        }
        if let Some(rest) = resolved.strip_prefix(r"\\?\") {
            return Ok(rest.to_string());
        }
    }
    Ok(resolved)
}

#[cfg(target_os = "macos")]
fn darwin_sdk_authority(
    canonical_environment: &BTreeMap<String, String>,
) -> Result<Value, BuildError> {
    let xcrun = Path::new("/usr/bin/xcrun");
    if !xcrun.is_file() {
        return Err(BuildError::Message(
            "trusted /usr/bin/xcrun is unavailable".to_string(),
        ));
    }
    let mut environment = BTreeMap::from([
        (
            "PATH".to_string(),
            canonical_environment.get("PATH").cloned().ok_or_else(|| {
                BuildError::Message("PATH is unavailable for xcrun SDK selection".to_string())
            })?,
        ),
        ("LC_ALL".to_string(), "C".to_string()),
        ("LANG".to_string(), "C".to_string()),
    ]);
    if let Some(value) = canonical_environment.get("TMPDIR") {
        environment.insert("TMPDIR".to_string(), resolved_environment_path(value)?);
    }
    let argv = vec![
        "/usr/bin/xcrun".to_string(),
        "--sdk".to_string(),
        "macosx".to_string(),
        "--show-sdk-path".to_string(),
    ];
    let output = Command::new(&argv[0])
        .args(&argv[1..])
        .env_clear()
        .envs(&environment)
        .output()
        .map_err(|error| {
            BuildError::Message(format!(
                "failed to select the macOS SDK with xcrun: {error}"
            ))
        })?;
    if !output.status.success() || !output.stderr.is_empty() {
        return Err(BuildError::Message(format!(
            "xcrun SDK selection failed:\n{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    let sdkroot = PathBuf::from(String::from_utf8_lossy(&output.stdout).trim().to_string());
    if !sdkroot.is_absolute() || !sdkroot.is_dir() {
        return Err(BuildError::Message(format!(
            "xcrun selected an unavailable macOS SDK: {}",
            sdkroot.display()
        )));
    }
    let settings = [
        sdkroot.join("SDKSettings.plist"),
        sdkroot.join("SDKSettings.json"),
    ]
    .into_iter()
    .find(|path| path.is_file())
    .ok_or_else(|| {
        BuildError::Message("selected macOS SDK lacks SDKSettings authority".to_string())
    })?;
    let xcrun_realpath = fs::canonicalize(xcrun).map_err(|error| {
        BuildError::Message(format!("failed to resolve /usr/bin/xcrun: {error}"))
    })?;
    let selection_command = serde_json::json!({
        "argv": argv,
        "command": command_text(&[
            "/usr/bin/xcrun".to_string(),
            "--sdk".to_string(),
            "macosx".to_string(),
            "--show-sdk-path".to_string(),
        ]),
        "cwd": Value::Null,
        "exit_code": output.status.code().unwrap_or(0),
        "environment": environment,
    });
    Ok(serde_json::json!({
        "schema": 1,
        "xcrun": {
            "path": "/usr/bin/xcrun",
            "realpath": xcrun_realpath.to_string_lossy(),
            "sha256": sha256_file(&xcrun_realpath)?,
        },
        "selection_command": selection_command,
        "sdkroot": fs::canonicalize(&sdkroot).unwrap_or(sdkroot).to_string_lossy(),
        "sdk_settings": {
            "path": fs::canonicalize(&settings).unwrap_or(settings.clone()).to_string_lossy(),
            "sha256": sha256_file(&settings)?,
        },
    }))
}

fn command_text(argv: &[String]) -> String {
    argv.iter()
        .map(|argument| shell_quote(argument))
        .collect::<Vec<_>>()
        .join(" ")
}

fn shell_quote(argument: &str) -> String {
    if !argument.is_empty()
        && argument.chars().all(|character| {
            character.is_ascii_alphanumeric()
                || matches!(
                    character,
                    '_' | '@' | '%' | '+' | '=' | ':' | ',' | '.' | '/' | '-'
                )
        })
    {
        return argument.to_string();
    }
    format!("'{}'", argument.replace('\'', "'\"'\"'"))
}

fn absolute_path(path: &Path) -> Result<PathBuf, BuildError> {
    if path.is_absolute() {
        return Ok(path.to_path_buf());
    }
    Ok(env::current_dir()
        .map_err(|error| BuildError::Message(format!("failed to read current directory: {error}")))?
        .join(path))
}

fn sha256_file(path: &Path) -> Result<String, BuildError> {
    let bytes = fs::read(path).map_err(|error| {
        BuildError::Message(format!("failed to hash {}: {error}", path.display()))
    })?;
    Ok(sha256_bytes(&bytes))
}

fn sha256_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_SEQUENCE: AtomicU64 = AtomicU64::new(0);

    fn test_root(name: &str) -> PathBuf {
        let root = env::temp_dir().join(format!(
            "nomo-build-metadata-{name}-{}-{}",
            std::process::id(),
            TEST_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        if root.exists() {
            fs::remove_dir_all(&root).unwrap();
        }
        fs::create_dir_all(&root).unwrap();
        root
    }

    fn emit_only_metadata(root: &Path, profile: BuildProfile) -> BuildMetadata {
        let generated = root.join("main.c");
        fs::write(&generated, b"int main(void) { return 0; }\n").unwrap();
        let producer = producer_executable_identity().unwrap();
        let configuration = codegen_cache_configuration(profile, PASS_PIPELINE_VERSION, &producer);
        let query_key = QueryKey::new(
            "x86_64-unknown-linux-gnu",
            "codegen-c",
            format!("test:{configuration}"),
            crate::incremental::ContentFingerprint::of_text("test source"),
        );
        BuildMetadata::new(
            profile,
            "x86_64-unknown-linux-gnu".to_string(),
            query_key,
            producer,
            None,
            Vec::new(),
            None,
            None,
            FileRecord::from_path(&generated).unwrap(),
            None,
            None,
        )
        .unwrap()
    }

    fn native_metadata(root: &Path, profile: BuildProfile) -> BuildMetadata {
        let generated_path = root.join("main.c");
        let object_path = root.join("main.o");
        let binary_path = root.join("program");
        fs::write(&generated_path, b"int main(void) { return 0; }\n").unwrap();
        fs::write(&object_path, b"object").unwrap();
        fs::write(&binary_path, b"binary").unwrap();
        let invocation = env::current_exe().unwrap();
        let realpath = fs::canonicalize(&invocation).unwrap();
        let compiler = CompilerIdentity {
            path: invocation.to_string_lossy().into_owned(),
            realpath: realpath.to_string_lossy().into_owned(),
            sha256: sha256_file(&realpath).unwrap(),
            version_output: "test compiler".to_string(),
            target_triple: "x86_64-unknown-linux-gnu".to_string(),
        };
        let command = |argv: Vec<String>, duration_ns: u64| CommandRecord {
            command: command_text(&argv),
            argv,
            cwd: Some(root.to_string_lossy().into_owned()),
            duration_ns,
            exit_code: 0,
            environment: serde_json::json!({"retained": {}, "cleared": []}),
        };
        let compile = command(
            vec![
                compiler.path.clone(),
                "-c".to_string(),
                generated_path.to_string_lossy().into_owned(),
                "-o".to_string(),
                object_path.to_string_lossy().into_owned(),
            ],
            1,
        );
        let link = command(
            vec![
                compiler.path.clone(),
                object_path.to_string_lossy().into_owned(),
                "-o".to_string(),
                binary_path.to_string_lossy().into_owned(),
            ],
            2,
        );
        let generated_c = FileRecord::from_path(&generated_path).unwrap();
        let binary = FileRecord::from_path(&binary_path).unwrap();
        let producer = producer_executable_identity().unwrap();
        let configuration = codegen_cache_configuration(profile, PASS_PIPELINE_VERSION, &producer);
        let query_key = QueryKey::new(
            "x86_64-unknown-linux-gnu",
            "codegen-c",
            format!("test-native:{configuration}"),
            crate::incremental::ContentFingerprint::of_text("native test source"),
        );
        if profile == BuildProfile::Release {
            let provenance = ReleaseBackendProvenance::new(
                compiler.clone(),
                vec![FileRecord::from_path(&object_path).unwrap()],
                vec![compile.clone()],
                link.clone(),
                generated_c.clone(),
                binary.clone(),
            );
            let provenance_record =
                write_release_provenance(&root.join("release-provenance.json"), &provenance)
                    .unwrap();
            BuildMetadata::new(
                profile,
                "x86_64-unknown-linux-gnu".to_string(),
                query_key,
                producer,
                Some(compiler),
                vec![compile],
                Some(link),
                None,
                generated_c,
                Some(binary),
                Some(provenance_record),
            )
            .unwrap()
        } else {
            BuildMetadata::new(
                profile,
                "x86_64-unknown-linux-gnu".to_string(),
                query_key,
                producer,
                Some(compiler),
                Vec::new(),
                None,
                Some(compile),
                generated_c,
                Some(binary),
                None,
            )
            .unwrap()
        }
    }

    fn assert_metadata_mutation_rejected(
        path: &Path,
        metadata: &BuildMetadata,
        mutate: impl FnOnce(&mut Value),
    ) {
        write_build_metadata(path, metadata).unwrap();
        let mut value = serde_json::from_slice::<Value>(&fs::read(path).unwrap()).unwrap();
        mutate(&mut value);
        atomic_write_canonical_json(path, &value).unwrap();
        assert!(validate_build_metadata(path).is_err());
    }

    #[test]
    fn release_flags_match_rfc_0043_and_v2() {
        assert_eq!(
            release_c_flags(),
            ["-std=c99", "-O3", "-DNDEBUG", "-fomit-frame-pointer"]
        );
        assert_eq!(release_driver_config_flags(), ["--no-default-config"]);
        for forbidden in [
            "-ffast-math",
            "-Ofast",
            "-flto",
            "-fprofile-generate",
            "-march=native",
            "-mcpu=native",
        ] {
            assert!(!release_c_flags().contains(&forbidden));
        }
    }

    #[test]
    fn producer_identity_is_content_addressed_path_independent_and_fail_closed() {
        let root = test_root("producer");
        let first_path = root.join("first-nomo");
        let second_path = root.join("second-nomo");
        fs::write(&first_path, b"identical producer bytes").unwrap();
        fs::copy(&first_path, &second_path).unwrap();
        let first = producer_executable_identity_from_path(&first_path).unwrap();
        let second = producer_executable_identity_from_path(&second_path).unwrap();
        assert_ne!(first.path, second.path);
        assert_eq!(first.sha256, second.sha256);
        assert_eq!(first.size_bytes, second.size_bytes);
        assert_eq!(first.package_version, second.package_version);
        let first_configuration =
            codegen_cache_configuration(BuildProfile::Release, PASS_PIPELINE_VERSION, &first);
        let second_configuration =
            codegen_cache_configuration(BuildProfile::Release, PASS_PIPELINE_VERSION, &second);
        assert_eq!(
            first_configuration, second_configuration,
            "identical producer bytes at another path must retain one cache identity"
        );
        let fingerprint = crate::incremental::ContentFingerprint::of_text("producer-bound source");
        let first_key = QueryKey::new(
            "test-target",
            "codegen-c",
            first_configuration,
            fingerprint.clone(),
        );
        let second_key = QueryKey::new(
            "test-target",
            "codegen-c",
            second_configuration,
            fingerprint.clone(),
        );
        let cache = crate::incremental::PersistentQueryCache::at_root(&root.join("cache"));
        cache
            .insert(&first_key, &"generated by identical bytes".to_string())
            .unwrap();
        assert_eq!(
            cache.get::<String>(&second_key).as_deref(),
            Some("generated by identical bytes"),
            "identical producer bytes at another path must hit the actual persistent QueryKey"
        );

        fs::write(&second_path, b"identical producer byte!").unwrap();
        let changed = producer_executable_identity_from_path(&second_path).unwrap();
        assert_ne!(first.sha256, changed.sha256);
        assert_eq!(first.package_version, changed.package_version);
        let changed_configuration =
            codegen_cache_configuration(BuildProfile::Release, PASS_PIPELINE_VERSION, &changed);
        assert_ne!(
            second_key.identity, changed_configuration,
            "one changed producer byte must force a distinct cache identity"
        );
        let changed_key = QueryKey::new(
            "test-target",
            "codegen-c",
            changed_configuration,
            fingerprint,
        );
        assert_eq!(
            cache.get::<String>(&changed_key),
            None,
            "same package version with one changed producer byte must miss"
        );
        assert!(producer_executable_identity_from_path(&root.join("missing")).is_err());
        assert!(producer_executable_identity_from_path(&root).is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn command_rendering_matches_posix_shlex_join() {
        assert_eq!(
            command_text(&[
                "/usr/bin/clang".to_string(),
                "plain".to_string(),
                "two words".to_string(),
                "has'quote".to_string(),
                String::new(),
            ]),
            "/usr/bin/clang plain 'two words' 'has'\"'\"'quote' ''"
        );
    }

    #[test]
    fn metadata_round_trip_is_canonical_and_profile_bound() {
        let root = test_root("roundtrip");
        let path = root.join("nomo-build-metadata.json");
        let debug = emit_only_metadata(&root, BuildProfile::Debug);
        write_build_metadata(&path, &debug).unwrap();
        let decoded = validate_build_metadata(&path).unwrap();
        assert_eq!(decoded, debug);
        let value = serde_json::from_slice::<Value>(&fs::read(&path).unwrap()).unwrap();
        let producer_json = canonical_compact_json(&value["producer_executable"]).unwrap();
        let compiler_json = canonical_compact_json(&value["compiler"]).unwrap();
        let commands_json = canonical_compact_json(&serde_json::json!({
            "compile_commands": value["compile_commands"].clone(),
            "link_command": value["link_command"].clone(),
            "combined_compile_link_command": value["combined_compile_link_command"].clone(),
        }))
        .unwrap();
        assert_eq!(
            value["content_binding"]["canonical_subdocuments"]["producer_identity"],
            producer_json
        );
        assert_eq!(
            value["content_binding"]["canonical_subdocuments"]["compiler_identity"],
            compiler_json
        );
        assert_eq!(
            value["content_binding"]["canonical_subdocuments"]["commands"],
            commands_json
        );
        assert_eq!(
            fs::read(&path).unwrap(),
            canonical_json_bytes(&debug).unwrap()
        );

        let release = emit_only_metadata(&root, BuildProfile::Release);
        assert_ne!(
            debug.cache_identity.cache_key,
            release.cache_identity.cache_key
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn metadata_validation_rejects_tampering_and_stale_content() {
        let root = test_root("tamper");
        let path = root.join("nomo-build-metadata.json");
        let metadata = native_metadata(&root, BuildProfile::Debug);
        for input in CACHE_INPUT_ORDER {
            assert_metadata_mutation_rejected(&path, &metadata, |value| {
                let inputs = value["cache_identity"]["inputs"].as_object_mut().unwrap();
                let original = inputs.get(*input).and_then(Value::as_str).unwrap();
                inputs.insert(
                    (*input).to_string(),
                    Value::String(format!("{original}-tampered")),
                );
            });
        }
        assert_metadata_mutation_rejected(&path, &metadata, |value| {
            value["cache_identity"]["cache_key"] = Value::String("0".repeat(64));
        });
        assert_metadata_mutation_rejected(&path, &metadata, |value| {
            value["cache_identity"]["query_key_json"] =
                Value::String("{\"tampered\":true}".to_string());
        });
        assert_metadata_mutation_rejected(&path, &metadata, |value| {
            value["cache_identity"]["query_key"]["fingerprint"] =
                Value::String(format!("sha256:{}", "1".repeat(64)));
        });
        for input in CONTENT_BINDING_INPUT_ORDER {
            assert_metadata_mutation_rejected(&path, &metadata, |value| {
                let inputs = value["content_binding"]["inputs"].as_object_mut().unwrap();
                let original = inputs.get(*input).and_then(Value::as_str).unwrap();
                inputs.insert(
                    (*input).to_string(),
                    Value::String(format!("{original}-tampered")),
                );
            });
        }
        assert_metadata_mutation_rejected(&path, &metadata, |value| {
            value["content_binding"]["domain"] = Value::String("tampered-domain".to_string());
        });
        assert_metadata_mutation_rejected(&path, &metadata, |value| {
            value["content_binding"]["input_order"]
                .as_array_mut()
                .unwrap()
                .reverse();
        });
        assert_metadata_mutation_rejected(&path, &metadata, |value| {
            value["content_binding"]["sha256"] = Value::String("3".repeat(64));
        });
        assert_metadata_mutation_rejected(&path, &metadata, |value| {
            value["content_binding"]["canonical_subdocuments"]["commands"] =
                Value::String("{\"tampered\":true}".to_string());
        });
        assert_metadata_mutation_rejected(&path, &metadata, |value| {
            value["producer_executable"]["size_bytes"] = Value::from(1_u64);
        });
        assert_metadata_mutation_rejected(&path, &metadata, |value| {
            value["target_triple"] = Value::String("aarch64-unknown-linux-gnu".to_string());
        });
        assert_metadata_mutation_rejected(&path, &metadata, |value| {
            value["compiler"]["version_output"] =
                Value::String("tampered compiler version".to_string());
        });
        assert_metadata_mutation_rejected(&path, &metadata, |value| {
            value["combined_compile_link_command"]["duration_ns"] = Value::from(999_u64);
        });
        assert_metadata_mutation_rejected(&path, &metadata, |value| {
            value["combined_compile_link_command"]["cwd"] =
                Value::String("/tampered/build/checkout".to_string());
        });
        assert_metadata_mutation_rejected(&path, &metadata, |value| {
            value["combined_compile_link_command"]["argv"][1] = Value::String("-O3".to_string());
        });
        let alternate = root.join("alternate.c");
        fs::write(&alternate, b"int main(void) { return 0; }\n").unwrap();
        let alternate_record = FileRecord::from_path(&alternate).unwrap();
        assert_eq!(alternate_record.sha256, metadata.generated_c.sha256);
        assert_metadata_mutation_rejected(&path, &metadata, |value| {
            value["generated_c"] = serde_json::to_value(alternate_record).unwrap();
        });
        assert_metadata_mutation_rejected(&path, &metadata, |value| {
            value["generated_c"]["sha256"] = Value::String("2".repeat(64));
        });
        let alternate_binary = root.join("alternate-program");
        fs::write(&alternate_binary, b"binary").unwrap();
        let alternate_binary_record = FileRecord::from_path(&alternate_binary).unwrap();
        assert_eq!(
            alternate_binary_record.sha256,
            metadata.binary.as_ref().unwrap().sha256
        );
        assert_metadata_mutation_rejected(&path, &metadata, |value| {
            value["binary"] = serde_json::to_value(alternate_binary_record).unwrap();
        });
        write_build_metadata(&path, &metadata).unwrap();
        fs::write(root.join("main.c"), b"int main(void) { return 1; }\n").unwrap();
        assert!(validate_build_metadata(&path).is_err());

        let release = native_metadata(&root, BuildProfile::Release);
        let provenance_path = root.join("release-provenance.json");
        let alternate_provenance_path = root.join("alternate-release-provenance.json");
        fs::copy(&provenance_path, &alternate_provenance_path).unwrap();
        let alternate_provenance_record =
            FileRecord::from_path(&alternate_provenance_path).unwrap();
        assert_eq!(
            alternate_provenance_record.sha256,
            release.release_provenance.as_ref().unwrap().sha256
        );
        assert_metadata_mutation_rejected(&path, &release, |value| {
            value["release_provenance"] =
                serde_json::to_value(alternate_provenance_record).unwrap();
        });
        write_build_metadata(&path, &release).unwrap();
        let mut provenance =
            serde_json::from_slice::<Value>(&fs::read(&provenance_path).unwrap()).unwrap();
        provenance["link_command"]["duration_ns"] = Value::from(777_u64);
        atomic_write_canonical_json(&provenance_path, &provenance).unwrap();
        assert!(validate_build_metadata(&path).is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn metadata_atomic_replacement_leaves_no_temporary_file() {
        let root = test_root("atomic");
        let path = root.join("nomo-build-metadata.json");
        let debug = emit_only_metadata(&root, BuildProfile::Debug);
        let release = emit_only_metadata(&root, BuildProfile::Release);
        write_build_metadata(&path, &debug).unwrap();
        write_build_metadata(&path, &release).unwrap();
        assert_eq!(validate_build_metadata(&path).unwrap(), release);
        let temporary_files = fs::read_dir(&root)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().ends_with(".tmp"))
            .count();
        assert_eq!(temporary_files, 0);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn build_artifact_paths_are_absolute_and_non_regular_outputs_are_rejected() {
        let root = test_root("paths");
        let relative = root.join("main.c");
        fs::write(&relative, b"int main(void) { return 0; }\n").unwrap();
        let record = FileRecord::from_path(&relative).unwrap();
        assert!(Path::new(&record.path).is_absolute());

        let directory = root.join("nomo-build-metadata.json");
        fs::create_dir(&directory).unwrap();
        assert!(remove_stale_file(&directory).is_err());
        assert!(atomic_write_canonical_json(&directory, &serde_json::json!({})).is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn strict_release_sidecar_rejects_extra_keys() {
        let value = serde_json::json!({
            "schema": 1,
            "complete_argv": true,
            "compiler": {
                "path": "/clang",
                "realpath": "/clang",
                "sha256": "0".repeat(64),
                "version_output": "clang",
                "target_triple": "x86_64-unknown-linux-gnu",
            },
            "objects": [],
            "compile_commands": [],
            "link_command": {
                "argv": ["/clang"],
                "command": "/clang",
                "cwd": "/checkout",
                "duration_ns": 0,
                "exit_code": 0,
                "environment": {},
            },
            "generated_c": {"path": "/main.c", "sha256": "0".repeat(64)},
            "binary": {"path": "/main", "sha256": "0".repeat(64)},
            "selected_profile": "release",
        });
        assert!(serde_json::from_value::<ReleaseBackendProvenance>(value).is_err());
    }
}
