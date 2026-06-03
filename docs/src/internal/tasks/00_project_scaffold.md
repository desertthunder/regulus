# Project scaffold tasks

## Goal

Create the compiler skeleton and make each phase callable from tests.

## Tasks

- [x] Define crate modules for `source`, `parse`, `ast`, `resolve`, `types`,
      `ir`, `wasm`, and `diagnostic`.
- [x] Add a top-level compile pipeline function that wires phases together.
- [x] Define a shared source file ID and span type.
- [x] Define a shared diagnostic type with code, message, and spans.
- [x] Add one end-to-end ignored test for `pub fn add(a, b) { a + b }`.
- [x] Document the current supported subset in the docs.

## Done when

The repository has a compiling skeleton, and tests can call each phase even if
some phases return `unsupported` diagnostics.
