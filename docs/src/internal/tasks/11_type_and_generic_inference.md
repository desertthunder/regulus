# Type and generic inference tasks

## Goal

Infer full Gleam expression, function, pattern, and module interface types.

## Tasks

### Inference core

- [ ] Add inference variables distinct from named generic parameters.
- [ ] Add type schemes for generalized values and functions.
- [ ] Implement substitutions and type walking.
- [ ] Implement unification for scalar, tuple, list, record, custom, opaque,
      function, and variable types.
- [ ] Implement occurs checks for recursive/infinite type rejection.

### Constraint generation

- [ ] Generate constraints for literals, variables, lets, calls, operators,
      pipelines, captures, anonymous functions, `use`, and field access.
- [ ] Generate constraints for tuples, lists, records, constructors, and record
      updates.
- [ ] Generate constraints for `case` subjects, guards, and branch results.
- [ ] Generate constraints for tuple, list, constructor, record, alias, and
      nested patterns.

### Generics and interfaces

- [ ] Generalize top-level functions and eligible local bindings.
- [ ] Instantiate generic values and constructors on lookup.
- [ ] Infer generic custom-type constructor uses and constructor patterns.
- [ ] Store inferred public schemes in module interfaces.
- [ ] Instantiate imported module interface schemes across project modules.

### Diagnostics and tests

- [ ] Add diagnostics for incompatible, ambiguous, and recursive types.
- [ ] Add diagnostics for generic arity and constructor/field mismatches.
- [ ] Add tests for unannotated identity, generic lists, generic custom types,
      polymorphic calls, pattern inference, and imported generic functions.
- [ ] Replace annotation-required tests where inference should now succeed.

## Done when

Unannotated common Gleam functions infer stable types, generic functions can be
used at multiple concrete types, and inference failures have source spans.
