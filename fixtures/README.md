# Fixtures

Fixtures are small Gleam snippets used by compiler tests and documentation.
Each fixture should focus on one language feature unless it is an end-to-end
case.

```text
fixtures/
  ast/          # tree-sitter-to-AST cases
  resolve/      # name-resolution success and failure cases
  typecheck/    # type-checking success and failure cases
  ir/           # core IR lowering cases
  wasm/         # WAT and wasm codegen cases
  e2e/          # full compile-and-run cases
  diagnostics/  # diagnostic rendering cases
```

Prefer small files with descriptive names. Keep expected output in tests or
snapshot files next to the test that owns it.
