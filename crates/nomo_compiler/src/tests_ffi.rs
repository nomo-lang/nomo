use super::*;

#[test]
fn accepts_cstring_and_opaque_extern_boundaries() {
    let source = r#"package app.main

import std.ffi

extern "C" {
    fn puts(message: CString) -> i32
    fn nomo_example_allocate() -> Opaque
    fn nomo_example_release(handle: Opaque) -> void
}

fn allocate() -> Opaque {
    unsafe {
        return nomo_example_allocate()
    }
}

fn release(handle: Opaque) -> void {
    unsafe {
        nomo_example_release(handle)
    }
}

fn main() -> void {
    let message: CString = CString.from_string("ffi values ok")
    unsafe {
        puts(message)
    }
    let handle: Opaque = allocate()
    release(handle)
}
"#;

    let program = parse_inline(source).unwrap();

    assert!(program.extern_functions.iter().any(|function| {
        function.symbol == "puts"
            && function.params == [ValueType::CString]
            && function.return_type == ValueType::I32
    }));
    assert!(program.extern_functions.iter().any(|function| {
        function.symbol == "nomo_example_allocate"
            && function.params.is_empty()
            && function.return_type == ValueType::Opaque
    }));
    let main = program
        .functions
        .iter()
        .find(|function| function.name == "main")
        .unwrap();
    assert!(matches!(
        &main.body[0],
        Statement::Let {
            value_type: ValueType::CString,
            initializer: ValueExpr::Call { name, .. },
            ..
        } if name == BUILTIN_CSTRING_FROM_STRING_EXPR
    ));
    assert!(matches!(
        &main.body[1],
        Statement::Expr(ValueExpr::Call { name, args })
            if name == "__nomo_extern::puts"
                && matches!(
                    args.as_slice(),
                    [ValueExpr::Call { name, .. }] if name == BUILTIN_CSTRING_DATA_EXPR
                )
    ));
}

#[test]
fn accepts_nominal_opaque_handle_boundaries() {
    let source = r#"package app.main

extern opaque type FileHandle

extern "C" {
    fn file_open() -> FileHandle
    fn file_close(handle: FileHandle) -> void
}

fn open() -> FileHandle {
    unsafe {
        return file_open()
    }
}

fn close(handle: FileHandle) -> void {
    unsafe {
        file_close(handle)
    }
}

fn main() -> void {
    let handle: FileHandle = open()
    close(handle)
}
"#;

    let program = parse_inline(source).unwrap();
    assert!(program.extern_functions.iter().any(|function| {
        function.symbol == "file_open"
            && function.return_type == ValueType::OpaqueHandle("FileHandle".to_string())
    }));
    assert!(program.extern_functions.iter().any(|function| {
        function.symbol == "file_close"
            && function.params == [ValueType::OpaqueHandle("FileHandle".to_string())]
    }));
}

#[test]
fn rejects_mixing_nominal_opaque_handles() {
    let source = r#"package app.main

extern opaque type FileHandle
extern opaque type SocketHandle

extern "C" {
    fn file_open() -> FileHandle
    fn socket_close(handle: SocketHandle) -> void
}

fn open() -> FileHandle {
    unsafe {
        return file_open()
    }
}

fn close_socket(handle: SocketHandle) -> void {
    unsafe {
        socket_close(handle)
    }
}

fn main() -> void {
    let handle: FileHandle = open()
    close_socket(handle)
}
"#;

    let error = parse_inline(source).unwrap_err();
    assert_eq!(error.code, "E0404");
    assert!(error.message.contains("SocketHandle"));
    assert!(error.message.contains("FileHandle"));
}

#[test]
fn rejects_constructing_nominal_opaque_handles() {
    let source = r#"package app.main

extern opaque type FileHandle

fn main() -> void {
    let handle: FileHandle = FileHandle {}
}
"#;

    let error = parse_inline(source).unwrap_err();
    assert_eq!(error.code, "E1522");
    assert!(error.message.contains("cannot be constructed"));
}

#[test]
fn rejects_duplicate_nominal_opaque_handle_types() {
    let source = r#"package app.main

extern opaque type FileHandle
extern opaque type FileHandle

fn main() -> void {
}
"#;

    let error = parse_inline(source).unwrap_err();
    assert_eq!(error.code, "E1521");
    assert!(error.message.contains("already defined"));
}

#[test]
fn accepts_nullable_handle_checks_and_explicit_unwrap() {
    let source = r#"package app.main

extern opaque type FileHandle

extern "C" {
    fn file_try_open() -> Nullable<FileHandle>
    fn file_close(handle: FileHandle) -> void
}

fn try_open() -> Nullable<FileHandle> {
    unsafe {
        return file_try_open()
    }
}

fn close(handle: FileHandle) -> void {
    unsafe {
        file_close(handle)
    }
}

fn main() -> void {
    let maybe: Nullable<FileHandle> = try_open()
    let handle: FileHandle = if maybe.is_null() {
        panic("missing handle")
    } else {
        maybe.unwrap()
    }
    close(handle)
    let empty: Nullable<FileHandle> = Nullable.none<FileHandle>()
}
"#;

    parse_inline(source).unwrap();
}

#[test]
fn rejects_using_nullable_handle_without_unwrap() {
    let source = r#"package app.main

extern opaque type FileHandle

extern "C" {
    fn file_try_open() -> Nullable<FileHandle>
    fn file_close(handle: FileHandle) -> void
}

fn try_open() -> Nullable<FileHandle> {
    unsafe {
        return file_try_open()
    }
}

fn close(handle: FileHandle) -> void {
    unsafe {
        file_close(handle)
    }
}

fn main() -> void {
    let maybe: Nullable<FileHandle> = try_open()
    close(maybe)
}
"#;

    let error = parse_inline(source).unwrap_err();
    assert_eq!(error.code, "E0404");
    assert!(error.message.contains("FileHandle"));
    assert!(error.message.contains("Nullable"));
}

#[test]
fn rejects_nullable_non_handle_types() {
    let source = r#"package app.main

extern "C" {
    fn invalid() -> Nullable<i32>
}

fn main() -> void {
}
"#;

    let error = parse_inline(source).unwrap_err();
    assert_eq!(error.code, "E0403");
    assert!(error.message.contains("unsupported extern return type"));
}

#[test]
fn accepts_owned_and_borrowed_handle_metadata_with_release_contract() {
    let source = r#"package app.main

extern opaque type FileHandle release file_close

extern "C" {
    fn file_open() -> Owned<FileHandle>
    fn file_marker(handle: Borrowed<FileHandle>) -> i32
    fn file_close(handle: Owned<FileHandle>) -> void
}

fn open() -> Owned<FileHandle> {
    unsafe {
        return file_open()
    }
}

fn marker(handle: Borrowed<FileHandle>) -> i32 {
    unsafe {
        return file_marker(handle)
    }
}

fn close(handle: Owned<FileHandle>) -> void {
    unsafe {
        file_close(handle)
    }
}

fn main() -> void {
    let handle: Owned<FileHandle> = open()
    let value: i32 = marker(handle.borrow())
    close(handle)
}
"#;

    parse_inline(source).unwrap();
}

#[test]
fn rejects_invalid_opaque_handle_release_contract() {
    let source = r#"package app.main

extern opaque type FileHandle release file_close

extern "C" {
    fn file_close(handle: FileHandle) -> void
}

fn main() -> void {
}
"#;

    let error = parse_inline(source).unwrap_err();
    assert_eq!(error.code, "E1523");
    assert!(error.message.contains("Owned<FileHandle>"));
}

#[test]
fn rejects_borrowed_handle_at_owned_release_boundary() {
    let source = r#"package app.main

extern opaque type FileHandle release file_close

extern "C" {
    fn file_open() -> Owned<FileHandle>
    fn file_close(handle: Owned<FileHandle>) -> void
}

fn open() -> Owned<FileHandle> {
    unsafe {
        return file_open()
    }
}

fn close(handle: Owned<FileHandle>) -> void {
    unsafe {
        file_close(handle)
    }
}

fn main() -> void {
    let handle: Owned<FileHandle> = open()
    close(handle.borrow())
}
"#;

    let error = parse_inline(source).unwrap_err();
    assert_eq!(error.code, "E0404");
}

#[test]
fn accepts_non_capturing_extern_c_callback() {
    let source = r#"package app.main

extern "C" {
    fn apply(value: i32, callback: extern "C" fn(i32) -> i32) -> i32
}

fn double(value: i32) -> i32 {
    return value * 2
}

fn call_apply(value: i32) -> i32 {
    unsafe {
        return apply(value, double)
    }
}

fn main() -> void {
    let result: i32 = call_apply(21)
}
"#;

    let program = parse_inline(source).unwrap();
    let apply = program
        .extern_functions
        .iter()
        .find(|function| function.symbol == "apply")
        .unwrap();
    assert!(matches!(
        &apply.params[1],
        ValueType::ExternCallback { params, return_type }
            if params == &[ValueType::I32] && return_type.as_ref() == &ValueType::I32
    ));
}

#[test]
fn accepts_fixed_layout_repr_c_struct_at_extern_boundary() {
    let source = r#"package app.main

#[repr(C)]
struct Header {
    tag: i32
    value: u64
}

extern "C" {
    fn inspect_header(header: Header) -> i32
}

fn inspect(header: Header) -> i32 {
    unsafe {
        return inspect_header(header)
    }
}

fn main() -> void {
    let header: Header = Header {
        tag: 1,
        value: 42,
    }
    let result: i32 = inspect(header)
}
"#;

    parse_inline(source).unwrap();
}

#[test]
fn rejects_non_repr_struct_at_extern_boundary() {
    let source = r#"package app.main

struct Header {
    tag: i32
}

extern "C" {
    fn inspect_header(header: Header) -> i32
}

fn main() -> void {
}
"#;

    let error = parse_inline(source).unwrap_err();
    assert_eq!(error.code, "E1519");
    assert!(error.message.contains("Header"));
}

#[test]
fn rejects_non_fixed_repr_c_field_type() {
    let source = r#"package app.main

#[repr(C)]
struct Header {
    label: string
}

fn main() -> void {
}
"#;

    let error = parse_inline(source).unwrap_err();
    assert_eq!(error.code, "E1530");
    assert!(error.message.contains("non-ABI-safe"));
}

#[test]
fn rejects_callback_with_mismatched_signature() {
    let source = r#"package app.main

extern "C" {
    fn apply(value: i32, callback: extern "C" fn(i32) -> i32) -> i32
}

fn wrong(value: i64) -> i32 {
    return 0
}

fn call_apply(value: i32) -> i32 {
    unsafe {
        return apply(value, wrong)
    }
}

fn main() -> void {
}
"#;

    let error = parse_inline(source).unwrap_err();
    assert_eq!(error.code, "E1525");
    assert!(error.message.contains("does not match"));
}

#[test]
fn rejects_callback_type_escaping_extern_parameter_position() {
    let source = r#"package app.main

fn retain(callback: extern "C" fn(i32) -> i32) -> void {
}

fn main() -> void {
}
"#;

    let error = parse_inline(source).unwrap_err();
    assert_eq!(error.code, "E0404");
    assert!(error.message.contains("may only appear"));
}

#[test]
fn rejects_callback_as_extern_return_type() {
    let source = r#"package app.main

extern "C" {
    fn callback_factory() -> extern "C" fn(i32) -> i32
}

fn main() -> void {
}
"#;

    let error = parse_inline(source).unwrap_err();
    assert_eq!(error.code, "E1525");
    assert!(error.message.contains("only be passed"));
}

#[test]
fn rejects_ffi_types_without_std_ffi_import() {
    let source = r#"package app.main

extern "C" {
    fn puts(message: CString) -> i32
}

fn main() -> void {
}
"#;

    let error = parse_inline(source).unwrap_err();

    assert_eq!(error.code, "E0301");
    assert!(
        error
            .message
            .contains("`CString` requires `import std.ffi`")
    );
}

#[test]
fn rejects_cstring_extern_return_type() {
    let source = r#"package app.main

import std.ffi

extern "C" {
    fn current_name() -> CString
}

fn main() -> void {
}
"#;

    let error = parse_inline(source).unwrap_err();

    assert_eq!(error.code, "E1519");
    assert!(error.message.contains("CString cannot be returned by C"));
}

#[test]
fn rejects_string_as_general_extern_parameter() {
    let source = r#"package app.main

extern "C" {
    fn consume(message: string) -> void
}

fn main() -> void {
}
"#;

    let error = parse_inline(source).unwrap_err();

    assert_eq!(error.code, "E1519");
    assert!(
        error
            .message
            .contains("parameter type `string` is not supported")
    );
}
