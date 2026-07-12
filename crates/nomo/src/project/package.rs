use super::{BuildError, Project, check_project};
use nomo_manifest::{
    DependencyAddSpec, parse_manifest_at_root, parse_manifest_document,
    remove_dependency_from_manifest, render_manifest_document, upsert_registry_dependency,
    validate_dependency_alias, validate_package_id, validate_version_like,
};
use nomo_resolver::{archive_checksum, build_package_archive, package_archive_filename};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublishPackage {
    pub package: String,
    pub version: String,
    pub archive_path: PathBuf,
    pub checksum: String,
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
    let archive =
        build_package_archive(&project.root, &manifest.package).map_err(BuildError::Message)?;
    let checksum = archive_checksum(&archive);
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
    Ok(PublishPackage {
        package,
        version,
        archive_path,
        checksum,
        size: archive.len(),
    })
}
