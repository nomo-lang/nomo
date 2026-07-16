use nomo::incremental::{
    PERSISTENT_CACHE_SCHEMA_VERSION, PersistentQueryCache, clean_incremental_cache,
};
use nomo::project::{discover_project, discover_workspace};
use std::env;
use std::path::{Path, PathBuf};

const USAGE: &str = "usage: nomo cache <stats|clean|prune> [path] [--max-bytes <bytes>]";

pub(super) fn run_cache_command(args: Vec<String>) -> Result<(), String> {
    let Some(command) = args.first().map(String::as_str) else {
        return Err(USAGE.to_string());
    };
    match command {
        "stats" => {
            let path = parse_optional_path(&args[1..])?;
            let root = cache_owner_root(&path)?;
            let cache = PersistentQueryCache::at_root(&root);
            print_stats(&cache)?;
            Ok(())
        }
        "clean" => {
            let path = parse_optional_path(&args[1..])?;
            let root = cache_owner_root(&path)?;
            let cleaned = clean_incremental_cache(&root)?;
            println!("cleaned {}", cleaned.display());
            Ok(())
        }
        "prune" => {
            let (path, max_bytes) = parse_prune_args(&args[1..])?;
            let root = cache_owner_root(&path)?;
            let cache = PersistentQueryCache::with_max_bytes(&root, max_bytes);
            let removed = cache.prune()?;
            println!("pruned {removed} entries from {}", cache.root().display());
            print_stats(&cache)?;
            Ok(())
        }
        _ => Err(USAGE.to_string()),
    }
}

fn parse_optional_path(args: &[String]) -> Result<PathBuf, String> {
    match args {
        [] => env::current_dir().map_err(|error| error.to_string()),
        [path] => Ok(PathBuf::from(path)),
        _ => Err(USAGE.to_string()),
    }
}

fn parse_prune_args(args: &[String]) -> Result<(PathBuf, u64), String> {
    let mut path = None;
    let mut max_bytes = None;
    let mut index = 0;
    while index < args.len() {
        if args[index] == "--max-bytes" {
            let value = args.get(index + 1).ok_or_else(|| USAGE.to_string())?;
            if max_bytes.is_some() {
                return Err("--max-bytes may only be specified once".to_string());
            }
            max_bytes = Some(
                value
                    .parse::<u64>()
                    .map_err(|_| format!("invalid cache capacity `{value}`"))?,
            );
            index += 2;
        } else if path.is_none() {
            path = Some(PathBuf::from(&args[index]));
            index += 1;
        } else {
            return Err(USAGE.to_string());
        }
    }
    let max_bytes = max_bytes.ok_or_else(|| USAGE.to_string())?;
    Ok((
        path.unwrap_or(env::current_dir().map_err(|error| error.to_string())?),
        max_bytes,
    ))
}

fn cache_owner_root(path: &Path) -> Result<PathBuf, String> {
    match discover_project(path) {
        Ok(project) => Ok(project.workspace_root.unwrap_or(project.root)),
        Err(project_error) => discover_workspace(path)
            .map(|workspace| workspace.root)
            .map_err(|_| project_error),
    }
}

fn print_stats(cache: &PersistentQueryCache) -> Result<(), String> {
    let stats = cache.stats()?;
    println!("cache {}", cache.root().display());
    println!("schema {PERSISTENT_CACHE_SCHEMA_VERSION}");
    println!("entries {}", stats.entries);
    println!("bytes {}", stats.bytes);
    println!("capacity-bytes {}", stats.capacity_bytes);
    Ok(())
}
