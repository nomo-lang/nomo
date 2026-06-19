# Nomo v0.1 Examples

Each example is a standalone Nomo project:

```sh
nomo check examples/hello
nomo run examples/hello
```

The examples track the v0.1 acceptance matrix in the RFC specification.

- `hello`: minimal `std.io` output
- `args`: `std.env.args()` and `Option<string>`
- `read_file`: `std.fs` and `Result`
- `result_chain`: `Result` with `?`
- `specific_array_new`: concrete `Array.new<T>` import
- `specific_import`: concrete function imports
- `specific_value_import`: concrete value-function imports
- `struct_methods`: struct methods
- `array_basic`: `Array<i32>` mutation and access
- `generic_function`: generic function instantiation
- `generic_struct`: generic struct instantiation
- `enum_struct_payload`: enum payloads carrying local structs
