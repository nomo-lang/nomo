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

## Implemented P1 Slice

On the native C99 backend, a suspend call chain that reaches
`task.yield_now()` or `task.sleep(...)` emits:

- a stack-allocated root frame with an explicit state and embedded child frames;
- one poll/drop pair per actually suspending function;
- direct child polls whose `PENDING` result propagates to the root;
- an inline initial poll;
- a one-entry current-thread ready queue path entered only after `PENDING`;
- a bounded owner-local timer table with generation-checked registrations,
  monotonic deadlines, deterministic deadline/generation ordering, and
  idempotent disarm;
- exact top-level local liveness across each yield or child call;
- per-field ownership bits for managed ARC/COW frame values;
- idempotent child-first frame drop that clears ownership before release.

This slice creates no OS thread, heap task, reactor, or atomic metadata. A
ready zero-duration timer neither registers nor enters the queue. A positive
timer is not polled again until its deadline moves the owner frame to the ready
queue. The generated context records poll, yield, frame-drop/live-frame,
enqueue/dequeue, and timer registration/expiry/cancellation/live/peak counters.
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
root drop share the same child-first idempotent cleanup path.

Browser WASM accepts the same source in its bounded sandbox interpreter.
`task.yield_now()` is currently a cooperative boundary there; it does not yet
return control to a host Promise or browser event loop. `task.sleep` does not
block or evaluate its duration in the browser sandbox; it returns
`TaskError { code: "runtime_unavailable", ... }`.

## Deliberate Restrictions

E0876 rejects unsupported suspension rather than miscompiling it. In this
slice, `task.yield_now()` and calls to parameterless actually suspending
functions must be standalone statements; `task.sleep(Duration)` must be the
initializer of an immutable `let` binding with `Result<void, TaskError>`. The
containing `suspend fn` remains non-generic, parameterless, and
`void`-returning. Immutable top-level scalar, string, struct, enum, Result, and
supported array locals may live across a suspension when every transitive
value field is frame-safe. Mutable locals, borrows, guards, resource handles
or wrappers containing them, recursive suspend graphs, suspension in control
flow or other expressions, general suspend function arguments/results, `?`,
explicit panic, spawn/join, cancellation, and reactor-backed I/O are later
slices.

The existing `task.spawn` API remains the legacy isolated native-worker API.
It is not an async task constructor and still maps one worker to one native
thread. RFC 0032 requires its migration to a bounded, lazy blocking pool.

## Correctness and Cost Gates

This slice checks exact spills, pre-suspension cleanup for dead locals,
child-before-parent ownership-bit clearing, repeated explicit drop, monotonic
no-early-wake behavior, zero-duration timer fast paths, and cancellation of an
armed child timer under generated-C tests and AddressSanitizer.
Later slices must still prove, rather than assume:

- exactly-once ARC/COW release on error, cancellation, timeout, and panic paths;
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
[`examples/async_timer`](../examples/async_timer).
