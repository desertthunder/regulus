# Core IR tasks

## Goal

Lower typed AST into a small, typed core IR.

## Tasks

- [x] Define core IR modules, functions, locals, expressions, blocks, literals,
      calls, branches, and returns.
- [x] Implement local allocation for parameters and `let` bindings.
- [x] Lower supported literals and variable references.
- [x] Lower direct function calls.
- [x] Lower blocks and simple branches with explicit evaluation order.
- [x] Preserve source spans or debug metadata where useful.
- [x] Add core IR snapshot tests.

## Done when

The typed scalar subset lowers to deterministic core IR with no remaining
Gleam-specific name resolution or parsing concerns.
