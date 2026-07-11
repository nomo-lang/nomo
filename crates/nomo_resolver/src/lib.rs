use nomo_manifest::{PackageMetadata, parse_manifest_at_root};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Write as _;
use std::path::{Component, Path, PathBuf};

mod registry_metadata;
mod registry_transport;

pub use registry_metadata::{
    RegistryPackageMetadata, RegistryVersionMetadata, RegistryVersionSummary,
    load_registry_package_metadata, load_registry_version_metadata,
};
pub use registry_transport::{
    RegistryHttpMethod, RegistryHttpRequest, RegistryHttpResponse, is_registry_http_endpoint,
    send_registry_http_request, validate_registry_http_endpoint,
};

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
    let metadata = match registry {
        Some(registry) if !offline || registry.starts_with("file://") => {
            load_registry_version_metadata(registry, package, version, authorization_header)?
        }
        _ => None,
    };
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
                unpack_package_archive(&archive, package, version, &source_root)?;
            } else {
                return Err(format!(
                    "cached registry dependency `{alias}` is missing its verified archive at {}",
                    archive_path.display()
                ));
            }
        }
        return Ok(Some(source_root));
    }
    if offline {
        return Ok(None);
    }
    let Some(registry) = registry else {
        return Ok(None);
    };
    let Some(archive) =
        read_registry_archive(alias, registry, package, version, authorization_header)?
    else {
        return Ok(None);
    };
    if let Some(metadata) = &metadata {
        verify_registry_archive_checksum(alias, metadata, &archive)?;
    }
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
    use super::{RegistryVersionMetadata, verify_registry_archive_checksum};

    #[test]
    fn rejects_registry_archive_checksum_mismatch() {
        let metadata = RegistryVersionMetadata {
            package: "fynn/utils".to_string(),
            version: "0.1.0".to_string(),
            checksum: format!("sha256:{}", "0".repeat(64)),
            yanked: false,
        };
        let error =
            verify_registry_archive_checksum("utils", &metadata, b"not-the-archive").unwrap_err();
        assert!(error.contains("archive checksum mismatch"), "{error}");
    }
}
