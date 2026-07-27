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
fn task_deadline_lowers_one_structured_marker_pair_and_cancel_check() {
    let source = r#"package app.main

import std.task
import std.time

suspend fn main() -> void {
    task.deadline(time.duration_millis(5)) {
        task.check_cancelled()
    }
}
"#;

    let program = parse_inline(source).unwrap();
    let main = program
        .functions
        .iter()
        .find(|function| function.name == "main")
        .unwrap();
    assert!(matches!(
        &main.body[0],
        Statement::Expr(ValueExpr::Call { name, args })
            if name == "__nomo_task_deadline_enter"
                && matches!(args.as_slice(), [ValueExpr::TimeDurationMillis { .. }])
    ));
    assert!(matches!(
        &main.body[1],
        Statement::Expr(ValueExpr::Call { name, args })
            if name == "__nomo_task_check_cancelled" && args.is_empty()
    ));
    assert!(matches!(
        &main.body[2],
        Statement::Expr(ValueExpr::Call { name, args })
            if name == "__nomo_task_deadline_exit" && args.is_empty()
    ));
}

#[test]
fn task_deadline_and_cancel_check_require_suspend_functions() {
    for body in [
        "task.deadline(time.duration_millis(1)) {\n        task.check_cancelled()\n    }",
        "task.check_cancelled()",
    ] {
        let source = format!(
            "package app.main\n\nimport std.task\nimport std.time\n\nfn main() -> void {{\n    {body}\n}}\n"
        );
        let error = parse_inline(&source).unwrap_err();
        assert_eq!(error.code, "E0870");
        assert!(error.message.contains("suspend"));
    }
}

#[test]
fn task_deadline_rejects_wrong_duration_and_unsupported_early_exit() {
    let wrong_duration = r#"package app.main

import std.task

suspend fn main() -> void {
    task.deadline(1) {
        task.check_cancelled()
    }
}
"#;
    let error = parse_inline(wrong_duration).unwrap_err();
    assert_eq!(error.code, "E0404");
    assert!(error.message.contains("Duration"));

    let early_return = r#"package app.main

import std.task
import std.time

suspend fn main() -> void {
    task.deadline(time.duration_millis(1)) {
        return
    }
}
"#;
    let error = parse_inline(early_return).unwrap_err();
    assert_eq!(error.code, "E0876");
    assert!(error.message.contains("deadline return"));
}

#[test]
fn task_deadline_rejects_nested_structured_scopes_in_the_first_slice() {
    let source = r#"package app.main

import std.task
import std.time

suspend fn main() -> void {
    task.deadline(time.duration_millis(1)) {
        task.scope {
        }
    }
}
"#;
    let error = parse_inline(source).unwrap_err();
    assert_eq!(error.code, "E0876");
    assert!(error.message.contains("task.deadline"));
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
fn async_tcp_connect_lowers_to_owner_affine_reactor_registration() {
    let source = r#"package app.main

import std.net
import std.result

suspend fn main() -> void {
    let result: Result<TcpStream, NetError> = net.connect("127.0.0.1", 9, 0)
}
"#;

    let program = parse_inline(source).unwrap();
    let main = program
        .functions
        .iter()
        .find(|function| function.name == "main")
        .unwrap();
    assert!(matches!(
        &main.body[0],
        Statement::Let {
            initializer: ValueExpr::Call { name, args },
            ..
        } if name == BUILTIN_NET_CONNECT_EXPR && args.len() == 3
    ));

    let c = compile_source_text_to_c_with_project_modules(
        Path::new("main.nomo"),
        source,
        None,
        &[],
        &[],
    )
    .unwrap();
    assert!(c.contains("nomo_async_tcp_connect_start"));
    assert!(c.contains("nomo_async_tcp_connect_resume"));
    assert!(c.contains("NOMO_ASYNC_PENDING_IO"));
    assert!(c.contains("nomo_async_io_handle_close_callback"));
    assert!(c.contains("NOMO_ASYNC_REACTOR_WRITE"));
    assert!(c.contains("#define NOMO_ASYNC_BLOCKING_POOL_QUEUE_CAPACITY 16u"));
    assert!(c.contains("#define NOMO_ASYNC_RESOLVER_MAX_ADDRESSES 16u"));
    assert!(c.contains("nomo_async_resolver_submit"));
    assert!(c.contains("nomo_async_tcp_resolver_complete"));
    assert!(c.contains("pthread_create"));
    assert!(c.contains("nomo_async_blocking_pool_shutdown"));
}

#[test]
fn rejects_question_propagation_directly_on_async_tcp_connect_in_the_first_slice() {
    let source = r#"package app.main

import std.net
import std.result

suspend fn connect_once() -> Result<TcpStream, NetError> {
    let stream: TcpStream = net.connect("127.0.0.1", 9, 100)?
    return Ok(stream)
}

fn main() -> void {
}
"#;

    let error = parse_inline(source).unwrap_err();
    assert_eq!(error.code, "E0876", "{error:?}");
    assert!(
        error
            .message
            .contains("`?` or panic in other expression positions"),
        "{error:?}"
    );
}

#[test]
fn async_tcp_connect_windows_uses_bounded_resolver_and_owner_affine_iocp() {
    let source = r#"package app.main

import std.net
import std.result

suspend fn main() -> void {
    let result: Result<TcpStream, NetError> = net.connect("127.0.0.1", 9, 100)
}
"#;

    let program = parse_inline(source).unwrap();
    let target = "x86_64-pc-windows-msvc"
        .parse::<nomo_target::TargetTriple>()
        .unwrap();
    let c = codegen::emit_c_for_target(&program, &target);

    for helper in [
        "WSASocketW",
        "WSAIoctl",
        "ConnectEx",
        "InetPtonA",
        "CreateIoCompletionPort",
        "GetQueuedCompletionStatus",
        "CancelIoEx",
        "SO_UPDATE_CONNECT_CONTEXT",
        "#define NOMO_ASYNC_IOCP_OPERATION_CAPACITY 64u",
        "nomo_async_io_handle_associate_reactor",
        "#define NOMO_ASYNC_BLOCKING_POOL_QUEUE_CAPACITY 16u",
        "#define NOMO_ASYNC_RESOLVER_MAX_ADDRESSES 16u",
        "CreateThread",
        "CONDITION_VARIABLE",
        "PostQueuedCompletionStatus",
        "pending_posts",
        "getaddrinfo(",
        "nomo_async_resolver_submit",
        "nomo_async_tcp_resolver_complete",
        "nomo_async_blocking_pool_shutdown",
        "hostname resolution failed",
    ] {
        assert!(c.contains(helper), "missing generated helper {helper}");
    }
    assert!(c.contains("NOMO_ASYNC_REACTOR_WRITE"));
    assert!(c.contains("nomo_async_reactor_register"));
    assert!(!c.contains("async TCP connect is not available on the Windows preview backend"));
    assert!(!c.contains("epoll_ctl("));
    assert!(!c.contains("kevent("));
    assert!(!c.contains("pthread_create"));
    assert!(c.contains(
        "nomo_string nomo_async_tcp_connect_host_0 = nomo_string_literal(\"127.0.0.1\");"
    ));
    assert!(c.contains("nomo_async_tcp_connect_host_0, 9, 100, context"));
    assert!(c.contains("nomo_string_release(nomo_async_tcp_connect_host_0);"));
}

#[test]
fn async_tcp_stream_io_lowers_to_bounded_owner_affine_operations() {
    let source = r#"package app.main

import std.net
import std.result

suspend fn exercise(stream: TcpStream) -> void {
    let bytes: Result<TcpChunk, NetError> = stream.read(4096, 100)
    let text: Result<TcpTextChunk, NetError> = stream.read_string(4096, 100)
    let wrote_bytes: Result<void, NetError> = stream.write([65, 66, 67], 100)
    let wrote_text: Result<void, NetError> = stream.write_string("ready", 100)
}

fn main() -> void {
}
"#;

    let program = parse_inline(source).unwrap();
    let exercise = program
        .functions
        .iter()
        .find(|function| function.name == "exercise")
        .unwrap();
    let names = exercise
        .body
        .iter()
        .filter_map(|statement| match statement {
            Statement::Let {
                initializer: ValueExpr::Call { name, .. },
                ..
            } => Some(name.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        names,
        vec![
            BUILTIN_TCP_STREAM_READ_EXPR,
            BUILTIN_TCP_STREAM_READ_STRING_EXPR,
            BUILTIN_TCP_STREAM_WRITE_EXPR,
            BUILTIN_TCP_STREAM_WRITE_STRING_EXPR,
        ]
    );

    let c = compile_source_text_to_c_with_project_modules(
        Path::new("main.nomo"),
        source,
        None,
        &[],
        &[],
    )
    .unwrap();
    for helper in [
        "nomo_async_tcp_read_start",
        "nomo_async_tcp_read_resume",
        "nomo_async_tcp_read_string_start",
        "nomo_async_tcp_read_string_resume",
        "nomo_async_tcp_write_start",
        "nomo_async_tcp_write_resume",
        "nomo_async_tcp_write_string_start",
        "nomo_async_tcp_write_string_resume",
        "nomo_async_io_handle_acquire",
        "nomo_async_tcp_io_cancel",
        "NOMO_ASYNC_TCP_SEND_FLAGS",
        "NOMO_ASYNC_TCP_WRITE_POLL_BUDGET 65536u",
    ] {
        assert!(c.contains(helper), "missing generated helper {helper}");
    }
}

#[test]
fn async_tcp_stream_io_windows_uses_overlapped_winsock_and_safe_late_completion_storage() {
    let source = r#"package app.main

import std.net
import std.result

suspend fn exercise(stream: TcpStream) -> void {
    let bytes: Result<TcpChunk, NetError> = stream.read(16, 100)
    let wrote: Result<void, NetError> = stream.write_string("secret", 100)
}

fn main() -> void {
}
"#;

    let program = parse_inline(source).unwrap();
    let target = "x86_64-pc-windows-msvc"
        .parse::<nomo_target::TargetTriple>()
        .unwrap();
    let c = codegen::emit_c_for_target(&program, &target);

    for helper in [
        "WSARecv",
        "WSASend",
        "OVERLAPPED",
        "CancelIoEx",
        "nomo_async_reactor_detach_buffer",
        "nomo_async_io_handle_associate_reactor",
        "NOMO_ASYNC_TCP_WRITE_POLL_BUDGET 65536u",
        "#define NOMO_ASYNC_IOCP_OPERATION_CAPACITY 64u",
    ] {
        assert!(c.contains(helper), "missing generated helper {helper}");
    }
    assert!(!c.contains("async TCP read is not available on the Windows preview backend"));
    assert!(!c.contains("async TCP string write is not available on the Windows preview backend"));
    assert!(
        c.contains("nomo_string nomo_async_tcp_io_payload_1 = nomo_string_literal(\"secret\");")
    );
    assert!(c.contains("nomo_async_tcp_io_payload_1, 100, context"));
    assert!(c.contains("nomo_string_release(nomo_async_tcp_io_payload_1);"));
}

#[test]
fn rejects_async_tcp_stream_io_from_synchronous_function() {
    let source = r#"package app.main

import std.net
import std.result

fn read_once(stream: TcpStream) -> void {
    let result: Result<TcpChunk, NetError> = stream.read(16, 100)
}

fn main() -> void {
}
"#;

    let error = parse_inline(source).unwrap_err();
    assert_eq!(error.code, "E0870");
    assert!(error.message.contains("suspend function"));
}

#[test]
fn rejects_async_tcp_connect_from_synchronous_function() {
    let source = r#"package app.main

import std.net
import std.result

fn main() -> void {
    let result: Result<TcpStream, NetError> = net.connect("127.0.0.1", 9, 100)
}
"#;

    let error = parse_inline(source).unwrap_err();
    assert_eq!(error.code, "E0870");
    assert!(error.message.contains("suspend function"));
}

#[test]
fn rejects_blocking_tcp_connect_from_suspend_function() {
    let source = r#"package app.main

import std.net

suspend fn main() -> void {
    let result: Result<TcpStream, NetError> = net.connect_blocking("127.0.0.1", 9)
}
"#;

    let error = parse_inline(source).unwrap_err();
    assert_eq!(error.code, "E0891", "{error:?}");
    assert!(error.message.contains("main -> net.connect_blocking"));
}

#[test]
fn rejects_blocking_tcp_stream_method_from_suspend_function() {
    let source = r#"package app.main

import std.net
import std.result

suspend fn read(stream: TcpStream) -> Result<string, NetError> {
    return stream.read_to_string_blocking()
}

fn main() -> void {
}
"#;

    let error = parse_inline(source).unwrap_err();
    assert_eq!(error.code, "E0891", "{error:?}");
    assert!(
        error
            .message
            .contains("read -> TcpStream.read_to_string_blocking")
    );
}

#[test]
fn allows_user_method_named_like_blocking_tcp_method_without_std_net() {
    let source = r#"package app.main

struct Reader {
    value: string
}

impl Reader {
    fn read_to_string_blocking(self) -> string {
        return self.value
    }
}

suspend fn main() -> void {
    let reader: Reader = Reader { value: "local" }
    let text: string = reader.read_to_string_blocking()
}
"#;

    parse_inline(source).unwrap();
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
    assert!(c.contains("#define NOMO_ASYNC_READY_CAPACITY 64u"));
    assert!(c.contains("nomo_async_ready_slot ready[NOMO_ASYNC_READY_CAPACITY];"));
    assert!(c.contains("nomo_async_ready_dequeue"));
    assert!(c.contains("context->ready_count += 1u;"));
    assert!(c.contains("context->ready_count -= 1u;"));
    assert!(c.contains("context->ready_queue_saturations += 1u;"));
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
fn async_frame_slice_rejects_mutable_locals_and_supports_explicit_panic_cleanup() {
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
    assert!(mutable_error.message.contains("mutable parameters/locals"));

    let panic_source = r#"package app.main

import std.debug
import std.task

suspend fn fail(message: string) -> void {
    task.yield_now()
    debug.panic(message)
}

suspend fn main() -> void {
    fail("cleanup is implemented")
}
"#;
    let c = compile_source_text_to_c_with_project_modules(
        Path::new("main.nomo"),
        panic_source,
        None,
        &[],
        &[],
    )
    .unwrap();
    assert!(c.contains("NOMO_ASYNC_PENDING_PANIC"));
    assert!(c.contains("context->panic_message_owned = 1u;"));
    assert!(
        c.contains("nomo_async_panic_message_1 = nomo_string_retain(nomo_async_panic_message_1);")
    );
    assert!(c.contains("nomo_async_cancel_main(&nomo__frame, &nomo__context);"));
    assert!(c.contains("nomo_panic_string(nomo__panic_message);"));

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

    let question_outside_scope = r#"package app.main

import std.num
import std.task

suspend fn parse_after_yield() -> Result<i64, NumError> {
    task.yield_now()
    let value: i64 = num.parse_i64("42")?
    return Ok(value)
}

suspend fn main() -> void {
    let value: Result<i64, NumError> = parse_after_yield()
}
"#;
    let question_error = parse_inline(question_outside_scope).unwrap_err();
    assert_eq!(question_error.code, "E0876");
    assert!(
        question_error
            .message
            .contains("`?` or panic in other expression positions")
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
fn nested_suspend_call_abi_lowers_arguments_and_results() {
    let source = r#"package app.main

import std.io
import std.task

suspend fn helper(value: string, count: u64) -> string {
    let owned: string = value
    task.yield_now()
    io.println(count)
    return owned
}

suspend fn main() -> void {
    let result: string = helper("value", 7)
    task.yield_now()
    io.println(result)
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

    assert!(c.contains("nomo_string nomo_async_parameter_nomo_value;"));
    assert!(c.contains("uint64_t nomo_async_parameter_nomo_count;"));
    assert!(c.contains("uint8_t nomo_async_parameter_owned_nomo_value;"));
    assert!(c.contains("nomo_string nomo_async_result;"));
    assert!(c.contains("uint8_t nomo_async_result_owned;"));
    assert!(c.contains(
        "frame->nomo_async_child_0.nomo_async_parameter_nomo_value = nomo_string_literal(\"value\")"
    ));
    assert!(c.contains("frame->nomo_async_child_0.nomo_async_parameter_nomo_count = 7;"));
    assert!(c.contains("nomo_string nomo_value = frame->nomo_async_parameter_nomo_value;"));
    assert!(c.contains(
        "frame->nomo_async_local_nomo_result = frame->nomo_async_child_0.nomo_async_result;"
    ));
    assert!(c.contains("frame->nomo_async_child_0.nomo_async_result_owned = 0u;"));
}

#[test]
fn nested_suspend_slice_rejects_unbound_results_recursion_and_control_flow() {
    let unbound_source = r#"package app.main

import std.task

suspend fn helper() -> string {
    task.yield_now()
    return "value"
}

suspend fn main() -> void {
    helper()
}
"#;
    let unbound_error = parse_inline(unbound_source).unwrap_err();
    assert_eq!(unbound_error.code, "E0876");
    assert!(unbound_error.message.contains("bind the result"));

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

#[test]
fn suspend_call_abi_rejects_mutable_affine_and_root_result_shapes() {
    let mutable_parameter_source = r#"package app.main

import std.task

suspend fn helper(mut value: string) -> void {
    task.yield_now()
}

suspend fn main() -> void {
    helper("value")
}
"#;
    let mutable_error = parse_inline(mutable_parameter_source).unwrap_err();
    assert_eq!(mutable_error.code, "E0876");
    assert!(mutable_error.message.contains("mutable parameters/locals"));

    let affine_parameter_source = r#"package app.main

import std.task

suspend fn helper(value: Task) -> void {
    task.yield_now()
}

suspend fn main() -> void {
    task.yield_now()
}
"#;
    let affine_error = parse_inline(affine_parameter_source).unwrap_err();
    assert_eq!(affine_error.code, "E0876");
    assert!(affine_error.message.contains("parameter `value`"));
    assert!(affine_error.message.contains("frame-safe"));

    let root_result_source = r#"package app.main

import std.task

suspend fn main() -> string {
    task.yield_now()
    return "not-an-exit-status-yet"
}
"#;
    let root_error = parse_inline(root_result_source).unwrap_err();
    assert_eq!(root_error.code, "E0876");
    assert!(
        root_error
            .message
            .contains("async `main` still returns `void`")
    );
}

#[test]
fn structured_void_scope_lowers_spawn_and_join_intrinsics() {
    let source = r#"package app.main

import std.task

suspend fn worker(value: string) -> void {
    task.yield_now()
}

suspend fn main() -> void {
    task.scope {
        let child = task.spawn worker("value")
        let joined: Result<void, TaskError> = task.join(child)
    }
}
"#;

    let program = parse_inline(source).unwrap();
    let main = program
        .functions
        .iter()
        .find(|function| function.name == "main")
        .unwrap();
    assert!(matches!(
        &main.body[0],
        Statement::Let {
            name,
            value_type: ValueType::Struct(task, task_args),
            initializer: ValueExpr::Call { name: call, args },
        } if name == "child"
            && task == "Task"
            && task_args == &[ValueType::Void]
            && call == "__nomo_structured_task_spawn::worker"
            && matches!(args.as_slice(), [ValueExpr::StringLiteral(value)] if value == "value")
    ));
    assert!(matches!(
        &main.body[1],
        Statement::Let {
            name,
            initializer: ValueExpr::Call { name: call, args },
            ..
        } if name == "joined"
            && call == "__nomo_structured_task_join"
            && matches!(args.as_slice(), [ValueExpr::Variable(handle)] if handle == "child")
    ));
}

#[test]
fn structured_spawn_marks_non_copy_arguments_as_publication_moves() {
    let source = r#"package app.main

import std.task

suspend fn worker(value: string) -> void {
    task.yield_now()
}

suspend fn launch(value: string) -> void {
    task.scope {
        let child = task.spawn worker(value)
        let joined: Result<void, TaskError> = task.join(child)
    }
}

suspend fn main() -> void {
}
"#;

    let program = parse_inline(source).unwrap();
    let launch = program
        .functions
        .iter()
        .find(|function| function.name == "launch")
        .unwrap();
    assert!(matches!(
        &launch.body[0],
        Statement::Let {
            initializer: ValueExpr::Call { name, args },
            ..
        } if name == "__nomo_structured_task_spawn::worker"
            && matches!(
                args.as_slice(),
                [ValueExpr::Call { name: move_name, args: move_args }]
                    if move_name == "__nomo_task_publication_move"
                        && matches!(
                            move_args.as_slice(),
                            [ValueExpr::Variable(value)] if value == "value"
                        )
            )
    ));

    let c = compile_source_text_to_c_with_project_modules(
        Path::new("main.nomo"),
        source,
        None,
        &[],
        &[],
    )
    .unwrap();
    assert!(c.contains("frame->nomo_async_child_0.nomo_async_parameter_nomo_value = nomo_value;"));
    assert!(c.contains("frame->nomo_async_parameter_owned_nomo_value = 0u;"));
    assert!(!c.contains(
        "frame->nomo_async_child_0.nomo_async_parameter_nomo_value = nomo_string_retain"
    ));
}

#[test]
fn structured_spawn_keeps_copy_arguments_available() {
    let source = r#"package app.main

import std.task

suspend fn worker(value: i64) -> void {
    task.yield_now()
}

suspend fn launch(value: i64) -> void {
    task.scope {
        let child = task.spawn worker(value)
        let kept: i64 = value
        let joined: Result<void, TaskError> = task.join(child)
    }
}

suspend fn main() -> void {
}
"#;

    let program = parse_inline(source).unwrap();
    let launch = program
        .functions
        .iter()
        .find(|function| function.name == "launch")
        .unwrap();
    assert!(matches!(
        &launch.body[0],
        Statement::Let {
            initializer: ValueExpr::Call { args, .. },
            ..
        } if matches!(args.as_slice(), [ValueExpr::Variable(value)] if value == "value")
    ));
}

#[test]
fn structured_spawn_derives_send_for_managed_aggregates() {
    let source = r#"package app.main

import std.array
import std.task

struct Envelope {
    message: string
    tags: Array<string>
}

suspend fn worker(value: Envelope) -> void {
    task.yield_now()
}

suspend fn launch(value: Envelope) -> void {
    task.scope {
        let child = task.spawn worker(value)
        let joined: Result<void, TaskError> = task.join(child)
    }
}

suspend fn main() -> void {
}
"#;

    let program = parse_inline(source).unwrap();
    let launch = program
        .functions
        .iter()
        .find(|function| function.name == "launch")
        .unwrap();
    assert!(matches!(
        &launch.body[0],
        Statement::Let {
            initializer: ValueExpr::Call { args, .. },
            ..
        } if matches!(
            args.as_slice(),
            [ValueExpr::Call { name, .. }] if name == "__nomo_task_publication_move"
        )
    ));
}

#[test]
fn structured_spawn_rejects_local_and_nested_local_values() {
    let direct = r#"package app.main

import std.fs
import std.task

suspend fn worker(file: File) -> void {
    task.yield_now()
}

suspend fn launch(file: File) -> void {
    task.scope {
        let child = task.spawn worker(file)
        let joined: Result<void, TaskError> = task.join(child)
    }
}

suspend fn main() -> void {
}
"#;
    let direct_error = parse_inline(direct).unwrap_err();
    assert_eq!(direct_error.code, "E0880");
    assert!(direct_error.message.contains("Local/!Send type `File`"));

    let nested = r#"package app.main

import std.fs
import std.task

struct Envelope {
    file: File
}

suspend fn worker(value: Envelope) -> void {
    task.yield_now()
}

suspend fn launch(value: Envelope) -> void {
    task.scope {
        let child = task.spawn worker(value)
        let joined: Result<void, TaskError> = task.join(child)
    }
}

suspend fn main() -> void {
}
"#;
    let nested_error = parse_inline(nested).unwrap_err();
    assert_eq!(nested_error.code, "E0883");
    assert!(nested_error.message.contains("Envelope.file"));
    assert!(nested_error.message.contains("Local/!Send type `File`"));
}

#[test]
fn structured_spawn_rejects_partial_duplicate_and_later_move_uses() {
    let partial = r#"package app.main

import std.task

struct Envelope {
    message: string
}

suspend fn worker(value: string) -> void {
    task.yield_now()
}

suspend fn launch(value: Envelope) -> void {
    task.scope {
        let child = task.spawn worker(value.message)
        let joined: Result<void, TaskError> = task.join(child)
    }
}

suspend fn main() -> void {
}
"#;
    let partial_error = parse_inline(partial).unwrap_err();
    assert_eq!(partial_error.code, "E0883");
    assert!(
        partial_error
            .message
            .contains("cannot move only `value.message`")
    );

    let duplicate = r#"package app.main

import std.task

suspend fn worker(left: string, right: string) -> void {
    task.yield_now()
}

suspend fn launch(value: string) -> void {
    task.scope {
        let child = task.spawn worker(value, value)
        let joined: Result<void, TaskError> = task.join(child)
    }
}

suspend fn main() -> void {
}
"#;
    let duplicate_error = parse_inline(duplicate).unwrap_err();
    assert_eq!(duplicate_error.code, "E0881");
    assert!(duplicate_error.message.contains("consumed more than once"));

    let later_use = r#"package app.main

import std.task

suspend fn worker(value: string) -> void {
    task.yield_now()
}

suspend fn launch(value: string) -> void {
    task.scope {
        let child = task.spawn worker(value)
        let reused: string = value
        let joined: Result<void, TaskError> = task.join(child)
    }
}

suspend fn main() -> void {
}
"#;
    let later_error = parse_inline(later_use).unwrap_err();
    assert_eq!(later_error.code, "E0881");
    assert!(later_error.message.contains("publication move"));
    assert!(later_error.message.contains("structured task.spawn"));

    let after_scope = r#"package app.main

import std.task

suspend fn worker(value: string) -> void {
    task.yield_now()
}

suspend fn launch(value: string) -> void {
    task.scope {
        let child = task.spawn worker(value)
        let joined: Result<void, TaskError> = task.join(child)
    }
    let reused: string = value
}

suspend fn main() -> void {
}
"#;
    let after_scope_error = parse_inline(after_scope).unwrap_err();
    assert_eq!(after_scope_error.code, "E0881");
    assert!(after_scope_error.message.contains("publication move"));

    let question_use = r#"package app.main

import std.task

fn keep(value: string) -> Result<string, TaskError> {
    return Ok(value)
}

suspend fn worker(value: string) -> void {
    task.yield_now()
}

suspend fn launch(value: string) -> Result<void, TaskError> {
    task.scope {
        let child = task.spawn worker(value)
        let reused: string = keep(value)?
        let joined: Result<void, TaskError> = task.join(child)
    }
    return Ok(void)
}

suspend fn main() -> void {
}
"#;
    let question_error = parse_inline(question_use).unwrap_err();
    assert_eq!(question_error.code, "E0881");
    assert!(question_error.message.contains("publication move"));
}

#[test]
fn structured_spawn_does_not_consume_reusable_constants() {
    let source = r#"package app.main

import std.task

const greeting: string = "hello"

suspend fn worker(value: string) -> void {
    task.yield_now()
}

suspend fn main() -> void {
    task.scope {
        let child = task.spawn worker(greeting)
        let kept: string = greeting
        let joined: Result<void, TaskError> = task.join(child)
    }
}
"#;

    let program = parse_inline(source).unwrap();
    let main = program
        .functions
        .iter()
        .find(|function| function.name == "main")
        .unwrap();
    assert!(matches!(
        &main.body[0],
        Statement::Let {
            initializer: ValueExpr::Call { args, .. },
            ..
        } if matches!(args.as_slice(), [ValueExpr::Variable(value)] if value == "greeting")
    ));
}

#[test]
fn structured_typed_scope_lowers_task_and_join_result_types() {
    let source = r#"package app.main

import std.task

suspend fn worker(value: string) -> string {
    task.yield_now()
    return value
}

suspend fn main() -> void {
    task.scope {
        let child = task.spawn worker("value")
        let joined: Result<string, TaskError> = task.join(child)
    }
}
"#;

    let program = parse_inline(source).unwrap();
    let main = program
        .functions
        .iter()
        .find(|function| function.name == "main")
        .unwrap();
    assert!(matches!(
        &main.body[0],
        Statement::Let {
            name,
            value_type: ValueType::Struct(task, task_args),
            initializer: ValueExpr::Call { name: call, .. },
        } if name == "child"
            && task == "Task"
            && task_args == &[ValueType::String]
            && call == "__nomo_structured_task_spawn::worker"
    ));
    assert!(matches!(
        &main.body[1],
        Statement::Let {
            name,
            value_type: ValueType::Enum(result, result_args),
            initializer: ValueExpr::Call { name: call, .. },
        } if name == "joined"
            && result == "Result"
            && result_args
                == &[
                    ValueType::String,
                    ValueType::Struct("TaskError".to_string(), Vec::new()),
                ]
            && call == "__nomo_structured_task_join"
    ));
}

#[test]
fn structured_cancel_is_suspend_capable_consuming_and_affine() {
    let source = r#"package app.main

import std.result
import std.task

suspend fn worker(value: string) -> string {
    task.yield_now()
    return value
}

suspend fn main() -> void {
    task.scope {
        let child = task.spawn worker("managed")
        let cancelled: Result<void, TaskError> = task.cancel(child)
        let ok: bool = result.is_ok(cancelled)
    }
}
"#;
    let program = parse_inline(source).unwrap();
    let main = program
        .functions
        .iter()
        .find(|function| function.name == "main")
        .unwrap();
    assert!(main.body.iter().any(|statement| {
        matches!(
            statement,
            Statement::Let {
                initializer: ValueExpr::Call { name, args },
                ..
            } if name == "__nomo_structured_task_cancel_join" && args.len() == 1
        )
    }));

    let c = compile_source_text_to_c_with_project_modules(
        Path::new("main.nomo"),
        source,
        None,
        &[],
        &[],
    )
    .unwrap();
    assert!(c.contains("NOMO_ASYNC_PENDING_CANCEL"));
    assert!(c.contains("nomo_async_cancel_worker(&frame->nomo_async_child_0, context);"));
    assert!(c.contains("nomo_async_cancel_join_result_1"));
    assert!(c.contains("nomo_async_drop_worker(&frame->nomo_async_child_0);"));

    let double_cancel = source.replace(
        "        let ok: bool = result.is_ok(cancelled)",
        "        let second: Result<void, TaskError> = task.cancel(child)",
    );
    let error = parse_inline(&double_cancel).unwrap_err();
    assert_eq!(error.code, "E0872");
    assert!(error.message.contains("cancelled more than once"));

    let join_after_cancel = source.replace(
        "        let ok: bool = result.is_ok(cancelled)",
        "        let joined: Result<string, TaskError> = task.join(child)",
    );
    let error = parse_inline(&join_after_cancel).unwrap_err();
    assert_eq!(error.code, "E0872");
    assert!(error.message.contains("after cancellation"));

    let cancel_after_join = source.replace(
        "        let cancelled: Result<void, TaskError> = task.cancel(child)\n        let ok: bool = result.is_ok(cancelled)",
        "        let joined: Result<string, TaskError> = task.join(child)\n        let cancelled: Result<void, TaskError> = task.cancel(child)",
    );
    let error = parse_inline(&cancel_after_join).unwrap_err();
    assert_eq!(error.code, "E0872");
    assert!(error.message.contains("after join"));
}

#[test]
fn structured_scope_return_after_join_lowers_typed_parent_result() {
    let source = r#"package app.main

import std.result
import std.task

suspend fn worker(value: string) -> string {
    task.yield_now()
    return value
}

suspend fn gather() -> string {
    task.scope {
        let child = task.spawn worker("value")
        let joined: Result<string, TaskError> = task.join(child)
        return result.unwrap_or(joined, "fallback")
    }
}

suspend fn main() -> void {
    let gathered: string = gather()
}
"#;

    let program = parse_inline(source).unwrap();
    let gather = program
        .functions
        .iter()
        .find(|function| function.name == "gather")
        .unwrap();
    assert_eq!(gather.body.len(), 3);
    assert!(matches!(gather.body[2], Statement::Return(Some(_))));

    let c = compile_source_text_to_c_with_project_modules(
        Path::new("main.nomo"),
        source,
        None,
        &[],
        &[],
    )
    .unwrap();
    assert!(c.contains("structured_waiter_frame = context->current_frame;"));
    assert!(!c.contains(".structured_waiter_frame = frame;"));
    assert!(c.matches("frame->nomo_async_result = ").count() >= 2);
    assert!(c.contains("nomo_async_drop_worker(&frame->nomo_async_child_0);"));
}

#[test]
fn structured_scope_question_join_cancels_live_siblings_and_spills_success_values() {
    let source = r#"package app.main

import std.result
import std.task

suspend fn worker(value: string) -> string {
    task.yield_now()
    return value
}

suspend fn gather() -> Result<string, TaskError> {
    task.scope {
        let left = task.spawn worker("left")
        let right = task.spawn worker("right")
        let left_value: string = task.join(left)?
        let right_value: string = task.join(right)?
        return Ok(left_value)
    }
}

suspend fn main() -> void {
    let gathered: Result<string, TaskError> = gather()
}
"#;

    let program = parse_inline(source).unwrap();
    let gather = program
        .functions
        .iter()
        .find(|function| function.name == "gather")
        .unwrap();
    assert_eq!(gather.body.len(), 7);
    assert!(matches!(
        &gather.body[2],
        Statement::Let {
            name,
            initializer: ValueExpr::Call { name: call, args },
            ..
        } if name == "__structured_question_result_0"
            && call == "__nomo_structured_task_join"
            && matches!(args.as_slice(), [ValueExpr::Variable(handle)] if handle == "left")
    ));
    assert!(matches!(
        &gather.body[3],
        Statement::QuestionLet {
            name,
            result_expr: ValueExpr::Variable(result),
            early_exit_actions,
            ..
        } if name == "left_value"
            && result == "__structured_question_result_0"
            && matches!(
                early_exit_actions.as_slice(),
                [ValueExpr::Call { name: action, args }]
                    if action == "__nomo_structured_task_cancel"
                        && matches!(args.as_slice(), [ValueExpr::Variable(handle)] if handle == "right")
            )
    ));
    assert!(matches!(
        &gather.body[5],
        Statement::QuestionLet {
            name,
            result_expr: ValueExpr::Variable(result),
            early_exit_actions,
            ..
        } if name == "right_value"
            && result == "__structured_question_result_1"
            && early_exit_actions.is_empty()
    ));

    let c = compile_source_text_to_c_with_project_modules(
        Path::new("main.nomo"),
        source,
        None,
        &[],
        &[],
    )
    .unwrap();
    assert!(c.contains("nomo_string nomo_async_local_nomo_left_value;"));
    assert!(c.contains("uint8_t nomo_async_owned_nomo_left_value;"));
    assert!(c.contains("nomo_async_cancel_worker(&frame->nomo_async_child_1, context);"));
    assert!(c.contains("frame->nomo_async_local_nomo_left_value = nomo_left_value;"));
    assert!(c.contains("frame->nomo_async_result_owned = 1u;"));
}

#[test]
fn structured_task_scope_rejects_invalid_ownership_and_target_shapes() {
    let outside_scope = r#"package app.main

import std.task

suspend fn worker() -> void {
}

suspend fn main() -> void {
    let child = task.spawn worker()
}
"#;
    let error = parse_inline(outside_scope).unwrap_err();
    assert_eq!(error.code, "E0871");

    let synchronous_scope = r#"package app.main

import std.task

fn main() -> void {
    task.scope {
    }
}
"#;
    let error = parse_inline(synchronous_scope).unwrap_err();
    assert_eq!(error.code, "E0870");

    let non_suspend_target = r#"package app.main

import std.task

fn worker() -> void {
}

suspend fn main() -> void {
    task.scope {
        let child = task.spawn worker()
        let joined: Result<void, TaskError> = task.join(child)
    }
}
"#;
    let error = parse_inline(non_suspend_target).unwrap_err();
    assert_eq!(error.code, "E0875");
    assert!(error.message.contains("must be declared `suspend fn`"));

    let panicking_target = r#"package app.main

import std.task

suspend fn worker() -> void {
    panic("structured child panic cleanup")
}

suspend fn main() -> void {
    task.scope {
        let child = task.spawn worker()
        let joined: Result<void, TaskError> = task.join(child)
    }
}
"#;
    let c = compile_source_text_to_c_with_project_modules(
        Path::new("main.nomo"),
        panicking_target,
        None,
        &[],
        &[],
    )
    .unwrap();
    assert!(c.contains("context->pending_reason = NOMO_ASYNC_PENDING_PANIC;"));
    assert!(c.contains("nomo_async_cancel_worker(&frame->nomo_async_child_0, context);"));
    assert!(c.contains("nomo_async_cancel_main(&nomo__frame, &nomo__context);"));
    assert!(c.contains("nomo_async_drop_main(&nomo__frame);"));

    let double_join = r#"package app.main

import std.task

suspend fn worker() -> void {
}

suspend fn main() -> void {
    task.scope {
        let child = task.spawn worker()
        let first: Result<void, TaskError> = task.join(child)
        let second: Result<void, TaskError> = task.join(child)
    }
}
"#;
    let error = parse_inline(double_join).unwrap_err();
    assert_eq!(error.code, "E0872");
    assert!(error.message.contains("joined more than once"));

    let escaped_handle = r#"package app.main

import std.io
import std.task

suspend fn worker() -> void {
}

suspend fn main() -> void {
    task.scope {
        let child = task.spawn worker()
        io.println(child)
        let joined: Result<void, TaskError> = task.join(child)
    }
}
"#;
    let error = parse_inline(escaped_handle).unwrap_err();
    assert_eq!(error.code, "E0872");
    assert!(error.message.contains("may only be consumed by task.join"));

    let unjoined_handles = r#"package app.main

import std.task

suspend fn worker() -> void {
}

suspend fn main() -> void {
    task.scope {
        let zebra = task.spawn worker()
        let alpha = task.spawn worker()
    }
}
"#;
    let program = parse_inline(unjoined_handles).unwrap();
    let main = program
        .functions
        .iter()
        .find(|function| function.name == "main")
        .unwrap();
    assert!(matches!(
        &main.body[2],
        Statement::Expr(ValueExpr::Call { name, args })
            if name == "__nomo_structured_task_cancel"
                && matches!(args.as_slice(), [ValueExpr::Variable(handle)] if handle == "alpha")
    ));
    assert!(matches!(
        &main.body[3],
        Statement::Expr(ValueExpr::Call { name, args })
            if name == "__nomo_structured_task_cancel"
                && matches!(args.as_slice(), [ValueExpr::Variable(handle)] if handle == "zebra")
    ));
    let c = compile_source_text_to_c_with_project_modules(
        Path::new("main.nomo"),
        unjoined_handles,
        None,
        &[],
        &[],
    )
    .unwrap();
    assert!(c.contains("nomo_async_executor_run_root"));
    assert!(c.contains("nomo_async_ready_cancel_frame(context, frame);"));
    assert!(c.contains("nomo_async_cancel_worker(&frame->nomo_async_child_0, context);"));
    assert!(c.contains("nomo_async_cancel_worker(&frame->nomo_async_child_1, context);"));

    let non_terminal_return = r#"package app.main

import std.task

suspend fn main() -> void {
    task.scope {
        return
        let value: i64 = 1
    }
}
"#;
    let error = parse_inline(non_terminal_return).unwrap_err();
    assert_eq!(error.code, "E0876");
    assert!(error.message.contains("final statement"));

    let unjoined_return = r#"package app.main

import std.task

suspend fn worker() -> void {
}

suspend fn main() -> void {
    task.scope {
        let child = task.spawn worker()
        return
    }
}
"#;
    let program = parse_inline(unjoined_return).unwrap();
    let main = program
        .functions
        .iter()
        .find(|function| function.name == "main")
        .unwrap();
    assert!(matches!(
        &main.body[1],
        Statement::Expr(ValueExpr::Call { name, args })
            if name == "__nomo_structured_task_cancel"
                && matches!(args.as_slice(), [ValueExpr::Variable(handle)] if handle == "child")
    ));
    assert!(matches!(main.body[2], Statement::Return(None)));
    let c = compile_source_text_to_c_with_project_modules(
        Path::new("main.nomo"),
        unjoined_return,
        None,
        &[],
        &[],
    )
    .unwrap();
    assert!(c.contains(concat!(
        "            nomo_async_cancel_worker(&frame->nomo_async_child_0, context);\n",
        "            nomo_async_drop_worker(&frame->nomo_async_child_0);\n",
        "            frame->structured_completed = 1u;"
    )));

    let typed_return_with_temporary_collision = r#"package app.main

import std.task

suspend fn worker() -> void {
}

suspend fn finish() -> string {
    task.scope {
        let child = task.spawn worker()
        let __nomo_structured_return_value: string = "value"
        return __nomo_structured_return_value
    }
}

suspend fn main() -> void {
    let value: string = finish()
}
"#;
    let program = parse_inline(typed_return_with_temporary_collision).unwrap();
    let finish = program
        .functions
        .iter()
        .find(|function| function.name == "finish")
        .unwrap();
    assert!(matches!(
        &finish.body[2],
        Statement::Let {
            name,
            value_type: ValueType::String,
            initializer: ValueExpr::Variable(value),
        } if name == "__nomo_structured_return_value_"
            && value == "__nomo_structured_return_value"
    ));
    assert!(matches!(
        &finish.body[3],
        Statement::Expr(ValueExpr::Call { name, args })
            if name == "__nomo_structured_task_cancel"
                && matches!(args.as_slice(), [ValueExpr::Variable(handle)] if handle == "child")
    ));
    assert!(matches!(
        &finish.body[4],
        Statement::Return(Some(ValueExpr::Variable(value)))
            if value == "__nomo_structured_return_value_"
    ));

    let returned_handle = r#"package app.main

import std.task

suspend fn worker() -> void {
}

suspend fn escape() -> void {
    task.scope {
        let child = task.spawn worker()
        return child
    }
}

suspend fn main() -> void {
}
"#;
    let error = parse_inline(returned_handle).unwrap_err();
    assert_eq!(error.code, "E0872");
    assert!(error.message.contains("cannot be returned"));
}

#[test]
fn structured_void_scope_emits_bounded_fifo_join_wakeup_and_drop_paths() {
    let source = r#"package app.main

import std.task

suspend fn worker(value: string) -> void {
    task.yield_now()
}

suspend fn main() -> void {
    task.scope {
        let child = task.spawn worker("value")
        let joined: Result<void, TaskError> = task.join(child)
    }
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

    assert!(c.contains("NOMO_ASYNC_PENDING_JOIN"));
    assert!(c.contains("structured_waiter_frame"));
    assert!(c.contains("structured_waiter_poll"));
    assert!(c.contains("structured_completed"));
    assert!(c.contains("owner executor ready queue is full"));
    assert!(c.contains("nomo_async_ready_enqueue(context, &frame->nomo_async_child_0"));
    assert!(c.contains("nomo_async_drop_worker(&frame->nomo_async_child_0)"));
    assert!(c.contains("context->task_spawns += 1u;"));
    assert!(c.contains("context->task_joins += 1u;"));
    assert!(c.contains("context->join_suspensions += 1u;"));
    assert!(!c.contains("pthread_create"));
    assert!(!c.contains("CreateThread"));
    assert!(!c.contains("__atomic_"));
    assert!(!c.contains("Interlocked"));
}

#[test]
fn structured_typed_scope_moves_child_result_into_join_once() {
    let source = r#"package app.main

import std.task

suspend fn worker(value: string) -> string {
    task.yield_now()
    return value
}

suspend fn main() -> void {
    task.scope {
        let child = task.spawn worker("value")
        let joined: Result<string, TaskError> = task.join(child)
    }
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

    assert!(c.contains(
        "frame->nomo_async_join_result_1.payload.nomo_payload_Ok = \
         frame->nomo_async_child_0.nomo_async_result;"
    ));
    assert!(c.contains("frame->nomo_async_child_0.nomo_async_result_owned = 0u;"));
    assert!(c.contains("frame->nomo_async_join_result_owned_1 = 1u;"));
    assert!(c.contains("nomo_async_drop_worker(&frame->nomo_async_child_0);"));
}

#[test]
fn bounded_channel_builtins_lower_with_inferred_element_types() {
    let source = r#"package app.main

import std.task
import std.option

suspend fn exchange(channel_value: Channel<string>, value: string) -> void {
    let sent: Result<void, ChannelSendError<string>> = task.send(channel_value, value)
    let received: Option<string> = task.receive(channel_value)
    let immediate: ChannelTryReceive<string> = task.try_receive(channel_value)
    task.close(channel_value)
}

suspend fn main() -> void {
    let created: Result<Channel<string>, ChannelError> = task.channel<string>(2)
}
"#;

    let program = parse_inline(source).unwrap();
    let exchange = program
        .functions
        .iter()
        .find(|function| function.name == "exchange")
        .unwrap();

    assert!(matches!(
        &exchange.body[0],
        Statement::Let {
            initializer: ValueExpr::Call { name, args },
            ..
        } if name == "__nomo_task_send::string"
            && matches!(
                args.as_slice(),
                [
                    ValueExpr::Variable(channel),
                    ValueExpr::Call { name: moved, args: moved_args }
                ] if channel == "channel_value"
                    && moved == BUILTIN_TASK_PUBLICATION_MOVE_EXPR
                    && matches!(moved_args.as_slice(), [ValueExpr::Variable(value)] if value == "value")
            )
    ));
    assert!(matches!(
        &exchange.body[1],
        Statement::Let {
            initializer: ValueExpr::Call { name, .. },
            ..
        } if name == "__nomo_task_receive::string"
    ));
    assert!(matches!(
        &exchange.body[2],
        Statement::Let {
            initializer: ValueExpr::Call { name, .. },
            ..
        } if name == "__nomo_task_try_receive::string"
    ));
    assert!(matches!(
        &exchange.body[3],
        Statement::Expr(ValueExpr::Call { name, .. })
            if name == "__nomo_task_close_channel::string"
    ));
}

#[test]
fn static_receive_timer_select_lowers_into_typed_ir_and_c99_state_machine() {
    let source = r#"package app.main

import std.task
import std.time

suspend fn choose(channel_value: Channel<string>) -> void {
    task.select {
        task.receive(channel_value) => received {
            let value = received
        }
        task.sleep(time.duration_millis(5)) => timeout {
            let value = timeout
        }
    }
}

fn main() -> void {
}
"#;

    let program = parse_inline(source).unwrap();
    let choose = program
        .functions
        .iter()
        .find(|function| function.name == "choose")
        .unwrap();
    let Statement::TaskSelect { arms } = &choose.body[0] else {
        panic!("expected typed task.select IR");
    };
    assert_eq!(arms.len(), 2);
    assert!(matches!(
        &arms[0].operation,
        TaskSelectOperation::Receive { element_type, .. }
            if element_type == &ValueType::String
    ));
    assert_eq!(
        arms[0].binding_type,
        ValueType::Enum("Option".to_string(), vec![ValueType::String])
    );
    assert!(matches!(
        &arms[1].operation,
        TaskSelectOperation::Sleep { .. }
    ));
    assert_eq!(
        arms[1].binding_type,
        ValueType::Enum(
            "Result".to_string(),
            vec![
                ValueType::Void,
                ValueType::Struct("TaskError".to_string(), Vec::new())
            ]
        )
    );

    let c = compile_source_text_to_c_with_project_modules(
        Path::new("main.nomo"),
        source,
        None,
        &[],
        &[],
    )
    .unwrap();
    assert!(c.contains("nomo_async_select_token_0"));
    assert!(c.contains("nomo_async_select_immediate_win"));
    assert!(c.contains("nomo_async_select_suspend"));
    assert!(c.contains("nomo_channel_receive_select_cancel_string"));
    assert!(c.contains("nomo_async_timer_select_cancel"));
    assert!(c.contains("NOMO_ASYNC_PENDING_SELECT"));
}

#[test]
fn static_select_rejects_unsupported_operations_and_early_arm_exits() {
    let unsupported = r#"package app.main

import std.task
import std.time

suspend fn choose(channel_value: Channel<string>) -> void {
    task.select {
        task.send(channel_value, "value") => sent {
            let value = sent
        }
        task.sleep(time.duration_millis(5)) => timeout {
            let value = timeout
        }
    }
}

fn main() -> void {
}
"#;
    let error = parse_inline(unsupported).unwrap_err();
    assert_eq!(error.code, "E0886");
    assert!(error.message.contains("supports only"));

    let early_exit = r#"package app.main

import std.task
import std.time

suspend fn choose(channel_value: Channel<string>) -> void {
    task.select {
        task.receive(channel_value) => received {
            return
        }
        task.sleep(time.duration_millis(5)) => timeout {
            let value = timeout
        }
    }
}

fn main() -> void {
}
"#;
    let error = parse_inline(early_exit).unwrap_err();
    assert_eq!(error.code, "E0876");
    assert!(error.message.contains("select early exits"));
}

#[test]
fn bounded_channel_requires_send_elements() {
    let source = r#"package app.main

import std.net
import std.task

suspend fn main() -> void {
    let created: Result<Channel<TcpStream>, ChannelError> = task.channel<TcpStream>(1)
}
"#;

    let error = parse_inline(source).unwrap_err();

    assert_eq!(error.code, "E0880");
    assert!(error.message.contains("task.channel element"));
    assert!(error.message.contains("TcpStream"));
    assert!(error.message.contains("Local/!Send"));
}

#[test]
fn channel_send_consumes_a_named_non_copy_value() {
    let source = r#"package app.main

import std.task

suspend fn worker(channel_value: Channel<string>, value: string) -> void {
    let sent: Result<void, ChannelSendError<string>> = task.send(channel_value, value)
    let invalid: string = value
}

suspend fn main() -> void {
}
"#;

    let error = parse_inline(source).unwrap_err();

    assert_eq!(error.code, "E0881");
    assert!(error.message.contains("task.send"));
    assert!(error.message.contains("binding `value`"));
}

#[test]
fn channel_handles_are_shared_across_structured_spawn() {
    let source = r#"package app.main

import std.task

suspend fn child(channel_value: Channel<string>) -> void {
    task.yield_now()
}

suspend fn parent(channel_value: Channel<string>) -> void {
    task.scope {
        let child_task = task.spawn child(channel_value)
        let still_available: Channel<string> = channel_value
        let joined: Result<void, TaskError> = task.join(child_task)
    }
}

suspend fn main() -> void {
}
"#;

    let program = parse_inline(source).unwrap();
    let parent = program
        .functions
        .iter()
        .find(|function| function.name == "parent")
        .unwrap();
    let Statement::Let {
        initializer: ValueExpr::Call { args, .. },
        ..
    } = &parent.body[0]
    else {
        panic!("expected structured spawn")
    };
    assert!(matches!(
        args.as_slice(),
        [ValueExpr::Variable(name)] if name == "channel_value"
    ));
}
