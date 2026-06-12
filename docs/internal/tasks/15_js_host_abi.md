# JS host ABI tasks

## Goal

Define and implement the first usable JavaScript host ABI for Regulus. This is
the primary usability milestone before broad stdlib completion.

## Milestone slice

- [ ] Compile a Gleam project with one selected JavaScript external.
- [ ] Emit or package bundler-oriented ES module glue for the Wasm artifact.
- [ ] Pass a JavaScript string into Gleam through the glue.
- [ ] Return a Gleam string to JavaScript through the glue.
- [ ] Add one smoke test that runs the whole path without handwritten pointer
      arithmetic in application code.

## Tasks

### ABI shape

- [ ] Define the JS host ABI document for scalar and managed values.
- [ ] Define import module names for shared JS, browser, bundler, and Node.js
      profiles.
- [ ] Add a profile-selection check that accepts browser, bundler, and Node.js
      and rejects unknown JS host profiles.
- [ ] Define supported exported function parameter and return shapes for JS
      hosts.
- [ ] Add source-spanned diagnostics for unsupported JS import and export
      shapes.

### String helpers

- [ ] Export a stable helper for allocating or writing a JS string into guest
      memory.
- [ ] Export stable helpers for reading string length and bytes from a managed
      string pointer.
- [ ] Add JS tests for passing strings from JS to Gleam imports and exports.
- [ ] Add JS tests for reading strings returned from exported Gleam functions.

### Managed value readers

- [ ] Export stable helpers for reading managed value tags, arity, and fields.
- [ ] Define the JS reader contract for tuples, records, and custom types.
- [ ] Define the JS reader contract for lists and lists of strings.
- [ ] Define the JS reader contract for `Result` and `Option` values.
- [ ] Add JS tests that read records, lists, `Result`, and `Option` values from
      exported Gleam functions.

### Opaque JS handles

- [ ] Define the runtime representation for opaque host handles.
- [ ] Define ownership and lifetime rules for JS handles passed to Gleam.
- [ ] Add ABI validation for externals that accept or return opaque handles.
- [ ] Add tests for passing a handle through Gleam without exposing JS object
      internals to guest memory.

### Bodyless externals

- [ ] Parse bodyless `@external` declarations from project and dependency
      source.
- [ ] Represent bodyless externals in AST, resolver interfaces, and typed
      module interfaces.
- [ ] Select the target-specific external before lowering.
- [ ] Lower selected JavaScript externals through the same ABI as handwritten
      project externals.
- [ ] Add diagnostics for missing selected externals and unsupported external
      ABI shapes.

### Browser profile

- [ ] Define browser import module names for fetch, local storage, time, and
      online state.
- [ ] Add browser-profile validation for allowed external modules and names.
- [ ] Add browser-page glue that instantiates Wasm with checked imports.
- [ ] Add a browser smoke test for one string import and one string export.

### Bundler profile

- [ ] Define ES module glue shape for bundler-based hosts.
- [ ] Add bundler-profile validation for allowed external modules and names.
- [ ] Add bundler glue that exposes checked imports and typed exported calls.
- [ ] Add a bundler smoke test for one string import and one string export.
- [ ] Add a bundler fixture for a string request input and string response
      output.
- [ ] Add a bundler fixture that returns tagged response data across the JS host
      ABI.

### Node.js profile

- [ ] Define Node.js loading for generated or packaged `.wasm` files.
- [ ] Add Node.js-profile validation for allowed external modules and names.
- [ ] Add Node.js glue that instantiates Wasm with checked imports.
- [ ] Add a Node.js smoke test for one string import and one string export.

### Generated or packaged JS glue

- [ ] Decide whether the CLI emits JS glue, copies a checked adapter template,
      or documents a stable handwritten adapter for the first milestone.
- [ ] Emit or package deterministic JS host adapter files for browser, bundler,
      and Node.js profiles.
- [ ] Include import and export metadata needed by JS glue in debug output.
- [ ] Add CLI tests for stable JS adapter artifact paths.

## Done when

Browser, bundler, and Node.js hosts can call compiled Gleam Wasm through a
documented JS host ABI without handwritten pointer arithmetic in example
application code.
