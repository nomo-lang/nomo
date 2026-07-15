use nomo::project::{
    BuildError, DependencyAddSpec, add_registry_dependency, add_registry_package_owner,
    add_registry_publisher_key, discover_project, login_registry, prepare_publish_package,
    publish_package_archive, publish_signed_package_archive, remove_dependency,
    remove_registry_package_owner, revoke_registry_publisher_key, search_registry_packages,
    sign_publish_package, yank_registry_package,
};
use std::env;
use std::path::PathBuf;

pub(super) fn run_login_command(args: Vec<String>) -> Result<(), String> {
    let (registry, token) =
        parse_login_args(args, "usage: nomo login --registry <url> --token <token>")?;
    let login = login_registry(&registry, &token)?;
    println!("logged in {}", login.registry);
    println!("credentials {}", login.credentials_path.display());
    Ok(())
}

pub(super) fn run_owner_command(args: Vec<String>) -> Result<(), String> {
    let [subcommand, rest @ ..] = args.as_slice() else {
        return Err("usage: nomo owner <add|remove|key> ... --registry <url>".to_string());
    };
    match subcommand.as_str() {
        "add" => {
            let (package, user, registry) = parse_owner_add_args(
                rest.to_vec(),
                "usage: nomo owner add <owner/package> <user> --registry <url>",
            )?;
            add_registry_package_owner(&registry, &package, &user)?;
            println!("added owner {user} to {package}");
            println!("registry {registry}");
            Ok(())
        }
        "remove" => {
            let (package, user, registry) = parse_owner_remove_args(
                rest.to_vec(),
                "usage: nomo owner remove <owner/package> <user> --registry <url>",
            )?;
            remove_registry_package_owner(&registry, &package, &user)?;
            println!("removed owner {user} from {package}");
            println!("registry {registry}");
            Ok(())
        }
        "key" => {
            let [action, key_args @ ..] = rest else {
                return Err("usage: nomo owner key <add|revoke> ... --registry <url>".to_string());
            };
            match action.as_str() {
                "add" => {
                    let (package, public_key, registry) = parse_owner_registry_args(
                        key_args.to_vec(),
                        "usage: nomo owner key add <owner/package> <ed25519-public-key-hex> --registry <url>",
                        "nomo owner key add requires --registry <url>",
                    )?;
                    let key_id = add_registry_publisher_key(&registry, &package, &public_key)?;
                    println!("registered publisher key {key_id} for {package}");
                    println!("registry {registry}");
                    Ok(())
                }
                "revoke" => {
                    let (package, key_id, registry) = parse_owner_registry_args(
                        key_args.to_vec(),
                        "usage: nomo owner key revoke <owner/package> <key-id> --registry <url>",
                        "nomo owner key revoke requires --registry <url>",
                    )?;
                    revoke_registry_publisher_key(&registry, &package, &key_id)?;
                    println!("revoked publisher key {key_id} for {package}");
                    println!("registry {registry}");
                    Ok(())
                }
                other => Err(format!("unknown owner key command `{other}`")),
            }
        }
        other => Err(format!("unknown owner command `{other}`")),
    }
}

pub(super) fn run_add_command(args: Vec<String>) -> Result<(), String> {
    let (path, spec) = parse_add_args(
        args,
        "usage: nomo add <alias>@<owner>/<package>:<version> [path] [--registry <url>]",
    )?;
    let project = discover_project(&path)?;
    let manifest = add_registry_dependency(&project, spec)?;
    println!("updated {}", manifest.display());
    Ok(())
}

pub(super) fn run_remove_command(args: Vec<String>) -> Result<(), String> {
    let (path, alias) = parse_remove_args(args, "usage: nomo remove <alias> [path]")?;
    let project = discover_project(&path)?;
    let manifest = remove_dependency(&project, &alias)?;
    println!("updated {}", manifest.display());
    Ok(())
}

pub(super) fn run_search_command(args: Vec<String>) -> Result<(), String> {
    let (query, registry) = parse_search_args(args, "usage: nomo search <query> --registry <url>")?;
    let results = search_registry_packages(&registry, &query)?;
    if results.is_empty() {
        println!("no packages found");
    } else {
        for result in results {
            match (result.version.as_deref(), result.description.as_deref()) {
                (Some(version), Some(description)) => {
                    println!("{} {} - {}", result.package, version, description)
                }
                (Some(version), None) => println!("{} {}", result.package, version),
                (None, Some(description)) => {
                    println!("{} - {}", result.package, description)
                }
                (None, None) => println!("{}", result.package),
            }
        }
    }
    Ok(())
}

pub(super) fn run_yank_command(args: Vec<String>) -> Result<(), String> {
    let (package, version, registry) = parse_yank_args(
        args,
        "usage: nomo yank <owner/package> <version> --registry <url>",
    )?;
    yank_registry_package(&registry, &package, &version)?;
    println!("yanked {package} {version}");
    println!("registry {registry}");
    Ok(())
}

pub(super) fn run_publish_command(args: Vec<String>) -> Result<(), String> {
    let args = parse_publish_args(
        args,
        "usage: nomo publish [path] (--dry-run | --registry <url>) [--output <dir>] [--signer <command>] [--envelope <file>] [--json-errors]",
    )?;
    let project = discover_project(&args.path)?;
    if args.dry_run {
        let package = match prepare_publish_package(&project, args.output.as_deref()) {
            Ok(package) => package,
            Err(BuildError::Diagnostic(diag)) if args.json => return Err(diag.json()),
            Err(err) => return Err(err.human()),
        };
        let package = if let Some(signer) = args.signer.as_deref() {
            sign_publish_package(package, signer, args.envelope.as_deref())
                .map_err(|err| err.human())?
        } else {
            package
        };
        println!("publish dry-run {} {}", package.package, package.version);
        println!("archive {}", package.archive_path.display());
        println!("checksum {}", package.checksum);
        println!("provenance {}", package.provenance_path.display());
        if let Some(envelope) = &package.envelope_path {
            println!("envelope {}", envelope.display());
        }
        println!("size {}", package.size);
    } else {
        let registry = args.registry.ok_or_else(|| {
            "nomo publish requires either --dry-run or --registry <url>".to_string()
        })?;
        let result = if let Some(signer) = args.signer.as_deref() {
            publish_signed_package_archive(
                &project,
                &registry,
                args.output.as_deref(),
                signer,
                args.envelope.as_deref(),
            )
        } else {
            publish_package_archive(&project, &registry, args.output.as_deref())
        };
        let package = match result {
            Ok(package) => package,
            Err(BuildError::Diagnostic(diag)) if args.json => return Err(diag.json()),
            Err(err) => return Err(err.human()),
        };
        println!("published {} {}", package.package, package.version);
        println!("archive {}", package.archive_path.display());
        println!("checksum {}", package.checksum);
        println!("provenance {}", package.provenance_path.display());
        if let Some(envelope) = &package.envelope_path {
            println!("envelope {}", envelope.display());
        }
        println!("size {}", package.size);
        println!("registry {registry}");
    }
    Ok(())
}

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

struct PublishArgs {
    path: PathBuf,
    dry_run: bool,
    registry: Option<String>,
    output: Option<PathBuf>,
    signer: Option<String>,
    envelope: Option<PathBuf>,
    json: bool,
}

fn parse_publish_args(args: Vec<String>, usage: &str) -> Result<PublishArgs, String> {
    let mut dry_run = false;
    let mut registry = None;
    let mut output = None;
    let mut json = false;
    let mut signer = None;
    let mut envelope = None;
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
        } else if let Some(value) = arg.strip_prefix("--signer=") {
            if value.is_empty() {
                return Err("--signer requires an external signer command".to_string());
            }
            signer = Some(value.to_string());
        } else if arg == "--signer" {
            index += 1;
            let Some(value) = args.get(index) else {
                return Err("--signer requires an external signer command".to_string());
            };
            signer = Some(value.clone());
        } else if let Some(value) = arg.strip_prefix("--envelope=") {
            if value.is_empty() {
                return Err("--envelope requires a file path".to_string());
            }
            envelope = Some(PathBuf::from(value));
        } else if arg == "--envelope" {
            index += 1;
            let Some(value) = args.get(index) else {
                return Err("--envelope requires a file path".to_string());
            };
            envelope = Some(PathBuf::from(value));
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
    if envelope.is_some() && signer.is_none() {
        return Err("--envelope requires --signer <command>".to_string());
    }
    Ok(PublishArgs {
        path: path.unwrap_or(env::current_dir().map_err(|err| err.to_string())?),
        dry_run,
        registry,
        output,
        signer,
        envelope,
        json,
    })
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
