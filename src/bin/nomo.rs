use nomo::project::{
    build_project, check_project, create_project, discover_project, run_project_with_args,
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
            let Some(name) = args.first() else {
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
            let (path, emit_c) = parse_path_and_emit_c(args)?;
            let project = discover_project(&path)?;
            let artifact = build_project(&project, emit_c)?;
            println!("built {}", artifact.display());
            Ok(())
        }
        "run" => {
            let (path, program_args) = parse_run_args(args)?;
            let project = discover_project(&path)?;
            let code = run_project_with_args(&project, &program_args)?;
            if code == 0 {
                Ok(())
            } else {
                Err(format!("program exited with status {code}"))
            }
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

fn parse_path_and_emit_c(args: Vec<String>) -> Result<(PathBuf, bool), String> {
    let mut emit_c = false;
    let mut path = None;
    for arg in args {
        if arg == "--emit-c" {
            emit_c = true;
        } else if path.is_none() {
            path = Some(PathBuf::from(arg));
        } else {
            return Err("usage: nomo build [path] [--emit-c]".to_string());
        }
    }
    Ok((
        path.unwrap_or(env::current_dir().map_err(|err| err.to_string())?),
        emit_c,
    ))
}

fn parse_run_args(args: Vec<String>) -> Result<(PathBuf, Vec<String>), String> {
    let current_dir = || env::current_dir().map_err(|err| err.to_string());
    if args.is_empty() {
        return Ok((current_dir()?, Vec::new()));
    }
    if args[0] == "--" {
        return Ok((current_dir()?, args.into_iter().skip(1).collect()));
    }

    let path = PathBuf::from(&args[0]);
    match args.get(1) {
        None => Ok((path, Vec::new())),
        Some(separator) if separator == "--" => {
            Ok((path, args.into_iter().skip(2).collect::<Vec<_>>()))
        }
        Some(_) => Err("usage: nomo run [path] [-- args...]".to_string()),
    }
}

fn print_help() {
    println!(
        "nomo 0.1.0\n\nCommands:\n  nomo new <name>\n  nomo check [path] [--json-errors]\n  nomo build [path] [--emit-c]\n  nomo run [path] [-- args...]\n"
    );
}
