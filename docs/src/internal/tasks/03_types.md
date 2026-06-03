# Type system tasks

## Goal

Attach types to resolved AST nodes for the initial scalar subset.

## Tasks

- [ ] Define internal type data for `Int`, `Float`, `String`, `Bool`, `Nil`, and
      function types.
- [ ] Parse or import simple function annotations.
- [ ] Type literals, variables, calls, blocks, `let`, and simple `case`.
- [ ] Check arity and argument compatibility for direct calls.
- [ ] Check branch result compatibility.
- [ ] Emit source-spanned type diagnostics.
- [ ] Keep the phase interface replaceable by future Gleam compiler type import.
- [ ] Add type-checking tests for valid and invalid snippets.

## Done when

A resolved module using supported scalar expressions can be converted into a
typed AST, and common type errors stop compilation before lowering.
