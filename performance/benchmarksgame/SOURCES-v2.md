# V2 sources, licensing, and derivation

The v2 C++20 and semantic-C sources are BSD 3-Clause derivatives covered by
`LICENSE-BENCHMARKSGAME.txt`. Frozen official C and Go sources, audited
upstream URLs, retrieval dates, and upstream-extracted hashes remain in
`SOURCES.md`. The v2 predecessor is nomo
`c6712c1da1f65fcbdf0ce037224d11482b6a7e35` with manifest SHA-256
`bd8e5016fb376741478806d13585ebc37ade2104995bd411a2a161592f65c15f`.

## Standard C++20 derivatives

Each C++ file preserves official C #8 control flow, loop order, arithmetic,
mutation, output, and one contiguous storage object per source array. The
governing ISO C++20 clarification is RFC 0043 merge
`75a7e14adc1ea06ccdc9a28c1dc0676ce8404a1c`.

ISO C++20 has no runtime VLA. Spectral-norm and fannkuch-redux therefore map
each C VLA from stack storage to one standard contiguous RAII dynamic array.
This storage-class difference is recorded in the manifest and every result
source lock. The mapping preserves array count, element type, logical
capacity, lexical lifetime, per-call allocation frequency, initialization
work, and access order. Each array is constructed once at final capacity and
never grows or reallocates. `std::unique_ptr<T[]>` avoids the extra
value-initialization that a sized `std::vector<T>` would introduce. There is no
custom allocator, stronger algorithm, precomputation, thread, or SIMD path.
The frozen C #8 remains an independent decisive comparator.

Every C++ build uses
`clang++ --no-default-config -std=c++20 -pedantic-errors -O3 -DNDEBUG -fomit-frame-pointer`
plus `-lm` where needed. Unit and CI tests repeat strict conformance,
allocation-site, forbidden-construct, hash, and small-output checks.

| Workload | Frozen C | ISO C++20 | SHA-256 | Allocation mapping |
| --- | --- | --- | --- | --- |
| spectral-norm | `reference/c/spectral-norm.c` | `reference/cpp/spectral-norm.cpp` | `81489a76f22b02f67cd51f03753eaf45e05c459a08c1c69e50d6812a9acdd4b2` | Each `double[N]` VLA maps to one `std::unique_ptr<double[]>` at the corresponding lexical site. Three allocation sites and initialization/access order remain; stack becomes dynamic. |
| n-body | `reference/c/n-body.c` | `reference/cpp/n-body.cpp` | `1c0b0942c7075dbfcfa3a2b08ee486f284325e17525b2c2f86301c0ec0a2b492` | The fixed five-element C array maps to `std::array<planet, 5>` with fixed stack storage. |
| fannkuch-redux | `reference/c/fannkuch-redux.c` | `reference/cpp/fannkuch-redux.cpp` | `28db75a4483148d430a2312dfd9088c46edf21b661c062d7ca90caf27209d962` | The three `int[n]` VLAs map to three `std::unique_ptr<int[]>` allocations in the same function lifetime. Initialization/access order remains; stack becomes dynamic. |

## Semantic-C diagnostics

Semantic-C is non-decisional. These controls derive from the official C and
frozen Nomo transliterations, making selected Nomo-side work explicit in C:
heap-backed length-carrying arrays, bounds checks, and value load/writeback
where applicable. They do not reproduce the full Nomo Runtime and never enter
a workload or suite verdict.

| Workload | Source | SHA-256 |
| --- | --- | --- |
| spectral-norm | `reference/semantic-c/spectral-norm.c` | `6951782dd59f50eb9eca1163b59d925fcda3bb8a88bdc7e722a43153fa5c1dc3` |
| n-body | `reference/semantic-c/n-body.c` | `284d2282e34a4a43c64dddc034cc7d087d7f1c62ed5c3097a7321d94f3123cd5` |
| fannkuch-redux | `reference/semantic-c/fannkuch-redux.c` | `35141667a9a8ac43cfcf1d47f98e401646901a108c41dceec4da35f5891b4669` |

Semantic-C uses the fixed `clang --no-default-config` C99 optimization
baseline. Its output is
checked on small inputs, but its timing is excluded from both five-lane
Williams protocols and every acceptance calculation.
