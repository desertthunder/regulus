# WASM backend and runtime tasks

## Goal

Emit WebAssembly for scalar and runtime-managed Gleam values.

## Tasks

- [ ] Emit memory operations for heap-managed values.
- [ ] Emit runtime calls for allocation, strings, lists, and equality.
- [ ] Emit direct and indirect function calls.
- [ ] Emit lowered pattern-matching control flow.
- [ ] Emit imports and exports using the host ABI.
- [ ] Add browser-oriented build tests where practical.
- [ ] Add Wasmtime execution tests for managed values.
- [ ] Keep WAT output deterministic.

## Done when

Real Gleam functions using managed values can be compiled to WASM and executed in
Wasmtime.
