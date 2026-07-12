# Editor Integrations

Nomo uses one language server across all supported editors. Install the
`nomo-lsp` binary for your platform from the
[`nomo-lsp` releases](https://github.com/nomo-lang/nomo-lsp/releases), extract
it, and place it on `PATH` before installing an editor extension. Source builds
only require the `nomo-lsp` checkout; its Cargo manifest fetches the pinned
compiler revision from Git automatically.

The language server provides compiler diagnostics, completion, hover,
go-to-definition, references, rename, document and workspace symbols, semantic
tokens, code actions, formatting, and inlay hints. Editor extensions add native
file registration, syntax highlighting, and client-specific configuration.

## VS Code and Open VSX Editors

Install the VSIX attached to a
[`vscode-nomo` release](https://github.com/nomo-lang/vscode-nomo/releases), or
install `nomo-lang.vscode-nomo` from the Visual Studio Marketplace or Open VSX
after the first registry publication. The `nomo.lsp.path` setting overrides the
default `nomo-lsp` executable lookup.

## Zed

Until the extension is accepted into the Zed extension registry, clone
[`zed-nomo`](https://github.com/nomo-lang/zed-nomo) and use **Extensions: Install
Dev Extension**. The extension pins the exact
[`tree-sitter-nomo`](https://github.com/nomo-lang/tree-sitter-nomo) grammar
revision recorded in `extension.toml` and finds `nomo-lsp` on `PATH`.

## JetBrains IDEs

Install the ZIP attached to an
[`intellij-nomo` release](https://github.com/nomo-lang/intellij-nomo/releases)
with **Settings > Plugins > Install Plugin from Disk**. Marketplace releases
can be installed by searching for **Nomo** after the first publication. The
plugin uses LSP4IJ and finds `nomo-lsp` on `PATH`.

## Release Compatibility

The preview toolchain, language server, grammar, and editor extensions use the
same `0.1.x` compatibility line. A tagged `nomo-lsp` release uses the compiler
revision pinned in its `Cargo.toml`, and each editor release workflow verifies
that its tag matches its package or extension version. Grammar consumers pin a
Git commit so highlighting changes remain reproducible independently of npm
publication.

The language server resolves source-defined standard-library symbols to the
canonical `std/src/*.nomo` files. This makes hover and go-to-definition useful
for imported `std.option`, `std.result`, `std.array`, and `std.string` APIs while
leaving representation-only `Array` layout navigation anchored to its source
module during the intrinsic migration.
