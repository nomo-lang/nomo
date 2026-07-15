use super::{BuildError, Project, PublishPackage, prepare_publish_package, sign_publish_package};
use nomo_manifest::{validate_package_id, validate_package_segment, validate_version_like};
use nomo_resolver::{
    RegistryHttpMethod, RegistryHttpRequest, RegistryHttpResponse, is_registry_http_endpoint,
    send_registry_http_request, validate_registry_http_endpoint,
};
use nomo_supply_chain::{PublisherKey, decode_hex, publisher_key_id};
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistrySearchResult {
    pub package: String,
    pub version: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistryLogin {
    pub credentials_path: PathBuf,
    pub registry: String,
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
    upload_registry_archive(registry, &package.package, &package.version, &archive)
        .map_err(BuildError::Message)?;
    Ok(package)
}

pub fn publish_signed_package_archive(
    project: &Project,
    registry: &str,
    output_dir: Option<&Path>,
    signer: &str,
    envelope_path: Option<&Path>,
) -> Result<PublishPackage, BuildError> {
    let package = sign_publish_package(
        prepare_publish_package(project, output_dir)?,
        signer,
        envelope_path,
    )?;
    let archive = fs::read(&package.archive_path).map_err(|err| {
        BuildError::Message(format!(
            "failed to read {} for registry upload: {err}",
            package.archive_path.display()
        ))
    })?;
    upload_registry_archive(registry, &package.package, &package.version, &archive)
        .map_err(BuildError::Message)?;
    let provenance = fs::read(&package.provenance_path).map_err(|err| {
        BuildError::Message(format!(
            "failed to read {} for registry upload: {err}",
            package.provenance_path.display()
        ))
    })?;
    upload_registry_release_document(
        registry,
        &package.package,
        &package.version,
        "/provenance",
        "provenance",
        &provenance,
    )
    .map_err(BuildError::Message)?;
    let envelope_path = package
        .envelope_path
        .as_ref()
        .expect("signed package has an envelope path");
    let envelope = fs::read(envelope_path).map_err(|err| {
        BuildError::Message(format!(
            "failed to read {} for registry upload: {err}",
            envelope_path.display()
        ))
    })?;
    upload_registry_release_document(
        registry,
        &package.package,
        &package.version,
        "/attestation",
        "attestation",
        &envelope,
    )
    .map_err(BuildError::Message)?;
    Ok(package)
}

pub fn search_registry_packages(
    registry: &str,
    query: &str,
) -> Result<Vec<RegistrySearchResult>, String> {
    if query.trim().is_empty() {
        return Err("nomo search requires a non-empty query".to_string());
    }
    let path = format!("/api/v1/packages?query={}", percent_encode_query(query));
    let response = registry_request(
        registry,
        &path,
        RegistryHttpMethod::Get,
        "application/json",
        None,
        &[],
    )
    .map_err(|err| format!("failed to request registry search: {err}"))?;
    ensure_registry_status(registry, "search", response.status, &[200])?;
    parse_registry_search_response(registry, &response.body)
}

pub fn login_registry(registry: &str, token: &str) -> Result<RegistryLogin, String> {
    validate_registry_http_endpoint(registry)?;
    validate_registry_token(token)?;
    let endpoint = canonical_registry_endpoint(registry);
    let credentials_path = registry_credentials_path()?;
    let mut document = read_registry_credentials(&credentials_path)?;
    upsert_registry_credential(&mut document, &endpoint, token);
    write_registry_credentials(&credentials_path, &document)?;
    Ok(RegistryLogin {
        credentials_path,
        registry: endpoint,
    })
}

pub fn add_registry_package_owner(registry: &str, package: &str, user: &str) -> Result<(), String> {
    validate_package_id(package)?;
    validate_package_segment("registry owner user", user)?;
    let path = registry_owner_path(package, user)?;
    let response = registry_request(
        registry,
        &path,
        RegistryHttpMethod::Put,
        "application/json",
        None,
        &[],
    )
    .map_err(|err| format!("failed to request owner add for `{package}` user `{user}`: {err}"))?;
    ensure_registry_status(
        registry,
        &format!("add owner `{user}` for `{package}`"),
        response.status,
        &[200, 201, 204],
    )
}

pub fn remove_registry_package_owner(
    registry: &str,
    package: &str,
    user: &str,
) -> Result<(), String> {
    validate_package_id(package)?;
    validate_package_segment("registry owner user", user)?;
    let path = registry_owner_path(package, user)?;
    let response = registry_request(
        registry,
        &path,
        RegistryHttpMethod::Delete,
        "application/json",
        None,
        &[],
    )
    .map_err(|err| {
        format!("failed to request owner remove for `{package}` user `{user}`: {err}")
    })?;
    ensure_registry_status(
        registry,
        &format!("remove owner `{user}` from `{package}`"),
        response.status,
        &[200, 202, 204],
    )
}

pub fn add_registry_publisher_key(
    registry: &str,
    package: &str,
    public_key: &str,
) -> Result<String, String> {
    validate_package_id(package)?;
    let public_key_bytes = decode_hex(public_key)?;
    if public_key_bytes.len() != 32 {
        return Err("ed25519 publisher public key must contain 32 bytes".to_string());
    }
    let key_id = publisher_key_id(&public_key_bytes);
    let path = registry_publisher_key_path(package, &key_id)?;
    let body = serde_json::to_vec(&PublisherKey {
        key_id: key_id.clone(),
        public_key: public_key.to_ascii_lowercase(),
    })
    .map_err(|err| err.to_string())?;
    let response = registry_request(
        registry,
        &path,
        RegistryHttpMethod::Put,
        "application/json",
        Some("application/json"),
        &body,
    )
    .map_err(|err| format!("failed to register publisher key for `{package}`: {err}"))?;
    ensure_registry_status(
        registry,
        &format!("register publisher key `{key_id}` for `{package}`"),
        response.status,
        &[200, 201, 204],
    )?;
    Ok(key_id)
}

pub fn revoke_registry_publisher_key(
    registry: &str,
    package: &str,
    key_id: &str,
) -> Result<(), String> {
    validate_package_id(package)?;
    let digest = key_id
        .strip_prefix("sha256:")
        .ok_or_else(|| "publisher key id must use sha256:<hex>".to_string())?;
    if digest.len() != 64 || !digest.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("publisher key id must contain 64 hexadecimal digits".to_string());
    }
    let path = registry_publisher_key_path(package, key_id)?;
    let response = registry_request(
        registry,
        &path,
        RegistryHttpMethod::Delete,
        "application/json",
        None,
        &[],
    )
    .map_err(|err| format!("failed to revoke publisher key `{key_id}`: {err}"))?;
    ensure_registry_status(
        registry,
        &format!("revoke publisher key `{key_id}` for `{package}`"),
        response.status,
        &[200, 202, 204],
    )
}

pub fn yank_registry_package(registry: &str, package: &str, version: &str) -> Result<(), String> {
    validate_package_id(package)?;
    validate_version_like("package version", version)?;
    let path = registry_package_version_path(package, version, "/yank")?;
    let response = registry_request(
        registry,
        &path,
        RegistryHttpMethod::Post,
        "application/json",
        None,
        &[],
    )
    .map_err(|err| format!("failed to request yank for `{package}` {version}: {err}"))?;
    ensure_registry_status(
        registry,
        &format!("yank `{package}` {version}"),
        response.status,
        &[200, 202, 204],
    )
}

fn upload_registry_archive(
    registry: &str,
    package: &str,
    version: &str,
    archive: &[u8],
) -> Result<(), String> {
    let path = registry_package_version_path(package, version, "")?;
    let response = registry_request(
        registry,
        &path,
        RegistryHttpMethod::Put,
        "application/json",
        Some("application/octet-stream"),
        archive,
    )
    .map_err(|err| format!("failed to upload package `{package}`: {err}"))?;
    ensure_registry_status(
        registry,
        &format!("publish `{package}` {version}"),
        response.status,
        &[200, 201, 204],
    )
}

fn upload_registry_release_document(
    registry: &str,
    package: &str,
    version: &str,
    suffix: &str,
    label: &str,
    body: &[u8],
) -> Result<(), String> {
    let path = registry_package_version_path(package, version, suffix)?;
    let response = registry_request(
        registry,
        &path,
        RegistryHttpMethod::Put,
        "application/json",
        Some("application/json"),
        body,
    )
    .map_err(|err| format!("failed to upload package {label}: {err}"))?;
    ensure_registry_status(
        registry,
        &format!("upload {label} for `{package}` {version}"),
        response.status,
        &[200, 201, 204],
    )
}

fn registry_request(
    registry: &str,
    path: &str,
    method: RegistryHttpMethod,
    accept: &str,
    content_type: Option<&str>,
    body: &[u8],
) -> Result<RegistryHttpResponse, String> {
    validate_registry_http_endpoint(registry)?;
    let authorization = registry_authorization_value(registry)?;
    send_registry_http_request(RegistryHttpRequest {
        endpoint: registry,
        path,
        method,
        accept,
        content_type,
        authorization: authorization.as_deref(),
        body,
    })
}

fn registry_owner_path(package: &str, user: &str) -> Result<String, String> {
    let (owner, name) = package.split_once('/').ok_or_else(|| {
        format!("registry package `{package}` must use canonical owner/package form")
    })?;
    Ok(format!("/api/v1/packages/{owner}/{name}/owners/{user}"))
}

fn registry_publisher_key_path(package: &str, key_id: &str) -> Result<String, String> {
    let (owner, name) = package.split_once('/').ok_or_else(|| {
        format!("registry package `{package}` must use canonical owner/package form")
    })?;
    Ok(format!(
        "/api/v1/packages/{owner}/{name}/publisher-keys/{key_id}"
    ))
}

fn registry_package_version_path(
    package: &str,
    version: &str,
    suffix: &str,
) -> Result<String, String> {
    let (owner, name) = package.split_once('/').ok_or_else(|| {
        format!("registry package `{package}` must use canonical owner/package form")
    })?;
    Ok(format!("/api/v1/packages/{owner}/{name}/{version}{suffix}"))
}

fn ensure_registry_status(
    registry: &str,
    operation: &str,
    status: u16,
    expected: &[u16],
) -> Result<(), String> {
    if expected.contains(&status) {
        Ok(())
    } else {
        Err(format!(
            "registry `{registry}` failed to {operation}: HTTP {status}"
        ))
    }
}

fn parse_registry_search_response(
    registry: &str,
    body: &[u8],
) -> Result<Vec<RegistrySearchResult>, String> {
    let response: RegistrySearchResponse = serde_json::from_slice(body)
        .map_err(|err| format!("registry `{registry}` returned invalid search JSON: {err}"))?;
    let results = match response {
        RegistrySearchResponse::Items(items) => items,
        RegistrySearchResponse::Packages { packages } => packages,
        RegistrySearchResponse::Results { results } => results,
    };
    results
        .into_iter()
        .map(|item| {
            validate_package_id(&item.package)?;
            if let Some(version) = &item.version {
                validate_version_like("package version", version)?;
            }
            Ok(RegistrySearchResult {
                package: item.package,
                version: item.version,
                description: item
                    .description
                    .filter(|description| !description.is_empty()),
            })
        })
        .collect()
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum RegistrySearchResponse {
    Items(Vec<RegistrySearchItem>),
    Packages { packages: Vec<RegistrySearchItem> },
    Results { results: Vec<RegistrySearchItem> },
}

#[derive(Debug, Deserialize)]
struct RegistrySearchItem {
    package: String,
    #[serde(default)]
    version: Option<String>,
    #[serde(default)]
    description: Option<String>,
}

#[derive(Debug, Default, Deserialize, Serialize)]
struct RegistryCredentialsDocument {
    #[serde(default)]
    registry: Vec<RegistryCredentialEntry>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct RegistryCredentialEntry {
    endpoint: String,
    token: String,
}

fn registry_authorization_value(registry: &str) -> Result<Option<String>, String> {
    Ok(registry_token(registry)?.map(|token| format!("Bearer {token}")))
}

pub(super) fn registry_dependency_authorization(
    registry: Option<&str>,
) -> Result<Option<String>, String> {
    let Some(registry) = registry else {
        return Ok(None);
    };
    if !is_registry_http_endpoint(registry) {
        return Ok(None);
    }
    registry_authorization_value(registry)
}

fn registry_token(registry: &str) -> Result<Option<String>, String> {
    let credentials_path = registry_credentials_path()?;
    if !credentials_path.is_file() {
        return Ok(None);
    }
    let endpoint = canonical_registry_endpoint(registry);
    let document = read_registry_credentials(&credentials_path)?;
    Ok(document
        .registry
        .into_iter()
        .find(|entry| canonical_registry_endpoint(&entry.endpoint) == endpoint)
        .map(|entry| entry.token))
}

fn read_registry_credentials(path: &Path) -> Result<RegistryCredentialsDocument, String> {
    if !path.is_file() {
        return Ok(RegistryCredentialsDocument::default());
    }
    let text = fs::read_to_string(path).map_err(|err| {
        format!(
            "failed to read registry credentials at {}: {err}",
            path.display()
        )
    })?;
    toml::from_str(&text).map_err(|err| {
        format!(
            "failed to parse registry credentials at {}: {err}",
            path.display()
        )
    })
}

fn write_registry_credentials(
    path: &Path,
    document: &RegistryCredentialsDocument,
) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            format!(
                "failed to create registry credentials directory {}: {err}",
                parent.display()
            )
        })?;
    }
    let mut text = toml::to_string_pretty(document)
        .map_err(|err| format!("failed to render registry credentials: {err}"))?;
    if !text.ends_with('\n') {
        text.push('\n');
    }
    fs::write(path, text).map_err(|err| {
        format!(
            "failed to write registry credentials at {}: {err}",
            path.display()
        )
    })
}

fn upsert_registry_credential(
    document: &mut RegistryCredentialsDocument,
    endpoint: &str,
    token: &str,
) {
    if let Some(entry) = document
        .registry
        .iter_mut()
        .find(|entry| canonical_registry_endpoint(&entry.endpoint) == endpoint)
    {
        entry.endpoint = endpoint.to_string();
        entry.token = token.to_string();
        return;
    }
    document.registry.push(RegistryCredentialEntry {
        endpoint: endpoint.to_string(),
        token: token.to_string(),
    });
}

fn registry_credentials_path() -> Result<PathBuf, String> {
    let root = if let Some(value) = env::var_os("NOMO_HOME") {
        PathBuf::from(value)
    } else if let Some(value) = env::var_os("HOME") {
        PathBuf::from(value).join(".nomo")
    } else {
        return Err("NOMO_HOME or HOME must be set to store registry credentials".to_string());
    };
    Ok(root.join("credentials.toml"))
}

fn canonical_registry_endpoint(registry: &str) -> String {
    registry.trim_end_matches('/').to_string()
}

fn validate_registry_token(token: &str) -> Result<(), String> {
    if token.is_empty() {
        return Err("registry token cannot be empty".to_string());
    }
    if token.contains('\r') || token.contains('\n') {
        return Err("registry token cannot contain newlines".to_string());
    }
    Ok(())
}

fn percent_encode_query(query: &str) -> String {
    let mut encoded = String::new();
    for byte in query.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char)
            }
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }
    encoded
}
