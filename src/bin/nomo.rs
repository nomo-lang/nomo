use nomo::project::{
    BuildError, DependencyResolutionOptions, build_project_with_options, check_project,
    clean_project, create_project, dependency_tree_with_options, discover_project,
    discover_workspace, resolve_project_dependencies_with_options,
    resolve_workspace_dependencies_with_options, run_project_with_args_and_diagnostics,
    run_standalone_script_with_args_and_diagnostics,
};
use nomo::{Diagnostic, format_source};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process;

fn main() {
    if let Err(err) = run() {
        eprintln!("{err}");
        process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut args = env::args().skip(1).collect::<Vec<_>>();
    let Some(command) = args.first().cloned() else {
        print_help();
        return Ok(());
    };
    args.remove(0);

    match command.as_str() {
        "new" => {
            let [name] = args.as_slice() else {
                return Err("usage: nomo new <name>".to_string());
            };
            let project =
                create_project(&env::current_dir().map_err(|err| err.to_string())?, name)?;
            println!("created {}", project.root.display());
            Ok(())
        }
        "check" => {
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
        "build" => {
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
        "run" => {
            let (path, program_args, json) = parse_run_args(args)?;
            let code = match discover_project(&path) {
                Ok(project) => {
                    match run_project_with_args_and_diagnostics(&project, &program_args) {
                        Ok(code) => code,
                        Err(BuildError::Diagnostic(diag)) if json => return Err(diag.json()),
                        Err(err) => return Err(err.human()),
                    }
                }
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
        "fmt" => {
            let (path, check, json) = parse_fmt_args(args)?;
            match format_path(&path, check) {
                Ok(changed) if check && changed => Err("format check failed".to_string()),
                Ok(changed) => {
                    if !changed {
                        println!("all files already formatted");
                    }
                    Ok(())
                }
                Err(FormatError::Diagnostic(diag)) if json => Err(diag.json()),
                Err(err) => Err(err.human()),
            }
        }
        "clean" => {
            let path = parse_optional_path(args, "usage: nomo clean [path]")?;
            let project = discover_project(&path)?;
            let cleaned = clean_project(&project)?;
            println!("cleaned {}", cleaned.display());
            Ok(())
        }
        "deps" => {
            let [subcommand, rest @ ..] = args.as_slice() else {
                return Err(
                    "usage: nomo deps <resolve|tree> [path] [--workspace] [--locked] [--offline] [--frozen]".to_string()
                );
            };
            let (path, workspace, deps) = parse_deps_args(
                rest.to_vec(),
                &format!(
                    "usage: nomo deps {subcommand} [path] [--workspace] [--locked] [--offline] [--frozen]"
                ),
            )?;
            match subcommand.as_str() {
                "resolve" => {
                    if workspace {
                        let workspace = discover_workspace(&path)?;
                        let lock = resolve_workspace_dependencies_with_options(&workspace, deps)?;
                        println!("resolved {}", lock.display());
                        return Ok(());
                    }
                    let project = discover_project(&path)?;
                    let lock = resolve_project_dependencies_with_options(&project, deps)?;
                    println!("resolved {}", lock.display());
                    Ok(())
                }
                "tree" => {
                    if workspace {
                        for project in discover_workspace(&path)?.members {
                            print!("{}", dependency_tree_with_options(&project, deps)?);
                        }
                    } else {
                        let project = discover_project(&path)?;
                        print!("{}", dependency_tree_with_options(&project, deps)?);
                    }
                    Ok(())
                }
                other => Err(format!("unknown deps command `{other}`")),
            }
        }
        "help" | "--help" | "-h" => {
            print_help();
            Ok(())
        }
        other => Err(format!("unknown command `{other}`")),
    }
}

fn parse_path_json_workspace(
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

fn parse_build_args(
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

fn parse_optional_path(args: Vec<String>, usage: &str) -> Result<PathBuf, String> {
    match args.as_slice() {
        [] => env::current_dir().map_err(|err| err.to_string()),
        [path] => Ok(PathBuf::from(path)),
        _ => Err(usage.to_string()),
    }
}

fn parse_deps_args(
    args: Vec<String>,
    usage: &str,
) -> Result<(PathBuf, bool, DependencyResolutionOptions), String> {
    let mut workspace = false;
    let mut deps = DependencyResolutionOptions::default();
    let mut path = None;
    for arg in args {
        if arg == "--workspace" {
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
        workspace,
        deps,
    ))
}

fn parse_run_args(args: Vec<String>) -> Result<(PathBuf, Vec<String>, bool), String> {
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

fn parse_fmt_args(args: Vec<String>) -> Result<(PathBuf, bool, bool), String> {
    let mut check = false;
    let mut json = false;
    let mut path = None;
    for arg in args {
        if arg == "--check" {
            check = true;
        } else if arg == "--json-errors" {
            json = true;
        } else if path.is_none() {
            path = Some(PathBuf::from(arg));
        } else {
            return Err("usage: nomo fmt [path] [--check] [--json-errors]".to_string());
        }
    }
    Ok((
        path.unwrap_or(env::current_dir().map_err(|err| err.to_string())?),
        check,
        json,
    ))
}

#[derive(Debug)]
enum FormatError {
    Diagnostic(Diagnostic),
    Message(String),
}

impl FormatError {
    fn human(&self) -> String {
        match self {
            FormatError::Diagnostic(diagnostic) => diagnostic.human(),
            FormatError::Message(message) => message.clone(),
        }
    }
}

fn format_path(path: &Path, check: bool) -> Result<bool, FormatError> {
    let files = format_targets(path)?;
    let mut changed = false;
    for file in files {
        let source = fs::read_to_string(&file).map_err(|err| {
            FormatError::Message(format!("failed to read {}: {err}", file.display()))
        })?;
        let formatted = format_source(&file, &source).map_err(FormatError::Diagnostic)?;
        if formatted != source {
            changed = true;
            if check {
                println!("would format {}", file.display());
            } else {
                fs::write(&file, formatted).map_err(|err| {
                    FormatError::Message(format!("failed to write {}: {err}", file.display()))
                })?;
                println!("formatted {}", file.display());
            }
        }
    }
    Ok(changed)
}

fn format_targets(path: &Path) -> Result<Vec<PathBuf>, FormatError> {
    if path.extension().and_then(|ext| ext.to_str()) == Some("nomo") {
        if !path.is_file() {
            return Err(FormatError::Message(format!(
                "source file not found: {}",
                path.display()
            )));
        }
        return Ok(vec![path.to_path_buf()]);
    }

    let project = discover_project(path).map_err(FormatError::Message)?;
    let src = project.root.join("src");
    if !src.is_dir() {
        return Err(FormatError::Message(format!(
            "source directory not found: {}",
            src.display()
        )));
    }
    let mut files = Vec::new();
    collect_nomo_files(&src, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect_nomo_files(dir: &Path, files: &mut Vec<PathBuf>) -> Result<(), FormatError> {
    for entry in fs::read_dir(dir).map_err(|err| {
        FormatError::Message(format!("failed to read directory {}: {err}", dir.display()))
    })? {
        let entry = entry.map_err(|err| FormatError::Message(err.to_string()))?;
        let path = entry.path();
        if path.is_dir() {
            collect_nomo_files(&path, files)?;
        } else if path.is_file() && path.extension().and_then(|ext| ext.to_str()) == Some("nomo") {
            files.push(path);
        }
    }
    Ok(())
}

fn is_nomo_source_file(path: &Path) -> bool {
    path.extension().and_then(|ext| ext.to_str()) == Some("nomo")
}

fn is_missing_manifest_error(message: &str) -> bool {
    message.starts_with("could not find nomo.toml")
}

fn print_help() {
    println!(
        "nomo 0.1.0\n\nCommands:\n  nomo new <name>\n  nomo check [path] [--json-errors] [--workspace]\n  nomo build [path] [--emit-c] [--json-errors] [--workspace] [--locked] [--offline] [--frozen]\n  nomo run [path] [--json-errors] [-- args...]\n  nomo fmt [path] [--check] [--json-errors]\n  nomo clean [path]\n  nomo deps <resolve|tree> [path] [--workspace] [--locked] [--offline] [--frozen]\n"
    );
}
