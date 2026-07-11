# Standard Library

Nomo v0.1 treats `std` as a built-in toolchain package. User projects can
import standard modules directly; `std` does not need to be listed in
`nomo.toml`, cannot be used as a dependency alias, and is not written as a
normal package entry in `nomo.lock`.

The toolchain-owned [`std/intrinsics.toml`](../std/intrinsics.toml) binds the
small set of identities that still require compiler or runtime support. The
compiler and `nomo doc --std` validate its schema, canonical package, source
mapping, and required `Option`/`Result`/`?` bindings. A broken binding reports
`E0800`; user packages cannot override this manifest.

```nomo
package app.main

import std.io
import std.fs
import std.result

fn read(path: string) -> Result<string, FsError> {
    return fs.read_to_string(path)
}

fn main() -> void {
    io.println("Hello, Nomo")
}
```

Most helpers are available through their module name after `import std.<name>`.
Some modules also support specific imports and value-method syntax, such as
`value.is_some()` for `Option<T>` helpers and `file.read_to_string()` for
`File` methods.

The public API for the core and extension modules is declared in the canonical
`std/src/*.nomo` files. The compiler continues to lower the representation- and
host-sensitive calls through its builtin/runtime backing while this source
migration is in progress; the source files are the documentation and semantic
surface for signatures and visibility. The source registry currently covers
`io`, `fs`, `path`, `env`, `process`, `time`, `num`, `math`, `char`, `os`,
`collections`, `hash`, `crypto`, `json`, `regex`, `debug`, `log`, `testing`,
`net`, and `http`.

## Propagation Carriers

`std.option` and `std.result` define the two standard carriers used by postfix
`?`.

Their canonical source files define the `Option<T>` and `Result<T, E>` enum
shapes plus the pure predicate and `unwrap_or` helpers. The compiler checks
those source contracts but keeps the existing injected carrier layout and
runtime ABI as a compatibility path. `map`, `map_err`, and `and_then` remain
controlled intrinsics until Nomo has function values.

- `Result.Ok(value)?` evaluates to `value`.
- `Result.Err(error)?` returns `Err(error)` from the current `Result` function.
- `Option.Some(value)?` evaluates to `value`.
- `Option.None?` returns `None` from the current `Option` function.

There is no `try` keyword or statement syntax in v0.1. Use postfix `?` for both
error and absence propagation. Cross-layer `Result` error conversion is written
explicitly with `result.map_err(named_converter)?`.

`std.option` helpers:

```nomo
option.is_some(value: Option<T>) -> bool
option.is_none(value: Option<T>) -> bool
option.unwrap_or(value: Option<T>, default: T) -> T
option.map(value: Option<T>, converter: fn(T) -> U) -> Option<U>
option.and_then(value: Option<T>, converter: fn(T) -> Option<U>) -> Option<U>
```

`std.result` helpers:

```nomo
result.is_ok(value: Result<T, E>) -> bool
result.is_err(value: Result<T, E>) -> bool
result.unwrap_or(value: Result<T, E>, default: T) -> T
result.map(value: Result<T, E>, converter: fn(T) -> U) -> Result<U, E>
result.map_err(value: Result<T, E1>, converter: fn(E1) -> E2) -> Result<T, E2>
result.and_then(value: Result<T, E>, converter: fn(T) -> Result<U, E>) -> Result<U, E>
```

In v0.1, `map`, `map_err`, and `and_then` use named, unqualified, non-generic
converter functions; closures are out of scope.

## Core Modules

`std.io` provides console I/O:

```nomo
io.print(value: string) -> void
io.println(value: string) -> void
io.eprint(value: string) -> void
io.eprintln(value: string) -> void
io.read_line() -> Result<string, IoError>
```

`std.array` provides value-semantics `Array<T>` helpers:

```nomo
Array.new<T>() -> Array<T>
Array.len(self) -> u64
Array.push(mut self, value: T) -> void
Array.get(self, index: u64) -> Option<T>
Array.pop(mut self) -> Option<T>
Array.remove(mut self, index: u64) -> Option<T>
Array.set(mut self, index: u64, value: T) -> void
Array.insert(mut self, index: u64, value: T) -> void
Array.clear(mut self) -> void
Array.iter(self) -> Array<T>
```

`get`, `pop`, and `remove` return `Option<T>`. `set` and `insert` panic when
the index is out of bounds. `iter` returns a snapshot value accepted by
`for ... in`.
The canonical `std/src/array.nomo` file declares this public API. Its
representation-sensitive bodies remain compiler/runtime-backed, and
`std/intrinsics.toml` pins the `array-header` ABI during the migration.

`std.string` provides string value helpers:

```nomo
string.len(value: string) -> u64
string.concat(value: string, other: string) -> string
string.is_empty(value: string) -> bool
string.contains(value: string, needle: string) -> bool
string.starts_with(value: string, prefix: string) -> bool
string.ends_with(value: string, suffix: string) -> bool
string.split(value: string, separator: string) -> Array<string>
string.trim(value: string) -> string
string.to_lower(value: string) -> string
string.to_upper(value: string) -> string
```

Strings are UTF-8 byte strings in v0.1. `trim` and case conversion use ASCII
character classes, and `split` panics when the separator is empty.
The canonical `std/src/string.nomo` file declares these helpers while the
runtime retains the immutable reference-counted `string-header` ABI.

`std.char` provides ASCII character-class helpers and conversion:

```nomo
char.is_digit(value: char) -> bool
char.is_alpha(value: char) -> bool
char.is_whitespace(value: char) -> bool
char.to_string(value: char) -> string
```

## Files, Paths, And Environment

`std.fs` provides fallible filesystem helpers:

```nomo
fs.read_to_string(path: string) -> Result<string, FsError>
fs.write_string(path: string, content: string) -> Result<void, FsError>
fs.read_bytes(path: string) -> Result<Array<u32>, FsError>
fs.write_bytes(path: string, bytes: Array<u32>) -> Result<void, FsError>
fs.exists(path: string) -> bool
fs.metadata(path: string) -> Result<FileMetadata, FsError>
fs.create_dir(path: string) -> Result<void, FsError>
fs.remove_dir(path: string) -> Result<void, FsError>
fs.read_dir(path: string) -> Result<Array<string>, FsError>
fs.open(path: string) -> Result<File, FsError>
```

`File` methods include `read_to_string`, `write_string`, and `close`.
`read_bytes`, `write_bytes`, and `crypto.random_bytes` use `Array<u32>` byte
values in the inclusive range `0..255`. `read_dir` returns entry names, skips
`.` and `..`, and `remove_dir` removes empty directories only.

`std.path` is pure string manipulation with POSIX-style `/` separators:

```nomo
path.join(left: string, right: string) -> string
path.basename(path: string) -> string
path.dirname(path: string) -> string
path.extension(path: string) -> string
path.normalize(path: string) -> string
path.is_absolute(path: string) -> bool
```

`std.env` exposes process environment helpers:

```nomo
env.args() -> Array<string>
env.get(name: string) -> Option<string>
env.set(name: string, value: string) -> void
env.cwd() -> string
env.home_dir() -> Option<string>
env.temp_dir() -> string
```

`env.set` mutates the current process environment and panics if the platform
call fails.

## Numbers, Time, And Host Data

`std.math` provides basic numeric helpers. `abs`, `min`, and `max` preserve the
input numeric type. `floor`, `ceil`, `round`, `sqrt`, `pow`, `sin`, and `cos`
operate on `f64` values.

`std.num` provides parsing, conversion, checked arithmetic, and wrapping
arithmetic:

```nomo
num.parse_i64(value: string) -> Result<i64, NumError>
num.parse_u64(value: string) -> Result<u64, NumError>
num.parse_f64(value: string) -> Result<f64, NumError>
num.to_string(value: i64 | i32 | u32 | u64 | f64) -> string
num.checked_add(left: integer, right: same integer type) -> Option<same integer type>
num.checked_sub(left: integer, right: same integer type) -> Option<same integer type>
num.checked_mul(left: integer, right: same integer type) -> Option<same integer type>
num.wrapping_add(left: integer, right: same integer type) -> same integer type
num.wrapping_sub(left: integer, right: same integer type) -> same integer type
num.wrapping_mul(left: integer, right: same integer type) -> same integer type
```

`std.time` provides wall-clock, monotonic-clock, duration, formatting, and sleep
helpers. Durations store signed milliseconds, `format_duration` returns strings
such as `1500ms`, and sleep helpers panic for negative durations.

`std.os` reports target information from the C compiler target:

```nomo
os.platform() -> string
os.arch() -> string
os.path_separator() -> string
os.line_ending() -> string
```

## Data, Hashing, And Text Processing

`std.hash` provides stable non-cryptographic FNV-1a helpers for strings and
byte arrays:

```nomo
hash.string(value: string) -> u64
hash.bytes(value: Array<u32>) -> u64
hash.new() -> HashState
hash.write_string(state: HashState, value: string) -> HashState
hash.write_bytes(state: HashState, value: Array<u32>) -> HashState
hash.finish(state: HashState) -> u64
```

`std.crypto` provides string digest helpers and OS random bytes:

```nomo
crypto.sha256(value: string) -> string
crypto.sha512(value: string) -> string
crypto.random_bytes(count: u64) -> Array<u32>
```

`std.json` currently stores validated raw JSON text:

```nomo
json.parse(value: string) -> Result<JsonValue, JsonError>
json.stringify(value: JsonValue) -> string
```

Structured JSON field or index access is not part of v0.1.

`std.regex` provides compiled regular expression helpers:

```nomo
regex.compile(pattern: string) -> Result<Regex, RegexError>
regex.is_match(regex: Regex, value: string) -> bool
regex.captures(regex: Regex, value: string) -> Option<Array<string>>
```

`captures` returns the full match followed by capture groups.

`std.collections` provides string-specialized collections:

```nomo
collections.map_new() -> StringMap
collections.map_len(map: StringMap) -> u64
collections.map_get(map: StringMap, key: string) -> Option<string>
collections.map_contains(map: StringMap, key: string) -> bool
collections.map_set(map: StringMap, key: string, value: string) -> StringMap
collections.map_remove(map: StringMap, key: string) -> StringMap

collections.set_new() -> StringSet
collections.set_len(set: StringSet) -> u64
collections.set_contains(set: StringSet, value: string) -> bool
collections.set_insert(set: StringSet, value: string) -> StringSet
collections.set_remove(set: StringSet, value: string) -> StringSet
```

## Processes And Networking

`std.process` provides synchronous process helpers:

```nomo
process.exit(code: i64) -> void
process.spawn(command: string) -> Result<i32, ProcessError>
process.status(command: string) -> Result<i32, ProcessError>
process.exec(command: string) -> Result<string, ProcessError>
process.output(command: string) -> Result<ProcessOutput, ProcessError>
```

`spawn` and `status` wait for a shell command and return its exit code. `exec`
captures stdout and treats a non-zero exit status as an error. `output` captures
stdout and stderr and returns `Ok(ProcessOutput)` even when the command exits
non-zero so callers can inspect `status`.

`std.net` provides blocking TCP and UDP helpers:

```nomo
net.connect(host: string, port: i64) -> Result<TcpStream, NetError>
net.listen(host: string, port: i64) -> Result<TcpListener, NetError>
net.udp_bind(host: string, port: i64) -> Result<UdpSocket, NetError>
```

`TcpListener`, `TcpStream`, and `UdpSocket` expose `accept`, `read_to_string`,
`write_string`, `recv_from_string`, `send_to_string`, and `close` methods as
appropriate. v0.1 uses blocking handles; listener address inspection, backlog
configuration, and nonblocking handles are out of scope.

`std.http` provides blocking plain-HTTP client and basic server helpers:

```nomo
http.get(url: string) -> Result<HttpResponse, HttpError>
http.post(url: string, body: string) -> Result<HttpResponse, HttpError>
http.listen(host: string, port: i64) -> Result<HttpServer, HttpError>
http.accept(server: HttpServer) -> Result<HttpExchange, HttpError>
http.respond_string(exchange: HttpExchange, status: i64, body: string) -> Result<void, HttpError>
http.close_server(server: HttpServer) -> void
http.close_exchange(exchange: HttpExchange) -> void
```

Use `defer http.close_exchange(exchange)` and
`defer http.close_server(server)` so cleanup runs on both normal returns and
`?` early returns. TLS, custom headers, redirects, chunked transfer decoding,
streaming bodies, routing, and concurrent server helpers are out of scope for
v0.1.

## Native FFI Values

`std.ffi` provides the value types used at explicit C boundaries:

```nomo
import std.ffi

CString.from_string(value: string) -> CString
```

`CString` owns a NUL-terminated copy of the source string and maps to
`const char *` when passed to an `extern "C"` function. C functions cannot
return `CString`, because Nomo cannot infer ownership for a foreign pointer.
`Opaque` maps to `void *`; it can be returned by an extern function, stored,
passed through Nomo functions, and passed back to another extern function. It
cannot be dereferenced, inspected, compared, or used in arithmetic. The owning
C API remains responsible for providing and calling the matching release
function.

## Testing, Debugging, And Logging

`std.testing` supports `#[test]` functions:

```nomo
testing.assert(condition: bool, message: string) -> void
testing.assert_equal<T: primitive-or-string>(left: T, right: T) -> void
testing.assert_error<T, E>(result: Result<T, E>) -> void
```

Failed assertions panic, which makes the current test fail under `nomo test`.

`std.debug` provides `debug.print`, `debug.println`, `debug.panic`, and
`debug.backtrace`. Debug print helpers write to stderr, and `debug.backtrace`
returns a stable placeholder string in v0.1.

`std.log` provides `log.debug`, `log.info`, `log.warn`, `log.error`, and
`log.enabled`. Log helpers write `[level] message` lines to stderr and are
filtered by `NOMO_LOG`; accepted levels are `debug`, `info`, `warn`, `error`,
and `off`. The default threshold is `info`.
