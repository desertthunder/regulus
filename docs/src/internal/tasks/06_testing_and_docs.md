# Testing and docs tasks

## Goal

Keep compiler behavior documented and protected by tests as the supported subset
grows.

## Tasks

- [ ] Add fixture directories for each compiler phase.
- [ ] Choose and configure a snapshot testing approach.
- [ ] Add diagnostic golden tests without terminal color dependencies.
- [ ] Add end-to-end fixtures that compile and execute in Wasmtime.
- [ ] Update docs whenever a new language construct is supported.
- [ ] Add a short contributor guide for adding a new AST feature through WASM.
- [ ] Keep `docs/src/SUMMARY.md` linked to internal specs and task lists.

## Done when

New contributors can see the planned pipeline, run tests, and follow one feature
from Gleam source through generated WebAssembly.
