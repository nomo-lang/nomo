# nomo

The reference compiler and project tooling for the [Nomo](https://github.com/nomo-lang)
programming language.

Nomo is a small language for systems tools, command-line programs and small
services. This crate is the heart of the ecosystem: it implements the compiler
front-end (lexer, parser, AST, type/semantic checks and diagnostics) and a C99
back-end. `.nomo` source is translated to C99 and then handed to the system C
compiler (`cc`) to produce a native executable.

This repository ships two binaries and one library:

- `nomo` — the project manager (`new` / `check` / `build` / `run`).
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
nomo clean [path]                 # remove generated build artifacts
```

A project is a directory containing a `nomo.toml` manifest and a `src/main.nomo`
entry point. `nomo build` writes generated C to `build/c/main.c` and the linked
executable to `build/bin/<name>`.

```bash
nomo new hello
cd hello
nomo run
```

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
