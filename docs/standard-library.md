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
`fmt`, `io`, `fs`, `path`, `env`, `process`, `time`, `num`, `math`, `char`, `os`,
`collections`, `hash`, `crypto`, `json`, `jsonrpc`, `cron`, `regex`, `debug`,
`log`, `testing`, `net`, `http`, `sqlite`, `task`, and `ffi`.

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

`std.fmt` provides type-safe value formatting without performing I/O:

```nomo
fmt.to_string(value) -> string
fmt.debug_string(value) -> string
fmt.format(template: string, values...) -> string
```

Primitive strings, numbers, characters, and booleans have built-in formatting.
Local non-generic structs participate by implementing `fmt.Display` or
`fmt.Debug`:

```nomo
import std.fmt

struct User {
    name: string
}

impl fmt.Display for User {
    fn to_string(self) -> string {
        return self.name
    }
}
```

`fmt.format` requires a compile-time string literal. `{}` uses `Display`,
`{:?}` uses `Debug`, and `{{` / `}}` emit literal braces. The compiler rejects
unknown placeholders and mismatched value counts. Nomo intentionally does not
provide C-style `%`-based `printf`.

`std.io` provides console I/O:

```nomo
io.print(values...) -> void
io.println(values...) -> void
io.eprint(values...) -> void
io.eprintln(values...) -> void
io.read_line() -> Result<string, IoError>
```

Print helpers format values through `std.fmt`; multiple values are separated by
one space. `print` and `println` target standard output, while `eprint` and
`eprintln` target standard error. Formatting results are consumed after their
bytes are written, while borrowed strings remain owned by the caller. The C99
backend writes multi-value output left-to-right without allocating an
intermediate concatenated string.

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
such as `1500ms`, and sleep helpers panic for negative durations. `sleep` and
`sleep_millis` block the current OS thread. They remain compatible in
synchronous code and legacy isolated workers, but E0891 rejects any transitive
path from a `suspend fn` to either operation. Suspend code uses the RFC 0035
nonblocking `task.sleep(Duration) -> Result<void, TaskError>` owner-local timer
described below.

### `std.cron`

`std.cron` provides bounded five-field UTC schedule calculation:

```nomo
cron.parse(expression: string) -> Result<CronSchedule, CronError>
cron.matches(schedule: CronSchedule, unix_millis: i64) -> Result<bool, CronError>
cron.next_after(schedule: CronSchedule, unix_millis: i64) -> Result<i64, CronError>
```

Fields are minute (`0..59`), hour (`0..23`), day of month (`1..31`), month
(`1..12`), and day of week (`0..6`, Sunday zero). A field accepts wildcard,
unsigned values, inclusive ranges, lists, and wildcard/range steps. Expressions
are limited to 256 bytes. Calculation uses the UTC proleptic Gregorian calendar
from 1970 through 9999 and does not depend on the host locale or timezone
database.

`matches` ignores seconds and milliseconds within the supplied UTC minute.
`next_after` returns a minute boundary strictly later than the supplied instant
and searches at most 4,208,400 minutes. `CronSchedule` is opaque; parse and
calculation errors return stable `syntax`, `range`, `limit`, `timestamp_range`,
or `no_match` codes without reproducing the rejected expression.

The module calculates schedules but does not own callbacks, threads, job
persistence, overlap, or missed-run policy. Native Agents compose it with
`std.time`, `std.task`, and optionally `std.sqlite`. The pure calculation
operations work in browser WASM and inside isolated tasks. See
`examples/cron_schedule`.

### `std.task`

`std.task` provides scope-owned current-thread tasks plus bounded, isolated
legacy native workers without introducing general closures or shared managed
values:

```nomo
task.yield_now() -> void // suspend-only
task.sleep(duration: Duration) -> Result<void, TaskError> // suspend-only
task.check_cancelled() -> void // suspend-only, non-suspending observation
task.scope { ... } // suspend-only structured scope
task.deadline(duration: Duration) { ... } // suspend-only structured deadline
task.channel<T>(capacity: u64) -> Result<Channel<T>, ChannelError>
task.send(channel: Channel<T>, value: T) -> Result<void, ChannelSendError<T>> // suspend-only
task.receive(channel: Channel<T>) -> Option<T> // suspend-only
task.try_send(channel: Channel<T>, value: T) -> ChannelTrySend<T>
task.try_receive(channel: Channel<T>) -> ChannelTryReceive<T>
task.close(channel: Channel<T>) -> void
let child = task.spawn child_function(arguments) // scope-owned Task<T>
task.join(child) -> Result<T, TaskError> // consumes structured child
task.cancel(child) -> Result<void, TaskError> // suspend-capable, consumes structured child
task.spawn(worker: task fn(TaskContext, string) -> string, input: string) -> Result<Task, TaskError>
task.is_cancelled(context: TaskContext) -> bool
task.join(task_value: Task, timeout_millis: u64) -> Result<TaskJoin, TaskError>
task.cancel(task_value: Task) -> Result<void, TaskError>
task.close(task_value: Task) -> Result<void, TaskError>
```

`task.yield_now` and `task.sleep` are the first new `suspend fn` runtime
primitives. In P1, yield and void suspend-function calls are standalone
statements; value-returning suspend calls and sleep are immutable top-level
`let` initializers. Native C99 execution uses a stack-allocated root
frame with embedded child frames and a current-thread executor. Child polls run
inline; yield enters the ready queue immediately, while a positive sleep
registers a bounded owner-local monotonic timer and enters the queue only after
expiry. A non-positive sleep completes inline without registration or queue
traffic. Immutable top-level locals with frame-safe transitive value fields can
live across suspension: only live values enter frames, and managed ARC/COW
fields use ownership bits so child-first normal completion and explicit early
root drop are idempotent. Immutable frame-safe parameters evaluate once and
managed results move before child drop. Mutable parameters/locals, resource
handles or wrappers containing them, recursive suspension, suspending argument
expressions, `?` outside the structured form below, and panic nested in another
expression remain E0876 until their cleanup paths land. A direct top-level
language `panic(message)` or `debug.panic(message)` statement uses the
structured panic path described below. Browser WASM
treats yield as a bounded cooperative boundary; sleep returns
`runtime_unavailable` without evaluating its duration. The host-driven event
backend is not implemented yet.

`task.deadline(duration) { ... }` evaluates its duration once. A non-positive
duration terminates the current suspend task with stable code `timeout` before
the body and without a timer registration. A positive duration uses one
owner-local monotonic timer; normal fallthrough disarms it, while expiry
cancels the current child subtree and pending registrations before task
completion. A parent observes the typed failure through structured join.
`task.check_cancelled()` is a non-suspending, allocation-free cooperative
check. Browser WASM rejects deadlines without evaluating the duration or body
until its host-driven backend lands.

P3-B adds a current-thread `Channel<T>` when `T` satisfies compiler-known
structural Send. Capacity is 1 through 65,536 elements and checked slot storage
is capped at 64 MiB. Sends consume their value; full/closed immediate attempts
and failed suspended sends return exactly one owner through
`ChannelTrySend<T>` or `ChannelSendError<T>`. Receives are FIFO, direct handoff
prefers the oldest blocked receiver, and a full queue resumes blocked senders
in FIFO order as slots become available. `close` is idempotent, wakes waiters,
and drains existing buffered values before `receive` returns `None`. Copies of
the handle share one owner-local control block; ordinary collections do not
gain locks or atomic reference counts.

Browser WASM construction returns `runtime_unavailable` without evaluating
capacity. Until its host-driven backend lands, other channel operations report
a capability error before evaluating operands that would be consumed. Native
examples and the exact counter/Go evidence gate are documented in
`examples/async_bounded_channel` and `performance/async/manifest-p3.json`.

P3-C adds the compiler-recognized
`task.select { operation => binding { ... } ... }` statement. Its first
native C99 slice accepts 2 through 8 direct `task.receive(Channel<T>)` and
`task.sleep(Duration)` arms. Operands evaluate exactly once from top to
bottom; immediate readiness resolves in source order. Otherwise all pending
arms share one owner-local token, the first winner eagerly cleans every loser,
and the owner frame is enqueued at most once. Each arm body must be non-empty,
non-suspending, and fall through normally. Browser WASM rejects the capability
before operand evaluation. General send/join selection and structured arm
exits remain later work. See `examples/async_static_select`.

The structured forms create true owner-local concurrency without changing
direct-style child calls. Each child target is currently a direct,
unqualified, non-generic top-level `suspend fn` with immutable frame-safe
parameters and result. Spawn evaluates the arguments once, derives
compiler-known structural Send, publication-moves each non-Copy named binding
into an embedded child frame, and schedules it on the bounded 64-entry FIFO.
Copy scalars remain available. Files, sockets, HTTP streams, process/SQLite/task
handles, FFI handles, and aggregates containing them are Local/!Send. P3-A has
no public `Send` interface or `move` keyword; E0880/E0883 identify capability
failures and E0881 diagnoses use after publication. The child
return type becomes `Task<T>`. Join suspends only until that child completes,
moves the result exactly once, and returns `Result<T, TaskError>`; queue
saturation is reported with the stable `queue_full` code. Each inferred
immutable handle must stay in its scope and may be consumed at most once by
join or structured cancel. Structured `task.cancel(child)` is a
suspend-capable cancel-and-join boundary: it propagates cancellation through
the child subtree, removes its ready/timer registrations, waits for terminal
cleanup, returns `Result<void, TaskError>`, and drops the consumed frame. An
already-completed child succeeds. The current-thread owner completes this
inline without allocation or new queue traffic; the generated pending ABI is
reserved for later owner-shard acknowledgement. Normal
scope fallthrough automatically cancels and drops unjoined children, including
ready-queue and timer cleanup. A nested helper may also use a final scope
`return`; its expression evaluates first, then unjoined children are cancelled
and dropped before the owned temporary moves and wakes the root frame. Nested
helpers may also use an immutable top-level
`let value: T = expression?`: Err/None is stored in the helper frame before
live children are cancelled and dropped, while a success payload follows the
normal liveness plan. The operand may be non-suspending or a direct
`task.join(handle)?`, and currently needs an explicit binding type. The first
deadline slice permits one non-nested, non-value-producing deadline block per
suspend function. It must fall through; deadline-body control flow, return,
`?`, panic, defer, unsafe, and nested scopes/deadlines remain E0876. Nested
scopes, nested control flow, non-final scope return, `?` in other positions,
panic nested in another expression, cancellation tokens, general deadline
exits, cross-shard channels, and general send/join select are not in this
slice. P3-C's static receive/timer arms still reject nested suspension and
non-fallthrough exits. A direct panic owns its
non-suspending message, stops the executor, recursively cancels the root task
tree, drops every frame, completes runtime shutdown and metrics export, then
prints and releases the original message before process exit. Browser WASM
does not execute structured children; join and structured cancel return
`runtime_unavailable`, while non-suspending `?` preserves the same early-exit
result and inert cleanup boundary, and a direct panic becomes the same
deterministic runtime error without executing a child.

The remaining functions above are the legacy blocking/native isolation
surface, not aliases for the new suspend task model. A suspend function is
rejected as a `task.spawn` worker. RFC 0032 will migrate this compatibility API
behind a bounded, lazy blocking pool; it still uses one native thread per
worker in this P1 slice.

A worker is a non-generic, non-capturing, top-level function in the caller's
package with the exact `fn(TaskContext, string) -> string` signature. Spawn
deep-copies one at-most-8-MiB input before creating the thread; completion
deep-copies one at-most-8-MiB output. A process may keep at most 64 live task
handles, and a join timeout may not exceed 900,000 milliseconds.
`Task` and `TaskContext` are runtime-owned opaque values; direct construction
or field access is rejected with `E0820`.

The legacy `task.cancel(Task)` below the structured overload is cooperative.
`TaskJoin.Cancelled` is observable only after the
worker sees or races with the request and returns; a request alone does not
terminate a thread. `join(..., 0)` is a nonblocking poll and returns
`TaskJoin.Timeout` while work remains. `close` returns `busy` until the worker
finishes, then joins the native thread and releases the handle. Closed handles
return the stable `closed` error.

See the [async runtime implementation guide](async-runtime.md),
`examples/async_yield`, `examples/async_timer`, and
`examples/async_bounded_channel` for the current suspend-task boundary.

#### Task safety

The compiler checks the worker's transitive call graph. Local computation,
managed values created inside the worker, arrays, strings, JSON, JSON-RPC,
cron calculation, regex,
collections, hashing, crypto, numeric/path/OS helpers, monotonic time/sleep,
`task.is_cancelled`, and nonstreaming `http.get`/`post`/`send` are accepted.
Unsafe/extern/FFI calls, nested tasks, filesystem/environment/process/TCP/UDP,
console/log/debug/testing, formatting through user implementations, wall-clock
time, HTTP streaming/server handles, value methods with unknown effects, and
unknown calls are rejected with `E0821` and a stable call path. Panics still
terminate the process in v0.1.

Unix-like targets use POSIX threads and Windows uses native threads through the
toolchain-owned runtime, so application code writes no C FFI. Browser WASM
returns `TaskError { code: "runtime_unavailable", ... }` from `spawn` without
invoking or evaluating the worker. See `examples/isolated_tasks` for
join/cancel/deep-copy behavior and `examples/concurrent_openai_compatible` for
two real HTTPS requests against a local TLS fixture.

`std.os` reports target information from the C compiler target:

```nomo
os.platform() -> string
os.arch() -> string
os.path_separator() -> string
os.line_ending() -> string
```

## Durable SQLite Persistence

`std.sqlite` provides bounded native persistence without application-side C
FFI, a host SQLite package, a database subprocess, or a separate service. The
toolchain carries the official SQLite 3.53.3 amalgamation, verifies its pinned
digests, and compiles it as a separate translation unit only when typed IR uses
this module. Programs that do not use `std.sqlite` do not materialize, compile,
or link SQLite.

The canonical API is:

```nomo
sqlite.open(path: string, mode: SqliteOpenMode, busy_timeout_millis: u64)
    -> Result<SqliteDatabase, SqliteError>
sqlite.open_memory(busy_timeout_millis: u64)
    -> Result<SqliteDatabase, SqliteError>
sqlite.execute(database: SqliteDatabase, sql: string, params: Array<SqliteValue>)
    -> Result<SqliteExecuteResult, SqliteError>
sqlite.query(database: SqliteDatabase, sql: string, params: Array<SqliteValue>)
    -> Result<SqliteQuery, SqliteError>
sqlite.next(query_value: SqliteQuery, max_row_bytes: u64)
    -> Result<Option<SqliteRow>, SqliteError>
sqlite.reset(query_value: SqliteQuery, params: Array<SqliteValue>)
    -> Result<void, SqliteError>
sqlite.close_query(query_value: SqliteQuery) -> Result<void, SqliteError>
sqlite.close(database: SqliteDatabase) -> Result<void, SqliteError>
```

`SqliteOpenMode` is `ReadOnly`, `ReadWrite`, or `ReadWriteCreate`.
`SqliteValue` maps SQLite `NULL`, signed 64-bit integers, doubles, UTF-8 text,
and BLOBs to `Null`, `Integer(i64)`, `Real(f64)`, `Text(string)`, and
`Blob(Array<u32>)`. BLOB elements must be in `0..=255`. Result columns preserve
order and duplicate names.

`execute` accepts exactly one parameterized statement and rejects a statement
that returns rows. `query` creates a prepared pull handle; every `next` copies
at most one complete row only after its columns pass encoding and byte limits.
Repeated pulls after completion return `None`. `reset` clears and replaces all
bindings for prepared-query reuse. Transactions are explicit SQL—there is no
implicit begin, retry, commit, or rollback.

`SqliteDatabase` and `SqliteQuery` are opaque runtime capabilities. They cannot
be constructed or have their fields read or written. `close` returns
`busy_handle` while a live query belongs to the connection. Closed and copied
stale capabilities return `closed`. Normal process exit finalizes any remaining
handles and prints only their counts, never paths, SQL, parameters, or rows.

The fixed v0.1 limits are:

- 32 live databases and 256 live queries per process;
- 4096 bytes per persistent path and 1 MiB per SQL statement;
- 1024 parameters, 8 MiB per text/BLOB value, and 16 MiB total parameter bytes;
- 256 result columns, 8 MiB per text/BLOB result, and a caller row limit in
  `1..=16 MiB`;
- busy timeout in `0..=300_000` milliseconds.

`SqliteError.code` is one of `invalid_request`, `limit`, `open`, `prepare`,
`bind`, `step`, `busy`, `constraint`, `read_only`, `corrupt`, `full`,
`encoding`, `unexpected_row`, `busy_handle`, `closed`,
`runtime_unavailable`, or `internal`. Messages are bounded and generic; SQL,
paths, schema identifiers, bound values, and SQLite error strings are not
copied into diagnostics.

SQLite handles are thread-confined. `std.sqlite` is rejected from `std.task`
workers with `E0821`; serialized SQLite compilation does not make Nomo
capabilities transferable. Browser WASM type-checks the API, but opening a
database returns `runtime_unavailable` and adds no filesystem imports.

See `examples/sqlite_memory` for pull/lifecycle basics and
`examples/sqlite_agent_memory` for a parameterized Agent checkpoint written in
an explicit transaction and recovered by a second process.

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

`std.json` stores bounded, validated raw JSON text and exposes structured
traversal and construction:

```nomo
json.parse(value: string) -> Result<JsonValue, JsonError>
json.stringify(value: JsonValue) -> string

json.kind(value: JsonValue) -> JsonKind
json.is_null(value: JsonValue) -> bool
json.as_bool(value: JsonValue) -> Option<bool>
json.number_text(value: JsonValue) -> Option<string>
json.as_string(value: JsonValue) -> Option<string>
json.array_items(value: JsonValue) -> Option<Array<JsonValue>>
json.object_members(value: JsonValue) -> Option<Array<JsonMember>>
json.get(value: JsonValue, key: string) -> Option<JsonValue>

json.from_null() -> JsonValue
json.from_bool(value: bool) -> JsonValue
json.from_number_text(value: string) -> Result<JsonValue, JsonError>
json.from_i64(value: i64) -> JsonValue
json.from_u64(value: u64) -> JsonValue
json.from_string(value: string) -> Result<JsonValue, JsonError>
json.from_array(values: Array<JsonValue>) -> Result<JsonValue, JsonError>
json.from_object(members: Array<JsonMember>) -> Result<JsonValue, JsonError>
```

`parse` preserves the complete input text for exact `stringify` round trips.
Nested arrays and object members retain document order. Object duplicates are
preserved, while `get` compares decoded names and returns the last match.
`number_text` preserves the exact JSON number token; use `std.num` for explicit
numeric conversion.

Every value is limited to 8 MiB, 128 nested arrays/objects, and 262,144 total
values. `JsonError.code` is `syntax`, `limit`, `unsupported_string`, or
`invalid_number`; its message never includes source text or secret-bearing
values. Because v0.1 strings are NUL-terminated, escaped U+0000 and unpaired
surrogates are rejected as `unsupported_string`. Native C and browser WASM
provide the same pure JSON operations.

`std.jsonrpc` builds on `std.json` with validated JSON-RPC 2.0 envelopes and
incremental newline-delimited framing:

```nomo
jsonrpc.decoder(max_message_bytes: u64)
    -> Result<JsonRpcDecoder, JsonRpcProtocolError>
jsonrpc.feed(decoder_value: JsonRpcDecoder, chunk: string)
    -> Result<JsonRpcDecodeBatch, JsonRpcProtocolError>
jsonrpc.finish(decoder_value: JsonRpcDecoder)
    -> Result<void, JsonRpcProtocolError>
jsonrpc.parse(value: JsonValue, max_message_bytes: u64)
    -> Result<JsonRpcMessage, JsonRpcProtocolError>
jsonrpc.encode(message: JsonRpcMessage, max_message_bytes: u64)
    -> Result<string, JsonRpcProtocolError>
jsonrpc.value(message: JsonRpcMessage) -> JsonValue
jsonrpc.kind(message: JsonRpcMessage) -> JsonRpcMessageKind

jsonrpc.request(id: JsonValue, method: string, params: Option<JsonValue>)
    -> Result<JsonRpcMessage, JsonRpcProtocolError>
jsonrpc.notification(method: string, params: Option<JsonValue>)
    -> Result<JsonRpcMessage, JsonRpcProtocolError>
jsonrpc.success(id: JsonValue, result: JsonValue)
    -> Result<JsonRpcMessage, JsonRpcProtocolError>
jsonrpc.failure(
    id: JsonValue,
    code: i64,
    message: string,
    data: Option<JsonValue>
) -> Result<JsonRpcMessage, JsonRpcProtocolError>
```

`JsonRpcMessageKind` is `Request`, `Notification`, `Success`, or `Error`.
`JsonRpcDecoder` and `JsonRpcMessage` are opaque value-state types. `feed`
returns a replacement decoder plus every complete message in the input chunk;
the original decoder remains unchanged. Chunks may split a UTF-8 message at
any byte position or contain multiple messages. One CR immediately before LF
is stripped, empty frames are rejected, and `finish` rejects any unterminated
suffix. `encode` emits the validated raw envelope followed by exactly one LF.

Requests require a string or number `id`, a string `method`, and
optional object/array `params`; notifications omit `id`. Success and error
responses accept string, number, or null `id`. Error objects require an exact
signed 64-bit integer `code` and string `message`. Reserved fields may not be
duplicated, while extension fields and their order are preserved.

The transport ceilings are 1,048,575 bytes per message, 1 MiB per `feed`
chunk, and 4096 decoded messages per call. `max_message_bytes` must be in
`1..=1,048,575`. `JsonRpcProtocolError.code` is `invalid_request`, `limit`,
`framing`, `json`, or `protocol`; messages are bounded and never copy input
payloads, method names, ids, error data, or other secret-bearing values.

Native C and browser WASM implement the same pure codec. The module does not
launch processes itself. Compose it with the shell-free `std.process`
controlled API, write only encoded messages to child stdin, feed only stdout
to the decoder, and route stderr separately as logs. See `examples/mcp_stdio`
for a two-request MCP client that handles fragmented/coalesced output and
correlates response ids without application-side C FFI.

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

`std.map` provides the v0.1 general-purpose key-value container:

```nomo
import std.array
import std.map

Map.new<K, V>() -> Map<K, V>
map.len<K, V>(map: Map<K, V>) -> u64
map.is_empty<K, V>(map: Map<K, V>) -> bool
map.contains_key<K, V>(map: Map<K, V>, key: K) -> bool
map.get<K, V>(map: Map<K, V>, key: K) -> Option<V>
map.set<K, V>(mut map: Map<K, V>, key: K, value: V) -> Option<V>
map.remove<K, V>(mut map: Map<K, V>, key: K) -> Option<V>
map.clear<K, V>(mut map: Map<K, V>) -> void
map.keys<K, V>(map: Map<K, V>) -> Array<K>
map.values<K, V>(map: Map<K, V>) -> Array<V>
```

`Map<K, V>` stores arbitrary generic values, including `JsonValue` and
application structs. Keys use Nomo equality (`==`), so `K` must be a type for
which equality is valid. The implementation preserves first-insertion order:
replacing a value does not move its key, removal closes the gap, and a later
reinsertion appends the key. `keys` and `values` return independent
copy-on-write snapshots in matching order, which provides deterministic entry
iteration by index.

`Map` intentionally has one deterministic implementation rather than a second
public `HashMap` alias. Its array-backed lookup is linear and its capacity is
bounded at 65,536 entries. This avoids exposing an incomplete hash/equality
contract or backend-dependent iteration order in v0.1; applications needing
untrusted, high-cardinality hash tables should use a bounded host service.

`StringMap` and `StringSet` in `std.collections` remain source- and
binary-compatible for v0.1. They are legacy string-specialized APIs; new code
should prefer `Map<string, V>`. No silent rewrite or removal occurs in this
release.

Array values may also be created and accessed directly:

```nomo
import std.array

let values = [1, 2, 3]                 // Array<i32>
let matrix = [[1, 2], [3, 4]]         // Array<Array<i32>>
let empty: Array<i32> = []
let first: i32 = values[0]
let mut editable = matrix
editable[0][1] = 7
```

An unconstrained integer literal defaults to `i32`. Literal elements must have
exactly the same type; array construction performs no implicit numeric
conversion. `[]` needs an expected `Array<T>` type. Indices have type `u64` and
an out-of-range `[]` read or write panics with `array index out of bounds` on
both native C and browser WASM. `Array.get` remains the non-panicking
`Option<T>` API.

Indexed assignment evaluates each index once, left to right, then the value
once. Nested writes perform copy-on-write at every array level and write the
updated child back to the root, so snapshots never observe later mutations.

## Processes And Networking

`std.process` retains these legacy blocking shell helpers:

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

New code that needs a long-lived child or incremental I/O uses the shell-free
controlled API:

```nomo
pub struct ProcessEnv {
    pub name: string
    pub value: string
}

pub struct ProcessCommand {
    pub program: string
    pub args: Array<string>
    pub cwd: Option<string>
    pub env: Array<ProcessEnv>
    pub inherit_env: bool
}

pub struct ProcessExit {
    pub code: i32
    pub signal: i32
}

pub struct ProcessControlError {
    pub code: string
    pub message: string
}

pub enum ProcessEvent {
    StdinFlushed
    Stdout(string)
    Stderr(string)
    Exited(ProcessExit)
}

process.start(command: ProcessCommand) -> Result<ProcessChild, ProcessControlError>
process.write_stdin(child: ProcessChild, data: string) -> Result<void, ProcessControlError>
process.close_stdin(child: ProcessChild) -> Result<void, ProcessControlError>
process.next_event(child: ProcessChild, max_chunk_bytes: u64, timeout_millis: u64) -> Result<ProcessEvent, ProcessControlError>
process.try_wait(child: ProcessChild) -> Result<Option<ProcessExit>, ProcessControlError>
process.terminate(child: ProcessChild) -> Result<void, ProcessControlError>
process.close_child(child: ProcessChild) -> void
```

`start` invokes `program` directly and never a shell. A program containing a
path separator is resolved directly; a bare name is searched in the final
child `PATH`. `cwd = None` inherits the current directory. With
`inherit_env = true`, explicit entries override inherited variables; with
`false`, only explicit variables and platform-required entries are present.
Environment names must be non-empty, contain neither `=` nor NUL, and be
unique (case-insensitively on Windows).

`write_stdin` accepts one non-empty UTF-8 payload of at most 1 MiB. Only one
payload may be pending. The caller waits for `StdinFlushed` before queuing the
next payload; a timeout preserves the pending suffix. `close_stdin` is
idempotent after the payload has flushed and returns `busy` while data remains
pending.

`next_event` accepts chunk sizes from 4 bytes through 1 MiB and positive
timeouts through 15 minutes. It preserves ordering independently within
stdout and stderr, does not split a UTF-8 scalar, and emits `Exited` only after
both streams reach EOF. Invalid UTF-8 or NUL is a `protocol` error and closes
the child. After `Exited`, `try_wait`, `terminate`, and `close_child` remain
safe; another `next_event` returns `invalid_request`.

Call `defer process.close_child(child)` immediately after `start`.
`close_child` is idempotent and forcibly terminates and reaps a child that is
still running. `ProcessControlError.code` is one of `invalid_request`, `busy`,
`spawn`, `io`, `timeout`, `protocol`, or `runtime_unavailable`. Its message and
default diagnostics never include program, argv, environment, cwd, stdin,
stdout, or stderr values. Native Unix-like and Windows adapters are owned by
the toolchain; application code declares no C FFI. Browser WASM rejects this
controlled API before argument evaluation.

See `examples/process_controlled` for two queued stdin messages and
multiplexed output/exit handling.

`std.net` provides the first owner-affine async TCP client slice plus explicit
blocking compatibility helpers:

```nomo
net.connect(host: string, port: i64, timeout_millis: u64) -> Result<TcpStream, NetError>
TcpStream.read(max_bytes: u64, timeout_millis: u64) -> Result<TcpChunk, NetError>
TcpStream.read_string(max_bytes: u64, timeout_millis: u64) -> Result<TcpTextChunk, NetError>
TcpStream.write(data: Array<u32>, timeout_millis: u64) -> Result<void, NetError>
TcpStream.write_string(content: string, timeout_millis: u64) -> Result<void, NetError>
TcpStream.close() -> void

net.connect_blocking(host: string, port: i64) -> Result<TcpStream, NetError>
net.listen(host: string, port: i64) -> Result<TcpListener, NetError>
net.udp_bind(host: string, port: i64) -> Result<UdpSocket, NetError>
```

`connect` is a direct-style suspend operation. P2-TCP-A accepts numeric IPv4
and IPv6 addresses, uses epoll on Linux and kqueue on macOS, and bounds the
timeout to 900,000 milliseconds. P2-TCP-C accepts a hostname of at most 253
bytes and resolves it on one lazily started worker behind a fixed 16-job
capacity. Resolver completion returns to the owner executor through the same
platform reactor; at most 16 IPv4/IPv6 candidates are attempted in resolver
order under one overall resolution-plus-connect deadline. Numeric addresses
do not initialize the worker or its completion registration. P2-TCP-D uses
`ConnectEx` through a fixed owner-local IOCP operation table and posts resolver
completion to the same owner IOCP on Windows. A zero timeout performs one
immediate numeric attempt. A zero-timeout hostname returns `Timeout` without
initializing the resolver pool or reactor on every native platform. The first
stackless slice requires
binding the complete `Result`; direct `?` propagation on these suspend I/O
operations remains an E0876 limitation.

`read` returns after at least one byte, EOF, timeout, cancellation, or error;
it never reads to EOF implicitly. `TcpChunk.data` uses the v0.1 byte
convention (`Array<u32>` values in `0..=255`). `read_string` returns one valid
UTF-8 chunk; invalid UTF-8 or embedded NUL is `NetErrorKind.Read` and never
exposes partial text. `eof` may be true with empty data.

One read/write payload is limited to 1,048,576 bytes and one timeout to
900,000 milliseconds. Zero makes one immediate attempt with no I/O
registration. A positive operation uses one owner-local timer and either one
epoll/kqueue one-shot registration when it would block, or one Windows
`WSARecv`/`WSASend` submission from a fixed 64-slot IOCP table. Writes retain
progress and the bounded unsent suffix across readiness events, cap work at
64 KiB per executor poll for fairness, suppress Unix `SIGPIPE`, and complete
the whole input or fail. At most one operation per stream direction may be
pending; a conflict returns `Busy`.

Timeout and structured cancellation deregister readiness, disarm the timer,
release the direction claim and retained payload exactly once, and leave the
stream open. `close` invalidates its owner-table generation and is idempotent
against stale copies. The runtime exports exact connect/read/write, readiness,
timeout/cancellation/error, live-operation, live-handle, and retained-byte
counters. See `examples/async_tcp_io`.

Resolver saturation returns `NetErrorKind.Limit`; lookup failure returns
`Resolve`. Error messages are fixed categories and never copy the hostname.
Cancellation removes a queued lookup immediately. A lookup already executing
inside the system resolver is cooperatively detached: its caller completes at
the deadline or cancellation boundary, while the executor retains the
completion registration and joins the worker after the system call returns.
This focused resolver pool is not yet the general, dynamically sized blocking
pool described by RFC 0032.

`NetError.kind` is a portable `NetErrorKind`; applications must not parse
platform details from `message`. `TcpStream` is Local/!Send and identifies its
owner-table slot and generation rather than exposing a raw socket as
authority. `close` is idempotent against stale generations.

P2-TCP-B/C/D execute read/write and bounded hostname resolution natively on
Linux, macOS, and Windows. Windows cancellation moves any in-flight payload
ownership into stable reactor storage, calls `CancelIoEx`, and drains late
completions before shutdown. Because the browser WASM sandbox has no raw-TCP
host capability, P2-TCP-E returns `NetErrorKind.Unsupported` before evaluating
the `net.connect` host, port, or timeout operands. Applications branch on
`kind`; the generic secret-safe message is not a parsing contract. A future
host-driven raw-TCP adapter remains a focused capability. A dedicated
`shutdown_write` half-close operation is not part of P2-TCP-B; callers must use
`close` until that focused lifecycle slice lands.

For the preview migration window, `connect_blocking`,
`read_to_string_blocking`, and `write_string_blocking` retain the old blocking
behavior. A suspend call graph reaching one of them reports E0891. Listener
accept and UDP remain blocking pending focused follow-up slices.

`std.http` provides a bounded blocking HTTP/HTTPS client and basic plain-HTTP
server helpers:

```nomo
http.send(request: HttpRequest) -> Result<HttpResponse, HttpError>
http.get(url: string) -> Result<HttpResponse, HttpError>
http.post(url: string, body: string) -> Result<HttpResponse, HttpError>
http.open_stream(request: HttpRequest, idle_timeout_millis: u64) -> Result<HttpStream, HttpError>
http.read_text(stream: HttpStream, max_chunk_bytes: u64) -> Result<HttpStreamChunk, HttpError>
http.next_sse(stream: HttpStream, max_event_bytes: u64) -> Result<Option<SseEvent>, HttpError>
http.cancel_stream(stream: HttpStream) -> void
http.close_stream(stream: HttpStream) -> void
http.listen(host: string, port: i64) -> Result<HttpServer, HttpError>
http.accept(server: HttpServer) -> Result<HttpExchange, HttpError>
http.respond_string(exchange: HttpExchange, status: i64, body: string) -> Result<void, HttpError>
http.close_server(server: HttpServer) -> void
http.close_exchange(exchange: HttpExchange) -> void
```

`HttpRequest` contains `method`, `url`, `headers`, `body`, `timeout_millis`,
and `max_response_bytes`. v0.1 accepts `GET` and `POST`. `HttpResponse`
contains `status`, ordered `headers`, and `body`; HTTP 4xx/5xx statuses remain
successful transport responses. `HttpError.code` is one of
`invalid_request`, `runtime_unavailable`, `dns`, `connect`, `tls`, `timeout`,
`response_too_large`, `protocol`, or `transport`.

`open_stream` returns after the response head is available. For streaming
requests, `HttpRequest.timeout_millis` bounds connect, TLS, request send, and
response-head receipt; `idle_timeout_millis` bounds each later pull that makes
no progress. `HttpStream` exposes `status` and ordered `headers`.
`max_response_bytes` remains a cumulative body limit with a 128 MiB hard
ceiling.

`read_text` returns non-empty UTF-8 chunks without splitting a Unicode scalar,
then `{ data: "", done: true }` at EOF. `max_chunk_bytes` is from 4 bytes
through 1 MiB. `next_sse` parses CRLF, CR, and LF framing, a leading BOM,
comments, multi-line `data`, `event`, persistent `id`, and decimal `retry`
fields. Its positive `max_event_bytes` limit has a 1 MiB ceiling. `[DONE]`
remains ordinary event data for the application to interpret. The first
`read_text` or `next_sse` call selects the stream mode; mixing modes returns
`invalid_request`.

Register `defer http.close_stream(stream)` immediately after opening.
`close_stream` and cooperative `cancel_stream` are idempotent, including for
copied stale handles. Cancellation takes effect between blocking pulls;
`idle_timeout_millis` bounds a currently blocked pull. Reads after close or
cancel return `invalid_request`.

HTTPS verifies certificates and host names through platform trust. There is no
insecure mode, redirects are disabled, response headers are limited to 64 KiB,
and response bodies have a hard 128 MiB ceiling. `get` and `post` use a
30-second timeout and an 8 MiB response limit. Header names and values are
validated before I/O; callers may set `Authorization` and `Content-Type` but
may not override framing headers such as `Host` or `Content-Length`.
On Unix-like targets, `NOMO_HTTP_CA_BUNDLE` adds a PEM trust root for
deterministic local testing without disabling host-name verification. Windows
uses its current-user and machine certificate stores instead.

The native adapter is owned by the toolchain runtime, so Nomo applications do
not declare C FFI or linker flags to use HTTPS. Native Unix-like targets use a
compatible libcurl runtime and Windows uses WinHTTP. The browser WASM sandbox
does not grant network access in v0.1; calling these helpers returns the stable
`NOMO-WASM-003` capability error without evaluating or logging request secrets.
A browser host-capability design remains a later RFC.

Use `defer http.close_exchange(exchange)` and
`defer http.close_server(server)` so cleanup runs on both normal returns and
`?` early returns. Binary streaming, connection pooling, routing, and
concurrent server helpers remain later slices.

## Native FFI Values

`std.ffi` declares the value types used at explicit C boundaries in
`std/src/ffi.nomo`; their layout and ownership rules remain compiler-owned:

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
