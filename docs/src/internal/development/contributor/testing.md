# Tests & Documentation

Compiler tests should be small, layered, and easy to connect to the docs. A
single language feature usually passes through parsing, AST construction, name
resolution, type checking, lowering, WebAssembly generation, and execution.

Tests should make that path visible without requiring every test to run the
whole compiler.

## Fixture layout

Fixtures live at the repository root:

```text
fixtures/
  ast/
  resolve/
  typecheck/
  ir/
  wasm/
  e2e/
  diagnostics/
  projects/
```

Each fixture should focus on one language feature unless it is an end-to-end
case. Use short Gleam snippets with descriptive names. End-to-end fixtures
should compile to WebAssembly and run in Wasmtime.

## Test layers

Use the narrowest test that proves the behavior:

- AST tests for tree-sitter-to-AST conversion.
- Resolver tests for successful and failed name lookup.
- Type-checker tests for successful and failed type rules.
- Core IR tests for lowering and local allocation.
- WAT snapshots for deterministic WebAssembly text output.
- Wasmtime tests for exported functions that should execute.
- Memory inspection tests for managed objects and runtime layout.
- Diagnostic snapshots for stable, color-free error rendering.

Backend tests should cover WAT snapshots, Wasmtime execution, and memory
inspection. Add focused diagnostics tests whenever a new unsupported ABI
or backend shape is recognized before WAT assembly.

Snapshot tests use `insta`. Prefer inline snapshots for short output and fixture
or snapshot files for larger IR, WAT, or diagnostic output.

## Diagnostics

Diagnostics are part of the compiler's behavior. A useful diagnostic should
include:

- a stable diagnostic code
- a short message
- a primary source span
- optional notes or secondary spans

Diagnostic tests should avoid terminal color. Colorized rendering can have its
own tests, but core diagnostic snapshots should compare plain text.

## Adding a language feature

When adding a Gleam feature, move through the compiler in order:

1. Add or update fixtures for the source syntax.
2. Check the tree-sitter node shape.
3. Add AST data and CST-to-AST conversion.
4. Add resolver behavior for any new names or scopes.
5. Add type-checker behavior.
6. Add core IR lowering.
7. Add WAT or Wasmtime tests if the feature can execute.
8. Update the relevant docs chapter and supported-subset page.

Small features may not touch every step. For example, a new scalar literal may
not need resolver changes. Runtime-managed values, patterns, and imports usually
need changes across several layers.

## Default checks

Before considering a change done, run:

```bash
cargo fmt
cargo test
mdbook build docs
```

Changes that add generated output should also check that snapshots are
stable and that fixture names clearly describe the behavior being tested.
