# Release Builds and Backend Provenance

Nomo has two native build profiles. Omitting `--release` selects `debug`;
passing it selects `release` for the complete command:

```sh
nomo build [path] --release
nomo run [path] --release -- [program arguments]
nomo test [path] --release
nomoc build input.nomo --release [--out generated.c]
```

For `nomo run`, options before `--` belong to Nomo and arguments after `--`
belong to the program. `nomo test --release` applies the profile to every
discovered test harness and test unit without changing discovery, isolation,
reporting, or exit status. `nomoc build` remains a C-emission command: without
`--out`, stdout contains only generated C; with `--out`, generated C is written
to that path.

`--release` and `--emit-c` are deliberately separate protocols and cannot be
combined. The combination exits nonzero, reports the conflict, and removes any
stale release evidence files. Plain `--emit-c` keeps the debug-profile
generated-C protocol used by external consumers.

## Native release backend

A host-native release build invokes a resolved Clang driver with a controlled
environment. The primary generated C translation unit is compiled with:

```text
<absolute-clang> --no-default-config -std=c99 -O3 -DNDEBUG \
  -fomit-frame-pointer -c <absolute-main.c> -o <absolute-object>
```

The object is linked with the same absolute driver and
`--no-default-config`. Nomo does not enable fast-math, LTO, PGO,
`-march=native`, or `-mcpu=native`. On POSIX hosts, `-lm` is added only when
the generated C contains a math-runtime call that requires it. The same
capability-based rule, rather than a project or workload name, selects dynamic
loader, task, SQLite, and Winsock link support. The libm capability scan
recognizes actual generated-C call tokens and ignores comments, string/character
literals, and longer user identifiers.

The controlled environment clears compiler-affecting variables and records the
exact retained/cleared projection. macOS records the selected Xcode developer
SDK root and settings digest, the `xcrun` identity and selection command, and
the distinction between the `/usr/bin/clang` invocation and its selected real
executable. Windows records the selected Program Files LLVM invocation, Visual
Studio installation, SDK roots, tool identities, and filtered compiler paths.

Configured native cross-links retain their target-specific toolchain arguments.
GNU cross drivers are identified by their target-prefixed `*-gcc` executable,
use `-dumpmachine` for the reported target, and receive the fixed C99/release
flags without Clang-only `--no-default-config`. Clang host release builds keep
the exact argv above. Recognized targets without a configured native linker fail
explicitly.
The browser `nomo-wasm` compiler/interpreter and its sandbox are not converted
into Clang builds by `--release`.

## `release-provenance.json`

Successful `nomo build --release` and project-form `nomo run --release` write:

```text
<project>/build/c/main.c
<project>/build/bin/<project>[.exe]
<project>/build/release-provenance.json
```

An explicit target places all three below
`build/<canonical-target>/`. The sidecar is canonical JSON and uses the frozen
release-backend schema 1. Its top level contains exactly:

```text
schema, complete_argv, compiler, objects, compile_commands, link_command,
generated_c, binary
```

It records the absolute compiler invocation and real path, compiler SHA-256,
complete version output, the compiler-reported target triple, complete
compile/link argv, command cwd, duration, exit status, controlled environment,
and SHA-256 bindings for generated C, objects, and the final binary. Consumers
of the single-translation-unit release protocol require exactly one object and
one compile command.

## `nomo-build-metadata.json`

Every project build writes a separate, versioned metadata document at:

```text
<project>/build/nomo-build-metadata.json
```

Explicit-target builds use
`build/<canonical-target>/nomo-build-metadata.json`. A `nomoc build` writes the
same filename below the input file's sibling `build/` directory for either
profile, while `generated_c.path` identifies either `--out` or the materialized
`build/c/main.c`.

Schema 1 contains:

- `selected_profile` (`debug` or `release`) and canonical `target_triple`;
- `producer_executable`, including the invocation path, resolved path, file
  size, package version, and SHA-256 of the exact `nomo` or `nomoc` executable
  that produced the output;
- `compiler`, full compile/link command records, `generated_c`, and `binary`
  content records when those stages exist;
- `release_provenance.path` and `release_provenance.sha256` for native release
  builds;
- `cache_identity`, including the exact persistent `query_key`, its versioned
  inputs/formula, and `cache_key`;
- `content_binding`, which independently binds this invocation's commands and
  outputs.

The cache identity is:

```text
SHA-256(UTF-8 bytes of query_key_json)
```

This is the same digest used as the persistent cache entry filename, not a
parallel evidence formula. `query_key_json` preserves the exact compact bytes
hashed by the cache, while `query_key` exposes the parsed object. Schema 1
discloses the query schema, toolchain, target, namespace, identity, and source
fingerprint alongside the selected profile, Nomo compiler/runtime revisions,
pass-pipeline version, and toolchain-configuration version/text/digest. The
compiler/runtime revision is `exe-sha256:<producer digest>` rather than the
human package version, and that revision is part of the actual persistent
`QueryKey`. Identical executable bytes at another path reuse the key; a
one-byte producer change, profile change, or pipeline change misses. Producer
identity failure is fail-closed before cache or build evidence is published.

`content_binding` uses the separate domain
`nomo-build-metadata-content-binding-v1`. Starting with that UTF-8 domain, then
each name and value in `input_order`, the hash appends each component as
`u64be(UTF-8 byte length) || UTF-8 bytes` and computes SHA-256 over the
concatenation. The ordered inputs bind the selected profile, target, cache key,
complete compiler identity, all command records (including argv, rendered
command, cwd, duration, exit status, and environment), and the absolute path
plus digest for generated C, the binary, and the release sidecar. It may differ
between otherwise equivalent builds because command duration is
invocation-specific. Metadata for one profile cannot authenticate an otherwise
identical binary or sidecar as the other profile, and an equal-content artifact
at another path cannot be substituted.

The three hashed structured inputs are independently reproducible without Rust
field-order assumptions. `content_binding.canonical_subdocuments` stores compact
UTF-8 canonical JSON for `producer_identity`, `compiler_identity`, and
`commands`; object keys are recursively sorted lexicographically, array order
is preserved, and JSON uses no insignificant whitespace. Each corresponding
input is the SHA-256 of those exact stored bytes. An external validator rebuilds
the same subdocuments from the top-level metadata values before trusting their
digests or the final framed binding.

Evidence publication is serialized by `<target-dir>/.nomo-build.lock`. A
successful release target publishes `nomo-build-metadata.json`, then a
workspace-owner receipt when the target is a member of a discovered workspace,
and finally `release-provenance.json` as the commit marker. Abort and cleanup
remove the sidecar, receipt, and metadata in the opposite order. The publisher
holds one target lock for the complete transaction, so another cooperative
build or cleanup cannot observe or leave a committed sidecar without its
metadata.

Workspace ownership state is outside `build/`:

```text
<workspace>/.nomo/state/release-evidence/v1/
  scope-id
  catalog.json
  .catalog.lock
```

The catalog is target- and profile-neutral. Its stable scope identifier outlives
membership changes, while its generation digest covers the current complete
`WorkspaceGraph`. Each member record binds the package identity, normalized
root-relative path, default-member status, and a member key derived from the
stable scope, package identity, and relative root. Only true graph members are
cataloged; excluded, unlisted, dependency, vendor, and nested-repository
projects are not inferred by scanning the directory tree.

Each successful workspace-member release target writes
`<target-dir>/.nomo-release-owner-v1.json` in the target transaction. The
receipt binds the catalog scope and member key, package/member identity,
profile, target and layout, and the hashes of that target's metadata and
release sidecar. A direct build of a healthy workspace member uses the same
catalog identity. A successful standalone project build removes an obsolete
workspace receipt instead of inheriting ownership from an old path.

After normal workspace discovery, Nomo refreshes the full catalog from the real
workspace graph and clears only members selected by that command
(`default-members`, all members, or one `--package`). If discovery fails, Nomo
does not recursively search for manifests. It searches upward for the nearest
trusted catalog without crossing a repository boundary, reapplies any
parseable package selection, derives member locations only from validated
catalog-relative paths, and clears a target only when its receipt and current
evidence hashes match. Missing, truncated, unknown-schema, stale, or mismatched
catalog/receipt state fails closed with an explicit cleanup error.

Catalog writers hold only `.catalog.lock`; publishers hold only one target
lock. Failure cleanup processes one member at a time: it takes that target lock,
then briefly takes the catalog lock to recheck the generation, and releases
both before considering another member. It never waits for a target lock while
holding the catalog lock and never holds two target locks.

Platform aliases outside the managed project or workspace boundary (for
example, macOS `/var` resolving to `/private/var`) are resolved before target
paths are constructed. Inside that canonical boundary, existing symlink
components, Windows reparse points, unsafe evidence/receipt files, and unsafe
lock files are rejected. Canonical target directories must remain inside the
canonical member build layout, and catalog member paths must remain inside the
current canonical workspace repository boundary. This is a cooperative
build-concurrency and accidental-path-substitution guarantee. It is not an
adversarial-filesystem or directory-swap/TOCTOU guarantee; such a guarantee
would require directory-handle-relative filesystem operations.

All evidence and workspace-state JSON files contain canonical bytes. Temporary
files are flushed and atomically replaced, with best-effort parent-directory
synchronization; a stronger platform-wide power-loss guarantee remains future
work. A consumer must reject unknown schema versions, non-canonical bytes,
missing files, changed artifact hashes, mismatched commands, or stale
compiler/producer identity. The release sidecar remains schema-compatible with
the frozen Benchmarks Game v2 contract; formal release-lane eligibility
additionally requires the benchmark runner's independent metadata-binding
contract.

For `nomo test --release`, the harness, generated runtime translation unit, and
declared FFI sources are compiled separately with the fixed release compile
flags. Raw `[ffi].link_args` are applied only by the link command. Such custom
link flags remain supported, but can make a build non-portable or ineligible
for a formal baseline whose validator permits only the fixed argv.
