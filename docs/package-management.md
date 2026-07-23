# Package Management

Nomo projects use `nomo.toml` for package metadata and dependencies, and
`nomo.lock` for resolved dependency state. Project commands use the manifest to
validate imports, load local modules, and resolve path or git dependencies.

## Package Manifests

A package manifest declares the package identity:

```toml
manifest-version = 2

[package]
namespace = "local"
name = "hello"
version = "0.1.0"
edition = "2026"
description = "A small Nomo application"
license = "MIT"
repository = "https://example.com/local/hello"
publish = false
```

Manifest v2 requires `namespace`, `name`, `version`, and `edition` for a
standalone package and rejects unknown fields. `std`, `nomo`, and `core` are
reserved namespaces. Omitting `manifest-version` selects the legacy v1
compatibility parser; migrate a project or complete workspace with:

```sh
nomo manifest migrate [path]
nomo manifest migrate [path] --check
```

Migration materializes legacy defaults and moves consumer-side trust policy to
`.nomo/config.toml`. It validates every output before replacing files and is
idempotent. `--check` reports whether a write would be required.

`std` is built in. User manifests do not need a `std` dependency, `std` cannot
be used as an ordinary dependency alias, and `std` is not written to the
lockfile as a normal package.

## FFI Link Metadata

Packages that declare `extern "C"` functions can add linker metadata:

```toml
[ffi]
libraries = ["sqlite3"]
library_paths = ["native/lib"]
sources = ["native/bridge.c"]
frameworks = ["Security"]
link_args = ["-Wl,-rpath,@loader_path"]
```

`libraries` become `-l<name>`, `library_paths` become `-L<path>`, package-relative
`sources` are compiled by `cc`, `frameworks` become macOS `-framework <name>`
arguments, and `link_args` are passed through as raw `cc` arguments. Relative
paths are resolved from the package root that declares them. Declared FFI source
files participate in package checksums, publish archives, and vendoring.
`nomo build`, `nomo run`, and `nomo test` aggregate metadata from the root package
and source dependencies; standalone script mode does not read a manifest and
therefore does not use `[ffi]`.

Target-specific native metadata uses restricted `arch`, `os`, and `env`
selectors. Each selector accepts one string or a set of strings:

```toml
[[ffi.target]]
os = ["linux"]
sources = ["native/linux.c"]
libraries = ["pthread"]

[[ffi.target]]
os = "macos"
frameworks = ["Security"]
```

Matching entries extend the common `[ffi]` metadata. Non-matching sources,
libraries, search paths, frameworks, and link arguments are not passed to the
target compiler.

## Dependencies

Dependency keys are source import aliases. For example:

```toml
[dependencies]
local_utils = { path = "../utils" }
renderer = { package = "nomo-lang/renderer", git = "https://github.com/nomo-lang/renderer.git", tag = "v0.1.0" }
json = { package = "nomo-lang/json", version = "^1.2.0" }
winapi = { package = "nomo-lang/winapi", version = "1.0.0", target = { os = "windows", arch = ["x86_64", "arm64"] } }
```

Each dependency must declare exactly one source kind:

- `path`: local package source, resolved by reading the target `nomo.toml`.
- `git`: git package source, cached under `.nomo/deps/git/`.
- `version`: registry source fetched from the configured file, HTTP, or HTTPS registry
  and cached under `.nomo/cache/registry/`.

In manifest v2, path dependencies can omit `package`; Nomo derives and validates
the canonical identity from the target manifest. Git and registry dependencies
keep an explicit canonical package identity.

An optional `target = { ... }` predicate makes an edge active only when every
specified dimension matches; multiple values within one dimension form a set.
`amd64`, `arm64`, and `macos` are accepted and canonicalized to `x86_64`,
`aarch64`, and `darwin`. Conditions that cannot match a supported target are
rejected. Resolution records every known edge in `nomo.lock`; compilation,
workspace member ordering, module loading, and FFI aggregation filter that
complete graph using one canonical target context.

Project imports use dependency aliases:

```nomo
import local_utils.path
import renderer.main
```

Local project modules use the project import root from `package app.main`.
`import app.util` resolves to `src/util.nomo`, then `src/util/main.nomo`.
Dependency modules use the same flat-then-directory lookup under the dependency
package `src/` directory.

## Workspaces

A workspace root contains `[workspace]` and may provide inherited package fields
or dependencies:

```toml
manifest-version = 2

[workspace]
members = ["apps/*", "packages/*"]
default-members = ["apps/app"]

[workspace.package]
namespace = "local"
version = "0.1.0"
edition = "2026"
license = "MIT"

[workspace.dependencies]
util = { path = "packages/util" }
```

Member manifests can inherit package fields and dependencies:

```toml
manifest-version = 2

[package]
name = "app"
inherit = "workspace"

[dependencies]
util = { workspace = true }
```

`package.name` always belongs to the member. `inherit = "workspace"` fills only
missing namespace, version, edition, and descriptive metadata after the package
has been proven to match `members` minus `exclude`. Workspace dependency paths
are interpreted from the workspace root and rebased for each member during
resolution.

Workspace-aware commands include:

```sh
nomo check --workspace
nomo build --workspace
nomo test --workspace
nomo doc --workspace
nomo deps resolve --workspace
nomo deps tree --workspace
```

## Lockfiles

`nomo deps resolve [path]` validates the manifest and writes `nomo.lock`.
Workspace member resolution writes the lockfile at the workspace root.

Lockfiles are standard TOML. Package entries are stored as `[[package]]` tables
with package ID, alias, source metadata, checksum, and dependency edges.
Workspace lockfiles also include `[[root]]` entries mapping workspace member
packages to direct dependency edges.

Path, git, and registry dependencies are locked with a `sha256:` checksum over
`nomo.toml`, `src/`, and any package-local C files declared by `[ffi].sources`.

## Git Cache

Git dependencies are cached under `.nomo/deps/git/`, keyed by canonical package
ID and source URL. Cache misses clone the repository. Cache hits fetch tags and
prune before checkout. Branch sources are fast-forwarded with `git pull
--ff-only`.

The resolver checks out `branch`, `tag`, or `rev`, validates the target
package's canonical package ID, and records the actual `HEAD` revision in the
lockfile. A manifest may specify only one git checkout selector.

## Locked, Offline, and Frozen

`--locked` requires an existing lockfile and rejects missing or out-of-date
direct dependency entries without rewriting `nomo.lock`.

`--offline` prevents git clone/fetch and registry downloads. It uses existing
lockfiles, source caches, or vendored dependency sources. Without a lockfile,
uncached network dependencies fail.

`--frozen` is equivalent to `--locked --offline`.

## Registry Metadata

An explicit HTTP or HTTPS registry exposes package metadata at:

```text
GET /api/v1/packages/<owner>/<package>
GET /api/v1/packages/<owner>/<package>/<version>
```

The package response contains the canonical `package` ID and a `versions` array.
Each version contains `version`, the `sha256:` checksum of the downloadable
`.nomo-package` archive, and `yanked`. The exact-version response contains the
same fields plus the canonical `package` ID.

Fresh online resolution fetches exact-version metadata before downloading an
archive, rejects a yanked version, and verifies the archive checksum before
unpacking. A lockfile stores a separate checksum over unpacked package sources;
locked or frozen builds use that checksum and may continue to use a yanked
version from cache or vendor without a metadata request. A file registry may
provide package metadata at `api/v1/packages/<owner>/<package>/index.json` and
version metadata at
`api/v1/packages/<owner>/<package>/<version>/metadata.json`. These files are
optional for compatibility with existing local file registries.
Dependency manifests support bare exact versions, caret ranges, tilde ranges,
and bounded comparison ranges. Wildcards, alternatives, implicit `latest`, and
`=` exact syntax are rejected. Fresh resolution selects the highest non-yanked
version satisfying every project or workspace constraint and writes only that
exact version to `nomo.lock`. HTTP package indexes are cached for offline range
resolution. An unsatisfiable graph reports a stable minimal constraint set with
dependency paths. One canonical package still has only one selected version.

These flags are accepted by build and dependency commands that need dependency
resolution:

```sh
nomo build --locked
nomo deps resolve --offline
nomo deps tree --frozen
```

## Supply-chain trust

Registry metadata may include a publisher signature, canonical provenance, and
an inclusion proof from a transparency log. Projects opt into verification in
the root `.nomo/config.toml`, keeping local operating policy out of publishable
package manifests:

```toml
config-version = 1

[registry]
policy = "signed+transparent"
transparency-keys = ["<64 hexadecimal Ed25519 log public key>"]
proof-max-age-seconds = 86400
offline-proof-max-age-seconds = 604800
max-future-skew-seconds = 300
gossip-checkpoints = ["trust/registry-peer.json"]
```

`checksum-only` is the compatibility default, `signed` requires an authorized
publisher key and signed provenance, and `signed+transparent` additionally
requires a log root key pinned by the project. A current log key may be reached
from that root only through ordered rotation statements signed by both the old
and new Ed25519 keys. Tree heads bind the log id, issuance time, signing key,
and preceding signed checkpoint. The resolver stores the verified publisher
key id, subject/provenance digests, and transparency tree head in `nomo.lock`;
cached and gossiped heads reject rollback, skipped history, or equivocation.
Offline locked builds reuse this evidence and never treat a downloaded bundle's
log key as a trust root.

Online proofs are accepted for 24 hours by default, while `--offline` resolution
allows a seven-day proof age. Heads more than five minutes in the future are
rejected. The three numeric project-config settings above override those defaults;
the offline limit must be at least the online limit. `gossip-checkpoints` paths
must remain inside the project or workspace root and may contain one signed
checkpoint or a JSON array of checkpoints distributed by CI, mirrors, or
registry peers. Successful
resolver verification writes the latest shareable checkpoint below
`.nomo/cache/registry/trust/<registry-id>/gossip-checkpoint.json`.

Publishers can keep signing keys outside Nomo credentials with an external
signer:

```sh
nomo owner key add owner/package <ed25519-public-key-hex> --registry <url>
nomo publish --registry <url> --signer /path/to/signer
nomo verify build/package/owner-package-1.0.0.nomo-package \
  --envelope build/package/owner-package-1.0.0.nomo-package.envelope.json \
  --key <ed25519-public-key-hex> \
  --provenance build/package/owner-package-1.0.0.nomo-package.provenance.json \
  --transparency transparency.json --log-key <pinned-ed25519-log-public-key-hex> \
  --gossip peer-checkpoint.json --write-gossip latest-checkpoint.json \
  --proof-max-age-seconds 86400
```

The signer receives only the canonical release subject on stdin. Private keys
never enter credentials, registry metadata, provenance, envelopes, or lockfiles.
See [Transparency Log Operations](transparency-operations.md) for the rotation,
gossip, freshness, and incident-response contract.

## Updating Dependencies

`nomo deps update [path] [alias-or-package]` refreshes the lockfile from current
manifest sources. Without a target it updates all direct dependencies. With a
target, the target must be a direct dependency alias or canonical package ID.

`--precise <version-or-rev>` updates only the source used for lockfile
generation:

- registry dependencies use the value as the exact selection when it satisfies
  the manifest requirement;
- git dependencies use the value as `rev`, clearing branch/tag selection;
- path dependencies reject `--precise`.

`nomo deps update` rewrites the full lockfile and does not edit `nomo.toml`.

## Vendoring

`nomo deps vendor [path]` ensures a lockfile exists, copies locked path and git
dependency sources into `vendor/`, and writes `vendor/nomo-vendor.toml`.

```sh
nomo deps vendor
nomo deps vendor --workspace
nomo deps vendor --dir third_party/nomo
nomo deps vendor --sync
```

`--sync` removes the vendor directory before copying. Cached registry package
sources are copied alongside path and git sources; uncached registry leaves are
recorded as skipped. Locked or offline project builds fall back to the default
`vendor/` directory when the original source or cache entry is missing.

## Cache Cleanup

`nomo deps clean-cache [path]` removes the project or workspace git dependency
cache. It does not remove `nomo.lock`, source files, vendor directories, or
build artifacts.
