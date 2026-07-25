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
fn task_sleep_requires_a_suspend_function() {
    let source = r#"package app.main

import std.task
import std.time

fn main() -> void {
    let waited: Result<void, TaskError> = task.sleep(time.duration_millis(1))
}
"#;

    let error = parse_inline(source).unwrap_err();

    assert_eq!(error.code, "E0870");
    assert!(error.message.contains("task.sleep"));
    assert!(error.message.contains("mark the caller `suspend`"));
}

#[test]
fn task_sleep_requires_one_duration_argument() {
    let missing = r#"package app.main

import std.task

suspend fn main() -> void {
    let waited: Result<void, TaskError> = task.sleep()
}
"#;
    let error = parse_inline(missing).unwrap_err();
    assert_eq!(error.code, "E0407");
    assert!(
        error
            .message
            .contains("task.sleep expects 1 argument(s), got 0")
    );

    let wrong_type = r#"package app.main

import std.task

suspend fn main() -> void {
    let waited: Result<void, TaskError> = task.sleep(1)
}
"#;
    let error = parse_inline(wrong_type).unwrap_err();
    assert_eq!(error.code, "E0404");
    assert_eq!(error.expected.as_deref(), Some("Duration"));
    assert_eq!(error.found.as_deref(), Some("i64"));
}

#[test]
fn task_sleep_lowers_a_duration_and_result_binding() {
    let source = r#"package app.main

import std.task
import std.time

suspend fn main() -> void {
    let waited: Result<void, TaskError> = task.sleep(time.duration_millis(5))
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
            name,
            value_type: ValueType::Enum(result, args),
            initializer: ValueExpr::Call { name: call, args: sleep_args },
        }] if name == "waited"
            && result == "Result"
            && args.as_slice() == [
                ValueType::Void,
                ValueType::Struct("TaskError".to_string(), Vec::new()),
            ]
            && call == BUILTIN_TASK_SLEEP_EXPR
            && matches!(sleep_args.as_slice(), [ValueExpr::TimeDurationMillis { .. }])
    ));
}

#[test]
fn specifically_imported_task_sleep_lowers_to_the_timer_runtime() {
    let source = r#"package app.main

import std.task.sleep
import std.time

suspend fn main() -> void {
    let waited: Result<void, TaskError> = sleep(time.duration_millis(0))
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

    assert!(c.contains("nomo_async_timer_start"));
    assert!(c.contains("NOMO_ASYNC_PENDING_TIMER"));
    assert!(c.contains("NOMO_ASYNC_TIMER_OUTCOME_OK"));
}

#[test]
fn task_sleep_result_must_be_bound_in_the_current_slice() {
    let source = r#"package app.main

import std.task
import std.time

suspend fn main() -> void {
    task.sleep(time.duration_millis(1))
}
"#;

    let error = parse_inline(source).unwrap_err();

    assert_eq!(error.code, "E0876");
    assert!(error.message.contains("`let`-bound `task.sleep(Duration)`"));
}

#[test]
fn rejects_blocking_sleep_directly_from_suspend_function() {
    let source = r#"package app.main

import std.time

suspend fn main() -> void {
    time.sleep_millis(1)
}
"#;

    let error = parse_inline(source).unwrap_err();

    assert_eq!(error.code, "E0891");
    assert!(error.message.contains("main -> time.sleep_millis"));
    assert!(error.message.contains("bounded blocking pool"));
    assert_eq!(error.line, 6);
}

#[test]
fn rejects_specifically_imported_blocking_sleep_from_suspend_function() {
    let source = r#"package app.main

import std.time.sleep_millis

suspend fn main() -> void {
    sleep_millis(1)
}
"#;

    let error = parse_inline(source).unwrap_err();

    assert_eq!(error.code, "E0891");
    assert!(error.message.contains("main -> time.sleep_millis"));
}

#[test]
fn rejects_transitive_blocking_sleep_with_a_secret_safe_call_path() {
    let source = r#"package app.main

import std.time

fn pause(secret: string) -> void {
    time.sleep_millis(1)
}

fn helper(secret: string) -> void {
    pause(secret)
}

suspend fn main() -> void {
    helper("sk-must-not-appear")
}
"#;

    let error = parse_inline(source).unwrap_err();

    assert_eq!(error.code, "E0891");
    assert!(
        error
            .message
            .contains("main -> helper -> pause -> time.sleep_millis")
    );
    assert!(!error.message.contains("sk-must-not-appear"));
}

#[test]
fn allows_blocking_sleep_from_synchronous_and_legacy_worker_functions() {
    let synchronous = r#"package app.main

import std.time

fn main() -> void {
    time.sleep_millis(0)
}
"#;
    parse_inline(synchronous).unwrap();

    let worker = r#"package app.main

import std.task
import std.time

fn worker(context: TaskContext, input: string) -> string {
    time.sleep_millis(0)
    return input
}

fn main() -> void {
    let started: Result<Task, TaskError> = task.spawn(worker, "ready")
}
"#;
    parse_inline(worker).unwrap();
}

#[test]
fn rejects_blocking_sleep_reached_through_an_imported_public_function() {
    let root = std::env::temp_dir().join(format!(
        "nomo-suspend-blocking-import-{}",
        std::process::id()
    ));
    let src = root.join("src");
    std::fs::create_dir_all(&src).unwrap();
    std::fs::write(
        src.join("worker.nomo"),
        "package app.worker\n\nimport std.time\n\npub fn pause() -> void {\n    time.sleep_millis(1)\n}\n",
    )
    .unwrap();
    let main = src.join("main.nomo");
    let source = r#"package app.main

import app.worker

suspend fn main() -> void {
    pause()
}
"#;

    let error =
        check_source_text_with_project_modules(&main, source, Some(&src), &[], &[]).unwrap_err();
    std::fs::remove_dir_all(&root).unwrap();

    assert_eq!(error.code, "E0891");
    assert!(error.message.contains("main -> pause -> time.sleep_millis"));
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
    assert!(c.contains("context->frame_drops += 1u;"));
    assert!(c.contains("context->peak_live_frames = context->live_frames;"));
    assert!(c.contains("NOMO_ASYNC_METRICS_PATH"));
    assert!(c.contains("\\\"runtime_abi\\\": 1"));
    assert!(c.contains("nomo_async_metrics_export(&nomo__context)"));
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
fn nested_suspend_calls_lower_to_embedded_stackless_frames() {
    let source = r#"package app.main

import std.io
import std.task

suspend fn leaf() -> void {
    let leaf_value: string = "leaf"
    task.yield_now()
    io.println(leaf_value)
}

suspend fn helper() -> void {
    let child: string = "child"
    leaf()
    io.println(child)
}

suspend fn main() -> void {
    let parent: string = "parent"
    helper()
    io.println(parent)
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

    assert!(c.contains("nomo_async_frame_leaf"));
    assert!(c.contains("nomo_async_frame_leaf nomo_async_child_1;"));
    assert!(c.contains("nomo_async_poll_leaf(&frame->nomo_async_child_1, context)"));
    assert!(c.contains("nomo_async_frame_helper"));
    assert!(c.contains("nomo_async_poll_helper"));
    assert!(c.contains("nomo_async_drop_helper"));
    assert!(c.contains("nomo_async_frame_helper nomo_async_child_1;"));
    assert!(c.contains("nomo_async_poll_helper(&frame->nomo_async_child_1, context)"));
    assert!(c.contains("nomo_async_drop_helper(&frame->nomo_async_child_1);"));
    assert!(c.contains("nomo_async_frame_main nomo__frame = {0};"));

    let root_drop = c
        .find("static void nomo_async_drop_main")
        .expect("root drop must be emitted");
    let root_drop_body = &c[root_drop..];
    let child_drop = root_drop_body
        .find("nomo_async_drop_helper(&frame->nomo_async_child_1);")
        .expect("root drop must destroy its child");
    let parent_release = root_drop_body
        .find("nomo_string_release(frame->nomo_async_local_nomo_parent);")
        .expect("root drop must release its frame-owned local");
    assert!(child_drop < parent_release);
}

#[test]
fn nested_suspend_slice_rejects_arguments_recursion_and_control_flow() {
    let parameter_source = r#"package app.main

import std.task

suspend fn helper(value: string) -> void {
    task.yield_now()
}

suspend fn main() -> void {
    helper("value")
}
"#;
    let parameter_error = parse_inline(parameter_source).unwrap_err();
    assert_eq!(parameter_error.code, "E0876");
    assert!(parameter_error.message.contains("arguments/results"));

    let recursive_source = r#"package app.main

import std.task

suspend fn first() -> void {
    second()
}

suspend fn second() -> void {
    task.yield_now()
    first()
}

suspend fn main() -> void {
    first()
}
"#;
    let recursive_error = parse_inline(recursive_source).unwrap_err();
    assert_eq!(recursive_error.code, "E0876");
    assert!(
        recursive_error
            .message
            .contains("recursive suspend call graph")
    );
    assert!(recursive_error.message.contains("first -> second -> first"));

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
