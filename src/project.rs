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
    let root = if path.is_file() {
        path.parent()
            .ok_or_else(|| format!("source file has no parent: {}", path.display()))?
            .to_path_buf()
    } else {
        path.to_path_buf()
    };
    let manifest = root.join("nomo.toml");
    let main = if path.extension().and_then(|ext| ext.to_str()) == Some("nomo") {
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

pub fn check_project(project: &Project) -> Result<(), Diagnostic> {
    check_source(&project.main).map(|_| ())
}

pub fn build_project(project: &Project, emit_c_only: bool) -> Result<PathBuf, String> {
    let c = compile_source_to_c(&project.main).map_err(|diag| diag.human())?;
    let c_dir = project.root.join("build/c");
    let bin_dir = project.root.join("build/bin");
    fs::create_dir_all(&c_dir).map_err(|err| err.to_string())?;
    fs::create_dir_all(&bin_dir).map_err(|err| err.to_string())?;

    let c_path = c_dir.join("main.c");
    fs::write(&c_path, c).map_err(|err| err.to_string())?;
    if emit_c_only {
        return Ok(c_path);
    }

    let bin_path = bin_dir.join(&project.name);
    let output = Command::new("cc")
        .arg(&c_path)
        .arg("-o")
        .arg(&bin_path)
        .output()
        .map_err(|err| format!("failed to run cc: {err}"))?;
    if !output.status.success() {
        return Err(format!(
            "cc failed:\n{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(bin_path)
}

pub fn run_project(project: &Project) -> Result<i32, String> {
    run_project_with_args(project, &[])
}

pub fn run_project_with_args(project: &Project, args: &[String]) -> Result<i32, String> {
    let bin = build_project(project, false)?;
    let status = Command::new(&bin)
        .args(args)
        .status()
        .map_err(|err| format!("failed to run {}: {err}", bin.display()))?;
    Ok(status.code().unwrap_or(1))
}

fn parse_project_name(manifest: &str) -> Option<String> {
    manifest.lines().find_map(|line| {
        let line = line.trim();
        let value = line.strip_prefix("name")?.trim_start();
        let value = value.strip_prefix('=')?.trim_start();
        let value = value.strip_prefix('"')?;
        let end = value.find('"')?;
        Some(value[..end].to_string())
    })
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
    fn runs_project_with_forwarded_args() {
        let root = std::env::temp_dir().join(format!("nomo-project-test-{}", std::process::id()));
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
}
