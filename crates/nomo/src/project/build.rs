use super::{
    BuildError, DependencyResolutionOptions, Project,
    project_ffi_link_metadata_for_target_with_options,
    project_module_context_for_target_with_options,
};
use crate::compiler::compile_source_to_c_with_project_modules_for_target;
use crate::incremental::{PersistentQueryCache, project_query_key};
use nomo_manifest::FfiLinkMetadata;
use nomo_target::TargetTriple;
use sha2::{Digest, Sha256};
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
    let target = TargetTriple::host().map_err(BuildError::Message)?;
    build_project_impl(project, emit_c_only, options, &target, false)
}

pub fn build_project_for_target_with_options(
    project: &Project,
    emit_c_only: bool,
    options: DependencyResolutionOptions,
    target: &TargetTriple,
) -> Result<PathBuf, BuildError> {
    build_project_impl(project, emit_c_only, options, target, true)
}

fn build_project_impl(
    project: &Project,
    emit_c_only: bool,
    options: DependencyResolutionOptions,
    target: &TargetTriple,
    target_scoped_artifacts: bool,
) -> Result<PathBuf, BuildError> {
    let context = project_module_context_for_target_with_options(project, options, target)
        .map_err(BuildError::Message)?;
    let ffi_link_metadata =
        project_ffi_link_metadata_for_target_with_options(project, options, target)
            .map_err(BuildError::Message)?;
    let cache_root = project
        .workspace_root
        .as_deref()
        .unwrap_or(project.root.as_path());
    let cache = PersistentQueryCache::at_root(cache_root);
    let cache_key = project_query_key(
        project,
        &context.external_modules,
        &[],
        target,
        "codegen-c",
        format!(
            "{}:{}:sqlite-{}:{}:{}:{:x}:{:x}",
            project.name,
            project.main.display(),
            nomo_codegen_c::BUNDLED_SQLITE_VERSION,
            nomo_codegen_c::BUNDLED_SQLITE3_C_SHA256,
            nomo_codegen_c::BUNDLED_SQLITE3_H_SHA256,
            Sha256::digest(nomo_codegen_c::BUNDLED_SQLITE_COMPILE_OPTIONS.join("\n")),
            Sha256::digest(nomo_codegen_c::BUNDLED_SQLITE_RUNTIME_SOURCE.as_bytes())
        ),
    );
    let c = match cache.get::<String>(&cache_key) {
        Some(cached) => cached,
        None => {
            let generated = compile_source_to_c_with_project_modules_for_target(
                &project.main,
                Some(&context.local_source_root),
                &context.external_import_roots,
                &context.external_modules,
                target,
            )
            .map_err(BuildError::Diagnostic)?;
            let _ = cache.insert(&cache_key, &generated);
            generated
        }
    };
    let target_dir = if target_scoped_artifacts {
        project.root.join("build").join(target.to_string())
    } else {
        project.root.join("build")
    };
    let c_dir = target_dir.join("c");
    let bin_dir = target_dir.join("bin");
    fs::create_dir_all(&c_dir).map_err(|err| BuildError::Message(err.to_string()))?;
    fs::create_dir_all(&bin_dir).map_err(|err| BuildError::Message(err.to_string()))?;

    let c_path = c_dir.join("main.c");
    let uses_native_tasks = generated_c_uses_native_tasks(&c);
    let uses_bundled_sqlite = generated_c_uses_bundled_sqlite(&c);
    fs::write(&c_path, c).map_err(|err| BuildError::Message(err.to_string()))?;
    materialize_bundled_sqlite(&c_dir, uses_bundled_sqlite)
        .map_err(|err| BuildError::Message(err.to_string()))?;
    if emit_c_only {
        return Ok(c_path);
    }

    let host = TargetTriple::host().map_err(BuildError::Message)?;
    let toolchain = target
        .c_toolchain_from(&host)
        .map_err(BuildError::Message)?;
    let bin_path = bin_dir.join(&project.name);
    let mut command = Command::new(&toolchain.program);
    command.args(&toolchain.args);
    configure_c_compile_command(
        &mut command,
        &c_path,
        &bin_path,
        &ffi_link_metadata,
        target,
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
    target: &TargetTriple,
    uses_native_tasks: bool,
    uses_bundled_sqlite: bool,
) {
    command.arg("-std=c99");
    if uses_bundled_sqlite {
        for option in nomo_codegen_c::BUNDLED_SQLITE_COMPILE_OPTIONS {
            command.arg(format!("-D{option}"));
        }
    }
    command.arg(c_path);
    if uses_bundled_sqlite {
        command.arg(c_path.with_file_name("sqlite3.c"));
    }
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
    if target.operating_system().as_str() == "linux" {
        command.arg("-ldl");
    }
    if target.operating_system().as_str() == "windows" {
        command.arg("-lws2_32");
    }
    if (uses_native_tasks || uses_bundled_sqlite) && target.operating_system().as_str() != "windows"
    {
        command.arg("-pthread");
    }
    if target.operating_system().as_str() != "windows" {
        command.arg("-lm");
    }
    command.arg("-o").arg(bin_path);
}

pub(super) fn generated_c_uses_native_tasks(source: &str) -> bool {
    source.contains("#define NOMO_TASK_MAX_LIVE")
}

pub(super) fn generated_c_uses_bundled_sqlite(source: &str) -> bool {
    source.contains("#define NOMO_SQLITE_MAX_DATABASES")
}

pub(super) fn materialize_bundled_sqlite(
    c_dir: &Path,
    enabled: bool,
) -> Result<(), std::io::Error> {
    if !enabled {
        return Ok(());
    }
    verify_bundled_sqlite_source(
        nomo_codegen_c::BUNDLED_SQLITE3_C,
        nomo_codegen_c::BUNDLED_SQLITE3_C_SHA256,
        "sqlite3.c",
    )?;
    verify_bundled_sqlite_source(
        nomo_codegen_c::BUNDLED_SQLITE3_H,
        nomo_codegen_c::BUNDLED_SQLITE3_H_SHA256,
        "sqlite3.h",
    )?;
    let sqlite_c_path = c_dir.join("sqlite3.c");
    let sqlite_h_path = c_dir.join("sqlite3.h");
    fs::write(&sqlite_c_path, nomo_codegen_c::BUNDLED_SQLITE3_C)?;
    fs::write(&sqlite_h_path, nomo_codegen_c::BUNDLED_SQLITE3_H)?;
    fs::write(
        c_dir.join("sqlite3-SOURCE.md"),
        nomo_codegen_c::BUNDLED_SQLITE_SOURCE,
    )?;
    verify_bundled_sqlite_source(
        &fs::read(sqlite_c_path)?,
        nomo_codegen_c::BUNDLED_SQLITE3_C_SHA256,
        "materialized sqlite3.c",
    )?;
    verify_bundled_sqlite_source(
        &fs::read(sqlite_h_path)?,
        nomo_codegen_c::BUNDLED_SQLITE3_H_SHA256,
        "materialized sqlite3.h",
    )?;
    Ok(())
}

fn verify_bundled_sqlite_source(
    source: &[u8],
    expected: &str,
    name: &str,
) -> Result<(), std::io::Error> {
    let actual = format!("{:x}", Sha256::digest(source));
    if actual == expected {
        return Ok(());
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        format!("bundled {name} digest mismatch"),
    ))
}
