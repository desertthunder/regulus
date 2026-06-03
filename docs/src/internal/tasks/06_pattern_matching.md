# Pattern matching tasks

## Goal

Support full Gleam pattern matching through type checking, lowering, and code
generation.

## Tasks

- [ ] Bind names introduced by all pattern forms.
- [ ] Type-check literal, variable, tuple, list, record, and constructor
      patterns.
- [ ] Support guards and multiple subjects.
- [ ] Design a pattern decision-tree or matching IR.
- [ ] Lower `case` and `let assert` patterns into matching IR.
- [ ] Add exhaustiveness and redundancy diagnostics where possible.
- [ ] Add fixtures for nested patterns and constructor patterns.

## Done when

Supported patterns compile to explicit matching logic, and invalid patterns
produce source-spanned diagnostics.
