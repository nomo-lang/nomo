#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};

pub const PACKAGE_ID: &str = "nomo-lang/std";
pub const IMPORT_ROOT: &str = "std";

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

pub const MODULES: &[StandardModule] = &[
    StandardModule {
        path: "std.array",
        docs: "Array helpers",
        items: ARRAY_ITEMS,
        doc_items: &[],
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
        doc_items: &[],
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
        doc_items: &[],
    },
    StandardModule {
        path: "std.string",
        docs: "string helpers",
        items: STRING_ITEMS,
        doc_items: &[],
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

pub fn modules() -> &'static [StandardModule] {
    MODULES
}

pub fn module(path: &str) -> Option<&'static StandardModule> {
    MODULES.iter().find(|module| module.path == path)
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
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
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
        assert_eq!(imports.len(), 198);
        assert!(imports.windows(2).all(|pair| pair[0] < pair[1]));
        assert!(imports.iter().all(|import| is_supported_import(import)));
        assert!(!is_supported_import("std.io.IoError"));
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
    fn toolchain_manifest_declares_canonical_standard_package() {
        let manifest = fs::read_to_string(manifest_path()).unwrap();
        assert!(manifest.contains("namespace = \"nomo-lang\""));
        assert!(manifest.contains("name = \"std\""));
    }
}
