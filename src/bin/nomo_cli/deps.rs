use nomo::project::{
    DependencyResolutionOptions, DependencyUpdateOptions, DependencyVendorOptions,
};
use std::env;
use std::path::PathBuf;

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
