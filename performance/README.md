# Compiler release-gate baseline

`scripts/compiler_release_gate.py` measures a clean native build and repeated
checks using the release binary. CI uploads the JSON evidence from each run and
fails when it exceeds `release-gate-thresholds.json`.

These preview thresholds catch gross regressions while allowing normal hosted
runner variance. They are the pre-incremental baseline for RFC 0016, which will
add representative workspace traces and edit-to-diagnostic measurements.
