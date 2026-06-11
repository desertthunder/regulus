# Gleam syntax

The parser uses tree-sitter and the AST builder converts accepted syntax into
compiler-owned nodes. Syntax that is parsed but not yet executable is preserved
as raw syntax with kind, source text, and source span. Later phases can produce
targeted diagnostics.

## Covered syntax

The AST layer represents current Gleam module syntax, including:

- constants and module attributes
- external functions and external types
- type aliases and custom type definitions
- records, record updates, and field access
- tuples, lists, bit arrays, and strings
- anonymous functions and function values
- pipelines, boolean operators, comparisons, and arithmetic operators
- `use`, `panic`, `todo`, `assert`, and `let assert`
- `case` syntax with guards and multiple subjects
- nested pattern forms

Known unsupported executable forms should fail in the phase responsible for the
missing semantics, not in parsing.

## AST invariants

AST construction follows these rules:

- reject tree-sitter error nodes
- preserve source order
- preserve source spans on all meaningful nodes
- leave names textual until name resolution
- do not infer types or resolve imports
- preserve raw syntax kind, source text, and span

These invariants keep parsing independent from later compiler phases and make
syntax fixtures useful even when runtime support for a construct is incomplete.
