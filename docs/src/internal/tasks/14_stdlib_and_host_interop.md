# Standard library and host interop tasks

## Goal

Support useful standard library modules and host calls.

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

### Host ABI

- [x] Define concrete host imports and adapters for Wasmtime, browser, and WASI
      targets where supported.
- [x] Define ABI rules for scalar values, strings, bit arrays, lists, tuples,
      records, custom types, functions, errors, and panics.
- [x] Define ownership rules for managed values crossing the host boundary.
- [x] Define how host code reads compiler memory and how compiler code receives
      host-provided managed values.
- [x] Add rich managed-value wrappers or adapter functions for ABI shapes that
      do not map directly to raw WASM parameters and results.

### Intrinsics and host calls

- [x] Implement or import IO functions for Wasmtime tests and browser examples.
- [x] Implement equality, string, bit-array, list, result, option, and debug
      helpers where needed by compiled programs.
- [x] Ensure host imports and adapters are target-specific and produce
      diagnostics on targets where they are unavailable.
- [x] Add Wasmtime tests for host imported functions.
- [x] Add fixtures using common Gleam stdlib modules.

### General external functions

- [ ] Lower non-stdlib external functions to Wasm imports.
- [ ] Preserve external module and function names in import metadata.
- [ ] Validate external import modules against the selected target.
- [ ] Reject unsupported external function ABI shapes before byte emission.
- [ ] Add tests for browser and Worker-style external imports.

### Browser and Worker adapters

- [ ] Add stable host helpers for writing host strings into guest memory.
- [ ] Add stable host helpers for reading guest strings from managed pointers.
- [ ] Add adapter conventions for lists of strings and small records.
- [ ] Add adapter conventions for `Result` and `Option` values crossing the
      host boundary.
- [ ] Define response-shape adapters for status, headers, and body data.
- [ ] Decide whether Cloudflare Workers are a target or browser host profile.
- [ ] Document import module names for browser and Worker host adapters.

### Higher-order intrinsics and runtime callbacks

- [ ] Define one closure-callback ABI for compiler intrinsics, runtime helpers,
      and compiler-generated adapters.
- [ ] Lower intrinsics that invoke user closures through IR or generated
      closure adapters, not bespoke WAT callback code.
- [ ] Reuse ordinary closure capture layout, indirect-call dispatch, type
      checks, and result ABI for intrinsic callbacks.
- [ ] Support callback-taking stdlib functions such as `list.map`,
      `list.fold`, `result.map`, `option.map`, `function.compose`, and
      `function.flip` through the shared mechanism.
- [ ] Add diagnostics for unsupported callback parameter, return, capture, or
      host boundary ABI shapes before WAT assembly.
- [ ] Add tests for closures passed to intrinsics, captured closures, nested
      callbacks, generic callbacks, and callback failures.

### JSON and structured data

- [ ] Define the JSON bridge from host JSON or JSON text to `Dynamic`.
- [ ] Map JSON null, bool, number, string, array, and object values to
      documented dynamic runtime shapes.
- [ ] Support full `gleam/dynamic` value construction and classification.
- [ ] Support full `gleam/dynamic/decode` compatibility so JSON decoding
      happens in Gleam.
- [ ] Lower decoder combinators that call user closures through IR or another
      compiler-owned plan rather than plain runtime WAT helpers.
- [ ] Reuse normal closure dispatch for decoder continuations used by `field`,
      `map`, `then`, `recursive`, and generated record decoders.
- [ ] Implement property lookup and path traversal for `field`, `subfield`,
      `at`, `optional_field`, and `optionally_at`.
- [ ] Implement collection decoding for `list` and `dict`, including nested
      error paths and error aggregation.
- [ ] Implement decoder composition for `success`, `failure`, `map`, `then`,
      `one_of`, `collapse_errors`, and `map_errors`.
- [ ] Implement `optional`, `recursive`, and `new_primitive_decoder` with the
      same behavior as the Gleam stdlib.
- [ ] Construct real `DecodeError(expected, found, path)` values.
- [ ] Add diagnostics for unsupported decoders and structured response shapes.
- [ ] Add fixtures for simple JSON input and JSON-like structured output.
- [ ] Add fixtures for nested objects, optional/null fields, lists, dicts,
      records, enum variants, `one_of`, and decode error paths.

### Group 2: remaining stdlib

- [x] Support `gleam/bool`.
- [x] Support `gleam/dict`.
- [x] Support `gleam/float`.
- [x] Support `gleam/function`.
- [x] Support `gleam/bit_array`.
- [ ] Support `gleam/bytes_tree`.
- [ ] Support `gleam/string_tree`.
- [ ] Support full `gleam/dynamic`.
- [ ] Support full `gleam/dynamic/decode`.
- [ ] Support `gleam/pair`.
- [ ] Support `gleam/set`.
- [ ] Support `gleam/uri`.
- [ ] Prefer compiling stdlib Gleam source for Group 2 where possible.
- [ ] Add target-specific intrinsics or host adapters only when source
      compilation is not enough.

## Done when

Small programs using selected Gleam stdlib functionality compile and execute
against a documented host interface, and unsupported stdlib usage fails with a
specific source-spanned diagnostic.
