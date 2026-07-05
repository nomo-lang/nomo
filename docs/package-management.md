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

## Dependencies

Dependency keys are source import aliases. For example:

```toml
[dependencies]
local_utils = { package = "local/utils", path = "../utils" }
fmt = { package = "nomo-lang/fmt", git = "https://github.com/nomo-lang/fmt.git", tag = "v0.1.0" }
json = { package = "nomo-lang/json", version = "0.1.0" }
```

Each dependency must declare exactly one source kind:

- `path`: local package source, resolved by reading the target `nomo.toml`.
- `git`: git package source, cached under `.nomo/deps/git/`.
- `version`: registry leaf source. Current v0.1 records it in the lockfile but
  does not fetch registry archives.

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

Path and git dependencies are locked with a `sha256:` checksum over
`nomo.toml` and `src/`. Registry leaves currently do not have archive checksums
because registry fetching is not implemented in v0.1.

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

`--offline` prevents git clone/fetch. It uses existing lockfiles, git cache
checkouts, or vendored dependency sources. Without a lockfile, uncached git
dependencies fail.

`--frozen` is equivalent to `--locked --offline`.

These flags are accepted by build and dependency commands that need dependency
resolution:

```sh
nomo build --locked
nomo deps resolve --offline
nomo deps tree --frozen
```

## Updating Dependencies

`nomo deps update [path] [alias-or-package]` refreshes the lockfile from current
manifest sources. Without a target it updates all direct dependencies. With a
target, the target must be a direct dependency alias or canonical package ID.

`--precise <version-or-rev>` updates only the source used for lockfile
generation:

- registry dependencies use the value as `version`;
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

`--sync` removes the vendor directory before copying. Registry dependencies are
recorded as skipped until registry archive fetching exists. Locked or offline
project builds fall back to the default `vendor/` directory when the original
locked path source or git cache checkout is missing.

## Cache Cleanup

`nomo deps clean-cache [path]` removes the project or workspace git dependency
cache. It does not remove `nomo.lock`, source files, vendor directories, or
build artifacts.
