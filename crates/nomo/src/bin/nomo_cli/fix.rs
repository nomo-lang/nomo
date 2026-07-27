use nomo::project::migrate_project_module_roots;
use std::env;
use std::path::PathBuf;

const USAGE: &str = "usage: nomo fix module-roots [path] [--check]";

pub(super) fn run_fix_command(args: Vec<String>) -> Result<(), String> {
    let Some(command) = args.first().map(String::as_str) else {
        return Err(USAGE.to_string());
    };
    if command != "module-roots" {
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
            return Err(format!("unknown fix module-roots option `{arg}`"));
        } else if path.is_none() {
            path = Some(PathBuf::from(arg));
        } else {
            return Err(USAGE.to_string());
        }
    }
    let path = path.unwrap_or(env::current_dir().map_err(|err| err.to_string())?);
    let result = migrate_project_module_roots(&path, check)?;
    if result.updated_files.is_empty() {
        println!(
            "module package declarations are canonical at {}",
            result.root.display()
        );
    } else {
        println!(
            "migrated {} source file{} under {}",
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
