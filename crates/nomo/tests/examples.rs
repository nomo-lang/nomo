use nomo::project::{build_project, check_project, discover_project};
use rcgen::{CertifiedKey, generate_simple_self_signed};
use rustls::pki_types::{PrivateKeyDer, PrivatePkcs8KeyDer};
use rustls::{ServerConfig, ServerConnection, StreamOwned};
use std::fs;
use std::io::{ErrorKind, Read, Write};
use std::net::{Shutdown, TcpListener};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

const REQUIRED_V0_1_EXAMPLES: &[&str] = &[
    "hello",
    "args",
    "read_file",
    "result_chain",
    "comments",
    "std_path",
    "std_process",
    "std_time",
    "std_json",
    "std_http",
    "openai_compatible",
    "concurrent_openai_compatible",
    "openai_streaming",
    "isolated_tasks",
    "suspend_ready",
    "async_yield",
    "async_call_abi",
    "async_timer",
    "async_tcp_connect",
    "async_tcp_io",
    "async_process_pipe_unix",
    "async_process_pipe_windows",
    "async_process_stress",
    "async_publication_move",
    "async_bounded_channel",
    "async_static_select",
    "async_structured_void",
    "async_structured_results",
    "async_structured_return",
    "async_structured_cancel",
    "async_structured_return_cancel",
    "async_structured_question_cancel",
    "async_structured_panic_cleanup",
    "async_structured_explicit_cancel",
    "async_structured_deadline",
    "mcp_stdio_blocking",
    "mcp_stdio_async",
    "nomo_test_basic",
    "nomo_doc_basic",
    "workspace_basic",
    "workspace_dependencies",
    "deps_git",
    "deps_vendor",
    "ffi_abs",
    "ffi_opaque",
    "ffi_puts",
    "ffi_typed_handle",
    "interface_display",
    "operators_arithmetic",
    "operators_logical",
    "operators_bitwise",
    "operators_assignment",
    "struct_methods",
    "array_basic",
    "mut_field_borrow",
];

#[test]
fn examples_check_and_run() {
    for example in example_projects() {
        clean_example_artifacts(&example);
        prepare_example(&example);
        clean_example_build_dirs(&example);
        if is_workspace_example(&example) {
            assert_workspace_example(&example);
            clean_example_artifacts(&example);
            continue;
        }

        assert_cli_check(&example);
        assert_cli_build_emit_c(&example);
        if cfg!(windows) && example_name(&example) == "async_process_pipe_unix" {
            clean_example_artifacts(&example);
            continue;
        }
        assert_cli_run(&example);
        assert_extra_cli_commands(&example);

        let project = discover_project(&example)
            .unwrap_or_else(|err| panic!("failed to discover {}: {err}", example.display()));
        check_project(&project).unwrap_or_else(|diag| {
            panic!("failed to check {}:\n{}", example.display(), diag.human())
        });
        let bin = build_project(&project, false)
            .unwrap_or_else(|err| panic!("failed to build {}: {err}", example.display()));
        let output = run_built_example(&project.root, &bin, &example);
        assert_eq!(
            output.status.code(),
            Some(expected_exit_code(&example)),
            "example exit status mismatch: {}\nstdout:\n{}\nstderr:\n{}",
            example.display(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert_example_output(&example, &output.stdout, &output.stderr);
        clean_example_artifacts(&example);
    }
}

#[test]
fn concurrent_openai_task_example_uses_two_local_tls_requests() {
    let example = workspace_root()
        .join("examples")
        .join("concurrent_openai_compatible");
    clean_example_artifacts(&example);
    let project = discover_project(&example).unwrap();
    check_project(&project).unwrap_or_else(|diagnostic| panic!("{}", diagnostic.human()));
    let bin = build_project(&project, false).unwrap();
    let output = run_built_example(&project.root, &bin, &example);
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_example_output(&example, &output.stdout, &output.stderr);
    let transcript = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!transcript.contains("local-task-token"), "{transcript}");
    clean_example_artifacts(&example);
}

#[test]
fn examples_tree_is_fmt_checked() {
    let examples_dir = workspace_root().join("examples");
    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("fmt")
        .arg("--check")
        .arg(&examples_dir)
        .output()
        .unwrap_or_else(|err| {
            panic!(
                "failed to run nomo fmt --check {}: {err}",
                examples_dir.display()
            )
        });

    assert!(
        output.status.success(),
        "nomo fmt --check failed for {}\nstdout:\n{}\nstderr:\n{}",
        examples_dir.display(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "all files already formatted\n"
    );
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn example_projects() -> Vec<PathBuf> {
    let examples_dir = workspace_root().join("examples");
    let mut examples = fs::read_dir(&examples_dir)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", examples_dir.display()))
        .filter_map(|entry| {
            let path = entry.ok()?.path();
            if path.join("nomo.toml").is_file() {
                Some(path)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    examples.sort();
    assert!(!examples.is_empty(), "no example projects found");
    for required in REQUIRED_V0_1_EXAMPLES {
        assert!(
            examples
                .iter()
                .any(|path| path.file_name().and_then(|name| name.to_str()) == Some(*required)),
            "missing required v0.1 example `{required}`"
        );
    }
    examples
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn assert_cli_check(example: &Path) {
    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("check")
        .arg(example)
        .output()
        .unwrap_or_else(|err| panic!("failed to run nomo check {}: {err}", example.display()));
    assert!(
        output.status.success(),
        "nomo check failed for {}\nstdout:\n{}\nstderr:\n{}",
        example.display(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn assert_cli_build_emit_c(example: &Path) {
    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("build")
        .arg(example)
        .arg("--emit-c")
        .output()
        .unwrap_or_else(|err| {
            panic!(
                "failed to run nomo build --emit-c {}: {err}",
                example.display()
            )
        });
    let c_path = example.join("build/c/main.c");
    let bin_path = example.join("build/bin").join(
        example
            .file_name()
            .unwrap_or_else(|| panic!("example path has no file name: {}", example.display())),
    );
    assert!(
        output.status.success(),
        "nomo build --emit-c failed for {}\nstdout:\n{}\nstderr:\n{}",
        example.display(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        format!("built {}\n", c_path.display())
    );
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(c_path.exists(), "missing generated C: {}", c_path.display());
    assert!(
        !bin_path.exists(),
        "--emit-c unexpectedly built native executable: {}",
        bin_path.display()
    );
}

fn assert_cli_run(example: &Path) {
    let output = match example_name(example) {
        "std_http" => run_with_http_example_server(|port| {
            Command::new(env!("CARGO_BIN_EXE_nomo"))
                .arg("run")
                .arg(example)
                .env("NOMO_EXAMPLE_ENV", "env get ok")
                .env("NOMO_HTTP_PORT", port.to_string())
                .output()
                .unwrap_or_else(|err| panic!("failed to run nomo run {}: {err}", example.display()))
        }),
        "openai_compatible" => run_with_openai_tls_server(|endpoint, ca_bundle| {
            Command::new(env!("CARGO_BIN_EXE_nomo"))
                .arg("run")
                .arg(example)
                .env("NOMO_OPENAI_BASE_URL", endpoint)
                .env("NOMO_OPENAI_API_KEY", "local-test-token")
                .env("NOMO_HTTP_CA_BUNDLE", ca_bundle)
                .output()
                .unwrap_or_else(|err| panic!("failed to run nomo run {}: {err}", example.display()))
        }),
        "concurrent_openai_compatible" => {
            run_with_concurrent_openai_tls_server(|endpoint, ca_bundle| {
                Command::new(env!("CARGO_BIN_EXE_nomo"))
                    .arg("run")
                    .arg(example)
                    .env("NOMO_OPENAI_BASE_URL", endpoint)
                    .env("NOMO_HTTP_CA_BUNDLE", ca_bundle)
                    .output()
                    .unwrap_or_else(|err| {
                        panic!("failed to run nomo run {}: {err}", example.display())
                    })
            })
        }
        "openai_streaming" => run_with_openai_streaming_tls_server(|endpoint, ca_bundle| {
            Command::new(env!("CARGO_BIN_EXE_nomo"))
                .arg("run")
                .arg(example)
                .env("NOMO_OPENAI_BASE_URL", endpoint)
                .env("NOMO_OPENAI_API_KEY", "local-streaming-token")
                .env("NOMO_HTTP_CA_BUNDLE", ca_bundle)
                .output()
                .unwrap_or_else(|err| panic!("failed to run nomo run {}: {err}", example.display()))
        }),
        "async_tcp_io" => run_with_tcp_echo_server(|port| {
            Command::new(env!("CARGO_BIN_EXE_nomo"))
                .arg("run")
                .arg(example)
                .env("NOMO_TCP_ECHO_PORT", port.to_string())
                .output()
                .unwrap_or_else(|err| panic!("failed to run nomo run {}: {err}", example.display()))
        }),
        "async_process_pipe_unix" | "async_process_pipe_windows" | "async_process_stress" => {
            run_with_async_process_fixture(|fixture| {
                Command::new(env!("CARGO_BIN_EXE_nomo"))
                    .arg("run")
                    .arg(example)
                    .env("NOMO_PROCESS_FIXTURE", fixture)
                    .output()
                    .unwrap_or_else(|err| {
                        panic!("failed to run nomo run {}: {err}", example.display())
                    })
            })
        }
        "mcp_stdio_async" => run_with_mcp_process_fixture(|fixture| {
            Command::new(env!("CARGO_BIN_EXE_nomo"))
                .arg("run")
                .arg(example)
                .env("NOMO_MCP_FIXTURE", fixture)
                .output()
                .unwrap_or_else(|err| panic!("failed to run nomo run {}: {err}", example.display()))
        }),
        _ => Command::new(env!("CARGO_BIN_EXE_nomo"))
            .arg("run")
            .arg(example)
            .env("NOMO_EXAMPLE_ENV", "env get ok")
            .output()
            .unwrap_or_else(|err| panic!("failed to run nomo run {}: {err}", example.display())),
    };
    if example_name(example) == "async_structured_panic_cleanup" {
        assert_eq!(output.status.code(), Some(1));
        assert_eq!(
            String::from_utf8_lossy(&output.stdout),
            "managed slow before\n"
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stderr),
            "panic: panic from child\nprogram exited with status 1\n"
        );
        return;
    }
    assert!(
        output.status.success(),
        "nomo run failed for {}",
        example.display()
    );
    assert_example_output(example, &output.stdout, &output.stderr);
}

fn assert_extra_cli_commands(example: &Path) {
    match example_name(example) {
        "nomo_test_basic" => assert_cli_test_basic(example),
        "nomo_doc_basic" => assert_cli_doc_basic(example),
        "deps_vendor" => assert_cli_deps_vendor(example),
        _ => {}
    }
}

fn assert_cli_test_basic(example: &Path) {
    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("test")
        .arg(example)
        .output()
        .unwrap_or_else(|err| panic!("failed to run nomo test {}: {err}", example.display()));
    assert!(
        output.status.success(),
        "nomo test failed for {}\nstdout:\n{}\nstderr:\n{}",
        example.display(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "running 1 tests\nok app.main.adds_numbers\n"
    );
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn assert_cli_doc_basic(example: &Path) {
    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("doc")
        .arg("--json")
        .arg(example)
        .output()
        .unwrap_or_else(|err| panic!("failed to run nomo doc --json {}: {err}", example.display()));
    assert!(
        output.status.success(),
        "nomo doc --json failed for {}\nstdout:\n{}\nstderr:\n{}",
        example.display(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("\"package\":\"local/nomo_doc_basic\""),
        "{stdout}"
    );
    assert!(
        stdout.contains("\"docs\":\"Basic documentation generation example.\""),
        "{stdout}"
    );
    assert!(stdout.contains("\"name\":\"greet\""), "{stdout}");
    assert!(
        stdout.contains("\"docs\":\"Greets a caller by name.\""),
        "{stdout}"
    );
    assert!(stdout.contains("\"name\":\"User\""), "{stdout}");
    assert!(
        stdout.contains("User-facing record documented from a block doc comment."),
        "{stdout}"
    );
    assert!(stdout.contains("\"kind\":\"field\""), "{stdout}");
    assert!(stdout.contains("\"name\":\"User.name\""), "{stdout}");
    assert!(
        stdout.contains("\"docs\":\"User display name.\""),
        "{stdout}"
    );
}

fn assert_cli_deps_vendor(example: &Path) {
    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("deps")
        .arg("vendor")
        .arg(example)
        .arg("--sync")
        .output()
        .unwrap_or_else(|err| {
            panic!(
                "failed to run nomo deps vendor --sync {}: {err}",
                example.display()
            )
        });
    assert!(
        output.status.success(),
        "nomo deps vendor --sync failed for {}\nstdout:\n{}\nstderr:\n{}",
        example.display(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        format!("vendored {}\n", example.join("vendor").display())
    );
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let lockfile = example.join("nomo.lock");
    assert!(
        lockfile.exists(),
        "missing lockfile: {}",
        lockfile.display()
    );

    let vendor_manifest_path = example.join("vendor/nomo-vendor.toml");
    let vendor_manifest = fs::read_to_string(&vendor_manifest_path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", vendor_manifest_path.display()));
    assert!(
        vendor_manifest.contains("source = \"path+fixtures/utils\""),
        "{vendor_manifest}"
    );
    assert!(
        vendor_manifest.contains("source = \"git+fixtures/remote_label\""),
        "{vendor_manifest}"
    );
    assert!(
        example
            .join("vendor/examples/utils/path/nomo.toml")
            .exists()
    );
    assert!(
        fs::read_dir(example.join("vendor/examples/remote-label"))
            .unwrap()
            .any(|entry| entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with("git-"))
    );

    let clean_output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("deps")
        .arg("clean-cache")
        .arg(example)
        .output()
        .unwrap_or_else(|err| {
            panic!(
                "failed to run nomo deps clean-cache {}: {err}",
                example.display()
            )
        });
    assert!(
        clean_output.status.success(),
        "nomo deps clean-cache failed for {}\nstdout:\n{}\nstderr:\n{}",
        example.display(),
        String::from_utf8_lossy(&clean_output.stdout),
        String::from_utf8_lossy(&clean_output.stderr)
    );

    let offline_output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("build")
        .arg(example)
        .arg("--offline")
        .arg("--emit-c")
        .output()
        .unwrap_or_else(|err| {
            panic!(
                "failed to run nomo build --offline --emit-c {}: {err}",
                example.display()
            )
        });
    assert!(
        offline_output.status.success(),
        "nomo build --offline --emit-c failed for {}\nstdout:\n{}\nstderr:\n{}",
        example.display(),
        String::from_utf8_lossy(&offline_output.stdout),
        String::from_utf8_lossy(&offline_output.stderr)
    );
    assert!(
        offline_output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&offline_output.stderr)
    );
}

fn assert_workspace_example(example: &Path) {
    assert_cli_workspace_check(example);
    assert_cli_workspace_build_emit_c(example);
    match example_name(example) {
        "workspace_basic" => {
            assert_cli_workspace_test_basic(example);
            assert_cli_workspace_doc_basic(example);
            assert_cli_workspace_deps_tree(
                example,
                &[
                    "examples/cli 0.1.0",
                    "examples/core 0.1.0",
                    "+-- core -> examples/core",
                ],
            );
        }
        "workspace_dependencies" => {
            assert_cli_workspace_deps_tree(
                example,
                &[
                    "examples/app 0.1.0",
                    "examples/util 0.1.0",
                    "+-- util -> examples/util",
                ],
            );
        }
        name => panic!("missing workspace example assertions for `{name}`"),
    }
}

fn assert_cli_workspace_check(example: &Path) {
    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("check")
        .arg("--workspace")
        .arg(example)
        .output()
        .unwrap_or_else(|err| {
            panic!(
                "failed to run nomo check --workspace {}: {err}",
                example.display()
            )
        });
    assert!(
        output.status.success(),
        "nomo check --workspace failed for {}\nstdout:\n{}\nstderr:\n{}",
        example.display(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    for member in workspace_members(example) {
        let main = member.join("src/main.nomo");
        assert!(
            stdout.contains(&format!("checked {}\n", main.display())),
            "{stdout}"
        );
    }
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn assert_cli_workspace_build_emit_c(example: &Path) {
    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("build")
        .arg("--workspace")
        .arg("--emit-c")
        .arg(example)
        .output()
        .unwrap_or_else(|err| {
            panic!(
                "failed to run nomo build --workspace --emit-c {}: {err}",
                example.display()
            )
        });
    assert!(
        output.status.success(),
        "nomo build --workspace --emit-c failed for {}\nstdout:\n{}\nstderr:\n{}",
        example.display(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    for member in workspace_members(example) {
        let c_path = member.join("build/c/main.c");
        assert!(
            stdout.contains(&format!("built {}\n", c_path.display())),
            "{stdout}"
        );
        assert!(c_path.exists(), "missing generated C: {}", c_path.display());
    }
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn assert_cli_workspace_test_basic(example: &Path) {
    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("test")
        .arg("--workspace")
        .arg(example)
        .output()
        .unwrap_or_else(|err| {
            panic!(
                "failed to run nomo test --workspace {}: {err}",
                example.display()
            )
        });
    assert!(
        output.status.success(),
        "nomo test --workspace failed for {}\nstdout:\n{}\nstderr:\n{}",
        example.display(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("running 2 tests\n"), "{stdout}");
    assert!(
        stdout.contains("ok app.main.cli_uses_core_math\n"),
        "{stdout}"
    );
    assert!(
        stdout.contains("ok core.main.core_adds_numbers\n"),
        "{stdout}"
    );
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn assert_cli_workspace_doc_basic(example: &Path) {
    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("doc")
        .arg("--workspace")
        .arg("--json")
        .arg(example)
        .output()
        .unwrap_or_else(|err| {
            panic!(
                "failed to run nomo doc --workspace --json {}: {err}",
                example.display()
            )
        });
    assert!(
        output.status.success(),
        "nomo doc --workspace --json failed for {}\nstdout:\n{}\nstderr:\n{}",
        example.display(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"package\":\"examples/cli\""), "{stdout}");
    assert!(stdout.contains("\"name\":\"run_cli\""), "{stdout}");
    assert!(
        stdout.contains("Runs the workspace CLI example."),
        "{stdout}"
    );
    assert!(stdout.contains("\"package\":\"examples/core\""), "{stdout}");
    assert!(stdout.contains("\"name\":\"add\""), "{stdout}");
    assert!(
        stdout.contains("Adds two numbers from the core package."),
        "{stdout}"
    );
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn assert_cli_workspace_deps_tree(example: &Path, expected: &[&str]) {
    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("deps")
        .arg("tree")
        .arg("--workspace")
        .arg(example)
        .output()
        .unwrap_or_else(|err| {
            panic!(
                "failed to run nomo deps tree --workspace {}: {err}",
                example.display()
            )
        });
    assert!(
        output.status.success(),
        "nomo deps tree --workspace failed for {}\nstdout:\n{}\nstderr:\n{}",
        example.display(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    for item in expected {
        assert!(stdout.contains(item), "{stdout}");
    }
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn run_built_example(project_root: &Path, bin: &Path, example: &Path) -> Output {
    match example_name(example) {
        "std_http" => run_with_http_example_server(|port| {
            Command::new(bin)
                .current_dir(project_root)
                .env("NOMO_EXAMPLE_ENV", "env get ok")
                .env("NOMO_HTTP_PORT", port.to_string())
                .output()
                .unwrap_or_else(|err| panic!("failed to run {}: {err}", bin.display()))
        }),
        "openai_compatible" => run_with_openai_tls_server(|endpoint, ca_bundle| {
            Command::new(bin)
                .current_dir(project_root)
                .env("NOMO_OPENAI_BASE_URL", endpoint)
                .env("NOMO_OPENAI_API_KEY", "local-test-token")
                .env("NOMO_HTTP_CA_BUNDLE", ca_bundle)
                .output()
                .unwrap_or_else(|err| panic!("failed to run {}: {err}", bin.display()))
        }),
        "concurrent_openai_compatible" => {
            run_with_concurrent_openai_tls_server(|endpoint, ca_bundle| {
                Command::new(bin)
                    .current_dir(project_root)
                    .env("NOMO_OPENAI_BASE_URL", endpoint)
                    .env("NOMO_HTTP_CA_BUNDLE", ca_bundle)
                    .output()
                    .unwrap_or_else(|err| panic!("failed to run {}: {err}", bin.display()))
            })
        }
        "openai_streaming" => run_with_openai_streaming_tls_server(|endpoint, ca_bundle| {
            Command::new(bin)
                .current_dir(project_root)
                .env("NOMO_OPENAI_BASE_URL", endpoint)
                .env("NOMO_OPENAI_API_KEY", "local-streaming-token")
                .env("NOMO_HTTP_CA_BUNDLE", ca_bundle)
                .output()
                .unwrap_or_else(|err| panic!("failed to run {}: {err}", bin.display()))
        }),
        "async_tcp_io" => run_with_tcp_echo_server(|port| {
            Command::new(bin)
                .current_dir(project_root)
                .env("NOMO_TCP_ECHO_PORT", port.to_string())
                .output()
                .unwrap_or_else(|err| panic!("failed to run {}: {err}", bin.display()))
        }),
        "async_process_pipe_unix" | "async_process_pipe_windows" | "async_process_stress" => {
            run_with_async_process_fixture(|fixture| {
                Command::new(bin)
                    .current_dir(project_root)
                    .env("NOMO_PROCESS_FIXTURE", fixture)
                    .output()
                    .unwrap_or_else(|err| panic!("failed to run {}: {err}", bin.display()))
            })
        }
        "mcp_stdio_async" => run_with_mcp_process_fixture(|fixture| {
            Command::new(bin)
                .current_dir(project_root)
                .env("NOMO_MCP_FIXTURE", fixture)
                .output()
                .unwrap_or_else(|err| panic!("failed to run {}: {err}", bin.display()))
        }),
        _ => Command::new(bin)
            .current_dir(project_root)
            .env("NOMO_EXAMPLE_ENV", "env get ok")
            .output()
            .unwrap_or_else(|err| panic!("failed to run {}: {err}", bin.display())),
    }
}

fn run_with_async_process_fixture<F>(run: F) -> Output
where
    F: FnOnce(&Path) -> Output,
{
    let root = std::env::temp_dir().join(format!(
        "nomo-async-process-example-fixture-{}",
        std::process::id()
    ));
    if root.exists() {
        fs::remove_dir_all(&root).unwrap();
    }
    fs::create_dir_all(&root).unwrap();
    let source = root.join("fixture.c");
    let binary = root.join(if cfg!(windows) {
        "fixture.exe"
    } else {
        "fixture"
    });
    let fixture_source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../performance/async/fixtures/process_pipe/fixture.c");
    fs::copy(&fixture_source, &source).unwrap_or_else(|error| {
        panic!(
            "failed to copy {} into the async process fixture: {error}",
            fixture_source.display()
        )
    });
    let compiled = Command::new(if cfg!(windows) { "clang" } else { "cc" })
        .arg("-std=c99")
        .arg(&source)
        .arg("-o")
        .arg(&binary)
        .output()
        .unwrap();
    assert!(
        compiled.status.success(),
        "failed to build async process example fixture\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compiled.stdout),
        String::from_utf8_lossy(&compiled.stderr)
    );
    let output = run(&binary);
    fs::remove_dir_all(root).unwrap();
    output
}

fn run_with_mcp_process_fixture<F>(run: F) -> Output
where
    F: FnOnce(&Path) -> Output,
{
    let root = std::env::temp_dir().join(format!(
        "nomo-mcp-process-example-fixture-{}",
        std::process::id()
    ));
    if root.exists() {
        fs::remove_dir_all(&root).unwrap();
    }
    fs::create_dir_all(&root).unwrap();
    let source = root.join("fixture.c");
    let binary = root.join(if cfg!(windows) {
        "fixture.exe"
    } else {
        "fixture"
    });
    fs::write(
        &source,
        r#"#define _POSIX_C_SOURCE 200809L
#include <stdio.h>
#include <string.h>
#ifdef _WIN32
#include <windows.h>
static void nomo_fixture_sleep(unsigned long millis) { Sleep((DWORD)millis); }
#else
#include <time.h>
static void nomo_fixture_sleep(unsigned long millis) {
    struct timespec duration;
    duration.tv_sec = (time_t)(millis / 1000UL);
    duration.tv_nsec = (long)(millis % 1000UL) * 1000000L;
    while (nanosleep(&duration, &duration) != 0) {}
}
#endif

int main(int argc, char **argv) {
    if (argc < 2 || strcmp(argv[1], "mcp") != 0) {
        return 2;
    }
    char line[65536];
    if (fgets(line, sizeof(line), stdin) == NULL) {
        return 3;
    }
    fputs("{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"protocolVersion\":\"2025-06-18\",", stdout);
    fflush(stdout);
    nomo_fixture_sleep(50UL);
    fputs("\"capabilities\":{},\"serverInfo\":{\"name\":\"fixture\",\"version\":\"1\"}}}\n", stdout);
    fflush(stdout);
    if (fgets(line, sizeof(line), stdin) == NULL) {
        return 4;
    }
    fputs(
        "{\"jsonrpc\":\"2.0\",\"method\":\"notifications/progress\",\"params\":{\"step\":1}}\n"
        "{\"jsonrpc\":\"2.0\",\"id\":2,\"result\":{\"tools\":[]}}\n",
        stdout
    );
    fflush(stdout);
    nomo_fixture_sleep(100UL);
    return 0;
}
"#,
    )
    .unwrap();
    let compiled = Command::new(if cfg!(windows) { "clang" } else { "cc" })
        .arg("-std=c99")
        .arg(&source)
        .arg("-o")
        .arg(&binary)
        .output()
        .unwrap();
    assert!(
        compiled.status.success(),
        "failed to build MCP process example fixture\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compiled.stdout),
        String::from_utf8_lossy(&compiled.stderr)
    );
    let output = run(&binary);
    fs::remove_dir_all(root).unwrap();
    output
}

fn run_with_http_example_server<F>(run: F) -> Output
where
    F: FnOnce(u16) -> Output,
{
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = std::thread::spawn(move || {
        let started = Instant::now();
        let mut handled = 0;
        while handled < 2 {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    stream
                        .set_read_timeout(Some(Duration::from_secs(2)))
                        .unwrap();
                    let request = read_http_request(&mut stream);
                    let body_start = request.find("\r\n\r\n").map(|index| index + 4).unwrap();
                    let body = &request[body_start..];
                    let (expected_line, expected_body, response_body) = if handled == 0 {
                        ("GET /hello HTTP/1.1", "", "get-ok")
                    } else {
                        ("POST /echo HTTP/1.1", "post-body", "post-ok")
                    };
                    assert!(
                        request.starts_with(expected_line),
                        "request was:\n{request}"
                    );
                    assert_eq!(body, expected_body);
                    let response = format!(
                        "HTTP/1.0 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        response_body.len(),
                        response_body
                    );
                    stream.write_all(response.as_bytes()).unwrap();
                    handled += 1;
                }
                Err(err)
                    if err.kind() == ErrorKind::WouldBlock
                        && started.elapsed() < Duration::from_secs(10) =>
                {
                    std::thread::sleep(Duration::from_millis(25));
                }
                Err(err) => panic!("failed to accept HTTP client connection: {err}"),
            }
        }
    });
    let output = run(port);
    server.join().unwrap();
    output
}

fn run_with_tcp_echo_server<F>(run: F) -> Output
where
    F: FnOnce(u16) -> Output,
{
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = std::thread::spawn(move || {
        let started = Instant::now();
        let mut stream = loop {
            match listener.accept() {
                Ok((stream, _)) => break stream,
                Err(err)
                    if err.kind() == ErrorKind::WouldBlock
                        && started.elapsed() < Duration::from_secs(10) =>
                {
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(err) => panic!("failed to accept TCP echo connection: {err}"),
            }
        };
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        stream
            .set_write_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut payload = [0_u8; 15];
        stream.read_exact(&mut payload).unwrap();
        assert_eq!(&payload, b"hello from Nomo");
        stream.write_all(&payload).unwrap();
        stream.flush().unwrap();
        stream.shutdown(Shutdown::Write).unwrap();
    });
    let output = run(port);
    server.join().unwrap();
    output
}

fn read_http_request(stream: &mut impl Read) -> String {
    let mut request = Vec::new();
    let mut buffer = [0_u8; 512];
    loop {
        let read = stream.read(&mut buffer).unwrap();
        if read == 0 {
            break;
        }
        request.extend_from_slice(&buffer[..read]);
        let text = String::from_utf8_lossy(&request);
        let header_end = text.find("\r\n\r\n");
        if let Some(header_end) = header_end {
            let content_length = text
                .lines()
                .find_map(|line| {
                    line.strip_prefix("Content-Length: ")
                        .and_then(|value| value.parse::<usize>().ok())
                })
                .unwrap_or(0);
            if request.len() >= header_end + 4 + content_length {
                break;
            }
        }
    }
    String::from_utf8(request).unwrap()
}

fn run_with_openai_tls_server<F>(run: F) -> Output
where
    F: FnOnce(&str, &Path) -> Output,
{
    let _ = rustls::crypto::ring::default_provider().install_default();
    let CertifiedKey { cert, signing_key } =
        generate_simple_self_signed(vec!["localhost".to_string()]).unwrap();
    let server_config = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(
            vec![cert.der().clone()],
            PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(signing_key.serialize_der())),
        )
        .unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = std::thread::spawn(move || {
        let started = Instant::now();
        let stream = loop {
            match listener.accept() {
                Ok((stream, _)) => break stream,
                Err(err)
                    if err.kind() == ErrorKind::WouldBlock
                        && started.elapsed() < Duration::from_secs(10) =>
                {
                    std::thread::sleep(Duration::from_millis(25));
                }
                Err(err) => panic!("failed to accept HTTPS client connection: {err}"),
            }
        };
        stream.set_nonblocking(false).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        stream
            .set_write_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let connection = ServerConnection::new(Arc::new(server_config)).unwrap();
        let mut stream = StreamOwned::new(connection, stream);
        let request = read_http_request(&mut stream);
        assert!(
            request.starts_with("POST /v1/chat/completions HTTP/1.1\r\n"),
            "request was:\n{request}"
        );
        let lower = request.to_ascii_lowercase();
        assert!(
            lower.contains("authorization: bearer local-test-token\r\n"),
            "request was:\n{request}"
        );
        assert!(
            lower.contains("content-type: application/json\r\n"),
            "request was:\n{request}"
        );
        let body_start = request.find("\r\n\r\n").map(|index| index + 4).unwrap();
        assert_eq!(
            &request[body_start..],
            "{\"model\":\"nomo-fixture\",\"messages\":[{\"role\":\"user\",\"content\":\"Hello from Nomo\"}],\"stream\":false}"
        );
        let response_body = "{\"id\":\"chatcmpl-local\",\"choices\":[{\"message\":{\"role\":\"assistant\",\"content\":\"fixture-ok\"}}]}";
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nX-Nomo-Fixture: tls\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            response_body.len(),
            response_body
        );
        stream.write_all(response.as_bytes()).unwrap();
        stream.flush().unwrap();
    });
    let ca_bundle =
        std::env::temp_dir().join(format!("nomo-openai-ca-{}-{port}.pem", std::process::id()));
    fs::write(&ca_bundle, cert.pem()).unwrap();
    let endpoint = format!("https://localhost:{port}");
    let output = run(&endpoint, &ca_bundle);
    server.join().unwrap();
    fs::remove_file(ca_bundle).unwrap();
    output
}

fn run_with_concurrent_openai_tls_server<F>(run: F) -> Output
where
    F: FnOnce(&str, &Path) -> Output,
{
    let _ = rustls::crypto::ring::default_provider().install_default();
    let CertifiedKey { cert, signing_key } =
        generate_simple_self_signed(vec!["localhost".to_string()]).unwrap();
    let server_config = Arc::new(
        ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(
                vec![cert.der().clone()],
                PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(signing_key.serialize_der())),
            )
            .unwrap(),
    );
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = std::thread::spawn(move || {
        let started = Instant::now();
        let rendezvous = Arc::new((Mutex::new(0_usize), Condvar::new()));
        let mut handlers = Vec::new();
        while handlers.len() < 2 {
            let (stream, _) = match listener.accept() {
                Ok(connection) => connection,
                Err(err)
                    if err.kind() == ErrorKind::WouldBlock
                        && started.elapsed() < Duration::from_secs(10) =>
                {
                    std::thread::sleep(Duration::from_millis(10));
                    continue;
                }
                Err(err) => panic!("failed to accept concurrent HTTPS connection: {err}"),
            };
            let server_config = server_config.clone();
            let rendezvous = rendezvous.clone();
            handlers.push(std::thread::spawn(move || {
                stream.set_nonblocking(false).unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                stream
                    .set_write_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let connection = ServerConnection::new(server_config).unwrap();
                let mut stream = StreamOwned::new(connection, stream);
                let request = read_http_request(&mut stream);
                assert!(
                    request.starts_with("POST /v1/chat/completions HTTP/1.1\r\n"),
                    "request was:\n{request}"
                );
                let lower = request.to_ascii_lowercase();
                assert!(
                    lower.contains("authorization: bearer local-task-token\r\n"),
                    "request was:\n{request}"
                );
                assert!(
                    lower.contains("content-type: application/json\r\n"),
                    "request was:\n{request}"
                );
                let body_start = request.find("\r\n\r\n").map(|index| index + 4).unwrap();
                assert_eq!(
                    &request[body_start..],
                    "{\"model\":\"nomo-task-fixture\",\"messages\":[{\"role\":\"user\",\"content\":\"Hello concurrently\"}],\"stream\":false}"
                );

                let (lock, condition) = &*rendezvous;
                let mut ready = lock.lock().unwrap();
                *ready += 1;
                condition.notify_all();
                let (ready, timeout) = condition
                    .wait_timeout_while(ready, Duration::from_secs(5), |count| *count < 2)
                    .unwrap();
                assert!(
                    !timeout.timed_out() && *ready == 2,
                    "two task HTTP requests did not overlap"
                );
                drop(ready);

                let response_body = "{\"id\":\"chatcmpl-task-local\",\"choices\":[{\"message\":{\"role\":\"assistant\",\"content\":\"concurrent-ok\"}}]}";
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    response_body.len(),
                    response_body
                );
                stream.write_all(response.as_bytes()).unwrap();
                stream.flush().unwrap();
            }));
        }
        for handler in handlers {
            handler.join().unwrap();
        }
    });
    let ca_bundle = std::env::temp_dir().join(format!(
        "nomo-concurrent-openai-ca-{}-{port}.pem",
        std::process::id()
    ));
    fs::write(&ca_bundle, cert.pem()).unwrap();
    let endpoint = format!("https://localhost:{port}");
    let output = run(&endpoint, &ca_bundle);
    server.join().unwrap();
    fs::remove_file(ca_bundle).unwrap();
    output
}

fn run_with_openai_streaming_tls_server<F>(run: F) -> Output
where
    F: FnOnce(&str, &Path) -> Output,
{
    let _ = rustls::crypto::ring::default_provider().install_default();
    let CertifiedKey { cert, signing_key } =
        generate_simple_self_signed(vec!["localhost".to_string()]).unwrap();
    let server_config = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(
            vec![cert.der().clone()],
            PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(signing_key.serialize_der())),
        )
        .unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = std::thread::spawn(move || {
        let started = Instant::now();
        let stream = loop {
            match listener.accept() {
                Ok((stream, _)) => break stream,
                Err(err)
                    if err.kind() == ErrorKind::WouldBlock
                        && started.elapsed() < Duration::from_secs(10) =>
                {
                    std::thread::sleep(Duration::from_millis(25));
                }
                Err(err) => panic!("failed to accept streaming HTTPS client connection: {err}"),
            }
        };
        stream.set_nonblocking(false).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        stream
            .set_write_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let connection = ServerConnection::new(Arc::new(server_config)).unwrap();
        let mut stream = StreamOwned::new(connection, stream);
        let request = read_http_request(&mut stream);
        assert!(
            request.starts_with("POST /v1/chat/completions HTTP/1.1\r\n"),
            "request was:\n{request}"
        );
        let lower = request.to_ascii_lowercase();
        assert!(
            lower.contains("authorization: bearer local-streaming-token\r\n"),
            "request was:\n{request}"
        );
        assert!(
            lower.contains("content-type: application/json\r\n"),
            "request was:\n{request}"
        );
        assert!(
            lower.contains("accept: text/event-stream\r\n"),
            "request was:\n{request}"
        );
        let body_start = request.find("\r\n\r\n").map(|index| index + 4).unwrap();
        assert_eq!(
            &request[body_start..],
            "{\"model\":\"nomo-fixture\",\"messages\":[{\"role\":\"user\",\"content\":\"Stream from Nomo\"}],\"stream\":true}"
        );
        stream
            .write_all(
                b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nX-Nomo-Fixture: tls-stream\r\nConnection: close\r\n\r\n",
            )
            .unwrap();
        stream.flush().unwrap();
        for chunk in [
            "event: token\r\n",
            "data: {\"choices\":[{\"delta\":{\"content\":\"Hel\"}}]}\r\n\r\n",
            "event: token\n",
            "data: {\"choices\":[{\"delta\":{\"content\":\"lo\"}}]}\n\n",
            "data: [DONE]\r\n\r\n",
        ] {
            stream.write_all(chunk.as_bytes()).unwrap();
            stream.flush().unwrap();
            std::thread::sleep(Duration::from_millis(10));
        }
    });
    let ca_bundle = std::env::temp_dir().join(format!(
        "nomo-openai-streaming-ca-{}-{port}.pem",
        std::process::id()
    ));
    fs::write(&ca_bundle, cert.pem()).unwrap();
    let endpoint = format!("https://localhost:{port}");
    let output = run(&endpoint, &ca_bundle);
    server.join().unwrap();
    fs::remove_file(ca_bundle).unwrap();
    output
}

fn example_name(example: &Path) -> &str {
    example
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_else(|| panic!("example path has no file name: {}", example.display()))
}

fn prepare_example(example: &Path) {
    match example_name(example) {
        "deps_git" => init_example_git_fixture(&example.join("fixtures/calc")),
        "deps_vendor" => init_example_git_fixture(&example.join("fixtures/remote_label")),
        _ => {}
    }
}

fn init_example_git_fixture(path: &Path) {
    let git_dir = path.join(".git");
    if git_dir.exists() {
        fs::remove_dir_all(&git_dir)
            .unwrap_or_else(|err| panic!("failed to remove {}: {err}", git_dir.display()));
    }
    run_git(path, &["init", "--quiet"]);
    run_git(path, &["config", "user.email", "nomo@example.invalid"]);
    run_git(path, &["config", "user.name", "Nomo Example"]);
    run_git(path, &["add", "nomo.toml", "src"]);
    run_git(path, &["commit", "--quiet", "-m", "initial"]);
}

fn run_git(path: &Path, args: &[&str]) {
    let output = Command::new("git")
        .arg("-C")
        .arg(path)
        .args(args)
        .output()
        .unwrap_or_else(|err| panic!("failed to run git in {}: {err}", path.display()));
    assert!(
        output.status.success(),
        "git -C {} {} failed\nstdout:\n{}\nstderr:\n{}",
        path.display(),
        args.join(" "),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn is_workspace_example(example: &Path) -> bool {
    matches!(
        example_name(example),
        "workspace_basic" | "workspace_dependencies"
    )
}

fn workspace_members(example: &Path) -> Vec<PathBuf> {
    match example_name(example) {
        "workspace_basic" => vec![example.join("apps/cli"), example.join("packages/core")],
        "workspace_dependencies" => vec![example.join("apps/app"), example.join("packages/util")],
        name => panic!("missing workspace member list for `{name}`"),
    }
}

fn assert_example_output(example: &Path, stdout: &[u8], stderr: &[u8]) {
    let stdout = String::from_utf8_lossy(stdout);
    let stderr = String::from_utf8_lossy(stderr);
    let Some(name) = example.file_name().and_then(|name| name.to_str()) else {
        panic!("example path has no file name: {}", example.display());
    };
    let expected = expected_stdout(name)
        .unwrap_or_else(|| panic!("missing expected stdout for example `{name}`"));
    let expected_stderr = expected_stderr(name);
    assert_eq!(
        stdout,
        expected,
        "example stdout mismatch: {}\nstderr:\n{}",
        example.display(),
        stderr
    );
    assert_eq!(
        stderr,
        expected_stderr,
        "example stderr mismatch: {}\nstdout:\n{}",
        example.display(),
        stdout
    );
    assert!(
        !stdout.contains("wrong"),
        "example printed failure sentinel: {}\nstdout:\n{}\nstderr:\n{}",
        example.display(),
        stdout,
        stderr
    );
}

fn clean_example_build_dirs(example: &Path) {
    if !example.exists() {
        return;
    }
    clean_build_dirs_recursive(example);
}

fn clean_example_artifacts(example: &Path) {
    clean_example_build_dirs(example);
    for path in [
        example.join("nomo.lock"),
        example.join("vendor"),
        example.join(".nomo"),
        example.join("fixtures/calc/.git"),
        example.join("fixtures/remote_label/.git"),
    ] {
        if path.is_dir() {
            fs::remove_dir_all(&path)
                .unwrap_or_else(|err| panic!("failed to clean {}: {err}", path.display()));
        } else if path.is_file() {
            fs::remove_file(&path)
                .unwrap_or_else(|err| panic!("failed to clean {}: {err}", path.display()));
        }
    }
}

fn clean_build_dirs_recursive(dir: &Path) {
    for entry in
        fs::read_dir(dir).unwrap_or_else(|err| panic!("failed to read {}: {err}", dir.display()))
    {
        let path = entry
            .unwrap_or_else(|err| panic!("failed to read entry in {}: {err}", dir.display()))
            .path();
        if !path.is_dir() {
            continue;
        }
        if path.file_name().and_then(|name| name.to_str()) == Some("build") {
            fs::remove_dir_all(&path)
                .unwrap_or_else(|err| panic!("failed to clean {}: {err}", path.display()));
        } else {
            clean_build_dirs_recursive(&path);
        }
    }
}

fn expected_stdout(example: &str) -> Option<&'static str> {
    Some(match example {
        "args" => "missing\n",
        "array_basic" => "array ok\n",
        "array_enum" => "enum array ok\n",
        "array_get_none" => "array get none ok\n",
        "array_nested" => "nested array ok\n",
        "array_option_lifecycle" => "array option lifecycle ok\n",
        "array_param_return" => "array param return ok\n",
        "array_reassign" => "array reassign ok\n",
        "array_struct" => "struct array ok\n",
        "array_swap" => "array swap ok\n",
        "array_value_semantics" => "array value semantics ok\n",
        "collection_literals" => "1\n7\n2\n0\n",
        "c_keywords" => "c keywords ok\n",
        "comments" => "comments ok\nhttp://example.test/*literal*/\n",
        "const" => "hello\nhello\nhello\nconst primitives ok\n",
        "cron_schedule" => "next=900000\n",
        "defer" => {
            "working\ncontinue cleanup\nbreak cleanup\nblock\ninner\nafter block\ninner early\nouter early\nclose\nflush\nlog\n"
        }
        "defer_question" => "defer before ? error\nfail\n",
        "deps_git" => "deps git ok\n",
        "deps_vendor" => "deps vendor ok\n",
        "enum_struct_payload" => "a@nomo.dev\n",
        "env_extended" => "set ok\ncwd ok\nhome ok\ntemp ok\n",
        "env_get" => "env get ok\n",
        "ffi_abs" => "ffi abs ok\n",
        "ffi_opaque" => "ffi opaque ok\n",
        "ffi_puts" => "ffi puts ok\n",
        "ffi_typed_handle" => "ffi typed handle ok\n",
        "file_handle" => "file handle ok\n",
        "generic_function" => "generic function ok\n",
        "generic_enum" => "generic enum ok\n",
        "generic_struct" => "generic struct ok\n",
        "generic_map" => "search search\npresent\nempty\n",
        "hello" => "Hello, Nomo\n",
        "if_let" => "if let ok\n",
        "io_print" => "stdout ok\n",
        "io_stderr" => "stdout ok\n",
        "interface_display" => "interface display ok\n",
        "isolated_tasks" => {
            "completed:alpha\ncompleted:beta\nrejoin completed:alpha\njoin-limit invalid_argument\nbusy-close busy\ntimeout:pending\ncancelled:cooperative\nclosed-handle closed\nbefore-copy\nlive-limit limit\ninput-limit limit\noutput-limit limit\n"
        }
        "suspend_ready" => "suspend ready\n",
        "async_yield" => "before yield\nframe-owned message\n3\n",
        "async_call_abi" => "argument\n7\nframe result\n",
        "async_timer" => "before timer\ntrue\ntrue\nafter timer\n",
        "async_tcp_connect" => "hostname resolution failed\n",
        "async_tcp_io" => "hello from Nomo\n",
        "async_structured_void" => {
            "left before\nright before\nleft after\nright after\ntrue true\n"
        }
        "async_publication_move" => "publication\n",
        "async_bounded_channel" => "first\nsecond\n",
        "async_static_select" => "winner\n",
        "async_structured_results" => {
            "left before\nright before\nleft after\nright after\nleft right\n"
        }
        "async_structured_return" => "gathered before\ngathered after\ngathered\n",
        "async_structured_cancel" => "slow before\ngate\nscope closed\n",
        "async_structured_return_cancel" => "managed before\ngate\nreturn evaluated\nmanaged\n",
        "async_structured_question_cancel" => "managed before\ngate\ntrue\n",
        "async_structured_panic_cleanup" => "managed slow before\n",
        "async_structured_explicit_cancel" => "managed before\ncancelled true\n",
        "async_structured_deadline" => "deadline elapsed true\n",
        "sqlite_agent_memory" => "usage: sqlite_agent_memory <write|read> <database-path>\n",
        "sqlite_memory" => {
            "inserted 1 1\nbusy-close busy_handle\nvalue hello from Nomo SQLite\nquery-done\nsqlite-ok\n"
        }
        "let_else" => "let else ok\n",
        "loops" => "counted\ncounted\ncounted\na\nb\nonce\n",
        "mcp_stdio_blocking" => "set NOMO_MCP_FIXTURE to an MCP stdio server executable\n",
        "mcp_stdio_async" => {
            "response 1 success\nserver notification\nresponse 2 success\nMCP stdio async exchange complete\n"
        }
        "mut_field_borrow" => "mut field borrow ok\n",
        "mut_methods" => "mut method ok\n",
        "newline_dot" => "newline dot ok\n",
        "nomo_doc_basic" => "nomo doc basic ok\n",
        "nomo_test_basic" => "nomo test basic ok\n",
        "option_helpers" => "predicates\nseed\nfallback\nseed!\nseed ok\n",
        "option_question" => "option ? ok\n",
        "option_result_lang_items" => "lang items ok\n",
        "operators_arithmetic" => "35\n-35\n",
        "operators_assignment" => "0\n",
        "operators_bitwise" => "15\n12\n3\n",
        "operators_logical" => "logical true\n",
        "package_path" => "package path ok\n",
        "prelude_variants" => "prelude variants ok\n",
        "prelude_shadow" => "shadow ok / qualified ok\n",
        "primitives" => "primitives ok\n",
        "process_controlled_blocking" => "set NOMO_PROCESS_FIXTURE to a line-oriented executable\n",
        "async_process_pipe_contract" => "spawn\n",
        "async_process_pipe_unix" => "stdin flushed\nasync:hello from Nomo\nexit 0 0\n",
        "async_process_pipe_windows" => "stdin flushed\nasync:hello from Nomo\nexit 0 0\n",
        "async_process_stress" => "saturation limit\nslot reuse 32\n",
        "pub_visibility" => "pub visibility ok\n",
        "read_file" => "file ok\n",
        "result_chain" => "result ok\n",
        "result_helpers" => "predicates\nseed\nfallback\nseed! ok\nerr\n",
        "result_main" => "result main ok\n",
        "result_map_err" => "mapped err ok\n",
        "specific_array_new" => "specific array new ok\n",
        "specific_import" => "specific import ok\n",
        "specific_type_import" => "specific type import ok\n",
        "specific_value_import" => "specific value import ok\n",
        "std_json" => "{\"lang\":\"nomo\",\"versions\":[1,true,null]}\ninvalid json syntax\n",
        "structured_json" => {
            "{\"model\":\"nomo-fixture\",\"messages\":[{\"role\":\"user\",\"content\":\"Hello \\\"Nomo\\\"\"}],\"stream\":false,\"max_tokens\":64}\nHello from structured JSON\n"
        }
        "std_http" => "get-ok\npost-ok\n",
        "openai_compatible" => {
            "200\ntls\n{\"id\":\"chatcmpl-local\",\"choices\":[{\"message\":{\"role\":\"assistant\",\"content\":\"fixture-ok\"}}]}\n"
        }
        "concurrent_openai_compatible" => {
            "{\"id\":\"chatcmpl-task-local\",\"choices\":[{\"message\":{\"role\":\"assistant\",\"content\":\"concurrent-ok\"}}]}\n{\"id\":\"chatcmpl-task-local\",\"choices\":[{\"message\":{\"role\":\"assistant\",\"content\":\"concurrent-ok\"}}]}\n"
        }
        "openai_streaming" => {
            "200\ntoken\n{\"choices\":[{\"delta\":{\"content\":\"Hel\"}}]}\ntoken\n{\"choices\":[{\"delta\":{\"content\":\"lo\"}}]}\nmessage\n[DONE]\n"
        }
        "std_fmt" => "display=Nomo debug=Nomo braces={}\nNomo\n",
        "std_path" => "/tmp/nomo.txt\nnomo.txt\n/tmp\ngz\n/tmp/b\n../b\nabsolute\n",
        "std_process" => "spawn-ok\nstatus-ok\nprocess-ok\nstatus-7\ncaptured-out\ncaptured-err\n",
        "std_time" => "1500\n2000\n1500ms\n",
        "string_extended" => "predicates\ntrim\ncase\nb\n",
        "string_lifecycle" => "string lifecycle ok\n",
        "string_methods" => "string methods ok\n",
        "struct_array_lifecycle" => "struct array lifecycle ok\n",
        "struct_methods" => "a@nomo.dev\n",
        "struct_option_field" => "struct option field ok\n",
        "struct_result_field" => "struct result field ok\n",
        "tail_expression" => "tail expression ok\n",
        _ => return None,
    })
}

fn expected_stderr(example: &str) -> &'static str {
    match example {
        "async_structured_panic_cleanup" => "panic: panic from child\n",
        "io_print" => "stderr ok\n",
        "io_stderr" => "stderr ok\n",
        _ => "",
    }
}

fn expected_exit_code(example: &Path) -> i32 {
    match example_name(example) {
        "async_structured_panic_cleanup" => 1,
        _ => 0,
    }
}
