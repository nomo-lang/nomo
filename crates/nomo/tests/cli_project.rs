use rcgen::{CertifiedKey, generate_simple_self_signed};
use rustls::pki_types::{PrivateKeyDer, PrivatePkcs8KeyDer};
use rustls::{ServerConfig, ServerConnection, StreamOwned};
use std::fs;
use std::io::{ErrorKind, Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream, UdpSocket as RustUdpSocket};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

const NOMO_HELP: &str = concat!(
    "nomo ",
    env!("CARGO_PKG_VERSION"),
    "\n\nCommands:\n  nomo new <name>\n  nomo check [path] [--json-errors] [--workspace]\n  nomo build [path] [--target <triple>] [--emit-c] [--json-errors] [--workspace] [--locked] [--offline] [--frozen]\n  nomo run [path] [--json-errors] [-- args...]\n  nomo fmt [path] [--check] [--json-errors]\n  nomo manifest migrate [path] [--check]\n  nomo test [path] [--workspace] [--package <package>] [--filter <text>] [--json] [--locked] [--offline] [--frozen]\n  nomo doc [path] [--workspace] [--package <package>] [--std] [--open] [--json] [--output <dir>]\n  nomo clean [path]\n  nomo cache stats [path]\n  nomo cache clean [path]\n  nomo cache prune [path] --max-bytes <bytes>\n  nomo login --registry <url> --token <token>\n  nomo owner add <owner/package> <user> --registry <url>\n  nomo owner remove <owner/package> <user> --registry <url>\n  nomo add <alias>@<owner>/<package>:<version> [path] [--registry <url>]\n  nomo remove <alias> [path]\n  nomo search <query> --registry <url>\n  nomo yank <owner/package> <version> --registry <url>\n  nomo publish [path] (--dry-run | --registry <url>) [--output <dir>] [--json-errors]\n  nomo deps resolve [path] [--workspace] [--locked] [--offline] [--frozen]\n  nomo deps tree [path] [--workspace] [--target <triple>] [--locked] [--offline] [--frozen]\n  nomo deps update [path] [alias-or-package] [--workspace] [--offline] [--precise <version-or-rev>]\n  nomo deps vendor [path] [--workspace] [--dir vendor] [--sync]\n  nomo deps clean-cache [path]\n\n"
);

const NOMOC_HELP: &str = concat!(
    "nomoc ",
    env!("CARGO_PKG_VERSION"),
    "\n\nCommands:\n  nomoc check <source.nomo> [--json-errors]\n  nomoc build <source.nomo> [--target <triple>] [--emit-c] [--out path] [--json-errors]\n\n"
);

fn expected_nomo_help() -> String {
    format!(
        "{}\n  nomo ffi bindgen <header> --package <package> --output <file> [--provenance <file>]\n\n  nomo owner key add <owner/package> <ed25519-public-key-hex> --registry <url>\n  nomo owner key revoke <owner/package> <key-id> --registry <url>\n  nomo publish [path] (--dry-run | --registry <url>) [--output <dir>] [--signer <command>] [--envelope <file>] [--json-errors]\n  nomo verify <archive> --envelope <file> --key <ed25519-public-key-hex> [--provenance <file>] [--transparency <file> --log-key <ed25519-public-key-hex>] [--cached-head <file>] [--gossip <file>] [--write-gossip <file>] [--proof-max-age-seconds <seconds>] [--offline-proof-max-age-seconds <seconds>] [--max-future-skew-seconds <seconds>] [--offline]\n",
        NOMO_HELP.trim_end()
    )
}

fn http_header<'a>(headers: &'a str, name: &str) -> Option<&'a str> {
    headers.lines().find_map(|line| {
        let (actual_name, value) = line.split_once(':')?;
        actual_name.eq_ignore_ascii_case(name).then(|| value.trim())
    })
}

#[test]
fn nomo_help_prints_command_summary() {
    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("help")
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        expected_nomo_help()
    );
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn nomo_help_flags_print_command_summary() {
    for flag in ["--help", "-h"] {
        let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
            .arg(flag)
            .output()
            .unwrap();

        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stdout),
            expected_nomo_help()
        );
        assert!(
            output.stderr.is_empty(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn manifest_migrate_cli_checks_writes_and_is_idempotent() {
    let root = temp_test_root("manifest-migrate-cli");
    reset_dir(&root);
    fs::write(
        root.join("nomo.toml"),
        "[package]\nnamespace = \"local\"\nname = \"legacy-demo\"\nversion = \"0.1.0\"\nedition = \"2026\"\n\n[trust]\npolicy = \"signed\"\n",
    )
    .unwrap();

    let check = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .args(["manifest", "migrate"])
        .arg(&root)
        .arg("--check")
        .output()
        .unwrap();
    assert!(!check.status.success());
    assert!(
        String::from_utf8_lossy(&check.stderr).contains("manifest migration required"),
        "{}",
        String::from_utf8_lossy(&check.stderr)
    );
    assert!(
        !fs::read_to_string(root.join("nomo.toml"))
            .unwrap()
            .contains("manifest-version")
    );

    let migrate = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .args(["manifest", "migrate"])
        .arg(&root)
        .output()
        .unwrap();
    assert!(
        migrate.status.success(),
        "{}",
        String::from_utf8_lossy(&migrate.stderr)
    );
    assert!(
        fs::read_to_string(root.join("nomo.toml"))
            .unwrap()
            .contains("manifest-version = 2")
    );
    assert!(root.join(".nomo/config.toml").is_file());

    let second = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .args(["manifest", "migrate"])
        .arg(&root)
        .arg("--check")
        .output()
        .unwrap();
    assert!(
        second.status.success(),
        "{}",
        String::from_utf8_lossy(&second.stderr)
    );
    assert!(String::from_utf8_lossy(&second.stdout).contains("manifest v2 is up to date"));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn nomoc_help_prints_command_summary() {
    let output = Command::new(env!("CARGO_BIN_EXE_nomoc"))
        .arg("help")
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), NOMOC_HELP);
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn nomoc_help_flags_print_command_summary() {
    for flag in ["--help", "-h"] {
        let output = Command::new(env!("CARGO_BIN_EXE_nomoc"))
            .arg(flag)
            .output()
            .unwrap();

        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(String::from_utf8_lossy(&output.stdout), NOMOC_HELP);
        assert!(
            output.stderr.is_empty(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn nomo_build_explicit_target_uses_canonical_isolated_artifact_directory() {
    let root = temp_test_root("build-explicit-target");
    reset_dir(&root);
    let create = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("new")
        .arg("target-demo")
        .current_dir(&root)
        .output()
        .unwrap();
    assert!(
        create.status.success(),
        "{}",
        String::from_utf8_lossy(&create.stderr)
    );

    let project = root.join("target-demo");
    let target = nomo::target::TargetTriple::host().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("build")
        .arg(&project)
        .arg("--emit-c")
        .arg("--target")
        .arg(target.to_string())
        .output()
        .unwrap();

    let c_path = project
        .join("build")
        .join(target.to_string())
        .join("c/main.c");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        format!("built {}\n", c_path.display())
    );
    let c = fs::read_to_string(&c_path).unwrap();
    assert!(c.starts_with(&format!("/* nomo target: {target} */\n")));
    assert!(c.contains(&format!("#define NOMO_TARGET_TRIPLE \"{target}\"")));
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn target_conditioned_dependencies_are_locked_completely_and_filtered_for_builds() {
    let root = temp_test_root("target-conditioned-dependencies");
    reset_dir(&root);
    let app = root.join("app");
    let linux = root.join("linux");
    let windows = root.join("windows");
    for package in [&app, &linux, &windows] {
        fs::create_dir_all(package.join("src")).unwrap();
    }
    fs::write(
        app.join("nomo.toml"),
        "[package]\nnamespace = \"app\"\nname = \"app\"\nversion = \"1.0.0\"\nedition = \"2026\"\n\n[dependencies]\nlinux = { package = \"app/linux\", path = \"../linux\", target = { os = \"linux\" } }\nwindows = { package = \"app/windows\", path = \"../windows\", target = { os = [\"windows\"] } }\n",
    )
    .unwrap();
    fs::write(
        app.join("src/main.nomo"),
        "package app\n\nfn main() -> void {\n}\n",
    )
    .unwrap();
    for (package, name) in [(&linux, "linux"), (&windows, "windows")] {
        fs::write(
            package.join("nomo.toml"),
            format!(
                "[package]\nnamespace = \"app\"\nname = \"{name}\"\nversion = \"1.0.0\"\nedition = \"2026\"\n"
            ),
        )
        .unwrap();
        fs::write(
            package.join("src/main.nomo"),
            format!("package {name}\n\npub fn value() -> i64 {{\n    return 1\n}}\n"),
        )
        .unwrap();
    }

    let resolve = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("deps")
        .arg("resolve")
        .arg(&app)
        .output()
        .unwrap();
    assert!(
        resolve.status.success(),
        "{}",
        String::from_utf8_lossy(&resolve.stderr)
    );
    let lock = fs::read_to_string(app.join("nomo.lock")).unwrap();
    assert!(lock.contains("app/linux"), "{lock}");
    assert!(lock.contains("app/windows"), "{lock}");
    assert!(lock.contains("os = [\"linux\"]"), "{lock}");
    assert!(lock.contains("os = [\"windows\"]"), "{lock}");

    for (target, included, excluded) in [
        ("x86_64-unknown-linux-gnu", "app/linux", "app/windows"),
        ("x86_64-pc-windows-msvc", "app/windows", "app/linux"),
    ] {
        let tree = Command::new(env!("CARGO_BIN_EXE_nomo"))
            .arg("deps")
            .arg("tree")
            .arg(&app)
            .arg("--locked")
            .arg("--target")
            .arg(target)
            .output()
            .unwrap();
        assert!(
            tree.status.success(),
            "{}",
            String::from_utf8_lossy(&tree.stderr)
        );
        let tree = String::from_utf8_lossy(&tree.stdout);
        assert!(tree.contains(included), "{tree}");
        assert!(!tree.contains(excluded), "{tree}");

        let build = Command::new(env!("CARGO_BIN_EXE_nomo"))
            .arg("build")
            .arg(&app)
            .arg("--locked")
            .arg("--emit-c")
            .arg("--target")
            .arg(target)
            .output()
            .unwrap();
        assert!(
            build.status.success(),
            "{}",
            String::from_utf8_lossy(&build.stderr)
        );
    }

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_build_rejects_unsupported_target_before_creating_artifacts() {
    let root = temp_test_root("build-invalid-target");
    reset_dir(&root);
    let project = root.join("invalid-target-demo");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nnamespace = \"local\"\nname = \"invalid-target-demo\"\nversion = \"1.0.0\"\nedition = \"2026\"\n",
    )
    .unwrap();
    fs::write(
        project.join("src/main.nomo"),
        "package app.main\n\nfn main() -> void {}\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("build")
        .arg(&project)
        .arg("--target")
        .arg("riscv64-unknown-linux-gnu")
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("unsupported target architecture `riscv64`")
    );
    assert!(!project.join("build").exists());
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomoc_build_embeds_the_requested_target_in_emitted_c() {
    let root = temp_test_root("nomoc-explicit-target");
    reset_dir(&root);
    let source = root.join("main.nomo");
    let output_path = root.join("target.c");
    fs::write(&source, "package app.main\n\nfn main() -> void {\n}\n").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomoc"))
        .arg("build")
        .arg(&source)
        .arg("--target")
        .arg("aarch64-unknown-linux-gnu")
        .arg("--out")
        .arg(&output_path)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let c = fs::read_to_string(&output_path).unwrap();
    assert!(c.starts_with("/* nomo target: aarch64-unknown-linux-gnu */\n"));
    assert!(c.contains("#define NOMO_TARGET_ARCH \"aarch64\""));
    assert!(c.contains("#define NOMO_TARGET_PLATFORM \"linux\""));
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_fmt_formats_standalone_source_file() {
    let root = temp_test_root("fmt-standalone");
    reset_dir(&root);
    let source = root.join("a.nomo");
    fs::write(
        &source,
        "package app . main\nfn main(){\nlet message:string=\"hi\"\n}\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("fmt")
        .arg(&source)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        format!("formatted {}\n", source.display())
    );
    assert_eq!(
        fs::read_to_string(&source).unwrap(),
        "package app.main\n\nfn main() -> void {\n    let message: string = \"hi\"\n}\n"
    );

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_fmt_preserves_comments_in_standalone_source_file() {
    let root = temp_test_root("fmt-comments");
    reset_dir(&root);
    let source = root.join("a.nomo");
    fs::write(
        &source,
        "package app . main\n\n/// Entry point\nfn main(){\nlet message:string=\"hi\" // greeting\n}\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("fmt")
        .arg(&source)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read_to_string(&source).unwrap(),
        "package app.main\n\n/// Entry point\nfn main() -> void {\n    let message: string = \"hi\" // greeting\n}\n"
    );

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_fmt_check_reports_differences_without_writing() {
    let root = temp_test_root("fmt-check");
    reset_dir(&root);
    let source = root.join("a.nomo");
    let original = "package app . main\nfn main(){\n}\n";
    fs::write(&source, original).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("fmt")
        .arg("--check")
        .arg(&source)
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        format!("would format {}\n", source.display())
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stderr),
        "format check failed\n"
    );
    assert_eq!(fs::read_to_string(&source).unwrap(), original);

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_fmt_formats_project_sources_recursively() {
    let root = temp_test_root("fmt-project");
    reset_dir(&root);
    let project = root.join("hello");
    fs::create_dir_all(project.join("src/math")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nnamespace = \"local\"\nname = \"hello\"\nversion = \"0.1.0\"\nedition = \"2026\"\n",
    )
    .unwrap();
    fs::write(
        project.join("src/main.nomo"),
        "package app.main\nimport app.math.main\nfn main(){\n}\n",
    )
    .unwrap();
    fs::write(
        project.join("src/math/main.nomo"),
        "package app.math.main\npub fn add(a:i32,b:i32)->i32{\nreturn a+b\n}\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("fmt")
        .arg(&project)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains(&format!(
        "formatted {}\n",
        project.join("src/main.nomo").display()
    )));
    assert!(stdout.contains(&format!(
        "formatted {}\n",
        project.join("src/math/main.nomo").display()
    )));
    assert_eq!(
        fs::read_to_string(project.join("src/main.nomo")).unwrap(),
        "package app.main\n\nimport app.math.main\n\nfn main() -> void {\n}\n"
    );
    assert_eq!(
        fs::read_to_string(project.join("src/math/main.nomo")).unwrap(),
        "package app.math.main\n\npub fn add(a: i32, b: i32) -> i32 {\n    return a + b\n}\n"
    );

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_fmt_json_errors_reports_parse_or_lex_diagnostic() {
    let root = temp_test_root("fmt-json-error");
    reset_dir(&root);
    let source = root.join("a.nomo");
    fs::write(
        &source,
        "package app.main\n\nfn main() -> void {\n    return;\n}\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("fmt")
        .arg("--json-errors")
        .arg(&source)
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("\"error_code\":\"E0211\""), "{stderr}");
    assert!(
        stderr.contains("expected newline after statement"),
        "{stderr}"
    );

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_doc_generates_html_and_search_index() {
    let root = temp_test_root("doc-html");
    reset_dir(&root);
    let project = root.join("hello");
    let output_dir = root.join("docs-out");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nnamespace = \"local\"\nname = \"hello\"\nversion = \"0.1.0\"\nedition = \"2026\"\n",
    )
    .unwrap();
    fs::write(
        project.join("src/main.nomo"),
        r#"//! Hello module docs.

package app.main

import std.ffi

/// Greets a caller.
pub fn greet(name: string) -> string {
    return "hello"
}

/**
 * User-facing record.
 * /* Nested detail. */
 * Still user-facing.
 */
pub struct User {
    /// User display name.
    pub name: string
}

/// Result status.
enum Status {
    /// Ready to run.
    Ready
}

/// Display contract.
pub interface Display {
    /// Converts to text.
    fn to_string(self) -> string
}

extern "C" {
    /// Writes a C string.
    fn puts(message: CString) -> i32
}
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("doc")
        .arg(&project)
        .arg("--output")
        .arg(&output_dir)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        format!("documented {}\n", output_dir.display())
    );
    let module_html = fs::read_to_string(output_dir.join("local/hello/app_main.html")).unwrap();
    assert!(module_html.contains("Hello module docs."), "{module_html}");
    assert!(module_html.contains("Greets a caller."), "{module_html}");
    assert!(
        module_html.contains("pub fn greet(name: string) -&gt; string"),
        "{module_html}"
    );
    assert!(module_html.contains("User-facing record."), "{module_html}");
    assert!(module_html.contains("Nested detail."), "{module_html}");
    assert!(module_html.contains("Still user-facing."), "{module_html}");
    assert!(module_html.contains("field User.name"), "{module_html}");
    assert!(module_html.contains("User display name."), "{module_html}");
    assert!(
        module_html.contains("variant Status.Ready"),
        "{module_html}"
    );
    assert!(module_html.contains("Ready to run."), "{module_html}");
    assert!(
        module_html.contains("pub interface Display"),
        "{module_html}"
    );
    assert!(module_html.contains("Display contract."), "{module_html}");
    assert!(
        module_html.contains("fn Display.to_string(self: Self) -&gt; string"),
        "{module_html}"
    );
    assert!(module_html.contains("Converts to text."), "{module_html}");
    assert!(
        module_html.contains("extern &quot;C&quot; fn puts(message: CString) -&gt; i32"),
        "{module_html}"
    );
    assert!(module_html.contains("Writes a C string."), "{module_html}");
    assert!(module_html.contains("private"), "{module_html}");
    let search = fs::read_to_string(output_dir.join("search-index.json")).unwrap();
    assert!(search.contains("\"name\":\"greet\""), "{search}");
    assert!(search.contains("\"kind\":\"struct\""), "{search}");
    assert!(search.contains("\"kind\":\"field\""), "{search}");
    assert!(search.contains("\"name\":\"User.name\""), "{search}");
    assert!(search.contains("\"kind\":\"variant\""), "{search}");
    assert!(search.contains("\"name\":\"Status.Ready\""), "{search}");
    assert!(search.contains("\"kind\":\"interface\""), "{search}");
    assert!(search.contains("\"name\":\"Display\""), "{search}");
    assert!(search.contains("\"kind\":\"interface_method\""), "{search}");
    assert!(
        search.contains("\"name\":\"Display.to_string\""),
        "{search}"
    );
    assert!(search.contains("\"kind\":\"extern_function\""), "{search}");
    assert!(search.contains("\"name\":\"puts\""), "{search}");

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_doc_open_opens_generated_index() {
    let root = temp_test_root("doc-open");
    reset_dir(&root);
    let project = root.join("hello");
    let output_dir = root.join("docs-out");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nnamespace = \"local\"\nname = \"hello\"\nversion = \"0.1.0\"\nedition = \"2026\"\n",
    )
    .unwrap();
    fs::write(
        project.join("src/main.nomo"),
        "//! Hello module docs.\n\npackage app.main\n\npub fn greet() -> string {\n    return \"hello\"\n}\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .env("NOMO_DOC_OPEN", "0")
        .arg("doc")
        .arg(&project)
        .arg("--open")
        .arg("--output")
        .arg(&output_dir)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        format!("documented {}\n", output_dir.display())
    );
    assert!(output_dir.join("index.html").is_file());

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_doc_json_reports_project_docs() {
    let root = temp_test_root("doc-json");
    reset_dir(&root);
    let project = root.join("hello");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nnamespace = \"local\"\nname = \"hello\"\nversion = \"0.1.0\"\nedition = \"2026\"\n",
    )
    .unwrap();
    fs::write(
        project.join("src/main.nomo"),
        "package app.main\n\nimport std.ffi\n\n/// Adds numbers.\npub fn add(a: i64, b: i64) -> i64 {\n    return a + b\n}\n\n/// Documented user.\nstruct User {\n    /// User display name.\n    pub name: string\n}\n\n/// Result status.\nenum Status {\n    /// Ready to run.\n    Ready\n}\n\n/// Display contract.\npub interface Display {\n    /// Converts to text.\n    fn to_string(self) -> string\n}\n\nextern \"C\" {\n    /// Writes a C string.\n    fn puts(message: CString) -> i32\n}\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("doc")
        .arg("--json")
        .arg(&project)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"package\":\"local/hello\""), "{stdout}");
    assert!(stdout.contains("\"name\":\"app.main\""), "{stdout}");
    assert!(stdout.contains("\"docs\":\"Adds numbers.\""), "{stdout}");
    assert!(
        stdout.contains("\"signature\":\"pub fn add(a: i64, b: i64) -> i64\""),
        "{stdout}"
    );
    assert!(stdout.contains("\"kind\":\"field\""), "{stdout}");
    assert!(stdout.contains("\"name\":\"User.name\""), "{stdout}");
    assert!(
        stdout.contains("\"docs\":\"User display name.\""),
        "{stdout}"
    );
    assert!(stdout.contains("\"kind\":\"variant\""), "{stdout}");
    assert!(stdout.contains("\"name\":\"Status.Ready\""), "{stdout}");
    assert!(stdout.contains("\"docs\":\"Ready to run.\""), "{stdout}");
    assert!(stdout.contains("\"kind\":\"interface\""), "{stdout}");
    assert!(stdout.contains("\"name\":\"Display\""), "{stdout}");
    assert!(
        stdout.contains("\"docs\":\"Display contract.\""),
        "{stdout}"
    );
    assert!(
        stdout.contains("\"signature\":\"pub interface Display\""),
        "{stdout}"
    );
    assert!(stdout.contains("\"kind\":\"interface_method\""), "{stdout}");
    assert!(
        stdout.contains("\"name\":\"Display.to_string\""),
        "{stdout}"
    );
    assert!(
        stdout.contains("\"signature\":\"fn Display.to_string(self: Self) -> string\""),
        "{stdout}"
    );
    assert!(
        stdout.contains("\"docs\":\"Converts to text.\""),
        "{stdout}"
    );
    assert!(stdout.contains("\"kind\":\"extern_function\""), "{stdout}");
    assert!(stdout.contains("\"name\":\"puts\""), "{stdout}");
    assert!(
        stdout.contains("\"signature\":\"extern \\\"C\\\" fn puts(message: CString) -> i32\""),
        "{stdout}"
    );
    assert!(
        stdout.contains("\"docs\":\"Writes a C string.\""),
        "{stdout}"
    );
    assert!(!project.join("build/doc").exists());

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_doc_workspace_json_reports_member_docs() {
    let root = temp_test_root("doc-workspace-json");
    reset_dir(&root);
    let app = root.join("apps/cli");
    let core = root.join("packages/core");
    fs::create_dir_all(app.join("src")).unwrap();
    fs::create_dir_all(core.join("src")).unwrap();
    fs::write(
        root.join("nomo.toml"),
        "[workspace]\nmembers = [\"apps/*\", \"packages/*\"]\n\n[workspace.package]\nnamespace = \"fynn\"\nedition = \"2026\"\n",
    )
    .unwrap();
    fs::write(
        app.join("nomo.toml"),
        "[package]\nname = \"cli\"\nversion = \"0.1.0\"\nnamespace.workspace = true\nedition.workspace = true\n",
    )
    .unwrap();
    fs::write(
        core.join("nomo.toml"),
        "[package]\nname = \"core\"\nversion = \"0.1.0\"\nnamespace.workspace = true\nedition.workspace = true\n",
    )
    .unwrap();
    fs::write(
        app.join("src/main.nomo"),
        "package app.main\n\n/// Runs the CLI.\npub fn run_cli() -> void {\n}\n",
    )
    .unwrap();
    fs::write(
        core.join("src/main.nomo"),
        "package core.main\n\n/// Runs the core package.\npub fn run_core() -> void {\n}\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("doc")
        .arg("--workspace")
        .arg("--json")
        .arg(&root)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"package\":\"fynn/cli\""), "{stdout}");
    assert!(stdout.contains("\"name\":\"run_cli\""), "{stdout}");
    assert!(stdout.contains("\"package\":\"fynn/core\""), "{stdout}");
    assert!(stdout.contains("\"name\":\"run_core\""), "{stdout}");
    assert!(!root.join("build/doc").exists());

    let filtered = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("doc")
        .arg("--workspace")
        .arg("--package")
        .arg("fynn/core")
        .arg("--json")
        .arg(&root)
        .output()
        .unwrap();

    assert!(
        filtered.status.success(),
        "{}",
        String::from_utf8_lossy(&filtered.stderr)
    );
    let filtered_stdout = String::from_utf8_lossy(&filtered.stdout);
    assert!(
        !filtered_stdout.contains("\"package\":\"fynn/cli\""),
        "{filtered_stdout}"
    );
    assert!(
        filtered_stdout.contains("\"package\":\"fynn/core\""),
        "{filtered_stdout}"
    );

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_doc_std_json_reports_builtin_modules() {
    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("doc")
        .arg("--std")
        .arg("--json")
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"package\":\"nomo-lang/std\""), "{stdout}");
    assert!(stdout.contains("\"name\":\"std.io\""), "{stdout}");
    assert!(stdout.contains("Printing and terminal I/O."), "{stdout}");
    assert!(stdout.contains("\"name\":\"std.fmt\""), "{stdout}");
    assert!(stdout.contains("Type-safe value formatting."), "{stdout}");
    assert!(stdout.contains("\"name\":\"Display\""), "{stdout}");
    assert!(
        stdout.contains("pub fn format(template: string) -> string"),
        "{stdout}"
    );
    assert!(stdout.contains("\"name\":\"std.testing\""), "{stdout}");
    assert!(stdout.contains("\"name\":\"Option\""), "{stdout}");
    assert!(stdout.contains("\"name\":\"Result\""), "{stdout}");
    assert!(stdout.contains("\"name\":\"new\""), "{stdout}");
    assert!(
        stdout.contains("pub fn split(value: string, separator: string) -> Array<string>"),
        "{stdout}"
    );
    assert!(
        stdout.contains("pub fn unwrap_or<T>(value: Option<T>, fallback: T) -> T"),
        "{stdout}"
    );
    assert!(stdout.contains("Test assertion helpers."), "{stdout}");
    assert!(stdout.contains("\"name\":\"std.debug\""), "{stdout}");
    assert!(
        stdout.contains("Debug print and panic helpers."),
        "{stdout}"
    );
    assert!(stdout.contains("\"name\":\"std.ffi\""), "{stdout}");
    assert!(stdout.contains("\"name\":\"CString\""), "{stdout}");
    assert!(stdout.contains("\"name\":\"Opaque\""), "{stdout}");
    assert!(
        stdout.contains("\"source\":\"std/src/ffi.nomo\""),
        "{stdout}"
    );
}

#[test]
fn nomo_test_runs_project_tests_with_local_modules() {
    let root = temp_test_root("test-local-modules");
    reset_dir(&root);
    let project = root.join("hello");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nnamespace = \"local\"\nname = \"hello\"\nversion = \"0.1.0\"\nedition = \"2026\"\n",
    )
    .unwrap();
    fs::write(
        project.join("src/math.nomo"),
        "package app.math\n\npub fn add(a: i64, b: i64) -> i64 {\n    return a + b\n}\n",
    )
    .unwrap();
    fs::write(
        project.join("src/main.nomo"),
        r#"package app.main

import app.math

#[test]
fn main_test() -> void {
}

#[test]
fn add_test() -> void {
    let total: i64 = add(1, 2)
    if total == 3 {
        void
    } else {
        panic("bad add")
    }
}

fn main() -> void {
    panic("original main should not run")
}
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("test")
        .arg(&project)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "running 2 tests\nok app.main.add_test\nok app.main.main_test\n"
    );
    assert!(project.join("build/test/c/app_main_add_test.c").is_file());
    assert!(project.join("build/test/c/app_main_main_test.c").is_file());

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_check_preserves_suspend_effects_across_local_modules() {
    let root = temp_test_root("check-suspend-local-modules");
    reset_dir(&root);
    let project = root.join("hello");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nnamespace = \"local\"\nname = \"hello\"\nversion = \"0.1.0\"\nedition = \"2026\"\n",
    )
    .unwrap();
    fs::write(
        project.join("src/worker.nomo"),
        "package app.worker\n\npub suspend fn fetch() -> string {\n    return \"ready\"\n}\n",
    )
    .unwrap();
    fs::write(
        project.join("src/main.nomo"),
        "package app.main\n\nimport app.worker\n\nfn main() -> void {\n    let value: string = fetch()\n}\n",
    )
    .unwrap();

    let rejected = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("check")
        .arg(&project)
        .output()
        .unwrap();
    assert!(!rejected.status.success());
    let stderr = String::from_utf8_lossy(&rejected.stderr);
    assert!(stderr.contains("E0870"), "{stderr}");
    assert!(stderr.contains("suspend function `fetch`"), "{stderr}");

    fs::write(
        project.join("src/main.nomo"),
        "package app.main\n\nimport app.worker\n\nsuspend fn main() -> void {\n    let value: string = fetch()\n}\n",
    )
    .unwrap();
    let accepted = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("check")
        .arg(&project)
        .output()
        .unwrap();
    assert!(
        accepted.status.success(),
        "{}",
        String::from_utf8_lossy(&accepted.stderr)
    );

    fs::write(
        project.join("src/worker.nomo"),
        "package app.worker\n\nimport std.task\n\npub suspend fn pause() -> void {\n    task.yield_now()\n}\n",
    )
    .unwrap();
    fs::write(
        project.join("src/main.nomo"),
        "package app.main\n\nimport app.worker\n\nsuspend fn main() -> void {\n    pause()\n}\n",
    )
    .unwrap();
    let nested = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(&project)
        .output()
        .unwrap();
    assert!(
        nested.status.success(),
        "{}",
        String::from_utf8_lossy(&nested.stderr)
    );
    assert!(nested.stdout.is_empty());

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_check_rejects_transitive_blocking_sleep_from_suspend_module_graph() {
    let root = temp_test_root("check-suspend-blocking-sleep");
    reset_dir(&root);
    let project = root.join("hello");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nnamespace = \"local\"\nname = \"hello\"\nversion = \"0.1.0\"\nedition = \"2026\"\n",
    )
    .unwrap();
    fs::write(
        project.join("src/worker.nomo"),
        "package app.worker\n\nimport std.time\n\npub fn pause() -> void {\n    time.sleep_millis(1)\n}\n",
    )
    .unwrap();
    fs::write(
        project.join("src/main.nomo"),
        "package app.main\n\nimport app.worker\n\nsuspend fn main() -> void {\n    pause()\n}\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("check")
        .arg(&project)
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("E0891"), "{stderr}");
    assert!(
        stderr.contains("main -> pause -> time.sleep_millis"),
        "{stderr}"
    );
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_test_json_reports_failures() {
    let root = temp_test_root("test-json-failure");
    reset_dir(&root);
    let project = root.join("hello");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nnamespace = \"local\"\nname = \"hello\"\nversion = \"0.1.0\"\nedition = \"2026\"\n",
    )
    .unwrap();
    fs::write(
        project.join("src/main.nomo"),
        "package app.main\n\n#[test]\nfn fails() -> void {\n    panic(\"boom\")\n}\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("test")
        .arg("--json")
        .arg(&project)
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"status\":\"failed\""), "{stdout}");
    assert!(
        stdout.contains("\"name\":\"app.main.fails\",\"status\":\"failed\""),
        "{stdout}"
    );
    assert!(stdout.contains("panic: boom"), "{stdout}");

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_test_runs_std_testing_assert_helpers() {
    let root = temp_test_root("test-std-testing-asserts");
    reset_dir(&root);
    let project = root.join("hello");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nnamespace = \"local\"\nname = \"hello\"\nversion = \"0.1.0\"\nedition = \"2026\"\n",
    )
    .unwrap();
    fs::write(
        project.join("src/main.nomo"),
        r#"package app.main

import std.result
import std.testing

fn fail() -> Result<i64, string> {
    return Err("boom")
}

#[test]
fn assert_helpers() -> void {
    testing.assert(true, "expected true")
    testing.assert_equal(42, 42)
    testing.assert_equal("same", "same")
    testing.assert_error(fail())
}
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("test")
        .arg(&project)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "running 1 tests\nok app.main.assert_helpers\n"
    );
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_test_filter_runs_matching_tests_only() {
    let root = temp_test_root("test-filter");
    reset_dir(&root);
    let project = root.join("hello");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nnamespace = \"local\"\nname = \"hello\"\nversion = \"0.1.0\"\nedition = \"2026\"\n",
    )
    .unwrap();
    fs::write(
        project.join("src/main.nomo"),
        "package app.main\n\n#[test]\nfn fast() -> void {\n}\n\n#[test]\nfn slow_array() -> void {\n}\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("test")
        .arg("--filter")
        .arg("array")
        .arg(&project)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "running 1 tests\nok app.main.slow_array\n"
    );

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_test_workspace_package_selects_one_member() {
    let root = temp_test_root("test-workspace-package");
    reset_dir(&root);
    let app = root.join("apps/cli");
    let core = root.join("packages/core");
    fs::create_dir_all(app.join("src")).unwrap();
    fs::create_dir_all(core.join("src")).unwrap();
    fs::write(
        root.join("nomo.toml"),
        "[workspace]\nmembers = [\"apps/*\", \"packages/*\"]\n\n[workspace.package]\nnamespace = \"fynn\"\nedition = \"2026\"\n",
    )
    .unwrap();
    fs::write(
        app.join("nomo.toml"),
        "[package]\nname = \"cli\"\nversion = \"0.1.0\"\nnamespace.workspace = true\nedition.workspace = true\n",
    )
    .unwrap();
    fs::write(
        core.join("nomo.toml"),
        "[package]\nname = \"core\"\nversion = \"0.1.0\"\nnamespace.workspace = true\nedition.workspace = true\n",
    )
    .unwrap();
    fs::write(
        app.join("src/main.nomo"),
        "package app.main\n\n#[test]\nfn cli_test() -> void {\n}\n",
    )
    .unwrap();
    fs::write(
        core.join("src/main.nomo"),
        "package core.main\n\n#[test]\nfn core_test() -> void {\n}\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("test")
        .arg("--workspace")
        .arg("--package")
        .arg("fynn/core")
        .arg(&root)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "running 1 tests\nok core.main.core_test\n"
    );

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_test_rejects_parameters() {
    let root = temp_test_root("test-rejects-params");
    reset_dir(&root);
    let project = root.join("hello");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nnamespace = \"local\"\nname = \"hello\"\nversion = \"0.1.0\"\nedition = \"2026\"\n",
    )
    .unwrap();
    fs::write(
        project.join("src/main.nomo"),
        "package app.main\n\n#[test]\nfn bad(value: i32) -> void {\n}\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("test")
        .arg(&project)
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("E1101"), "{stderr}");
    assert!(
        stderr.contains("`#[test]` functions must not take parameters"),
        "{stderr}"
    );

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_test_rejects_suspend_functions_until_async_runner_is_available() {
    let root = temp_test_root("test-rejects-suspend");
    reset_dir(&root);
    let project = root.join("hello");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nnamespace = \"local\"\nname = \"hello\"\nversion = \"0.1.0\"\nedition = \"2026\"\n",
    )
    .unwrap();
    fs::write(
        project.join("src/main.nomo"),
        "package app.main\n\n#[test]\nsuspend fn async_test() -> void {\n}\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("test")
        .arg(&project)
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("E1101"), "{stderr}");
    assert!(
        stderr.contains(
            "`#[test]` functions must be synchronous until the async test runner is available"
        ),
        "{stderr}"
    );

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_fmt_formats_loose_source_directory_recursively() {
    let root = temp_test_root("fmt-loose-directory");
    reset_dir(&root);
    let dir = root.join("loose");
    fs::create_dir_all(dir.join("nested")).unwrap();
    fs::write(dir.join("main.nomo"), "package app . main\nfn main(){\n}\n").unwrap();
    fs::write(
        dir.join("nested/helper.nomo"),
        "package app . helper\npub fn ok()->bool{\nreturn true\n}\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("fmt")
        .arg(&dir)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains(&format!("formatted {}\n", dir.join("main.nomo").display())));
    assert!(stdout.contains(&format!(
        "formatted {}\n",
        dir.join("nested/helper.nomo").display()
    )));
    assert_eq!(
        fs::read_to_string(dir.join("main.nomo")).unwrap(),
        "package app.main\n\nfn main() -> void {\n}\n"
    );
    assert_eq!(
        fs::read_to_string(dir.join("nested/helper.nomo")).unwrap(),
        "package app.helper\n\npub fn ok() -> bool {\n    return true\n}\n"
    );

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_fmt_empty_directory_without_manifest_reports_no_sources() {
    let root = temp_test_root("fmt-empty-directory");
    reset_dir(&root);
    let dir = root.join("loose");
    fs::create_dir_all(&dir).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("fmt")
        .arg(&dir)
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("no .nomo files found under"), "{stderr}");

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_run_standalone_script_without_main() {
    let root = temp_test_root("run-script");
    reset_dir(&root);
    let source = root.join("a.nomo");
    fs::write(
        &source,
        "package app.main\n\nimport std.io\n\nlet message: string = \"script ok\"\nio.println(message)\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(&source)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), "script ok\n");

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_run_standalone_file_with_explicit_main() {
    let root = temp_test_root("run-standalone-main");
    reset_dir(&root);
    let source = root.join("a.nomo");
    fs::write(
        &source,
        "package app.main\n\nimport std.io\n\nfn main() -> void {\n    io.println(\"main ok\")\n}\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(&source)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), "main ok\n");

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_run_and_test_use_manifest_ffi_link_metadata() {
    let root = temp_test_root("ffi-link-metadata");
    reset_dir(&root);
    let project = root.join("ffi-link");
    let native = project.join("native");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::create_dir_all(&native).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nnamespace = \"local\"\nname = \"ffi-link\"\nversion = \"0.1.0\"\nedition = \"2026\"\n\n[ffi]\nlibraries = [\"nomoffi\"]\nlibrary_paths = [\"native\"]\n",
    )
    .unwrap();
    fs::write(
        native.join("nomoffi.c"),
        "int native_answer(int value) { return value * 2; }\n",
    )
    .unwrap();
    let compile_output = Command::new("cc")
        .arg("-c")
        .arg(native.join("nomoffi.c"))
        .arg("-o")
        .arg(native.join("nomoffi.o"))
        .output()
        .unwrap();
    assert!(
        compile_output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&compile_output.stdout),
        String::from_utf8_lossy(&compile_output.stderr)
    );
    let archive_output = Command::new("ar")
        .arg("rcs")
        .arg(native.join("libnomoffi.a"))
        .arg(native.join("nomoffi.o"))
        .output()
        .unwrap();
    assert!(
        archive_output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&archive_output.stdout),
        String::from_utf8_lossy(&archive_output.stderr)
    );
    fs::write(
        project.join("src/main.nomo"),
        r#"package app.main

import std.io

extern "C" {
    fn native_answer(value: i32) -> i32
}

#[test]
fn native_answer_test() -> void {
    unsafe {
        let answer: i32 = native_answer(10)
    }
    let status: string = if answer == 20 {
        "ok"
    } else {
        panic("ffi test link failed")
    }
}

fn main() -> void {
    unsafe {
        let answer: i32 = native_answer(21)
    }
    if answer == 42 {
        io.println("ffi link ok")
    } else {
        panic("ffi run link failed")
    }
}
"#,
    )
    .unwrap();

    let run_output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(&project)
        .output()
        .unwrap();
    assert!(
        run_output.status.success(),
        "{}",
        String::from_utf8_lossy(&run_output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&run_output.stdout), "ffi link ok\n");

    let test_output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("test")
        .arg(&project)
        .output()
        .unwrap();
    assert!(
        test_output.status.success(),
        "{}",
        String::from_utf8_lossy(&test_output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&test_output.stdout).contains("ok app.main.native_answer_test")
    );

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_run_executes_binary_arithmetic() {
    let root = temp_test_root("run-arithmetic");
    reset_dir(&root);
    let source = root.join("a.nomo");
    fs::write(
        &source,
        r#"package app.main

import std.io

fn main() -> void {
    let value: i64 = 20 - 3 * 4 / 2 % 5
    if value == 19 {
        io.println("arithmetic ok")
    } else {
        io.println("wrong")
    }
}
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(&source)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), "arithmetic ok\n");

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_run_executes_three_clause_for_with_inference_and_multi_println() {
    let root = temp_test_root("run-three-clause-for");
    reset_dir(&root);
    let source = root.join("a.nomo");
    fs::write(
        &source,
        r#"package app.main

import std.io

fn greeting() -> string {
    return "Hello, native"
}

fn main() -> void {
    let message = greeting()
    for let i: ui64 = 0; i < 3; i++ {
        io.println(message, i)
    }
}
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(&source)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "Hello, native 0\nHello, native 1\nHello, native 2\n"
    );

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_run_formats_scalars_and_display_structs() {
    let root = temp_test_root("run-std-fmt");
    reset_dir(&root);
    let source = root.join("a.nomo");
    fs::write(
        &source,
        r#"package app.main

import std.fmt
import std.io

struct User {
    name: string
}

impl fmt.Display for User {
    fn to_string(self) -> string {
        return self.name
    }
}

impl fmt.Debug for User {
    fn debug_string(self) -> string {
        return self.name
    }
}

fn main() -> void {
    let user: User = User { name: "Nomo" }
    let message = fmt.format("display={} debug={:?} count={{ {} }}", user, user, 3)
    io.println(message)
    io.println(user)
}
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(&source)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "display=Nomo debug=Nomo count={ 3 }\nNomo\n"
    );

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_run_short_circuits_logical_operators() {
    let root = temp_test_root("run-logical-short-circuit");
    reset_dir(&root);
    let source = root.join("a.nomo");
    fs::write(
        &source,
        r#"package app.main

import std.io

fn explode() -> bool {
    panic("should not run")
}

fn main() -> void {
    let ok: bool = true || explode()
    let also_ok: bool = false && explode()
    if ok && !also_ok {
        io.println("logical ok")
    } else {
        io.println("wrong")
    }
}
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(&source)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), "logical ok\n");

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_run_executes_bitwise_operators() {
    let root = temp_test_root("run-bitwise");
    reset_dir(&root);
    let source = root.join("a.nomo");
    fs::write(
        &source,
        r#"package app.main

import std.io

fn main() -> void {
    let value: i64 = 7 & 3 | 8 ^ 2 << 1
    let cleared: i64 = value &^ 3
    let shifted: i64 = cleared >> 2
    if shifted == 3 {
        io.println("bitwise ok")
    } else {
        io.println("wrong")
    }
}
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(&source)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), "bitwise ok\n");

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_run_executes_signed_shifts_portably() {
    let root = temp_test_root("run-signed-shifts");
    reset_dir(&root);
    let source = root.join("a.nomo");
    fs::write(
        &source,
        r#"package app.main

import std.io
import std.num

fn main() -> void {
    let negative: i64 = 0 - 8
    let minus_one: i64 = 0 - 1
    let negative32: i32 = negative as i32
    let first: i64 = negative >> 1
    let second: i64 = minus_one >> 63
    let third: i32 = negative32 >> 2
    let fourth: i64 = negative << 2
    let fifth: i32 = negative32 << 3
    io.println(num.to_string(first))
    io.println(num.to_string(second))
    io.println(num.to_string(third))
    io.println(num.to_string(fourth))
    io.println(num.to_string(fifth))
}
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(&source)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "-4\n-1\n-2\n-32\n-64\n"
    );

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_run_executes_unary_negation_and_parentheses() {
    let root = temp_test_root("run-unary-negation-parentheses");
    reset_dir(&root);
    let source = root.join("a.nomo");
    fs::write(
        &source,
        r#"package app.main

import std.io
import std.num

fn main() -> void {
    let base: i64 = -(2 + 3) * 4
    let shifted: i64 = (-8) >> 1
    let min32: i32 = -2147483648
    let ratio: f64 = -1.5
    io.println(num.to_string(base))
    io.println(num.to_string(shifted))
    io.println(num.to_string(min32))
    if ratio < 0.0 {
        io.println("negative f64")
    } else {
        io.println("wrong")
    }
}
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(&source)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "-20\n-4\n-2147483648\nnegative f64\n"
    );

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomoc_still_rejects_top_level_script_statements() {
    let root = temp_test_root("nomoc-script-reject");
    reset_dir(&root);
    let source = root.join("a.nomo");
    fs::write(&source, "package app.main\n\nlet value: i32 = 1\n").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomoc"))
        .arg("check")
        .arg(&source)
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("top-level script statements"), "{stderr}");

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn project_check_rejects_top_level_script_statements() {
    let root = temp_test_root("project-script-reject");
    reset_dir(&root);
    let project = root.join("hello");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nnamespace = \"local\"\nname = \"hello\"\nversion = \"0.1.0\"\nedition = \"2026\"\n",
    )
    .unwrap();
    fs::write(
        project.join("src/main.nomo"),
        "package app.main\n\nlet value: i32 = 1\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("check")
        .arg(&project)
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("top-level script statements"), "{stderr}");

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_run_source_file_with_bad_manifest_does_not_fallback_to_script_mode() {
    let root = temp_test_root("run-bad-manifest-no-script-fallback");
    reset_dir(&root);
    let project = root.join("hello");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nnamespace = \"std\"\nname = \"hello\"\nversion = \"0.1.0\"\nedition = \"2026\"\n",
    )
    .unwrap();
    fs::write(
        project.join("src/main.nomo"),
        "package app.main\n\nimport std.io\n\nlet message: string = \"should not run\"\nio.println(message)\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(project.join("src/main.nomo"))
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("reserved"), "{stderr}");

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_new_run_and_clean_project() {
    let root = temp_test_root("new-run-clean");
    reset_dir(&root);

    let new_output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("new")
        .arg("hello")
        .current_dir(&root)
        .output()
        .unwrap();
    assert!(
        new_output.status.success(),
        "{}",
        String::from_utf8_lossy(&new_output.stderr)
    );

    let project = root.join("hello");
    let manifest = project.join("nomo.toml");
    let source = project.join("src/main.nomo");
    assert!(manifest.exists());
    assert!(source.exists());
    assert_eq!(
        fs::read_to_string(&manifest).unwrap(),
        format!(
            "manifest-version = 2\n\n[package]\nnamespace = \"local\"\nname = \"hello\"\nversion = \"{}\"\nedition = \"2026\"\npublish = false\n",
            env!("CARGO_PKG_VERSION")
        )
    );
    let source_text = fs::read_to_string(&source).unwrap();
    assert!(source_text.contains("package hello"));
    assert!(source_text.contains("import std.io"));
    assert!(source_text.contains("fn main() -> void"));

    let check_output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("check")
        .arg(&project)
        .output()
        .unwrap();
    assert!(
        check_output.status.success(),
        "{}",
        String::from_utf8_lossy(&check_output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&check_output.stdout),
        format!("checked {}\n", source.display())
    );

    let run_output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(&project)
        .output()
        .unwrap();
    assert!(
        run_output.status.success(),
        "{}",
        String::from_utf8_lossy(&run_output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&run_output.stdout), "Hello, Nomo\n");
    assert!(project.join("build/bin/hello").exists());

    let clean_output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("clean")
        .arg(&project)
        .output()
        .unwrap();
    assert!(
        clean_output.status.success(),
        "{}",
        String::from_utf8_lossy(&clean_output.stderr)
    );
    assert!(!project.join("build").exists());

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_persistent_cache_survives_processes_and_recovers_from_corruption() {
    let root = temp_test_root("persistent-cache-cli");
    reset_dir(&root);
    let created = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("new")
        .arg("cached")
        .current_dir(&root)
        .output()
        .unwrap();
    assert!(
        created.status.success(),
        "{}",
        String::from_utf8_lossy(&created.stderr)
    );
    let project = root.join("cached");
    let source = project.join("src/main.nomo");
    let original = fs::read_to_string(&source).unwrap();

    let cold_check = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("check")
        .arg(&project)
        .env("NOMO_INCREMENTAL_TRACE", "1")
        .output()
        .unwrap();
    assert!(cold_check.status.success());
    assert!(
        String::from_utf8_lossy(&cold_check.stderr)
            .contains("incremental-cache write semantic-check-success"),
        "{}",
        String::from_utf8_lossy(&cold_check.stderr)
    );

    let warm_check = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("check")
        .arg(&project)
        .env("NOMO_INCREMENTAL_TRACE", "1")
        .output()
        .unwrap();
    assert!(warm_check.status.success());
    assert!(
        String::from_utf8_lossy(&warm_check.stderr)
            .contains("incremental-cache hit semantic-check-success"),
        "{}",
        String::from_utf8_lossy(&warm_check.stderr)
    );

    for expected_event in ["write", "hit"] {
        let build = Command::new(env!("CARGO_BIN_EXE_nomo"))
            .arg("build")
            .arg(&project)
            .arg("--emit-c")
            .env("NOMO_INCREMENTAL_TRACE", "1")
            .output()
            .unwrap();
        assert!(
            build.status.success(),
            "{}",
            String::from_utf8_lossy(&build.stderr)
        );
        assert!(
            String::from_utf8_lossy(&build.stderr)
                .contains(&format!("incremental-cache {expected_event} codegen-c")),
            "{}",
            String::from_utf8_lossy(&build.stderr)
        );
    }

    let stats = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .args(["cache", "stats"])
        .arg(&project)
        .output()
        .unwrap();
    assert!(stats.status.success());
    let stats_stdout = String::from_utf8_lossy(&stats.stdout);
    assert!(stats_stdout.contains("schema 1\n"), "{stats_stdout}");
    assert!(stats_stdout.contains("entries 2\n"), "{stats_stdout}");

    let cache_root = project.join(".nomo/cache/incremental/v1");
    let check_entry = find_incremental_cache_entry(&cache_root, "semantic-check-success");
    fs::write(&check_entry, b"{truncated").unwrap();
    let recovered = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("check")
        .arg(&project)
        .env("NOMO_INCREMENTAL_TRACE", "1")
        .output()
        .unwrap();
    assert!(recovered.status.success());
    let recovered_stderr = String::from_utf8_lossy(&recovered.stderr);
    assert!(recovered_stderr.contains("incremental-cache corrupt"));
    assert!(recovered_stderr.contains("incremental-cache write"));

    fs::write(
        &source,
        original.replace("let message: string", "let message: i64"),
    )
    .unwrap();
    let changed = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("check")
        .arg(&project)
        .output()
        .unwrap();
    assert!(
        !changed.status.success(),
        "changed source reused stale success"
    );
    fs::write(&source, original).unwrap();

    let pruned = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .args(["cache", "prune"])
        .arg(&project)
        .args(["--max-bytes", "0"])
        .output()
        .unwrap();
    assert!(pruned.status.success());
    assert!(
        String::from_utf8_lossy(&pruned.stdout).contains("entries 0\n"),
        "{}",
        String::from_utf8_lossy(&pruned.stdout)
    );

    let cleaned = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .args(["cache", "clean"])
        .arg(&project)
        .output()
        .unwrap();
    assert!(cleaned.status.success());
    assert!(!project.join(".nomo/cache/incremental").exists());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn nomo_deps_resolve_writes_lockfile_for_namespace_first_manifest() {
    let root = temp_test_root("deps-resolve");
    reset_dir(&root);

    let new_output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("new")
        .arg("hello")
        .current_dir(&root)
        .output()
        .unwrap();
    assert!(
        new_output.status.success(),
        "{}",
        String::from_utf8_lossy(&new_output.stderr)
    );

    let project = root.join("hello");
    let utils = root.join("utils");
    let json = root.join("json");
    let json_rev = init_git_package(&json, "nomo-lang", "json");
    fs::create_dir_all(utils.join("src")).unwrap();
    fs::write(utils.join("src/main.nomo"), "package utils.main\n").unwrap();
    fs::write(
        utils.join("nomo.toml"),
        "[package]\nnamespace = \"fynn\"\nname = \"utils\"\nversion = \"0.1.0\"\nedition = \"2026\"\n",
    )
    .unwrap();
    fs::write(
        project.join("nomo.toml"),
        format!(
            "[package]\nnamespace = \"fynn\"\nname = \"hello\"\nversion = \"0.1.0\"\nedition = \"2026\"\n\n[dependencies]\nstd = {{ package = \"nomo-lang/std\", version = \"0.1.0\" }}\njson = {{ package = \"nomo-lang/json\", git = \"{}\", rev = \"{}\" }}\nlocal_utils = {{ package = \"fynn/utils\", path = \"../utils\" }}\n",
            json.display(),
            json_rev
        ),
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("deps")
        .arg("resolve")
        .arg(&project)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        format!("resolved {}\n", project.join("nomo.lock").display())
    );
    let lockfile = fs::read_to_string(project.join("nomo.lock")).unwrap();
    assert_checksum_lines(&lockfile, 2);
    assert_eq!(
        strip_checksum_lines(&lockfile),
        format!(
            "# This file is generated by `nomo deps resolve`.\n\n[[package]]\nid = \"nomo-lang/json\"\nalias = \"json\"\nsource = \"git+{}\"\nrev = \"{}\"\n\n[[package]]\nid = \"fynn/utils\"\nalias = \"local_utils\"\nsource = \"path+../utils\"\n",
            json.display(),
            json_rev
        )
    );

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_deps_tree_prints_dependency_aliases() {
    let root = temp_test_root("deps-tree");
    reset_dir(&root);
    let project = root.join("hello");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(project.join("src/main.nomo"), "package app.main\n").unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nnamespace = \"fynn\"\nname = \"hello\"\nversion = \"0.1.0\"\nedition = \"2026\"\n\n[dependencies]\nstd = { package = \"nomo-lang/std\", version = \"0.1.0\" }\njson = { package = \"nomo-lang/json\", version = \"0.1.0\" }\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("deps")
        .arg("tree")
        .arg(&project)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "fynn/hello 0.1.0\n+-- json -> nomo-lang/json 0.1.0 (registry)\n"
    );

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_workspace_member_inherits_package_and_dependencies() {
    let root = temp_test_root("workspace-member-inheritance");
    reset_dir(&root);
    let app = root.join("apps/cli");
    let core = root.join("packages/core");
    fs::create_dir_all(app.join("src")).unwrap();
    fs::create_dir_all(core.join("src")).unwrap();
    fs::write(
        root.join("nomo.toml"),
        "[workspace]\nmembers = [\"apps/*\", \"packages/*\"]\ndefault-members = [\"apps/cli\"]\n\n[workspace.package]\nnamespace = \"fynn\"\nedition = \"2026\"\n\n[workspace.dependencies]\ncore = { package = \"fynn/core\", path = \"packages/core\" }\njson = { package = \"nomo-lang/json\", version = \"0.1.0\" }\n",
    )
    .unwrap();
    fs::write(
        app.join("nomo.toml"),
        "[package]\nname = \"cli\"\nversion = \"0.1.0\"\nnamespace.workspace = true\nedition.workspace = true\n\n[dependencies]\ncore.workspace = true\njson.workspace = true\n",
    )
    .unwrap();
    fs::write(
        core.join("nomo.toml"),
        "[package]\nname = \"core\"\nversion = \"0.1.0\"\nnamespace.workspace = true\nedition.workspace = true\n",
    )
    .unwrap();
    fs::write(
        app.join("src/main.nomo"),
        "package app.main\n\nimport json.parser\nimport core.math\n\nfn main() -> void {\n    let total: i64 = add(40, 2)\n}\n",
    )
    .unwrap();
    fs::write(
        core.join("src/math.nomo"),
        "package core.math\n\npub fn add(a: i64, b: i64) -> i64 {\n    return a + b\n}\n",
    )
    .unwrap();
    fs::write(
        core.join("src/main.nomo"),
        "package core.main\n\nfn main() -> void {\n}\n",
    )
    .unwrap();

    let check_output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("check")
        .arg(&app)
        .output()
        .unwrap();

    assert!(
        check_output.status.success(),
        "{}",
        String::from_utf8_lossy(&check_output.stderr)
    );

    let workspace_check_output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("check")
        .arg("--workspace")
        .arg(&root)
        .output()
        .unwrap();

    assert!(
        workspace_check_output.status.success(),
        "{}",
        String::from_utf8_lossy(&workspace_check_output.stderr)
    );
    let workspace_check = String::from_utf8_lossy(&workspace_check_output.stdout);
    assert!(
        workspace_check.contains(&format!(
            "checked {}\n",
            app.join("src/main.nomo").display()
        )),
        "{workspace_check}"
    );
    assert!(
        workspace_check.contains(&format!(
            "checked {}\n",
            core.join("src/main.nomo").display()
        )),
        "{workspace_check}"
    );

    let workspace_build_output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("build")
        .arg("--workspace")
        .arg("--emit-c")
        .arg(&root)
        .output()
        .unwrap();

    assert!(
        workspace_build_output.status.success(),
        "{}",
        String::from_utf8_lossy(&workspace_build_output.stderr)
    );
    let workspace_build = String::from_utf8_lossy(&workspace_build_output.stdout);
    assert!(
        workspace_build.contains(&format!("built {}\n", app.join("build/c/main.c").display())),
        "{workspace_build}"
    );
    assert!(
        workspace_build.contains(&format!(
            "built {}\n",
            core.join("build/c/main.c").display()
        )),
        "{workspace_build}"
    );
    assert!(app.join("build/c/main.c").exists());
    assert!(core.join("build/c/main.c").exists());

    let workspace_tree_output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("deps")
        .arg("tree")
        .arg("--workspace")
        .arg(&root)
        .output()
        .unwrap();

    assert!(
        workspace_tree_output.status.success(),
        "{}",
        String::from_utf8_lossy(&workspace_tree_output.stderr)
    );
    let workspace_tree = String::from_utf8_lossy(&workspace_tree_output.stdout);
    assert!(
        workspace_tree.contains("fynn/cli 0.1.0"),
        "{workspace_tree}"
    );
    assert!(
        workspace_tree.contains("fynn/core 0.1.0"),
        "{workspace_tree}"
    );
    assert!(
        workspace_tree.contains("+-- core -> fynn/core"),
        "{workspace_tree}"
    );
    assert!(
        workspace_tree.contains("+-- json -> nomo-lang/json 0.1.0 (registry)"),
        "{workspace_tree}"
    );

    let workspace_resolve_output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("deps")
        .arg("resolve")
        .arg("--workspace")
        .arg(&root)
        .output()
        .unwrap();

    assert!(
        workspace_resolve_output.status.success(),
        "{}",
        String::from_utf8_lossy(&workspace_resolve_output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&workspace_resolve_output.stdout),
        format!("resolved {}\n", root.join("nomo.lock").display())
    );
    let workspace_lockfile = fs::read_to_string(root.join("nomo.lock")).unwrap();
    assert!(
        workspace_lockfile.contains("[[root]]\nid = \"fynn/cli\"\n"),
        "{workspace_lockfile}"
    );
    assert!(
        workspace_lockfile.contains("[[root]]\nid = \"fynn/core\"\n"),
        "{workspace_lockfile}"
    );
    assert!(
        workspace_lockfile
            .contains("dependencies = [\"core -> fynn/core\", \"json -> nomo-lang/json\"]"),
        "{workspace_lockfile}"
    );
    assert!(
        workspace_lockfile.contains("source = \"path+packages/core\""),
        "{workspace_lockfile}"
    );

    let locked_tree_output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("deps")
        .arg("tree")
        .arg(&app)
        .output()
        .unwrap();

    assert!(
        locked_tree_output.status.success(),
        "{}",
        String::from_utf8_lossy(&locked_tree_output.stderr)
    );
    let locked_tree = String::from_utf8_lossy(&locked_tree_output.stdout);
    assert!(locked_tree.contains("fynn/cli 0.1.0"), "{locked_tree}");
    assert!(
        locked_tree.contains("+-- core -> fynn/core"),
        "{locked_tree}"
    );
    assert!(
        locked_tree.contains("+-- json -> nomo-lang/json 0.1.0 (registry)"),
        "{locked_tree}"
    );

    let locked_workspace_tree_output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("deps")
        .arg("tree")
        .arg("--workspace")
        .arg(&root)
        .output()
        .unwrap();

    assert!(
        locked_workspace_tree_output.status.success(),
        "{}",
        String::from_utf8_lossy(&locked_workspace_tree_output.stderr)
    );
    let locked_workspace_tree = String::from_utf8_lossy(&locked_workspace_tree_output.stdout);
    assert!(
        locked_workspace_tree.contains("fynn/cli 0.1.0"),
        "{locked_workspace_tree}"
    );
    assert!(
        locked_workspace_tree.contains("fynn/core 0.1.0"),
        "{locked_workspace_tree}"
    );
    assert!(
        locked_workspace_tree.contains("+-- core -> fynn/core"),
        "{locked_workspace_tree}"
    );
    assert!(
        locked_workspace_tree.contains("+-- json -> nomo-lang/json 0.1.0 (registry)"),
        "{locked_workspace_tree}"
    );

    let resolve_output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("deps")
        .arg("resolve")
        .arg(&app)
        .output()
        .unwrap();

    assert!(
        resolve_output.status.success(),
        "{}",
        String::from_utf8_lossy(&resolve_output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&resolve_output.stdout),
        format!("resolved {}\n", root.join("nomo.lock").display())
    );

    let tree_output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("deps")
        .arg("tree")
        .arg(&app)
        .output()
        .unwrap();

    assert!(
        tree_output.status.success(),
        "{}",
        String::from_utf8_lossy(&tree_output.stderr)
    );
    let tree = String::from_utf8_lossy(&tree_output.stdout);
    assert!(tree.contains("fynn/cli 0.1.0"), "{tree}");
    assert!(tree.contains("+-- core -> fynn/core"), "{tree}");
    assert!(
        tree.contains("+-- json -> nomo-lang/json 0.1.0 (registry)"),
        "{tree}"
    );
    assert!(root.join("nomo.lock").exists());
    assert!(!app.join("nomo.lock").exists());

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_deps_resolve_records_explicit_registry_source() {
    let root = temp_test_root("deps-registry-source");
    reset_dir(&root);
    let project = root.join("hello");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(project.join("src/main.nomo"), "package app.main\n").unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nnamespace = \"fynn\"\nname = \"hello\"\nversion = \"0.1.0\"\nedition = \"2026\"\n\n[dependencies]\njson = { package = \"nomo-lang/json\", version = \"0.1.0\", registry = \"https://packages.nomo.test\" }\n",
    )
    .unwrap();

    let resolve_output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("deps")
        .arg("resolve")
        .arg(&project)
        .arg("--offline")
        .output()
        .unwrap();

    assert!(
        resolve_output.status.success(),
        "{}",
        String::from_utf8_lossy(&resolve_output.stderr)
    );
    let lockfile = fs::read_to_string(project.join("nomo.lock")).unwrap();
    assert_checksum_lines(&lockfile, 0);
    assert_eq!(
        strip_checksum_lines(&lockfile),
        "# This file is generated by `nomo deps resolve`.\n\n[[package]]\nid = \"nomo-lang/json\"\nalias = \"json\"\nversion = \"0.1.0\"\nsource = \"registry+https://packages.nomo.test\"\n"
    );

    let tree_output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("deps")
        .arg("tree")
        .arg(&project)
        .arg("--offline")
        .output()
        .unwrap();

    assert!(
        tree_output.status.success(),
        "{}",
        String::from_utf8_lossy(&tree_output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&tree_output.stdout),
        "fynn/hello 0.1.0\n+-- json -> nomo-lang/json 0.1.0 (registry https://packages.nomo.test)\n"
    );

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_deps_resolve_locks_git_branch_to_head_rev() {
    let root = temp_test_root("deps-git-branch");
    reset_dir(&root);
    let project = root.join("hello");
    let json = root.join("json");
    init_git_package(&json, "nomo-lang", "json");
    run_git(&json, &["checkout", "--quiet", "-b", "stable"]);
    fs::write(json.join("src/main.nomo"), "package json.main\n\n").unwrap();
    run_git(&json, &["add", "src/main.nomo"]);
    run_git(&json, &["commit", "--quiet", "-m", "stable branch"]);
    let stable_rev = git_head_rev(&json);

    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(project.join("src/main.nomo"), "package app.main\n").unwrap();
    fs::write(
        project.join("nomo.toml"),
        format!(
            "[package]\nnamespace = \"fynn\"\nname = \"hello\"\nversion = \"0.1.0\"\nedition = \"2026\"\n\n[dependencies]\njson = {{ package = \"nomo-lang/json\", git = \"{}\", branch = \"stable\" }}\n",
            json.display()
        ),
    )
    .unwrap();

    let resolve_output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("deps")
        .arg("resolve")
        .arg(&project)
        .output()
        .unwrap();

    assert!(
        resolve_output.status.success(),
        "{}",
        String::from_utf8_lossy(&resolve_output.stderr)
    );
    let lockfile = fs::read_to_string(project.join("nomo.lock")).unwrap();
    assert_checksum_lines(&lockfile, 1);
    assert_eq!(
        strip_checksum_lines(&lockfile),
        format!(
            "# This file is generated by `nomo deps resolve`.\n\n[[package]]\nid = \"nomo-lang/json\"\nalias = \"json\"\nsource = \"git+{}\"\nbranch = \"stable\"\nrev = \"{}\"\n",
            json.display(),
            stable_rev
        )
    );

    let tree_output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("deps")
        .arg("tree")
        .arg(&project)
        .output()
        .unwrap();

    assert!(
        tree_output.status.success(),
        "{}",
        String::from_utf8_lossy(&tree_output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&tree_output.stdout),
        format!(
            "fynn/hello 0.1.0\n+-- json -> nomo-lang/json (git {}@stable#{})\n",
            json.display(),
            stable_rev
        )
    );

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_deps_resolve_reuses_git_cache_and_fetches_branch_updates() {
    let root = temp_test_root("deps-git-cache-reuse");
    reset_dir(&root);
    let project = root.join("hello");
    let json = root.join("json");
    init_git_package(&json, "nomo-lang", "json");
    run_git(&json, &["checkout", "--quiet", "-b", "stable"]);
    fs::write(json.join("src/main.nomo"), "package json.main\n\n").unwrap();
    run_git(&json, &["add", "src/main.nomo"]);
    run_git(&json, &["commit", "--quiet", "-m", "stable branch"]);
    let first_rev = git_head_rev(&json);

    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(project.join("src/main.nomo"), "package app.main\n").unwrap();
    fs::write(
        project.join("nomo.toml"),
        format!(
            "[package]\nnamespace = \"fynn\"\nname = \"hello\"\nversion = \"0.1.0\"\nedition = \"2026\"\n\n[dependencies]\njson = {{ package = \"nomo-lang/json\", git = \"{}\", branch = \"stable\" }}\n",
            json.display()
        ),
    )
    .unwrap();

    let first_resolve = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("deps")
        .arg("resolve")
        .arg(&project)
        .output()
        .unwrap();
    assert!(
        first_resolve.status.success(),
        "{}",
        String::from_utf8_lossy(&first_resolve.stderr)
    );
    let checkout = find_git_cache_checkout(&project, "json");
    let marker = checkout.join(".cache-marker");
    fs::write(&marker, "kept\n").unwrap();

    fs::write(
        json.join("src/main.nomo"),
        "package json.main\n\npub fn version() -> i64 {\n    return 2\n}\n",
    )
    .unwrap();
    run_git(&json, &["add", "src/main.nomo"]);
    run_git(&json, &["commit", "--quiet", "-m", "stable update"]);
    let second_rev = git_head_rev(&json);
    assert_ne!(first_rev, second_rev);

    let second_resolve = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("deps")
        .arg("resolve")
        .arg(&project)
        .output()
        .unwrap();
    assert!(
        second_resolve.status.success(),
        "{}",
        String::from_utf8_lossy(&second_resolve.stderr)
    );
    assert!(marker.exists(), "git cache checkout was recreated");
    let lockfile = fs::read_to_string(project.join("nomo.lock")).unwrap();
    assert!(
        lockfile.contains(&format!("rev = \"{second_rev}\"")),
        "{lockfile}"
    );
    assert!(
        !lockfile.contains(&format!("rev = \"{first_rev}\"")),
        "{lockfile}"
    );

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_deps_update_rewrites_git_branch_lockfile() {
    let root = temp_test_root("deps-update-git-branch");
    reset_dir(&root);
    let project = root.join("hello");
    let json = root.join("json");
    init_git_package(&json, "nomo-lang", "json");
    run_git(&json, &["checkout", "--quiet", "-b", "stable"]);
    fs::write(json.join("src/main.nomo"), "package json.main\n\n").unwrap();
    run_git(&json, &["add", "src/main.nomo"]);
    run_git(&json, &["commit", "--quiet", "-m", "stable branch"]);
    let first_rev = git_head_rev(&json);

    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(project.join("src/main.nomo"), "package app.main\n").unwrap();
    fs::write(
        project.join("nomo.toml"),
        format!(
            "[package]\nnamespace = \"fynn\"\nname = \"hello\"\nversion = \"0.1.0\"\nedition = \"2026\"\n\n[dependencies]\njson = {{ package = \"nomo-lang/json\", git = \"{}\", branch = \"stable\" }}\n",
            json.display()
        ),
    )
    .unwrap();

    let resolve_output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("deps")
        .arg("resolve")
        .arg(&project)
        .output()
        .unwrap();
    assert!(
        resolve_output.status.success(),
        "{}",
        String::from_utf8_lossy(&resolve_output.stderr)
    );
    let lockfile = fs::read_to_string(project.join("nomo.lock")).unwrap();
    assert!(
        lockfile.contains(&format!("rev = \"{first_rev}\"")),
        "{lockfile}"
    );

    fs::write(
        json.join("src/main.nomo"),
        "package json.main\n\npub fn version() -> i64 {\n    return 2\n}\n",
    )
    .unwrap();
    run_git(&json, &["add", "src/main.nomo"]);
    run_git(&json, &["commit", "--quiet", "-m", "stable update"]);
    let second_rev = git_head_rev(&json);
    assert_ne!(first_rev, second_rev);

    let update_output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("deps")
        .arg("update")
        .arg(&project)
        .arg("json")
        .output()
        .unwrap();
    assert!(
        update_output.status.success(),
        "{}",
        String::from_utf8_lossy(&update_output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&update_output.stdout),
        format!("updated {}\n", project.join("nomo.lock").display())
    );
    let updated_lockfile = fs::read_to_string(project.join("nomo.lock")).unwrap();
    assert!(
        updated_lockfile.contains(&format!("rev = \"{second_rev}\"")),
        "{updated_lockfile}"
    );
    assert!(
        !updated_lockfile.contains(&format!("rev = \"{first_rev}\"")),
        "{updated_lockfile}"
    );

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_add_and_remove_edit_registry_dependency_manifest() {
    let root = temp_test_root("deps-add-remove");
    reset_dir(&root);
    let project = root.join("hello");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(project.join("src/main.nomo"), "package app.main\n").unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nnamespace = \"fynn\"\nname = \"hello\"\nversion = \"0.1.0\"\nedition = \"2026\"\n",
    )
    .unwrap();

    let add_output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("add")
        .arg("json@nomo-lang/json:0.1.0")
        .arg(&project)
        .arg("--registry")
        .arg("https://packages.nomo.test")
        .output()
        .unwrap();

    assert!(
        add_output.status.success(),
        "{}",
        String::from_utf8_lossy(&add_output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&add_output.stdout),
        format!("updated {}\n", project.join("nomo.toml").display())
    );
    let manifest = fs::read_to_string(project.join("nomo.toml")).unwrap();
    assert!(manifest.contains("[dependencies.json]\n"), "{manifest}");
    assert!(
        manifest.contains("package = \"nomo-lang/json\""),
        "{manifest}"
    );
    assert!(manifest.contains("version = \"0.1.0\""), "{manifest}");
    assert!(
        manifest.contains("registry = \"https://packages.nomo.test\""),
        "{manifest}"
    );

    let tree_output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("deps")
        .arg("tree")
        .arg(&project)
        .arg("--offline")
        .output()
        .unwrap();

    assert!(
        tree_output.status.success(),
        "{}",
        String::from_utf8_lossy(&tree_output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&tree_output.stdout),
        "fynn/hello 0.1.0\n+-- json -> nomo-lang/json 0.1.0 (registry https://packages.nomo.test)\n"
    );

    let remove_output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("remove")
        .arg("json")
        .arg(&project)
        .output()
        .unwrap();

    assert!(
        remove_output.status.success(),
        "{}",
        String::from_utf8_lossy(&remove_output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&remove_output.stdout),
        format!("updated {}\n", project.join("nomo.toml").display())
    );
    let manifest = fs::read_to_string(project.join("nomo.toml")).unwrap();
    assert!(!manifest.contains("[dependencies"), "{manifest}");
    assert!(!manifest.contains("nomo-lang/json"), "{manifest}");

    let tree_output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("deps")
        .arg("tree")
        .arg(&project)
        .output()
        .unwrap();

    assert!(
        tree_output.status.success(),
        "{}",
        String::from_utf8_lossy(&tree_output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&tree_output.stdout),
        "fynn/hello 0.1.0\n(no dependencies)\n"
    );

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_add_rejects_duplicate_dependency_alias() {
    let root = temp_test_root("deps-add-duplicate");
    reset_dir(&root);
    let project = root.join("hello");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(project.join("src/main.nomo"), "package app.main\n").unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nnamespace = \"fynn\"\nname = \"hello\"\nversion = \"0.1.0\"\nedition = \"2026\"\n\n[dependencies]\njson = { package = \"nomo-lang/json\", version = \"0.1.0\" }\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("add")
        .arg("json@nomo-lang/json:0.2.0")
        .arg(&project)
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("dependency `json` already exists"),
        "{stderr}"
    );

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_search_queries_http_registry() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let registry_addr = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut request = Vec::new();
        let mut buffer = [0u8; 1024];
        loop {
            let read = stream.read(&mut buffer).unwrap();
            assert!(read > 0, "connection closed before HTTP headers arrived");
            request.extend_from_slice(&buffer[..read]);
            if request.windows(4).any(|window| window == b"\r\n\r\n") {
                break;
            }
        }
        let request = String::from_utf8_lossy(&request);
        assert!(
            request.starts_with("GET /api/v1/packages?query=json%20lib HTTP/1.1\r\n"),
            "{request}"
        );
        assert_eq!(http_header(&request, "Accept"), Some("application/json"));
        let body = r#"[{"package":"nomo-lang/json","version":"0.1.0","description":"JSON parser"},{"package":"fynn/json-tools","version":"0.2.0"}]"#;
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        stream.write_all(response.as_bytes()).unwrap();
    });

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("search")
        .arg("json lib")
        .arg("--registry")
        .arg(format!("http://{registry_addr}"))
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    server.join().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("nomo-lang/json 0.1.0 - JSON parser\n"),
        "{stdout}"
    );
    assert!(stdout.contains("fynn/json-tools 0.2.0\n"), "{stdout}");
}

#[test]
fn nomo_search_requires_registry() {
    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("search")
        .arg("json")
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("nomo search requires --registry <url>"),
        "{stderr}"
    );
}

#[test]
fn nomo_login_writes_registry_credentials() {
    let root = temp_test_root("registry-login");
    reset_dir(&root);
    let nomo_home = root.join("nomo-home");

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("login")
        .arg("--registry")
        .arg("http://packages.example.test/")
        .arg("--token")
        .arg("secret-token")
        .env("NOMO_HOME", &nomo_home)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("logged in http://packages.example.test\n"),
        "{stdout}"
    );
    assert!(
        stdout.contains(&format!(
            "credentials {}\n",
            nomo_home.join("credentials.toml").display()
        )),
        "{stdout}"
    );
    let credentials = fs::read_to_string(nomo_home.join("credentials.toml")).unwrap();
    assert!(
        credentials.contains("endpoint = \"http://packages.example.test\""),
        "{credentials}"
    );
    assert!(
        credentials.contains("token = \"secret-token\""),
        "{credentials}"
    );

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_login_accepts_https_registry_credentials() {
    let root = temp_test_root("registry-login-https");
    reset_dir(&root);
    let nomo_home = root.join("nomo-home");

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("login")
        .arg("--registry")
        .arg("https://packages.example.test/api/")
        .arg("--token")
        .arg("secret-token")
        .env("NOMO_HOME", &nomo_home)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout)
            .contains("logged in https://packages.example.test/api\n")
    );
    let credentials = fs::read_to_string(nomo_home.join("credentials.toml")).unwrap();
    assert!(
        credentials.contains("endpoint = \"https://packages.example.test/api\""),
        "{credentials}"
    );
    assert!(
        credentials.contains("token = \"secret-token\""),
        "{credentials}"
    );

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_owner_add_uses_logged_in_registry_token() {
    let root = temp_test_root("registry-owner-add");
    reset_dir(&root);
    let nomo_home = root.join("nomo-home");
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let registry_addr = listener.local_addr().unwrap();
    let registry = format!("http://{registry_addr}");

    let login = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("login")
        .arg("--registry")
        .arg(&registry)
        .arg("--token")
        .arg("owner-token")
        .env("NOMO_HOME", &nomo_home)
        .output()
        .unwrap();
    assert!(
        login.status.success(),
        "{}",
        String::from_utf8_lossy(&login.stderr)
    );

    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut request = Vec::new();
        let mut buffer = [0u8; 1024];
        loop {
            let read = stream.read(&mut buffer).unwrap();
            assert!(read > 0, "connection closed before HTTP headers arrived");
            request.extend_from_slice(&buffer[..read]);
            if request.windows(4).any(|window| window == b"\r\n\r\n") {
                break;
            }
        }
        let request = String::from_utf8_lossy(&request);
        assert!(
            request.starts_with("PUT /api/v1/packages/fynn/hello/owners/alice HTTP/1.1\r\n"),
            "{request}"
        );
        assert_eq!(
            http_header(&request, "Authorization"),
            Some("Bearer owner-token")
        );
        assert_eq!(http_header(&request, "Content-Length"), Some("0"));
        stream
            .write_all(b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
            .unwrap();
    });

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("owner")
        .arg("add")
        .arg("fynn/hello")
        .arg("alice")
        .arg("--registry")
        .arg(&registry)
        .env("NOMO_HOME", &nomo_home)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    server.join().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("added owner alice to fynn/hello\n"),
        "{stdout}"
    );
    assert!(
        stdout.contains(&format!("registry {registry}\n")),
        "{stdout}"
    );
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_owner_add_requires_registry() {
    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("owner")
        .arg("add")
        .arg("fynn/hello")
        .arg("alice")
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("nomo owner add requires --registry <url>"),
        "{stderr}"
    );
}

#[test]
fn nomo_owner_remove_uses_logged_in_registry_token() {
    let root = temp_test_root("registry-owner-remove");
    reset_dir(&root);
    let nomo_home = root.join("nomo-home");
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let registry_addr = listener.local_addr().unwrap();
    let registry = format!("http://{registry_addr}");

    let login = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("login")
        .arg("--registry")
        .arg(&registry)
        .arg("--token")
        .arg("owner-token")
        .env("NOMO_HOME", &nomo_home)
        .output()
        .unwrap();
    assert!(
        login.status.success(),
        "{}",
        String::from_utf8_lossy(&login.stderr)
    );

    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut request = Vec::new();
        let mut buffer = [0u8; 1024];
        loop {
            let read = stream.read(&mut buffer).unwrap();
            assert!(read > 0, "connection closed before HTTP headers arrived");
            request.extend_from_slice(&buffer[..read]);
            if request.windows(4).any(|window| window == b"\r\n\r\n") {
                break;
            }
        }
        let request = String::from_utf8_lossy(&request);
        assert!(
            request.starts_with("DELETE /api/v1/packages/fynn/hello/owners/alice HTTP/1.1\r\n"),
            "{request}"
        );
        assert_eq!(
            http_header(&request, "Authorization"),
            Some("Bearer owner-token")
        );
        assert_eq!(http_header(&request, "Content-Length"), Some("0"));
        stream
            .write_all(b"HTTP/1.1 202 Accepted\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
            .unwrap();
    });

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("owner")
        .arg("remove")
        .arg("fynn/hello")
        .arg("alice")
        .arg("--registry")
        .arg(&registry)
        .env("NOMO_HOME", &nomo_home)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    server.join().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("removed owner alice from fynn/hello\n"),
        "{stdout}"
    );
    assert!(
        stdout.contains(&format!("registry {registry}\n")),
        "{stdout}"
    );
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_owner_remove_requires_registry() {
    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("owner")
        .arg("remove")
        .arg("fynn/hello")
        .arg("alice")
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("nomo owner remove requires --registry <url>"),
        "{stderr}"
    );
}

#[test]
fn nomo_owner_publisher_key_registration_and_revocation_use_separate_endpoints() {
    let root = temp_test_root("registry-publisher-keys");
    reset_dir(&root);
    let nomo_home = root.join("nomo-home");
    let public_key = "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a";
    let key_id =
        nomo_supply_chain::publisher_key_id(&nomo_supply_chain::decode_hex(public_key).unwrap());
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let registry = format!("http://{}", listener.local_addr().unwrap());
    let expected_key_id = key_id.clone();
    let server = thread::spawn(move || {
        for method in ["PUT", "DELETE"] {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = Vec::new();
            let mut buffer = [0u8; 1024];
            let (header_end, content_length) = loop {
                let read = stream.read(&mut buffer).unwrap();
                assert!(read > 0);
                request.extend_from_slice(&buffer[..read]);
                if let Some(end) = request.windows(4).position(|window| window == b"\r\n\r\n") {
                    let end = end + 4;
                    let headers = String::from_utf8_lossy(&request[..end]);
                    let length = http_header(&headers, "Content-Length")
                        .unwrap_or("0")
                        .parse::<usize>()
                        .unwrap();
                    break (end, length);
                }
            };
            while request.len() < header_end + content_length {
                let read = stream.read(&mut buffer).unwrap();
                assert!(read > 0);
                request.extend_from_slice(&buffer[..read]);
            }
            let headers = String::from_utf8_lossy(&request[..header_end]);
            let expected_path = format!(
                "{method} /api/v1/packages/fynn/hello/publisher-keys/{expected_key_id} HTTP/1.1\r\n"
            );
            assert!(headers.starts_with(&expected_path), "{headers}");
            if method == "PUT" {
                let body = String::from_utf8_lossy(&request[header_end..]);
                assert!(body.contains(public_key), "{body}");
                assert!(body.contains(&expected_key_id), "{body}");
                assert!(!body.contains("PRIVATE"), "{body}");
            } else {
                assert_eq!(content_length, 0);
            }
            stream
                .write_all(
                    b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                )
                .unwrap();
        }
    });

    let add = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("owner")
        .arg("key")
        .arg("add")
        .arg("fynn/hello")
        .arg(public_key)
        .arg("--registry")
        .arg(&registry)
        .env("NOMO_HOME", &nomo_home)
        .output()
        .unwrap();
    assert!(
        add.status.success(),
        "{}",
        String::from_utf8_lossy(&add.stderr)
    );
    assert!(String::from_utf8_lossy(&add.stdout).contains(&key_id));

    let revoke = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("owner")
        .arg("key")
        .arg("revoke")
        .arg("fynn/hello")
        .arg(&key_id)
        .arg("--registry")
        .arg(&registry)
        .env("NOMO_HOME", &nomo_home)
        .output()
        .unwrap();
    assert!(
        revoke.status.success(),
        "{}",
        String::from_utf8_lossy(&revoke.stderr)
    );
    server.join().unwrap();
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_yank_marks_http_registry_package_version() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let registry_addr = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut request = Vec::new();
        let mut buffer = [0u8; 1024];
        loop {
            let read = stream.read(&mut buffer).unwrap();
            assert!(read > 0, "connection closed before HTTP headers arrived");
            request.extend_from_slice(&buffer[..read]);
            if request.windows(4).any(|window| window == b"\r\n\r\n") {
                break;
            }
        }
        let request = String::from_utf8_lossy(&request);
        assert!(
            request.starts_with("POST /api/v1/packages/fynn/hello/0.1.0/yank HTTP/1.1\r\n"),
            "{request}"
        );
        assert_eq!(http_header(&request, "Content-Length"), Some("0"));
        stream
            .write_all(b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
            .unwrap();
    });

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("yank")
        .arg("fynn/hello")
        .arg("0.1.0")
        .arg("--registry")
        .arg(format!("http://{registry_addr}"))
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    server.join().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("yanked fynn/hello 0.1.0\n"), "{stdout}");
    assert!(
        stdout.contains(&format!("registry http://{registry_addr}\n")),
        "{stdout}"
    );
}

#[test]
fn nomo_yank_uses_logged_in_registry_token() {
    let root = temp_test_root("registry-yank-auth");
    reset_dir(&root);
    let nomo_home = root.join("nomo-home");
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let registry_addr = listener.local_addr().unwrap();
    let registry = format!("http://{registry_addr}");

    let login = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("login")
        .arg("--registry")
        .arg(&registry)
        .arg("--token")
        .arg("secret-token")
        .env("NOMO_HOME", &nomo_home)
        .output()
        .unwrap();
    assert!(
        login.status.success(),
        "{}",
        String::from_utf8_lossy(&login.stderr)
    );

    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut request = Vec::new();
        let mut buffer = [0u8; 1024];
        loop {
            let read = stream.read(&mut buffer).unwrap();
            assert!(read > 0, "connection closed before HTTP headers arrived");
            request.extend_from_slice(&buffer[..read]);
            if request.windows(4).any(|window| window == b"\r\n\r\n") {
                break;
            }
        }
        let request = String::from_utf8_lossy(&request);
        assert_eq!(
            http_header(&request, "Authorization"),
            Some("Bearer secret-token")
        );
        stream
            .write_all(b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
            .unwrap();
    });

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("yank")
        .arg("fynn/hello")
        .arg("0.1.0")
        .arg("--registry")
        .arg(&registry)
        .env("NOMO_HOME", &nomo_home)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    server.join().unwrap();
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_yank_requires_registry() {
    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("yank")
        .arg("fynn/hello")
        .arg("0.1.0")
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("nomo yank requires --registry <url>"),
        "{stderr}"
    );
}

#[test]
fn nomo_publish_dry_run_builds_package_archive_and_checksum() {
    let root = temp_test_root("publish-dry-run");
    reset_dir(&root);
    let project = root.join("hello");
    let out_dir = root.join("packages");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::create_dir_all(project.join("native")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nnamespace = \"fynn\"\nname = \"hello\"\nversion = \"0.1.0\"\nedition = \"2026\"\n\n[ffi]\nsources = [\"native/bridge.c\"]\n",
    )
    .unwrap();
    fs::write(
        project.join("src/main.nomo"),
        "package app.main\n\nfn main() -> void {\n}\n",
    )
    .unwrap();
    fs::write(
        project.join("src/util.nomo"),
        "package app.util\n\npub fn answer() -> i64 {\n    return 42\n}\n",
    )
    .unwrap();
    fs::write(
        project.join("native/bridge.c"),
        "void nomo_example_bridge(void) {}\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("publish")
        .arg(&project)
        .arg("--dry-run")
        .arg("--output")
        .arg(&out_dir)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let archive = out_dir.join("fynn-hello-0.1.0.nomo-package");
    assert!(
        stdout.contains("publish dry-run fynn/hello 0.1.0\n"),
        "{stdout}"
    );
    assert!(
        stdout.contains(&format!("archive {}\n", archive.display())),
        "{stdout}"
    );
    assert!(stdout.contains("checksum sha256:"), "{stdout}");
    assert!(stdout.contains("size "), "{stdout}");
    assert!(archive.is_file());
    let archive_text = fs::read_to_string(&archive).unwrap();
    assert!(
        archive_text.starts_with("nomo-package-v1\n"),
        "{archive_text}"
    );
    assert!(
        archive_text.contains("package fynn/hello\n"),
        "{archive_text}"
    );
    assert!(archive_text.contains("version 0.1.0\n"), "{archive_text}");
    assert!(archive_text.contains("file nomo.toml "), "{archive_text}");
    assert!(
        archive_text.contains("file src/main.nomo "),
        "{archive_text}"
    );
    assert!(
        archive_text.contains("file src/util.nomo "),
        "{archive_text}"
    );
    assert!(
        archive_text.contains("file native/bridge.c "),
        "{archive_text}"
    );

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_publish_respects_manifest_v2_publish_false() {
    let root = temp_test_root("publish-disabled");
    reset_dir(&root);
    let project = root.join("private-app");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "manifest-version = 2\n\n[package]\nnamespace = \"local\"\nname = \"private-app\"\nversion = \"0.1.0\"\nedition = \"2026\"\npublish = false\n",
    )
    .unwrap();
    fs::write(
        project.join("src/main.nomo"),
        "package app.main\n\nfn main() -> void {\n}\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("publish")
        .arg(&project)
        .arg("--dry-run")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("publish = false"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    fs::remove_dir_all(root).unwrap();
}

#[cfg(unix)]
#[test]
fn nomo_publish_rejects_package_file_symlink_escape() {
    use std::os::unix::fs::symlink;

    let root = temp_test_root("publish-source-symlink-escape");
    reset_dir(&root);
    let project = root.join("hello");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::create_dir_all(project.join("native")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nnamespace = \"fynn\"\nname = \"hello\"\nversion = \"0.1.0\"\nedition = \"2026\"\n\n[ffi]\nsources = [\"native/bridge.c\"]\n",
    )
    .unwrap();
    fs::write(
        project.join("src/main.nomo"),
        "package app.main\n\nfn main() -> void {\n}\n",
    )
    .unwrap();
    let outside = root.join("outside.c");
    fs::write(&outside, "void outside(void) {}\n").unwrap();
    symlink(&outside, project.join("native/bridge.c")).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("publish")
        .arg(&project)
        .arg("--dry-run")
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("escapes the package root through a symbolic link"),
        "{stderr}"
    );

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_publish_without_dry_run_or_registry_reports_required_mode() {
    let root = temp_test_root("publish-requires-mode");
    reset_dir(&root);
    let project = root.join("hello");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nnamespace = \"fynn\"\nname = \"hello\"\nversion = \"0.1.0\"\nedition = \"2026\"\n",
    )
    .unwrap();
    fs::write(
        project.join("src/main.nomo"),
        "package app.main\n\nfn main() -> void {\n}\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("publish")
        .arg(&project)
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("nomo publish requires either --dry-run or --registry <url>"),
        "{stderr}"
    );

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_publish_uploads_archive_to_http_registry() {
    let root = temp_test_root("publish-http-upload");
    reset_dir(&root);
    let project = root.join("hello");
    let out_dir = root.join("packages");
    let nomo_home = root.join("nomo-home");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nnamespace = \"fynn\"\nname = \"hello\"\nversion = \"0.1.0\"\nedition = \"2026\"\n",
    )
    .unwrap();
    fs::write(
        project.join("src/main.nomo"),
        "package app.main\n\nfn main() -> void {\n}\n",
    )
    .unwrap();

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let registry_addr = listener.local_addr().unwrap();
    let registry = format!("http://{registry_addr}");
    let login = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("login")
        .arg("--registry")
        .arg(&registry)
        .arg("--token")
        .arg("publish-token")
        .env("NOMO_HOME", &nomo_home)
        .output()
        .unwrap();
    assert!(
        login.status.success(),
        "{}",
        String::from_utf8_lossy(&login.stderr)
    );

    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut request = Vec::new();
        let mut buffer = [0u8; 1024];
        let header_end = loop {
            let read = stream.read(&mut buffer).unwrap();
            assert!(read > 0, "connection closed before HTTP headers arrived");
            request.extend_from_slice(&buffer[..read]);
            if let Some(header_end) = request.windows(4).position(|window| window == b"\r\n\r\n") {
                break header_end;
            }
        };
        let headers = String::from_utf8_lossy(&request[..header_end]);
        assert!(
            headers.starts_with("PUT /api/v1/packages/fynn/hello/0.1.0 HTTP/1.1\r\n"),
            "{headers}"
        );
        assert_eq!(
            http_header(&headers, "Content-Type"),
            Some("application/octet-stream")
        );
        assert_eq!(
            http_header(&headers, "Authorization"),
            Some("Bearer publish-token")
        );
        let content_length = http_header(&headers, "Content-Length")
            .and_then(|value| value.parse::<usize>().ok())
            .expect("missing Content-Length");
        let body_start = header_end + 4;
        while request.len() - body_start < content_length {
            let read = stream.read(&mut buffer).unwrap();
            assert!(read > 0, "connection closed before upload body finished");
            request.extend_from_slice(&buffer[..read]);
        }
        let body = &request[body_start..body_start + content_length];
        let body_text = String::from_utf8_lossy(body);
        assert!(body_text.starts_with("nomo-package-v1\n"), "{body_text}");
        assert!(body_text.contains("package fynn/hello\n"), "{body_text}");
        assert!(body_text.contains("version 0.1.0\n"), "{body_text}");
        stream
            .write_all(b"HTTP/1.1 201 Created\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
            .unwrap();
    });

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("publish")
        .arg(&project)
        .arg("--registry")
        .arg(&registry)
        .arg("--output")
        .arg(&out_dir)
        .env("NOMO_HOME", &nomo_home)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    server.join().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let archive = out_dir.join("fynn-hello-0.1.0.nomo-package");
    assert!(stdout.contains("published fynn/hello 0.1.0\n"), "{stdout}");
    assert!(
        stdout.contains(&format!("archive {}\n", archive.display())),
        "{stdout}"
    );
    assert!(stdout.contains("checksum sha256:"), "{stdout}");
    assert!(stdout.contains("size "), "{stdout}");
    assert!(
        stdout.contains(&format!("registry {registry}\n")),
        "{stdout}"
    );
    assert!(archive.is_file());

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_project_commands_use_file_registry_dependency_module_public_api() {
    let root = temp_test_root("file-registry-dependency-module-public-api");
    reset_dir(&root);
    let package = root.join("utils");
    let registry = root.join("registry");
    let registry_download = registry.join("api/v1/packages/fynn/utils/0.1.0/download");
    let archive_out = root.join("archive-out");
    let project = root.join("hello");
    let source = project.join("src/main.nomo");
    let c_path = project.join("build/c/main.c");
    fs::create_dir_all(package.join("src")).unwrap();
    fs::create_dir_all(project.join("src")).unwrap();
    fs::create_dir_all(registry_download.parent().unwrap()).unwrap();
    fs::write(
        package.join("nomo.toml"),
        "[package]\nnamespace = \"fynn\"\nname = \"utils\"\nversion = \"0.1.0\"\nedition = \"2026\"\n",
    )
    .unwrap();
    fs::write(
        package.join("src/main.nomo"),
        "package utils.main\n\nfn main() -> void {\n}\n",
    )
    .unwrap();
    fs::write(
        package.join("src/path.nomo"),
        r#"package utils.path

pub struct Segment {
    value: i64
}

pub fn join(a: i64, b: i64) -> i64 {
    return a + b
}

pub fn make_segment(value: i64) -> Segment {
    return Segment { value: value }
}
"#,
    )
    .unwrap();

    let publish_output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("publish")
        .arg(&package)
        .arg("--dry-run")
        .arg("--output")
        .arg(&archive_out)
        .output()
        .unwrap();
    assert!(
        publish_output.status.success(),
        "{}",
        String::from_utf8_lossy(&publish_output.stderr)
    );
    fs::copy(
        archive_out.join("fynn-utils-0.1.0.nomo-package"),
        &registry_download,
    )
    .unwrap();
    let archive = fs::read(&registry_download).unwrap();
    let archive_checksum = nomo_resolver::archive_checksum(&archive);
    let registry_metadata = registry_download.parent().unwrap().join("metadata.json");
    fs::write(
        &registry_metadata,
        format!(
            "{{\"package\":\"fynn/utils\",\"version\":\"0.1.0\",\"checksum\":\"{archive_checksum}\",\"yanked\":false}}\n"
        ),
    )
    .unwrap();

    fs::write(
        project.join("nomo.toml"),
        format!(
            "[package]\nnamespace = \"fynn\"\nname = \"hello\"\nversion = \"0.1.0\"\nedition = \"2026\"\n\n[dependencies]\nlocal_utils = {{ package = \"fynn/utils\", version = \"0.1.0\", registry = \"file://{}\" }}\n",
            registry.display()
        ),
    )
    .unwrap();
    fs::write(
        &source,
        r#"package app.main

import local_utils.path

fn main() -> void {
    let total: i64 = join(40, 2)
    let segment: Segment = make_segment(total)
}
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("build")
        .arg(&project)
        .arg("--emit-c")
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let cache_version_dir = project.join(".nomo/cache/registry/fynn/utils/0.1.0");
    let cached_source_exists = fs::read_dir(&cache_version_dir)
        .unwrap()
        .any(|entry| entry.unwrap().path().join("source/src/path.nomo").is_file());
    assert!(cached_source_exists);
    let generated_c = fs::read_to_string(c_path).unwrap();
    assert!(generated_c.contains("nomo_pkg_utils_path_fn_join"));
    assert!(!generated_c.contains("nomo_pkg_app_main_fn_join"));
    assert!(generated_c.contains("nomo_pkg_utils_path_struct_Segment"));
    assert!(!generated_c.contains("nomo_pkg_app_main_struct_Segment"));

    let resolve_output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("deps")
        .arg("resolve")
        .arg(&project)
        .output()
        .unwrap();
    assert!(
        resolve_output.status.success(),
        "{}",
        String::from_utf8_lossy(&resolve_output.stderr)
    );
    let lockfile = fs::read_to_string(project.join("nomo.lock")).unwrap();
    assert!(
        lockfile.contains("source = \"registry+file://"),
        "{lockfile}"
    );
    assert!(lockfile.contains("checksum = \"sha256:"), "{lockfile}");

    fs::write(
        &registry_metadata,
        format!(
            "{{\"package\":\"fynn/utils\",\"version\":\"0.1.0\",\"checksum\":\"{archive_checksum}\",\"yanked\":true}}\n"
        ),
    )
    .unwrap();
    let locked_build = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("build")
        .arg(&project)
        .arg("--locked")
        .arg("--emit-c")
        .output()
        .unwrap();
    assert!(
        locked_build.status.success(),
        "{}",
        String::from_utf8_lossy(&locked_build.stderr)
    );

    let vendor_output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("deps")
        .arg("vendor")
        .arg(&project)
        .output()
        .unwrap();
    assert!(
        vendor_output.status.success(),
        "{}",
        String::from_utf8_lossy(&vendor_output.stderr)
    );
    assert!(
        project
            .join("vendor/fynn/utils/0.1.0/src/path.nomo")
            .is_file()
    );

    fs::remove_file(project.join("nomo.lock")).unwrap();
    let fresh_build = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("build")
        .arg(&project)
        .arg("--emit-c")
        .output()
        .unwrap();
    assert!(!fresh_build.status.success());
    let stderr = String::from_utf8_lossy(&fresh_build.stderr);
    assert!(
        stderr.contains("package `fynn/utils` version `0.1.0` is yanked"),
        "{stderr}"
    );

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_project_commands_use_http_registry_dependency_module_public_api() {
    let root = temp_test_root("http-registry-dependency-module-public-api");
    reset_dir(&root);
    let package = root.join("utils");
    let archive_out = root.join("archive-out");
    let project = root.join("hello");
    let nomo_home = root.join("nomo-home");
    let source = project.join("src/main.nomo");
    let c_path = project.join("build/c/main.c");
    fs::create_dir_all(package.join("src")).unwrap();
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        package.join("nomo.toml"),
        "[package]\nnamespace = \"fynn\"\nname = \"utils\"\nversion = \"0.1.0\"\nedition = \"2026\"\n",
    )
    .unwrap();
    fs::write(
        package.join("src/main.nomo"),
        "package utils.main\n\nfn main() -> void {\n}\n",
    )
    .unwrap();
    fs::write(
        package.join("src/path.nomo"),
        r#"package utils.path

pub fn join(a: i64, b: i64) -> i64 {
    return a + b
}
"#,
    )
    .unwrap();

    let publish_output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("publish")
        .arg(&package)
        .arg("--dry-run")
        .arg("--output")
        .arg(&archive_out)
        .output()
        .unwrap();
    assert!(
        publish_output.status.success(),
        "{}",
        String::from_utf8_lossy(&publish_output.stderr)
    );
    let archive = fs::read(archive_out.join("fynn-utils-0.1.0.nomo-package")).unwrap();
    let archive_checksum = nomo_resolver::archive_checksum(&archive);
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let registry_addr = listener.local_addr().unwrap();
    let registry = format!("http://{registry_addr}");
    let login = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("login")
        .arg("--registry")
        .arg(&registry)
        .arg("--token")
        .arg("dependency-token")
        .env("NOMO_HOME", &nomo_home)
        .output()
        .unwrap();
    assert!(
        login.status.success(),
        "{}",
        String::from_utf8_lossy(&login.stderr)
    );
    let server = thread::spawn(move || {
        for request_index in 0..3 {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = Vec::new();
            let mut buffer = [0u8; 1024];
            loop {
                let read = stream.read(&mut buffer).unwrap();
                if read == 0 {
                    break;
                }
                request.extend_from_slice(&buffer[..read]);
                if request.windows(4).any(|window| window == b"\r\n\r\n") {
                    break;
                }
            }
            let request = String::from_utf8_lossy(&request);
            assert_eq!(
                http_header(&request, "Authorization"),
                Some("Bearer dependency-token")
            );
            if request_index != 1 {
                assert!(
                    request.starts_with("GET /api/v1/packages/fynn/utils/0.1.0 HTTP/1.1\r\n"),
                    "{request}"
                );
                assert_eq!(http_header(&request, "Accept"), Some("application/json"));
                let body = format!(
                    "{{\"package\":\"fynn/utils\",\"version\":\"0.1.0\",\"checksum\":\"{archive_checksum}\",\"yanked\":false}}"
                );
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                stream.write_all(response.as_bytes()).unwrap();
            } else {
                assert!(
                    request
                        .starts_with("GET /api/v1/packages/fynn/utils/0.1.0/download HTTP/1.1\r\n"),
                    "{request}"
                );
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    archive.len()
                );
                stream.write_all(response.as_bytes()).unwrap();
                stream.write_all(&archive).unwrap();
            }
        }
    });

    fs::write(
        project.join("nomo.toml"),
        format!(
            "[package]\nnamespace = \"fynn\"\nname = \"hello\"\nversion = \"0.1.0\"\nedition = \"2026\"\n\n[dependencies]\nlocal_utils = {{ package = \"fynn/utils\", version = \"0.1.0\", registry = \"{}\" }}\n",
            registry
        ),
    )
    .unwrap();
    fs::write(
        &source,
        r#"package app.main

import local_utils.path

fn main() -> void {
    let total: i64 = join(40, 2)
}
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("build")
        .arg(&project)
        .arg("--emit-c")
        .env("NOMO_HOME", &nomo_home)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    server.join().unwrap();
    let generated_c = fs::read_to_string(c_path).unwrap();
    assert!(generated_c.contains("nomo_pkg_utils_path_fn_join"));
    assert!(!generated_c.contains("nomo_pkg_app_main_fn_join"));

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_deps_resolve_selects_highest_registry_version_for_range() {
    let root = temp_test_root("registry-range-highest");
    reset_dir(&root);
    let package = root.join("json");
    let registry = root.join("registry");
    let registry_version = registry.join("api/v1/packages/nomo-lang/json/1.9.0");
    let archive_out = root.join("archive-out");
    let project = root.join("app");
    fs::create_dir_all(package.join("src")).unwrap();
    fs::create_dir_all(project.join("src")).unwrap();
    fs::create_dir_all(&registry_version).unwrap();
    fs::write(
        package.join("nomo.toml"),
        "[package]\nnamespace = \"nomo-lang\"\nname = \"json\"\nversion = \"1.9.0\"\nedition = \"2026\"\n",
    )
    .unwrap();
    fs::write(
        package.join("src/main.nomo"),
        "package json.main\n\npub fn version() -> i64 {\n    return 19\n}\n\nfn main() -> void {\n}\n",
    )
    .unwrap();

    let publish = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("publish")
        .arg(&package)
        .arg("--dry-run")
        .arg("--output")
        .arg(&archive_out)
        .output()
        .unwrap();
    assert!(
        publish.status.success(),
        "{}",
        String::from_utf8_lossy(&publish.stderr)
    );
    let archive = fs::read(archive_out.join("nomo-lang-json-1.9.0.nomo-package")).unwrap();
    let checksum = nomo_resolver::archive_checksum(&archive);
    fs::write(registry_version.join("download"), &archive).unwrap();
    fs::write(
        registry_version.join("metadata.json"),
        format!(
            "{{\"package\":\"nomo-lang/json\",\"version\":\"1.9.0\",\"checksum\":\"{checksum}\",\"yanked\":false}}\n"
        ),
    )
    .unwrap();
    fs::write(
        registry_version.parent().unwrap().join("index.json"),
        format!(
            "{{\"package\":\"nomo-lang/json\",\"versions\":[{{\"version\":\"2.0.0\",\"checksum\":\"sha256:{}\",\"yanked\":false}},{{\"version\":\"1.2.0\",\"checksum\":\"sha256:{}\",\"yanked\":false}},{{\"version\":\"1.9.0\",\"checksum\":\"{checksum}\",\"yanked\":false}}]}}\n",
            "2".repeat(64),
            "1".repeat(64)
        ),
    )
    .unwrap();
    fs::write(project.join("src/main.nomo"), "package app.main\n").unwrap();
    fs::write(
        project.join("nomo.toml"),
        format!(
            "[package]\nnamespace = \"fynn\"\nname = \"app\"\nversion = \"0.0.0-20260715120000\"\nedition = \"2026\"\n\n[dependencies]\njson = {{ package = \"nomo-lang/json\", version = \"^1.2.0\", registry = \"file://{}\" }}\n",
            registry.display()
        ),
    )
    .unwrap();

    let resolve = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("deps")
        .arg("resolve")
        .arg(&project)
        .output()
        .unwrap();
    assert!(
        resolve.status.success(),
        "{}",
        String::from_utf8_lossy(&resolve.stderr)
    );
    let lockfile = fs::read_to_string(project.join("nomo.lock")).unwrap();
    assert!(lockfile.contains("version = \"1.9.0\""), "{lockfile}");
    assert!(!lockfile.contains("version = \"^1.2.0\""), "{lockfile}");

    let tree = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("deps")
        .arg("tree")
        .arg(&project)
        .output()
        .unwrap();
    assert!(
        tree.status.success(),
        "{}",
        String::from_utf8_lossy(&tree.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&tree.stdout),
        format!(
            "fynn/app 0.0.0-20260715120000\n+-- json -> nomo-lang/json 1.9.0 (registry file://{})\n",
            registry.display()
        )
    );

    fs::remove_dir_all(&registry).unwrap();
    let locked_offline_tree = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("deps")
        .arg("tree")
        .arg(&project)
        .arg("--locked")
        .arg("--offline")
        .output()
        .unwrap();
    assert!(
        locked_offline_tree.status.success(),
        "{}",
        String::from_utf8_lossy(&locked_offline_tree.stderr)
    );
    assert_eq!(locked_offline_tree.stdout, tree.stdout);

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn nomo_workspace_resolve_selects_one_version_for_all_member_constraints() {
    let root = temp_test_root("workspace-registry-range-single-version");
    reset_dir(&root);
    let package = root.join("json");
    let registry = root.join("registry");
    let archive_out = root.join("archive-out");
    let api = root.join("apps/api");
    let worker = root.join("apps/worker");
    fs::create_dir_all(package.join("src")).unwrap();
    fs::create_dir_all(api.join("src")).unwrap();
    fs::create_dir_all(worker.join("src")).unwrap();
    fs::write(
        package.join("src/main.nomo"),
        "package json.main\n\nfn main() -> void {\n}\n",
    )
    .unwrap();

    let mut versions = Vec::new();
    for version in ["1.4.0", "1.9.0"] {
        fs::write(
            package.join("nomo.toml"),
            format!(
                "[package]\nnamespace = \"nomo-lang\"\nname = \"json\"\nversion = \"{version}\"\nedition = \"2026\"\n"
            ),
        )
        .unwrap();
        let publish = Command::new(env!("CARGO_BIN_EXE_nomo"))
            .arg("publish")
            .arg(&package)
            .arg("--dry-run")
            .arg("--output")
            .arg(&archive_out)
            .output()
            .unwrap();
        assert!(
            publish.status.success(),
            "{}",
            String::from_utf8_lossy(&publish.stderr)
        );
        let archive =
            fs::read(archive_out.join(format!("nomo-lang-json-{version}.nomo-package"))).unwrap();
        let checksum = nomo_resolver::archive_checksum(&archive);
        let registry_version = registry.join(format!("api/v1/packages/nomo-lang/json/{version}"));
        fs::create_dir_all(&registry_version).unwrap();
        fs::write(registry_version.join("download"), archive).unwrap();
        fs::write(
            registry_version.join("metadata.json"),
            format!(
                "{{\"package\":\"nomo-lang/json\",\"version\":\"{version}\",\"checksum\":\"{checksum}\",\"yanked\":false}}\n"
            ),
        )
        .unwrap();
        versions.push((version, checksum));
    }
    let index = registry.join("api/v1/packages/nomo-lang/json/index.json");
    fs::write(
        index,
        format!(
            "{{\"package\":\"nomo-lang/json\",\"versions\":[{{\"version\":\"2.1.0\",\"checksum\":\"sha256:{}\",\"yanked\":false}},{{\"version\":\"{}\",\"checksum\":\"{}\",\"yanked\":false}},{{\"version\":\"{}\",\"checksum\":\"{}\",\"yanked\":false}}]}}\n",
            "2".repeat(64),
            versions[1].0,
            versions[1].1,
            versions[0].0,
            versions[0].1
        ),
    )
    .unwrap();

    fs::write(
        root.join("nomo.toml"),
        "[workspace]\nmembers = [\"apps/*\"]\n",
    )
    .unwrap();
    fs::write(api.join("src/main.nomo"), "package api.main\n").unwrap();
    fs::write(worker.join("src/main.nomo"), "package worker.main\n").unwrap();
    fs::write(
        api.join("nomo.toml"),
        format!(
            "[package]\nnamespace = \"fynn\"\nname = \"api\"\nversion = \"0.1.0\"\nedition = \"2026\"\n\n[dependencies]\njson = {{ package = \"nomo-lang/json\", version = \"^1.0.0\", registry = \"file://{}\" }}\n",
            registry.display()
        ),
    )
    .unwrap();
    fs::write(
        worker.join("nomo.toml"),
        format!(
            "[package]\nnamespace = \"fynn\"\nname = \"worker\"\nversion = \"0.1.0\"\nedition = \"2026\"\n\n[dependencies]\njson = {{ package = \"nomo-lang/json\", version = \">=1.0, <1.5\", registry = \"file://{}\" }}\n",
            registry.display()
        ),
    )
    .unwrap();

    let resolve = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("deps")
        .arg("resolve")
        .arg("--workspace")
        .arg(&root)
        .output()
        .unwrap();
    assert!(
        resolve.status.success(),
        "{}",
        String::from_utf8_lossy(&resolve.stderr)
    );
    let lockfile = fs::read_to_string(root.join("nomo.lock")).unwrap();
    assert!(lockfile.contains("version = \"1.4.0\""), "{lockfile}");
    assert!(!lockfile.contains("version = \"1.9.0\""), "{lockfile}");
    assert_eq!(lockfile.matches("id = \"nomo-lang/json\"").count(), 1);

    let locked_tree = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("deps")
        .arg("tree")
        .arg("--workspace")
        .arg("--locked")
        .arg("--offline")
        .arg(&root)
        .output()
        .unwrap();
    assert!(
        locked_tree.status.success(),
        "{}",
        String::from_utf8_lossy(&locked_tree.stderr)
    );
    let tree = String::from_utf8_lossy(&locked_tree.stdout);
    assert_eq!(tree.matches("nomo-lang/json 1.4.0").count(), 2, "{tree}");

    fs::write(
        worker.join("nomo.toml"),
        format!(
            "[package]\nnamespace = \"fynn\"\nname = \"worker\"\nversion = \"0.1.0\"\nedition = \"2026\"\n\n[dependencies]\njson = {{ package = \"nomo-lang/json\", version = \"^2.0.0\", registry = \"file://{}\" }}\n",
            registry.display()
        ),
    )
    .unwrap();
    let conflict = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("deps")
        .arg("resolve")
        .arg("--workspace")
        .arg(&root)
        .output()
        .unwrap();
    assert!(!conflict.status.success());
    let stderr = String::from_utf8_lossy(&conflict.stderr);
    assert!(
        stderr.contains("`^1.0.0` required by fynn/api -> nomo-lang/json"),
        "{stderr}"
    );
    assert!(
        stderr.contains("`^2.0.0` required by fynn/worker -> nomo-lang/json"),
        "{stderr}"
    );
    assert!(
        stderr.contains("available versions: 1.4.0, 1.9.0, 2.1.0"),
        "{stderr}"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn nomo_deps_resolve_reuses_cached_http_registry_range_offline() {
    let root = temp_test_root("registry-range-http-offline");
    reset_dir(&root);
    let package = root.join("json");
    let archive_out = root.join("archive-out");
    let project = root.join("app");
    fs::create_dir_all(package.join("src")).unwrap();
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        package.join("nomo.toml"),
        "[package]\nnamespace = \"nomo-lang\"\nname = \"json\"\nversion = \"1.9.0\"\nedition = \"2026\"\n",
    )
    .unwrap();
    fs::write(
        package.join("src/main.nomo"),
        "package json.main\n\nfn main() -> void {\n}\n",
    )
    .unwrap();
    let publish = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("publish")
        .arg(&package)
        .arg("--dry-run")
        .arg("--output")
        .arg(&archive_out)
        .output()
        .unwrap();
    assert!(
        publish.status.success(),
        "{}",
        String::from_utf8_lossy(&publish.stderr)
    );
    let archive = fs::read(archive_out.join("nomo-lang-json-1.9.0.nomo-package")).unwrap();
    let checksum = nomo_resolver::archive_checksum(&archive);

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let registry = format!("http://{}", listener.local_addr().unwrap());
    let server_checksum = checksum.clone();
    let server = thread::spawn(move || {
        for request_index in 0..3 {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = Vec::new();
            let mut buffer = [0u8; 1024];
            loop {
                let read = stream.read(&mut buffer).unwrap();
                if read == 0 {
                    break;
                }
                request.extend_from_slice(&buffer[..read]);
                if request.windows(4).any(|window| window == b"\r\n\r\n") {
                    break;
                }
            }
            let request = String::from_utf8_lossy(&request);
            let (expected_path, content_type, body) = match request_index {
                0 => (
                    "/api/v1/packages/nomo-lang/json",
                    "application/json",
                    format!(
                        "{{\"package\":\"nomo-lang/json\",\"versions\":[{{\"version\":\"1.9.0\",\"checksum\":\"{server_checksum}\",\"yanked\":false}}]}}"
                    )
                    .into_bytes(),
                ),
                1 => (
                    "/api/v1/packages/nomo-lang/json/1.9.0",
                    "application/json",
                    format!(
                        "{{\"package\":\"nomo-lang/json\",\"version\":\"1.9.0\",\"checksum\":\"{server_checksum}\",\"yanked\":false}}"
                    )
                    .into_bytes(),
                ),
                _ => (
                    "/api/v1/packages/nomo-lang/json/1.9.0/download",
                    "application/octet-stream",
                    archive.clone(),
                ),
            };
            assert!(
                request.starts_with(&format!("GET {expected_path} HTTP/1.1\r\n")),
                "{request}"
            );
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            stream.write_all(response.as_bytes()).unwrap();
            stream.write_all(&body).unwrap();
        }
    });

    fs::write(project.join("src/main.nomo"), "package app.main\n").unwrap();
    fs::write(
        project.join("nomo.toml"),
        format!(
            "[package]\nnamespace = \"fynn\"\nname = \"app\"\nversion = \"0.1.0\"\nedition = \"2026\"\n\n[dependencies]\njson = {{ package = \"nomo-lang/json\", version = \"^1.2.0\", registry = \"{registry}\" }}\n"
        ),
    )
    .unwrap();

    let online = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("deps")
        .arg("resolve")
        .arg(&project)
        .output()
        .unwrap();
    assert!(
        online.status.success(),
        "{}",
        String::from_utf8_lossy(&online.stderr)
    );
    server.join().unwrap();
    fs::remove_file(project.join("nomo.lock")).unwrap();

    let offline = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("deps")
        .arg("resolve")
        .arg(&project)
        .arg("--offline")
        .output()
        .unwrap();
    assert!(
        offline.status.success(),
        "{}",
        String::from_utf8_lossy(&offline.stderr)
    );
    let lockfile = fs::read_to_string(project.join("nomo.lock")).unwrap();
    assert!(lockfile.contains("version = \"1.9.0\""), "{lockfile}");

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn nomo_deps_update_precise_rewrites_registry_lockfile() {
    let root = temp_test_root("deps-update-precise-registry");
    reset_dir(&root);
    let project = root.join("hello");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(project.join("src/main.nomo"), "package app.main\n").unwrap();
    let manifest = "[package]\nnamespace = \"fynn\"\nname = \"hello\"\nversion = \"0.1.0\"\nedition = \"2026\"\n\n[dependencies]\njson = { package = \"nomo-lang/json\", version = \">=0.1.0, <0.3.0\", registry = \"https://packages.nomo.test\" }\n";
    fs::write(project.join("nomo.toml"), manifest).unwrap();

    let update_output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("deps")
        .arg("update")
        .arg(&project)
        .arg("json")
        .arg("--precise")
        .arg("0.2.0")
        .arg("--offline")
        .output()
        .unwrap();

    assert!(
        update_output.status.success(),
        "{}",
        String::from_utf8_lossy(&update_output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&update_output.stdout),
        format!("updated {}\n", project.join("nomo.lock").display())
    );
    assert_eq!(
        fs::read_to_string(project.join("nomo.toml")).unwrap(),
        manifest
    );
    let lockfile = fs::read_to_string(project.join("nomo.lock")).unwrap();
    assert!(lockfile.contains("version = \"0.2.0\""), "{lockfile}");
    assert!(
        lockfile.contains("source = \"registry+https://packages.nomo.test\""),
        "{lockfile}"
    );
    assert!(
        !lockfile.contains("version = \">=0.1.0, <0.3.0\""),
        "{lockfile}"
    );

    let locked_tree = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("deps")
        .arg("tree")
        .arg(&project)
        .arg("--locked")
        .arg("--offline")
        .output()
        .unwrap();
    assert!(
        locked_tree.status.success(),
        "{}",
        String::from_utf8_lossy(&locked_tree.stderr)
    );

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_deps_update_precise_rejects_version_outside_manifest_requirement() {
    let root = temp_test_root("deps-update-precise-outside-range");
    reset_dir(&root);
    let project = root.join("hello");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(project.join("src/main.nomo"), "package app.main\n").unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nnamespace = \"fynn\"\nname = \"hello\"\nversion = \"0.1.0\"\nedition = \"2026\"\n\n[dependencies]\njson = { package = \"nomo-lang/json\", version = \"^1.2.0\", registry = \"https://packages.nomo.test\" }\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("deps")
        .arg("update")
        .arg(&project)
        .arg("json")
        .arg("--precise")
        .arg("2.0.0")
        .arg("--offline")
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(
            "dependency `json` precise version `2.0.0` does not satisfy manifest requirement `^1.2.0`"
        ),
        "{stderr}"
    );
    assert!(!project.join("nomo.lock").exists());

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn nomo_deps_update_precise_rewrites_git_lockfile_to_rev() {
    let root = temp_test_root("deps-update-precise-git");
    reset_dir(&root);
    let project = root.join("hello");
    let json = root.join("json");
    init_git_package(&json, "nomo-lang", "json");
    run_git(&json, &["checkout", "--quiet", "-b", "stable"]);
    fs::write(json.join("src/main.nomo"), "package json.main\n\n").unwrap();
    run_git(&json, &["add", "src/main.nomo"]);
    run_git(&json, &["commit", "--quiet", "-m", "stable branch"]);
    let first_rev = git_head_rev(&json);
    fs::write(
        json.join("src/main.nomo"),
        "package json.main\n\npub fn version() -> i64 {\n    return 2\n}\n",
    )
    .unwrap();
    run_git(&json, &["add", "src/main.nomo"]);
    run_git(&json, &["commit", "--quiet", "-m", "stable update"]);
    let second_rev = git_head_rev(&json);
    assert_ne!(first_rev, second_rev);
    run_git(&json, &["checkout", "--quiet", &first_rev]);

    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(project.join("src/main.nomo"), "package app.main\n").unwrap();
    fs::write(
        project.join("nomo.toml"),
        format!(
            "[package]\nnamespace = \"fynn\"\nname = \"hello\"\nversion = \"0.1.0\"\nedition = \"2026\"\n\n[dependencies]\njson = {{ package = \"nomo-lang/json\", git = \"{}\", branch = \"stable\" }}\n",
            json.display()
        ),
    )
    .unwrap();

    let update_output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("deps")
        .arg("update")
        .arg(&project)
        .arg("nomo-lang/json")
        .arg(format!("--precise={second_rev}"))
        .output()
        .unwrap();

    assert!(
        update_output.status.success(),
        "{}",
        String::from_utf8_lossy(&update_output.stderr)
    );
    let lockfile = fs::read_to_string(project.join("nomo.lock")).unwrap();
    assert!(
        lockfile.contains(&format!("rev = \"{second_rev}\"")),
        "{lockfile}"
    );
    assert!(
        !lockfile.contains(&format!("rev = \"{first_rev}\"")),
        "{lockfile}"
    );
    assert!(!lockfile.contains("branch = \"stable\""), "{lockfile}");
    assert!(
        fs::read_to_string(project.join("nomo.toml"))
            .unwrap()
            .contains("branch = \"stable\"")
    );

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_deps_update_precise_requires_target() {
    let root = temp_test_root("deps-update-precise-requires-target");
    reset_dir(&root);
    let project = root.join("hello");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(project.join("src/main.nomo"), "package app.main\n").unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nnamespace = \"fynn\"\nname = \"hello\"\nversion = \"0.1.0\"\nedition = \"2026\"\n\n[dependencies]\njson = { package = \"nomo-lang/json\", version = \"0.1.0\" }\n",
    )
    .unwrap();

    let update_output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("deps")
        .arg("update")
        .arg(&project)
        .arg("--precise")
        .arg("0.2.0")
        .output()
        .unwrap();

    assert!(!update_output.status.success());
    let stderr = String::from_utf8_lossy(&update_output.stderr);
    assert!(stderr.contains("--precise requires"), "{stderr}");
    assert!(!project.join("nomo.lock").exists());

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_deps_update_precise_rejects_path_dependency() {
    let root = temp_test_root("deps-update-precise-path");
    reset_dir(&root);
    let app = root.join("app");
    let utils = root.join("utils");
    fs::create_dir_all(app.join("src")).unwrap();
    fs::create_dir_all(utils.join("src")).unwrap();
    fs::write(app.join("src/main.nomo"), "package app.main\n").unwrap();
    fs::write(utils.join("src/main.nomo"), "package utils.main\n").unwrap();
    fs::write(
        app.join("nomo.toml"),
        "[package]\nnamespace = \"fynn\"\nname = \"app\"\nversion = \"0.1.0\"\nedition = \"2026\"\n\n[dependencies]\nlocal_utils = { package = \"fynn/utils\", path = \"../utils\" }\n",
    )
    .unwrap();
    fs::write(
        utils.join("nomo.toml"),
        "[package]\nnamespace = \"fynn\"\nname = \"utils\"\nversion = \"0.1.0\"\nedition = \"2026\"\n",
    )
    .unwrap();

    let update_output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("deps")
        .arg("update")
        .arg(&app)
        .arg("local_utils")
        .arg("--precise")
        .arg("0.2.0")
        .output()
        .unwrap();

    assert!(!update_output.status.success());
    let stderr = String::from_utf8_lossy(&update_output.stderr);
    assert!(
        stderr.contains("cannot be updated with --precise"),
        "{stderr}"
    );
    assert!(!app.join("nomo.lock").exists());

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_deps_update_rejects_unknown_target() {
    let root = temp_test_root("deps-update-unknown-target");
    reset_dir(&root);
    let project = root.join("hello");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(project.join("src/main.nomo"), "package app.main\n").unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nnamespace = \"fynn\"\nname = \"hello\"\nversion = \"0.1.0\"\nedition = \"2026\"\n\n[dependencies]\njson = { package = \"nomo-lang/json\", version = \"0.1.0\" }\n",
    )
    .unwrap();

    let update_output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("deps")
        .arg("update")
        .arg(&project)
        .arg("missing")
        .output()
        .unwrap();

    assert!(!update_output.status.success());
    let stderr = String::from_utf8_lossy(&update_output.stderr);
    assert!(stderr.contains("is not a direct dependency"), "{stderr}");
    assert!(!project.join("nomo.lock").exists());

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_deps_clean_cache_removes_git_cache() {
    let root = temp_test_root("deps-clean-cache");
    reset_dir(&root);
    let project = root.join("hello");
    let json = root.join("json");
    let json_rev = init_git_package(&json, "nomo-lang", "json");

    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(project.join("src/main.nomo"), "package app.main\n").unwrap();
    fs::write(
        project.join("nomo.toml"),
        format!(
            "[package]\nnamespace = \"fynn\"\nname = \"hello\"\nversion = \"0.1.0\"\nedition = \"2026\"\n\n[dependencies]\njson = {{ package = \"nomo-lang/json\", git = \"{}\", rev = \"{}\" }}\n",
            json.display(),
            json_rev
        ),
    )
    .unwrap();

    let resolve_output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("deps")
        .arg("resolve")
        .arg(&project)
        .output()
        .unwrap();
    assert!(
        resolve_output.status.success(),
        "{}",
        String::from_utf8_lossy(&resolve_output.stderr)
    );
    let cache_root = project.join(".nomo/deps/git");
    assert!(cache_root.exists());

    let clean_output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("deps")
        .arg("clean-cache")
        .arg(&project)
        .output()
        .unwrap();
    assert!(
        clean_output.status.success(),
        "{}",
        String::from_utf8_lossy(&clean_output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&clean_output.stdout),
        format!("cleaned {}\n", cache_root.display())
    );
    assert!(!cache_root.exists());

    let second_clean = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("deps")
        .arg("clean-cache")
        .arg(&project)
        .output()
        .unwrap();
    assert!(
        second_clean.status.success(),
        "{}",
        String::from_utf8_lossy(&second_clean.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&second_clean.stdout),
        format!("cleaned {}\n", cache_root.display())
    );

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_deps_vendor_copies_locked_path_and_git_sources() {
    let root = temp_test_root("deps-vendor");
    reset_dir(&root);
    let project = root.join("hello");
    let utils = root.join("utils");
    let json = root.join("json");
    let json_rev = init_git_package_with_source(
        &json,
        "nomo-lang",
        "json",
        "package json.main\n\npub fn version() -> i64 {\n    return 1\n}\n",
    );
    fs::create_dir_all(project.join("src")).unwrap();
    fs::create_dir_all(utils.join("src")).unwrap();
    fs::create_dir_all(utils.join("native")).unwrap();
    fs::write(project.join("src/main.nomo"), "package app.main\n").unwrap();
    fs::write(utils.join("src/main.nomo"), "package utils.main\n").unwrap();
    fs::write(
        utils.join("nomo.toml"),
        "[package]\nnamespace = \"fynn\"\nname = \"utils\"\nversion = \"0.1.0\"\nedition = \"2026\"\n\n[ffi]\nsources = [\"native/bridge.c\"]\n",
    )
    .unwrap();
    fs::write(utils.join("native/bridge.c"), "void bridge(void) {}\n").unwrap();
    fs::write(
        project.join("nomo.toml"),
        format!(
            "[package]\nnamespace = \"fynn\"\nname = \"hello\"\nversion = \"0.1.0\"\nedition = \"2026\"\n\n[dependencies]\njson = {{ package = \"nomo-lang/json\", git = \"{}\", rev = \"{}\" }}\nlocal_utils = {{ package = \"fynn/utils\", path = \"../utils\" }}\n",
            json.display(),
            json_rev
        ),
    )
    .unwrap();
    fs::create_dir_all(project.join("vendor")).unwrap();
    fs::write(project.join("vendor/stale.txt"), "stale\n").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("deps")
        .arg("vendor")
        .arg(&project)
        .arg("--sync")
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        format!("vendored {}\n", project.join("vendor").display())
    );
    assert!(project.join("nomo.lock").exists());
    assert!(!project.join("vendor/stale.txt").exists());
    assert!(
        project
            .join(format!(
                "vendor/nomo-lang/json/git-{}/nomo.toml",
                &json_rev[..12]
            ))
            .exists()
    );
    assert!(project.join("vendor/fynn/utils/path/nomo.toml").exists());
    assert!(
        project
            .join("vendor/fynn/utils/path/native/bridge.c")
            .exists()
    );
    assert!(
        !project
            .join(format!(
                "vendor/nomo-lang/json/git-{}/.git",
                &json_rev[..12]
            ))
            .exists()
    );
    let vendor_manifest = fs::read_to_string(project.join("vendor/nomo-vendor.toml")).unwrap();
    assert!(
        vendor_manifest.contains(&format!("source = \"git+{}\"", json.display())),
        "{vendor_manifest}"
    );
    assert!(
        vendor_manifest.contains("path = \"nomo-lang/json/git-"),
        "{vendor_manifest}"
    );
    assert!(
        vendor_manifest.contains("source = \"path+../utils\""),
        "{vendor_manifest}"
    );
    assert_checksum_lines(&vendor_manifest, 2);

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_build_offline_uses_vendored_git_source_when_cache_is_missing() {
    let root = temp_test_root("deps-vendor-offline-build");
    reset_dir(&root);
    let project = root.join("hello");
    let json = root.join("json");
    let _initial_json_rev =
        init_git_package_with_source(&json, "nomo-lang", "json", "package json.main\n\n");
    fs::write(
        json.join("src/path.nomo"),
        "package json.path\n\npub fn add(a: i64, b: i64) -> i64 {\n    return a + b\n}\n",
    )
    .unwrap();
    run_git(&json, &["add", "src/path.nomo"]);
    run_git(&json, &["commit", "--quiet", "-m", "add path module"]);
    let json_rev = git_head_rev(&json);

    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("src/main.nomo"),
        "package app.main\n\nimport json.path\n\nfn main() -> void {\n    let total: i64 = add(40, 2)\n}\n",
    )
    .unwrap();
    fs::write(
        project.join("nomo.toml"),
        format!(
            "[package]\nnamespace = \"fynn\"\nname = \"hello\"\nversion = \"0.1.0\"\nedition = \"2026\"\n\n[dependencies]\njson = {{ package = \"nomo-lang/json\", git = \"{}\", rev = \"{}\" }}\n",
            json.display(),
            json_rev
        ),
    )
    .unwrap();

    let vendor_output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("deps")
        .arg("vendor")
        .arg(&project)
        .output()
        .unwrap();
    assert!(
        vendor_output.status.success(),
        "{}",
        String::from_utf8_lossy(&vendor_output.stderr)
    );

    let clean_output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("deps")
        .arg("clean-cache")
        .arg(&project)
        .output()
        .unwrap();
    assert!(
        clean_output.status.success(),
        "{}",
        String::from_utf8_lossy(&clean_output.stderr)
    );

    let build_output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("build")
        .arg(&project)
        .arg("--offline")
        .arg("--emit-c")
        .output()
        .unwrap();
    assert!(
        build_output.status.success(),
        "{}",
        String::from_utf8_lossy(&build_output.stderr)
    );
    assert!(project.join("build/c/main.c").exists());

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_deps_resolve_locks_git_tag_to_head_rev() {
    let root = temp_test_root("deps-git-tag");
    reset_dir(&root);
    let project = root.join("hello");
    let json = root.join("json");
    init_git_package(&json, "nomo-lang", "json");
    run_git(&json, &["tag", "v0.1.0"]);
    let tag_rev = git_head_rev(&json);

    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(project.join("src/main.nomo"), "package app.main\n").unwrap();
    fs::write(
        project.join("nomo.toml"),
        format!(
            "[package]\nnamespace = \"fynn\"\nname = \"hello\"\nversion = \"0.1.0\"\nedition = \"2026\"\n\n[dependencies]\njson = {{ package = \"nomo-lang/json\", git = \"{}\", tag = \"v0.1.0\" }}\n",
            json.display()
        ),
    )
    .unwrap();

    let resolve_output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("deps")
        .arg("resolve")
        .arg(&project)
        .output()
        .unwrap();

    assert!(
        resolve_output.status.success(),
        "{}",
        String::from_utf8_lossy(&resolve_output.stderr)
    );
    let lockfile = fs::read_to_string(project.join("nomo.lock")).unwrap();
    assert_checksum_lines(&lockfile, 1);
    assert_eq!(
        strip_checksum_lines(&lockfile),
        format!(
            "# This file is generated by `nomo deps resolve`.\n\n[[package]]\nid = \"nomo-lang/json\"\nalias = \"json\"\nsource = \"git+{}\"\ntag = \"v0.1.0\"\nrev = \"{}\"\n",
            json.display(),
            tag_rev
        )
    );

    let tree_output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("deps")
        .arg("tree")
        .arg(&project)
        .output()
        .unwrap();

    assert!(
        tree_output.status.success(),
        "{}",
        String::from_utf8_lossy(&tree_output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&tree_output.stdout),
        format!(
            "fynn/hello 0.1.0\n+-- json -> nomo-lang/json (git {}@v0.1.0#{})\n",
            json.display(),
            tag_rev
        )
    );

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_deps_tree_rejects_stale_git_checksum_when_cache_exists() {
    let root = temp_test_root("deps-tree-stale-git-checksum");
    reset_dir(&root);
    let project = root.join("hello");
    let json = root.join("json");
    let json_rev = init_git_package(&json, "nomo-lang", "json");

    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(project.join("src/main.nomo"), "package app.main\n").unwrap();
    fs::write(
        project.join("nomo.toml"),
        format!(
            "[package]\nnamespace = \"fynn\"\nname = \"hello\"\nversion = \"0.1.0\"\nedition = \"2026\"\n\n[dependencies]\njson = {{ package = \"nomo-lang/json\", git = \"{}\", rev = \"{}\" }}\n",
            json.display(),
            json_rev
        ),
    )
    .unwrap();

    let resolve_output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("deps")
        .arg("resolve")
        .arg(&project)
        .output()
        .unwrap();
    assert!(
        resolve_output.status.success(),
        "{}",
        String::from_utf8_lossy(&resolve_output.stderr)
    );
    let checkout = find_git_cache_checkout(&project, "json");
    fs::write(
        checkout.join("src/main.nomo"),
        "package json.main\n\nfn changed() -> void {}\n",
    )
    .unwrap();

    let tree_output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("deps")
        .arg("tree")
        .arg(&project)
        .output()
        .unwrap();

    assert!(!tree_output.status.success());
    assert!(tree_output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&tree_output.stderr);
    assert!(stderr.contains("checksum mismatch"), "{stderr}");
    assert!(stderr.contains("nomo-lang/json"), "{stderr}");

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_deps_resolve_and_tree_include_transitive_path_dependencies() {
    let root = temp_test_root("deps-transitive-path");
    reset_dir(&root);
    let app = root.join("app");
    let utils = root.join("utils");
    fs::create_dir_all(app.join("src")).unwrap();
    fs::create_dir_all(utils.join("src")).unwrap();
    fs::write(app.join("src/main.nomo"), "package app.main\n").unwrap();
    fs::write(utils.join("src/main.nomo"), "package utils.main\n").unwrap();
    fs::write(
        app.join("nomo.toml"),
        "[package]\nnamespace = \"fynn\"\nname = \"app\"\nversion = \"0.1.0\"\nedition = \"2026\"\n\n[dependencies]\nlocal_utils = { package = \"fynn/utils\", path = \"../utils\" }\n",
    )
    .unwrap();
    fs::write(
        utils.join("nomo.toml"),
        "[package]\nnamespace = \"fynn\"\nname = \"utils\"\nversion = \"0.1.0\"\nedition = \"2026\"\n\n[dependencies]\ncli = { package = \"nomo-lang/cli\", version = \"0.2.1\" }\n",
    )
    .unwrap();

    let resolve_output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("deps")
        .arg("resolve")
        .arg(&app)
        .output()
        .unwrap();

    assert!(
        resolve_output.status.success(),
        "{}",
        String::from_utf8_lossy(&resolve_output.stderr)
    );
    let lockfile = fs::read_to_string(app.join("nomo.lock")).unwrap();
    assert_checksum_lines(&lockfile, 1);
    assert_eq!(
        strip_checksum_lines(&lockfile),
        "# This file is generated by `nomo deps resolve`.\n\n[[package]]\nid = \"fynn/utils\"\nalias = \"local_utils\"\nsource = \"path+../utils\"\ndependencies = [\"cli -> nomo-lang/cli\"]\n\n[[package]]\nid = \"nomo-lang/cli\"\nalias = \"cli\"\nversion = \"0.2.1\"\nsource = \"registry+nomo-lang/cli\"\n"
    );

    let tree_output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("deps")
        .arg("tree")
        .arg(&app)
        .output()
        .unwrap();

    assert!(
        tree_output.status.success(),
        "{}",
        String::from_utf8_lossy(&tree_output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&tree_output.stdout),
        "fynn/app 0.1.0\n+-- local_utils -> fynn/utils (path ../utils)\n    +-- cli -> nomo-lang/cli 0.2.1 (registry)\n"
    );

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_deps_resolve_reports_full_package_cycle() {
    let root = temp_test_root("deps-path-cycle");
    reset_dir(&root);
    let app = root.join("app");
    let utils = root.join("utils");
    fs::create_dir_all(app.join("src")).unwrap();
    fs::create_dir_all(utils.join("src")).unwrap();
    fs::write(app.join("src/main.nomo"), "package app.main\n").unwrap();
    fs::write(utils.join("src/main.nomo"), "package utils.main\n").unwrap();
    fs::write(
        app.join("nomo.toml"),
        "[package]\nnamespace = \"fynn\"\nname = \"app\"\nversion = \"0.1.0\"\nedition = \"2026\"\n\n[dependencies]\nutils = { package = \"fynn/utils\", path = \"../utils\" }\n",
    )
    .unwrap();
    fs::write(
        utils.join("nomo.toml"),
        "[package]\nnamespace = \"fynn\"\nname = \"utils\"\nversion = \"0.1.0\"\nedition = \"2026\"\n\n[dependencies]\napp = { package = \"fynn/app\", path = \"../app\" }\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("deps")
        .arg("resolve")
        .arg(&app)
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("cyclic package dependency: fynn/app -> fynn/utils -> fynn/app"),
        "{stderr}"
    );
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_deps_tree_reads_existing_lockfile() {
    let root = temp_test_root("deps-tree-lockfile");
    reset_dir(&root);
    let app = root.join("app");
    let utils = root.join("utils");
    fs::create_dir_all(app.join("src")).unwrap();
    fs::create_dir_all(utils.join("src")).unwrap();
    fs::write(app.join("src/main.nomo"), "package app.main\n").unwrap();
    fs::write(utils.join("src/main.nomo"), "package utils.main\n").unwrap();
    fs::write(
        app.join("nomo.toml"),
        "[package]\nnamespace = \"fynn\"\nname = \"app\"\nversion = \"0.1.0\"\nedition = \"2026\"\n\n[dependencies]\nlocal_utils = { package = \"fynn/utils\", path = \"../utils\" }\n",
    )
    .unwrap();
    fs::write(
        utils.join("nomo.toml"),
        "[package]\nnamespace = \"fynn\"\nname = \"utils\"\nversion = \"0.1.0\"\nedition = \"2026\"\n\n[dependencies]\ncli = { package = \"nomo-lang/cli\", version = \"0.2.1\" }\n",
    )
    .unwrap();

    let resolve_output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("deps")
        .arg("resolve")
        .arg(&app)
        .output()
        .unwrap();
    assert!(
        resolve_output.status.success(),
        "{}",
        String::from_utf8_lossy(&resolve_output.stderr)
    );
    fs::remove_dir_all(&utils).unwrap();

    let tree_output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("deps")
        .arg("tree")
        .arg(&app)
        .output()
        .unwrap();

    assert!(
        tree_output.status.success(),
        "{}",
        String::from_utf8_lossy(&tree_output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&tree_output.stdout),
        "fynn/app 0.1.0\n+-- local_utils -> fynn/utils (path ../utils)\n    +-- cli -> nomo-lang/cli 0.2.1 (registry)\n"
    );

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_locked_flags_require_and_validate_lockfile() {
    let root = temp_test_root("locked-flags");
    reset_dir(&root);
    let app = root.join("app");
    fs::create_dir_all(app.join("src")).unwrap();
    fs::write(
        app.join("src/main.nomo"),
        "package app.main\n\nfn main() -> void {\n}\n",
    )
    .unwrap();
    fs::write(
        app.join("nomo.toml"),
        "[package]\nnamespace = \"fynn\"\nname = \"app\"\nversion = \"0.1.0\"\nedition = \"2026\"\n\n[dependencies]\njson = { package = \"nomo-lang/json\", version = \"0.1.0\" }\n",
    )
    .unwrap();

    let missing_build = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("build")
        .arg("--locked")
        .arg("--emit-c")
        .arg(&app)
        .output()
        .unwrap();
    assert!(!missing_build.status.success());
    let stderr = String::from_utf8_lossy(&missing_build.stderr);
    assert!(stderr.contains("nomo.lock is required"), "{stderr}");

    let missing_frozen_build = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("build")
        .arg("--frozen")
        .arg("--emit-c")
        .arg(&app)
        .output()
        .unwrap();
    assert!(!missing_frozen_build.status.success());
    let stderr = String::from_utf8_lossy(&missing_frozen_build.stderr);
    assert!(stderr.contains("nomo.lock is required"), "{stderr}");

    let missing_tree = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("deps")
        .arg("tree")
        .arg("--locked")
        .arg(&app)
        .output()
        .unwrap();
    assert!(!missing_tree.status.success());
    let stderr = String::from_utf8_lossy(&missing_tree.stderr);
    assert!(stderr.contains("nomo.lock is required"), "{stderr}");

    let resolve_output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("deps")
        .arg("resolve")
        .arg(&app)
        .output()
        .unwrap();
    assert!(
        resolve_output.status.success(),
        "{}",
        String::from_utf8_lossy(&resolve_output.stderr)
    );

    let locked_build = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("build")
        .arg("--locked")
        .arg("--emit-c")
        .arg(&app)
        .output()
        .unwrap();
    assert!(
        locked_build.status.success(),
        "{}",
        String::from_utf8_lossy(&locked_build.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&locked_build.stdout),
        format!("built {}\n", app.join("build/c/main.c").display())
    );

    let frozen_build = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("build")
        .arg("--frozen")
        .arg("--emit-c")
        .arg(&app)
        .output()
        .unwrap();
    assert!(
        frozen_build.status.success(),
        "{}",
        String::from_utf8_lossy(&frozen_build.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&frozen_build.stdout),
        format!("built {}\n", app.join("build/c/main.c").display())
    );

    let frozen_tree = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("deps")
        .arg("tree")
        .arg("--frozen")
        .arg(&app)
        .output()
        .unwrap();
    assert!(
        frozen_tree.status.success(),
        "{}",
        String::from_utf8_lossy(&frozen_tree.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&frozen_tree.stdout),
        "fynn/app 0.1.0\n+-- json -> nomo-lang/json 0.1.0 (registry)\n"
    );

    fs::write(
        app.join("nomo.toml"),
        "[package]\nnamespace = \"fynn\"\nname = \"app\"\nversion = \"0.1.0\"\nedition = \"2026\"\n\n[dependencies]\njson = { package = \"nomo-lang/json\", version = \"0.2.0\" }\n",
    )
    .unwrap();

    let stale_resolve = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("deps")
        .arg("resolve")
        .arg("--locked")
        .arg(&app)
        .output()
        .unwrap();
    assert!(!stale_resolve.status.success());
    let stderr = String::from_utf8_lossy(&stale_resolve.stderr);
    assert!(stderr.contains("nomo.lock is out of date"), "{stderr}");

    let stale_frozen_resolve = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("deps")
        .arg("resolve")
        .arg("--frozen")
        .arg(&app)
        .output()
        .unwrap();
    assert!(!stale_frozen_resolve.status.success());
    let stderr = String::from_utf8_lossy(&stale_frozen_resolve.stderr);
    assert!(stderr.contains("nomo.lock is out of date"), "{stderr}");

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_offline_resolve_rejects_uncached_git_dependency() {
    let root = temp_test_root("offline-git-missing-cache");
    reset_dir(&root);
    let app = root.join("app");
    let json = root.join("json");
    let json_rev = init_git_package(&json, "nomo-lang", "json");
    fs::create_dir_all(app.join("src")).unwrap();
    fs::write(app.join("src/main.nomo"), "package app.main\n").unwrap();
    fs::write(
        app.join("nomo.toml"),
        format!(
            "[package]\nnamespace = \"fynn\"\nname = \"app\"\nversion = \"0.1.0\"\nedition = \"2026\"\n\n[dependencies]\njson = {{ package = \"nomo-lang/json\", git = \"{}\", rev = \"{}\" }}\n",
            json.display(),
            json_rev
        ),
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("deps")
        .arg("resolve")
        .arg("--offline")
        .arg(&app)
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("offline mode cannot fetch git dependency"),
        "{stderr}"
    );
    assert!(!app.join("nomo.lock").exists());

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_deps_tree_rejects_stale_path_checksum_when_source_exists() {
    let root = temp_test_root("deps-tree-stale-checksum");
    reset_dir(&root);
    let app = root.join("app");
    let utils = root.join("utils");
    fs::create_dir_all(app.join("src")).unwrap();
    fs::create_dir_all(utils.join("src")).unwrap();
    fs::write(app.join("src/main.nomo"), "package app.main\n").unwrap();
    fs::write(utils.join("src/main.nomo"), "package utils.main\n").unwrap();
    fs::write(
        app.join("nomo.toml"),
        "[package]\nnamespace = \"fynn\"\nname = \"app\"\nversion = \"0.1.0\"\nedition = \"2026\"\n\n[dependencies]\nlocal_utils = { package = \"fynn/utils\", path = \"../utils\" }\n",
    )
    .unwrap();
    fs::write(
        utils.join("nomo.toml"),
        "[package]\nnamespace = \"fynn\"\nname = \"utils\"\nversion = \"0.1.0\"\nedition = \"2026\"\n",
    )
    .unwrap();

    let resolve_output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("deps")
        .arg("resolve")
        .arg(&app)
        .output()
        .unwrap();
    assert!(
        resolve_output.status.success(),
        "{}",
        String::from_utf8_lossy(&resolve_output.stderr)
    );
    fs::write(
        utils.join("src/main.nomo"),
        "package utils.main\n\nfn changed() -> void {}\n",
    )
    .unwrap();

    let tree_output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("deps")
        .arg("tree")
        .arg(&app)
        .output()
        .unwrap();

    assert!(!tree_output.status.success());
    assert!(tree_output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&tree_output.stderr);
    assert!(stderr.contains("checksum mismatch"), "{stderr}");
    assert!(stderr.contains("fynn/utils"), "{stderr}");

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_deps_rejects_dependency_with_multiple_sources() {
    let root = temp_test_root("deps-multiple-sources");
    reset_dir(&root);
    let project = root.join("hello");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(project.join("src/main.nomo"), "package app.main\n").unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nnamespace = \"fynn\"\nname = \"hello\"\nversion = \"0.1.0\"\nedition = \"2026\"\n\n[dependencies]\nutils = { package = \"fynn/utils\", path = \"../utils\", version = \"0.1.0\" }\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("deps")
        .arg("resolve")
        .arg(&project)
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("exactly one source"), "{stderr}");
    assert!(!project.join("nomo.lock").exists());

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_deps_resolve_reports_conflicting_version_requirements() {
    let root = temp_test_root("deps-conflict");
    reset_dir(&root);
    let app = root.join("app");
    let utils = root.join("utils");
    fs::create_dir_all(app.join("src")).unwrap();
    fs::create_dir_all(utils.join("src")).unwrap();
    fs::write(app.join("src/main.nomo"), "package app.main\n").unwrap();
    fs::write(utils.join("src/main.nomo"), "package utils.main\n").unwrap();
    fs::write(
        app.join("nomo.toml"),
        "[package]\nnamespace = \"fynn\"\nname = \"app\"\nversion = \"0.1.0\"\nedition = \"2026\"\n\n[dependencies]\nlocal_utils = { package = \"fynn/utils\", path = \"../utils\" }\ncli = { package = \"nomo-lang/cli\", version = \"0.2.0\" }\n",
    )
    .unwrap();
    fs::write(
        utils.join("nomo.toml"),
        "[package]\nnamespace = \"fynn\"\nname = \"utils\"\nversion = \"0.1.0\"\nedition = \"2026\"\n\n[dependencies]\ncli = { package = \"nomo-lang/cli\", version = \"0.2.1\" }\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("deps")
        .arg("resolve")
        .arg(&app)
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(
            "failed to resolve package `nomo-lang/cli`: no available version satisfies all constraints"
        ),
        "{stderr}"
    );
    assert!(
        stderr.contains("`0.2.1` required by fynn/app -> fynn/utils -> nomo-lang/cli"),
        "{stderr}"
    );
    assert!(
        stderr.contains("`0.2.0` required by fynn/app -> nomo-lang/cli"),
        "{stderr}"
    );
    assert!(
        stderr.contains("available versions: 0.2.0, 0.2.1"),
        "{stderr}"
    );
    assert!(!app.join("nomo.lock").exists());

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_deps_rejects_url_like_package_identity() {
    let root = temp_test_root("deps-reject-url-identity");
    reset_dir(&root);
    let project = root.join("hello");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(project.join("src/main.nomo"), "package app.main\n").unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nnamespace = \"fynn\"\nname = \"hello\"\nversion = \"0.1.0\"\nedition = \"2026\"\n\n[dependencies]\njson = { package = \"github.com/nomo-lang/json\", version = \"0.1.0\" }\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("deps")
        .arg("resolve")
        .arg(&project)
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("must contain exactly one `/`"), "{stderr}");
    assert!(!project.join("nomo.lock").exists());

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_deps_rejects_reserved_package_namespace() {
    let root = temp_test_root("deps-reject-reserved-namespace");
    reset_dir(&root);
    let project = root.join("hello");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(project.join("src/main.nomo"), "package app.main\n").unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nnamespace = \"core\"\nname = \"hello\"\nversion = \"0.1.0\"\nedition = \"2026\"\n\n[dependencies]\nstd = { package = \"nomo-lang/std\", version = \"0.1.0\" }\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("deps")
        .arg("resolve")
        .arg(&project)
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("reserved"), "{stderr}");
    assert!(stderr.contains("core"), "{stderr}");
    assert!(!project.join("nomo.lock").exists());

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_deps_rejects_reserved_dependency_namespace() {
    let root = temp_test_root("deps-reject-reserved-dep-namespace");
    reset_dir(&root);
    let project = root.join("hello");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(project.join("src/main.nomo"), "package app.main\n").unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nnamespace = \"fynn\"\nname = \"hello\"\nversion = \"0.1.0\"\nedition = \"2026\"\n\n[dependencies]\nmagic = { package = \"nomo/magic\", version = \"0.1.0\" }\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("deps")
        .arg("resolve")
        .arg(&project)
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("reserved"), "{stderr}");
    assert!(stderr.contains("nomo"), "{stderr}");
    assert!(!project.join("nomo.lock").exists());

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_project_commands_accept_imports_from_dependency_aliases() {
    let root = temp_test_root("dependency-alias-imports");
    reset_dir(&root);
    let project = root.join("hello");
    let source = project.join("src/main.nomo");
    let c_path = project.join("build/c/main.c");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nnamespace = \"fynn\"\nname = \"hello\"\nversion = \"0.1.0\"\nedition = \"2026\"\n\n[dependencies]\njson = { package = \"nomo-lang/json\", version = \"0.1.0\" }\n",
    )
    .unwrap();
    fs::write(
        &source,
        "package app.main\n\nimport json.parser\n\nfn main() -> void {\n}\n",
    )
    .unwrap();

    let check_output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("check")
        .arg(&project)
        .output()
        .unwrap();

    assert!(
        check_output.status.success(),
        "{}",
        String::from_utf8_lossy(&check_output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&check_output.stdout),
        format!("checked {}\n", source.display())
    );

    let build_output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("build")
        .arg(&project)
        .arg("--emit-c")
        .output()
        .unwrap();

    assert!(
        build_output.status.success(),
        "{}",
        String::from_utf8_lossy(&build_output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&build_output.stdout),
        format!("built {}\n", c_path.display())
    );
    assert!(c_path.exists());

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_project_commands_load_local_flat_module() {
    let root = temp_test_root("local-flat-module");
    reset_dir(&root);
    let project = root.join("hello");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nnamespace = \"fynn\"\nname = \"hello\"\nversion = \"0.1.0\"\nedition = \"2026\"\n",
    )
    .unwrap();
    fs::write(
        project.join("src/main.nomo"),
        r#"package app.main

import app.math

fn main() -> void {
    let total: i64 = add(40, 2)
}
"#,
    )
    .unwrap();
    fs::write(
        project.join("src/math.nomo"),
        r#"package app.math

pub fn add(a: i64, b: i64) -> i64 {
    return a + b
}
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("check")
        .arg(&project)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_project_commands_load_local_directory_module() {
    let root = temp_test_root("local-directory-module");
    reset_dir(&root);
    let project = root.join("hello");
    fs::create_dir_all(project.join("src/math")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nnamespace = \"fynn\"\nname = \"hello\"\nversion = \"0.1.0\"\nedition = \"2026\"\n",
    )
    .unwrap();
    fs::write(
        project.join("src/main.nomo"),
        r#"package app.main

import app.math

fn main() -> void {
    let total: i64 = add(1, 2)
}
"#,
    )
    .unwrap();
    fs::write(
        project.join("src/math/main.nomo"),
        r#"package app.math

pub fn add(a: i64, b: i64) -> i64 {
    return a + b
}
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("check")
        .arg(&project)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_project_commands_reject_private_local_module_api() {
    let root = temp_test_root("local-module-private-api");
    reset_dir(&root);
    let project = root.join("hello");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nnamespace = \"fynn\"\nname = \"hello\"\nversion = \"0.1.0\"\nedition = \"2026\"\n",
    )
    .unwrap();
    fs::write(
        project.join("src/main.nomo"),
        r#"package app.main

import app.math

fn main() -> void {
    let total: i64 = hidden()
}
"#,
    )
    .unwrap();
    fs::write(
        project.join("src/math.nomo"),
        r#"package app.math

fn hidden() -> i64 {
    return 99
}
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("check")
        .arg(&project)
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("unknown function `hidden`"), "{stderr}");
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_project_commands_reject_missing_local_module() {
    let root = temp_test_root("local-module-missing");
    reset_dir(&root);
    let project = root.join("hello");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nnamespace = \"fynn\"\nname = \"hello\"\nversion = \"0.1.0\"\nedition = \"2026\"\n",
    )
    .unwrap();
    fs::write(
        project.join("src/main.nomo"),
        "package app.main\n\nimport app.missing\n\nfn main() -> void {\n}\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("check")
        .arg(&project)
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("E0903"), "{stderr}");
    assert!(stderr.contains("app.missing"), "{stderr}");
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_project_commands_reject_module_package_mismatch() {
    let root = temp_test_root("local-module-package-mismatch");
    reset_dir(&root);
    let project = root.join("hello");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nnamespace = \"fynn\"\nname = \"hello\"\nversion = \"0.1.0\"\nedition = \"2026\"\n",
    )
    .unwrap();
    fs::write(
        project.join("src/main.nomo"),
        "package app.main\n\nimport app.math\n\nfn main() -> void {\n}\n",
    )
    .unwrap();
    fs::write(project.join("src/math.nomo"), "package app.other\n").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("check")
        .arg(&project)
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("E0904"), "{stderr}");
    assert!(stderr.contains("app.math"), "{stderr}");
    assert!(stderr.contains("app.other"), "{stderr}");
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_project_commands_reject_local_module_import_cycles() {
    let root = temp_test_root("local-module-cycle");
    reset_dir(&root);
    let project = root.join("hello");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nnamespace = \"fynn\"\nname = \"hello\"\nversion = \"0.1.0\"\nedition = \"2026\"\n",
    )
    .unwrap();
    fs::write(
        project.join("src/main.nomo"),
        r#"package app.main

import app.a

fn main() -> void {
    let value: i64 = a()
}
"#,
    )
    .unwrap();
    fs::write(
        project.join("src/a.nomo"),
        r#"package app.a

import app.b

pub fn a() -> i64 {
    return b()
}
"#,
    )
    .unwrap();
    fs::write(
        project.join("src/b.nomo"),
        r#"package app.b

import app.a

pub fn b() -> i64 {
    return 42
}
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("check")
        .arg(&project)
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("E0607"), "{stderr}");
    assert!(stderr.contains("app.a -> app.b -> app.a"), "{stderr}");
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_project_commands_use_path_dependency_public_api() {
    let root = temp_test_root("path-dependency-public-api");
    reset_dir(&root);
    let dependency = root.join("calc");
    let project = root.join("hello");
    let source = project.join("src/main.nomo");
    let c_path = project.join("build/c/main.c");
    let bin_path = project.join("build/bin/hello");
    fs::create_dir_all(dependency.join("src")).unwrap();
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        dependency.join("nomo.toml"),
        "[package]\nnamespace = \"fynn\"\nname = \"calc\"\nversion = \"0.1.0\"\nedition = \"2026\"\n",
    )
    .unwrap();
    fs::write(
        dependency.join("src/main.nomo"),
        r#"package calc.main

pub struct Pair {
    value: i64
}

pub fn add(a: i64, b: i64) -> i64 {
    return a + b
}

pub fn make_pair(value: i64) -> Pair {
    return Pair { value: value }
}

fn hidden() -> i64 {
    return 99
}
"#,
    )
    .unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nnamespace = \"fynn\"\nname = \"hello\"\nversion = \"0.1.0\"\nedition = \"2026\"\n\n[dependencies]\ncalc = { package = \"fynn/calc\", path = \"../calc\" }\n",
    )
    .unwrap();
    fs::write(
        &source,
        r#"package app.main

import calc.main

fn main() -> void {
    let total: i64 = add(40, 2)
    let pair: Pair = make_pair(total)
}
"#,
    )
    .unwrap();

    let check_output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("check")
        .arg(&project)
        .output()
        .unwrap();

    assert!(
        check_output.status.success(),
        "{}",
        String::from_utf8_lossy(&check_output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&check_output.stdout),
        format!("checked {}\n", source.display())
    );

    let build_output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("build")
        .arg(&project)
        .output()
        .unwrap();

    assert!(
        build_output.status.success(),
        "{}",
        String::from_utf8_lossy(&build_output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&build_output.stdout),
        format!("built {}\n", bin_path.display())
    );
    assert!(bin_path.exists());
    let generated_c = fs::read_to_string(c_path).unwrap();
    assert!(generated_c.contains("nomo_pkg_calc_main_fn_add"));
    assert!(!generated_c.contains("nomo_pkg_app_main_fn_add"));
    assert!(generated_c.contains("nomo_pkg_calc_main_struct_Pair"));
    assert!(!generated_c.contains("nomo_pkg_app_main_struct_Pair"));

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_project_commands_use_path_dependency_module_public_api() {
    let root = temp_test_root("path-dependency-module-public-api");
    reset_dir(&root);
    let dependency = root.join("utils");
    let project = root.join("hello");
    let source = project.join("src/main.nomo");
    let c_path = project.join("build/c/main.c");
    fs::create_dir_all(dependency.join("src")).unwrap();
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        dependency.join("nomo.toml"),
        "[package]\nnamespace = \"fynn\"\nname = \"utils\"\nversion = \"0.1.0\"\nedition = \"2026\"\n",
    )
    .unwrap();
    fs::write(dependency.join("src/main.nomo"), "package utils.main\n").unwrap();
    fs::write(
        dependency.join("src/path.nomo"),
        r#"package utils.path

pub struct Segment {
    value: i64
}

pub fn join(a: i64, b: i64) -> i64 {
    return a + b
}

pub fn make_segment(value: i64) -> Segment {
    return Segment { value: value }
}
"#,
    )
    .unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nnamespace = \"fynn\"\nname = \"hello\"\nversion = \"0.1.0\"\nedition = \"2026\"\n\n[dependencies]\nlocal_utils = { package = \"fynn/utils\", path = \"../utils\" }\n",
    )
    .unwrap();
    fs::write(
        &source,
        r#"package app.main

import local_utils.path

fn main() -> void {
    let total: i64 = join(40, 2)
    let segment: Segment = make_segment(total)
}
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("build")
        .arg(&project)
        .arg("--emit-c")
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let generated_c = fs::read_to_string(c_path).unwrap();
    assert!(generated_c.contains("nomo_pkg_utils_path_fn_join"));
    assert!(!generated_c.contains("nomo_pkg_app_main_fn_join"));
    assert!(generated_c.contains("nomo_pkg_utils_path_struct_Segment"));
    assert!(!generated_c.contains("nomo_pkg_app_main_struct_Segment"));
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_project_commands_type_check_path_dependency_public_api() {
    let root = temp_test_root("path-dependency-api-type-check");
    reset_dir(&root);
    let dependency = root.join("calc");
    let project = root.join("hello");
    fs::create_dir_all(dependency.join("src")).unwrap();
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        dependency.join("nomo.toml"),
        "[package]\nnamespace = \"fynn\"\nname = \"calc\"\nversion = \"0.1.0\"\nedition = \"2026\"\n",
    )
    .unwrap();
    fs::write(
        dependency.join("src/main.nomo"),
        "package calc.main\n\npub fn add(a: i64, b: i64) -> i64 {\n    return a + b\n}\n",
    )
    .unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nnamespace = \"fynn\"\nname = \"hello\"\nversion = \"0.1.0\"\nedition = \"2026\"\n\n[dependencies]\ncalc = { package = \"fynn/calc\", path = \"../calc\" }\n",
    )
    .unwrap();
    fs::write(
        project.join("src/main.nomo"),
        r#"package app.main

import calc.main

fn main() -> void {
    let total: string = add(40, 2)
}
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("check")
        .arg(&project)
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("cannot initialize `total` as `string` from `i64`"),
        "{stderr}"
    );

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_project_commands_reject_private_path_dependency_api() {
    let root = temp_test_root("path-dependency-private-api");
    reset_dir(&root);
    let dependency = root.join("calc");
    let project = root.join("hello");
    fs::create_dir_all(dependency.join("src")).unwrap();
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        dependency.join("nomo.toml"),
        "[package]\nnamespace = \"fynn\"\nname = \"calc\"\nversion = \"0.1.0\"\nedition = \"2026\"\n",
    )
    .unwrap();
    fs::write(
        dependency.join("src/main.nomo"),
        "package calc.main\n\nfn hidden() -> i64 {\n    return 99\n}\n",
    )
    .unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nnamespace = \"fynn\"\nname = \"hello\"\nversion = \"0.1.0\"\nedition = \"2026\"\n\n[dependencies]\ncalc = { package = \"fynn/calc\", path = \"../calc\" }\n",
    )
    .unwrap();
    fs::write(
        project.join("src/main.nomo"),
        r#"package app.main

import calc.main

fn main() -> void {
    let value: i64 = hidden()
}
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("check")
        .arg(&project)
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("unknown function `hidden`"), "{stderr}");

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_project_commands_use_git_dependency_public_api() {
    let root = temp_test_root("git-dependency-public-api");
    reset_dir(&root);
    let dependency = root.join("calc");
    let project = root.join("hello");
    let source = project.join("src/main.nomo");
    let c_path = project.join("build/c/main.c");
    let calc_rev = init_git_package_with_source(
        &dependency,
        "fynn",
        "calc",
        r#"package calc.main

pub struct Pair {
    value: i64
}

pub fn add(a: i64, b: i64) -> i64 {
    return a + b
}

pub fn make_pair(value: i64) -> Pair {
    return Pair { value: value }
}

fn hidden() -> i64 {
    return 99
}
"#,
    );
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        format!(
            "[package]\nnamespace = \"fynn\"\nname = \"hello\"\nversion = \"0.1.0\"\nedition = \"2026\"\n\n[dependencies]\ncalc = {{ package = \"fynn/calc\", git = \"{}\", rev = \"{}\" }}\n",
            dependency.display(),
            calc_rev
        ),
    )
    .unwrap();
    fs::write(
        &source,
        r#"package app.main

import calc.main

fn main() -> void {
    let total: i64 = add(40, 2)
    let pair: Pair = make_pair(total)
}
"#,
    )
    .unwrap();

    let check_output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("check")
        .arg(&project)
        .output()
        .unwrap();

    assert!(
        check_output.status.success(),
        "{}",
        String::from_utf8_lossy(&check_output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&check_output.stdout),
        format!("checked {}\n", source.display())
    );

    let build_output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("build")
        .arg(&project)
        .arg("--emit-c")
        .output()
        .unwrap();

    assert!(
        build_output.status.success(),
        "{}",
        String::from_utf8_lossy(&build_output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&build_output.stdout),
        format!("built {}\n", c_path.display())
    );
    assert!(c_path.exists());
    let generated_c = fs::read_to_string(c_path).unwrap();
    assert!(generated_c.contains("nomo_pkg_calc_main_fn_add"));
    assert!(!generated_c.contains("nomo_pkg_app_main_fn_add"));
    assert!(generated_c.contains("nomo_pkg_calc_main_struct_Pair"));
    assert!(!generated_c.contains("nomo_pkg_app_main_struct_Pair"));

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_project_commands_use_git_dependency_module_public_api() {
    let root = temp_test_root("git-dependency-module-public-api");
    reset_dir(&root);
    let dependency = root.join("utils");
    let project = root.join("hello");
    let source = project.join("src/main.nomo");
    let c_path = project.join("build/c/main.c");
    fs::create_dir_all(dependency.join("src")).unwrap();
    fs::write(
        dependency.join("nomo.toml"),
        "[package]\nnamespace = \"fynn\"\nname = \"utils\"\nversion = \"0.1.0\"\nedition = \"2026\"\n",
    )
    .unwrap();
    fs::write(dependency.join("src/main.nomo"), "package utils.main\n").unwrap();
    fs::write(
        dependency.join("src/path.nomo"),
        r#"package utils.path

pub fn join(a: i64, b: i64) -> i64 {
    return a + b
}
"#,
    )
    .unwrap();
    run_git(&dependency, &["init", "--quiet"]);
    run_git(
        &dependency,
        &["config", "user.email", "nomo@example.invalid"],
    );
    run_git(&dependency, &["config", "user.name", "Nomo Test"]);
    run_git(&dependency, &["add", "nomo.toml", "src"]);
    run_git(&dependency, &["commit", "--quiet", "-m", "initial"]);
    let utils_rev = git_head_rev(&dependency);

    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        format!(
            "[package]\nnamespace = \"fynn\"\nname = \"hello\"\nversion = \"0.1.0\"\nedition = \"2026\"\n\n[dependencies]\nlocal_utils = {{ package = \"fynn/utils\", git = \"{}\", rev = \"{}\" }}\n",
            dependency.display(),
            utils_rev
        ),
    )
    .unwrap();
    fs::write(
        &source,
        r#"package app.main

import local_utils.path

fn main() -> void {
    let total: i64 = join(40, 2)
}
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("build")
        .arg(&project)
        .arg("--emit-c")
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let generated_c = fs::read_to_string(c_path).unwrap();
    assert!(generated_c.contains("nomo_pkg_utils_path_fn_join"));
    assert!(!generated_c.contains("nomo_pkg_app_main_fn_join"));
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_project_commands_reject_imports_without_dependency_alias() {
    let root = temp_test_root("dependency-alias-missing");
    reset_dir(&root);
    let project = root.join("hello");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nnamespace = \"fynn\"\nname = \"hello\"\nversion = \"0.1.0\"\nedition = \"2026\"\n\n[dependencies]\njson = { package = \"nomo-lang/json\", version = \"0.1.0\" }\n",
    )
    .unwrap();
    fs::write(
        project.join("src/main.nomo"),
        "package app.main\n\nimport yaml.parser\n\nfn main() -> void {\n}\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("check")
        .arg(&project)
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("unsupported import `yaml.parser`"),
        "{stderr}"
    );

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomoc_still_rejects_external_dependency_imports() {
    let root = temp_test_root("nomoc-external-import");
    reset_dir(&root);
    let source = root.join("main.nomo");
    fs::write(
        &source,
        "package app.main\n\nimport json.parser\n\nfn main() -> void {\n}\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomoc"))
        .arg("check")
        .arg(&source)
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("unsupported import `json.parser`"),
        "{stderr}"
    );

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_commands_default_to_current_project_directory() {
    let root = temp_test_root("current-dir-commands");
    reset_dir(&root);

    let new_output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("new")
        .arg("hello")
        .current_dir(&root)
        .output()
        .unwrap();
    assert!(
        new_output.status.success(),
        "{}",
        String::from_utf8_lossy(&new_output.stderr)
    );

    let project = root.join("hello");
    let project_output = project.canonicalize().unwrap();
    let c_path = project.join("build/c/main.c");
    let bin_path = project.join("build/bin/hello");

    let check_output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("check")
        .current_dir(&project)
        .output()
        .unwrap();
    assert!(
        check_output.status.success(),
        "{}",
        String::from_utf8_lossy(&check_output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&check_output.stdout),
        format!(
            "checked {}\n",
            project_output.join("src/main.nomo").display()
        )
    );

    let build_output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("build")
        .arg("--emit-c")
        .current_dir(&project)
        .output()
        .unwrap();
    assert!(
        build_output.status.success(),
        "{}",
        String::from_utf8_lossy(&build_output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&build_output.stdout),
        format!(
            "built {}\n",
            project_output.join("build/c/main.c").display()
        )
    );
    assert!(c_path.exists());
    assert!(!bin_path.exists());

    let run_output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .current_dir(&project)
        .output()
        .unwrap();
    assert!(
        run_output.status.success(),
        "{}",
        String::from_utf8_lossy(&run_output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&run_output.stdout), "Hello, Nomo\n");
    assert!(bin_path.exists());

    let clean_output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("clean")
        .current_dir(&project)
        .output()
        .unwrap();
    assert!(
        clean_output.status.success(),
        "{}",
        String::from_utf8_lossy(&clean_output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&clean_output.stdout),
        format!("cleaned {}\n", project_output.join("build").display())
    );
    assert!(!project.join("build").exists());

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_commands_default_to_nested_project_directory() {
    let root = temp_test_root("nested-dir-commands");
    reset_dir(&root);

    let new_output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("new")
        .arg("hello")
        .current_dir(&root)
        .output()
        .unwrap();
    assert!(
        new_output.status.success(),
        "{}",
        String::from_utf8_lossy(&new_output.stderr)
    );

    let project = root.join("hello");
    let project_output = project.canonicalize().unwrap();
    let src_dir = project.join("src");
    let c_path = project.join("build/c/main.c");
    let bin_path = project.join("build/bin/hello");

    let check_output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("check")
        .current_dir(&src_dir)
        .output()
        .unwrap();
    assert!(
        check_output.status.success(),
        "{}",
        String::from_utf8_lossy(&check_output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&check_output.stdout),
        format!(
            "checked {}\n",
            project_output.join("src/main.nomo").display()
        )
    );

    let build_output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("build")
        .arg("--emit-c")
        .current_dir(&src_dir)
        .output()
        .unwrap();
    assert!(
        build_output.status.success(),
        "{}",
        String::from_utf8_lossy(&build_output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&build_output.stdout),
        format!(
            "built {}\n",
            project_output.join("build/c/main.c").display()
        )
    );
    assert!(c_path.exists());
    assert!(!bin_path.exists());

    let run_output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .current_dir(&src_dir)
        .output()
        .unwrap();
    assert!(
        run_output.status.success(),
        "{}",
        String::from_utf8_lossy(&run_output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&run_output.stdout), "Hello, Nomo\n");
    assert!(bin_path.exists());

    let clean_output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("clean")
        .current_dir(&src_dir)
        .output()
        .unwrap();
    assert!(
        clean_output.status.success(),
        "{}",
        String::from_utf8_lossy(&clean_output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&clean_output.stdout),
        format!("cleaned {}\n", project_output.join("build").display())
    );
    assert!(!project.join("build").exists());

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_commands_accept_source_file_path_under_project() {
    let root = temp_test_root("source-file-cli-commands");
    reset_dir(&root);

    let new_output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("new")
        .arg("hello")
        .current_dir(&root)
        .output()
        .unwrap();
    assert!(
        new_output.status.success(),
        "{}",
        String::from_utf8_lossy(&new_output.stderr)
    );

    let project = root.join("hello");
    let source = project.join("src/main.nomo");
    let c_path = project.join("build/c/main.c");
    let bin_path = project.join("build/bin/hello");

    let check_output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("check")
        .arg(&source)
        .output()
        .unwrap();
    assert!(
        check_output.status.success(),
        "{}",
        String::from_utf8_lossy(&check_output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&check_output.stdout),
        format!("checked {}\n", source.display())
    );

    let build_output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("build")
        .arg(&source)
        .arg("--emit-c")
        .output()
        .unwrap();
    assert!(
        build_output.status.success(),
        "{}",
        String::from_utf8_lossy(&build_output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&build_output.stdout),
        format!("built {}\n", c_path.display())
    );
    assert!(c_path.exists());
    assert!(!bin_path.exists());

    let run_output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(&source)
        .output()
        .unwrap();
    assert!(
        run_output.status.success(),
        "{}",
        String::from_utf8_lossy(&run_output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&run_output.stdout), "Hello, Nomo\n");
    assert!(bin_path.exists());

    let clean_output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("clean")
        .arg(&source)
        .output()
        .unwrap();
    assert!(
        clean_output.status.success(),
        "{}",
        String::from_utf8_lossy(&clean_output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&clean_output.stdout),
        format!("cleaned {}\n", project.join("build").display())
    );
    assert!(!project.join("build").exists());

    let native_build_output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("build")
        .arg(&source)
        .output()
        .unwrap();
    assert!(
        native_build_output.status.success(),
        "{}",
        String::from_utf8_lossy(&native_build_output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&native_build_output.stdout),
        format!("built {}\n", bin_path.display())
    );
    assert!(c_path.exists());
    assert!(bin_path.exists());

    let bin_output = Command::new(&bin_path).output().unwrap();
    assert!(
        bin_output.status.success(),
        "{}",
        String::from_utf8_lossy(&bin_output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&bin_output.stdout), "Hello, Nomo\n");

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_clean_rejects_extra_arguments_without_deleting_build_dir() {
    let root = temp_test_root("clean-extra-args");
    reset_dir(&root);
    let project = root.join("hello");
    let build_dir = project.join("build");
    fs::create_dir_all(&build_dir).unwrap();
    fs::write(project.join("nomo.toml"), "[package]\nname = \"hello\"\n").unwrap();
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(project.join("src/main.nomo"), "package app.main\n").unwrap();
    fs::write(build_dir.join("keep.txt"), "keep").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("clean")
        .arg(&project)
        .arg("extra")
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(
        output.stdout.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("usage: nomo clean [path]"), "{stderr}");
    assert!(build_dir.join("keep.txt").exists());

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_new_rejects_extra_arguments() {
    let root = temp_test_root("new-extra-args");
    reset_dir(&root);

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("new")
        .arg("hello")
        .arg("extra")
        .current_dir(&root)
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("usage: nomo new <name>"), "{stderr}");
    assert!(!root.join("hello").exists());

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_new_rejects_invalid_project_name() {
    let root = temp_test_root("new-invalid-name");
    reset_dir(&root);

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("new")
        .arg("1bad")
        .current_dir(&root)
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("invalid project name `1bad`"), "{stderr}");
    assert!(
        output.stdout.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(!root.join("1bad").exists());

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_new_rejects_existing_destination_without_overwrite() {
    let root = temp_test_root("new-existing-destination");
    reset_dir(&root);
    let project = root.join("hello");
    fs::create_dir_all(&project).unwrap();
    fs::write(project.join("keep.txt"), "do not overwrite").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("new")
        .arg("hello")
        .current_dir(&root)
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("destination already exists"), "{stderr}");
    assert!(stderr.contains(&project.display().to_string()), "{stderr}");
    assert!(
        output.stdout.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert_eq!(
        fs::read_to_string(project.join("keep.txt")).unwrap(),
        "do not overwrite"
    );
    assert!(!project.join("nomo.toml").exists());
    assert!(!project.join("src/main.nomo").exists());

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomoc_emit_c_can_be_compiled_with_system_cc() {
    let root = temp_test_root("nomoc-emit-c");
    reset_dir(&root);

    let new_output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("new")
        .arg("hello")
        .current_dir(&root)
        .output()
        .unwrap();
    assert!(
        new_output.status.success(),
        "{}",
        String::from_utf8_lossy(&new_output.stderr)
    );

    let project = root.join("hello");
    let source = project.join("src/main.nomo");
    let c_path = project.join("build/main.c");
    let bin_path = project.join("build/hello-manual");

    let build_output = Command::new(env!("CARGO_BIN_EXE_nomoc"))
        .arg("build")
        .arg(&source)
        .arg("--emit-c")
        .arg("--out")
        .arg(&c_path)
        .output()
        .unwrap();
    assert!(
        build_output.status.success(),
        "{}",
        String::from_utf8_lossy(&build_output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&build_output.stdout),
        format!("emitted {}\n", c_path.display())
    );
    assert!(
        String::from_utf8_lossy(&build_output.stderr).is_empty(),
        "{}",
        String::from_utf8_lossy(&build_output.stderr)
    );
    assert!(c_path.exists());

    let cc_output = Command::new("cc")
        .arg("-std=c99")
        .arg(&c_path)
        .arg("-o")
        .arg(&bin_path)
        .output()
        .unwrap();
    assert!(
        cc_output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&cc_output.stdout),
        String::from_utf8_lossy(&cc_output.stderr)
    );

    let run_output = Command::new(&bin_path).output().unwrap();
    assert!(
        run_output.status.success(),
        "{}",
        String::from_utf8_lossy(&run_output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&run_output.stdout), "Hello, Nomo\n");

    let check_output = Command::new(env!("CARGO_BIN_EXE_nomoc"))
        .arg("check")
        .arg(&source)
        .arg("--json-errors")
        .output()
        .unwrap();
    assert!(
        check_output.status.success(),
        "{}",
        String::from_utf8_lossy(&check_output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&check_output.stdout),
        format!("checked {}\n", source.display())
    );
    assert!(
        String::from_utf8_lossy(&check_output.stderr).is_empty(),
        "{}",
        String::from_utf8_lossy(&check_output.stderr)
    );

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomoc_builds_standalone_source_file_to_compilable_c() {
    let root = temp_test_root("nomoc-standalone-source");
    reset_dir(&root);

    let source = root.join("main.nomo");
    let c_path = root.join("out/main.c");
    let bin_path = root.join("out/standalone");
    fs::write(
        &source,
        r#"package app.main

import std.io

fn main() -> void {
    io.println("standalone ok")
}
"#,
    )
    .unwrap();

    let build_output = Command::new(env!("CARGO_BIN_EXE_nomoc"))
        .arg("build")
        .arg(&source)
        .arg("--emit-c")
        .arg("--out")
        .arg(&c_path)
        .output()
        .unwrap();
    assert!(
        build_output.status.success(),
        "{}",
        String::from_utf8_lossy(&build_output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&build_output.stdout),
        format!("emitted {}\n", c_path.display())
    );
    assert!(
        String::from_utf8_lossy(&build_output.stderr).is_empty(),
        "{}",
        String::from_utf8_lossy(&build_output.stderr)
    );
    assert!(c_path.exists());

    let cc_output = Command::new("cc")
        .arg("-std=c99")
        .arg(&c_path)
        .arg("-o")
        .arg(&bin_path)
        .output()
        .unwrap();
    assert!(
        cc_output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&cc_output.stdout),
        String::from_utf8_lossy(&cc_output.stderr)
    );

    let run_output = Command::new(&bin_path).output().unwrap();
    assert!(
        run_output.status.success(),
        "{}",
        String::from_utf8_lossy(&run_output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&run_output.stdout),
        "standalone ok\n"
    );
    assert!(
        String::from_utf8_lossy(&run_output.stderr).is_empty(),
        "{}",
        String::from_utf8_lossy(&run_output.stderr)
    );

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomoc_build_rejects_missing_out_path() {
    let root = temp_test_root("nomoc-missing-out-path");
    reset_dir(&root);
    let source = root.join("main.nomo");
    fs::write(&source, "package app.main\n").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomoc"))
        .arg("build")
        .arg(&source)
        .arg("--out")
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(
        output.stdout.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(
            "usage: nomoc build <source.nomo> [--target <triple>] [--emit-c] [--out path] [--json-errors]",
        ),
        "{stderr}"
    );

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomoc_build_prints_c_to_stdout_without_out_path() {
    let root = temp_test_root("nomoc-stdout-c");
    reset_dir(&root);

    let new_output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("new")
        .arg("hello")
        .current_dir(&root)
        .output()
        .unwrap();
    assert!(
        new_output.status.success(),
        "{}",
        String::from_utf8_lossy(&new_output.stderr)
    );

    let project = root.join("hello");
    let source = project.join("src/main.nomo");
    let c_path = project.join("build/stdout-main.c");
    let bin_path = project.join("build/stdout-hello");

    let build_output = Command::new(env!("CARGO_BIN_EXE_nomoc"))
        .arg("build")
        .arg(&source)
        .output()
        .unwrap();
    assert!(
        build_output.status.success(),
        "{}",
        String::from_utf8_lossy(&build_output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&build_output.stderr).is_empty(),
        "{}",
        String::from_utf8_lossy(&build_output.stderr)
    );
    let c = String::from_utf8_lossy(&build_output.stdout);
    assert!(c.contains("#include <stdio.h>"), "{c}");
    assert!(c.contains("nomo_fn_main"), "{c}");

    fs::create_dir_all(c_path.parent().unwrap()).unwrap();
    fs::write(&c_path, build_output.stdout).unwrap();

    let cc_output = Command::new("cc")
        .arg("-std=c99")
        .arg(&c_path)
        .arg("-o")
        .arg(&bin_path)
        .output()
        .unwrap();
    assert!(
        cc_output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&cc_output.stdout),
        String::from_utf8_lossy(&cc_output.stderr)
    );

    let run_output = Command::new(&bin_path).output().unwrap();
    assert!(
        run_output.status.success(),
        "{}",
        String::from_utf8_lossy(&run_output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&run_output.stdout), "Hello, Nomo\n");

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_build_emit_c_writes_project_c_artifact() {
    let root = temp_test_root("nomo-build-emit-c");
    reset_dir(&root);

    let new_output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("new")
        .arg("hello")
        .current_dir(&root)
        .output()
        .unwrap();
    assert!(
        new_output.status.success(),
        "{}",
        String::from_utf8_lossy(&new_output.stderr)
    );

    let project = root.join("hello");
    let c_path = project.join("build/c/main.c");
    let bin_path = project.join("build/bin/hello");
    let manual_bin_path = project.join("build/hello-manual");

    let build_output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("build")
        .arg(&project)
        .arg("--emit-c")
        .output()
        .unwrap();
    assert!(
        build_output.status.success(),
        "{}",
        String::from_utf8_lossy(&build_output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&build_output.stdout),
        format!("built {}\n", c_path.display())
    );
    assert!(c_path.exists());
    assert!(!bin_path.exists());

    let cc_output = Command::new("cc")
        .arg("-std=c99")
        .arg(&c_path)
        .arg("-o")
        .arg(&manual_bin_path)
        .output()
        .unwrap();
    assert!(
        cc_output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&cc_output.stdout),
        String::from_utf8_lossy(&cc_output.stderr)
    );

    let run_output = Command::new(&manual_bin_path).output().unwrap();
    assert!(
        run_output.status.success(),
        "{}",
        String::from_utf8_lossy(&run_output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&run_output.stdout), "Hello, Nomo\n");

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_build_creates_project_executable() {
    let root = temp_test_root("nomo-build-executable");
    reset_dir(&root);

    let new_output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("new")
        .arg("hello")
        .current_dir(&root)
        .output()
        .unwrap();
    assert!(
        new_output.status.success(),
        "{}",
        String::from_utf8_lossy(&new_output.stderr)
    );

    let project = root.join("hello");
    let bin_path = project.join("build/bin/hello");
    let c_path = project.join("build/c/main.c");

    let build_output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("build")
        .arg(&project)
        .output()
        .unwrap();
    assert!(
        build_output.status.success(),
        "{}",
        String::from_utf8_lossy(&build_output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&build_output.stdout),
        format!("built {}\n", bin_path.display())
    );
    assert!(c_path.exists());
    assert!(bin_path.exists());

    let run_output = Command::new(&bin_path).output().unwrap();
    assert!(
        run_output.status.success(),
        "{}",
        String::from_utf8_lossy(&run_output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&run_output.stdout), "Hello, Nomo\n");

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn generated_c_runtime_smoke_passes_with_address_sanitizer_when_available() {
    let root = temp_test_root("asan-runtime-smoke");
    reset_dir(&root);

    if !cc_supports_address_sanitizer(&root) {
        fs::remove_dir_all(&root).unwrap();
        return;
    }

    let source = root.join("main.nomo");
    let c_path = root.join("main.c");
    let bin_path = root.join("asan-runtime-smoke");

    fs::write(
        &source,
        r#"package app.main

import std.array
import std.io
import std.json
import std.jsonrpc

struct Bag {
    items: Array<string>
}

fn fail() -> Result<string, string> {
    return Err("stop")
}

fn cleanup(label: string) -> void {
    io.println(label)
}

fn label(value: Option<string>) -> string {
    return match value {
        Some(text) => text
        None => "missing"
    }
}

fn json_roundtrip() -> Result<string, JsonError> {
    let parsed: JsonValue = json.parse("{\"items\":[{\"name\":\"nomo\"}],\"ok\":true}")?
    let selected: Option<JsonValue> = json.get(parsed, "items")
    match selected {
        None => {
            return Ok("missing items")
        }
        Some(items) => {
            let values: Option<Array<JsonValue>> = json.array_items(items)
            match values {
                None => {
                    return Ok("items is not an array")
                }
                Some(entries) => {
                    let first: Option<JsonValue> = entries.get(0)
                    match first {
                        None => {
                            return Ok("items is empty")
                        }
                        Some(entry) => {
                            let mut copies: Array<JsonValue> = Array.new<JsonValue>()
                            copies.push(parsed)
                            copies.push(entry)
                            let built: JsonValue = json.from_array(copies)?
                            return Ok(json.stringify(built))
                        }
                    }
                }
            }
        }
    }
}

fn jsonrpc_roundtrip() -> Result<u64, JsonRpcProtocolError> {
    let initial: JsonRpcDecoder = jsonrpc.decoder(4096 as u64)?
    let partial: JsonRpcDecodeBatch = jsonrpc.feed(initial, "{\"jsonrpc\":\"2.0\",\"id\":1,\"meth")?
    let complete: JsonRpcDecodeBatch = jsonrpc.feed(partial.decoder, "od\":\"initialize\"}\n{\"jsonrpc\":\"2.0\",\"method\":\"ready\"}\n")?
    let messages: Array<JsonRpcMessage> = complete.messages
    jsonrpc.finish(complete.decoder)?
    return Ok(messages.len())
}

fn run() -> Result<string, string> {
    let mut items: Array<string> = Array.new<string>()
    items.push("one")
    let snapshot: Array<string> = items
    items.set(0, "two")

    let before: string = label(snapshot.get(0))
    let after: string = label(items.get(0))
    let check_before: string = if before != "one" {
        panic("array cow failed")
    } else {
        "ok"
    }
    let check_after: string = if after != "two" {
        panic("array write failed")
    } else {
        check_before
    }

    let mut bag: Bag = Bag { items: items }
    let mut replacement: Array<string> = Array.new<string>()
    replacement.push("three")
    bag.items = replacement
    replacement.set(0, "four")

    let bag_snapshot: Array<string> = bag.items
    let from_bag: string = label(bag_snapshot.get(0))
    let from_replacement: string = label(replacement.get(0))
    let check_bag: string = if from_bag != "three" {
        panic("field cow failed")
    } else {
        check_after
    }
    let check_replacement: string = if from_replacement != "four" {
        panic("replacement write failed")
    } else {
        check_bag
    }

    defer cleanup("cleanup")
    let value: string = fail()?
    return Ok(value)
}

fn main() -> void {
    for let i: u64 = 0; i < 256; i++ {
        let json_result: Result<string, JsonError> = json_roundtrip()
        match json_result {
            Err(err) => {
                panic(err.message)
            }
            Ok(value) => {
                let checked: string = if value == "missing items" {
                    panic("json traversal failed")
                } else {
                    value
                }
            }
        }

        let rpc_result: Result<u64, JsonRpcProtocolError> = jsonrpc_roundtrip()
        match rpc_result {
            Err(err) => {
                panic(err.message)
            }
            Ok(count) => {
                let rpc_checked: u64 = if count != 2 {
                    panic("JSON-RPC decoder replacement failed")
                } else {
                    count
                }
            }
        }
    }

    let result: Result<string, string> = run()
    match result {
        Ok(value) => {
            io.println(value)
        }
        Err(err) => {
            io.println(err)
        }
    }
}
"#,
    )
    .unwrap();

    let build_output = Command::new(env!("CARGO_BIN_EXE_nomoc"))
        .arg("build")
        .arg(&source)
        .arg("--emit-c")
        .arg("--out")
        .arg(&c_path)
        .output()
        .unwrap();
    assert!(
        build_output.status.success(),
        "{}",
        String::from_utf8_lossy(&build_output.stderr)
    );

    let cc_output = Command::new("cc")
        .arg("-fsanitize=address")
        .arg("-fno-omit-frame-pointer")
        .arg("-g")
        .arg(&c_path)
        .arg("-o")
        .arg(&bin_path)
        .output()
        .unwrap();
    assert!(
        cc_output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&cc_output.stdout),
        String::from_utf8_lossy(&cc_output.stderr)
    );

    let asan_options = if cfg!(target_os = "macos") {
        "detect_leaks=0:abort_on_error=1"
    } else {
        "detect_leaks=1:abort_on_error=1"
    };
    let run_output = Command::new(&bin_path)
        .env("ASAN_OPTIONS", asan_options)
        .output()
        .unwrap();
    assert!(
        run_output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run_output.stdout),
        String::from_utf8_lossy(&run_output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&run_output.stdout),
        "cleanup\nstop\n"
    );
    assert!(
        String::from_utf8_lossy(&run_output.stderr).is_empty(),
        "{}",
        String::from_utf8_lossy(&run_output.stderr)
    );

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn async_frame_completion_and_early_drop_are_asan_clean_when_available() {
    let root = temp_test_root("asan-async-frame-drop");
    reset_dir(&root);

    if !cc_supports_address_sanitizer(&root) {
        fs::remove_dir_all(&root).unwrap();
        return;
    }

    let source = root.join("main.nomo");
    let generated_c = root.join("generated.c");
    fs::write(
        &source,
        r#"package app.main

import std.array
import std.io
import std.result
import std.string
import std.task
import std.time

struct Envelope {
    body: string
}

enum State {
    Ready(string)
    Empty
}

fn child_input() -> string {
    return string.to_upper("child")
}

suspend fn child(message: string, values: Array<string>) -> string {
    let child_message: string = message
    let child_values: Array<string> = values
    io.println("child-before")
    let immediate: Result<void, TaskError> = task.sleep(time.duration_millis(0))
    let waited: Result<void, TaskError> = task.sleep(time.duration_millis(1))
    io.println(result.is_ok(immediate))
    let child_count: u64 = child_values.len()
    io.println(child_message)
    io.println(child_count)
    return child_message
}

suspend fn main() -> void {
    let message: string = "live"
    let values: Array<string> = ["alpha", "beta"]
    let envelope: Envelope = Envelope { body: "payload" }
    let state: State = State.Ready("state")
    io.println("before")
    let child_result: string = child(child_input(), ["nested", "frame"])
    io.println(message, envelope.body, child_result)
    let state_copy: State = state
    task.yield_now()
    let count: u64 = values.len()
    io.println("after", count)
}
"#,
    )
    .unwrap();

    let build_output = Command::new(env!("CARGO_BIN_EXE_nomoc"))
        .arg("build")
        .arg(&source)
        .arg("--emit-c")
        .arg("--out")
        .arg(&generated_c)
        .output()
        .unwrap();
    assert!(
        build_output.status.success(),
        "{}",
        String::from_utf8_lossy(&build_output.stderr)
    );

    let generated = fs::read_to_string(&generated_c).unwrap();
    assert!(generated.contains("nomo_async_timer_disarm"));
    assert!(generated.contains("nomo_async_parameter_owned_nomo_message"));
    assert!(generated.contains("nomo_async_parameter_owned_nomo_values"));
    assert!(generated.contains("nomo_async_result_owned"));
    let drop_call = "    nomo_async_drop_main(&nomo__frame);\n";
    assert_eq!(generated.matches(drop_call).count(), 1);
    let completed = generated.replacen(
        drop_call,
        "    nomo_async_drop_main(&nomo__frame);\n    nomo_async_drop_main(&nomo__frame);\n",
        1,
    );

    let mut early = completed.clone();
    let executor_call = early
        .rfind("nomo_async_executor_run_root(")
        .expect("generated main must invoke the root executor");
    early.replace_range(
        executor_call..executor_call + "nomo_async_executor_run_root".len(),
        "nomo_async_poll_once_root",
    );
    let main_start = early
        .find("int main(void)")
        .expect("generated C must contain main");
    early.insert_str(
        main_start,
        "static int nomo_async_poll_once_root(\n\
         void *frame,\n\
         nomo_async_poll_fn poll,\n\
         nomo_async_context *context\n\
         ) {\n\
             (void)poll(frame, context);\n\
             return 0;\n\
         }\n\n",
    );

    let asan_options = if cfg!(target_os = "macos") {
        "detect_leaks=0:abort_on_error=1"
    } else {
        "detect_leaks=1:abort_on_error=1"
    };
    for (name, c_source, expected_stdout) in [
        (
            "completed",
            completed,
            "before\nchild-before\ntrue\nCHILD\n2\nlive payload CHILD\nafter 2\n",
        ),
        ("early", early, "before\nchild-before\n"),
    ] {
        let c_path = root.join(format!("{name}.c"));
        let bin_path = root.join(format!("asan-async-frame-{name}"));
        let metrics_path = root.join(format!("asan-async-frame-{name}-metrics.json"));
        fs::write(&c_path, c_source).unwrap();

        let cc_output = Command::new("cc")
            .arg("-fsanitize=address")
            .arg("-fno-omit-frame-pointer")
            .arg("-g")
            .arg(&c_path)
            .arg("-o")
            .arg(&bin_path)
            .output()
            .unwrap();
        assert!(
            cc_output.status.success(),
            "{} compile stdout:\n{}\ncompile stderr:\n{}",
            name,
            String::from_utf8_lossy(&cc_output.stdout),
            String::from_utf8_lossy(&cc_output.stderr)
        );

        let run_output = Command::new(&bin_path)
            .env("ASAN_OPTIONS", asan_options)
            .env("NOMO_ASYNC_METRICS_PATH", &metrics_path)
            .output()
            .unwrap();
        assert!(
            run_output.status.success(),
            "{} stdout:\n{}\nstderr:\n{}",
            name,
            String::from_utf8_lossy(&run_output.stdout),
            String::from_utf8_lossy(&run_output.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&run_output.stdout),
            expected_stdout,
            "{name}"
        );
        assert!(
            String::from_utf8_lossy(&run_output.stderr).is_empty(),
            "{} stderr:\n{}",
            name,
            String::from_utf8_lossy(&run_output.stderr)
        );
        let metrics: serde_json::Value =
            serde_json::from_slice(&fs::read(&metrics_path).unwrap()).unwrap();
        assert_eq!(metrics["counters"]["timer_registrations"], 1, "{name}");
        assert_eq!(metrics["counters"]["live_timers"], 0, "{name}");
        if name == "completed" {
            assert_eq!(metrics["counters"]["timer_expirations"], 1);
            assert_eq!(metrics["counters"]["timer_cancellations"], 0);
        } else {
            assert_eq!(metrics["counters"]["timer_expirations"], 0);
            assert_eq!(metrics["counters"]["timer_cancellations"], 1);
        }
    }

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn structured_task_completion_and_parent_drop_are_asan_clean_when_available() {
    let root = temp_test_root("asan-structured-task-drop");
    reset_dir(&root);

    if !cc_supports_address_sanitizer(&root) {
        fs::remove_dir_all(&root).unwrap();
        return;
    }

    let source = root.join("main.nomo");
    let generated_c = root.join("generated.c");
    fs::write(
        &source,
        r#"package app.main

import std.io
import std.result
import std.string
import std.task

fn message(value: string) -> string {
    return string.to_upper(value)
}

suspend fn child(value: string) -> string {
    io.println(value, "before")
    task.yield_now()
    io.println(value, "after")
    return value
}

suspend fn gather() -> string {
    task.scope {
        let left = task.spawn child(message("left"))
        let right = task.spawn child(message("right"))
        let joined_left: Result<string, TaskError> = task.join(left)
        let joined_right: Result<string, TaskError> = task.join(right)
        let left_value: string = result.unwrap_or(joined_left, "left failed")
        let right_value: string = result.unwrap_or(joined_right, "right failed")
        return string.concat(left_value, right_value)
    }
}

suspend fn main() -> void {
    let gathered: string = gather()
    io.println(gathered)
}
"#,
    )
    .unwrap();

    let build_output = Command::new(env!("CARGO_BIN_EXE_nomoc"))
        .arg("build")
        .arg(&source)
        .arg("--emit-c")
        .arg("--out")
        .arg(&generated_c)
        .output()
        .unwrap();
    assert!(
        build_output.status.success(),
        "{}",
        String::from_utf8_lossy(&build_output.stderr)
    );

    let generated = fs::read_to_string(&generated_c).unwrap();
    assert!(generated.contains("structured_waiter_frame"));
    assert!(generated.contains("structured_waiter_frame = context->current_frame"));
    assert!(generated.contains("nomo_async_parameter_owned_nomo_value"));
    let drop_call = "    nomo_async_drop_main(&nomo__frame);\n";
    let completed = generated.replacen(
        drop_call,
        "    nomo_async_drop_main(&nomo__frame);\n    nomo_async_drop_main(&nomo__frame);\n",
        1,
    );
    let mut early = completed.clone();
    let executor_call = early
        .rfind("nomo_async_executor_run_root(")
        .expect("generated main must invoke the root executor");
    early.replace_range(
        executor_call..executor_call + "nomo_async_executor_run_root".len(),
        "nomo_async_poll_once_root",
    );
    let main_start = early
        .find("int main(void)")
        .expect("generated C must contain main");
    early.insert_str(
        main_start,
        "static int nomo_async_poll_once_root(\n\
         void *frame,\n\
         nomo_async_poll_fn poll,\n\
         nomo_async_context *context\n\
         ) {\n\
             (void)poll(frame, context);\n\
             return 0;\n\
         }\n\n",
    );

    let asan_options = if cfg!(target_os = "macos") {
        "detect_leaks=0:abort_on_error=1"
    } else {
        "detect_leaks=1:abort_on_error=1"
    };
    for (name, c_source, expected_stdout) in [
        (
            "completed",
            completed,
            "LEFT before\nRIGHT before\nLEFT after\nRIGHT after\nLEFTRIGHT\n",
        ),
        ("early", early, ""),
    ] {
        let c_path = root.join(format!("{name}.c"));
        let bin_path = root.join(format!("asan-structured-task-{name}"));
        fs::write(&c_path, c_source).unwrap();

        let cc_output = Command::new("cc")
            .arg("-fsanitize=address")
            .arg("-fno-omit-frame-pointer")
            .arg("-g")
            .arg(&c_path)
            .arg("-o")
            .arg(&bin_path)
            .output()
            .unwrap();
        assert!(
            cc_output.status.success(),
            "{} compile stdout:\n{}\ncompile stderr:\n{}",
            name,
            String::from_utf8_lossy(&cc_output.stdout),
            String::from_utf8_lossy(&cc_output.stderr)
        );

        let run_output = Command::new(&bin_path)
            .env("ASAN_OPTIONS", asan_options)
            .output()
            .unwrap();
        assert!(
            run_output.status.success(),
            "{} stdout:\n{}\nstderr:\n{}",
            name,
            String::from_utf8_lossy(&run_output.stdout),
            String::from_utf8_lossy(&run_output.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&run_output.stdout),
            expected_stdout,
            "{name}"
        );
        assert!(
            String::from_utf8_lossy(&run_output.stderr).is_empty(),
            "{} stderr:\n{}",
            name,
            String::from_utf8_lossy(&run_output.stderr)
        );
    }

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn structured_spawn_publication_move_is_asan_clean_when_available() {
    let root = temp_test_root("asan-structured-publication-move");
    reset_dir(&root);

    if !cc_supports_address_sanitizer(&root) {
        fs::remove_dir_all(&root).unwrap();
        return;
    }

    let source = root.join("main.nomo");
    let generated_c = root.join("generated.c");
    let bin_path = root.join(if cfg!(windows) {
        "asan-structured-publication-move.exe"
    } else {
        "asan-structured-publication-move"
    });
    fs::write(
        &source,
        r#"package app.main

import std.array
import std.io
import std.result
import std.task

struct AgentMessage {
    content: string
    tags: Array<string>
}

suspend fn consume(message: AgentMessage) -> string {
    task.yield_now()
    return message.content
}

suspend fn launch(message: AgentMessage) -> string {
    task.scope {
        let child = task.spawn consume(message)
        let joined: Result<string, TaskError> = task.join(child)
        return result.unwrap_or(joined, "unavailable")
    }
}

suspend fn main() -> void {
    let content: string = launch(AgentMessage {
        content: "publication",
        tags: ["agent", "task"]
    })
    io.println(content)
}
"#,
    )
    .unwrap();

    let build_output = Command::new(env!("CARGO_BIN_EXE_nomoc"))
        .arg("build")
        .arg(&source)
        .arg("--emit-c")
        .arg("--out")
        .arg(&generated_c)
        .output()
        .unwrap();
    assert!(
        build_output.status.success(),
        "{}",
        String::from_utf8_lossy(&build_output.stderr)
    );

    let generated = fs::read_to_string(&generated_c).unwrap();
    assert!(
        generated.contains(
            "frame->nomo_async_child_0.nomo_async_parameter_nomo_message = nomo_message;"
        )
    );
    assert!(generated.contains("frame->nomo_async_parameter_owned_nomo_message = 0u;"));
    assert!(!generated.contains(
        "frame->nomo_async_child_0.nomo_async_parameter_nomo_message = nomo_retain_AgentMessage"
    ));

    let cc_output = Command::new("cc")
        .arg("-fsanitize=address")
        .arg("-fno-omit-frame-pointer")
        .arg("-g")
        .arg(&generated_c)
        .arg("-o")
        .arg(&bin_path)
        .output()
        .unwrap();
    assert!(
        cc_output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&cc_output.stdout),
        String::from_utf8_lossy(&cc_output.stderr)
    );

    let asan_options = if cfg!(target_os = "macos") {
        "detect_leaks=0:abort_on_error=1"
    } else {
        "detect_leaks=1:abort_on_error=1"
    };
    let run_output = Command::new(&bin_path)
        .env("ASAN_OPTIONS", asan_options)
        .output()
        .unwrap();
    assert!(
        run_output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&run_output.stdout),
        String::from_utf8_lossy(&run_output.stderr)
    );
    assert_eq!(run_output.stdout, b"publication\n");

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn structured_scope_auto_cancel_is_asan_clean_when_available() {
    let root = temp_test_root("asan-structured-scope-auto-cancel");
    reset_dir(&root);

    if !cc_supports_address_sanitizer(&root) {
        fs::remove_dir_all(&root).unwrap();
        return;
    }

    let source = root.join("main.nomo");
    let generated_c = root.join("generated.c");
    let bin_path = root.join(if cfg!(windows) {
        "asan-structured-scope-auto-cancel.exe"
    } else {
        "asan-structured-scope-auto-cancel"
    });
    let metrics_path = root.join("metrics.json");
    fs::write(
        &source,
        r#"package app.main

import std.io
import std.task
import std.time

suspend fn slow_child(value: string) -> void {
    io.println(value, "before")
    let waited: Result<void, TaskError> = task.sleep(time.duration_millis(10000))
    io.println(value, "after")
}

suspend fn gate_child() -> void {
    io.println("gate")
}

suspend fn queued_child(value: string) -> void {
    io.println(value, "unexpected")
}

suspend fn main() -> void {
    task.scope {
        let slow = task.spawn slow_child("managed")
        let gate = task.spawn gate_child()
        let joined_gate: Result<void, TaskError> = task.join(gate)
        let queued_alpha = task.spawn queued_child("alpha-managed")
        let queued_zebra = task.spawn queued_child("zebra-managed")
    }
    io.println("scope closed")
}
"#,
    )
    .unwrap();

    let build_output = Command::new(env!("CARGO_BIN_EXE_nomoc"))
        .arg("build")
        .arg(&source)
        .arg("--emit-c")
        .arg("--out")
        .arg(&generated_c)
        .output()
        .unwrap();
    assert!(
        build_output.status.success(),
        "{}",
        String::from_utf8_lossy(&build_output.stderr)
    );

    let generated = fs::read_to_string(&generated_c).unwrap();
    assert!(generated.contains("nomo_async_ready_cancel_frame(context, frame);"));
    assert!(generated.contains("nomo_async_cancel_slow_child"));
    assert!(generated.contains("nomo_async_cancel_queued_child"));
    assert!(generated.contains("nomo_async_parameter_owned_nomo_value"));

    let cc_output = Command::new("cc")
        .arg("-fsanitize=address")
        .arg("-fno-omit-frame-pointer")
        .arg("-g")
        .arg(&generated_c)
        .arg("-o")
        .arg(&bin_path)
        .output()
        .unwrap();
    assert!(
        cc_output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&cc_output.stdout),
        String::from_utf8_lossy(&cc_output.stderr)
    );

    let asan_options = if cfg!(target_os = "macos") {
        "detect_leaks=0:abort_on_error=1"
    } else {
        "detect_leaks=1:abort_on_error=1"
    };
    let run_output = Command::new(&bin_path)
        .env("ASAN_OPTIONS", asan_options)
        .env("NOMO_ASYNC_METRICS_PATH", &metrics_path)
        .output()
        .unwrap();
    assert!(
        run_output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run_output.stdout),
        String::from_utf8_lossy(&run_output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&run_output.stdout),
        "managed before\ngate\nscope closed\n"
    );
    assert!(run_output.stderr.is_empty());

    let metrics: serde_json::Value =
        serde_json::from_slice(&fs::read(&metrics_path).unwrap()).unwrap();
    assert_eq!(metrics["counters"]["frame_drops"], 3);
    assert_eq!(metrics["counters"]["ready_queue_enqueues"], 5);
    assert_eq!(metrics["counters"]["ready_queue_dequeues"], 3);
    assert_eq!(metrics["counters"]["ready_queue_cancellations"], 2);
    assert_eq!(metrics["counters"]["task_spawns"], 4);
    assert_eq!(metrics["counters"]["task_cancellations"], 3);
    assert_eq!(metrics["counters"]["timer_registrations"], 1);
    assert_eq!(metrics["counters"]["timer_expirations"], 0);
    assert_eq!(metrics["counters"]["timer_cancellations"], 1);
    assert_eq!(metrics["counters"]["live_timers"], 0);

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn io_owned_temporaries_are_asan_clean_when_available() {
    let root = temp_test_root("asan-io-owned-temporaries");
    reset_dir(&root);

    if !cc_supports_address_sanitizer(&root) {
        fs::remove_dir_all(&root).unwrap();
        return;
    }

    let source = root.join("main.nomo");
    let generated_c = root.join("generated.c");
    let bin_path = root.join("asan-io-owned-temporaries");
    fs::write(
        &source,
        r#"package app.main

import std.array
import std.io
import std.string

fn render() -> string {
    return string.to_upper("call")
}

fn main() -> void {
    let message: string = "borrowed"
    let values: Array<string> = ["first"]
    io.print(message, 7)
    io.println("done", 8)
    io.println(values[0])
    io.println(render())
    io.eprint("error", 9)
    io.eprintln("done", 10)
    defer io.println("deferred", 11)
}
"#,
    )
    .unwrap();

    let build_output = Command::new(env!("CARGO_BIN_EXE_nomoc"))
        .arg("build")
        .arg(&source)
        .arg("--emit-c")
        .arg("--out")
        .arg(&generated_c)
        .output()
        .unwrap();
    assert!(
        build_output.status.success(),
        "{}",
        String::from_utf8_lossy(&build_output.stderr)
    );

    let generated = fs::read_to_string(&generated_c).unwrap();
    assert!(!generated.contains("nomo_string_concat(nomo_message"));
    assert!(generated.contains("nomo_string nomo__io_value = nomo_num_i64_to_string(7);"));
    assert!(generated.contains("nomo_string nomo__io_value = nomo_fn_render();"));
    assert!(generated.contains("nomo_string_release(nomo__io_value);"));

    let cc_output = Command::new("cc")
        .arg("-fsanitize=address")
        .arg("-fno-omit-frame-pointer")
        .arg("-g")
        .arg(&generated_c)
        .arg("-o")
        .arg(&bin_path)
        .output()
        .unwrap();
    assert!(
        cc_output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&cc_output.stdout),
        String::from_utf8_lossy(&cc_output.stderr)
    );

    let asan_options = if cfg!(target_os = "macos") {
        "detect_leaks=0:abort_on_error=1"
    } else {
        "detect_leaks=1:abort_on_error=1"
    };
    let run_output = Command::new(&bin_path)
        .env("ASAN_OPTIONS", asan_options)
        .output()
        .unwrap();
    assert!(
        run_output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run_output.stdout),
        String::from_utf8_lossy(&run_output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&run_output.stdout),
        "borrowed 7done 8\nfirst\nCALL\ndeferred 11\n"
    );
    assert_eq!(
        String::from_utf8_lossy(&run_output.stderr),
        "error 9done 10\n"
    );

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn async_ready_queue_is_bounded_fifo_across_wraparound() {
    let root = temp_test_root("async-ready-queue-fifo");
    reset_dir(&root);
    let source = root.join("main.nomo");
    let generated_c = root.join("generated.c");
    let binary = root.join(if cfg!(windows) {
        "async-ready-queue.exe"
    } else {
        "async-ready-queue"
    });
    fs::write(
        &source,
        r#"package app.main

import std.task

suspend fn main() -> void {
    task.yield_now()
}
"#,
    )
    .unwrap();

    let build_output = Command::new(env!("CARGO_BIN_EXE_nomoc"))
        .arg("build")
        .arg(&source)
        .arg("--emit-c")
        .arg("--out")
        .arg(&generated_c)
        .output()
        .unwrap();
    assert!(
        build_output.status.success(),
        "{}",
        String::from_utf8_lossy(&build_output.stderr)
    );

    let mut generated = fs::read_to_string(&generated_c).unwrap();
    let main_start = generated
        .find("int main(void)")
        .expect("generated C must contain main");
    generated.insert_str(
        main_start,
        r#"typedef struct {
    uint32_t id;
} nomo_async_queue_probe_frame;

static nomo_async_poll nomo_async_queue_probe_poll(
    void *raw_frame,
    nomo_async_context *context
) {
    (void)raw_frame;
    (void)context;
    return NOMO_ASYNC_POLL_READY;
}

static int nomo_async_ready_queue_probe(void) {
    nomo_async_context context = {0};
    nomo_async_queue_probe_frame frames[96] = {0};
    for (uint32_t index = 0u; index < 96u; index += 1u) {
        frames[index].id = index;
    }
    for (uint32_t index = 0u; index < NOMO_ASYNC_READY_CAPACITY; index += 1u) {
        if (nomo_async_ready_enqueue(
                &context,
                &frames[index],
                nomo_async_queue_probe_poll
            ) != 0) {
            return 1;
        }
    }
    if (nomo_async_ready_enqueue(
            &context,
            &frames[64],
            nomo_async_queue_probe_poll
        ) == 0) {
        return 2;
    }
    for (uint32_t expected = 0u; expected < 32u; expected += 1u) {
        void *raw_frame = NULL;
        nomo_async_poll_fn poll = NULL;
        if (nomo_async_ready_dequeue(&context, &raw_frame, &poll) != 0
            || poll != nomo_async_queue_probe_poll
            || ((nomo_async_queue_probe_frame *)raw_frame)->id != expected) {
            return 3;
        }
    }
    for (uint32_t index = 64u; index < 96u; index += 1u) {
        if (nomo_async_ready_enqueue(
                &context,
                &frames[index],
                nomo_async_queue_probe_poll
            ) != 0) {
            return 4;
        }
    }
    for (uint32_t expected = 32u; expected < 96u; expected += 1u) {
        void *raw_frame = NULL;
        nomo_async_poll_fn poll = NULL;
        if (nomo_async_ready_dequeue(&context, &raw_frame, &poll) != 0
            || poll != nomo_async_queue_probe_poll
            || ((nomo_async_queue_probe_frame *)raw_frame)->id != expected) {
            return 5;
        }
    }
    if (context.ready_count != 0u
        || context.ready_queue_enqueues != 96u
        || context.ready_queue_dequeues != 96u
        || context.ready_queue_saturations != 1u) {
        return 6;
    }
    return 0;
}

"#,
    );
    generated = generated.replacen(
        "int main(void) {\n",
        "int main(void) {\n    if (nomo_async_ready_queue_probe() != 0) {\n        return 77;\n    }\n",
        1,
    );
    fs::write(&generated_c, generated).unwrap();

    let cc_output = Command::new("cc")
        .arg(&generated_c)
        .arg("-o")
        .arg(&binary)
        .output()
        .unwrap();
    assert!(
        cc_output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&cc_output.stdout),
        String::from_utf8_lossy(&cc_output.stderr)
    );
    let output = Command::new(&binary).output().unwrap();
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn async_runtime_exports_versioned_counters_without_polluting_program_output() {
    let root = temp_test_root("async-runtime-counters");
    reset_dir(&root);
    let project = root.join("counter_probe");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nname = \"counter_probe\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(
        project.join("src/main.nomo"),
        r#"package app.main

import std.io
import std.task

suspend fn yield_once() -> void {
    io.println("child-before")
    task.yield_now()
    io.println("child-after")
}

suspend fn main() -> void {
    io.println("before")
    yield_once()
    task.yield_now()
    io.println("after")
}
"#,
    )
    .unwrap();

    let metrics_path = root.join("runtime-counters.json");
    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(&project)
        .env("NOMO_ASYNC_METRICS_PATH", &metrics_path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "before\nchild-before\nchild-after\nafter\n"
    );
    assert!(output.stderr.is_empty());

    let metrics: serde_json::Value =
        serde_json::from_slice(&fs::read(&metrics_path).unwrap()).unwrap();
    assert_eq!(metrics["schema"], 1);
    assert_eq!(metrics["runtime"], "nomo-c99-current-thread");
    assert_eq!(metrics["runtime_abi"], 1);
    assert_eq!(metrics["counter_catalog_schema"], 1);
    assert_eq!(metrics["counters"]["poll_calls"], 5);
    assert_eq!(metrics["counters"]["cooperative_yields"], 2);
    assert_eq!(metrics["counters"]["frame_allocations"], 0);
    assert_eq!(metrics["counters"]["frame_drops"], 2);
    assert_eq!(metrics["counters"]["peak_live_frames"], 2);
    assert_eq!(metrics["counters"]["ready_queue_enqueues"], 2);
    assert_eq!(metrics["counters"]["ready_queue_dequeues"], 2);
    assert_eq!(metrics["counters"]["ready_queue_saturations"], 0);
    assert_eq!(metrics["counters"]["task_spawns"], 0);
    assert_eq!(metrics["counters"]["task_joins"], 0);
    assert_eq!(metrics["counters"]["join_suspensions"], 0);
    assert_eq!(metrics["counters"]["timer_registrations"], 0);
    assert_eq!(metrics["counters"]["timer_expirations"], 0);
    assert_eq!(metrics["counters"]["timer_cancellations"], 0);
    assert_eq!(metrics["counters"]["live_timers"], 0);
    assert_eq!(metrics["counters"]["peak_live_timers"], 0);
    assert_eq!(metrics["counters"]["reactor_initializations"], 0);
    assert_eq!(metrics["counters"]["reactor_waits"], 0);
    assert_eq!(metrics["counters"]["reactor_timeouts"], 0);
    assert_eq!(metrics["counters"]["reactor_completions"], 0);
    assert_eq!(metrics["counters"]["reactor_errors"], 0);
    assert_eq!(metrics["counters"]["reactor_shutdowns"], 0);
    assert_eq!(metrics["counters"]["live_reactors"], 0);
    assert_eq!(metrics["counters"]["peak_live_reactors"], 0);
    assert!(metrics["unavailable"]["local_retain"].is_string());
    assert!(metrics["unavailable"]["local_release"].is_string());
    assert!(metrics["unavailable"]["live_timers"].is_null());
    assert!(
        !fs::read_to_string(&metrics_path)
            .unwrap()
            .contains(metrics_path.to_string_lossy().as_ref())
    );

    let secret_path = root
        .join("super-secret-metrics-directory")
        .join("runtime-counters.json");
    let rejected = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(&project)
        .env("NOMO_ASYNC_METRICS_PATH", &secret_path)
        .output()
        .unwrap();
    assert!(!rejected.status.success());
    assert_eq!(
        String::from_utf8_lossy(&rejected.stdout),
        "before\nchild-before\nchild-after\nafter\n"
    );
    let stderr = String::from_utf8_lossy(&rejected.stderr);
    assert_eq!(
        stderr,
        "error: async metrics export failed\nprogram exited with status 1\n"
    );
    assert!(!stderr.contains("super-secret"));

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn structured_scope_return_cancel_is_asan_clean_when_available() {
    let root = temp_test_root("asan-structured-scope-return-cancel");
    reset_dir(&root);

    if !cc_supports_address_sanitizer(&root) {
        fs::remove_dir_all(&root).unwrap();
        return;
    }

    let source = root.join("main.nomo");
    let generated_c = root.join("generated.c");
    let bin_path = root.join(if cfg!(windows) {
        "asan-structured-scope-return-cancel.exe"
    } else {
        "asan-structured-scope-return-cancel"
    });
    let metrics_path = root.join("metrics.json");
    fs::write(
        &source,
        r#"package app.main

import std.io
import std.string
import std.task
import std.time

suspend fn slow_child(value: string) -> void {
    io.println(value, "before")
    let waited: Result<void, TaskError> = task.sleep(time.duration_millis(10000))
    io.println(value, "after")
}

suspend fn gate_child() -> void {
    io.println("gate")
}

fn prepare(value: string) -> string {
    io.println("return evaluated")
    return value
}

suspend fn finish(value: string) -> string {
    task.scope {
        let child_value: string = value.concat("")
        let slow = task.spawn slow_child(child_value)
        let gate = task.spawn gate_child()
        let joined_gate: Result<void, TaskError> = task.join(gate)
        return prepare(value)
    }
}

suspend fn main() -> void {
    let result: string = finish("managed")
    io.println(result)
}
"#,
    )
    .unwrap();

    let build_output = Command::new(env!("CARGO_BIN_EXE_nomoc"))
        .arg("build")
        .arg(&source)
        .arg("--emit-c")
        .arg("--out")
        .arg(&generated_c)
        .output()
        .unwrap();
    assert!(
        build_output.status.success(),
        "{}",
        String::from_utf8_lossy(&build_output.stderr)
    );

    let generated = fs::read_to_string(&generated_c).unwrap();
    assert!(generated.contains(concat!(
        "            nomo_string nomo___nomo_structured_return_value = nomo_fn_prepare(nomo_value);\n",
        "            nomo_async_cancel_slow_child(&frame->nomo_async_child_1, context);\n",
        "            nomo_async_drop_slow_child(&frame->nomo_async_child_1);\n",
        "            frame->nomo_async_result = nomo___nomo_structured_return_value;"
    )));
    assert!(generated.contains("frame->nomo_async_result_owned = 1u;"));
    assert!(generated.contains("nomo_string_release(frame->nomo_async_parameter_nomo_value);"));

    let cc_output = Command::new("cc")
        .arg("-fsanitize=address")
        .arg("-fno-omit-frame-pointer")
        .arg("-g")
        .arg(&generated_c)
        .arg("-o")
        .arg(&bin_path)
        .output()
        .unwrap();
    assert!(
        cc_output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&cc_output.stdout),
        String::from_utf8_lossy(&cc_output.stderr)
    );

    let asan_options = if cfg!(target_os = "macos") {
        "detect_leaks=0:abort_on_error=1"
    } else {
        "detect_leaks=1:abort_on_error=1"
    };
    let run_output = Command::new(&bin_path)
        .env("ASAN_OPTIONS", asan_options)
        .env("NOMO_ASYNC_METRICS_PATH", &metrics_path)
        .output()
        .unwrap();
    assert!(
        run_output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run_output.stdout),
        String::from_utf8_lossy(&run_output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&run_output.stdout),
        "managed before\ngate\nreturn evaluated\nmanaged\n"
    );
    assert!(run_output.stderr.is_empty());

    let metrics: serde_json::Value =
        serde_json::from_slice(&fs::read(&metrics_path).unwrap()).unwrap();
    assert_eq!(metrics["counters"]["frame_drops"], 4);
    assert_eq!(metrics["counters"]["ready_queue_enqueues"], 3);
    assert_eq!(metrics["counters"]["ready_queue_dequeues"], 3);
    assert_eq!(metrics["counters"]["task_spawns"], 2);
    assert_eq!(metrics["counters"]["task_cancellations"], 1);
    assert_eq!(metrics["counters"]["timer_registrations"], 1);
    assert_eq!(metrics["counters"]["timer_expirations"], 0);
    assert_eq!(metrics["counters"]["timer_cancellations"], 1);
    assert_eq!(metrics["counters"]["live_timers"], 0);

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn structured_scope_question_propagation_cancels_unjoined_child_and_is_asan_clean() {
    let root = temp_test_root("structured-scope-question-cancel");
    reset_dir(&root);
    let project = root.join("structured_scope_question_cancel");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nname = \"structured_scope_question_cancel\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(
        project.join("src/main.nomo"),
        r#"package app.main

import std.io
import std.num
import std.result
import std.task
import std.time

suspend fn slow_child(value: string) -> void {
    io.println(value, "before")
    let waited: Result<void, TaskError> = task.sleep(time.duration_millis(10000))
    io.println(value, "after")
}

suspend fn gate_child() -> void {
    io.println("gate")
}

suspend fn finish(value: string) -> Result<void, NumError> {
    task.scope {
        let slow = task.spawn slow_child(value)
        let gate = task.spawn gate_child()
        let joined_gate: Result<void, TaskError> = task.join(gate)
        let parsed: i64 = num.parse_i64("not-a-number")?
        return Ok(void)
    }
}

suspend fn main() -> void {
    let outcome: Result<void, NumError> = finish("managed")
    io.println(result.is_err(outcome))
}
"#,
    )
    .unwrap();

    let metrics_path = root.join("structured-question-cancel-counters.json");
    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(&project)
        .env("NOMO_ASYNC_METRICS_PATH", &metrics_path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "managed before\ngate\ntrue\n"
    );
    assert!(output.stderr.is_empty());

    let generated_c = project.join("build/c/main.c");
    let generated = fs::read_to_string(&generated_c).unwrap();
    let question = generated
        .find("nomo_async_question_result_3 = nomo_num_parse_i64")
        .unwrap();
    let result_owned = generated[question..]
        .find("frame->nomo_async_result_owned = 1u;")
        .map(|index| question + index)
        .unwrap();
    let cancel = generated[result_owned..]
        .find("nomo_async_cancel_slow_child(&frame->nomo_async_child_0, context);")
        .map(|index| result_owned + index)
        .unwrap();
    let drop_child = generated[cancel..]
        .find("nomo_async_drop_slow_child(&frame->nomo_async_child_0);")
        .map(|index| cancel + index)
        .unwrap();
    let complete = generated[drop_child..]
        .find("frame->structured_completed = 1u;")
        .map(|index| drop_child + index)
        .unwrap();
    assert!(question < result_owned);
    assert!(result_owned < cancel);
    assert!(cancel < drop_child);
    assert!(drop_child < complete);

    let metrics: serde_json::Value =
        serde_json::from_slice(&fs::read(&metrics_path).unwrap()).unwrap();
    assert_eq!(metrics["counters"]["poll_calls"], 6);
    assert_eq!(metrics["counters"]["cooperative_yields"], 0);
    assert_eq!(metrics["counters"]["frame_allocations"], 0);
    assert_eq!(metrics["counters"]["frame_drops"], 4);
    assert_eq!(metrics["counters"]["peak_live_frames"], 4);
    assert_eq!(metrics["counters"]["ready_queue_enqueues"], 3);
    assert_eq!(metrics["counters"]["ready_queue_dequeues"], 3);
    assert_eq!(metrics["counters"]["ready_queue_saturations"], 0);
    assert_eq!(metrics["counters"]["ready_queue_cancellations"], 0);
    assert_eq!(metrics["counters"]["task_spawns"], 2);
    assert_eq!(metrics["counters"]["task_joins"], 1);
    assert_eq!(metrics["counters"]["join_suspensions"], 1);
    assert_eq!(metrics["counters"]["task_cancellations"], 1);
    assert_eq!(metrics["counters"]["timer_registrations"], 1);
    assert_eq!(metrics["counters"]["timer_expirations"], 0);
    assert_eq!(metrics["counters"]["timer_cancellations"], 1);
    assert_eq!(metrics["counters"]["live_timers"], 0);
    assert_eq!(metrics["counters"]["peak_live_timers"], 1);

    if cc_supports_address_sanitizer(&root) {
        let bin_path = root.join(if cfg!(windows) {
            "asan-structured-scope-question-cancel.exe"
        } else {
            "asan-structured-scope-question-cancel"
        });
        let asan_metrics_path = root.join("asan-metrics.json");
        let cc_output = Command::new("cc")
            .arg("-fsanitize=address")
            .arg("-fno-omit-frame-pointer")
            .arg("-g")
            .arg(&generated_c)
            .arg("-o")
            .arg(&bin_path)
            .output()
            .unwrap();
        assert!(
            cc_output.status.success(),
            "{}{}",
            String::from_utf8_lossy(&cc_output.stdout),
            String::from_utf8_lossy(&cc_output.stderr)
        );
        let asan_options = if cfg!(target_os = "macos") {
            "detect_leaks=0:abort_on_error=1"
        } else {
            "detect_leaks=1:abort_on_error=1"
        };
        let asan_output = Command::new(&bin_path)
            .env("ASAN_OPTIONS", asan_options)
            .env("NOMO_ASYNC_METRICS_PATH", &asan_metrics_path)
            .output()
            .unwrap();
        assert!(
            asan_output.status.success(),
            "stdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&asan_output.stdout),
            String::from_utf8_lossy(&asan_output.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&asan_output.stdout),
            "managed before\ngate\ntrue\n"
        );
        assert!(asan_output.stderr.is_empty());
        let asan_metrics: serde_json::Value =
            serde_json::from_slice(&fs::read(&asan_metrics_path).unwrap()).unwrap();
        assert_eq!(asan_metrics["counters"], metrics["counters"]);
    }

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn structured_scope_option_question_none_cancels_live_child_before_completion() {
    let root = temp_test_root("structured-scope-option-question-cancel");
    reset_dir(&root);
    let project = root.join("structured_scope_option_question_cancel");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nname = \"structured_scope_option_question_cancel\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(
        project.join("src/main.nomo"),
        r#"package app.main

import std.io
import std.option
import std.task
import std.time

fn missing() -> Option<string> {
    return None
}

suspend fn slow_child() -> void {
    io.println("slow before")
    let waited: Result<void, TaskError> = task.sleep(time.duration_millis(10000))
    io.println("slow after")
}

suspend fn gate_child() -> void {
    io.println("gate")
}

suspend fn finish() -> Option<void> {
    task.scope {
        let slow = task.spawn slow_child()
        let gate = task.spawn gate_child()
        let joined_gate: Result<void, TaskError> = task.join(gate)
        let value: string = missing()?
        return Some(void)
    }
}

suspend fn main() -> void {
    let outcome: Option<void> = finish()
    io.println(option.is_none(outcome))
}
"#,
    )
    .unwrap();

    let metrics_path = root.join("structured-option-question-cancel-counters.json");
    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(&project)
        .env("NOMO_ASYNC_METRICS_PATH", &metrics_path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "slow before\ngate\ntrue\n"
    );
    assert!(output.stderr.is_empty());

    let generated = fs::read_to_string(project.join("build/c/main.c")).unwrap();
    let question = generated
        .find("nomo_async_question_result_3 = nomo_fn_missing()")
        .unwrap();
    let none_result = generated[question..]
        .find("frame->nomo_async_result = (nomo_enum_Option_void){.tag = nomo_enum_Option_void_None};")
        .map(|index| question + index)
        .unwrap();
    let cancel = generated[none_result..]
        .find("nomo_async_cancel_slow_child(&frame->nomo_async_child_0, context);")
        .map(|index| none_result + index)
        .unwrap();
    let complete = generated[cancel..]
        .find("frame->structured_completed = 1u;")
        .map(|index| cancel + index)
        .unwrap();
    assert!(question < none_result);
    assert!(none_result < cancel);
    assert!(cancel < complete);
    assert!(generated[none_result..cancel].contains("frame->nomo_async_result_owned = 1u;"));

    let metrics: serde_json::Value =
        serde_json::from_slice(&fs::read(&metrics_path).unwrap()).unwrap();
    assert_eq!(metrics["counters"]["task_spawns"], 2);
    assert_eq!(metrics["counters"]["task_joins"], 1);
    assert_eq!(metrics["counters"]["task_cancellations"], 1);
    assert_eq!(metrics["counters"]["timer_registrations"], 1);
    assert_eq!(metrics["counters"]["timer_expirations"], 0);
    assert_eq!(metrics["counters"]["timer_cancellations"], 1);
    assert_eq!(metrics["counters"]["live_timers"], 0);

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn structured_explicit_cancel_consumes_child_and_is_asan_clean() {
    let root = temp_test_root("structured-explicit-cancel");
    reset_dir(&root);
    let project = root.join("structured_explicit_cancel");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nname = \"structured_explicit_cancel\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(
        project.join("src/main.nomo"),
        r#"package app.main

import std.io
import std.result
import std.task
import std.time

suspend fn slow_child(value: string) -> void {
    io.println(value, "before")
    let waited: Result<void, TaskError> = task.sleep(time.duration_millis(10000))
    io.println(value, "after")
}

suspend fn gate() -> void {
    task.yield_now()
}

suspend fn main() -> void {
    task.scope {
        let child = task.spawn slow_child("managed")
        let gate_task = task.spawn gate()
        let gate_joined: Result<void, TaskError> = task.join(gate_task)
        let cancelled: Result<void, TaskError> = task.cancel(child)
        io.println("cancelled", result.is_ok(cancelled))
    }
}
"#,
    )
    .unwrap();

    let metrics_path = root.join("structured-explicit-cancel-counters.json");
    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(&project)
        .env("NOMO_ASYNC_METRICS_PATH", &metrics_path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "managed before\ncancelled true\n"
    );
    assert!(output.stderr.is_empty());

    let generated_c = project.join("build/c/main.c");
    let generated = fs::read_to_string(&generated_c).unwrap();
    let cancel = generated
        .find("nomo_async_cancel_slow_child(&frame->nomo_async_child_0, context);")
        .unwrap();
    let pending = generated[cancel..]
        .find("context->pending_reason = NOMO_ASYNC_PENDING_CANCEL;")
        .map(|index| cancel + index)
        .unwrap();
    let result = generated[pending..]
        .find("frame->nomo_async_cancel_join_result_3.tag")
        .map(|index| pending + index)
        .unwrap();
    let drop_child = generated[result..]
        .find("nomo_async_drop_slow_child(&frame->nomo_async_child_0);")
        .map(|index| result + index)
        .unwrap();
    assert!(cancel < pending);
    assert!(pending < result);
    assert!(result < drop_child);

    let metrics: serde_json::Value =
        serde_json::from_slice(&fs::read(&metrics_path).unwrap()).unwrap();
    assert_eq!(metrics["counters"]["poll_calls"], 5);
    assert_eq!(metrics["counters"]["cooperative_yields"], 1);
    assert_eq!(metrics["counters"]["frame_allocations"], 0);
    assert_eq!(metrics["counters"]["frame_drops"], 3);
    assert_eq!(metrics["counters"]["peak_live_frames"], 3);
    assert_eq!(metrics["counters"]["ready_queue_enqueues"], 4);
    assert_eq!(metrics["counters"]["ready_queue_dequeues"], 4);
    assert_eq!(metrics["counters"]["ready_queue_saturations"], 0);
    assert_eq!(metrics["counters"]["ready_queue_cancellations"], 0);
    assert_eq!(metrics["counters"]["task_spawns"], 2);
    assert_eq!(metrics["counters"]["task_joins"], 1);
    assert_eq!(metrics["counters"]["join_suspensions"], 1);
    assert_eq!(metrics["counters"]["task_cancellations"], 1);
    assert_eq!(metrics["counters"]["timer_registrations"], 1);
    assert_eq!(metrics["counters"]["timer_expirations"], 0);
    assert_eq!(metrics["counters"]["timer_cancellations"], 1);
    assert_eq!(metrics["counters"]["live_timers"], 0);
    assert_eq!(metrics["counters"]["peak_live_timers"], 1);

    if cc_supports_address_sanitizer(&root) {
        let bin_path = root.join(if cfg!(windows) {
            "asan-structured-explicit-cancel.exe"
        } else {
            "asan-structured-explicit-cancel"
        });
        let asan_metrics_path = root.join("asan-metrics.json");
        let cc_output = Command::new("cc")
            .arg("-fsanitize=address")
            .arg("-fno-omit-frame-pointer")
            .arg("-g")
            .arg(&generated_c)
            .arg("-o")
            .arg(&bin_path)
            .output()
            .unwrap();
        assert!(
            cc_output.status.success(),
            "{}{}",
            String::from_utf8_lossy(&cc_output.stdout),
            String::from_utf8_lossy(&cc_output.stderr)
        );
        let asan_options = if cfg!(target_os = "macos") {
            "detect_leaks=0:abort_on_error=1"
        } else {
            "detect_leaks=1:abort_on_error=1"
        };
        let asan_output = Command::new(&bin_path)
            .env("ASAN_OPTIONS", asan_options)
            .env("NOMO_ASYNC_METRICS_PATH", &asan_metrics_path)
            .output()
            .unwrap();
        assert!(
            asan_output.status.success(),
            "stdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&asan_output.stdout),
            String::from_utf8_lossy(&asan_output.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&asan_output.stdout),
            "managed before\ncancelled true\n"
        );
        assert!(asan_output.stderr.is_empty());
        let asan_metrics: serde_json::Value =
            serde_json::from_slice(&fs::read(&asan_metrics_path).unwrap()).unwrap();
        assert_eq!(asan_metrics["counters"], metrics["counters"]);
    }

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn structured_explicit_cancel_handles_not_started_and_completed_children() {
    let root = temp_test_root("structured-explicit-cancel-terminal-states");
    reset_dir(&root);
    let project = root.join("structured_explicit_cancel_terminal_states");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nname = \"structured_explicit_cancel_terminal_states\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(
        project.join("src/main.nomo"),
        r#"package app.main

import std.io
import std.result
import std.task

suspend fn ready_child(value: string) -> void {
    io.println(value)
}

suspend fn gate() -> void {
    task.yield_now()
}

suspend fn main() -> void {
    task.scope {
        let before_start = task.spawn ready_child("before-start body")
        let before_start_cancelled: Result<void, TaskError> = task.cancel(before_start)
        io.println("before-start", result.is_ok(before_start_cancelled))
    }
    task.scope {
        let completed = task.spawn ready_child("completed body")
        let completed_gate = task.spawn gate()
        let completed_gate_joined: Result<void, TaskError> = task.join(completed_gate)
        let completed_cancelled: Result<void, TaskError> = task.cancel(completed)
        io.println("completed", result.is_ok(completed_cancelled))
    }
}
"#,
    )
    .unwrap();

    let metrics_path = root.join("structured-explicit-cancel-terminal-states.json");
    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(&project)
        .env("NOMO_ASYNC_METRICS_PATH", &metrics_path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "before-start true\ncompleted body\ncompleted true\n"
    );
    assert!(output.stderr.is_empty());

    let metrics: serde_json::Value =
        serde_json::from_slice(&fs::read(&metrics_path).unwrap()).unwrap();
    assert_eq!(metrics["counters"]["poll_calls"], 5);
    assert_eq!(metrics["counters"]["cooperative_yields"], 1);
    assert_eq!(metrics["counters"]["frame_allocations"], 0);
    assert_eq!(metrics["counters"]["frame_drops"], 3);
    assert_eq!(metrics["counters"]["peak_live_frames"], 3);
    assert_eq!(metrics["counters"]["ready_queue_enqueues"], 5);
    assert_eq!(metrics["counters"]["ready_queue_dequeues"], 4);
    assert_eq!(metrics["counters"]["ready_queue_saturations"], 0);
    assert_eq!(metrics["counters"]["ready_queue_cancellations"], 1);
    assert_eq!(metrics["counters"]["task_spawns"], 3);
    assert_eq!(metrics["counters"]["task_joins"], 1);
    assert_eq!(metrics["counters"]["join_suspensions"], 1);
    assert_eq!(metrics["counters"]["task_cancellations"], 1);
    assert_eq!(metrics["counters"]["timer_registrations"], 0);
    assert_eq!(metrics["counters"]["timer_expirations"], 0);
    assert_eq!(metrics["counters"]["timer_cancellations"], 0);
    assert_eq!(metrics["counters"]["live_timers"], 0);

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn structured_deadline_times_out_child_and_is_asan_clean() {
    let root = temp_test_root("structured-deadline-timeout");
    reset_dir(&root);
    let project = root.join("structured_deadline_timeout");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nname = \"structured_deadline_timeout\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(
        project.join("src/main.nomo"),
        r#"package app.main

import std.io
import std.result
import std.task
import std.time

suspend fn bounded_work() -> string {
    task.deadline(time.duration_millis(5)) {
        let waited: Result<void, TaskError> = task.sleep(time.duration_millis(10000))
        task.check_cancelled()
    }
    return "completed"
}

suspend fn main() -> void {
    task.scope {
        let child = task.spawn bounded_work()
        let joined: Result<string, TaskError> = task.join(child)
        io.println("deadline elapsed", result.is_err(joined))
    }
}
"#,
    )
    .unwrap();

    let metrics_path = root.join("structured-deadline-counters.json");
    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(&project)
        .env("NOMO_ASYNC_METRICS_PATH", &metrics_path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "deadline elapsed true\n"
    );
    assert!(output.stderr.is_empty());

    let metrics: serde_json::Value =
        serde_json::from_slice(&fs::read(&metrics_path).unwrap()).unwrap();
    assert_eq!(metrics["counters"]["poll_calls"], 4);
    assert_eq!(metrics["counters"]["frame_allocations"], 0);
    assert_eq!(metrics["counters"]["frame_drops"], 2);
    assert_eq!(metrics["counters"]["task_spawns"], 1);
    assert_eq!(metrics["counters"]["task_joins"], 1);
    assert_eq!(metrics["counters"]["join_suspensions"], 1);
    assert_eq!(metrics["counters"]["deadline_registrations"], 1);
    assert_eq!(metrics["counters"]["deadline_expirations"], 1);
    assert_eq!(metrics["counters"]["deadline_cancellations"], 0);
    assert_eq!(metrics["counters"]["timer_registrations"], 2);
    assert_eq!(metrics["counters"]["timer_expirations"], 1);
    assert_eq!(metrics["counters"]["timer_cancellations"], 1);
    assert_eq!(metrics["counters"]["live_timers"], 0);
    assert_eq!(metrics["counters"]["peak_live_timers"], 2);

    let generated_c = project.join("build/c/main.c");
    let generated = fs::read_to_string(&generated_c).unwrap();
    assert!(generated.contains("nomo_async_deadline_arm("));
    assert!(generated.contains("NOMO_ASYNC_TASK_FAILURE_TIMEOUT"));
    assert!(generated.contains("nomo_async_task_failure_code"));

    if cc_supports_address_sanitizer(&root) {
        let bin_path = root.join(if cfg!(windows) {
            "asan-structured-deadline.exe"
        } else {
            "asan-structured-deadline"
        });
        let asan_metrics_path = root.join("asan-metrics.json");
        let cc_output = Command::new("cc")
            .arg("-fsanitize=address")
            .arg("-fno-omit-frame-pointer")
            .arg("-g")
            .arg(&generated_c)
            .arg("-o")
            .arg(&bin_path)
            .output()
            .unwrap();
        assert!(
            cc_output.status.success(),
            "{}{}",
            String::from_utf8_lossy(&cc_output.stdout),
            String::from_utf8_lossy(&cc_output.stderr)
        );
        let asan_options = if cfg!(target_os = "macos") {
            "detect_leaks=0:abort_on_error=1"
        } else {
            "detect_leaks=1:abort_on_error=1"
        };
        let asan_output = Command::new(&bin_path)
            .env("ASAN_OPTIONS", asan_options)
            .env("NOMO_ASYNC_METRICS_PATH", &asan_metrics_path)
            .output()
            .unwrap();
        assert!(
            asan_output.status.success(),
            "stdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&asan_output.stdout),
            String::from_utf8_lossy(&asan_output.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&asan_output.stdout),
            "deadline elapsed true\n"
        );
        assert!(asan_output.stderr.is_empty());
        let asan_metrics: serde_json::Value =
            serde_json::from_slice(&fs::read(&asan_metrics_path).unwrap()).unwrap();
        assert_eq!(asan_metrics["counters"], metrics["counters"]);
    }

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn root_zero_deadline_fails_before_body_without_registering_a_timer() {
    let root = temp_test_root("root-zero-deadline");
    reset_dir(&root);
    let project = root.join("root_zero_deadline");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nname = \"root_zero_deadline\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(
        project.join("src/main.nomo"),
        r#"package app.main

import std.io
import std.task
import std.time

suspend fn bounded() -> void {
    task.deadline(time.duration_millis(0)) {
        io.println("deadline-body-secret")
    }
}

suspend fn main() -> void {
    bounded()
    io.println("deadline-caller-after-secret")
}
"#,
    )
    .unwrap();

    let metrics_path = root.join("root-zero-deadline-counters.json");
    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(&project)
        .env("NOMO_ASYNC_METRICS_PATH", &metrics_path)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        stderr,
        "error: async task failed: timeout\nprogram exited with status 1\n"
    );
    assert!(!stderr.contains("deadline-body-secret"));
    assert!(!stderr.contains("deadline-caller-after-secret"));

    let metrics: serde_json::Value =
        serde_json::from_slice(&fs::read(&metrics_path).unwrap()).unwrap();
    assert_eq!(metrics["counters"]["deadline_registrations"], 0);
    assert_eq!(metrics["counters"]["deadline_expirations"], 1);
    assert_eq!(metrics["counters"]["deadline_cancellations"], 0);
    assert_eq!(metrics["counters"]["timer_registrations"], 0);
    assert_eq!(metrics["counters"]["timer_expirations"], 0);
    assert_eq!(metrics["counters"]["timer_cancellations"], 0);
    assert_eq!(metrics["counters"]["live_timers"], 0);

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn positive_deadline_disarms_after_normal_fallthrough() {
    let root = temp_test_root("normal-deadline-fallthrough");
    reset_dir(&root);
    let project = root.join("normal_deadline_fallthrough");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nname = \"normal_deadline_fallthrough\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(
        project.join("src/main.nomo"),
        r#"package app.main

import std.io
import std.task
import std.time

suspend fn main() -> void {
    task.deadline(time.duration_millis(10000)) {
        task.check_cancelled()
        io.println("completed")
    }
}
"#,
    )
    .unwrap();

    let metrics_path = root.join("normal-deadline-counters.json");
    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(&project)
        .env("NOMO_ASYNC_METRICS_PATH", &metrics_path)
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "completed\n");
    assert!(output.stderr.is_empty());

    let metrics: serde_json::Value =
        serde_json::from_slice(&fs::read(&metrics_path).unwrap()).unwrap();
    assert_eq!(metrics["counters"]["deadline_registrations"], 1);
    assert_eq!(metrics["counters"]["deadline_expirations"], 0);
    assert_eq!(metrics["counters"]["deadline_cancellations"], 1);
    assert_eq!(metrics["counters"]["timer_registrations"], 1);
    assert_eq!(metrics["counters"]["timer_expirations"], 0);
    assert_eq!(metrics["counters"]["timer_cancellations"], 1);
    assert_eq!(metrics["counters"]["live_timers"], 0);

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn structured_cancel_observes_an_already_failed_deadline_child() {
    let root = temp_test_root("cancel-failed-deadline-child");
    reset_dir(&root);
    let project = root.join("cancel_failed_deadline_child");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nname = \"cancel_failed_deadline_child\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(
        project.join("src/main.nomo"),
        r#"package app.main

import std.io
import std.result
import std.task
import std.time

suspend fn bounded() -> void {
    task.deadline(time.duration_millis(0)) {
        io.println("failed-child-body-secret")
    }
}

suspend fn gate() -> void {
    task.yield_now()
}

suspend fn main() -> void {
    task.scope {
        let failed = task.spawn bounded()
        let gate_task = task.spawn gate()
        let gate_joined: Result<void, TaskError> = task.join(gate_task)
        let cancelled: Result<void, TaskError> = task.cancel(failed)
        io.println("cancel observed timeout", result.is_err(cancelled))
    }
}
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(&project)
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "cancel observed timeout true\n"
    );
    assert!(output.stderr.is_empty());

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn structured_child_panic_cancels_root_siblings_and_is_asan_clean() {
    let root = temp_test_root("structured-child-panic-cleanup");
    reset_dir(&root);
    let project = root.join("structured_child_panic_cleanup");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nname = \"structured_child_panic_cleanup\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(
        project.join("src/main.nomo"),
        r#"package app.main

import std.io
import std.string
import std.task
import std.time

suspend fn slow_child(value: string) -> void {
    io.println(value, "slow before")
    let waited: Result<void, TaskError> = task.sleep(time.duration_millis(10000))
    io.println(value, "slow after")
}

suspend fn panicking_child(prefix: string) -> void {
    task.yield_now()
    let message: string = prefix.concat(" child")
    panic(message)
}

suspend fn main() -> void {
    task.scope {
        let slow = task.spawn slow_child("managed")
        let failure = task.spawn panicking_child("panic from")
        let joined: Result<void, TaskError> = task.join(failure)
    }
}
"#,
    )
    .unwrap();

    let metrics_path = root.join("structured-child-panic-counters.json");
    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(&project)
        .env("NOMO_ASYNC_METRICS_PATH", &metrics_path)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "managed slow before\n"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("panic: panic from child"), "{stderr}");
    assert!(stderr.contains("program exited with status 1"), "{stderr}");

    let generated_c = project.join("build/c/main.c");
    let generated = fs::read_to_string(&generated_c).unwrap();
    let retain = generated
        .find("nomo_async_panic_message_2 = nomo_string_retain(nomo_async_panic_message_2);")
        .unwrap();
    let publish = generated[retain..]
        .find("context->panic_message = nomo_async_panic_message_2;")
        .map(|index| retain + index)
        .unwrap();
    let local_release = generated[publish..]
        .find("nomo_string_release(nomo_message);")
        .map(|index| publish + index)
        .unwrap();
    let propagate = generated[local_release..]
        .find("context->pending_reason = NOMO_ASYNC_PENDING_PANIC;")
        .map(|index| local_release + index)
        .unwrap();
    assert!(retain < publish);
    assert!(publish < local_release);
    assert!(local_release < propagate);
    let root_cancel = generated
        .find("nomo_async_cancel_main(&nomo__frame, &nomo__context);")
        .unwrap();
    let root_drop = generated[root_cancel..]
        .find("nomo_async_drop_main(&nomo__frame);")
        .map(|index| root_cancel + index)
        .unwrap();
    assert!(root_cancel < root_drop);

    let metrics: serde_json::Value =
        serde_json::from_slice(&fs::read(&metrics_path).unwrap()).unwrap();
    assert_eq!(metrics["counters"]["poll_calls"], 4);
    assert_eq!(metrics["counters"]["cooperative_yields"], 1);
    assert_eq!(metrics["counters"]["frame_allocations"], 0);
    assert_eq!(metrics["counters"]["frame_drops"], 3);
    assert_eq!(metrics["counters"]["peak_live_frames"], 3);
    assert_eq!(metrics["counters"]["task_spawns"], 2);
    assert_eq!(metrics["counters"]["task_joins"], 1);
    assert_eq!(metrics["counters"]["join_suspensions"], 1);
    assert_eq!(metrics["counters"]["task_cancellations"], 3);
    assert_eq!(metrics["counters"]["timer_registrations"], 1);
    assert_eq!(metrics["counters"]["timer_expirations"], 0);
    assert_eq!(metrics["counters"]["timer_cancellations"], 1);
    assert_eq!(metrics["counters"]["live_timers"], 0);

    if cc_supports_address_sanitizer(&root) {
        let bin_path = root.join(if cfg!(windows) {
            "asan-structured-child-panic-cleanup.exe"
        } else {
            "asan-structured-child-panic-cleanup"
        });
        let asan_metrics_path = root.join("asan-metrics.json");
        let cc_output = Command::new("cc")
            .arg("-fsanitize=address")
            .arg("-fno-omit-frame-pointer")
            .arg("-g")
            .arg(&generated_c)
            .arg("-o")
            .arg(&bin_path)
            .output()
            .unwrap();
        assert!(
            cc_output.status.success(),
            "{}{}",
            String::from_utf8_lossy(&cc_output.stdout),
            String::from_utf8_lossy(&cc_output.stderr)
        );
        let asan_options = if cfg!(target_os = "macos") {
            "detect_leaks=0:abort_on_error=1"
        } else {
            "detect_leaks=1:abort_on_error=1"
        };
        let asan_output = Command::new(&bin_path)
            .env("ASAN_OPTIONS", asan_options)
            .env("NOMO_ASYNC_METRICS_PATH", &asan_metrics_path)
            .output()
            .unwrap();
        assert!(!asan_output.status.success());
        assert_eq!(
            String::from_utf8_lossy(&asan_output.stdout),
            "managed slow before\n"
        );
        assert_eq!(
            String::from_utf8_lossy(&asan_output.stderr),
            "panic: panic from child\n"
        );
        let asan_metrics: serde_json::Value =
            serde_json::from_slice(&fs::read(&asan_metrics_path).unwrap()).unwrap();
        assert_eq!(asan_metrics["counters"], metrics["counters"]);
    }

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn structured_scope_question_join_spills_managed_success_across_later_suspension() {
    let root = temp_test_root("structured-scope-question-join-success");
    reset_dir(&root);
    let project = root.join("structured_scope_question_join_success");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nname = \"structured_scope_question_join_success\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(
        project.join("src/main.nomo"),
        r#"package app.main

import std.io
import std.result
import std.task

suspend fn worker(value: string) -> string {
    task.yield_now()
    return value
}

suspend fn gather() -> Result<string, TaskError> {
    task.scope {
        let left = task.spawn worker("left")
        let right = task.spawn worker("right")
        let left_value: string = task.join(left)?
        let right_value: string = task.join(right)?
        return Ok(left_value)
    }
}

suspend fn main() -> void {
    let gathered: Result<string, TaskError> = gather()
    io.println(result.unwrap_or(gathered, "failed"))
}
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(&project)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), "left\n");
    assert!(output.stderr.is_empty());

    let generated_c = project.join("build/c/main.c");
    let generated = fs::read_to_string(&generated_c).unwrap();
    assert!(generated.contains("nomo_string nomo_async_local_nomo_left_value;"));
    assert!(generated.contains("uint8_t nomo_async_owned_nomo_left_value;"));
    let question_success = generated
        .find("nomo_left_value = nomo_async_question_result_3.payload.nomo_payload_Ok;")
        .unwrap();
    let retain = generated[question_success..]
        .find("nomo_left_value = nomo_string_retain(nomo_left_value);")
        .map(|index| question_success + index)
        .unwrap();
    let spill = generated[retain..]
        .find("frame->nomo_async_local_nomo_left_value = nomo_left_value;")
        .map(|index| retain + index)
        .unwrap();
    assert!(question_success < retain);
    assert!(retain < spill);
    assert!(generated.contains("nomo_async_cancel_worker(&frame->nomo_async_child_1, context);"));

    if cc_supports_address_sanitizer(&root) {
        let bin_path = root.join(if cfg!(windows) {
            "asan-structured-scope-question-join-success.exe"
        } else {
            "asan-structured-scope-question-join-success"
        });
        let cc_output = Command::new("cc")
            .arg("-fsanitize=address")
            .arg("-fno-omit-frame-pointer")
            .arg("-g")
            .arg(&generated_c)
            .arg("-o")
            .arg(&bin_path)
            .output()
            .unwrap();
        assert!(
            cc_output.status.success(),
            "{}{}",
            String::from_utf8_lossy(&cc_output.stdout),
            String::from_utf8_lossy(&cc_output.stderr)
        );
        let asan_options = if cfg!(target_os = "macos") {
            "detect_leaks=0:abort_on_error=1"
        } else {
            "detect_leaks=1:abort_on_error=1"
        };
        let asan_output = Command::new(&bin_path)
            .env("ASAN_OPTIONS", asan_options)
            .output()
            .unwrap();
        assert!(
            asan_output.status.success(),
            "stdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&asan_output.stdout),
            String::from_utf8_lossy(&asan_output.stderr)
        );
        assert_eq!(String::from_utf8_lossy(&asan_output.stdout), "left\n");
        assert!(asan_output.stderr.is_empty());
    }

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn structured_tasks_use_bounded_fifo_and_surface_typed_queue_saturation() {
    let root = temp_test_root("structured-typed-tasks");
    reset_dir(&root);
    let project = root.join("structured_tasks");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nname = \"structured_tasks\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();

    let mut source = String::from(
        "package app.main\n\nimport std.io\nimport std.result\nimport std.task\n\n\
         suspend fn child() -> string {\n    return \"done\"\n}\n\n\
         suspend fn main() -> void {\n    task.scope {\n",
    );
    for index in 0..65 {
        source.push_str(&format!("        let child_{index} = task.spawn child()\n"));
    }
    for index in 0..65 {
        source.push_str(&format!(
            "        let joined_{index}: Result<string, TaskError> = task.join(child_{index})\n"
        ));
    }
    source.push_str("        io.println(result.is_err(joined_64))\n    }\n}\n");
    fs::write(project.join("src/main.nomo"), source).unwrap();

    let metrics_path = root.join("structured-task-counters.json");
    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(&project)
        .env("NOMO_ASYNC_METRICS_PATH", &metrics_path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), "true\n");
    assert!(output.stderr.is_empty());

    let metrics: serde_json::Value =
        serde_json::from_slice(&fs::read(&metrics_path).unwrap()).unwrap();
    assert_eq!(metrics["counters"]["poll_calls"], 66);
    assert_eq!(metrics["counters"]["cooperative_yields"], 0);
    assert_eq!(metrics["counters"]["frame_allocations"], 0);
    assert_eq!(metrics["counters"]["frame_drops"], 65);
    assert_eq!(metrics["counters"]["peak_live_frames"], 65);
    assert_eq!(metrics["counters"]["ready_queue_enqueues"], 65);
    assert_eq!(metrics["counters"]["ready_queue_dequeues"], 65);
    assert_eq!(metrics["counters"]["ready_queue_saturations"], 1);
    assert_eq!(metrics["counters"]["task_spawns"], 65);
    assert_eq!(metrics["counters"]["task_joins"], 65);
    assert_eq!(metrics["counters"]["join_suspensions"], 1);
    assert_eq!(metrics["counters"]["live_timers"], 0);

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn structured_scope_normal_exit_cancels_unjoined_timer_child() {
    let root = temp_test_root("structured-scope-auto-cancel");
    reset_dir(&root);
    let project = root.join("structured_scope_auto_cancel");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nname = \"structured_scope_auto_cancel\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(
        project.join("src/main.nomo"),
        r#"package app.main

import std.io
import std.task
import std.time

suspend fn slow_child() -> void {
    io.println("slow before")
    let waited: Result<void, TaskError> = task.sleep(time.duration_millis(10000))
    task.scope {
        let never_initialized = task.spawn queued_child("nested")
    }
    io.println("slow after")
}

suspend fn gate_child() -> void {
    io.println("gate")
}

suspend fn queued_child(value: string) -> void {
    io.println(value, "unexpected")
}

suspend fn main() -> void {
    task.scope {
        let slow = task.spawn slow_child()
        let gate = task.spawn gate_child()
        let joined_gate: Result<void, TaskError> = task.join(gate)
        let queued_alpha = task.spawn queued_child("alpha")
        let queued_zebra = task.spawn queued_child("zebra")
    }
    io.println("scope closed")
}
"#,
    )
    .unwrap();

    let metrics_path = root.join("structured-cancel-counters.json");
    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(&project)
        .env("NOMO_ASYNC_METRICS_PATH", &metrics_path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "slow before\ngate\nscope closed\n"
    );
    assert!(output.stderr.is_empty());

    let generated = fs::read_to_string(project.join("build/c/main.c")).unwrap();
    assert!(generated.contains("nomo_async_ready_cancel_frame(context, frame);"));
    assert!(
        generated.contains("nomo_async_cancel_slow_child(&frame->nomo_async_child_0, context);")
    );
    assert!(generated.contains("nomo_async_cancel_queued_child"));
    assert!(generated.contains("if (frame->initialized == 0u || frame->cancelled != 0u"));
    assert!(generated.contains("nomo_async_timer_disarm"));

    let metrics: serde_json::Value =
        serde_json::from_slice(&fs::read(&metrics_path).unwrap()).unwrap();
    assert_eq!(metrics["counters"]["poll_calls"], 4);
    assert_eq!(metrics["counters"]["frame_allocations"], 0);
    assert_eq!(metrics["counters"]["frame_drops"], 3);
    assert_eq!(metrics["counters"]["peak_live_frames"], 3);
    assert_eq!(metrics["counters"]["ready_queue_enqueues"], 5);
    assert_eq!(metrics["counters"]["ready_queue_dequeues"], 3);
    assert_eq!(metrics["counters"]["ready_queue_saturations"], 0);
    assert_eq!(metrics["counters"]["ready_queue_cancellations"], 2);
    assert_eq!(metrics["counters"]["task_spawns"], 4);
    assert_eq!(metrics["counters"]["task_joins"], 1);
    assert_eq!(metrics["counters"]["join_suspensions"], 1);
    assert_eq!(metrics["counters"]["task_cancellations"], 3);
    assert_eq!(metrics["counters"]["timer_registrations"], 1);
    assert_eq!(metrics["counters"]["timer_expirations"], 0);
    assert_eq!(metrics["counters"]["timer_cancellations"], 1);
    assert_eq!(metrics["counters"]["live_timers"], 0);
    assert_eq!(metrics["counters"]["peak_live_timers"], 1);

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn structured_scope_return_cancels_unjoined_child_before_waking_parent() {
    let root = temp_test_root("structured-scope-return-cancel");
    reset_dir(&root);
    let project = root.join("structured_scope_return_cancel");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nname = \"structured_scope_return_cancel\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(
        project.join("src/main.nomo"),
        r#"package app.main

import std.io
import std.string
import std.task
import std.time

suspend fn slow_child(value: string) -> void {
    io.println(value, "before")
    let waited: Result<void, TaskError> = task.sleep(time.duration_millis(10000))
    io.println(value, "after")
}

suspend fn gate_child() -> void {
    io.println("gate")
}

fn prepare(value: string) -> string {
    io.println("return evaluated")
    return value
}

suspend fn finish(value: string) -> string {
    task.scope {
        let child_value: string = value.concat("")
        let slow = task.spawn slow_child(child_value)
        let gate = task.spawn gate_child()
        let joined_gate: Result<void, TaskError> = task.join(gate)
        return prepare(value)
    }
}

suspend fn main() -> void {
    let result: string = finish("managed")
    io.println(result)
}
"#,
    )
    .unwrap();

    let metrics_path = root.join("structured-return-cancel-counters.json");
    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(&project)
        .env("NOMO_ASYNC_METRICS_PATH", &metrics_path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "managed before\ngate\nreturn evaluated\nmanaged\n"
    );
    assert!(output.stderr.is_empty());

    let generated = fs::read_to_string(project.join("build/c/main.c")).unwrap();
    assert!(
        generated.contains(concat!(
            "            nomo_string nomo___nomo_structured_return_value = nomo_fn_prepare(nomo_value);\n",
            "            nomo_async_cancel_slow_child(&frame->nomo_async_child_1, context);\n",
            "            nomo_async_drop_slow_child(&frame->nomo_async_child_1);\n",
            "            frame->nomo_async_result = nomo___nomo_structured_return_value;"
        )),
        "{generated}"
    );

    let metrics: serde_json::Value =
        serde_json::from_slice(&fs::read(&metrics_path).unwrap()).unwrap();
    assert_eq!(metrics["counters"]["poll_calls"], 6);
    assert_eq!(metrics["counters"]["cooperative_yields"], 0);
    assert_eq!(metrics["counters"]["frame_allocations"], 0);
    assert_eq!(metrics["counters"]["frame_drops"], 4);
    assert_eq!(metrics["counters"]["peak_live_frames"], 4);
    assert_eq!(metrics["counters"]["ready_queue_enqueues"], 3);
    assert_eq!(metrics["counters"]["ready_queue_dequeues"], 3);
    assert_eq!(metrics["counters"]["ready_queue_saturations"], 0);
    assert_eq!(metrics["counters"]["ready_queue_cancellations"], 0);
    assert_eq!(metrics["counters"]["task_spawns"], 2);
    assert_eq!(metrics["counters"]["task_joins"], 1);
    assert_eq!(metrics["counters"]["join_suspensions"], 1);
    assert_eq!(metrics["counters"]["task_cancellations"], 1);
    assert_eq!(metrics["counters"]["timer_registrations"], 1);
    assert_eq!(metrics["counters"]["timer_expirations"], 0);
    assert_eq!(metrics["counters"]["timer_cancellations"], 1);
    assert_eq!(metrics["counters"]["live_timers"], 0);
    assert_eq!(metrics["counters"]["peak_live_timers"], 1);

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn bounded_channel_applies_fifo_backpressure_with_exact_counters_and_asan_cleanup() {
    let root = temp_test_root("async-bounded-channel-runtime");
    reset_dir(&root);
    let project = root.join("async_bounded_channel");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nname = \"async_bounded_channel\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(
        project.join("src/main.nomo"),
        r#"package app.main

import std.io
import std.option
import std.result
import std.task

fn make_channel() -> Channel<string> {
    let created: Result<Channel<string>, ChannelError> = task.channel<string>(1)
    return match created {
        Result.Ok(channel_value) => channel_value
        Result.Err(error) => panic(error.message)
    }
}

suspend fn producer(channel_value: Channel<string>) -> void {
    let first: Result<void, ChannelSendError<string>> = task.send(channel_value, "first")
    let second: Result<void, ChannelSendError<string>> = task.send(channel_value, "second")
}

suspend fn consumer(channel_value: Channel<string>) -> void {
    task.yield_now()
    let first: Option<string> = task.receive(channel_value)
    let second: Option<string> = task.receive(channel_value)
    io.println(option.unwrap_or(first, "missing"))
    io.println(option.unwrap_or(second, "missing"))
    task.close(channel_value)
}

suspend fn main() -> void {
    let channel_value: Channel<string> = make_channel()
    task.scope {
        let producer_task = task.spawn producer(channel_value)
        let consumer_task = task.spawn consumer(channel_value)
        let producer_result: Result<void, TaskError> = task.join(producer_task)
        let consumer_result: Result<void, TaskError> = task.join(consumer_task)
    }
}
"#,
    )
    .unwrap();

    let metrics_path = root.join("channel-counters.json");
    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(&project)
        .env("NOMO_ASYNC_METRICS_PATH", &metrics_path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), "first\nsecond\n");
    assert!(output.stderr.is_empty());

    let generated_c = project.join("build/c/main.c");
    let generated = fs::read_to_string(&generated_c).unwrap();
    assert!(generated.contains("nomo_channel_control_string"));
    assert!(generated.contains("nomo_channel_send_start_string"));
    assert!(generated.contains("nomo_channel_receive_start_string"));
    assert!(generated.contains("nomo_channel_send_registration_string"));
    assert!(generated.contains("NOMO_ASYNC_PENDING_CHANNEL"));
    assert!(!generated.contains("pthread_create"));
    assert!(!generated.contains("CreateThread"));
    assert!(!generated.contains("__atomic_"));
    assert!(!generated.contains("Interlocked"));

    let metrics: serde_json::Value =
        serde_json::from_slice(&fs::read(&metrics_path).unwrap()).unwrap();
    assert_eq!(metrics["schema"], 1);
    assert_eq!(metrics["runtime"], "nomo-c99-current-thread");
    assert_eq!(metrics["counters"]["frame_allocations"], 0);
    assert_eq!(metrics["counters"]["channel_constructions"], 1);
    assert_eq!(metrics["counters"]["channel_sends"], 2);
    assert_eq!(metrics["counters"]["channel_receives"], 2);
    assert_eq!(metrics["counters"]["channel_buffered_sends"], 2);
    assert_eq!(metrics["counters"]["channel_buffered_receives"], 2);
    assert_eq!(metrics["counters"]["channel_direct_handoffs"], 0);
    assert_eq!(metrics["counters"]["channel_send_suspensions"], 1);
    assert_eq!(metrics["counters"]["channel_receive_suspensions"], 0);
    assert_eq!(metrics["counters"]["channel_wakeups"], 1);
    assert_eq!(metrics["counters"]["channel_closes"], 1);
    assert_eq!(metrics["counters"]["channel_cancellations"], 0);
    assert_eq!(metrics["counters"]["live_channel_buffered_elements"], 0);
    assert_eq!(
        metrics["counters"]["peak_live_channel_buffered_elements"],
        1
    );
    assert_eq!(metrics["counters"]["live_channel_send_waiters"], 0);
    assert_eq!(metrics["counters"]["peak_live_channel_send_waiters"], 1);
    assert_eq!(metrics["counters"]["live_channel_receive_waiters"], 0);
    assert_eq!(metrics["counters"]["peak_live_channel_receive_waiters"], 0);

    if cc_supports_address_sanitizer(&root) {
        let bin_path = root.join(if cfg!(windows) {
            "asan-async-bounded-channel.exe"
        } else {
            "asan-async-bounded-channel"
        });
        let asan_metrics_path = root.join("asan-channel-counters.json");
        let cc_output = Command::new("cc")
            .arg("-std=c99")
            .arg("-fsanitize=address")
            .arg("-fno-omit-frame-pointer")
            .arg("-g")
            .arg(&generated_c)
            .arg("-o")
            .arg(&bin_path)
            .output()
            .unwrap();
        assert!(
            cc_output.status.success(),
            "{}{}",
            String::from_utf8_lossy(&cc_output.stdout),
            String::from_utf8_lossy(&cc_output.stderr)
        );
        let asan_options = if cfg!(target_os = "macos") {
            "detect_leaks=0:abort_on_error=1"
        } else {
            "detect_leaks=1:abort_on_error=1"
        };
        let asan_output = Command::new(&bin_path)
            .env("ASAN_OPTIONS", asan_options)
            .env("NOMO_ASYNC_METRICS_PATH", &asan_metrics_path)
            .output()
            .unwrap();
        assert!(
            asan_output.status.success(),
            "stdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&asan_output.stdout),
            String::from_utf8_lossy(&asan_output.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&asan_output.stdout),
            "first\nsecond\n"
        );
        assert!(asan_output.stderr.is_empty());
        let asan_metrics: serde_json::Value =
            serde_json::from_slice(&fs::read(&asan_metrics_path).unwrap()).unwrap();
        assert_eq!(asan_metrics["counters"], metrics["counters"]);
    }

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn static_receive_timer_select_is_source_ordered_and_cleans_losers() {
    let root = temp_test_root("async-static-select-runtime");
    reset_dir(&root);
    let project = root.join("async_static_select");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nname = \"async_static_select\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(
        project.join("src/main.nomo"),
        r#"package app.main

import std.io
import std.option
import std.result
import std.task
import std.time

fn make_channel() -> Channel<string> {
    let created: Result<Channel<string>, ChannelError> = task.channel<string>(1)
    return match created {
        Result.Ok(channel_value) => channel_value
        Result.Err(error) => panic(error.message)
    }
}

suspend fn publish(channel_value: Channel<string>) -> void {
    task.yield_now()
    let sent: Result<void, ChannelSendError<string>> = task.send(channel_value, "winner")
}

suspend fn main() -> void {
    let idle_channel: Channel<string> = make_channel()
    let ready_channel: Channel<string> = make_channel()
    let selected_prefix: string = "selected"
    task.scope {
        let publisher = task.spawn publish(ready_channel)
        task.select {
            task.receive(idle_channel) => idle {
                io.println(option.unwrap_or(idle, "idle closed"))
            }
            task.receive(ready_channel) => received {
                io.println(selected_prefix)
                io.println(option.unwrap_or(received, "ready closed"))
            }
            task.sleep(time.duration_millis(100)) => timeout {
                io.println("timeout")
            }
        }
        let publisher_result: Result<void, TaskError> = task.join(publisher)
    }

    let first_channel: Channel<string> = make_channel()
    let second_channel: Channel<string> = make_channel()
    let first_sent: Result<void, ChannelSendError<string>> = task.send(first_channel, "first")
    let second_sent: Result<void, ChannelSendError<string>> = task.send(second_channel, "second")
    task.select {
        task.receive(first_channel) => first {
            io.println(option.unwrap_or(first, "first closed"))
        }
        task.receive(second_channel) => second {
            io.println(option.unwrap_or(second, "second closed"))
        }
        task.sleep(time.duration_millis(0)) => immediate {
            io.println("immediate timer")
        }
    }

    let timer_channel: Channel<string> = make_channel()
    task.select {
        task.receive(timer_channel) => received {
            io.println("unexpected timer receive")
        }
        task.sleep(time.duration_millis(1)) => waited {
            io.println("timer")
        }
    }

    task.close(idle_channel)
    task.close(ready_channel)
    task.close(first_channel)
    task.close(second_channel)
    task.close(timer_channel)
}
"#,
    )
    .unwrap();

    let metrics_path = root.join("select-counters.json");
    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(&project)
        .env("NOMO_ASYNC_METRICS_PATH", &metrics_path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "selected\nwinner\nfirst\ntimer\n"
    );
    assert!(output.stderr.is_empty());

    let generated_c = project.join("build/c/main.c");
    let generated = fs::read_to_string(&generated_c).unwrap();
    assert!(generated.contains("nomo_async_select_claim"));
    assert!(generated.contains("nomo_async_select_suspend"));
    assert!(generated.contains("nomo_channel_receive_select_cancel_string"));
    assert!(generated.contains("nomo_async_timer_select_cancel"));
    assert!(!generated.contains("pthread_create"));
    assert!(!generated.contains("CreateThread"));
    assert!(!generated.contains("__atomic_"));
    assert!(!generated.contains("Interlocked"));

    let metrics: serde_json::Value =
        serde_json::from_slice(&fs::read(&metrics_path).unwrap()).unwrap();
    assert_eq!(metrics["counters"]["select_registrations"], 5);
    assert_eq!(metrics["counters"]["select_immediate_wins"], 1);
    assert_eq!(metrics["counters"]["select_suspended_wins"], 2);
    assert_eq!(metrics["counters"]["select_loser_cancellations"], 3);
    assert_eq!(metrics["counters"]["select_cancellations"], 0);
    assert_eq!(metrics["counters"]["live_channel_receive_waiters"], 0);
    assert_eq!(metrics["counters"]["live_timers"], 0);

    if cc_supports_address_sanitizer(&root) {
        let bin_path = root.join(if cfg!(windows) {
            "asan-async-static-select.exe"
        } else {
            "asan-async-static-select"
        });
        let asan_metrics_path = root.join("asan-select-counters.json");
        let cc_output = Command::new("cc")
            .arg("-std=c99")
            .arg("-fsanitize=address")
            .arg("-fno-omit-frame-pointer")
            .arg("-g")
            .arg(&generated_c)
            .arg("-o")
            .arg(&bin_path)
            .output()
            .unwrap();
        assert!(
            cc_output.status.success(),
            "{}{}",
            String::from_utf8_lossy(&cc_output.stdout),
            String::from_utf8_lossy(&cc_output.stderr)
        );
        let asan_options = if cfg!(target_os = "macos") {
            "detect_leaks=0:abort_on_error=1"
        } else {
            "detect_leaks=1:abort_on_error=1"
        };
        let asan_output = Command::new(&bin_path)
            .env("ASAN_OPTIONS", asan_options)
            .env("NOMO_ASYNC_METRICS_PATH", &asan_metrics_path)
            .output()
            .unwrap();
        assert!(
            asan_output.status.success(),
            "stdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&asan_output.stdout),
            String::from_utf8_lossy(&asan_output.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&asan_output.stdout),
            "selected\nwinner\nfirst\ntimer\n"
        );
        assert!(asan_output.stderr.is_empty());
        let asan_metrics: serde_json::Value =
            serde_json::from_slice(&fs::read(&asan_metrics_path).unwrap()).unwrap();
        assert_eq!(asan_metrics["counters"], metrics["counters"]);
    }

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn static_select_deadline_cancels_every_pending_registration_once() {
    let root = temp_test_root("async-static-select-deadline");
    reset_dir(&root);
    let project = root.join("async_static_select_deadline");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nname = \"async_static_select_deadline\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(
        project.join("src/main.nomo"),
        r#"package app.main

import std.io
import std.result
import std.task
import std.time

fn make_channel() -> Channel<string> {
    let created: Result<Channel<string>, ChannelError> = task.channel<string>(1)
    return match created {
        Result.Ok(channel_value) => channel_value
        Result.Err(error) => panic(error.message)
    }
}

suspend fn bounded_select(channel_value: Channel<string>) -> void {
    task.deadline(time.duration_millis(5)) {
        task.select {
            task.receive(channel_value) => received {
                io.println("unexpected receive")
            }
            task.sleep(time.duration_millis(100)) => waited {
                io.println("unexpected timer")
            }
        }
    }
}

suspend fn main() -> void {
    let channel_value: Channel<string> = make_channel()
    task.scope {
        let child = task.spawn bounded_select(channel_value)
        let joined: Result<void, TaskError> = task.join(child)
        io.println(result.is_err(joined))
    }
    task.close(channel_value)
}
"#,
    )
    .unwrap();

    let metrics_path = root.join("select-deadline-counters.json");
    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(&project)
        .env("NOMO_ASYNC_METRICS_PATH", &metrics_path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), "true\n");
    assert!(output.stderr.is_empty());

    let metrics: serde_json::Value =
        serde_json::from_slice(&fs::read(&metrics_path).unwrap()).unwrap();
    assert_eq!(metrics["counters"]["deadline_expirations"], 1);
    assert_eq!(metrics["counters"]["select_registrations"], 2);
    assert_eq!(metrics["counters"]["select_immediate_wins"], 0);
    assert_eq!(metrics["counters"]["select_suspended_wins"], 0);
    assert_eq!(metrics["counters"]["select_loser_cancellations"], 0);
    assert_eq!(metrics["counters"]["select_cancellations"], 2);
    assert_eq!(metrics["counters"]["live_channel_receive_waiters"], 0);
    assert_eq!(metrics["counters"]["live_timers"], 0);

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn bounded_channel_close_wakes_sender_and_returns_the_single_unsent_owner() {
    let root = temp_test_root("async-bounded-channel-close");
    reset_dir(&root);
    let project = root.join("async_bounded_channel_close");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nname = \"async_bounded_channel_close\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(
        project.join("src/main.nomo"),
        r#"package app.main

import std.io
import std.option
import std.result
import std.task

fn make_channel() -> Channel<string> {
    let created: Result<Channel<string>, ChannelError> = task.channel<string>(1)
    return match created {
        Result.Ok(channel_value) => channel_value
        Result.Err(error) => panic(error.message)
    }
}

fn recover(result: Result<void, ChannelSendError<string>>) -> string {
    return match result {
        Result.Ok(value) => "unexpected"
        Result.Err(failure) => failure.value
    }
}

suspend fn sender(channel_value: Channel<string>) -> void {
    let first: Result<void, ChannelSendError<string>> = task.send(channel_value, "first")
    let pending_value: string = "second"
    let second: Result<void, ChannelSendError<string>> = task.send(channel_value, pending_value)
    io.println(recover(second))
}

suspend fn closer(channel_value: Channel<string>) -> void {
    task.yield_now()
    task.close(channel_value)
    task.close(channel_value)
}

suspend fn main() -> void {
    let channel_value: Channel<string> = make_channel()
    task.scope {
        let sender_task = task.spawn sender(channel_value)
        let closer_task = task.spawn closer(channel_value)
        let sender_result: Result<void, TaskError> = task.join(sender_task)
        let closer_result: Result<void, TaskError> = task.join(closer_task)
    }
    let buffered: Option<string> = task.receive(channel_value)
    let drained: Option<string> = task.receive(channel_value)
    io.println(option.unwrap_or(buffered, "missing"))
    io.println(option.is_none(drained))
}
"#,
    )
    .unwrap();

    let metrics_path = root.join("channel-close-counters.json");
    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(&project)
        .env("NOMO_ASYNC_METRICS_PATH", &metrics_path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "second\nfirst\ntrue\n"
    );
    assert!(output.stderr.is_empty());

    let metrics: serde_json::Value =
        serde_json::from_slice(&fs::read(&metrics_path).unwrap()).unwrap();
    assert_eq!(metrics["counters"]["frame_allocations"], 0);
    assert_eq!(metrics["counters"]["publication_moves"], 1);
    assert_eq!(metrics["counters"]["channel_constructions"], 1);
    assert_eq!(metrics["counters"]["channel_sends"], 1);
    assert_eq!(metrics["counters"]["channel_receives"], 1);
    assert_eq!(metrics["counters"]["channel_buffered_sends"], 1);
    assert_eq!(metrics["counters"]["channel_buffered_receives"], 1);
    assert_eq!(metrics["counters"]["channel_direct_handoffs"], 0);
    assert_eq!(metrics["counters"]["channel_send_suspensions"], 1);
    assert_eq!(metrics["counters"]["channel_receive_suspensions"], 0);
    assert_eq!(metrics["counters"]["channel_wakeups"], 1);
    assert_eq!(metrics["counters"]["channel_closes"], 1);
    assert_eq!(metrics["counters"]["channel_cancellations"], 0);
    assert_eq!(metrics["counters"]["live_channel_buffered_elements"], 0);
    assert_eq!(
        metrics["counters"]["peak_live_channel_buffered_elements"],
        1
    );
    assert_eq!(metrics["counters"]["live_channel_send_waiters"], 0);
    assert_eq!(metrics["counters"]["peak_live_channel_send_waiters"], 1);

    if cc_supports_address_sanitizer(&root) {
        let generated_c = project.join("build/c/main.c");
        let bin_path = root.join(if cfg!(windows) {
            "asan-async-bounded-channel-close.exe"
        } else {
            "asan-async-bounded-channel-close"
        });
        let asan_metrics_path = root.join("asan-channel-close-counters.json");
        let cc_output = Command::new("cc")
            .arg("-std=c99")
            .arg("-fsanitize=address")
            .arg("-fno-omit-frame-pointer")
            .arg("-g")
            .arg(&generated_c)
            .arg("-o")
            .arg(&bin_path)
            .output()
            .unwrap();
        assert!(
            cc_output.status.success(),
            "{}{}",
            String::from_utf8_lossy(&cc_output.stdout),
            String::from_utf8_lossy(&cc_output.stderr)
        );
        let asan_options = if cfg!(target_os = "macos") {
            "detect_leaks=0:abort_on_error=1"
        } else {
            "detect_leaks=1:abort_on_error=1"
        };
        let asan_output = Command::new(&bin_path)
            .env("ASAN_OPTIONS", asan_options)
            .env("NOMO_ASYNC_METRICS_PATH", &asan_metrics_path)
            .output()
            .unwrap();
        assert!(
            asan_output.status.success(),
            "stdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&asan_output.stdout),
            String::from_utf8_lossy(&asan_output.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&asan_output.stdout),
            "second\nfirst\ntrue\n"
        );
        assert!(asan_output.stderr.is_empty());
        let asan_metrics: serde_json::Value =
            serde_json::from_slice(&fs::read(&asan_metrics_path).unwrap()).unwrap();
        assert_eq!(asan_metrics["counters"], metrics["counters"]);
    }

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn bounded_channel_try_operations_validate_capacity_wrap_and_close_drain_order() {
    let root = temp_test_root("bounded-channel-try-operations");
    reset_dir(&root);
    let project = root.join("bounded_channel_try_operations");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nname = \"bounded_channel_try_operations\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(
        project.join("src/main.nomo"),
        r#"package app.main

import std.io
import std.result
import std.task

fn channel_error_code(result: Result<Channel<string>, ChannelError>) -> string {
    return match result {
        Result.Ok(channel_value) => "unexpected"
        Result.Err(error) => error.code
    }
}

fn make_channel() -> Channel<string> {
    let created: Result<Channel<string>, ChannelError> = task.channel<string>(2)
    return match created {
        Result.Ok(channel_value) => channel_value
        Result.Err(error) => panic(error.message)
    }
}

fn print_send(result: ChannelTrySend<string>) -> void {
    match result {
        ChannelTrySend.Sent => {
            io.println("sent")
        }
        ChannelTrySend.Full(value) => {
            io.println("full", value)
        }
        ChannelTrySend.Closed(value) => {
            io.println("closed", value)
        }
        ChannelTrySend.Failed(failure) => {
            io.println("failed", failure.value)
        }
    }
}

fn receive_text(result: ChannelTryReceive<string>) -> string {
    return match result {
        ChannelTryReceive.Value(value) => value
        ChannelTryReceive.Empty => "empty"
        ChannelTryReceive.Closed => "closed"
    }
}

fn main() -> void {
    io.println(channel_error_code(task.channel<string>(0)))
    io.println(channel_error_code(task.channel<string>(65537)))
    let channel_value: Channel<string> = make_channel()
    io.println(receive_text(task.try_receive(channel_value)))
    print_send(task.try_send(channel_value, "first"))
    print_send(task.try_send(channel_value, "second"))
    print_send(task.try_send(channel_value, "third"))
    io.println(receive_text(task.try_receive(channel_value)))
    print_send(task.try_send(channel_value, "third"))
    task.close(channel_value)
    task.close(channel_value)
    io.println(receive_text(task.try_receive(channel_value)))
    io.println(receive_text(task.try_receive(channel_value)))
    io.println(receive_text(task.try_receive(channel_value)))
    print_send(task.try_send(channel_value, "fourth"))
}
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(&project)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        concat!(
            "invalid_capacity\n",
            "capacity_limit\n",
            "empty\n",
            "sent\n",
            "sent\n",
            "full third\n",
            "first\n",
            "sent\n",
            "second\n",
            "third\n",
            "closed\n",
            "closed fourth\n"
        )
    );
    assert!(output.stderr.is_empty());

    if cc_supports_address_sanitizer(&root) {
        let generated_c = project.join("build/c/main.c");
        let bin_path = root.join(if cfg!(windows) {
            "asan-bounded-channel-try-operations.exe"
        } else {
            "asan-bounded-channel-try-operations"
        });
        let cc_output = Command::new("cc")
            .arg("-std=c99")
            .arg("-fsanitize=address")
            .arg("-fno-omit-frame-pointer")
            .arg("-g")
            .arg(&generated_c)
            .arg("-o")
            .arg(&bin_path)
            .output()
            .unwrap();
        assert!(
            cc_output.status.success(),
            "{}{}",
            String::from_utf8_lossy(&cc_output.stdout),
            String::from_utf8_lossy(&cc_output.stderr)
        );
        let asan_options = if cfg!(target_os = "macos") {
            "detect_leaks=0:abort_on_error=1"
        } else {
            "detect_leaks=1:abort_on_error=1"
        };
        let asan_output = Command::new(&bin_path)
            .env("ASAN_OPTIONS", asan_options)
            .output()
            .unwrap();
        assert!(
            asan_output.status.success(),
            "stdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&asan_output.stdout),
            String::from_utf8_lossy(&asan_output.stderr)
        );
        assert_eq!(asan_output.stdout, output.stdout);
        assert!(asan_output.stderr.is_empty());
    }
    if cc_supports_undefined_sanitizer(&root) {
        let generated_c = project.join("build/c/main.c");
        let bin_path = root.join(if cfg!(windows) {
            "ubsan-bounded-channel-try-operations.exe"
        } else {
            "ubsan-bounded-channel-try-operations"
        });
        let cc_output = Command::new("cc")
            .arg("-std=c99")
            .arg("-fsanitize=undefined")
            .arg("-fno-sanitize-recover=undefined")
            .arg("-fno-omit-frame-pointer")
            .arg("-g")
            .arg(&generated_c)
            .arg("-o")
            .arg(&bin_path)
            .output()
            .unwrap();
        assert!(
            cc_output.status.success(),
            "{}{}",
            String::from_utf8_lossy(&cc_output.stdout),
            String::from_utf8_lossy(&cc_output.stderr)
        );
        let ubsan_output = Command::new(&bin_path)
            .env("UBSAN_OPTIONS", "halt_on_error=1:print_stacktrace=1")
            .output()
            .unwrap();
        assert!(
            ubsan_output.status.success(),
            "stdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&ubsan_output.stdout),
            String::from_utf8_lossy(&ubsan_output.stderr)
        );
        assert_eq!(ubsan_output.stdout, output.stdout);
        assert!(ubsan_output.stderr.is_empty());
    }

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn bounded_channel_accepts_exact_element_and_byte_limits_and_rejects_one_field_over() {
    let root = temp_test_root("bounded-channel-capacity-boundaries");
    reset_dir(&root);
    let project = root.join("bounded_channel_capacity_boundaries");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nname = \"bounded_channel_capacity_boundaries\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();

    let mut source = String::from(
        "package app.main\n\nimport std.io\nimport std.result\nimport std.task\n\nstruct AtByteLimit {\n",
    );
    for index in 0..128 {
        source.push_str(&format!("    field_{index}: u64\n"));
    }
    source.push_str("}\n\nstruct AboveByteLimit {\n");
    for index in 0..129 {
        source.push_str(&format!("    field_{index}: u64\n"));
    }
    source.push_str(
        r#"}

fn above_limit_code(created: Result<Channel<AboveByteLimit>, ChannelError>) -> string {
    return match created {
        Result.Ok(channel_value) => "unexpected"
        Result.Err(error) => error.code
    }
}

fn main() -> void {
    io.println(result.is_ok(task.channel<u64>(65536)))
    io.println(result.is_ok(task.channel<AtByteLimit>(65536)))
    io.println(above_limit_code(task.channel<AboveByteLimit>(65536)))
}
"#,
    );
    fs::write(project.join("src/main.nomo"), source).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(&project)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "true\ntrue\ncapacity_limit\n"
    );
    assert!(output.stderr.is_empty());

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn bounded_channel_deadline_cancels_one_waiter_without_leaking_registration() {
    let root = temp_test_root("bounded-channel-deadline-cancel");
    reset_dir(&root);
    let project = root.join("bounded_channel_deadline_cancel");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nname = \"bounded_channel_deadline_cancel\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(
        project.join("src/main.nomo"),
        r#"package app.main

import std.io
import std.option
import std.result
import std.task
import std.time

fn make_channel() -> Channel<string> {
    let created: Result<Channel<string>, ChannelError> = task.channel<string>(1)
    return match created {
        Result.Ok(channel_value) => channel_value
        Result.Err(error) => panic(error.message)
    }
}

suspend fn wait_for_value(channel_value: Channel<string>) -> void {
    task.deadline(time.duration_millis(5)) {
        let received: Option<string> = task.receive(channel_value)
        io.println(option.unwrap_or(received, "unexpected"))
    }
}

suspend fn main() -> void {
    let channel_value: Channel<string> = make_channel()
    task.scope {
        let waiter = task.spawn wait_for_value(channel_value)
        let joined: Result<void, TaskError> = task.join(waiter)
        io.println(result.is_err(joined))
    }
    task.close(channel_value)
}
"#,
    )
    .unwrap();

    let metrics_path = root.join("channel-cancel-counters.json");
    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(&project)
        .env("NOMO_ASYNC_METRICS_PATH", &metrics_path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), "true\n");
    assert!(output.stderr.is_empty());

    let generated = fs::read_to_string(project.join("build/c/main.c")).unwrap();
    assert!(generated.contains("nomo_channel_receive_cancel_string"));
    assert!(generated.contains("nomo_async_timer_wait_next"));
    let metrics: serde_json::Value =
        serde_json::from_slice(&fs::read(&metrics_path).unwrap()).unwrap();
    assert_eq!(metrics["counters"]["frame_allocations"], 0);
    assert_eq!(metrics["counters"]["channel_constructions"], 1);
    assert_eq!(metrics["counters"]["channel_sends"], 0);
    assert_eq!(metrics["counters"]["channel_receives"], 0);
    assert_eq!(metrics["counters"]["channel_send_suspensions"], 0);
    assert_eq!(metrics["counters"]["channel_receive_suspensions"], 1);
    assert_eq!(metrics["counters"]["channel_wakeups"], 0);
    assert_eq!(metrics["counters"]["channel_closes"], 1);
    assert_eq!(metrics["counters"]["channel_cancellations"], 1);
    assert_eq!(metrics["counters"]["live_channel_receive_waiters"], 0);
    assert_eq!(metrics["counters"]["peak_live_channel_receive_waiters"], 1);
    assert_eq!(metrics["counters"]["deadline_registrations"], 1);
    assert_eq!(metrics["counters"]["deadline_expirations"], 1);
    assert_eq!(metrics["counters"]["live_timers"], 0);

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn bounded_channel_hands_values_to_blocked_receivers_in_waiter_fifo_order() {
    let root = temp_test_root("bounded-channel-direct-handoff");
    reset_dir(&root);
    let project = root.join("bounded_channel_direct_handoff");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nname = \"bounded_channel_direct_handoff\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(
        project.join("src/main.nomo"),
        r#"package app.main

import std.io
import std.option
import std.result
import std.task

fn make_channel() -> Channel<string> {
    let created: Result<Channel<string>, ChannelError> = task.channel<string>(1)
    return match created {
        Result.Ok(channel_value) => channel_value
        Result.Err(error) => panic(error.message)
    }
}

suspend fn receiver(channel_value: Channel<string>) -> void {
    let received: Option<string> = task.receive(channel_value)
    io.println(option.unwrap_or(received, "missing"))
}

suspend fn sender(channel_value: Channel<string>) -> void {
    task.yield_now()
    let first: Result<void, ChannelSendError<string>> = task.send(channel_value, "first")
    let second: Result<void, ChannelSendError<string>> = task.send(channel_value, "second")
}

suspend fn main() -> void {
    let channel_value: Channel<string> = make_channel()
    task.scope {
        let first_receiver = task.spawn receiver(channel_value)
        let second_receiver = task.spawn receiver(channel_value)
        let sender_task = task.spawn sender(channel_value)
        let first_joined: Result<void, TaskError> = task.join(first_receiver)
        let second_joined: Result<void, TaskError> = task.join(second_receiver)
        let sender_joined: Result<void, TaskError> = task.join(sender_task)
    }
    task.close(channel_value)
}
"#,
    )
    .unwrap();

    let metrics_path = root.join("channel-handoff-counters.json");
    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(&project)
        .env("NOMO_ASYNC_METRICS_PATH", &metrics_path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), "first\nsecond\n");
    assert!(output.stderr.is_empty());

    let metrics: serde_json::Value =
        serde_json::from_slice(&fs::read(&metrics_path).unwrap()).unwrap();
    assert_eq!(metrics["counters"]["channel_constructions"], 1);
    assert_eq!(metrics["counters"]["channel_sends"], 2);
    assert_eq!(metrics["counters"]["channel_receives"], 2);
    assert_eq!(metrics["counters"]["channel_buffered_sends"], 0);
    assert_eq!(metrics["counters"]["channel_buffered_receives"], 0);
    assert_eq!(metrics["counters"]["channel_direct_handoffs"], 2);
    assert_eq!(metrics["counters"]["channel_send_suspensions"], 0);
    assert_eq!(metrics["counters"]["channel_receive_suspensions"], 2);
    assert_eq!(metrics["counters"]["channel_wakeups"], 2);
    assert_eq!(metrics["counters"]["channel_closes"], 1);
    assert_eq!(metrics["counters"]["channel_cancellations"], 0);
    assert_eq!(metrics["counters"]["live_channel_receive_waiters"], 0);
    assert_eq!(metrics["counters"]["peak_live_channel_receive_waiters"], 2);

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn bounded_channel_deadline_cancels_blocked_sender_and_drops_published_value_once() {
    let root = temp_test_root("bounded-channel-sender-cancel");
    reset_dir(&root);
    let project = root.join("bounded_channel_sender_cancel");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nname = \"bounded_channel_sender_cancel\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(
        project.join("src/main.nomo"),
        r#"package app.main

import std.io
import std.option
import std.result
import std.task
import std.time

fn make_channel() -> Channel<string> {
    let created: Result<Channel<string>, ChannelError> = task.channel<string>(1)
    return match created {
        Result.Ok(channel_value) => channel_value
        Result.Err(error) => panic(error.message)
    }
}

suspend fn blocked_sender(channel_value: Channel<string>) -> void {
    task.deadline(time.duration_millis(5)) {
        let pending_value: string = "cancelled-value"
        let sent: Result<void, ChannelSendError<string>> = task.send(channel_value, pending_value)
    }
}

suspend fn main() -> void {
    let channel_value: Channel<string> = make_channel()
    let primed: ChannelTrySend<string> = task.try_send(channel_value, "buffered")
    task.scope {
        let sender = task.spawn blocked_sender(channel_value)
        let joined: Result<void, TaskError> = task.join(sender)
        io.println(result.is_err(joined))
    }
    let buffered: Option<string> = task.receive(channel_value)
    io.println(option.unwrap_or(buffered, "missing"))
    task.close(channel_value)
}
"#,
    )
    .unwrap();

    let metrics_path = root.join("channel-sender-cancel-counters.json");
    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(&project)
        .env("NOMO_ASYNC_METRICS_PATH", &metrics_path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), "true\nbuffered\n");
    assert!(output.stderr.is_empty());

    let metrics: serde_json::Value =
        serde_json::from_slice(&fs::read(&metrics_path).unwrap()).unwrap();
    assert_eq!(metrics["counters"]["frame_allocations"], 0);
    assert_eq!(metrics["counters"]["publication_moves"], 1);
    assert_eq!(metrics["counters"]["channel_constructions"], 1);
    assert_eq!(metrics["counters"]["channel_sends"], 0);
    assert_eq!(metrics["counters"]["channel_receives"], 1);
    assert_eq!(metrics["counters"]["channel_send_suspensions"], 1);
    assert_eq!(metrics["counters"]["channel_receive_suspensions"], 0);
    assert_eq!(metrics["counters"]["channel_wakeups"], 0);
    assert_eq!(metrics["counters"]["channel_closes"], 1);
    assert_eq!(metrics["counters"]["channel_cancellations"], 1);
    assert_eq!(metrics["counters"]["live_channel_buffered_elements"], 0);
    assert_eq!(
        metrics["counters"]["peak_live_channel_buffered_elements"],
        1
    );
    assert_eq!(metrics["counters"]["live_channel_send_waiters"], 0);
    assert_eq!(metrics["counters"]["peak_live_channel_send_waiters"], 1);
    assert_eq!(metrics["counters"]["deadline_expirations"], 1);
    assert_eq!(metrics["counters"]["live_timers"], 0);

    if cc_supports_address_sanitizer(&root) {
        let generated_c = project.join("build/c/main.c");
        let bin_path = root.join(if cfg!(windows) {
            "asan-bounded-channel-sender-cancel.exe"
        } else {
            "asan-bounded-channel-sender-cancel"
        });
        let asan_metrics_path = root.join("asan-channel-sender-cancel-counters.json");
        let cc_output = Command::new("cc")
            .arg("-std=c99")
            .arg("-fsanitize=address")
            .arg("-fno-omit-frame-pointer")
            .arg("-g")
            .arg(&generated_c)
            .arg("-o")
            .arg(&bin_path)
            .output()
            .unwrap();
        assert!(
            cc_output.status.success(),
            "{}{}",
            String::from_utf8_lossy(&cc_output.stdout),
            String::from_utf8_lossy(&cc_output.stderr)
        );
        let asan_options = if cfg!(target_os = "macos") {
            "detect_leaks=0:abort_on_error=1"
        } else {
            "detect_leaks=1:abort_on_error=1"
        };
        let asan_output = Command::new(&bin_path)
            .env("ASAN_OPTIONS", asan_options)
            .env("NOMO_ASYNC_METRICS_PATH", &asan_metrics_path)
            .output()
            .unwrap();
        assert!(
            asan_output.status.success(),
            "stdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&asan_output.stdout),
            String::from_utf8_lossy(&asan_output.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&asan_output.stdout),
            "true\nbuffered\n"
        );
        assert!(asan_output.stderr.is_empty());
        let asan_metrics: serde_json::Value =
            serde_json::from_slice(&fs::read(&asan_metrics_path).unwrap()).unwrap();
        assert_eq!(asan_metrics["counters"], metrics["counters"]);
    }

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn bounded_channel_root_panic_drops_buffer_and_linked_sender_once_under_asan() {
    let root = temp_test_root("bounded-channel-panic-cleanup");
    reset_dir(&root);
    let project = root.join("bounded_channel_panic_cleanup");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nname = \"bounded_channel_panic_cleanup\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(
        project.join("src/main.nomo"),
        r#"package app.main

import std.result
import std.task

fn make_channel() -> Channel<string> {
    let created: Result<Channel<string>, ChannelError> = task.channel<string>(1)
    return match created {
        Result.Ok(channel_value) => channel_value
        Result.Err(error) => panic(error.message)
    }
}

suspend fn blocked_sender(channel_value: Channel<string>) -> void {
    let buffered: Result<void, ChannelSendError<string>> = task.send(channel_value, "buffered")
    let pending_value: string = "linked"
    let pending: Result<void, ChannelSendError<string>> = task.send(channel_value, pending_value)
}

suspend fn panicking_child(channel_value: Channel<string>) -> void {
    task.yield_now()
    task.close(channel_value)
    panic("channel panic")
}

suspend fn main() -> void {
    let channel_value: Channel<string> = make_channel()
    task.scope {
        let sender = task.spawn blocked_sender(channel_value)
        let failure = task.spawn panicking_child(channel_value)
        let joined: Result<void, TaskError> = task.join(failure)
    }
}
"#,
    )
    .unwrap();

    let metrics_path = root.join("channel-panic-counters.json");
    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(&project)
        .env("NOMO_ASYNC_METRICS_PATH", &metrics_path)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert_eq!(
        String::from_utf8_lossy(&output.stderr),
        "panic: channel panic\nprogram exited with status 1\n"
    );

    let metrics: serde_json::Value =
        serde_json::from_slice(&fs::read(&metrics_path).unwrap()).unwrap();
    assert_eq!(metrics["counters"]["channel_constructions"], 1);
    assert_eq!(metrics["counters"]["channel_sends"], 1);
    assert_eq!(metrics["counters"]["channel_buffered_sends"], 1);
    assert_eq!(metrics["counters"]["channel_send_suspensions"], 1);
    assert_eq!(metrics["counters"]["channel_wakeups"], 1);
    assert_eq!(metrics["counters"]["channel_closes"], 1);
    assert_eq!(metrics["counters"]["channel_cancellations"], 1);
    assert_eq!(metrics["counters"]["live_channel_buffered_elements"], 0);
    assert_eq!(metrics["counters"]["live_channel_send_waiters"], 0);
    assert_eq!(metrics["counters"]["peak_live_channel_send_waiters"], 1);

    if cc_supports_address_sanitizer(&root) {
        let generated_c = project.join("build/c/main.c");
        let bin_path = root.join(if cfg!(windows) {
            "asan-bounded-channel-panic-cleanup.exe"
        } else {
            "asan-bounded-channel-panic-cleanup"
        });
        let asan_metrics_path = root.join("asan-channel-panic-counters.json");
        let cc_output = Command::new("cc")
            .arg("-std=c99")
            .arg("-fsanitize=address")
            .arg("-fno-omit-frame-pointer")
            .arg("-g")
            .arg(&generated_c)
            .arg("-o")
            .arg(&bin_path)
            .output()
            .unwrap();
        assert!(
            cc_output.status.success(),
            "{}{}",
            String::from_utf8_lossy(&cc_output.stdout),
            String::from_utf8_lossy(&cc_output.stderr)
        );
        let asan_options = if cfg!(target_os = "macos") {
            "detect_leaks=0:abort_on_error=1"
        } else {
            "detect_leaks=1:abort_on_error=1"
        };
        let asan_output = Command::new(&bin_path)
            .env("ASAN_OPTIONS", asan_options)
            .env("NOMO_ASYNC_METRICS_PATH", &asan_metrics_path)
            .output()
            .unwrap();
        assert!(!asan_output.status.success());
        assert!(asan_output.stdout.is_empty());
        assert_eq!(
            String::from_utf8_lossy(&asan_output.stderr),
            "panic: channel panic\n"
        );
        let asan_metrics: serde_json::Value =
            serde_json::from_slice(&fs::read(&asan_metrics_path).unwrap()).unwrap();
        assert_eq!(asan_metrics["counters"], metrics["counters"]);
    }

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn async_timer_uses_monotonic_owner_local_waiting_and_exact_counters() {
    let root = temp_test_root("async-timer-runtime");
    reset_dir(&root);
    let source = root.join("main.nomo");
    let generated_c = root.join("generated.c");
    let binary = root.join(if cfg!(windows) {
        "async-timer.exe"
    } else {
        "async-timer"
    });
    let metrics_path = root.join("timer-metrics.json");
    fs::write(
        &source,
        r#"package app.main

import std.io
import std.result
import std.task
import std.time

suspend fn main() -> void {
    let started: i64 = time.monotonic_millis()
    io.println("before")
    let immediate: Result<void, TaskError> = task.sleep(time.duration_millis(0))
    io.println(result.is_ok(immediate))
    let waited: Result<void, TaskError> = task.sleep(time.duration_millis(25))
    io.println(result.is_ok(waited))
    let elapsed: i64 = time.monotonic_millis() - started
    io.println(elapsed >= 25)
    io.println("after")
}
"#,
    )
    .unwrap();

    let build_output = Command::new(env!("CARGO_BIN_EXE_nomoc"))
        .arg("build")
        .arg(&source)
        .arg("--emit-c")
        .arg("--out")
        .arg(&generated_c)
        .output()
        .unwrap();
    assert!(
        build_output.status.success(),
        "{}",
        String::from_utf8_lossy(&build_output.stderr)
    );

    let generated = fs::read_to_string(&generated_c).unwrap();
    assert!(generated.contains("NOMO_ASYNC_PENDING_TIMER"));
    assert!(generated.contains("nomo_async_timer_start"));
    assert!(generated.contains("nomo_async_timer_wait_next"));
    assert!(generated.contains("nomo_async_timer_disarm"));
    assert!(generated.contains("nomo_async_reactor_wait"));
    assert!(generated.contains("nomo_async_reactor_shutdown"));
    if cfg!(target_os = "linux") {
        assert!(generated.contains("epoll_create(1)"));
        assert!(generated.contains("epoll_wait("));
    } else if cfg!(target_os = "macos") {
        assert!(generated.contains("kqueue()"));
        assert!(generated.contains("kevent("));
    } else if cfg!(windows) {
        assert!(generated.contains("CreateIoCompletionPort"));
        assert!(generated.contains("GetQueuedCompletionStatus"));
    }
    let timer_wait = generated
        .split("static int nomo_async_timer_wait_next")
        .nth(1)
        .unwrap()
        .split("static nomo_async_poll nomo_async_poll_task")
        .next()
        .unwrap();
    assert!(timer_wait.contains("nomo_async_reactor_wait"));
    assert!(!timer_wait.contains("nomo_time_sleep_millis"));
    assert!(generated.contains("timer_registrations"));
    assert!(generated.contains("timer_expirations"));
    assert!(generated.contains("timer_cancellations"));
    assert!(generated.contains("peak_live_timers"));
    assert!(generated.contains("context->pending_reason == NOMO_ASYNC_PENDING_YIELD"));
    assert!(generated.contains("context->pending_reason != NOMO_ASYNC_PENDING_TIMER"));
    assert!(!generated.contains("pthread_create"));
    assert!(!generated.contains("CreateThread"));
    assert!(!generated.contains("__atomic_"));
    assert!(!generated.contains("Interlocked"));

    let cc_output = Command::new("cc")
        .arg(&generated_c)
        .arg("-o")
        .arg(&binary)
        .output()
        .unwrap();
    assert!(
        cc_output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&cc_output.stdout),
        String::from_utf8_lossy(&cc_output.stderr)
    );

    let started = Instant::now();
    let output = Command::new(&binary)
        .env("NOMO_ASYNC_METRICS_PATH", &metrics_path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "bounded timer took {:?}",
        started.elapsed()
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "before\ntrue\ntrue\ntrue\nafter\n"
    );
    assert!(output.stderr.is_empty());

    let metrics: serde_json::Value =
        serde_json::from_slice(&fs::read(&metrics_path).unwrap()).unwrap();
    assert_eq!(metrics["schema"], 1);
    assert_eq!(metrics["runtime"], "nomo-c99-current-thread");
    assert_eq!(metrics["runtime_abi"], 1);
    assert_eq!(metrics["counter_catalog_schema"], 1);
    assert_eq!(metrics["counters"]["poll_calls"], 2);
    assert_eq!(metrics["counters"]["cooperative_yields"], 0);
    assert_eq!(metrics["counters"]["frame_allocations"], 0);
    assert_eq!(metrics["counters"]["frame_drops"], 1);
    assert_eq!(metrics["counters"]["peak_live_frames"], 1);
    assert_eq!(metrics["counters"]["ready_queue_enqueues"], 1);
    assert_eq!(metrics["counters"]["ready_queue_dequeues"], 1);
    assert_eq!(metrics["counters"]["ready_queue_saturations"], 0);
    assert_eq!(metrics["counters"]["task_spawns"], 0);
    assert_eq!(metrics["counters"]["task_joins"], 0);
    assert_eq!(metrics["counters"]["join_suspensions"], 0);
    assert_eq!(metrics["counters"]["timer_registrations"], 1);
    assert_eq!(metrics["counters"]["timer_expirations"], 1);
    assert_eq!(metrics["counters"]["timer_cancellations"], 0);
    assert_eq!(metrics["counters"]["live_timers"], 0);
    assert_eq!(metrics["counters"]["peak_live_timers"], 1);
    assert_eq!(metrics["counters"]["reactor_initializations"], 1);
    assert_eq!(metrics["counters"]["reactor_waits"], 1);
    assert_eq!(metrics["counters"]["reactor_timeouts"], 1);
    assert_eq!(metrics["counters"]["reactor_completions"], 0);
    assert_eq!(metrics["counters"]["reactor_errors"], 0);
    assert_eq!(metrics["counters"]["reactor_shutdowns"], 1);
    assert_eq!(metrics["counters"]["live_reactors"], 0);
    assert_eq!(metrics["counters"]["peak_live_reactors"], 1);
    assert!(metrics["unavailable"]["local_retain"].is_string());
    assert!(metrics["unavailable"]["local_release"].is_string());
    assert!(metrics["unavailable"]["live_timers"].is_null());

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_run_allows_option_question_early_return() {
    let root = temp_test_root("option-question-early-return");
    reset_dir(&root);
    let project = root.join("option_question");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nname = \"option_question\"\nversion = \"0.1.0\"\n\n[dependencies]\nstd = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(
        project.join("src/main.nomo"),
        r#"package app.main

import std.io

fn load() -> Option<string> {
    return None
}

fn compute() -> Option<string> {
    let text: string = load()?
    io.println("after")
    return Some(text)
}

fn main() -> void {
    let result: Option<string> = compute()
    match result {
        Some(text) => {
            io.println(text)
        }
        None => {
            io.println("fallback")
        }
    }
}
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(&project)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), "fallback\n");
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_run_executes_std_log_helpers() {
    let root = temp_test_root("std-log-helpers");
    reset_dir(&root);
    let project = root.join("std_log_helpers");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nname = \"std_log_helpers\"\nversion = \"0.1.0\"\n\n[dependencies]\nstd = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(
        project.join("src/main.nomo"),
        r#"package app.main

import std.io
import std.log

fn main() -> void {
    log.debug("debug")
    log.info("info")
    log.warn("warn")
    log.error("error")
    if log.enabled("debug") {
        io.println("debug-enabled")
    } else {
        io.println("debug-disabled")
    }
}
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(&project)
        .env_remove("NOMO_LOG")
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), "debug-disabled\n");
    assert_eq!(
        String::from_utf8_lossy(&output.stderr),
        "[info] info\n[warn] warn\n[error] error\n"
    );

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(&project)
        .env("NOMO_LOG", "debug")
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), "debug-enabled\n");
    assert_eq!(
        String::from_utf8_lossy(&output.stderr),
        "[debug] debug\n[info] info\n[warn] warn\n[error] error\n"
    );

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_run_executes_std_hash_helpers() {
    let root = temp_test_root("std-hash-helpers");
    reset_dir(&root);
    let project = root.join("std_hash_helpers");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nname = \"std_hash_helpers\"\nversion = \"0.1.0\"\n\n[dependencies]\nstd = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(
        project.join("src/main.nomo"),
        r#"package app.main

import std.hash
import std.array.Array
import std.io
import std.num

fn main() -> void {
    let mut bytes: Array<u32> = Array.new<u32>()
    bytes.push(110 as u32)
    bytes.push(111 as u32)
    bytes.push(109 as u32)
    bytes.push(111 as u32)
    let direct: u64 = hash.string("nomo")
    let direct_bytes: u64 = hash.bytes(bytes)
    let empty: HashState = hash.new()
    let written: HashState = hash.write_bytes(empty, bytes)
    let incremental: u64 = hash.finish(written)
    io.println(num.to_string(direct))
    io.println(num.to_string(direct_bytes))
    if direct == direct_bytes && direct == incremental {
        io.println("same")
    } else {
        io.println("different")
    }
}
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(&project)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "4330230535792317134\n4330230535792317134\nsame\n"
    );
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_run_executes_std_crypto_sha_helpers() {
    let root = temp_test_root("std-crypto-helpers");
    reset_dir(&root);
    let project = root.join("std_crypto_helpers");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nname = \"std_crypto_helpers\"\nversion = \"0.1.0\"\n\n[dependencies]\nstd = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(
        project.join("src/main.nomo"),
        r#"package app.main

import std.crypto
import std.io
import std.num
import std.array.Array

fn all_bytes(values: Array<u32>) -> bool {
    let mut bad_count: i64 = 0
    for value in values {
        let value_bad: i64 = if value > 255 as u32 {
            1
        } else {
            0
        }
        bad_count = bad_count + value_bad
    }
    return bad_count == 0
}

fn main() -> void {
    io.println(crypto.sha256("nomo"))
    io.println(crypto.sha512("nomo"))
    let bytes: Array<u32> = crypto.random_bytes(4 as u64)
    io.println(num.to_string(bytes.len()))
    if all_bytes(bytes) {
        io.println("bytes ok")
    } else {
        io.println("bytes bad")
    }
}
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(&project)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "b2ef23fca2e63b943302abdf09318c938f43dc167676929643102591b6eeeff0\nf64a797448cbf54b2220274f024a6dfa4bb1c86c8bca1a3eaaf320bbf40c2a09a48385d62b050fc28b9ce85e36e619a8e06e0722baf4ad2c5449c424080f74b3\n4\nbytes ok\n"
    );
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_run_executes_std_json_parse_and_stringify() {
    let root = temp_test_root("std-json-helpers");
    reset_dir(&root);
    let project = root.join("std_json_helpers");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nname = \"std_json_helpers\"\nversion = \"0.1.0\"\n\n[dependencies]\nstd = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(
        project.join("src/main.nomo"),
        r#"package app.main

import std.io
import std.json

fn main() -> void {
    let parsed: Result<JsonValue, JsonError> = json.parse("{\"lang\":\"nomo\",\"versions\":[1,true,null]}")
    match parsed {
        Ok(value) => {
            io.println(json.stringify(value))
        }
        Err(err) => {
            io.println(err.message)
        }
    }

    let broken: Result<JsonValue, JsonError> = json.parse("{\"lang\":")
    match broken {
        Ok(value) => {
            io.println(json.stringify(value))
        }
        Err(err) => {
            io.println(err.message)
        }
    }
}
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(&project)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "{\"lang\":\"nomo\",\"versions\":[1,true,null]}\ninvalid json syntax\n"
    );
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_run_executes_bounded_jsonrpc_framing_conformance() {
    let root = temp_test_root("std-jsonrpc-conformance");
    reset_dir(&root);
    let project = root.join("jsonrpc_conformance");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nname = \"jsonrpc_conformance\"\nversion = \"0.1.0\"\n\n[dependencies]\nstd = \"0.1.0\"\n",
    )
    .unwrap();
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/jsonrpc_conformance.nomo");
    fs::copy(fixture, project.join("src/main.nomo")).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(&project)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).replace('\r', ""),
        "0\n2\nrequest\n{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\"}\nnotification\n{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n{\"jsonrpc\":\"2.0\",\"id\":7,\"method\":\"tools/list\"}\n{\"jsonrpc\":\"2.0\",\"id\":7,\"result\":true}\n{\"jsonrpc\":\"2.0\",\"id\":7,\"error\":{\"code\":-32601,\"message\":\"missing\"}}\n"
    );
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn cron_conformance_fixture_matches_native_runtime() {
    let root = temp_test_root("std-cron-conformance");
    reset_dir(&root);
    let project = root.join("cron_conformance");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nname = \"cron_conformance\"\nversion = \"0.1.0\"\n\n[dependencies]\nstd = \"0.1.0\"\n",
    )
    .unwrap();
    fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/cron_conformance.nomo"),
        project.join("src/main.nomo"),
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(&project)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout).replace("\r\n", "\n");
    assert_eq!(
        stdout,
        "true\n60000\n900000\n3600000\n68169600000\n4233686400000\ntrue\nfalse\ntrue\nfalse\nrange 0\nrange 0\nsyntax 0\nrange 0\nlimit 5\nsyntax 5 invalid cron expression syntax\ntimestamp_range 5\nno_match 5\n"
    );
    assert!(!stdout.contains("NOMO_CRON_SECRET_SENTINEL"));
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn cron_native_expression_limit_accepts_the_exact_boundary() {
    let root = temp_test_root("std-cron-expression-boundary");
    reset_dir(&root);
    let project = root.join("cron_expression_boundary");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nname = \"cron_expression_boundary\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();

    let exact = format!("{} 00 * * *", vec!["0"; 124].join(","));
    assert_eq!(exact.len(), 256);
    let overflow = format!("{exact} ");
    assert_eq!(overflow.len(), 257);
    fs::write(
        project.join("src/main.nomo"),
        format!(
            r#"package app.main

import std.cron
import std.io

fn main() -> void {{
    let exact: Result<CronSchedule, CronError> = cron.parse("{exact}")
    match exact {{
        Ok(value) => {{
            io.println("exact-ok")
        }}
        Err(error) => {{
            panic(error.message)
        }}
    }}
    let overflow: Result<CronSchedule, CronError> = cron.parse("{overflow}")
    match overflow {{
        Ok(value) => {{
            panic("overflow expression was accepted")
        }}
        Err(error) => {{
            io.println(error.code)
        }}
    }}
}}
"#
        ),
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(&project)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), "exact-ok\nlimit\n");

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn jsonrpc_native_errors_are_bounded_and_secret_safe() {
    let root = temp_test_root("std-jsonrpc-errors");
    reset_dir(&root);
    let project = root.join("jsonrpc_errors");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nname = \"jsonrpc_errors\"\nversion = \"0.1.0\"\n\n[dependencies]\nstd = \"0.1.0\"\n",
    )
    .unwrap();
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/jsonrpc_errors.nomo");
    fs::copy(fixture, project.join("src/main.nomo")).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(&project)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout).replace('\r', "");
    for expected in [
        "zero invalid_request invalid JSON-RPC argument\n",
        "array protocol invalid JSON-RPC 2.0 envelope\n",
        "newline framing invalid JSON-RPC newline framing\n",
        "duplicate protocol invalid JSON-RPC 2.0 envelope\n",
        "fractional-code protocol invalid JSON-RPC 2.0 envelope\n",
        "extension ok\n",
        "empty-line framing invalid JSON-RPC newline framing\n",
        "malformed json invalid bounded JSON input\n",
        "partial framing invalid JSON-RPC newline framing\n",
        "line-limit limit JSON-RPC limit exceeded\n",
        "bool-id protocol invalid JSON-RPC 2.0 envelope\n",
        "scalar-params protocol invalid JSON-RPC 2.0 envelope\n",
    ] {
        assert!(
            stdout.contains(expected),
            "missing {expected:?} in:\n{stdout}"
        );
    }
    assert!(!stdout.contains("NOMO_JSONRPC_SECRET_SENTINEL"));
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn jsonrpc_native_limits_accept_exact_boundaries_and_reject_overflow() {
    const MAX_MESSAGE_BYTES: usize = 1_048_575;
    const MAX_CHUNK_BYTES: usize = 1_048_576;
    const MAX_BATCH_MESSAGES: usize = 4_096;

    let root = temp_test_root("std-jsonrpc-boundaries");
    reset_dir(&root);
    let project = root.join("jsonrpc_boundaries");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nname = \"jsonrpc_boundaries\"\nversion = \"0.1.0\"\n\n[dependencies]\nstd = \"0.1.0\"\n",
    )
    .unwrap();

    let prefix = r#"{"jsonrpc":"2.0","method":"m","_pad":""#;
    let suffix = r#""}"#;
    let exact_message = format!(
        "{prefix}{}{suffix}",
        "x".repeat(MAX_MESSAGE_BYTES - prefix.len() - suffix.len())
    );
    assert_eq!(exact_message.len(), MAX_MESSAGE_BYTES);
    let exact_chunk = exact_message + "\n";
    assert_eq!(exact_chunk.len(), MAX_CHUNK_BYTES);
    fs::write(project.join("exact.txt"), exact_chunk).unwrap();
    fs::write(
        project.join("chunk_over.txt"),
        "x".repeat(MAX_CHUNK_BYTES + 1),
    )
    .unwrap();

    let line = "{\"jsonrpc\":\"2.0\",\"method\":\"m\"}\n";
    fs::write(
        project.join("batch_exact.txt"),
        line.repeat(MAX_BATCH_MESSAGES),
    )
    .unwrap();
    fs::write(
        project.join("batch_over.txt"),
        line.repeat(MAX_BATCH_MESSAGES + 1),
    )
    .unwrap();

    fs::write(
        project.join("src/main.nomo"),
        r#"package jsonrpc_boundaries.main

import std.fs
import std.io
import std.jsonrpc
import std.array.Array

fn read_fixture(path: string) -> string {
    match fs.read_to_string(path) {
        Ok(value) => {
            return value
        }
        Err(error) => {
            io.println(path, "read-error")
            return ""
        }
    }
}

fn feed_case(label: string, input: string) -> void {
    match jsonrpc.decoder(1048575 as u64) {
        Ok(decoder_value) => {
            match jsonrpc.feed(decoder_value, input) {
                Ok(batch) => {
                    let messages: Array<JsonRpcMessage> = batch.messages
                    let count: u64 = messages.len()
                    io.println(label, "ok", count)
                }
                Err(error) => {
                    io.println(label, error.code)
                }
            }
        }
        Err(error) => {
            io.println(label, error.code)
        }
    }
}

fn main() -> void {
    feed_case("message-exact", read_fixture("exact.txt"))
    feed_case("chunk-over", read_fixture("chunk_over.txt"))
    feed_case("batch-exact", read_fixture("batch_exact.txt"))
    feed_case("batch-over", read_fixture("batch_over.txt"))
}
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(".")
        .current_dir(&project)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).replace('\r', ""),
        "message-exact ok 1\nchunk-over limit\nbatch-exact ok 4096\nbatch-over limit\n"
    );
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_run_executes_structured_json_agent_payloads() {
    let root = temp_test_root("structured-json-agent");
    reset_dir(&root);
    let project = root.join("structured_json_agent");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nname = \"structured_json_agent\"\nversion = \"0.1.0\"\n\n[dependencies]\nstd = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(
        project.join("src/main.nomo"),
        r#"package structured_json_agent.main

import std.array.Array
import std.io
import std.json

fn request_body() -> Result<JsonValue, JsonError> {
    let mut message_members: Array<JsonMember> = Array.new<JsonMember>()
    message_members.push(JsonMember { key: "role", value: json.from_string("user")? })
    message_members.push(JsonMember { key: "content", value: json.from_string("Hello \"Nomo\"\n😀")? })
    let message: JsonValue = json.from_object(message_members)?
    let mut messages: Array<JsonValue> = Array.new<JsonValue>()
    messages.push(message)
    let mut request: Array<JsonMember> = Array.new<JsonMember>()
    request.push(JsonMember { key: "model", value: json.from_string("fixture")? })
    request.push(JsonMember { key: "messages", value: json.from_array(messages)? })
    request.push(JsonMember { key: "stream", value: json.from_bool(false) })
    request.push(JsonMember { key: "max_tokens", value: json.from_u64(64 as u64) })
    request.push(JsonMember { key: "penalty", value: json.from_i64(-2) })
    request.push(JsonMember { key: "metadata", value: json.from_null() })
    return json.from_object(request)
}

fn main() -> void {
    let request: Result<JsonValue, JsonError> = request_body()
    match request {
        Err(err) => {
            io.println(err.code, err.offset)
        }
        Ok(value) => {
            io.println(json.stringify(value))
        }
    }

    let response: Result<JsonValue, JsonError> = json.parse(" {\"choices\":[{\"message\":{\"content\":\"Hello from model\"}}],\"usage\":1E+2} ")
    match response {
        Err(err) => {
            io.println(err.code, err.offset)
        }
        Ok(root) => {
            match json.kind(root) {
                JsonKind.Null => {
                    io.println("null")
                }
                JsonKind.Boolean => {
                    io.println("boolean")
                }
                JsonKind.Number => {
                    io.println("number")
                }
                JsonKind.String => {
                    io.println("string")
                }
                JsonKind.Array => {
                    io.println("array")
                }
                JsonKind.Object => {
                    io.println("object")
                }
            }
            let usage: Option<JsonValue> = json.get(root, "usage")
            match usage {
                None => {
                    io.println("missing usage")
                }
                Some(value) => {
                    match json.number_text(value) {
                        None => {
                            io.println("usage is not a number")
                        }
                        Some(text) => {
                            io.println(text)
                        }
                    }
                }
            }
            let choices: Option<JsonValue> = json.get(root, "choices")
            match choices {
                None => {
                    io.println("missing choices")
                }
                Some(value) => {
                    let items: Option<Array<JsonValue>> = json.array_items(value)
                    match items {
                        None => {
                            io.println("choices is not an array")
                        }
                        Some(values) => {
                            match values.get(0) {
                                None => {
                                    io.println("choices is empty")
                                }
                                Some(choice) => {
                                    match json.get(choice, "message") {
                                        None => {
                                            io.println("missing message")
                                        }
                                        Some(message) => {
                                            match json.get(message, "content") {
                                                None => {
                                                    io.println("missing content")
                                                }
                                                Some(content) => {
                                                    match json.as_string(content) {
                                                        None => {
                                                            io.println("content is not a string")
                                                        }
                                                        Some(text) => {
                                                            io.println(text)
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    let duplicate: Result<JsonValue, JsonError> = json.parse("{\"\\u0064up\":1,\"dup\":2}")
    match duplicate {
        Err(err) => {
            io.println(err.code)
        }
        Ok(value) => {
            match json.object_members(value) {
                None => {
                    io.println("not object")
                }
                Some(members) => {
                    io.println(members.len())
                }
            }
            match json.get(value, "dup") {
                None => {
                    io.println("missing dup")
                }
                Some(found) => {
                    match json.number_text(found) {
                        None => {
                            io.println("dup is not number")
                        }
                        Some(text) => {
                            io.println(text)
                        }
                    }
                }
            }
        }
    }

    let invalid: Result<JsonValue, JsonError> = json.parse("\"\\u0000\"")
    match invalid {
        Ok(value) => {
            io.println(json.stringify(value))
        }
        Err(err) => {
            io.println(err.code, err.offset)
        }
    }
    let secret: Result<JsonValue, JsonError> = json.parse("{\"token\":\"NOMO_JSON_SECRET_SENTINEL\"")
    match secret {
        Ok(value) => {
            io.println(json.stringify(value))
        }
        Err(err) => {
            io.println(err.code, err.offset, err.message)
        }
    }
}
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(&project)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout).replace('\r', "");
    assert_eq!(
        stdout,
        "{\"model\":\"fixture\",\"messages\":[{\"role\":\"user\",\"content\":\"Hello \\\"Nomo\\\"\\n😀\"}],\"stream\":false,\"max_tokens\":64,\"penalty\":-2,\"metadata\":null}\nobject\n1E+2\nHello from model\n2\n2\nunsupported_string 1\nsyntax 36 invalid json syntax\n"
    );
    assert!(!stdout.contains("NOMO_JSON_SECRET_SENTINEL"));
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn structured_json_conformance_fixture_matches_native_runtime() {
    let root = temp_test_root("structured-json-conformance");
    reset_dir(&root);
    let project = root.join("structured_json_conformance");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nname = \"structured_json_conformance\"\nversion = \"0.1.0\"\n\n[dependencies]\nstd = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(
        project.join("src/main.nomo"),
        include_str!("../../../tests/fixtures/structured_json_conformance.nomo"),
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(&project)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout).replace('\r', "");
    assert_eq!(
        stdout,
        "true\n6\nnull\nboolean\nnumber\nstring\narray\nobject\ntrue\ntrue\nwrong-kind-none\n1E+2\ntrue\n0\n0\nnon-object-none\n2\nname\nname\n2\nmissing-none\n\"A\\n\\\"\\\\😀\"\n{\"null\":null,\"bool\":false,\"i64\":-9223372036854775808,\"u64\":18446744073709551615}\n😀\ninvalid_number 1 invalid json number\nunsupported_string 1\nunsupported_string 1\nsyntax 29 invalid json syntax\n"
    );
    assert!(!stdout.contains("NOMO_JSON_SECRET_SENTINEL"));
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn structured_json_enforces_native_boundary_limits() {
    const MAX_BYTES: usize = 8 * 1024 * 1024;
    const MAX_DEPTH: usize = 128;
    const MAX_VALUES: usize = 262_144;

    let root = temp_test_root("structured-json-boundaries");
    reset_dir(&root);
    let project = root.join("structured_json_boundaries");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nname = \"structured_json_boundaries\"\nversion = \"0.1.0\"\n\n[dependencies]\nstd = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(
        project.join("src/main.nomo"),
        r#"package structured_json_boundaries.main

import std.array.Array
import std.fs
import std.io
import std.json
import std.string

fn show_result(result: Result<JsonValue, JsonError>) -> void {
    match result {
        Ok(value) => {
            let raw: string = json.stringify(value)
            io.println("ok", raw.len())
        }
        Err(err) => {
            io.println(err.code, err.offset)
        }
    }
}

fn parse_file(path: string) -> void {
    let input: Result<string, FsError> = fs.read_to_string(path)
    match input {
        Err(err) => {
            io.println("fs", err.message)
        }
        Ok(text) => {
            show_result(json.parse(text))
        }
    }
}

fn construct_string_file(path: string) -> void {
    let input: Result<string, FsError> = fs.read_to_string(path)
    match input {
        Err(err) => {
            io.println("fs", err.message)
        }
        Ok(text) => {
            show_result(json.from_string(text))
        }
    }
}

fn nested_value(count: u64) -> Result<JsonValue, JsonError> {
    let mut value: JsonValue = json.from_null()
    for let i: u64 = 0; i < count; i++ {
        let mut values: Array<JsonValue> = Array.new<JsonValue>()
        values.push(value)
        value = json.from_array(values)?
    }
    return Ok(value)
}

fn value_array(count: u64) -> Result<JsonValue, JsonError> {
    let mut values: Array<JsonValue> = Array.new<JsonValue>()
    for let i: u64 = 0; i < count; i++ {
        values.push(json.from_null())
    }
    return json.from_array(values)
}

fn number_boundaries() -> void {
    let smallest: JsonValue = json.from_i64(-9223372036854775807 - 1)
    let largest_number: u64 = (9223372036854775807 as u64) * 2 + 1
    let largest: JsonValue = json.from_u64(largest_number)
    io.println(json.stringify(smallest))
    io.println(json.stringify(largest))
    show_result(json.from_number_text("01"))
    show_result(json.from_number_text("1E+2"))
}

fn empty_containers() -> void {
    let empty_values: Array<JsonValue> = Array.new<JsonValue>()
    let empty_members: Array<JsonMember> = Array.new<JsonMember>()
    show_result(json.from_array(empty_values))
    show_result(json.from_object(empty_members))
}

fn wrong_kind_access() -> void {
    let value: JsonValue = json.from_bool(true)
    io.println(json.is_null(value))
    match json.as_bool(value) {
        None => {
            io.println("missing bool")
        }
        Some(flag) => {
            io.println(flag)
        }
    }
    match json.as_string(value) {
        None => {
            io.println("none")
        }
        Some(text) => {
            io.println(text)
        }
    }
}

fn main() -> void {
    parse_file("text_exact.json")
    parse_file("text_over.json")
    parse_file("depth_exact.json")
    parse_file("depth_over.json")
    parse_file("values_exact.json")
    parse_file("values_over.json")
    construct_string_file("string_exact.txt")
    construct_string_file("string_over.txt")
    show_result(nested_value(128))
    show_result(nested_value(129))
    show_result(value_array(262143))
    show_result(value_array(262144))
    number_boundaries()
    empty_containers()
    wrong_kind_access()
}
"#,
    )
    .unwrap();

    let exact_text = format!("\"{}\"", "a".repeat(MAX_BYTES - 2));
    let oversized_text = format!("\"{}\"", "a".repeat(MAX_BYTES - 1));
    let exact_depth = format!("{}null{}", "[".repeat(MAX_DEPTH), "]".repeat(MAX_DEPTH));
    let oversized_depth = format!(
        "{}null{}",
        "[".repeat(MAX_DEPTH + 1),
        "]".repeat(MAX_DEPTH + 1)
    );
    let exact_values = format!("[{}]", vec!["null"; MAX_VALUES - 1].join(","));
    let oversized_values = format!("[{}]", vec!["null"; MAX_VALUES].join(","));

    assert_eq!(exact_text.len(), MAX_BYTES);
    assert_eq!(oversized_text.len(), MAX_BYTES + 1);
    fs::write(project.join("text_exact.json"), exact_text).unwrap();
    fs::write(project.join("text_over.json"), oversized_text).unwrap();
    fs::write(project.join("depth_exact.json"), exact_depth).unwrap();
    fs::write(project.join("depth_over.json"), oversized_depth).unwrap();
    fs::write(project.join("values_exact.json"), exact_values).unwrap();
    fs::write(project.join("values_over.json"), oversized_values).unwrap();
    fs::write(project.join("string_exact.txt"), "a".repeat(MAX_BYTES - 2)).unwrap();
    fs::write(project.join("string_over.txt"), "a".repeat(MAX_BYTES - 1)).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .current_dir(&project)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).replace('\r', ""),
        format!(
            "ok {MAX_BYTES}\nlimit {MAX_BYTES}\nok 260\nlimit {MAX_DEPTH}\nok 1310716\nlimit 1310716\nok {MAX_BYTES}\nlimit 0\nok 260\nlimit 0\nok 1310716\nlimit 0\n-9223372036854775808\n18446744073709551615\ninvalid_number 1\nok 4\nok 2\nok 2\nfalse\ntrue\nnone\n"
        )
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn structured_json_native_runtime_rejects_invalid_utf8() {
    let root = temp_test_root("structured-json-invalid-utf8");
    reset_dir(&root);
    let source = root.join("main.nomo");
    let c_path = root.join("main.c");
    let bin_path = root.join(if cfg!(windows) {
        "structured-json-invalid-utf8.exe"
    } else {
        "structured-json-invalid-utf8"
    });
    fs::write(
        &source,
        r#"package structured_json_invalid_utf8.main

import std.json

fn main() -> void {
    let value: Result<JsonValue, JsonError> = json.parse("{}")
}
"#,
    )
    .unwrap();

    let generated = Command::new(env!("CARGO_BIN_EXE_nomoc"))
        .arg("build")
        .arg(&source)
        .arg("--emit-c")
        .arg("--out")
        .arg(&c_path)
        .output()
        .unwrap();
    assert!(
        generated.status.success(),
        "{}",
        String::from_utf8_lossy(&generated.stderr)
    );

    let mut c = fs::read_to_string(&c_path).unwrap();
    let main_start = c
        .find("int main(void) {")
        .expect("generated C contains main");
    c.truncate(main_start);
    c.push_str(
        r#"int main(void) {
    const char invalid[] = {'"', (char)0xc3, '(', '"', '\0'};
    nomo_json_cursor cursor;
    if (nomo_json_validate(invalid, 4U, &cursor)) {
        return 1;
    }
    if (
        cursor.error_code == NULL
        || strcmp(cursor.error_code, "unsupported_string") != 0
    ) {
        return 2;
    }
    if (cursor.error_offset != 2U) {
        return 3;
    }
    return 0;
}
"#,
    );
    fs::write(&c_path, c).unwrap();

    let compiler = if cfg!(windows) { "clang" } else { "cc" };
    let compiled = Command::new(compiler)
        .arg("-std=c99")
        .arg(&c_path)
        .arg("-o")
        .arg(&bin_path)
        .output()
        .unwrap();
    assert!(
        compiled.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compiled.stdout),
        String::from_utf8_lossy(&compiled.stderr)
    );
    let output = Command::new(&bin_path).output().unwrap();
    assert!(
        output.status.success(),
        "status: {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn nomo_run_executes_std_regex_helpers_with_question() {
    let root = temp_test_root("std-regex-helpers");
    reset_dir(&root);
    let project = root.join("std_regex_helpers");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nname = \"std_regex_helpers\"\nversion = \"0.1.0\"\n\n[dependencies]\nstd = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(
        project.join("src/main.nomo"),
        r#"package app.main

import std.array
import std.io
import std.num
import std.regex

fn print_group(groups: Array<string>, index: u64) -> void {
    match groups.get(index) {
        Some(value) => {
            io.println(value)
        }
        None => {
            io.println("missing")
        }
    }
}

fn main() -> Result<void, RegexError> {
    let rx: Regex = regex.compile("(nomo)-([0-9]+)")?
    if regex.is_match(rx, "hello nomo-42") {
        io.println("matched")
    } else {
        io.println("missing")
    }

    match regex.captures(rx, "hello nomo-42") {
        Some(groups) => {
            io.println(num.to_string(groups.len()))
            print_group(groups, 0)
            print_group(groups, 1)
            print_group(groups, 2)
        }
        None => {
            io.println("no-captures")
        }
    }
    return Ok(void)
}
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(&project)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "matched\n3\nnomo-42\nnomo\n42\n"
    );
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_run_executes_std_collections_helpers() {
    let root = temp_test_root("std-collections-helpers");
    reset_dir(&root);
    let project = root.join("std_collections_helpers");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nname = \"std_collections_helpers\"\nversion = \"0.1.0\"\n\n[dependencies]\nstd = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(
        project.join("src/main.nomo"),
        r#"package app.main

import std.collections
import std.io
import std.num
import std.option

fn main() -> void {
    let mut map: StringMap = collections.map_new()
    map = collections.map_set(map, "lang", "nomo")
    map = collections.map_set(map, "tool", "compiler")
    map = collections.map_set(map, "lang", "nomo2")
    io.println(option.unwrap_or(collections.map_get(map, "lang"), "missing"))
    io.println(num.to_string(collections.map_len(map)))
    map = collections.map_remove(map, "tool")
    io.println(num.to_string(collections.map_len(map)))
    if collections.map_contains(map, "tool") {
        io.println("tool-present")
    } else {
        io.println("tool-missing")
    }

    let mut set: StringSet = collections.set_new()
    set = collections.set_insert(set, "nomo")
    set = collections.set_insert(set, "nomo")
    set = collections.set_insert(set, "lang")
    io.println(num.to_string(collections.set_len(set)))
    if collections.set_contains(set, "lang") {
        io.println("lang-present")
    } else {
        io.println("lang-missing")
    }
    set = collections.set_remove(set, "lang")
    io.println(num.to_string(collections.set_len(set)))
    if collections.set_contains(set, "lang") {
        io.println("lang-present")
    } else {
        io.println("lang-missing")
    }
}
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(&project)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "nomo2\n2\n1\ntool-missing\n2\nlang-present\n1\nlang-missing\n"
    );
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_run_uses_question_for_result_propagation() {
    let root = temp_test_root("question-result-propagation");
    reset_dir(&root);
    let project = root.join("question_result_propagation");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nname = \"question_result_propagation\"\nversion = \"0.1.0\"\n\n[dependencies]\nstd = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(
        project.join("src/main.nomo"),
        r#"package app.main

import std.io

fn load_value() -> Result<string, string> {
    return Ok("question")
}

fn compute() -> Result<string, string> {
    let value: string = load_value()?
    return Ok(value)
}

fn main() -> void {
    let result: Result<string, string> = compute()
    match result {
        Ok(value) => {
            io.println(value)
        }
        Err(err) => {
            io.println(err)
        }
    }
}
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(&project)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), "question\n");
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_run_uses_question_for_option_propagation() {
    let root = temp_test_root("question-option-propagation");
    reset_dir(&root);
    let project = root.join("question_option_propagation");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nname = \"question_option_propagation\"\nversion = \"0.1.0\"\n\n[dependencies]\nstd = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(
        project.join("src/main.nomo"),
        r#"package app.main

import std.io

fn load_value() -> Option<string> {
    return Some("question")
}

fn compute() -> Option<string> {
    return load_value()?
}

fn main() -> void {
    let result: Option<string> = compute()
    match result {
        Some(value) => {
            io.println(value)
        }
        None => {
            io.println("none")
        }
    }
}
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(&project)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), "question\n");
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_run_reports_result_main_error_status() {
    let root = temp_test_root("result-main-error");
    reset_dir(&root);
    let project = root.join("err_main");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nname = \"err_main\"\nversion = \"0.1.0\"\n\n[dependencies]\nstd = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(
        project.join("src/main.nomo"),
        r#"package app.main

import std.result.Result

enum AppError {
    Failed(string)
}

fn main() -> Result<void, AppError> {
    return Result.Err(AppError.Failed("boom"))
}
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(&project)
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("program exited with status 1"), "{stderr}");

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_run_reports_direct_panic_status() {
    let root = temp_test_root("direct-panic");
    reset_dir(&root);
    let project = root.join("direct_panic");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nname = \"direct_panic\"\nversion = \"0.1.0\"\n\n[dependencies]\nstd = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(
        project.join("src/main.nomo"),
        r#"package app.main

fn main() -> void {
    panic("boom")
}
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(&project)
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stdout.is_empty(), "{stdout}");
    assert!(stderr.contains("panic: boom"), "{stderr}");
    assert!(stderr.contains("program exited with status 1"), "{stderr}");

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn managed_sync_panic_message_survives_local_cleanup_and_is_asan_clean() {
    let root = temp_test_root("managed-sync-panic");
    reset_dir(&root);
    let project = root.join("managed_sync_panic");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nname = \"managed_sync_panic\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(
        project.join("src/main.nomo"),
        r#"package app.main

import std.string

fn main() -> void {
    let prefix: string = "managed"
    let message: string = prefix.concat(" sync panic")
    panic(message)
}
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(&project)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("panic: managed sync panic"), "{stderr}");
    assert!(stderr.contains("program exited with status 1"), "{stderr}");

    let generated_c = project.join("build/c/main.c");
    let generated = fs::read_to_string(&generated_c).unwrap();
    let capture = generated
        .find("nomo_string nomo__panic_message = nomo_message;")
        .unwrap();
    let retain = generated[capture..]
        .find("nomo__panic_message = nomo_string_retain(nomo__panic_message);")
        .map(|index| capture + index)
        .unwrap();
    let release = generated[retain..]
        .find("nomo_string_release(nomo_message);")
        .map(|index| retain + index)
        .unwrap();
    let terminate = generated[release..]
        .find("nomo_panic_string(nomo__panic_message);")
        .map(|index| release + index)
        .unwrap();
    assert!(capture < retain);
    assert!(retain < release);
    assert!(release < terminate);

    if cc_supports_address_sanitizer(&root) {
        let bin_path = root.join(if cfg!(windows) {
            "asan-managed-sync-panic.exe"
        } else {
            "asan-managed-sync-panic"
        });
        let cc_output = Command::new("cc")
            .arg("-fsanitize=address")
            .arg("-fno-omit-frame-pointer")
            .arg("-g")
            .arg(&generated_c)
            .arg("-o")
            .arg(&bin_path)
            .output()
            .unwrap();
        assert!(
            cc_output.status.success(),
            "{}{}",
            String::from_utf8_lossy(&cc_output.stdout),
            String::from_utf8_lossy(&cc_output.stderr)
        );
        let asan_options = if cfg!(target_os = "macos") {
            "detect_leaks=0:abort_on_error=1"
        } else {
            "detect_leaks=1:abort_on_error=1"
        };
        let asan_output = Command::new(&bin_path)
            .env("ASAN_OPTIONS", asan_options)
            .output()
            .unwrap();
        assert!(!asan_output.status.success());
        assert!(asan_output.stdout.is_empty());
        assert_eq!(
            String::from_utf8_lossy(&asan_output.stderr),
            "panic: managed sync panic\n"
        );
    }

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_run_reports_debug_panic_status() {
    let root = temp_test_root("debug-panic");
    reset_dir(&root);
    let project = root.join("debug_panic");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nname = \"debug_panic\"\nversion = \"0.1.0\"\n\n[dependencies]\nstd = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(
        project.join("src/main.nomo"),
        r#"package app.main

import std.debug

fn main() -> void {
    debug.panic("debug-boom")
}
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(&project)
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stdout.is_empty(), "{stdout}");
    assert!(stderr.contains("panic: debug-boom"), "{stderr}");
    assert!(stderr.contains("program exited with status 1"), "{stderr}");

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_run_reports_array_set_panic_status() {
    let root = temp_test_root("array-set-panic");
    reset_dir(&root);
    let project = root.join("array_panic");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nname = \"array_panic\"\nversion = \"0.1.0\"\n\n[dependencies]\nstd = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(
        project.join("src/main.nomo"),
        r#"package app.main

import std.array

fn main() -> void {
    let mut items: Array<i32> = Array.new<i32>()
    items.push(1)
    items.set(1, 2)
}
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(&project)
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stdout.is_empty(), "{stdout}");
    assert!(
        stderr.contains("panic: Array.set index out of bounds"),
        "{stderr}"
    );
    assert!(stderr.contains("program exited with status 1"), "{stderr}");

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_run_reports_division_by_zero_panic_status() {
    let root = temp_test_root("division-by-zero-panic");
    reset_dir(&root);
    let project = root.join("division_panic");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nname = \"division_panic\"\nversion = \"0.1.0\"\n\n[dependencies]\nstd = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(
        project.join("src/main.nomo"),
        r#"package app.main

import std.io

fn main() -> void {
    let value: i64 = 1 / 0
    io.println("wrong")
}
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(&project)
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stdout.is_empty(), "{stdout}");
    assert!(stderr.contains("panic: division by zero"), "{stderr}");
    assert!(stderr.contains("program exited with status 1"), "{stderr}");

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_run_reports_signed_overflow_panic_status() {
    let root = temp_test_root("signed-overflow-panic");
    reset_dir(&root);
    let project = root.join("overflow_panic");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nname = \"overflow_panic\"\nversion = \"0.1.0\"\n\n[dependencies]\nstd = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(
        project.join("src/main.nomo"),
        r#"package app.main

import std.io

fn main() -> void {
    let max: i64 = 9223372036854775807
    let value: i64 = max + 1
    io.println("wrong")
}
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(&project)
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stdout.is_empty(), "{stdout}");
    assert!(
        stderr.contains("panic: signed integer overflow"),
        "{stderr}"
    );
    assert!(stderr.contains("program exited with status 1"), "{stderr}");

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_run_reports_invalid_shift_panic_status() {
    let root = temp_test_root("invalid-shift-panic");
    reset_dir(&root);
    let project = root.join("shift_panic");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nname = \"shift_panic\"\nversion = \"0.1.0\"\n\n[dependencies]\nstd = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(
        project.join("src/main.nomo"),
        r#"package app.main

import std.io

fn main() -> void {
    let value: i64 = 1 << 64
    io.println("wrong")
}
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(&project)
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stdout.is_empty(), "{stdout}");
    assert!(stderr.contains("panic: invalid shift amount"), "{stderr}");
    assert!(stderr.contains("program exited with status 1"), "{stderr}");

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_run_reports_signed_left_shift_overflow_panic_status() {
    let root = temp_test_root("signed-left-shift-overflow-panic");
    reset_dir(&root);
    let project = root.join("shift_overflow_panic");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nname = \"shift_overflow_panic\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(
        project.join("src/main.nomo"),
        r#"package app.main

fn main() -> void {
    let value: i64 = 4611686018427387904 << 1
}
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(&project)
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stdout.is_empty(), "{stdout}");
    assert!(
        stderr.contains("panic: signed integer overflow"),
        "{stderr}"
    );
    assert!(stderr.contains("program exited with status 1"), "{stderr}");

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_run_handles_fs_read_error_as_result_value() {
    let root = temp_test_root("fs-read-error-result");
    reset_dir(&root);
    let project = root.join("fs_error");
    let missing_file = root.join("missing-input.txt");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nname = \"fs_error\"\nversion = \"0.1.0\"\n\n[dependencies]\nstd = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(
        project.join("src/main.nomo"),
        format!(
            r#"package app.main

import std.fs
import std.io

fn main() -> void {{
    let result: Result<string, FsError> = fs.read_to_string("{}")
    let message: string = match result {{
        Ok(text) => "wrong"
        Err(err) => if err.message == "" {{
            "wrong"
        }} else {{
            "fs error ok"
        }}
    }}
    io.println(message)
}}
"#,
            missing_file.display()
        ),
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(&project)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), "fs error ok\n");
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_run_handles_fs_write_error_as_result_value() {
    let root = temp_test_root("fs-write-error-result");
    reset_dir(&root);
    let project = root.join("fs_write_error");
    let directory_target = root.join("not-a-file");
    fs::create_dir_all(&directory_target).unwrap();
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nname = \"fs_write_error\"\nversion = \"0.1.0\"\n\n[dependencies]\nstd = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(
        project.join("src/main.nomo"),
        format!(
            r#"package app.main

import std.fs
import std.io

fn main() -> void {{
    let result: Result<void, FsError> = fs.write_string("{}", "content")
    let message: string = match result {{
        Ok(value) => "wrong"
        Err(err) => if err.message == "" {{
            "wrong"
        }} else {{
            "fs write error ok"
        }}
    }}
    io.println(message)
}}
"#,
            directory_target.display()
        ),
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(&project)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "fs write error ok\n"
    );
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_run_executes_fs_read_and_write_bytes() {
    let root = temp_test_root("fs-bytes");
    reset_dir(&root);
    let project = root.join("fs_bytes");
    let output_path = project.join("out.bin");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nname = \"fs_bytes\"\nversion = \"0.1.0\"\n\n[dependencies]\nstd = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(
        project.join("src/main.nomo"),
        format!(
            r#"package app.main

import std.array
import std.fs
import std.io
import std.num

fn label(value: Option<u32>) -> string {{
    return match value {{
        Some(byte) => num.to_string(byte)
        None => "missing"
    }}
}}

fn main() -> Result<void, FsError> {{
    let mut bytes: Array<u32> = Array.new<u32>()
    bytes.push(65 as u32)
    bytes.push(66 as u32)
    bytes.push(255 as u32)
    fs.write_bytes("{}", bytes)?
    let read: Array<u32> = fs.read_bytes("{}")?
    io.println(num.to_string(read.len()))
    io.println(label(read.get(0 as u64)))
    io.println(label(read.get(1 as u64)))
    io.println(label(read.get(2 as u64)))
    return Result.Ok(void)
}}
"#,
            output_path.display(),
            output_path.display()
        ),
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(&project)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), "3\n65\n66\n255\n");
    assert_eq!(fs::read(&output_path).unwrap(), vec![65, 66, 255]);
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_run_handles_fs_open_error_as_result_value() {
    let root = temp_test_root("fs-open-error-result");
    reset_dir(&root);
    let project = root.join("fs_open_error");
    let missing_file = root.join("missing-open.txt");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nname = \"fs_open_error\"\nversion = \"0.1.0\"\n\n[dependencies]\nstd = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(
        project.join("src/main.nomo"),
        format!(
            r#"package app.main

import std.fs
import std.io

fn main() -> void {{
    let result: Result<File, FsError> = fs.open("{}")
    let message: string = match result {{
        Ok(file) => "wrong"
        Err(err) => if err.message == "" {{
            "wrong"
        }} else {{
            "fs open error ok"
        }}
    }}
    io.println(message)
}}
"#,
            missing_file.display()
        ),
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(&project)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "fs open error ok\n"
    );
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_run_executes_file_read_and_write_string_methods() {
    let root = temp_test_root("file-read-write-string-methods");
    reset_dir(&root);
    let project = root.join("file_methods");
    let target_file = root.join("target.txt");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(&target_file, "").unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nname = \"file_methods\"\nversion = \"0.1.0\"\n\n[dependencies]\nstd = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(
        project.join("src/main.nomo"),
        format!(
            r#"package app.main

import std.fs
import std.io

fn checked(path: string) -> Result<string, FsError> {{
    let file: File = fs.open(path)?
    file.write_string("via file")?
    let text: string = file.read_to_string()?
    file.close()
    return Ok(text)
}}

fn main() -> void {{
    match checked("{}") {{
        Ok(text) => {{
            io.println(text)
        }}
        Err(err) => {{
            io.println(err.message)
        }}
    }}
}}
"#,
            target_file.display()
        ),
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(&project)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), "via file\n");
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_run_executes_fs_directory_helpers() {
    let root = temp_test_root("fs-directory-helpers");
    reset_dir(&root);
    let project = root.join("fs_dirs");
    let empty_dir = root.join("empty");
    let list_dir = root.join("list");
    let list_a = list_dir.join("a.txt");
    let list_b = list_dir.join("b.txt");
    let marker = root.join("marker.txt");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nname = \"fs_dirs\"\nversion = \"0.1.0\"\n\n[dependencies]\nstd = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(
        project.join("src/main.nomo"),
        format!(
            r#"package app.main

import std.array
import std.fs
import std.io

fn has_entry(entries: Array<string>, needle: string) -> bool {{
    let mut found: bool = false
    for entry in entries {{
        found = found || entry == needle
    }}
    return found
}}

fn checked() -> Result<void, FsError> {{
    fs.create_dir("{}")?
    let exists_message: string = if fs.exists("{}") {{ "empty exists" }} else {{ "empty missing" }}
    io.println(exists_message)
    let empty_entries: Array<string> = fs.read_dir("{}")?
    let empty_message: string = if empty_entries.len() == 0 {{ "empty read" }} else {{ "empty unexpected" }}
    io.println(empty_message)
    fs.remove_dir("{}")?
    let remove_message: string = if fs.exists("{}") {{ "remove failed" }} else {{ "empty removed" }}
    io.println(remove_message)
    fs.create_dir("{}")?
    fs.write_string("{}", "a")?
    fs.write_string("{}", "b")?
    let metadata: FileMetadata = fs.metadata("{}")?
    let metadata_message: string = if metadata.is_file && !metadata.is_dir && metadata.size == 1 as u64 {{ "metadata ok" }} else {{ "metadata wrong" }}
    io.println(metadata_message)
    let entries: Array<string> = fs.read_dir("{}")?
    let has_a: bool = has_entry(entries, "a.txt")
    let has_b: bool = has_entry(entries, "b.txt")
    let list_message: string = if has_a && has_b {{ "list read" }} else {{ "list missing" }}
    io.println(list_message)
    return fs.write_string("{}", "ok")?
}}

fn main() -> void {{
    match checked() {{
        Ok(value) => {{
            io.println("fs dirs ok")
        }}
        Err(err) => {{
            io.println(err.message)
        }}
    }}
}}
"#,
            empty_dir.display(),
            empty_dir.display(),
            empty_dir.display(),
            empty_dir.display(),
            empty_dir.display(),
            list_dir.display(),
            list_a.display(),
            list_b.display(),
            list_a.display(),
            list_dir.display(),
            marker.display()
        ),
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(&project)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "empty exists\nempty read\nempty removed\nmetadata ok\nlist read\nfs dirs ok\n"
    );
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_run_handles_missing_env_get_as_none() {
    let root = temp_test_root("env-get-none");
    reset_dir(&root);
    let project = root.join("env_none");
    let var_name = format!("NOMO_ABSENT_ENV_{}", std::process::id());
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nname = \"env_none\"\nversion = \"0.1.0\"\n\n[dependencies]\nstd = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(
        project.join("src/main.nomo"),
        format!(
            r#"package app.main

import std.env
import std.io

fn main() -> void {{
    let value: Option<string> = env.get("{}")
    let message: string = match value {{
        Some(text) => "wrong"
        None => "env none ok"
    }}
    io.println(message)
}}
"#,
            var_name
        ),
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(&project)
        .env_remove(&var_name)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), "env none ok\n");
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_run_executes_extended_std_env_helpers() {
    let root = temp_test_root("env-extended");
    reset_dir(&root);
    let project = root.join("env_extended");
    let var_name = format!("NOMO_SET_ENV_{}", std::process::id());
    let home_dir = root.join("home");
    let temp_dir = root.join("tmp");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::create_dir_all(&home_dir).unwrap();
    fs::create_dir_all(&temp_dir).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nname = \"env_extended\"\nversion = \"0.1.0\"\n\n[dependencies]\nstd = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(
        project.join("src/main.nomo"),
        format!(
            r#"package app.main

import std.env
import std.io
import std.option
import std.string

fn main() -> void {{
    env.set("{}", "set ok")
    let value: Option<string> = env.get("{}")
    let label: string = match value {{
        Some(text) => text
        None => "missing"
    }}
    io.println(label)

    let cwd_path: string = env.cwd()
    if cwd_path.contains("env_extended") {{
        io.println("cwd ok")
    }} else {{
        io.println("wrong cwd")
    }}

    let home: Option<string> = env.home_dir()
    let home_label: string = match home {{
        Some(path) => path
        None => "missing home"
    }}
    io.println(home_label)
    io.println(env.temp_dir())
}}
"#,
            var_name, var_name
        ),
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(&project)
        .env("HOME", &home_dir)
        .env("TMPDIR", &temp_dir)
        .env_remove(&var_name)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        format!(
            "set ok\ncwd ok\n{}\n{}\n",
            home_dir.display(),
            temp_dir.display()
        )
    );
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_run_forwards_program_arguments_after_separator() {
    let root = temp_test_root("run-args");
    reset_dir(&root);
    let project = root.join("args_demo");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nname = \"args_demo\"\nversion = \"0.1.0\"\n\n[dependencies]\nstd = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(
        project.join("src/main.nomo"),
        r#"package app.main

import std.array
import std.env
import std.io

fn main() -> void {
    let args: Array<string> = env.args()
    let first: Option<string> = args.get(1)
    let message: string = match first {
        Some(text) => text
        None => "missing"
    }
    io.println(message)
}
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(&project)
        .arg("--")
        .arg("from-cli")
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), "from-cli\n");

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_run_executes_std_path_helpers() {
    let root = temp_test_root("std-path-helpers");
    reset_dir(&root);
    let project = root.join("path_demo");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nname = \"path_demo\"\nversion = \"0.1.0\"\n\n[dependencies]\nstd = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(
        project.join("src/main.nomo"),
        r#"package app.main

import std.io
import std.path

fn main() -> void {
    io.println(path.join("/tmp", "nomo.txt"))
    io.println(path.basename("/tmp/nomo.txt"))
    io.println(path.dirname("/tmp/nomo.txt"))
    io.println(path.extension("archive.tar.gz"))
    io.println(path.normalize("/tmp//a/../b/./"))
    io.println(path.normalize("a/../../b"))
    if path.is_absolute("/tmp") {
        io.println("absolute")
    } else {
        io.println("relative")
    }
}
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(&project)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "/tmp/nomo.txt\nnomo.txt\n/tmp\ngz\n/tmp/b\n../b\nabsolute\n"
    );
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_run_executes_extended_std_string_helpers() {
    let root = temp_test_root("std-string-helpers");
    reset_dir(&root);
    let project = root.join("string_demo");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nname = \"string_demo\"\nversion = \"0.1.0\"\n\n[dependencies]\nstd = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(
        project.join("src/main.nomo"),
        r#"package app.main

import std.array
import std.io
import std.option
import std.string

fn main() -> void {
    let text: string = "  NoMo  "
    if !text.is_empty() && text.contains("No") && text.starts_with("  N") && text.ends_with("  ") {
        io.println("predicates")
    } else {
        io.println("bad predicates")
    }
    if text.trim() == "NoMo" {
        io.println("trim")
    } else {
        io.println("bad trim")
    }
    if text.to_lower() == "  nomo  " && text.to_upper() == "  NOMO  " {
        io.println("case")
    } else {
        io.println("bad case")
    }
    let csv: string = "a,b,c"
    let parts: Array<string> = csv.split(",")
    let second: Option<string> = parts.get(1)
    let label: string = match second {
        Some(value) => value
        None => "missing"
    }
    io.println(label)
}
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(&project)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "predicates\ntrim\ncase\nb\n"
    );
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_run_executes_std_math_helpers() {
    let root = temp_test_root("std-math-helpers");
    reset_dir(&root);
    let project = root.join("math_demo");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nname = \"math_demo\"\nversion = \"0.1.0\"\n\n[dependencies]\nstd = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(
        project.join("src/main.nomo"),
        r#"package app.main

import std.io
import std.math

fn main() -> void {
    if math.abs(0 - 7) == 7 {
        io.println("abs")
    } else {
        io.println("bad abs")
    }
    if math.min(3, 9) == 3 && math.max(3, 9) == 9 {
        io.println("minmax")
    } else {
        io.println("bad minmax")
    }
    if math.floor(3.8) == 3.0 && math.ceil(3.1) == 4.0 && math.round(3.5) == 4.0 {
        io.println("rounding")
    } else {
        io.println("bad rounding")
    }
    if math.sqrt(9.0) == 3.0 && math.pow(2.0, 3.0) == 8.0 {
        io.println("power")
    } else {
        io.println("bad power")
    }
    if math.sin(0.0) == 0.0 && math.cos(0.0) == 1.0 {
        io.println("trig")
    } else {
        io.println("bad trig")
    }
}
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(&project)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "abs\nminmax\nrounding\npower\ntrig\n"
    );
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_run_executes_std_char_helpers() {
    let root = temp_test_root("std-char-helpers");
    reset_dir(&root);
    let project = root.join("char_demo");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nname = \"char_demo\"\nversion = \"0.1.0\"\n\n[dependencies]\nstd = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(
        project.join("src/main.nomo"),
        r#"package app.main

import std.char
import std.io

fn main() -> void {
    let digit: string = if char.is_digit('7') { "digit" } else { "bad digit" }
    let alpha: string = if char.is_alpha('N') { "alpha" } else { "bad alpha" }
    let space: string = if char.is_whitespace(' ') { "space" } else { "bad space" }
    let ascii: string = if !char.is_alpha('語') { "ascii-only" } else { "bad ascii" }
    io.println(digit)
    io.println(alpha)
    io.println(space)
    io.println(ascii)
    io.println(char.to_string('語'))
}
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(&project)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "digit\nalpha\nspace\nascii-only\n語\n"
    );
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_run_executes_std_os_helpers() {
    let root = temp_test_root("std-os-helpers");
    reset_dir(&root);
    let project = root.join("os_demo");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nname = \"os_demo\"\nversion = \"0.1.0\"\n\n[dependencies]\nstd = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(
        project.join("src/main.nomo"),
        r#"package app.main

import std.io
import std.os

fn main() -> void {
    io.println(os.platform())
    io.println(os.arch())
    io.println(os.path_separator())
    let ending: string = if os.line_ending() == "\n" { "lf" } else { "crlf" }
    io.println(ending)
}
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(&project)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let expected_platform = if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else if cfg!(target_os = "freebsd") {
        "freebsd"
    } else {
        "unknown"
    };
    let expected_arch = if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else if cfg!(target_arch = "x86_64") {
        "x86_64"
    } else if cfg!(target_arch = "x86") {
        "x86"
    } else if cfg!(target_arch = "arm") {
        "arm"
    } else {
        "unknown"
    };
    let expected_separator = if cfg!(windows) { "\\" } else { "/" };
    let expected_ending = if cfg!(windows) { "crlf" } else { "lf" };
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        format!("{expected_platform}\n{expected_arch}\n{expected_separator}\n{expected_ending}\n")
    );
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_run_executes_std_time_helpers() {
    let root = temp_test_root("std-time-helpers");
    reset_dir(&root);
    let project = root.join("time_helpers");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nname = \"time_helpers\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(
        project.join("src/main.nomo"),
        r#"package app.main

import std.io
import std.time

fn main() -> void {
    let now: i64 = time.now_millis()
    let before: i64 = time.monotonic_millis()
    time.sleep_millis(0)
    let after: i64 = time.monotonic_millis()
    if now > 0 && after >= before {
        io.println("ok")
    } else {
        io.println("bad")
    }
}
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(&project)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), "ok\n");
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_run_executes_std_debug_helpers() {
    let root = temp_test_root("std-debug-helpers");
    reset_dir(&root);
    let project = root.join("debug_helpers");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nname = \"debug_helpers\"\nversion = \"0.1.0\"\n\n[dependencies]\nstd = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(
        project.join("src/main.nomo"),
        r#"package app.main

import std.debug
import std.io

fn main() -> void {
    debug.print("debug-")
    debug.println("ok")
    io.println(debug.backtrace())
}
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(&project)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "backtrace unavailable\n"
    );
    assert_eq!(String::from_utf8_lossy(&output.stderr), "debug-ok\n");

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_run_executes_std_time_duration_helpers() {
    let root = temp_test_root("std-time-duration-helpers");
    reset_dir(&root);
    let project = root.join("time_duration_helpers");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nname = \"time_duration_helpers\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(
        project.join("src/main.nomo"),
        r#"package app.main

import std.io
import std.num
import std.time

fn main() -> void {
    let short: Duration = time.duration_millis(1500)
    let long: Duration = time.duration_seconds(2)
    time.sleep(time.duration_millis(0))
    io.println(num.to_string(time.duration_as_millis(short)))
    io.println(num.to_string(time.duration_as_millis(long)))
    io.println(time.format_duration(short))
}
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(&project)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "1500\n2000\n1500ms\n"
    );
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_run_executes_std_process_helpers() {
    let root = temp_test_root("std-process-helpers");
    reset_dir(&root);
    let project = root.join("process_helpers");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nname = \"process_helpers\"\nversion = \"0.1.0\"\n\n[dependencies]\nstd = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(
        project.join("src/main.nomo"),
        r#"package app.main

import std.io
import std.process

fn main() -> void {
    let spawned: Result<i32, ProcessError> = process.spawn("printf spawn-ok >/dev/null")
    match spawned {
        Ok(code) => {
            if code == 0 {
                io.println("spawn-ok")
            } else {
                io.println("spawn-bad")
            }
        }
        Err(err) => {
            io.println(err.message)
        }
    }
    let status: Result<i32, ProcessError> = process.status("printf status-ok >/dev/null")
    match status {
        Ok(code) => {
            if code == 0 {
                io.println("status-ok")
            } else {
                io.println("status-bad")
            }
        }
        Err(err) => {
            io.println(err.message)
        }
    }
    let output: Result<string, ProcessError> = process.exec("printf process-ok")
    match output {
        Ok(text) => {
            io.println(text)
        }
        Err(err) => {
            io.println(err.message)
        }
    }
    let captured: Result<ProcessOutput, ProcessError> = process.output("printf captured-out; printf captured-err 1>&2; exit 7")
    match captured {
        Ok(value) => {
            let marker: string = if value.status == 7 { "status-7" } else { "bad-status" }
            io.println(marker)
            io.println(value.stdout)
            io.println(value.stderr)
        }
        Err(err) => {
            io.println(err.message)
        }
    }
}
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(&project)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "spawn-ok\nstatus-ok\nprocess-ok\nstatus-7\ncaptured-out\ncaptured-err\n"
    );
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    fs::remove_dir_all(&root).unwrap();
}

fn build_controlled_process_fixture(root: &Path) -> PathBuf {
    let fixture_source = root.join("process_fixture.c");
    let fixture_binary = if cfg!(windows) {
        root.join("process_fixture.exe")
    } else {
        root.join("process_fixture")
    };
    fs::write(
        &fixture_source,
        r#"#define _POSIX_C_SOURCE 200809L
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#ifdef _WIN32
#include <direct.h>
#include <windows.h>
#define NOMO_FIXTURE_GETCWD _getcwd
static void nomo_fixture_sleep(unsigned long millis) { Sleep((DWORD)millis); }
#else
#include <time.h>
#include <unistd.h>
#define NOMO_FIXTURE_GETCWD getcwd
static void nomo_fixture_sleep(unsigned long millis) {
    struct timespec duration;
    duration.tv_sec = (time_t)(millis / 1000UL);
    duration.tv_nsec = (long)(millis % 1000UL) * 1000000L;
    while (nanosleep(&duration, &duration) != 0) {}
}
#endif

int main(int argc, char **argv) {
    const char *mode = argc > 1 ? argv[1] : "echo";
    if (strcmp(mode, "stall") == 0) {
        nomo_fixture_sleep(2000UL);
        return 0;
    }
    if (strcmp(mode, "hold") == 0) {
        nomo_fixture_sleep(10000UL);
        return 0;
    }
    if (strcmp(mode, "pressure") == 0) {
        char out[4096];
        char err[4096];
        memset(out, 'O', sizeof(out));
        memset(err, 'E', sizeof(err));
        for (int index = 0; index < 64; index += 1) {
            fwrite(out, 1, sizeof(out), stdout);
            fflush(stdout);
            fwrite(err, 1, sizeof(err), stderr);
            fflush(stderr);
        }
        fputs("stdout-order:0|1|2", stdout);
        fflush(stdout);
        fputs("stderr-order:0|1|2", stderr);
        fflush(stderr);
        return 0;
    }
    if (strcmp(mode, "split-utf8") == 0) {
        const unsigned char text[] = {0xf0, 0x9f, 0x98, 0x80, '\n'};
        fwrite(text, 1, 2, stdout);
        fflush(stdout);
        nomo_fixture_sleep(50UL);
        fwrite(text + 2, 1, 3, stdout);
        fflush(stdout);
        return 0;
    }
    if (strcmp(mode, "invalid-utf8") == 0) {
        fputc(0xff, stdout);
        fflush(stdout);
        nomo_fixture_sleep(2000UL);
        return 0;
    }
    if (strcmp(mode, "secret-output") == 0) {
        fputs("stdout-secret-token", stdout);
        fputc(0xff, stdout);
        fflush(stdout);
        fputs("stderr-secret-token", stderr);
        fputc(0xff, stderr);
        fflush(stderr);
        nomo_fixture_sleep(2000UL);
        return 0;
    }
    if (strcmp(mode, "environment") == 0) {
        const char *visible = getenv("VISIBLE");
        const char *inherited = getenv("NOMO_INHERITED_MARKER");
        printf("env:%s\n", visible == NULL ? "" : visible);
        printf("inherited:%s\n", inherited == NULL ? "" : inherited);
        return 0;
    }
    if (strcmp(mode, "mcp") == 0) {
        char line[65536];
        fputs("mcp-fixture-ready\n", stderr);
        fflush(stderr);
        if (fgets(line, sizeof(line), stdin) == NULL) {
            return 2;
        }
        fputs("{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"protocolVersion\":\"2025-06-18\",", stdout);
        fflush(stdout);
        nomo_fixture_sleep(50UL);
        fputs("\"capabilities\":{},\"serverInfo\":{\"name\":\"fixture\",\"version\":\"1\"}}}\n", stdout);
        fflush(stdout);
        if (fgets(line, sizeof(line), stdin) == NULL) {
            return 3;
        }
        fputs("{\"jsonrpc\":\"2.0\",\"method\":\"notifications/progress\",\"params\":{\"step\":1}}\n", stdout);
        fputs("{\"jsonrpc\":\"2.0\",\"id\":2,\"result\":{\"tools\":[]}}\n", stdout);
        fputs("mcp-fixture-complete\n", stderr);
        fflush(stdout);
        fflush(stderr);
        return 0;
    }

    char cwd[4096];
    const char *visible = getenv("VISIBLE");
    printf("argv:%s|%s\n", argc > 2 ? argv[2] : "", argc > 3 ? argv[3] : "");
    if (NOMO_FIXTURE_GETCWD(cwd, sizeof(cwd)) != NULL) {
        printf("cwd:%s\n", cwd);
    }
    printf("env:%s\n", visible == NULL ? "missing" : visible);
    fprintf(stderr, "fixture-stderr-ready\n");
    fflush(stdout);
    fflush(stderr);
    char line[4096];
    while (fgets(line, sizeof(line), stdin) != NULL) {
        printf("out:%s", line);
        fprintf(stderr, "err:%s", line);
        fflush(stdout);
        fflush(stderr);
    }
    return 7;
}
"#,
    )
    .unwrap();
    let compiler = if cfg!(windows) { "clang" } else { "cc" };
    let compiled = Command::new(compiler)
        .arg("-std=c99")
        .arg(&fixture_source)
        .arg("-o")
        .arg(&fixture_binary)
        .output()
        .unwrap();
    assert!(
        compiled.status.success(),
        "fixture stdout:\n{}\nfixture stderr:\n{}",
        String::from_utf8_lossy(&compiled.stdout),
        String::from_utf8_lossy(&compiled.stderr)
    );
    fs::canonicalize(fixture_binary).unwrap()
}

#[test]
fn mcp_stdio_example_completes_two_jsonrpc_exchanges() {
    let root = temp_test_root("mcp-stdio-example");
    reset_dir(&root);
    let fixture_binary = build_controlled_process_fixture(&root);
    let example = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/mcp_stdio");

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(example)
        .env("NOMO_MCP_FIXTURE", fixture_binary)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).replace('\r', ""),
        "response 1 success\nserver notification\nresponse 2 success\nMCP stdio exchange complete\n"
    );
    let stderr = String::from_utf8_lossy(&output.stderr).replace('\r', "");
    assert!(stderr.contains("mcp-fixture-ready\n"), "{stderr}");
    assert!(stderr.contains("mcp-fixture-complete\n"), "{stderr}");
    assert!(!stderr.contains("NOMO_JSONRPC_SECRET_SENTINEL"));

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn nomo_run_executes_controlled_process_stdio_without_a_shell() {
    let root = temp_test_root("controlled-process-stdio");
    reset_dir(&root);
    let fixture_binary = build_controlled_process_fixture(&root);

    let child_cwd = root.join("child cwd");
    fs::create_dir_all(&child_cwd).unwrap();
    let child_cwd = fs::canonicalize(child_cwd).unwrap();
    let project = root.join("controlled_process");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nname = \"controlled_process\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(
        project.join("src/main.nomo"),
        r#"package controlled_process.main

import std.array.Array
import std.env
import std.io
import std.process

fn print_event(event: ProcessEvent) -> bool {
    let mut done: bool = false
    match event {
        ProcessEvent.StdinFlushed => {
        }
        ProcessEvent.Stdout(text) => {
            io.print(text)
        }
        ProcessEvent.Stderr(text) => {
            io.print(text)
        }
        ProcessEvent.Exited(status) => {
            io.println("exit", status.code, status.signal)
            done = true
        }
    }
    return done
}

fn write_message(child: ProcessChild, message: string) -> Result<void, ProcessControlError> {
    process.write_stdin(child, message)?
    let mut flushed: bool = false
    for !flushed {
        let event: ProcessEvent = process.next_event(child, 4096, 5000)?
        match event {
            ProcessEvent.StdinFlushed => {
                flushed = true
            }
            ProcessEvent.Stdout(text) => {
                io.print(text)
            }
            ProcessEvent.Stderr(text) => {
                io.print(text)
            }
            ProcessEvent.Exited(status) => {
                io.println("early-exit", status.code, status.signal)
                return Ok(void)
            }
        }
    }
    return Ok(void)
}

fn run_inherited_environment(program: string) -> Result<void, ProcessControlError> {
    let mut args: Array<string> = Array.new<string>()
    args.push("environment")
    let mut environment: Array<ProcessEnv> = Array.new<ProcessEnv>()
    environment.push(ProcessEnv { name: "VISIBLE", value: "overridden-value" })
    let command: ProcessCommand = ProcessCommand { program: program, args: args, cwd: None, env: environment, inherit_env: true }
    let child: ProcessChild = process.start(command)?
    defer process.close_child(child)
    process.close_stdin(child)?
    process.close_stdin(child)?
    let mut done: bool = false
    for !done {
        done = print_event(process.next_event(child, 4096, 15000)?)
    }
    return Ok(void)
}

fn run(program: string, cwd: string) -> Result<void, ProcessControlError> {
    let mut args: Array<string> = Array.new<string>()
    args.push("echo")
    args.push("space value")
    args.push("quote\"value")
    let mut environment: Array<ProcessEnv> = Array.new<ProcessEnv>()
    environment.push(ProcessEnv { name: "VISIBLE", value: "child-value" })
    let command: ProcessCommand = ProcessCommand { program: program, args: args, cwd: Some(cwd), env: environment, inherit_env: false }
    let child: ProcessChild = process.start(command)?
    defer process.close_child(child)
    let initial: Option<ProcessExit> = process.try_wait(child)?
    match initial {
        Some(status) => {
            io.println("unexpected-exit", status.code)
        }
        None => {
            io.println("running")
        }
    }
    write_message(child, "first message\n")?
    write_message(child, "second message\n")?
    process.close_stdin(child)?
    process.close_stdin(child)?
    let mut done: bool = false
    for !done {
        done = print_event(process.next_event(child, 4096, 15000)?)
    }
    let final_status: Option<ProcessExit> = process.try_wait(child)?
    match final_status {
        Some(status) => {
            io.println("wait", status.code, status.signal)
        }
        None => {
            io.println("wait missing")
        }
    }
    process.terminate(child)?
    process.terminate(child)?
    let after_exit: Result<ProcessEvent, ProcessControlError> = process.next_event(child, 4096, 100)
    match after_exit {
        Ok(event) => {
            io.println("after-exit unexpected")
        }
        Err(error) => {
            io.println("after-exit", error.code)
        }
    }
    let copied: ProcessChild = child
    process.close_child(child)
    process.close_child(copied)
    run_inherited_environment(program)?
    return Ok(void)
}

fn main() -> void {
    let program: Option<string> = env.get("NOMO_PROCESS_FIXTURE")
    let cwd: Option<string> = env.get("NOMO_PROCESS_CWD")
    match program {
        Some(program_value) => {
            match cwd {
                Some(cwd_value) => {
                    let result: Result<void, ProcessControlError> = run(program_value, cwd_value)
                    match result {
                        Ok(done) => {
                        }
                        Err(error) => {
                            io.println("error", error.code, error.message)
                        }
                    }
                }
                None => {
                    io.println("missing cwd")
                }
            }
        }
        None => {
            io.println("missing fixture")
        }
    }
}
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(&project)
        .env("NOMO_PROCESS_FIXTURE", &fixture_binary)
        .env("NOMO_PROCESS_CWD", &child_cwd)
        .env("NOMO_INHERITED_MARKER", "parent-value")
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout).replace('\r', "");
    for expected in [
        "running\n",
        "argv:space value|quote\"value\n",
        &format!("cwd:{}\n", child_cwd.display()),
        "env:child-value\n",
        "fixture-stderr-ready\n",
        "out:first message\n",
        "err:first message\n",
        "out:second message\n",
        "err:second message\n",
        "exit 7 0\n",
        "wait 7 0\n",
        "after-exit invalid_request\n",
        "env:overridden-value\n",
        "inherited:parent-value\n",
        "exit 0 0\n",
    ] {
        assert!(
            stdout.contains(expected),
            "missing {expected:?} in:\n{stdout}"
        );
    }
    assert!(!stdout.contains("error "), "{stdout}");
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn controlled_processes_enforce_limits_timeouts_and_protocol_safety() {
    let root = temp_test_root("controlled-process-safety");
    reset_dir(&root);
    let fixture_binary = build_controlled_process_fixture(&root);
    let missing_binary = root.join("missing-program-secret-token");
    let project = root.join("controlled_process_safety");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nname = \"controlled_process_safety\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(
        project.join("src/main.nomo"),
        r#"package controlled_process_safety.main

import std.array.Array
import std.env
import std.io
import std.process
import std.string

fn command(program: string, mode: string) -> ProcessCommand {
    let mut args: Array<string> = Array.new<string>()
    args.push(mode)
    let environment: Array<ProcessEnv> = Array.new<ProcessEnv>()
    return ProcessCommand { program: program, args: args, cwd: None, env: environment, inherit_env: true }
}

fn report_void(label: string, result: Result<void, ProcessControlError>) -> void {
    match result {
        Ok(done) => {
            io.println(label, "ok")
        }
        Err(error) => {
            io.println(label, error.code)
        }
    }
}

fn run_timeout(program: string) -> Result<void, ProcessControlError> {
    let child: ProcessChild = process.start(command(program, "stall"))?
    defer process.close_child(child)
    let mut payload: string = "x"
    for let index: u64 = 0; index < 20; index++ {
        payload = string.concat(payload, payload)
    }
    process.write_stdin(child, payload)?
    let timed: Result<ProcessEvent, ProcessControlError> = process.next_event(child, 4096, 50)
    match timed {
        Ok(event) => {
            io.println("timeout unexpected")
        }
        Err(error) => {
            io.println("timeout", error.code)
        }
    }
    let observed: Option<ProcessExit> = process.try_wait(child)?
    match observed {
        Some(status) => {
            io.println("timeout exited", status.code)
        }
        None => {
            io.println("timeout usable")
        }
    }
    report_void("pending-close", process.close_stdin(child))
    report_void("pending-busy", process.write_stdin(child, "again"))
    report_void("too-large", process.write_stdin(child, string.concat(payload, "x")))
    process.terminate(child)?
    process.terminate(child)?
    process.close_child(child)
    process.close_child(child)
    io.println("terminate idempotent")
    return Ok(void)
}

fn run_pressure(program: string) -> Result<void, ProcessControlError> {
    let child: ProcessChild = process.start(command(program, "pressure"))?
    defer process.close_child(child)
    process.close_stdin(child)?
    let mut stdout_bytes: u64 = 0
    let mut stderr_bytes: u64 = 0
    let mut stdout_text: string = ""
    let mut stderr_text: string = ""
    let mut done: bool = false
    for !done {
        let event: ProcessEvent = process.next_event(child, 4096, 5000)?
        match event {
            ProcessEvent.StdinFlushed => {
            }
            ProcessEvent.Stdout(text) => {
                stdout_bytes += string.len(text)
                stdout_text = string.concat(stdout_text, text)
            }
            ProcessEvent.Stderr(text) => {
                stderr_bytes += string.len(text)
                stderr_text = string.concat(stderr_text, text)
            }
            ProcessEvent.Exited(status) => {
                io.println(
                    "pressure",
                    stdout_bytes,
                    stderr_bytes,
                    string.ends_with(stdout_text, "stdout-order:0|1|2"),
                    string.ends_with(stderr_text, "stderr-order:0|1|2"),
                    status.code
                )
                done = true
            }
        }
    }
    return Ok(void)
}

fn run_split_utf8(program: string) -> Result<void, ProcessControlError> {
    let child: ProcessChild = process.start(command(program, "split-utf8"))?
    defer process.close_child(child)
    process.close_stdin(child)?
    let mut bytes: u64 = 0
    let mut saw_scalar: bool = false
    let mut done: bool = false
    for !done {
        let event: ProcessEvent = process.next_event(child, 4, 5000)?
        match event {
            ProcessEvent.StdinFlushed => {
            }
            ProcessEvent.Stdout(text) => {
                bytes += string.len(text)
                saw_scalar = saw_scalar || text == "😀"
            }
            ProcessEvent.Stderr(text) => {
                io.print(text)
            }
            ProcessEvent.Exited(status) => {
                io.println("utf8", bytes, saw_scalar, status.code)
                done = true
            }
        }
    }
    return Ok(void)
}

fn run_invalid_utf8(program: string) -> void {
    let started: Result<ProcessChild, ProcessControlError> = process.start(command(program, "invalid-utf8"))
    match started {
        Ok(child) => {
            let event: Result<ProcessEvent, ProcessControlError> = process.next_event(child, 4096, 5000)
            match event {
                Ok(value) => {
                    io.println("protocol unexpected")
                }
                Err(error) => {
                    io.println("protocol", error.code)
                }
            }
            let stale: Result<Option<ProcessExit>, ProcessControlError> = process.try_wait(child)
            match stale {
                Ok(value) => {
                    io.println("stale unexpected")
                }
                Err(error) => {
                    io.println("stale", error.code)
                }
            }
            process.close_child(child)
            process.close_child(child)
        }
        Err(error) => {
            io.println("protocol start", error.code)
        }
    }
}

fn run_secret_output(program: string) -> void {
    let started: Result<ProcessChild, ProcessControlError> = process.start(command(program, "secret-output"))
    match started {
        Ok(child) => {
            let closed: Result<void, ProcessControlError> = process.close_stdin(child)
            match closed {
                Ok(done) => {
                }
                Err(error) => {
                    io.println("secret-output close", error.code, error.message)
                }
            }
            let mut done: bool = false
            for !done {
                let next: Result<ProcessEvent, ProcessControlError> = process.next_event(child, 4096, 5000)
                match next {
                    Ok(event) => {
                        match event {
                            ProcessEvent.StdinFlushed => {
                            }
                            ProcessEvent.Stdout(text) => {
                            }
                            ProcessEvent.Stderr(text) => {
                            }
                            ProcessEvent.Exited(status) => {
                                io.println("secret-output unexpected")
                                done = true
                            }
                        }
                    }
                    Err(error) => {
                        io.println("secret-output", error.code, error.message)
                        done = true
                    }
                }
            }
            process.close_child(child)
            process.close_child(child)
        }
        Err(error) => {
            io.println("secret-output start", error.code, error.message)
        }
    }
}

fn run_bounds(program: string) -> Result<void, ProcessControlError> {
    let child: ProcessChild = process.start(command(program, "hold"))?
    defer process.close_child(child)
    let small: Result<ProcessEvent, ProcessControlError> = process.next_event(child, 3, 100)
    match small {
        Ok(event) => {
            io.println("small unexpected")
        }
        Err(error) => {
            io.println("small", error.code)
        }
    }
    let zero: Result<ProcessEvent, ProcessControlError> = process.next_event(child, 4096, 0)
    match zero {
        Ok(event) => {
            io.println("zero unexpected")
        }
        Err(error) => {
            io.println("zero", error.code)
        }
    }
    let maximum: Result<ProcessEvent, ProcessControlError> = process.next_event(child, 1048576, 10)
    match maximum {
        Ok(event) => {
            io.println("maximum unexpected")
        }
        Err(error) => {
            io.println("maximum", error.code)
        }
    }
    let above: Result<ProcessEvent, ProcessControlError> = process.next_event(child, 1048577, 10)
    match above {
        Ok(event) => {
            io.println("above unexpected")
        }
        Err(error) => {
            io.println("above", error.code)
        }
    }
    process.terminate(child)?
    return Ok(void)
}

fn run_close_reaps(program: string) -> void {
    let started: Result<ProcessChild, ProcessControlError> = process.start(command(program, "hold"))
    match started {
        Ok(child) => {
            let copied: ProcessChild = child
            process.close_child(child)
            process.close_child(copied)
            let stale: Result<Option<ProcessExit>, ProcessControlError> = process.try_wait(copied)
            match stale {
                Ok(value) => {
                    io.println("close-reaped unexpected")
                }
                Err(error) => {
                    io.println("close-reaped", error.code)
                }
            }
        }
        Err(error) => {
            io.println("close-reaped start", error.code)
        }
    }
}

fn run_command_secret_errors(program: string) -> void {
    let mut args: Array<string> = Array.new<string>()
    args.push("argv-secret-token")
    let mut invalid_environment: Array<ProcessEnv> = Array.new<ProcessEnv>()
    invalid_environment.push(ProcessEnv { name: "BAD=env-name-secret-token", value: "env-value-secret-token" })
    let invalid: ProcessCommand = ProcessCommand {
        program: program,
        args: args,
        cwd: Some("cwd-secret-token"),
        env: invalid_environment,
        inherit_env: true
    }
    let invalid_started: Result<ProcessChild, ProcessControlError> = process.start(invalid)
    match invalid_started {
        Ok(child) => {
            io.println("invalid-name unexpected")
            process.close_child(child)
        }
        Err(error) => {
            io.println("invalid-name", error.code, error.message)
        }
    }

    let duplicate_args: Array<string> = Array.new<string>()
    let mut duplicate_environment: Array<ProcessEnv> = Array.new<ProcessEnv>()
    duplicate_environment.push(ProcessEnv { name: "DUPLICATE_SECRET", value: "first-secret-token" })
    duplicate_environment.push(ProcessEnv { name: "DUPLICATE_SECRET", value: "second-secret-token" })
    let duplicate: ProcessCommand = ProcessCommand {
        program: program,
        args: duplicate_args,
        cwd: None,
        env: duplicate_environment,
        inherit_env: true
    }
    let duplicate_started: Result<ProcessChild, ProcessControlError> = process.start(duplicate)
    match duplicate_started {
        Ok(child) => {
            io.println("duplicate unexpected")
            process.close_child(child)
        }
        Err(error) => {
            io.println("duplicate", error.code, error.message)
        }
    }
}

fn run_stdin_secret_error(program: string) -> Result<void, ProcessControlError> {
    let child: ProcessChild = process.start(command(program, "hold"))?
    defer process.close_child(child)
    process.close_stdin(child)?
    let rejected: Result<void, ProcessControlError> = process.write_stdin(child, "stdin-secret-token")
    match rejected {
        Ok(done) => {
            io.println("stdin-secret unexpected")
        }
        Err(error) => {
            io.println("stdin-secret", error.code, error.message)
        }
    }
    return Ok(void)
}

fn run_missing(program: string) -> void {
    let started: Result<ProcessChild, ProcessControlError> = process.start(command(program, "echo"))
    match started {
        Ok(child) => {
            io.println("missing unexpected")
            process.close_child(child)
        }
        Err(error) => {
            io.println("missing", error.code, error.message)
        }
    }
}

fn main() -> void {
    let fixture: Option<string> = env.get("NOMO_PROCESS_FIXTURE")
    let missing: Option<string> = env.get("NOMO_MISSING_PROCESS")
    match fixture {
        Some(program) => {
            report_void("timeout-run", run_timeout(program))
            report_void("pressure-run", run_pressure(program))
            report_void("utf8-run", run_split_utf8(program))
            run_invalid_utf8(program)
            run_secret_output(program)
            report_void("bounds-run", run_bounds(program))
            run_close_reaps(program)
            run_command_secret_errors(program)
            report_void("stdin-secret-run", run_stdin_secret_error(program))
        }
        None => {
            io.println("missing fixture")
        }
    }
    match missing {
        Some(program) => {
            run_missing(program)
        }
        None => {
            io.println("missing missing-program")
        }
    }
}
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(&project)
        .env("NOMO_PROCESS_FIXTURE", &fixture_binary)
        .env("NOMO_MISSING_PROCESS", &missing_binary)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout).replace('\r', "");
    let utf8_summary = if cfg!(windows) {
        "utf8 6 true 0\n"
    } else {
        "utf8 5 true 0\n"
    };
    for expected in [
        "timeout timeout\n",
        "timeout usable\n",
        "pending-close busy\n",
        "pending-busy busy\n",
        "too-large invalid_request\n",
        "terminate idempotent\n",
        "timeout-run ok\n",
        "pressure 262162 262162 true true 0\n",
        "pressure-run ok\n",
        utf8_summary,
        "utf8-run ok\n",
        "protocol protocol\n",
        "stale invalid_request\n",
        "secret-output protocol process output is not valid supported text\n",
        "small invalid_request\n",
        "zero invalid_request\n",
        "maximum timeout\n",
        "above invalid_request\n",
        "bounds-run ok\n",
        "close-reaped invalid_request\n",
        "invalid-name invalid_request invalid process command\n",
        "duplicate invalid_request invalid process command\n",
        "stdin-secret invalid_request process stdin is closed\n",
        "stdin-secret-run ok\n",
        "missing spawn failed to start process\n",
    ] {
        assert!(
            stdout.contains(expected),
            "missing {expected:?} in:\n{stdout}"
        );
    }
    assert!(!stdout.contains("unexpected"), "{stdout}");
    for secret in [
        "missing-program-secret-token",
        "argv-secret-token",
        "env-name-secret-token",
        "env-value-secret-token",
        "cwd-secret-token",
        "first-secret-token",
        "second-secret-token",
        "stdin-secret-token",
        "stdout-secret-token",
        "stderr-secret-token",
    ] {
        assert!(!stdout.contains(secret), "leaked {secret:?} in:\n{stdout}");
        assert!(
            !String::from_utf8_lossy(&output.stderr).contains(secret),
            "leaked {secret:?} in stderr:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_run_executes_std_net_tcp_stream_helpers() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut request = [0_u8; 4];
        stream.read_exact(&mut request).unwrap();
        assert_eq!(&request, b"ping");
        stream.write_all(b"pong").unwrap();
    });

    let root = temp_test_root("std-net-tcp-stream-helpers");
    reset_dir(&root);
    let project = root.join("net_tcp_stream_helpers");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nname = \"net_tcp_stream_helpers\"\nversion = \"0.1.0\"\n\n[dependencies]\nstd = \"0.1.0\"\n",
    )
    .unwrap();
    let source = r#"package app.main

import std.io
import std.net

fn request() -> Result<string, NetError> {
    let stream: TcpStream = net.connect("127.0.0.1", __PORT__)?
    stream.write_string("ping")?
    let text: string = stream.read_to_string()?
    stream.close()
    return Ok(text)
}

fn main() -> void {
    let result: Result<string, NetError> = request()
    match result {
        Ok(text) => {
            io.println(text)
        }
        Err(err) => {
            io.println(err.message)
        }
    }
}
"#
    .replace("__PORT__", &port.to_string());
    fs::write(project.join("src/main.nomo"), source).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(&project)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), "pong\n");
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    server.join().unwrap();
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_run_executes_std_net_tcp_listener_helpers_without_std_dependency() {
    let probe = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = probe.local_addr().unwrap().port();
    drop(probe);

    let root = temp_test_root("std-net-tcp-listener-helpers");
    reset_dir(&root);
    let project = root.join("net_tcp_listener_helpers");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nname = \"net_tcp_listener_helpers\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    let source = r#"package app.main

import std.io
import std.net

fn serve() -> Result<void, NetError> {
    let listener: TcpListener = net.listen("127.0.0.1", __PORT__)?
    let stream: TcpStream = listener.accept()?
    let text: string = stream.read_to_string()?
    stream.write_string("pong:")?
    stream.write_string(text)?
    stream.close()
    listener.close()
    return Ok(void)
}

fn main() -> void {
    let result: Result<void, NetError> = serve()
    match result {
        Ok(value) => {
        }
        Err(err) => {
            io.println(err.message)
        }
    }
}
"#
    .replace("__PORT__", &port.to_string());
    fs::write(project.join("src/main.nomo"), source).unwrap();

    let mut child = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(&project)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    let started = Instant::now();
    let mut stream = loop {
        match TcpStream::connect(("127.0.0.1", port)) {
            Ok(stream) => break stream,
            Err(err) if started.elapsed() < Duration::from_secs(10) => {
                if let Some(status) = child.try_wait().unwrap() {
                    let output = child.wait_with_output().unwrap();
                    panic!(
                        "nomo server exited early with {status}\nstdout:\n{}\nstderr:\n{}",
                        String::from_utf8_lossy(&output.stdout),
                        String::from_utf8_lossy(&output.stderr)
                    );
                }
                std::thread::sleep(Duration::from_millis(50));
                let _ = err;
            }
            Err(err) => {
                let _ = child.kill();
                let output = child.wait_with_output().unwrap();
                panic!(
                    "failed to connect to nomo listener: {err}\nstdout:\n{}\nstderr:\n{}",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                );
            }
        }
    };

    stream.write_all(b"ping").unwrap();
    stream.shutdown(Shutdown::Write).unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    assert_eq!(response, "pong:ping");

    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.is_empty());
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_run_executes_std_net_udp_socket_helpers_without_std_dependency() {
    let probe = RustUdpSocket::bind("127.0.0.1:0").unwrap();
    let port = probe.local_addr().unwrap().port();
    drop(probe);

    let root = temp_test_root("std-net-udp-socket-helpers");
    reset_dir(&root);
    let project = root.join("net_udp_socket_helpers");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nname = \"net_udp_socket_helpers\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    let source = r#"package app.main

import std.io
import std.net

fn serve() -> Result<void, NetError> {
    let socket: UdpSocket = net.udp_bind("127.0.0.1", __PORT__)?
    let packet: UdpDatagram = socket.recv_from_string(1024)?
    socket.send_to_string("pong", packet.host, packet.port)?
    socket.close()
    return Ok(void)
}

fn main() -> void {
    let result: Result<void, NetError> = serve()
    match result {
        Ok(value) => {
        }
        Err(err) => {
            io.println(err.message)
        }
    }
}
"#
    .replace("__PORT__", &port.to_string());
    fs::write(project.join("src/main.nomo"), source).unwrap();

    let mut child = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(&project)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    let client = RustUdpSocket::bind("127.0.0.1:0").unwrap();
    client
        .set_read_timeout(Some(Duration::from_millis(200)))
        .unwrap();
    let started = Instant::now();
    let mut response = [0_u8; 32];
    loop {
        client.send_to(b"ping", ("127.0.0.1", port)).unwrap();
        match client.recv_from(&mut response) {
            Ok((len, _)) => {
                assert_eq!(&response[..len], b"pong");
                break;
            }
            Err(err)
                if matches!(err.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut)
                    && started.elapsed() < Duration::from_secs(10) =>
            {
                if let Some(status) = child.try_wait().unwrap() {
                    let output = child.wait_with_output().unwrap();
                    panic!(
                        "nomo udp server exited early with {status}\nstdout:\n{}\nstderr:\n{}",
                        String::from_utf8_lossy(&output.stdout),
                        String::from_utf8_lossy(&output.stderr)
                    );
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(err) => {
                let _ = child.kill();
                let output = child.wait_with_output().unwrap();
                panic!(
                    "failed to receive UDP response: {err}\nstdout:\n{}\nstderr:\n{}",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                );
            }
        }
    }

    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.is_empty());
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_run_executes_std_http_client_helpers_without_std_dependency() {
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
                    let text = String::from_utf8(request).unwrap();
                    let body_start = text.find("\r\n\r\n").map(|index| index + 4).unwrap();
                    let body = &text[body_start..];
                    let (expected_line, expected_body, response_status, response_body) =
                        if handled == 0 {
                            ("GET /hello HTTP/1.1", "", "200 OK", "get-ok")
                        } else {
                            (
                                "POST /echo HTTP/1.1",
                                "post-body",
                                "429 Too Many Requests",
                                "post-ok",
                            )
                        };
                    assert!(text.starts_with(expected_line), "request was:\n{text}");
                    assert_eq!(body, expected_body);
                    let response = format!(
                        "HTTP/1.0 {response_status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        response_body.len(),
                        response_body
                    );
                    stream.write_all(response.as_bytes()).unwrap();
                    handled += 1;
                }
                Err(err)
                    if err.kind() == ErrorKind::WouldBlock
                        // `nomo run` compiles generated C before opening the
                        // connection; that startup can exceed ten seconds on
                        // the Windows CI runner.
                        && started.elapsed() < Duration::from_secs(60) =>
                {
                    std::thread::sleep(Duration::from_millis(25));
                }
                Err(err) => panic!("failed to accept HTTP client connection: {err}"),
            }
        }
    });

    let root = temp_test_root("std-http-client-helpers");
    reset_dir(&root);
    let project = root.join("http_client_helpers");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nname = \"http_client_helpers\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    let source = r#"package app.main

import std.http
import std.io

fn request() -> Result<void, HttpError> {
    let first: HttpResponse = http.get("http://127.0.0.1:__PORT__/hello")?
    io.println(first.body)
    let second: HttpResponse = http.post("http://127.0.0.1:__PORT__/echo", "post-body")?
    io.println(second.body)
    return Ok(void)
}

fn main() -> void {
    let result: Result<void, HttpError> = request()
    match result {
        Ok(value) => {
        }
        Err(err) => {
            io.println(err.message)
        }
    }
}
"#
    .replace("__PORT__", &port.to_string());
    fs::write(project.join("src/main.nomo"), source).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(&project)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout).replace("\r\n", "\n");
    assert_eq!(stdout, "get-ok\npost-ok\n");
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    server.join().unwrap();

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn structured_http_requests_enforce_limits_and_redact_secrets() {
    let body_listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let body_port = body_listener.local_addr().unwrap().port();
    let body_server = std::thread::spawn(move || {
        let (mut stream, _) = body_listener.accept().unwrap();
        let mut request = [0_u8; 1024];
        let _ = stream.read(&mut request).unwrap();
        stream
            .write_all(
                b"HTTP/1.1 200 OK\r\nContent-Length: 32\r\nConnection: close\r\n\r\n0123456789abcdefghijklmnopqrstuv",
            )
            .unwrap();
    });

    let timeout_listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let timeout_port = timeout_listener.local_addr().unwrap().port();
    let timeout_server = std::thread::spawn(move || {
        let (_stream, _) = timeout_listener.accept().unwrap();
        std::thread::sleep(Duration::from_millis(300));
    });

    let _ = rustls::crypto::ring::default_provider().install_default();
    let CertifiedKey { cert, signing_key } =
        generate_simple_self_signed(vec!["localhost".to_string()]).unwrap();
    let tls_config = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(
            vec![cert.der().clone()],
            PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(signing_key.serialize_der())),
        )
        .unwrap();
    let tls_listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let tls_port = tls_listener.local_addr().unwrap().port();
    let tls_server = std::thread::spawn(move || {
        let (stream, _) = tls_listener.accept().unwrap();
        let connection = ServerConnection::new(Arc::new(tls_config)).unwrap();
        let mut stream = StreamOwned::new(connection, stream);
        let mut request = [0_u8; 1024];
        let _ = stream.read(&mut request);
    });

    let root = temp_test_root("structured-http-limits");
    reset_dir(&root);
    let project = root.join("structured_http_limits");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nname = \"structured_http_limits\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    let source = r#"package structured_http_limits.main

import std.array.Array
import std.http
import std.io

fn invalid_header() -> void {
    let mut headers: Array<HttpHeader> = Array.new<HttpHeader>()
    headers.push(HttpHeader {
        name: "Authorization",
        value: "Bearer header-secret\r\nInjected: bad"
    })
    let request: HttpRequest = HttpRequest {
        method: "POST",
        url: "http://127.0.0.1:1/invalid",
        headers: headers,
        body: "body-secret",
        timeout_millis: 1000,
        max_response_bytes: 1024
    }
    let result: Result<HttpResponse, HttpError> = http.send(request)
    match result {
        Ok(response) => {
            io.println("invalid unexpected-ok")
        }
        Err(err) => {
            io.println("invalid", err.code, err.message)
        }
    }
}

fn body_limit() -> void {
    let headers: Array<HttpHeader> = Array.new<HttpHeader>()
    let request: HttpRequest = HttpRequest {
        method: "GET",
        url: "http://127.0.0.1:__BODY_PORT__/large",
        headers: headers,
        body: "",
        timeout_millis: 1000,
        max_response_bytes: 8
    }
    let result: Result<HttpResponse, HttpError> = http.send(request)
    match result {
        Ok(response) => {
            io.println("body unexpected-ok")
        }
        Err(err) => {
            io.println("body", err.code, err.message)
        }
    }
}

fn request_timeout() -> void {
    let headers: Array<HttpHeader> = Array.new<HttpHeader>()
    let request: HttpRequest = HttpRequest {
        method: "GET",
        url: "http://127.0.0.1:__TIMEOUT_PORT__/slow",
        headers: headers,
        body: "",
        timeout_millis: 100,
        max_response_bytes: 1024
    }
    let result: Result<HttpResponse, HttpError> = http.send(request)
    match result {
        Ok(response) => {
            io.println("timeout unexpected-ok")
        }
        Err(err) => {
            io.println("timeout", err.code, err.message)
        }
    }
}

fn tls_failure() -> void {
    let mut headers: Array<HttpHeader> = Array.new<HttpHeader>()
    headers.push(HttpHeader {
        name: "Authorization",
        value: "Bearer tls-header-secret"
    })
    let request: HttpRequest = HttpRequest {
        method: "POST",
        url: "https://127.0.0.1:__TLS_PORT__/failure?api_key=query-secret",
        headers: headers,
        body: "tls-body-secret",
        timeout_millis: 1000,
        max_response_bytes: 1024
    }
    let result: Result<HttpResponse, HttpError> = http.send(request)
    match result {
        Ok(response) => {
            io.println("tls unexpected-ok")
        }
        Err(err) => {
            io.println("tls", err.code, err.message)
        }
    }
}

fn main() -> void {
    invalid_header()
    body_limit()
    request_timeout()
    tls_failure()
}
"#
    .replace("__BODY_PORT__", &body_port.to_string())
    .replace("__TIMEOUT_PORT__", &timeout_port.to_string())
    .replace("__TLS_PORT__", &tls_port.to_string());
    fs::write(project.join("src/main.nomo"), source).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(&project)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout).replace("\r\n", "\n");
    assert!(
        stdout.contains("invalid invalid_request invalid or reserved HTTP header\n"),
        "{stdout}"
    );
    assert!(
        stdout.contains("body response_too_large HTTP response exceeded its configured limit\n"),
        "{stdout}"
    );
    assert!(
        stdout.contains("timeout timeout HTTP request timed out\n"),
        "{stdout}"
    );
    assert!(
        stdout.contains("tls tls HTTPS certificate or handshake failed\n"),
        "{stdout}"
    );
    for secret in [
        "header-secret",
        "body-secret",
        "tls-header-secret",
        "tls-body-secret",
        "query-secret",
    ] {
        assert!(
            !stdout.contains(secret),
            "secret leaked in stdout: {stdout}"
        );
        assert!(
            !String::from_utf8_lossy(&output.stderr).contains(secret),
            "secret leaked in stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    body_server.join().unwrap();
    timeout_server.join().unwrap();
    tls_server.join().unwrap();
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn structured_http_stream_parses_incremental_sse_events() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut request = [0_u8; 2048];
        let _ = stream.read(&mut request).unwrap();
        stream
            .write_all(
                b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n",
            )
            .unwrap();
        stream.flush().unwrap();

        let first_scalar = "你".as_bytes();
        let chunks: [&[u8]; 8] = [
            b"\xef\xbb",
            b"\xbf: comment\r\n",
            b"event: token\r\nid: 7\r\nretry: 1500\r\ndata: ",
            &first_scalar[..1],
            &first_scalar[1..],
            "好\r\ndata: world\n\r\n".as_bytes(),
            b"data: [DONE]\r\n\r\n",
            b"",
        ];
        for chunk in chunks {
            stream.write_all(chunk).unwrap();
            stream.flush().unwrap();
            std::thread::sleep(Duration::from_millis(10));
        }
    });

    let root = temp_test_root("structured-http-stream-sse");
    reset_dir(&root);
    let project = root.join("structured_http_stream_sse");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nname = \"structured_http_stream_sse\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    let source = r#"package structured_http_stream_sse.main

import std.array.Array
import std.http
import std.io

fn print_retry(retry: Option<u64>) -> void {
    match retry {
        Some(value) => {
            io.println(value)
        }
        None => {
            io.println("none")
        }
    }
}

fn run() -> Result<void, HttpError> {
    let headers: Array<HttpHeader> = Array.new<HttpHeader>()
    let request: HttpRequest = HttpRequest {
        method: "GET",
        url: "http://127.0.0.1:__PORT__/events",
        headers: headers,
        body: "",
        timeout_millis: 1000,
        max_response_bytes: 1048576
    }
    let stream: HttpStream = http.open_stream(request, 1000)?
    defer http.close_stream(stream)

    let first: Option<SseEvent> = http.next_sse(stream, 1024)?
    match first {
        Some(event) => {
            io.println(event.event)
            io.println(event.data)
            io.println(event.id)
            print_retry(event.retry_millis)
        }
        None => {
            io.println("unexpected first eof")
        }
    }

    let second: Option<SseEvent> = http.next_sse(stream, 1024)?
    match second {
        Some(event) => {
            io.println(event.event)
            io.println(event.data)
            io.println(event.id)
            print_retry(event.retry_millis)
        }
        None => {
            io.println("unexpected second eof")
        }
    }

    let third: Option<SseEvent> = http.next_sse(stream, 1024)?
    match third {
        Some(event) => {
            io.println("unexpected third event")
        }
        None => {
            io.println("eof")
        }
    }
    return Ok(void)
}

fn main() -> void {
    let result: Result<void, HttpError> = run()
    match result {
        Ok(value) => {
        }
        Err(err) => {
            io.println("error", err.code, err.message)
        }
    }
}
"#
    .replace("__PORT__", &port.to_string());
    fs::write(project.join("src/main.nomo"), source).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(&project)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout).replace("\r\n", "\n");
    assert_eq!(
        stdout,
        "token\n你好\nworld\n7\n1500\nmessage\n[DONE]\n7\nnone\neof\n"
    );
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    server.join().unwrap();
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn structured_http_stream_reads_utf8_text_and_rejects_closed_or_mixed_modes() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut request = [0_u8; 2048];
        let _ = stream.read(&mut request).unwrap();
        let body = "A你好B🙂C";
        stream
            .write_all(
                format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                )
                .as_bytes(),
            )
            .unwrap();
        for chunk in body.as_bytes().chunks(2) {
            stream.write_all(chunk).unwrap();
            stream.flush().unwrap();
            std::thread::sleep(Duration::from_millis(5));
        }
    });

    let root = temp_test_root("structured-http-stream-text");
    reset_dir(&root);
    let project = root.join("structured_http_stream_text");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nname = \"structured_http_stream_text\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    let source = r#"package structured_http_stream_text.main

import std.array.Array
import std.http
import std.io
import std.string

fn finish_text(text: string, chunks: u64) -> Result<void, HttpError> {
    io.println(text)
    io.println(chunks > 1)
    return Ok(void)
}

fn collect_text(stream: HttpStream, text: string, chunks: u64) -> Result<void, HttpError> {
    let chunk: HttpStreamChunk = http.read_text(stream, 4)?
    return if chunk.done {
        finish_text(text, chunks)
    } else {
        collect_text(stream, text.concat(chunk.data), chunks + 1)
    }
}

fn run() -> Result<void, HttpError> {
    let headers: Array<HttpHeader> = Array.new<HttpHeader>()
    let request: HttpRequest = HttpRequest {
        method: "GET",
        url: "http://127.0.0.1:__PORT__/text",
        headers: headers,
        body: "",
        timeout_millis: 1000,
        max_response_bytes: 1024
    }
    let stream: HttpStream = http.open_stream(request, 1000)?
    collect_text(stream, "", 0)?

    let mixed: Result<Option<SseEvent>, HttpError> = http.next_sse(stream, 1024)
    match mixed {
        Ok(event) => {
            io.println("unexpected mixed mode")
        }
        Err(err) => {
            io.println(err.code)
        }
    }

    http.close_stream(stream)
    http.close_stream(stream)
    let closed: Result<HttpStreamChunk, HttpError> = http.read_text(stream, 4)
    match closed {
        Ok(chunk) => {
            io.println("unexpected closed read")
        }
        Err(err) => {
            io.println(err.code)
        }
    }
    return Ok(void)
}

fn main() -> void {
    let result: Result<void, HttpError> = run()
    match result {
        Ok(value) => {
        }
        Err(err) => {
            io.println("error", err.code, err.message)
        }
    }
}
"#
    .replace("__PORT__", &port.to_string());
    fs::write(project.join("src/main.nomo"), source).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(&project)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout).replace("\r\n", "\n");
    assert_eq!(
        stdout,
        "A你好B🙂C\ntrue\ninvalid_request\ninvalid_request\n"
    );
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    server.join().unwrap();
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn structured_http_stream_enforces_limits_timeouts_cancel_and_secret_redaction() {
    let limit_listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let limit_port = limit_listener.local_addr().unwrap().port();
    let limit_server = std::thread::spawn(move || {
        let (mut stream, _) = limit_listener.accept().unwrap();
        let mut request = [0_u8; 2048];
        let _ = stream.read(&mut request).unwrap();
        stream
            .write_all(
                b"HTTP/1.1 200 OK\r\nContent-Length: 16\r\nConnection: close\r\n\r\n0123456789abcdef",
            )
            .unwrap();
    });

    let timeout_listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let timeout_port = timeout_listener.local_addr().unwrap().port();
    let timeout_server = std::thread::spawn(move || {
        let (mut stream, _) = timeout_listener.accept().unwrap();
        let mut request = [0_u8; 2048];
        let _ = stream.read(&mut request).unwrap();
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 1\r\nConnection: close\r\n\r\n")
            .unwrap();
        stream.flush().unwrap();
        std::thread::sleep(Duration::from_millis(300));
    });

    let utf8_listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let utf8_port = utf8_listener.local_addr().unwrap().port();
    let utf8_server = std::thread::spawn(move || {
        let (mut stream, _) = utf8_listener.accept().unwrap();
        let mut request = [0_u8; 2048];
        let _ = stream.read(&mut request).unwrap();
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 1\r\nConnection: close\r\n\r\n\xff")
            .unwrap();
    });

    let cancel_listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let cancel_port = cancel_listener.local_addr().unwrap().port();
    let cancel_server = std::thread::spawn(move || {
        let (mut stream, _) = cancel_listener.accept().unwrap();
        let mut request = Vec::new();
        let mut buffer = [0_u8; 512];
        loop {
            let read = stream.read(&mut buffer).unwrap();
            request.extend_from_slice(&buffer[..read]);
            if read == 0 || request.windows(4).any(|window| window == b"\r\n\r\n") {
                break;
            }
        }
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 1\r\nConnection: close\r\n\r\n")
            .unwrap();
        stream.flush().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let mut closed = [0_u8; 1];
        assert_eq!(stream.read(&mut closed).unwrap(), 0);
    });

    let sse_limit_listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let sse_limit_port = sse_limit_listener.local_addr().unwrap().port();
    let sse_limit_server = std::thread::spawn(move || {
        let (mut stream, _) = sse_limit_listener.accept().unwrap();
        let mut request = [0_u8; 2048];
        let _ = stream.read(&mut request).unwrap();
        stream
            .write_all(
                b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\ndata: received-sse-secret-and-too-large\r\n\r\n",
            )
            .unwrap();
    });

    let root = temp_test_root("structured-http-stream-failures");
    reset_dir(&root);
    let project = root.join("structured_http_stream_failures");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nname = \"structured_http_stream_failures\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    let source = r#"package structured_http_stream_failures.main

import std.array.Array
import std.http
import std.io

fn request_for(url: string, max_response_bytes: u64) -> HttpRequest {
    let mut headers: Array<HttpHeader> = Array.new<HttpHeader>()
    headers.push(HttpHeader {
        name: "Authorization",
        value: "Bearer stream-header-secret"
    })
    return HttpRequest {
        method: "GET",
        url: url,
        headers: headers,
        body: "",
        timeout_millis: 1000,
        max_response_bytes: max_response_bytes
    }
}

fn response_limit() -> Result<void, HttpError> {
    let stream: HttpStream = http.open_stream(
        request_for("http://127.0.0.1:__LIMIT_PORT__/large?api_key=query-secret", 8),
        1000
    )?
    defer http.close_stream(stream)
    let result: Result<HttpStreamChunk, HttpError> = http.read_text(stream, 1024)
    match result {
        Ok(chunk) => {
            io.println("limit unexpected-ok")
        }
        Err(err) => {
            io.println("limit", err.code, err.message)
        }
    }
    return Ok(void)
}

fn idle_timeout() -> Result<void, HttpError> {
    let stream: HttpStream = http.open_stream(
        request_for("http://127.0.0.1:__TIMEOUT_PORT__/slow?token=timeout-secret", 1024),
        100
    )?
    defer http.close_stream(stream)
    let result: Result<HttpStreamChunk, HttpError> = http.read_text(stream, 1024)
    match result {
        Ok(chunk) => {
            io.println("timeout unexpected-ok")
        }
        Err(err) => {
            io.println("timeout", err.code, err.message)
        }
    }
    return Ok(void)
}

fn invalid_utf8() -> Result<void, HttpError> {
    let stream: HttpStream = http.open_stream(
        request_for("http://127.0.0.1:__UTF8_PORT__/invalid", 1024),
        1000
    )?
    defer http.close_stream(stream)
    let result: Result<HttpStreamChunk, HttpError> = http.read_text(stream, 1024)
    match result {
        Ok(chunk) => {
            io.println("utf8 unexpected-ok")
        }
        Err(err) => {
            io.println("utf8", err.code, err.message)
        }
    }
    return Ok(void)
}

fn canceled_stream() -> Result<void, HttpError> {
    let stream: HttpStream = http.open_stream(
        request_for("http://127.0.0.1:__CANCEL_PORT__/cancel", 1024),
        1000
    )?
    let invalid_limit: Result<HttpStreamChunk, HttpError> = http.read_text(stream, 3)
    match invalid_limit {
        Ok(chunk) => {
            io.println("chunk-limit unexpected-ok")
        }
        Err(err) => {
            io.println("chunk-limit", err.code, err.message)
        }
    }
    http.cancel_stream(stream)
    http.cancel_stream(stream)
    let result: Result<HttpStreamChunk, HttpError> = http.read_text(stream, 1024)
    match result {
        Ok(chunk) => {
            io.println("cancel unexpected-ok")
        }
        Err(err) => {
            io.println("cancel", err.code, err.message)
        }
    }
    return Ok(void)
}

fn oversized_sse_event() -> Result<void, HttpError> {
    let stream: HttpStream = http.open_stream(
        request_for("http://127.0.0.1:__SSE_LIMIT_PORT__/events", 1024),
        1000
    )?
    defer http.close_stream(stream)
    let result: Result<Option<SseEvent>, HttpError> = http.next_sse(stream, 8)
    match result {
        Ok(event) => {
            io.println("sse-limit unexpected-ok")
        }
        Err(err) => {
            io.println("sse-limit", err.code, err.message)
        }
    }
    return Ok(void)
}

fn main() -> void {
    let limit_result: Result<void, HttpError> = response_limit()
    let timeout_result: Result<void, HttpError> = idle_timeout()
    let utf8_result: Result<void, HttpError> = invalid_utf8()
    let cancel_result: Result<void, HttpError> = canceled_stream()
    let sse_limit_result: Result<void, HttpError> = oversized_sse_event()
}
"#
    .replace("__LIMIT_PORT__", &limit_port.to_string())
    .replace("__TIMEOUT_PORT__", &timeout_port.to_string())
    .replace("__UTF8_PORT__", &utf8_port.to_string())
    .replace("__CANCEL_PORT__", &cancel_port.to_string())
    .replace("__SSE_LIMIT_PORT__", &sse_limit_port.to_string());
    fs::write(project.join("src/main.nomo"), source).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(&project)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout).replace("\r\n", "\n");
    assert!(
        stdout.contains("limit response_too_large HTTP response exceeded its configured limit\n"),
        "{stdout}"
    );
    assert!(
        stdout.contains("timeout timeout HTTP request timed out\n"),
        "{stdout}"
    );
    assert!(
        stdout.contains("utf8 protocol HTTP response stream was not valid UTF-8 text\n"),
        "{stdout}"
    );
    assert!(
        stdout.contains("chunk-limit invalid_request invalid HTTP stream chunk limit\n"),
        "{stdout}"
    );
    assert!(
        stdout.contains("cancel invalid_request invalid or closed HTTP stream\n"),
        "{stdout}"
    );
    assert!(
        stdout.contains("sse-limit response_too_large SSE event exceeded its configured limit\n"),
        "{stdout}"
    );
    for secret in [
        "stream-header-secret",
        "query-secret",
        "timeout-secret",
        "received-sse-secret",
    ] {
        assert!(
            !stdout.contains(secret),
            "secret leaked in stdout: {stdout}"
        );
        assert!(
            !String::from_utf8_lossy(&output.stderr).contains(secret),
            "secret leaked in stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    limit_server.join().unwrap();
    timeout_server.join().unwrap();
    utf8_server.join().unwrap();
    cancel_server.join().unwrap();
    sse_limit_server.join().unwrap();
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_run_executes_std_http_server_helpers_without_std_dependency() {
    let probe = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = probe.local_addr().unwrap().port();
    drop(probe);

    let root = temp_test_root("std-http-server-helpers");
    reset_dir(&root);
    let project = root.join("http_server_helpers");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nname = \"http_server_helpers\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    let source = r#"package app.main

import std.http
import std.io

fn serve() -> Result<void, HttpError> {
    let server: HttpServer = http.listen("127.0.0.1", __PORT__)?
    defer http.close_server(server)
    let exchange: HttpExchange = http.accept(server)?
    defer http.close_exchange(exchange)
    http.respond_string(exchange, 200, exchange.body)?
    return Ok(void)
}

fn main() -> void {
    let result: Result<void, HttpError> = serve()
    match result {
        Ok(value) => {
        }
        Err(err) => {
            io.println(err.message)
        }
    }
}
"#
    .replace("__PORT__", &port.to_string());
    fs::write(project.join("src/main.nomo"), source).unwrap();

    let mut child = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(&project)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    let started = Instant::now();
    let mut stream = loop {
        match TcpStream::connect(("127.0.0.1", port)) {
            Ok(stream) => break stream,
            Err(err) if started.elapsed() < Duration::from_secs(10) => {
                if let Some(status) = child.try_wait().unwrap() {
                    let output = child.wait_with_output().unwrap();
                    panic!(
                        "nomo http server exited early with {status}\nstdout:\n{}\nstderr:\n{}",
                        String::from_utf8_lossy(&output.stdout),
                        String::from_utf8_lossy(&output.stderr)
                    );
                }
                std::thread::sleep(Duration::from_millis(50));
                let _ = err;
            }
            Err(err) => {
                let _ = child.kill();
                let output = child.wait_with_output().unwrap();
                panic!(
                    "failed to connect to nomo http server: {err}\nstdout:\n{}\nstderr:\n{}",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                );
            }
        }
    };

    let request = "POST /echo HTTP/1.0\r\nHost: 127.0.0.1\r\nContent-Length: 11\r\nConnection: close\r\n\r\nserver-body";
    stream.write_all(request.as_bytes()).unwrap();
    stream.shutdown(Shutdown::Write).unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    assert!(
        response.starts_with("HTTP/1.0 200 OK\r\n"),
        "response was:\n{response}"
    );
    assert!(
        response.ends_with("\r\n\r\nserver-body"),
        "response was:\n{response}"
    );

    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.is_empty());
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_run_executes_extended_std_array_helpers() {
    let root = temp_test_root("std-array-helpers");
    reset_dir(&root);
    let project = root.join("array_demo");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nname = \"array_demo\"\nversion = \"0.1.0\"\n\n[dependencies]\nstd = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(
        project.join("src/main.nomo"),
        r#"package app.main

import std.array
import std.io

fn print_option(value: Option<string>, missing: string) -> void {
    match value {
        Some(text) => {
            io.println(text)
        }
        None => {
            io.println(missing)
        }
    }
}

fn main() -> void {
    let mut items: Array<string> = Array.new<string>()
    items.push("a")
    items.push("c")
    items.insert(1, "b")
    for item in items.iter() {
        io.println(item)
    }
    let removed: Option<string> = items.remove(0)
    let popped: Option<string> = items.pop()
    let first: Option<string> = items.get(0)
    print_option(removed, "missing remove")
    print_option(popped, "missing pop")
    print_option(first, "missing first")
    items.clear()
    if items.len() == 0 {
        io.println("cleared")
    } else {
        io.println("not cleared")
    }
    let empty_pop: Option<string> = items.pop()
    let empty_remove: Option<string> = items.remove(0)
    print_option(empty_pop, "empty pop")
    print_option(empty_remove, "empty remove")
}
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(&project)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "a\nb\nc\na\nc\nb\ncleared\nempty pop\nempty remove\n"
    );
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_run_executes_std_num_helpers_with_question() {
    let root = temp_test_root("std-num-helpers");
    reset_dir(&root);
    let project = root.join("num_helpers");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nname = \"num_helpers\"\nversion = \"0.1.0\"\n\n[dependencies]\nstd = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(
        project.join("src/main.nomo"),
        r#"package app.main

import std.io
import std.num
import std.result

fn main() -> Result<void, NumError> {
    let integer: i64 = num.parse_i64("42")?
    let unsigned: u64 = num.parse_u64("7")?
    let decimal: f64 = num.parse_f64("3.5")?
    io.println(num.to_string(integer))
    io.println(num.to_string(unsigned))
    io.println(num.to_string(decimal))
    let bad: Result<i64, NumError> = num.parse_i64("oops")
    if result.is_err(bad) {
        io.println("bad")
    } else {
        io.println("unexpected")
    }
    return Ok(void)
}
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(&project)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), "42\n7\n3.5\nbad\n");
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_run_executes_std_num_checked_and_wrapping_helpers() {
    let root = temp_test_root("std-num-checked-wrapping");
    reset_dir(&root);
    let project = root.join("num_checked_wrapping");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nname = \"num_checked_wrapping\"\nversion = \"0.1.0\"\n\n[dependencies]\nstd = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(
        project.join("src/main.nomo"),
        r#"package app.main

import std.io
import std.num

fn main() -> void {
    let checked: Option<i64> = num.checked_add(9223372036854775807, 1)
    match checked {
        Option.Some(value) => {
            io.println(num.to_string(value))
        }
        Option.None => {
            io.println("none")
        }
    }
    let wrapped: i64 = num.wrapping_add(9223372036854775807, 1)
    io.println(num.to_string(wrapped))
    let unsigned: u64 = num.wrapping_sub(0 as u64, 1 as u64)
    io.println(num.to_string(unsigned))
}
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(&project)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "none\n-9223372036854775808\n18446744073709551615\n"
    );
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_run_executes_std_io_read_line() {
    let root = temp_test_root("std-io-read-line");
    reset_dir(&root);
    let project = root.join("io_read_line");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nname = \"io_read_line\"\nversion = \"0.1.0\"\n\n[dependencies]\nstd = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(
        project.join("src/main.nomo"),
        r#"package app.main

import std.io
import std.result

fn main() -> Result<void, IoError> {
    let line: string = io.read_line()?
    io.println(line)
    return Ok(void)
}
"#,
    )
    .unwrap();

    let mut child = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(&project)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(b"typed input\n")
        .unwrap();
    let output = child.wait_with_output().unwrap();

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), "typed input\n");
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_run_allows_print_calls_in_void_if_branches() {
    let root = temp_test_root("if-print-branches");
    reset_dir(&root);
    let project = root.join("if_print");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nname = \"if_print\"\nversion = \"0.1.0\"\n\n[dependencies]\nstd = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(
        project.join("src/main.nomo"),
        r#"package app.main

import std.io

fn main() -> void {
    let ok: bool = true
    if ok {
        io.println("if print ok")
    } else {
        io.println("wrong")
    }
    let err: bool = false
    if err {
        io.println("wrong")
    } else {
        io.eprintln("if error print ok")
    }
}
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(&project)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), "if print ok\n");
    assert_eq!(
        String::from_utf8_lossy(&output.stderr),
        "if error print ok\n"
    );

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomoc_build_runs_statement_update_operators() {
    let root = temp_test_root("statement-update-operators");
    reset_dir(&root);
    let source = root.join("statement-updates.nomo");
    let c_path = root.join("statement-updates.c");
    let bin_path = root.join("statement-updates");
    fs::write(
        &source,
        r#"package app.main

import std.io

struct Counter {
    value: i64
}

fn main() -> void {
    let mut value: i64 = 10
    value += 5
    value -= 3
    value *= 4
    value /= 6
    value %= 5
    value <<= 2
    value >>= 1
    value &= 6
    value |= 8
    value ^= 3
    value &^= 1
    value++
    value--

    let mut counter: Counter = Counter { value: 1 }
    counter.value += 2
    counter.value++
    counter.value--

    if value == 12 && counter.value == 3 {
        io.println("statement updates ok")
    } else {
        io.println("wrong")
    }
}
"#,
    )
    .unwrap();

    let build_output = Command::new(env!("CARGO_BIN_EXE_nomoc"))
        .arg("build")
        .arg(&source)
        .arg("--emit-c")
        .arg("--out")
        .arg(&c_path)
        .output()
        .unwrap();
    assert!(
        build_output.status.success(),
        "{}",
        String::from_utf8_lossy(&build_output.stderr)
    );

    let cc_output = Command::new("cc")
        .arg(&c_path)
        .arg("-o")
        .arg(&bin_path)
        .output()
        .unwrap();
    assert!(
        cc_output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&cc_output.stdout),
        String::from_utf8_lossy(&cc_output.stderr)
    );

    let run_output = Command::new(&bin_path).output().unwrap();
    assert!(
        run_output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run_output.stdout),
        String::from_utf8_lossy(&run_output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&run_output.stdout),
        "statement updates ok\n"
    );
    assert!(
        String::from_utf8_lossy(&run_output.stderr).is_empty(),
        "{}",
        String::from_utf8_lossy(&run_output.stderr)
    );

    fs::remove_dir_all(&root).unwrap();
}

#[cfg(unix)]
#[test]
fn nomo_publish_external_signer_and_independent_verify_round_trip() {
    use std::os::unix::fs::PermissionsExt;

    if Command::new("openssl").arg("version").output().is_err()
        || Command::new("xxd").arg("-h").output().is_err()
    {
        return;
    }
    let root = temp_test_root("publish-sign-verify");
    reset_dir(&root);
    let project = root.join("signed");
    let output_dir = root.join("out");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nnamespace = \"fynn\"\nname = \"signed\"\nversion = \"1.0.0\"\nedition = \"2026\"\n",
    )
    .unwrap();
    fs::write(
        project.join("src/main.nomo"),
        "package app.main\n\nfn main() -> void {\n}\n",
    )
    .unwrap();

    let private_key = root.join("publisher-private.pem");
    let generate = Command::new("openssl")
        .arg("genpkey")
        .arg("-algorithm")
        .arg("ED25519")
        .arg("-out")
        .arg(&private_key)
        .output()
        .unwrap();
    assert!(
        generate.status.success(),
        "{}",
        String::from_utf8_lossy(&generate.stderr)
    );
    let public_der = Command::new("openssl")
        .arg("pkey")
        .arg("-in")
        .arg(&private_key)
        .arg("-pubout")
        .arg("-outform")
        .arg("DER")
        .output()
        .unwrap();
    assert!(public_der.status.success());
    let public_key = nomo_supply_chain::encode_hex(
        &public_der.stdout[public_der.stdout.len().saturating_sub(32)..],
    );

    let signer = root.join("external-signer.sh");
    fs::write(
        &signer,
        format!(
            "#!/bin/sh\nset -eu\npayload=\"${{TMPDIR:-/tmp}}/nomo-signer-payload-$$\"\nsignature=\"${{TMPDIR:-/tmp}}/nomo-signer-signature-$$\"\ntrap 'rm -f \"$payload\" \"$signature\"' EXIT\ntee \"$payload\" >/dev/null\nopenssl pkeyutl -sign -rawin -inkey '{}' -in \"$payload\" -out \"$signature\"\nsig=$(xxd -p -c 256 \"$signature\")\nprintf '{{\"algorithm\":\"ed25519\",\"public_key\":\"{}\",\"signature\":\"%s\"}}\\n' \"$sig\"\n",
            private_key.display(),
            public_key
        ),
    )
    .unwrap();
    let mut permissions = fs::metadata(&signer).unwrap().permissions();
    permissions.set_mode(0o700);
    fs::set_permissions(&signer, permissions).unwrap();

    let publish = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("publish")
        .arg(&project)
        .arg("--dry-run")
        .arg("--output")
        .arg(&output_dir)
        .arg("--signer")
        .arg(&signer)
        .output()
        .unwrap();
    assert!(
        publish.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&publish.stdout),
        String::from_utf8_lossy(&publish.stderr)
    );
    let archive = output_dir.join("fynn-signed-1.0.0.nomo-package");
    let provenance = PathBuf::from(format!("{}.provenance.json", archive.display()));
    let envelope = PathBuf::from(format!("{}.envelope.json", archive.display()));
    assert!(archive.is_file());
    assert!(provenance.is_file());
    assert!(envelope.is_file());

    let verify = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("verify")
        .arg(&archive)
        .arg("--envelope")
        .arg(&envelope)
        .arg("--key")
        .arg(&public_key)
        .arg("--provenance")
        .arg(&provenance)
        .output()
        .unwrap();
    assert!(
        verify.status.success(),
        "{}",
        String::from_utf8_lossy(&verify.stderr)
    );
    let stdout = String::from_utf8_lossy(&verify.stdout);
    assert!(
        stdout.contains("verified fynn/signed 1.0.0 sha256:"),
        "{stdout}"
    );
    let public_artifacts = format!(
        "{}{}",
        fs::read_to_string(&provenance).unwrap(),
        fs::read_to_string(&envelope).unwrap()
    );
    assert!(!public_artifacts.contains("PRIVATE KEY"));
    assert!(!public_artifacts.contains(&private_key.display().to_string()));

    let tampered = root.join("tampered.nomo-package");
    let mut bytes = fs::read(&archive).unwrap();
    bytes.push(b'!');
    fs::write(&tampered, bytes).unwrap();
    let rejected = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("verify")
        .arg(&tampered)
        .arg("--envelope")
        .arg(&envelope)
        .arg("--key")
        .arg(&public_key)
        .arg("--provenance")
        .arg(&provenance)
        .output()
        .unwrap();
    assert!(!rejected.status.success());
    assert!(String::from_utf8_lossy(&rejected.stderr).contains("archive checksum does not match"));

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let registry = format!("http://{}", listener.local_addr().unwrap());
    let server = thread::spawn(move || {
        for (suffix, content_type) in [
            ("", "application/octet-stream"),
            ("/provenance", "application/json"),
            ("/attestation", "application/json"),
        ] {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = Vec::new();
            let mut buffer = [0u8; 4096];
            let (header_end, content_length) = loop {
                let read = stream.read(&mut buffer).unwrap();
                assert!(read > 0);
                request.extend_from_slice(&buffer[..read]);
                if let Some(end) = request.windows(4).position(|window| window == b"\r\n\r\n") {
                    let end = end + 4;
                    let headers = String::from_utf8_lossy(&request[..end]);
                    let length = http_header(&headers, "Content-Length")
                        .unwrap()
                        .parse::<usize>()
                        .unwrap();
                    break (end, length);
                }
            };
            while request.len() < header_end + content_length {
                let read = stream.read(&mut buffer).unwrap();
                assert!(read > 0);
                request.extend_from_slice(&buffer[..read]);
            }
            let headers = String::from_utf8_lossy(&request[..header_end]);
            assert!(
                headers.starts_with(&format!(
                    "PUT /api/v1/packages/fynn/signed/1.0.0{suffix} HTTP/1.1\r\n"
                )),
                "{headers}"
            );
            assert_eq!(http_header(&headers, "Content-Type"), Some(content_type));
            if suffix == "/attestation" {
                let body = String::from_utf8_lossy(&request[header_end..]);
                assert!(body.contains("\"algorithm\": \"ed25519\""), "{body}");
                assert!(!body.contains("PRIVATE KEY"), "{body}");
            }
            stream
                .write_all(
                    b"HTTP/1.1 201 Created\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                )
                .unwrap();
        }
    });
    let registry_publish = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("publish")
        .arg(&project)
        .arg("--registry")
        .arg(&registry)
        .arg("--output")
        .arg(root.join("registry-out"))
        .arg("--signer")
        .arg(&signer)
        .env("NOMO_HOME", root.join("empty-nomo-home"))
        .output()
        .unwrap();
    assert!(
        registry_publish.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&registry_publish.stdout),
        String::from_utf8_lossy(&registry_publish.stderr)
    );
    server.join().unwrap();

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_ffi_bindgen_generates_bindings_that_link_and_run() {
    let root = temp_test_root("ffi-bindgen-link-run");
    reset_dir(&root);
    let project = root.join("ffi-bindgen");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::create_dir_all(project.join("native")).unwrap();
    let header = project.join("native/api.h");
    let bindings = project.join("src/bindings.nomo");
    let provenance = project.join("bindings.provenance.json");

    fs::write(
        project.join("nomo.toml"),
        "[package]\nnamespace = \"local\"\nname = \"ffi-bindgen\"\nversion = \"0.0.0-20260713145859\"\nedition = \"2026\"\n\n[ffi]\nsources = [\"native/api.c\"]\n",
    )
    .unwrap();
    fs::write(
        &header,
        r#"#include <stdint.h>

typedef struct FileHandle FileHandle;
typedef struct Point {
    int32_t x;
    int32_t y;
} Point;

FileHandle *file_open(void);
int32_t file_marker(FileHandle *handle);
void file_close(FileHandle *handle);
int32_t point_sum(Point point);
int32_t apply_callback(int32_t value, int32_t (*callback)(int32_t));
"#,
    )
    .unwrap();
    fs::write(
        project.join("native/api.c"),
        r#"#include "api.h"
#include <stdlib.h>

struct FileHandle { int32_t marker; };

FileHandle *file_open(void) {
    FileHandle *handle = malloc(sizeof(FileHandle));
    if (handle != NULL) { handle->marker = 40; }
    return handle;
}
int32_t file_marker(FileHandle *handle) { return handle->marker; }
void file_close(FileHandle *handle) { free(handle); }
int32_t point_sum(Point point) { return point.x + point.y; }
int32_t apply_callback(int32_t value, int32_t (*callback)(int32_t)) {
    return callback(value);
}
"#,
    )
    .unwrap();

    let bindgen_output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("ffi")
        .arg("bindgen")
        .arg(&header)
        .arg("--package")
        .arg("app.bindings")
        .arg("--output")
        .arg(&bindings)
        .arg("--provenance")
        .arg(&provenance)
        .output()
        .unwrap();
    assert!(
        bindgen_output.status.success(),
        "{}",
        String::from_utf8_lossy(&bindgen_output.stderr)
    );
    assert!(bindings.is_file());
    assert!(provenance.is_file());
    let generated = fs::read_to_string(&bindings).unwrap();
    assert!(generated.contains("extern opaque type FileHandle release file_close"));
    assert!(generated.contains("#[repr(C)]\npub struct Point"));
    assert!(generated.contains("callback: extern \"C\" fn(i32) -> i32"));
    let provenance_json = fs::read_to_string(&provenance).unwrap();
    assert!(provenance_json.contains("\"source_sha256\""));
    assert!(provenance_json.contains("\"generator\": \"nomo ffi bindgen\""));

    fs::write(
        project.join("src/main.nomo"),
        r#"package app.main

import app.bindings
import std.io

fn double(value: i32) -> i32 {
    return value * 2
}

fn open() -> Nullable<Owned<FileHandle>> {
    unsafe {
        return file_open()
    }
}

fn marker(handle: Borrowed<FileHandle>) -> i32 {
    unsafe {
        return file_marker(handle)
    }
}

fn close(handle: Owned<FileHandle>) -> void {
    unsafe {
        file_close(handle)
    }
}

fn sum(point: Point) -> i32 {
    unsafe {
        return point_sum(point)
    }
}

fn callback(value: i32) -> i32 {
    unsafe {
        return apply_callback(value, double)
    }
}

fn main() -> void {
    let maybe: Nullable<Owned<FileHandle>> = open()
    let handle: Owned<FileHandle> = maybe.unwrap()
    let observed: i32 = marker(handle.borrow())
    let point: Point = Point {
        x: 1,
        y: 1,
    }
    let total: i32 = sum(point) + callback(20)
    close(handle)
    if observed == 40 && total == 42 {
        io.println("ffi bindgen ok")
    } else {
        panic("ffi bindgen failed")
    }
}
"#,
    )
    .unwrap();

    let run_output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(&project)
        .output()
        .unwrap();
    assert!(
        run_output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run_output.stdout),
        String::from_utf8_lossy(&run_output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&run_output.stdout),
        "ffi bindgen ok\n"
    );

    fs::remove_dir_all(&root).unwrap();
}

fn reset_dir(path: &Path) {
    if path.exists() {
        fs::remove_dir_all(path).unwrap();
    }
    fs::create_dir_all(path).unwrap();
}

fn init_git_package(path: &Path, namespace: &str, name: &str) -> String {
    init_git_package_with_source(path, namespace, name, "package package.main\n")
}

fn init_git_package_with_source(path: &Path, namespace: &str, name: &str, source: &str) -> String {
    fs::create_dir_all(path.join("src")).unwrap();
    fs::write(path.join("src/main.nomo"), source).unwrap();
    fs::write(
        path.join("nomo.toml"),
        format!(
            "[package]\nnamespace = \"{namespace}\"\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2026\"\n"
        ),
    )
    .unwrap();

    run_git(path, &["init", "--quiet"]);
    run_git(path, &["config", "user.email", "nomo@example.invalid"]);
    run_git(path, &["config", "user.name", "Nomo Test"]);
    run_git(path, &["add", "nomo.toml", "src/main.nomo"]);
    run_git(path, &["commit", "--quiet", "-m", "initial"]);

    let output = Command::new("git")
        .arg("-C")
        .arg(path)
        .arg("rev-parse")
        .arg("HEAD")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

fn run_git(path: &Path, args: &[&str]) {
    let output = Command::new("git")
        .arg("-C")
        .arg(path)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn git_head_rev(path: &Path) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(path)
        .arg("rev-parse")
        .arg("HEAD")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

fn find_git_cache_checkout(project: &Path, alias: &str) -> PathBuf {
    let cache_root = project.join(".nomo/deps/git");
    let entries = fs::read_dir(&cache_root)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", cache_root.display()));
    let checkouts = entries
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.is_dir())
        .collect::<Vec<_>>();
    if checkouts.len() == 1 {
        return checkouts[0].clone();
    }
    panic!("missing git cache checkout for alias `{alias}`");
}

fn find_incremental_cache_entry(root: &Path, namespace: &str) -> PathBuf {
    find_incremental_cache_entry_if_present(root, namespace).unwrap_or_else(|| {
        panic!(
            "missing incremental cache entry for namespace `{namespace}` below {}",
            root.display()
        )
    })
}

fn find_incremental_cache_entry_if_present(root: &Path, namespace: &str) -> Option<PathBuf> {
    for entry in fs::read_dir(root)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", root.display()))
    {
        let path = entry.unwrap().path();
        if path.is_dir() {
            if let Some(found) = find_incremental_cache_entry_if_present(&path, namespace) {
                return Some(found);
            }
        } else if path.extension().and_then(|extension| extension.to_str()) == Some("json")
            && fs::read_to_string(&path)
                .map(|text| text.contains(&format!("\"namespace\":\"{namespace}\"")))
                .unwrap_or(false)
        {
            return Some(path);
        }
    }
    None
}

fn strip_checksum_lines(text: &str) -> String {
    text.lines()
        .filter(|line| !line.trim_start().starts_with("checksum = "))
        .map(|line| format!("{line}\n"))
        .collect()
}

fn assert_checksum_lines(text: &str, expected: usize) {
    let lines = text
        .lines()
        .filter(|line| line.trim_start().starts_with("checksum = "))
        .collect::<Vec<_>>();
    assert_eq!(lines.len(), expected, "lockfile:\n{text}");
    for line in lines {
        let checksum = line
            .trim()
            .strip_prefix("checksum = \"sha256:")
            .and_then(|value| value.strip_suffix('"'))
            .unwrap_or_else(|| panic!("invalid checksum line `{line}`"));
        assert_eq!(checksum.len(), 64, "invalid checksum line `{line}`");
        assert!(
            checksum.chars().all(|ch| ch.is_ascii_hexdigit()),
            "invalid checksum line `{line}`"
        );
    }
}

fn temp_test_root(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "nomo-cli-project-test-{name}-{}",
        std::process::id()
    ))
}

fn cc_supports_address_sanitizer(root: &Path) -> bool {
    let source = root.join("asan-probe.c");
    let bin = root.join("asan-probe");
    fs::write(&source, "int main(void) { return 0; }\n").unwrap();

    let Ok(output) = Command::new("cc")
        .arg("-fsanitize=address")
        .arg(&source)
        .arg("-o")
        .arg(&bin)
        .output()
    else {
        return false;
    };
    if !output.status.success() {
        return false;
    }

    let Ok(output) = Command::new(&bin)
        .env("ASAN_OPTIONS", "detect_leaks=0:abort_on_error=1")
        .output()
    else {
        return false;
    };
    output.status.success()
}

fn cc_supports_undefined_sanitizer(root: &Path) -> bool {
    let source = root.join("ubsan-probe.c");
    let bin = root.join("ubsan-probe");
    fs::write(&source, "int main(void) { return 0; }\n").unwrap();

    let Ok(output) = Command::new("cc")
        .arg("-fsanitize=undefined")
        .arg("-fno-sanitize-recover=undefined")
        .arg(&source)
        .arg("-o")
        .arg(&bin)
        .output()
    else {
        return false;
    };
    if !output.status.success() {
        return false;
    }

    let Ok(output) = Command::new(&bin)
        .env("UBSAN_OPTIONS", "halt_on_error=1")
        .output()
    else {
        return false;
    };
    output.status.success()
}
