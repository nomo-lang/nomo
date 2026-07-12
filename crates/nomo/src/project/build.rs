use super::{
    BuildError, DependencyResolutionOptions, Project, project_ffi_link_metadata_with_options,
    project_module_context_with_options,
};
use crate::compiler::compile_source_to_c_with_project_modules;
use nomo_manifest::FfiLinkMetadata;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

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
    let context =
        project_module_context_with_options(project, options).map_err(BuildError::Message)?;
    let ffi_link_metadata =
        project_ffi_link_metadata_with_options(project, options).map_err(BuildError::Message)?;
    let c = compile_source_to_c_with_project_modules(
        &project.main,
        Some(&context.local_source_root),
        &context.external_import_roots,
        &context.external_modules,
    )
    .map_err(BuildError::Diagnostic)?;
    let c_dir = project.root.join("build/c");
    let bin_dir = project.root.join("build/bin");
    fs::create_dir_all(&c_dir).map_err(|err| BuildError::Message(err.to_string()))?;
    fs::create_dir_all(&bin_dir).map_err(|err| BuildError::Message(err.to_string()))?;

    let c_path = c_dir.join("main.c");
    fs::write(&c_path, c).map_err(|err| BuildError::Message(err.to_string()))?;
    if emit_c_only {
        return Ok(c_path);
    }

    let bin_path = bin_dir.join(&project.name);
    let mut command = Command::new("cc");
    configure_c_compile_command(&mut command, &c_path, &bin_path, &ffi_link_metadata);
    let output = command
        .output()
        .map_err(|err| BuildError::Message(format!("failed to run cc: {err}")))?;
    if !output.status.success() {
        return Err(BuildError::Message(format!(
            "cc failed:\n{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    Ok(bin_path)
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
) {
    command.arg("-std=c99").arg(c_path);
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
    command.arg("-lm").arg("-o").arg(bin_path);
}
