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

- [E0101](E0101.md) - unterminated string literal
- [E0102](E0102.md) - unexpected character
- [E0103](E0103.md) - integer literal too large
- [E0104](E0104.md) - invalid character literal or reserved word
- [E0105](E0105.md) - unknown character escape
- [E0106](E0106.md) - empty character literal
- [E0107](E0107.md) - character literal has too many characters
- [E0108](E0108.md) - unterminated block comment
- [E0200](E0200.md) - missing package declaration
- [E0201](E0201.md) - invalid top-level declaration order
- [E0202](E0202.md) - expected function declaration
- [E0203](E0203.md) - missing function parameter list
- [E0206](E0206.md) - missing function body
- [E0207](E0207.md) - unterminated function body
- [E0208](E0208.md) - expected expression
- [E0209](E0209.md) - missing closing parenthesis
- [E0210](E0210.md) - invalid call argument list
- [E0211](E0211.md) - missing required newline
- [E0212](E0212.md) - invalid let statement
- [E0213](E0213.md) - missing initializer
- [E0214](E0214.md) - missing parameter type separator
- [E0215](E0215.md) - invalid parameter separator
- [E0216](E0216.md) - expected return statement
- [E0217](E0217.md) - invalid assignment or postfix update
- [E0218](E0218.md) - expected struct declaration
- [E0219](E0219.md) - missing struct body
- [E0220](E0220.md) - unterminated struct body
- [E0221](E0221.md) - missing struct field type separator
- [E0222](E0222.md) - missing struct literal body
- [E0223](E0223.md) - missing struct literal field separator
- [E0224](E0224.md) - invalid struct literal field separator
- [E0225](E0225.md) - unterminated struct literal
- [E0226](E0226.md) - expected enum declaration
- [E0227](E0227.md) - missing enum body
- [E0228](E0228.md) - unterminated enum body
- [E0229](E0229.md) - expected match expression
- [E0230](E0230.md) - missing match body
- [E0231](E0231.md) - unterminated match body
- [E0232](E0232.md) - missing match arm arrow
- [E0233](E0233.md) - missing enum payload closing parenthesis
- [E0234](E0234.md) - invalid match binding
- [E0235](E0235.md) - invalid generic parameter or match arm body
- [E0236](E0236.md) - invalid generic type argument separator
- [E0237](E0237.md) - duplicate generic type parameter
- [E0238](E0238.md) - unsupported match wildcard
- [E0240](E0240.md) - expected if expression
- [E0241](E0241.md) - missing if branch body
- [E0242](E0242.md) - unterminated expression block
- [E0244](E0244.md) - missing else branch
- [E0245](E0245.md) - missing else branch body
- [E0246](E0246.md) - expected panic expression
- [E0247](E0247.md) - missing panic argument list
- [E0248](E0248.md) - missing panic closing parenthesis
- [E0250](E0250.md) - expected impl block
- [E0251](E0251.md) - invalid impl target type
- [E0252](E0252.md) - missing impl body
- [E0253](E0253.md) - unterminated impl body
- [E0254](E0254.md) - invalid impl body member
- [E0255](E0255.md) - unsupported impl target
- [E0256](E0256.md) - invalid method self parameter
- [E0257](E0257.md) - invalid method receiver type
- [E0258](E0258.md) - invalid interface implementation
- [E0260](E0260.md) - expected for statement
- [E0261](E0261.md) - missing infinite for body
- [E0262](E0262.md) - missing for-in separator
- [E0263](E0263.md) - missing for-in body
- [E0264](E0264.md) - missing conditional for body
- [E0265](E0265.md) - expected defer statement
- [E0266](E0266.md) - unterminated statement block
- [E0267](E0267.md) - missing let-else else or const declaration
- [E0268](E0268.md) - missing let-else body or const type separator
- [E0269](E0269.md) - missing const initializer or if-let statement
- [E0270](E0270.md) - missing if-let let keyword
- [E0271](E0271.md) - missing if-let initializer separator
- [E0272](E0272.md) - missing if-let body
- [E0273](E0273.md) - missing if-let else body
- [E0274](E0274.md) - unsupported wildcard import
- [E0300](E0300.md) - expected identifier
- [E0301](E0301.md) - unknown name or missing import
- [E0302](E0302.md) - duplicate local binding
- [E0303](E0303.md) - unknown variable
- [E0304](E0304.md) - duplicate callable or constant definition
- [E0305](E0305.md) - unknown function or non-callable local
- [E0306](E0306.md) - duplicate struct definition
- [E0307](E0307.md) - duplicate struct field
- [E0308](E0308.md) - unknown struct field
- [E0309](E0309.md) - unknown struct or impl target
- [E0403](E0403.md) - unsupported type or required type annotation
- [E0404](E0404.md) - type mismatch
- [E0420](E0420.md) - invalid question operator carrier
- [E0421](E0421.md) - incompatible question operator propagation
- [E0422](E0422.md) - unsupported question operator position
- [E0501](E0501.md) - immutable value mutation
- [E0901](E0901.md) - manifest or project configuration error
- [E0902](E0902.md) - project source processing error
- [E0903](E0903.md) - module not found
- [E0904](E0904.md) - module package mismatch
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
