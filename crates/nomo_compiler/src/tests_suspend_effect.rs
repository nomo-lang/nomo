use super::*;

#[test]
fn lowers_suspend_effect_into_typed_ir() {
    let source = r#"package app

fn normalize(value: string) -> string {
    return value
}

suspend fn load(value: string) -> string {
    return normalize(value)
}

suspend fn main() {
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
    let source = r#"package app

suspend fn load() -> string {
    return "ready"
}

fn main() {
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
fn rejects_unsuffixed_async_http_from_synchronous_function_without_leaking_arguments() {
    let source = r#"package app

import std.http
import std.result

fn main() {
    let result: Result<HttpResponse, HttpError> = http.get("https://example.invalid/?token=http-secret-sentinel")
}
"#;

    let error = parse_inline(source).unwrap_err();

    assert_eq!(error.code, "E0870");
    assert!(error.message.contains("suspend function `http.get`"));
    assert!(error.message.contains("http.get_blocking"));
    assert!(!error.message.contains("http-secret-sentinel"));
    assert_eq!(error.text, "http.get(...)");
}

#[test]
fn preserves_suspend_effect_for_generic_instances() {
    let source = r#"package app

suspend fn identity<T>(value: T) -> T {
    return value
}

suspend fn main() {
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
    let source = r#"package app

struct Client {
}

impl Client {
    suspend fn load(self) -> string {
        return "ready"
    }
}

fn main() {
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
    let source = r#"package app

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

fn main() {
}
"#;

    let error = parse_inline(source).unwrap_err();

    assert_eq!(error.code, "E0258");
    assert!(error.message.contains("suspend effect does not match"));
}

#[test]
fn suspend_worker_is_not_a_legacy_task_callback() {
    let source = r#"package app

import std.task

suspend fn worker(context: TaskContext, input: string) -> string {
    return input
}

fn main() {
    let started: Result<Task, TaskError> = task.spawn(worker, "ready")
}
"#;

    let error = parse_inline(source).unwrap_err();

    assert_eq!(error.code, "E0820");
    assert!(error.message.contains("must have signature"));
}

#[test]
fn synchronous_codegen_has_no_async_runtime_or_frame_metadata() {
    let source = r#"package app

fn helper() -> string {
    return "ready"
}

fn main() {
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
    let source = r#"package app

suspend fn ready() -> string {
    return "ready"
}

suspend fn main() {
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
    let source = r#"package app

import std.task

fn main() {
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
    let source = r#"package app

import std.task
import std.time

fn main() {
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
    let missing = r#"package app

import std.task

suspend fn main() {
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

    let wrong_type = r#"package app

import std.task

suspend fn main() {
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
    let source = r#"package app

import std.task
import std.time

suspend fn main() {
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
    let source = r#"package app

import std.task
import std.time

suspend fn main() {
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
            "package app\n\nimport std.task\nimport std.time\n\nfn main() {{\n    {body}\n}}\n"
        );
        let error = parse_inline(&source).unwrap_err();
        assert_eq!(error.code, "E0870");
        assert!(error.message.contains("suspend"));
    }
}

#[test]
fn task_deadline_rejects_wrong_duration_and_unsupported_early_exit() {
    let wrong_duration = r#"package app

import std.task

suspend fn main() {
    task.deadline(1) {
        task.check_cancelled()
    }
}
"#;
    let error = parse_inline(wrong_duration).unwrap_err();
    assert_eq!(error.code, "E0404");
    assert!(error.message.contains("Duration"));

    let early_return = r#"package app

import std.task
import std.time

suspend fn main() {
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
    let source = r#"package app

import std.task
import std.time

suspend fn main() {
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
    let source = r#"package app

import std.task.sleep
import std.time

suspend fn main() {
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
    let source = r#"package app

import std.task
import std.time

suspend fn main() {
    task.sleep(time.duration_millis(1))
}
"#;

    let error = parse_inline(source).unwrap_err();

    assert_eq!(error.code, "E0876");
    assert!(error.message.contains("`let`-bound `task.sleep(Duration)`"));
}

#[test]
fn rejects_blocking_sleep_directly_from_suspend_function() {
    let source = r#"package app

import std.time

suspend fn main() {
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
fn rejects_explicit_blocking_http_from_suspend_without_leaking_arguments() {
    let source = r#"package app

import std.http

suspend fn main() {
    let result: Result<HttpResponse, HttpError> = http.get_blocking("https://example.invalid/?token=http-secret-sentinel")
}
"#;

    let error = parse_inline(source).unwrap_err();

    assert_eq!(error.code, "E0891");
    assert!(error.message.contains("main -> http.get_blocking"));
    assert!(error.message.contains("nonblocking suspend equivalent"));
    assert!(!error.message.contains("http-secret-sentinel"));
    assert_eq!(error.text, "http.get_blocking(...)");
}

#[test]
fn rejects_specifically_imported_blocking_http_stream_pull() {
    let source = r#"package app

import std.http.HttpError
import std.http.BlockingHttpStream
import std.http.SseEvent
import std.http.next_sse_blocking

suspend fn poll(stream: BlockingHttpStream) {
    let result: Result<Option<SseEvent>, HttpError> = next_sse_blocking(stream, 1024)
}

fn main() {
}
"#;

    let error = parse_inline(source).unwrap_err();

    assert_eq!(error.code, "E0891");
    assert!(error.message.contains("poll -> http.next_sse_blocking"));
}

#[test]
fn rejects_transitive_blocking_process_call_from_suspend() {
    let source = r#"package app

import std.process

fn invoke() -> Result<string, ProcessError> {
    return process.exec("process-secret-sentinel")
}

suspend fn main() {
    let result: Result<string, ProcessError> = invoke()
}
"#;

    let error = parse_inline(source).unwrap_err();

    assert_eq!(error.code, "E0891");
    assert!(error.message.contains("main -> invoke -> process.exec"));
    assert!(!error.message.contains("process-secret-sentinel"));
    assert_eq!(error.text, "process.exec(...)");
}

#[test]
fn allows_nonwaiting_process_poll_from_suspend_compatibility_code() {
    let source = r#"package app

import std.process

suspend fn observe(child: ProcessChild) {
    let result: Result<Option<ProcessExit>, ProcessControlError> = process.try_wait(child)
}

fn main() {
}
"#;

    parse_inline(source).unwrap();
}

#[test]
fn allows_explicit_blocking_http_in_synchronous_compatibility_code() {
    let source = r#"package app

import std.http
import std.result

fn main() {
    let result: Result<HttpResponse, HttpError> = http.get_blocking("https://example.invalid/")
}
"#;

    parse_inline(source).unwrap();
}

#[test]
fn async_http_surface_lowers_to_typed_placeholder_suspend_abi() {
    let source = r#"package app

import std.array.Array
import std.http
import std.result

suspend fn pull(stream: HttpStream) {
    let chunk: Result<HttpStreamChunk, HttpError> = http.read_text(stream, 4096)
    let event: Result<Option<SseEvent>, HttpError> = http.next_sse(stream, 65536)
}

suspend fn main() {
    let get_result: Result<HttpResponse, HttpError> = http.get("https://example.invalid/get")
    let post_result: Result<HttpResponse, HttpError> = http.post("https://example.invalid/post", "{}")
    let headers: Array<HttpHeader> = Array.new<HttpHeader>()
    let request: HttpRequest = HttpRequest {
        method: "POST",
        url: "https://example.invalid/v1/chat/completions",
        headers: headers,
        body: "{}",
        timeout_millis: 1000,
        max_response_bytes: 4096
    }
    let send_result: Result<HttpResponse, HttpError> = http.send(request)
    let stream_result: Result<HttpStream, HttpError> = http.open_stream(request, 1000)
}
"#;

    let program = parse_inline(source).unwrap();
    let main = program
        .functions
        .iter()
        .find(|function| function.name == "main")
        .unwrap();
    assert!(main.is_suspend);
    assert!(main.body.iter().any(|statement| matches!(
        statement,
        Statement::Let {
            initializer: ValueExpr::Call { name, args },
            ..
        } if name == BUILTIN_HTTP_SEND_EXPR && args.len() == 1
    )));

    let c = compile_source_text_to_c_with_project_modules(
        Path::new("main.nomo"),
        source,
        None,
        &[],
        &[],
    )
    .unwrap();
    for symbol in [
        "nomo_async_http_get_start",
        "nomo_async_http_post_resume",
        "nomo_async_http_send_start",
        "nomo_async_http_open_stream_resume",
        "nomo_async_http_read_text_start",
        "nomo_async_http_next_sse_resume",
        "nomo_async_http_cancel",
        "nomo_async_http_runtime_shutdown",
    ] {
        assert!(c.contains(symbol), "missing generated C symbol `{symbol}`");
    }
    assert!(c.contains("runtime_unavailable"));
    assert!(!c.contains("curl_easy_perform"));
    assert!(!c.contains("WinHttpSendRequest"));
}

#[test]
fn async_tcp_connect_lowers_to_owner_affine_reactor_registration() {
    let source = r#"package app

import std.net
import std.result

suspend fn main() {
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
fn async_process_intrinsics_lower_to_owner_affine_state_machine_abi() {
    let source = r#"package app

import std.process
import std.result

suspend fn launch(command: ProcessCommand) {
    let started: Result<ProcessChild, ProcessControlError> = process.start(command, 100)
}

suspend fn pull(child: ProcessChild) {
    let event: Result<ProcessEvent, ProcessControlError> = process.next_event(child, 4096, 100)
}

fn main() {
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
        } if name == BUILTIN_PROCESS_START_EXPR && args.len() == 2
    ));
    let pull = program
        .functions
        .iter()
        .find(|function| function.name == "pull")
        .unwrap();
    assert!(matches!(
        &pull.body[0],
        Statement::Let {
            initializer: ValueExpr::Call { name, args },
            ..
        } if name == BUILTIN_PROCESS_NEXT_EVENT_EXPR && args.len() == 3
    ));

    let c = compile_source_text_to_c_with_project_modules(
        Path::new("main.nomo"),
        source,
        None,
        &[],
        &[],
    )
    .unwrap();
    for expected in [
        "nomo_async_process_registration",
        "nomo_async_process_spawn_start",
        "nomo_async_process_spawn_resume",
        "nomo_async_process_event_start",
        "nomo_async_process_event_resume",
        "nomo_async_process_cancel",
        "nomo_async_process_command_0",
    ] {
        assert!(c.contains(expected), "missing generated helper {expected}");
    }
    assert!(c.contains("nomo_async_process_result_owned_0 = 1u"));
    assert!(!c.contains("nomo_process_control_states"));
    if cfg!(windows) {
        assert_eq!(c.matches("CreateThread(").count(), 1);
        assert!(c.contains("CreateNamedPipeW"));
        assert!(c.contains("RegisterWaitForSingleObject"));
        assert!(c.contains("WT_EXECUTEONLYONCE"));
        assert!(c.contains("FILE_FLAG_OVERLAPPED"));
        assert!(!c.contains("async process pipes are not available"));
        assert!(!c.contains("pthread_create"));
    } else {
        assert!(!c.contains("CreateThread"));
        assert!(c.contains("pthread_create"));
        assert!(c.contains("nomo_async_process_pool_watch"));
        assert!(c.contains("NOMO_ASYNC_REACTOR_PROCESS"));
        assert!(!c.contains("async process pipes are not available"));
    }

    for target_text in [
        "x86_64-unknown-linux-gnu",
        "aarch64-apple-darwin",
        "x86_64-pc-windows-msvc",
    ] {
        let target = target_text.parse::<nomo_target::TargetTriple>().unwrap();
        let target_c = codegen::emit_c_for_target(&program, &target);
        assert!(target_c.contains("nomo_async_process_spawn_start"));
        assert!(target_c.contains("nomo_async_process_event_start"));
        assert!(!target_c.contains("nomo_process_control_states"));
        match target_text {
            "x86_64-unknown-linux-gnu" => {
                assert!(!target_c.contains("CreateThread"));
                assert!(target_c.contains("pthread_create"));
                assert!(target_c.contains("epoll_create(1)"));
                assert!(target_c.contains("SYS_pidfd_open"));
                assert!(target_c.contains("extern long syscall(long number, ...);"));
                assert!(!target_c.contains("async process pipes are not available"));
            }
            "aarch64-apple-darwin" => {
                assert!(!target_c.contains("CreateThread"));
                assert!(target_c.contains("pthread_create"));
                assert!(target_c.contains("kqueue()"));
                assert!(target_c.contains("EVFILT_PROC"));
                assert!(!target_c.contains("async process pipes are not available"));
            }
            "x86_64-pc-windows-msvc" => {
                assert_eq!(target_c.matches("CreateThread(").count(), 1);
                assert!(!target_c.contains("pthread_create"));
                assert!(target_c.contains("CreateNamedPipeW"));
                assert!(target_c.contains("RegisterWaitForSingleObject"));
                assert!(target_c.contains("WT_EXECUTEONLYONCE"));
                assert!(target_c.contains("FILE_FLAG_OVERLAPPED"));
                assert!(target_c.contains("CancelIoEx"));
                assert!(target_c.contains("nomo_async_process_pool_maybe_idle"));
                let connect_index = target_c
                    .find("ConnectNamedPipe(server, &connected_overlapped)")
                    .expect("Windows process pipe must initiate its server connection");
                let client_index = target_c
                    .find("HANDLE client = CreateFileW(")
                    .expect("Windows process pipe must open its child endpoint");
                assert!(
                    connect_index < client_index,
                    "Windows process pipe must connect its server before opening the client"
                );
                let initialize_start = target_c
                    .find("static int nomo_async_process_pool_initialize(")
                    .expect("Windows process pool initializer must be emitted");
                let runtime_get_start = target_c
                    .find("static nomo_async_process_runtime *nomo_async_process_runtime_get(")
                    .expect("Windows process runtime getter must be emitted");
                assert!(
                    !target_c[initialize_start..runtime_get_start]
                        .contains("nomo_async_reactor_post_activate("),
                    "an idle Windows process pool must not keep the executor alive"
                );
                let submit_start = target_c
                    .find("static int nomo_async_process_pool_submit_start(")
                    .expect("Windows process submit path must be emitted");
                let cancel_start = target_c
                    .find("static void nomo_async_process_pool_cancel_start(")
                    .expect("Windows process cancel path must be emitted");
                assert!(
                    target_c[submit_start..cancel_start]
                        .contains("nomo_async_reactor_post_activate("),
                    "Windows process completion wake must activate on demand"
                );
                assert!(!target_c.contains("nomo_member_program.len"));
                assert!(!target_c.contains("length != data.len"));
                assert!(!target_c.contains("nomo_process_windows_reader_thread"));
                assert!(!target_c.contains("nomo_process_windows_writer_thread"));
                assert!(!target_c.contains("async process pipes are not available"));
            }
            _ => unreachable!(),
        }
    }
}

#[test]
fn rejects_async_process_start_from_synchronous_function() {
    let source = r#"package app

import std.process
import std.result

fn launch(command_secret_sentinel: ProcessCommand) {
    let started: Result<ProcessChild, ProcessControlError> = process.start(command_secret_sentinel, 100)
}

fn main() {
}
"#;

    let error = parse_inline(source).unwrap_err();
    assert_eq!(error.code, "E0870", "{error:?}");
    assert!(error.message.contains("suspend function"));
    assert_eq!(error.text, "process.start(...)");
    assert!(!error.message.contains("command_secret_sentinel"));
    assert!(!error.text.contains("command_secret_sentinel"));
}

#[test]
fn rejects_blocking_process_migration_api_from_suspend_without_arguments() {
    let source = r#"package app

import std.process

suspend fn launch(command: ProcessCommand) {
    let started: Result<BlockingProcessChild, ProcessControlError> = process.start_blocking(command)
}

fn main() {
}
"#;

    let error = parse_inline(source).unwrap_err();
    assert_eq!(error.code, "E0891", "{error:?}");
    assert!(error.message.contains("launch -> process.start_blocking"));
    assert_eq!(error.text, "process.start_blocking(...)");
}

#[test]
fn rejects_question_propagation_directly_on_async_tcp_connect_in_the_first_slice() {
    let source = r#"package app

import std.net
import std.result

suspend fn connect_once() -> Result<TcpStream, NetError> {
    let stream: TcpStream = net.connect("127.0.0.1", 9, 100)?
    return Ok(stream)
}

fn main() {
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
    let source = r#"package app

import std.net
import std.result

suspend fn main() {
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
    let source = r#"package app

import std.net
import std.result

suspend fn exercise(stream: TcpStream) {
    let bytes: Result<TcpChunk, NetError> = stream.read(4096, 100)
    let text: Result<TcpTextChunk, NetError> = stream.read_string(4096, 100)
    let wrote_bytes: Result<void, NetError> = stream.write([65, 66, 67], 100)
    let wrote_text: Result<void, NetError> = stream.write_string("ready", 100)
    let shutdown: Result<void, NetError> = stream.shutdown_write()
}

fn main() {
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
            BUILTIN_TCP_STREAM_SHUTDOWN_WRITE_EXPR,
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
        "nomo_tcp_stream_shutdown_write",
        "nomo_async_io_handle_shutdown_write_callback",
        "if (slot->write_busy != 0u)",
        "if (slot->write_shutdown != 0u)",
        "shutdown(slot->handle, NOMO_SOCKET_SHUTDOWN_WRITE)",
        "slot->write_shutdown = 1u",
        "NOMO_ASYNC_TCP_SEND_FLAGS",
        "NOMO_ASYNC_TCP_WRITE_POLL_BUDGET 65536u",
    ] {
        assert!(c.contains(helper), "missing generated helper {helper}");
    }
}

#[test]
fn async_tcp_stream_io_windows_uses_overlapped_winsock_and_safe_late_completion_storage() {
    let source = r#"package app

import std.net
import std.result

suspend fn exercise(stream: TcpStream) {
    let bytes: Result<TcpChunk, NetError> = stream.read(16, 100)
    let wrote: Result<void, NetError> = stream.write_string("secret", 100)
    let shutdown: Result<void, NetError> = stream.shutdown_write()
}

fn main() {
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
        "nomo_tcp_stream_shutdown_write",
        "nomo_async_io_handle_shutdown_write_callback",
        "#define NOMO_SOCKET_SHUTDOWN_WRITE SD_SEND",
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
    let source = r#"package app

import std.net
import std.result

fn read_once(stream: TcpStream) {
    let result: Result<TcpChunk, NetError> = stream.read(16, 100)
}

fn main() {
}
"#;

    let error = parse_inline(source).unwrap_err();
    assert_eq!(error.code, "E0870");
    assert!(error.message.contains("suspend function"));
}

#[test]
fn rejects_async_tcp_connect_from_synchronous_function() {
    let source = r#"package app

import std.net
import std.result

fn main() {
    let result: Result<TcpStream, NetError> = net.connect("127.0.0.1", 9, 100)
}
"#;

    let error = parse_inline(source).unwrap_err();
    assert_eq!(error.code, "E0870");
    assert!(error.message.contains("suspend function"));
}

#[test]
fn rejects_blocking_tcp_connect_from_suspend_function() {
    let source = r#"package app

import std.net

suspend fn main() {
    let result: Result<TcpStream, NetError> = net.connect_blocking("127.0.0.1", 9)
}
"#;

    let error = parse_inline(source).unwrap_err();
    assert_eq!(error.code, "E0891", "{error:?}");
    assert!(error.message.contains("main -> net.connect_blocking"));
}

#[test]
fn rejects_blocking_tcp_stream_method_from_suspend_function() {
    let source = r#"package app

import std.net
import std.result

suspend fn read(stream: TcpStream) -> Result<string, NetError> {
    return stream.read_to_string_blocking()
}

fn main() {
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
    let source = r#"package app

struct Reader {
    value: string
}

impl Reader {
    fn read_to_string_blocking(self) -> string {
        return self.value
    }
}

suspend fn main() {
    let reader: Reader = Reader { value: "local" }
    let text: string = reader.read_to_string_blocking()
}
"#;

    parse_inline(source).unwrap();
}

#[test]
fn rejects_specifically_imported_blocking_sleep_from_suspend_function() {
    let source = r#"package app

import std.time.sleep_millis

suspend fn main() {
    sleep_millis(1)
}
"#;

    let error = parse_inline(source).unwrap_err();

    assert_eq!(error.code, "E0891");
    assert!(error.message.contains("main -> time.sleep_millis"));
}

#[test]
fn rejects_transitive_blocking_sleep_with_a_secret_safe_call_path() {
    let source = r#"package app

import std.time

fn pause(secret: string) {
    time.sleep_millis(1)
}

fn helper(secret: string) {
    pause(secret)
}

suspend fn main() {
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
    let synchronous = r#"package app

import std.time

fn main() {
    time.sleep_millis(0)
}
"#;
    parse_inline(synchronous).unwrap();

    let worker = r#"package app

import std.task
import std.time

fn worker(context: TaskContext, input: string) -> string {
    time.sleep_millis(0)
    return input
}

fn main() {
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
        "package app.worker\n\nimport std.time\n\npub fn pause() {\n    time.sleep_millis(1)\n}\n",
    )
    .unwrap();
    let main = src.join("main.nomo");
    let source = r#"package app

import app.worker

suspend fn main() {
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
    let source = r#"package app

import std.task.yield_now

suspend fn main() {
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
    let source = r#"package app

import std.io
import std.task

suspend fn main() {
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
    let source = r#"package app

import std.io
import std.task

suspend fn main() {
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
    let source = r#"package app

import std.array
import std.io
import std.task

suspend fn main() {
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
    let mutable_source = r#"package app

import std.io
import std.task

suspend fn main() {
    let mut message: string = "mutable"
    task.yield_now()
    io.println(message)
}
"#;
    let mutable_error = parse_inline(mutable_source).unwrap_err();
    assert_eq!(mutable_error.code, "E0876");
    assert!(
        mutable_error
            .message
            .contains("mutable locals not owned by the supported loop")
    );

    let panic_source = r#"package app

import std.debug
import std.task

suspend fn fail(message: string) {
    task.yield_now()
    debug.panic(message)
}

suspend fn main() {
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

    let handle_source = r#"package app

import std.io
import std.task

fn worker(context: TaskContext, input: string) -> string {
    return input
}

suspend fn main() {
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

    let question_outside_scope = r#"package app

import std.num
import std.task

suspend fn parse_after_yield() -> Result<i64, NumError> {
    task.yield_now()
    let value: i64 = num.parse_i64("42")?
    return Ok(value)
}

suspend fn main() {
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
    let source = r#"package app

import std.io
import std.task

suspend fn leaf() {
    let leaf_value: string = "leaf"
    task.yield_now()
    io.println(leaf_value)
}

suspend fn helper() {
    let child: string = "child"
    leaf()
    io.println(child)
}

suspend fn main() {
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
    let source = r#"package app

import std.io
import std.task

suspend fn helper(value: string, count: u64) -> string {
    let owned: string = value
    task.yield_now()
    io.println(count)
    return owned
}

suspend fn main() {
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
    let unbound_source = r#"package app

import std.task

suspend fn helper() -> string {
    task.yield_now()
    return "value"
}

suspend fn main() {
    helper()
}
"#;
    let unbound_error = parse_inline(unbound_source).unwrap_err();
    assert_eq!(unbound_error.code, "E0876");
    assert!(unbound_error.message.contains("bind the result"));

    let recursive_source = r#"package app

import std.task

suspend fn first() {
    second()
}

suspend fn second() {
    task.yield_now()
    first()
}

suspend fn main() {
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

    let nested_source = r#"package app

import std.task

suspend fn main() {
    for {
        task.yield_now()
    }
}
"#;
    let nested_error = parse_inline(nested_source).unwrap_err();
    assert_eq!(nested_error.code, "E0876");
    assert!(nested_error.message.contains("nested loops"));
}

#[test]
fn bounded_suspending_loop_lowers_loop_carried_scalar_and_managed_state() {
    let source = r#"package app

import std.io
import std.task
import std.time

suspend fn main() {
    let finished_before_loop: string = "finished"
    task.yield_now()
    io.println(finished_before_loop)
    let mut remaining: u64 = 3
    let mut message: string = "initial"
    for remaining > 0 {
        task.yield_now()
        let slept: Result<void, TaskError> = task.sleep(time.duration_millis(0))
        message = "updated"
        remaining = remaining - 1
    }
    io.println(message)
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

    assert!(c.contains("nomo_async_loop_condition_0:"));
    assert!(c.contains("goto nomo_async_loop_condition_0;"));
    assert!(c.contains("nomo_async_loop_after_0:"));
    assert!(c.contains("frame->nomo_async_local_nomo_remaining"));
    assert!(c.contains("frame->nomo_async_local_nomo_message"));
    assert!(c.contains("nomo_async_assign_nomo_message_"));
    assert!(c.contains("nomo_string_release(frame->nomo_async_local_nomo_message);"));
    let condition = c
        .split("nomo_async_loop_condition_0:")
        .nth(1)
        .unwrap()
        .split("nomo_async_loop_after_0:")
        .next()
        .unwrap();
    assert!(!condition.contains("nomo_finished_before_loop = frame->"));
    assert_eq!(
        c.matches("nomo_async_frame_main nomo__frame = {0};")
            .count(),
        1
    );
}

#[test]
fn bounded_suspending_loop_accepts_a_transitive_suspend_call() {
    let source = r#"package app

import std.io
import std.task

suspend fn one_round() {
    task.yield_now()
}

suspend fn main() {
    let mut remaining: u64 = 3
    for remaining > 0 {
        one_round()
        remaining = remaining - 1
    }
    io.println(remaining)
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

    assert!(c.contains("nomo_async_loop_condition_0:"));
    assert!(c.contains("nomo_async_child_"));
    assert!(c.contains("goto nomo_async_loop_condition_0;"));
}

#[test]
fn bounded_suspending_loop_rejects_nested_condition_and_early_exit_shapes() {
    let nested = r#"package app

import std.task

suspend fn main() {
    let mut running: bool = true
    for running {
        for running {
            task.yield_now()
        }
        running = false
    }
}
"#;
    let nested_error = parse_inline(nested).unwrap_err();
    assert_eq!(nested_error.code, "E0876");
    assert!(nested_error.message.contains("nested loops"));

    let suspending_condition = r#"package app

import std.task

suspend fn ready() -> bool {
    task.yield_now()
    return true
}

suspend fn main() {
    for ready() {
        task.yield_now()
    }
}
"#;
    let condition_error = parse_inline(suspending_condition).unwrap_err();
    assert_eq!(condition_error.code, "E0876");
    assert!(
        condition_error
            .message
            .contains("suspending loop conditions")
    );

    let early_exit = r#"package app

import std.task

suspend fn main() {
    let mut running: bool = true
    for running {
        task.yield_now()
        break
    }
}
"#;
    let early_error = parse_inline(early_exit).unwrap_err();
    assert_eq!(early_error.code, "E0876");
    assert!(early_error.message.contains("loop early exits"));
}

#[test]
fn suspend_call_abi_rejects_mutable_affine_and_root_result_shapes() {
    let mutable_parameter_source = r#"package app

import std.task

suspend fn helper(mut value: string) {
    task.yield_now()
}

suspend fn main() {
    helper("value")
}
"#;
    let mutable_error = parse_inline(mutable_parameter_source).unwrap_err();
    assert_eq!(mutable_error.code, "E0876");
    assert!(mutable_error.message.contains("mutable parameters"));

    let affine_parameter_source = r#"package app

import std.task

suspend fn helper(value: Task) {
    task.yield_now()
}

suspend fn main() {
    task.yield_now()
}
"#;
    let affine_error = parse_inline(affine_parameter_source).unwrap_err();
    assert_eq!(affine_error.code, "E0876");
    assert!(affine_error.message.contains("parameter `value`"));
    assert!(affine_error.message.contains("frame-safe"));

    let root_result_source = r#"package app

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
    let source = r#"package app

import std.task

suspend fn worker(value: string) {
    task.yield_now()
}

suspend fn main() {
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
    let source = r#"package app

import std.task

suspend fn worker(value: string) {
    task.yield_now()
}

suspend fn launch(value: string) {
    task.scope {
        let child = task.spawn worker(value)
        let joined: Result<void, TaskError> = task.join(child)
    }
}

suspend fn main() {
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
    let source = r#"package app

import std.task

suspend fn worker(value: i64) {
    task.yield_now()
}

suspend fn launch(value: i64) {
    task.scope {
        let child = task.spawn worker(value)
        let kept: i64 = value
        let joined: Result<void, TaskError> = task.join(child)
    }
}

suspend fn main() {
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
    let source = r#"package app

import std.array
import std.task

struct Envelope {
    message: string
    tags: Array<string>
}

suspend fn worker(value: Envelope) {
    task.yield_now()
}

suspend fn launch(value: Envelope) {
    task.scope {
        let child = task.spawn worker(value)
        let joined: Result<void, TaskError> = task.join(child)
    }
}

suspend fn main() {
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
    let direct = r#"package app

import std.fs
import std.task

suspend fn worker(file: File) {
    task.yield_now()
}

suspend fn launch(file: File) {
    task.scope {
        let child = task.spawn worker(file)
        let joined: Result<void, TaskError> = task.join(child)
    }
}

suspend fn main() {
}
"#;
    let direct_error = parse_inline(direct).unwrap_err();
    assert_eq!(direct_error.code, "E0880");
    assert!(direct_error.message.contains("Local/!Send type `File`"));

    let nested = r#"package app

import std.fs
import std.task

struct Envelope {
    file: File
}

suspend fn worker(value: Envelope) {
    task.yield_now()
}

suspend fn launch(value: Envelope) {
    task.scope {
        let child = task.spawn worker(value)
        let joined: Result<void, TaskError> = task.join(child)
    }
}

suspend fn main() {
}
"#;
    let nested_error = parse_inline(nested).unwrap_err();
    assert_eq!(nested_error.code, "E0883");
    assert!(nested_error.message.contains("Envelope.file"));
    assert!(nested_error.message.contains("Local/!Send type `File`"));

    let process = r#"package app

import std.process
import std.task

suspend fn worker(child: ProcessChild) {
    task.yield_now()
}

suspend fn launch(child: ProcessChild) {
    task.scope {
        let task_value = task.spawn worker(child)
        let joined: Result<void, TaskError> = task.join(task_value)
    }
}

suspend fn main() {
}
"#;
    let process_error = parse_inline(process).unwrap_err();
    assert_eq!(process_error.code, "E0880");
    assert!(
        process_error
            .message
            .contains("Local/!Send type `ProcessChild`")
    );
}

#[test]
fn structured_spawn_rejects_partial_duplicate_and_later_move_uses() {
    let partial = r#"package app

import std.task

struct Envelope {
    message: string
}

suspend fn worker(value: string) {
    task.yield_now()
}

suspend fn launch(value: Envelope) {
    task.scope {
        let child = task.spawn worker(value.message)
        let joined: Result<void, TaskError> = task.join(child)
    }
}

suspend fn main() {
}
"#;
    let partial_error = parse_inline(partial).unwrap_err();
    assert_eq!(partial_error.code, "E0883");
    assert!(
        partial_error
            .message
            .contains("cannot move only `value.message`")
    );

    let duplicate = r#"package app

import std.task

suspend fn worker(left: string, right: string) {
    task.yield_now()
}

suspend fn launch(value: string) {
    task.scope {
        let child = task.spawn worker(value, value)
        let joined: Result<void, TaskError> = task.join(child)
    }
}

suspend fn main() {
}
"#;
    let duplicate_error = parse_inline(duplicate).unwrap_err();
    assert_eq!(duplicate_error.code, "E0881");
    assert!(duplicate_error.message.contains("consumed more than once"));

    let later_use = r#"package app

import std.task

suspend fn worker(value: string) {
    task.yield_now()
}

suspend fn launch(value: string) {
    task.scope {
        let child = task.spawn worker(value)
        let reused: string = value
        let joined: Result<void, TaskError> = task.join(child)
    }
}

suspend fn main() {
}
"#;
    let later_error = parse_inline(later_use).unwrap_err();
    assert_eq!(later_error.code, "E0881");
    assert!(later_error.message.contains("publication move"));
    assert!(later_error.message.contains("structured task.spawn"));

    let after_scope = r#"package app

import std.task

suspend fn worker(value: string) {
    task.yield_now()
}

suspend fn launch(value: string) {
    task.scope {
        let child = task.spawn worker(value)
        let joined: Result<void, TaskError> = task.join(child)
    }
    let reused: string = value
}

suspend fn main() {
}
"#;
    let after_scope_error = parse_inline(after_scope).unwrap_err();
    assert_eq!(after_scope_error.code, "E0881");
    assert!(after_scope_error.message.contains("publication move"));

    let question_use = r#"package app

import std.task

fn keep(value: string) -> Result<string, TaskError> {
    return Ok(value)
}

suspend fn worker(value: string) {
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

suspend fn main() {
}
"#;
    let question_error = parse_inline(question_use).unwrap_err();
    assert_eq!(question_error.code, "E0881");
    assert!(question_error.message.contains("publication move"));
}

#[test]
fn structured_spawn_does_not_consume_reusable_constants() {
    let source = r#"package app

import std.task

const greeting: string = "hello"

suspend fn worker(value: string) {
    task.yield_now()
}

suspend fn main() {
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
    let source = r#"package app

import std.task

suspend fn worker(value: string) -> string {
    task.yield_now()
    return value
}

suspend fn main() {
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
    let source = r#"package app

import std.result
import std.task

suspend fn worker(value: string) -> string {
    task.yield_now()
    return value
}

suspend fn main() {
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
    let source = r#"package app

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

suspend fn main() {
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
    let source = r#"package app

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

suspend fn main() {
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
    let outside_scope = r#"package app

import std.task

suspend fn worker() {
}

suspend fn main() {
    let child = task.spawn worker()
}
"#;
    let error = parse_inline(outside_scope).unwrap_err();
    assert_eq!(error.code, "E0871");

    let synchronous_scope = r#"package app

import std.task

fn main() {
    task.scope {
    }
}
"#;
    let error = parse_inline(synchronous_scope).unwrap_err();
    assert_eq!(error.code, "E0870");

    let non_suspend_target = r#"package app

import std.task

fn worker() {
}

suspend fn main() {
    task.scope {
        let child = task.spawn worker()
        let joined: Result<void, TaskError> = task.join(child)
    }
}
"#;
    let error = parse_inline(non_suspend_target).unwrap_err();
    assert_eq!(error.code, "E0875");
    assert!(error.message.contains("must be declared `suspend fn`"));

    let panicking_target = r#"package app

import std.task

suspend fn worker() {
    panic("structured child panic cleanup")
}

suspend fn main() {
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

    let double_join = r#"package app

import std.task

suspend fn worker() {
}

suspend fn main() {
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

    let escaped_handle = r#"package app

import std.io
import std.task

suspend fn worker() {
}

suspend fn main() {
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

    let unjoined_handles = r#"package app

import std.task

suspend fn worker() {
}

suspend fn main() {
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

    let non_terminal_return = r#"package app

import std.task

suspend fn main() {
    task.scope {
        return
        let value: i64 = 1
    }
}
"#;
    let error = parse_inline(non_terminal_return).unwrap_err();
    assert_eq!(error.code, "E0876");
    assert!(error.message.contains("final statement"));

    let unjoined_return = r#"package app

import std.task

suspend fn worker() {
}

suspend fn main() {
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

    let typed_return_with_temporary_collision = r#"package app

import std.task

suspend fn worker() {
}

suspend fn finish() -> string {
    task.scope {
        let child = task.spawn worker()
        let __nomo_structured_return_value: string = "value"
        return __nomo_structured_return_value
    }
}

suspend fn main() {
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

    let returned_handle = r#"package app

import std.task

suspend fn worker() {
}

suspend fn escape() {
    task.scope {
        let child = task.spawn worker()
        return child
    }
}

suspend fn main() {
}
"#;
    let error = parse_inline(returned_handle).unwrap_err();
    assert_eq!(error.code, "E0872");
    assert!(error.message.contains("cannot be returned"));
}

#[test]
fn structured_void_scope_emits_bounded_fifo_join_wakeup_and_drop_paths() {
    let source = r#"package app

import std.task

suspend fn worker(value: string) {
    task.yield_now()
}

suspend fn main() {
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
    let source = r#"package app

import std.task

suspend fn worker(value: string) -> string {
    task.yield_now()
    return value
}

suspend fn main() {
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
    let source = r#"package app

import std.task
import std.option

suspend fn exchange(channel_value: Channel<string>, value: string) {
    let sent: Result<void, ChannelSendError<string>> = task.send(channel_value, value)
    let received: Option<string> = task.receive(channel_value)
    let immediate: ChannelTryReceive<string> = task.try_receive(channel_value)
    task.close(channel_value)
}

suspend fn main() {
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
    let source = r#"package app

import std.task
import std.time

suspend fn choose(channel_value: Channel<string>) {
    task.select {
        task.receive(channel_value) => received {
            let value = received
        }
        task.sleep(time.duration_millis(5)) => timeout {
            let value = timeout
        }
    }
}

fn main() {
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
fn static_send_join_select_lowers_staged_and_affine_ownership() {
    let source = r#"package app

import std.task
import std.time

suspend fn child() -> string {
    task.yield_now()
    return "child"
}

suspend fn choose(channel_value: Channel<string>) {
    let payload: string = "value"
    task.scope {
        let child_handle = task.spawn child()
        task.select {
            task.send(channel_value, payload) => sent {
                let value = sent
            }
            task.join(child_handle) => joined {
                let value = joined
            }
        }
    }
}

fn main() {
}
"#;
    let program = parse_inline(source).unwrap();
    let choose = program
        .functions
        .iter()
        .find(|function| function.name == "choose")
        .unwrap();
    let Statement::TaskSelect { arms } = &choose.body[2] else {
        panic!("expected static send/join select");
    };
    let TaskSelectOperation::Send {
        value,
        element_type: ValueType::String,
        ..
    } = &arms[0].operation
    else {
        panic!("expected static send arm");
    };
    assert!(matches!(
        value.as_ref(),
        ValueExpr::Call { name, .. } if name == BUILTIN_TASK_PUBLICATION_MOVE_EXPR
    ));
    assert!(matches!(
        &arms[1].operation,
        TaskSelectOperation::Join { handle } if handle == "child_handle"
    ));

    let c = compile_source_text_to_c_with_project_modules(
        Path::new("main.nomo"),
        source,
        None,
        &[],
        &[],
    )
    .unwrap();
    assert!(c.contains("nomo_async_select_send_value_owned_"));
    assert!(c.contains("nomo_channel_send_select_cancel_string"));
    assert!(c.contains("nomo_async_join_select_cancel_child"));
    assert!(c.contains("structured_waiter_select_token"));
}

#[test]
fn static_select_accepts_direct_return_and_rejects_invalid_or_nested_exits() {
    let early_exit = r#"package app

import std.task
import std.time

suspend fn choose(channel_value: Channel<string>) -> string {
    task.select {
        task.send(channel_value, "value") => sent {
            return "send"
        }
        task.sleep(time.duration_millis(5)) => timeout {
            return "timer"
        }
    }
    return "unreachable"
}

fn main() {
}
"#;
    let c = compile_source_text_to_c_with_project_modules(
        Path::new("main.nomo"),
        early_exit,
        None,
        &[],
        &[],
    )
    .unwrap();
    assert!(c.contains("frame->nomo_async_result = nomo_string_literal(\"send\")"));
    assert!(c.contains("frame->nomo_async_result = nomo_string_literal(\"timer\")"));

    let unsupported = r#"package app

import std.task
import std.time

suspend fn choose(channel_value: Channel<string>) {
    task.select {
        task.check_cancelled() => checked {
            let value = checked
        }
        task.sleep(time.duration_millis(5)) => timeout {
            let value = timeout
        }
    }
}

fn main() {
}
"#;
    let error = parse_inline(unsupported).unwrap_err();
    assert_eq!(error.code, "E0886");
    assert!(error.message.contains("supports only"));

    let nested_exit = r#"package app

import std.task
import std.time

suspend fn choose(channel_value: Channel<string>) {
    task.select {
        task.receive(channel_value) => received {
            for {
                return
            }
        }
        task.sleep(time.duration_millis(5)) => timeout {
            let value = timeout
        }
    }
}

fn main() {
}
"#;
    let error = parse_inline(nested_exit).unwrap_err();
    assert_eq!(error.code, "E0876");
    assert!(error.message.contains("direct arm statements"));
}

#[test]
fn static_select_reports_e0887_for_moved_values_and_join_handles_after_select() {
    let moved_value = r#"package app

import std.task
import std.time

suspend fn choose(channel_value: Channel<string>) {
    let payload: string = "value"
    task.select {
        task.send(channel_value, payload) => sent {
            let value = sent
        }
        task.sleep(time.duration_millis(5)) => timeout {
            let value = timeout
        }
    }
    let escaped: string = payload
}

fn main() {
}
"#;
    let error = parse_inline(moved_value).unwrap_err();
    assert_eq!(error.code, "E0887");
    assert!(error.message.contains("staged"));

    let join_handle = r#"package app

import std.task
import std.time

suspend fn child() -> string {
    task.yield_now()
    return "child"
}

suspend fn choose(channel_value: Channel<string>) {
    task.scope {
        let child_handle = task.spawn child()
        task.select {
            task.receive(channel_value) => received {
                let value = received
            }
            task.join(child_handle) => joined {
                let value = joined
            }
        }
        let joined: Result<string, TaskError> = task.join(child_handle)
    }
}

fn main() {
}
"#;
    let error = parse_inline(join_handle).unwrap_err();
    assert_eq!(error.code, "E0887");
    assert!(error.message.contains("unavailable after task.select"));
}

#[test]
fn static_select_early_question_and_panic_paths_carry_structured_cleanup() {
    let source = r#"package app

import std.result
import std.task
import std.time

fn fail() -> Result<string, string> {
    return Result.Err("failed")
}

suspend fn child() -> string {
    task.yield_now()
    return "child"
}

suspend fn choose(channel_value: Channel<string>) -> Result<string, string> {
    task.scope {
        let child_handle = task.spawn child()
        task.select {
            task.send(channel_value, "value") => sent {
                let value: string = fail()?
                return Result.Ok(value)
            }
            task.join(child_handle) => joined {
                panic("boom")
            }
        }
    }
    return Result.Ok("fallthrough")
}

fn main() {
}
"#;
    let program = parse_inline(source).unwrap();
    let choose = program
        .functions
        .iter()
        .find(|function| function.name == "choose")
        .unwrap();
    let Statement::TaskSelect { arms } = &choose.body[1] else {
        panic!("expected task.select after the structured spawn");
    };
    assert!(matches!(
        &arms[0].body[0],
        Statement::QuestionLet {
            early_exit_actions,
            ..
        } if early_exit_actions.len() == 1
    ));
    assert!(matches!(
        arms[1].body.as_slice(),
        [
            Statement::Let { .. },
            Statement::Expr(ValueExpr::Call { name, .. }),
            Statement::Panic(ValueExpr::Variable(_))
        ] if name == BUILTIN_TASK_STRUCTURED_CANCEL_EXPR
    ));

    let c = compile_source_text_to_c_with_project_modules(
        Path::new("main.nomo"),
        source,
        None,
        &[],
        &[],
    )
    .unwrap();
    assert!(c.contains("nomo_async_question_result_"));
    assert!(c.contains("context->pending_reason = NOMO_ASYNC_PENDING_PANIC"));
    assert!(c.contains("nomo_async_cancel_child"));
}

#[test]
fn bounded_channel_requires_send_elements() {
    let source = r#"package app

import std.net
import std.task

suspend fn main() {
    let created: Result<Channel<TcpStream>, ChannelError> = task.channel<TcpStream>(1)
}
"#;

    let error = parse_inline(source).unwrap_err();

    assert_eq!(error.code, "E0880");
    assert!(error.message.contains("task.channel element"));
    assert!(error.message.contains("TcpStream"));
    assert!(error.message.contains("Local/!Send"));

    let process = r#"package app

import std.process
import std.task

suspend fn main() {
    let created: Result<Channel<ProcessChild>, ChannelError> = task.channel<ProcessChild>(1)
}
"#;

    let process_error = parse_inline(process).unwrap_err();

    assert_eq!(process_error.code, "E0880");
    assert!(process_error.message.contains("task.channel element"));
    assert!(process_error.message.contains("ProcessChild"));
    assert!(process_error.message.contains("Local/!Send"));
}

#[test]
fn channel_send_consumes_a_named_non_copy_value() {
    let source = r#"package app

import std.task

suspend fn worker(channel_value: Channel<string>, value: string) {
    let sent: Result<void, ChannelSendError<string>> = task.send(channel_value, value)
    let invalid: string = value
}

suspend fn main() {
}
"#;

    let error = parse_inline(source).unwrap_err();

    assert_eq!(error.code, "E0881");
    assert!(error.message.contains("task.send"));
    assert!(error.message.contains("binding `value`"));
}

#[test]
fn channel_handles_are_shared_across_structured_spawn() {
    let source = r#"package app

import std.task

suspend fn child(channel_value: Channel<string>) {
    task.yield_now()
}

suspend fn parent(channel_value: Channel<string>) {
    task.scope {
        let child_task = task.spawn child(channel_value)
        let still_available: Channel<string> = channel_value
        let joined: Result<void, TaskError> = task.join(child_task)
    }
}

suspend fn main() {
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
