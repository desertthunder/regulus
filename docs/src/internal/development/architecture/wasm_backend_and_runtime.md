# WASM backend and runtime

The backend compiles core IR into deterministic WebAssembly modules that run in
Wasmtime. It emits readable WAT first, assembles that WAT into binary Wasm, and
reports backend diagnostics before assembly when an IR or ABI shape is not
supported.

## Backend pipeline

The backend entry points are in `crates/core/src/wasm.rs`:

```text
core IR module -> WAT string -> Wasm bytes
```

`emit_wat` produces deterministic text output for snapshots and debugging.
`emit` then assembles the text with the `wat` crate. Tests usually inspect the
WAT and execute the bytes in Wasmtime.

## Runtime prelude

Generated modules include the runtime prelude only when a runtime-managed value
or helper is used. Runtime snippets live in
`crates/core/src/wasm/helpers.rs` and are inserted by the `RuntimePrelude`
builder.

The prelude currently provides:

- exported linear memory
- a mutable bump-allocation heap pointer
- `__alloc` for aligned, checked allocation and heap growth
- byte and slot copy helpers
- string construction, comparison, concatenation, and inspection
- bit-array construction, append, slicing, bit access, and matching
- list construction and deconstruction
- tuple, record, custom, closure, opaque, error, and panic-value allocation
- field access helpers
- recursive equality and ordering helpers
- panic, assertion, match-failure, and debug helpers

The prelude is not user code and it is not emitted for modules that only need
plain scalar WebAssembly.

Static managed literals are emitted as data segments before the dynamic heap.
The object layout is documented in
[Runtime representation](./runtime_representation.md).

## Supported emitted forms

The backend emits:

- scalar literals, string literals, bit-array literals, tuples, lists, records,
  custom values, and function values
- locals, local assignment, blocks, and expression evaluation
- direct calls, imported calls, exported calls, and indirect calls through
  function values
- arithmetic, comparison, string-concat, equality, and boolean operators
- branches, guards, scalar pattern tests, managed tag tests, pattern bindings,
  and `let assert` failure paths
- lowered pipeline, use, list-deconstruct, bit-string-deconstruct, and
  record-update IR forms
- memory allocation, loads, and stores for backend-lowered memory operations
- static data for managed literals and constants where possible

Unsupported forms produce source-spanned `WasmError` diagnostics rather than
letting WAT assembly fail.

## Current limits

The backend is complete for the current typed IR surface, not for the whole
Gleam language. These areas still need design or implementation before every
accepted source program can execute with full Gleam semantics:

- explicit arena reset points beyond instance reset
- target-specific browser and WASI adapters
- standard library and dependency-backed imports

## ABI rules

Internal calls pass scalars as raw WebAssembly values and managed values as
`i32` memory pointers. Public exports, module imports, and host imports are
validated before emission. The backend supports explicit target selection for
Wasmtime, browser, and WASI-oriented modules.

Host imports must use the target's host module name:

| Target   | Host module              |
| -------- | ------------------------ |
| Wasmtime | `env`                    |
| Browser  | `browser`                |
| WASI     | `wasi_snapshot_preview1` |

The current raw ABI supports:

| Gleam type     | WASM type     |
| -------------- | ------------- |
| `Int`          | `i64`         |
| `Float`        | `f64`         |
| `Bool`         | `i32`         |
| `Nil`          | no result     |
| managed values | `i32` pointer |

Managed values can be exported directly for low-level Wasmtime tests. String
exports with no parameters also get `__data` and `__len` adapter exports so host
code can read the string payload without duplicating the call shape. Safer
browser or user-facing APIs should use adapters that read or write memory using
the documented object layout.

Unsupported target/import/type combinations produce `WasmError` diagnostics
before WAT assembly.
