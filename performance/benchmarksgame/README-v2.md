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
Git attributes force the complete benchmark source, fixture, manifest, schema,
documentation, and runner authority surface to LF bytes on every checkout;
source hashes are never silently newline-normalized by the harness.

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
`clang --no-default-config -std=c99 -O3 -DNDEBUG -fomit-frame-pointer` plus
`-lm` where required. Replay derives that link-library contract from the
artifact's already-bound build host, never from the reviewer process, so an
unchanged Linux/macOS command remains valid when audited on Windows and vice
versa.
The regression authority exercises five artifact states—correctness success,
correctness failure, formal unavailable, prepared, and completed—from each of
the Linux, macOS, and Windows producer contracts on each of the three reviewer
platforms. All 45 offline replays must pass without reading reviewer paths,
tools, environment variables, repositories, or processes.
It never reuses a release artifact. Each mode has its own correctness gate,
warmups, 30-block schedules, batches, statistics, and verdict. Samples are
never pooled across modes, and both modes must pass before the suite can pass.
For every workload, candidate and main each receive separate release and
emit-C project copies. Each project record binds the frozen original Nomo
source and `nomo.toml`, their bundle-local copies, the exact compiler checkout
and commit, and the build-command working directory. Reference binaries, all
four Nomo projects, generated-C files, release objects, and final binaries
must have distinct lexical and filesystem identities; nested projects,
cross-lane paths, hard links, and synchronized sample rebinding are rejected.
The embedded release backend generated-C/binary records must exactly equal the
outer formal records, and the strict-JSON sidecar must exactly equal the
embedded backend object.

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
`CARGO_TARGET_DIR`, isolated `CARGO_HOME`, and authority-selected `RUSTC`
overrides. The isolated Cargo home is empty at build start and removed after
the build; every Cargo config on the checkout-to-filesystem-root search path
is rejected unless it is a tracked file inside the exact checkout and its path
and SHA-256 are recorded. Tracked config may not replace rustc or install a
rustc wrapper; configs are parsed as TOML, so table, dotted, and quoted
`build.rustc`, `rustc-wrapper`, and `rustc-workspace-wrapper` keys are all
rejected. The harness records the absolute Cargo proxy used to select the
toolchain, executes the absolute sibling `rustup` invocation path without
resolving away its multicall basename, records its realpath and hash, and runs
`rustup which cargo` plus `rustup which rustc` in each exact checkout. The
self-build then directly executes the selected `<sysroot>/bin/cargo` with
`RUSTC=<sysroot>/bin/rustc`; it never executes a rustup multicall proxy as the
compiler. Compiler-build authority records and revalidates actual Cargo and
rustc hashes, Cargo version, `rustc -vV` commit fields, sysroot, toolchain, and
every `rustc_driver` artifact hash. Candidate and main must resolve to the
same complete Rust toolchain identity.
Go builds set `GOENV=off` and clear `GOFLAGS`, so `$HOME/go/env` and a parent
`GOENV` cannot alter the build. Every Go compilation receives fresh,
bundle-local `GOCACHE` and `GOMODCACHE` directories that must not exist before
the build and are removed afterward. Their absolute paths are part of the
recorded sanitized command environment and are recomputed from the locked
copied-source directory by the validator; parent cache and `LocalAppData`
values are never restored. CI passes the setup-go-selected absolute Go
invocation path; provenance additionally binds its realpath, SHA-256, exact
patch version, and `GOROOT`, while the generic canonical PATH remains minimal.
`DEVELOPER_DIR` and `TOOLCHAINS` are cleared
and recorded as cleared for every probe and build. On Darwin,
`/usr/bin/xcrun --find clang` and `clang++` run in that same sanitized environment; the
invocation shim identity and selected compiler path, realpath, SHA-256,
version, target, and probe commands are frozen and revalidated before formal
measurement. `/usr/bin/xcrun --sdk macosx --show-sdk-path` independently
selects the system SDK from a trusted root; its path, SDK settings hash, and
command are bound, and that constructed `SDKROOT` is used instead of any
parent value so Rust C dependencies and benchmark Clang builds see the same
SDK authority. Every Clang and Clang++ probe, reference
compile, generated-C compile, and release-backend compile/link command includes
exactly one `--no-default-config`; explicit or default driver configs are
therefore outside the contract. A backend record with a missing, self-reported,
or differently sanitized environment is unavailable even when compiler,
argv, and hashes otherwise look correct.

On Windows, build commands do not inherit `INCLUDE`, `LIB`, `LIBPATH`, or
compiler flags from the parent process. The authority locates the fixed Visual
Studio Installer `vswhere.exe` from WinAPI Known Folder roots and runs
`-all` from a constructed environment containing only API-derived system,
Program Files, ProgramData, temp, COMSPEC, PATHEXT, and architecture values.
It retains raw stdout/stderr bytes and hashes, filters for complete,
launchable, non-prerelease Visual Studio 2022 VC candidates, and selects by
numeric installation version descending, install date descending, then
canonical path, failing on an ambiguous top record. The chosen JSON,
`VsDevCmd.bat`, `cl.exe`, and `link.exe` hashes are recorded. The authority
also binds the one trusted Program Files LLVM directory and the exact
`clang.exe`/`clang++.exe` paths, realpaths, and hashes. That directory is in
the reduced PATH used by self-build, emit-C, and future release backend
commands, so Nomo's internal `Command::new("clang")` resolves to the same
driver as the reference compiler. INCLUDE, LIB, LIBPATH, and the reduced
build PATH treat `VsDevCmd` output only as candidates: entries are retained
only when they are needed by this C99/C++20 suite and lie under the selected
Visual Studio VC or Windows 10 SDK/UCRT roots. Excluded PATH, INCLUDE, LIB,
and LIBPATH entries (including .NET/NETFX metadata and reference paths) are
recorded with their reason; LIBPATH may be empty. The final PATH then adds
only the bound LLVM directory and the WinAPI system directory. Poisoned
parent variables never enter the command. The authority hashes representative
VC/UCRT/SDK headers and libraries (`vcruntime.h`, `ctype.h`, `windows.h`,
`sdkddkver.h`, `libcmt.lib`, `ucrt.lib`, and `kernel32.lib`) in addition to
recording the declared versions. Validation repeats discovery before
accepting evidence. Native Windows CI verifies that the release-backend
environment resolves the bound bare Clang, compiles and runs C99 and ISO
C++20 SDK-header probes, and runs a normal Nomo project build through that
same PATH before the five-lane correctness gate; the full release command
remains unavailable until the public `nomo build --release` contract exists.

C and C++ are absolute parity comparators. Main is the regression comparator.
Go remains diagnostic. Semantic-C is an untimed correctness-only diagnostic
control and never enters a schedule or verdict.

## ISO C++20 allocation mapping

Every C++ source is compiled with
`clang++ --no-default-config -std=c++20 -pedantic-errors -O3 -DNDEBUG -fomit-frame-pointer`,
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

Reference, baseline Nomo emit-C, formal candidate/main release, formal emit-C,
and generated-C Clang preflight failures are retained as strict
`build_failures` evidence. This includes both observable command failure and a
command that exits zero without producing each exact expected binary,
generated-C file, or mandatory release-provenance file. An absent target is
recorded as `missing-output`; a directory, symlink, FIFO, or other non-regular
target is `invalid-output`. Both retain the successful command record, expected
lexical path, stdout/stderr, lane and phase, source identity, and a
non-following `lstat` classification. A formal build failure produces an
`unavailable` prepare result and log while retaining the already-created bundle
evidence; it never becomes a prepared or measurable bundle. The validator
replays that live classification exactly, so a symlink to a regular file can
never masquerade as a compiler-created binary. CI uploads the result JSON and
its sidecar evidence log with `if: always()`.

Every invocation requires a new result path and, for correctness/prepare, a
new build bundle. The harness never overwrites an earlier result, log, project,
generated C file, or binary. Immediately before every C, C++, Go, semantic-C,
Nomo release, Nomo emit-C, and generated-C Clang command, its exact output
targets must be absent; afterward they must be newly present as regular files
before their SHA-256 identities can enter provenance. A same-`--output` rerun
fails before repository/toolchain inspection or process execution and preserves
the prior evidence.

`--output` must end in the exact `.json` extension. Its sidecar is derived by
replacing that extension with `.log`; the default correctness/prepare build
bundle is the sibling path obtained by removing `.json`. An explicit prepared
bundle follows the same separation rule. Before creating a directory or
starting a process, the harness rejects equal or ancestor/descendant
relationships among the result, sidecar, and independent bundle paths using
the actual target filesystem's case-sensitivity. It preserves the caller's
lexical paths, checks every final component and parent with non-following
`lstat`, and rejects dangling links, junctions, linked parents, non-directory
parents, or any existing creation target before canonical containment checks.
A filesystem identity comparison normalizes every path component to Unicode
NFC before applying the target filesystem's case-folding rule, so canonically
equivalent composed/decomposed names cannot alias an output, sidecar, or bundle
only after work has started. On Windows, an existing target itself, or
otherwise its nearest existing lexical prefix, is also expanded through
`GetLongPathNameW` before comparison. This prevents either an 8.3 short-name
parent or a short final bundle component from disguising an output inside a
prepared bundle.
A transient same-directory case probe is removed before the authority or any
build process runs. The harness checks that every newly created path is absent
and reserves
`prepared-bundle.json` plus `qualification-request.json` only as fixed children
of a new prepared bundle.

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
  --cargo /absolute/rustup-proxy/bin/cargo \
  --go /absolute/setup-go/bin/go \
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
The recursive payload inventory excludes only the two root control files
`prepared-bundle.json` and `qualification-request.json` to avoid a
self-referential file hash. Their complete canonical contracts, including the
timestamp, exact prepared result, bundle binding, dynamic policy, and ordered
required checks, are nevertheless inputs to the bundle authority digest.
Every payload inventory entry binds its SHA-256, regular-file type, and a link
count of exactly one. Preparation and measurement reject symlinks, Windows
junctions, non-regular payloads, hardlinks, external lexical aliases, and any
linked component between a formal project or decisive file and the bundle
root before resolving a path.
Both control files must be byte-for-byte canonical JSON. Duplicate JSON keys
are rejected for the manifest, result, prepared metadata, qualification
request, environment authorization, and release sidecars.

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
  --go /absolute/setup-go/bin/go \
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

Downloaded correctness results, completed formal results, and complete
prepared bundles can be replayed without executing a compiler, probing a
reviewer toolchain, consulting the reviewer build environment, or requiring
the original absolute checkout paths:

```sh
python3 scripts/benchmarksgame_v2.py \
  --mode validate \
  --manifest performance/benchmarksgame/manifest-v2.json \
  --artifact /absolute/downloaded-result.json

python3 scripts/benchmarksgame_v2.py \
  --mode validate \
  --manifest performance/benchmarksgame/manifest-v2.json \
  --artifact /absolute/downloaded-prepared-bundle
```

Offline replay derives paths, build environments, host link flags, compilers,
commands, and targets from the artifact's authenticated host/toolchain/source
authority. For a completed result it also reconstructs the canonical static
authorization document from the embedded checks and bindings and requires its
SHA-256 to match the recorded independent qualification file. Live `prepare`
and `measure` retain the separate official-origin, checkout, toolchain,
authorization-file, and prepared-file revalidation gates.

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
`/usr/bin/osascript`, and `/usr/sbin/sysctl`; a PATH-shadowed replacement is
rejected even when it has a self-consistent hash. Windows observations use
exact WinAPI authorities: `GetSystemPowerStatus` for AC and battery-saver
state, `CallNtPowerInformation` for processor thermal-limit and instantaneous
system-idleness evidence, `EnumPageFilesW` for current pagefile use, and
`GetProcessAffinityMask` for the process affinity mask. Their complete raw
fields are hashed and re-parsed during offline replay. The
`powercfg /getactivescheme` command is not accepted as proof of AC power or
battery-saver state.
On the canonical Apple Silicon host,
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
runtime environment for every successful or failed sample. The canonical
default and Go runtime projections are recorded in the stable toolchain
identity before preparation, bound by the qualification request, and reused
for offline replay; a reviewer machine's PATH, temp directory, or platform is
never substituted into downloaded evidence.

The result records complete argument vectors and rendered commands,
stdout/stderr and replayable raw stdout bytes plus raw/normalized hashes,
toolchain versions, source/generated-C/binary hashes, repository state,
compiler-build provenance, host facts, collector identity, qualification,
schedule, raw samples, stability calculations, gates, and verdicts. The
correctness baseline reference record has exactly six build commands: C, ISO
C++20, semantic-C, Go, Nomo `--emit-c`, and the fixed Clang compilation of
that unmodified generated C. The two Nomo baseline commands are mandatory
whenever any baseline generated-C, source, binary, or command evidence is
present; the emit command is pinned to the bound Nomo tool and copied project,
and the Clang command is pinned to the bound compiler, generated C, output
binary, fixed flags, and host-derived link libraries. Formal reference-only
records have exactly the first four commands. Missing commands and unrecorded
extra commands are rejected by both the schema and semantic replay. Reference
source, copied-source, compiler-output, and binary key sets are exact, and all
reference command working directories are bound to the recorded producer
repository root. Each formal slot must repeat its enclosing lane, complete
clean repository record, and exact Nomo path and SHA. Release generated C has
only the `unmodified_after_build` marker; emit-C generated C has only
`unmodified_after_emit`. Missing or cross-mode markers are invalid.
The collector descriptor is an exact host-derived contract: POSIX requires
`wait4` process-group CPU/RSS accounting and Windows requires the Job Object
collector. Wall/CPU/RSS units and implementation version are fixed; each
sample collector id, CPU total, and stdout hashes are recomputed from that
descriptor and raw evidence. The runner validates every result against the
checked-in Draft 2020-12 schema and
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
