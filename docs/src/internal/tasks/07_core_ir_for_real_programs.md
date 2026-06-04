# Core IR for real programs tasks

## Goal

Extend core IR so it can represent real Gleam programs and runtime-managed
values without depending on parser or resolver syntax details.

## Tasks

### Module structure and declarations

- [x] Add IR support for constants and constant evaluation order.
- [x] Add module initialization for constants, static data, and runtime setup.
- [x] Represent imports, exports, and module-qualified references in a form the
      backend can emit or reject with targeted diagnostics.
- [x] Represent public/private declaration metadata needed by build outputs and
      debug dumps.
- [x] Preserve source spans for declarations, expressions, and generated failure
      paths.

### Values and representation types

- [ ] Add IR representation types for scalar values and heap-managed values.
- [ ] Add IR forms for strings, tuples, lists, records, and custom-type
      constructors.
- [ ] Add IR forms for record field access and record updates.
- [ ] Add IR forms for list construction, list deconstruction, and tuple element
      access.
- [ ] Add IR forms for equality and comparison operations, including runtime
      equality calls for managed values.
- [ ] Add explicit memory-operation forms needed by the WASM backend.

### Functions and calls

- [ ] Add closure and function-value representation.
- [ ] Add indirect-call support for function values.
- [ ] Represent anonymous functions and captured variables.
- [ ] Represent pipeline, `use`, labelled argument, and higher-order call
      lowering once the type checker supplies enough metadata.
- [ ] Add call ABI metadata for values crossing module or host boundaries.

### Control flow and failures

- [ ] Add lowered pattern-matching control flow for all supported pattern forms.
- [ ] Lower tuple, list, record, constructor, and nested managed-value patterns
      once IR can construct and inspect those runtime values.
- [ ] Represent guards, branch fallthrough, successful bindings, and failure
      paths explicitly.
- [ ] Represent `let assert`, `panic`, `todo`, and `assert` failure paths.
- [ ] Represent blocks, sequencing, and evaluation order without relying on AST
      statement shape.
- [ ] Consider splitting high-level IR, lowered control-flow IR, and
      WASM-oriented IR if one structure becomes too broad.

### Testing and debug output

- [ ] Add deterministic IR snapshots for real-language fixtures.
- [ ] Add fixtures that cover carried-forward syntax forms: records, tuples,
      lists, custom types, constructors, pipelines, anonymous functions, guards,
      and `let assert`.
- [ ] Add diagnostics for typed constructs that still cannot lower, with spans
      on the unsupported construct.
- [ ] Keep IR debug output stable enough for contributor-facing snapshots.

## Done when

Typed Gleam modules lower to IR without depending on parser or name-resolution
syntax details, and every unsupported typed construct fails with a precise
source-spanned diagnostic.
