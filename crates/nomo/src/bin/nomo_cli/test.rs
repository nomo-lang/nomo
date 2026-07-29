use super::cli_common::{filter_projects_by_package, validate_project_package};
use nomo::project::{
    BuildProfile, DependencyResolutionOptions, ProjectTestOptions, clear_requested_build_metadata,
    clear_requested_workspace_build_metadata, discover_project, discover_workspace,
    run_project_tests_with_options,
};
use nomo::target::TargetTriple;
use nomo_test::{json_report, reports_have_failures, text_report};
use std::env;
use std::path::PathBuf;
use std::process;

const TEST_USAGE: &str = "usage: nomo test [path] [--release] [--workspace] [--package <package>] [--filter <text>] [--json] [--locked] [--offline] [--frozen]";

pub(super) fn run_test_command(args: Vec<String>) -> Result<(), String> {
    if args.as_slice() == ["--help"] || args.as_slice() == ["-h"] {
        println!("{TEST_USAGE}");
        return Ok(());
    }
    let (path, workspace, package, filter, json, deps, profile) = parse_test_args(args)?;
    let target = TargetTriple::host()?;
    if workspace {
        clear_requested_workspace_build_metadata(&path, &target, false)
            .map_err(|error| error.human())?;
    } else {
        clear_requested_build_metadata(&path, &target, false).map_err(|error| error.human())?;
    }
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
                        profile,
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
                    profile,
                },
            )
            .map_err(|err| err.human())?,
        );
    }
    if json {
        println!("{}", json_report(&reports));
        if reports_have_failures(&reports) {
            process::exit(1);
        }
        return Ok(());
    }
    print!("{}", text_report(&reports));
    if reports_have_failures(&reports) {
        Err("test failed".to_string())
    } else {
        Ok(())
    }
}

pub(super) fn parse_test_args(
    args: Vec<String>,
) -> Result<
    (
        PathBuf,
        bool,
        Option<String>,
        Option<String>,
        bool,
        DependencyResolutionOptions,
        BuildProfile,
    ),
    String,
> {
    let mut workspace = false;
    let mut package = None;
    let mut filter = None;
    let mut json = false;
    let mut deps = DependencyResolutionOptions::default();
    let mut path = None;
    let mut profile = BuildProfile::Debug;
    let mut release_seen = false;
    let mut index = 0;
    while let Some(arg) = args.get(index) {
        if arg == "--workspace" {
            workspace = true;
        } else if arg == "--release" {
            if release_seen {
                return Err("--release may only be specified once".to_string());
            }
            release_seen = true;
            profile = BuildProfile::Release;
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
            return Err(TEST_USAGE.to_string());
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
        profile,
    ))
}
