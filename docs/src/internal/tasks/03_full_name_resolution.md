# Full name resolution tasks

## Goal

Resolve all value, type, constructor, field, module, and imported names across a
project.

## Tasks

- [ ] Split symbols into namespaces that match Gleam's rules.
- [ ] Resolve qualified module references across project modules.
- [ ] Resolve unqualified imports for values, types, and constructors.
- [ ] Resolve prelude names.
- [ ] Resolve custom type constructors and record fields.
- [ ] Enforce visibility rules for public and private declarations.
- [ ] Detect ambiguous imports and private-name access.
- [ ] Add cross-module resolver fixtures.

## Done when

Every supported reference either resolves to a stable symbol ID or produces a
specific diagnostic with a source span.
