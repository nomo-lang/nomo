# Compiler release-gate baseline

`scripts/compiler_release_gate.py` measures a clean native build and repeated
checks using the release binary. CI uploads the JSON evidence from each run and
fails when it exceeds `release-gate-thresholds.json`.

These preview thresholds catch gross regressions while allowing normal hosted
runner variance. They are the pre-incremental baseline for RFC 0016, which will
add representative workspace traces and edit-to-diagnostic measurements.

The versioned async-runtime evidence contract lives in
[`performance/async/`](async/README.md), with a
[Chinese guide](async/README.zh-CN.md). P0 validates the zero-cost and harness
pipeline only; it makes no Nomo-versus-Go performance claim.

The [Benchmarks Game CPU baseline](benchmarksgame/README.md) provides three
naive, single-thread scalar Nomo/C/Go comparisons with correctness fixtures,
full command and artifact provenance, and an exploratory-only local result
contract.
