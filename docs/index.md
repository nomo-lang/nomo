# Nomo Documentation

This directory contains user-facing documentation for the current Nomo preview
toolchain.

- [Package Management](package-management.md) explains manifests, workspaces,
  lockfiles, dependency cache behavior, vendoring, and locked/offline builds.
- [Standard Library](standard-library.md) describes the built-in `std` modules,
  propagation carriers, and v0.1 module boundaries.
- [Editor Integrations](editor-integrations.md) explains how to install
  `nomo-lsp` and the VS Code, Zed, and JetBrains clients.
- [Cross Compilation](cross-compilation.md) documents canonical target triples,
  isolated artifacts, ABI facts, and the first supported native cross-link path.
- [Persistent Incremental Cache](incremental-cache.md) explains cross-process
  semantic/codegen reuse, corruption recovery, capacity controls, and cleanup.
- [Typed C FFI](ffi.md) covers nominal handles, nullability, ownership metadata,
  C records, restricted callbacks, and deterministic header bindings.
- [Diagnostics](diagnostics/index.md) lists stable `E` diagnostic codes used by
  CLI JSON output and LSP diagnostics.
