# Full Gleam syntax tasks

## Goal

Represent all tree-sitter Gleam syntax in the compiler AST.

## Tasks

- [x] Add AST nodes for constants, attributes, external declarations, type
      aliases, and custom types.
- [x] Add AST nodes for records, tuples, lists, bit arrays, and record updates.
- [x] Add AST nodes for anonymous functions, pipelines, operators, `use`,
      `panic`, `todo`, `assert`, and `let assert`.
- [x] Add AST nodes for full `case` syntax and guards.
- [x] Add AST nodes for all pattern forms.
- [x] Convert each supported tree-sitter node into AST.
- [x] Add diagnostics for malformed or unsupported grammar edge cases.
- [x] Add AST fixtures for each language construct.

## Done when

A valid Gleam module using any language syntax can be parsed into AST or produce
a targeted diagnostic for a known limitation.
