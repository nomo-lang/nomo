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
`yield_counter_probe` plus `timer_counter_probe`. Each probe executes outside
measured samples with `NOMO_ASYNC_METRICS_PATH` set, then validates the
versioned current-thread JSON contract against `counter-catalog.json`. The
two-frame, two-yield chain transfers one managed argument and result, then
requires zero heap/slab frame allocations, two idempotent frame drops, peak two
live frames, two queue round trips, five polls, and two cooperative yields. The
timer probe requires an inline
zero-duration ready path plus exactly one positive registration, expiry and
queue round trip, two polls, no cancellation, and zero live timers at exit.

All RFC-required async workloads already have manifest entries. Unsupported
workloads remain disabled with their implementation phase recorded, so missing
runtime coverage cannot look like a pass. Owner-local timer registration,
expiry, cancellation, live and peak-live counters are now available. ARC
primitive counters remain explicitly unavailable rather than being reported
as zero. Mutable/affine suspend parameters, complete unwind paths, structured
spawn/join, and the multi-task timer-wheel workload are not complete.

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
```

The harness rejects a mismatched Go patch, fewer than five measured runs,
different output bytes, unexpected stderr, non-zero async/atomic symbol counts,
incorrect generated-C symbol counts, unknown/negative/missing runtime counters,
failed builds, and dirty checkouts when `--require-clean` is used. Every result
records SHA-256 identities for the manifest, harness, counter catalog, sources,
Nomo/Go/C toolchains, and produced binaries. A requested metrics path that
cannot be opened fails with a generic message and never prints the path.

CI uploads raw P0 and P1 JSON instead of treating hosted-runner timing as a
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
