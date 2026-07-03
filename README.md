# nomo

The reference compiler and project tooling for the [Nomo](https://github.com/nomo-lang)
programming language.

Nomo is a small language for systems tools, command-line programs and small
services. This crate is the heart of the ecosystem: it implements the compiler
front-end (lexer, parser, AST, type/semantic checks and diagnostics) and a C99
back-end. `.nomo` source is translated to C99 and then handed to the system C
compiler (`cc`) to produce a native executable.

This repository ships two binaries and one library:

- `nomo` — the project manager (`new` / `check` / `build` / `run` / `fmt`).
- `nomoc` — the compiler driver that operates on a single `.nomo` source file.
- `nomo` (lib crate) — the reusable compiler API, consumed by other repositories
  such as the [`nomo-lsp`](https://github.com/nomo-lang/nomo-lsp) language server.

## Role in the Nomo ecosystem

`nomo` is the upstream dependency for the rest of the toolchain. The
[`nomo-lsp`](https://github.com/nomo-lang/nomo-lsp) language server links against
this crate (via a `path` dependency) to produce diagnostics, and the editor
extensions ([`vscode-nomo`](https://github.com/nomo-lang/vscode-nomo),
[`zed-nomo`](https://github.com/nomo-lang/zed-nomo),
[`intellij-nomo`](https://github.com/nomo-lang/intellij-nomo)) talk to that
server in turn. Language decisions are tracked in the
[`rfcs`](https://github.com/nomo-lang/rfcs) repository.

## Requirements

- A recent stable Rust toolchain (the crate uses edition 2024).
- A system C compiler reachable as `cc` on your `PATH` (e.g. `clang` or `gcc`),
  used to compile the generated C99.

## Build and install

```bash
cargo build --release
# or install both binaries onto your PATH:
cargo install --path .
```

## Using `nomo` (project manager)

```bash
nomo new <name>                  # scaffold a new project (nomo.toml + src/main.nomo)
nomo check [path] [--json-errors] [--workspace] # type-check one project or every workspace member
nomo build [path] [--emit-c] [--json-errors] [--workspace] [--locked] [--offline] [--frozen] # compile one project or every workspace member
nomo run [path] [--json-errors] [-- args...] # build then run, forwarding args after `--`
nomo fmt [path] [--check] [--json-errors] # format project src/**/*.nomo or one source file
nomo test [path] [--workspace] [--package <package>] [--filter <text>] [--json] [--locked] [--offline] [--frozen] # discover and run #[test] functions
nomo doc [path] [--workspace] [--package <package>] [--std] [--open] [--json] [--output <dir>] # generate HTML docs or JSON doc data
nomo clean [path]                 # remove generated build artifacts
nomo deps resolve [path] [--workspace] [--locked] [--offline] [--frozen] # resolve one package or the full workspace lockfile
nomo deps tree [path] [--workspace] [--locked] [--offline] [--frozen] # print one package dependency tree or all workspace member trees
nomo deps update [path] [alias-or-package] [--workspace] [--offline] [--precise <version-or-rev>] # refresh all or one direct dependency lock entry
nomo deps vendor [path] [--workspace] [--dir vendor] [--sync] # copy locked path/git dependency sources into vendor/
nomo deps clean-cache [path]      # remove the project or workspace git dependency cache
```

A project is a directory containing a `nomo.toml` manifest and a `src/main.nomo`
entry point. `nomo build` writes generated C to `build/c/main.c` and the linked
executable to `build/bin/<name>`.

`nomo run <source.nomo>` also supports a standalone script file when the file is
not inside a project manifest. The file still starts with `package`, may define
imports and declarations, and may omit `fn main`; in that case, top-level
statements after all declarations are compiled as a synthesized `main() -> void`.
Explicit `main` functions and top-level script statements cannot be mixed.

`nomo fmt` is an AST-based v0.1 formatter. With no path or a directory path, it
discovers the project manifest and formats `src/**/*.nomo` in stable path order.
With a direct `.nomo` file path, it formats only that file and does not require a
manifest. `--check` prints `would format <path>` without writing and exits with
failure if any target differs. The formatter emits canonical whitespace and
indentation; it does not preserve original layout trivia. The lexer accepts
Rust-style line comments (`//`, `///`, `//!`) and nested block comments (`/* */`,
`/** */`, `/*! */`) as trivia, but `nomo fmt` rejects commented input for now
instead of silently dropping comments.

`nomo test` discovers top-level `#[test]` functions under project `src/**/*.nomo`.
Test functions must be non-generic, take no parameters, return `void`, and must
not be named `main`. Each test is compiled through the same project module and
dependency resolver path as `nomo build`, with a temporary runner `main()` that
calls the test. `--filter` keeps tests whose full name contains the filter text,
`--workspace` runs every workspace member, `--package` selects a package id or
member name, and `--json` prints a machine-readable test report.

`nomo doc` extracts Rust-style doc comments (`//!`, `///`, `/*! */`, `/** */`)
from project source files and combines them with parsed signatures,
visibility, source locations, and module names. By default it writes
`build/doc/index.html`, package/module HTML pages, and `search-index.json`.
`--json` prints the same documentation model to stdout without writing files.
`--workspace` documents workspace members, `--package` selects one member, and
`--std` adds the current built-in standard-library module index.

Current expression support includes binary numeric arithmetic (`+`, `-`, `*`,
`/`, `%`) with standard precedence, logical operators (`&&`, `||`, `!`) with
short-circuit evaluation, bitwise operators (`&`, `|`, `^`, `&^`, `<<`, `>>`),
plus equality and ordering comparisons. Statement-level update operators include
postfix `++`/`--` and compound assignment `+=`, `-=`, `*=`, `/=`, `%=`, `<<=`,
`>>=`, `&=`, `^=`, `|=`, and `&^=` for mutable variables and mutable struct
fields; they are not expressions and do not produce values. `%` and bitwise
operators are restricted to integer operands; `/` works for integer and `f64`
operands. Runtime divide-by-zero, signed `i32`/`i64` arithmetic overflow, and
invalid shift amounts panic.

```bash
nomo new hello
cd hello
nomo run
```

## Package manifests and dependencies

Nomo uses a namespace-first package model. A package's stable identity is
`<namespace>/<name>`; repository URLs, local paths and registry versions are
dependency sources rather than language-level package names. The namespaces
`std`, `nomo`, and `core` are reserved for the language and standard tooling.
`std` is built in: projects can import `std.*` modules without declaring a
`std` dependency in `nomo.toml`, and `std` is not written to `nomo.lock`.

New projects use this manifest shape:

```toml
[package]
namespace = "local"
name = "hello"
version = "0.1.0"
edition = "2026"
```

Dependency keys are local import aliases. For example:

```toml
[dependencies]
json = { package = "nomo-lang/json", version = "0.1.0" }
json_private = { package = "nomo-lang/json", version = "0.1.0", registry = "https://packages.example.com" }
local_utils = { package = "fynn/utils", path = "../utils" }
http = { package = "nomo-lang/http", git = "https://github.com/nomo-lang/http.git", rev = "2a4b8c1" }
cli = { package = "nomo-lang/cli", git = "https://github.com/nomo-lang/cli.git", branch = "stable" }
fmt = { package = "nomo-lang/fmt", git = "https://github.com/nomo-lang/fmt.git", tag = "v0.1.0" }
```

Workspace roots can share package defaults and dependency declarations with
member packages:

```toml
[workspace]
members = ["apps/*", "packages/*"]
default-members = ["apps/cli"]

[workspace.package]
namespace = "fynn"
edition = "2026"

[workspace.dependencies]
json = { package = "nomo-lang/json", version = "0.1.0" }
core = { package = "fynn/core", path = "packages/core" }
```

```toml
[package]
name = "cli"
version = "0.1.0"
namespace.workspace = true
edition.workspace = true

[dependencies]
json.workspace = true
core.workspace = true
```

```nomo
import json.parser
import local_utils.path
import http.client
```

Project commands (`nomo check`, `nomo build`, and `nomo run`) validate those
aliases from `nomo.toml`, so `import json.parser` is accepted only when `json`
is declared as a dependency alias or inherited from `[workspace.dependencies]`.
Project files can import sibling modules:
`import app.util` resolves to `src/util.nomo`, falling back to
`src/util/main.nomo`; `import app.main` resolves to `src/main.nomo`.
Dependency modules use the same Flat+Dir lookup under the dependency `src/`
directory, so `import local_utils.path` resolves to `src/path.nomo` or
`src/path/main.nomo` in that dependency. Imported local modules and imported
`path`/`git` dependencies contribute public API to the current v0.1 compile
unit, so public functions, constants, structs, enums, and public methods can
participate in type checking and generated C. Private dependency and module
items are not exported. Generated C function and nominal type symbols use each
item's source package path for mangling, so dependency APIs keep their
dependency package identity instead of being emitted as part of the root
application package.
`nomoc` remains a standalone source-file driver and only accepts built-in
`std.*` imports. Existing manifests that still declare
`std = { package = "nomo-lang/std", version = "0.1.0" }` or `std = "0.1.0"`
are accepted as compatibility input, but the declaration is ignored as a normal
dependency.
`nomo deps resolve` for a workspace member writes `nomo.lock` at the workspace
root. `nomo check --workspace`, `nomo build --workspace`,
`nomo deps resolve --workspace`, and `nomo deps tree --workspace` discover the
workspace root, expand `members` minus `exclude`, and visit each member package
in stable path order. Other
workspace-wide batch commands are planned for later workspace graph slices.

`nomo deps resolve [path]` validates the manifest and writes `nomo.lock`.
`nomo deps resolve --workspace [path]` writes a single workspace-root lockfile
that records each member as a `[[root]]` entry and stores shared locked package
entries once.
`nomo deps tree [path]` prints dependency aliases and canonical package IDs. If
`nomo.lock` exists, `tree` reads the locked dependency graph; otherwise it
resolves the current manifest sources. `nomo.lock` is standard TOML: package
entries are encoded as `[[package]]` tables with `id`, `alias`, `source`,
optional source metadata, `checksum`, and dependency edge strings. Workspace
lockfiles additionally use `[[root]]` tables to map member package IDs to their
direct dependency edges. Invalid TOML, unknown lockfile fields, and mismatched
field types are rejected. When locked `path` sources or matching git cache
checkouts are still available, `tree` verifies their `sha256:` checksums before
printing; missing path sources and git cache entries are treated as offline
locked entries. Git sources use a project-local `.nomo/deps/git/` cache keyed by
the canonical package ID and source URL. Cache misses clone the repository;
cache hits run `git fetch --tags --prune origin` before checking out the
requested `branch`, `tag`, or `rev`. Branch sources also run `git pull
--ff-only`. The checkout is validated against the expected canonical package ID
and locked to the actual `HEAD` revision. Resolved `path` and `git` packages include a
`sha256:` checksum over `nomo.toml` and `src/` contents. Registry sources are
recorded as leaf sources in v0.1, optionally with an explicit `registry`
endpoint, but do not include checksums because v0.1 does not fetch registry
archives. Public registry fetching and full version solving are deliberately
left for later versions. A dependency must specify exactly one source among
`path`, `git`, and `version`.
If the same canonical package ID resolves to conflicting sources or versions,
v0.1 reports an error instead of trying to solve multiple versions.
`--locked` is accepted by `nomo build`, `nomo deps resolve`, and
`nomo deps tree`; it requires an existing lockfile and rejects missing or
out-of-date direct dependencies without rewriting `nomo.lock`. `--offline`
prevents git fetch/clone and uses existing lockfiles or git cache checkouts;
without a lockfile, uncached git dependencies fail instead of going to the
network. `--frozen` is equivalent to `--locked --offline`.
`nomo deps update [path] [alias-or-package]` refreshes the lockfile from the
current manifest sources. Without a target it updates all dependencies; with an
alias or canonical package ID it first verifies that the target is a direct
dependency, then rewrites the lockfile. The current implementation rewrites the
full lockfile. `--precise <version-or-rev>` requires a direct dependency target,
updates only the in-memory source used for lockfile generation, and never edits
`nomo.toml`: registry dependencies use the precise value as `version`, git
dependencies use it as `rev` with any branch/tag selector cleared, and path
dependencies are rejected.
`nomo deps vendor [path]` ensures a lockfile exists, copies locked `path` and
`git` dependency sources into `vendor/`, and writes `vendor/nomo-vendor.toml`.
`--dir <path>` selects a different output directory, and `--sync` removes the
vendor directory before copying. Registry dependencies are recorded as skipped
until registry archive fetching exists. Locked/offline project builds and checks
fall back to the default `vendor/` directory when a locked path source or git
cache checkout is missing.
`nomo deps clean-cache [path]` removes the project or workspace
`.nomo/deps/git` cache. The command is idempotent and does not remove
`nomo.lock`, source files, or build artifacts.

## Using `nomoc` (compiler driver)

`nomoc` works directly on a single source file rather than a project:

```bash
nomoc check <source.nomo> [--json-errors]        # parse and type-check
nomoc build <source.nomo> [--emit-c] [--out path] [--json-errors] # emit C99
```

The `--json-errors` flag produces machine-readable diagnostics (with positions
and fix suggestions) suitable for tooling on `check` and `build`.
`nomoc` remains a compiler driver rather than a script runner; top-level script
statements are accepted only by `nomo run <source.nomo>`.

## Language notes

The postfix `?` operator works on both standard carriers in v0.1:
`Result.Ok(value)` unwraps to `value`, `Result.Err(error)` returns the error
early, `Option.Some(value)` unwraps to `value`, and `Option.None` returns
`None` early from the current `Option`-returning function.
There is no `try` keyword or statement syntax in v0.1; postfix `?` is the
only error/absence propagation syntax.

`std.path` provides pure string path helpers:
`path.join`, `path.basename`, `path.dirname`, `path.extension`,
`path.normalize`, and `path.is_absolute`. The v0.1 behavior uses POSIX-style
`/` separators and does not query the host filesystem or resolve symlinks.

`std.math` provides numeric helpers: `math.abs`, `math.min`, and `math.max`
operate on matching numeric types, while `math.floor`, `math.ceil`,
`math.round`, `math.sqrt`, `math.pow`, `math.sin`, and `math.cos` operate on
`f64` values.

`std.string` provides value helpers: `string.len`, `string.concat`,
`string.is_empty`, `string.contains`, `string.starts_with`, `string.ends_with`,
`string.split`, `string.trim`, `string.to_lower`, and `string.to_upper`.
The helpers operate on UTF-8 byte strings; `trim` and case conversion use ASCII
character classes in v0.1. `string.split(value, separator)` returns
`Array<string>` and panics if the separator is empty.

Diagnostics use stable `E`-prefixed error codes across human output, JSON
output, LSP diagnostics, and editor quick fixes. The first diagnostic reference
pages live under [`docs/diagnostics/`](docs/diagnostics/index.md).

## Library crate

The `nomo` library exposes the compiler pipeline for embedding. Key entry points
include `check_source`, `check_source_text` and `compile_source_to_c`, alongside
the `lexer`, `parser`, `ast`, `compiler`, `codegen`, `diagnostic`, `semantic`
and `project` modules. The `semantic` module exposes current-document symbol
queries plus project-aware hover, definition, and reference queries over local
`src/**/*.nomo` modules for editor integrations.

## Tests and examples

```bash
cargo test
```

Runnable sample programs live under [`examples/`](examples/).

## License

MIT. See [LICENSE](LICENSE).
