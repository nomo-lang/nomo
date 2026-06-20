use crate::compiler::{check_source, compile_source_to_c};
use crate::diagnostic::Diagnostic;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone)]
pub struct Project {
    pub root: PathBuf,
    pub name: String,
    pub main: PathBuf,
}

#[derive(Debug)]
pub enum BuildError {
    Diagnostic(Diagnostic),
    Message(String),
}

impl BuildError {
    pub fn human(&self) -> String {
        match self {
            BuildError::Diagnostic(diagnostic) => diagnostic.human(),
            BuildError::Message(message) => message.clone(),
        }
    }
}

pub fn create_project(root: &Path, name: &str) -> Result<Project, String> {
    if !is_package_name(name) {
        return Err(format!("invalid project name `{name}`"));
    }
    let project_root = root.join(name);
    if project_root.exists() {
        return Err(format!(
            "destination already exists: {}",
            project_root.display()
        ));
    }
    fs::create_dir_all(project_root.join("src")).map_err(|err| err.to_string())?;
    fs::write(
        project_root.join("nomo.toml"),
        format!(
            "[package]\nname = \"{name}\"\nversion = \"0.1.0\"\n\n[dependencies]\nstd = \"0.1.0\"\n"
        ),
    )
    .map_err(|err| err.to_string())?;
    fs::write(
        project_root.join("src/main.nomo"),
        "package app.main\n\nimport std.io\n\nfn greeting() -> string {\n    return \"Hello, Nomo\"\n}\n\nfn main() -> void {\n    let message: string = greeting()\n    io.println(message)\n}\n",
    )
    .map_err(|err| err.to_string())?;
    discover_project(&project_root)
}

pub fn discover_project(path: &Path) -> Result<Project, String> {
    let source_file = path.extension().and_then(|ext| ext.to_str()) == Some("nomo");
    let search_root = if source_file {
        path.parent()
            .ok_or_else(|| format!("source file has no parent: {}", path.display()))?
    } else {
        path
    };
    let root = find_manifest_root(search_root).ok_or_else(|| {
        format!(
            "could not find nomo.toml for {}; use `nomoc` for standalone source files",
            path.display()
        )
    })?;
    let manifest = root.join("nomo.toml");
    let main = if source_file {
        path.to_path_buf()
    } else {
        root.join("src/main.nomo")
    };
    let name = if manifest.exists() {
        parse_project_name(&fs::read_to_string(&manifest).map_err(|err| err.to_string())?)
            .unwrap_or_else(|| {
                root.file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string()
            })
    } else {
        root.file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string()
    };
    Ok(Project { root, name, main })
}

fn find_manifest_root(start: &Path) -> Option<PathBuf> {
    for candidate in start.ancestors() {
        if candidate.join("nomo.toml").exists() {
            return Some(candidate.to_path_buf());
        }
    }
    None
}

pub fn check_project(project: &Project) -> Result<(), Diagnostic> {
    check_source(&project.main).map(|_| ())
}

pub fn build_project(project: &Project, emit_c_only: bool) -> Result<PathBuf, String> {
    build_project_with_diagnostics(project, emit_c_only).map_err(|err| err.human())
}

pub fn build_project_with_diagnostics(
    project: &Project,
    emit_c_only: bool,
) -> Result<PathBuf, BuildError> {
    let c = compile_source_to_c(&project.main).map_err(BuildError::Diagnostic)?;
    let c_dir = project.root.join("build/c");
    let bin_dir = project.root.join("build/bin");
    fs::create_dir_all(&c_dir).map_err(|err| BuildError::Message(err.to_string()))?;
    fs::create_dir_all(&bin_dir).map_err(|err| BuildError::Message(err.to_string()))?;

    let c_path = c_dir.join("main.c");
    fs::write(&c_path, c).map_err(|err| BuildError::Message(err.to_string()))?;
    if emit_c_only {
        return Ok(c_path);
    }

    let bin_path = bin_dir.join(&project.name);
    let output = Command::new("cc")
        .arg("-std=c99")
        .arg(&c_path)
        .arg("-o")
        .arg(&bin_path)
        .output()
        .map_err(|err| BuildError::Message(format!("failed to run cc: {err}")))?;
    if !output.status.success() {
        return Err(BuildError::Message(format!(
            "cc failed:\n{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    Ok(bin_path)
}

pub fn clean_project(project: &Project) -> Result<PathBuf, String> {
    let build_dir = project.root.join("build");
    if build_dir.exists() {
        fs::remove_dir_all(&build_dir).map_err(|err| err.to_string())?;
    }
    Ok(build_dir)
}

pub fn run_project(project: &Project) -> Result<i32, String> {
    run_project_with_args(project, &[])
}

pub fn run_project_with_args(project: &Project, args: &[String]) -> Result<i32, String> {
    run_project_with_args_and_diagnostics(project, args).map_err(|err| err.human())
}

pub fn run_project_with_args_and_diagnostics(
    project: &Project,
    args: &[String],
) -> Result<i32, BuildError> {
    let bin = build_project_with_diagnostics(project, false)?;
    let bin = if bin.is_absolute() {
        bin
    } else {
        std::env::current_dir()
            .map_err(|err| BuildError::Message(err.to_string()))?
            .join(bin)
    };
    let status = Command::new(&bin)
        .current_dir(&project.root)
        .args(args)
        .status()
        .map_err(|err| BuildError::Message(format!("failed to run {}: {err}", bin.display())))?;
    Ok(status.code().unwrap_or(1))
}

fn parse_project_name(manifest: &str) -> Option<String> {
    let mut in_package = false;
    for line in manifest.lines() {
        let line = line.trim();
        if line.starts_with('[') && line.ends_with(']') {
            in_package = line == "[package]";
            continue;
        }
        if !in_package {
            continue;
        }
        let Some(value) = line.strip_prefix("name") else {
            continue;
        };
        let Some(value) = value.trim_start().strip_prefix('=') else {
            continue;
        };
        let Some(value) = value.trim_start().strip_prefix('"') else {
            continue;
        };
        let end = value.find('"')?;
        return Some(value[..end].to_string());
    }
    None
}

fn is_package_name(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|ch| ch == '-' || ch == '_' || ch.is_ascii_alphanumeric())
        && value
            .chars()
            .next()
            .is_some_and(|ch| ch == '_' || ch.is_ascii_alphabetic())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_manifest_name() {
        let manifest = "[package]\nname = \"demo\"\nversion = \"0.1.0\"\n";
        assert_eq!(parse_project_name(manifest), Some("demo".to_string()));
    }

    #[test]
    fn parses_manifest_name_only_from_package_section() {
        let manifest = "[dependencies]\nname = \"dep\"\n\n[package]\nname = \"demo\"\n";
        assert_eq!(parse_project_name(manifest), Some("demo".to_string()));
    }

    #[test]
    fn ignores_name_outside_package_section() {
        let manifest = "[dependencies]\nname = \"dep\"\n";
        assert_eq!(parse_project_name(manifest), None);
    }

    #[test]
    fn falls_back_to_directory_name_when_package_name_is_missing() {
        let root = temp_test_root("manifest-fallback");
        if root.exists() {
            fs::remove_dir_all(&root).unwrap();
        }
        let project_root = root.join("fallback_demo");
        fs::create_dir_all(project_root.join("src")).unwrap();
        fs::write(
            project_root.join("nomo.toml"),
            "[dependencies]\nname = \"dep\"\n",
        )
        .unwrap();
        fs::write(project_root.join("src/main.nomo"), "package app.main\n").unwrap();

        let project = discover_project(&project_root).unwrap();

        assert_eq!(project.name, "fallback_demo");
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn discovers_project_root_from_source_file_path() {
        let root = temp_test_root("discover-source-file");
        if root.exists() {
            fs::remove_dir_all(&root).unwrap();
        }
        fs::create_dir_all(&root).unwrap();
        let project = create_project(&root, "source_demo").unwrap();

        let discovered = discover_project(&project.root.join("src/main.nomo")).unwrap();

        assert_eq!(discovered.root, project.root);
        assert_eq!(discovered.name, "source_demo");
        assert_eq!(discovered.main, project.root.join("src/main.nomo"));
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn builds_source_file_path_under_project_root() {
        let root = temp_test_root("source-file-build");
        if root.exists() {
            fs::remove_dir_all(&root).unwrap();
        }
        fs::create_dir_all(&root).unwrap();
        let project = create_project(&root, "source_build_demo").unwrap();
        let discovered = discover_project(&project.root.join("src/main.nomo")).unwrap();

        let artifact = build_project(&discovered, true).unwrap();

        assert_eq!(artifact, project.root.join("build/c/main.c"));
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn cleans_project_build_directory() {
        let root = temp_test_root("clean-project");
        if root.exists() {
            fs::remove_dir_all(&root).unwrap();
        }
        fs::create_dir_all(&root).unwrap();
        let project = create_project(&root, "clean_demo").unwrap();
        let artifact = build_project(&project, true).unwrap();
        assert!(artifact.exists());

        let cleaned = clean_project(&project).unwrap();

        assert_eq!(cleaned, project.root.join("build"));
        assert!(!cleaned.exists());
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn rejects_directory_without_manifest() {
        let root = temp_test_root("missing-manifest-dir");
        if root.exists() {
            fs::remove_dir_all(&root).unwrap();
        }
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("src/main.nomo"), "package app.main\n").unwrap();

        let err = discover_project(&root).unwrap_err();

        assert!(err.contains("nomo.toml"));
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn rejects_standalone_source_file_without_manifest() {
        let root = temp_test_root("standalone-source");
        if root.exists() {
            fs::remove_dir_all(&root).unwrap();
        }
        fs::create_dir_all(&root).unwrap();
        let source = root.join("main.nomo");
        fs::write(&source, "package app.main\n").unwrap();

        let err = discover_project(&source).unwrap_err();

        assert!(err.contains("nomo.toml"));
        assert!(err.contains("nomoc"));
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn runs_project_with_forwarded_args() {
        let root = temp_test_root("forwarded-args");
        if root.exists() {
            fs::remove_dir_all(&root).unwrap();
        }
        fs::create_dir_all(&root).unwrap();
        let project = create_project(&root, "args_demo").unwrap();
        fs::write(
            project.root.join("src/main.nomo"),
            r#"package app.main

import std.env
import std.io
import std.array

fn main() -> void {
    let args: Array<string> = env.args()
    let size: u64 = args.len()
    let status: string = if size == 2 {
        "ok"
    } else {
        panic("expected one forwarded arg")
    }
    io.println(status)
}
"#,
        )
        .unwrap();

        let status = run_project_with_args(&project, &["hello".to_string()]).unwrap();
        assert_eq!(status, 0);
        fs::remove_dir_all(&root).unwrap();
    }

    fn temp_test_root(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("nomo-project-test-{name}-{}", std::process::id()))
    }
}
