# Standard library and host interop tasks

## Goal

Support useful standard library modules and host calls.

## Tasks

- [ ] Decide which stdlib modules are compiled, shimmed, or intrinsic.
- [ ] Define host imports for Wasmtime and browser targets.
- [ ] Define ABI rules for strings and managed values across host boundaries.
- [ ] Implement selected stdlib functions needed by examples.
- [ ] Add diagnostics for unsupported stdlib or host calls.
- [ ] Add Wasmtime tests for host imported functions.
- [ ] Add fixtures using common Gleam stdlib modules.

## Done when

Small programs using selected Gleam stdlib functionality compile and execute
against a documented host interface.
