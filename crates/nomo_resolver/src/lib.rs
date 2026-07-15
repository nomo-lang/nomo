use nomo_manifest::{PackageMetadata, parse_manifest_at_root};
use nomo_supply_chain::{
    CachedTreeHead, TrustPolicy, VerifiedReleaseEvidence, sha256_digest, verify_release_envelope,
    verify_transparency_bundle,
};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Write as _;
use std::path::{Component, Path, PathBuf};

mod registry_metadata;
mod registry_transport;
mod solver;

pub use nomo_manifest::{PackageVersion, VersionConstraint};
pub use registry_metadata::{
    RegistryPackageMetadata, RegistryVersionMetadata, RegistryVersionSummary,
    load_registry_package_metadata, load_registry_version_metadata,
};
pub use registry_transport::{
    RegistryHttpMethod, RegistryHttpRequest, RegistryHttpResponse, is_registry_http_endpoint,
    send_registry_http_request, validate_registry_http_endpoint,
};
pub use solver::{ConstraintOrigin, ResolutionConflict, VersionCandidate, select_highest_version};

/// Resolve one manifest requirement to an exact registry version.
///
/// Exact requirements do not require an index request. Ranges load and cache
/// the package index, then use the deterministic candidate selector. Offline
/// range resolution requires a previously cached index (or a local `file://`
/// registry).
pub fn resolve_registry_version(
    base_root: &Path,
    alias: &str,
    package: &str,
    requirement: &str,
    registry: Option<&str>,
    offline: bool,
    authorization_header: Option<&str>,
) -> Result<String, String> {
    let requirement = VersionConstraint::parse(requirement)?;
    if let VersionConstraint::Exact(version) = requirement {
        return Ok(version.to_string());
    }

    let registry = registry.ok_or_else(|| {
        format!(
            "registry dependency `{alias}` package `{package}` uses range `{requirement}` but no registry endpoint is configured"
        )
    })?;
    let candidates = load_registry_version_candidates(
        base_root,
        alias,
        package,
        registry,
        offline,
        authorization_header,
    )?;
    let origin = ConstraintOrigin {
        requirement,
        dependency_path: vec![alias.to_string(), package.to_string()],
    };
    select_highest_version(package, &candidates, &[origin])
        .map(|version| version.to_string())
        .map_err(|conflict| conflict.render())
}

pub fn load_registry_version_candidates(
    base_root: &Path,
    alias: &str,
    package: &str,
    registry: &str,
    offline: bool,
    authorization_header: Option<&str>,
) -> Result<Vec<VersionCandidate>, String> {
    let metadata = if offline && !registry.starts_with("file://") {
        read_cached_registry_package_metadata(base_root, package, registry)?.ok_or_else(|| {
            format!(
                "offline resolution has no cached package index for dependency `{alias}` package `{package}` from `{registry}`"
            )
        })?
    } else {
        let metadata =
            load_registry_package_metadata(registry, package, authorization_header)?.ok_or_else(
                || {
                    format!(
                        "registry `{registry}` does not contain package index metadata for dependency `{alias}` package `{package}`"
                    )
                },
            )?;
        if !registry.starts_with("file://") {
            write_cached_registry_package_metadata(base_root, registry, &metadata)?;
        }
        metadata
    };
    metadata
        .versions
        .iter()
        .map(|candidate| {
            let version = PackageVersion::parse(&candidate.version)?;
            Ok(VersionCandidate {
                version,
                yanked: candidate.yanked,
            })
        })
        .collect::<Result<Vec<_>, String>>()
}

pub fn package_checksum(root: &Path) -> Result<String, String> {
    let mut files = package_source_files(root)?;
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

pub fn build_package_archive(root: &Path, package: &PackageMetadata) -> Result<Vec<u8>, String> {
    let mut files = package_source_files(root)?;
    files.sort();

    let mut archive = Vec::new();
    writeln!(&mut archive, "nomo-package-v1").expect("write to Vec cannot fail");
    writeln!(
        &mut archive,
        "package {}/{}",
        package.namespace, package.name
    )
    .expect("write to Vec cannot fail");
    writeln!(&mut archive, "version {}", package.version).expect("write to Vec cannot fail");
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

pub fn package_archive_filename(package: &str, version: &str) -> String {
    format!("{}-{}.nomo-package", package.replace('/', "-"), version)
}

pub fn archive_checksum(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("sha256:{}", hex_lower(&hasher.finalize()))
}

pub fn unpack_package_archive(
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

pub fn resolve_registry_source(
    base_root: &Path,
    alias: &str,
    package: &str,
    version: &str,
    registry: Option<&str>,
    offline: bool,
    authorization_header: Option<&str>,
) -> Result<Option<PathBuf>, String> {
    resolve_registry_source_with_policy(
        base_root,
        alias,
        package,
        version,
        registry,
        offline,
        authorization_header,
        TrustPolicy::ChecksumOnly,
        &[],
    )
    .map(|resolved| resolved.root)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedRegistrySource {
    pub root: Option<PathBuf>,
    pub evidence: Option<VerifiedReleaseEvidence>,
}

#[allow(clippy::too_many_arguments)]
pub fn resolve_registry_source_with_policy(
    base_root: &Path,
    alias: &str,
    package: &str,
    version: &str,
    registry: Option<&str>,
    offline: bool,
    authorization_header: Option<&str>,
    trust_policy: TrustPolicy,
    trusted_transparency_keys: &[String],
) -> Result<VerifiedRegistrySource, String> {
    let metadata = match registry {
        Some(registry) if registry.starts_with("file://") => {
            load_registry_version_metadata(registry, package, version, authorization_header)?
        }
        Some(registry) if offline => {
            read_cached_registry_version_metadata(base_root, package, version, registry)?
        }
        Some(registry) => {
            let metadata =
                load_registry_version_metadata(registry, package, version, authorization_header)?;
            if let Some(metadata) = &metadata {
                write_cached_registry_version_metadata(base_root, registry, metadata)?;
            }
            metadata
        }
        None => None,
    };
    if trust_policy != TrustPolicy::ChecksumOnly && metadata.is_none() {
        return Err(format!(
            "registry dependency `{alias}` requires `{}` trust metadata, but none is available{}",
            trust_policy.as_str(),
            if offline { " in the offline cache" } else { "" }
        ));
    }
    if metadata.as_ref().is_some_and(|metadata| metadata.yanked) {
        return Err(format!(
            "registry dependency `{alias}` package `{package}` version `{version}` is yanked; an existing lockfile may continue to use it"
        ));
    }
    if let Some(source_root) = registry_cached_source_root(base_root, package, version, registry)? {
        if let Some(metadata) = &metadata {
            let archive_path = registry_cache_root(base_root, package, version, registry)
                .join("package.nomo-package");
            if archive_path.is_file() {
                let archive = fs::read(&archive_path).map_err(|err| {
                    format!(
                        "failed to read cached registry dependency `{alias}` archive at {}: {err}",
                        archive_path.display()
                    )
                })?;
                verify_registry_archive_checksum(alias, metadata, &archive)?;
                let evidence = verify_registry_supply_chain(
                    base_root,
                    registry.expect("registry metadata requires a registry endpoint"),
                    alias,
                    metadata,
                    trust_policy,
                    trusted_transparency_keys,
                )?;
                unpack_package_archive(&archive, package, version, &source_root)?;
                verify_registry_manifest_checksum(metadata, &source_root, trust_policy)?;
                return Ok(VerifiedRegistrySource {
                    root: Some(source_root),
                    evidence,
                });
            } else {
                return Err(format!(
                    "cached registry dependency `{alias}` is missing its verified archive at {}",
                    archive_path.display()
                ));
            }
        }
        return Ok(VerifiedRegistrySource {
            root: Some(source_root),
            evidence: None,
        });
    }
    if offline {
        return Ok(VerifiedRegistrySource {
            root: None,
            evidence: None,
        });
    }
    let Some(registry) = registry else {
        return Ok(VerifiedRegistrySource {
            root: None,
            evidence: None,
        });
    };
    let Some(archive) =
        read_registry_archive(alias, registry, package, version, authorization_header)?
    else {
        return Ok(VerifiedRegistrySource {
            root: None,
            evidence: None,
        });
    };
    let evidence = if let Some(metadata) = &metadata {
        verify_registry_archive_checksum(alias, metadata, &archive)?;
        verify_registry_supply_chain(
            base_root,
            registry,
            alias,
            metadata,
            trust_policy,
            trusted_transparency_keys,
        )?
    } else {
        None
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
    if let Some(metadata) = &metadata {
        verify_registry_manifest_checksum(metadata, &source_root, trust_policy)?;
    }
    Ok(VerifiedRegistrySource {
        root: Some(source_root),
        evidence,
    })
}

fn verify_registry_supply_chain(
    base_root: &Path,
    registry: &str,
    alias: &str,
    metadata: &RegistryVersionMetadata,
    trust_policy: TrustPolicy,
    trusted_transparency_keys: &[String],
) -> Result<Option<VerifiedReleaseEvidence>, String> {
    if trust_policy == TrustPolicy::ChecksumOnly {
        return Ok(None);
    }
    let envelope = metadata.signature.as_ref().ok_or_else(|| {
        format!(
            "registry dependency `{alias}` requires a publisher signature under `{}` policy",
            trust_policy.as_str()
        )
    })?;
    verify_release_envelope(envelope, &envelope.subject, &metadata.publisher_keys).map_err(
        |error| format!("registry dependency `{alias}` publisher verification failed: {error}"),
    )?;
    if let Some(expected) = envelope.subject.provenance_digest.as_deref() {
        let provenance = metadata
            .provenance
            .as_ref()
            .ok_or_else(|| format!("registry dependency `{alias}` is missing signed provenance"))?;
        let rendered = provenance.render()?;
        if sha256_digest(rendered.as_bytes()) != expected {
            return Err(format!(
                "registry dependency `{alias}` provenance digest does not match the signed release"
            ));
        }
    }

    let mut evidence = VerifiedReleaseEvidence {
        key_id: envelope.signature.key_id.clone(),
        subject_digest: envelope.subject.digest()?,
        provenance_digest: envelope.subject.provenance_digest.clone(),
        transparency_root: None,
        transparency_size: None,
    };
    if trust_policy == TrustPolicy::SignedTransparent {
        let bundle = metadata.transparency.as_ref().ok_or_else(|| {
            format!("registry dependency `{alias}` requires a transparency inclusion proof")
        })?;
        let head_path = registry_trust_head_cache_path(base_root, registry);
        let cached = read_cached_tree_head(&head_path)?;
        let verified = verify_transparency_bundle(
            bundle,
            envelope,
            cached.as_ref(),
            trusted_transparency_keys,
        )
        .map_err(|error| {
            format!("registry dependency `{alias}` transparency verification failed: {error}")
        })?;
        write_cached_tree_head(&head_path, &verified)?;
        evidence.transparency_root = Some(verified.root_hash);
        evidence.transparency_size = Some(verified.tree_size);
    }
    Ok(Some(evidence))
}

fn verify_registry_manifest_checksum(
    metadata: &RegistryVersionMetadata,
    source_root: &Path,
    trust_policy: TrustPolicy,
) -> Result<(), String> {
    if trust_policy == TrustPolicy::ChecksumOnly {
        return Ok(());
    }
    let Some(envelope) = &metadata.signature else {
        return Ok(());
    };
    let path = source_root.join("nomo.toml");
    let bytes = fs::read(&path).map_err(|err| {
        format!(
            "failed to read signed manifest at {}: {err}",
            path.display()
        )
    })?;
    let actual = sha256_digest(&bytes);
    if actual == envelope.subject.manifest_checksum {
        Ok(())
    } else {
        Err(format!(
            "signed manifest checksum mismatch for `{}` {}: expected {}, found {actual}",
            metadata.package, metadata.version, envelope.subject.manifest_checksum
        ))
    }
}

fn verify_registry_archive_checksum(
    alias: &str,
    metadata: &RegistryVersionMetadata,
    archive: &[u8],
) -> Result<(), String> {
    let actual = archive_checksum(archive);
    if actual == metadata.checksum {
        Ok(())
    } else {
        Err(format!(
            "registry dependency `{alias}` archive checksum mismatch: expected {}, found {actual}",
            metadata.checksum
        ))
    }
}

pub fn registry_cached_source_root(
    base_root: &Path,
    package: &str,
    version: &str,
    registry: Option<&str>,
) -> Result<Option<PathBuf>, String> {
    let source_root = registry_cache_root(base_root, package, version, registry).join("source");
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

pub fn collect_source_files(dir: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
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

pub fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

pub fn package_source_files(root: &Path) -> Result<Vec<PathBuf>, String> {
    let canonical_root = fs::canonicalize(root).map_err(|err| {
        format!(
            "failed to resolve package root at {}: {err}",
            root.display()
        )
    })?;
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
    let parsed = parse_manifest_at_root(root)?;
    for source in parsed.ffi.sources {
        if !source.is_file() {
            return Err(format!(
                "package FFI source is missing: {}",
                source.display()
            ));
        }
        source.strip_prefix(root).map_err(|_| {
            format!(
                "package FFI source must be inside the package root: {}",
                source.display()
            )
        })?;
        files.push(source);
    }
    for file in &files {
        let canonical_file = fs::canonicalize(file).map_err(|err| {
            format!(
                "failed to resolve package file at {}: {err}",
                file.display()
            )
        })?;
        if canonical_file.strip_prefix(&canonical_root).is_err() {
            return Err(format!(
                "package file escapes the package root through a symbolic link: {}",
                file.display()
            ));
        }
    }
    files.sort();
    files.dedup();
    Ok(files)
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

fn registry_package_index_cache_path(base_root: &Path, package: &str, registry: &str) -> PathBuf {
    let mut root = base_root.join(".nomo/cache/registry");
    for segment in package.split('/') {
        root.push(segment);
    }
    root.push("index");
    root.push(registry_cache_key(Some(registry)));
    root.push("index.json");
    root
}

fn registry_version_metadata_cache_path(
    base_root: &Path,
    package: &str,
    version: &str,
    registry: &str,
) -> PathBuf {
    registry_cache_root(base_root, package, version, Some(registry)).join("metadata.json")
}

fn read_cached_registry_version_metadata(
    base_root: &Path,
    package: &str,
    version: &str,
    registry: &str,
) -> Result<Option<RegistryVersionMetadata>, String> {
    let path = registry_version_metadata_cache_path(base_root, package, version, registry);
    if !path.is_file() {
        return Ok(None);
    }
    let bytes = fs::read(&path).map_err(|err| {
        format!(
            "failed to read cached registry version metadata at {}: {err}",
            path.display()
        )
    })?;
    let metadata: RegistryVersionMetadata = serde_json::from_slice(&bytes).map_err(|err| {
        format!(
            "cached registry version metadata at {} is invalid: {err}",
            path.display()
        )
    })?;
    if metadata.package != package || metadata.version != version {
        return Err(format!(
            "cached registry version metadata at {} contains `{} {}`, expected `{package} {version}`",
            path.display(),
            metadata.package,
            metadata.version
        ));
    }
    Ok(Some(metadata))
}

fn write_cached_registry_version_metadata(
    base_root: &Path,
    registry: &str,
    metadata: &RegistryVersionMetadata,
) -> Result<(), String> {
    let path = registry_version_metadata_cache_path(
        base_root,
        &metadata.package,
        &metadata.version,
        registry,
    );
    write_json_cache(&path, metadata, "registry version metadata")
}

fn registry_trust_head_cache_path(base_root: &Path, registry: &str) -> PathBuf {
    base_root
        .join(".nomo/cache/registry/trust")
        .join(registry_cache_key(Some(registry)))
        .join("tree-head.json")
}

fn read_cached_tree_head(path: &Path) -> Result<Option<CachedTreeHead>, String> {
    if !path.is_file() {
        return Ok(None);
    }
    let bytes = fs::read(path).map_err(|err| {
        format!(
            "failed to read cached transparency head at {}: {err}",
            path.display()
        )
    })?;
    serde_json::from_slice(&bytes).map(Some).map_err(|err| {
        format!(
            "cached transparency head at {} is invalid: {err}",
            path.display()
        )
    })
}

fn write_cached_tree_head(path: &Path, head: &CachedTreeHead) -> Result<(), String> {
    write_json_cache(path, head, "transparency head")
}

fn write_json_cache<T: serde::Serialize>(
    path: &Path,
    value: &T,
    label: &str,
) -> Result<(), String> {
    let parent = path
        .parent()
        .expect("registry cache path always has a parent");
    fs::create_dir_all(parent).map_err(|err| {
        format!(
            "failed to create {label} cache at {}: {err}",
            parent.display()
        )
    })?;
    let bytes =
        serde_json::to_vec(value).map_err(|err| format!("failed to encode {label}: {err}"))?;
    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, bytes).map_err(|err| {
        format!(
            "failed to write {label} cache at {}: {err}",
            temporary.display()
        )
    })?;
    fs::rename(&temporary, path).map_err(|err| {
        format!(
            "failed to install {label} cache at {}: {err}",
            path.display()
        )
    })
}

fn read_cached_registry_package_metadata(
    base_root: &Path,
    package: &str,
    registry: &str,
) -> Result<Option<RegistryPackageMetadata>, String> {
    let path = registry_package_index_cache_path(base_root, package, registry);
    if !path.is_file() {
        return Ok(None);
    }
    let bytes = fs::read(&path).map_err(|err| {
        format!(
            "failed to read cached registry package index at {}: {err}",
            path.display()
        )
    })?;
    let metadata: RegistryPackageMetadata = serde_json::from_slice(&bytes).map_err(|err| {
        format!(
            "cached registry package index at {} is invalid: {err}",
            path.display()
        )
    })?;
    if metadata.package != package {
        return Err(format!(
            "cached registry package index at {} contains `{}`, expected `{package}`",
            path.display(),
            metadata.package
        ));
    }
    Ok(Some(metadata))
}

fn write_cached_registry_package_metadata(
    base_root: &Path,
    registry: &str,
    metadata: &RegistryPackageMetadata,
) -> Result<(), String> {
    let path = registry_package_index_cache_path(base_root, &metadata.package, registry);
    let parent = path
        .parent()
        .expect("registry package index cache path has a parent");
    fs::create_dir_all(parent).map_err(|err| {
        format!(
            "failed to create registry package index cache at {}: {err}",
            parent.display()
        )
    })?;
    let bytes = serde_json::to_vec(metadata)
        .map_err(|err| format!("failed to encode registry package index: {err}"))?;
    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, bytes).map_err(|err| {
        format!(
            "failed to write registry package index cache at {}: {err}",
            temporary.display()
        )
    })?;
    fs::rename(&temporary, &path).map_err(|err| {
        format!(
            "failed to install registry package index cache at {}: {err}",
            path.display()
        )
    })
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
    authorization_header: Option<&str>,
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
    if is_registry_http_endpoint(registry) {
        return fetch_registry_archive(alias, registry, package, version, authorization_header)
            .map(Some);
    }
    Ok(None)
}

fn fetch_registry_archive(
    alias: &str,
    registry: &str,
    package: &str,
    version: &str,
    authorization_header: Option<&str>,
) -> Result<Vec<u8>, String> {
    let (owner, name) = package.split_once('/').ok_or_else(|| {
        format!("registry package `{package}` must use canonical owner/package form")
    })?;
    let path = format!("/api/v1/packages/{owner}/{name}/{version}/download");
    let authorization = authorization_header
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let response = send_registry_http_request(RegistryHttpRequest {
        endpoint: registry,
        path: &path,
        method: RegistryHttpMethod::Get,
        accept: "application/octet-stream",
        content_type: None,
        authorization,
        body: &[],
    })
    .map_err(|err| format!("failed to fetch registry dependency `{alias}`: {err}"))?;
    if response.status != 200 {
        return Err(format!(
            "registry `{registry}` failed to fetch dependency `{alias}`: HTTP {}",
            response.status
        ));
    }
    Ok(response.body)
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

fn validate_checksum(label: &str, checksum: &str) -> Result<(), String> {
    let Some(hex) = checksum.strip_prefix("sha256:") else {
        return Err(format!("{label} checksum must use `sha256:<hex>`"));
    };
    let valid = hex.len() == 64 && hex.chars().all(|ch| ch.is_ascii_hexdigit());
    if valid {
        Ok(())
    } else {
        Err(format!(
            "{label} checksum must contain 64 hexadecimal digits"
        ))
    }
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

#[cfg(test)]
mod tests {
    use super::{
        RegistryPackageMetadata, RegistryVersionMetadata, RegistryVersionSummary, archive_checksum,
        build_package_archive, registry_trust_head_cache_path, resolve_registry_source_with_policy,
        resolve_registry_version, verify_registry_archive_checksum,
        write_cached_registry_package_metadata, write_cached_tree_head,
    };
    use nomo_manifest::{PackageMetadata, RegistryTrustPolicy};
    use nomo_supply_chain::{
        CachedTreeHead, ExternalSignerResponse, PROVENANCE_SCHEMA, PublisherKey, ReleaseProvenance,
        ReleaseSubject, SignedTreeHead, TransparencyBundle, TransparencyEvent,
        TransparencyEventKind, TransparencyLog, encode_hex, envelope_from_signer_response,
        publisher_key_id, sha256_digest,
    };
    use ring::signature::{Ed25519KeyPair, KeyPair};
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_root(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "nomo-resolver-{label}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn rejects_registry_archive_checksum_mismatch() {
        let metadata = RegistryVersionMetadata {
            package: "fynn/utils".to_string(),
            version: "0.1.0".to_string(),
            checksum: format!("sha256:{}", "0".repeat(64)),
            yanked: false,
            publisher_keys: Vec::new(),
            signature: None,
            provenance: None,
            transparency: None,
        };
        let error =
            verify_registry_archive_checksum("utils", &metadata, b"not-the-archive").unwrap_err();
        assert!(error.contains("archive checksum mismatch"), "{error}");
    }

    #[test]
    fn signed_policy_verifies_file_registry_and_returns_lockfile_evidence() {
        let root = temp_root("signed-file-registry");
        let package_root = root.join("package");
        let registry = root.join("registry");
        let registry_version = registry.join("api/v1/packages/nomo-lang/demo/1.0.0");
        fs::create_dir_all(package_root.join("src")).unwrap();
        fs::create_dir_all(&registry_version).unwrap();
        let manifest = b"[package]\nnamespace = \"nomo-lang\"\nname = \"demo\"\nversion = \"1.0.0\"\nedition = \"2026\"\n";
        fs::write(package_root.join("nomo.toml"), manifest).unwrap();
        fs::write(
            package_root.join("src/main.nomo"),
            "package demo.main\n\npub fn answer() -> i64 {\n    return 42\n}\n",
        )
        .unwrap();
        let package_metadata = PackageMetadata {
            namespace: "nomo-lang".to_string(),
            name: "demo".to_string(),
            version: "1.0.0".to_string(),
            edition: "2026".to_string(),
        };
        let archive = build_package_archive(&package_root, &package_metadata).unwrap();
        let checksum = archive_checksum(&archive);
        fs::write(registry_version.join("download"), &archive).unwrap();

        let provenance = ReleaseProvenance {
            schema: PROVENANCE_SCHEMA,
            builder: "independent-test-builder".to_string(),
            builder_version: "1".to_string(),
            package: "nomo-lang/demo".to_string(),
            version: "1.0.0".to_string(),
            archive_checksum: checksum.clone(),
            manifest_checksum: sha256_digest(manifest),
        };
        let provenance_text = provenance.render().unwrap();
        let subject = ReleaseSubject::new(
            "nomo-lang/demo".to_string(),
            "1.0.0".to_string(),
            checksum.clone(),
            sha256_digest(manifest),
            Some(sha256_digest(provenance_text.as_bytes())),
        )
        .unwrap();
        let seed = [7_u8; 32];
        let key = Ed25519KeyPair::from_seed_unchecked(&seed).unwrap();
        let public_key = encode_hex(key.public_key().as_ref());
        let key_id = publisher_key_id(key.public_key().as_ref());
        let envelope = envelope_from_signer_response(
            subject.clone(),
            ExternalSignerResponse {
                algorithm: "ed25519".to_string(),
                key_id: Some(key_id.clone()),
                public_key: public_key.clone(),
                signature: encode_hex(key.sign(&subject.canonical_bytes().unwrap()).as_ref()),
            },
        )
        .unwrap();
        let mut metadata = RegistryVersionMetadata {
            package: "nomo-lang/demo".to_string(),
            version: "1.0.0".to_string(),
            checksum,
            yanked: false,
            publisher_keys: vec![PublisherKey {
                key_id: key_id.clone(),
                public_key,
            }],
            signature: Some(envelope),
            provenance: Some(provenance),
            transparency: None,
        };
        fs::write(
            registry_version.join("metadata.json"),
            serde_json::to_vec(&metadata).unwrap(),
        )
        .unwrap();
        let endpoint = format!("file://{}", registry.display());
        let resolved = resolve_registry_source_with_policy(
            &root,
            "demo",
            "nomo-lang/demo",
            "1.0.0",
            Some(&endpoint),
            false,
            None,
            RegistryTrustPolicy::Signed,
            &[],
        )
        .unwrap();
        assert!(resolved.root.unwrap().join("src/main.nomo").is_file());
        let evidence = resolved.evidence.unwrap();
        assert_eq!(evidence.key_id, key_id);
        assert!(evidence.provenance_digest.is_some());
        assert!(evidence.transparency_root.is_none());
        let serialized = serde_json::to_string(&metadata).unwrap();
        assert!(!serialized.contains(&encode_hex(&seed)));

        let envelope = metadata.signature.clone().unwrap();
        let log = TransparencyLog::new(vec![
            TransparencyEvent {
                sequence: 0,
                kind: TransparencyEventKind::KeyRegistered {
                    package: "nomo-lang/demo".to_string(),
                    key_id: key_id.clone(),
                    public_key: metadata.publisher_keys[0].public_key.clone(),
                },
            },
            TransparencyEvent {
                sequence: 1,
                kind: TransparencyEventKind::Release {
                    package: "nomo-lang/demo".to_string(),
                    version: "1.0.0".to_string(),
                    subject_digest: envelope.subject.digest().unwrap(),
                    key_id: key_id.clone(),
                },
            },
        ])
        .unwrap();
        let mut head = SignedTreeHead {
            tree_size: 2,
            root_hash: log.root_hash().unwrap(),
            algorithm: "ed25519".to_string(),
            key_id: key_id.clone(),
            signature: String::new(),
        };
        head.signature = encode_hex(key.sign(&head.canonical_bytes().unwrap()).as_ref());
        metadata.transparency = Some(TransparencyBundle {
            head: head.clone(),
            log_public_key: metadata.publisher_keys[0].public_key.clone(),
            release: log.inclusion(1).unwrap(),
            key_events: vec![log.inclusion(0).unwrap()],
        });
        let trusted_log_key = metadata.publisher_keys[0].public_key.clone();
        fs::write(
            registry_version.join("metadata.json"),
            serde_json::to_vec(&metadata).unwrap(),
        )
        .unwrap();
        let transparent_root = root.join("transparent-client");
        fs::create_dir_all(&transparent_root).unwrap();
        let transparent = resolve_registry_source_with_policy(
            &transparent_root,
            "demo",
            "nomo-lang/demo",
            "1.0.0",
            Some(&endpoint),
            false,
            None,
            RegistryTrustPolicy::SignedTransparent,
            std::slice::from_ref(&trusted_log_key),
        )
        .unwrap();
        let transparent_evidence = transparent.evidence.unwrap();
        assert_eq!(
            transparent_evidence.transparency_root,
            Some(head.root_hash.clone())
        );
        assert_eq!(transparent_evidence.transparency_size, Some(2));
        let head_cache = registry_trust_head_cache_path(&transparent_root, &endpoint);
        write_cached_tree_head(
            &head_cache,
            &CachedTreeHead {
                tree_size: 3,
                root_hash: sha256_digest(b"newer-head"),
            },
        )
        .unwrap();
        let rollback_error = resolve_registry_source_with_policy(
            &transparent_root,
            "demo",
            "nomo-lang/demo",
            "1.0.0",
            Some(&endpoint),
            false,
            None,
            RegistryTrustPolicy::SignedTransparent,
            std::slice::from_ref(&trusted_log_key),
        )
        .unwrap_err();
        assert!(rollback_error.contains("rollback"), "{rollback_error}");

        let mut unsigned = metadata;
        unsigned.signature = None;
        fs::write(
            registry_version.join("metadata.json"),
            serde_json::to_vec(&unsigned).unwrap(),
        )
        .unwrap();
        let error = resolve_registry_source_with_policy(
            &root,
            "demo",
            "nomo-lang/demo",
            "1.0.0",
            Some(&endpoint),
            false,
            None,
            RegistryTrustPolicy::Signed,
            &[],
        )
        .unwrap_err();
        assert!(error.contains("requires a publisher signature"), "{error}");

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn resolves_a_range_from_a_file_registry_index() {
        let root = temp_root("file-index");
        let registry = root.join("registry");
        let index = registry.join("api/v1/packages/nomo-lang/json/index.json");
        fs::create_dir_all(index.parent().unwrap()).unwrap();
        fs::write(
            &index,
            format!(
                r#"{{"package":"nomo-lang/json","versions":[{{"version":"1.2.0","checksum":"sha256:{}","yanked":false}},{{"version":"1.9.0","checksum":"sha256:{}","yanked":false}},{{"version":"2.0.0","checksum":"sha256:{}","yanked":false}}]}}"#,
                "a".repeat(64),
                "b".repeat(64),
                "c".repeat(64)
            ),
        )
        .unwrap();
        let endpoint = format!("file://{}", registry.display());

        let selected = resolve_registry_version(
            &root,
            "json",
            "nomo-lang/json",
            "^1.2.0",
            Some(&endpoint),
            false,
            None,
        )
        .unwrap();

        assert_eq!(selected, "1.9.0");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn offline_range_resolution_uses_the_cached_index() {
        let root = temp_root("offline-index");
        let registry = "https://packages.example.com";
        write_cached_registry_package_metadata(
            &root,
            registry,
            &RegistryPackageMetadata {
                package: "nomo-lang/json".to_string(),
                versions: vec![
                    RegistryVersionSummary {
                        version: "1.4.0".to_string(),
                        checksum: format!("sha256:{}", "a".repeat(64)),
                        yanked: false,
                    },
                    RegistryVersionSummary {
                        version: "1.8.0".to_string(),
                        checksum: format!("sha256:{}", "b".repeat(64)),
                        yanked: false,
                    },
                ],
            },
        )
        .unwrap();

        let selected = resolve_registry_version(
            &root,
            "json",
            "nomo-lang/json",
            ">=1.0, <2.0",
            Some(registry),
            true,
            None,
        )
        .unwrap();

        assert_eq!(selected, "1.8.0");
        fs::remove_dir_all(root).unwrap();
    }
}
