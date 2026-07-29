use super::build_metadata::{
    BuildMetadata, FileRecord, PASS_PIPELINE_VERSION, ReleaseBackendProvenance,
    codegen_cache_configuration, inspect_compiler, release_c_flags, release_driver_config_flags,
    remove_stale_file, resolve_executable, run_recorded_command, write_build_metadata,
    write_release_provenance,
};
use super::{
    BuildError, BuildProfile, DependencyResolutionOptions, Project,
    project_ffi_link_metadata_for_target_with_options,
    project_module_context_for_target_with_options,
};
use crate::compiler::compile_source_to_c_with_module_identity_for_target;
use crate::incremental::{ContentFingerprint, PersistentQueryCache, QueryKey, project_query_key};
use nomo_manifest::FfiLinkMetadata;
use nomo_target::{CToolchain, TargetTriple};
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
    build_project_with_profile_options(project, emit_c_only, options, BuildProfile::Debug)
}

pub fn build_project_with_profile_options(
    project: &Project,
    emit_c_only: bool,
    options: DependencyResolutionOptions,
    profile: BuildProfile,
) -> Result<PathBuf, BuildError> {
    let target = TargetTriple::host().map_err(BuildError::Message)?;
    build_project_impl(project, emit_c_only, options, &target, false, profile)
}

pub fn build_project_for_target_with_options(
    project: &Project,
    emit_c_only: bool,
    options: DependencyResolutionOptions,
    target: &TargetTriple,
) -> Result<PathBuf, BuildError> {
    build_project_for_target_with_profile_options(
        project,
        emit_c_only,
        options,
        target,
        BuildProfile::Debug,
    )
}

pub fn build_project_for_target_with_profile_options(
    project: &Project,
    emit_c_only: bool,
    options: DependencyResolutionOptions,
    target: &TargetTriple,
    profile: BuildProfile,
) -> Result<PathBuf, BuildError> {
    build_project_impl(project, emit_c_only, options, target, true, profile)
}

pub fn build_standalone_release_c(
    source: &Path,
    generated_source: &str,
    output: Option<&Path>,
    target: &TargetTriple,
) -> Result<PathBuf, BuildError> {
    let host = TargetTriple::host().map_err(BuildError::Message)?;
    let source_parent = source.parent().unwrap_or_else(|| Path::new("."));
    let target_dir = if target == &host {
        source_parent.join("build")
    } else {
        source_parent.join("build").join(target.to_string())
    };
    let c_dir = target_dir.join("c");
    let object_dir = target_dir.join("obj");
    let bin_dir = target_dir.join("bin");
    fs::create_dir_all(&c_dir).map_err(|error| BuildError::Message(error.to_string()))?;
    fs::create_dir_all(&object_dir).map_err(|error| BuildError::Message(error.to_string()))?;
    fs::create_dir_all(&bin_dir).map_err(|error| BuildError::Message(error.to_string()))?;
    let c_path = c_dir.join("main.c");
    let stem = source
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("program");
    let default_name = if target.operating_system().as_str() == "windows" {
        format!("{stem}.exe")
    } else {
        stem.to_string()
    };
    let bin_path = output
        .map(Path::to_path_buf)
        .unwrap_or_else(|| bin_dir.join(default_name));
    if let Some(parent) = bin_path.parent() {
        fs::create_dir_all(parent).map_err(|error| BuildError::Message(error.to_string()))?;
    }
    let provenance_path = target_dir.join("release-provenance.json");
    let metadata_path = target_dir.join("nomo-build-metadata.json");
    remove_stale_file(&provenance_path)?;
    remove_stale_file(&metadata_path)?;
    fs::write(&c_path, generated_source).map_err(|error| BuildError::Message(error.to_string()))?;
    let generated_c = FileRecord::from_path(&c_path)?;
    let toolchain = c_toolchain_for_profile(target, &host, BuildProfile::Release)?;
    let compiler_path = resolve_executable(&toolchain.program, BuildProfile::Release)?;
    let compiler = inspect_compiler(&compiler_path, &toolchain.args, true)?;
    let ffi = FfiLinkMetadata::default();
    let (objects, compile_commands) = compile_release_translation_units(
        &compiler_path,
        &toolchain,
        &c_path,
        &c_dir,
        &object_dir,
        &ffi,
        false,
    )?;
    remove_stale_file(&bin_path)?;
    let link_argv = release_link_argv(
        &compiler_path,
        &toolchain,
        &objects,
        &bin_path,
        &ffi,
        target,
        generated_c_uses_native_tasks(generated_source),
        false,
        generated_c_uses_math(generated_source),
        generated_c_uses_dynamic_loader(generated_source),
        generated_c_uses_winsock(generated_source),
    )?;
    let link_command = run_recorded_command(link_argv, BuildProfile::Release)?;
    let object_records = objects
        .iter()
        .map(|path| FileRecord::from_path(path))
        .collect::<Result<Vec<_>, _>>()?;
    let binary = FileRecord::from_path(&bin_path)?;
    let provenance = ReleaseBackendProvenance::new(
        compiler.clone(),
        object_records,
        compile_commands.clone(),
        link_command.clone(),
        generated_c.clone(),
        binary.clone(),
    );
    let provenance_record = write_release_provenance(&provenance_path, &provenance)?;
    let metadata = BuildMetadata::new(
        BuildProfile::Release,
        target.to_string(),
        standalone_codegen_query_key(source, target, BuildProfile::Release, "script")?,
        Some(compiler),
        compile_commands,
        Some(link_command),
        None,
        generated_c,
        Some(binary),
        Some(provenance_record),
    )?;
    write_build_metadata(&metadata_path, &metadata)?;
    Ok(bin_path)
}

pub fn record_standalone_c_build_metadata(
    source: &Path,
    generated_source: &str,
    output: Option<&Path>,
    target: &TargetTriple,
    profile: BuildProfile,
) -> Result<PathBuf, BuildError> {
    let host = TargetTriple::host().map_err(BuildError::Message)?;
    let source_parent = source.parent().unwrap_or_else(|| Path::new("."));
    let target_dir = if target == &host {
        source_parent.join("build")
    } else {
        source_parent.join("build").join(target.to_string())
    };
    let c_path = match output {
        Some(path) => path.to_path_buf(),
        None => target_dir.join("c/main.c"),
    };
    if output.is_none() {
        let parent = c_path.parent().ok_or_else(|| {
            BuildError::Message(format!(
                "generated-C output has no parent: {}",
                c_path.display()
            ))
        })?;
        fs::create_dir_all(parent).map_err(|error| BuildError::Message(error.to_string()))?;
        fs::write(&c_path, generated_source)
            .map_err(|error| BuildError::Message(error.to_string()))?;
    }
    let provenance_path = target_dir.join("release-provenance.json");
    let metadata_path = target_dir.join("nomo-build-metadata.json");
    remove_stale_file(&provenance_path)?;
    remove_stale_file(&metadata_path)?;
    let metadata = BuildMetadata::new(
        profile,
        target.to_string(),
        standalone_codegen_query_key(source, target, profile, "nomoc")?,
        None,
        Vec::new(),
        None,
        None,
        FileRecord::from_path(&c_path)?,
        None,
        None,
    )?;
    write_build_metadata(&metadata_path, &metadata)?;
    Ok(metadata_path)
}

pub fn compile_standalone_source_with_profile_cache(
    source: &Path,
    target: &TargetTriple,
    profile: BuildProfile,
) -> Result<String, BuildError> {
    let cache_key = standalone_codegen_query_key(source, target, profile, "nomoc")?;
    let cache_root = source.parent().unwrap_or_else(|| Path::new("."));
    let cache = PersistentQueryCache::at_root(cache_root);
    if let Some(cached) = cache.get::<String>(&cache_key) {
        return Ok(cached);
    }
    let generated = crate::compiler::compile_source_to_c_for_target(source, target)
        .map_err(BuildError::Diagnostic)?;
    let _ = cache.insert(&cache_key, &generated);
    Ok(generated)
}

pub fn compile_standalone_script_with_profile_cache(
    source: &Path,
    target: &TargetTriple,
    profile: BuildProfile,
) -> Result<String, BuildError> {
    let cache_key = standalone_codegen_query_key(source, target, profile, "script")?;
    let cache_root = source.parent().unwrap_or_else(|| Path::new("."));
    let cache = PersistentQueryCache::at_root(cache_root);
    if let Some(cached) = cache.get::<String>(&cache_key) {
        return Ok(cached);
    }
    let generated = crate::compiler::compile_script_source_to_c_for_target(source, target)
        .map_err(BuildError::Diagnostic)?;
    let _ = cache.insert(&cache_key, &generated);
    Ok(generated)
}

pub fn clear_project_build_metadata(
    project: &Project,
    target: &TargetTriple,
    target_scoped_artifacts: bool,
) -> Result<(), BuildError> {
    let target_dir = if target_scoped_artifacts {
        project.root.join("build").join(target.to_string())
    } else {
        project.root.join("build")
    };
    clear_build_metadata_at(&target_dir)
}

pub fn clear_standalone_build_metadata(
    source: &Path,
    target: &TargetTriple,
) -> Result<(), BuildError> {
    let host = TargetTriple::host().map_err(BuildError::Message)?;
    let source_parent = source.parent().unwrap_or_else(|| Path::new("."));
    let target_dir = if target == &host {
        source_parent.join("build")
    } else {
        source_parent.join("build").join(target.to_string())
    };
    clear_build_metadata_at(&target_dir)
}

fn clear_build_metadata_at(target_dir: &Path) -> Result<(), BuildError> {
    remove_stale_file(&target_dir.join("release-provenance.json"))?;
    remove_stale_file(&target_dir.join("nomo-build-metadata.json"))
}

fn build_project_impl(
    project: &Project,
    emit_c_only: bool,
    options: DependencyResolutionOptions,
    target: &TargetTriple,
    target_scoped_artifacts: bool,
    profile: BuildProfile,
) -> Result<PathBuf, BuildError> {
    let target_dir = if target_scoped_artifacts {
        project.root.join("build").join(target.to_string())
    } else {
        project.root.join("build")
    };
    let provenance_path = target_dir.join("release-provenance.json");
    let metadata_path = target_dir.join("nomo-build-metadata.json");
    remove_stale_file(&provenance_path)?;
    remove_stale_file(&metadata_path)?;
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
            "{}:{}:{}",
            project.name,
            project.main.display(),
            codegen_cache_configuration(profile, PASS_PIPELINE_VERSION),
        ),
    );
    let c = match cache.get::<String>(&cache_key) {
        Some(cached) => cached,
        None => {
            let generated = compile_source_to_c_with_module_identity_for_target(
                &project.main,
                &context.local_source_root,
                &context.local_identity,
                &context.external_import_roots,
                &context.external_modules,
                target,
            )
            .map_err(BuildError::Diagnostic)?;
            let _ = cache.insert(&cache_key, &generated);
            generated
        }
    };
    let c_dir = target_dir.join("c");
    let object_dir = target_dir.join("obj");
    let bin_dir = target_dir.join("bin");
    fs::create_dir_all(&c_dir).map_err(|err| BuildError::Message(err.to_string()))?;
    fs::create_dir_all(&object_dir).map_err(|err| BuildError::Message(err.to_string()))?;
    fs::create_dir_all(&bin_dir).map_err(|err| BuildError::Message(err.to_string()))?;

    let c_path = c_dir.join("main.c");
    let uses_native_tasks = generated_c_uses_native_tasks(&c);
    let uses_bundled_sqlite = generated_c_uses_bundled_sqlite(&c);
    let uses_math = generated_c_uses_math(&c);
    let uses_dynamic_loader = generated_c_uses_dynamic_loader(&c);
    let uses_winsock = generated_c_uses_winsock(&c);
    fs::write(&c_path, c).map_err(|err| BuildError::Message(err.to_string()))?;
    materialize_bundled_sqlite(&c_dir, uses_bundled_sqlite)
        .map_err(|err| BuildError::Message(err.to_string()))?;
    let generated_c = FileRecord::from_path(&c_path)?;
    if emit_c_only {
        let metadata = BuildMetadata::new(
            profile,
            target.to_string(),
            cache_key.clone(),
            None,
            Vec::new(),
            None,
            None,
            generated_c,
            None,
            None,
        )?;
        write_build_metadata(&metadata_path, &metadata)?;
        return Ok(c_path);
    }

    let host = TargetTriple::host().map_err(BuildError::Message)?;
    let toolchain = c_toolchain_for_profile(target, &host, profile)?;
    let bin_name = if target.operating_system().as_str() == "windows" {
        format!("{}.exe", project.name)
    } else {
        project.name.clone()
    };
    let bin_path = bin_dir.join(bin_name);
    let compiler_path = resolve_executable(&toolchain.program, profile)?;
    let compiler = inspect_compiler(
        &compiler_path,
        &toolchain.args,
        profile == BuildProfile::Release,
    )?;
    if profile == BuildProfile::Release {
        let (objects, compile_commands) = compile_release_translation_units(
            &compiler_path,
            &toolchain,
            &c_path,
            &c_dir,
            &object_dir,
            &ffi_link_metadata,
            uses_bundled_sqlite,
        )?;
        remove_stale_file(&bin_path)?;
        let link_argv = release_link_argv(
            &compiler_path,
            &toolchain,
            &objects,
            &bin_path,
            &ffi_link_metadata,
            target,
            uses_native_tasks,
            uses_bundled_sqlite,
            uses_math,
            uses_dynamic_loader,
            uses_winsock,
        )?;
        let link_command = run_recorded_command(link_argv, BuildProfile::Release)?;
        let object_records = objects
            .iter()
            .map(|path| FileRecord::from_path(path))
            .collect::<Result<Vec<_>, _>>()?;
        let binary = FileRecord::from_path(&bin_path)?;
        let provenance = ReleaseBackendProvenance::new(
            compiler.clone(),
            object_records,
            compile_commands.clone(),
            link_command.clone(),
            generated_c.clone(),
            binary.clone(),
        );
        let provenance_record = write_release_provenance(&provenance_path, &provenance)?;
        let metadata = BuildMetadata::new(
            profile,
            target.to_string(),
            cache_key.clone(),
            Some(compiler),
            compile_commands,
            Some(link_command),
            None,
            generated_c,
            Some(binary),
            Some(provenance_record),
        )?;
        write_build_metadata(&metadata_path, &metadata)?;
    } else {
        let mut command = Command::new(&compiler_path);
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
        let combined_command = run_recorded_command(command_argv(&command), BuildProfile::Debug)?;
        let binary = FileRecord::from_path(&bin_path)?;
        let metadata = BuildMetadata::new(
            profile,
            target.to_string(),
            cache_key,
            Some(compiler),
            Vec::new(),
            None,
            Some(combined_command),
            generated_c,
            Some(binary),
            None,
        )?;
        write_build_metadata(&metadata_path, &metadata)?;
    }
    Ok(bin_path)
}

fn standalone_codegen_query_key(
    source: &Path,
    target: &TargetTriple,
    profile: BuildProfile,
    mode: &str,
) -> Result<QueryKey, BuildError> {
    let source_path = absolute_build_path(source)?;
    let source_bytes = fs::read(&source_path).map_err(|error| {
        BuildError::Message(format!(
            "failed to fingerprint standalone source {}: {error}",
            source_path.display()
        ))
    })?;
    let configuration = codegen_cache_configuration(profile, PASS_PIPELINE_VERSION);
    Ok(QueryKey::new(
        target.to_string(),
        "codegen-c",
        format!(
            "standalone-{mode}:{}:{configuration}",
            source_path.display()
        ),
        ContentFingerprint::of_bytes(&source_bytes),
    ))
}

fn c_toolchain_for_profile(
    target: &TargetTriple,
    host: &TargetTriple,
    profile: BuildProfile,
) -> Result<CToolchain, BuildError> {
    let mut toolchain = target.c_toolchain_from(host).map_err(BuildError::Message)?;
    if profile == BuildProfile::Release
        && (target == host
            || (target.operating_system().as_str() == "darwin"
                && host.operating_system().as_str() == "darwin"))
    {
        toolchain.program = "clang".to_string();
    }
    Ok(toolchain)
}

fn compile_release_translation_units(
    compiler_path: &Path,
    toolchain: &CToolchain,
    main_c_path: &Path,
    c_dir: &Path,
    object_dir: &Path,
    ffi_link_metadata: &FfiLinkMetadata,
    uses_bundled_sqlite: bool,
) -> Result<(Vec<PathBuf>, Vec<super::build_metadata::CommandRecord>), BuildError> {
    let mut sources = vec![main_c_path.to_path_buf()];
    if uses_bundled_sqlite {
        sources.push(c_dir.join("sqlite3.c"));
    }
    sources.extend(ffi_link_metadata.sources.iter().cloned());
    let mut objects = Vec::with_capacity(sources.len());
    let mut commands = Vec::with_capacity(sources.len());
    for (index, source) in sources.iter().enumerate() {
        let stem = source
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("translation-unit");
        let object = object_dir.join(format!("{index}-{stem}.o"));
        remove_stale_file(&object)?;
        let mut argv = vec![compiler_path.to_string_lossy().into_owned()];
        argv.extend(toolchain.args.iter().cloned());
        argv.extend(
            release_driver_config_flags()
                .iter()
                .map(|flag| flag.to_string()),
        );
        argv.extend(release_c_flags().iter().map(|flag| flag.to_string()));
        if uses_bundled_sqlite {
            argv.extend(
                nomo_codegen_c::BUNDLED_SQLITE_COMPILE_OPTIONS
                    .iter()
                    .map(|option| format!("-D{option}")),
            );
        }
        argv.extend([
            "-c".to_string(),
            absolute_build_path(source)?.to_string_lossy().into_owned(),
            "-o".to_string(),
            absolute_build_path(&object)?.to_string_lossy().into_owned(),
        ]);
        commands.push(run_recorded_command(argv, BuildProfile::Release)?);
        objects.push(object);
    }
    Ok((objects, commands))
}

fn release_link_argv(
    compiler_path: &Path,
    toolchain: &CToolchain,
    objects: &[PathBuf],
    bin_path: &Path,
    ffi_link_metadata: &FfiLinkMetadata,
    target: &TargetTriple,
    uses_native_tasks: bool,
    uses_bundled_sqlite: bool,
    uses_math: bool,
    uses_dynamic_loader: bool,
    uses_winsock: bool,
) -> Result<Vec<String>, BuildError> {
    let mut argv = vec![compiler_path.to_string_lossy().into_owned()];
    argv.extend(toolchain.args.iter().cloned());
    argv.extend(
        release_driver_config_flags()
            .iter()
            .map(|flag| flag.to_string()),
    );
    for path in objects {
        argv.push(absolute_build_path(path)?.to_string_lossy().into_owned());
    }
    argv.extend([
        "-o".to_string(),
        absolute_build_path(bin_path)?
            .to_string_lossy()
            .into_owned(),
    ]);
    for path in &ffi_link_metadata.library_paths {
        argv.push(format!("-L{}", path.display()));
    }
    for library in &ffi_link_metadata.libraries {
        argv.push(format!("-l{library}"));
    }
    for framework in &ffi_link_metadata.frameworks {
        argv.extend(["-framework".to_string(), framework.clone()]);
    }
    argv.extend(ffi_link_metadata.link_args.iter().cloned());
    if uses_dynamic_loader && target.operating_system().as_str() == "linux" {
        argv.push("-ldl".to_string());
    }
    if uses_winsock && target.operating_system().as_str() == "windows" {
        argv.push("-lws2_32".to_string());
    }
    if (uses_native_tasks || uses_bundled_sqlite) && target.operating_system().as_str() != "windows"
    {
        argv.push("-pthread".to_string());
    }
    if uses_math && target.operating_system().as_str() != "windows" {
        argv.push("-lm".to_string());
    }
    Ok(argv)
}

fn command_argv(command: &Command) -> Vec<String> {
    std::iter::once(command.get_program())
        .chain(command.get_args())
        .map(|part| part.to_string_lossy().into_owned())
        .collect()
}

fn absolute_build_path(path: &Path) -> Result<PathBuf, BuildError> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        Ok(std::env::current_dir()
            .map_err(|error| BuildError::Message(error.to_string()))?
            .join(path))
    }
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
        || source.contains("#define NOMO_ASYNC_BLOCKING_POOL_MAX_THREADS")
}

pub(super) fn generated_c_uses_bundled_sqlite(source: &str) -> bool {
    source.contains("#define NOMO_SQLITE_MAX_DATABASES")
}

pub(super) fn generated_c_uses_math(source: &str) -> bool {
    c_calls_any_symbol(
        source,
        &[
            "sqrt", "sqrtf", "pow", "powf", "fmod", "fmodf", "floor", "floorf", "ceil", "ceilf",
            "sin", "sinf", "cos", "cosf", "tan", "tanf", "exp", "expf", "log", "logf",
        ],
    )
}

fn c_calls_any_symbol(source: &str, symbols: &[&str]) -> bool {
    #[derive(Clone, Copy)]
    enum State {
        Code,
        LineComment,
        BlockComment,
        String,
        Character,
    }

    let bytes = source.as_bytes();
    let mut state = State::Code;
    let mut index = 0;
    while index < bytes.len() {
        match state {
            State::Code if bytes[index..].starts_with(b"//") => {
                state = State::LineComment;
                index += 2;
            }
            State::Code if bytes[index..].starts_with(b"/*") => {
                state = State::BlockComment;
                index += 2;
            }
            State::Code if bytes[index] == b'"' => {
                state = State::String;
                index += 1;
            }
            State::Code if bytes[index] == b'\'' => {
                state = State::Character;
                index += 1;
            }
            State::Code if bytes[index].is_ascii_alphabetic() || bytes[index] == b'_' => {
                let start = index;
                index += 1;
                while index < bytes.len()
                    && (bytes[index].is_ascii_alphanumeric() || bytes[index] == b'_')
                {
                    index += 1;
                }
                let identifier = &source[start..index];
                index = skip_c_whitespace_and_comments(bytes, index);
                if bytes.get(index) == Some(&b'(') && symbols.contains(&identifier) {
                    return true;
                }
            }
            State::LineComment if bytes[index] == b'\n' => {
                state = State::Code;
                index += 1;
            }
            State::BlockComment if bytes[index..].starts_with(b"*/") => {
                state = State::Code;
                index += 2;
            }
            State::String | State::Character if bytes[index] == b'\\' => {
                index = (index + 2).min(bytes.len());
            }
            State::String if bytes[index] == b'"' => {
                state = State::Code;
                index += 1;
            }
            State::Character if bytes[index] == b'\'' => {
                state = State::Code;
                index += 1;
            }
            _ => index += 1,
        }
    }
    false
}

fn skip_c_whitespace_and_comments(bytes: &[u8], mut index: usize) -> usize {
    loop {
        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        if bytes[index..].starts_with(b"//") {
            index += 2;
            while index < bytes.len() && bytes[index] != b'\n' {
                index += 1;
            }
            continue;
        }
        if bytes[index..].starts_with(b"/*") {
            index += 2;
            while index < bytes.len() && !bytes[index..].starts_with(b"*/") {
                index += 1;
            }
            index = (index + 2).min(bytes.len());
            continue;
        }
        return index;
    }
}

pub(super) fn generated_c_uses_dynamic_loader(source: &str) -> bool {
    source.contains("dlopen(") || source.contains("dlsym(") || source.contains("dlclose(")
}

pub(super) fn generated_c_uses_winsock(source: &str) -> bool {
    source.contains("WSAStartup(")
        || source.contains("socket(")
        || source.contains("getaddrinfo(")
        || source.contains("WSAPoll(")
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_c_capabilities_select_libm_without_workload_names() {
        assert!(generated_c_uses_math("double value = sqrt(input);"));
        assert!(generated_c_uses_math(
            "float value = powf /* intrinsic call */ (left, right);"
        ));
        assert!(!generated_c_uses_math("int main(void) { return 0; }"));
        assert!(!generated_c_uses_math(
            "const char *name = \"sqrt(\"; /* pow(left, right) */"
        ));
        assert!(!generated_c_uses_math("double my_sqrt(double value);"));
    }

    #[test]
    fn codegen_cache_separates_profile_and_misses_after_pipeline_change() {
        let debug = codegen_cache_configuration(BuildProfile::Debug, PASS_PIPELINE_VERSION);
        let release = codegen_cache_configuration(BuildProfile::Release, PASS_PIPELINE_VERSION);
        let next_pipeline =
            codegen_cache_configuration(BuildProfile::Release, PASS_PIPELINE_VERSION + 1);
        assert_ne!(debug, release);
        assert_ne!(release, next_pipeline);
        assert!(release.contains(super::super::build_metadata::COMPILER_REVISION));
        assert!(release.contains(super::super::build_metadata::RUNTIME_REVISION));

        let root = std::env::temp_dir().join(format!(
            "nomo-codegen-cache-profile-pipeline-{}",
            std::process::id()
        ));
        if root.exists() {
            fs::remove_dir_all(&root).unwrap();
        }
        let cache = PersistentQueryCache::at_root(&root);
        let fingerprint = crate::incremental::ContentFingerprint::of_text("unchanged sources");
        let current_key = crate::incremental::QueryKey::new(
            "test-target",
            "codegen-c",
            release,
            fingerprint.clone(),
        );
        let next_key = crate::incremental::QueryKey::new(
            "test-target",
            "codegen-c",
            next_pipeline,
            fingerprint,
        );
        assert_eq!(cache.get::<String>(&current_key), None);
        cache
            .insert(&current_key, &"generated C".to_string())
            .unwrap();
        assert_eq!(
            cache.get::<String>(&current_key).as_deref(),
            Some("generated C")
        );
        assert_eq!(
            cache.get::<String>(&next_key),
            None,
            "a pass-pipeline version change must miss even with identical sources"
        );
        fs::remove_dir_all(&root).unwrap();
    }
}
