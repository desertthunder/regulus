# Types and Gleam compiler interop

The compiler needs type information before lowering to core IR. The long-term
preference is to reuse or import type information from the official Gleam
compiler. A local checker can begin as a narrow subset so the backend can be
built and tested.

## Responsibilities

- Attach a type to every expression and pattern accepted by lowering.
- Check function arity and argument types.
- Check `let` bindings and block result types.
- Check branch compatibility in `case` expressions.
- Produce diagnostics with source spans.

## Initial type subset

- `Int`
- `Float`
- `String`
- `Bool`
- `Nil`
- Function types

Custom types, generics, records, and opaque types can be staged later. Each new
feature should be reflected in AST, resolution, lowering, and runtime support.

## Interop direction

The type subsystem should be isolated behind a phase interface so it can be
replaced or augmented with data from the Gleam compiler. Avoid baking local type
checker assumptions into the AST or WASM backend.

Possible future import data:

- Module interface types.
- Resolved names from package dependencies.
- Inferred expression or declaration types.
- Constructor and record metadata.

## Invariants

- Lowering receives only typed input.
- Type errors stop compilation before core IR generation.
- Runtime representation decisions are not made by the type checker, except
  through explicit type metadata passed to lowering.
