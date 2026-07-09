use nomo::project::DependencyAddSpec;
use std::env;
use std::path::PathBuf;

pub(super) fn parse_add_args(
    args: Vec<String>,
    usage: &str,
) -> Result<(PathBuf, DependencyAddSpec), String> {
    let mut registry = None;
    let mut values = Vec::new();
    let mut index = 0;
    while let Some(arg) = args.get(index) {
        if let Some(value) = arg.strip_prefix("--registry=") {
            if value.is_empty() {
                return Err("--registry requires a registry endpoint".to_string());
            }
            registry = Some(value.to_string());
        } else if arg == "--registry" {
            index += 1;
            let Some(value) = args.get(index) else {
                return Err("--registry requires a registry endpoint".to_string());
            };
            registry = Some(value.clone());
        } else if arg.starts_with('-') {
            return Err(usage.to_string());
        } else {
            values.push(arg.clone());
        }
        index += 1;
    }

    let current_dir = || env::current_dir().map_err(|err| err.to_string());
    match values.as_slice() {
        [spec] => Ok((current_dir()?, parse_dependency_add_spec(spec, registry)?)),
        [spec, path] => Ok((
            PathBuf::from(path),
            parse_dependency_add_spec(spec, registry)?,
        )),
        _ => Err(usage.to_string()),
    }
}

pub(super) fn parse_remove_args(
    args: Vec<String>,
    usage: &str,
) -> Result<(PathBuf, String), String> {
    let current_dir = || env::current_dir().map_err(|err| err.to_string());
    match args.as_slice() {
        [alias] => Ok((current_dir()?, alias.clone())),
        [alias, path] => Ok((PathBuf::from(path), alias.clone())),
        _ => Err(usage.to_string()),
    }
}

pub(super) fn parse_search_args(
    args: Vec<String>,
    usage: &str,
) -> Result<(String, String), String> {
    let mut query = None;
    let mut registry = None;
    let mut index = 0;
    while let Some(arg) = args.get(index) {
        if let Some(value) = arg.strip_prefix("--registry=") {
            if value.is_empty() {
                return Err("--registry requires a registry endpoint".to_string());
            }
            registry = Some(value.to_string());
        } else if arg == "--registry" {
            index += 1;
            let Some(value) = args.get(index) else {
                return Err("--registry requires a registry endpoint".to_string());
            };
            registry = Some(value.clone());
        } else if arg.starts_with('-') {
            return Err(usage.to_string());
        } else if query.is_none() {
            query = Some(arg.clone());
        } else {
            return Err(usage.to_string());
        }
        index += 1;
    }
    let query = query.ok_or_else(|| usage.to_string())?;
    let registry = registry.ok_or_else(|| "nomo search requires --registry <url>".to_string())?;
    Ok((query, registry))
}

pub(super) fn parse_login_args(args: Vec<String>, usage: &str) -> Result<(String, String), String> {
    let mut registry = None;
    let mut token = None;
    let mut index = 0;
    while let Some(arg) = args.get(index) {
        if let Some(value) = arg.strip_prefix("--registry=") {
            if value.is_empty() {
                return Err("--registry requires a registry endpoint".to_string());
            }
            registry = Some(value.to_string());
        } else if arg == "--registry" {
            index += 1;
            let Some(value) = args.get(index) else {
                return Err("--registry requires a registry endpoint".to_string());
            };
            registry = Some(value.clone());
        } else if let Some(value) = arg.strip_prefix("--token=") {
            if value.is_empty() {
                return Err("--token requires a registry token".to_string());
            }
            token = Some(value.to_string());
        } else if arg == "--token" {
            index += 1;
            let Some(value) = args.get(index) else {
                return Err("--token requires a registry token".to_string());
            };
            token = Some(value.clone());
        } else {
            return Err(usage.to_string());
        }
        index += 1;
    }
    let registry = registry.ok_or_else(|| "nomo login requires --registry <url>".to_string())?;
    let token = token.ok_or_else(|| "nomo login requires --token <token>".to_string())?;
    Ok((registry, token))
}

pub(super) fn parse_owner_add_args(
    args: Vec<String>,
    usage: &str,
) -> Result<(String, String, String), String> {
    parse_owner_registry_args(args, usage, "nomo owner add requires --registry <url>")
}

pub(super) fn parse_owner_remove_args(
    args: Vec<String>,
    usage: &str,
) -> Result<(String, String, String), String> {
    parse_owner_registry_args(args, usage, "nomo owner remove requires --registry <url>")
}

pub(super) fn parse_yank_args(
    args: Vec<String>,
    usage: &str,
) -> Result<(String, String, String), String> {
    let mut registry = None;
    let mut positional = Vec::new();
    let mut index = 0;
    while let Some(arg) = args.get(index) {
        if let Some(value) = arg.strip_prefix("--registry=") {
            if value.is_empty() {
                return Err("--registry requires a registry endpoint".to_string());
            }
            registry = Some(value.to_string());
        } else if arg == "--registry" {
            index += 1;
            let Some(value) = args.get(index) else {
                return Err("--registry requires a registry endpoint".to_string());
            };
            registry = Some(value.clone());
        } else if arg.starts_with('-') {
            return Err(usage.to_string());
        } else {
            positional.push(arg.clone());
        }
        index += 1;
    }
    let [package, version] = positional.as_slice() else {
        return Err(usage.to_string());
    };
    let registry = registry.ok_or_else(|| "nomo yank requires --registry <url>".to_string())?;
    Ok((package.clone(), version.clone(), registry))
}

pub(super) fn parse_publish_args(
    args: Vec<String>,
    usage: &str,
) -> Result<(PathBuf, bool, Option<String>, Option<PathBuf>, bool), String> {
    let mut dry_run = false;
    let mut registry = None;
    let mut output = None;
    let mut json = false;
    let mut path = None;
    let mut index = 0;
    while let Some(arg) = args.get(index) {
        if arg == "--dry-run" {
            dry_run = true;
        } else if arg == "--json-errors" {
            json = true;
        } else if let Some(value) = arg.strip_prefix("--registry=") {
            if value.is_empty() {
                return Err("--registry requires a URL".to_string());
            }
            registry = Some(value.to_string());
        } else if arg == "--registry" {
            index += 1;
            let Some(value) = args.get(index) else {
                return Err("--registry requires a URL".to_string());
            };
            registry = Some(value.clone());
        } else if let Some(value) = arg.strip_prefix("--output=") {
            if value.is_empty() {
                return Err("--output requires a directory".to_string());
            }
            output = Some(PathBuf::from(value));
        } else if arg == "--output" {
            index += 1;
            let Some(value) = args.get(index) else {
                return Err("--output requires a directory".to_string());
            };
            output = Some(PathBuf::from(value));
        } else if arg.starts_with('-') {
            return Err(usage.to_string());
        } else if path.is_none() {
            path = Some(PathBuf::from(arg));
        } else {
            return Err(usage.to_string());
        }
        index += 1;
    }
    if dry_run && registry.is_some() {
        return Err(
            "nomo publish accepts either --dry-run or --registry <url>, not both".to_string(),
        );
    }
    Ok((
        path.unwrap_or(env::current_dir().map_err(|err| err.to_string())?),
        dry_run,
        registry,
        output,
        json,
    ))
}

fn parse_dependency_add_spec(
    spec: &str,
    registry: Option<String>,
) -> Result<DependencyAddSpec, String> {
    let Some((alias, package_and_version)) = spec.split_once('@') else {
        return Err(
            "dependency spec must use `alias@owner/package:version`, for example `json@nomo-lang/json:0.1.0`"
                .to_string(),
        );
    };
    let Some((package, version)) = package_and_version.rsplit_once(':') else {
        return Err(
            "dependency spec must include a version after `:`, for example `json@nomo-lang/json:0.1.0`"
                .to_string(),
        );
    };
    if alias.is_empty() || package.is_empty() || version.is_empty() {
        return Err(
            "dependency spec must use `alias@owner/package:version`, for example `json@nomo-lang/json:0.1.0`"
                .to_string(),
        );
    }
    Ok(DependencyAddSpec {
        alias: alias.to_string(),
        package: package.to_string(),
        version: version.to_string(),
        registry,
    })
}

fn parse_owner_registry_args(
    args: Vec<String>,
    usage: &str,
    missing_registry: &str,
) -> Result<(String, String, String), String> {
    let mut registry = None;
    let mut positional = Vec::new();
    let mut index = 0;
    while let Some(arg) = args.get(index) {
        if let Some(value) = arg.strip_prefix("--registry=") {
            if value.is_empty() {
                return Err("--registry requires a registry endpoint".to_string());
            }
            registry = Some(value.to_string());
        } else if arg == "--registry" {
            index += 1;
            let Some(value) = args.get(index) else {
                return Err("--registry requires a registry endpoint".to_string());
            };
            registry = Some(value.clone());
        } else if arg.starts_with('-') {
            return Err(usage.to_string());
        } else {
            positional.push(arg.clone());
        }
        index += 1;
    }
    let [package, user] = positional.as_slice() else {
        return Err(usage.to_string());
    };
    let registry = registry.ok_or_else(|| missing_registry.to_string())?;
    Ok((package.clone(), user.clone(), registry))
}
