use super::{BuildError, Project, check_project};
use nomo_manifest::{
    DependencyAddSpec, parse_manifest_at_root, parse_manifest_document,
    remove_dependency_from_manifest, render_manifest_document, upsert_registry_dependency,
    validate_dependency_alias, validate_package_id, validate_version_like,
};
use nomo_resolver::{archive_checksum, build_package_archive, package_archive_filename};
use nomo_supply_chain::{
    ExternalSignerResponse, PROVENANCE_SCHEMA, ReleaseProvenance, ReleaseSubject,
    SignedReleaseEnvelope, envelope_from_signer_response, sha256_digest,
};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublishPackage {
    pub package: String,
    pub version: String,
    pub archive_path: PathBuf,
    pub checksum: String,
    pub manifest_checksum: String,
    pub provenance_path: PathBuf,
    pub provenance_digest: String,
    pub envelope_path: Option<PathBuf>,
    pub envelope: Option<SignedReleaseEnvelope>,
    pub size: usize,
}

pub fn add_registry_dependency(
    project: &Project,
    spec: DependencyAddSpec,
) -> Result<PathBuf, String> {
    validate_dependency_alias(&spec.alias)?;
    if spec.alias == "std" {
        return Err(
            "dependency alias `std` is reserved for the built-in standard library".to_string(),
        );
    }
    validate_package_id(&spec.package)?;
    validate_version_like(
        &format!("dependency `{}` version", spec.alias),
        &spec.version,
    )?;
    if spec.registry.as_deref().is_some_and(str::is_empty) {
        return Err("--registry requires a non-empty registry endpoint".to_string());
    }

    let manifest_path = project.root.join("nomo.toml");
    let text = fs::read_to_string(&manifest_path).map_err(|err| err.to_string())?;
    let mut document = parse_manifest_document(&text)?;
    upsert_registry_dependency(&mut document, &spec)?;
    let rendered = render_manifest_document(&document)?;
    fs::write(&manifest_path, rendered).map_err(|err| err.to_string())?;

    parse_manifest_at_root(&project.root)?;
    Ok(manifest_path)
}

pub fn remove_dependency(project: &Project, alias: &str) -> Result<PathBuf, String> {
    validate_dependency_alias(alias)?;
    if alias == "std" {
        return Err(
            "dependency alias `std` is reserved for the built-in standard library".to_string(),
        );
    }

    let manifest_path = project.root.join("nomo.toml");
    let text = fs::read_to_string(&manifest_path).map_err(|err| err.to_string())?;
    let mut document = parse_manifest_document(&text)?;
    remove_dependency_from_manifest(&mut document, alias)?;
    let rendered = render_manifest_document(&document)?;
    fs::write(&manifest_path, rendered).map_err(|err| err.to_string())?;

    parse_manifest_at_root(&project.root)?;
    Ok(manifest_path)
}

pub fn prepare_publish_package(
    project: &Project,
    output_dir: Option<&Path>,
) -> Result<PublishPackage, BuildError> {
    let manifest_path = project.root.join("nomo.toml");
    if !manifest_path.is_file() {
        return Err(BuildError::Message(format!(
            "package is missing {}",
            manifest_path.display()
        )));
    }
    let src = project.root.join("src");
    if !src.is_dir() {
        return Err(BuildError::Message(format!(
            "package is missing {}",
            src.display()
        )));
    }

    check_project(project).map_err(BuildError::Diagnostic)?;
    let manifest = parse_manifest_at_root(&project.root).map_err(BuildError::Message)?;
    if manifest.details.publish == Some(false) {
        return Err(BuildError::Message(format!(
            "package `{}/{}` is marked `publish = false` in nomo.toml",
            manifest.package.namespace, manifest.package.name
        )));
    }
    let archive =
        build_package_archive(&project.root, &manifest.package).map_err(BuildError::Message)?;
    let checksum = archive_checksum(&archive);
    let manifest_bytes = fs::read(&manifest_path).map_err(|err| {
        BuildError::Message(format!("failed to read {}: {err}", manifest_path.display()))
    })?;
    let manifest_checksum = sha256_digest(&manifest_bytes);
    let package = format!("{}/{}", manifest.package.namespace, manifest.package.name);
    let version = manifest.package.version;
    let output_dir = output_dir
        .map(Path::to_path_buf)
        .unwrap_or_else(|| project.root.join("build/package"));
    fs::create_dir_all(&output_dir).map_err(|err| BuildError::Message(err.to_string()))?;
    let archive_path = output_dir.join(package_archive_filename(&package, &version));
    fs::write(&archive_path, &archive).map_err(|err| {
        BuildError::Message(format!("failed to write {}: {err}", archive_path.display()))
    })?;
    let provenance = ReleaseProvenance {
        schema: PROVENANCE_SCHEMA,
        builder: "nomo publish".to_string(),
        builder_version: env!("CARGO_PKG_VERSION").to_string(),
        package: package.clone(),
        version: version.clone(),
        archive_checksum: checksum.clone(),
        manifest_checksum: manifest_checksum.clone(),
    };
    let provenance_text = provenance.render().map_err(BuildError::Message)?;
    let provenance_digest = sha256_digest(provenance_text.as_bytes());
    let provenance_path = PathBuf::from(format!("{}.provenance.json", archive_path.display()));
    fs::write(&provenance_path, provenance_text).map_err(|err| {
        BuildError::Message(format!(
            "failed to write {}: {err}",
            provenance_path.display()
        ))
    })?;
    Ok(PublishPackage {
        package,
        version,
        archive_path,
        checksum,
        manifest_checksum,
        provenance_path,
        provenance_digest,
        envelope_path: None,
        envelope: None,
        size: archive.len(),
    })
}

pub fn sign_publish_package(
    mut package: PublishPackage,
    signer: &str,
    envelope_path: Option<&Path>,
) -> Result<PublishPackage, BuildError> {
    if signer.trim().is_empty() {
        return Err(BuildError::Message(
            "external signer command cannot be empty".to_string(),
        ));
    }
    let subject = ReleaseSubject::new(
        package.package.clone(),
        package.version.clone(),
        package.checksum.clone(),
        package.manifest_checksum.clone(),
        Some(package.provenance_digest.clone()),
    )
    .map_err(BuildError::Message)?;
    let mut child = Command::new(signer)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| {
            BuildError::Message(format!("failed to start external signer `{signer}`: {err}"))
        })?;
    child
        .stdin
        .take()
        .expect("piped signer stdin is available")
        .write_all(&subject.canonical_bytes().map_err(BuildError::Message)?)
        .map_err(|err| {
            BuildError::Message(format!("failed to send payload to external signer: {err}"))
        })?;
    let output = child
        .wait_with_output()
        .map_err(|err| BuildError::Message(format!("failed to wait for external signer: {err}")))?;
    if !output.status.success() {
        return Err(BuildError::Message(format!(
            "external signer failed with status {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    let response: ExternalSignerResponse =
        serde_json::from_slice(&output.stdout).map_err(|err| {
            BuildError::Message(format!("external signer returned invalid JSON: {err}"))
        })?;
    let envelope = envelope_from_signer_response(subject, response).map_err(BuildError::Message)?;
    let mut rendered = serde_json::to_string_pretty(&envelope)
        .map_err(|err| BuildError::Message(err.to_string()))?;
    rendered.push('\n');
    let envelope_path = envelope_path.map(Path::to_path_buf).unwrap_or_else(|| {
        PathBuf::from(format!("{}.envelope.json", package.archive_path.display()))
    });
    if let Some(parent) = envelope_path.parent() {
        fs::create_dir_all(parent).map_err(|err| BuildError::Message(err.to_string()))?;
    }
    fs::write(&envelope_path, rendered).map_err(|err| {
        BuildError::Message(format!(
            "failed to write {}: {err}",
            envelope_path.display()
        ))
    })?;
    package.envelope_path = Some(envelope_path);
    package.envelope = Some(envelope);
    Ok(package)
}
