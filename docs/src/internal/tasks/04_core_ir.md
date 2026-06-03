# Core IR tasks

## Goal

Lower typed AST into a small, typed core IR.

## Tasks

- [ ] Define core IR modules, functions, locals, expressions, blocks, literals,
      calls, branches, and returns.
- [ ] Implement local allocation for parameters and `let` bindings.
- [ ] Lower supported literals and variable references.
- [ ] Lower direct function calls.
- [ ] Lower blocks and simple branches with explicit evaluation order.
- [ ] Preserve source spans or debug metadata where useful.
- [ ] Add core IR snapshot tests.

## Done when

The typed scalar subset lowers to deterministic core IR with no remaining
Gleam-specific name resolution or parsing concerns.
