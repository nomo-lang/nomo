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
