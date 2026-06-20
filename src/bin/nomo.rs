use nomo::project::{
    BuildError, build_project_with_diagnostics, check_project, clean_project, create_project,
    discover_project, run_project_with_args_and_diagnostics,
};
use std::env;
use std::path::PathBuf;
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
            let (path, json) = parse_path_and_json(args)?;
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
        "build" => {
            let (path, emit_c, json) = parse_path_and_emit_c_and_json(args)?;
            let project = discover_project(&path)?;
            let artifact = match build_project_with_diagnostics(&project, emit_c) {
                Ok(artifact) => artifact,
                Err(BuildError::Diagnostic(diag)) if json => return Err(diag.json()),
                Err(err) => return Err(err.human()),
            };
            println!("built {}", artifact.display());
            Ok(())
        }
        "run" => {
            let (path, program_args, json) = parse_run_args(args)?;
            let project = discover_project(&path)?;
            let code = match run_project_with_args_and_diagnostics(&project, &program_args) {
                Ok(code) => code,
                Err(BuildError::Diagnostic(diag)) if json => return Err(diag.json()),
                Err(err) => return Err(err.human()),
            };
            if code == 0 {
                Ok(())
            } else {
                Err(format!("program exited with status {code}"))
            }
        }
        "clean" => {
            let path = parse_optional_path(args, "usage: nomo clean [path]")?;
            let project = discover_project(&path)?;
            let cleaned = clean_project(&project)?;
            println!("cleaned {}", cleaned.display());
            Ok(())
        }
        "help" | "--help" | "-h" => {
            print_help();
            Ok(())
        }
        other => Err(format!("unknown command `{other}`")),
    }
}

fn parse_path_and_json(args: Vec<String>) -> Result<(PathBuf, bool), String> {
    let mut json = false;
    let mut path = None;
    for arg in args {
        if arg == "--json-errors" {
            json = true;
        } else if path.is_none() {
            path = Some(PathBuf::from(arg));
        } else {
            return Err("usage: nomo check [path] [--json-errors]".to_string());
        }
    }
    Ok((
        path.unwrap_or(env::current_dir().map_err(|err| err.to_string())?),
        json,
    ))
}

fn parse_path_and_emit_c_and_json(args: Vec<String>) -> Result<(PathBuf, bool, bool), String> {
    let mut emit_c = false;
    let mut json = false;
    let mut path = None;
    for arg in args {
        if arg == "--emit-c" {
            emit_c = true;
        } else if arg == "--json-errors" {
            json = true;
        } else if path.is_none() {
            path = Some(PathBuf::from(arg));
        } else {
            return Err("usage: nomo build [path] [--emit-c] [--json-errors]".to_string());
        }
    }
    Ok((
        path.unwrap_or(env::current_dir().map_err(|err| err.to_string())?),
        emit_c,
        json,
    ))
}

fn parse_optional_path(args: Vec<String>, usage: &'static str) -> Result<PathBuf, String> {
    match args.as_slice() {
        [] => env::current_dir().map_err(|err| err.to_string()),
        [path] => Ok(PathBuf::from(path)),
        _ => Err(usage.to_string()),
    }
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

fn print_help() {
    println!(
        "nomo 0.1.0\n\nCommands:\n  nomo new <name>\n  nomo check [path] [--json-errors]\n  nomo build [path] [--emit-c] [--json-errors]\n  nomo run [path] [--json-errors] [-- args...]\n  nomo clean [path]\n"
    );
}
