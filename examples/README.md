# Nomo v0.1 Examples

Each example is a standalone Nomo project:

```sh
nomo check examples/hello
nomo run examples/hello
```

The examples track the v0.1 acceptance matrix in the RFC specification.

- `hello`: minimal `std.io` output
- `io_print`: `std.io.print` and `std.io.eprint` output without automatic newlines
- `io_stderr`: `std.io.eprintln` writing to stderr while stdout remains independent
- `primitives`: fixed-width primitives, Unicode `char`, and explicit casts
- `comments`: line, block, doc, nested block, and trailing comments
- `operators_arithmetic`: arithmetic precedence, grouping, and unary negation
- `operators_logical`: logical negation and short-circuit boolean operators
- `operators_bitwise`: bitwise operators, bit clear, and shifts
- `operators_assignment`: compound assignment operators and increment/decrement
- `args`: `std.env.args()` and `Option<string>`
- `env_get`: `std.env.get()` and `Option<string>`
- `env_extended`: `std.env.set()`, `cwd()`, `home_dir()`, and `temp_dir()`
- `read_file`: `std.fs` and `Result`
- `file_handle`: `std.fs.open`, `File.close`, and `defer`
- `result_chain`: `Result` with `?`
- `result_helpers`: `Result` predicates, fallback, map, map_err, and and_then helpers with `?`
- `result_map_err`: `Result.map_err(converter)?` across error types
- `result_main`: `main() -> Result<void, E>` entrypoint handling with `map_err(...)?`
- `option_helpers`: `Option` predicates, fallback, map, and and_then helpers
- `option_question`: `Option<T>` early absence propagation with `?`
- `prelude_variants`: unqualified `Ok`/`Err`/`Some`/`None` core variants
- `prelude_shadow`: local names shadow prelude variants while qualified variants remain usable
- `specific_array_new`: concrete `Array.new<T>` and array method imports
- `specific_import`: concrete function imports
- `specific_type_import`: concrete type imports
- `specific_value_import`: concrete value-function imports
- `string_methods`: `string` value methods (`len`, `concat`)
- `string_extended`: `std.string` predicates, trim, ASCII case conversion, and split
- `string_lifecycle`: `string` parameter, return, and caller reuse lifecycle
- `struct_methods`: struct methods
- `mut_field_borrow`: `mut` field-path arguments, parameter forwarding, and non-overlapping field borrows
- `mut_methods`: `mut self` methods writing back through receiver borrows
- `newline_dot`: line-start `.` continuation for method calls and qualified variants
- `option_result_lang_items`: nested `Result<Option<T>, E>` through `?`, prelude variants, and `match`
- `package_path`: non-`app.main` package path through C symbol mangling and runtime execution
- `pub_visibility`: `pub` structs, fields, enums, functions, and methods through runtime execution
- `struct_option_field`: struct fields carrying `Option<T>`
- `struct_result_field`: struct fields carrying `Result<T, E>`
- `array_basic`: `Array<i32>` mutation and access
- `array_enum`: `Array<T>` with enum elements
- `array_get_none`: `Array.get` returning `None` out of bounds
- `array_nested`: nested `Array<Array<T>>` values
- `array_option_lifecycle`: `Option<Array<T>>` reassignment and payload release
- `array_param_return`: `Array<T>` value parameter retained across return
- `array_reassign`: `Array<T>` reassignment retaining new storage and releasing old storage
- `struct_array_lifecycle`: struct fields and custom enum payloads carrying `Array<T>`
- `array_struct`: `Array<T>` with local struct elements
- `array_swap`: `Array.set`, `mut` call arguments, and statement `match` arms
- `array_value_semantics`: `Array<T>` assignment followed by isolated mutation
- `c_keywords`: C keyword-like field and variant names through the C backend
- `generic_function`: generic function instantiation
- `generic_enum`: generic enum instantiation with multiple concrete payload types
- `generic_struct`: generic struct instantiation
- `enum_struct_payload`: enum payloads carrying local structs
- `tail_expression`: final expression as the function return value
- `loops`: `for` in all three forms (`for {}`, `for cond {}`, `for x in xs {}`) with `break`
- `let_else`: `let PATTERN = expr else { ... }` enum payload extraction
- `if_let`: `if let PATTERN = expr { ... }` single-variant enum branching
- `defer`: deferred method calls running in reverse order at scope exit
- `defer_question`: deferred cleanup before `?` early error propagation
- `const`: package-level constants referenced from functions
