use super::*;

#[test]
fn source_carrier_contract_matches_compiler_injected_carriers() {
    nomo_std::validate_intrinsic_source_contract().unwrap();
    let source = r#"package app.main

import std.option
import std.result

fn main() -> void {
    let option: Option<i64> = Some(1)
    let result: Result<i64, string> = Ok(1)
}
"#;

    let program = parse_inline(source).unwrap();
    let option = program
        .enums
        .iter()
        .find(|item| item.name == "Option")
        .unwrap();
    assert_eq!(option.type_params, vec!["T"]);
    assert_eq!(
        option
            .variants
            .iter()
            .map(|variant| variant.name.as_str())
            .collect::<Vec<_>>(),
        vec!["Some", "None"]
    );
    let result = program
        .enums
        .iter()
        .find(|item| item.name == "Result")
        .unwrap();
    assert_eq!(result.type_params, vec!["T", "E"]);
    assert_eq!(
        result
            .variants
            .iter()
            .map(|variant| variant.name.as_str())
            .collect::<Vec<_>>(),
        vec!["Ok", "Err"]
    );
}

#[test]
fn source_carrier_helpers_typecheck_as_library_modules() {
    for module_name in ["std.option", "std.result"] {
        let module = nomo_std::module(module_name).unwrap();
        let path = nomo_std::module_source_path(module);
        let source = std::fs::read_to_string(&path).unwrap();
        let program = check_module_source_text_with_project_modules_and_overrides(
            &path,
            &source,
            None,
            &[],
            &[],
            &[],
        )
        .unwrap();
        assert_eq!(program.package, module_name);
    }
}

#[test]
fn rejects_unknown_std_import() {
    let source = r#"package app.main

import std.typo

fn main() -> void {
}
"#;

    let err = parse_inline(source).unwrap_err();
    assert_eq!(err.code, "E0301");
    assert!(err.message.contains("std.typo"));
}

#[test]
fn rejects_unknown_specific_std_import() {
    let source = r#"package app.main

import std.io.flush

fn main() -> void {
}
"#;

    let err = parse_inline(source).unwrap_err();
    assert_eq!(err.code, "E0301");
    assert!(err.message.contains("std.io.flush"));
}

#[test]
fn rejects_non_std_import_in_v0_1() {
    let source = r#"package app.main

import app.other

fn main() -> void {
}
"#;

    let err = parse_inline(source).unwrap_err();
    assert_eq!(err.code, "E0301");
    assert!(err.message.contains("app.other"));
}

#[test]
fn rejects_std_module_calls_without_imports() {
    for (source, symbol, import) in [
        (
            "package app.main\nfn main() -> void {\n    let count: u64 = string.len(\"hi\")\n}\n",
            "string.len",
            "std.string",
        ),
        (
            "package app.main\nfn main() -> void {\n    let result: Result<string, FsError> = fs.read_to_string(\"missing.txt\")\n}\n",
            "fs.read_to_string",
            "std.fs",
        ),
        (
            "package app.main\nfn main() -> void {\n    let value: Option<string> = env.get(\"HOME\")\n}\n",
            "env.get",
            "std.env",
        ),
        (
            "package app.main\nfn main() -> void {\n    let name: string = path.basename(\"/tmp/nomo.txt\")\n}\n",
            "path.basename",
            "std.path",
        ),
        (
            "package app.main\nfn main() -> void {\n    let value: i64 = math.abs(0 - 1)\n}\n",
            "math.abs",
            "std.math",
        ),
        (
            "package app.main\nfn main() -> void {\n    let items = Array.new<i32>()\n}\n",
            "Array.new",
            "std.array",
        ),
    ] {
        let err = parse_inline(source).unwrap_err();
        assert_eq!(err.code, "E0301");
        assert!(err.message.contains(symbol), "{:?}", err.message);
        assert!(err.message.contains(import), "{:?}", err.message);
    }
}

#[test]
fn rejects_standard_library_types_without_imports() {
    for (source, type_name, import) in [
        (
            "package app.main\nfn parse() -> Result<i32, string> {\n    return 1\n}\nfn main() -> void {\n}\n",
            "Result",
            "std.result",
        ),
        (
            "package app.main\nfn label(value: Option<i32>) -> void {\n}\nfn main() -> void {\n}\n",
            "Option",
            "std.option",
        ),
        (
            "package app.main\nstruct Bag {\n    items: Array<i32>\n}\nfn main() -> void {\n}\n",
            "Array",
            "std.array",
        ),
        (
            "package app.main\nfn report(error: FsError) -> void {\n}\nfn main() -> void {\n}\n",
            "FsError",
            "std.fs",
        ),
    ] {
        let err = parse_inline(source).unwrap_err();
        assert_eq!(err.code, "E0301", "{:?}", err);
        assert!(err.message.contains(type_name), "{:?}", err.message);
        assert!(err.message.contains(import), "{:?}", err.message);
    }
}

#[test]
fn accepts_fs_read_and_write_builtins() {
    let source = r#"package app.main

import std.fs
import std.io
import std.array.Array

fn load(path: string) -> Result<string, FsError> {
    let text: string = fs.read_to_string(path)?
    return Result.Ok(text)
}

fn load_bytes(path: string) -> Result<Array<u32>, FsError> {
    let bytes: Array<u32> = fs.read_bytes(path)?
    return Result.Ok(bytes)
}

fn save(path: string, content: string) -> Result<void, FsError> {
    return fs.write_string(path, content)
}

fn save_bytes(path: string, bytes: Array<u32>) -> Result<void, FsError> {
    return fs.write_bytes(path, bytes)
}

fn main() -> void {
    let write_result: Result<void, FsError> = save("/tmp/nomo-fs-test.txt", "hello")
    let read_result: Result<string, FsError> = load("/tmp/nomo-fs-test.txt")
    let byte_read_result: Result<Array<u32>, FsError> = load_bytes("/tmp/nomo-fs-test.txt")
    io.println("done")
}
"#;

    let program = parse_inline(source).unwrap();
    assert!(program.structs.iter().any(|item| item.name == "FsError"));
    assert!(program.enums.iter().any(|item| item.name == "Result"));
    let load = program.functions.iter().find(|f| f.name == "load").unwrap();
    assert_eq!(
        load.return_type,
        ValueType::Enum(
            "Result".to_string(),
            vec![
                ValueType::String,
                ValueType::Struct("FsError".to_string(), Vec::new()),
            ],
        )
    );
    assert!(matches!(
        load.body[0],
        Statement::QuestionLet {
            result_expr: ValueExpr::FsReadToString { .. },
            ..
        }
    ));
    let load_bytes = program
        .functions
        .iter()
        .find(|f| f.name == "load_bytes")
        .unwrap();
    assert!(matches!(
        load_bytes.body[0],
        Statement::QuestionLet {
            result_expr: ValueExpr::FsReadBytes { .. },
            ..
        }
    ));
    let save = program.functions.iter().find(|f| f.name == "save").unwrap();
    assert!(matches!(
        save.body[0],
        Statement::Return(Some(ValueExpr::FsWriteString { .. }))
    ));
    let save_bytes = program
        .functions
        .iter()
        .find(|f| f.name == "save_bytes")
        .unwrap();
    assert!(matches!(
        save_bytes.body[0],
        Statement::Return(Some(ValueExpr::FsWriteBytes { .. }))
    ));
}

#[test]
fn accepts_fs_open_and_file_close_defer() {
    let source = r#"package app.main

import std.fs
import std.io

fn close_and_label(file: File) -> string {
    defer file.close()
    return "ok"
}

fn main() -> void {
    let result: Result<File, FsError> = fs.open("/tmp/nomo-file.txt")
    let message: string = match result {
        Result.Ok(file) => close_and_label(file)
        Result.Err(err) => err.message
    }
    io.println(message)
}
"#;

    let program = parse_inline(source).unwrap();
    assert!(program.structs.iter().any(|item| item.name == "File"));
    let close_and_label = program
        .functions
        .iter()
        .find(|f| f.name == "close_and_label")
        .unwrap();
    assert_eq!(
        close_and_label.params[0].value_type,
        ValueType::Struct("File".to_string(), Vec::new())
    );
    assert!(matches!(
        close_and_label.body[0],
        Statement::Defer {
            call: DeferredCall::Expr(ValueExpr::FileClose { .. })
        }
    ));
    let main = program.functions.iter().find(|f| f.name == "main").unwrap();
    assert!(matches!(
        main.body[0],
        Statement::Let {
            value_type: ValueType::Enum(ref name, ref args),
            initializer: ValueExpr::FsOpen { .. },
            ..
        } if name == "Result"
            && args == &vec![
                ValueType::Struct("File".to_string(), Vec::new()),
                ValueType::Struct("FsError".to_string(), Vec::new()),
            ]
    ));
}

#[test]
fn accepts_specific_fs_builtin_imports() {
    let source = r#"package app.main

import std.fs.read_to_string
import std.fs.write_string
import std.fs.read_bytes
import std.fs.write_bytes
import std.io
import std.array.Array

fn load(path: string) -> Result<string, FsError> {
    let text: string = read_to_string(path)?
    return Result.Ok(text)
}

fn load_bytes(path: string) -> Result<Array<u32>, FsError> {
    let bytes: Array<u32> = read_bytes(path)?
    return Result.Ok(bytes)
}

fn save(path: string, content: string) -> Result<void, FsError> {
    return write_string(path, content)
}

fn save_bytes(path: string, bytes: Array<u32>) -> Result<void, FsError> {
    return write_bytes(path, bytes)
}

fn main() -> void {
    let write_result: Result<void, FsError> = save("/tmp/nomo-fs-test.txt", "hello")
    let read_result: Result<string, FsError> = load("/tmp/nomo-fs-test.txt")
    let byte_read_result: Result<Array<u32>, FsError> = load_bytes("/tmp/nomo-fs-test.txt")
    io.println("done")
}
"#;

    let program = parse_inline(source).unwrap();
    assert!(program.structs.iter().any(|item| item.name == "FsError"));
    assert!(program.enums.iter().any(|item| item.name == "Result"));
    let load = program.functions.iter().find(|f| f.name == "load").unwrap();
    assert!(matches!(
        load.body[0],
        Statement::QuestionLet {
            result_expr: ValueExpr::FsReadToString { .. },
            ..
        }
    ));
    let load_bytes = program
        .functions
        .iter()
        .find(|f| f.name == "load_bytes")
        .unwrap();
    assert!(matches!(
        load_bytes.body[0],
        Statement::QuestionLet {
            result_expr: ValueExpr::FsReadBytes { .. },
            ..
        }
    ));
    let save = program.functions.iter().find(|f| f.name == "save").unwrap();
    assert!(matches!(
        save.body[0],
        Statement::Return(Some(ValueExpr::FsWriteString { .. }))
    ));
    let save_bytes = program
        .functions
        .iter()
        .find(|f| f.name == "save_bytes")
        .unwrap();
    assert!(matches!(
        save_bytes.body[0],
        Statement::Return(Some(ValueExpr::FsWriteBytes { .. }))
    ));
}

#[test]
fn accepts_file_read_and_write_string_methods() {
    let source = r#"package app.main

import std.fs

fn rewrite(file: File) -> Result<string, FsError> {
    file.write_string("file ok")?
    let text: string = file.read_to_string()?
    file.close()
    return Ok(text)
}

fn main() -> void {
}
"#;

    let program = parse_inline(source).unwrap();
    let rewrite = program
        .functions
        .iter()
        .find(|f| f.name == "rewrite")
        .unwrap();
    assert!(matches!(
        rewrite.body[0],
        Statement::QuestionLet {
            result_expr: ValueExpr::FileWriteString { .. },
            ..
        }
    ));
    assert!(rewrite.body.iter().any(|stmt| matches!(
        stmt,
        Statement::QuestionLet {
            result_expr: ValueExpr::FileReadToString { .. },
            ..
        }
    )));
    assert!(
        rewrite
            .body
            .iter()
            .any(|stmt| matches!(stmt, Statement::Expr(ValueExpr::FileClose { .. })))
    );
}

#[test]
fn accepts_net_tcp_stream_builtins() {
    let source = r#"package app.main

import std.net

fn request(host: string, port: i64) -> Result<string, NetError> {
    let stream: TcpStream = net.connect(host, port)?
    stream.write_string("ping")?
    let text: string = stream.read_to_string()?
    stream.close()
    return Ok(text)
}

fn main() -> void {
}
"#;

    let program = parse_inline(source).unwrap();
    assert!(program.structs.iter().any(|item| item.name == "NetError"));
    assert!(program.structs.iter().any(|item| item.name == "TcpStream"));
    let request = program
        .functions
        .iter()
        .find(|f| f.name == "request")
        .unwrap();
    assert!(matches!(
        request.body[0],
        Statement::QuestionLet {
            value_type: ValueType::Struct(ref name, ref args),
            result_expr: ValueExpr::NetConnect { .. },
            ..
        } if name == "TcpStream" && args.is_empty()
    ));
    assert!(request.body.iter().any(|stmt| matches!(
        stmt,
        Statement::QuestionLet {
            value_type: ValueType::Void,
            result_expr: ValueExpr::TcpStreamWriteString { .. },
            ..
        }
    )));
    assert!(request.body.iter().any(|stmt| matches!(
        stmt,
        Statement::QuestionLet {
            value_type: ValueType::String,
            result_expr: ValueExpr::TcpStreamReadToString { .. },
            ..
        }
    )));
    assert!(
        request
            .body
            .iter()
            .any(|stmt| matches!(stmt, Statement::Expr(ValueExpr::TcpStreamClose { .. })))
    );
}

#[test]
fn accepts_specific_net_connect_import() {
    let source = r#"package app.main

import std.net.connect
import std.result

fn request(host: string, port: i64) -> Result<TcpStream, NetError> {
    return connect(host, port)
}

fn main() -> void {
}
"#;

    let program = parse_inline(source).unwrap();
    let request = program
        .functions
        .iter()
        .find(|f| f.name == "request")
        .unwrap();
    assert!(matches!(
        request.body[0],
        Statement::Return(Some(ValueExpr::NetConnect { .. }))
    ));
}

#[test]
fn accepts_net_tcp_listener_builtins() {
    let source = r#"package app.main

import std.net

fn serve(host: string, port: i64) -> Result<void, NetError> {
    let listener: TcpListener = net.listen(host, port)?
    let stream: TcpStream = listener.accept()?
    stream.write_string("pong")?
    stream.close()
    listener.close()
    return Ok(void)
}

fn main() -> void {
}
"#;

    let program = parse_inline(source).unwrap();
    assert!(
        program
            .structs
            .iter()
            .any(|item| item.name == "TcpListener")
    );
    let serve = program
        .functions
        .iter()
        .find(|f| f.name == "serve")
        .unwrap();
    assert!(matches!(
        serve.body[0],
        Statement::QuestionLet {
            value_type: ValueType::Struct(ref name, ref args),
            result_expr: ValueExpr::NetListen { .. },
            ..
        } if name == "TcpListener" && args.is_empty()
    ));
    assert!(serve.body.iter().any(|stmt| matches!(
        stmt,
        Statement::QuestionLet {
            value_type: ValueType::Struct(name, args),
            result_expr: ValueExpr::TcpListenerAccept { .. },
            ..
        } if name == "TcpStream" && args.is_empty()
    )));
    assert!(
        serve
            .body
            .iter()
            .any(|stmt| matches!(stmt, Statement::Expr(ValueExpr::TcpListenerClose { .. })))
    );
}

#[test]
fn accepts_specific_net_listen_import() {
    let source = r#"package app.main

import std.net.listen
import std.result

fn open(host: string, port: i64) -> Result<TcpListener, NetError> {
    return listen(host, port)
}

fn main() -> void {
}
"#;

    let program = parse_inline(source).unwrap();
    let open = program.functions.iter().find(|f| f.name == "open").unwrap();
    assert!(matches!(
        open.body[0],
        Statement::Return(Some(ValueExpr::NetListen { .. }))
    ));
}

#[test]
fn accepts_net_udp_socket_builtins() {
    let source = r#"package app.main

import std.net

fn serve(host: string, port: i64) -> Result<void, NetError> {
    let socket: UdpSocket = net.udp_bind(host, port)?
    let packet: UdpDatagram = socket.recv_from_string(1024)?
    socket.send_to_string(packet.data, packet.host, packet.port)?
    socket.close()
    return Ok(void)
}

fn main() -> void {
}
"#;

    let program = parse_inline(source).unwrap();
    assert!(program.structs.iter().any(|item| item.name == "UdpSocket"));
    assert!(
        program
            .structs
            .iter()
            .any(|item| item.name == "UdpDatagram")
    );
    let serve = program
        .functions
        .iter()
        .find(|f| f.name == "serve")
        .unwrap();
    assert!(matches!(
        serve.body[0],
        Statement::QuestionLet {
            value_type: ValueType::Struct(ref name, ref args),
            result_expr: ValueExpr::NetUdpBind { .. },
            ..
        } if name == "UdpSocket" && args.is_empty()
    ));
    assert!(serve.body.iter().any(|stmt| matches!(
        stmt,
        Statement::QuestionLet {
            value_type: ValueType::Struct(name, args),
            result_expr: ValueExpr::UdpSocketRecvFromString { .. },
            ..
        } if name == "UdpDatagram" && args.is_empty()
    )));
    assert!(serve.body.iter().any(|stmt| matches!(
        stmt,
        Statement::QuestionLet {
            value_type: ValueType::Void,
            result_expr: ValueExpr::UdpSocketSendToString { .. },
            ..
        }
    )));
    assert!(
        serve
            .body
            .iter()
            .any(|stmt| matches!(stmt, Statement::Expr(ValueExpr::UdpSocketClose { .. })))
    );
}

#[test]
fn accepts_specific_net_udp_bind_import() {
    let source = r#"package app.main

import std.net.udp_bind
import std.result

fn open(host: string, port: i64) -> Result<UdpSocket, NetError> {
    return udp_bind(host, port)
}

fn main() -> void {
}
"#;

    let program = parse_inline(source).unwrap();
    let open = program.functions.iter().find(|f| f.name == "open").unwrap();
    assert!(matches!(
        open.body[0],
        Statement::Return(Some(ValueExpr::NetUdpBind { .. }))
    ));
}

#[test]
fn accepts_fs_directory_builtins() {
    let source = r#"package app.main

import std.fs
import std.array
import std.io

fn prepare(path: string) -> Result<Array<string>, FsError> {
    let present: bool = fs.exists(path)
    let metadata: FileMetadata = fs.metadata(path)?
    fs.create_dir(path)?
    let entries: Array<string> = fs.read_dir(path)?
    fs.remove_dir(path)?
    return Ok(entries)
}

fn main() -> void {
    let entries: Result<Array<string>, FsError> = prepare("/tmp/nomo-dir")
    io.println("done")
}
"#;

    let program = parse_inline(source).unwrap();
    assert!(program.structs.iter().any(|item| item.name == "FsError"));
    assert!(
        program
            .structs
            .iter()
            .any(|item| item.name == "FileMetadata")
    );
    assert!(program.enums.iter().any(|item| item.name == "Result"));
    let prepare = program
        .functions
        .iter()
        .find(|f| f.name == "prepare")
        .unwrap();
    assert_eq!(
        prepare.return_type,
        ValueType::Enum(
            "Result".to_string(),
            vec![
                ValueType::Array(Box::new(ValueType::String)),
                ValueType::Struct("FsError".to_string(), Vec::new()),
            ],
        )
    );
    assert!(matches!(
        prepare.body[1],
        Statement::QuestionLet {
            result_expr: ValueExpr::FsMetadata { .. },
            ..
        }
    ));
    assert!(matches!(
        prepare.body[0],
        Statement::Let {
            initializer: ValueExpr::FsExists { .. },
            ..
        }
    ));
}

#[test]
fn accepts_specific_fs_directory_imports() {
    let source = r#"package app.main

import std.fs.exists
import std.fs.metadata
import std.fs.create_dir
import std.fs.remove_dir
import std.fs.read_dir
import std.array

fn prepare(path: string) -> Result<Array<string>, FsError> {
    let present: bool = exists(path)
    let metadata: FileMetadata = metadata(path)?
    create_dir(path)?
    let entries: Array<string> = read_dir(path)?
    remove_dir(path)?
    return Ok(entries)
}

fn main() -> void {
}
"#;

    let program = parse_inline(source).unwrap();
    let prepare = program
        .functions
        .iter()
        .find(|f| f.name == "prepare")
        .unwrap();
    assert!(matches!(
        prepare.body[0],
        Statement::Let {
            initializer: ValueExpr::FsExists { .. },
            ..
        }
    ));
    assert!(matches!(
        prepare.body[1],
        Statement::QuestionLet {
            result_expr: ValueExpr::FsMetadata { .. },
            ..
        }
    ));
}

#[test]
fn accepts_env_get_builtin() {
    let source = r#"package app.main

import std.env
import std.io

fn main() -> void {
    let value: Option<string> = env.get("NOMO_TEST_ENV")
    let message: string = match value {
        Option.Some(text) => text
        Option.None => "missing"
    }
    io.println(message)
}
"#;

    let program = parse_inline(source).unwrap();
    assert!(program.enums.iter().any(|item| item.name == "Option"));
    let main = program.functions.iter().find(|f| f.name == "main").unwrap();
    assert!(matches!(
        main.body[0],
        Statement::Let {
            value_type: ValueType::Enum(ref name, ref args),
            initializer: ValueExpr::EnvGet { .. },
            ..
        } if name == "Option" && args == &vec![ValueType::String]
    ));
}

#[test]
fn accepts_env_args_builtin() {
    let source = r#"package app.main

import std.env
import std.io
import std.array

fn main() -> void {
    let args: Array<string> = env.args()
    let first: Option<string> = args.get(1)
    let message: string = match first {
        Option.Some(text) => text
        Option.None => "missing"
    }
    io.println(message)
}
"#;

    let program = parse_inline(source).unwrap();
    assert!(program.enums.iter().any(|item| item.name == "Option"));
    let main = program.functions.iter().find(|f| f.name == "main").unwrap();
    assert!(matches!(
        main.body[0],
        Statement::Let {
            value_type: ValueType::Array(ref element),
            initializer: ValueExpr::EnvArgs,
            ..
        } if element.as_ref() == &ValueType::String
    ));
    assert!(matches!(
        main.body[1],
        Statement::Let {
            value_type: ValueType::Enum(ref name, ref args),
            initializer: ValueExpr::ArrayGet {
                element_type: ValueType::String,
                ..
            },
            ..
        } if name == "Option" && args == &vec![ValueType::String]
    ));
}

#[test]
fn accepts_extended_env_builtins() {
    let source = r#"package app.main

import std.env
import std.io

fn main() -> void {
    env.set("NOMO_TEST_ENV", "value")
    let cwd: string = env.cwd()
    let home: Option<string> = env.home_dir()
    let temp: string = env.temp_dir()
    io.println(cwd)
}
"#;

    let program = parse_inline(source).unwrap();
    assert!(program.enums.iter().any(|item| item.name == "Option"));
    let main = program.functions.iter().find(|f| f.name == "main").unwrap();
    assert!(matches!(
        main.body[0],
        Statement::Expr(ValueExpr::EnvSet { .. })
    ));
    assert!(matches!(
        main.body[1],
        Statement::Let {
            value_type: ValueType::String,
            initializer: ValueExpr::EnvCwd,
            ..
        }
    ));
    assert!(matches!(
        main.body[2],
        Statement::Let {
            value_type: ValueType::Enum(ref name, ref args),
            initializer: ValueExpr::EnvHomeDir,
            ..
        } if name == "Option" && args == &vec![ValueType::String]
    ));
    assert!(matches!(
        main.body[3],
        Statement::Let {
            value_type: ValueType::String,
            initializer: ValueExpr::EnvTempDir,
            ..
        }
    ));
}

#[test]
fn accepts_specific_env_builtin_imports() {
    let source = r#"package app.main

import std.env.args
import std.env.cwd
import std.env.get
import std.env.home_dir
import std.env.set
import std.env.temp_dir
import std.io
import std.array

fn main() -> void {
    set("NOMO_TEST_ENV", "value")
    let values: Array<string> = args()
    let home: Option<string> = get("HOME")
    let cwd_path: string = cwd()
    let maybe_home: Option<string> = home_dir()
    let temp_path: string = temp_dir()
    let message: string = match home {
        Option.Some(text) => text
        Option.None => "missing"
    }
    io.println(message)
}
"#;

    let program = parse_inline(source).unwrap();
    assert!(program.enums.iter().any(|item| item.name == "Option"));
    let main = program.functions.iter().find(|f| f.name == "main").unwrap();
    assert!(matches!(
        main.body[0],
        Statement::Expr(ValueExpr::EnvSet { .. })
    ));
    assert!(matches!(
        main.body[1],
        Statement::Let {
            value_type: ValueType::Array(ref element),
            initializer: ValueExpr::EnvArgs,
            ..
        } if element.as_ref() == &ValueType::String
    ));
    assert!(matches!(
        main.body[2],
        Statement::Let {
            value_type: ValueType::Enum(ref name, ref args),
            initializer: ValueExpr::EnvGet { .. },
            ..
        } if name == "Option" && args == &vec![ValueType::String]
    ));
    assert!(matches!(
        main.body[3],
        Statement::Let {
            value_type: ValueType::String,
            initializer: ValueExpr::EnvCwd,
            ..
        }
    ));
    assert!(matches!(
        main.body[4],
        Statement::Let {
            value_type: ValueType::Enum(ref name, ref args),
            initializer: ValueExpr::EnvHomeDir,
            ..
        } if name == "Option" && args == &vec![ValueType::String]
    ));
    assert!(matches!(
        main.body[5],
        Statement::Let {
            value_type: ValueType::String,
            initializer: ValueExpr::EnvTempDir,
            ..
        }
    ));
}

#[test]
fn accepts_imported_result_lang_item() {
    let source = r#"package app.main

import std.result

fn parse() -> Result<i64, string> {
    return Result.Ok(41)
}

fn main() -> void {
    let value: Result<i64, string> = parse()
}
"#;

    let program = parse_inline(source).unwrap();
    assert!(program.enums.iter().any(|item| item.name == "Result"));
    let parse = program
        .functions
        .iter()
        .find(|f| f.name == "parse")
        .unwrap();
    assert_eq!(
        parse.return_type,
        ValueType::Enum(
            "Result".to_string(),
            vec![ValueType::Int, ValueType::String],
        )
    );
    assert!(matches!(
        parse.body[0],
        Statement::Return(Some(ValueExpr::EnumVariant {
            ref enum_name,
            ref variant,
            ..
        })) if enum_name == "Result" && variant == "Ok"
    ));
}

#[test]
fn accepts_imported_option_lang_item() {
    let source = r#"package app.main

import std.option
import std.io

fn label(value: Option<string>) -> string {
    return match value {
        Option.Some(text) => text
        Option.None => "missing"
    }
}

fn main() -> void {
    let value: Option<string> = Option.None
    let text: string = label(value)
    io.println(text)
}
"#;

    let program = parse_inline(source).unwrap();
    assert!(program.enums.iter().any(|item| item.name == "Option"));
    let main = program.functions.iter().find(|f| f.name == "main").unwrap();
    assert!(matches!(
        main.body[0],
        Statement::Let {
            value_type: ValueType::Enum(ref name, ref args),
            initializer: ValueExpr::EnumVariant {
                ref enum_name,
                ref variant,
                ..
            },
            ..
        } if name == "Option"
            && args == &vec![ValueType::String]
            && enum_name == "Option"
            && variant == "None"
    ));
}
