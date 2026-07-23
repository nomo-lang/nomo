# Nomo WebAssembly runtime

`nomo-wasm` brings the production Nomo lexer, parser, semantic checker, typed
IR, and a capability-free interpreter into the browser. It is the execution
engine used by `play.nomo-lang.org`.

The WebAssembly module deliberately has no imports. Programs can use pure
language features such as functions, typed variables, arithmetic, conditions,
loops, strings, arrays, structs, enums, and console output. Host-sensitive
standard-library operations are rejected at runtime:

- filesystem and interactive input;
- environment variables and subprocesses;
- network and HTTP;
- clocks, sleeping, and other ambient host state.

Execution is bounded by an instruction-fuel limit, maximum call depth, a
256 KiB source limit, a 64 MiB WebAssembly memory ceiling, and a combined
stdout/stderr byte limit. The browser integration also runs the module inside a
disposable Web Worker and terminates it on a wall-clock timeout.

Build and verify the browser module:

```sh
rustup target add wasm32-unknown-unknown
cargo build --locked --release --target wasm32-unknown-unknown -p nomo-wasm
node scripts/check_browser_wasm.mjs \
  target/wasm32-unknown-unknown/release/nomo_wasm.wasm
```

The raw ABI exports allocation, check/run, and result-buffer functions so the
module does not need JavaScript glue generation:

- `nomo_alloc` / `nomo_dealloc`
- `nomo_check` / `nomo_run`
- `nomo_result_ptr` / `nomo_result_len`
