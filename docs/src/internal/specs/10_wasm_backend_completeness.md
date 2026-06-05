# WASM backend completeness

The WASM backend should support every typed IR form needed by complete Gleam
programs. Backend completeness means any program accepted by the language phases
can either be emitted as valid WebAssembly for the selected target, or rejected
with a precise target/ABI diagnostic.

## Scope

This spec covers target representation, runtime support, imports, exports, and
WebAssembly emission. Language semantics before code generation are covered by
the syntax, resolution, type checking, pattern matching, and IR specs.

## Value representation

The backend must define and implement WASM representations for:

- integers, floats, bools, nil, and ordering values
- strings and UTF-8 data
- bit arrays and segment operations
- lists, tuples, records, and custom types
- results, options, errors, and panic values
- function values, closures, captures, and indirect calls
- opaque and dependency-defined runtime values

Managed values use the layout in
[Runtime representation](../development/runtime_representation.md). `Result`
and `Option` are ordinary custom values. Runtime errors, panic payloads, and
opaque values use dedicated runtime tags so helpers can distinguish them from
user variants.

## Runtime operations

The runtime should provide allocation, access, and helper operations for:

- string creation, comparison, concatenation, and inspection
- bit-array construction, append, slicing, and pattern matching
- list construction, traversal, equality, and deconstruction
- tuple, record, and custom-type field access
- closure allocation and invocation
- structural equality and ordering where Gleam requires them
- panic, todo, assert, and pattern-match failure paths
- debug rendering for arbitrary values

## Control flow and calls

Code generation should emit deterministic WASM for:

- blocks, lets, assignments to locals, and module initialization
- branches, guards, and lowered pattern matching
- direct calls, imported calls, exported calls, and indirect calls
- tail-position calls where useful, without changing semantics
- all operators and short-circuiting boolean expressions

## Imports, exports, and ABI

The backend must define target-specific ABI rules for:

- module exports
- module imports
- host imports
- scalar and managed values crossing boundaries
- ownership and lifetime of managed values
- adapters for ABI shapes that raw WASM cannot express directly

Targets should include Wasmtime first, then browser and WASI support when their
host interfaces are defined.

## Validation

Every emitted module should assemble and validate as WebAssembly. Tests should
cover WAT snapshots, Wasmtime execution, memory-layout inspection, import/export
ABI checks, runtime helper behavior, and diagnostics for unsupported target
combinations.

## Done when

Any typed IR produced for supported Gleam language features can be emitted and
run for the selected WASM target, or fails before assembly with a clear
source-spanned backend or ABI diagnostic.
