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
nonblocking `task.sleep(Duration) -> Result<void, TaskError>` API remains
Proposed until its argument/result lowering and owner-local timer runtime land.

## Implemented P1 Slice

On the native C99 backend, a suspend call chain that reaches
`task.yield_now()` emits:

- a stack-allocated root frame with an explicit state and embedded child frames;
- one poll/drop pair per actually suspending function;
- direct child polls whose `PENDING` result propagates to the root;
- an inline initial poll;
- a one-entry current-thread ready queue path entered only after `PENDING`;
- exact top-level local liveness across each yield or child call;
- per-field ownership bits for managed ARC/COW frame values;
- idempotent child-first frame drop that clears ownership before release.

This slice creates no OS thread, heap task, reactor, or atomic metadata. The
generated context records poll, yield, frame-drop/live-frame, enqueue, and
dequeue counters. Native programs export the versioned
`nomo-c99-current-thread` JSON payload only when
`NOMO_ASYNC_METRICS_PATH` is set; ordinary runs perform no metrics I/O.
The P1 benchmark collects that payload in a separate probe after measured runs.
ARC primitive counters and timers remain explicitly unavailable rather than
being reported as zero.
Locals that die before a suspension are released without entering the frame.
Immutable locals used after a suspension are moved into the frame; only those
referenced in a resumed segment are reintroduced as non-owning C aliases.
An embedded child is polled inline and does not allocate or enter the ready
queue when it completes synchronously. Normal completion and explicit early
root drop share the same child-first idempotent cleanup path.

Browser WASM accepts the same source in its bounded sandbox interpreter.
`task.yield_now()` is currently a cooperative boundary there; it does not yet
return control to a host Promise or browser event loop.

## Deliberate Restrictions

E0876 rejects unsupported suspension rather than miscompiling it. In this
slice, `task.yield_now()` and calls to actually suspending functions must be
standalone statements in non-generic, parameterless `suspend fn` functions
returning `void`. Immutable top-level scalar, string, struct, enum, and
supported array locals may live across a suspension when every transitive
value field is frame-safe. Mutable locals, borrows, guards, resource handles
or wrappers containing them, recursive suspend graphs, suspension in control
flow or expressions, arguments/results, `?`, explicit panic, timers,
spawn/join, cancellation, and reactor-backed I/O are later slices.

The existing `task.spawn` API remains the legacy isolated native-worker API.
It is not an async task constructor and still maps one worker to one native
thread. RFC 0032 requires its migration to a bounded, lazy blocking pool.

## Correctness and Cost Gates

This slice checks exact spills, pre-suspension cleanup for dead locals,
child-before-parent ownership-bit clearing, and repeated explicit drop under
generated-C tests and AddressSanitizer.
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
[`performance/async`](../performance/async/README.md). The runnable current
slice is [`examples/async_yield`](../examples/async_yield).
