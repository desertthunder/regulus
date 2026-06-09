# Host pointers and reset boundaries

Managed values cross the raw host boundary as borrowed `i32` pointers into
WebAssembly linear memory. The guest owns the object. The host may read it using
the exported memory and the documented object layout.

For a Wasmtime test, the pattern is direct:

1. Call an exported function.
2. Receive an `i32` pointer.
3. Read the exported memory at that offset.
4. Check the object tag, size word, and payload.

The pointer remains stable after the call returns. Later allocations and memory
growth do not move the object.

## Borrowed, not owned

A borrowed pointer is valid until a reset boundary. Today the main reset
boundary is the Wasm instance itself: create a new instance, get a new heap.
Future explicit arena reset points will also be reset boundaries.

Hosts must not treat guest pointers as durable handles. They are offsets into a
particular memory instance at a particular runtime epoch. If a browser adapter
wants a JavaScript string that outlives the call, it should copy the bytes out
of guest memory. If it wants to pass a value back into the guest later, it
should use an adapter with documented ownership rules.

## Browser memory views

Browser hosts have one extra hazard. JavaScript reads WebAssembly memory through
an `ArrayBuffer` and typed array views. When memory grows, existing views may no
longer be the right view of the current memory buffer.[^js-grow] Browser
adapters should refresh their typed arrays after calls that can allocate.

This does not change guest pointer stability. The `i32` offset is still the
same offset. The host-side view used to read that offset may need to be rebuilt.

## Why this boundary is small

The raw ABI does not try to expose Gleam values as rich host objects. It exposes
scalars as WebAssembly values and managed values as pointers. Higher-level
adapters can decode strings, lists, records, and custom values for a particular
host. Keeping the raw boundary small lets tests exercise the same representation
that generated code uses internally.

[^js-grow]: MDN, "`WebAssembly.Memory.prototype.grow()`": https://developer.mozilla.org/en-US/docs/WebAssembly/Reference/JavaScript_interface/Memory/grow
