use nomo::incremental::PersistentQueryCache;
use nomo::project::{
    BuildProfile, compile_standalone_source_with_profile_cache, record_standalone_c_build_metadata,
};
use nomo::target::TargetTriple;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
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
    fs::canonicalize(root).unwrap()
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
        fs::write(project.join("src").join("main.nomo"), source).unwrap();
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

fn normalized_stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).replace("\r\n", "\n")
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

fn build_release(path: &Path) {
    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("build")
        .arg(path)
        .arg("--release")
        .output()
        .unwrap();
    assert_success(&output);
}

fn build_workspace_release(path: &Path) {
    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("build")
        .arg(path)
        .args(["--workspace", "--release"])
        .output()
        .unwrap();
    assert_success(&output);
}

fn release_evidence_snapshot(project: &Path) -> BTreeMap<String, Vec<u8>> {
    [
        "nomo-build-metadata.json",
        ".nomo-release-owner-v1.json",
        "release-provenance.json",
    ]
    .into_iter()
    .filter_map(|name| {
        let path = project.join("build").join(name);
        path.is_file()
            .then(|| (name.to_string(), fs::read(path).unwrap()))
    })
    .collect()
}

fn assert_release_evidence_snapshot(project: &Path, expected: &BTreeMap<String, Vec<u8>>) {
    assert_eq!(&release_evidence_snapshot(project), expected);
}

fn assert_release_evidence_absent(project: &Path) {
    for name in [
        "nomo-build-metadata.json",
        ".nomo-release-owner-v1.json",
        "release-provenance.json",
    ] {
        assert!(
            !project.join("build").join(name).exists(),
            "{} still has {name}",
            project.display()
        );
    }
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

fn canonical_json_external(value: &Value) -> String {
    fn canonicalize(value: &Value) -> Value {
        match value {
            Value::Array(values) => Value::Array(values.iter().map(canonicalize).collect()),
            Value::Object(values) => {
                let values = values
                    .iter()
                    .map(|(name, value)| (name.clone(), canonicalize(value)))
                    .collect::<BTreeMap<_, _>>();
                Value::Object(values.into_iter().collect())
            }
            value => value.clone(),
        }
    }
    serde_json::to_string(&canonicalize(value)).unwrap()
}

fn assert_independently_recomputable_binding(metadata: &Value) {
    let producer = canonical_json_external(&metadata["producer_executable"]);
    let compiler = canonical_json_external(&metadata["compiler"]);
    let commands = canonical_json_external(&serde_json::json!({
        "compile_commands": metadata["compile_commands"].clone(),
        "link_command": metadata["link_command"].clone(),
        "combined_compile_link_command": metadata["combined_compile_link_command"].clone(),
    }));
    let subdocuments = &metadata["content_binding"]["canonical_subdocuments"];
    assert_eq!(subdocuments["producer_identity"], producer);
    assert_eq!(subdocuments["compiler_identity"], compiler);
    assert_eq!(subdocuments["commands"], commands);
    assert_eq!(
        metadata["content_binding"]["inputs"]["producer_identity_sha256"],
        format!("{:x}", Sha256::digest(producer.as_bytes()))
    );
    assert_eq!(
        metadata["content_binding"]["inputs"]["compiler_identity_sha256"],
        format!("{:x}", Sha256::digest(compiler.as_bytes()))
    );
    assert_eq!(
        metadata["content_binding"]["inputs"]["commands_sha256"],
        format!("{:x}", Sha256::digest(commands.as_bytes()))
    );
    assert_eq!(
        metadata["content_binding"]["sha256"],
        recompute_content_binding(metadata)
    );
}

fn collect_json_files(root: &Path, files: &mut Vec<PathBuf>) {
    if !root.exists() {
        return;
    }
    for entry in fs::read_dir(root).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            collect_json_files(&path, files);
        } else if path.extension().and_then(|extension| extension.to_str()) == Some("json") {
            files.push(path);
        }
    }
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
fn release_subcommand_help_is_documented_and_side_effect_free() {
    let root = test_root("help");
    let build = root.join("build");
    fs::create_dir_all(&build).unwrap();
    let sidecar = build.join("release-provenance.json");
    let metadata = build.join("nomo-build-metadata.json");
    fs::write(&sidecar, b"stale-sidecar").unwrap();
    fs::write(&metadata, b"stale-metadata").unwrap();
    let expected_sidecar = fs::read(&sidecar).unwrap();
    let expected_metadata = fs::read(&metadata).unwrap();
    let expected_entries = fs::read_dir(&build).unwrap().count();

    for (program, args, usage) in [
        (
            env!("CARGO_BIN_EXE_nomo"),
            &["build", "--help"][..],
            "usage: nomo build",
        ),
        (
            env!("CARGO_BIN_EXE_nomo"),
            &["run", "--help"][..],
            "usage: nomo run",
        ),
        (
            env!("CARGO_BIN_EXE_nomo"),
            &["test", "--help"][..],
            "usage: nomo test",
        ),
        (
            env!("CARGO_BIN_EXE_nomoc"),
            &["build", "--help"][..],
            "usage: nomoc build",
        ),
    ] {
        let output = Command::new(program)
            .args(args)
            .current_dir(&root)
            .output()
            .unwrap();
        assert_success(&output);
        assert!(output.stderr.is_empty(), "{args:?}");
        let stdout = String::from_utf8(output.stdout).unwrap();
        assert!(stdout.contains(usage), "{stdout}");
        assert!(stdout.contains("--release"), "{stdout}");
        assert_eq!(fs::read(&sidecar).unwrap(), expected_sidecar);
        assert_eq!(fs::read(&metadata).unwrap(), expected_metadata);
        assert_eq!(fs::read_dir(&build).unwrap().count(), expected_entries);
    }
    fs::remove_dir_all(root).unwrap();
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
    let c_path = build.join("c").join("main.c");
    let binary = executable(build.join("bin").join("release-demo"));
    let sidecar_path = build.join("release-provenance.json");
    let metadata_path = build.join("nomo-build-metadata.json");
    let sidecar = read_json(&sidecar_path);
    let metadata = read_json(&metadata_path);
    assert_eq!(
        metadata["producer_executable"]["path"],
        env!("CARGO_BIN_EXE_nomo")
    );
    assert_eq!(
        metadata["producer_executable"]["sha256"],
        sha256(Path::new(env!("CARGO_BIN_EXE_nomo")))
    );
    assert_eq!(
        metadata["producer_executable"]["size_bytes"],
        fs::metadata(env!("CARGO_BIN_EXE_nomo")).unwrap().len()
    );
    assert_independently_recomputable_binding(&metadata);

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
            std::env::current_dir().unwrap().to_string_lossy().as_ref()
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
    assert_eq!(normalized_stdout(&run), "Hello, Nomo\n");
    fs::write(
        project.join("src").join("main.nomo"),
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
fn manifest_failure_removes_stale_release_evidence_before_discovery() {
    let root = test_root("manifest-failure");
    let project = create_project(&root, "manifest-failure", None);
    resolve_dependencies(&project, false);
    let seed = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("build")
        .arg(&project)
        .arg("--release")
        .output()
        .unwrap();
    assert_success(&seed);
    let sidecar = project.join("build").join("release-provenance.json");
    let metadata = project.join("build").join("nomo-build-metadata.json");
    assert!(sidecar.is_file());
    assert!(metadata.is_file());

    fs::write(project.join("nomo.toml"), "[package\nbroken = true\n").unwrap();
    let failed = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("build")
        .arg(&project)
        .arg("--release")
        .output()
        .unwrap();
    assert!(!failed.status.success());
    assert!(!sidecar.exists());
    assert!(!metadata.exists());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn workspace_cleanup_uses_real_members_and_preserves_every_nonmember_boundary() {
    let root = test_root("workspace-stale-evidence");
    let first = create_project(&root, "first-member", None);
    let second = create_project(&root, "second-member", None);
    let excluded = create_project(&root, "excluded-project", None);
    let path_dependency = create_project(&root, "path-dependency", None);
    let unlisted_parent = root.join("unlisted");
    fs::create_dir_all(&unlisted_parent).unwrap();
    let unlisted = create_project(&unlisted_parent, "unlisted-project", None);
    let nested_repository = create_project(&root, "nested-repository", None);
    fs::create_dir_all(nested_repository.join(".git")).unwrap();
    let vendor_root = root.join("vendor");
    fs::create_dir_all(&vendor_root).unwrap();
    let vendor_dependency = create_project(&vendor_root, "vendor-dependency", None);
    let workspace_manifest = "[workspace]\nmembers = [\"first-member\", \"second-member\", \"excluded-project\"]\nexclude = [\"excluded-project\"]\n";
    fs::write(root.join("nomo.toml"), workspace_manifest).unwrap();
    let mut first_manifest = fs::read_to_string(first.join("nomo.toml")).unwrap();
    first_manifest.push_str(
        "\n[dependencies]\npath_dependency = { package = \"local/path-dependency\", path = \"../path-dependency\" }\n",
    );
    fs::write(first.join("nomo.toml"), &first_manifest).unwrap();
    resolve_dependencies(&root, true);

    let sentinels = [
        &excluded,
        &path_dependency,
        &unlisted,
        &nested_repository,
        &vendor_dependency,
    ];
    build_workspace_release(&root);
    for sentinel in sentinels {
        build_release(sentinel);
    }
    let sentinel_snapshots = sentinels
        .iter()
        .map(|project| (project.to_path_buf(), release_evidence_snapshot(project)))
        .collect::<Vec<_>>();

    build_workspace_release(&root);
    for (project, snapshot) in &sentinel_snapshots {
        assert_release_evidence_snapshot(project, snapshot);
    }
    assert!(
        first
            .join("build")
            .join(".nomo-release-owner-v1.json")
            .is_file()
    );
    assert!(
        second
            .join("build")
            .join(".nomo-release-owner-v1.json")
            .is_file()
    );

    let assert_member_evidence_absent = || {
        for project in [&first, &second] {
            assert_release_evidence_absent(project);
        }
    };

    fs::write(root.join("nomo.toml"), "[workspace]\nmembers = [\n").unwrap();
    fs::write(first.join("nomo.toml"), "[package\nbroken = true\n").unwrap();
    let failed_build = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("build")
        .arg(&root)
        .args(["--workspace", "--release"])
        .output()
        .unwrap();
    assert!(!failed_build.status.success());
    assert_member_evidence_absent();
    for (project, snapshot) in &sentinel_snapshots {
        assert_release_evidence_snapshot(project, snapshot);
    }

    fs::write(root.join("nomo.toml"), workspace_manifest).unwrap();
    fs::write(first.join("nomo.toml"), &first_manifest).unwrap();
    build_workspace_release(&root);
    fs::write(root.join("nomo.toml"), "[workspace]\nmembers = [\n").unwrap();
    fs::write(first.join("nomo.toml"), "[package\nbroken = true\n").unwrap();
    let failed_test = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("test")
        .arg(&root)
        .args(["--workspace", "--release"])
        .output()
        .unwrap();
    assert!(!failed_test.status.success());
    assert_member_evidence_absent();
    for (project, snapshot) in &sentinel_snapshots {
        assert_release_evidence_snapshot(project, snapshot);
    }
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn workspace_package_selection_removed_member_and_path_reuse_never_expand_cleanup_scope() {
    let root = test_root("workspace-selection");
    let first = create_project(&root, "first-member", None);
    let second = create_project(&root, "second-member", None);
    let first_manifest = fs::read_to_string(first.join("nomo.toml")).unwrap();
    let workspace_manifest = "[workspace]\nmembers = [\"first-member\", \"second-member\"]\ndefault-members = [\"first-member\"]\n";
    fs::write(root.join("nomo.toml"), workspace_manifest).unwrap();
    resolve_dependencies(&root, true);

    build_workspace_release(&root);
    let second_before_package_test = release_evidence_snapshot(&second);
    let package_test = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("test")
        .arg(&root)
        .args(["--workspace", "--package", "first-member", "--release"])
        .output()
        .unwrap();
    assert_success(&package_test);
    assert_release_evidence_absent(&first);
    assert_release_evidence_snapshot(&second, &second_before_package_test);

    build_workspace_release(&root);
    let second_before_broken_package_test = release_evidence_snapshot(&second);
    fs::write(root.join("nomo.toml"), "[workspace]\nmembers = [\n").unwrap();
    fs::write(first.join("nomo.toml"), "[package\nbroken = true\n").unwrap();
    let failed_package_test = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("test")
        .arg(&root)
        .args(["--workspace", "--package", "first-member", "--release"])
        .output()
        .unwrap();
    assert!(!failed_package_test.status.success());
    assert_release_evidence_absent(&first);
    assert_release_evidence_snapshot(&second, &second_before_broken_package_test);

    fs::write(root.join("nomo.toml"), workspace_manifest).unwrap();
    fs::write(first.join("nomo.toml"), &first_manifest).unwrap();
    build_workspace_release(&root);
    let first_before_missing_package = release_evidence_snapshot(&first);
    let second_before_missing_package = release_evidence_snapshot(&second);
    fs::write(root.join("nomo.toml"), "[workspace]\nmembers = [\n").unwrap();
    let missing_package = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("test")
        .arg(&root)
        .args(["--workspace", "--package", "missing", "--release"])
        .output()
        .unwrap();
    assert!(!missing_package.status.success());
    assert_release_evidence_snapshot(&first, &first_before_missing_package);
    assert_release_evidence_snapshot(&second, &second_before_missing_package);

    fs::write(
        root.join("nomo.toml"),
        "[workspace]\nmembers = [\"first-member\"]\ndefault-members = [\"first-member\"]\n",
    )
    .unwrap();
    build_workspace_release(&root);
    let second_after_removal = release_evidence_snapshot(&second);
    fs::write(
        second.join("nomo.toml"),
        "manifest-version = 2\n\n[package]\nnamespace = \"replacement\"\nname = \"replacement-member\"\nversion = \"1.0.0\"\nedition = \"2026\"\npublish = false\n",
    )
    .unwrap();
    fs::write(
        second.join("src").join("main.nomo"),
        "package replacement_member\n\nfn main() {\n}\n",
    )
    .unwrap();
    build_release(&second);
    let replacement_evidence = release_evidence_snapshot(&second);
    assert!(
        !replacement_evidence.contains_key(".nomo-release-owner-v1.json"),
        "an unlisted replacement project must not inherit workspace ownership"
    );
    assert_ne!(replacement_evidence, second_after_removal);

    fs::write(root.join("nomo.toml"), "[workspace]\nmembers = [\n").unwrap();
    let failed_after_removal = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("build")
        .arg(&root)
        .args(["--workspace", "--release"])
        .output()
        .unwrap();
    assert!(!failed_after_removal.status.success());
    assert_release_evidence_snapshot(&second, &replacement_evidence);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn workspace_catalog_is_target_neutral_and_corruption_never_authorizes_deletion() {
    let root = test_root("workspace-catalog-corruption");
    let member = create_project(&root, "catalog-member", None);
    let workspace_manifest = "[workspace]\nmembers = [\"catalog-member\"]\n";
    fs::write(root.join("nomo.toml"), workspace_manifest).unwrap();
    resolve_dependencies(&root, true);
    build_workspace_release(&root);

    let state = root
        .join(".nomo")
        .join("state")
        .join("release-evidence")
        .join("v1");
    let scope_path = state.join("scope-id");
    let catalog_path = state.join("catalog.json");
    let scope_before = fs::read(&scope_path).unwrap();
    let catalog_before = fs::read(&catalog_path).unwrap();
    let catalog = read_json(&catalog_path);
    for forbidden in [
        "selected_profile",
        "target_triple",
        "target_scoped_artifacts",
        "layout",
    ] {
        assert!(catalog.get(forbidden).is_none(), "{forbidden}");
    }
    assert!(catalog["stable_scope_id"].as_str().is_some());
    assert!(catalog["catalog_generation"].as_str().is_some());
    assert_eq!(catalog["members"].as_array().unwrap().len(), 1);
    assert!(catalog["members"][0]["relative_root"].as_str().is_some());
    assert!(catalog["members"][0]["member_key"].as_str().is_some());

    let host = TargetTriple::host().unwrap().to_string();
    let explicit_host = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("build")
        .arg(&root)
        .args(["--workspace", "--release", "--target", &host])
        .output()
        .unwrap();
    assert_success(&explicit_host);
    assert_eq!(fs::read(&scope_path).unwrap(), scope_before);
    assert_eq!(fs::read(&catalog_path).unwrap(), catalog_before);
    assert!(
        member
            .join("build")
            .join(&host)
            .join(".nomo-release-owner-v1.json")
            .is_file()
    );

    let host_metadata = fs::read(member.join("build").join("nomo-build-metadata.json")).unwrap();
    let host_sidecar = fs::read(member.join("build").join("release-provenance.json")).unwrap();
    fs::write(root.join("nomo.toml"), "[workspace]\nmembers = [\n").unwrap();
    fs::write(&catalog_path, b"{\n").unwrap();
    let corrupt_catalog = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("build")
        .arg(&root)
        .args(["--workspace", "--release"])
        .output()
        .unwrap();
    assert!(!corrupt_catalog.status.success());
    assert!(
        String::from_utf8_lossy(&corrupt_catalog.stderr).contains("could not be safely cleared")
    );
    assert_eq!(
        fs::read(member.join("build").join("nomo-build-metadata.json")).unwrap(),
        host_metadata
    );
    assert_eq!(
        fs::read(member.join("build").join("release-provenance.json")).unwrap(),
        host_sidecar
    );

    fs::write(&catalog_path, &catalog_before).unwrap();
    let receipt_path = member.join("build").join(".nomo-release-owner-v1.json");
    fs::write(&receipt_path, b"{\n").unwrap();
    let corrupt_receipt = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("test")
        .arg(&root)
        .args(["--workspace", "--release"])
        .output()
        .unwrap();
    assert!(!corrupt_receipt.status.success());
    assert!(
        String::from_utf8_lossy(&corrupt_receipt.stderr).contains("could not be safely cleared")
    );
    assert_eq!(
        fs::read(member.join("build").join("nomo-build-metadata.json")).unwrap(),
        host_metadata
    );
    assert_eq!(
        fs::read(member.join("build").join("release-provenance.json")).unwrap(),
        host_sidecar
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn concurrent_release_builds_publish_one_self_consistent_final_evidence_pair() {
    let root = test_root("concurrent-build");
    let project = create_project(&root, "concurrent-build", None);
    resolve_dependencies(&project, false);
    let spawn = || {
        Command::new(env!("CARGO_BIN_EXE_nomo"))
            .arg("build")
            .arg(&project)
            .arg("--release")
            .spawn()
            .unwrap()
    };
    let first = spawn();
    let second = spawn();
    let first = first.wait_with_output().unwrap();
    let second = second.wait_with_output().unwrap();
    assert_success(&first);
    assert_success(&second);

    let build = project.join("build");
    let sidecar_path = build.join("release-provenance.json");
    let metadata_path = build.join("nomo-build-metadata.json");
    let sidecar = read_json(&sidecar_path);
    let metadata = read_json(&metadata_path);
    assert_eq!(
        metadata["release_provenance"]["sha256"],
        sha256(&sidecar_path)
    );
    assert_eq!(metadata["compiler"], sidecar["compiler"]);
    assert_eq!(metadata["compile_commands"], sidecar["compile_commands"]);
    assert_eq!(metadata["link_command"], sidecar["link_command"]);
    assert_eq!(metadata["generated_c"], sidecar["generated_c"]);
    assert_eq!(metadata["binary"], sidecar["binary"]);
    assert_independently_recomputable_binding(&metadata);
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
    assert_eq!(normalized_stdout(&run), "--release\n");
    assert_eq!(
        read_json(&project.join("build").join("nomo-build-metadata.json"),)["selected_profile"],
        "release"
    );

    fs::write(
        project.join("src").join("main.nomo"),
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
    assert!(
        project
            .join("build")
            .join("test")
            .join("release")
            .join("c")
            .is_dir()
    );
    assert!(
        project
            .join("build")
            .join("test")
            .join("release")
            .join("bin")
            .is_dir()
    );

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
            .join("build")
            .join("release-provenance.json")
            .exists()
    );
    assert!(
        conflict_project
            .join("build")
            .join("nomo-build-metadata.json")
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
            .join("build")
            .join("release-provenance.json")
            .exists()
    );
    assert!(
        !conflict_project
            .join("build")
            .join("nomo-build-metadata.json")
            .exists()
    );
    fs::remove_dir_all(root).unwrap();
    fs::remove_dir_all(workspace_root).unwrap();
    fs::remove_dir_all(conflict_root).unwrap();
}

#[test]
fn standalone_release_run_uses_lexical_absolute_paths_for_every_input_form() {
    let root = test_root("standalone-run-paths");
    let nested = root.join("nested");
    fs::create_dir_all(&nested).unwrap();
    let cases = [
        (
            root.join("bare.nomo"),
            PathBuf::from("bare.nomo"),
            root.join("build"),
            "bare",
        ),
        (
            nested.join("nested.nomo"),
            PathBuf::from("nested").join("nested.nomo"),
            nested.join("build"),
            "nested",
        ),
        (
            root.join("absolute.nomo"),
            root.join("absolute.nomo"),
            root.join("build"),
            "absolute",
        ),
    ];
    for (source, argument, build, expected) in cases {
        fs::write(
            &source,
            format!(
                "package {expected}\n\nimport std.io\n\nfn main() {{\n    io.println(\"{expected}\")\n}}\n"
            ),
        )
        .unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
            .arg("run")
            .arg(&argument)
            .arg("--release")
            .current_dir(&root)
            .output()
            .unwrap();
        assert_success(&output);
        assert_eq!(normalized_stdout(&output), format!("{expected}\n"));
        assert!(build.join("release-provenance.json").is_file());
        assert!(build.join("nomo-build-metadata.json").is_file());
        assert!(executable(build.join("bin").join(expected)).is_file());
    }
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn release_tests_compile_every_translation_unit_with_fixed_optimization_flags() {
    let root = test_root("release-test-ffi-flags");
    let project = create_project(
        &root,
        "release-test-ffi-flags",
        Some("package release_test_ffi_flags\n\n#[test]\nfn optimized_release_harness() {\n}\n"),
    );
    let native = project.join("native");
    fs::create_dir_all(&native).unwrap();
    fs::write(
        native.join("guard.c"),
        "#ifndef NDEBUG\n#error release test FFI translation unit requires NDEBUG\n#endif\n#ifndef __OPTIMIZE__\n#error release test FFI translation unit requires optimization\n#endif\nint nomo_release_test_ffi_guard(void) { return 1; }\n",
    )
    .unwrap();
    let mut manifest = fs::read_to_string(project.join("nomo.toml")).unwrap();
    manifest.push_str("\n[ffi]\nsources = [\"native/guard.c\"]\nlink_args = [\"-O0\"]\n");
    fs::write(project.join("nomo.toml"), manifest).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("test")
        .arg(&project)
        .arg("--release")
        .output()
        .unwrap();
    assert_success(&output);
    assert!(
        normalized_stdout(&output).contains("ok release_test_ffi_flags.optimized_release_harness")
    );
    fs::remove_dir_all(root).unwrap();
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
    let metadata_path = root.join("build").join("nomo-build-metadata.json");
    let metadata = read_json(&metadata_path);
    assert_eq!(
        metadata["producer_executable"]["path"],
        env!("CARGO_BIN_EXE_nomoc")
    );
    assert_eq!(
        metadata["producer_executable"]["sha256"],
        sha256(Path::new(env!("CARGO_BIN_EXE_nomoc")))
    );
    assert_independently_recomputable_binding(&metadata);
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

    let output_c = root.join("out").join("generated.c");
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
            .join("build")
            .join("nomo-build-metadata.json")
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
    assert!(
        !conflict_root
            .join("build")
            .join("release-provenance.json")
            .exists()
    );
    assert!(
        !conflict_root
            .join("build")
            .join("nomo-build-metadata.json")
            .exists()
    );
    fs::remove_dir_all(root).unwrap();
    fs::remove_dir_all(conflict_root).unwrap();
}

#[test]
fn nomoc_rejects_lexical_canonical_symlink_and_hardlink_source_aliases() {
    let root = test_root("nomoc-alias");
    let source = root.join("single.nomo");
    let original = b"package single\n\nfn main() {\n}\n";
    fs::write(&source, original).unwrap();
    fs::create_dir_all(root.join("nested")).unwrap();
    let mut aliases = vec![
        source.clone(),
        root.join("nested").join("..").join("single.nomo"),
    ];
    let hardlink = root.join("hardlink.nomo");
    fs::hard_link(&source, &hardlink).unwrap();
    aliases.push(hardlink);
    #[cfg(unix)]
    {
        let symlink = root.join("symlink.nomo");
        std::os::unix::fs::symlink(&source, &symlink).unwrap();
        aliases.push(symlink);
    }

    for alias in aliases {
        let output = Command::new(env!("CARGO_BIN_EXE_nomoc"))
            .arg("build")
            .arg(&source)
            .arg("--release")
            .arg("--out")
            .arg(&alias)
            .output()
            .unwrap();
        assert!(!output.status.success(), "alias={}", alias.display());
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("must not overwrite or alias source"),
            "alias={}\nstderr={}",
            alias.display(),
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(fs::read(&source).unwrap(), original);
        assert!(!root.join("build").join("nomo-build-metadata.json").exists());
        assert!(!root.join("build").join("release-provenance.json").exists());
    }
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn standalone_metadata_uses_the_exact_query_key_used_before_source_mutation() {
    let root = test_root("standalone-exact-query-key");
    let source = root.join("single.nomo");
    fs::write(&source, "package single\n\nfn main() {\n}\n").unwrap();
    let target = TargetTriple::host().unwrap();
    let generated =
        compile_standalone_source_with_profile_cache(&source, &target, BuildProfile::Release)
            .unwrap();
    let cache = PersistentQueryCache::at_root(&root);
    let mut entries = Vec::new();
    collect_json_files(cache.root(), &mut entries);
    assert_eq!(entries.len(), 1);
    let actual_cache_key = entries[0]
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap()
        .to_string();

    fs::write(
        &source,
        "package single\n\nfn main() {\n    let changed: i64 = 1\n}\n",
    )
    .unwrap();
    record_standalone_c_build_metadata(&source, &generated, None, &target, BuildProfile::Release)
        .unwrap();
    let metadata = read_json(&root.join("build").join("nomo-build-metadata.json"));
    assert_eq!(
        metadata["cache_identity"]["cache_key"], actual_cache_key,
        "metadata must retain the exact QueryKey used by compilation instead of rereading source"
    );
    fs::remove_dir_all(root).unwrap();
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
        let metadata = read_json(&project.join("build").join("nomo-build-metadata.json"));
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
    let emit_metadata = read_json(&project.join("build").join("nomo-build-metadata.json"));
    assert_eq!(emit_metadata["selected_profile"], "debug");
    assert_eq!(
        emit_metadata["cache_identity"]["cache_key"],
        Value::String(debug_key.unwrap())
    );
    fs::remove_dir_all(root).unwrap();
}

#[cfg(target_os = "linux")]
#[test]
fn linux_release_links_libm_only_for_generated_raw_math_calls() {
    let root = test_root("libm");
    let math_source = r#"package math_calls

import std.math

fn main() {
    let floor_value: f64 = math.floor(1.25)
    let ceil_value: f64 = math.ceil(1.25)
    let round_value: f64 = math.round(1.25)
    let sqrt_value: f64 = math.sqrt(4.0)
    let sin_value: f64 = math.sin(0.5)
    let cos_value: f64 = math.cos(0.5)
    let pow_value: f64 = math.pow(2.0, 3.0)
    let total: f64 = floor_value + ceil_value + round_value + sqrt_value + sin_value + cos_value + pow_value
    if total < 0.0 {
        panic("unreachable")
    } else {
        void
    }
}
"#;
    let math_project = create_project(&root, "math-calls", Some(math_source));
    resolve_dependencies(&math_project, false);
    let math_build = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("build")
        .arg(&math_project)
        .arg("--release")
        .output()
        .unwrap();
    assert_success(&math_build);
    let math_sidecar = read_json(&math_project.join("build").join("release-provenance.json"));
    assert!(
        math_sidecar["link_command"]["argv"]
            .as_array()
            .unwrap()
            .iter()
            .any(|argument| argument == "-lm")
    );

    let control_source = r#"package math_control

import std.math

fn main() {
    let absolute: i64 = math.abs(-5)
    let minimum: i64 = math.min(absolute, 10)
    let maximum: i64 = math.max(minimum, 20)
    if maximum < 0 {
        panic("unreachable")
    } else {
        void
    }
}
"#;
    let control_project = create_project(&root, "math-control", Some(control_source));
    resolve_dependencies(&control_project, false);
    let control_build = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("build")
        .arg(&control_project)
        .arg("--release")
        .output()
        .unwrap();
    assert_success(&control_build);
    let control_sidecar = read_json(
        &control_project
            .join("build")
            .join("release-provenance.json"),
    );
    assert!(
        !control_sidecar["link_command"]["argv"]
            .as_array()
            .unwrap()
            .iter()
            .any(|argument| argument == "-lm")
    );
    fs::remove_dir_all(root).unwrap();
}

#[cfg(target_os = "linux")]
#[test]
fn formal_benchmark_projects_retain_generic_libm_capability_results() {
    let root = test_root("formal-libm");
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let raw_math_calls = [
        "sqrt(", "sqrtf(", "pow(", "powf(", "fmod(", "fmodf(", "floor(", "floorf(", "ceil(",
        "ceilf(", "round(", "roundf(", "sin(", "sinf(", "cos(", "cosf(", "tan(", "tanf(", "exp(",
        "expf(", "log(", "logf(",
    ];
    for name in ["spectral-norm", "n-body", "fannkuch-redux"] {
        let source = repository
            .join("performance")
            .join("benchmarksgame")
            .join("reference")
            .join("nomo")
            .join(name);
        let project = root.join(name);
        fs::create_dir_all(project.join("src")).unwrap();
        fs::copy(source.join("nomo.toml"), project.join("nomo.toml")).unwrap();
        fs::copy(
            source.join("src").join("main.nomo"),
            project.join("src").join("main.nomo"),
        )
        .unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
            .arg("build")
            .arg(&project)
            .arg("--release")
            .output()
            .unwrap();
        assert_success(&output);
        let sidecar = read_json(&project.join("build").join("release-provenance.json"));
        let has_libm = sidecar["link_command"]["argv"]
            .as_array()
            .unwrap()
            .iter()
            .any(|argument| argument == "-lm");
        let generated_c =
            fs::read_to_string(project.join("build").join("c").join("main.c")).unwrap();
        let generated_c_has_raw_math_call =
            raw_math_calls.iter().any(|call| generated_c.contains(call));
        assert_eq!(has_libm, generated_c_has_raw_math_call, "{name}");
    }
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
        normalized_stdout(&release),
        "one\ntwo\nleft\nright\nnegative f64\ncleanup\nstop\n"
    );
    let sidecar = read_json(&project.join("build").join("release-provenance.json"));
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
        normalized_stdout(&release),
        "cleanup\ncleanup\ncleanup\nstop\n",
        "this regression records the pre-existing direct match scrutinee gap; it is not a semantic-conformance assertion"
    );
    fs::remove_dir_all(root).unwrap();
}
