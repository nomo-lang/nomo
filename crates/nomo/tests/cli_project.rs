use std::fs;
use std::io::{ErrorKind, Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream, UdpSocket as RustUdpSocket};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

const NOMO_HELP: &str = "nomo 0.1.0\n\nCommands:\n  nomo new <name>\n  nomo check [path] [--json-errors] [--workspace]\n  nomo build [path] [--emit-c] [--json-errors] [--workspace] [--locked] [--offline] [--frozen]\n  nomo run [path] [--json-errors] [-- args...]\n  nomo fmt [path] [--check] [--json-errors]\n  nomo test [path] [--workspace] [--package <package>] [--filter <text>] [--json] [--locked] [--offline] [--frozen]\n  nomo doc [path] [--workspace] [--package <package>] [--std] [--open] [--json] [--output <dir>]\n  nomo clean [path]\n  nomo login --registry <url> --token <token>\n  nomo owner add <owner/package> <user> --registry <url>\n  nomo owner remove <owner/package> <user> --registry <url>\n  nomo add <alias>@<owner>/<package>:<version> [path] [--registry <url>]\n  nomo remove <alias> [path]\n  nomo search <query> --registry <url>\n  nomo yank <owner/package> <version> --registry <url>\n  nomo publish [path] (--dry-run | --registry <url>) [--output <dir>] [--json-errors]\n  nomo deps <resolve|tree> [path] [--workspace] [--locked] [--offline] [--frozen]\n  nomo deps update [path] [alias-or-package] [--workspace] [--offline] [--precise <version-or-rev>]\n  nomo deps vendor [path] [--workspace] [--dir vendor] [--sync]\n  nomo deps clean-cache [path]\n\n";

const NOMOC_HELP: &str = "nomoc 0.1.0\n\nCommands:\n  nomoc check <source.nomo> [--json-errors]\n  nomoc build <source.nomo> [--emit-c] [--out path] [--json-errors]\n\n";

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
    assert_eq!(String::from_utf8_lossy(&output.stdout), NOMO_HELP);
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
        assert_eq!(String::from_utf8_lossy(&output.stdout), NOMO_HELP);
        assert!(
            output.stderr.is_empty(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
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
    assert!(stderr.contains("\"error_code\":\"E0102\""), "{stderr}");
    assert!(stderr.contains("semicolons are not supported"), "{stderr}");

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
        "[package]\nnamespace = \"local\"\nname = \"hello\"\nversion = \"0.1.0\"\nedition = \"2026\"\n"
    );
    let source_text = fs::read_to_string(&source).unwrap();
    assert!(source_text.contains("package app.main"));
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
        r#"package local_utils.path

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
    assert!(generated_c.contains("nomo_pkg_local_utils_path_fn_join"));
    assert!(!generated_c.contains("nomo_pkg_app_main_fn_join"));
    assert!(generated_c.contains("nomo_pkg_local_utils_path_struct_Segment"));
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
        r#"package local_utils.path

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
    assert!(generated_c.contains("nomo_pkg_local_utils_path_fn_join"));
    assert!(!generated_c.contains("nomo_pkg_app_main_fn_join"));

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_deps_update_precise_rewrites_registry_lockfile() {
    let root = temp_test_root("deps-update-precise-registry");
    reset_dir(&root);
    let project = root.join("hello");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(project.join("src/main.nomo"), "package app.main\n").unwrap();
    let manifest = "[package]\nnamespace = \"fynn\"\nname = \"hello\"\nversion = \"0.1.0\"\nedition = \"2026\"\n\n[dependencies]\njson = { package = \"nomo-lang/json\", version = \"0.1.0\", registry = \"https://packages.nomo.test\" }\n";
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
    assert!(!lockfile.contains("version = \"0.1.0\""), "{lockfile}");

    fs::remove_dir_all(&root).unwrap();
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
fn nomo_deps_resolve_rejects_conflicting_package_sources() {
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
    assert!(stderr.contains("conflicting sources"), "{stderr}");
    assert!(stderr.contains("nomo-lang/cli"), "{stderr}");
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
        r#"package local_utils.path

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
    assert!(generated_c.contains("nomo_pkg_local_utils_path_fn_join"));
    assert!(!generated_c.contains("nomo_pkg_app_main_fn_join"));
    assert!(generated_c.contains("nomo_pkg_local_utils_path_struct_Segment"));
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
        r#"package local_utils.path

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
    assert!(generated_c.contains("nomo_pkg_local_utils_path_fn_join"));
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
        stderr.contains("usage: nomoc build <source.nomo> [--emit-c] [--out path] [--json-errors]"),
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

    let run_output = Command::new(&bin_path)
        .env("ASAN_OPTIONS", "detect_leaks=0:abort_on_error=1")
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
        "{\"lang\":\"nomo\",\"versions\":[1,true,null]}\ninvalid json\n"
    );
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    fs::remove_dir_all(&root).unwrap();
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
                    let (expected_line, expected_body, response_body) = if handled == 0 {
                        ("GET /hello HTTP/1.0", "", "get-ok")
                    } else {
                        ("POST /echo HTTP/1.0", "post-body", "post-ok")
                    };
                    assert!(text.starts_with(expected_line), "request was:\n{text}");
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
    assert_eq!(String::from_utf8_lossy(&output.stdout), "get-ok\npost-ok\n");
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    server.join().unwrap();

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
