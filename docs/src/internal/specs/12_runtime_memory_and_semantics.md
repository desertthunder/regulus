# Runtime memory and value semantics

The current runtime uses a bump allocator and minimal helper semantics. Complete
runtime support needs explicit memory management choices and full value helper
behavior.

## Memory management

The runtime must choose and document a long-term strategy for managed values:

- bump allocation with explicit reset points
- reference counting
- tracing garbage collection
- host-owned arenas
- another documented strategy

The chosen strategy must define allocation failure behavior, heap growth, object
lifetime, ownership across host boundaries, and whether values can move.

## Equality and ordering

Runtime equality should match Gleam semantics for all comparable values. Managed
values need recursive structural equality with cycle-safe or acyclic guarantees.

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

## Done when

Runtime-managed programs can allocate, compare, order, inspect, debug, and
report failures consistently without relying on pointer identity or unbounded
unchecked heap growth.
