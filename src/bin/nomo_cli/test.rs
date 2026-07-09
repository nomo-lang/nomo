use super::cli_common::{filter_projects_by_package, validate_project_package};
use nomo::project::{
    DependencyResolutionOptions, ProjectTestOptions, ProjectTestReport, ProjectTestStatus,
    discover_project, discover_workspace, run_project_tests_with_options,
};
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

pub(super) fn print_test_reports(reports: &[ProjectTestReport]) {
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

pub(super) fn reports_have_failures(reports: &[ProjectTestReport]) -> bool {
    reports.iter().any(ProjectTestReport::has_failures)
}

pub(super) fn test_reports_json(reports: &[ProjectTestReport]) -> String {
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
