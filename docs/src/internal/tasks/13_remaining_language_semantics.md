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

- [ ] Implement segment matching for integer, bytes, bit-string, string, and
      variable-sized segments.
- [ ] Implement size, unit, signed/unsigned, endian, and validation rules.
- [ ] Bind bit-string pattern variables with correct spans and types.
- [ ] Add lowering and Wasm tests for successful and failed matches.

### Closures and captures

- [ ] Represent scalar and managed captures in closure environments.
- [ ] Emit closure allocation for captured scalar and managed values.
- [ ] Emit closure invocation that passes recovered captures correctly.
- [ ] Add tests for captured values, partial application, and indirect calls.

### `use` lowering

- [ ] Lower `use` to callback-passing IR with explicit evaluation order.
- [ ] Preserve callback parameters, captures, and failure paths.
- [ ] Emit Wasm for lowered `use` callbacks.
- [ ] Add execution tests for common `use` patterns.

### Record updates

- [ ] Resolve and type-check updated fields against declaration order.
- [ ] Lower record updates into explicit field copy and replacement operations.
- [ ] Emit Wasm allocation for updated records/custom values.
- [ ] Add tests for scalar and managed field updates.

## Done when

Target groups, bit-string matching, captured closures, `use`, and record updates
execute with documented semantics or produce source-spanned diagnostics.
