const defaultWasmUrl = new URL("./__WASM_FILE__", import.meta.url);
const encoder = new TextEncoder();
const decoder = new TextDecoder();

let instance;
let memory;

export async function init(wasm = defaultWasmUrl, imports = {}) {
  const wrappedImports = wrapImports(imports);
  if (wasm instanceof WebAssembly.Module) {
    instance = await WebAssembly.instantiate(wasm, wrappedImports);
  } else {
    const bytes = await readWasmBytes(wasm);
    const result = await WebAssembly.instantiate(bytes, wrappedImports);
    instance = result.instance;
  }
  memory = instance.exports.memory;
  if (!memory) {
    throw new Error("Regulus Wasm module does not export memory");
  }
  return instance.exports;
}

export function callString(name, ...args) {
  ensureInstance();
  const fn = instance.exports[name];
  if (typeof fn !== "function") {
    throw new Error(`Regulus export "${name}" is not a function`);
  }
  return readString(fn(...args.map(writeString)));
}

export function exportString(name) {
  return (...args) => callString(name, ...args);
}

export function writeString(value) {
  ensureInstance();
  const bytes = encoder.encode(String(value));
  const data = instance.exports.__regulus_alloc(bytes.length);
  new Uint8Array(memory.buffer, data, bytes.length).set(bytes);
  return instance.exports.__regulus_string_new(data, bytes.length);
}

export function readString(ptr) {
  ensureInstance();
  const len = instance.exports.__regulus_string_len(ptr);
  const data = instance.exports.__regulus_string_data(ptr);
  const bytes = new Uint8Array(memory.buffer, data, len);
  return decoder.decode(bytes);
}

function wrapImports(imports) {
  const wrapped = {};
  for (const [moduleName, moduleImports] of Object.entries(imports)) {
    wrapped[moduleName] = {};
    for (const [name, fn] of Object.entries(moduleImports)) {
      wrapped[moduleName][name] = (ptr) => writeString(fn(readString(ptr)));
    }
  }
  return wrapped;
}

async function readWasmBytes(wasm) {
  if (wasm instanceof ArrayBuffer || ArrayBuffer.isView(wasm)) {
    return wasm;
  }
  const url = wasm instanceof URL ? wasm : new URL(wasm, import.meta.url);
  if (url.protocol === "file:") {
    const fs = await import("node:fs/promises");
    return fs.readFile(url);
  }
  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(`Could not fetch Wasm artifact: ${response.status}`);
  }
  return response.arrayBuffer();
}

function ensureInstance() {
  if (!instance || !memory) {
    throw new Error("Regulus Wasm module has not been initialized");
  }
}
