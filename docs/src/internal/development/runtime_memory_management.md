# Runtime memory management

Runtime memory management is implemented in the WebAssembly runtime prelude.
The prelude is emitted only for modules that need managed values or runtime
helpers.

## Current model

Regulus uses a resettable bump arena in linear memory.

- Static managed objects are emitted as data segments before the dynamic heap.
- `$__heap` is the next dynamic allocation offset.
- `__alloc` aligns each request to 8 bytes.
- Allocation advances the heap pointer and never frees individual objects.
- Objects are non-moving until the Wasm instance is reset or a future explicit
  arena reset runs.

This keeps allocation small and makes raw `i32` managed-value pointers stable
for generated code and Wasmtime tests.

## Growth and failure

Every allocator path must call `__alloc` or a helper that delegates to it.
`__alloc` checks whether the aligned allocation end fits in current linear
memory. If it does not fit, it computes the required page count and calls
`memory.grow`.

Allocation fails when:

- size arithmetic overflows,
- the required page count cannot be represented, or
- `memory.grow` returns `-1`.

Allocation failure records a tag-10 panic payload through `__last_panic`. The
payload reason tag is `1`; slot 0 is the requested allocation size and slot 1 is
the heap pointer before allocation.

## Host ownership

Managed pointers exported to a host are borrowed. The guest runtime owns the
object. A borrowed pointer remains stable after the exporting call returns, but
the host must not retain it across Wasm instance reset or a future arena reset.

Browser adapters must refresh typed array views after memory growth. Wasmtime
tests can read the exported memory directly after each call.

## Deferred work

Reference counting and tracing garbage collection are deferred. Either design
needs complete root metadata, stack and local tracking, clear host ownership
rules, and tests for pointer lifetime across host calls.

See also:

- [Runtime representation](./architecture/runtime_representation.md)
- [WASM backend and runtime](./architecture/wasm_backend_and_runtime.md)
