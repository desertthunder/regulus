# Gleam types and interfaces tasks

## Goal

Provide full Gleam type information to lowering.

## Tasks

- [ ] Define type data for tuples, lists, records, custom types, generics, and
      opaque types.
- [ ] Represent module interfaces, constructors, and fields.
- [ ] Decide the official Gleam compiler interop format.
- [ ] Import dependency module type information.
- [ ] Type-check records, constructors, operators, guards, and patterns.
- [ ] Preserve typed expression and declaration metadata for lowering.
- [ ] Add tests for valid and invalid generic/custom-type programs.

## Done when

Typed data is available for real Gleam modules, including imported declarations
and custom types.
