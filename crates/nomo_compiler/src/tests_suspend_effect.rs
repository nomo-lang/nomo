use super::*;

#[test]
fn lowers_suspend_effect_into_typed_ir() {
    let source = r#"package app.main

fn normalize(value: string) -> string {
    return value
}

suspend fn load(value: string) -> string {
    return normalize(value)
}

suspend fn main() -> void {
    let value: string = load("ready")
}
"#;

    let program = parse_inline(source).unwrap();
    let normalize = program
        .functions
        .iter()
        .find(|function| function.name == "normalize")
        .unwrap();
    let load = program
        .functions
        .iter()
        .find(|function| function.name == "load")
        .unwrap();
    let main = program
        .functions
        .iter()
        .find(|function| function.name == "main")
        .unwrap();

    assert!(!normalize.is_suspend);
    assert!(load.is_suspend);
    assert!(main.is_suspend);
}

#[test]
fn rejects_suspend_call_from_synchronous_function() {
    let source = r#"package app.main

suspend fn load() -> string {
    return "ready"
}

fn main() -> void {
    let value: string = load()
}
"#;

    let error = parse_inline(source).unwrap_err();

    assert_eq!(error.code, "E0870");
    assert!(error.message.contains("synchronous function"));
    assert!(error.message.contains("suspend function `load`"));
    assert!(error.message.contains("mark the caller `suspend`"));
    assert_eq!(error.line, 8);
}

#[test]
fn preserves_suspend_effect_for_generic_instances() {
    let source = r#"package app.main

suspend fn identity<T>(value: T) -> T {
    return value
}

suspend fn main() -> void {
    let value: string = identity<string>("ready")
}
"#;

    let program = parse_inline(source).unwrap();
    let instance = program
        .functions
        .iter()
        .find(|function| function.name.starts_with("identity_"))
        .unwrap();

    assert!(instance.is_suspend);
}

#[test]
fn rejects_suspend_method_call_from_synchronous_function() {
    let source = r#"package app.main

struct Client {
}

impl Client {
    suspend fn load(self) -> string {
        return "ready"
    }
}

fn main() -> void {
    let client: Client = Client {}
    let value: string = client.load()
}
"#;

    let error = parse_inline(source).unwrap_err();

    assert_eq!(error.code, "E0870");
    assert!(error.message.contains("Client.load"));
    assert_eq!(error.line, 14);
}

#[test]
fn requires_interface_implementations_to_match_suspend_effect() {
    let source = r#"package app.main

interface Loader {
    suspend fn load(self) -> string
}

struct Client {
}

impl Loader for Client {
    fn load(self) -> string {
        return "ready"
    }
}

fn main() -> void {
}
"#;

    let error = parse_inline(source).unwrap_err();

    assert_eq!(error.code, "E0258");
    assert!(error.message.contains("suspend effect does not match"));
}

#[test]
fn suspend_worker_is_not_a_legacy_task_callback() {
    let source = r#"package app.main

import std.task

suspend fn worker(context: TaskContext, input: string) -> string {
    return input
}

fn main() -> void {
    let started: Result<Task, TaskError> = task.spawn(worker, "ready")
}
"#;

    let error = parse_inline(source).unwrap_err();

    assert_eq!(error.code, "E0820");
    assert!(error.message.contains("must have signature"));
}

#[test]
fn synchronous_codegen_has_no_async_runtime_or_frame_metadata() {
    let source = r#"package app.main

fn helper() -> string {
    return "ready"
}

fn main() -> void {
    let value: string = helper()
}
"#;

    let c = compile_source_text_to_c_with_project_modules(
        Path::new("main.nomo"),
        source,
        None,
        &[],
        &[],
    )
    .unwrap();

    assert!(!c.contains("nomo_poll"));
    assert!(!c.contains("nomo_frame"));
    assert!(!c.contains("nomo_executor"));
    assert!(!c.contains("nomo_reactor"));
}

#[test]
fn always_ready_suspend_codegen_has_no_async_runtime_or_frame_metadata() {
    let source = r#"package app.main

suspend fn ready() -> string {
    return "ready"
}

suspend fn main() -> void {
    let value: string = ready()
}
"#;

    let c = compile_source_text_to_c_with_project_modules(
        Path::new("main.nomo"),
        source,
        None,
        &[],
        &[],
    )
    .unwrap();

    assert!(!c.contains("nomo_async_"));
    assert!(!c.contains("nomo_executor"));
    assert!(!c.contains("nomo_reactor"));
}

#[test]
fn yield_now_requires_a_suspend_function() {
    let source = r#"package app.main

import std.task

fn main() -> void {
    task.yield_now()
}
"#;

    let error = parse_inline(source).unwrap_err();

    assert_eq!(error.code, "E0870");
    assert!(error.message.contains("task.yield_now"));
    assert!(error.message.contains("mark the caller `suspend`"));
}

#[test]
fn specifically_imported_yield_now_lowers_to_the_current_thread_executor() {
    let source = r#"package app.main

import std.task.yield_now

suspend fn main() -> void {
    yield_now()
}
"#;

    let c = compile_source_text_to_c_with_project_modules(
        Path::new("main.nomo"),
        source,
        None,
        &[],
        &[],
    )
    .unwrap();

    assert!(c.contains("nomo_async_frame_main"));
    assert!(c.contains("nomo_async_executor_run_root"));
    assert!(c.contains("context->yield_count += 1u;"));
}

#[test]
fn root_yield_lowers_to_a_stackless_current_thread_executor() {
    let source = r#"package app.main

import std.io
import std.task

suspend fn main() -> void {
    io.println("before")
    task.yield_now()
    io.println("middle")
    task.yield_now()
    io.println("after")
}
"#;

    let c = compile_source_text_to_c_with_project_modules(
        Path::new("main.nomo"),
        source,
        None,
        &[],
        &[],
    )
    .unwrap();

    assert!(c.contains("nomo_async_frame_main"));
    assert!(c.contains("nomo_async_poll_main"));
    assert!(c.contains("nomo_async_executor_run_root"));
    assert!(c.contains("nomo_async_ready_enqueue"));
    assert!(c.contains("context->ready_occupied = 1u;"));
    assert!(c.contains("context->ready_occupied = 0u;"));
    assert!(c.contains("context->yield_count += 1u;"));
    assert!(c.contains("context->ready_queue_enqueues += 1u;"));
    assert!(c.contains("case 2u:"));
    assert!(c.contains("nomo_async_drop_main(&nomo__frame);"));
    assert!(!c.contains("pthread_create"));
    assert!(!c.contains("CreateThread"));
    assert!(!c.contains("__atomic_"));
    assert!(!c.contains("Interlocked"));
}

#[test]
fn async_frame_spills_only_live_locals_and_clears_ownership_before_drop() {
    let source = r#"package app.main

import std.io
import std.task

suspend fn main() -> void {
    let live: string = "live"
    let dead: string = "dead"
    let count: u64 = 7
    io.println(dead)
    task.yield_now()
    io.println(live)
    io.println(count)
}
"#;

    let c = compile_source_text_to_c_with_project_modules(
        Path::new("main.nomo"),
        source,
        None,
        &[],
        &[],
    )
    .unwrap();

    let live_field = "nomo_async_local_nomo_live";
    let live_owned = "nomo_async_owned_nomo_live";
    let count_field = "nomo_async_local_nomo_count";
    let dead_field = "nomo_async_local_nomo_dead";
    assert!(c.contains(&format!("nomo_string {live_field};")));
    assert!(c.contains(&format!("uint64_t {count_field};")));
    assert!(!c.contains(dead_field));
    assert!(c.contains(&format!("frame->{live_owned} = 1u;")));
    assert!(c.contains(&format!("nomo_string nomo_live = frame->{live_field};")));
    assert!(c.contains("nomo_string_release(nomo_dead);"));

    let clear = c
        .find(&format!("frame->{live_owned} = 0u;"))
        .expect("frame ownership bit must be cleared");
    let release = c
        .find(&format!("nomo_string_release(frame->{live_field});"))
        .expect("frame-owned string must be released");
    assert!(clear < release);
    assert_eq!(
        c.matches(&format!("nomo_string_release(frame->{live_field});"))
            .count(),
        1
    );
}

#[test]
fn async_frame_spills_and_drops_cow_arrays() {
    let source = r#"package app.main

import std.array
import std.io
import std.task

suspend fn main() -> void {
    let values: Array<i32> = [2, 3, 5]
    task.yield_now()
    io.println(values.len())
}
"#;

    let c = compile_source_text_to_c_with_project_modules(
        Path::new("main.nomo"),
        source,
        None,
        &[],
        &[],
    )
    .unwrap();

    let field = "nomo_async_local_nomo_values";
    let owned = "nomo_async_owned_nomo_values";
    assert!(c.contains(&format!("{field};")));
    assert!(c.contains(&format!("frame->{owned} = 1u;")));
    assert!(c.contains(&format!("frame->{owned} = 0u;")));
    assert!(c.contains(&format!("_release(frame->{field});")));
}

#[test]
fn async_frame_slice_rejects_mutable_locals_and_explicit_panic_cleanup() {
    let mutable_source = r#"package app.main

import std.io
import std.task

suspend fn main() -> void {
    let mut message: string = "mutable"
    task.yield_now()
    io.println(message)
}
"#;
    let mutable_error = parse_inline(mutable_source).unwrap_err();
    assert_eq!(mutable_error.code, "E0876");
    assert!(mutable_error.message.contains("mutable locals"));

    let panic_source = r#"package app.main

import std.task

suspend fn main() -> void {
    task.yield_now()
    panic("cleanup is not implemented yet")
}
"#;
    let panic_error = parse_inline(panic_source).unwrap_err();
    assert_eq!(panic_error.code, "E0876");
    assert!(panic_error.message.contains("explicit panic"));

    let handle_source = r#"package app.main

import std.io
import std.task

fn worker(context: TaskContext, input: string) -> string {
    return input
}

suspend fn main() -> void {
    let started: Result<Task, TaskError> = task.spawn(worker, "input")
    task.yield_now()
    io.println("after")
}
"#;
    let handle_error = parse_inline(handle_source).unwrap_err();
    assert_eq!(handle_error.code, "E0876");
    assert!(handle_error.message.contains("started"));
    assert!(
        handle_error
            .message
            .contains("without a P1 frame move/drop implementation")
    );
}

#[test]
fn p1_yield_slice_rejects_non_root_and_nested_suspension() {
    let helper_source = r#"package app.main

import std.task

suspend fn helper() -> void {
    task.yield_now()
}

suspend fn main() -> void {
    helper()
}
"#;
    let helper_error = parse_inline(helper_source).unwrap_err();
    assert_eq!(helper_error.code, "E0876");
    assert!(helper_error.message.contains("non-root suspension"));

    let nested_source = r#"package app.main

import std.task

suspend fn main() -> void {
    for {
        task.yield_now()
    }
}
"#;
    let nested_error = parse_inline(nested_source).unwrap_err();
    assert_eq!(nested_error.code, "E0876");
    assert!(nested_error.message.contains("nested control flow"));
}
