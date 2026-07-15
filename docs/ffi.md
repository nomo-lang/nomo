# Typed C FFI

Nomo keeps C interop explicit: declarations describe the ABI, every foreign
call remains inside `unsafe`, and raw pointer arithmetic is not exposed.

## Typed handles

Declare a foreign handle family and its release function:

```nomo
extern opaque type FileHandle release file_close

extern "C" {
    fn file_open() -> Nullable<Owned<FileHandle>>
    fn file_marker(handle: Borrowed<FileHandle>) -> i32
    fn file_close(handle: Owned<FileHandle>) -> void
}
```

Handle types are nominal and cannot be constructed or mixed. `Owned<T>` marks
an ownership-transfer boundary, `Borrowed<T>` marks a temporary view, and
`.borrow()` obtains that view from an owned handle. The compiler validates the
release signature, but this preview still uses explicit `close` calls and does
not implement full linear move tracking or automatic destruction.

`Nullable<T>` is available only for handles. Use `is_null()` to inspect it and
`unwrap()` for an explicit checked conversion; unwrapping null panics and exits.

## C records and callbacks

`#[repr(C)]` is restricted to non-generic fixed-layout records with ABI-safe
fields. The compiler computes size, alignment, and offsets from the selected
target ABI and rejects ordinary structs at a by-value extern boundary.

```nomo
#[repr(C)]
pub struct Point {
    pub x: i32
    pub y: i32
}

extern "C" {
    fn point_sum(point: Point) -> i32
    fn apply(value: i32, callback: extern "C" fn(i32) -> i32) -> i32
}
```

Callbacks must be exact-signature, non-capturing top-level functions. They may
not be stored, returned, retained by C, or invoked from a foreign thread. Panic
uses the fail-fast runtime path and never unwinds through C.

## Generate bindings

Generate ordinary Nomo source and an auditable provenance record:

```bash
nomo ffi bindgen native/api.h \
  --package app.bindings \
  --output src/bindings.nomo \
  --provenance bindings.provenance.json
```

If `--provenance` is omitted, the default is
`<output>.provenance.json`. Output is deterministic for the same header,
package, and toolchain version. Provenance records the source path, SHA-256,
generator version, declaration counts, and limitations.

The controlled header subset supports:

- opaque `typedef struct Name Name;` declarations;
- fixed-field struct typedefs;
- ordinary function declarations;
- restricted function-pointer parameters;
- fixed-width integer types, `int`, `double`, `_Bool`/`bool`, `size_t`, and
  `const char *` parameters.

It deliberately rejects unions, bitfields, arrays, flexible arrays, variadic
functions, multiple pointer indirection, and unknown scalar spellings. Suffixes
`_close`, `_free`, `_destroy`, and `_release` identify single-handle release
functions deterministically; this heuristic drives `Owned`/`Borrowed` output,
so generated bindings must be reviewed before use. The command does not run C
code or modify the build graph.

Checked-in generated source and provenance are included in package checksums,
publish archives, and vendoring like other package files. Native sources and
linker settings remain in the manifest `[ffi]` table.
