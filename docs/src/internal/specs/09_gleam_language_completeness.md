# Gleam language completeness

The compiler should eventually accept and compile the whole Gleam language, not
only the current experimental subset. Completeness means every language feature
has a documented path through parsing, AST construction, resolution, type
checking, lowering, code generation, and diagnostics.

## Scope

This spec covers language semantics before target-specific code generation. The
WASM-specific requirements are in
[WASM backend completeness](./10_wasm_backend_completeness.md).

## Syntax and declarations

The compiler should provide structured AST and semantic support for:

- modules, imports, aliases, and unqualified imports
- public and private functions
- constants
- custom types, opaque types, constructors, and record fields
- type aliases and external types
- external functions and target-specific declarations
- attributes, documentation comments, and deprecation metadata
- all expression and pattern forms accepted by Gleam

Raw syntax can remain a temporary parser escape hatch, but executable language
features should not depend on raw text parsing in later phases.

## Name resolution

Resolution should match Gleam module rules for:

- values, types, constructors, fields, modules, and labels
- qualified and unqualified imports
- aliases and shadowing
- public, private, and opaque module boundaries
- dependency package modules and package interfaces
- prelude names and generated names

Unknown, private, duplicate, and ambiguous names should produce source-spanned
diagnostics.

## Type checking

The type layer should support Gleam's full type system:

- local and top-level type inference
- generic functions and generic custom types
- function types, anonymous functions, and captures
- tuples, lists, records, bit arrays, strings, and numbers
- constructor calls, labeled fields, and record updates
- modules interfaces, opaque types, and imported types
- operators, guards, pipelines, `use`, `panic`, `todo`, and `assert`
- pattern typing and branch result compatibility

The compiler may import type information from the official Gleam compiler where
that is more reliable than reimplementing all inference rules locally.

## Pattern matching

All pattern forms should be represented, typed, and lowered explicitly:

- literals, discards, variables, aliases, and nested patterns
- tuple, list, record, constructor, and bit-string patterns
- multiple subjects and guards
- `let assert` patterns

The compiler should report impossible, unreachable, redundant, and
non-exhaustive patterns where practical.

## Lowering contract

Lowering should receive typed, resolved syntax and produce IR that no longer
requires Gleam-specific parsing decisions. Evaluation order, scopes, captures,
pattern bindings, failure paths, and module initialization should be explicit.

## Done when

A project using the complete Gleam language can be compiled through typed IR,
and unsupported target behavior is reported as a target/backend limitation rather
than a missing language feature.
