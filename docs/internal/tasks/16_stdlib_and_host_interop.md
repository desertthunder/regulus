# Standard library and host interop tasks

## Goal

Compile upstream `gleam_stdlib` from source and remove the stdlib registry.

## Direction

- [ ] Keep explicit stdlib intrinsics limited to compiler/runtime primitives.
- [ ] Prefer compiled upstream Gleam source for library behavior.
- [ ] Use bodyless externals and validated stdlib shims for native behavior.
- [ ] Use the JS host ABI for JavaScript-backed stdlib and dependency calls.
- [ ] Replace registry behavior module by module as source fixtures compile.
- [ ] Finish by deleting the stdlib registry instead of preserving it as an
      interface cache or compatibility table.

## Tasks

### Standard library strategy

- [x] Decide which stdlib modules are compiled from Gleam source, shimmed as
      host imports, or implemented as compiler/runtime intrinsics.
- [x] Define a table-driven stdlib registry for module interfaces and lowering
      strategies.
- [x] Load or model dependency package metadata needed for stdlib modules.
- [x] Resolve stdlib module interfaces consistently with project module
      interfaces.
- [x] Load external module interfaces from dependency metadata where available.
- [x] Type-check selected stdlib functions using the same interface data as user
      modules.
- [x] Type-check dependency functions, constructors, and types through imported
      module interface schemes.
- [x] Add diagnostics for unsupported stdlib modules, functions, types, or
      target combinations.
- [ ] Mark each registry entry as compiled source, temporary interface,
      intrinsic, stdlib shim, or target host adapter.
- [ ] Record the intended replacement path for each temporary registry member.
- [ ] Move compiler-owned primitives out of the stdlib registry and into the
      normal runtime or external primitive tables.
- [ ] Delete registry entries once their interfaces come from package metadata
      or compiled source.

### Upstream stdlib audit and migration

- [ ] Add a fixture that compiles the published upstream `gleam_stdlib` source
      as a dependency package.
- [ ] Snapshot the first compile blocker for each upstream stdlib module.
- [ ] Group blocker reports by source language feature, target selection,
      dependency package asset, runtime primitive, and ABI shape.
- [ ] Compile `gleam/pair` from upstream source as the first registry-removal
      proof.
- [ ] Compile pure portions of `gleam/order`, `gleam/result`, `gleam/option`,
      `gleam/list`, `gleam/int`, and `gleam/float` from upstream source where
      the current runtime is sufficient.
- [ ] Keep registry-backed behavior only where the blocker report shows a real
      native, runtime, target, or ABI dependency.
- [ ] Remove the table-driven stdlib registry once every remaining member is
      represented by package source, package metadata, validated shims, or
      runtime primitives.

### Target attributes

- [x] Filter target-group declarations before typing and lowering so
      target-specific externals can reuse local names safely.
- [ ] Preserve standalone `@target(erlang)` and `@target(javascript)`
      attributes on parsed declarations.
- [ ] Apply target filtering to upstream functions, constants, types, and
      externals, not only grouped externals.
- [ ] Treat upstream `javascript` declarations as available to browser,
      bundler, and Node.js profiles.
- [ ] Add duplicate-name fixtures where target selection prevents conflicts,
      including the upstream `gleam/set` token shape.
- [ ] Add diagnostics for declarations eliminated by target filtering when they
      are referenced from selected code.

### Bodyless types and `anything`

- [x] Preserve bodyless runtime types as external type interfaces.
- [ ] Define the internal type representation for `anything`.
- [ ] Allow `anything` in stdlib-native externals such as dynamic casts,
      dynamic indexes, and `string.inspect`.
- [ ] Reject unsupported user exports, imports, and general ABI positions that
      use `anything`.
- [ ] Add diagnostics that distinguish `anything` from ordinary generic type
      variables when lowering cannot support it.
- [ ] Add fixtures for upstream `dynamic.cast`, `dynamic/decode.bare_index`,
      and `string.inspect`.

### Stdlib native shims

- [ ] Decide whether JS stdlib shims are packaged as source assets, mapped to
      runtime helpers, or replaced by compiler intrinsics.
- [ ] Validate stdlib-relative JS external modules such as
      `../gleam_stdlib.mjs` and `../dict.mjs` for the stdlib package only.
- [ ] Preserve upstream external module and function names in diagnostics and
      JS metadata even when a Regulus helper is used internally.
- [ ] Reject arbitrary user relative JS imports unless a separate package asset
      policy defines them.
- [ ] Add fixtures for selected upstream JS externals that exercise stdlib
      package asset resolution.

### Group 1: initial useful stdlib

- [x] Model interfaces for `gleam/io`, `gleam/int`, `gleam/string`,
      `gleam/list`, `gleam/result`, `gleam/option`, and `gleam/order`.
- [x] Implement `gleam/io.println` and `gleam/io.print` as host calls.
- [x] Implement `gleam/int.to_string` as an intrinsic or runtime helper.
- [x] Implement `gleam/string.append`, `concat`, `length`, and `is_empty`.
- [x] Implement `gleam/list.length` and `gleam/list.reverse`.
- [x] Support `gleam/result.Result`, `Ok`, and `Error` in interfaces and
      lowering.
- [x] Support `gleam/option.Option`, `Some`, and `None` in interfaces and
      lowering.
- [x] Support `gleam/order.Order`, `Lt`, `Eq`, and `Gt` in interfaces and
      lowering.
- [ ] Replace eligible group-1 registry behavior with compiled upstream source
      after the source fixture and native blockers are in place.

### Host ABI

- [x] Define concrete host imports and adapters for Wasmtime, browser, and WASI
      targets where supported.
- [x] Define ABI rules for scalar values, strings, bit arrays, lists, tuples,
      records, custom types, functions, errors, and panics.
- [x] Define ownership rules for managed values crossing the host boundary.
- [x] Define how host code reads compiler memory and how compiler code receives
      host-provided managed values.
- [x] Add rich managed-value wrappers or adapter functions for ABI shapes that
      do not map directly to raw Wasm parameters and results.
- [ ] Define how dynamic values and opaque native stdlib values cross the JS
      host ABI.

### Intrinsics and host calls

- [x] Implement or import IO functions for Wasmtime tests and browser examples.
- [x] Implement equality, string, bit-array, list, result, option, and debug
      helpers where needed by compiled programs.
- [x] Ensure host imports and adapters are target-specific and produce
      diagnostics on targets where they are unavailable.
- [x] Add Wasmtime tests for host imported functions.
- [x] Add fixtures using common Gleam stdlib modules.

### General external functions

- [x] Lower non-stdlib external functions to Wasm imports.
- [x] Preserve external module and function names in import metadata.
- [x] Validate external import modules against the selected target.
- [x] Add a centralized external ABI validator with source-spanned diagnostics
      for unsupported parameter and return shapes.
- [x] Reject unsupported external function ABI shapes before byte emission.
- [x] Add table-driven tests for selected target groups, unsupported ABI
      shapes, supported managed shapes, and JS host imports.
- [ ] Split ordinary user JS import validation from validated dependency
      package asset imports.

### Higher-order intrinsics and runtime callbacks

- [x] Define one closure-callback ABI for compiler intrinsics, runtime helpers,
      and compiler-generated adapters.
- [x] Lower intrinsics that invoke user closures through IR or generated
      closure adapters, not bespoke WAT callback code.
- [x] Reuse ordinary closure capture layout, indirect-call dispatch, type
      checks, and result ABI for intrinsic callbacks.
- [x] Support callback-taking stdlib functions such as `list.map`,
      `list.fold`, `result.map`, `option.map`, `function.compose`, and
      `function.flip` through the shared mechanism.
- [x] Add diagnostics for unsupported callback parameter, return, capture, or
      host boundary ABI shapes before WAT assembly.
- [x] Add tests for closures passed to intrinsics, captured closures, nested
      callbacks, generic callbacks, and callback failures.

### Dict and collection runtime

- [x] Support the current registry-backed `gleam/dict` surface.
- [ ] Define whether upstream dict uses `dict.mjs`, a Regulus runtime helper,
      or compiler intrinsics.
- [ ] Implement or shim the native `Dict` and `TransientDict`
      representations used by upstream source.
- [ ] Define equality and hashing semantics for dict keys.
- [ ] Support callback ABI shapes needed by dict fold, map, filter, and merge
      operations.
- [ ] Define how dict values cross the JS host ABI and structured output
      helpers.
- [ ] Add an upstream `gleam/dict` compile fixture and blocker report.
- [ ] Add behavior fixtures for insert, delete, get, fold, merge, equality,
      and transient update paths.

### Dynamic values and structured data

- [ ] Define the JSON bridge from host JSON or JSON text to `Dynamic`.
- [ ] Map JSON null, bool, number, string, array, and object values to
      documented dynamic runtime shapes.
- [ ] Support full `gleam/dynamic` value construction and classification.
- [ ] Support `anything` in dynamic cast and dynamic index boundaries.
- [ ] Implement primitive dynamic runtime operations for classification,
      property lookup, index lookup, null checks, list traversal, object
      traversal, and value construction.
- [ ] Add compile fixtures for upstream `gleam/dynamic` and
      `gleam/dynamic/decode`.
- [ ] Reuse normal closure dispatch for decoder continuations used by `field`,
      `map`, `then`, `recursive`, and generated record decoders.
- [ ] Implement missing primitives required by compiled decoder source for path
      traversal, collection traversal, error aggregation, recursion, and custom
      primitive decoders.
- [ ] Construct real `DecodeError(expected, found, path)` values through
      compiled stdlib code or a documented primitive constructor.
- [ ] Add diagnostics for unsupported dynamic operations, dependency modules,
      bridge shapes, and structured response shapes.
- [ ] Add fixtures for simple JSON input and JSON-like structured output.
- [ ] Add fixtures for nested objects, optional/null fields, lists, dicts,
      records, enum variants, `one_of`, and decode error paths.

### Text, binary, and URI primitives

- [x] Support the current registry-backed `gleam/bit_array` surface.
- [ ] Define native helper strategy for upstream `gleam/string`.
- [ ] Implement or shim Unicode codepoint, grapheme, slicing, replace, split,
      casing, trimming, inspect, and parse helpers required by upstream string
      source.
- [ ] Implement or shim `gleam/string_tree` and its iodata-style conversion
      helpers.
- [ ] Implement or shim `gleam/bytes_tree` and byte-tree flattening helpers.
- [ ] Implement or shim base16, base64, byte slicing, and byte classification
      helpers used by upstream stdlib.
- [ ] Implement or shim URI parsing, percent encode/decode, query handling, and
      reconstruction helpers needed by `gleam/uri`.
- [ ] Expand bit-string construction and deconstruction to sized segments used
      by upstream `gleam/bit_array`.
- [ ] Add diagnostics for unsupported segment options, byte alignment, and
      binary pattern forms.
- [ ] Add upstream compile fixtures for `gleam/string`,
      `gleam/string_tree`, `gleam/bytes_tree`, `gleam/bit_array`, and
      `gleam/uri`.

### Runtime scope

- [ ] Add a runtime helper inventory grouped by allocation, managed values,
      closures, equality, debug, dynamic values, native shims, opaque handles,
      and host adapters.
- [ ] Add tests that prove dynamic decoder combinators call normal compiled
      closures rather than runtime-specific callback paths.
- [ ] Replace any runtime helper that implements library-level decoder,
      routing, URI, or response behavior with a compile fixture for the library
      code.
- [ ] Add unsupported-feature diagnostics for any runtime primitive requested by
      compiled library code but not implemented.

### Group 2: remaining stdlib

- [x] Support `gleam/bool`.
- [x] Support `gleam/dict`.
- [x] Support `gleam/float`.
- [x] Support `gleam/function`.
- [x] Support `gleam/bit_array`.
- [ ] Compile or support `gleam/pair`.
- [ ] Compile or support `gleam/set` after target attributes and dict runtime
      support are in place.
- [ ] Compile or support `gleam/bytes_tree`.
- [ ] Compile or support `gleam/string_tree`.
- [ ] Compile or support full `gleam/dynamic`.
- [ ] Compile or support full `gleam/dynamic/decode`.
- [ ] Compile or support `gleam/uri`.
- [ ] Prefer compiling stdlib Gleam source or using bodyless externals for
      Group 2 where possible.
- [ ] Add target-specific intrinsics or host adapters only when source
      compilation and the JS host ABI are not enough.

## Done when

Small programs using selected Gleam stdlib functionality compile and execute
against a documented host interface, unsupported stdlib usage fails with a
specific source-spanned diagnostic, and the stdlib registry has been removed.
The compiler may still have runtime primitive tables, external import
validation, and dependency package metadata, but none of those tables should be
a bespoke stdlib interface registry.
