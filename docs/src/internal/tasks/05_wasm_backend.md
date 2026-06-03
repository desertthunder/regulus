# WebAssembly backend tasks

## Goal

Generate readable WAT and executable `.wasm` for supported core IR.

## Tasks

- [x] Define a WASM emitter API that accepts core IR.
- [x] Emit modules, function signatures, locals, constants, calls, and returns.
- [x] Map `Int`, `Float`, and `Bool` to WASM scalar types.
- [x] Export public scalar functions for tests.
- [x] Add WAT snapshot tests.
- [x] Add a path to assemble WAT into `.wasm` or emit binary WASM directly.
- [x] Add Wasmtime tests for simple exported functions.
- [x] Document unsupported runtime-managed values such as strings.

## Done when

A simple public Gleam function using scalar values can be compiled, loaded in
Wasmtime, called, and checked for the expected result.
