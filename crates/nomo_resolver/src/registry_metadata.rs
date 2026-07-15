use super::registry_transport::{
    RegistryHttpMethod, RegistryHttpRequest, is_registry_http_endpoint, send_registry_http_request,
};
use nomo_manifest::{validate_package_id, validate_version_like};
use nomo_supply_chain::{
    PublisherKey, ReleaseProvenance, SignedReleaseEnvelope, TransparencyBundle,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct RegistryPackageMetadata {
    pub package: String,
    pub versions: Vec<RegistryVersionSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct RegistryVersionSummary {
    pub version: String,
    pub checksum: String,
    #[serde(default)]
    pub yanked: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct RegistryVersionMetadata {
    pub package: String,
    pub version: String,
    pub checksum: String,
    #[serde(default)]
    pub yanked: bool,
    #[serde(default)]
    pub publisher_keys: Vec<PublisherKey>,
    #[serde(default)]
    pub signature: Option<SignedReleaseEnvelope>,
    #[serde(default)]
    pub provenance: Option<ReleaseProvenance>,
    #[serde(default)]
    pub transparency: Option<TransparencyBundle>,
}

pub fn load_registry_package_metadata(
    registry: &str,
    package: &str,
    authorization: Option<&str>,
) -> Result<Option<RegistryPackageMetadata>, String> {
    validate_package_id(package)?;
    let (owner, name) = package_segments(package)?;
    let metadata = if let Some(root) = registry.strip_prefix("file://") {
        let path = Path::new(root)
            .join("api/v1/packages")
            .join(owner)
            .join(name)
            .join("index.json");
        read_optional_json_file(&path, "registry package metadata")?
    } else if is_registry_http_endpoint(registry) {
        let path = format!("/api/v1/packages/{owner}/{name}");
        Some(fetch_json(registry, &path, authorization, package, None)?)
    } else {
        return Ok(None);
    };
    let Some(metadata) = metadata else {
        return Ok(None);
    };
    validate_package_metadata(registry, package, &metadata)?;
    Ok(Some(metadata))
}

pub fn load_registry_version_metadata(
    registry: &str,
    package: &str,
    version: &str,
    authorization: Option<&str>,
) -> Result<Option<RegistryVersionMetadata>, String> {
    validate_package_id(package)?;
    validate_version_like("registry package version", version)?;
    let (owner, name) = package_segments(package)?;
    let metadata = if let Some(root) = registry.strip_prefix("file://") {
        let path = Path::new(root)
            .join("api/v1/packages")
            .join(owner)
            .join(name)
            .join(version)
            .join("metadata.json");
        read_optional_json_file(&path, "registry version metadata")?
    } else if is_registry_http_endpoint(registry) {
        let path = format!("/api/v1/packages/{owner}/{name}/{version}");
        Some(fetch_json(
            registry,
            &path,
            authorization,
            package,
            Some(version),
        )?)
    } else {
        return Ok(None);
    };
    let Some(metadata) = metadata else {
        return Ok(None);
    };
    validate_version_metadata(registry, package, version, &metadata)?;
    Ok(Some(metadata))
}

fn fetch_json<T: for<'de> Deserialize<'de>>(
    registry: &str,
    path: &str,
    authorization: Option<&str>,
    package: &str,
    version: Option<&str>,
) -> Result<T, String> {
    let response = send_registry_http_request(RegistryHttpRequest {
        endpoint: registry,
        path,
        method: RegistryHttpMethod::Get,
        accept: "application/json",
        content_type: None,
        authorization,
        body: &[],
    })?;
    if response.status != 200 {
        let subject = version
            .map(|version| format!("package `{package}` version `{version}`"))
            .unwrap_or_else(|| format!("package `{package}`"));
        return Err(format!(
            "registry `{registry}` failed to fetch metadata for {subject}: HTTP {}",
            response.status
        ));
    }
    serde_json::from_slice(&response.body).map_err(|err| {
        format!("registry `{registry}` returned invalid metadata JSON for `{package}`: {err}")
    })
}

fn read_optional_json_file<T: for<'de> Deserialize<'de>>(
    path: &Path,
    label: &str,
) -> Result<Option<T>, String> {
    if !path.is_file() {
        return Ok(None);
    }
    let bytes = fs::read(path)
        .map_err(|err| format!("failed to read {label} at {}: {err}", path.display()))?;
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|err| format!("invalid {label} at {}: {err}", path.display()))
}

fn validate_package_metadata(
    registry: &str,
    expected_package: &str,
    metadata: &RegistryPackageMetadata,
) -> Result<(), String> {
    if metadata.package != expected_package {
        return Err(format!(
            "registry `{registry}` returned package metadata for `{}`, expected `{expected_package}`",
            metadata.package
        ));
    }
    let mut versions = BTreeSet::new();
    for version in &metadata.versions {
        validate_version_like("registry metadata version", &version.version)?;
        validate_checksum(&version.checksum)?;
        if !versions.insert(version.version.as_str()) {
            return Err(format!(
                "registry `{registry}` package index for `{expected_package}` contains duplicate version `{}`",
                version.version
            ));
        }
    }
    Ok(())
}

fn validate_version_metadata(
    registry: &str,
    expected_package: &str,
    expected_version: &str,
    metadata: &RegistryVersionMetadata,
) -> Result<(), String> {
    if metadata.package != expected_package {
        return Err(format!(
            "registry `{registry}` returned version metadata for `{}`, expected `{expected_package}`",
            metadata.package
        ));
    }
    validate_version_like("registry metadata version", &metadata.version)?;
    if metadata.version != expected_version {
        return Err(format!(
            "registry `{registry}` returned version `{}`, expected `{expected_version}` for `{expected_package}`",
            metadata.version
        ));
    }
    validate_checksum(&metadata.checksum)?;
    if let Some(envelope) = &metadata.signature {
        envelope.subject.validate()?;
        if envelope.subject.package != expected_package
            || envelope.subject.version != expected_version
            || envelope.subject.archive_checksum != metadata.checksum
        {
            return Err(format!(
                "registry `{registry}` returned a signed release subject that does not match `{expected_package}` {expected_version}"
            ));
        }
    }
    Ok(())
}

fn validate_checksum(checksum: &str) -> Result<(), String> {
    let Some(hex) = checksum.strip_prefix("sha256:") else {
        return Err("registry metadata checksum must use `sha256:<hex>`".to_string());
    };
    if hex.len() == 64 && hex.chars().all(|ch| ch.is_ascii_hexdigit()) {
        Ok(())
    } else {
        Err("registry metadata checksum must contain 64 hexadecimal digits".to_string())
    }
}

fn package_segments(package: &str) -> Result<(&str, &str), String> {
    package.split_once('/').ok_or_else(|| {
        format!("registry package `{package}` must use canonical owner/package form")
    })
}

#[cfg(test)]
mod tests {
    use super::{load_registry_package_metadata, load_registry_version_metadata};
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

    #[test]
    fn loads_package_and_version_metadata_over_http() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            for (expected_path, body) in [
                (
                    "/api/v1/packages/fynn/utils",
                    r#"{"package":"fynn/utils","versions":[{"version":"0.1.0","checksum":"sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","yanked":false}]}"#,
                ),
                (
                    "/api/v1/packages/fynn/utils/0.1.0",
                    r#"{"package":"fynn/utils","version":"0.1.0","checksum":"sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","yanked":true}"#,
                ),
            ] {
                let (mut stream, _) = listener.accept().unwrap();
                let mut request = Vec::new();
                let mut buffer = [0_u8; 1024];
                loop {
                    let read = stream.read(&mut buffer).unwrap();
                    assert!(read > 0);
                    request.extend_from_slice(&buffer[..read]);
                    if request.windows(4).any(|window| window == b"\r\n\r\n") {
                        break;
                    }
                }
                let request = String::from_utf8(request).unwrap();
                assert!(request.starts_with(&format!("GET {expected_path} HTTP/1.1\r\n")));
                assert!(
                    request
                        .to_ascii_lowercase()
                        .contains("authorization: bearer metadata-token\r\n")
                );
                write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                )
                .unwrap();
            }
        });

        let registry = format!("http://{address}");
        let package =
            load_registry_package_metadata(&registry, "fynn/utils", Some("Bearer metadata-token"))
                .unwrap()
                .unwrap();
        assert_eq!(package.versions.len(), 1);
        let version = load_registry_version_metadata(
            &registry,
            "fynn/utils",
            "0.1.0",
            Some("Bearer metadata-token"),
        )
        .unwrap()
        .unwrap();
        assert!(version.yanked);
        server.join().unwrap();
    }

    #[test]
    fn rejects_duplicate_package_versions() {
        let root = std::env::temp_dir().join(format!(
            "nomo-registry-metadata-{}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        let path = root.join("api/v1/packages/fynn/utils");
        std::fs::create_dir_all(&path).unwrap();
        std::fs::write(
            path.join("index.json"),
            r#"{"package":"fynn/utils","versions":[{"version":"0.1.0","checksum":"sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"},{"version":"0.1.0","checksum":"sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"}]}"#,
        )
        .unwrap();
        let error = load_registry_package_metadata(
            &format!("file://{}", root.display()),
            "fynn/utils",
            None,
        )
        .unwrap_err();
        assert!(error.contains("duplicate version `0.1.0`"), "{error}");
        std::fs::remove_dir_all(root).unwrap();
    }
}
