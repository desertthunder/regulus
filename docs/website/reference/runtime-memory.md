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
allocator phase. This keeps examples and longer-running tests usable before
freeing, arena reset, reference counting, or garbage collection exist. Future
targets may choose a fixed-memory policy, but dynamic growth remains the
default capacity strategy for the general runtime.

## Host pointers

Managed pointers are guest-memory offsets. Hosts may inspect values through
documented adapters or exported helpers, but must not mutate runtime object
memory.

Hosts must not cache JavaScript typed-array views across calls that may allocate
or grow memory. Wasmtime hosts must also avoid retaining raw host pointers or
unsafe slices across growth, because the underlying memory can relocate.

[^wasm-grow]: MDN, [`memory.grow`](https://developer.mozilla.org/en-US/docs/WebAssembly/Reference/Memory/grow).
[^js-grow]: MDN, [`WebAssembly.Memory.prototype.grow()`](https://developer.mozilla.org/en-US/docs/WebAssembly/Reference/JavaScript_interface/Memory/grow).
[^wasmtime-grow]: Wasmtime Rust API, [`Memory::grow`](https://docs.wasmtime.dev/api/wasmtime/struct.Memory.html#method.grow).
