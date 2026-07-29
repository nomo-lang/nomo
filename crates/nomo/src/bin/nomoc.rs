use nomo::check_source;
use nomo::project::{
    BuildError, BuildProfile, clear_standalone_build_metadata,
    compile_standalone_source_with_profile_cache, record_standalone_c_build_metadata,
};
use nomo::target::TargetTriple;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process;

struct BuildArgs {
    source: PathBuf,
    out: Option<PathBuf>,
    json: bool,
    target: TargetTriple,
    emit_c: bool,
    profile: BuildProfile,
}

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
            let BuildArgs {
                source,
                out,
                json,
                target,
                emit_c,
                profile,
            } = parse_build_args(args)?;
            clear_standalone_build_metadata(&source, &target).map_err(|error| error.human())?;
            if emit_c && profile == BuildProfile::Release {
                return Err("`--release` and `--emit-c` cannot be used together".to_string());
            }
            let c = match compile_standalone_source_with_profile_cache(&source, &target, profile) {
                Ok(c) => c,
                Err(BuildError::Diagnostic(diag)) if json => return Err(diag.json()),
                Err(error) => return Err(error.human()),
            };
            if let Some(out) = out {
                if let Some(parent) = out.parent() {
                    fs::create_dir_all(parent).map_err(|err| err.to_string())?;
                }
                fs::write(&out, &c).map_err(|err| err.to_string())?;
                record_standalone_c_build_metadata(&source, &c, Some(&out), &target, profile)
                    .map_err(|error| error.human())?;
                println!("emitted {}", out.display());
            } else {
                record_standalone_c_build_metadata(&source, &c, None, &target, profile)
                    .map_err(|error| error.human())?;
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

fn parse_build_args(args: Vec<String>) -> Result<BuildArgs, String> {
    let mut source = None;
    let mut out = None;
    let mut json = false;
    let mut target = None;
    let mut emit_c = false;
    let mut profile = BuildProfile::Debug;
    let mut release_seen = false;
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--emit-c" => emit_c = true,
            "--release" => {
                if release_seen {
                    return Err("--release may only be specified once".to_string());
                }
                release_seen = true;
                profile = BuildProfile::Release;
            }
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
        .map(|source| {
            TargetTriple::host().map(|host| BuildArgs {
                source,
                out,
                json,
                target: target.unwrap_or(host),
                emit_c,
                profile,
            })
        })
        .ok_or_else(build_usage)?
}

fn build_usage() -> String {
    "usage: nomoc build <source.nomo> [--target <triple>] [--release] [--emit-c] [--out path] [--json-errors]".to_string()
}

fn print_help() {
    println!(
        "nomoc {}\n\nCommands:\n  nomoc check <source.nomo> [--json-errors]\n  nomoc build <source.nomo> [--target <triple>] [--release] [--emit-c] [--out path] [--json-errors]\n",
        env!("CARGO_PKG_VERSION")
    );
}
