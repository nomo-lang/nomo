# Nomo repository agent guide

This file governs work inside the `nomo` compiler repository. Organization-wide
branch, signing, pull-request, CI, and coordination rules still apply.

## Write gate

- Confirm the checkout is a clean synchronized `main` and that no active task
  owns this repository before creating a branch.
- Syntax, semantic, manifest, diagnostic, standard-library API, or public CLI
  changes require a bilingual Proposed RFC merged in `rfcs` first.
- Never overwrite, stash, reformat, commit, or otherwise absorb another task's
  working tree.
- Use a focused feature branch, signed commits, a pull request, required CI,
  merge, and a final clean synchronized `main`.

## Boundary ownership

| Surface | Primary owner | Required downstream review |
| --- | --- | --- |
| Tokens, grammar-facing AST, parser | `crates/nomo_syntax` | formatter, compiler, docs, LSP bridge, Tree-sitter/editors |
| Module graph, typing, diagnostics, lowering | `crates/nomo_compiler` | CLI, C99/WASM, LSP, examples |
| C99 code generation | `crates/nomo_codegen_c` | runtime ABI, native fixtures, cross-target gates |
| Native/runtime support | `crates/nomo_runtime` | generated C, lifecycle counters, Unix/Windows/WASM capability gates |
| WebAssembly | `crates/nomo_wasm` | import-free verifier, Playground artifact/provenance |
| Project/CLI/package operations | `crates/nomo` plus manifest/lockfile/resolver crates | help text, integration tests, README, setup action |
| Standard library | `std/` plus `std/intrinsics.toml` | compiler registry, docs, LSP completion, examples |
| Shared editor semantics | `crates/nomo_lsp_bridge` | pinned `nomo-lsp` revision and editor fixtures |

Keep reusable language/runtime behavior in these boundaries. Do not solve a
language-wide gap with application-specific C or Rust glue.

## Canonical syntax contract

- The source package root is derived only from `[package].name` converted to
  lower_snake_case.
- `src/main.nomo` declares the root; nested `main.nomo` declares its directory
  module. Namespace, canonical owner/package identity, and dependency aliases
  do not enter the package's own declaration.
- E0904 covers entry and imported-module package/path mismatches.
- No-return declarations canonically omit `-> void`; value-position `void` and
  callable return types remain explicit.
- Legacy syntax may appear only in tests whose name or assertion proves the
  documented compatibility/diagnostic window.

## Verification

Run focused tests while iterating, then the applicable repository gates:

```sh
cargo fmt --check
cargo clippy --workspace --locked -- -D warnings
cargo test --workspace --locked
cargo build --workspace --release --locked
cargo run --locked --bin nomo -- fmt --check examples
cargo run --locked --bin nomo -- fmt --check std
python3 scripts/check_syntax_governance.py --nomo target/release/nomo
python3 -m unittest discover -s scripts/tests
cargo build --locked --release --target wasm32-unknown-unknown -p nomo-wasm
node scripts/check_browser_wasm.mjs \
  target/wasm32-unknown-unknown/release/nomo_wasm.wasm
```

For async/runtime changes also run the affected native lifecycle integration
tests, counter manifests, generated-C symbol assertions, AddressSanitizer
fixtures, and Windows/macOS/Linux gates represented in CI. For package changes,
exercise locked/offline/frozen, workspace, vendoring, signature, transparency,
and archive paths as applicable.

## Evidence and documentation

- Treat compiler tests and CI as implementation evidence, not automatic
  production readiness.
- Preserve versioned benchmark payloads, toolchain pins, output bytes, schemas,
  counters, and claim-eligibility rules.
- Update `examples/`, diagnostics docs, CLI help, README, SPEC/RFC status, LSP,
  grammar/editors, Playground, and website whenever the changed contract reaches
  those surfaces.
- Do not edit generated build outputs or benchmark results as source.
