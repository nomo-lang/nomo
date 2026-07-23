import { readFile } from "node:fs/promises";

const [wasmPath] = process.argv.slice(2);
if (!wasmPath) {
  throw new Error("usage: node scripts/check_browser_wasm.mjs <module.wasm>");
}

const bytes = await readFile(wasmPath);
const module = new WebAssembly.Module(bytes);
const imports = WebAssembly.Module.imports(module);
if (imports.length !== 0) {
  throw new Error(`browser WASM must have no host imports: ${JSON.stringify(imports)}`);
}

const requiredExports = new Set([
  "memory",
  "nomo_alloc",
  "nomo_check",
  "nomo_dealloc",
  "nomo_result_len",
  "nomo_result_ptr",
  "nomo_run",
]);
const exports = new Set(WebAssembly.Module.exports(module).map(({ name }) => name));
for (const name of requiredExports) {
  if (!exports.has(name)) {
    throw new Error(`browser WASM is missing required export ${name}`);
  }
}

const { exports: runtime } = await WebAssembly.instantiate(module, {});
if (runtime.nomo_alloc(256 * 1024 + 1) !== 0) {
  throw new Error("browser WASM accepted source beyond the 256 KiB limit");
}
const source = `package app.main

import std.io
import std.num

fn greeting() -> string {
    return "Hello, WASM"
}

fn main() -> void {
    let message: string = greeting()
    let mut i: u64 = 0
    for i < 3 {
        io.println(message)
        io.println(num.to_string(i))
        i++
    }
}
`;
const input = new TextEncoder().encode(source);
const inputPointer = runtime.nomo_alloc(input.length);
if (inputPointer === 0) {
  throw new Error("browser WASM rejected the smoke-test source allocation");
}
new Uint8Array(runtime.memory.buffer, inputPointer, input.length).set(input);
runtime.nomo_run(inputPointer, input.length, 100_000n, 64 * 1024);
const resultPointer = runtime.nomo_result_ptr();
const resultLength = runtime.nomo_result_len();
const result = JSON.parse(
  new TextDecoder().decode(
    new Uint8Array(runtime.memory.buffer, resultPointer, resultLength),
  ),
);
runtime.nomo_dealloc(inputPointer, input.length);

if (result.status !== "success") {
  throw new Error(`browser WASM smoke failed: ${JSON.stringify(result)}`);
}
if (result.stdout !== "Hello, WASM\n0\nHello, WASM\n1\nHello, WASM\n2\n") {
  throw new Error(`unexpected browser WASM output: ${JSON.stringify(result.stdout)}`);
}

let memoryLimitEnforced = false;
try {
  runtime.memory.grow(1024);
} catch (error) {
  memoryLimitEnforced = error instanceof RangeError;
}
if (!memoryLimitEnforced) {
  throw new Error("browser WASM memory is not capped at 64 MiB");
}

console.log(
  JSON.stringify({
    status: result.status,
    engine: result.engine,
    imports: imports.length,
    memoryLimitMiB: 64,
    bytes: bytes.length,
    steps: result.stats.steps,
  }),
);
