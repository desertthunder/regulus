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
  projects/     # whole Gleam project fixtures
```

Prefer small files with descriptive names. Keep expected output in tests or
snapshot files next to the test that owns it.

`fixtures/projects/scalar_app` is the smallest full project fixture. It should
expand over time toward a realistic Gleam application.

The planned endgame sample project will be a Lustre app, but the fixture should
grow only as the compiler gains support for the required language and runtime features.
