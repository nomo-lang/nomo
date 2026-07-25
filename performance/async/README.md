# Async runtime benchmark harness

Language: English | [中文](README.zh-CN.md)

This directory implements the evidence contract from RFC 0034. It does not
claim that the P0 compiler is a production async runtime. The only enabled
workloads are:

- `sync_unused`, which compiles a synchronous program using strings, arrays,
  and the deterministic ordered map, then rejects async/thread/atomic symbols
  in generated C;
- `ready_call_control`, which runs the same deterministic calculation in an
  always-ready Nomo `suspend fn` chain and a pinned Go reference. It validates
  source, binary, output, toolchain, sampling, and result-schema plumbing, but
  is explicitly ineligible for a performance claim.

All RFC-required async workloads already have manifest entries. They remain
disabled with their implementation phase recorded, so missing runtime coverage
cannot look like a passing benchmark.

## Run

Build the Nomo CLI, use the exact Go patch named in `manifest.json`, and run:

```sh
cargo build --release --locked --bin nomo
python3 scripts/async_benchmark.py \
  --nomo target/release/nomo \
  --require-clean \
  --output performance/results/async-p0.json
```

The harness rejects a mismatched Go patch, fewer than five measured runs,
different output bytes, unexpected stderr, non-zero async/atomic symbol counts,
failed builds, and dirty checkouts when `--require-clean` is used. Every result
records SHA-256 identities for the manifest, harness, counter catalog, sources,
Nomo/Go/C toolchains, and produced binaries.

CI uploads raw P0 JSON instead of committing runner-specific timing as a stable
baseline. Controlled-host evidence must also set `NOMO_BENCH_POWER_MODE` and
enforce process affinity once those controls arrive with P1/P2. P0 records
per-process wall time, CPU time, and POSIX `wait4` peak RSS, but it does not yet
record steady RSS. These samples only test the harness pipeline, and no
Nomo-versus-Go ratio is calculated.

## Change control

Changing workload semantics, output bytes, build flags, measurement method, or
the result schema requires a schema/series version change. Do not replace a
missed target by changing the Go version, payload, safety checks, or sample
selection.
