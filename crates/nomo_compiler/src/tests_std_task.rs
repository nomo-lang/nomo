use super::*;

#[test]
fn lowers_isolated_task_lifecycle_calls() {
    let source = r#"package app.main

import std.task

fn worker(context: TaskContext, input: string) -> string {
    if task.is_cancelled(context) {
        "cancelled"
    } else {
        input
    }
}

fn main() -> void {
    let started: Result<Task, TaskError> = task.spawn(worker, "hello")
}
"#;

    let program = parse_inline(source).unwrap();
    let main = program
        .functions
        .iter()
        .find(|function| function.name == "main")
        .unwrap();
    assert!(matches!(
        main.body.as_slice(),
        [Statement::Let {
            initializer: ValueExpr::Call { name, args },
            ..
        }] if name == BUILTIN_TASK_SPAWN_EXPR
            && matches!(args.as_slice(), [ValueExpr::FunctionRef(worker), ValueExpr::StringLiteral(input)]
                if worker == "worker" && input == "hello")
    ));
    let worker = program
        .functions
        .iter()
        .find(|function| function.name == "worker")
        .unwrap();
    assert!(worker.body.iter().any(|statement| {
        statement_contains_task_call(statement, BUILTIN_TASK_IS_CANCELLED_EXPR)
    }));
}

#[test]
fn rejects_task_worker_with_wrong_signature() {
    let source = r#"package app.main

import std.task

fn worker(input: string) -> string {
    return input
}

fn main() -> void {
    let started: Result<Task, TaskError> = task.spawn(worker, "hello")
}
"#;

    let error = parse_inline(source).unwrap_err();
    assert_eq!(error.code, "E0820");
    assert!(error.message.contains("fn(TaskContext, string) -> string"));
}

#[test]
fn rejects_task_callback_type_outside_canonical_std_spawn() {
    let source = r#"package app.main

fn misuse(worker: task fn(string) -> string) -> void {
}

fn main() -> void {
}
"#;

    let error = parse_inline(source).unwrap_err();
    assert_eq!(error.code, "E0820");
    assert!(error.message.contains("canonical `std.task.spawn`"));
}

#[test]
fn rejects_forged_task_handles_and_context_field_access() {
    let forged = r#"package app.main

import std.task

fn main() -> void {
    let forged: Task = Task { handle: 1 }
}
"#;
    let error = parse_inline(forged).unwrap_err();
    assert_eq!(error.code, "E0820");
    assert!(error.message.contains("cannot be constructed"));

    let exposed = r#"package app.main

import std.num
import std.task

fn worker(context: TaskContext, input: string) -> string {
    return num.to_string(context.handle)
}

fn main() -> void {
    let started: Result<Task, TaskError> = task.spawn(worker, "hello")
}
"#;
    let error = parse_inline(exposed).unwrap_err();
    assert_eq!(error.code, "E0820");
    assert!(error.message.contains("does not expose its fields"));

    let mutated = r#"package app.main

import std.task

fn worker(context: TaskContext, input: string) -> string {
    return input
}

fn run() -> Result<void, TaskError> {
    let mut started: Task = task.spawn(worker, "hello")?
    started.handle = 1
    return Ok(void)
}

fn main() -> void {
    let result: Result<void, TaskError> = run()
}
"#;
    let error = parse_inline(mutated).unwrap_err();
    assert_eq!(error.code, "E0820");
    assert!(error.message.contains("does not expose its fields"));
}

#[test]
fn rejects_transitive_task_unsafe_effect_with_a_call_path() {
    let source = r#"package app.main

import std.io
import std.task

fn helper(input: string) -> string {
    io.println(input)
    return input
}

fn worker(context: TaskContext, input: string) -> string {
    return helper(input)
}

fn main() -> void {
    let started: Result<Task, TaskError> = task.spawn(worker, "secret")
}
"#;

    let error = parse_inline(source).unwrap_err();
    assert_eq!(error.code, "E0821");
    assert!(error.message.contains("worker -> helper"));
    assert!(error.message.contains("io.println"));
    assert!(!error.message.contains("secret"));
}

#[test]
fn rejects_nested_task_spawn_from_a_worker() {
    let source = r#"package app.main

import std.task

fn worker(context: TaskContext, input: string) -> string {
    let nested: Result<Task, TaskError> = task.spawn(worker, input)
    return input
}

fn main() -> void {
    let started: Result<Task, TaskError> = task.spawn(worker, "hello")
}
"#;

    let error = parse_inline(source).unwrap_err();
    assert_eq!(error.code, "E0821");
    assert!(error.message.contains("task.spawn"));
}

#[test]
fn rejects_local_methods_that_alias_task_safe_value_operations() {
    let source = r#"package app.main

import std.io
import std.task

struct Probe {
    value: string
}

impl Probe {
    fn get(self) -> string {
        io.println(self.value)
        return self.value
    }
}

fn worker(context: TaskContext, input: string) -> string {
    let probe: Probe = Probe { value: input }
    return probe.get()
}

fn main() -> void {
    let started: Result<Task, TaskError> = task.spawn(worker, "secret")
}
"#;

    let error = parse_inline(source).unwrap_err();
    assert_eq!(error.code, "E0821");
    assert!(
        error
            .message
            .contains("local method `get` with unclassified effects")
    );
    assert!(!error.message.contains("secret"));
}

#[test]
fn emits_task_shutdown_after_an_early_returning_main() {
    let source = r#"package app.main

import std.task

fn worker(context: TaskContext, input: string) -> string {
    return input
}

fn main() -> void {
    let started: Result<Task, TaskError> = task.spawn(worker, "hello")
    return
}
"#;

    let program = parse_inline(source).unwrap();
    let generated = nomo_codegen_c::emit_c(&program);

    assert!(generated.contains("void nomo_fn_main(void)"));
    assert!(generated.contains("nomo_fn_main();\n    nomo_task_shutdown();"));
}

fn statement_contains_task_call(statement: &Statement, expected: &str) -> bool {
    match statement {
        Statement::If { condition, .. } => {
            matches!(condition, ValueExpr::Call { name, .. } if name == expected)
        }
        Statement::Return(Some(ValueExpr::If { condition, .. })) => {
            matches!(condition.as_ref(), ValueExpr::Call { name, .. } if name == expected)
        }
        _ => false,
    }
}
