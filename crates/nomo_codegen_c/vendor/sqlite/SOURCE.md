# Bundled SQLite Source

- Upstream: https://www.sqlite.org/
- Version: 3.53.3
- Archive: `sqlite-amalgamation-3530300.zip`
- Download URL: https://www.sqlite.org/2026/sqlite-amalgamation-3530300.zip
- Retrieved: 2026-07-25
- Archive SHA3-256: `d45c688a8cb23f68611a894a756a12d7eb6ab6e9e2468ca70adbeab3808b5ab9`
- `sqlite3.c` SHA3-256: `28e484abdaa43630e34040ef6ed92be973a1ad54107803d8af5145b889c23ed7`
- `sqlite3.h` SHA3-256: `8444bae1916728ffb4a8c7b00434616b12cf3df813b3dd52127849a7c0387d9c`
- `sqlite3.c` SHA-256: `87497ab605bedd0dbee27a209c1eeff8c89b229b13f921a7efdbb81a13f779fd`
- `sqlite3.h` SHA-256: `4ff81af4849acabc76fc8349abb926814395072617ca18e08800abf734ab7612`

Only the upstream `sqlite3.c` and `sqlite3.h` amalgamation files are bundled.
Nomo does not patch either file. The CLI materializes and compiles them only
for programs that use `std.sqlite`.

## Public-domain notice

SQLite is dedicated to the public domain by its authors. The upstream
copyright and public-domain statement is:
https://www.sqlite.org/copyright.html

## Nomo compile options

The emitted `sqlite3-SOURCE.md` accompanies `sqlite3.c` and `sqlite3.h`.
Rebuild the amalgamation as a separate C99 translation unit with these pinned
preprocessor definitions:

```text
SQLITE_THREADSAFE=1
SQLITE_DQS=0
SQLITE_DEFAULT_FOREIGN_KEYS=1
SQLITE_DEFAULT_MEMSTATUS=0
SQLITE_TRUSTED_SCHEMA=0
SQLITE_ENABLE_API_ARMOR
SQLITE_OMIT_LOAD_EXTENSION
SQLITE_MAX_LENGTH=16777216
SQLITE_MAX_SQL_LENGTH=1048576
SQLITE_MAX_COLUMN=256
SQLITE_MAX_VARIABLE_NUMBER=1024
SQLITE_MAX_EXPR_DEPTH=100
SQLITE_MAX_FUNCTION_ARG=100
SQLITE_MAX_COMPOUND_SELECT=64
SQLITE_MAX_LIKE_PATTERN_LENGTH=1024
```

The toolchain cache key includes the SQLite version, both extracted-file
digests, this compile-option set, and the Nomo wrapper source.
