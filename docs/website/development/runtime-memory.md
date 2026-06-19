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
- Objects are non-moving until the Wasm instance is reset or explicit arena
  reset runs.

This keeps allocation small and makes raw `i32` managed-value pointers stable
for generated code and Wasmtime tests.

## Helper inventory

Runtime helpers are grouped by use:

- allocation: `__alloc`, `__allocation_fail`, `__last_panic`,
  `__arena_mark`, and `__arena_reset`
- managed values: tuple, record, custom, closure, opaque, option, order, error,
  and panic constructors, plus raw field readers
- closures: allocation and indirect-call capture layout helpers
- equality and ordering: structural equality and comparison for runtime values
- debug: debug tags, panic/error reasons, and payload readers
- dynamic values: constructors, classifiers, field readers, decoder
  constructors, and decoder runners
- host adapters: JS exports for allocation, strings, managed value tags,
  arity, constructors, fields, and opaque handle readers

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

Runtime allocation tests cover:

- growth without moving existing managed objects
- page-boundary allocations
- exact heap-limit allocations
- deterministic failure before the configured heap limit
- deterministic failure when `memory.grow` cannot grow
- structured panic payloads for allocation failure
- arena reset reuse of dynamic allocations after a mark
- rejection of invalid arena reset marks

## Host ownership

Managed pointers exported to a host are borrowed. The guest runtime owns the
object. A borrowed pointer remains stable after the exporting call returns, but
the host must not retain it across Wasm instance reset or a future arena reset.

Browser adapters must refresh typed array views after memory growth. Wasmtime
tests can read the exported memory directly after each call.

Host-provided managed pointers must come from the same guest instance or from
that instance's exported adapter helpers. Hosts must not synthesize pointers,
reuse pointers across instances or resets, or pass pointers to directly mutated
runtime memory.

Opaque host handles split ownership. The guest runtime owns only the opaque
wrapper object containing a type tag and adapter handle id. The adapter owns the
host value behind that id. Clearing or replacing the adapter handle table
invalidates handle ids even if old wrapper pointers still exist.

JS host ABI validation rejects ownership-ambiguous managed imports. Imports may
receive scalars, strings, or opaque handles. Structured managed values may be
returned from exports through reader helpers, but they are not accepted as JS
host import parameters until writer and ownership rules are explicit.

## Host reader validation

Exported JS adapter reader helpers validate object headers before reading.

These failures trap:

- non-zero pointers outside memory
- unknown runtime object tags
- object payloads whose declared size extends past memory
- string readers called for non-string objects
- handle readers called for non-opaque objects
- field readers called for strings, bit arrays, closures, or opaque objects
- field indexes greater than or equal to the object arity

Only these reader results are sentinel values:

- `__regulus_value_tag(0)` returns `0` for nil-list/null.
- `__regulus_value_constructor(ptr)` returns `0` for valid non-constructor
  objects.
- `__regulus_value_arity(ptr)` returns `0` for valid strings and bit arrays.

All other malformed helper calls are caller bugs.

## Arena reset reclamation

Arena reset is the selected reclamation strategy. It is the smallest step
beyond the bump allocator: save a heap mark before a bounded scope, then reset
`$__heap` to that mark when the scope ends.

`__arena_mark() -> i32` returns the current heap pointer. `__arena_reset(mark)`
sets `$__heap` back to a previous mark. Reset traps if the mark is before the
dynamic heap start, after the current heap, or not 8-byte aligned.

Reset invalidates every dynamic object allocated after the mark. Static data and
dynamic objects allocated before the mark remain valid. Later allocations may
reuse reset-owned memory.

Generated JavaScript adapters wrap exported Gleam calls in an arena scope. The
adapter marks before encoding JS arguments, calls the Wasm export, decodes the
return into JS-owned data, and resets in a `finally` block. Raw Wasm and
Wasmtime exports are not automatically reset because those callers may inspect
borrowed managed pointers after the call.

The CLI `run` command is ABI-aware for managed returns. When arena helpers are
available, it marks before calling an export, decodes the result for display,
and resets before printing. String returns are printed as text. Other managed
returns use the runtime debug renderer.

Compiler-generated code must not return or retain pointers allocated after a
mark that will be reset. General internal reset scopes still require escape
analysis or region tracking.

Reference counting is not the selected strategy for this milestone. It would
require generated retain/release operations for every managed assignment, field
store, capture, return, and host boundary. It would also need handle-table
integration and a cycle policy.

Tracing garbage collection is also not selected. Wasm does not expose the
operand stack or locals to the runtime, so Regulus would need an explicit
shadow stack, stack maps, or generated root-registration code before a tracing
collector can find live managed values.

See also:

- [Runtime representation](./runtime-representation.md)
- [WASM backend and runtime](./wasm-backend-and-runtime.md)
