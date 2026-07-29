use super::build_metadata::{
    PASS_PIPELINE_VERSION, codegen_cache_configuration, producer_executable_identity,
    resolve_executable, run_recorded_command,
};
use super::{
    BuildError, BuildProfile, DependencyResolutionOptions, Project, ProjectModuleContext,
    configure_c_compile_command, package_id, project_ffi_link_metadata_with_options,
    project_module_context_with_options,
};
use crate::compiler::compile_source_text_to_c_with_module_identity;
use crate::diagnostic::Diagnostic;
use crate::incremental::{PersistentQueryCache, project_query_key};
use crate::{lexer, parser};
use nomo_manifest::{FfiLinkMetadata, parse_manifest_at_root};
use nomo_target::TargetTriple;
use nomo_test::{TestCaseResult, TestReport, TestStatus};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProjectTestOptions {
    pub filter: Option<String>,
    pub resolution: DependencyResolutionOptions,
    pub profile: BuildProfile,
}

pub fn run_project_tests_with_options(
    project: &Project,
    options: ProjectTestOptions,
) -> Result<TestReport, BuildError> {
    let manifest = parse_manifest_at_root(&project.root).map_err(BuildError::Message)?;
    let project_id = package_id(&manifest.package);
    let context = project_module_context_with_options(project, options.resolution)
        .map_err(BuildError::Message)?;
    let ffi_link_metadata = project_ffi_link_metadata_with_options(project, options.resolution)
        .map_err(BuildError::Message)?;
    let mut test_sources = discover_project_tests(project)?;
    test_sources.sort_by(|left, right| left.name.cmp(&right.name));
    if let Some(filter) = options.filter.as_deref() {
        test_sources.retain(|test| test.name.contains(filter));
    }

    let test_root = if options.profile == BuildProfile::Release {
        project.root.join("build/test/release")
    } else {
        project.root.join("build/test")
    };
    let c_dir = test_root.join("c");
    let bin_dir = test_root.join("bin");
    fs::create_dir_all(&c_dir).map_err(|err| BuildError::Message(err.to_string()))?;
    fs::create_dir_all(&bin_dir).map_err(|err| BuildError::Message(err.to_string()))?;

    let mut results = Vec::new();
    for test in test_sources {
        let started = Instant::now();
        let result = run_single_project_test(
            project,
            &context,
            &ffi_link_metadata,
            &test,
            &c_dir,
            &bin_dir,
            options.profile,
        );
        let duration_ms = started.elapsed().as_millis();
        match result {
            Ok(()) => results.push(TestCaseResult {
                name: test.name,
                status: TestStatus::Ok,
                duration_ms,
                message: None,
            }),
            Err(message) => results.push(TestCaseResult {
                name: test.name,
                status: TestStatus::Failed,
                duration_ms,
                message: Some(message),
            }),
        }
    }

    Ok(TestReport {
        project: project_id,
        tests: results,
    })
}

#[derive(Debug, Clone)]
struct DiscoveredTest {
    name: String,
    function_name: String,
    source_path: PathBuf,
    source: String,
}

fn discover_project_tests(project: &Project) -> Result<Vec<DiscoveredTest>, BuildError> {
    let src = project.root.join("src");
    let mut files = Vec::new();
    collect_nomo_source_files(&src, &mut files).map_err(BuildError::Message)?;
    let mut tests = Vec::new();
    for source_path in files {
        let source = fs::read_to_string(&source_path).map_err(|err| {
            BuildError::Message(format!("failed to read {}: {err}", source_path.display()))
        })?;
        let tokens = lexer::lex(&source_path, &source).map_err(BuildError::Diagnostic)?;
        let ast = parser::parse(&source_path, &tokens).map_err(BuildError::Diagnostic)?;
        for function in ast.functions.iter().filter(|function| function.is_test) {
            validate_test_function(&source_path, function)?;
            let mut name = function.package.join(".");
            name.push('.');
            name.push_str(&function.name);
            tests.push(DiscoveredTest {
                name,
                function_name: function.name.clone(),
                source_path: source_path.clone(),
                source: source.clone(),
            });
        }
    }
    Ok(tests)
}

fn collect_nomo_source_files(dir: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    for entry in fs::read_dir(dir)
        .map_err(|err| format!("failed to read source directory {}: {err}", dir.display()))?
    {
        let entry = entry.map_err(|err| err.to_string())?;
        let path = entry.path();
        if path.is_dir() {
            collect_nomo_source_files(&path, files)?;
        } else if path.is_file() && path.extension().and_then(|ext| ext.to_str()) == Some("nomo") {
            files.push(path);
        }
    }
    Ok(())
}

fn validate_test_function(
    source_path: &Path,
    function: &crate::ast::Function,
) -> Result<(), BuildError> {
    let error = |message: &str| {
        BuildError::Diagnostic(Diagnostic::new(
            "E1101",
            message,
            source_path,
            function.span.line,
            function.span.column,
            function.span.length,
            &function.span.text,
        ))
    };
    if function.name == "main" {
        return Err(error("`#[test]` function cannot be named `main`"));
    }
    if function.is_suspend {
        return Err(error(
            "`#[test]` functions must be synchronous until the async test runner is available",
        ));
    }
    if !function.type_params.is_empty() {
        return Err(error(
            "`#[test]` functions must not declare type parameters",
        ));
    }
    if !function.params.is_empty() {
        return Err(error("`#[test]` functions must not take parameters"));
    }
    if !is_void_type(&function.return_type) {
        return Err(error("`#[test]` functions must return `void`"));
    }
    Ok(())
}

fn is_void_type(type_ref: &crate::ast::TypeRef) -> bool {
    type_ref.path == ["void"] && type_ref.args.is_empty()
}

fn run_single_project_test(
    project: &Project,
    context: &ProjectModuleContext,
    ffi_link_metadata: &FfiLinkMetadata,
    test: &DiscoveredTest,
    c_dir: &Path,
    bin_dir: &Path,
    profile: BuildProfile,
) -> Result<(), String> {
    let runner_source = test_runner_source(&test.source, &test.function_name);
    let target = TargetTriple::host()?;
    let cache_root = project
        .workspace_root
        .as_deref()
        .unwrap_or(project.root.as_path());
    let cache = PersistentQueryCache::at_root(cache_root);
    let producer_executable = producer_executable_identity().map_err(|error| error.human())?;
    let cache_key = project_query_key(
        project,
        &context.external_modules,
        &[(test.source_path.clone(), runner_source.clone())],
        &target,
        "codegen-c-test",
        format!(
            "{}:{}:{}",
            project.name,
            test.name,
            codegen_cache_configuration(profile, PASS_PIPELINE_VERSION, &producer_executable)
        ),
    );
    let c = match cache.get::<String>(&cache_key) {
        Some(cached) => cached,
        None => {
            let generated = compile_source_text_to_c_with_module_identity(
                &test.source_path,
                &runner_source,
                &context.local_source_root,
                &context.local_identity,
                &context.external_import_roots,
                &context.external_modules,
            )
            .map_err(|diag| diag.human())?;
            let _ = cache.insert(&cache_key, &generated);
            generated
        }
    };
    let file_stem = safe_test_artifact_name(&test.name);
    let c_path = c_dir.join(format!("{file_stem}.c"));
    let bin_path = bin_dir.join(&file_stem);
    let uses_native_tasks = super::build::generated_c_uses_native_tasks(&c);
    let uses_bundled_sqlite = super::build::generated_c_uses_bundled_sqlite(&c);
    fs::write(&c_path, &c).map_err(|err| format!("failed to write {}: {err}", c_path.display()))?;
    super::build::materialize_bundled_sqlite(c_dir, uses_bundled_sqlite)
        .map_err(|err| format!("failed to materialize bundled SQLite: {err}"))?;
    let mut toolchain = target.c_toolchain_from(&target)?;
    if profile == BuildProfile::Release {
        toolchain.program = "clang".to_string();
    }
    let compiler = if profile == BuildProfile::Release {
        resolve_executable(&toolchain.program, profile).map_err(|error| error.human())?
    } else {
        PathBuf::from(&toolchain.program)
    };
    if profile == BuildProfile::Release {
        let object_dir = c_dir.parent().unwrap_or(c_dir).join("obj").join(&file_stem);
        fs::create_dir_all(&object_dir)
            .map_err(|error| format!("failed to create {}: {error}", object_dir.display()))?;
        let (objects, _) = super::build::compile_release_translation_units(
            &compiler,
            &toolchain,
            &c_path,
            c_dir,
            &object_dir,
            ffi_link_metadata,
            uses_bundled_sqlite,
        )
        .map_err(|error| error.human())?;
        let link_argv = super::build::release_link_argv(
            &compiler,
            &toolchain,
            &objects,
            &bin_path,
            ffi_link_metadata,
            &target,
            uses_native_tasks,
            uses_bundled_sqlite,
            super::build::generated_c_uses_math(&c),
            super::build::generated_c_uses_dynamic_loader(&c),
            super::build::generated_c_uses_winsock(&c),
        )
        .map_err(|error| error.human())?;
        run_recorded_command(link_argv, profile).map_err(|error| error.human())?;
    } else {
        let mut command = Command::new(&compiler);
        command.args(&toolchain.args);
        configure_c_compile_command(
            &mut command,
            &c_path,
            &bin_path,
            ffi_link_metadata,
            &target,
            uses_native_tasks,
            uses_bundled_sqlite,
        );
        let output = command.output().map_err(|err| {
            format!(
                "failed to run C compiler `{}` for target `{target}`: {err}",
                toolchain.program
            )
        })?;
        if !output.status.success() {
            return Err(format!(
                "C compiler failed for target `{target}`:\n{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            ));
        }
    }

    let run_path = fs::canonicalize(&bin_path)
        .map_err(|err| format!("failed to resolve {}: {err}", bin_path.display()))?;
    let output = Command::new(&run_path)
        .current_dir(&project.root)
        .output()
        .map_err(|err| format!("failed to run {}: {err}", run_path.display()))?;
    if output.status.success() {
        return Ok(());
    }
    let status = output.status.code().unwrap_or(1);
    let message = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    if message.trim().is_empty() {
        Err(format!("test exited with status {status}"))
    } else {
        Err(message.trim().to_string())
    }
}

fn test_runner_source(source: &str, function_name: &str) -> String {
    let mut runner = rename_existing_main(source);
    if !runner.ends_with('\n') {
        runner.push('\n');
    }
    runner.push_str("\nfn main() {\n    ");
    runner.push_str(function_name);
    runner.push_str("()\n}\n");
    runner
}

fn rename_existing_main(source: &str) -> String {
    let mut output = String::new();
    for line in source.lines() {
        let trimmed = line.trim_start();
        if is_main_declaration_start(trimmed) {
            output.push_str(&line.replacen("fn main", "fn __nomo_original_main", 1));
        } else {
            output.push_str(line);
        }
        output.push('\n');
    }
    output
}

fn is_main_declaration_start(trimmed: &str) -> bool {
    let rest = trimmed
        .strip_prefix("fn main")
        .or_else(|| trimmed.strip_prefix("pub fn main"));
    rest.is_some_and(|rest| {
        rest.starts_with('(')
            || rest
                .chars()
                .next()
                .is_some_and(|ch| ch.is_ascii_whitespace())
    })
}

fn safe_test_artifact_name(name: &str) -> String {
    name.chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}
