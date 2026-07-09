use super::cli_common::{filter_projects_by_package, validate_project_package};
use nomo::doc::{
    collect_project_docs, generate_project_docs, generate_std_docs, render_packages_json,
    std_doc_package, write_doc_index,
};
use nomo::project::{discover_project, discover_workspace, project_package_id};
use std::env;
use std::path::{Path, PathBuf};
use std::process;

pub(super) fn run_doc_command(args: Vec<String>) -> Result<(), String> {
    let (path, path_provided, workspace, package, include_std, open, json, output) =
        parse_doc_args(args)?;
    if json && open {
        return Err("--open is not valid with --json".to_string());
    }
    let mut packages = Vec::new();
    let mut output_root = output;
    if workspace {
        let workspace = discover_workspace(&path)?;
        output_root = Some(output_root.unwrap_or_else(|| workspace.root.join("build/doc")));
        let mut projects = workspace.members;
        if let Some(package) = package.as_deref() {
            projects = filter_projects_by_package(projects, package)?;
        }
        for project in projects {
            let package_id = project_package_id(&project)?;
            if json {
                packages
                    .push(collect_project_docs(&project, &package_id).map_err(|err| err.human())?);
            } else {
                packages.push(
                    generate_project_docs(&project, output_root.as_ref().expect("doc output root"))
                        .map_err(|err| err.human())?,
                );
            }
        }
    } else if path_provided || !include_std {
        let project = discover_project(&path)?;
        if let Some(package) = package.as_deref() {
            validate_project_package(&project, package)?;
        }
        output_root = Some(output_root.unwrap_or_else(|| project.root.join("build/doc")));
        let package_id = project_package_id(&project)?;
        if json {
            packages.push(collect_project_docs(&project, &package_id).map_err(|err| err.human())?);
        } else {
            packages.push(
                generate_project_docs(&project, output_root.as_ref().expect("doc output root"))
                    .map_err(|err| err.human())?,
            );
        }
    }
    if include_std {
        output_root = Some(output_root.unwrap_or_else(|| {
            env::current_dir()
                .unwrap_or_else(|_| PathBuf::from("."))
                .join("build/doc")
        }));
        if json {
            packages.push(std_doc_package());
        } else {
            packages.push(
                generate_std_docs(output_root.as_ref().expect("doc output root"))
                    .map_err(|err| err.human())?,
            );
        }
    }
    if json {
        println!("{}", render_packages_json(&packages));
        return Ok(());
    }
    let output_root = output_root.unwrap_or_else(|| path.join("build/doc"));
    let docs = write_doc_index(&output_root, &packages).map_err(|err| err.human())?;
    println!("documented {}", docs.display());
    if open {
        open_doc_index(&docs)?;
    }
    Ok(())
}

pub(super) fn parse_doc_args(
    args: Vec<String>,
) -> Result<
    (
        PathBuf,
        bool,
        bool,
        Option<String>,
        bool,
        bool,
        bool,
        Option<PathBuf>,
    ),
    String,
> {
    let usage = "usage: nomo doc [path] [--workspace] [--package <package>] [--std] [--open] [--json] [--output <dir>]";
    let mut workspace = false;
    let mut package = None;
    let mut include_std = false;
    let mut open = false;
    let mut json = false;
    let mut output = None;
    let mut path = None;
    let mut index = 0;
    while let Some(arg) = args.get(index) {
        if arg == "--workspace" {
            workspace = true;
        } else if let Some(value) = arg.strip_prefix("--package=") {
            if value.is_empty() {
                return Err("--package requires a package id or name".to_string());
            }
            package = Some(value.to_string());
        } else if arg == "--package" {
            index += 1;
            let Some(value) = args.get(index) else {
                return Err("--package requires a package id or name".to_string());
            };
            package = Some(value.clone());
        } else if arg == "--std" {
            include_std = true;
        } else if arg == "--open" {
            open = true;
        } else if arg == "--json" {
            json = true;
        } else if let Some(value) = arg.strip_prefix("--output=") {
            if value.is_empty() {
                return Err("--output requires a directory".to_string());
            }
            output = Some(PathBuf::from(value));
        } else if arg == "--output" {
            index += 1;
            let Some(value) = args.get(index) else {
                return Err("--output requires a directory".to_string());
            };
            output = Some(PathBuf::from(value));
        } else if path.is_none() {
            path = Some(PathBuf::from(arg));
        } else {
            return Err(usage.to_string());
        }
        index += 1;
    }
    let path_provided = path.is_some();
    Ok((
        path.unwrap_or(env::current_dir().map_err(|err| err.to_string())?),
        path_provided,
        workspace,
        package,
        include_std,
        open,
        json,
        output,
    ))
}

fn open_doc_index(path: &Path) -> Result<(), String> {
    let index = path.join("index.html");
    if !index.is_file() {
        return Err(format!("failed to open missing {}", index.display()));
    }
    if env::var_os("NOMO_DOC_OPEN").as_deref() == Some(std::ffi::OsStr::new("0")) {
        return Ok(());
    }
    let status = if cfg!(target_os = "macos") {
        process::Command::new("open").arg(&index).status()
    } else if cfg!(target_os = "windows") {
        process::Command::new("cmd")
            .args(["/C", "start", ""])
            .arg(&index)
            .status()
    } else {
        process::Command::new("xdg-open").arg(&index).status()
    }
    .map_err(|err| format!("failed to open {}: {err}", index.display()))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("failed to open {}", index.display()))
    }
}
