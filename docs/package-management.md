# Package Management

Nomo projects use `nomo.toml` for package metadata and dependencies, and
`nomo.lock` for resolved dependency state. Project commands use the manifest to
validate imports, load local modules, and resolve path or git dependencies.

## Package Manifests

A package manifest declares the package identity:

```toml
[package]
namespace = "local"
name = "hello"
version = "0.1.0"
edition = "2026"
```

`namespace`, `name`, `version`, and `edition` are validated by the project
loader. `std`, `nomo`, and `core` are reserved namespaces.

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

## Dependencies

Dependency keys are source import aliases. For example:

```toml
[dependencies]
local_utils = { package = "local/utils", path = "../utils" }
fmt = { package = "nomo-lang/fmt", git = "https://github.com/nomo-lang/fmt.git", tag = "v0.1.0" }
json = { package = "nomo-lang/json", version = "^1.2.0" }
```

Each dependency must declare exactly one source kind:

- `path`: local package source, resolved by reading the target `nomo.toml`.
- `git`: git package source, cached under `.nomo/deps/git/`.
- `version`: registry source fetched from the configured file, HTTP, or HTTPS registry
  and cached under `.nomo/cache/registry/`.

Project imports use dependency aliases:

```nomo
import local_utils.path
import fmt.main
```

Local project modules use the project import root from `package app.main`.
`import app.util` resolves to `src/util.nomo`, then `src/util/main.nomo`.
Dependency modules use the same flat-then-directory lookup under the dependency
package `src/` directory.

## Workspaces

A workspace root contains `[workspace]` and may provide inherited package fields
or dependencies:

```toml
[workspace]
members = ["apps/*", "packages/*"]

[workspace.package]
namespace = "local"
edition = "2026"

[workspace.dependencies]
util = { package = "local/util", path = "packages/util" }
```

Member manifests can inherit package fields and dependencies:

```toml
[package]
name = "app"
version = "0.1.0"
namespace.workspace = true
edition.workspace = true

[dependencies]
util.workspace = true
```

Workspace dependency paths are interpreted from the workspace root and rebased
for each member during resolution.

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
the manifest:

```toml
[trust]
policy = "signed+transparent"
transparency-keys = ["<64 hexadecimal Ed25519 log public key>"]
```

`checksum-only` is the compatibility default, `signed` requires an authorized
publisher key and signed provenance, and `signed+transparent` additionally
requires a log key pinned by the project. The resolver stores the verified
publisher key id, subject/provenance digests, and transparency tree head in
`nomo.lock`; cached tree heads reject rollback or equivocation. Offline locked
builds reuse this evidence and never treat a downloaded bundle's log key as a
trust root.

Publishers can keep signing keys outside Nomo credentials with an external
signer:

```sh
nomo owner key add owner/package <ed25519-public-key-hex> --registry <url>
nomo publish --registry <url> --signer /path/to/signer
nomo verify build/package/owner-package-1.0.0.nomo-package \
  --envelope build/package/owner-package-1.0.0.nomo-package.envelope.json \
  --key <ed25519-public-key-hex> \
  --provenance build/package/owner-package-1.0.0.nomo-package.provenance.json \
  --transparency transparency.json --log-key <ed25519-log-public-key-hex>
```

The signer receives only the canonical release subject on stdin. Private keys
never enter credentials, registry metadata, provenance, envelopes, or lockfiles.

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
