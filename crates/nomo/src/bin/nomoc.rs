use nomo::{check_source, compile_source_to_c};
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
            let (source, out, json) = parse_build_args(args)?;
            let c = match compile_source_to_c(&source) {
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

fn parse_build_args(args: Vec<String>) -> Result<(PathBuf, Option<PathBuf>, bool), String> {
    let mut source = None;
    let mut out = None;
    let mut json = false;
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--emit-c" => {}
            "--json-errors" => json = true,
            "--out" => {
                let Some(value) = iter.next() else {
                    return Err(
                        "usage: nomoc build <source.nomo> [--emit-c] [--out path] [--json-errors]"
                            .to_string(),
                    );
                };
                out = Some(PathBuf::from(value));
            }
            _ if source.is_none() => source = Some(PathBuf::from(arg)),
            _ => {
                return Err(
                    "usage: nomoc build <source.nomo> [--emit-c] [--out path] [--json-errors]"
                        .to_string(),
                );
            }
        }
    }
    source.map(|source| (source, out, json)).ok_or_else(|| {
        "usage: nomoc build <source.nomo> [--emit-c] [--out path] [--json-errors]".to_string()
    })
}

fn print_help() {
    println!(
        "nomoc 0.1.0\n\nCommands:\n  nomoc check <source.nomo> [--json-errors]\n  nomoc build <source.nomo> [--emit-c] [--out path] [--json-errors]\n"
    );
}
