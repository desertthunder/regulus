# Gleam types and interfaces

The type layer records enough Gleam type information for lowering, backend ABI
selection, and project-module interfaces. It is intentionally local and
explicit, while leaving room to consume official Gleam compiler metadata later.

## Type representation

The compiler represents:

- `Int`, `Float`, `String`, `BitArray`, `Bool`, and `Nil`
- tuples and lists
- function types
- custom types and records
- generic type parameters
- opaque types
- constructors and fields
- module interfaces

Typed expressions are recorded by source span so lowering can query the checked
type of each expression and preserve useful diagnostic locations.

## Checks

The checker handles current executable language forms:

- typed function parameters and return annotations
- literals, variables, and local bindings
- direct calls, arity, and argument types
- records and custom-type construction
- field access and tuple access
- tuple, list, constructor, and nested patterns
- branch result compatibility
- guard expressions
- basic exhaustiveness and unreachable-branch diagnostics

Unsupported executable forms should produce type diagnostics or pass through as
metadata only when a later phase is responsible for rejecting them.

## Module interfaces

Each checked module records an interface containing public values, type
declarations, constructors, fields, opaque declarations, and generic parameters.
Project type checking uses these interfaces for imported members and visibility
rules.

## Interop direction

The local interface data is shaped so official Gleam dependency metadata can
populate the same structures in the future. That avoids making
lowering and code generation depend on how type facts were discovered.
