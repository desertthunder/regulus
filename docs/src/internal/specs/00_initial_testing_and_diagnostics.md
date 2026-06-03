# Initial testing and diagnostics

The project should grow through small phase-level tests plus end-to-end compiler
tests. Diagnostics are part of the compiler API and should be tested like code
generation.

## Test layers

- AST snapshots from Gleam snippets.
- Name-resolution success and failure tests.
- Type-checking success and failure tests.
- Core IR snapshots.
- WAT snapshots.
- Wasmtime execution tests for exported scalar functions.

## Fixtures

Use small Gleam snippets in test fixtures. Each fixture should focus on one
language feature unless it is an end-to-end integration test.

Suggested fixture layout:

```text
tests/fixtures/
  ast/
  resolve/
  typecheck/
  ir/
  wasm/
```

## Diagnostic expectations

Diagnostics should include:

- A stable diagnostic code.
- A short message.
- A primary source span.
- Optional notes or secondary spans.

Snapshot tests should avoid depending on terminal color. The renderer can have a
separate golden test if needed.

## CI expectations

The default check should run formatting, unit tests, snapshot tests, and any
Wasmtime tests that do not require browser tooling.
