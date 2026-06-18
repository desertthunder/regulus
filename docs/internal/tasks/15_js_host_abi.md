# JS host ABI tasks

## Goal

Define and implement the first usable JavaScript host ABI for Regulus. This is
the primary usability milestone before broad stdlib completion.

The public ABI contract lives in
[JavaScript host ABI contract](../../website/development/js_abi.md).

## Milestone slice

- [x] Compile a Gleam project with one selected JavaScript external.
- [x] Emit or package bundler-oriented ES module glue for the Wasm artifact.
- [x] Pass a JavaScript string into Gleam through the glue.
- [x] Return a Gleam string to JavaScript through the glue.
- [x] Add one smoke test that runs the whole path without handwritten pointer
      arithmetic in application code.

## Tasks

### ABI shape

- [x] Define the JS host ABI document for scalar and managed values.
- [x] Define import module names for shared JS, browser, bundler, and Node.js
      profiles.
- [x] Add a profile-selection check that accepts browser, bundler, and Node.js
      and rejects unknown JS host profiles.
- [x] Define supported exported function parameter and return shapes for JS
      hosts.
- [x] Add source-spanned diagnostics for unsupported JS import and export
      shapes.

### String helpers

- [x] Export a stable helper for allocating or writing a JS string into guest
      memory.
- [x] Export stable helpers for reading string length and bytes from a managed
      string pointer.
- [x] Add JS tests for passing strings from JS to Gleam imports and exports.
- [x] Add JS tests for reading strings returned from exported Gleam functions.

### Managed value readers

- [x] Export stable helpers for reading managed value tags, arity, and fields.
- [x] Define the JS reader contract for tuples, records, and custom types.
- [x] Define the JS reader contract for lists and lists of strings.
- [x] Define the JS reader contract for `Result` and `Option` values.
- [x] Implement JS adapter readers for tuples, records, and custom types.
- [x] Implement JS adapter readers for lists and lists of strings.
- [x] Implement JS adapter readers for `Result` and `Option` values.
- [x] Add export metadata for structured values consumed by JS readers.
- [x] Allow supported structured return shapes for JS exports once typed readers
      are available.
- [x] Add JS tests that read records, lists, `Result`, and `Option` values from
      exported Gleam functions.

### Opaque JS handles

- [x] Define the runtime representation for opaque host handles.
- [x] Implement the runtime representation for opaque host handles.
- [x] Define ownership and lifetime rules for JS handles passed to Gleam.
- [x] Implement JS handle table ownership and release behavior.
- [x] Add ABI validation for externals that accept or return opaque handles.
- [x] Add JS adapter conversion for opaque handle imports and exports.
- [x] Add e2e tests for passing a handle through Gleam without exposing JS
      object internals to guest memory.

### Bodyless externals

- [x] Parse bodyless `@external` declarations from project and dependency
      source.
- [x] Represent bodyless externals in AST, resolver interfaces, and typed
      module interfaces.
- [x] Select the target-specific external before lowering.
- [x] Lower selected JavaScript externals through the same ABI as handwritten
      project externals.
- [x] Add diagnostics for missing selected externals and unsupported external
      ABI shapes.

### Browser profile

- [x] Define browser import module names for fetch, local storage, time, and
      online state.
- [x] Implement browser adapter imports for the defined browser APIs.
- [x] Add browser-profile validation for allowed external modules and names.
- [x] Add browser-page glue that instantiates Wasm with checked imports.
- [x] Add a browser smoke test for one string import and one string export.

### Bundler profile

- [x] Define ES module glue shape for bundler-based hosts.
- [x] Add bundler-profile validation for allowed external modules and names.
- [x] Add bundler glue that exposes checked imports and typed exported calls.
- [x] Add a bundler smoke test for one string import and one string export.
- [x] Add a bundler fixture for a string request input and string response
      output.
- [x] Add a bundler fixture that returns tagged response data across the JS host
      ABI.
- [x] Wire bundler glue to structured value readers for tagged response data.

### Node.js profile

- [x] Define Node.js loading for generated or packaged `.wasm` files.
- [x] Implement Node.js loading for generated or packaged `.wasm` files.
- [x] Add Node.js-profile validation for allowed external modules and names.
- [x] Add Node.js glue that instantiates Wasm with checked imports.
- [x] Add a Node.js smoke test for one string import and one string export.

### Generated or packaged JS glue

- [x] Decide whether the CLI emits JS glue, copies a checked adapter template,
      or documents a stable handwritten adapter for the first milestone.
- [ ] Emit or package deterministic JS host adapter files for browser and
      Node.js profiles.
- [x] Include import and export metadata needed by generated bundler glue.
- [ ] Include import and export metadata in explicit debug output where useful.
- [x] Generate or expose typed JS wrappers from import and export metadata.
- [x] Add CLI tests for stable bundler JS adapter artifact paths.

## Done when

Browser, bundler, and Node.js hosts can call compiled Gleam Wasm through a
documented JS host ABI without handwritten pointer arithmetic in example
application code.
