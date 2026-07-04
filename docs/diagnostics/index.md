# Nomo Diagnostics

Nomo diagnostics use stable `E`-prefixed error codes. Human-readable compiler
output, JSON diagnostics, LSP diagnostics, and editor quick fixes should all use
the same code.

## Ranges

| Range | Category |
| --- | --- |
| `E0100-E0199` | Lexer, comments, and tokenization |
| `E0200-E0299` | Parser and syntax |
| `E0300-E0399` | Name resolution |
| `E0400-E0499` | Type checking |
| `E0500-E0599` | Mutability, borrow, and escape checks |
| `E0600-E0699` | Module, package, and visibility |
| `E0700-E0799` | C backend and runtime layout |
| `E0800-E0899` | Standard library and runtime API |
| `E0900-E0999` | Manifest, lockfile, and dependency resolver |
| `E1000-E1099` | Workspace |
| `E1100-E1199` | Test runner |
| `E1200-E1299` | Documentation generator |
| `E1300-E1399` | LSP semantic API |
| `E1400-E1499` | Registry and publish |
| `E1500-E1599` | FFI and unsafe |

## Documented Codes

- [E0102](E0102.md) - unexpected character
- [E0200](E0200.md) - missing package declaration
- [E0301](E0301.md) - unknown name or missing import
- [E0404](E0404.md) - type mismatch
- [E0501](E0501.md) - immutable value mutation
- [E0901](E0901.md) - manifest or project configuration error
- [E1500](E1500.md) - expected interface declaration
- [E1501](E1501.md) - missing interface body
- [E1502](E1502.md) - unterminated interface body
- [E1503](E1503.md) - invalid interface member
- [E1504](E1504.md) - expected interface method
- [E1505](E1505.md) - missing interface method parameters
- [E1510](E1510.md) - expected extern declaration
- [E1511](E1511.md) - unsupported extern ABI
- [E1512](E1512.md) - missing extern block body
- [E1513](E1513.md) - unterminated extern block
- [E1514](E1514.md) - invalid extern declaration
- [E1515](E1515.md) - expected extern function
- [E1516](E1516.md) - missing extern function parameters
- [E1517](E1517.md) - expected unsafe block
- [E1518](E1518.md) - missing unsafe block body
- [E1519](E1519.md) - invalid FFI or unsafe boundary

More codes should be added as implementation slices stabilize their diagnostics.
