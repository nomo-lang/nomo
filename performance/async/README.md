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
`yield_counter_probe`. The probe executes outside measured samples with
`NOMO_ASYNC_METRICS_PATH` set, then validates the versioned current-thread JSON
contract against `counter-catalog.json`. For a two-frame, two-yield chain it
requires zero heap/slab frame allocations, two idempotent frame drops, peak
two live frames, two queue round trips, five polls, and two cooperative yields.

All RFC-required async workloads already have manifest entries. Unsupported
workloads remain disabled with their implementation phase recorded, so missing
runtime coverage cannot look like a pass. ARC primitive counters and timers
remain explicitly unavailable in the P1 payload rather than being reported as
zero. Argument/result frames, complete unwind paths, structured spawn/join, and
timers are not complete.

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
