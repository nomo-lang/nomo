# Cross Compilation

Nomo uses one canonical target model from the CLI through C emission and native
linking. A canonical target has four components:

```text
arch-vendor-os-env
```

The current target families are:

- `x86_64-unknown-linux-gnu` and `aarch64-unknown-linux-gnu`
- `x86_64-apple-darwin-none` and `aarch64-apple-darwin-none`
- `x86_64-pc-windows-msvc` and `aarch64-pc-windows-msvc`

The standard three-part Apple spellings, such as `x86_64-apple-darwin`, are
accepted and canonicalized by adding the explicit `none` environment. `amd64`,
`arm64`, and `macos` are accepted as input aliases, but artifact paths and
generated metadata always use canonical spellings. Unsupported architectures,
operating systems, environments, and incoherent combinations fail before any
build artifacts are created.

Use an explicit target when building a project:

```sh
nomo build . --target x86_64-apple-darwin
nomo build . --target aarch64-unknown-linux-gnu --emit-c
nomoc build src/main.nomo --target aarch64-unknown-linux-gnu --out main.c
```

Explicit-target artifacts are isolated under
`build/<canonical-target>/{c,bin}`. Generated C records
`NOMO_TARGET_TRIPLE`, `NOMO_TARGET_ARCH`, and `NOMO_TARGET_PLATFORM`; the
standard-library `std.os` helpers therefore report the selected compiler target
rather than the machine running `nomo`.

The first configured native cross-link path is between Apple silicon and
macOS x86-64. It uses Apple Clang's explicit `-target` option and the installed
macOS SDK. CI runs on an arm64 macOS host, links an x86-64 executable, verifies
the resulting Mach-O architecture with `lipo` and `file`, and uploads the
target-scoped C and binary artifacts as evidence.

Other recognized targets support target-aware C emission today. Native linking
to a non-host target fails with a diagnostic until an explicit compiler,
linker, and sysroot bundle is configured. Target predicates in manifests,
conditional lockfile edges, and user-defined JSON targets remain future RFC
0017 slices; arbitrary build scripts are not part of this design.
