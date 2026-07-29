# Persistent Incremental Cache

Nomo keeps compiler-owned query results in a project or workspace cache so
separate CLI processes can reuse work without changing build correctness. The
cache is always an optimization: deleting it, losing it, or rejecting an entry
must produce the same diagnostics and generated output as a clean invocation.

## Cached values

The first persistent layer stores conservative values for:

- a successful `nomo check` result keyed by all project and dependency Nomo
  sources, relevant manifests, the host target, query schema, and toolchain
  version;
- generated C from project build/run/emit-C commands, standalone `nomoc` and
  script runs, and individual test harnesses, keyed by their complete source
  inputs plus the selected target, `debug` or `release` profile,
  the SHA-256 revision of the exact producer executable, pass-pipeline version,
  and relevant native-toolchain configuration.

Failed checks are not persisted. Linking still runs on every build, so the
selected system or cross C toolchain remains authoritative for the final
artifact. The in-memory semantic session continues to cache check and symbol
values for editor processes.

## Storage and recovery

Entries are stored below:

```text
.nomo/cache/incremental/v1/<digest-prefix>/<query-digest>.json
```

The directory version is the disk schema. Query keys include the compiler query
schema and toolchain version, so incompatible results naturally miss instead of
being migrated.

Each entry includes the complete query key, an encoded value, and a SHA-256
value checksum. Writers create a unique temporary file in the destination
directory, flush it, and atomically replace the final path. Concurrent writers
may replace the same content-addressed key, while readers see either the old
complete value or the new complete value. A parse error, wrong schema, key
mismatch, invalid encoding, or checksum mismatch removes the entry and falls
back to a clean computation.

## Capacity and cleanup

The default cache capacity is 512 MiB. Set a per-invocation limit in bytes with:

```bash
NOMO_INCREMENTAL_CACHE_MAX_BYTES=1073741824 nomo build
```

New writes evict the oldest entry files until the cache is within capacity.
The same policy can be applied explicitly:

```bash
nomo cache stats [path]
nomo cache prune [path] --max-bytes 268435456
nomo cache clean [path]
```

`stats` reports the active schema directory, entry count, stored bytes, and
configured capacity. `prune` is safe to run while other processes are using the
cache because a missing entry is a normal miss. `clean` removes only the
incremental cache. `nomo clean` remains scoped to generated build artifacts.

Debug and release entries never alias. A cold debug build may warm only its
debug key; a cold release build writes a distinct release key, and alternating
profiles reuses only the matching entry. Incrementing the pass-pipeline version
or changing a bound toolchain input forces a miss. The producer revision uses
the executable bytes (`exe-sha256:<digest>`), not `CARGO_PKG_VERSION` or Git
availability: copying identical bytes to another path keeps the key, while a
one-byte producer change misses. Failure to read and hash the producer fails
closed without publishing cache or build evidence.

The public `nomo-build-metadata.json` document discloses the cache-key formula,
ordered inputs, exact serialized `QueryKey`, and resulting key for the completed
build. The disclosed key is passed directly from the compilation/cache lookup;
metadata generation does not reread source files to derive a parallel key. See
[Release Builds and Backend Provenance](release-builds.md).

Set `NOMO_INCREMENTAL_TRACE=1` to print `hit`, `write`, and `corrupt` events to
stderr for diagnostics and benchmark collection.

## Correctness gates

The test suite compares persistent and clean checks across deterministic
randomized edit sequences. CLI integration tests execute cold and warm cache
paths in separate processes, alternate debug and release builds, validate a
pipeline-version miss, corrupt an on-disk entry, verify automatic recovery,
reject a stale success after a source edit, exercise persistent C codegen reuse,
and force capacity eviction.
