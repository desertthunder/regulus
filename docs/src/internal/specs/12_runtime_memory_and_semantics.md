# Open runtime memory and value semantics

Current runtime layout and helper behavior are documented in
[Runtime representation][runtime] and [WASM backend and runtime][wasm]. This
spec tracks the remaining design decisions.

## Memory management

Regulus uses a resettable bump arena for managed values. Allocation moves a heap
pointer forward, objects are non-moving, and individual objects are not freed.
The arena lifetime lasts until the Wasm instance is reset or a future explicit
arena reset point is reached.

Allocator paths should check available memory and use `memory.grow` before
failing. Allocation failure is a structured runtime panic payload. Host code
borrows managed pointers; those pointers remain stable until reset.

Reference counting and tracing garbage collection are deferred until the
compiler has complete root metadata, stack/local tracking, and host ownership
rules.

## Equality and ordering

Runtime equality matches Gleam semantics for comparable scalar and managed
values. Managed value equality is recursive and structural for strings, bit
arrays, lists, tuples, records, custom values, results, options, errors, and
panics. Runtime values are acyclic, so equality does not need cycle detection.

Closures and opaque values only compare equal when they are the same object.
Distinct closure or opaque objects are not structurally comparable.

Ordering helpers support orderable scalar values and recursive ordering for
managed values used by runtime helpers. Strings and bit arrays use lexicographic
byte ordering. Lists, tuples, records, custom values, errors, and panics compare
slot-by-slot after tag, length, or constructor metadata. Closures and opaque
values have pointer ordering only as a runtime fallback. Source-level ordering
operators reject unsupported types before WAT assembly.

## Inspection and debug rendering

String inspection and debug rendering produce deterministic text for runtime
objects. Rendering handles nested lists, tuples, records, custom values, bit
arrays, strings, opaque placeholders, errors, and panics. Scalar values in
runtime slots render as their numeric slot value.

Debug rendering is a host inspection facility. It does not replace source-level
Gleam `String.inspect` semantics for every type name or constructor name.

## Error and panic reporting

Panic, todo, assert, pattern-match failure, and runtime error objects use the
existing tag-9 error and tag-10 panic layouts: reason tag at offset 8 followed
by 8-byte payload slots. Hosts can inspect reason tags, payload slots, and
rendered payload fields without knowing compiler internals.

## Active tasks

See [Runtime memory tasks](../tasks/12_runtime_memory_and_semantics.md).

[runtime]: ../development/architecture/runtime_representation.md
[wasm]: ../development/architecture/wasm_backend_and_runtime.md
