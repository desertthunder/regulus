# Tree-sitter to AST

Tree-sitter is responsible for recognizing Gleam syntax. Our AST builder is
responsible for converting a permissive concrete syntax tree into compiler data
structures with source spans and clear invariants.

## Responsibilities

- Run tree-sitter over `source.gleam`.
- Reject parse errors before AST construction continues.
- Convert CST nodes into a small compiler AST.
- Preserve source spans on every declaration, expression, pattern, and type
  annotation node.
- Normalize syntactic trivia away from later phases.

## Initial AST subset

The first AST should cover enough Gleam to compile simple modules:

- Module name and imports.
- Public and private top-level functions.
- Integer, float, string, bool, and unit literals.
- Variables and function calls.
- `let` bindings.
- Blocks and simple `case` expressions.
- Type annotations where present.

Records, custom types, bit arrays, use expressions, and advanced pattern forms
can be added after the first end-to-end pipeline works.

## Invariants

- No AST node represents a tree-sitter error node.
- Lists are ordered exactly as they appear in source.
- Names are stored as source text plus span, not as resolved identifiers.
- AST construction does not perform name resolution or type inference.

## Diagnostics

AST construction diagnostics should describe malformed or unsupported syntax in
terms of Gleam source, not tree-sitter internals. Unsupported syntax is allowed
early on, but each unsupported construct should have a targeted diagnostic.
