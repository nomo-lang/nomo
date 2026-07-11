#![forbid(unsafe_code)]

use nomo_syntax::ast::{SourceFile, TypeRef};
use nomo_syntax::lexer::lex;
use nomo_syntax::parser::parse;
use serde::Deserialize;
use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

pub const PACKAGE_ID: &str = "nomo-lang/std";
pub const IMPORT_ROOT: &str = "std";
pub const INTRINSIC_MANIFEST_SCHEMA: u32 = 1;
pub const INTRINSIC_MANIFEST_SOURCE: &str = include_str!("../intrinsics.toml");
const OPTION_SOURCE: &str = include_str!("option.nomo");
const RESULT_SOURCE: &str = include_str!("result.nomo");
const ARRAY_SOURCE: &str = include_str!("array.nomo");
const STRING_SOURCE: &str = include_str!("string.nomo");

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct IntrinsicManifest {
    pub schema: u32,
    pub package: String,
    #[serde(rename = "binding")]
    pub bindings: Vec<IntrinsicBinding>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct IntrinsicBinding {
    pub id: String,
    pub module: String,
    pub declaration: String,
    pub kind: String,
    pub abi: String,
    pub source: String,
    pub required: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StandardModule {
    pub path: &'static str,
    pub docs: &'static str,
    pub items: &'static [&'static str],
    pub doc_items: &'static [StandardDocItem],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StandardDocItem {
    pub kind: &'static str,
    pub name: &'static str,
    pub signature: &'static str,
    pub docs: &'static str,
}

const ARRAY_ITEMS: &[&str] = &[
    "Array", "clear", "get", "insert", "iter", "len", "new", "pop", "push", "remove", "set",
];
const CHAR_ITEMS: &[&str] = &["is_alpha", "is_digit", "is_whitespace", "to_string"];
const COLLECTIONS_ITEMS: &[&str] = &[
    "StringMap",
    "StringSet",
    "map_contains",
    "map_get",
    "map_len",
    "map_new",
    "map_remove",
    "map_set",
    "set_contains",
    "set_insert",
    "set_len",
    "set_new",
    "set_remove",
];
const CRYPTO_ITEMS: &[&str] = &["random_bytes", "sha256", "sha512"];
const DEBUG_ITEMS: &[&str] = &["backtrace", "panic", "print", "println"];
const ENV_ITEMS: &[&str] = &["args", "cwd", "get", "home_dir", "set", "temp_dir"];
const FFI_ITEMS: &[&str] = &["CString", "Opaque"];
const FS_ITEMS: &[&str] = &[
    "File",
    "FileMetadata",
    "FsError",
    "create_dir",
    "exists",
    "metadata",
    "open",
    "read_bytes",
    "read_dir",
    "read_to_string",
    "remove_dir",
    "write_bytes",
    "write_string",
];
const HASH_ITEMS: &[&str] = &[
    "HashState",
    "bytes",
    "finish",
    "new",
    "string",
    "write_bytes",
    "write_string",
];
const HTTP_ITEMS: &[&str] = &[
    "HttpError",
    "HttpExchange",
    "HttpResponse",
    "HttpServer",
    "accept",
    "close_exchange",
    "close_server",
    "get",
    "listen",
    "post",
    "respond_string",
];
const IO_ITEMS: &[&str] = &["eprint", "eprintln", "print", "println", "read_line"];
const JSON_ITEMS: &[&str] = &["JsonError", "JsonValue", "parse", "stringify"];
const LOG_ITEMS: &[&str] = &["debug", "enabled", "error", "info", "warn"];
const MATH_ITEMS: &[&str] = &[
    "abs", "ceil", "cos", "floor", "max", "min", "pow", "round", "sin", "sqrt",
];
const NET_ITEMS: &[&str] = &[
    "NetError",
    "TcpListener",
    "TcpStream",
    "UdpDatagram",
    "UdpSocket",
    "connect",
    "listen",
    "udp_bind",
];
const NUM_ITEMS: &[&str] = &[
    "NumError",
    "checked_add",
    "checked_mul",
    "checked_sub",
    "parse_f64",
    "parse_i64",
    "parse_u64",
    "to_string",
    "wrapping_add",
    "wrapping_mul",
    "wrapping_sub",
];
const OPTION_ITEMS: &[&str] = &[
    "Option",
    "and_then",
    "is_none",
    "is_some",
    "map",
    "unwrap_or",
];
const OS_ITEMS: &[&str] = &["arch", "line_ending", "path_separator", "platform"];
const PATH_ITEMS: &[&str] = &[
    "basename",
    "dirname",
    "extension",
    "is_absolute",
    "join",
    "normalize",
];
const PROCESS_ITEMS: &[&str] = &[
    "ProcessError",
    "ProcessOutput",
    "exec",
    "exit",
    "output",
    "spawn",
    "status",
];
const REGEX_ITEMS: &[&str] = &["Regex", "RegexError", "captures", "compile", "is_match"];
const RESULT_ITEMS: &[&str] = &[
    "Result",
    "and_then",
    "is_err",
    "is_ok",
    "map",
    "map_err",
    "unwrap_or",
];
const STRING_ITEMS: &[&str] = &[
    "concat",
    "contains",
    "ends_with",
    "is_empty",
    "len",
    "split",
    "starts_with",
    "to_lower",
    "to_upper",
    "trim",
];
const TESTING_ITEMS: &[&str] = &["assert", "assert_equal", "assert_error"];
const TIME_ITEMS: &[&str] = &[
    "Duration",
    "duration_as_millis",
    "duration_millis",
    "duration_seconds",
    "format_duration",
    "monotonic_millis",
    "now_millis",
    "sleep",
    "sleep_millis",
];

const FFI_DOC_ITEMS: &[StandardDocItem] = &[
    StandardDocItem {
        kind: "type",
        name: "CString",
        signature: "pub type CString",
        docs: "Owned NUL-terminated string value passed to C as const char *.",
    },
    StandardDocItem {
        kind: "type",
        name: "Opaque",
        signature: "pub type Opaque",
        docs: "Uninspectable native handle represented as void * at C boundaries.",
    },
    StandardDocItem {
        kind: "function",
        name: "CString.from_string",
        signature: "pub fn CString.from_string(value: string) -> CString",
        docs: "Creates an owned C string copy from a Nomo string.",
    },
];

const OPTION_DOC_ITEMS: &[StandardDocItem] = &[
    StandardDocItem {
        kind: "enum",
        name: "Option",
        signature: "pub enum Option<T>",
        docs: "A value that may be present or absent.",
    },
    StandardDocItem {
        kind: "function",
        name: "is_some",
        signature: "pub fn is_some<T>(value: Option<T>) -> bool",
        docs: "Reports whether an option contains a value.",
    },
    StandardDocItem {
        kind: "function",
        name: "is_none",
        signature: "pub fn is_none<T>(value: Option<T>) -> bool",
        docs: "Reports whether an option is absent.",
    },
    StandardDocItem {
        kind: "function",
        name: "unwrap_or",
        signature: "pub fn unwrap_or<T>(value: Option<T>, fallback: T) -> T",
        docs: "Returns the contained value or a fallback.",
    },
];

const RESULT_DOC_ITEMS: &[StandardDocItem] = &[
    StandardDocItem {
        kind: "enum",
        name: "Result",
        signature: "pub enum Result<T, E>",
        docs: "A successful value or an error.",
    },
    StandardDocItem {
        kind: "function",
        name: "is_ok",
        signature: "pub fn is_ok<T, E>(value: Result<T, E>) -> bool",
        docs: "Reports whether a result is successful.",
    },
    StandardDocItem {
        kind: "function",
        name: "is_err",
        signature: "pub fn is_err<T, E>(value: Result<T, E>) -> bool",
        docs: "Reports whether a result contains an error.",
    },
    StandardDocItem {
        kind: "function",
        name: "unwrap_or",
        signature: "pub fn unwrap_or<T, E>(value: Result<T, E>, fallback: T) -> T",
        docs: "Returns the successful value or a fallback.",
    },
];

const ARRAY_DOC_ITEMS: &[StandardDocItem] = &[
    StandardDocItem {
        kind: "function",
        name: "new",
        signature: "pub fn new<T>() -> Array<T>",
        docs: "Creates an empty value-semantics array.",
    },
    StandardDocItem {
        kind: "function",
        name: "len",
        signature: "pub fn len<T>(self: Array<T>) -> u64",
        docs: "Returns the number of elements in an array.",
    },
    StandardDocItem {
        kind: "function",
        name: "push",
        signature: "pub fn push<T>(mut self: Array<T>, value: T) -> void",
        docs: "Appends an element to an array.",
    },
    StandardDocItem {
        kind: "function",
        name: "get",
        signature: "pub fn get<T>(self: Array<T>, index: u64) -> Option<T>",
        docs: "Returns an element when the index is in bounds.",
    },
    StandardDocItem {
        kind: "function",
        name: "set",
        signature: "pub fn set<T>(mut self: Array<T>, index: u64, value: T) -> void",
        docs: "Replaces an element at an index.",
    },
    StandardDocItem {
        kind: "function",
        name: "insert",
        signature: "pub fn insert<T>(mut self: Array<T>, index: u64, value: T) -> void",
        docs: "Inserts an element at an index.",
    },
    StandardDocItem {
        kind: "function",
        name: "pop",
        signature: "pub fn pop<T>(mut self: Array<T>) -> Option<T>",
        docs: "Removes and returns the final element.",
    },
    StandardDocItem {
        kind: "function",
        name: "remove",
        signature: "pub fn remove<T>(mut self: Array<T>, index: u64) -> Option<T>",
        docs: "Removes and returns an element at an index.",
    },
    StandardDocItem {
        kind: "function",
        name: "clear",
        signature: "pub fn clear<T>(mut self: Array<T>) -> void",
        docs: "Removes all elements from an array.",
    },
    StandardDocItem {
        kind: "function",
        name: "iter",
        signature: "pub fn iter<T>(self: Array<T>) -> Array<T>",
        docs: "Returns a value snapshot suitable for iteration.",
    },
];

const STRING_DOC_ITEMS: &[StandardDocItem] = &[
    StandardDocItem {
        kind: "function",
        name: "len",
        signature: "pub fn len(value: string) -> u64",
        docs: "Returns the UTF-8 byte length of a string.",
    },
    StandardDocItem {
        kind: "function",
        name: "concat",
        signature: "pub fn concat(value: string, other: string) -> string",
        docs: "Concatenates two strings.",
    },
    StandardDocItem {
        kind: "function",
        name: "is_empty",
        signature: "pub fn is_empty(value: string) -> bool",
        docs: "Reports whether a string has no bytes.",
    },
    StandardDocItem {
        kind: "function",
        name: "contains",
        signature: "pub fn contains(value: string, needle: string) -> bool",
        docs: "Reports whether a string contains another string.",
    },
    StandardDocItem {
        kind: "function",
        name: "starts_with",
        signature: "pub fn starts_with(value: string, prefix: string) -> bool",
        docs: "Reports whether a string starts with a prefix.",
    },
    StandardDocItem {
        kind: "function",
        name: "ends_with",
        signature: "pub fn ends_with(value: string, suffix: string) -> bool",
        docs: "Reports whether a string ends with a suffix.",
    },
    StandardDocItem {
        kind: "function",
        name: "split",
        signature: "pub fn split(value: string, separator: string) -> Array<string>",
        docs: "Splits a string by a non-empty separator.",
    },
    StandardDocItem {
        kind: "function",
        name: "trim",
        signature: "pub fn trim(value: string) -> string",
        docs: "Removes ASCII whitespace from both ends of a string.",
    },
    StandardDocItem {
        kind: "function",
        name: "to_lower",
        signature: "pub fn to_lower(value: string) -> string",
        docs: "Converts ASCII letters to lower case.",
    },
    StandardDocItem {
        kind: "function",
        name: "to_upper",
        signature: "pub fn to_upper(value: string) -> string",
        docs: "Converts ASCII letters to upper case.",
    },
];

pub const MODULES: &[StandardModule] = &[
    StandardModule {
        path: "std.array",
        docs: "Array helpers",
        items: ARRAY_ITEMS,
        doc_items: ARRAY_DOC_ITEMS,
    },
    StandardModule {
        path: "std.char",
        docs: "character helpers",
        items: CHAR_ITEMS,
        doc_items: &[],
    },
    StandardModule {
        path: "std.collections",
        docs: "string map and string set helpers",
        items: COLLECTIONS_ITEMS,
        doc_items: &[],
    },
    StandardModule {
        path: "std.crypto",
        docs: "cryptographic digest helpers",
        items: CRYPTO_ITEMS,
        doc_items: &[],
    },
    StandardModule {
        path: "std.debug",
        docs: "debug print and panic helpers",
        items: DEBUG_ITEMS,
        doc_items: &[],
    },
    StandardModule {
        path: "std.env",
        docs: "process environment helpers",
        items: ENV_ITEMS,
        doc_items: &[],
    },
    StandardModule {
        path: "std.ffi",
        docs: "C string and opaque native handle types",
        items: FFI_ITEMS,
        doc_items: FFI_DOC_ITEMS,
    },
    StandardModule {
        path: "std.fs",
        docs: "filesystem helpers",
        items: FS_ITEMS,
        doc_items: &[],
    },
    StandardModule {
        path: "std.hash",
        docs: "stable non-cryptographic hashing helpers",
        items: HASH_ITEMS,
        doc_items: &[],
    },
    StandardModule {
        path: "std.http",
        docs: "blocking plain-HTTP client and server helpers",
        items: HTTP_ITEMS,
        doc_items: &[],
    },
    StandardModule {
        path: "std.io",
        docs: "printing and terminal I/O",
        items: IO_ITEMS,
        doc_items: &[],
    },
    StandardModule {
        path: "std.json",
        docs: "JSON parse and stringify helpers",
        items: JSON_ITEMS,
        doc_items: &[],
    },
    StandardModule {
        path: "std.log",
        docs: "leveled logging helpers",
        items: LOG_ITEMS,
        doc_items: &[],
    },
    StandardModule {
        path: "std.math",
        docs: "numeric helpers",
        items: MATH_ITEMS,
        doc_items: &[],
    },
    StandardModule {
        path: "std.net",
        docs: "blocking TCP and UDP helpers",
        items: NET_ITEMS,
        doc_items: &[],
    },
    StandardModule {
        path: "std.num",
        docs: "numeric parsing and conversion helpers",
        items: NUM_ITEMS,
        doc_items: &[],
    },
    StandardModule {
        path: "std.option",
        docs: "Option carrier helpers",
        items: OPTION_ITEMS,
        doc_items: OPTION_DOC_ITEMS,
    },
    StandardModule {
        path: "std.os",
        docs: "target OS helpers",
        items: OS_ITEMS,
        doc_items: &[],
    },
    StandardModule {
        path: "std.path",
        docs: "path manipulation helpers",
        items: PATH_ITEMS,
        doc_items: &[],
    },
    StandardModule {
        path: "std.process",
        docs: "process helpers",
        items: PROCESS_ITEMS,
        doc_items: &[],
    },
    StandardModule {
        path: "std.regex",
        docs: "regular expression helpers",
        items: REGEX_ITEMS,
        doc_items: &[],
    },
    StandardModule {
        path: "std.result",
        docs: "Result carrier helpers",
        items: RESULT_ITEMS,
        doc_items: RESULT_DOC_ITEMS,
    },
    StandardModule {
        path: "std.string",
        docs: "string helpers",
        items: STRING_ITEMS,
        doc_items: STRING_DOC_ITEMS,
    },
    StandardModule {
        path: "std.testing",
        docs: "test assertion helpers",
        items: TESTING_ITEMS,
        doc_items: &[],
    },
    StandardModule {
        path: "std.time",
        docs: "clock and sleep helpers",
        items: TIME_ITEMS,
        doc_items: &[],
    },
];

const SOURCE_DEFINED_MODULES: &[&str] = &[
    "std.array",
    "std.char",
    "std.collections",
    "std.crypto",
    "std.debug",
    "std.env",
    "std.fs",
    "std.hash",
    "std.json",
    "std.io",
    "std.log",
    "std.math",
    "std.num",
    "std.option",
    "std.os",
    "std.path",
    "std.process",
    "std.regex",
    "std.result",
    "std.string",
    "std.testing",
    "std.time",
];

pub fn modules() -> &'static [StandardModule] {
    MODULES
}

pub fn module(path: &str) -> Option<&'static StandardModule> {
    MODULES.iter().find(|module| module.path == path)
}

pub fn intrinsic_manifest() -> Result<IntrinsicManifest, String> {
    parse_intrinsic_manifest(INTRINSIC_MANIFEST_SOURCE)
}

pub fn parse_intrinsic_manifest(source: &str) -> Result<IntrinsicManifest, String> {
    toml::from_str(source).map_err(|error| format!("failed to parse std intrinsics.toml: {error}"))
}

pub fn validate_intrinsic_manifest() -> Result<(), String> {
    validate_intrinsic_manifest_source(INTRINSIC_MANIFEST_SOURCE)?;
    validate_intrinsic_source_contract()?;
    validate_source_api_surface()
}

pub fn validate_intrinsic_manifest_source(source: &str) -> Result<(), String> {
    let manifest = parse_intrinsic_manifest(source)?;
    if manifest.schema != INTRINSIC_MANIFEST_SCHEMA {
        return Err(format!(
            "unsupported std intrinsic manifest schema {}; expected {}",
            manifest.schema, INTRINSIC_MANIFEST_SCHEMA
        ));
    }
    if manifest.package != PACKAGE_ID {
        return Err(format!(
            "std intrinsic manifest package `{}` does not match canonical package `{PACKAGE_ID}`",
            manifest.package
        ));
    }
    if manifest.bindings.is_empty() {
        return Err("std intrinsic manifest must declare at least one binding".to_string());
    }

    let mut ids = BTreeSet::new();
    let mut declarations = BTreeSet::new();
    let mut required = BTreeSet::new();
    for binding in &manifest.bindings {
        if binding.id.trim().is_empty()
            || binding.module.trim().is_empty()
            || binding.declaration.trim().is_empty()
            || binding.kind.trim().is_empty()
            || binding.abi.trim().is_empty()
            || binding.source.trim().is_empty()
        {
            return Err("std intrinsic bindings must have non-empty fields".to_string());
        }
        if !ids.insert(binding.id.as_str()) {
            return Err(format!(
                "duplicate std intrinsic binding id `{}`",
                binding.id
            ));
        }
        if !declarations.insert((binding.module.as_str(), binding.declaration.as_str())) {
            return Err(format!(
                "duplicate std intrinsic declaration `{}::{}`",
                binding.module, binding.declaration
            ));
        }
        let module = module(&binding.module).ok_or_else(|| {
            format!(
                "std intrinsic `{}` references unknown module `{}`",
                binding.id, binding.module
            )
        })?;
        let expected_source = format!("src/{}", module_source_relative_path(module).display());
        if binding.source != expected_source {
            return Err(format!(
                "std intrinsic `{}` source `{}` does not match `{expected_source}`",
                binding.id, binding.source
            ));
        }
        match binding.kind.as_str() {
            "carrier" | "ffi" | "layout" | "operator" | "runtime" => {}
            other => {
                return Err(format!(
                    "std intrinsic `{}` has unsupported binding kind `{other}`",
                    binding.id
                ));
            }
        }
        if binding.kind == "carrier"
            && !module.items.iter().any(|item| *item == binding.declaration)
        {
            return Err(format!(
                "std carrier `{}` is not present in module registry `{}`",
                binding.declaration, binding.module
            ));
        }
        if binding.kind == "operator" && binding.declaration != "?" {
            return Err(format!(
                "std operator binding `{}` must name `?`",
                binding.id
            ));
        }
        if binding.required {
            required.insert(binding.id.as_str());
        }
    }

    for (id, expected_module, expected_declaration, expected_kind, expected_abi) in [
        ("option", "std.option", "Option", "carrier", "enum-carrier"),
        ("result", "std.result", "Result", "carrier", "enum-carrier"),
        (
            "question",
            "std.option",
            "?",
            "operator",
            "carrier-propagation",
        ),
        ("array", "std.array", "Array", "layout", "array-header"),
        ("string", "std.string", "string", "layout", "string-header"),
    ] {
        let Some(binding) = manifest.bindings.iter().find(|binding| binding.id == id) else {
            return Err(format!(
                "std intrinsic manifest is missing required binding `{id}`"
            ));
        };
        if !required.contains(id) {
            return Err(format!(
                "std intrinsic binding `{id}` must be marked required"
            ));
        }
        if binding.module != expected_module
            || binding.declaration != expected_declaration
            || binding.kind != expected_kind
            || binding.abi != expected_abi
        {
            return Err(format!(
                "std intrinsic binding `{id}` does not match the required canonical identity"
            ));
        }
    }
    Ok(())
}

pub fn validate_intrinsic_source_contract() -> Result<(), String> {
    let option = parse_source_contract("std/src/option.nomo", OPTION_SOURCE)?;
    validate_package(&option, "std.option")?;
    validate_carrier(&option, "Option", 1, &[("Some", Some("T")), ("None", None)])?;
    validate_function(
        &option,
        "is_some",
        &["T"],
        &[("Option", &["T"][..])],
        ("bool", &[][..]),
    )?;
    validate_function(
        &option,
        "is_none",
        &["T"],
        &[("Option", &["T"][..])],
        ("bool", &[][..]),
    )?;
    validate_function(
        &option,
        "unwrap_or",
        &["T"],
        &[("Option", &["T"][..]), ("T", &[][..])],
        ("T", &[][..]),
    )?;

    let result = parse_source_contract("std/src/result.nomo", RESULT_SOURCE)?;
    validate_package(&result, "std.result")?;
    validate_carrier(
        &result,
        "Result",
        2,
        &[("Ok", Some("T")), ("Err", Some("E"))],
    )?;
    validate_function(
        &result,
        "is_ok",
        &["T", "E"],
        &[("Result", &["T", "E"][..])],
        ("bool", &[][..]),
    )?;
    validate_function(
        &result,
        "is_err",
        &["T", "E"],
        &[("Result", &["T", "E"][..])],
        ("bool", &[][..]),
    )?;
    validate_function(
        &result,
        "unwrap_or",
        &["T", "E"],
        &[("Result", &["T", "E"][..]), ("T", &[][..])],
        ("T", &[][..]),
    )?;

    let array = parse_source_contract("std/src/array.nomo", ARRAY_SOURCE)?;
    validate_package(&array, "std.array")?;
    validate_function(&array, "new", &["T"], &[], ("Array", &["T"][..]))?;
    validate_function(
        &array,
        "len",
        &["T"],
        &[("Array", &["T"][..])],
        ("u64", &[][..]),
    )?;
    validate_function(
        &array,
        "push",
        &["T"],
        &[("Array", &["T"][..]), ("T", &[][..])],
        ("void", &[][..]),
    )?;
    validate_function(
        &array,
        "get",
        &["T"],
        &[("Array", &["T"][..]), ("u64", &[][..])],
        ("Option", &["T"][..]),
    )?;
    for name in ["set", "insert"] {
        validate_function(
            &array,
            name,
            &["T"],
            &[("Array", &["T"][..]), ("u64", &[][..]), ("T", &[][..])],
            ("void", &[][..]),
        )?;
    }
    validate_function(
        &array,
        "pop",
        &["T"],
        &[("Array", &["T"][..])],
        ("Option", &["T"][..]),
    )?;
    validate_function(
        &array,
        "remove",
        &["T"],
        &[("Array", &["T"][..]), ("u64", &[][..])],
        ("Option", &["T"][..]),
    )?;
    validate_function(
        &array,
        "clear",
        &["T"],
        &[("Array", &["T"][..])],
        ("void", &[][..]),
    )?;
    validate_function(
        &array,
        "iter",
        &["T"],
        &[("Array", &["T"][..])],
        ("Array", &["T"][..]),
    )?;

    let string = parse_source_contract("std/src/string.nomo", STRING_SOURCE)?;
    validate_package(&string, "std.string")?;
    for (name, parameters, return_type) in [
        ("len", vec![("string", &[][..])], ("u64", &[][..])),
        (
            "concat",
            vec![("string", &[][..]), ("string", &[][..])],
            ("string", &[][..]),
        ),
        ("is_empty", vec![("string", &[][..])], ("bool", &[][..])),
        (
            "contains",
            vec![("string", &[][..]), ("string", &[][..])],
            ("bool", &[][..]),
        ),
        (
            "starts_with",
            vec![("string", &[][..]), ("string", &[][..])],
            ("bool", &[][..]),
        ),
        (
            "ends_with",
            vec![("string", &[][..]), ("string", &[][..])],
            ("bool", &[][..]),
        ),
        (
            "split",
            vec![("string", &[][..]), ("string", &[][..])],
            ("Array", &["string"][..]),
        ),
        ("trim", vec![("string", &[][..])], ("string", &[][..])),
        ("to_lower", vec![("string", &[][..])], ("string", &[][..])),
        ("to_upper", vec![("string", &[][..])], ("string", &[][..])),
    ] {
        validate_function(&string, name, &[], &parameters, return_type)?;
    }
    Ok(())
}

pub fn validate_source_api_surface() -> Result<(), String> {
    for module_path in SOURCE_DEFINED_MODULES {
        let module = module(module_path)
            .ok_or_else(|| format!("source-defined standard module `{module_path}` is unknown"))?;
        let path = module_source_path(module);
        let source = fs::read_to_string(&path)
            .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
        let ast = parse_source_contract(&path.display().to_string(), &source)?;
        validate_package(&ast, module_path)?;

        let mut actual = BTreeSet::new();
        actual.extend(
            ast.structs
                .iter()
                .filter(|item| item.public)
                .map(|item| item.name.as_str()),
        );
        actual.extend(
            ast.enums
                .iter()
                .filter(|item| item.public)
                .map(|item| item.name.as_str()),
        );
        actual.extend(
            ast.interfaces
                .iter()
                .filter(|item| item.public)
                .map(|item| item.name.as_str()),
        );
        actual.extend(
            ast.consts
                .iter()
                .filter(|item| item.public)
                .map(|item| item.name.as_str()),
        );
        actual.extend(
            ast.functions
                .iter()
                .filter(|item| item.public)
                .map(|item| item.name.as_str()),
        );

        let mut expected = module.items.iter().copied().collect::<BTreeSet<_>>();
        if *module_path == "std.array" {
            // Array's layout remains compiler-owned during the source migration.
            expected.remove("Array");
        }
        if *module_path == "std.io" {
            // IoError is an implicit compiler type, not an importable item.
            actual.remove("IoError");
        }
        if *module_path == "std.option" {
            // Higher-order helpers remain compiler-backed until function values exist.
            expected.remove("map");
            expected.remove("and_then");
        }
        if *module_path == "std.result" {
            // Higher-order helpers remain compiler-backed until function values exist.
            expected.remove("map");
            expected.remove("map_err");
            expected.remove("and_then");
        }
        if actual != expected {
            return Err(format!(
                "source API for `{module_path}` does not match the standard registry: expected {:?}, found {:?}",
                expected, actual
            ));
        }
    }
    Ok(())
}

fn parse_source_contract(path: &str, source: &str) -> Result<SourceFile, String> {
    let path = Path::new(path);
    let tokens = lex(path, source).map_err(|error| format!("{path:?}: {}", error.message))?;
    parse(path, &tokens).map_err(|error| format!("{path:?}: {}", error.message))
}

fn validate_package(source: &SourceFile, expected: &str) -> Result<(), String> {
    let actual = source.package.join(".");
    if actual == expected {
        Ok(())
    } else {
        Err(format!(
            "std source package `{actual}` does not match `{expected}`"
        ))
    }
}

fn validate_carrier(
    source: &SourceFile,
    name: &str,
    type_parameter_count: usize,
    variants: &[(&str, Option<&str>)],
) -> Result<(), String> {
    let Some(carrier) = source.enums.iter().find(|item| item.name == name) else {
        return Err(format!("std source is missing carrier enum `{name}`"));
    };
    if !carrier.public || carrier.type_params.len() != type_parameter_count {
        return Err(format!(
            "std carrier `{name}` has the wrong visibility or generic shape"
        ));
    }
    if carrier.variants.len() != variants.len()
        || carrier.variants.iter().zip(variants).any(
            |(actual, (expected_name, expected_payload))| {
                actual.name != *expected_name
                    || actual
                        .payload
                        .as_ref()
                        .and_then(|payload| payload.path.first())
                        .map(String::as_str)
                        != *expected_payload
            },
        )
    {
        return Err(format!("std carrier `{name}` has the wrong variant shape"));
    }
    Ok(())
}

fn validate_function(
    source: &SourceFile,
    name: &str,
    type_parameters: &[&str],
    parameters: &[(&str, &[&str])],
    return_type: (&str, &[&str]),
) -> Result<(), String> {
    let Some(function) = source.functions.iter().find(|item| item.name == name) else {
        return Err(format!("std source is missing pure helper `{name}`"));
    };
    if !function.public
        || !function
            .type_params
            .iter()
            .map(String::as_str)
            .eq(type_parameters.iter().copied())
    {
        return Err(format!(
            "std helper `{name}` has the wrong visibility or generic parameters"
        ));
    }
    if function.params.len() != parameters.len()
        || function
            .params
            .iter()
            .zip(parameters)
            .any(|(actual, (expected_name, expected_args))| {
                !type_ref_matches(&actual.type_ref, expected_name, expected_args)
            })
        || !type_ref_matches(&function.return_type, return_type.0, return_type.1)
    {
        return Err(format!("std helper `{name}` has the wrong type signature"));
    }
    Ok(())
}

fn type_ref_matches(type_ref: &TypeRef, name: &str, args: &[&str]) -> bool {
    type_ref.path.len() == 1
        && type_ref.path[0] == name
        && type_ref.args.len() == args.len()
        && type_ref
            .args
            .iter()
            .zip(args)
            .all(|(actual, expected)| type_ref_matches(actual, expected, &[]))
}

pub fn is_supported_import(import: &str) -> bool {
    MODULES.iter().any(|module| {
        import == module.path
            || import
                .strip_prefix(module.path)
                .and_then(|suffix| suffix.strip_prefix('.'))
                .is_some_and(|item| module.items.contains(&item))
    })
}

pub fn all_imports() -> Vec<String> {
    let mut imports = MODULES
        .iter()
        .flat_map(|module| {
            std::iter::once(module.path.to_string()).chain(
                module
                    .items
                    .iter()
                    .map(|item| format!("{}.{}", module.path, item)),
            )
        })
        .collect::<Vec<_>>();
    imports.sort();
    imports.dedup();
    imports
}

pub fn manifest_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("nomo.toml")
}

pub fn source_root() -> PathBuf {
    if let Some(root) = env::var_os("NOMO_STD_SOURCE_ROOT") {
        return PathBuf::from(root);
    }

    let compiled_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    if compiled_root.is_dir() {
        return compiled_root;
    }

    if let Ok(executable) = env::current_exe() {
        for ancestor in executable.ancestors() {
            let installed_root = ancestor.join("std/src");
            if installed_root.is_dir() {
                return installed_root;
            }
        }
    }

    compiled_root
}

pub fn module_source_path(module: &StandardModule) -> PathBuf {
    source_root().join(module_source_relative_path(module))
}

pub fn module_source_relative_path(module: &StandardModule) -> PathBuf {
    let name = module
        .path
        .strip_prefix("std.")
        .expect("standard module paths use the std root");
    PathBuf::from(format!("{name}.nomo"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;
    use std::fs;

    #[test]
    fn standard_import_registry_is_sorted_unique_and_complete() {
        let imports = all_imports();
        assert_eq!(imports.len(), 199);
        assert!(imports.windows(2).all(|pair| pair[0] < pair[1]));
        assert!(imports.iter().all(|import| is_supported_import(import)));
        assert!(!is_supported_import("std.io.IoError"));
        assert!(is_supported_import("std.num.to_string"));
        assert!(!is_supported_import("std.io.flush"));
    }

    #[test]
    fn standard_modules_have_matching_nomo_sources() {
        let mut packages = BTreeSet::new();
        for module in modules() {
            assert!(packages.insert(module.path));
            let path = module_source_path(module);
            let source = fs::read_to_string(&path).unwrap_or_else(|error| {
                panic!("failed to read standard module {}: {error}", path.display())
            });
            assert!(source.starts_with("//! "), "{}", path.display());
            assert!(
                source.contains(&format!("\npackage {}\n", module.path)),
                "{}",
                path.display()
            );
        }
    }

    #[test]
    fn intrinsic_manifest_is_valid_and_points_at_canonical_sources() {
        validate_intrinsic_manifest().unwrap();
        let manifest = intrinsic_manifest().unwrap();
        for binding in manifest.bindings {
            assert!(
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join(binding.source)
                    .is_file()
            );
        }
    }

    #[test]
    fn source_defined_modules_match_the_standard_registry() {
        validate_source_api_surface().unwrap();
    }

    #[test]
    fn intrinsic_manifest_rejects_duplicate_binding_ids() {
        let source = INTRINSIC_MANIFEST_SOURCE.replace("id = \"result\"", "id = \"option\"");
        let error = validate_intrinsic_manifest_source(&source).unwrap_err();
        assert!(
            error.contains("duplicate std intrinsic binding id `option`"),
            "{error}"
        );
    }

    #[test]
    fn intrinsic_manifest_rejects_unknown_modules() {
        let source = INTRINSIC_MANIFEST_SOURCE
            .replace("module = \"std.result\"", "module = \"std.missing\"");
        let error = validate_intrinsic_manifest_source(&source).unwrap_err();
        assert!(error.contains("unknown module `std.missing`"), "{error}");
    }

    #[test]
    fn intrinsic_manifest_rejects_required_identity_drift() {
        let source = INTRINSIC_MANIFEST_SOURCE
            .replace("abi = \"carrier-propagation\"", "abi = \"enum-carrier\"");
        let error = validate_intrinsic_manifest_source(&source).unwrap_err();
        assert!(
            error.contains("binding `question` does not match the required canonical identity"),
            "{error}"
        );
    }

    #[test]
    fn intrinsic_manifest_requires_array_and_string_layout_identities() {
        for (needle, expected) in [
            ("abi = \"array-header\"", "array"),
            ("abi = \"string-header\"", "string"),
        ] {
            let source = INTRINSIC_MANIFEST_SOURCE.replace(needle, "abi = \"drifted\"");
            let error = validate_intrinsic_manifest_source(&source).unwrap_err();
            assert!(
                error.contains(&format!("binding `{expected}` does not match")),
                "{error}"
            );
        }
    }

    #[test]
    fn toolchain_manifest_declares_canonical_standard_package() {
        let manifest = fs::read_to_string(manifest_path()).unwrap();
        assert!(manifest.contains("namespace = \"nomo-lang\""));
        assert!(manifest.contains("name = \"std\""));
    }
}
