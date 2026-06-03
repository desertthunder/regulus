# Full Gleam syntax tasks

## Goal

Represent all tree-sitter Gleam syntax in the compiler AST.

## Tasks

- [ ] Add AST nodes for constants, attributes, external declarations, type
      aliases, and custom types.
- [ ] Add AST nodes for records, tuples, lists, bit arrays, and record updates.
- [ ] Add AST nodes for anonymous functions, pipelines, operators, `use`,
      `panic`, `todo`, `assert`, and `let assert`.
- [ ] Add AST nodes for full `case` syntax and guards.
- [ ] Add AST nodes for all pattern forms.
- [ ] Convert each supported tree-sitter node into AST.
- [ ] Add diagnostics for malformed or unsupported grammar edge cases.
- [ ] Add AST fixtures for each language construct.

## Done when

A valid Gleam module using any language syntax can be parsed into AST or produce
a targeted diagnostic for a known limitation.
