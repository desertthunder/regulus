# Project scaffold tasks

## Goal

Create the compiler skeleton and make each phase callable from tests.

## Tasks

- [ ] Define crate modules for `source`, `parse`, `ast`, `resolve`, `types`,
      `ir`, `wasm`, and `diagnostic`.
- [ ] Add a top-level compile pipeline function that wires phases together.
- [ ] Define a shared source file ID and span type.
- [ ] Define a shared diagnostic type with code, message, and spans.
- [ ] Add one end-to-end ignored or pending test for `pub fn add(a, b) { a + b }`.
- [ ] Document the current supported subset in the docs.

## Done when

The repository has a compiling skeleton, and tests can call each phase even if
some phases return `unsupported` diagnostics.
