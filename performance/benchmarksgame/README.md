# Nomo Benchmarks Game CPU baseline

This is a reproducible, exploratory baseline for three naive, single-thread,
scalar Benchmarks Game `#8` programs:

| Workload | Correctness input | Formal input |
| --- | ---: | ---: |
| spectral-norm | 100 | 5500 |
| n-body | 1000 | 50000000 |
| fannkuch-redux | 7 | 12 |

The suite compares a pure Nomo transliteration, the official C gcc `#8`
source, and the official Go `#8` source. It evaluates only single-thread CPU
execution, current `Array`/copy-on-write behavior, floating-point work, and
the current C99 code generator. It does not support claims about asynchronous
I/O, concurrency, the whole language, or one language being generally faster
than another. It is not a Benchmarks Game submission.

## Run the gates

Build the current repository compiler first:

```sh
cargo build --locked --release --bin nomo
```

Run the CI-safe small-input gate:

```sh
python3 scripts/benchmarksgame.py \
  --mode correctness \
  --nomo target/release/nomo \
  --require-clean \
  --output performance/results/benchmarksgame/correctness.json
```

Run the formal local measurement only on an otherwise idle machine:

```sh
python3 scripts/benchmarksgame.py \
  --mode measure \
  --nomo target/release/nomo \
  --require-clean \
  --output performance/results/benchmarksgame/local-formal.json
```

The runner rejects a Nomo or Go version mismatch and a non-Clang C compiler.
It never silently changes compilers or flags. Results, generated C, source
snapshots, and binaries are written below the ignored
`performance/results/benchmarksgame/` directory. Compilation time is recorded
separately and is never counted as run time.

## Fair comparison contract

For Nomo, the runner executes `nomo build <project> --emit-c`, preserves the
emitted `main.c` without editing it, and compiles it with:

```text
clang -std=c99 -O3 -DNDEBUG -fomit-frame-pointer <generated.c> -o <binary>
```

The official C source uses the same Clang executable and flags. Spectral-norm
and n-body add `-lm` to both Nomo and C link commands. Go uses a normal
optimized `go build`; each Go run has `GOMAXPROCS=1`. Every actual build and
run command is stored as an argument vector and a shell-rendered command in
the result.

Before formal timing, all nine small-input Nomo/C/Go outputs must exactly match
the versioned fixtures. Each formal implementation's first run captures and
verifies the formal output and remains eligible for minimum wall time. There
is no in-process or extra process warmup. Later stdout goes to `/dev/null`.

The runner performs 12 rounds. For each workload, it seeds `random.Random`
with `20260727 + workload_index` and independently shuffles Nomo/C/Go in each
round. It reports:

- all raw samples;
- minimum, median, and inclusive-quartile IQR wall time across 12 runs;
- mean CPU time and two-sided 95% Student-t confidence interval from runs
  2 through 12;
- every per-run peak RSS and the median and maximum;
- `relative_time_vs_go = nomo_min_wall / go_min_wall`;
- `relative_time_vs_c = nomo_min_wall / c_min_wall`.

Only a relative value below `1.0` means Nomo was faster for that workload.
If an implementation's first formal run reaches 10 minutes, it is explicitly
downgraded to one sample and twelve-run statistics remain null. Every run has
a one-hour hard timeout.

Local results always declare:

```text
exploratory=true
affinity_enforced=false
claim_eligible=false
```

They are not directly comparable with old Benchmarks Game Linux measurements:
the operating system, architecture, CPU, compiler versions, isolation,
cache/swap handling, affinity, and measurement container differ.

## Result contract

`schema/result.schema.json` describes the stored JSON. In addition to the
statistics above, provenance includes the Nomo commit and dirty state,
manifest SHA, generated C SHA, every source and benchmark-binary SHA, complete
build commands, Clang/Go/Nomo versions, and OS/architecture/CPU/core counts.
The runner validates the same essential invariants before writing the result.

## Ten-workload readiness matrix

| Workload | Status | Reason |
| --- | --- | --- |
| spectral-norm | Implemented now | Current scalar floating-point and Array/COW facilities are sufficient. |
| n-body | Implemented now | Current struct, scalar floating-point, and Array/COW facilities are sufficient. |
| fannkuch-redux | Implemented now | Current scalar permutation and Array/COW facilities are sufficient. |
| fasta | Deferred: buffered bytes/I/O | Needs benchmark-suitable buffered byte output. |
| mandelbrot | Deferred: buffered bytes/I/O | Needs benchmark-suitable buffered binary output. |
| reverse-complement | Deferred: buffered bytes/I/O | Needs benchmark-suitable buffered byte input and output. |
| k-nucleotide | Deferred: core standard library | Needs the required core text/counting facilities. |
| pidigits | Deferred: core standard library | Needs a suitable core arbitrary-precision integer facility. |
| regex-redux | Deferred: core standard library | Needs the required core regex and byte-processing facilities. |
| binary-trees | Deferred: allocation model | Needs an explicit allocation/ownership model for a fair comparison. |

This baseline intentionally adds neither fasta nor any RFC, compiler/runtime
change, public language API, or standard-library API.
