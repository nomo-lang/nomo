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

const encoder = new TextEncoder();
const decoder = new TextDecoder();
const runSource = (source) => {
  const input = encoder.encode(source);
  const inputPointer = runtime.nomo_alloc(input.length);
  if (inputPointer === 0) {
    throw new Error("browser WASM rejected source allocation");
  }
  try {
    new Uint8Array(runtime.memory.buffer, inputPointer, input.length).set(input);
    runtime.nomo_run(inputPointer, input.length, 100_000n, 64 * 1024);
    const resultPointer = runtime.nomo_result_ptr();
    const resultLength = runtime.nomo_result_len();
    return JSON.parse(
      decoder.decode(
        new Uint8Array(runtime.memory.buffer, resultPointer, resultLength),
      ),
    );
  } finally {
    runtime.nomo_dealloc(inputPointer, input.length);
  }
};

const smokeSource = `package app.main

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
const result = runSource(smokeSource);

if (result.status !== "success") {
  throw new Error(`browser WASM smoke failed: ${JSON.stringify(result)}`);
}
if (result.stdout !== "Hello, WASM\n0\nHello, WASM\n1\nHello, WASM\n2\n") {
  throw new Error(`unexpected browser WASM output: ${JSON.stringify(result.stdout)}`);
}

const processSource = `package app.main

import std.process

fn command() -> ProcessCommand {
    panic("release-wasm-process-command-must-not-run")
}

fn timeout() -> u64 {
    panic("release-wasm-process-timeout-must-not-run")
}

suspend fn main() -> void {
    let result: Result<ProcessChild, ProcessControlError> = process.start(command(), timeout())
}
`;
const processResult = runSource(processSource);
if (
  processResult.status !== "runtime_error" ||
  processResult.runtime_error?.code !== "NOMO-WASM-003" ||
  !processResult.runtime_error.message.includes("process") ||
  !processResult.runtime_error.message.includes("browser sandbox")
) {
  throw new Error(
    `browser WASM process capability gate failed: ${JSON.stringify(processResult)}`,
  );
}
for (const secret of [
  "release-wasm-process-command-must-not-run",
  "release-wasm-process-timeout-must-not-run",
  "__nomo_process_start_async",
]) {
  if (
    processResult.runtime_error.message.includes(secret) ||
    processResult.stderr.includes(secret)
  ) {
    throw new Error(
      `browser WASM process capability gate leaked or evaluated ${secret}`,
    );
  }
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
    processCapability: processResult.runtime_error.code,
    steps: result.stats.steps,
  }),
);
