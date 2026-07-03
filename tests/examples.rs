use nomo::project::{build_project, check_project, discover_project};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const REQUIRED_V0_1_EXAMPLES: &[&str] = &[
    "hello",
    "args",
    "read_file",
    "result_chain",
    "struct_methods",
    "array_basic",
    "mut_field_borrow",
];

#[test]
fn examples_check_and_run() {
    for example in example_projects() {
        clean_example_build_dir(&example);
        assert_cli_check(&example);
        assert_cli_build_emit_c(&example);
        assert_cli_run(&example);

        let project = discover_project(&example)
            .unwrap_or_else(|err| panic!("failed to discover {}: {err}", example.display()));
        check_project(&project).unwrap_or_else(|diag| {
            panic!("failed to check {}:\n{}", example.display(), diag.human())
        });
        let bin = build_project(&project, false)
            .unwrap_or_else(|err| panic!("failed to build {}: {err}", example.display()));
        let output = Command::new(&bin)
            .current_dir(&project.root)
            .env("NOMO_EXAMPLE_ENV", "env get ok")
            .output()
            .unwrap_or_else(|err| panic!("failed to run {}: {err}", bin.display()));
        assert!(
            output.status.success(),
            "example exited unsuccessfully: {}\nstdout:\n{}\nstderr:\n{}",
            example.display(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert_example_output(&example, &output.stdout, &output.stderr);
        clean_example_build_dir(&example);
    }
}

fn example_projects() -> Vec<PathBuf> {
    let examples_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples");
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
    let output = Command::new(env!("CARGO_BIN_EXE_nomo"))
        .arg("run")
        .arg(example)
        .env("NOMO_EXAMPLE_ENV", "env get ok")
        .output()
        .unwrap_or_else(|err| panic!("failed to run nomo run {}: {err}", example.display()));
    assert!(
        output.status.success(),
        "nomo run failed for {}\nstdout:\n{}\nstderr:\n{}",
        example.display(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_example_output(example, &output.stdout, &output.stderr);
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

fn clean_example_build_dir(example: &Path) {
    let build_dir = example.join("build");
    if build_dir.exists() {
        fs::remove_dir_all(&build_dir)
            .unwrap_or_else(|err| panic!("failed to clean {}: {err}", build_dir.display()));
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
        "c_keywords" => "c keywords ok\n",
        "const" => "hello\nhello\nhello\nconst primitives ok\n",
        "defer" => {
            "working\ncontinue cleanup\nbreak cleanup\nblock\ninner\nafter block\ninner early\nouter early\nclose\nflush\nlog\n"
        }
        "defer_question" => "defer before ? error\nfail\n",
        "enum_struct_payload" => "a@nomo.dev\n",
        "env_get" => "env get ok\n",
        "file_handle" => "file handle ok\n",
        "generic_function" => "generic function ok\n",
        "generic_enum" => "generic enum ok\n",
        "generic_struct" => "generic struct ok\n",
        "hello" => "Hello, Nomo\n",
        "if_let" => "if let ok\n",
        "io_stderr" => "stdout ok\n",
        "let_else" => "let else ok\n",
        "loops" => "counted\ncounted\ncounted\na\nb\nonce\n",
        "mut_field_borrow" => "mut field borrow ok\n",
        "mut_methods" => "mut method ok\n",
        "newline_dot" => "newline dot ok\n",
        "option_result_lang_items" => "lang items ok\n",
        "package_path" => "package path ok\n",
        "prelude_variants" => "prelude variants ok\n",
        "prelude_shadow" => "shadow ok / qualified ok\n",
        "primitives" => "primitives ok\n",
        "pub_visibility" => "pub visibility ok\n",
        "read_file" => "file ok\n",
        "result_chain" => "result ok\n",
        "result_main" => "result main ok\n",
        "result_map_err" => "mapped err ok\n",
        "specific_array_new" => "specific array new ok\n",
        "specific_import" => "specific import ok\n",
        "specific_type_import" => "specific type import ok\n",
        "specific_value_import" => "specific value import ok\n",
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
        "io_stderr" => "stderr ok\n",
        _ => "",
    }
}
