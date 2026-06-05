# WASM backend completeness tasks

## Goal

Emit and run valid WebAssembly for every typed IR form produced by complete
Gleam language support.

## Tasks

### Value representation

- [x] Define final WASM representations for all scalar and managed Gleam values.
- [x] Support strings, bit arrays, lists, tuples, records, custom types,
      closures, opaque values, results, options, errors, and panics.
- [x] Document memory layout, tags, alignment, ownership, and lifetime rules.
- [x] Add layout tests for every managed value kind.

### Runtime helpers

- [ ] Implement dynamic allocation for all managed values.
- [ ] Implement string helpers, including comparison, concatenation, and
      inspection.
- [ ] Implement bit-array helpers for construction, append, slicing, and
      matching.
- [ ] Implement list, tuple, record, custom-type, closure, equality, ordering,
      panic, assertion, and debug helpers.
- [ ] Ensure runtime helpers are deterministic and target-independent where
      possible.

### Code generation

- [ ] Emit every IR expression and instruction form.
- [ ] Emit branches, guards, lowered patterns, and failure paths.
- [ ] Emit all operators and short-circuiting boolean expressions.
- [ ] Emit direct calls, imported calls, exported calls, indirect calls, and
      closure calls.
- [ ] Emit module constants and module initialization in dependency order.

### Imports, exports, and targets

- [ ] Define ABI rules for module exports, module imports, and host imports.
- [ ] Add adapters for ABI shapes that raw WASM cannot express directly.
- [ ] Define Wasmtime, browser, and WASI target host interfaces.
- [ ] Diagnose unsupported target/function/type combinations before assembly.
- [ ] Keep generated WAT and Wasm deterministic for tests.

### Validation

- [ ] Add WAT snapshots for all backend forms.
- [ ] Add Wasmtime execution tests for scalar, managed, control-flow, closure,
      import, and export behavior.
- [ ] Add memory inspection tests for dynamic and static objects.
- [ ] Add diagnostics tests for unsupported ABI and target combinations.

## Done when

Every typed IR form from supported Gleam programs either emits valid WebAssembly
and runs for the selected target, or fails with a source-spanned backend or ABI
diagnostic before WAT assembly.
