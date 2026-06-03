# Full Gleam syntax

The AST should cover all Gleam syntax accepted by tree-sitter. Unsupported nodes
are useful for early milestones, but a feature-complete compiler needs every
syntax form represented with source spans and clear invariants.

## Syntax to cover

- Constants and module attributes.
- External functions and external types.
- Type aliases and custom type definitions.
- Records, record updates, and field access.
- Tuples, lists, bit arrays, and strings.
- Anonymous functions and function values.
- Pipelines, boolean operators, comparisons, and arithmetic operators.
- `use`, `panic`, `todo`, `assert`, and `let assert`.
- Full `case` syntax with guards and multiple subjects.
- All pattern forms, including nested patterns.
- Documentation comments if they are needed for generated metadata.

## Invariants

- AST construction rejects tree-sitter error nodes.
- AST nodes preserve source order and source spans.
- Names remain textual until name resolution.
- AST construction does not infer types or resolve imports.
