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
nomo check [path] [--json-errors] # type-check the project
nomo build [path] [--emit-c] [--json-errors] # compile to a native binary (or stop at C with --emit-c)
nomo run [path] [--json-errors] [-- args...] # build then run, forwarding args after `--`
nomo fmt [path] [--check] [--json-errors] # format project src/**/*.nomo or one source file
nomo clean [path]                 # remove generated build artifacts
```

A project is a directory containing a `nomo.toml` manifest and a `src/main.nomo`
entry point. `nomo build` writes generated C to `build/c/main.c` and the linked
executable to `build/bin/<name>`.

`nomo fmt` is an AST-based v0.1 formatter. With no path or a directory path, it
discovers the project manifest and formats `src/**/*.nomo` in stable path order.
With a direct `.nomo` file path, it formats only that file and does not require a
manifest. `--check` prints `would format <path>` without writing and exits with
failure if any target differs. The formatter emits canonical whitespace and
indentation; it does not preserve original layout trivia.

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

New projects use this manifest shape:

```toml
[package]
namespace = "local"
name = "hello"
version = "0.1.0"
edition = "2026"

[dependencies]
std = { package = "nomo-lang/std", version = "0.1.0" }
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

```nomo
import json.parser
import local_utils.path
import http.client
```

Project commands (`nomo check`, `nomo build`, and `nomo run`) validate those
aliases from `nomo.toml`, so `import json.parser` is accepted only when `json`
is declared as a dependency alias. Project files can import sibling modules:
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
`std.*` imports.

`nomo deps resolve [path]` validates the manifest and writes `nomo.lock`.
`nomo deps tree [path]` prints dependency aliases and canonical package IDs. If
`nomo.lock` exists, `tree` reads the locked dependency graph; otherwise it
resolves the current manifest sources. When locked `path` sources or matching
git cache checkouts are still available, `tree` verifies their `sha256:`
checksums before printing; missing path sources and git cache entries are
treated as offline locked entries. Git sources are cloned into a
project-local `.nomo/deps/git/` cache, checked out to the requested `branch`,
`tag`, or `rev` when provided, validated against the expected canonical package
ID, and locked to the actual `HEAD` revision. Resolved `path` and `git` packages include a
`sha256:` checksum over `nomo.toml` and `src/` contents. Registry sources are
recorded as leaf sources in v0.1, optionally with an explicit `registry`
endpoint, but do not include checksums because v0.1 does not fetch registry
archives. Public registry fetching and full version solving are deliberately
left for later versions. A dependency must specify exactly one source among
`path`, `git`, and `version`.
If the same canonical package ID resolves to conflicting sources or versions,
v0.1 reports an error instead of trying to solve multiple versions.

## Using `nomoc` (compiler driver)

`nomoc` works directly on a single source file rather than a project:

```bash
nomoc check <source.nomo> [--json-errors]        # parse and type-check
nomoc build <source.nomo> [--emit-c] [--out path] [--json-errors] # emit C99
```

The `--json-errors` flag produces machine-readable diagnostics (with positions
and fix suggestions) suitable for tooling on `check` and `build`.

## Library crate

The `nomo` library exposes the compiler pipeline for embedding. Key entry points
include `check_source`, `check_source_text` and `compile_source_to_c`, alongside
the `lexer`, `parser`, `ast`, `compiler`, `codegen`, `diagnostic` and `project`
modules.

## Tests and examples

```bash
cargo test
```

Runnable sample programs live under [`examples/`](examples/).

## License

MIT. See [LICENSE](LICENSE).
