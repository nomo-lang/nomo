use nomo::target::TargetTriple;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

static TEST_SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn test_root(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "nomo-cli-release-{name}-{}-{}",
        std::process::id(),
        TEST_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    if root.exists() {
        fs::remove_dir_all(&root).unwrap();
    }
    fs::create_dir_all(&root).unwrap();
    root
}

fn create_project(root: &Path, name: &str, source: Option<&str>) -> PathBuf {
    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("new")
        .arg(name)
        .current_dir(root)
        .output()
        .unwrap();
    assert_success(&output);
    let project = root.join(name);
    if let Some(source) = source {
        fs::write(project.join("src/main.nomo"), source).unwrap();
    }
    project
}

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn resolve_dependencies(path: &Path, workspace: bool) {
    let mut command = Command::new(env!("CARGO_BIN_EXE_nomo"));
    command.arg("deps").arg("resolve").arg(path);
    if workspace {
        command.arg("--workspace");
    }
    let output = command.output().unwrap();
    assert_success(&output);
}

fn read_json(path: &Path) -> Value {
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

fn sha256(path: &Path) -> String {
    format!("{:x}", Sha256::digest(fs::read(path).unwrap()))
}

fn recompute_content_binding(metadata: &Value) -> String {
    let binding = &metadata["content_binding"];
    let mut hasher = Sha256::new();
    let mut add_part = |part: &str| {
        hasher.update((part.len() as u64).to_be_bytes());
        hasher.update(part.as_bytes());
    };
    add_part(binding["domain"].as_str().unwrap());
    for name in binding["input_order"].as_array().unwrap() {
        let name = name.as_str().unwrap();
        add_part(name);
        add_part(binding["inputs"][name].as_str().unwrap());
    }
    format!("{:x}", hasher.finalize())
}

fn executable(path: PathBuf) -> PathBuf {
    if cfg!(windows) {
        path.with_extension("exe")
    } else {
        path
    }
}

fn shlex_join(argv: &[Value]) -> String {
    argv.iter()
        .map(|argument| {
            let argument = argument.as_str().unwrap();
            if !argument.is_empty()
                && argument.chars().all(|character| {
                    character.is_ascii_alphanumeric()
                        || matches!(
                            character,
                            '_' | '@' | '%' | '+' | '=' | ':' | ',' | '.' | '/' | '-'
                        )
                })
            {
                argument.to_string()
            } else {
                format!("'{}'", argument.replace('\'', "'\"'\"'"))
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[test]
fn build_help_and_release_artifacts_satisfy_schema_one() {
    let help = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .args(["build", "--help"])
        .output()
        .unwrap();
    assert_success(&help);
    let help_text = String::from_utf8_lossy(&help.stdout);
    assert!(help_text.contains("--release"), "{help_text}");
    assert!(help_text.contains("--emit-c"), "{help_text}");

    let root = test_root("build-contract");
    let project = create_project(&root, "release-demo", None);
    resolve_dependencies(&project, false);
    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("build")
        .arg(&project)
        .args(["--release", "--locked", "--offline"])
        .output()
        .unwrap();
    assert_success(&output);
    let build = project.join("build");
    let c_path = build.join("c/main.c");
    let binary = executable(build.join("bin/release-demo"));
    let sidecar_path = build.join("release-provenance.json");
    let metadata_path = build.join("nomo-build-metadata.json");
    let sidecar = read_json(&sidecar_path);
    let metadata = read_json(&metadata_path);

    let expected_sidecar_keys = BTreeSet::from([
        "binary",
        "compile_commands",
        "complete_argv",
        "compiler",
        "generated_c",
        "link_command",
        "objects",
        "schema",
    ]);
    assert_eq!(
        sidecar
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>(),
        expected_sidecar_keys
    );
    assert_eq!(sidecar["schema"], 1);
    assert_eq!(sidecar["complete_argv"], true);
    assert_eq!(sidecar["objects"].as_array().unwrap().len(), 1);
    assert_eq!(sidecar["compile_commands"].as_array().unwrap().len(), 1);
    let compiler = &sidecar["compiler"];
    for field in [
        "path",
        "realpath",
        "sha256",
        "version_output",
        "target_triple",
    ] {
        assert!(compiler.get(field).is_some(), "missing compiler.{field}");
    }
    assert!(Path::new(compiler["path"].as_str().unwrap()).is_absolute());
    assert!(Path::new(compiler["realpath"].as_str().unwrap()).is_absolute());
    assert_eq!(
        sha256(Path::new(compiler["realpath"].as_str().unwrap())),
        compiler["sha256"]
    );
    let object = &sidecar["objects"][0];
    let object_path = Path::new(object["path"].as_str().unwrap());
    assert!(object_path.is_absolute() && object_path.is_file());
    assert_eq!(sha256(object_path), object["sha256"]);

    let compile = &sidecar["compile_commands"][0];
    let link = &sidecar["link_command"];
    let command_keys = BTreeSet::from([
        "argv",
        "command",
        "cwd",
        "duration_ns",
        "environment",
        "exit_code",
    ]);
    for command in [compile, link] {
        assert_eq!(
            command
                .as_object()
                .unwrap()
                .keys()
                .map(String::as_str)
                .collect::<BTreeSet<_>>(),
            command_keys
        );
        assert_eq!(command["exit_code"], 0);
        assert!(command["duration_ns"].as_u64().is_some());
        assert_eq!(
            command["cwd"],
            std::env::current_dir()
                .unwrap()
                .canonicalize()
                .unwrap()
                .to_string_lossy()
                .as_ref()
        );
        assert_eq!(
            command["command"],
            shlex_join(command["argv"].as_array().unwrap())
        );
        assert_eq!(command["environment"], compile["environment"]);
    }
    let expected_compile = vec![
        compiler["path"].clone(),
        Value::String("--no-default-config".to_string()),
        Value::String("-std=c99".to_string()),
        Value::String("-O3".to_string()),
        Value::String("-DNDEBUG".to_string()),
        Value::String("-fomit-frame-pointer".to_string()),
        Value::String("-c".to_string()),
        Value::String(c_path.to_string_lossy().into_owned()),
        Value::String("-o".to_string()),
        object["path"].clone(),
    ];
    assert_eq!(compile["argv"], Value::Array(expected_compile));
    let expected_link = vec![
        compiler["path"].clone(),
        Value::String("--no-default-config".to_string()),
        object["path"].clone(),
        Value::String("-o".to_string()),
        Value::String(binary.to_string_lossy().into_owned()),
    ];
    assert_eq!(
        link["argv"],
        Value::Array(expected_link),
        "a non-math program must not receive -lm"
    );
    for forbidden in [
        "-ffast-math",
        "-Ofast",
        "-flto",
        "-fprofile-generate",
        "-march=native",
        "-mcpu=native",
    ] {
        assert!(
            !compile["argv"]
                .as_array()
                .unwrap()
                .contains(&Value::String(forbidden.to_string()))
        );
        assert!(
            !link["argv"]
                .as_array()
                .unwrap()
                .contains(&Value::String(forbidden.to_string()))
        );
    }
    if cfg!(target_os = "macos") {
        assert!(compile["environment"].get("darwin_sdk").is_some());
    }
    if cfg!(windows) {
        assert!(compile["environment"].get("windows_toolchain").is_some());
    }

    assert_eq!(
        sidecar["generated_c"]["path"],
        c_path.to_string_lossy().as_ref()
    );
    assert_eq!(sidecar["generated_c"]["sha256"], sha256(&c_path));
    assert_eq!(sidecar["binary"]["path"], binary.to_string_lossy().as_ref());
    assert_eq!(sidecar["binary"]["sha256"], sha256(&binary));
    assert_eq!(metadata["schema"], 1);
    assert_eq!(metadata["selected_profile"], "release");
    assert_eq!(
        metadata["release_provenance"]["path"],
        sidecar_path.to_string_lossy().as_ref()
    );
    assert_eq!(
        metadata["release_provenance"]["sha256"],
        sha256(&sidecar_path)
    );
    for input in [
        "profile",
        "target_triple",
        "compiler_revision",
        "runtime_revision",
        "pass_pipeline_version",
        "toolchain_config_version",
        "toolchain_config_sha256",
    ] {
        assert!(
            metadata["cache_identity"]["inputs"].get(input).is_some(),
            "missing cache input {input}"
        );
    }
    assert_eq!(metadata["cache_identity"]["inputs"]["profile"], "release");
    assert_eq!(
        metadata["cache_identity"]["cache_key"],
        format!(
            "{:x}",
            Sha256::digest(
                metadata["cache_identity"]["query_key_json"]
                    .as_str()
                    .unwrap()
                    .as_bytes()
            )
        )
    );
    assert_eq!(metadata["content_binding"]["schema"], 1);
    assert_eq!(
        metadata["content_binding"]["domain"],
        "nomo-build-metadata-content-binding-v1"
    );
    assert_eq!(metadata["content_binding"]["inputs"]["profile"], "release");
    assert_eq!(
        metadata["content_binding"]["inputs"]["cache_key"],
        metadata["cache_identity"]["cache_key"]
    );
    assert_eq!(
        metadata["content_binding"]["inputs"]["generated_c_path"],
        metadata["generated_c"]["path"]
    );
    assert_eq!(
        metadata["content_binding"]["inputs"]["generated_c_sha256"],
        metadata["generated_c"]["sha256"]
    );
    assert_eq!(
        metadata["content_binding"]["inputs"]["binary_path"],
        metadata["binary"]["path"]
    );
    assert_eq!(
        metadata["content_binding"]["inputs"]["binary_sha256"],
        metadata["binary"]["sha256"]
    );
    assert_eq!(
        metadata["content_binding"]["inputs"]["release_provenance_path"],
        metadata["release_provenance"]["path"]
    );
    assert_eq!(
        metadata["content_binding"]["inputs"]["release_provenance_sha256"],
        metadata["release_provenance"]["sha256"]
    );
    assert_eq!(
        metadata["content_binding"]["sha256"],
        recompute_content_binding(&metadata)
    );

    let run = Command::new(&binary).output().unwrap();
    assert_success(&run);
    assert_eq!(String::from_utf8_lossy(&run.stdout), "Hello, Nomo\n");
    fs::write(
        project.join("src/main.nomo"),
        "package release_demo\n\nfn main( {\n",
    )
    .unwrap();
    let failed_rebuild = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("build")
        .arg(&project)
        .arg("--release")
        .output()
        .unwrap();
    assert!(!failed_rebuild.status.success());
    assert!(!sidecar_path.exists());
    assert!(!metadata_path.exists());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn release_cli_forms_compose_and_conflicts_fail_explicitly() {
    let root = test_root("cli-forms");
    let source = r#"package args_demo

import std.array
import std.env
import std.io

fn main() {
    let args: Array<string> = env.args()
    let first: Option<string> = args.get(1)
    let message: string = match first {
        Some(text) => text
        None => "missing"
    }
    io.println(message)
}
"#;
    let project = create_project(&root, "args-demo", Some(source));
    resolve_dependencies(&project, false);
    let run = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(&project)
        .arg("--release")
        .arg("--")
        .arg("--release")
        .output()
        .unwrap();
    assert_success(&run);
    assert_eq!(String::from_utf8_lossy(&run.stdout), "--release\n");
    assert_eq!(
        read_json(&project.join("build/nomo-build-metadata.json"))["selected_profile"],
        "release"
    );

    fs::write(
        project.join("src/main.nomo"),
        "package args_demo\n\n#[test]\nfn release_test() {\n}\n",
    )
    .unwrap();
    let invoke_tests = |release: bool| {
        let mut command = Command::new(env!("CARGO_BIN_EXE_nomo"));
        command
            .arg("test")
            .arg(&project)
            .args(["--locked", "--offline"])
            .env("NOMO_INCREMENTAL_TRACE", "1");
        if release {
            command.arg("--release");
        }
        command.output().unwrap()
    };
    for (release, expected) in [
        (false, "write"),
        (false, "hit"),
        (true, "write"),
        (true, "hit"),
    ] {
        let tests = invoke_tests(release);
        assert_success(&tests);
        assert_eq!(
            String::from_utf8_lossy(&tests.stdout),
            "running 1 tests\nok args_demo.release_test\n"
        );
        let stderr = String::from_utf8_lossy(&tests.stderr);
        assert!(
            stderr.contains(&format!("incremental-cache {expected} codegen-c-test")),
            "release={release} expected={expected}\n{stderr}"
        );
    }
    assert!(project.join("build/test/release/c").is_dir());
    assert!(project.join("build/test/release/bin").is_dir());

    let workspace_root = test_root("workspace-options");
    let workspace_member = create_project(&workspace_root, "workspace-member", None);
    fs::write(
        workspace_root.join("nomo.toml"),
        "[workspace]\nmembers = [\"workspace-member\"]\n",
    )
    .unwrap();
    resolve_dependencies(&workspace_root, true);
    let host = TargetTriple::host().unwrap().to_string();
    let workspace_build = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("build")
        .arg(&workspace_root)
        .args([
            "--workspace",
            "--release",
            "--locked",
            "--offline",
            "--json-errors",
            "--target",
        ])
        .arg(&host)
        .output()
        .unwrap();
    assert_success(&workspace_build);
    assert_eq!(
        read_json(
            &workspace_member
                .join("build")
                .join(&host)
                .join("nomo-build-metadata.json")
        )["selected_profile"],
        "release"
    );

    let conflict_root = test_root("build-conflict");
    let conflict_project = create_project(&conflict_root, "conflict", None);
    let seed = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("build")
        .arg(&conflict_project)
        .arg("--release")
        .output()
        .unwrap();
    assert_success(&seed);
    assert!(
        conflict_project
            .join("build/release-provenance.json")
            .exists()
    );
    assert!(
        conflict_project
            .join("build/nomo-build-metadata.json")
            .exists()
    );
    let conflict = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("build")
        .arg(&conflict_project)
        .args(["--release", "--emit-c"])
        .output()
        .unwrap();
    assert!(!conflict.status.success());
    assert!(
        String::from_utf8_lossy(&conflict.stderr)
            .contains("`--release` and `--emit-c` cannot be used together")
    );
    assert!(
        !conflict_project
            .join("build/release-provenance.json")
            .exists()
    );
    assert!(
        !conflict_project
            .join("build/nomo-build-metadata.json")
            .exists()
    );
    fs::remove_dir_all(root).unwrap();
    fs::remove_dir_all(workspace_root).unwrap();
    fs::remove_dir_all(conflict_root).unwrap();
}

#[test]
fn nomoc_release_preserves_pure_c_stdout_and_out_contracts() {
    let root = test_root("nomoc");
    let source = root.join("single.nomo");
    fs::write(&source, "package single\n\nfn main() {\n}\n").unwrap();

    let stdout_build = Command::new(env!("CARGO_BIN_EXE_nomoc"))
        .arg("build")
        .arg(&source)
        .arg("--release")
        .output()
        .unwrap();
    assert_success(&stdout_build);
    let stdout = String::from_utf8(stdout_build.stdout).unwrap();
    assert!(stdout.contains("#include <"), "{stdout}");
    assert!(!stdout.contains("built "));
    assert!(!stdout.contains("metadata"));
    assert!(stdout_build.stderr.is_empty());
    let metadata_path = root.join("build/nomo-build-metadata.json");
    let metadata = read_json(&metadata_path);
    assert_eq!(metadata["selected_profile"], "release");
    assert!(metadata["binary"].is_null());
    assert!(metadata["release_provenance"].is_null());
    assert_eq!(metadata["cache_identity"]["inputs"]["profile"], "release");
    let release_cache_key = metadata["cache_identity"]["cache_key"].clone();
    let release_hit = Command::new(env!("CARGO_BIN_EXE_nomoc"))
        .arg("build")
        .arg(&source)
        .arg("--release")
        .env("NOMO_INCREMENTAL_TRACE", "1")
        .output()
        .unwrap();
    assert_success(&release_hit);
    assert!(String::from_utf8_lossy(&release_hit.stdout).contains("#include <"));
    let release_hit_stderr = String::from_utf8_lossy(&release_hit.stderr);
    let release_trace_path = release_hit_stderr
        .lines()
        .find(|line| line.contains("incremental-cache hit codegen-c:"))
        .and_then(|line| line.rsplit_once(' ').map(|(_, path)| Path::new(path)))
        .expect("nomoc release cache hit must expose its real cache entry path");
    assert_eq!(
        release_trace_path
            .file_stem()
            .and_then(|stem| stem.to_str()),
        release_cache_key.as_str(),
        "nomoc metadata must disclose its actual persistent QueryKey digest"
    );

    let debug_build = Command::new(env!("CARGO_BIN_EXE_nomoc"))
        .arg("build")
        .arg(&source)
        .output()
        .unwrap();
    assert_success(&debug_build);
    let debug_stdout = String::from_utf8(debug_build.stdout).unwrap();
    assert!(debug_stdout.contains("#include <"), "{debug_stdout}");
    assert!(debug_build.stderr.is_empty());
    let debug_metadata = read_json(&metadata_path);
    assert_eq!(debug_metadata["selected_profile"], "debug");
    assert_eq!(
        debug_metadata["cache_identity"]["inputs"]["profile"],
        "debug"
    );
    assert_ne!(
        debug_metadata["cache_identity"]["cache_key"], release_cache_key,
        "nomoc debug and release cache identities must not alias"
    );

    let output_c = root.join("out/generated.c");
    let host = TargetTriple::host().unwrap().to_string();
    let out_build = Command::new(env!("CARGO_BIN_EXE_nomoc"))
        .arg("build")
        .arg(&source)
        .arg("--release")
        .arg("--target")
        .arg(&host)
        .arg("--out")
        .arg(&output_c)
        .output()
        .unwrap();
    assert_success(&out_build);
    assert_eq!(
        String::from_utf8_lossy(&out_build.stdout),
        format!("emitted {}\n", output_c.display())
    );
    assert!(
        fs::read_to_string(&output_c)
            .unwrap()
            .contains("#include <")
    );
    assert_eq!(
        read_json(&metadata_path)["generated_c"]["path"],
        output_c.to_string_lossy().as_ref()
    );
    fs::write(&source, "package single\n\nfn main( {\n").unwrap();
    let failed_rebuild = Command::new(env!("CARGO_BIN_EXE_nomoc"))
        .arg("build")
        .arg(&source)
        .arg("--release")
        .output()
        .unwrap();
    assert!(!failed_rebuild.status.success());
    assert!(!metadata_path.exists());

    let conflict_root = test_root("nomoc-conflict");
    let conflict_source = conflict_root.join("single.nomo");
    fs::write(&conflict_source, "package single\n\nfn main() {\n}\n").unwrap();
    let seed = Command::new(env!("CARGO_BIN_EXE_nomoc"))
        .arg("build")
        .arg(&conflict_source)
        .arg("--release")
        .output()
        .unwrap();
    assert_success(&seed);
    assert!(
        conflict_root
            .join("build/nomo-build-metadata.json")
            .exists()
    );
    let conflict = Command::new(env!("CARGO_BIN_EXE_nomoc"))
        .arg("build")
        .arg(&conflict_source)
        .args(["--release", "--emit-c"])
        .output()
        .unwrap();
    assert!(!conflict.status.success());
    assert!(
        String::from_utf8_lossy(&conflict.stderr)
            .contains("`--release` and `--emit-c` cannot be used together")
    );
    assert!(!conflict_root.join("build/release-provenance.json").exists());
    assert!(
        !conflict_root
            .join("build/nomo-build-metadata.json")
            .exists()
    );
    fs::remove_dir_all(root).unwrap();
    fs::remove_dir_all(conflict_root).unwrap();
}

#[test]
fn codegen_cache_is_warm_per_profile_and_never_crosses_profiles() {
    let root = test_root("cache");
    let project = create_project(&root, "cache-demo", None);
    let invoke = |release: bool| {
        let mut command = Command::new(env!("CARGO_BIN_EXE_nomo"));
        command
            .arg("build")
            .arg(&project)
            .env("NOMO_INCREMENTAL_TRACE", "1");
        if release {
            command.arg("--release");
        }
        command.output().unwrap()
    };
    let mut debug_key = None;
    let mut release_key = None;
    for (release, expected) in [
        (false, "write"),
        (false, "hit"),
        (true, "write"),
        (true, "hit"),
        (false, "hit"),
    ] {
        let output = invoke(release);
        assert_success(&output);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains(&format!("incremental-cache {expected} codegen-c")),
            "release={release} expected={expected}\n{stderr}"
        );
        let metadata = read_json(&project.join("build/nomo-build-metadata.json"));
        let metadata_key = metadata["cache_identity"]["cache_key"]
            .as_str()
            .unwrap()
            .to_string();
        let remembered = if release {
            &mut release_key
        } else {
            &mut debug_key
        };
        if let Some(remembered) = remembered.as_ref() {
            assert_eq!(
                &metadata_key, remembered,
                "cold/warm builds for one profile must disclose one stable cache key"
            );
        } else {
            *remembered = Some(metadata_key.clone());
        }
        if expected == "hit" {
            let trace_path = stderr
                .lines()
                .find(|line| line.contains("incremental-cache hit codegen-c:"))
                .and_then(|line| line.rsplit_once(' ').map(|(_, path)| Path::new(path)))
                .expect("codegen cache hit trace must include its real cache entry path");
            assert_eq!(
                trace_path.file_stem().and_then(|stem| stem.to_str()),
                Some(metadata_key.as_str()),
                "metadata cache key must equal the actual persistent QueryKey digest"
            );
        }
    }
    assert_ne!(debug_key, release_key);
    let emit_c = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("build")
        .arg(&project)
        .arg("--emit-c")
        .env("NOMO_INCREMENTAL_TRACE", "1")
        .output()
        .unwrap();
    assert_success(&emit_c);
    assert!(String::from_utf8_lossy(&emit_c.stderr).contains("incremental-cache hit codegen-c"));
    let emit_metadata = read_json(&project.join("build/nomo-build-metadata.json"));
    assert_eq!(emit_metadata["selected_profile"], "debug");
    assert_eq!(
        emit_metadata["cache_identity"]["cache_key"],
        Value::String(debug_key.unwrap())
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn release_preserves_bounds_overflow_divzero_cow_order_cleanup_io_and_float_contract() {
    let root = test_root("semantics");
    let failures = [
        (
            "bounds_case",
            r#"package bounds_case

import std.array

fn main() {
    let mut items: Array<i32> = Array.new<i32>()
    items.push(1)
    items.set(1, 2)
}
"#,
            "panic: Array.set index out of bounds",
        ),
        (
            "overflow_case",
            r#"package overflow_case

fn main() {
    let max: i64 = 9223372036854775807
    let value: i64 = max + 1
}
"#,
            "panic: signed integer overflow",
        ),
        (
            "divzero_case",
            r#"package divzero_case

fn main() {
    let value: i64 = 1 / 0
}
"#,
            "panic: division by zero",
        ),
    ];
    for (name, source, expected) in failures {
        let project = create_project(&root, &name.replace('_', "-"), Some(source));
        let debug = Command::new(env!("CARGO_BIN_EXE_nomo"))
            .arg("run")
            .arg(&project)
            .output()
            .unwrap();
        let release = Command::new(env!("CARGO_BIN_EXE_nomo"))
            .arg("run")
            .arg(&project)
            .arg("--release")
            .output()
            .unwrap();
        assert!(!debug.status.success());
        assert!(!release.status.success());
        assert!(String::from_utf8_lossy(&debug.stderr).contains(expected));
        assert!(String::from_utf8_lossy(&release.stderr).contains(expected));
    }

    let source = r#"package observable_case

import std.array
import std.io
import std.result

fn label(value: Option<string>) -> string {
    return match value {
        Some(text) => text
        None => "missing"
    }
}

fn mark(text: string) -> i64 {
    io.println(text)
    return 1
}

fn add(left: i64, right: i64) -> i64 {
    return left + right
}

fn fail() -> Result<string, string> {
    return Err("stop")
}

fn cleanup() {
    io.println("cleanup")
}

fn early() -> Result<string, string> {
    defer cleanup()
    let value: string = fail()?
    return Ok(value)
}

fn main() {
    let mut items: Array<string> = Array.new<string>()
    items.push("one")
    let snapshot: Array<string> = items
    items.set(0, "two")
    io.println(label(snapshot.get(0)))
    io.println(label(items.get(0)))
    let left: i64 = mark("left")
    let right: i64 = mark("right")
    let total: i64 = add(left, right)
    let ratio: f64 = -1.5
    if ratio < 0.0 {
        io.println("negative f64")
    } else {
        io.println("wrong")
    }
    let outcome: Result<string, string> = early()
    match outcome {
        Ok(value) => {
            io.println(value)
        }
        Err(message) => {
            io.println(message)
        }
    }
}
"#;
    let project = create_project(&root, "observable-case", Some(source));
    let debug = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(&project)
        .output()
        .unwrap();
    let release = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(&project)
        .arg("--release")
        .output()
        .unwrap();
    assert_success(&debug);
    assert_success(&release);
    assert_eq!(release.stdout, debug.stdout);
    assert_eq!(
        String::from_utf8_lossy(&release.stdout),
        "one\ntwo\nleft\nright\nnegative f64\ncleanup\nstop\n"
    );
    let sidecar = read_json(&project.join("build/release-provenance.json"));
    let all_argv = sidecar["compile_commands"]
        .as_array()
        .unwrap()
        .iter()
        .chain(std::iter::once(&sidecar["link_command"]))
        .flat_map(|command| command["argv"].as_array().unwrap())
        .filter_map(Value::as_str)
        .collect::<Vec<_>>();
    assert!(!all_argv.contains(&"-ffast-math"));
    assert!(!all_argv.contains(&"-Ofast"));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn release_does_not_widen_existing_direct_match_scrutinee_evaluation_gap() {
    let root = test_root("legacy-match-scrutinee-gap");
    let source = r#"package match_gap

import std.io
import std.result

fn fail() -> Result<string, string> {
    return Err("stop")
}

fn cleanup() {
    io.println("cleanup")
}

fn early() -> Result<string, string> {
    defer cleanup()
    let value: string = fail()?
    return Ok(value)
}

fn main() {
    match early() {
        Ok(value) => {
            io.println(value)
        }
        Err(message) => {
            io.println(message)
        }
    }
}
"#;
    let project = create_project(&root, "match-gap", Some(source));
    let debug = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(&project)
        .output()
        .unwrap();
    let release = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(&project)
        .arg("--release")
        .output()
        .unwrap();
    assert_success(&debug);
    assert_success(&release);
    assert_eq!(release.stdout, debug.stdout);
    assert_eq!(
        String::from_utf8_lossy(&release.stdout),
        "cleanup\ncleanup\ncleanup\nstop\n",
        "this regression records the pre-existing direct match scrutinee gap; it is not a semantic-conformance assertion"
    );
    fs::remove_dir_all(root).unwrap();
}
