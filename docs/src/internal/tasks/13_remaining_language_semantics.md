# Remaining language semantics tasks

## Goal

Replace placeholder semantics for represented language forms with full behavior.

## Tasks

### Target groups

- [ ] Define target selection rules for Wasmtime, browser, WASI, and generic
      Wasm output.
- [ ] Exclude non-selected declarations from resolution, type checking,
      lowering, and backend emission.
- [ ] Preserve selected target-group declarations in module interfaces.
- [ ] Add diagnostics for unsupported target-specific declarations.
- [ ] Add tests for target-group selection and conflicts.

### Bit-string matching

- [x] Implement segment matching for integer, bytes, bit-string, string, and
      variable-sized segments.
- [x] Implement size, unit, signed/unsigned, endian, and validation rules.
- [x] Bind bit-string pattern variables with correct spans and types.
- [x] Add lowering and Wasm tests for successful and failed matches.

### Closures and captures

- [x] Represent scalar and managed captures in closure environments.
- [x] Emit closure allocation for captured scalar and managed values.
- [x] Emit closure invocation that passes recovered captures correctly.
- [x] Add tests for captured values, partial application, and indirect calls.

### `use` lowering

- [x] Lower `use` to callback-passing IR with explicit evaluation order.
- [x] Preserve callback parameters, captures, and failure paths.
- [x] Emit Wasm for lowered `use` callbacks.
- [x] Add execution tests for common `use` patterns.

### Record updates

- [ ] Resolve and type-check updated fields against declaration order.
- [ ] Infer field access and record-update field types from known record
      declarations.
- [ ] Support open-record-style inference is with diagnostics.
- [ ] Lower record updates into explicit field copy and replacement operations.
- [ ] Emit Wasm allocation for updated records/custom values.
- [ ] Add tests for scalar and managed field updates.

### Type checker soundness

- [ ] Replace approximate local generalization with outer-scope free-variable
      handling.
- [ ] Prevent generic or ambiguous types from reaching IR and Wasm phases that
      need concrete runtime representation.
- [ ] Add type-checker tests for local generalization, field inference, and
      generic leakage into lowering.

## Done when

Target groups, bit-string matching, captured closures, `use`, and record updates
execute with documented semantics or produce source-spanned diagnostics.
