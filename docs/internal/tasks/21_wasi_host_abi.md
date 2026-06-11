# WASI host ABI tasks

## Goal

Define the first concrete WASI host profile for compiled Gleam Wasm.

## Tasks

### Profile definition

- [ ] Define the supported WASI preview or component model target.
- [ ] Document required imports for the first WASI profile.
- [ ] Add target validation for unsupported WASI imports and exports.
- [ ] Decide which CLI target spelling selects the WASI profile.

### IO support

- [ ] Implement `gleam/io.print` through WASI `fd_write`.
- [ ] Implement `gleam/io.println` through WASI `fd_write`.
- [ ] Add Wasmtime tests that instantiate the module with WASI imports.
- [ ] Add diagnostics for `gleam/io` members unsupported on WASI.

### ABI support

- [ ] Reuse scalar ABI rules for WASI exports.
- [ ] Reuse managed string reader helpers for WASI hosts.
- [ ] Add smoke tests for scalar and string exports on the WASI target.
- [ ] Reject unsupported managed import/export shapes before byte emission.

### Deferred capabilities

- [ ] Track filesystem support separately from stdout and stderr.
- [ ] Track clocks, random bytes, environment data, sockets, and process APIs as
      separate capability groups.
- [ ] Add unsupported-feature diagnostics for each deferred capability.

## Done when

A small Gleam program using `gleam/io.print` or `gleam/io.println` runs through
a WASI host, and unsupported WASI APIs fail with clear target diagnostics.
