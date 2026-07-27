# Nomo

Reference compiler, runtime, standard library, and project tooling for the
early-preview [Nomo programming language](https://www.nomo-lang.org).

Nomo lowers typed source to readable C99 and invokes the platform C compiler
for native executables. The repository also builds an import-free WebAssembly
compiler/interpreter used by the public Playground.

## Preview status

Nomo has no stable `v0.1.0` release. The current packaged baseline is
[`v0.0.0-20260721120555`](https://github.com/nomo-lang/nomo/releases/tag/v0.0.0-20260721120555);
timestamp releases are prerelease snapshots with no cross-snapshot
compatibility promise.

Current `main` is newer than the packaged snapshot and includes
manifest-derived module roots, the `nomo fix module-roots` migration,
canonical implicit-void formatting, and bounded P3-C/P3-D static
`task.select` runtime slices.

Internal tests establish implementation evidence, not production readiness.
Review the
[Release Gate](https://github.com/nomo-lang/rfcs/blob/main/RELEASE-GATE.md)
before making platform, performance, stability, security, or ecosystem claims.

## Install

Download a pinned archive and `SHA256SUMS` from
[GitHub Releases](https://github.com/nomo-lang/nomo/releases), or use
[`setup-nomo`](https://github.com/nomo-lang/setup-nomo):

```yaml
- uses: nomo-lang/setup-nomo@main
  with:
    version: v0.0.0-20260721120555
```

`version: latest` is reserved for a future non-prerelease release and currently
does not select timestamp snapshots.

Build current source with stable Rust and a C99 compiler available as `cc`:

```sh
cargo build --workspace --release --locked
cargo install --path crates/nomo --locked
```

The install provides `nomo` (project CLI) and `nomoc` (single-file compiler
driver).

## Quick start

```sh
nomo new hello-world
cd hello-world
nomo fmt .
nomo check .
nomo run .
```

`[package].name = "hello-world"` deterministically maps to the lower_snake_case
source root:

```nomo
package hello_world

import std.io

fn main() {
    io.println("Hello, Nomo")
}
```

`src/main.nomo` declares the manifest root itself, not `<root>.main`.
`src/http/main.nomo` declares `<root>.http`. Namespace, canonical
`owner/package` identity, and dependency aliases never enter the package's own
source declaration.

No-return functions, methods, `suspend fn`, interface methods, and extern
functions canonically omit `-> void`. Explicit arrows remain parser-compatible
during the documented snapshot window. The `void` type still appears in
`Result<void, E>`, `Ok(void)`, and callable types such as
`task fn(string) -> void`.

Migrate an older project atomically:

```sh
nomo fix module-roots . --check
nomo fix module-roots .
nomo fmt .
```

The migration changes only the current package's declarations; it does not
rewrite dependency aliases or dependency source.

## Verified capabilities

The repository's current automated gates cover:

- lexer, parser, AST, semantic checks, structured diagnostics, formatter, docs,
  and compiler/LSP query bridges;
- C99 emission, native linking, standalone files, projects, workspaces,
  cross-target emission, and selected real cross-linked artifacts;
- immutable-by-default values, explicit mutation, structs, enums, generics,
  interfaces, `Option`, `Result`, arrays, nested copy-on-write indexing, and
  deterministic insertion-ordered `Map`;
- manifests, lockfiles, path/git/registry dependencies, vendoring, offline and
  frozen resolution, package archives, checksums, signatures, transparency
  proofs, and controlled typed C FFI;
- built-in standard modules for I/O, paths, files, processes, JSON, HTTP,
  SQLite, time, testing, and related preview facilities;
- direct-style `suspend`, a bounded current-thread executor, structured tasks,
  timers/deadlines, bounded channels, static selection, owner-affine TCP and
  process operations, cancellation, and lifecycle counters;
- an import-free, fuel-limited WebAssembly compiler/interpreter boundary;
- examples, benchmark correctness fixtures, and release-gate evidence.

See [`examples/README.md`](examples/README.md) for the executable matrix.

## Known boundaries

- Language, standard-library, manifest, and editor contracts may change between
  timestamp snapshots.
- Native TLS/HTTP and async transport slices remain capability-specific and do
  not imply a general production service runtime.
- The current executor is deliberately bounded; unsupported concurrency or
  ownership cases remain explicit errors or deferred work.
- WASM has no filesystem, process, environment, network, clock, or input host
  capabilities.
- Benchmark results are workload- and environment-specific, claim-ineligible
  unless their versioned evidence contract says otherwise.
- Internal CI is not external adoption, long-term compatibility, or exhaustive
  platform certification.

## CLI

Run `nomo --help` for the complete current contract.

| Command | Purpose |
| --- | --- |
| `nomo new <name>` | Create `nomo.toml` and canonical `src/main.nomo` |
| `nomo check [path] [--workspace]` | Resolve and type-check a project/workspace |
| `nomo build [path] [--target T] [--emit-c]` | Emit C99 and link a native artifact |
| `nomo run [path] [-- args...]` | Build and execute |
| `nomo fmt [path] [--check]` | Canonical source formatting |
| `nomo fix module-roots [path] [--check]` | Migrate manifest-derived package roots |
| `nomo manifest migrate [path] [--check]` | Atomically migrate manifest schema/trust policy |
| `nomo test [path] [--workspace]` | Discover and execute `#[test]` functions |
| `nomo doc [path] [--workspace] [--std]` | Generate HTML or JSON API documentation |
| `nomo add`, `remove`, `search`, `yank` | Manage dependencies and registry state |
| `nomo deps resolve`, `tree`, `update` | Resolve and inspect lockfile graphs |
| `nomo deps vendor`, `clean-cache` | Vendor sources and manage dependency cache |
| `nomo publish`, `verify` | Build or verify integrity-protected package archives |
| `nomo ffi bindgen` | Generate reviewed bindings for a controlled C-header subset |
| `nomo cache stats`, `clean`, `prune` | Manage the rebuildable incremental cache |
| `nomo clean` | Remove generated project build artifacts |

Common dependency/release flags include `--locked`, `--offline`, `--frozen`,
`--json-errors`, and workspace/package selectors. Consult CLI help rather than
copying an older snapshot's long command line.

## Platforms and artifacts

Preview archives are built for:

| Host | Packaged target |
| --- | --- |
| Linux x86-64 | `x86_64-unknown-linux-gnu` |
| macOS Intel | `x86_64-apple-darwin` |
| macOS Apple silicon | `aarch64-apple-darwin` |
| Windows x86-64 | `x86_64-pc-windows-msvc` |

CI additionally links and executes selected Linux arm64 artifacts under QEMU
and verifies macOS arm64-to-x86-64 cross-links. `--emit-c` supports recognized
targets even when a cross-linker is unavailable. See
[`docs/cross-compilation.md`](docs/cross-compilation.md).

## Repository map

| Path | Responsibility |
| --- | --- |
| `crates/nomo_syntax` | AST, lexer, parser, formatter-facing syntax |
| `crates/nomo_compiler` | Module graph, semantic analysis, typed/lowered IR |
| `crates/nomo_codegen_c` | IR-specific C99 generation |
| `crates/nomo_runtime` | Platform-aware C runtime emission and async runtime |
| `crates/nomo_wasm` | Browser compiler/interpreter ABI and sandbox |
| `crates/nomo` | User CLI, project orchestration, registry, docs, tests |
| `crates/nomo_lsp_bridge` | Shared editor queries and signature surfaces |
| `std/` | Toolchain standard-library sources and intrinsic registry |
| `examples/` | Executable acceptance projects |
| `performance/` | Versioned correctness/performance evidence contracts |
| `scripts/` | Release, governance, benchmark, and WASM verification gates |

See [`AGENTS.md`](AGENTS.md) before changing compiler/runtime boundaries.

## Development checks

Pull requests run a focused smoke matrix; `main` runs the broader multi-platform
and evidence gates. At minimum:

```sh
cargo fmt --check
cargo clippy --workspace --locked -- -D warnings
cargo test --workspace --locked
cargo build --workspace --release --locked
cargo run --locked --bin nomo -- fmt --check examples
cargo run --locked --bin nomo -- fmt --check std
python3 scripts/check_syntax_governance.py --nomo target/release/nomo
cargo build --locked --release --target wasm32-unknown-unknown -p nomo-wasm
node scripts/check_browser_wasm.mjs \
  target/wasm32-unknown-unknown/release/nomo_wasm.wasm
```

Async and benchmark harness changes also run:

```sh
python3 -m unittest discover -s scripts/tests
python3 scripts/benchmarksgame.py --mode correctness \
  --nomo target/release/nomo --require-clean
python3 scripts/async_benchmark.py --nomo target/release/nomo --require-clean
```

Follow repository CI for platform-specific gates; do not replace failed
evidence by loosening payloads, versions, counters, or thresholds.

## Authoritative documentation

- [English specification](https://github.com/nomo-lang/rfcs/blob/main/en/SPEC.md)
- [中文规范](https://github.com/nomo-lang/rfcs/blob/main/zh-CN/SPEC.md)
- [RFC index](https://github.com/nomo-lang/rfcs)
- [Roadmap](https://github.com/nomo-lang/rfcs/blob/main/ROADMAP.md)
- [Non-normative whitepaper](https://github.com/nomo-lang/rfcs/blob/main/WHITEPAPER-v0.1.md)
- [Shared contribution guide](https://github.com/nomo-lang/.github/blob/main/CONTRIBUTING.md)

Language or public CLI changes require an RFC before implementation and must
update compiler, LSP, grammar, editor, examples, and documentation surfaces.

## License

See [LICENSE](LICENSE).
