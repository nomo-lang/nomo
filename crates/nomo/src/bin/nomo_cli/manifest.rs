use nomo::project::migrate_project_manifests;
use std::env;
use std::path::PathBuf;

const USAGE: &str = "usage: nomo manifest migrate [path] [--check]";

pub(super) fn run_manifest_command(args: Vec<String>) -> Result<(), String> {
    let Some(command) = args.first().map(String::as_str) else {
        return Err(USAGE.to_string());
    };
    if command != "migrate" {
        return Err(USAGE.to_string());
    }

    let mut path = None;
    let mut check = false;
    for arg in &args[1..] {
        if arg == "--check" {
            if check {
                return Err("`--check` may only be specified once".to_string());
            }
            check = true;
        } else if arg.starts_with('-') {
            return Err(format!("unknown manifest migrate option `{arg}`"));
        } else if path.is_none() {
            path = Some(PathBuf::from(arg));
        } else {
            return Err(USAGE.to_string());
        }
    }
    let path = path.unwrap_or(env::current_dir().map_err(|err| err.to_string())?);
    let result = migrate_project_manifests(&path, check)?;
    if result.updated_files.is_empty() {
        println!("manifest v2 is up to date at {}", result.root.display());
    } else {
        println!(
            "migrated {} file{} under {}",
            result.updated_files.len(),
            if result.updated_files.len() == 1 {
                ""
            } else {
                "s"
            },
            result.root.display()
        );
        for path in result.updated_files {
            println!("  {}", path.display());
        }
    }
    Ok(())
}
