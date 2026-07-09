use super::{BuildError, Project, PublishPackage, prepare_publish_package};
use nomo_manifest::{validate_package_id, validate_package_segment, validate_version_like};
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::io::{Read, Write};
use std::net::TcpStream;
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
    upload_http_registry_archive(registry, &package.package, &package.version, &archive)
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
    if !registry.starts_with("http://") {
        return Err(format!(
            "registry search currently supports only http:// endpoints, got `{registry}`"
        ));
    }
    let request = http_registry_search_request(registry, query)?;
    let mut stream = TcpStream::connect((&*request.host, request.port)).map_err(|err| {
        format!(
            "failed to connect to registry `{}` for search: {err}",
            request.authority
        )
    })?;
    let request_text = format!(
        "GET {} HTTP/1.1\r\nHost: {}\r\nUser-Agent: nomo/0.1\r\nAccept: application/json\r\nConnection: close\r\n\r\n",
        request.path, request.authority
    );
    stream
        .write_all(request_text.as_bytes())
        .map_err(|err| format!("failed to request registry search: {err}"))?;
    let mut response = Vec::new();
    stream
        .read_to_end(&mut response)
        .map_err(|err| format!("failed to read registry search response: {err}"))?;
    parse_http_registry_search_response(registry, &response)
}

pub fn login_registry(registry: &str, token: &str) -> Result<RegistryLogin, String> {
    if !registry.starts_with("http://") {
        return Err(format!(
            "registry login currently supports only http:// endpoints, got `{registry}`"
        ));
    }
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
    if !registry.starts_with("http://") {
        return Err(format!(
            "registry owner management currently supports only http:// endpoints, got `{registry}`"
        ));
    }
    let request = http_registry_owner_add_request(registry, package, user)?;
    let mut stream = TcpStream::connect((&*request.host, request.port)).map_err(|err| {
        format!(
            "failed to connect to registry `{}` to add owner `{user}` for `{package}`: {err}",
            request.authority
        )
    })?;
    let authorization = registry_authorization_header(registry)?;
    let request_text = format!(
        "PUT {} HTTP/1.1\r\nHost: {}\r\nUser-Agent: nomo/0.1\r\n{}Content-Length: 0\r\nConnection: close\r\n\r\n",
        request.path, request.authority, authorization
    );
    stream.write_all(request_text.as_bytes()).map_err(|err| {
        format!("failed to request owner add for `{package}` user `{user}`: {err}")
    })?;
    let mut response = Vec::new();
    stream.read_to_end(&mut response).map_err(|err| {
        format!("failed to read registry owner response for `{package}` user `{user}`: {err}")
    })?;
    parse_http_registry_owner_add_response(registry, package, user, &response)
}

pub fn remove_registry_package_owner(
    registry: &str,
    package: &str,
    user: &str,
) -> Result<(), String> {
    validate_package_id(package)?;
    validate_package_segment("registry owner user", user)?;
    if !registry.starts_with("http://") {
        return Err(format!(
            "registry owner management currently supports only http:// endpoints, got `{registry}`"
        ));
    }
    let request = http_registry_owner_request(registry, package, user)?;
    let mut stream = TcpStream::connect((&*request.host, request.port)).map_err(|err| {
        format!(
            "failed to connect to registry `{}` to remove owner `{user}` from `{package}`: {err}",
            request.authority
        )
    })?;
    let authorization = registry_authorization_header(registry)?;
    let request_text = format!(
        "DELETE {} HTTP/1.1\r\nHost: {}\r\nUser-Agent: nomo/0.1\r\n{}Content-Length: 0\r\nConnection: close\r\n\r\n",
        request.path, request.authority, authorization
    );
    stream.write_all(request_text.as_bytes()).map_err(|err| {
        format!("failed to request owner remove for `{package}` user `{user}`: {err}")
    })?;
    let mut response = Vec::new();
    stream.read_to_end(&mut response).map_err(|err| {
        format!("failed to read registry owner response for `{package}` user `{user}`: {err}")
    })?;
    parse_http_registry_owner_remove_response(registry, package, user, &response)
}

pub fn yank_registry_package(registry: &str, package: &str, version: &str) -> Result<(), String> {
    validate_package_id(package)?;
    validate_version_like("package version", version)?;
    if !registry.starts_with("http://") {
        return Err(format!(
            "registry yank currently supports only http:// endpoints, got `{registry}`"
        ));
    }
    let request = http_registry_yank_request(registry, package, version)?;
    let mut stream = TcpStream::connect((&*request.host, request.port)).map_err(|err| {
        format!(
            "failed to connect to registry `{}` to yank `{package}` {version}: {err}",
            request.authority
        )
    })?;
    let authorization = registry_authorization_header(registry)?;
    let request_text = format!(
        "POST {} HTTP/1.1\r\nHost: {}\r\nUser-Agent: nomo/0.1\r\n{}Content-Length: 0\r\nConnection: close\r\n\r\n",
        request.path, request.authority, authorization
    );
    stream
        .write_all(request_text.as_bytes())
        .map_err(|err| format!("failed to request yank for `{package}` {version}: {err}"))?;
    let mut response = Vec::new();
    stream
        .read_to_end(&mut response)
        .map_err(|err| format!("failed to read registry yank response for `{package}`: {err}"))?;
    parse_http_registry_yank_response(registry, package, version, &response)
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
    let authorization = registry_authorization_header(registry)?;
    let request_text = format!(
        "PUT {} HTTP/1.1\r\nHost: {}\r\nUser-Agent: nomo/0.1\r\n{}Content-Type: application/octet-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        request.path,
        request.authority,
        authorization,
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

fn http_registry_upload_request(
    registry: &str,
    package: &str,
    version: &str,
) -> Result<HttpRegistryRequest, String> {
    http_registry_api_request(registry, package, version, "")
}

fn http_registry_yank_request(
    registry: &str,
    package: &str,
    version: &str,
) -> Result<HttpRegistryRequest, String> {
    http_registry_api_request(registry, package, version, "/yank")
}

fn http_registry_owner_add_request(
    registry: &str,
    package: &str,
    user: &str,
) -> Result<HttpRegistryRequest, String> {
    http_registry_owner_request(registry, package, user)
}

fn http_registry_owner_request(
    registry: &str,
    package: &str,
    user: &str,
) -> Result<HttpRegistryRequest, String> {
    let (host, port, authority, base_path) = http_registry_base(registry)?;
    let (owner, name) = package.split_once('/').ok_or_else(|| {
        format!("registry package `{package}` must use canonical owner/package form")
    })?;
    Ok(HttpRegistryRequest {
        host,
        port,
        authority,
        path: format!("{base_path}/api/v1/packages/{owner}/{name}/owners/{user}"),
    })
}

fn http_registry_search_request(
    registry: &str,
    query: &str,
) -> Result<HttpRegistryRequest, String> {
    let (host, port, authority, base_path) = http_registry_base(registry)?;
    Ok(HttpRegistryRequest {
        host,
        port,
        authority,
        path: format!(
            "{}/api/v1/packages?query={}",
            base_path.trim_end_matches('/'),
            percent_encode_query(query)
        ),
    })
}

fn http_registry_api_request(
    registry: &str,
    package: &str,
    version: &str,
    suffix: &str,
) -> Result<HttpRegistryRequest, String> {
    let (host, port, authority, base_path) = http_registry_base(registry)?;
    let (owner, name) = package.split_once('/').ok_or_else(|| {
        format!("registry package `{package}` must use canonical owner/package form")
    })?;
    Ok(HttpRegistryRequest {
        host,
        port,
        authority,
        path: format!("{base_path}/api/v1/packages/{owner}/{name}/{version}{suffix}"),
    })
}

fn http_registry_base(registry: &str) -> Result<(String, u16, String, String), String> {
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
    Ok((
        host,
        port,
        authority.to_string(),
        base_path.trim_end_matches('/').to_string(),
    ))
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

fn parse_http_registry_search_response(
    registry: &str,
    response: &[u8],
) -> Result<Vec<RegistrySearchResult>, String> {
    let Some(header_end) = response.windows(4).position(|window| window == b"\r\n\r\n") else {
        return Err(format!(
            "registry `{registry}` returned a malformed search response"
        ));
    };
    let headers = String::from_utf8(response[..header_end].to_vec())
        .map_err(|_| format!("registry `{registry}` returned non-UTF-8 search headers"))?;
    let status = headers
        .lines()
        .next()
        .ok_or_else(|| format!("registry `{registry}` returned an empty search response"))?;
    if !status.starts_with("HTTP/1.1 200 ") && !status.starts_with("HTTP/1.0 200 ") {
        return Err(format!("registry `{registry}` failed to search: {status}"));
    }
    let response: RegistrySearchResponse = serde_json::from_slice(&response[header_end + 4..])
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

fn parse_http_registry_yank_response(
    registry: &str,
    package: &str,
    version: &str,
    response: &[u8],
) -> Result<(), String> {
    let Some(header_end) = response.windows(4).position(|window| window == b"\r\n\r\n") else {
        return Err(format!(
            "registry `{registry}` returned a malformed yank response for `{package}` {version}"
        ));
    };
    let headers = String::from_utf8(response[..header_end].to_vec()).map_err(|_| {
        format!("registry `{registry}` returned non-UTF-8 yank headers for `{package}` {version}")
    })?;
    let status = headers
        .lines()
        .next()
        .ok_or_else(|| format!("registry `{registry}` returned an empty yank response"))?;
    if !status.starts_with("HTTP/1.1 200 ")
        && !status.starts_with("HTTP/1.1 202 ")
        && !status.starts_with("HTTP/1.1 204 ")
        && !status.starts_with("HTTP/1.0 200 ")
        && !status.starts_with("HTTP/1.0 202 ")
        && !status.starts_with("HTTP/1.0 204 ")
    {
        return Err(format!(
            "registry `{registry}` failed to yank `{package}` {version}: {status}"
        ));
    }
    Ok(())
}

fn parse_http_registry_owner_add_response(
    registry: &str,
    package: &str,
    user: &str,
    response: &[u8],
) -> Result<(), String> {
    let Some(header_end) = response.windows(4).position(|window| window == b"\r\n\r\n") else {
        return Err(format!(
            "registry `{registry}` returned a malformed owner response for `{package}` user `{user}`"
        ));
    };
    let headers = String::from_utf8(response[..header_end].to_vec()).map_err(|_| {
        format!(
            "registry `{registry}` returned non-UTF-8 owner headers for `{package}` user `{user}`"
        )
    })?;
    let status = headers
        .lines()
        .next()
        .ok_or_else(|| format!("registry `{registry}` returned an empty owner response"))?;
    if !status.starts_with("HTTP/1.1 200 ")
        && !status.starts_with("HTTP/1.1 201 ")
        && !status.starts_with("HTTP/1.1 204 ")
        && !status.starts_with("HTTP/1.0 200 ")
        && !status.starts_with("HTTP/1.0 201 ")
        && !status.starts_with("HTTP/1.0 204 ")
    {
        return Err(format!(
            "registry `{registry}` failed to add owner `{user}` for `{package}`: {status}"
        ));
    }
    Ok(())
}

fn parse_http_registry_owner_remove_response(
    registry: &str,
    package: &str,
    user: &str,
    response: &[u8],
) -> Result<(), String> {
    let Some(header_end) = response.windows(4).position(|window| window == b"\r\n\r\n") else {
        return Err(format!(
            "registry `{registry}` returned a malformed owner response for `{package}` user `{user}`"
        ));
    };
    let headers = String::from_utf8(response[..header_end].to_vec()).map_err(|_| {
        format!(
            "registry `{registry}` returned non-UTF-8 owner headers for `{package}` user `{user}`"
        )
    })?;
    let status = headers
        .lines()
        .next()
        .ok_or_else(|| format!("registry `{registry}` returned an empty owner response"))?;
    if !status.starts_with("HTTP/1.1 200 ")
        && !status.starts_with("HTTP/1.1 202 ")
        && !status.starts_with("HTTP/1.1 204 ")
        && !status.starts_with("HTTP/1.0 200 ")
        && !status.starts_with("HTTP/1.0 202 ")
        && !status.starts_with("HTTP/1.0 204 ")
    {
        return Err(format!(
            "registry `{registry}` failed to remove owner `{user}` from `{package}`: {status}"
        ));
    }
    Ok(())
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

fn registry_authorization_header(registry: &str) -> Result<String, String> {
    let Some(token) = registry_token(registry)? else {
        return Ok(String::new());
    };
    Ok(format!("Authorization: Bearer {token}\r\n"))
}

pub(super) fn registry_dependency_authorization(
    registry: Option<&str>,
) -> Result<Option<String>, String> {
    let Some(registry) = registry else {
        return Ok(None);
    };
    if !registry.starts_with("http://") {
        return Ok(None);
    }
    registry_authorization_header(registry).map(Some)
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
