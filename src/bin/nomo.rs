#![allow(clippy::result_large_err, clippy::type_complexity)]

use nomo::doc::{
    collect_project_docs, generate_project_docs, generate_std_docs, render_packages_json,
    std_doc_package, write_doc_index,
};
use nomo::project::{
    BuildError, DependencyAddSpec, DependencyResolutionOptions, DependencyUpdateOptions,
    DependencyVendorOptions, ProjectTestOptions, ProjectTestReport, ProjectTestStatus,
    add_registry_dependency, build_project_with_options, check_project, clean_dependency_cache,
    clean_project, create_project, dependency_tree_with_options, discover_project,
    discover_workspace, prepare_publish_package, project_package_id, publish_package_archive,
    remove_dependency, resolve_project_dependencies_with_options,
    resolve_workspace_dependencies_with_options, run_project_tests_with_options,
    run_project_with_args_and_diagnostics, run_standalone_script_with_args_and_diagnostics,
    update_project_dependencies, update_workspace_dependencies, vendor_project_dependencies,
    vendor_workspace_dependencies,
};
use nomo::{Diagnostic, format_source};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
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
        "fmt" => {
            let (path, check, json) = parse_fmt_args(args)?;
            match format_path(&path, check) {
                Ok(changed) if check && changed => Err("format check failed".to_string()),
                Ok(changed) => {
                    if !changed {
                        println!("all files already formatted");
                    }
                    Ok(())
                }
                Err(FormatError::Diagnostic(diag)) if json => Err(diag.json()),
                Err(err) => Err(err.human()),
            }
        }
        "test" => {
            let (path, workspace, package, filter, json, deps) = parse_test_args(args)?;
            let mut reports = Vec::new();
            if workspace {
                let mut projects = discover_workspace(&path)?.members;
                if let Some(package) = package.as_deref() {
                    projects = filter_projects_by_package(projects, package)?;
                }
                for project in projects {
                    reports.push(
                        run_project_tests_with_options(
                            &project,
                            ProjectTestOptions {
                                filter: filter.clone(),
                                resolution: deps,
                            },
                        )
                        .map_err(|err| err.human())?,
                    );
                }
            } else {
                let project = discover_project(&path)?;
                if let Some(package) = package.as_deref() {
                    validate_project_package(&project, package)?;
                }
                reports.push(
                    run_project_tests_with_options(
                        &project,
                        ProjectTestOptions {
                            filter,
                            resolution: deps,
                        },
                    )
                    .map_err(|err| err.human())?,
                );
            }
            if json {
                println!("{}", test_reports_json(&reports));
                if reports_have_failures(&reports) {
                    process::exit(1);
                }
                return Ok(());
            }
            print_test_reports(&reports);
            if reports_have_failures(&reports) {
                Err("test failed".to_string())
            } else {
                Ok(())
            }
        }
        "doc" => {
            let (path, path_provided, workspace, package, include_std, open, json, output) =
                parse_doc_args(args)?;
            if json && open {
                return Err("--open is not valid with --json".to_string());
            }
            let mut packages = Vec::new();
            let mut output_root = output;
            if workspace {
                let workspace = discover_workspace(&path)?;
                output_root = Some(output_root.unwrap_or_else(|| workspace.root.join("build/doc")));
                let mut projects = workspace.members;
                if let Some(package) = package.as_deref() {
                    projects = filter_projects_by_package(projects, package)?;
                }
                for project in projects {
                    let package_id = project_package_id(&project)?;
                    if json {
                        packages.push(
                            collect_project_docs(&project, &package_id)
                                .map_err(|err| err.human())?,
                        );
                    } else {
                        packages.push(
                            generate_project_docs(
                                &project,
                                output_root.as_ref().expect("doc output root"),
                            )
                            .map_err(|err| err.human())?,
                        );
                    }
                }
            } else if path_provided || !include_std {
                let project = discover_project(&path)?;
                if let Some(package) = package.as_deref() {
                    validate_project_package(&project, package)?;
                }
                output_root = Some(output_root.unwrap_or_else(|| project.root.join("build/doc")));
                let package_id = project_package_id(&project)?;
                if json {
                    packages.push(
                        collect_project_docs(&project, &package_id).map_err(|err| err.human())?,
                    );
                } else {
                    packages.push(
                        generate_project_docs(
                            &project,
                            output_root.as_ref().expect("doc output root"),
                        )
                        .map_err(|err| err.human())?,
                    );
                }
            }
            if include_std {
                output_root = Some(output_root.unwrap_or_else(|| {
                    env::current_dir()
                        .unwrap_or_else(|_| PathBuf::from("."))
                        .join("build/doc")
                }));
                if json {
                    packages.push(std_doc_package());
                } else {
                    packages.push(
                        generate_std_docs(output_root.as_ref().expect("doc output root"))
                            .map_err(|err| err.human())?,
                    );
                }
            }
            if json {
                println!("{}", render_packages_json(&packages));
                return Ok(());
            }
            let output_root = output_root.unwrap_or_else(|| path.join("build/doc"));
            let docs = write_doc_index(&output_root, &packages).map_err(|err| err.human())?;
            println!("documented {}", docs.display());
            if open {
                open_doc_index(&docs)?;
            }
            Ok(())
        }
        "clean" => {
            let path = parse_optional_path(args, "usage: nomo clean [path]")?;
            let project = discover_project(&path)?;
            let cleaned = clean_project(&project)?;
            println!("cleaned {}", cleaned.display());
            Ok(())
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

fn parse_path_json_workspace(
    args: Vec<String>,
    usage: &str,
) -> Result<(PathBuf, bool, bool), String> {
    let mut json = false;
    let mut workspace = false;
    let mut path = None;
    for arg in args {
        if arg == "--json-errors" {
            json = true;
        } else if arg == "--workspace" {
            workspace = true;
        } else if path.is_none() {
            path = Some(PathBuf::from(arg));
        } else {
            return Err(usage.to_string());
        }
    }
    Ok((
        path.unwrap_or(env::current_dir().map_err(|err| err.to_string())?),
        json,
        workspace,
    ))
}

fn parse_build_args(
    args: Vec<String>,
    usage: &str,
) -> Result<(PathBuf, bool, bool, bool, DependencyResolutionOptions), String> {
    let mut emit_c = false;
    let mut json = false;
    let mut workspace = false;
    let mut deps = DependencyResolutionOptions::default();
    let mut path = None;
    for arg in args {
        if arg == "--emit-c" {
            emit_c = true;
        } else if arg == "--json-errors" {
            json = true;
        } else if arg == "--workspace" {
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
        emit_c,
        json,
        workspace,
        deps,
    ))
}

fn parse_optional_path(args: Vec<String>, usage: &str) -> Result<PathBuf, String> {
    match args.as_slice() {
        [] => env::current_dir().map_err(|err| err.to_string()),
        [path] => Ok(PathBuf::from(path)),
        _ => Err(usage.to_string()),
    }
}

fn parse_add_args(args: Vec<String>, usage: &str) -> Result<(PathBuf, DependencyAddSpec), String> {
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

fn parse_remove_args(args: Vec<String>, usage: &str) -> Result<(PathBuf, String), String> {
    let current_dir = || env::current_dir().map_err(|err| err.to_string());
    match args.as_slice() {
        [alias] => Ok((current_dir()?, alias.clone())),
        [alias, path] => Ok((PathBuf::from(path), alias.clone())),
        _ => Err(usage.to_string()),
    }
}

fn parse_publish_args(
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

fn parse_deps_args(
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

fn parse_deps_update_args(
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

fn parse_deps_vendor_args(
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

fn parse_run_args(args: Vec<String>) -> Result<(PathBuf, Vec<String>, bool), String> {
    let current_dir = || env::current_dir().map_err(|err| err.to_string());
    let mut path = None;
    let mut json = false;
    let mut index = 0;
    while let Some(arg) = args.get(index) {
        if arg == "--" {
            return Ok((
                path.unwrap_or(current_dir()?),
                args.into_iter().skip(index + 1).collect(),
                json,
            ));
        }
        if arg == "--json-errors" {
            json = true;
        } else if path.is_none() {
            path = Some(PathBuf::from(arg));
        } else {
            return Err("usage: nomo run [path] [--json-errors] [-- args...]".to_string());
        }
        index += 1;
    }
    Ok((path.unwrap_or(current_dir()?), Vec::new(), json))
}

fn parse_fmt_args(args: Vec<String>) -> Result<(PathBuf, bool, bool), String> {
    let mut check = false;
    let mut json = false;
    let mut path = None;
    for arg in args {
        if arg == "--check" {
            check = true;
        } else if arg == "--json-errors" {
            json = true;
        } else if path.is_none() {
            path = Some(PathBuf::from(arg));
        } else {
            return Err("usage: nomo fmt [path] [--check] [--json-errors]".to_string());
        }
    }
    Ok((
        path.unwrap_or(env::current_dir().map_err(|err| err.to_string())?),
        check,
        json,
    ))
}

fn parse_test_args(
    args: Vec<String>,
) -> Result<
    (
        PathBuf,
        bool,
        Option<String>,
        Option<String>,
        bool,
        DependencyResolutionOptions,
    ),
    String,
> {
    let usage = "usage: nomo test [path] [--workspace] [--package <package>] [--filter <text>] [--json] [--locked] [--offline] [--frozen]";
    let mut workspace = false;
    let mut package = None;
    let mut filter = None;
    let mut json = false;
    let mut deps = DependencyResolutionOptions::default();
    let mut path = None;
    let mut index = 0;
    while let Some(arg) = args.get(index) {
        if arg == "--workspace" {
            workspace = true;
        } else if let Some(value) = arg.strip_prefix("--package=") {
            if value.is_empty() {
                return Err("--package requires a package id or name".to_string());
            }
            package = Some(value.to_string());
        } else if arg == "--package" {
            index += 1;
            let Some(value) = args.get(index) else {
                return Err("--package requires a package id or name".to_string());
            };
            package = Some(value.clone());
        } else if let Some(value) = arg.strip_prefix("--filter=") {
            if value.is_empty() {
                return Err("--filter requires text".to_string());
            }
            filter = Some(value.to_string());
        } else if arg == "--filter" {
            index += 1;
            let Some(value) = args.get(index) else {
                return Err("--filter requires text".to_string());
            };
            filter = Some(value.clone());
        } else if arg == "--json" {
            json = true;
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
        index += 1;
    }
    Ok((
        path.unwrap_or(env::current_dir().map_err(|err| err.to_string())?),
        workspace,
        package,
        filter,
        json,
        deps,
    ))
}

fn parse_doc_args(
    args: Vec<String>,
) -> Result<
    (
        PathBuf,
        bool,
        bool,
        Option<String>,
        bool,
        bool,
        bool,
        Option<PathBuf>,
    ),
    String,
> {
    let usage = "usage: nomo doc [path] [--workspace] [--package <package>] [--std] [--open] [--json] [--output <dir>]";
    let mut workspace = false;
    let mut package = None;
    let mut include_std = false;
    let mut open = false;
    let mut json = false;
    let mut output = None;
    let mut path = None;
    let mut index = 0;
    while let Some(arg) = args.get(index) {
        if arg == "--workspace" {
            workspace = true;
        } else if let Some(value) = arg.strip_prefix("--package=") {
            if value.is_empty() {
                return Err("--package requires a package id or name".to_string());
            }
            package = Some(value.to_string());
        } else if arg == "--package" {
            index += 1;
            let Some(value) = args.get(index) else {
                return Err("--package requires a package id or name".to_string());
            };
            package = Some(value.clone());
        } else if arg == "--std" {
            include_std = true;
        } else if arg == "--open" {
            open = true;
        } else if arg == "--json" {
            json = true;
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
        } else if path.is_none() {
            path = Some(PathBuf::from(arg));
        } else {
            return Err(usage.to_string());
        }
        index += 1;
    }
    let path_provided = path.is_some();
    Ok((
        path.unwrap_or(env::current_dir().map_err(|err| err.to_string())?),
        path_provided,
        workspace,
        package,
        include_std,
        open,
        json,
        output,
    ))
}

fn filter_projects_by_package(
    projects: Vec<nomo::project::Project>,
    package: &str,
) -> Result<Vec<nomo::project::Project>, String> {
    let mut matched = Vec::new();
    for project in projects {
        if project_matches_package(&project, package)? {
            matched.push(project);
        }
    }
    if matched.is_empty() {
        return Err(format!("workspace package `{package}` was not found"));
    }
    Ok(matched)
}

fn validate_project_package(project: &nomo::project::Project, package: &str) -> Result<(), String> {
    if project_matches_package(project, package)? {
        Ok(())
    } else {
        Err(format!("project package does not match `{package}`"))
    }
}

fn project_matches_package(
    project: &nomo::project::Project,
    package: &str,
) -> Result<bool, String> {
    let package_id = project_package_id(project)?;
    Ok(package_id == package || project.name == package)
}

fn print_test_reports(reports: &[ProjectTestReport]) {
    let total = reports
        .iter()
        .map(|report| report.tests.len())
        .sum::<usize>();
    println!("running {total} tests");
    for report in reports {
        for test in &report.tests {
            match test.status {
                ProjectTestStatus::Ok => println!("ok {}", test.name),
                ProjectTestStatus::Failed => {
                    println!("fail {}", test.name);
                    if let Some(message) = test.message.as_deref() {
                        println!("  {message}");
                    }
                }
            }
        }
    }
}

fn reports_have_failures(reports: &[ProjectTestReport]) -> bool {
    reports.iter().any(ProjectTestReport::has_failures)
}

fn test_reports_json(reports: &[ProjectTestReport]) -> String {
    let status = if reports_have_failures(reports) {
        "failed"
    } else {
        "ok"
    };
    let mut json = format!("{{\"status\":\"{status}\",\"tests\":[");
    let mut first = true;
    for report in reports {
        for test in &report.tests {
            if !first {
                json.push(',');
            }
            first = false;
            let status = match test.status {
                ProjectTestStatus::Ok => "ok",
                ProjectTestStatus::Failed => "failed",
            };
            json.push_str(&format!(
                "{{\"name\":\"{}\",\"status\":\"{}\",\"duration_ms\":{}",
                json_escape(&test.name),
                status,
                test.duration_ms
            ));
            if let Some(message) = test.message.as_deref() {
                json.push_str(&format!(",\"message\":\"{}\"", json_escape(message)));
            }
            json.push('}');
        }
    }
    json.push_str("]}");
    json
}

fn json_escape(value: &str) -> String {
    value
        .chars()
        .flat_map(|ch| match ch {
            '"' => "\\\"".chars().collect::<Vec<_>>(),
            '\\' => "\\\\".chars().collect::<Vec<_>>(),
            '\n' => "\\n".chars().collect::<Vec<_>>(),
            '\r' => "\\r".chars().collect::<Vec<_>>(),
            '\t' => "\\t".chars().collect::<Vec<_>>(),
            ch if ch.is_control() => format!("\\u{:04x}", ch as u32).chars().collect(),
            ch => vec![ch],
        })
        .collect()
}

fn open_doc_index(path: &Path) -> Result<(), String> {
    let index = path.join("index.html");
    if !index.is_file() {
        return Err(format!("failed to open missing {}", index.display()));
    }
    if env::var_os("NOMO_DOC_OPEN").as_deref() == Some(std::ffi::OsStr::new("0")) {
        return Ok(());
    }
    let status = if cfg!(target_os = "macos") {
        process::Command::new("open").arg(&index).status()
    } else if cfg!(target_os = "windows") {
        process::Command::new("cmd")
            .args(["/C", "start", ""])
            .arg(&index)
            .status()
    } else {
        process::Command::new("xdg-open").arg(&index).status()
    }
    .map_err(|err| format!("failed to open {}: {err}", index.display()))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("failed to open {}", index.display()))
    }
}

#[derive(Debug)]
enum FormatError {
    Diagnostic(Diagnostic),
    Message(String),
}

impl FormatError {
    fn human(&self) -> String {
        match self {
            FormatError::Diagnostic(diagnostic) => diagnostic.human(),
            FormatError::Message(message) => message.clone(),
        }
    }
}

fn format_path(path: &Path, check: bool) -> Result<bool, FormatError> {
    let files = format_targets(path)?;
    let mut changed = false;
    for file in files {
        let source = fs::read_to_string(&file).map_err(|err| {
            FormatError::Message(format!("failed to read {}: {err}", file.display()))
        })?;
        let formatted = format_source(&file, &source).map_err(FormatError::Diagnostic)?;
        if formatted != source {
            changed = true;
            if check {
                println!("would format {}", file.display());
            } else {
                fs::write(&file, formatted).map_err(|err| {
                    FormatError::Message(format!("failed to write {}: {err}", file.display()))
                })?;
                println!("formatted {}", file.display());
            }
        }
    }
    Ok(changed)
}

fn format_targets(path: &Path) -> Result<Vec<PathBuf>, FormatError> {
    if path.extension().and_then(|ext| ext.to_str()) == Some("nomo") {
        if !path.is_file() {
            return Err(FormatError::Message(format!(
                "source file not found: {}",
                path.display()
            )));
        }
        return Ok(vec![path.to_path_buf()]);
    }

    match discover_project(path) {
        Ok(project) => return format_project_targets(&project.root),
        Err(project_err) => {
            if let Ok(workspace) = discover_workspace(path) {
                let mut files = Vec::new();
                for project in workspace.members {
                    files.extend(format_project_targets(&project.root)?);
                }
                files.sort();
                files.dedup();
                return Ok(files);
            }
            if !is_missing_manifest_error(&project_err) || !path.is_dir() {
                return Err(FormatError::Message(project_err));
            }
        }
    }

    let mut files = Vec::new();
    collect_nomo_files(path, &mut files)?;
    files.sort();
    if files.is_empty() {
        return Err(FormatError::Message(format!(
            "no .nomo files found under {}",
            path.display()
        )));
    }
    Ok(files)
}

fn format_project_targets(root: &Path) -> Result<Vec<PathBuf>, FormatError> {
    let src = root.join("src");
    if !src.is_dir() {
        return Err(FormatError::Message(format!(
            "source directory not found: {}",
            src.display()
        )));
    }
    let mut files = Vec::new();
    collect_nomo_files(&src, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect_nomo_files(dir: &Path, files: &mut Vec<PathBuf>) -> Result<(), FormatError> {
    for entry in fs::read_dir(dir).map_err(|err| {
        FormatError::Message(format!("failed to read directory {}: {err}", dir.display()))
    })? {
        let entry = entry.map_err(|err| FormatError::Message(err.to_string()))?;
        let path = entry.path();
        if path.is_dir() {
            collect_nomo_files(&path, files)?;
        } else if path.is_file() && path.extension().and_then(|ext| ext.to_str()) == Some("nomo") {
            files.push(path);
        }
    }
    Ok(())
}

fn is_nomo_source_file(path: &Path) -> bool {
    path.extension().and_then(|ext| ext.to_str()) == Some("nomo")
}

fn is_missing_manifest_error(message: &str) -> bool {
    message.starts_with("could not find nomo.toml")
}

fn print_help() {
    println!(
        "nomo 0.1.0\n\nCommands:\n  nomo new <name>\n  nomo check [path] [--json-errors] [--workspace]\n  nomo build [path] [--emit-c] [--json-errors] [--workspace] [--locked] [--offline] [--frozen]\n  nomo run [path] [--json-errors] [-- args...]\n  nomo fmt [path] [--check] [--json-errors]\n  nomo test [path] [--workspace] [--package <package>] [--filter <text>] [--json] [--locked] [--offline] [--frozen]\n  nomo doc [path] [--workspace] [--package <package>] [--std] [--open] [--json] [--output <dir>]\n  nomo clean [path]\n  nomo add <alias>@<owner>/<package>:<version> [path] [--registry <url>]\n  nomo remove <alias> [path]\n  nomo publish [path] (--dry-run | --registry <url>) [--output <dir>] [--json-errors]\n  nomo deps <resolve|tree> [path] [--workspace] [--locked] [--offline] [--frozen]\n  nomo deps update [path] [alias-or-package] [--workspace] [--offline] [--precise <version-or-rev>]\n  nomo deps vendor [path] [--workspace] [--dir vendor] [--sync]\n  nomo deps clean-cache [path]\n"
    );
}
