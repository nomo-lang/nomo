# Async runtime benchmark harness

Language: English | [中文](README.zh-CN.md)

This directory implements the evidence contract from RFC 0034. It does not
claim that the current P1 compiler is a production async runtime. The P0
manifest keeps these controls:

- `sync_unused`, which compiles a synchronous program using strings, arrays,
  and the deterministic ordered map, then rejects async/thread/atomic symbols
  in generated C;
- `ready_call_control`, which runs the same deterministic calculation in an
  always-ready Nomo `suspend fn` chain and a pinned Go reference. It validates
  source, binary, output, toolchain, sampling, and result-schema plumbing, but
  is explicitly ineligible for a performance claim.

The separate `manifest-p1.json` repeats both zero-cost controls and enables
`yield_counter_probe`, `timer_counter_probe`, `task_spawn_complete`,
`structured_cancel_probe`, `structured_return_cancel_probe`, and
`structured_question_cancel_probe`, plus
`structured_explicit_cancel_probe` and
`structured_deadline_probe`, `structured_panic_cleanup_probe`, and
`publication_move_probe`. Each counter
probe executes outside measured
samples with `NOMO_ASYNC_METRICS_PATH` set, then validates the versioned
current-thread JSON contract against `counter-catalog.json`. Panic is an
explicit expected-failure contract with exact stderr and exit status; it is
never accepted as an accidental command failure. The
two-frame, two-yield chain transfers one managed argument and result, then
requires zero heap/slab frame allocations, two idempotent frame drops, peak two
live frames, two queue round trips, five polls, and two cooperative yields. The
timer probe requires an inline
zero-duration ready path plus exactly one positive registration, expiry and
queue round trip, two polls, no cancellation, and zero live timers at exit.
The spawn/complete workload runs 32 scope-owned Nomo `Task<void>` children and
32 Go goroutines under the same pinned single-core configuration. It validates
exact spawn/join/join-suspension counters and frame/queue cleanup, but remains
claim-ineligible while the runtime is current-thread-only.

All RFC-required async workloads already have manifest entries. Unsupported
workloads remain disabled with their implementation phase recorded, so missing
runtime coverage cannot look like a pass. Owner-local timer registration,
expiry, cancellation, live and peak-live counters are now available. The
current-thread executor uses a bounded 64-entry FIFO and reports rejected
enqueue attempts through `ready_queue_saturations`; the multi-task saturation
workload additionally proves that the rejected spawn becomes a typed join
error. Cancelled queued entries and incomplete tasks are accounted separately
by `ready_queue_cancellations` and `task_cancellations`. Structured `Task<T>`
results now have generated-C, native, WASM-boundary,
post-join nested-scope return, and AddressSanitizer correctness coverage.
Scope cancellation additionally covers armed-timer and ready-queue cleanup,
plus typed final-return and `?` error-propagation paths that cancel before
root-frame wakeup, through enabled runtime-counter gates. The explicit
cancel-and-join gate consumes a scope-owned handle, disarms its live timer,
returns a typed success only after terminal cleanup, and preserves the
allocation-free current-thread fast path. The panic gate
preserves a managed child message, cancels an armed-timer sibling through the
root, drops all frames, exports counters, and only then exits with the original
panic. ARC primitive counters remain explicitly unavailable rather than being
reported as zero. Mutable/affine suspend parameters, non-final return, `?` in
other positions, nested-expression or runtime-originated panic unwind,
cancellation propagation, and the multi-task timer-wheel workload are not
complete.
The deadline gate arms a deadline and a longer child sleep on the same
owner-local table, requires timeout to win, cancels the sleep registration,
materializes a typed join error, and validates the three deadline-specific
counters without adding frame allocation or atomic symbols.
The publication-move gate transfers one managed aggregate with nested COW
storage into a structured child, requires exactly one `publication_moves`
event, and rejects generated retain, thread, atomic, and heap-frame evidence.

`manifest-p3.json` adds the current-thread bounded-channel gate. A capacity-eight
Nomo ring and a pinned `GOMAXPROCS=1` Go channel each transfer 32 `u64` values.
The Nomo probe requires exact buffered/direct-handoff, suspension, wakeup,
close, and live/peak buffer/waiter counters. It also repeats `sync_unused` and
the async yield probe with `nomo_channel_` forbidden, proving that code which
does not construct a channel carries no channel storage or metadata. The
Nomo/Go samples are evidence only and do not authorize a performance claim.

## Run

Build the Nomo CLI, use the exact Go patch named in `manifest.json`, and run:

```sh
cargo build --release --locked --bin nomo
python3 scripts/async_benchmark.py \
  --nomo target/release/nomo \
  --require-clean \
  --output performance/results/async-p0.json
python3 scripts/async_benchmark.py \
  --nomo target/release/nomo \
  --manifest performance/async/manifest-p1.json \
  --require-clean \
  --output performance/results/async-p1.json
python3 scripts/async_benchmark.py \
  --nomo target/release/nomo \
  --manifest performance/async/manifest-p3.json \
  --require-clean \
  --output performance/results/async-p3.json
```

The harness rejects a mismatched Go patch, fewer than five measured runs,
different output bytes, unexpected stderr, non-zero async/atomic symbol counts,
incorrect generated-C symbol counts, unknown/negative/missing runtime counters,
failed builds, and dirty checkouts when `--require-clean` is used. Every result
records SHA-256 identities for the manifest, harness, counter catalog, sources,
Nomo/Go/C toolchains, and produced binaries. A requested metrics path that
cannot be opened fails with a generic message and never prints the path.

CI uploads raw P0, P1, and P3 JSON instead of treating hosted-runner timing as a
stable baseline. Controlled-host evidence must also set
`NOMO_BENCH_POWER_MODE` and enforce process affinity once that control lands.
The current samples record per-process wall time, CPU time, and POSIX `wait4`
peak RSS, but not steady RSS. They test harness and counter plumbing only; no
Nomo-versus-Go ratio is calculated.

## Change control

Changing workload semantics, output bytes, build flags, measurement method, or
the result schema requires a schema/series version change. Do not replace a
missed target by changing the Go version, payload, safety checks, or sample
selection.
