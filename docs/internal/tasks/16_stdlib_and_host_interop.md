# Standard Library And Host Interop Tasks

## Goal

Compile the published `gleam_stdlib` Hex package as a normal dependency.

Stdlib support means implementing the language, dependency loader, target
selection, package asset validation, runtime primitives, and host ABI deeply
enough that Regulus does not reimplement standard-library behavior.

## Direction

- [x] Treat `gleam_stdlib` as dependency package source, not compiler-owned
      source.
- [x] Keep the existing stdlib registry explicitly transitional.
- [ ] Prefer upstream Gleam source for all library behavior.
- [ ] Use runtime helpers only for language, representation, and ABI
      primitives.
- [ ] Validate stdlib JS shims as dependency package assets.
- [ ] Delete registry entries as package source, metadata, assets, or runtime
      primitives replace them.
- [ ] Finish with no bespoke stdlib interface registry.

## Tasks

### Package Source Migration

- [x] Add fixtures that load the published `gleam_stdlib` package source as a
      dependency.
- [x] Snapshot the first compile blocker for every upstream stdlib module.
- [x] Group blockers by source language feature, target filtering, dependency
      metadata, package asset, runtime primitive, and host ABI.
- [x] Compile `gleam/pair` from upstream source as the first registry deletion
      proof.
- [x] Compile pure portions of `gleam/order`, `gleam/result`, `gleam/option`,
      `gleam/list`, `gleam/int`, `gleam/float`, `gleam/bool`, and
      `gleam/function`.
- [x] Keep any temporary registry entry only when the blocker report shows a
      real compiler, runtime, target, package asset, or ABI gap.
- [ ] Delete the registry once all remaining behavior is represented by package
      source, package metadata, validated assets, or runtime primitives.

Current blocker groups are snapshotted in
`loader::dependency::tests::snapshots_first_compile_blocker_for_each_upstream_stdlib_module`.
Retained registry entries record their blocker group and deletion condition in
`StdlibModule.retention` and `StdlibMember.retention`.

- Source language feature: full `gleam/order` still hits current
  exhaustiveness handling for multi-constructor branches.
- Target filtering: full `gleam/bit_array` and `gleam/set` now first need
  their imported dependency modules before standalone `@target` declarations
  can be isolated.
- Dependency metadata: `gleam/bytes_tree`, `gleam/dict`,
  `gleam/dynamic/decode`, `gleam/float`, `gleam/int`, `gleam/list`,
  `gleam/result`, `gleam/string`, and `gleam/uri` first need their imported
  dependency modules loaded as source.
- Package asset: full `gleam/dynamic`, `gleam/option`, and
  `gleam/string_tree` first fail around imported package modules or
  package-relative native externals.
- Runtime primitive: no module currently reaches a first blocker in this group.
- Host ABI: no module currently reaches a first blocker in this group.
- No current blocker through lowering: full `gleam/bool`, `gleam/function`,
  `gleam/io`, and `gleam/pair`, plus pure portions of `gleam/order`,
  `gleam/result`, `gleam/option`, `gleam/list`, `gleam/int`, and
  `gleam/float`.

### Registry Exit Plan

- [x] Model initial interfaces for common stdlib modules.
- [x] Resolve stdlib module interfaces consistently with project modules.
- [x] Type-check dependency functions, constructors, and types through imported
      module interface schemes.
- [x] Add diagnostics for unsupported stdlib modules, functions, types, and
      target combinations.
- [x] Mark every registry entry as temporary interface, upstream source,
      package asset, runtime primitive, or target host adapter.
- [x] Record the deletion condition for every temporary entry.
- [x] Move compiler-owned primitives out of the stdlib registry and into
      normal runtime, external, or ABI tables.
- [ ] Reject adding new registry behavior unless it has a removal condition.

### Dependency Metadata And Assets

- [x] Load or model dependency package metadata needed for stdlib modules.
- [x] Load external module interfaces from dependency metadata where available.
- [ ] Load stdlib source and package assets from the same package root.
- [ ] Validate stdlib-relative JS external modules such as
      `../gleam_stdlib.mjs` and `../dict.mjs` for the stdlib package only.
- [ ] Preserve upstream external module and function names in diagnostics and
      JS metadata, even when a Regulus helper is used internally.
- [ ] Reject arbitrary user relative JS imports unless a separate package asset
      policy defines them.
- [ ] Add fixtures for upstream JS externals that exercise package asset
      resolution.

### Target Selection

- [x] Filter target-group declarations before typing and lowering.
- [ ] Preserve standalone `@target(erlang)` and `@target(javascript)`
      attributes on parsed declarations.
- [ ] Apply target filtering to functions, constants, types, and externals.
- [ ] Treat upstream `javascript` declarations as available to browser,
      bundler, and Node.js profiles.
- [ ] Add duplicate-name fixtures where target selection prevents conflicts,
      including upstream `gleam/set` shapes.
- [ ] Add diagnostics for selected code that references declarations removed
      by target filtering.

### Native Types And `anything`

- [x] Preserve bodyless runtime types as external type interfaces.
- [ ] Define the internal type representation for `anything`.
- [ ] Allow `anything` in stdlib-native externals such as dynamic casts,
      dynamic indexes, and `string.inspect`.
- [ ] Reject unsupported user exports, imports, and general ABI positions that
      use `anything`.
- [ ] Add diagnostics that distinguish `anything` from ordinary generic type
      variables.
- [ ] Add upstream fixtures for `dynamic.cast`,
      `dynamic/decode.bare_index`, and `string.inspect`.

### Runtime Primitive Scope

- [x] Implement allocation, managed values, strings, bit arrays, lists,
      equality, debug, result, option, order, and closure helpers needed by
      current programs.
- [x] Implement Wasmtime and browser IO host calls where supported.
- [x] Reject unavailable host calls and unsupported ABI shapes before byte
      emission.
- [x] Inventory every current `runtime::stdlib_runtime_primitive` entry by
      owner, blocker, and deletion condition.
- [x] Mark entries that are true compiler/runtime primitives and keep only
      those in runtime or ABI tables.
- [x] Mark library-level entries that must be replaced by compiled upstream
      source.
- [x] Delete the first unblocked source-backed runtime dispatch entries:
      `gleam/bool.negate`, `gleam/list.fold`, `gleam/list.map`,
      `gleam/option.map`, and `gleam/result.map`.
- [x] Load the source-proven scalar stdlib module subset by default for
      `gleam_stdlib` dependency packages, while keeping registry interfaces
      transitional.
- [x] Keep dependency package functions internal during project linking so
      public generic stdlib helpers are not exported through the host ABI.
- [x] Delete the next source-backed scalar runtime dispatch entries:
      `gleam/bool.to_string`, `gleam/float.compare`, `gleam/float.max`,
      `gleam/float.min`, `gleam/float.negate`,
      `gleam/function.identity`, `gleam/list.length`, and
      `gleam/list.reverse`.
- [x] For remaining unblocked library-level entries, add an upstream source
      proof before deleting the runtime dispatch arm. Checklist: compile the
      upstream function, add a behavior fixture, assert the linked dump uses
      `gleam_stdlib:...`, then assert it no longer uses
      `__stdlib_gleam_*`.
- [x] Remove the remaining registry-path blockers before deleting scalar
      runtime dispatch arms: upstream bodies for fixture-missing functions and
      native replacements for bodyless externals.
- [ ] Delete remaining scalar and registry-retained library dispatch arms once
      source loading is the default path for those modules.
- [ ] Keep host adapters such as `gleam/io.print` and `gleam/io.println` in
      ABI tables, not the stdlib registry.
- [ ] Add unsupported-feature diagnostics for runtime primitives requested by
      compiled library code but not implemented.

The current primitive inventory is maintained in
[`16_runtime_primitive_inventory.md`](../specs/16_runtime_primitive_inventory.md).

### Host ABI And Externals

- [x] Define ABI rules for scalars, strings, bit arrays, lists, tuples,
      records, custom types, functions, errors, and panics.
- [x] Define managed value ownership rules across the host boundary.
- [x] Lower non-stdlib external functions to Wasm imports.
- [x] Preserve external module and function names in import metadata.
- [x] Validate external import modules against the selected target.
- [x] Reject unsupported external parameter and return shapes before byte
      emission.
- [x] Add table-driven tests for target groups, ABI shapes, and JS host
      imports.
- [ ] Define how dynamic values and opaque native stdlib values cross the JS
      host ABI.
- [ ] Split ordinary user JS import validation from dependency package asset
      validation.

### Higher-Order Calls

- [x] Define one closure-callback ABI for compiler intrinsics, runtime helpers,
      and compiler-generated adapters.
- [x] Lower intrinsics that invoke user closures through ordinary IR or
      generated closure adapters.
- [x] Reuse closure capture layout, indirect-call dispatch, type checks, and
      result ABI for intrinsic callbacks.
- [x] Support callback-taking stdlib functions such as `list.map`,
      `list.fold`, `result.map`, `option.map`, `function.compose`, and
      `function.flip` through the shared mechanism.
- [x] Add diagnostics for unsupported callback parameter, return, capture, or
      host boundary ABI shapes.
- [ ] Add tests proving upstream decoder and collection combinators call
      normal compiled closures rather than runtime-specific callback paths.

### Collections, Dynamic, Text, And Binary

- [x] Support current registry-backed `gleam/dict` and `gleam/bit_array`
      surfaces.
- [ ] Decide whether upstream dict uses `dict.mjs`, a runtime primitive, or
      another validated dependency asset.
- [ ] Define equality, hashing, transient update, and JS ABI rules for native
      dict values.
- [ ] Define the JSON bridge from host JSON or JSON text to `Dynamic`.
- [ ] Implement primitive dynamic operations for classification, construction,
      lookup, traversal, and error payloads.
- [ ] Reuse normal closure dispatch for decoder combinators such as `field`,
      `map`, `then`, `one_of`, and `recursive`.
- [ ] Implement or validate primitives/assets for Unicode, string slicing,
      casing, trimming, inspect, parse, string trees, byte trees, base16,
      base64, byte slicing, URI parsing, percent encoding, and query handling.
- [ ] Expand bit-string construction and matching to upstream segment forms.
- [ ] Add upstream compile fixtures for `gleam/dict`, `gleam/dynamic`,
      `gleam/dynamic/decode`, `gleam/string`, `gleam/string_tree`,
      `gleam/bytes_tree`, `gleam/bit_array`, `gleam/set`, and `gleam/uri`.

### User-Facing Coverage

- [x] Add fixtures using common Gleam stdlib modules.
- [ ] Add stdlib package smoke tests that compile selected upstream modules
      from source.
- [ ] Add behavior fixtures for selected compiled upstream modules.
- [ ] Add diagnostics fixtures for unsupported package assets, native types,
      target declarations, dynamic operations, and bit-string segment forms.
- [ ] Add JS host smoke tests for stdlib functions that require package assets.

## Done When

Small programs using stdlib functionality compile and execute through the
documented host interfaces, unsupported stdlib usage fails with a specific
source-spanned diagnostic, and `gleam_stdlib` is no longer represented by a
bespoke compiler registry. The compiler may still have runtime primitive
tables, external ABI validation, host adapters, and dependency package metadata.
