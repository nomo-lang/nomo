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

The compiler also rejects a `suspend fn` whose transitive call graph reaches
the blocking compatibility APIs `time.sleep` or `time.sleep_millis`. E0891
reports only the function/API call path, never argument values. Synchronous
functions and legacy isolated workers retain the blocking APIs. The
nonblocking `task.sleep(Duration) -> Result<void, TaskError>` API is available
on the native C99 current-thread backend. Its duration is evaluated once, a
non-positive duration completes inline, and a positive duration registers an
owner-local monotonic timer. The browser sandbox returns a stable
`runtime_unavailable` result until its host-driven timer backend lands.

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

An immutable top-level
`let cancelled: Result<void, TaskError> = task.cancel(handle)` is the
structured consuming cancel-and-join operation. It requests cancellation,
waits for terminal child cleanup, then consumes and drops the handle. An
already-completed child returns `Ok(void)`; a child whose spawn was rejected
returns the same stable `queue_full` error. This overload is distinct from the
legacy synchronous `task.cancel(Task)` worker request.

## Implemented P1 Slice

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
- embedded structured child frames enqueued onto the same bounded FIFO, plus a
  single owner-local waiter edge that re-enqueues the parent when its child
  completes;
- a typed `TaskError { code: "queue_full", ... }` materialized by join when a
  structured spawn cannot enter the 64-entry ready queue;
- exactly-once transfer of a typed child result into the successful join
  payload before the child frame is dropped;
- a structured cancel-and-join suspension boundary that propagates
  cancellation through the child subtree, removes ready/timer registrations,
  waits for terminal cleanup, returns `Result<void, TaskError>`, and drops the
  consumed child frame;
- compiler-inserted scope cleanup on normal fallthrough and final `return` that
  cancels unjoined children, removes their ready-queue entries, disarms owned
  timers, and drops their frames before the next statement or return
  completion;
- owned Err/None propagation from a direct structured `?` binding, with live
  sibling cleanup before helper completion and parent wakeup;
- exact top-level local liveness across each yield or child call;
- per-field ownership bits for managed ARC/COW frame values;
- idempotent child-first frame drop that clears ownership before release.

This slice creates no OS thread, heap task, reactor, or atomic metadata. A
ready zero-duration timer neither registers nor enters the queue. A positive
timer is not polled again until its deadline moves the owner frame to the ready
queue. The generated context records poll, yield, frame-drop/live-frame,
enqueue/dequeue/saturation/cancellation, structured
spawn/join/join-suspension/cancellation, and timer
registration/expiry/cancellation/live/peak counters.
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
frame-safe call arguments evaluate exactly once from left to right. Shared
managed values are retained into the child frame, owned temporaries transfer
directly, and an owned result moves into its immutable caller binding before
the child frame is dropped.

That inline fast path describes ordinary direct suspend calls. A structured
spawn is intentionally concurrent: it evaluates immutable frame-safe arguments
once, initializes an embedded child frame, and schedules that frame on the
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
stable error and consume the inert browser handle.

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
tokens/deadlines, and reactor-backed I/O are later slices.

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
deadlines, channels, and select remain later slices. E0871, E0872, E0875, and
E0876 reject unsupported cases before code generation.

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

The P0/P1 controls and raw evidence format live in
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
