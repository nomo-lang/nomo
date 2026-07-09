#![allow(clippy::result_large_err, clippy::type_complexity)]

#[path = "nomo_cli/common.rs"]
mod cli_common;
#[path = "nomo_cli/deps.rs"]
mod cli_deps;
#[path = "nomo_cli/doc.rs"]
mod cli_doc;
#[path = "nomo_cli/fmt.rs"]
mod cli_fmt;
#[path = "nomo_cli/registry.rs"]
mod cli_registry;
#[path = "nomo_cli/test.rs"]
mod cli_test;

use cli_common::{
    is_missing_manifest_error, is_nomo_source_file, parse_build_args, parse_optional_path,
    parse_path_json_workspace, parse_run_args,
};
use cli_deps::{parse_deps_args, parse_deps_update_args, parse_deps_vendor_args};
use cli_doc::run_doc_command;
use cli_fmt::run_fmt_command;
use cli_registry::{
    parse_add_args, parse_login_args, parse_owner_add_args, parse_owner_remove_args,
    parse_publish_args, parse_remove_args, parse_search_args, parse_yank_args,
};
use cli_test::run_test_command;
use nomo::project::{
    BuildError, add_registry_dependency, add_registry_package_owner, build_project_with_options,
    check_project, clean_dependency_cache, clean_project, create_project,
    dependency_tree_with_options, discover_project, discover_workspace, login_registry,
    prepare_publish_package, publish_package_archive, remove_dependency,
    remove_registry_package_owner, resolve_project_dependencies_with_options,
    resolve_workspace_dependencies_with_options, run_project_with_args_and_diagnostics,
    run_standalone_script_with_args_and_diagnostics, search_registry_packages,
    update_project_dependencies, update_workspace_dependencies, vendor_project_dependencies,
    vendor_workspace_dependencies, yank_registry_package,
};
use std::env;
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
        "new" => {
            let [name] = args.as_slice() else {
                return Err("usage: nomo new <name>".to_string());
            };
            let project =
                create_project(&env::current_dir().map_err(|err| err.to_string())?, name)?;
            println!("created {}", project.root.display());
            Ok(())
        }
        "check" => {
            let (path, json, workspace) = parse_path_json_workspace(
                args,
                "usage: nomo check [path] [--json-errors] [--workspace]",
            )?;
            if workspace {
                for project in discover_workspace(&path)?.members {
                    match check_project(&project) {
                        Ok(()) => println!("checked {}", project.main.display()),
                        Err(diag) if json => return Err(diag.json()),
                        Err(diag) => return Err(diag.human()),
                    }
                }
                Ok(())
            } else {
                let project = discover_project(&path)?;
                match check_project(&project) {
                    Ok(()) => {
                        println!("checked {}", project.main.display());
                        Ok(())
                    }
                    Err(diag) if json => Err(diag.json()),
                    Err(diag) => Err(diag.human()),
                }
            }
        }
        "build" => {
            let (path, emit_c, json, workspace, deps) = parse_build_args(
                args,
                "usage: nomo build [path] [--emit-c] [--json-errors] [--workspace] [--locked] [--offline] [--frozen]",
            )?;
            if workspace {
                for project in discover_workspace(&path)?.members {
                    let artifact = match build_project_with_options(&project, emit_c, deps) {
                        Ok(artifact) => artifact,
                        Err(BuildError::Diagnostic(diag)) if json => return Err(diag.json()),
                        Err(err) => return Err(err.human()),
                    };
                    println!("built {}", artifact.display());
                }
            } else {
                let project = discover_project(&path)?;
                let artifact = match build_project_with_options(&project, emit_c, deps) {
                    Ok(artifact) => artifact,
                    Err(BuildError::Diagnostic(diag)) if json => return Err(diag.json()),
                    Err(err) => return Err(err.human()),
                };
                println!("built {}", artifact.display());
            }
            Ok(())
        }
        "run" => {
            let (path, program_args, json) = parse_run_args(args)?;
            let code = match discover_project(&path) {
                Ok(project) => {
                    match run_project_with_args_and_diagnostics(&project, &program_args) {
                        Ok(code) => code,
                        Err(BuildError::Diagnostic(diag)) if json => return Err(diag.json()),
                        Err(err) => return Err(err.human()),
                    }
                }
                Err(err) if is_nomo_source_file(&path) && is_missing_manifest_error(&err) => {
                    match run_standalone_script_with_args_and_diagnostics(&path, &program_args) {
                        Ok(code) => code,
                        Err(BuildError::Diagnostic(diag)) if json => return Err(diag.json()),
                        Err(err) => return Err(err.human()),
                    }
                }
                Err(err) => return Err(err),
            };
            if code == 0 {
                Ok(())
            } else {
                Err(format!("program exited with status {code}"))
            }
        }
        "fmt" => run_fmt_command(args),
        "test" => run_test_command(args),
        "doc" => run_doc_command(args),
        "clean" => {
            let path = parse_optional_path(args, "usage: nomo clean [path]")?;
            let project = discover_project(&path)?;
            let cleaned = clean_project(&project)?;
            println!("cleaned {}", cleaned.display());
            Ok(())
        }
        "login" => {
            let (registry, token) =
                parse_login_args(args, "usage: nomo login --registry <url> --token <token>")?;
            let login = login_registry(&registry, &token)?;
            println!("logged in {}", login.registry);
            println!("credentials {}", login.credentials_path.display());
            Ok(())
        }
        "owner" => {
            let [subcommand, rest @ ..] = args.as_slice() else {
                return Err(
                    "usage: nomo owner <add|remove> <owner/package> <user> --registry <url>"
                        .to_string(),
                );
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
                other => Err(format!("unknown owner command `{other}`")),
            }
        }
        "add" => {
            let (path, spec) = parse_add_args(
                args,
                "usage: nomo add <alias>@<owner>/<package>:<version> [path] [--registry <url>]",
            )?;
            let project = discover_project(&path)?;
            let manifest = add_registry_dependency(&project, spec)?;
            println!("updated {}", manifest.display());
            Ok(())
        }
        "remove" => {
            let (path, alias) = parse_remove_args(args, "usage: nomo remove <alias> [path]")?;
            let project = discover_project(&path)?;
            let manifest = remove_dependency(&project, &alias)?;
            println!("updated {}", manifest.display());
            Ok(())
        }
        "search" => {
            let (query, registry) =
                parse_search_args(args, "usage: nomo search <query> --registry <url>")?;
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
        "yank" => {
            let (package, version, registry) = parse_yank_args(
                args,
                "usage: nomo yank <owner/package> <version> --registry <url>",
            )?;
            yank_registry_package(&registry, &package, &version)?;
            println!("yanked {package} {version}");
            println!("registry {registry}");
            Ok(())
        }
        "publish" => {
            let (path, dry_run, registry, output, json) = parse_publish_args(
                args,
                "usage: nomo publish [path] (--dry-run | --registry <url>) [--output <dir>] [--json-errors]",
            )?;
            let project = discover_project(&path)?;
            if dry_run {
                let package = match prepare_publish_package(&project, output.as_deref()) {
                    Ok(package) => package,
                    Err(BuildError::Diagnostic(diag)) if json => return Err(diag.json()),
                    Err(err) => return Err(err.human()),
                };
                println!("publish dry-run {} {}", package.package, package.version);
                println!("archive {}", package.archive_path.display());
                println!("checksum {}", package.checksum);
                println!("size {}", package.size);
            } else {
                let registry = registry.ok_or_else(|| {
                    "nomo publish requires either --dry-run or --registry <url>".to_string()
                })?;
                let package = match publish_package_archive(&project, &registry, output.as_deref())
                {
                    Ok(package) => package,
                    Err(BuildError::Diagnostic(diag)) if json => return Err(diag.json()),
                    Err(err) => return Err(err.human()),
                };
                println!("published {} {}", package.package, package.version);
                println!("archive {}", package.archive_path.display());
                println!("checksum {}", package.checksum);
                println!("size {}", package.size);
                println!("registry {registry}");
            }
            Ok(())
        }
        "deps" => {
            let [subcommand, rest @ ..] = args.as_slice() else {
                return Err(
                    "usage: nomo deps <resolve|tree|update|vendor|clean-cache> [path] [--workspace] [--locked] [--offline] [--frozen]".to_string()
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
                    let (path, workspace, deps) = parse_deps_args(
                        rest.to_vec(),
                        &format!(
                            "usage: nomo deps {subcommand} [path] [--workspace] [--locked] [--offline] [--frozen]"
                        ),
                    )?;
                    if workspace {
                        for project in discover_workspace(&path)?.members {
                            print!("{}", dependency_tree_with_options(&project, deps)?);
                        }
                    } else {
                        let project = discover_project(&path)?;
                        print!("{}", dependency_tree_with_options(&project, deps)?);
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
                        let lock =
                            update_workspace_dependencies(&workspace, target.as_deref(), deps)?;
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
                    let path =
                        parse_optional_path(rest.to_vec(), "usage: nomo deps clean-cache [path]")?;
                    let cleaned = clean_dependency_cache(&path)?;
                    println!("cleaned {}", cleaned.display());
                    Ok(())
                }
                other => Err(format!("unknown deps command `{other}`")),
            }
        }
        "help" | "--help" | "-h" => {
            print_help();
            Ok(())
        }
        other => Err(format!("unknown command `{other}`")),
    }
}

fn print_help() {
    println!(
        "nomo 0.1.0\n\nCommands:\n  nomo new <name>\n  nomo check [path] [--json-errors] [--workspace]\n  nomo build [path] [--emit-c] [--json-errors] [--workspace] [--locked] [--offline] [--frozen]\n  nomo run [path] [--json-errors] [-- args...]\n  nomo fmt [path] [--check] [--json-errors]\n  nomo test [path] [--workspace] [--package <package>] [--filter <text>] [--json] [--locked] [--offline] [--frozen]\n  nomo doc [path] [--workspace] [--package <package>] [--std] [--open] [--json] [--output <dir>]\n  nomo clean [path]\n  nomo login --registry <url> --token <token>\n  nomo owner add <owner/package> <user> --registry <url>\n  nomo owner remove <owner/package> <user> --registry <url>\n  nomo add <alias>@<owner>/<package>:<version> [path] [--registry <url>]\n  nomo remove <alias> [path]\n  nomo search <query> --registry <url>\n  nomo yank <owner/package> <version> --registry <url>\n  nomo publish [path] (--dry-run | --registry <url>) [--output <dir>] [--json-errors]\n  nomo deps <resolve|tree> [path] [--workspace] [--locked] [--offline] [--frozen]\n  nomo deps update [path] [alias-or-package] [--workspace] [--offline] [--precise <version-or-rev>]\n  nomo deps vendor [path] [--workspace] [--dir vendor] [--sync]\n  nomo deps clean-cache [path]\n"
    );
}
