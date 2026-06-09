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

### Group 2: remaining stdlib

- [ ] Support `gleam/bool`.
- [ ] Support `gleam/dict`.
- [ ] Support `gleam/float`.
- [ ] Support `gleam/function`.
- [ ] Support `gleam/bit_array`.
- [ ] Support `gleam/bytes_tree`.
- [ ] Support `gleam/string_tree`.
- [ ] Support `gleam/dynamic`.
- [ ] Support `gleam/dynamic/decode`.
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
