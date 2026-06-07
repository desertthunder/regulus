# Open runtime memory and value semantics

Current runtime layout and helper behavior are documented in
[Runtime representation](../development/runtime_representation.md) and
[WASM backend and runtime](../development/wasm_backend_and_runtime.md). This
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

Runtime equality should match Gleam semantics for all comparable values.
Managed values need recursive structural equality with cycle-safe or acyclic
guarantees.

Ordering helpers should support every type Gleam allows to be ordered and should
reject unsupported ordering before code generation when possible.

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
