# Standard library and host interop tasks

## Goal

Support useful standard library modules and host calls.

## Tasks

### Standard library strategy

- [ ] Decide which stdlib modules are compiled from Gleam source, shimmed as
      host imports, or implemented as compiler/runtime intrinsics.
- [ ] Define a small initial stdlib support set for examples and tests.
- [ ] Load or model dependency package metadata needed for stdlib modules.
- [ ] Resolve stdlib module interfaces consistently with project module
      interfaces.
- [ ] Type-check selected stdlib functions using the same interface data as user
      modules.
- [ ] Add diagnostics for unsupported stdlib modules, functions, types, or target
      combinations.

### Host ABI

- [ ] Define host imports for Wasmtime, browser, and WASI targets where
      supported.
- [ ] Define ABI rules for scalar values, strings, bit arrays, lists, tuples,
      records, custom types, functions, errors, and panics.
- [ ] Define ownership rules for managed values crossing the host boundary.
- [ ] Define how host code reads compiler memory and how compiler code receives
      host-provided managed values.
- [ ] Add wrappers or adapter functions for ABI shapes that do not map directly
      to raw WASM parameters and results.

### Intrinsics and host calls

- [ ] Implement selected stdlib functions needed by examples.
- [ ] Implement or import IO functions for Wasmtime tests and browser examples.
- [ ] Implement equality, string, bit-array, list, result, option, and debug
      helpers where needed by compiled programs.
- [ ] Ensure host imports are target-specific and produce diagnostics on targets
      where they are unavailable.
- [ ] Add Wasmtime tests for host imported functions.
- [ ] Add fixtures using common Gleam stdlib modules.

## Done when

Small programs using selected Gleam stdlib functionality compile and execute
against a documented host interface, and unsupported stdlib usage fails with a
specific source-spanned diagnostic.
