# Type and generic inference

The type checker should infer Gleam function, expression, pattern, and module
interface types without requiring annotations for every parameter.

## Scope

Inference must cover:

- local variables and unannotated function parameters
- generic functions and generic custom types
- tuples, lists, records, functions, opaque types, and constructors
- literals, calls, operators, pipelines, captures, anonymous functions, and
  `use`
- tuple, list, constructor, record, alias, and nested patterns
- `case` subjects, guards, branch results, and exhaustiveness inputs
- public module interfaces and imported module interfaces

## Inference model

The checker should introduce inference variables for unknown types, generate
constraints while checking expressions and patterns, and solve those constraints
with unification.

Unification must include:

- substitutions for inference variables
- recursive unification for type structures
- occurs checks to reject infinite types
- source-spanned errors for incompatible types

Generic functions and values should use type schemes. A scheme records
generalized variables and a type. Each lookup of a generic value should
instantiate fresh inference variables.

## Generalization

Top-level functions and eligible local bindings should generalize unsolved type
variables according to Gleam's rules. Variables constrained by an outer scope
must not be generalized accidentally.

Public inferred types must be written into module interfaces so other modules
can instantiate them.

## Diagnostics

Inference diagnostics should explain:

- incompatible types
- ambiguous types
- recursive/infinite types
- generic arity mismatches
- constructor and record field mismatches
- branch result mismatches
- pattern and subject mismatches

## Done when

Common Gleam functions can omit annotations and still produce stable typed
module interfaces, or fail with clear source-spanned inference diagnostics.
