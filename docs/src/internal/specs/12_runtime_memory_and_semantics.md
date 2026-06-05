# Open runtime memory and value semantics

Current runtime layout and helper behavior are documented in
[Runtime representation](../development/runtime_representation.md) and
[WASM backend and runtime](../development/wasm_backend_and_runtime.md). This
spec tracks the remaining design decisions.

## Memory management

Choose and document a long-term strategy for managed values:

- bump allocation with explicit reset points
- reference counting
- tracing garbage collection
- host-owned arenas
- another documented strategy

The chosen strategy must define allocation failure behavior, heap growth, object
lifetime, ownership across host boundaries, and whether values can move.

## Equality and ordering

Runtime equality should match Gleam semantics for all comparable values.
Managed values need recursive structural equality with cycle-safe or acyclic
guarantees.

Ordering helpers should support every type Gleam allows to be ordered and should
reject unsupported ordering before code generation when possible.

## Inspection and debug rendering

String inspection and debug rendering should produce deterministic text for
arbitrary runtime values. Rendering should handle nested lists, tuples, records,
custom values, bit arrays, strings, nil, booleans, numbers, opaque values,
errors, and panics.

## Error and panic reporting

Panic, todo, assert, pattern-match failure, and runtime error paths should carry
a structured payload when useful. Hosts should be able to inspect or render the
payload without knowing compiler internals.

## Active tasks

See [Runtime memory tasks](../tasks/12_runtime_memory_and_semantics.md).
