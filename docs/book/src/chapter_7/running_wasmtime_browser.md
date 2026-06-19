# Running WebAssembly in Wasmtime and the browser

The WebAssembly core specification deliberately separates modules from their
embedding environment. A host decides how modules are loaded, how imports are
provided, and how exports are accessed.[^overview]

Regulus should treat Wasmtime and the browser as different hosts for the same
core module. Both hosts instantiate a module and call exports. They differ in
their APIs, import objects, memory views, IO conventions, and packaging.

## Wasmtime

Wasmtime is the primary execution target for compiler tests. The Rust API makes
the execution pipeline explicit:

```rust
let engine = wasmtime::Engine::default();
let module = wasmtime::Module::new(&engine, wasm_bytes)?;
let mut store = wasmtime::Store::new(&engine, ());
let instance = wasmtime::Instance::new(&mut store, &module, &[])?;
let id = instance.get_typed_func::<i64, i64>(&mut store, "id")?;

assert_eq!(id.call(&mut store, 42)?, 42);
```

Wasmtime distinguishes the compiled `Module` from the instantiated `Instance`.
An instance is where exports can be acquired and called.[^wasmtime] The
low-level `Instance::new` API instantiates a module with a list of imports and
runs the module's start function if it has one.[^instance]

For modules with named imports, Wasmtime's `Linker` is usually the better test
tool. It maps host functions, memories, or globals to the import names expected
by the module.

## Testing exported functions

Scalar exported functions should use typed Wasmtime calls:

```rust
let add_one = instance.get_typed_func::<i64, i64>(&mut store, "add_one")?;
assert_eq!(add_one.call(&mut store, 41)?, 42);
```

Managed values should return pointers. A test can then read exported memory:

```rust
let ptr = make_string.call(&mut store, ())?;
let memory = instance.get_memory(&mut store, "memory").unwrap();
let data = memory.data(&store);
let tag = u32::from_le_bytes(data[ptr as usize..ptr as usize + 4].try_into()?);
```

This style checks the real ABI instead of only checking WAT text.

## Browser API

Browsers expose WebAssembly through JavaScript APIs. The official specs index
separates the core spec from embedding APIs, including the JavaScript API and
Web API.[^specs]

A browser host usually fetches bytes and instantiates them with an import
object:

```js
import { initBrowserPage } from "./module.mjs";

const exports = await initBrowserPage(fetch("module.wasm"));

console.log(exports.id(42n));
```

`instantiateStreaming` is a browser Web API convenience for compiling and
instantiating from a `Response`. Hosts that cannot stream can fetch an
`ArrayBuffer` and call `WebAssembly.instantiate` instead.

The browser import module is named `browser`. Generated JS adapters provide the
checked instantiation path for browser-target modules. Browser imports print and
return control to compiled code; the compiler preserves the debugged value on
its own stack.

## Browser memory access

When a module exports memory, JavaScript sees it as a `WebAssembly.Memory`
object. Its `buffer` can be wrapped in typed arrays:

```js
const memory = instance.exports.memory;
const bytes = new Uint8Array(memory.buffer);
```

That gives JavaScript access to the same linear memory used by compiled Gleam
code. Raw memory access is useful for tests and low-level adapters, but it is
not a pleasant public API. Browser-facing wrappers should translate raw pointers
and lengths into JavaScript strings, arrays, or objects according to the runtime
ABI.

## WASI and future targets

WASI is a system interface for running WebAssembly outside the browser, with
capabilities such as files, clocks, and random numbers.[^specs] It is different
from both raw Wasmtime tests and browser JavaScript interop. Regulus already has
target names for Wasmtime, browser, and WASI in the CLI. The backend should keep
the core module ABI explicit so those targets can add host interfaces without
rewriting ordinary code generation.

[^overview]: WebAssembly Core Specification, "Overview": https://webassembly.github.io/spec/core/intro/overview.html

[^wasmtime]: Wasmtime Rust API documentation: https://docs.wasmtime.dev/api/wasmtime/

[^instance]: Wasmtime `Instance` documentation: https://docs.wasmtime.dev/api/wasmtime/struct.Instance.html

[^specs]: WebAssembly specifications index: https://webassembly.org/specs/
