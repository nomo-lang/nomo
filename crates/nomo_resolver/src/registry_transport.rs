use ureq::tls::{RootCerts, TlsConfig};

const MAX_REGISTRY_RESPONSE_BYTES: u64 = 128 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegistryHttpMethod {
    Get,
    Put,
    Post,
    Delete,
}

impl RegistryHttpMethod {
    fn as_str(self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Put => "PUT",
            Self::Post => "POST",
            Self::Delete => "DELETE",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct RegistryHttpRequest<'a> {
    pub endpoint: &'a str,
    pub path: &'a str,
    pub method: RegistryHttpMethod,
    pub accept: &'a str,
    pub content_type: Option<&'a str>,
    pub authorization: Option<&'a str>,
    pub body: &'a [u8],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistryHttpResponse {
    pub status: u16,
    pub body: Vec<u8>,
}

pub fn is_registry_http_endpoint(endpoint: &str) -> bool {
    endpoint.starts_with("http://") || endpoint.starts_with("https://")
}

pub fn validate_registry_http_endpoint(endpoint: &str) -> Result<(), String> {
    if !is_registry_http_endpoint(endpoint) {
        return Err(format!(
            "registry endpoint `{endpoint}` must start with http:// or https://"
        ));
    }
    if endpoint.contains('?') || endpoint.contains('#') {
        return Err(format!(
            "registry endpoint `{endpoint}` cannot contain a query or fragment"
        ));
    }
    let authority = endpoint
        .split_once("://")
        .map(|(_, rest)| rest)
        .and_then(|rest| rest.split('/').next())
        .unwrap_or_default();
    if authority.is_empty() {
        return Err("registry endpoint is missing a host".to_string());
    }
    Ok(())
}

pub fn send_registry_http_request(
    request: RegistryHttpRequest<'_>,
) -> Result<RegistryHttpResponse, String> {
    let agent = ureq::Agent::config_builder()
        .http_status_as_error(false)
        .tls_config(
            TlsConfig::builder()
                .root_certs(RootCerts::PlatformVerifier)
                .build(),
        )
        .build()
        .new_agent();
    send_registry_http_request_with_agent(request, &agent)
}

fn send_registry_http_request_with_agent(
    request: RegistryHttpRequest<'_>,
    agent: &ureq::Agent,
) -> Result<RegistryHttpResponse, String> {
    validate_registry_http_endpoint(request.endpoint)?;
    let url = registry_request_url(request.endpoint, request.path);

    macro_rules! headers {
        ($builder:expr) => {{
            let builder = $builder
                .header("User-Agent", "nomo/0.1")
                .header("Accept", request.accept);
            let builder = match request.authorization {
                Some(value) => builder.header("Authorization", value),
                None => builder,
            };
            match request.content_type {
                Some(value) => builder.header("Content-Type", value),
                None => builder,
            }
        }};
    }

    let response = match request.method {
        RegistryHttpMethod::Get => headers!(agent.get(&url)).call(),
        RegistryHttpMethod::Delete => headers!(agent.delete(&url))
            .force_send_body()
            .send(request.body),
        RegistryHttpMethod::Put => headers!(agent.put(&url)).send(request.body),
        RegistryHttpMethod::Post => headers!(agent.post(&url)).send(request.body),
    }
    .map_err(|err| {
        format!(
            "registry request {} {url} failed: {err}",
            request.method.as_str()
        )
    })?;
    let status = response.status().as_u16();
    let mut body = response.into_body();
    let body = body
        .with_config()
        .limit(MAX_REGISTRY_RESPONSE_BYTES)
        .read_to_vec()
        .map_err(|err| {
            format!(
                "failed to read registry response for {} {url}: {err}",
                request.method.as_str()
            )
        })?;
    Ok(RegistryHttpResponse { status, body })
}

fn registry_request_url(endpoint: &str, path: &str) -> String {
    let endpoint = endpoint.trim_end_matches('/');
    if path.starts_with('/') {
        format!("{endpoint}{path}")
    } else {
        format!("{endpoint}/{path}")
    }
}

#[cfg(test)]
mod tests {
    use super::{
        RegistryHttpMethod, RegistryHttpRequest, is_registry_http_endpoint,
        send_registry_http_request, send_registry_http_request_with_agent,
        validate_registry_http_endpoint,
    };
    use rcgen::{CertifiedKey, generate_simple_self_signed};
    use rustls::pki_types::{PrivateKeyDer, PrivatePkcs8KeyDer};
    use rustls::{ServerConfig, ServerConnection, StreamOwned};
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::Arc;
    use std::thread;
    use ureq::tls::{Certificate, RootCerts, TlsConfig};

    #[test]
    fn validates_http_and_https_registry_endpoints() {
        assert!(is_registry_http_endpoint("http://packages.example.test"));
        assert!(is_registry_http_endpoint(
            "https://packages.example.test/api"
        ));
        assert!(validate_registry_http_endpoint("https://packages.example.test/api").is_ok());
        assert!(validate_registry_http_endpoint("file:///tmp/registry").is_err());
        assert!(validate_registry_http_endpoint("https:///missing-host").is_err());
        assert!(validate_registry_http_endpoint("https://example.test/api?token=bad").is_err());
    }

    #[test]
    fn sends_registry_requests_and_decodes_chunked_responses() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
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
            assert!(request.starts_with("GET /registry/api/v1/ping HTTP/1.1\r\n"));
            assert!(
                request
                    .to_ascii_lowercase()
                    .contains("authorization: bearer test-token\r\n")
            );
            stream
                .write_all(
                    b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n4\r\npong\r\n0\r\n\r\n",
                )
                .unwrap();
        });

        let endpoint = format!("http://{address}/registry");
        let response = send_registry_http_request(RegistryHttpRequest {
            endpoint: &endpoint,
            path: "/api/v1/ping",
            method: RegistryHttpMethod::Get,
            accept: "text/plain",
            content_type: None,
            authorization: Some("Bearer test-token"),
            body: &[],
        })
        .unwrap();

        server.join().unwrap();
        assert_eq!(response.status, 200);
        assert_eq!(response.body, b"pong");
    }

    #[test]
    fn sends_registry_requests_over_verified_https() {
        let _ = rustls::crypto::ring::default_provider().install_default();
        let CertifiedKey { cert, signing_key } =
            generate_simple_self_signed(vec!["localhost".to_string()]).unwrap();
        let server_config = ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(
                vec![cert.der().clone()],
                PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(signing_key.serialize_der())),
            )
            .unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let connection = ServerConnection::new(Arc::new(server_config)).unwrap();
            let mut stream = StreamOwned::new(connection, stream);
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
            assert!(request.starts_with("GET /api/v1/secure HTTP/1.1\r\n"));
            assert!(
                request
                    .to_ascii_lowercase()
                    .contains("authorization: bearer tls-token\r\n")
            );
            stream
                .write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Length: 6\r\nConnection: close\r\n\r\nsecure",
                )
                .unwrap();
            stream.flush().unwrap();
        });

        let root = Certificate::from_der(cert.der().as_ref()).to_owned();
        let agent = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .tls_config(
                TlsConfig::builder()
                    .root_certs(RootCerts::new_with_certs(&[root]))
                    .build(),
            )
            .build()
            .new_agent();
        let endpoint = format!("https://localhost:{}", address.port());
        let response = send_registry_http_request_with_agent(
            RegistryHttpRequest {
                endpoint: &endpoint,
                path: "/api/v1/secure",
                method: RegistryHttpMethod::Get,
                accept: "text/plain",
                content_type: None,
                authorization: Some("Bearer tls-token"),
                body: &[],
            },
            &agent,
        )
        .unwrap();

        server.join().unwrap();
        assert_eq!(response.status, 200);
        assert_eq!(response.body, b"secure");
    }
}
