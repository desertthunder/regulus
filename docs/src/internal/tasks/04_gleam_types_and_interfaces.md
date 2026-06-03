# Gleam types and interfaces tasks

## Goal

Provide full Gleam type information to lowering.

## Tasks

- [x] Define type data for tuples, lists, records, custom types, generics, and
      opaque types.
- [x] Represent module interfaces, constructors, and fields.
- [x] Decide the official Gleam compiler interop format.
- [x] Import dependency module type information.
- [x] Type-check records, constructors, operators, guards, and patterns.
- [x] Preserve typed expression and declaration metadata for lowering.
- [x] Add tests for valid and invalid generic/custom-type programs.

## Done when

Typed data is available for real Gleam modules, including imported declarations
and custom types.
