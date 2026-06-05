# Core IR

Core IR is the compiler-owned representation passed from typed Gleam syntax to
WebAssembly code generation. It keeps evaluation order, locals, calls, managed
values, and failure paths explicit.

## Module structure

An IR module records:

- source span
- imports and declarations
- constants
- module initialization metadata
- references and exports
- functions

Functions record parameters, locals, return type, call ABI, body, and source
span. Locals use stable IDs so backend emission does not repeat name lookup.

## Expression forms

The IR represents current executable and backend-facing forms:

- typed literals
- local reads and writes
- direct calls
- indirect calls through function values
- function values and anonymous functions
- blocks with ordered instructions and a result
- branch expressions with clauses, patterns, and guards
- tuples, lists, records, custom constructors, and list cons
- strings and bit arrays as runtime-managed values
- field and tuple-element access
- runtime equality and comparison metadata
- memory allocation, loads, and stores
- failure paths for panic, todo, assert, and match failure

Unsupported source forms should fail during lowering with source-spanned
diagnostics, unless they are intentionally represented for a later backend pass.

## ABI metadata

Calls carry ABI metadata so code generation can distinguish internal calls,
module imports, host imports, and exports. ABI values preserve representation
hints for scalar and managed values.

## Pattern lowering

Source-level pattern syntax lowers into explicit IR branch clauses. Pattern
tests and bindings remain structured enough for the backend to emit
WAT deterministically, while no longer depending on raw Gleam syntax.

## Debug output

IR debug output is deterministic. Tests use snapshots to make changes to
lowering, locals, call ABI data, and managed-value forms easy to inspect.
