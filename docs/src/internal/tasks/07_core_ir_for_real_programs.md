# Core IR for real programs tasks

## Goal

Extend core IR so it can represent real Gleam programs and runtime-managed
values.

## Tasks

- [ ] Add IR support for constants and module initialization.
- [ ] Add representation types for heap-managed values.
- [ ] Add records, tuples, lists, custom types, and constructors.
- [ ] Add closure and indirect-call support.
- [ ] Add lowered pattern-matching control flow.
- [ ] Add memory-operation forms needed by the WASM backend.
- [ ] Consider splitting high-level IR and WASM-oriented IR.
- [ ] Add deterministic IR snapshots for real-language fixtures.

## Done when

Typed Gleam modules lower to IR without depending on parser or name-resolution
syntax details.
