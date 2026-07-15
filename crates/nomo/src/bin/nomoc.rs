use nomo::target::TargetTriple;
use nomo::{check_source, compile_source_to_c_for_target};
use std::env;
use std::fs;
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
        "check" => {
            let (source, json) = parse_source_and_json(args)?;
            match check_source(&source) {
                Ok(_) => {
                    println!("checked {}", source.display());
                    Ok(())
                }
                Err(diag) if json => Err(diag.json()),
                Err(diag) => Err(diag.human()),
            }
        }
        "build" => {
            let (source, out, json, target) = parse_build_args(args)?;
            let c = match compile_source_to_c_for_target(&source, &target) {
                Ok(c) => c,
                Err(diag) if json => return Err(diag.json()),
                Err(diag) => return Err(diag.human()),
            };
            if let Some(out) = out {
                if let Some(parent) = out.parent() {
                    fs::create_dir_all(parent).map_err(|err| err.to_string())?;
                }
                fs::write(&out, c).map_err(|err| err.to_string())?;
                println!("emitted {}", out.display());
            } else {
                print!("{c}");
            }
            Ok(())
        }
        "help" | "--help" | "-h" => {
            print_help();
            Ok(())
        }
        other => Err(format!("unknown command `{other}`")),
    }
}

fn parse_source_and_json(args: Vec<String>) -> Result<(PathBuf, bool), String> {
    let mut json = false;
    let mut source = None;
    for arg in args {
        if arg == "--json-errors" {
            json = true;
        } else if source.is_none() {
            source = Some(PathBuf::from(arg));
        } else {
            return Err("usage: nomoc check <source.nomo> [--json-errors]".to_string());
        }
    }
    source
        .map(|source| (source, json))
        .ok_or_else(|| "usage: nomoc check <source.nomo> [--json-errors]".to_string())
}

fn parse_build_args(
    args: Vec<String>,
) -> Result<(PathBuf, Option<PathBuf>, bool, TargetTriple), String> {
    let mut source = None;
    let mut out = None;
    let mut json = false;
    let mut target = None;
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--emit-c" => {}
            "--json-errors" => json = true,
            "--target" => {
                let Some(value) = iter.next() else {
                    return Err(build_usage());
                };
                if target.is_some() {
                    return Err("--target may only be specified once".to_string());
                }
                target = Some(value.parse::<TargetTriple>()?);
            }
            "--out" => {
                let Some(value) = iter.next() else {
                    return Err(build_usage());
                };
                out = Some(PathBuf::from(value));
            }
            _ if source.is_none() => source = Some(PathBuf::from(arg)),
            _ => {
                return Err(build_usage());
            }
        }
    }
    source
        .map(|source| TargetTriple::host().map(|host| (source, out, json, target.unwrap_or(host))))
        .ok_or_else(build_usage)?
}

fn build_usage() -> String {
    "usage: nomoc build <source.nomo> [--target <triple>] [--emit-c] [--out path] [--json-errors]"
        .to_string()
}

fn print_help() {
    println!(
        "nomoc {}\n\nCommands:\n  nomoc check <source.nomo> [--json-errors]\n  nomoc build <source.nomo> [--target <triple>] [--emit-c] [--out path] [--json-errors]\n",
        env!("CARGO_PKG_VERSION")
    );
}
