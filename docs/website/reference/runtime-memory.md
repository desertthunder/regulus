# Runtime memory

Regulus stores managed values in WebAssembly linear memory. Dynamic allocation
uses a bump pointer, aligns object sizes, and grows linear memory when the
current page count is not large enough for the next allocation.

## Memory growth

`memory.grow` is standard Wasm behavior. It returns the old page count or `-1`
on failure, and Wasm pages are currently 64KiB.[^wasm-grow] Browser
`WebAssembly.Memory.grow()` is widely available, but growing detaches existing
JavaScript `ArrayBuffer` views, so host adapters must reacquire memory views
after calls.[^js-grow] Wasmtime also supports growth, but host memory can
relocate and growth can fail because of maximum limits, resource limiters, or
OOM.[^wasmtime-grow]

Regulus allows dynamic memory growth by default during the current bump
allocator phase, bounded by an explicit maximum page count. This keeps examples
and longer-running tests usable before freeing, arena reset, reference counting,
or garbage collection exist. Future targets may choose a fixed-memory policy,
but dynamic growth remains the default capacity strategy for the general
runtime.

## Host pointers

Managed pointers are guest-memory offsets. Hosts may inspect values through
documented adapters or exported helpers, but must not mutate runtime object
memory.

Host-held managed pointers remain valid while all of these are true:

- the WebAssembly instance is alive
- the pointer came from the same instance
- the value has not been invalidated by arena reset
- the host treats the pointer as borrowed and read-only

Current runtime objects are non-moving, so memory growth does not change the
numeric pointer value. It can still invalidate host-side memory views. Hosts
that need to keep a value should keep the numeric pointer and reacquire memory
views before reading.

Hosts must not cache JavaScript typed-array views across calls that may allocate
or grow memory. Wasmtime hosts must also avoid retaining raw host pointers or
unsafe slices across growth, because the underlying memory can relocate.

Host-provided managed pointers are borrowed pointers into the same guest
memory. A host must only pass a managed pointer that was returned by the same
instance or created through that instance's exported adapter helpers. Hosts must
not synthesize pointers, pass pointers from another instance, pass pointers
after instance reset, or pass pointers to memory they mutated directly.

Opaque host handles are runtime objects that contain a type tag and an adapter
handle-table id. The adapter owns the JavaScript value behind the id. The guest
runtime only owns the small opaque wrapper object in linear memory; it does not
own, free, or inspect the JavaScript value. Clearing or replacing the adapter
handle table invalidates handle ids even if an old opaque wrapper pointer still
exists.

JavaScript host ABI validation rejects ownership-ambiguous managed imports.
Imports may receive scalars, strings, or opaque handles. Structured managed
values are supported as exported return values through reader helpers, but not
as JS host import parameters until writer and ownership rules are explicit.

## Host reader failures

JavaScript host targets export low-level reader helpers for strings, managed
values, and opaque handles. These helpers validate runtime object headers before
reading guest memory.

The following failures trap:

- non-zero pointers that do not fit in memory
- unknown runtime object tags
- object payloads whose declared size extends past memory
- string readers called for non-string objects
- handle readers called for non-opaque objects
- field readers called for strings, bit arrays, closures, or opaque objects
- field indexes greater than or equal to the object arity

Only these reader results are sentinels instead of traps:

- `__regulus_value_tag(0)` returns `0` for the nil-list/null pointer.
- `__regulus_value_constructor(ptr)` returns `0` for valid objects that do not
  carry a constructor or reason tag.
- `__regulus_value_arity(ptr)` returns `0` for valid strings and bit arrays.

All other malformed helper calls should be treated as caller bugs. Hosts should
use the generated adapter layer when possible instead of calling raw readers
directly.

[^wasm-grow]: MDN, [`memory.grow`](https://developer.mozilla.org/en-US/docs/WebAssembly/Reference/Memory/grow).
[^js-grow]: MDN, [`WebAssembly.Memory.prototype.grow()`](https://developer.mozilla.org/en-US/docs/WebAssembly/Reference/JavaScript_interface/Memory/grow).
[^wasmtime-grow]: Wasmtime Rust API, [`Memory::grow`](https://docs.wasmtime.dev/api/wasmtime/struct.Memory.html#method.grow).
