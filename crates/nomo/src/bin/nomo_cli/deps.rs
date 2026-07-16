use super::cli_common::parse_optional_path;
use nomo::project::{
    DependencyResolutionOptions, DependencyUpdateOptions, DependencyVendorOptions,
    clean_dependency_cache, dependency_tree_for_target_with_options, dependency_tree_with_options,
    discover_project, discover_workspace, discover_workspace_for_target,
    resolve_project_dependencies_with_options, resolve_workspace_dependencies_with_options,
    update_project_dependencies, update_workspace_dependencies, vendor_project_dependencies,
    vendor_workspace_dependencies,
};
use nomo::target::TargetTriple;
use std::env;
use std::path::PathBuf;

pub(super) fn run_deps_command(args: Vec<String>) -> Result<(), String> {
    let [subcommand, rest @ ..] = args.as_slice() else {
        return Err(
            "usage: nomo deps <resolve|tree|update|vendor|clean-cache> [path] [--workspace] [--target <triple>] [--locked] [--offline] [--frozen]".to_string()
        );
    };
    match subcommand.as_str() {
        "resolve" => {
            let (path, workspace, deps) = parse_deps_args(
                rest.to_vec(),
                &format!(
                    "usage: nomo deps {subcommand} [path] [--workspace] [--locked] [--offline] [--frozen]"
                ),
            )?;
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
            let (path, workspace, deps, target) = parse_deps_tree_args(
                rest.to_vec(),
                &format!(
                    "usage: nomo deps {subcommand} [path] [--workspace] [--target <triple>] [--locked] [--offline] [--frozen]"
                ),
            )?;
            if workspace {
                let workspace = match &target {
                    Some(target) => discover_workspace_for_target(&path, target)?,
                    None => discover_workspace(&path)?,
                };
                for project in workspace.members {
                    let tree = match &target {
                        Some(target) => {
                            dependency_tree_for_target_with_options(&project, deps, target)?
                        }
                        None => dependency_tree_with_options(&project, deps)?,
                    };
                    print!("{tree}");
                }
            } else {
                let project = discover_project(&path)?;
                let tree = match &target {
                    Some(target) => {
                        dependency_tree_for_target_with_options(&project, deps, target)?
                    }
                    None => dependency_tree_with_options(&project, deps)?,
                };
                print!("{tree}");
            }
            Ok(())
        }
        "update" => {
            let (path, target, workspace, deps) = parse_deps_update_args(
                rest.to_vec(),
                "usage: nomo deps update [path] [alias-or-package] [--workspace] [--offline] [--precise <version-or-rev>]",
            )?;
            if workspace {
                let workspace = discover_workspace(&path)?;
                let lock = update_workspace_dependencies(&workspace, target.as_deref(), deps)?;
                println!("updated {}", lock.display());
                return Ok(());
            }
            let project = discover_project(&path)?;
            let lock = update_project_dependencies(&project, target.as_deref(), deps)?;
            println!("updated {}", lock.display());
            Ok(())
        }
        "vendor" => {
            let (path, workspace, options) = parse_deps_vendor_args(
                rest.to_vec(),
                "usage: nomo deps vendor [path] [--workspace] [--dir vendor] [--sync]",
            )?;
            if workspace {
                let workspace = discover_workspace(&path)?;
                let vendor = vendor_workspace_dependencies(&workspace, options)?;
                println!("vendored {}", vendor.display());
                return Ok(());
            }
            let project = discover_project(&path)?;
            let vendor = vendor_project_dependencies(&project, options)?;
            println!("vendored {}", vendor.display());
            Ok(())
        }
        "clean-cache" => {
            let path = parse_optional_path(rest.to_vec(), "usage: nomo deps clean-cache [path]")?;
            let cleaned = clean_dependency_cache(&path)?;
            println!("cleaned {}", cleaned.display());
            Ok(())
        }
        other => Err(format!("unknown deps command `{other}`")),
    }
}

pub(super) fn parse_deps_tree_args(
    args: Vec<String>,
    usage: &str,
) -> Result<
    (
        PathBuf,
        bool,
        DependencyResolutionOptions,
        Option<TargetTriple>,
    ),
    String,
> {
    let mut workspace = false;
    let mut deps = DependencyResolutionOptions::default();
    let mut target = None;
    let mut path = None;
    let mut index = 0;
    while let Some(arg) = args.get(index) {
        if arg == "--workspace" {
            workspace = true;
        } else if arg == "--locked" {
            deps.locked = true;
        } else if arg == "--offline" {
            deps.offline = true;
        } else if arg == "--frozen" {
            deps.locked = true;
            deps.offline = true;
        } else if let Some(value) = arg.strip_prefix("--target=") {
            if value.is_empty() || target.is_some() {
                return Err(usage.to_string());
            }
            target = Some(value.parse()?);
        } else if arg == "--target" {
            index += 1;
            let Some(value) = args.get(index) else {
                return Err("--target requires a target triple".to_string());
            };
            if target.is_some() {
                return Err(usage.to_string());
            }
            target = Some(value.parse()?);
        } else if path.is_none() {
            path = Some(PathBuf::from(arg));
        } else {
            return Err(usage.to_string());
        }
        index += 1;
    }
    Ok((
        path.unwrap_or(env::current_dir().map_err(|err| err.to_string())?),
        workspace,
        deps,
        target,
    ))
}

pub(super) fn parse_deps_args(
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

pub(super) fn parse_deps_update_args(
    args: Vec<String>,
    usage: &str,
) -> Result<(PathBuf, Option<String>, bool, DependencyUpdateOptions), String> {
    let mut workspace = false;
    let mut deps = DependencyUpdateOptions::default();
    let mut values = Vec::new();
    let mut index = 0;
    while let Some(arg) = args.get(index) {
        if arg == "--workspace" {
            workspace = true;
        } else if arg == "--offline" {
            deps.resolution.offline = true;
        } else if let Some(value) = arg.strip_prefix("--precise=") {
            if value.is_empty() {
                return Err("--precise requires a version or git revision".to_string());
            }
            deps.precise = Some(value.to_string());
        } else if arg == "--precise" {
            index += 1;
            let Some(value) = args.get(index) else {
                return Err("--precise requires a version or git revision".to_string());
            };
            deps.precise = Some(value.clone());
        } else if arg == "--locked" || arg == "--frozen" {
            return Err(format!("{arg} is not valid for nomo deps update"));
        } else {
            values.push(arg.clone());
        }
        index += 1;
    }

    let current_dir = || env::current_dir().map_err(|err| err.to_string());
    match values.as_slice() {
        [] => Ok((current_dir()?, None, workspace, deps)),
        [one] => {
            let candidate = PathBuf::from(one);
            if candidate.exists() {
                Ok((candidate, None, workspace, deps))
            } else {
                Ok((current_dir()?, Some(one.clone()), workspace, deps))
            }
        }
        [path, target] => Ok((PathBuf::from(path), Some(target.clone()), workspace, deps)),
        _ => Err(usage.to_string()),
    }
}

pub(super) fn parse_deps_vendor_args(
    args: Vec<String>,
    usage: &str,
) -> Result<(PathBuf, bool, DependencyVendorOptions), String> {
    let mut workspace = false;
    let mut options = DependencyVendorOptions::default();
    let mut path = None;
    let mut index = 0;
    while let Some(arg) = args.get(index) {
        if arg == "--workspace" {
            workspace = true;
        } else if arg == "--sync" {
            options.sync = true;
        } else if let Some(value) = arg.strip_prefix("--dir=") {
            if value.is_empty() {
                return Err("--dir requires a vendor directory".to_string());
            }
            options.dir = PathBuf::from(value);
        } else if arg == "--dir" {
            index += 1;
            let Some(value) = args.get(index) else {
                return Err("--dir requires a vendor directory".to_string());
            };
            options.dir = PathBuf::from(value);
        } else if path.is_none() {
            path = Some(PathBuf::from(arg));
        } else {
            return Err(usage.to_string());
        }
        index += 1;
    }
    Ok((
        path.unwrap_or(env::current_dir().map_err(|err| err.to_string())?),
        workspace,
        options,
    ))
}
