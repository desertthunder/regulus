# Runtime representation tasks

## Goal

Design and implement the in-memory representation for Gleam values in WASM.

## Tasks

- [x] Specify object headers, tags, and alignment.
- [x] Specify string encoding and layout.
- [x] Specify list, tuple, record, and custom-type layouts.
- [x] Specify closure and function-value representation.
- [x] Choose allocation and ownership or garbage-collection strategy.
- [x] Define the host ABI for managed values.
- [x] Implement runtime allocation helpers.
- [x] Add memory-layout tests in Wasmtime.

## Done when

The backend can allocate, pass, inspect, and return runtime-managed values using
a documented representation.
