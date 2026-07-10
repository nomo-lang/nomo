use super::cli_common::{filter_projects_by_package, validate_project_package};
use nomo::project::{
    DependencyResolutionOptions, ProjectTestOptions, discover_project, discover_workspace,
    run_project_tests_with_options,
};
use nomo_test::{json_report, reports_have_failures, text_report};
use std::env;
use std::path::PathBuf;
use std::process;

pub(super) fn run_test_command(args: Vec<String>) -> Result<(), String> {
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
