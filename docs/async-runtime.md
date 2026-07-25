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
- [RFC 0033](https://github.com/nomo-lang/rfcs/blob/main/en/rfcs/0033-concurrency-capabilities-and-shared-storage.md)
  defines transfer and sharing capabilities.
- [RFC 0034](https://github.com/nomo-lang/rfcs/blob/main/en/rfcs/0034-async-runtime-acceptance-gates.md)
  defines correctness, portability, memory, and performance gates.

[中文版本](async-runtime.zh-CN.md)

## Language Surface

Functions that may suspend use `suspend fn`. Calls remain direct-style:

```nomo
package app.main

import std.io
import std.task

suspend fn main() -> void {
    io.println("before")
    task.yield_now()
    io.println("after")
}
```

A normal `fn` cannot call a suspend function. The compiler reports E0870
instead of adding hidden runtime behavior. Merely declaring or calling an
always-ready suspend function does not create an executor.

## Implemented P1 Slice

On the native C99 backend, a root program with `task.yield_now()` emits:

- a stack-allocated root frame with an explicit state;
- a poll function that returns `READY` or `PENDING`;
- an inline initial poll;
- a one-entry current-thread ready queue path entered only after `PENDING`;
- an idempotent root-frame drop function.

This slice creates no OS thread, heap task, reactor, or atomic metadata. The
generated context records poll, yield, enqueue, and dequeue counters internally;
versioned benchmark export is deferred until the P1 counter contract is ready.

Browser WASM accepts the same source in its bounded sandbox interpreter.
`task.yield_now()` is currently a cooperative boundary there; it does not yet
return control to a host Promise or browser event loop.

## Deliberate Restrictions

E0876 rejects unsupported suspension rather than miscompiling it. In this
slice, `task.yield_now()` must be a standalone statement in a parameterless
root `suspend fn main() -> void`. Locals live across a yield, nested suspend
calls, suspension in control flow or expressions, non-void results, timers,
spawn/join, cancellation, and reactor-backed I/O are later slices.

The existing `task.spawn` API remains the legacy isolated native-worker API.
It is not an async task constructor and still maps one worker to one native
thread. RFC 0032 requires its migration to a bounded, lazy blocking pool.

## Correctness and Cost Gates

Later slices must prove, rather than assume:

- exact liveness spills and exactly-once ARC/COW release on completion, error,
  cancellation, timeout, and panic paths;
- no unsafe mutable borrow or guard crossing a suspension point;
- no runtime, thread, coroutine metadata, or atomic collection cost for
  programs that do not use suspension;
- no allocation or ready-queue operation on the synchronous-ready path;
- C99 and browser-WASM compatibility, followed by Linux, macOS/BSD, and
  Windows reactor coverage;
- fair, version-pinned Nomo-versus-Go measurements without weakening either
  workload.

The P0 controls and raw evidence format live in
[`performance/async`](../performance/async/README.md). The runnable current
slice is [`examples/async_yield`](../examples/async_yield).
