# Nomo MIR

`nomo_mir` is the typed control-flow and proof layer between checked compiler IR
and backend emission. It is intentionally backend-neutral: blocks contain
typed ownership/effect operations, terminators retain normal control flow, and
panic edges remain explicit.

The first release-only optimization tracks flat primitive arrays as
`Unique`, `Shared`, `Unknown`, or `Moved`. A COW detach is marked elidable only
when the fixed point proves `Unique` on every normal predecessor. The resulting
C99 store still performs its bounds check and element release/retain.

Safety fallbacks are part of the pass contract:

- nested or managed array elements are not optimized;
- aliases, aggregation, iteration, publication, calls, mutable borrows,
  resizing, and root reassignment break the proof;
- functions with suspend or defer behavior remain on the checked COW path;
- no detach is moved to a loop preheader or across index/RHS evaluation.

This is not a benchmark-specific layer. Project, package, function, source, and
content hashes are absent from its analysis.
