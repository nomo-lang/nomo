# Async Runtime

This guide records implementation status for the Proposed async and concurrency
RFCs. It is evidence about the current toolchain, not a claim that every RFC
acceptance gate has passed.

- [RFC 0031](https://github.com/nomo-lang/rfcs/blob/main/en/rfcs/0031-direct-style-suspend-functions-and-structured-concurrency.md)
  defines direct-style suspend effects, stackless lowering, frame destruction,
  and structured concurrency.
- [RFC 0032](https://github.com/nomo-lang/rfcs/blob/main/en/rfcs/0032-sharded-executor-reactor-and-blocking-pool.md)
  defines the executor/reactor, owner affinity, platform backends, and blocking
  pool migration.
- [RFC 0033](https://github.com/nomo-lang/rfcs/blob/main/en/rfcs/0033-task-ownership-transfer-and-concurrent-values.md)
  defines transfer and sharing capabilities.
- [RFC 0034](https://github.com/nomo-lang/rfcs/blob/main/en/rfcs/0034-async-runtime-acceptance-and-benchmark-gates.md)
  defines correctness, portability, memory, and performance gates.
- [RFC 0035](https://github.com/nomo-lang/rfcs/blob/main/en/rfcs/0035-monotonic-suspend-timers-and-blocking-sleep-migration.md)
  defines owner-local timers and the blocking-sleep boundary.
- [RFC 0036](https://github.com/nomo-lang/rfcs/blob/main/en/rfcs/0036-bounded-channels-publication-moves-and-static-select.md)
  defines bounded channels, consuming publication, and the later static-select
  surface.
- [RFC 0037](https://github.com/nomo-lang/rfcs/blob/main/en/rfcs/0037-owner-affine-async-tcp-client-and-blocking-migration.md)
  defines the bounded owner-affine async TCP client and blocking migration.

[中文版本](async-runtime.zh-CN.md)

## Language Surface

Functions that may suspend use `suspend fn`. Calls remain direct-style:

```nomo
package app.main

import std.io
import std.task

suspend fn yield_once() -> void {
    task.yield_now()
}

suspend fn main() -> void {
    io.println("before")
    yield_once()
    io.println("after")
}
```

A normal `fn` cannot call a suspend function. The compiler reports E0870
instead of adding hidden runtime behavior. Merely declaring or calling an
always-ready suspend function does not create an executor.

The compiler also rejects a `suspend fn` whose transitive call graph reaches a
quarantined blocking compatibility API. E0891 covers blocking sleep and TCP
compatibility calls, HTTP/HTTPS request and stream progress, blocking HTTP
server progress, legacy shell helpers, and process lifecycle operations that
can spawn, wait, terminate, or reap. It reports only the function/API call
path, never argument values. Synchronous functions and legacy isolated workers
retain the blocking APIs. The
nonblocking `task.sleep(Duration) -> Result<void, TaskError>` API is available
on the native C99 current-thread backend. Its duration is evaluated once, a
non-positive duration completes inline, and a positive duration registers an
owner-local monotonic timer. The browser sandbox returns a stable
`runtime_unavailable` result until its host-driven timer backend lands.

The P2-TCP-A/B/C/D slices provide direct-style connect, bounded incremental
reads, and complete writes:

```nomo
import std.net
import std.result

suspend fn main() -> void {
    let connected: Result<TcpStream, NetError> =
        net.connect("localhost", 8080, 1000)
}

suspend fn exchange(stream: TcpStream) -> void {
    let wrote: Result<void, NetError> =
        stream.write_string("ping", 1000)
    let shutdown: Result<void, NetError> =
        stream.shutdown_write()
    let received: Result<TcpTextChunk, NetError> =
        stream.read_string(4096, 1000)
}
```

Linux and macOS attempt each socket in nonblocking mode and suspend through
generation-checked epoll/kqueue registrations. Windows numeric IPv4/IPv6
connects use `ConnectEx`; reads and writes use `WSARecv`/`WSASend` and a fixed
64-slot owner-local IOCP operation table. Numeric addresses do not start an OS
thread. On every native platform, a hostname of at most 253 bytes enters one
lazy resolver worker through a 16-live-job bounded capacity; completion returns
through the owner reactor, and up to 16 IPv4/IPv6 candidates are attempted in
resolver order. Unix uses a nonblocking completion pipe; Windows posts the
completion to the owner IOCP. Resolution and every candidate share one
monotonic deadline bounded to 15 minutes. A zero hostname timeout returns
inline without initializing the pool or reactor.
`TcpStream` remains bound to its owner executor and is Local/!Send.

The browser WASM sandbox does not expose raw TCP. It returns
`NetErrorKind.Unsupported` before evaluating any `net.connect` host, port, or
timeout operand, so rejected capability calls cannot execute or diagnose
operand secrets. A future host-driven adapter must preserve the same typed,
bounded contract.

Each stream permits one pending read and one pending write; another operation
in the same direction returns `Busy`. `read` returns one `Array<u32>` byte
chunk, `read_string` validates one UTF-8 chunk, and neither reads to EOF. Each
payload is bounded to 1 MiB. Writes retain only their unsent suffix across
one-shot readiness, advance at most 64 KiB per executor poll for fairness, and
either complete the whole payload or return an error. Timeout and structured
cancellation remove the registration and retained buffer while leaving the
stream reusable unless it is closed.

`shutdown_write` is a synchronous, allocation-free write half-close. It never
cancels a pending write: that conflict returns `Busy`. A successful call is
idempotent, leaves reads available, and makes later writes return `Closed`.
Stale or fully closed handles also return `Closed`; a native failure returns
`Write` without changing the write-half state. Final `close` remains the
terminal idempotent cleanup path.

A saturated resolver queue returns `Limit`, and lookup failure returns
`Resolve`; neither diagnostic copies the hostname. Queued cancellation removes
the job immediately. Cancellation of an in-progress system resolver call is
cooperative: the caller reaches its terminal result, but executor shutdown
waits for that lookup's completion so the worker and owner registration can be
cleaned exactly once. This one-worker resolver is a focused P2-TCP-C slice, not
the general RFC 0032 blocking pool. On Windows, timeout or structured
cancellation detaches any pending read/write buffer from the coroutine frame,
requests `CancelIoEx`, and lets the fixed IOCP slot own that buffer until the
late completion is drained. Reactor shutdown drains all live IOCP slots before
closing the completion port, so frame destruction never leaves an
`OVERLAPPED` pointer into freed frame storage.
The preview blocking names are
`net.connect_blocking`, `read_to_string_blocking`, and
`write_string_blocking`; reaching them from a suspend call graph reports
E0891. This stackless slice binds each complete `Result` as shown above;
placing `?` directly on these I/O operations remains an E0876 limitation
until the general suspend-question lowering slice lands. P2-TCP-F implements
`shutdown_write` through `SHUT_WR` on Unix and `SD_SEND` on Windows. See
[`examples/async_tcp_io`](../examples/async_tcp_io).

P2-PROC-A separates the process contracts. `process.start(command, timeout)`
and `process.next_event(child, max_bytes, timeout)` are suspend intrinsics over
an owner-affine Local/!Send `ProcessChild`; their C99 frames carry typed
start/resume/cancel state and exactly-once result ownership. P2-PROC-B
implements the Unix native path with one lazy bounded process worker for
start/reap jobs and owner-reactor registrations for nonblocking pipes. Linux
uses epoll plus `pidfd` when available, falling back to one bounded
reaper/wakeup source; macOS uses kqueue plus `EVFILT_PROC`, with that same
source closing the exit-registration race. Event timeout and task cancellation
remove interests exactly once while preserving the child and pending stdin
suffix. Close never waits, and runtime shutdown reaps every child. Windows
P2-PROC-C uses overlapped named pipes associated with the owner IOCP. One
lazy bounded worker performs process creation, while `RegisterWaitForSingleObject`
posts generation-checked exit completion back to the same IOCP. Stdin and
both output streams use stable reactor operation slots plus `CancelIoEx`
late-completion draining; the async path creates no per-child reader or writer
threads. `iocp_operations_started` counts only operations accepted for
completion delivery, while `live_iocp_operations` also covers a slot reserved
during the immediate system-call attempt; a synchronous EOF or start failure
therefore releases capacity without fabricating a submitted operation.

RFC 0024 behavior remains temporarily available only through
`BlockingProcessChild` and explicit `_blocking` names, all quarantined by
E0891. See
[`examples/async_process_pipe_contract`](../examples/async_process_pipe_contract)
and
[`examples/async_process_pipe_unix`](../examples/async_process_pipe_unix) or
[`examples/async_process_pipe_windows`](../examples/async_process_pipe_windows).

The first structured-concurrency slice uses an explicit lexical scope and
explicit concurrency creation while keeping child calls direct-style:

```nomo
import std.result
import std.task

suspend fn child(message: string) -> string {
    task.yield_now()
    return message
}

suspend fn main() -> void {
    task.scope {
        let handle = task.spawn child("ready")
        let joined: Result<string, TaskError> = task.join(handle)
        let completed: bool = result.is_ok(joined)
    }
}
```

`task.spawn child(args)` differs deliberately from the legacy
`task.spawn(worker, input)` call with parentheses. The structured form infers
a scope-owned `Task<T>` from the child return type; the one-argument join
consumes it exactly once and returns `Result<T, TaskError>`.

### Publication moves and compiler-known Send

Structured spawn is now a publication boundary. The compiler derives a
private capability for every child parameter; user code cannot implement or
override it in this slice:

| Value | Publication behavior |
| --- | --- |
| numeric, `bool`, `char` | Copy; the source binding remains available |
| `string`, `CString`, `Array<T>`, `Map<K,V>`, ordinary structs/enums | Send when every nested type is Send; a named binding is consumed |
| `File`, sockets, HTTP server/exchange/stream, `ProcessChild`, `BlockingProcessChild`, SQLite/task/FFI handles | Local/!Send and rejected |

```nomo
let message: AgentMessage = build_message()
task.scope {
    let child = task.spawn consume(message)
    // E0881: message was publication-moved into child
}
```

There is deliberately no `move` keyword and no public `Send` interface.
Consumption is determined by the compiler-known spawn argument position.
Constants remain reusable, and owned temporaries transfer directly. Moving
only a managed field such as `message.content` is rejected in P3-A; move the
whole aggregate or construct a temporary. E0880 reports a direct Local value,
E0883 reports the first nested field that prevents structural Send, and E0881
reports duplicate or later use after publication.

The IR marks each consuming argument explicitly. Native C99 initializes the
embedded child parameter first, sets the child's ownership bit, then clears
the parent parameter/local ownership bit. It does not retain the moved value,
and child cancellation, queue rejection, join, panic, and parent drop all
converge on the existing exactly-once child-frame cleanup. Cross-shard COW
detach is intentionally deferred until the sharded executor exists; the
current implementation has only one owner thread.

An immutable top-level
`let cancelled: Result<void, TaskError> = task.cancel(handle)` is the
structured consuming cancel-and-join operation. It requests cancellation,
waits for terminal child cleanup, then consumes and drops the handle. An
already-completed child returns `Ok(void)`; a child whose spawn was rejected
returns the same stable `queue_full` error. This overload is distinct from the
legacy synchronous `task.cancel(Task)` worker request.

The first deadline slice is another compiler-recognized structured scope:

```nomo
import std.task
import std.time

suspend fn bounded_work() -> string {
    task.deadline(time.duration_millis(50)) {
        let waited: Result<void, TaskError> =
            task.sleep(time.duration_millis(1000))
        task.check_cancelled()
    }
    return "completed"
}
```

The duration is evaluated exactly once. A non-positive duration terminates the
current suspend task with `TaskError { code: "timeout", ... }` before executing
the body and without registering a timer. A positive duration arms one
owner-local monotonic timer. Normal fallthrough disarms it; expiry cancels the
current frame's child subtree and pending timer/ready registrations before
completing the task. A structured parent observes that failure through
`task.join` rather than receiving a fabricated child value. A root timeout
prints only the stable code and exits nonzero.

`task.check_cancelled()` is non-suspending and does not allocate or enqueue.
It is an explicit cooperative observation point; the generated state machine
also checks immediately before and after runtime suspension boundaries.
Timeout wins if a ready operation and its deadline are both observable at a
resume boundary.

### Bounded channels

The P3-B current-thread slice adds a typed bounded FIFO:

```nomo
let created: Result<Channel<string>, ChannelError> =
    task.channel<string>(8)
let sent: Result<void, ChannelSendError<string>> =
    task.send(channel_value, message)
let received: Option<string> = task.receive(channel_value)
```

`task.channel<T>(capacity)` accepts 1 through 65,536 elements and at most
64 MiB of checked slot storage. It returns stable `invalid_capacity`,
`capacity_limit`, or `allocation` errors without formatting user values.
`task.send` and `task.receive` are direct-style suspension points;
`task.try_send` and `task.try_receive` never suspend. `task.close` is
idempotent, wakes blocked operations, rejects new sends, and preserves buffered
FIFO values until drained.

A named non-Copy send value is publication-moved. Success transfers its single
owner to a receiver or ring slot. Full, closed, and runtime failures return
exactly one owner through `ChannelTrySend<T>` or `ChannelSendError<T>`.
Channel-handle copies share one current-thread control block; ordinary arrays,
maps, and strings keep non-atomic task-local ARC/COW storage. This slice adds
no atomic shim or cross-shard sharing.

Native C99 uses an owner-local ring plus FIFO sender and receiver
registrations. A waiting receiver receives a value directly; otherwise a full
ring suspends the sender. Cancellation, timeout, close, wake-before-resume,
frame drop, and normal completion unlink each registration and release or
return a staged value exactly once. See
[`examples/async_bounded_channel`](../examples/async_bounded_channel).

Browser WASM does not yet provide the host-driven channel backend. Construction
returns `runtime_unavailable` without evaluating capacity. Other channel
operations report a sandbox capability error before evaluating a channel
operand or send value that would be consumed.

### Static receive/timer select

P3-C adds one compiler-recognized statement with 2 through 8 static arms:

```nomo
task.select {
    task.receive(messages) => message {
        consume(message)
    }
    task.sleep(time.duration_millis(50)) => timeout {
        observe(timeout)
    }
}
```

The first slice accepts only direct `task.receive(Channel<T>)` and
`task.sleep(Duration)` operations. Every operand evaluates exactly once from
top to bottom before cancellation/deadline readiness checks. If multiple arms
are already ready, the first source-order arm wins. Otherwise each arm
registers against the same owner-local select token; the first successful
claim eagerly unlinks or disarms every loser and enqueues the owner frame at
most once. A late loser event cannot execute an arm body.

Each arm binds its operation result (`Option<T>` or
`Result<void, TaskError>`) in a non-empty lexical body. This initial lowering
requires normal fallthrough and rejects `return`, `break`, `continue`, `?`,
panic, defer, nested scope/deadline/select, and suspending arm bodies with
E0876. Send/join/select operations and general structured exits remain later
slices. Browser WASM reports `runtime_unavailable` before evaluating any arm
operand rather than approximating select sequentially. See
[`examples/async_static_select`](../examples/async_static_select).

## Implemented P1, P2 Reactor/P2-TCP-A/B/C/D-numeric, and P3-B/P3-C Slices

On the native C99 backend, a suspend call chain that reaches
`task.yield_now()` or `task.sleep(...)` emits:

- a stack-allocated root frame with an explicit state and embedded child frames;
- one poll/drop pair per actually suspending function;
- direct child polls whose `PENDING` result propagates to the root;
- an inline initial poll;
- a bounded 64-entry owner-local FIFO ready queue entered only after
  `PENDING`, with explicit saturation rather than unbounded growth;
- a bounded owner-local timer table with generation-checked registrations,
  monotonic deadlines, deterministic deadline/generation ordering, and
  idempotent disarm;
- a lazily initialized owner-local platform reactor: epoll on Linux, kqueue on
  macOS, and IOCP on Windows. Positive timers enter that reactor with a bounded
  timeout; ready-only work and non-positive timers do not initialize it;
- a bounded 64-slot I/O owner table with slot generations and exclusive close,
  plus one embedded connect registration and timer for each pending TCP
  candidate on epoll/kqueue, or one fixed-table IOCP operation for each
  Windows connect/read/write submission;
- one lazy resolver worker behind 16 fixed job slots, a nonblocking Unix owner
  wake pipe or posted Windows IOCP completion, at most 16 copied address
  candidates, one overall deadline, and exact
  queue/running/cancelled/completed/live/peak lifecycle counters;
- one embedded read/write registration per source operation, one pending
  operation per stream direction, one-shot readiness rearming, complete
  partial-write progress, bounded retained-byte metrics, and exactly-once
  timeout/cancellation cleanup;
- Windows `OVERLAPPED` storage that remains in the owner reactor rather than
  the coroutine frame, with `CancelIoEx`, detached payload ownership for late
  completions, fixed 64-operation backpressure, and shutdown draining;
- embedded structured child frames enqueued onto the same bounded FIFO, plus a
  single owner-local waiter edge that re-enqueues the parent when its child
  completes;
- a typed `TaskError { code: "queue_full", ... }` materialized by join when a
  structured spawn cannot enter the 64-entry ready queue;
- exactly-once transfer of a typed child result into the successful join
  payload before the child frame is dropped;
- compiler-known structural Send checking plus ownership-bit transfer for
  non-Copy named structured-spawn arguments;
- a structured cancel-and-join suspension boundary that propagates
  cancellation through the child subtree, removes ready/timer registrations,
  waits for terminal cleanup, returns `Result<void, TaskError>`, and drops the
  consumed child frame;
- one non-nested `task.deadline(Duration)` scope per suspend function, with
  immediate non-positive timeout, saturating monotonic deadline calculation,
  deterministic ready/timeout checks, typed child failure, and child-first
  cancellation cleanup;
- static receive/timer select tokens with source-order ready arbitration,
  exactly-once operand evaluation, one owner-frame wake, eager loser cleanup,
  and no heap task or per-select allocation;
- compiler-inserted scope cleanup on normal fallthrough and final `return` that
  cancels unjoined children, removes their ready-queue entries, disarms owned
  timers, and drops their frames before the next statement or return
  completion;
- owned Err/None propagation from a direct structured `?` binding, with live
  sibling cleanup before helper completion and parent wakeup;
- exact top-level local liveness across each yield or child call;
- per-field ownership bits for managed ARC/COW frame values;
- idempotent child-first frame drop that clears ownership before release.

This slice creates no OS thread, heap task, or atomic metadata. A ready
zero-duration timer neither registers, enters the queue, nor initializes the
reactor. A positive timer lazily creates one owner-local epoll, kqueue, or IOCP
instance, waits through it rather than `Sleep`/`nanosleep`, and closes it before
metrics export. The timer is not polled again until its deadline moves the
owner frame to the ready queue. The generated context records poll, yield,
frame-drop/live-frame,
enqueue/dequeue/saturation/cancellation, structured
spawn/publication-move/join/join-suspension/cancellation, deadline
registration/expiry/disarm, and timer
registration/expiry/cancellation/live/peak counters. It also records reactor
initialization, wait, timeout, completion, error, shutdown, and live/peak
lifecycle counters. The pure-yield probe requires every reactor counter to
remain zero; the positive-timer probe requires one initialization, wait,
timeout, and shutdown with zero live reactors at exit.
The P3-B channel slice also records construction binding, buffered/direct
delivery, suspension, wakeup, close/cancellation, and live/peak buffer and
waiter counts.
Native programs export the versioned
`nomo-c99-current-thread` JSON payload only when
`NOMO_ASYNC_METRICS_PATH` is set; ordinary runs perform no metrics I/O.
The P1 benchmark collects that payload in a separate probe after measured runs.
ARC primitive counters remain explicitly unavailable rather than being
reported as zero.
Locals that die before a suspension are released without entering the frame.
Immutable locals used after a suspension are moved into the frame; only those
referenced in a resumed segment are reintroduced as non-owning C aliases.
An embedded child is polled inline and does not allocate or enter the ready
queue when it completes synchronously. Normal completion and explicit early
root drop share the same child-first idempotent cleanup path. Immutable
frame-safe call arguments evaluate exactly once from left to right. Ordinary
direct suspend calls retain shared managed values into the child frame.
Structured spawn instead publication-moves non-Copy named bindings; owned
temporaries transfer directly in both forms. An owned result moves into its
immutable caller binding before the child frame is dropped.

That inline fast path describes ordinary direct suspend calls. A structured
spawn is intentionally concurrent: it evaluates immutable frame-safe
arguments once, validates compiler-known Send, publication-moves non-Copy
named bindings into an embedded child frame, and schedules that frame on the
bounded FIFO. Join suspends only while the selected child is incomplete. Child
completion wakes one owner-local waiter, and both explicit join cleanup and
parent cleanup use idempotent child drop. This slice creates no heap task, OS
thread, atomic reference count, or global work-stealing queue.

Structured cancel is suspend-capable for the future shard-acknowledgement
path, but the current-thread owner can complete cancellation and frame cleanup
inline. The ready fast path therefore allocates nothing and does not add a
queue round trip. The generated ABI still records
`NOMO_ASYNC_PENDING_CANCEL` so a later owner-shard implementation can suspend
the parent until the owner acknowledges terminal cleanup without changing
source semantics.

Browser WASM accepts the same source in its bounded sandbox interpreter.
`task.yield_now()` is currently a cooperative boundary there; it does not yet
return control to a host Promise or browser event loop. `task.sleep` does not
block or evaluate its duration in the browser sandbox; it returns
`TaskError { code: "runtime_unavailable", ... }`. Structured child bodies are
also not evaluated there yet; their join and structured cancel return the same
stable error and consume the inert browser handle. `task.deadline` currently
returns a sandbox capability error without evaluating either its duration or
body. Channel operations use the capability behavior described above. Static
select is also a capability error before operand evaluation; host-driven
browser deadlines, channels, and select remain later backend slices.

## Deliberate Restrictions

E0876 rejects unsupported suspension rather than miscompiling it. In this
slice, `task.yield_now()` and value-less calls to actually suspending functions
must be standalone statements. A value-returning suspend call and
`task.sleep(Duration)` must initialize an immutable top-level `let`. The
containing `suspend fn` remains non-generic; its parameters, result, and
cross-suspension locals must be immutable frame-safe scalar, string, struct,
enum, Result, or supported array values. Async `main` still returns `void`.
Mutable parameters/locals, borrows, guards, resource handles or wrappers
containing them, recursive suspend graphs, suspension in control flow, nested
expressions or argument expressions, `?` outside the direct structured binding
described below, panic nested inside another expression, cancellation
tokens, and reactor-backed socket/process/HTTP operations are later slices. The
current P2 foundation normalizes timer waiting only; it does not claim that a
network or process handle is nonblocking yet.

The current deadline slice permits one non-nested
`task.deadline(Duration) { ... }` per suspend function. Its body has the same
top-level structured spawn/join/cancel ownership rules as `task.scope` and may
contain the supported direct suspension shapes plus
`task.check_cancelled()`. It is deliberately non-value-producing and must
fall through normally. Nested deadlines/scopes, control flow, `return`, `?`,
panic, defer, or unsafe inside the deadline body require the later general
structured-exit and nested-deadline lowering. No public cancellation-token
value exists in v0.1.

Structured spawn/join is available only in a top-level `task.scope` body. Each
spawn handle must use an inferred immutable binding, remain in that scope, and
may be consumed at most once by a direct immutable `task.join(handle)` or
`task.cancel(handle)` binding. Structured cancel returns
`Result<void, TaskError>` only after the child is terminal and its
registrations are removed; an already-completed child succeeds. The handle
cannot then be joined or cancelled again. The target must be a direct
unqualified,
non-generic top-level `suspend fn` with immutable frame-safe parameters and
result. Its return type becomes `Task<T>` and
`task.join(handle) -> Result<T, TaskError>`.
A final `return` first evaluates its expression into a private owned temporary,
then performs compiler-inserted cancellation and drop of every unjoined child.
The temporary moves into the helper frame before that helper completes and
wakes its root frame. Normal fallthrough uses the same cleanup before the next
statement. An immutable top-level `let value: T = expression?` evaluates its
operand once. On Err/None, the propagated carrier is stored as owned frame
state, every child live at that statement is cancelled and dropped, and the
helper completes and wakes its parent. On success, the payload may cross a
later suspension through the normal frame liveness plan. `expression` may be
non-suspending or the direct `task.join(handle)?` form; an explicit type
annotation remains required in this slice. A cancelled child body does not
resume. A direct top-level `panic(message)` statement evaluates and owns its
non-suspending message before propagation. A child panic stops the executor;
the root recursively cancels every incomplete child, removes ready entries,
disarms timers, drops all frames, runs runtime shutdown and metrics export,
then prints and releases the original message before exiting with status 1.
`debug.panic` uses the same statement path. Browser WASM returns the same
runtime error while keeping structured child bodies inert. Nested scopes,
nested scope control flow, non-final scope return, defer/unsafe blocks, `?` in
other positions, panic nested in another expression, cancellation tokens,
nested/general deadline exits, cross-shard channels, and general send/join
select remain later slices. P3-C's static receive/timer select is limited to
non-empty fallthrough arm bodies without nested suspension.
E0871, E0872, E0875, and E0876 reject unsupported cases before code
generation.

The existing `task.spawn` API remains the legacy isolated native-worker API.
It is not an async task constructor and still maps one worker to one native
thread. RFC 0032 requires its migration to a bounded, lazy blocking pool.

## Correctness and Cost Gates

This slice checks exact spills, pre-suspension cleanup for dead locals,
child-before-parent ownership-bit clearing, repeated explicit drop, monotonic
no-early-wake behavior, zero-duration timer fast paths, and cancellation of an
armed child timer under generated-C tests and AddressSanitizer. Structured
tasks additionally test FIFO interleaving, one-shot join ownership, waiter
wakeup, typed queue saturation, browser non-execution, and idempotent child
cleanup. Managed typed results additionally test child-to-join ownership
transfer, root-frame wakeup from a nested helper, post-join scope return, and
repeated parent drop under AddressSanitizer. Scope cancellation tests cover an
armed timer, never-polled ready children, a typed helper return, and typed `?`
error propagation that cancel before root-frame wakeup, including managed
parameter, propagated error, and result release under AddressSanitizer. Panic
tests cover managed sync messages, a spawned panicking child, recursive root
cancellation, an armed sibling timer, exact frame/task/timer counters, and the
browser-WASM boundary. Explicit structured-cancel tests cover an armed timer,
the exact result/handle ownership transition, generated pending ABI, native
and browser behavior, exact counters, and AddressSanitizer cleanup.
Deadline tests additionally cover non-positive body suppression, normal
disarm, timeout while a child frame owns an armed sleep, typed join failure,
root secret-safe failure, exact timer/deadline counters, browser
non-evaluation, and AddressSanitizer cleanup.
Bounded-channel tests cover element and byte limits, FIFO wraparound, direct
handoff, full/empty try operations, buffered close, blocked sender/receiver
wakeup, repeated close, timeout cancellation, typed value recovery,
cross-suspension handle liveness, exact counters, native C99 and browser
capability behavior, and AddressSanitizer/UndefinedBehaviorSanitizer cleanup.
Static-select tests additionally cover source-order immediate readiness,
suspend-and-wake arbitration, receive/timer loser removal, exact select/live
resource counters, C99 generation, browser operand suppression, and
AddressSanitizer cleanup.
The P3 manifest runs the same capacity-eight 32-value exchange against pinned
single-core Go while keeping the result ineligible for a performance claim.
Later slices must still prove, rather than assume:

- exactly-once ARC/COW release on the remaining error, cancellation, timeout,
  and nested-expression or runtime-originated panic paths;
- no unsafe mutable borrow or guard crossing a suspension point;
- no runtime, thread, coroutine metadata, or atomic collection cost for
  programs that do not use suspension;
- no allocation or ready-queue operation on the synchronous-ready path;
- C99 and browser-WASM compatibility, followed by Linux, macOS/BSD, and
  Windows reactor coverage;
- fair, version-pinned Nomo-versus-Go measurements without weakening either
  workload.

The P0/P1/P3 controls and raw evidence format live in
[`performance/async`](../performance/async/README.md). Runnable examples are
[`examples/async_yield`](../examples/async_yield) and
[`examples/async_timer`](../examples/async_timer), plus
[`examples/async_structured_void`](../examples/async_structured_void) and
[`examples/async_structured_results`](../examples/async_structured_results),
plus
[`examples/async_structured_return`](../examples/async_structured_return) and
[`examples/async_structured_cancel`](../examples/async_structured_cancel), plus
[`examples/async_structured_return_cancel`](../examples/async_structured_return_cancel)
and
[`examples/async_structured_question_cancel`](../examples/async_structured_question_cancel),
plus
[`examples/async_structured_explicit_cancel`](../examples/async_structured_explicit_cancel),
plus
[`examples/async_structured_panic_cleanup`](../examples/async_structured_panic_cleanup).
The bounded FIFO example is
[`examples/async_bounded_channel`](../examples/async_bounded_channel).
The static receive/timer selection example is
[`examples/async_static_select`](../examples/async_static_select).
