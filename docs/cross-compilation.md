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

Dependencies use a restricted, statically evaluated target predicate. Values
may be one string or a set; dimensions are combined with `and`, while values
within one dimension are combined with `or`:

```toml
[dependencies]
unix = { package = "nomo-lang/unix", version = "1.0.0", target = { os = ["linux", "macos"] } }
simd = { package = "nomo-lang/simd", path = "../simd", target = { arch = ["x86_64", "arm64"] } }

[ffi]
libraries = ["common"]

[[ffi.target]]
os = "linux"
sources = ["native/linux.c"]
libraries = ["pthread"]
```

The supported predicate fields are `arch`, `os`, and `env`. The same aliases as
target triples are accepted and canonicalized. Arbitrary expressions,
negation, environment-variable reads, and build-script execution are rejected.
Conditions that cannot match one of the six recognized targets fail manifest
validation.

`nomo deps resolve` records the complete known graph. Conditional lockfile
edges retain their canonical predicate instead of deleting edges that are
inactive on the resolving host. `nomo deps tree --target <triple>` shows the
filtered graph used for that target. Workspace ordering, module discovery, FFI
metadata, and compilation apply the same filter.

Explicit-target artifacts are isolated under
`build/<canonical-target>/{c,bin}`. Generated C records
`NOMO_TARGET_TRIPLE`, `NOMO_TARGET_ARCH`, and `NOMO_TARGET_PLATFORM`; the
standard-library `std.os` helpers therefore report the selected compiler target
rather than the machine running `nomo`.

Two native cross-link families are configured:

- Apple silicon and macOS x86-64 use Apple Clang's explicit `-target` option
  and the installed macOS SDK. CI runs on an arm64 macOS host, links an x86-64
  executable, verifies its Mach-O architecture with `lipo` and `file`, and
  uploads target-scoped evidence.
- GNU/Linux x86-64 and arm64 use the target-prefixed GNU compiler
  (`aarch64-linux-gnu-gcc` or `x86_64-linux-gnu-gcc`). CI installs the arm64
  compiler and sysroot on an x86-64 host, links an AArch64 ELF, verifies it with
  `readelf` and `file`, executes it with QEMU against the target sysroot, and
  uploads the target-scoped artifacts.

Other recognized host/target pairs support target-aware C emission today.
Native linking to an unconfigured non-host pair fails with a diagnostic until
an explicit compiler, linker, and sysroot bundle is added. User-defined JSON
targets remain a future RFC 0017 extension; arbitrary build scripts are not
part of this design.
