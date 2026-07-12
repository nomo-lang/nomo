use nomo::project::{
    BuildError, DependencyResolutionOptions, Project, build_project_with_options, check_project,
    clean_project, discover_project, discover_workspace, project_package_id,
    run_project_with_args_and_diagnostics, run_standalone_script_with_args_and_diagnostics,
};
use std::env;
use std::path::{Path, PathBuf};

pub(super) fn run_check_command(args: Vec<String>) -> Result<(), String> {
    let (path, json, workspace) = parse_path_json_workspace(
        args,
        "usage: nomo check [path] [--json-errors] [--workspace]",
    )?;
    if workspace {
        for project in discover_workspace(&path)?.members {
            match check_project(&project) {
                Ok(()) => println!("checked {}", project.main.display()),
                Err(diag) if json => return Err(diag.json()),
                Err(diag) => return Err(diag.human()),
            }
        }
        Ok(())
    } else {
        let project = discover_project(&path)?;
        match check_project(&project) {
            Ok(()) => {
                println!("checked {}", project.main.display());
                Ok(())
            }
            Err(diag) if json => Err(diag.json()),
            Err(diag) => Err(diag.human()),
        }
    }
}

pub(super) fn run_build_command(args: Vec<String>) -> Result<(), String> {
    let (path, emit_c, json, workspace, deps) = parse_build_args(
        args,
        "usage: nomo build [path] [--emit-c] [--json-errors] [--workspace] [--locked] [--offline] [--frozen]",
    )?;
    if workspace {
        for project in discover_workspace(&path)?.members {
            let artifact = match build_project_with_options(&project, emit_c, deps) {
                Ok(artifact) => artifact,
                Err(BuildError::Diagnostic(diag)) if json => return Err(diag.json()),
                Err(err) => return Err(err.human()),
            };
            println!("built {}", artifact.display());
        }
    } else {
        let project = discover_project(&path)?;
        let artifact = match build_project_with_options(&project, emit_c, deps) {
            Ok(artifact) => artifact,
            Err(BuildError::Diagnostic(diag)) if json => return Err(diag.json()),
            Err(err) => return Err(err.human()),
        };
        println!("built {}", artifact.display());
    }
    Ok(())
}

pub(super) fn run_run_command(args: Vec<String>) -> Result<(), String> {
    let (path, program_args, json) = parse_run_args(args)?;
    let code = match discover_project(&path) {
        Ok(project) => match run_project_with_args_and_diagnostics(&project, &program_args) {
            Ok(code) => code,
            Err(BuildError::Diagnostic(diag)) if json => return Err(diag.json()),
            Err(err) => return Err(err.human()),
        },
        Err(err) if is_nomo_source_file(&path) && is_missing_manifest_error(&err) => {
            match run_standalone_script_with_args_and_diagnostics(&path, &program_args) {
                Ok(code) => code,
                Err(BuildError::Diagnostic(diag)) if json => return Err(diag.json()),
                Err(err) => return Err(err.human()),
            }
        }
        Err(err) => return Err(err),
    };
    if code == 0 {
        Ok(())
    } else {
        Err(format!("program exited with status {code}"))
    }
}

pub(super) fn run_clean_command(args: Vec<String>) -> Result<(), String> {
    let path = parse_optional_path(args, "usage: nomo clean [path]")?;
    let project = discover_project(&path)?;
    let cleaned = clean_project(&project)?;
    println!("cleaned {}", cleaned.display());
    Ok(())
}

pub(super) fn parse_path_json_workspace(
    args: Vec<String>,
    usage: &str,
) -> Result<(PathBuf, bool, bool), String> {
    let mut json = false;
    let mut workspace = false;
    let mut path = None;
    for arg in args {
        if arg == "--json-errors" {
            json = true;
        } else if arg == "--workspace" {
            workspace = true;
        } else if path.is_none() {
            path = Some(PathBuf::from(arg));
        } else {
            return Err(usage.to_string());
        }
    }
    Ok((
        path.unwrap_or(env::current_dir().map_err(|err| err.to_string())?),
        json,
        workspace,
    ))
}

pub(super) fn parse_build_args(
    args: Vec<String>,
    usage: &str,
) -> Result<(PathBuf, bool, bool, bool, DependencyResolutionOptions), String> {
    let mut emit_c = false;
    let mut json = false;
    let mut workspace = false;
    let mut deps = DependencyResolutionOptions::default();
    let mut path = None;
    for arg in args {
        if arg == "--emit-c" {
            emit_c = true;
        } else if arg == "--json-errors" {
            json = true;
        } else if arg == "--workspace" {
            workspace = true;
        } else if arg == "--locked" {
            deps.locked = true;
        } else if arg == "--offline" {
            deps.offline = true;
        } else if arg == "--frozen" {
            deps.locked = true;
            deps.offline = true;
        } else if path.is_none() {
            path = Some(PathBuf::from(arg));
        } else {
            return Err(usage.to_string());
        }
    }
    Ok((
        path.unwrap_or(env::current_dir().map_err(|err| err.to_string())?),
        emit_c,
        json,
        workspace,
        deps,
    ))
}

pub(super) fn parse_optional_path(args: Vec<String>, usage: &str) -> Result<PathBuf, String> {
    match args.as_slice() {
        [] => env::current_dir().map_err(|err| err.to_string()),
        [path] => Ok(PathBuf::from(path)),
        _ => Err(usage.to_string()),
    }
}

pub(super) fn parse_run_args(args: Vec<String>) -> Result<(PathBuf, Vec<String>, bool), String> {
    let current_dir = || env::current_dir().map_err(|err| err.to_string());
    let mut path = None;
    let mut json = false;
    let mut index = 0;
    while let Some(arg) = args.get(index) {
        if arg == "--" {
            return Ok((
                path.unwrap_or(current_dir()?),
                args.into_iter().skip(index + 1).collect(),
                json,
            ));
        }
        if arg == "--json-errors" {
            json = true;
        } else if path.is_none() {
            path = Some(PathBuf::from(arg));
        } else {
            return Err("usage: nomo run [path] [--json-errors] [-- args...]".to_string());
        }
        index += 1;
    }
    Ok((path.unwrap_or(current_dir()?), Vec::new(), json))
}

pub(super) fn filter_projects_by_package(
    projects: Vec<Project>,
    package: &str,
) -> Result<Vec<Project>, String> {
    let mut matched = Vec::new();
    for project in projects {
        if project_matches_package(&project, package)? {
            matched.push(project);
        }
    }
    if matched.is_empty() {
        return Err(format!("workspace package `{package}` was not found"));
    }
    Ok(matched)
}

pub(super) fn validate_project_package(project: &Project, package: &str) -> Result<(), String> {
    if project_matches_package(project, package)? {
        Ok(())
    } else {
        Err(format!("project package does not match `{package}`"))
    }
}

pub(super) fn is_nomo_source_file(path: &Path) -> bool {
    path.extension().and_then(|ext| ext.to_str()) == Some("nomo")
}

pub(super) fn is_missing_manifest_error(message: &str) -> bool {
    message.starts_with("could not find nomo.toml")
}

fn project_matches_package(project: &Project, package: &str) -> Result<bool, String> {
    let package_id = project_package_id(project)?;
    Ok(package_id == package || project.name == package)
}
