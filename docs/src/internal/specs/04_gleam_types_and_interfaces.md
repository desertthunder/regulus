# Gleam types and interfaces

The compiler needs full Gleam type information for real programs. The preferred
path is to reuse information from the official Gleam compiler where practical,
while keeping a local representation that lowering and code generation can use.

## Responsibilities

- Represent Gleam scalar, tuple, list, function, custom, record, and generic
  types.
- Represent constructors, fields, opaque types, and module interfaces.
- Type patterns and guards.
- Check calls, operators, branches, records, and custom-type construction.
- Import type information from dependency modules.
- Report type diagnostics with source spans.

## Interop boundary

The type layer should be able to consume official Gleam compiler metadata rather
than requiring this project to reimplement all inference rules. If local
checking remains useful for tests, it should share the same output structures as
imported type data.

## Required output

Lowering should receive typed declarations, typed expressions, constructor
metadata, field metadata, and enough representation hints for runtime values.
