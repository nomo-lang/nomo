use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[test]
fn nomoc_check_emits_json_errors() {
    let root = temp_test_root("nomoc-json");
    reset_dir(&root);
    let source = root.join("bad.nomo");
    fs::write(&source, invalid_source()).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomoc"))
        .arg("check")
        .arg(&source)
        .arg("--json-errors")
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert_json_diagnostic(&output.stderr, &source);
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomoc_build_emits_json_errors() {
    let root = temp_test_root("nomoc-build-json");
    reset_dir(&root);
    let source = root.join("bad.nomo");
    fs::write(&source, invalid_source()).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomoc"))
        .arg("build")
        .arg(&source)
        .arg("--emit-c")
        .arg("--json-errors")
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert_json_diagnostic(&output.stderr, &source);
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_check_emits_json_errors() {
    let root = temp_test_root("nomo-json");
    reset_dir(&root);
    let project = root.join("json_demo");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nname = \"json_demo\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(project.join("src/main.nomo"), invalid_source()).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("check")
        .arg(&project)
        .arg("--json-errors")
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert_json_diagnostic(&output.stderr, &project.join("src/main.nomo"));
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_check_source_file_path_emits_json_errors() {
    let root = temp_test_root("nomo-source-file-json");
    reset_dir(&root);
    let project = root.join("json_source_file_demo");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nname = \"json_source_file_demo\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    let source = project.join("src/main.nomo");
    fs::write(&source, invalid_source()).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("check")
        .arg(&source)
        .arg("--json-errors")
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert_json_diagnostic(&output.stderr, &source);
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_build_emits_json_errors() {
    let root = temp_test_root("nomo-build-json");
    reset_dir(&root);
    let project = root.join("json_build_demo");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nname = \"json_build_demo\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(project.join("src/main.nomo"), invalid_source()).unwrap();
    let build_dir = project.join("build");

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("build")
        .arg(&project)
        .arg("--emit-c")
        .arg("--json-errors")
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(
        output.stdout.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert_json_diagnostic(&output.stderr, &project.join("src/main.nomo"));
    assert!(!build_dir.exists());
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_build_source_file_path_emits_json_errors() {
    let root = temp_test_root("nomo-build-source-file-json");
    reset_dir(&root);
    let project = root.join("json_build_source_file_demo");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nname = \"json_build_source_file_demo\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    let source = project.join("src/main.nomo");
    fs::write(&source, invalid_source()).unwrap();
    let build_dir = project.join("build");

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("build")
        .arg(&source)
        .arg("--emit-c")
        .arg("--json-errors")
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(
        output.stdout.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert_json_diagnostic(&output.stderr, &source);
    assert!(!build_dir.exists());
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_native_build_source_file_path_emits_json_errors() {
    let root = temp_test_root("nomo-native-build-source-file-json");
    reset_dir(&root);
    let project = root.join("json_native_build_source_file_demo");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nname = \"json_native_build_source_file_demo\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    let source = project.join("src/main.nomo");
    fs::write(&source, invalid_source()).unwrap();
    let build_dir = project.join("build");

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("build")
        .arg(&source)
        .arg("--json-errors")
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(
        output.stdout.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert_json_diagnostic(&output.stderr, &source);
    assert!(!build_dir.exists());
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_run_emits_json_errors() {
    let root = temp_test_root("nomo-run-json");
    reset_dir(&root);
    let project = root.join("json_run_demo");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nname = \"json_run_demo\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(project.join("src/main.nomo"), invalid_source()).unwrap();
    let build_dir = project.join("build");

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(&project)
        .arg("--json-errors")
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(
        output.stdout.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert_json_diagnostic(&output.stderr, &project.join("src/main.nomo"));
    assert!(!build_dir.exists());
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_run_source_file_path_emits_json_errors() {
    let root = temp_test_root("nomo-run-source-file-json");
    reset_dir(&root);
    let project = root.join("json_run_source_file_demo");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nname = \"json_run_source_file_demo\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    let source = project.join("src/main.nomo");
    fs::write(&source, invalid_source()).unwrap();
    let build_dir = project.join("build");

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(&source)
        .arg("--json-errors")
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(
        output.stdout.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert_json_diagnostic(&output.stderr, &source);
    assert!(!build_dir.exists());
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomoc_check_emits_json_for_lexical_errors() {
    let root = temp_test_root("nomoc-json-lexical");
    reset_dir(&root);
    let source = root.join("bad-token.nomo");
    fs::write(&source, invalid_lexical_source()).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomoc"))
        .arg("check")
        .arg(&source)
        .arg("--json-errors")
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(
        output.stdout.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert_lexical_json_diagnostic(&output.stderr, &source);
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_check_emits_json_for_lexical_errors() {
    let root = temp_test_root("nomo-json-lexical");
    reset_dir(&root);
    let project = root.join("lexical_demo");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nname = \"lexical_demo\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(project.join("src/main.nomo"), invalid_lexical_source()).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("check")
        .arg(&project)
        .arg("--json-errors")
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(
        output.stdout.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert_lexical_json_diagnostic(&output.stderr, &project.join("src/main.nomo"));
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomoc_check_emits_json_for_semicolon_lexical_error() {
    let root = temp_test_root("nomoc-json-semicolon");
    reset_dir(&root);
    let source = root.join("semicolon.nomo");
    fs::write(&source, invalid_semicolon_source()).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomoc"))
        .arg("check")
        .arg(&source)
        .arg("--json-errors")
        .output()
        .unwrap();

    assert!(!output.status.success());
    let text = String::from_utf8_lossy(&output.stderr);
    let text = text.trim();
    assert_single_json_object(text);
    assert!(text.contains("\"status\":\"error\""), "{text}");
    assert!(text.contains("\"error_code\":\"E0102\""), "{text}");
    assert!(
        text.contains(
            "\"message\":\"semicolons are not supported in v0.1; use a newline to separate statements\""
        ),
        "{text}"
    );
    assert!(text.contains("\"line\":4"), "{text}");
    assert!(text.contains("\"column\":23"), "{text}");
    assert!(text.contains("\"suggestions\":[]"), "{text}");
    assert!(
        output.stdout.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomoc_check_emits_json_for_reserved_future_keywords() {
    let root = temp_test_root("nomoc-json-reserved-future-keyword");
    reset_dir(&root);
    let source = root.join("reserved-future-keyword.nomo");
    fs::write(
        &source,
        "package app.main\n\nfn main() -> void {\n    go work()\n}\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomoc"))
        .arg("check")
        .arg(&source)
        .arg("--json-errors")
        .output()
        .unwrap();

    assert!(!output.status.success());
    let text = String::from_utf8_lossy(&output.stderr);
    let text = text.trim();
    assert_single_json_object(text);
    assert!(text.contains("\"status\":\"error\""), "{text}");
    assert!(text.contains("\"error_code\":\"E0104\""), "{text}");
    assert!(
        text.contains("\"message\":\"`go` is reserved for future Nomo versions\""),
        "{text}"
    );
    assert!(text.contains("\"line\":4"), "{text}");
    assert!(text.contains("\"column\":5"), "{text}");
    assert!(text.contains("\"length\":2"), "{text}");
    assert!(text.contains("\"suggestions\":[]"), "{text}");
    assert!(
        output.stdout.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomoc_check_emits_json_for_parser_errors() {
    let root = temp_test_root("nomoc-json-parser");
    reset_dir(&root);
    let source = root.join("bad-syntax.nomo");
    fs::write(&source, invalid_parser_source()).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomoc"))
        .arg("check")
        .arg(&source)
        .arg("--json-errors")
        .output()
        .unwrap();

    assert!(!output.status.success());
    let text = String::from_utf8_lossy(&output.stderr);
    let text = text.trim();
    assert_single_json_object(text);
    assert!(text.contains("\"status\":\"error\""), "{text}");
    assert!(text.contains("\"error_code\":\"E0211\""), "{text}");
    assert!(
        text.contains("\"message\":\"expected newline after package declaration\""),
        "{text}"
    );
    assert!(text.contains("\"line\":1"), "{text}");
    assert!(text.contains("\"column\":18"), "{text}");
    assert!(text.contains("\"suggestions\":[]"), "{text}");
    assert!(
        output.stdout.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomo_check_emits_json_for_parser_errors() {
    let root = temp_test_root("nomo-json-parser");
    reset_dir(&root);
    let project = root.join("parser_demo");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("nomo.toml"),
        "[package]\nname = \"parser_demo\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(project.join("src/main.nomo"), invalid_parser_source()).unwrap();
    let build_dir = project.join("build");

    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("check")
        .arg(&project)
        .arg("--json-errors")
        .output()
        .unwrap();

    assert!(!output.status.success());
    let text = String::from_utf8_lossy(&output.stderr);
    let text = text.trim();
    assert_single_json_object(text);
    assert!(text.contains("\"status\":\"error\""), "{text}");
    assert!(text.contains("\"error_code\":\"E0211\""), "{text}");
    assert!(
        text.contains("\"message\":\"expected newline after package declaration\""),
        "{text}"
    );
    assert!(text.contains("\"line\":1"), "{text}");
    assert!(text.contains("\"column\":18"), "{text}");
    assert!(text.contains("\"suggestions\":[]"), "{text}");
    assert!(
        output.stdout.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(!build_dir.exists());

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomoc_check_emits_json_suggestions() {
    let root = temp_test_root("nomoc-json-suggestions");
    reset_dir(&root);
    let source = root.join("missing-import.nomo");
    fs::write(
        &source,
        "package app.main\n\nfn main() -> void {\n    println(\"hi\")\n}\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomoc"))
        .arg("check")
        .arg(&source)
        .arg("--json-errors")
        .output()
        .unwrap();

    assert!(!output.status.success());
    let text = String::from_utf8_lossy(&output.stderr);
    let text = text.trim();
    assert_single_json_object(text);
    assert!(text.contains("\"error_code\":\"E0301\""), "{text}");
    assert!(text.contains("\"suggestions\":[{"), "{text}");
    assert!(text.contains("\"action\":\"replace_text\""), "{text}");
    assert!(
        text.contains("\"text\":\"import std.io.println\\n\""),
        "{text}"
    );
    assert!(
        text.contains("\"description\":\"add `import std.io.println` to use `println`\""),
        "{text}"
    );
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomoc_check_emits_json_for_missing_standard_type_import() {
    let root = temp_test_root("nomoc-json-missing-std-type-import");
    reset_dir(&root);
    let source = root.join("missing-standard-type-import.nomo");
    fs::write(
        &source,
        "package app.main\n\nfn parse() -> Result<i32, string> {\n    return 1\n}\n\nfn main() -> void {\n}\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomoc"))
        .arg("check")
        .arg(&source)
        .arg("--json-errors")
        .output()
        .unwrap();

    assert!(!output.status.success());
    let text = String::from_utf8_lossy(&output.stderr);
    let text = text.trim();
    assert_single_json_object(text);
    assert!(text.contains("\"status\":\"error\""), "{text}");
    assert!(text.contains("\"error_code\":\"E0301\""), "{text}");
    assert!(
        text.contains("\"message\":\"`Result` requires `import std.result`\""),
        "{text}"
    );
    assert!(text.contains("\"line\":3"), "{text}");
    assert!(text.contains("\"suggestions\":[]"), "{text}");

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomoc_check_emits_json_for_missing_main() {
    let root = temp_test_root("nomoc-json-missing-main");
    reset_dir(&root);
    let source = root.join("missing-main.nomo");
    fs::write(
        &source,
        "package app.main\n\nfn helper() -> string {\n    return \"ok\"\n}\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomoc"))
        .arg("check")
        .arg(&source)
        .arg("--json-errors")
        .output()
        .unwrap();

    assert!(!output.status.success());
    let text = String::from_utf8_lossy(&output.stderr);
    let text = text.trim();
    assert_single_json_object(text);
    assert!(text.contains("\"status\":\"error\""), "{text}");
    assert!(text.contains("\"error_code\":\"E0201\""), "{text}");
    assert!(
        text.contains("\"message\":\"expected `fn main() -> void { ... }`\""),
        "{text}"
    );
    assert!(text.contains("\"line\":1"), "{text}");
    assert!(text.contains("\"suggestions\":[]"), "{text}");

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomoc_check_emits_json_for_invalid_main_return_type() {
    let root = temp_test_root("nomoc-json-invalid-main-return");
    reset_dir(&root);
    let source = root.join("invalid-main-return.nomo");
    fs::write(
        &source,
        "package app.main\n\nfn main() -> i32 {\n    return 1\n}\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomoc"))
        .arg("check")
        .arg(&source)
        .arg("--json-errors")
        .output()
        .unwrap();

    assert!(!output.status.success());
    let text = String::from_utf8_lossy(&output.stderr);
    let text = text.trim();
    assert_single_json_object(text);
    assert!(text.contains("\"status\":\"error\""), "{text}");
    assert!(text.contains("\"error_code\":\"E0401\""), "{text}");
    assert!(
        text.contains("\"message\":\"v0.1 `main` must return `void` or `Result<void, E>`\""),
        "{text}"
    );
    assert!(text.contains("\"line\":1"), "{text}");
    assert!(text.contains("\"suggestions\":[]"), "{text}");

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomoc_check_emits_json_for_unsupported_wildcard_imports() {
    let root = temp_test_root("nomoc-json-wildcard-import");
    reset_dir(&root);
    let source = root.join("wildcard-import.nomo");
    fs::write(
        &source,
        "package app.main\n\nimport std.io.*\n\nfn main() -> void {\n}\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomoc"))
        .arg("check")
        .arg(&source)
        .arg("--json-errors")
        .output()
        .unwrap();

    assert!(!output.status.success());
    let text = String::from_utf8_lossy(&output.stderr);
    let text = text.trim();
    assert_single_json_object(text);
    assert!(text.contains("\"status\":\"error\""), "{text}");
    assert!(text.contains("\"error_code\":\"E0274\""), "{text}");
    assert!(
        text.contains("\"message\":\"wildcard imports are not supported in v0.1\""),
        "{text}"
    );
    assert!(text.contains("\"line\":3"), "{text}");
    assert!(text.contains("\"suggestions\":[]"), "{text}");

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomoc_check_emits_json_for_duplicate_local_binding() {
    let root = temp_test_root("nomoc-json-duplicate-local");
    reset_dir(&root);
    let source = root.join("duplicate-local.nomo");
    fs::write(
        &source,
        "package app.main\n\nfn main() -> void {\n    let name: string = \"one\"\n    let name: string = \"two\"\n}\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomoc"))
        .arg("check")
        .arg(&source)
        .arg("--json-errors")
        .output()
        .unwrap();

    assert!(!output.status.success());
    let text = String::from_utf8_lossy(&output.stderr);
    let text = text.trim();
    assert_single_json_object(text);
    assert!(text.contains("\"status\":\"error\""), "{text}");
    assert!(text.contains("\"error_code\":\"E0302\""), "{text}");
    assert!(
        text.contains("\"message\":\"variable `name` is already defined in this scope\""),
        "{text}"
    );
    assert!(text.contains("\"line\":5"), "{text}");
    assert!(text.contains("\"suggestions\":[]"), "{text}");

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomoc_check_emits_json_for_unknown_variable_read() {
    let root = temp_test_root("nomoc-json-unknown-variable");
    reset_dir(&root);
    let source = root.join("unknown-variable.nomo");
    fs::write(
        &source,
        "package app.main\n\nfn main() -> void {\n    let value: i32 = missing\n}\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomoc"))
        .arg("check")
        .arg(&source)
        .arg("--json-errors")
        .output()
        .unwrap();

    assert!(!output.status.success());
    let text = String::from_utf8_lossy(&output.stderr);
    let text = text.trim();
    assert_single_json_object(text);
    assert!(text.contains("\"status\":\"error\""), "{text}");
    assert!(text.contains("\"error_code\":\"E0303\""), "{text}");
    assert!(
        text.contains("\"message\":\"unknown variable `missing`\""),
        "{text}"
    );
    assert!(text.contains("\"line\":4"), "{text}");
    assert!(text.contains("\"suggestions\":[]"), "{text}");

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomoc_check_emits_json_for_unsupported_match_wildcard() {
    let root = temp_test_root("nomoc-json-match-wildcard");
    reset_dir(&root);
    let source = root.join("match-wildcard.nomo");
    fs::write(
        &source,
        "package app.main\n\nfn label(value: Option<string>) -> string {\n    return match value {\n        Some(text) => text\n        _ => \"missing\"\n    }\n}\n\nfn main() -> void {\n}\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomoc"))
        .arg("check")
        .arg(&source)
        .arg("--json-errors")
        .output()
        .unwrap();

    assert!(!output.status.success());
    let text = String::from_utf8_lossy(&output.stderr);
    let text = text.trim();
    assert_single_json_object(text);
    assert!(text.contains("\"status\":\"error\""), "{text}");
    assert!(text.contains("\"error_code\":\"E0238\""), "{text}");
    assert!(
        text.contains("\"message\":\"`_` match patterns are not supported in v0.1\""),
        "{text}"
    );
    assert!(text.contains("\"line\":6"), "{text}");
    assert!(text.contains("\"suggestions\":[]"), "{text}");

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomoc_check_emits_json_for_non_exhaustive_match() {
    let root = temp_test_root("nomoc-json-non-exhaustive-match");
    reset_dir(&root);
    let source = root.join("non-exhaustive-match.nomo");
    fs::write(
        &source,
        "package app.main\n\nenum Color {\n    Red\n    Blue\n}\n\nfn label(color: Color) -> string {\n    return match color {\n        Color.Red => \"red\"\n    }\n}\n\nfn main() -> void {\n}\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomoc"))
        .arg("check")
        .arg(&source)
        .arg("--json-errors")
        .output()
        .unwrap();

    assert!(!output.status.success());
    let text = String::from_utf8_lossy(&output.stderr);
    let text = text.trim();
    assert_single_json_object(text);
    assert!(text.contains("\"status\":\"error\""), "{text}");
    assert!(text.contains("\"error_code\":\"E0318\""), "{text}");
    assert!(
        text.contains("\"message\":\"match is missing arm `Color.Blue`\""),
        "{text}"
    );
    assert!(text.contains("\"line\":9"), "{text}");
    assert!(text.contains("\"suggestions\":[]"), "{text}");

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomoc_check_emits_json_for_stdlib_arity_error() {
    let root = temp_test_root("nomoc-json-stdlib-arity");
    reset_dir(&root);
    let source = root.join("stdlib-arity.nomo");
    fs::write(
        &source,
        "package app.main\n\nimport std.fs\n\nfn main() -> void {\n    let result: Result<void, FsError> = fs.write_string(\"/tmp/nomo-missing-content.txt\")\n}\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomoc"))
        .arg("check")
        .arg(&source)
        .arg("--json-errors")
        .output()
        .unwrap();

    assert!(!output.status.success());
    let text = String::from_utf8_lossy(&output.stderr);
    let text = text.trim();
    assert_single_json_object(text);
    assert!(text.contains("\"status\":\"error\""), "{text}");
    assert!(text.contains("\"error_code\":\"E0407\""), "{text}");
    assert!(
        text.contains("\"message\":\"`fs.write_string` expects path and content strings\""),
        "{text}"
    );
    assert!(text.contains("\"line\":6"), "{text}");
    assert!(text.contains("\"suggestions\":[]"), "{text}");

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomoc_check_emits_json_for_question_error_type_mismatch() {
    let root = temp_test_root("nomoc-json-question-error-mismatch");
    reset_dir(&root);
    let source = root.join("question-error-mismatch.nomo");
    fs::write(
        &source,
        "package app.main\n\nimport std.result\n\nstruct AppError {\n    message: string\n}\n\nfn parse() -> Result<i32, string> {\n    return Ok(1)\n}\n\nfn compute() -> Result<i32, AppError> {\n    let value: i32 = parse()?\n    return Ok(value)\n}\n\nfn main() -> void {\n}\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomoc"))
        .arg("check")
        .arg(&source)
        .arg("--json-errors")
        .output()
        .unwrap();

    assert!(!output.status.success());
    let text = String::from_utf8_lossy(&output.stderr);
    let text = text.trim();
    assert_single_json_object(text);
    assert!(text.contains("\"status\":\"error\""), "{text}");
    assert!(text.contains("\"error_code\":\"E0404\""), "{text}");
    assert!(
        text.contains("\"message\":\"`?` error type is `string` but function returns `AppError`\""),
        "{text}"
    );
    assert!(text.contains("\"line\":14"), "{text}");
    assert!(text.contains("\"expected\":\"AppError\""), "{text}");
    assert!(text.contains("\"found\":\"string\""), "{text}");
    assert!(text.contains("\"suggestions\":[]"), "{text}");

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomoc_check_emits_json_for_question_on_non_result_value() {
    let root = temp_test_root("nomoc-json-question-non-result");
    reset_dir(&root);
    let source = root.join("question-non-result.nomo");
    fs::write(
        &source,
        "package app.main\n\nimport std.result\n\nfn parse() -> i32 {\n    return 1\n}\n\nfn compute() -> Result<i32, string> {\n    let value: i32 = parse()?\n    return Ok(value)\n}\n\nfn main() -> void {\n}\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomoc"))
        .arg("check")
        .arg(&source)
        .arg("--json-errors")
        .output()
        .unwrap();

    assert!(!output.status.success());
    let text = String::from_utf8_lossy(&output.stderr);
    let text = text.trim();
    assert_single_json_object(text);
    assert!(text.contains("\"status\":\"error\""), "{text}");
    assert!(text.contains("\"error_code\":\"E0420\""), "{text}");
    assert!(
        text.contains("\"message\":\"`?` can only be used with `Result<T, E>` or `Option<T>`\""),
        "{text}"
    );
    assert!(text.contains("\"line\":10"), "{text}");
    assert!(text.contains("\"suggestions\":[]"), "{text}");

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomoc_check_emits_json_for_question_in_non_result_function() {
    let root = temp_test_root("nomoc-json-question-non-result-function");
    reset_dir(&root);
    let source = root.join("question-non-result-function.nomo");
    fs::write(
        &source,
        "package app.main\n\nimport std.result\n\nfn parse() -> Result<i32, string> {\n    return Ok(1)\n}\n\nfn main() -> void {\n    let value: i32 = parse()?\n}\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomoc"))
        .arg("check")
        .arg(&source)
        .arg("--json-errors")
        .output()
        .unwrap();

    assert!(!output.status.success());
    let text = String::from_utf8_lossy(&output.stderr);
    let text = text.trim();
    assert_single_json_object(text);
    assert!(text.contains("\"status\":\"error\""), "{text}");
    assert!(text.contains("\"error_code\":\"E0421\""), "{text}");
    assert!(
        text.contains(
            "\"message\":\"`?` on Result<T, E> requires the current function to return Result<U, E>\""
        ),
        "{text}"
    );
    assert!(text.contains("\"line\":10"), "{text}");
    assert!(text.contains("\"suggestions\":[]"), "{text}");

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomoc_check_emits_json_for_unsupported_question_position() {
    let root = temp_test_root("nomoc-json-question-unsupported-position");
    reset_dir(&root);
    let source = root.join("question-unsupported-position.nomo");
    fs::write(
        &source,
        "package app.main\n\nimport std.result\n\nfn parse() -> Result<string, string> {\n    return Ok(\"value\")\n}\n\nconst VALUE: string = parse()?\n\nfn main() -> void {\n}\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomoc"))
        .arg("check")
        .arg(&source)
        .arg("--json-errors")
        .output()
        .unwrap();

    assert!(!output.status.success());
    let text = String::from_utf8_lossy(&output.stderr);
    let text = text.trim();
    assert_single_json_object(text);
    assert!(text.contains("\"status\":\"error\""), "{text}");
    assert!(text.contains("\"error_code\":\"E0422\""), "{text}");
    assert!(
        text.contains(
            "\"message\":\"`?` is currently supported only in statement-level expressions with unconditional evaluation\""
        ),
        "{text}"
    );
    assert!(text.contains("\"suggestions\":[]"), "{text}");

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomoc_check_emits_json_for_implicit_numeric_conversion() {
    let root = temp_test_root("nomoc-json-implicit-numeric-conversion");
    reset_dir(&root);
    let source = root.join("implicit-numeric-conversion.nomo");
    fs::write(
        &source,
        "package app.main\n\nfn main() -> void {\n    let age: i32 = 18\n    let ratio: f64 = age\n}\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomoc"))
        .arg("check")
        .arg(&source)
        .arg("--json-errors")
        .output()
        .unwrap();

    assert!(!output.status.success());
    let text = String::from_utf8_lossy(&output.stderr);
    let text = text.trim();
    assert_single_json_object(text);
    assert!(text.contains("\"status\":\"error\""), "{text}");
    assert!(text.contains("\"error_code\":\"E0404\""), "{text}");
    assert!(
        text.contains("\"message\":\"cannot initialize `ratio` as `f64` from `i32`\""),
        "{text}"
    );
    assert!(text.contains("\"line\":5"), "{text}");
    assert!(text.contains("\"expected\":\"f64\""), "{text}");
    assert!(text.contains("\"found\":\"i32\""), "{text}");
    assert!(text.contains("\"suggestions\":[]"), "{text}");

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomoc_check_emits_json_for_implicit_numeric_conversion_in_call_argument() {
    let root = temp_test_root("nomoc-json-implicit-call-numeric-conversion");
    reset_dir(&root);
    let source = root.join("implicit-call-numeric-conversion.nomo");
    fs::write(
        &source,
        "package app.main\n\nfn inspect(value: i64) -> void {\n}\n\nfn main() -> void {\n    let count: i32 = 41\n    inspect(count)\n}\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomoc"))
        .arg("check")
        .arg(&source)
        .arg("--json-errors")
        .output()
        .unwrap();

    assert!(!output.status.success());
    let text = String::from_utf8_lossy(&output.stderr);
    let text = text.trim();
    assert_single_json_object(text);
    assert!(text.contains("\"status\":\"error\""), "{text}");
    assert!(text.contains("\"error_code\":\"E0404\""), "{text}");
    assert!(
        text.contains("\"message\":\"argument 1 to `inspect` is `i32` but expected `i64`\""),
        "{text}"
    );
    assert!(text.contains("\"line\":8"), "{text}");
    assert!(text.contains("\"expected\":\"i64\""), "{text}");
    assert!(text.contains("\"found\":\"i32\""), "{text}");
    assert!(text.contains("\"suggestions\":[]"), "{text}");

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomoc_check_emits_json_for_int_not_builtin() {
    let root = temp_test_root("nomoc-json-int-not-builtin");
    reset_dir(&root);
    let source = root.join("int-not-builtin.nomo");
    fs::write(
        &source,
        "package app.main\n\nfn main() -> void {\n    let count: int = 1\n}\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomoc"))
        .arg("check")
        .arg(&source)
        .arg("--json-errors")
        .output()
        .unwrap();

    assert!(!output.status.success());
    let text = String::from_utf8_lossy(&output.stderr);
    let text = text.trim();
    assert_single_json_object(text);
    assert!(text.contains("\"status\":\"error\""), "{text}");
    assert!(text.contains("\"error_code\":\"E0403\""), "{text}");
    assert!(
        text.contains(
            "\"message\":\"`int` is not a v0.1 builtin type; use `i64` or an explicit-width integer type (`i32`, `u32`, `u64`)\""
        ),
        "{text}"
    );
    assert!(text.contains("\"line\":4"), "{text}");
    assert!(text.contains("\"suggestions\":[]"), "{text}");

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomoc_check_emits_json_for_assignment_to_immutable_variable() {
    let root = temp_test_root("nomoc-json-immutable-assign");
    reset_dir(&root);
    let source = root.join("immutable-assign.nomo");
    fs::write(
        &source,
        "package app.main\n\nfn main() -> void {\n    let count: i32 = 1\n    count = 2\n}\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomoc"))
        .arg("check")
        .arg(&source)
        .arg("--json-errors")
        .output()
        .unwrap();

    assert!(!output.status.success());
    let text = String::from_utf8_lossy(&output.stderr);
    let text = text.trim();
    assert_single_json_object(text);
    assert!(text.contains("\"status\":\"error\""), "{text}");
    assert!(text.contains("\"error_code\":\"E0501\""), "{text}");
    assert!(
        text.contains("\"message\":\"cannot assign to immutable variable `count`\""),
        "{text}"
    );
    assert!(text.contains("\"line\":5"), "{text}");
    assert!(text.contains("\"suggestions\":[]"), "{text}");

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomoc_check_emits_json_for_assignment_to_immutable_parameter() {
    let root = temp_test_root("nomoc-json-immutable-param-assign");
    reset_dir(&root);
    let source = root.join("immutable-param-assign.nomo");
    fs::write(
        &source,
        "package app.main\n\nfn bump(value: i32) -> i32 {\n    value = value + 1\n    return value\n}\n\nfn main() -> void {\n}\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomoc"))
        .arg("check")
        .arg(&source)
        .arg("--json-errors")
        .output()
        .unwrap();

    assert!(!output.status.success());
    let text = String::from_utf8_lossy(&output.stderr);
    let text = text.trim();
    assert_single_json_object(text);
    assert!(text.contains("\"status\":\"error\""), "{text}");
    assert!(text.contains("\"error_code\":\"E0501\""), "{text}");
    assert!(
        text.contains("\"message\":\"cannot assign to immutable parameter `value`\""),
        "{text}"
    );
    assert!(text.contains("\"line\":4"), "{text}");
    assert!(text.contains("\"suggestions\":[]"), "{text}");

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomoc_check_emits_json_for_field_assignment_to_immutable_parameter() {
    let root = temp_test_root("nomoc-json-immutable-param-field-assign");
    reset_dir(&root);
    let source = root.join("immutable-param-field-assign.nomo");
    fs::write(
        &source,
        "package app.main\n\nstruct Counter {\n    value: i32\n}\n\nfn bump(counter: Counter) -> void {\n    counter.value = counter.value + 1\n}\n\nfn main() -> void {\n}\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomoc"))
        .arg("check")
        .arg(&source)
        .arg("--json-errors")
        .output()
        .unwrap();

    assert!(!output.status.success());
    let text = String::from_utf8_lossy(&output.stderr);
    let text = text.trim();
    assert_single_json_object(text);
    assert!(text.contains("\"status\":\"error\""), "{text}");
    assert!(text.contains("\"error_code\":\"E0501\""), "{text}");
    assert!(
        text.contains("\"message\":\"cannot assign to field of immutable parameter `counter`\""),
        "{text}"
    );
    assert!(text.contains("\"line\":8"), "{text}");
    assert!(text.contains("\"suggestions\":[]"), "{text}");

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomoc_check_emits_json_for_mutating_array_method_on_immutable_parameter() {
    let root = temp_test_root("nomoc-json-array-mut-immutable-param");
    reset_dir(&root);
    let source = root.join("array-mut-immutable-param.nomo");
    fs::write(
        &source,
        "package app.main\n\nimport std.array\n\nfn push_one(items: Array<i32>) -> void {\n    items.push(1)\n}\n\nfn main() -> void {\n}\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomoc"))
        .arg("check")
        .arg(&source)
        .arg("--json-errors")
        .output()
        .unwrap();

    assert!(!output.status.success());
    let text = String::from_utf8_lossy(&output.stderr);
    let text = text.trim();
    assert_single_json_object(text);
    assert!(text.contains("\"status\":\"error\""), "{text}");
    assert!(text.contains("\"error_code\":\"E0501\""), "{text}");
    assert!(
        text.contains(
            "\"message\":\"cannot call mutating Array method on immutable parameter `items`\""
        ),
        "{text}"
    );
    assert!(text.contains("\"line\":6"), "{text}");
    assert!(text.contains("\"suggestions\":[]"), "{text}");

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomoc_check_emits_json_for_missing_mut_call_argument() {
    let root = temp_test_root("nomoc-json-missing-mut-call-arg");
    reset_dir(&root);
    let source = root.join("missing-mut-call-arg.nomo");
    fs::write(
        &source,
        "package app.main\n\nfn inspect(mut value: i64) -> i64 {\n    return value\n}\n\nfn main() -> void {\n    let mut count: i64 = 41\n    let answer: i64 = inspect(count)\n}\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomoc"))
        .arg("check")
        .arg(&source)
        .arg("--json-errors")
        .output()
        .unwrap();

    assert!(!output.status.success());
    let text = String::from_utf8_lossy(&output.stderr);
    let text = text.trim();
    assert_single_json_object(text);
    assert!(text.contains("\"status\":\"error\""), "{text}");
    assert!(text.contains("\"error_code\":\"E0500\""), "{text}");
    assert!(
        text.contains("\"message\":\"argument 1 to `inspect` must be passed as `mut`\""),
        "{text}"
    );
    assert!(text.contains("\"line\":9"), "{text}");
    assert!(text.contains("\"suggestions\":[]"), "{text}");

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomoc_check_emits_json_for_extra_mut_call_argument() {
    let root = temp_test_root("nomoc-json-extra-mut-call-arg");
    reset_dir(&root);
    let source = root.join("extra-mut-call-arg.nomo");
    fs::write(
        &source,
        "package app.main\n\nfn inspect(value: i64) -> i64 {\n    return value\n}\n\nfn main() -> void {\n    let mut count: i64 = 41\n    let answer: i64 = inspect(mut count)\n}\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomoc"))
        .arg("check")
        .arg(&source)
        .arg("--json-errors")
        .output()
        .unwrap();

    assert!(!output.status.success());
    let text = String::from_utf8_lossy(&output.stderr);
    let text = text.trim();
    assert_single_json_object(text);
    assert!(text.contains("\"status\":\"error\""), "{text}");
    assert!(text.contains("\"error_code\":\"E0504\""), "{text}");
    assert!(
        text.contains("\"message\":\"argument 1 to `inspect` is not declared `mut`\""),
        "{text}"
    );
    assert!(text.contains("\"line\":9"), "{text}");
    assert!(text.contains("\"suggestions\":[]"), "{text}");

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomoc_check_emits_json_for_immutable_variable_as_mut_call_argument() {
    let root = temp_test_root("nomoc-json-immutable-mut-call-arg");
    reset_dir(&root);
    let source = root.join("immutable-mut-call-arg.nomo");
    fs::write(
        &source,
        "package app.main\n\nfn inspect(mut value: i64) -> i64 {\n    return value\n}\n\nfn main() -> void {\n    let count: i64 = 41\n    let answer: i64 = inspect(mut count)\n}\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomoc"))
        .arg("check")
        .arg(&source)
        .arg("--json-errors")
        .output()
        .unwrap();

    assert!(!output.status.success());
    let text = String::from_utf8_lossy(&output.stderr);
    let text = text.trim();
    assert_single_json_object(text);
    assert!(text.contains("\"status\":\"error\""), "{text}");
    assert!(text.contains("\"error_code\":\"E0501\""), "{text}");
    assert!(
        text.contains("\"message\":\"cannot pass immutable variable `count` as `mut`\""),
        "{text}"
    );
    assert!(text.contains("\"line\":9"), "{text}");
    assert!(text.contains("\"suggestions\":[]"), "{text}");

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn nomoc_check_emits_json_for_duplicate_mut_borrow_in_call() {
    let root = temp_test_root("nomoc-json-duplicate-mut-borrow");
    reset_dir(&root);
    let source = root.join("duplicate-mut-borrow.nomo");
    fs::write(
        &source,
        "package app.main\n\nfn combine(mut left: i64, mut right: i64) -> i64 {\n    return left + right\n}\n\nfn main() -> void {\n    let mut count: i64 = 41\n    let answer: i64 = combine(mut count, mut count)\n}\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_nomoc"))
        .arg("check")
        .arg(&source)
        .arg("--json-errors")
        .output()
        .unwrap();

    assert!(!output.status.success());
    let text = String::from_utf8_lossy(&output.stderr);
    let text = text.trim();
    assert_single_json_object(text);
    assert!(text.contains("\"status\":\"error\""), "{text}");
    assert!(text.contains("\"error_code\":\"E0502\""), "{text}");
    assert!(
        text.contains(
            "\"message\":\"mutable borrow `count` conflicts with active mutable borrow `count` in this call\""
        ),
        "{text}"
    );
    assert!(text.contains("\"line\":9"), "{text}");
    assert!(text.contains("\"suggestions\":[]"), "{text}");

    fs::remove_dir_all(&root).unwrap();
}

fn invalid_source() -> &'static str {
    "package app.main\n\nfn main() -> void {\n    let value: i32 = \"bad\"\n}\n"
}

fn invalid_lexical_source() -> &'static str {
    "package app.main\n\nfn main() -> void {\n    @\n}\n"
}

fn invalid_semicolon_source() -> &'static str {
    "package app.main\n\nfn main() -> void {\n    let value: i32 = 1;\n}\n"
}

fn invalid_parser_source() -> &'static str {
    "package app.main import std.io\n\nfn main() -> void {\n}\n"
}

fn assert_json_diagnostic(stderr: &[u8], source: &Path) {
    let text = String::from_utf8_lossy(stderr);
    let text = text.trim();
    assert_single_json_object(text);
    assert_eq!(
        text,
        format!(
            "{{\"status\":\"error\",\"error_code\":\"E0404\",\"severity\":\"error\",\"message\":\"cannot initialize `value` as `i32` from `string`\",\"source\":{{\"file\":\"{}\",\"line\":4,\"column\":5,\"length\":1,\"text\":\"    let value: i32 = \\\"bad\\\"\"}},\"expected\":\"i32\",\"found\":\"string\",\"suggestions\":[]}}",
            source.display()
        )
    );
}

fn assert_lexical_json_diagnostic(stderr: &[u8], source: &Path) {
    let text = String::from_utf8_lossy(stderr);
    let text = text.trim();
    assert_single_json_object(text);
    assert_eq!(
        text,
        format!(
            "{{\"status\":\"error\",\"error_code\":\"E0102\",\"severity\":\"error\",\"message\":\"unexpected character `@`\",\"source\":{{\"file\":\"{}\",\"line\":4,\"column\":5,\"length\":1,\"text\":\"    @\"}},\"suggestions\":[]}}",
            source.display()
        )
    );
}

fn assert_single_json_object(text: &str) {
    assert!(
        text.starts_with('{'),
        "stderr was not a JSON object: {text}"
    );
    assert!(text.ends_with('}'), "stderr was not a JSON object: {text}");

    let mut depth = 0usize;
    let mut in_string = false;
    let mut escape = false;
    let mut closed_at = None;

    for (index, ch) in text.char_indices() {
        if in_string {
            if escape {
                escape = false;
            } else if ch == '\\' {
                escape = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }

        match ch {
            '"' => in_string = true,
            '{' => depth += 1,
            '}' => {
                assert!(depth > 0, "JSON object closed before it opened: {text}");
                depth -= 1;
                if depth == 0 {
                    closed_at = Some(index);
                }
            }
            _ => {}
        }
    }

    assert!(!in_string, "JSON object ended inside a string: {text}");
    assert_eq!(depth, 0, "JSON object braces are not balanced: {text}");
    assert_eq!(
        closed_at,
        text.char_indices().last().map(|(index, _)| index),
        "stderr contained trailing data after JSON object: {text}"
    );
}

fn reset_dir(path: &Path) {
    if path.exists() {
        fs::remove_dir_all(path).unwrap();
    }
    fs::create_dir_all(path).unwrap();
}

fn temp_test_root(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("nomo-cli-json-test-{name}-{}", std::process::id()))
}
