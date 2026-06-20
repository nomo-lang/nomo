use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const NOMO_HELP: &str = "nomo 0.1.0\n\nCommands:\n  nomo new <name>\n  nomo check [path] [--json-errors]\n  nomo build [path] [--emit-c] [--json-errors]\n  nomo run [path] [--json-errors] [-- args...]\n  nomo clean [path]\n\n";

const NOMOC_HELP: &str = "nomoc 0.1.0\n\nCommands:\n  nomoc check <source.nomo> [--json-errors]\n  nomoc build <source.nomo> [--emit-c] [--out path] [--json-errors]\n\n";

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
        "[package]\nname = \"hello\"\nversion = \"0.1.0\"\n\n[dependencies]\nstd = \"0.1.0\"\n"
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

fn reset_dir(path: &Path) {
    if path.exists() {
        fs::remove_dir_all(path).unwrap();
    }
    fs::create_dir_all(path).unwrap();
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
