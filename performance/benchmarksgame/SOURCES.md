# Sources and derivation

The reference programs and the Nomo transliterations in this directory are
covered by the Benchmarks Game BSD 3-Clause license in
`LICENSE-BENCHMARKSGAME.txt`. The rest of the Nomo repository remains under
its existing license.

The six official `#8` program pages were fetched on 2026-07-27. The first
HTML `<pre>` block was decoded as UTF-8, HTML character references were
resolved, a single page-layout leading newline was removed, and one final
newline was enforced before the upstream-extracted SHA-256 was calculated.
The checked-in C and Go files retain that source code and comments with
trailing whitespace normalized for repository hygiene. The manifest records
both the upstream-extracted SHA and the exact checked-in file SHA.

| Workload | Reference | Official page | Upstream-extracted SHA-256 | Checked-in SHA-256 |
| --- | --- | --- | --- | --- |
| spectral-norm | C gcc #8 | <https://benchmarksgame-team.pages.debian.net/benchmarksgame/program/spectralnorm-gcc-8.html> | `2fd32f77fbab6513c9e7330f16e249458dd7abdb5138a9d4694652af3e84f4f5` | `1f7f71ce5fc6f87432b3801fb57c3e8a619da2527c1b801154b8102c7af66c3e` |
| spectral-norm | Go #8 | <https://benchmarksgame-team.pages.debian.net/benchmarksgame/program/spectralnorm-go-8.html> | `8f277cc7c9f4b0cb287c9bd697d047fc8d40562d960030d9c718a948073dd6ab` | `862a4ca6a79a7457c253e88a77b0189583201fb551b86aec7221ce7c3e079810` |
| n-body | C gcc #8 | <https://benchmarksgame-team.pages.debian.net/benchmarksgame/program/nbody-gcc-8.html> | `e6e067338140f86ec3aca2b43e2770f3dbe6c7c2d6108934671c58f00af13317` | `a8649dd7babc5b9178fc363f4d61b468662c703668c2f8f4ddeab206b3e7e879` |
| n-body | Go #8 | <https://benchmarksgame-team.pages.debian.net/benchmarksgame/program/nbody-go-8.html> | `31b4bbb15ee17fcbce6e37a5c3c7ea3b94a369d335fd9bb69edfb361f866f80e` | `83e645802266f9d30a1093e97fc934dbb7dc6bd55ada49f58f45d63340c6e76a` |
| fannkuch-redux | C gcc #8 | <https://benchmarksgame-team.pages.debian.net/benchmarksgame/program/fannkuchredux-gcc-8.html> | `65c8d4b7b5697e1ee1ccff75aec8fb66ae97811fbd96b02a0ef46941c49dea97` | `4d3135b2ed7a2fedb12b731c0f1a6bf901d763ac8421208fab6c4997c3ca9d80` |
| fannkuch-redux | Go #8 | <https://benchmarksgame-team.pages.debian.net/benchmarksgame/program/fannkuchredux-go-8.html> | `e746d5097b2ecde2df42387b1da17a0d8c4214d5ffdc396dd2498b11ba19c3e4` | `806403dc4801db2c7c8894f923d18bb3f0b8ed3741ac48890a65ccd38fcaa4a2` |

The Nomo programs are direct scalar transliterations of the corresponding
naive `#8` control flow and arithmetic. Nomo-specific adaptations are limited
to explicit `u64` indices, current `Array<T>` construction, explicit
copy-on-write writeback of struct values, current argument parsing, and the
private `format_fixed_9` used by spectral-norm and n-body. They do not use FFI,
threads, `suspend`, handwritten C, SIMD, disabled bounds checks, or edited
generated C.

Method and license sources:

- <https://benchmarksgame-team.pages.debian.net/benchmarksgame/performance/comparable.html>
- <https://benchmarksgame-team.pages.debian.net/benchmarksgame/how-programs-are-measured.html>
- <https://benchmarksgame-team.pages.debian.net/benchmarksgame/license.html>
