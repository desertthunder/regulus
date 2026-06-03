# Runtime representation tasks

## Goal

Design and implement the in-memory representation for Gleam values in WASM.

## Tasks

- [ ] Specify object headers, tags, and alignment.
- [ ] Specify string encoding and layout.
- [ ] Specify list, tuple, record, and custom-type layouts.
- [ ] Specify closure and function-value representation.
- [ ] Choose allocation and ownership or garbage-collection strategy.
- [ ] Define the host ABI for managed values.
- [ ] Implement runtime allocation helpers.
- [ ] Add memory-layout tests in Wasmtime.

## Done when

The backend can allocate, pass, inspect, and return runtime-managed values using
a documented representation.
