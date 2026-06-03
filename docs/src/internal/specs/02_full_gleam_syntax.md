# Full Gleam syntax

The AST should cover all Gleam syntax accepted by tree-sitter. Syntax that is
not yet executable can be represented as raw syntax nodes with kind, source, and
span, so the parser and AST builder can accept real Gleam modules while later
compiler work catches up.

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
- Raw syntax nodes preserve tree-sitter kind, source text, and span.
