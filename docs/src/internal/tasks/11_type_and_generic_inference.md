# Type and generic inference tasks

## Goal

Infer full Gleam expression, function, pattern, and module interface types.

## Tasks

### Inference core

- [x] Add inference variables distinct from named generic parameters.
- [x] Add type schemes for generalized values and functions.
- [x] Implement substitutions and type walking.
- [x] Implement unification for scalar, tuple, list, record, custom, opaque,
      function, and variable types.
- [x] Implement occurs checks for recursive/infinite type rejection.

### Constraint generation

- [x] Generate constraints for literals, variables, lets, calls, operators,
      pipelines, captures, anonymous functions, `use`, and field access.
- [x] Generate constraints for tuples, lists, records, constructors, and record
      updates.
- [x] Generate constraints for `case` subjects, guards, and branch results.
- [x] Generate constraints for tuple, list, constructor, record, alias, and
      nested patterns.

### Generics and interfaces

- [x] Generalize top-level functions and eligible local bindings.
- [x] Instantiate generic values and constructors on lookup.
- [x] Infer generic custom-type constructor uses and constructor patterns.
- [x] Store inferred public schemes in module interfaces.
- [x] Instantiate imported module interface schemes across project modules.

### Diagnostics and tests

- [ ] Add diagnostics for incompatible, ambiguous, and recursive types.
- [ ] Add diagnostics for generic arity and constructor/field mismatches.
- [ ] Add tests for unannotated identity, generic lists, generic custom types,
      polymorphic calls, pattern inference, and imported generic functions.
- [ ] Replace annotation-required tests where inference should now succeed.

## Done when

Unannotated common Gleam functions infer stable types, generic functions can be
used at multiple concrete types, and inference failures have source spans.
