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
