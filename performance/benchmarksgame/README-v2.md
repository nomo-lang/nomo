# RFC 0043 Benchmarks Game parity harness v2

This is the measurement-authority slice of RFC 0043. It lives beside the
unchanged v1 CPU baseline and does not implement compiler optimization or a
public CLI capability. The ISO C++20 allocation clarification is frozen at RFC
merge `75a7e14adc1ea06ccdc9a28c1dc0676ce8404a1c`; it changes no workload,
input, statistic, threshold, or comparator role.

The v1 contract remains `manifest.json`, `schema/result.schema.json`, and
`scripts/benchmarksgame.py`. V2 uses `manifest-v2.json`,
`schema/result-v2.schema.json`, and `scripts/benchmarksgame_v2.py`. A v1
artifact keeps its original 12-run minimum-wall interpretation and is never
reinterpreted as v2.

## Frozen suite and two build modes

Formal inputs remain spectral-norm 5500, n-body 50000000, and
fannkuch-redux 12. The manifest locks every Nomo, official C, Go, C++20,
semantic-C, fixture, and predecessor SHA-256.

Each of two independent formal build modes has the same five timed lanes:

- candidate Nomo at an exact commit;
- main Nomo at the exact current `origin/main` commit;
- frozen official C gcc `#8`;
- BSD-derived ISO C++20;
- frozen official Go `#8` with `GOMAXPROCS=1`.

The `release` mode uses the real `nomo build <project> --release` binary. That
command must also emit `build/release-provenance.json`: a machine-readable,
complete backend record whose Clang realpath, SHA-256, version, target triple,
C99 compile argv, link argv, generated-C SHA, object SHA, and final binary SHA
match the harness-selected toolchain and produced files. Missing or
unverifiable metadata makes the release lane unavailable. The `emit-c` mode
separately runs `nomo build <project> --emit-c`, hashes the
unmodified generated C, and compiles it with
`clang -std=c99 -O3 -DNDEBUG -fomit-frame-pointer` plus `-lm` where required.
It never reuses a release artifact. Each mode has its own correctness gate,
warmups, 30-block schedules, batches, statistics, and verdict. Samples are
never pooled across modes, and both modes must pass before the suite can pass.

The optimizer-facing `release-provenance.json` contract includes build
environment provenance, not only argv. Every backend `compile_commands[]`
entry and `link_command` must satisfy
`schema/result-v2.schema.json#/$defs/command`, and its `command.environment`
must exactly equal the harness canonical sanitized projection produced by
`sanitized_build_environment`: `retained`, `cleared`, and
`cleared_values_recorded` must match, compiler-affecting names from
`COMPILER_AFFECTING_ENVIRONMENT` must be cleared without recording their
values, and no override is permitted for backend compile/link commands.
Only the separately validated compiler self-build may use the explicit
`CARGO_TARGET_DIR` override. A backend record with a missing, self-reported,
or differently sanitized environment is unavailable even when compiler,
argv, and hashes otherwise look correct.

C and C++ are absolute parity comparators. Main is the regression comparator.
Go remains diagnostic. Semantic-C is an untimed correctness-only diagnostic
control and never enters a schedule or verdict.

## ISO C++20 allocation mapping

Every C++ source is compiled with
`clang++ -std=c++20 -pedantic-errors -O3 -DNDEBUG -fomit-frame-pointer`,
so a C++ language extension is a hard error.

Spectral-norm and fannkuch-redux map each runtime C VLA to one contiguous
`std::unique_ptr<T[]>` RAII dynamic array. The stack-to-dynamic difference is
explicit. Array count, element type, logical capacity, lexical lifetime,
per-call allocation frequency, initialization work, and access order remain
aligned with C #8. Each dynamic array is constructed once at final capacity
and cannot grow or reallocate. N-body uses `std::array` for the corresponding
fixed stack array. No reference uses a custom allocator, stronger algorithm,
precomputation, thread, or SIMD path. See `SOURCES-v2.md`.

## CI-safe correctness

Install the pinned Draft 2020-12 validator, build the repository driver, and
run the small-input gate:

```sh
python3 -m pip install -r scripts/requirements-benchmarksgame-v2.txt
cargo build --locked --release --bin nomo
python3 scripts/benchmarksgame_v2.py \
  --mode correctness \
  --nomo target/release/nomo \
  --require-clean \
  --output performance/results/benchmarksgame-v2/correctness.json
```

This historical correctness-only path compares frozen Nomo emit-C, C, strict
C++20, semantic-C, and Go output. It cannot produce a performance verdict.
Linux, macOS, and Windows CI run it without timing thresholds; Windows also
executes all five implementations through the Job Object collector. Windows
uses `PROC_THREAD_ATTRIBUTE_JOB_LIST` with
`EXTENDED_STARTUPINFO_PRESENT` so the suspended child is atomically born in
the `KILL_ON_JOB_CLOSE` Job before its primary thread can run; there is no
post-launch assignment race or unowned suspended-child fallback. Fixtures are
LF. Collectors apply only `CRLF -> LF` to captured stdout, retain both raw and
normalized SHA-256 values plus the rule identifier, and perform no other
whitespace normalization.

## Formal invocation and provenance

Candidate and main must be separate clean detached Git checkouts at distinct
full commit SHAs of the official `nomo-lang/nomo` origin. Candidate must be
advertised by that origin; main must match both local and remote `origin/main`.
Fake/local origins are rejected. Formal measurement accepts only the canonical
checked-in manifest at its code-locked digest from a clean authority checkout;
an external suite root cannot replace a frozen source. The
harness builds each Nomo driver itself with
`cargo build --locked --release --bin nomo` in a fresh isolated target
directory, confirms the checkout remained clean at the exact commit, hashes
the driver, then probes `nomo build --help` for `--release` and `--emit-c`.
Arbitrary prebuilt compiler paths are not accepted.

Formal work is deliberately two-stage. First, `prepare` performs every
compiler and benchmark build once, freezes a content-addressed immutable
contract, and writes the exact qualification request. The bundle is not
physically made read-only by this script, so it must not be modified after
preparation; any content change is rejected. The result output must be outside
the bundle:

```sh
python3 scripts/benchmarksgame_v2.py \
  --mode prepare \
  --nomo target/release/nomo \
  --candidate-checkout /absolute/candidate-checkout \
  --candidate-commit 0123456789abcdef0123456789abcdef01234567 \
  --main-checkout /absolute/main-checkout \
  --main-commit 89abcdef0123456789abcdef0123456789abcdef \
  --cargo cargo \
  --prepared-bundle /absolute/benchmarksgame-v2-prepared \
  --require-clean \
  --output /absolute/results/benchmarksgame-v2-prepared-result.json
```

Successful preparation exits zero with status `prepared` and prints the
bundle digest plus:

```text
/absolute/benchmarksgame-v2-prepared/qualification-request.json
```

The metadata `prepared_at_utc` timestamp is part of the bundle digest. Even a
different otherwise-valid UTC timestamp invalidates the prepared bundle.

An independent benchmark authority reviews that exact request and bundle,
then creates `/absolute/authority/environment.json` using
`environment-qualification.example.json`. Its `bindings`, `dynamic_policy`,
and unique `required_checks` must exactly match the request; each required
check needs qualified evidence. Changing the bundle, request, source, command,
compiler, or binary invalidates the approval.

Only then may measurement consume the existing bundle. It never rebuilds or
overwrites prepared content, and its output must again be outside the bundle:

```sh
python3 scripts/benchmarksgame_v2.py \
  --mode measure \
  --nomo target/release/nomo \
  --prepared-bundle /absolute/benchmarksgame-v2-prepared \
  --environment-qualification /absolute/authority/environment.json \
  --require-clean \
  --output /absolute/results/benchmarksgame-v2-formal.json
```

Before executing a single benchmark, `measure` revalidates the full prepared
Draft schema and semantic authority, canonical request, bundle inventory,
live file SHA-256 values, official checkouts, and all executable paths. Every
candidate, main, C, C++20, semantic-C, and Go binary must remain inside the
bundle. A failed preflight executes neither a build nor a collector.

Qualification JSON is static authorization only. Its complete, unique RFC
check set and evidence hashes bind the actual canonical host, reference
toolchains, complete frozen source lock, exact commits, both self-built Nomo
hashes, and prepared bundle digest. It cannot self-certify measurement-time
state. Every batch
attempt independently captures before/after power, frequency/governor,
thermal, load, swap, and affinity observations from system APIs or commands.
Snapshots carry fresh timestamps, monotonic counters, host bindings, and
canonical digests; snapshots cannot be reused between attempts or build modes.
An anomaly invalidates only that batch and its complete raw evidence remains.
Dynamic command evidence uses a controlled locale and canonical PATH, and the
authority additionally locks Darwin commands to `/usr/bin/pmset`,
`/usr/bin/osascript`, and `/usr/sbin/sysctl`, and Windows power inspection to
the real `System32/powercfg.exe`; a PATH-shadowed replacement is rejected even
when it has a self-consistent hash. On the canonical Apple Silicon host,
frequency/governor is recorded as `not-applicable` under RFC 0043's
“where applicable” rule. The authority accepts only the exact, unique current
three-`Note:` `pmset -g therm` shape saying that thermal warning, performance
warning, and CPU power status have not been published. Those lines do not
imply a fabricated percentage: numeric, partial, duplicate, unknown, warning,
or error output is ineligible until a real sample and versioned grammar are
approved. Foundation `NSProcessInfo.thermalState` through the pinned system
`osascript` is decisive and only state `0` (`nominal`) qualifies. Because
unknown or unsupported Foundation states may map to nominal, eligibility also
requires the bound Darwin/arm64 host identity and exact three-line `pmset`
corroboration. The authority does not claim to observe DVFS directly;
paired/interleaved references plus drift and paired-ratio RSD gates mitigate
unobservable DVFS variation.

Timed processes receive a constructed runtime environment, never
`os.environ.copy()`. POSIX retains only the canonical locale, PATH, and temp
directory; Windows retains only canonical locale, WinAPI-derived
`SystemRoot`/`WINDIR`, and temp directories. Dynamic-loader, library-path,
allocator, Go tuning, and `NOMO_*` parent variables are absent from both the
actual child environment and recorded artifact. Go is the sole exception and
adds exactly `GOMAXPROCS=1`; semantic validation recomputes the complete
runtime environment for every successful or failed sample.

The result records complete argument vectors and rendered commands,
stdout/stderr and raw/normalized stdout hashes,
toolchain versions, source/generated-C/binary hashes, repository state,
compiler-build provenance, host facts, collector identity, qualification,
schedule, raw samples, stability calculations, gates, and verdicts. The
runner validates every result against the checked-in Draft 2020-12 schema and
then recomputes drift, RSD, confidence bounds, gates, and verdicts from raw
samples.

## Schedule, statistics, stability, and verdicts

For each workload and build mode, every lane gets two process warmups followed
by 30 paired blocks. The frozen five-treatment Williams design has ten rows
repeated three times: every lane occurs six times in every position, and each
directed adjacent pair occurs six times. Every output is captured and checked.
Warmups are not samples; compilation is excluded; outliers are never removed.

For comparator `q` and block `i`:

```text
x[w,q,i] = ln(candidate_wall[w,i] / comparator_wall[q,w,i])
R[w,q]   = exp(mean(x))
U99[w,q] = exp(mean(x) + 2.462021360150384 * stdev(x) / sqrt(30))
```

Suite block values are the equal-weight mean of the three workload log ratios.
Only a ratio below 1.0 means candidate Nomo was faster. Inequalities are
inclusive:

| Gate | Required result |
| --- | --- |
| Each workload vs C | `U99 <= 1.05` |
| Each workload vs C++20 | `U99 <= 1.05` |
| Suite vs C | `R <= 1.00` and `U99 <= 1.03` |
| Suite vs C++20 | `R <= 1.00` and `U99 <= 1.03` |
| Each workload vs main | `U99 <= 1.03` |
| Suite vs main | `U99 <= 1.02` |

C or C++ first-half/second-half geometric-mean drift above 2%, or
candidate/C, candidate/C++, or candidate/main paired-ratio RSD above 3%,
invalidates the complete batch. Exactly 2% and 3% pass. Go drift is retained
as a diagnostic warning and never invalidates parity. A timeout, output
failure, collector failure, or other anomaly also invalidates the batch.
Samples already collected for a failing workload are retained incrementally
with the exact reason. Complete stability statistics remain in the artifact
when an after-snapshot invalidates an otherwise complete batch. Rejected
artifacts are retained. At most one automatic rerun is allowed; a
second anomaly terminates that mode as ineligible.

Two independent stable batches in each mode must pass every gate independently.
No samples are merged. Any unavailable, ineligible, or failing mode prevents a
suite parity pass.

## Scope

A future pass would apply only to these frozen single-thread scalar workloads,
commands, toolchains, protocol, and qualified canonical host. It is not a
general Nomo-versus-C/C++/Go claim and says nothing about async, I/O,
concurrency, packaging, production readiness, or stable release status. Shared
CI checks source locks, builds, correctness, schemas, collectors, statistics,
and provenance but never imposes wall-time thresholds.
