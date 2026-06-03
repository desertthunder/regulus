# WASM backend and runtime

The backend should compile core IR into WebAssembly modules that can run in
Wasmtime and browsers. It must work with the runtime representation for managed
values and provide a clear host ABI.

## Responsibilities

- Emit WASM for scalar and runtime-managed values.
- Emit memory operations for heap objects.
- Emit direct and indirect calls.
- Emit control flow for lowered pattern matching.
- Emit imports and exports.
- Support Wasmtime tests and browser-oriented builds.
- Keep WAT output deterministic for debugging and snapshots.

## Runtime integration

The backend should define or import runtime functions for allocation, string
handling, list construction, equality, panic paths, and any required host IO.

## Entry points

Public Gleam functions may be exported when their ABI is supported. Executable
modules should also support a conventional entry point once project compilation
is available.
