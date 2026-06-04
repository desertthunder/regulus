# WASM backend and runtime tasks

## Goal

Emit WebAssembly for scalar and runtime-managed Gleam values.

## Tasks

### Runtime memory and values

- [ ] Emit memory operations for heap-managed values using the documented object
      headers, tags, alignment, and payload layouts.
- [ ] Emit allocation paths for strings, lists, tuples, records, custom-type
      variants, and closures.
- [ ] Emit static data for literals and constants where possible.
- [ ] Emit runtime calls for allocation, string handling, list operations,
      equality, panic paths, and assertion failures.
- [ ] Emit safe field, tuple-element, list-head, and list-tail access according
      to the runtime representation.
- [ ] Add memory-layout tests in Wasmtime for each managed value kind, not only
      strings.

### Functions and control flow

- [ ] Emit direct function calls.
- [ ] Emit indirect calls for function values and closures.
- [ ] Emit closure allocation and captured environment access.
- [ ] Emit lowered pattern-matching control flow for scalar and managed values.
- [ ] Emit guard checks, branch fallthrough, and `let assert` failure paths.
- [ ] Emit `panic`, `todo`, and `assert` control flow.
- [ ] Emit loops or recursive-call-friendly structures where needed by lowered
      stdlib or user code.

### ABI, imports, and exports

- [ ] Emit imports and exports using the host ABI.
- [ ] Decide which public Gleam functions can be exported directly and which
      need wrappers.
- [ ] Emit wrappers for strings and managed values crossing the host boundary.
- [ ] Support Wasmtime, browser, and WASI target settings where selected by the
      CLI.
- [ ] Reject unsupported ABI shapes with diagnostics before WAT assembly.

### Determinism and tests

- [ ] Keep WAT output deterministic.
- [ ] Add WAT snapshots for scalar, managed-value, control-flow, import, and
      export cases.
- [ ] Add Wasmtime execution tests for managed values, constructor matches,
      string/list operations, closure calls, and host imports.
- [ ] Add browser-oriented build tests where practical.
- [ ] Ensure unsupported backend forms produce source-spanned diagnostics rather
      than WAT assembly failures.

## Done when

Real Gleam functions using managed values can be compiled to WASM and executed in
Wasmtime, and unsupported backend shapes produce clear diagnostics.
