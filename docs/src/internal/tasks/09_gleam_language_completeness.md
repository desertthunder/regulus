# Gleam language completeness tasks

## Goal

Compile the complete Gleam language through typed IR, independent of any single
backend target.

## Tasks

### Syntax and AST

- [x] Replace raw executable syntax with structured AST nodes.
- [x] Add structured constants, attributes, externals, target groups, and docs.
- [x] Add structured operators, pipelines, `use`, anonymous functions, and
      captures.
- [x] Add structured record construction, record updates, tuples, lists, and bit
      arrays.
- [x] Preserve spans and source order for every new AST node.

### Resolution

- [x] Resolve all value, type, constructor, field, module, and label names.
- [x] Resolve aliases, unqualified imports, dependency modules, and prelude
      names according to Gleam rules.
- [x] Enforce public, private, and opaque module boundaries.
- [x] Load dependency package interfaces or official compiler metadata.
- [x] Add diagnostics for unknown, duplicate, private, and ambiguous names.

### Type checking

- [ ] Implement or import full local and top-level type inference.
- [ ] Type-check generic functions and generic custom types.
- [ ] Type-check records, updates, labels, constructors, tuples, lists, strings,
      bit arrays, functions, captures, and operators.
- [ ] Type-check pipelines, `use`, guards, `panic`, `todo`, `assert`, and
      `let assert`.
- [ ] Type-check imported functions, types, constructors, and opaque values.
- [ ] Preserve complete module interfaces for downstream phases.

### Pattern matching

- [ ] Support all pattern forms in AST, resolver, type checker, and lowering.
- [ ] Lower patterns into explicit decision logic or match IR.
- [ ] Bind names from nested patterns, aliases, records, lists, and bit strings.
- [ ] Add exhaustiveness, unreachable branch, and redundant pattern diagnostics
      where practical.

### Lowering

- [ ] Lower every typed expression into compiler IR.
- [ ] Make evaluation order, captures, scopes, failure paths, and initialization
      explicit.
- [ ] Represent module constants, module initialization, and dependency calls.
- [ ] Reject only target/backend limitations after language lowering succeeds.

## Done when

A complete Gleam program can be parsed, resolved, type-checked, and lowered to
IR without relying on raw executable syntax or ad-hoc stdlib tables for language
semantics.
