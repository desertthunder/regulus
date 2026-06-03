# Full name resolution tasks

## Goal

Resolve all value, type, constructor, field, module, and imported names across a
project.

## Tasks

- [x] Split symbols into namespaces that match Gleam's rules.
- [x] Resolve qualified module references across project modules.
- [x] Resolve unqualified imports for values, types, and constructors.
- [x] Resolve prelude names.
- [x] Resolve custom type constructors and record fields.
- [x] Enforce visibility rules for public and private declarations.
- [x] Detect ambiguous imports and private-name access.
- [x] Add cross-module resolver fixtures.

## Done when

Every supported reference either resolves to a stable symbol ID or produces a
specific diagnostic with a source span.
