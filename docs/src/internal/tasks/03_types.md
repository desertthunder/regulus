# Type system tasks

## Goal

Attach types to resolved AST nodes for the initial scalar subset.

## Tasks

- [x] Define internal type data for `Int`, `Float`, `String`, `Bool`, `Nil`, and
      function types.
- [x] Parse or import simple function annotations.
- [x] Type literals, variables, calls, blocks, `let`, and simple `case`.
- [x] Check arity and argument compatibility for direct calls.
- [x] Check branch result compatibility.
- [x] Emit source-spanned type diagnostics.
- [x] Keep the phase interface replaceable by future Gleam compiler type import.
- [x] Add type-checking tests for valid and invalid snippets.

## Done when

A resolved module using supported scalar expressions can be converted into a
typed AST, and common type errors stop compilation before lowering.
