use super::*;

#[test]
fn lowers_the_complete_cron_surface() {
    let source = r#"package app.main

import std.cron

fn run() -> Result<void, CronError> {
    let schedule: CronSchedule = cron.parse("*/15 * * * *")?
    let selected: bool = cron.matches(schedule, 0)?
    let next: i64 = cron.next_after(schedule, 0)?
    return Ok(void)
}

fn main() -> void {
    let result: Result<void, CronError> = run()
}
"#;

    let program = parse_inline(source).unwrap();
    for expected in ["CronError", "CronSchedule"] {
        assert!(program.structs.iter().any(|item| item.name == expected));
    }
    let debug = format!("{:?}", program.functions);
    for operation in ["Parse", "Matches", "NextAfter"] {
        assert!(
            debug.contains(&format!("operation: {operation}")),
            "missing cron operation {operation}"
        );
    }
}

#[test]
fn rejects_cron_argument_type_mismatches() {
    let source = r#"package app.main

import std.cron

fn main() -> void {
    let parsed: Result<CronSchedule, CronError> = cron.parse(1)
}
"#;
    let error = parse_inline(source).unwrap_err();
    assert_eq!(error.code, "E0404");
    assert!(error.message.contains("string"));
}

#[test]
fn lowers_specifically_imported_cron_function() {
    let source = r#"package app.main

import std.cron.CronError
import std.cron.CronSchedule
import std.cron.parse

fn main() -> void {
    let parsed: Result<CronSchedule, CronError> = parse("* * * * *")
}
"#;

    let program = parse_inline(source).unwrap();
    assert!(format!("{:?}", program.functions).contains("operation: Parse"));
}

#[test]
fn permits_cron_calculation_inside_isolated_tasks() {
    let source = r#"package app.main

import std.cron
import std.task

fn worker(context: TaskContext, input: string) -> string {
    let parsed: Result<CronSchedule, CronError> = cron.parse(input)
    return input
}

fn main() -> void {
    let started: Result<Task, TaskError> = task.spawn(worker, "* * * * *")
}
"#;

    parse_inline(source).unwrap();
}

#[test]
fn rejects_forged_cron_schedules_and_private_field_access() {
    let forged = r#"package app.main

import std.cron

fn main() -> void {
    let schedule: CronSchedule = CronSchedule { expression: "* * * * *" }
}
"#;
    let error = parse_inline(forged).unwrap_err();
    assert_eq!(error.code, "E0850");
    assert!(error.message.contains("cannot be constructed"));

    let exposed = r#"package app.main

import std.cron
import std.io

fn main() -> void {
    let parsed: Result<CronSchedule, CronError> = cron.parse("* * * * *")
    match parsed {
        Ok(schedule) => {
            io.println(schedule.expression)
        }
        Err(error) => {
        }
    }
}
"#;
    let error = parse_inline(exposed).unwrap_err();
    assert_eq!(error.code, "E0850");
    assert!(error.message.contains("does not expose its fields"));

    let mutated = r#"package app.main

import std.cron

fn run() -> Result<void, CronError> {
    let mut schedule: CronSchedule = cron.parse("* * * * *")?
    schedule.expression = "0 * * * *"
    return Ok(void)
}

fn main() -> void {
    let result: Result<void, CronError> = run()
}
"#;
    let error = parse_inline(mutated).unwrap_err();
    assert_eq!(error.code, "E0850");
    assert!(error.message.contains("does not expose its fields"));
}

#[test]
fn reports_missing_cron_type_import() {
    let source = r#"package app.main

fn keep(schedule: CronSchedule) -> void {
}

fn main() -> void {
}
"#;
    let error = parse_inline(source).unwrap_err();
    assert_eq!(error.code, "E0301");
    assert_eq!(error.message, "`CronSchedule` requires `import std.cron`");
}
