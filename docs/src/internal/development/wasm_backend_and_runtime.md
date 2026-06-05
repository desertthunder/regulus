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

Generated modules include a runtime prelude only when a runtime-managed value or
helper is used. The prelude currently provides:

- exported linear memory
- a mutable bump-allocation heap pointer
- `__alloc` for aligned allocation
- panic traps
- pointer equality
- list construction
- a placeholder bit-array append helper

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
- branches, guards, scalar pattern tests, managed tag tests, pattern bindings,
  and `let assert` failure paths
- memory allocation, loads, and stores for backend-lowered memory operations
- static data for managed literals and constants where possible

Unsupported forms produce source-spanned `WasmError` diagnostics rather than
letting WAT assembly fail.

## ABI rules

Internal calls pass scalars as raw WebAssembly values and managed values as
`i32` memory pointers. Public exports, module imports, and host imports are
validated before emission. The current raw ABI supports:

| Gleam type | WASM type |
| --- | --- |
| `Int` | `i64` |
| `Float` | `f64` |
| `Bool` | `i32` |
| `Nil` | no result |
| managed values | `i32` pointer |

Managed values can be exported directly for low-level Wasmtime tests. Safer
browser or user-facing APIs should use adapters that read or write memory using
the documented object layout.

## Tests

Backend tests should cover three layers:

1. WAT snapshots for deterministic text output.
2. Wasmtime execution for exported behavior.
3. Memory inspection for managed objects and runtime layout.

Add focused diagnostics tests whenever a new unsupported ABI or backend shape is
recognized before assembly.
