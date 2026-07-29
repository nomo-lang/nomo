use super::{
    BuildError, BuildProfile, DependencyResolutionOptions, Project,
    build_project_with_profile_options, build_standalone_release_c,
    compile_standalone_script_with_profile_cache, configure_c_compile_command,
};
use nomo_manifest::FfiLinkMetadata;
use nomo_target::TargetTriple;
use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
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
    run_project_with_args_and_profile_and_diagnostics(project, args, BuildProfile::Debug)
}

pub fn run_project_with_args_and_profile_and_diagnostics(
    project: &Project,
    args: &[String],
    profile: BuildProfile,
) -> Result<i32, BuildError> {
    let bin = build_project_with_profile_options(
        project,
        false,
        DependencyResolutionOptions::default(),
        profile,
    )?;
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
    run_standalone_script_with_args_and_profile_and_diagnostics(source, args, BuildProfile::Debug)
}

pub fn run_standalone_script_with_args_and_profile_and_diagnostics(
    source: &Path,
    args: &[String],
    profile: BuildProfile,
) -> Result<i32, BuildError> {
    let source = lexical_absolute_path(source)?;
    let target = TargetTriple::host().map_err(BuildError::Message)?;
    super::clear_standalone_build_metadata(&source, &target)?;
    let generated = compile_standalone_script_with_profile_cache(&source, &target, profile)?;
    if profile == BuildProfile::Release {
        let bin_path = build_standalone_release_c(&source, &generated, None, &target)?;
        let current_dir = source.parent().unwrap_or_else(|| Path::new("."));
        let status = Command::new(&bin_path)
            .current_dir(current_dir)
            .args(args)
            .status()
            .map_err(|error| {
                BuildError::Message(format!("failed to run {}: {error}", bin_path.display()))
            })?;
        return Ok(status.code().unwrap_or(1));
    }
    let stem = source
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("script");
    let mut hasher = DefaultHasher::new();
    source.hash(&mut hasher);
    generated.generated_source().hash(&mut hasher);
    profile.as_str().hash(&mut hasher);
    let build_dir = std::env::temp_dir().join(format!("nomo-script-{:016x}", hasher.finish()));
    let c_dir = build_dir.join("c");
    let bin_dir = build_dir.join("bin");
    fs::create_dir_all(&c_dir).map_err(|err| BuildError::Message(err.to_string()))?;
    fs::create_dir_all(&bin_dir).map_err(|err| BuildError::Message(err.to_string()))?;

    let c_path = c_dir.join("main.c");
    let uses_native_tasks =
        super::build::generated_c_uses_native_tasks(generated.generated_source());
    let uses_bundled_sqlite =
        super::build::generated_c_uses_bundled_sqlite(generated.generated_source());
    fs::write(&c_path, generated.generated_source())
        .map_err(|err| BuildError::Message(err.to_string()))?;
    super::build::materialize_bundled_sqlite(&c_dir, uses_bundled_sqlite)
        .map_err(|err| BuildError::Message(err.to_string()))?;
    let bin_path = bin_dir.join(stem);
    let toolchain = target
        .c_toolchain_from(&target)
        .map_err(BuildError::Message)?;
    let mut command = Command::new(&toolchain.program);
    command.args(&toolchain.args);
    configure_c_compile_command(
        &mut command,
        &c_path,
        &bin_path,
        &FfiLinkMetadata::default(),
        &target,
        uses_native_tasks,
        uses_bundled_sqlite,
    );
    let output = command.output().map_err(|err| {
        BuildError::Message(format!(
            "failed to run C compiler `{}` for target `{target}`: {err}",
            toolchain.program
        ))
    })?;
    if !output.status.success() {
        return Err(BuildError::Message(format!(
            "C compiler failed for target `{target}`:\n{}{}",
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

fn lexical_absolute_path(path: &Path) -> Result<PathBuf, BuildError> {
    if path.is_absolute() {
        return Ok(path.to_path_buf());
    }
    Ok(std::env::current_dir()
        .map_err(|error| BuildError::Message(error.to_string()))?
        .join(path))
}
