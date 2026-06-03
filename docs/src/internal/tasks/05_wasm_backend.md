# WebAssembly backend tasks

## Goal

Generate readable WAT and executable `.wasm` for supported core IR.

## Tasks

- [ ] Define a WASM emitter API that accepts core IR.
- [ ] Emit modules, function signatures, locals, constants, calls, and returns.
- [ ] Map `Int`, `Float`, and `Bool` to WASM scalar types.
- [ ] Export public scalar functions for tests.
- [ ] Add WAT snapshot tests.
- [ ] Add a path to assemble WAT into `.wasm` or emit binary WASM directly.
- [ ] Add Wasmtime tests for simple exported functions.
- [ ] Document unsupported runtime-managed values such as strings.

## Done when

A simple public Gleam function using scalar values can be compiled, loaded in
Wasmtime, called, and checked for the expected result.
