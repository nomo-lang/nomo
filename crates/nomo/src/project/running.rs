use super::{BuildError, Project, build_project_with_diagnostics, configure_c_compile_command};
use crate::compiler::compile_script_source_to_c;
use nomo_manifest::FfiLinkMetadata;
use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::process::Command;

pub fn run_project(project: &Project) -> Result<i32, String> {
    run_project_with_args(project, &[])
}

pub fn run_project_with_args(project: &Project, args: &[String]) -> Result<i32, String> {
    run_project_with_args_and_diagnostics(project, args).map_err(|err| err.human())
}

pub fn run_project_with_args_and_diagnostics(
    project: &Project,
    args: &[String],
) -> Result<i32, BuildError> {
    let bin = build_project_with_diagnostics(project, false)?;
    let bin = if bin.is_absolute() {
        bin
    } else {
        std::env::current_dir()
            .map_err(|err| BuildError::Message(err.to_string()))?
            .join(bin)
    };
    let status = Command::new(&bin)
        .current_dir(&project.root)
        .args(args)
        .status()
        .map_err(|err| BuildError::Message(format!("failed to run {}: {err}", bin.display())))?;
    Ok(status.code().unwrap_or(1))
}

pub fn run_standalone_script_with_args_and_diagnostics(
    source: &Path,
    args: &[String],
) -> Result<i32, BuildError> {
    let c = compile_script_source_to_c(source).map_err(BuildError::Diagnostic)?;
    let stem = source
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("script");
    let mut hasher = DefaultHasher::new();
    source.hash(&mut hasher);
    c.hash(&mut hasher);
    let build_dir = std::env::temp_dir().join(format!("nomo-script-{:016x}", hasher.finish()));
    let c_dir = build_dir.join("c");
    let bin_dir = build_dir.join("bin");
    fs::create_dir_all(&c_dir).map_err(|err| BuildError::Message(err.to_string()))?;
    fs::create_dir_all(&bin_dir).map_err(|err| BuildError::Message(err.to_string()))?;

    let c_path = c_dir.join("main.c");
    fs::write(&c_path, c).map_err(|err| BuildError::Message(err.to_string()))?;
    let bin_path = bin_dir.join(stem);
    let mut command = Command::new("cc");
    configure_c_compile_command(
        &mut command,
        &c_path,
        &bin_path,
        &FfiLinkMetadata::default(),
    );
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

    let current_dir = source.parent().unwrap_or_else(|| Path::new("."));
    let status = Command::new(&bin_path)
        .current_dir(current_dir)
        .args(args)
        .status()
        .map_err(|err| {
            BuildError::Message(format!("failed to run {}: {err}", bin_path.display()))
        })?;
    Ok(status.code().unwrap_or(1))
}
