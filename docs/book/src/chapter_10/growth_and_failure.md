# Growth and allocation failure

WebAssembly memory is measured in pages. One page is 64 KiB, and `memory.grow`
asks the engine to add more pages to a memory instance. On success it returns
the previous page count. On failure it returns `-1`.[^memory-grow]

Regulus treats growth as part of allocation, not as a separate operation that
callers perform by hand. Every runtime helper that constructs a managed value
uses `__alloc` or calls another helper that uses it.

## Checked growth

`__alloc` first computes the aligned allocation end. If the end fits in current
memory, no growth is needed. If it does not fit, the helper computes how many
additional pages are required and calls `memory.grow`.

That check has to cover arithmetic before the final store. A requested size can
overflow when aligned or added to the heap pointer. The required page count can
overflow before `memory.grow` receives it. Those cases are allocation failure.
The runtime must not wrap around and overwrite earlier objects.

After a successful growth, the old pointer values remain valid as byte offsets.
The module's memory has more bytes, but the objects already written keep the
same offsets. That gives Regulus stable managed pointers while still allowing
large strings, lists, or nested values to grow beyond the initial page.

## Structured failure

Allocation failure produces a runtime panic payload. The prelude records it in
`__last_panic`, then traps.

The payload uses the same tag-10 panic object family described in chapter 6:

```text
tag:        10
reason:     1
payload 0: requested allocation size
payload 1: heap pointer before allocation
```

This keeps failed growth inspectable. A Wasmtime test can call an exported
function, observe the trap, then read `__last_panic` to confirm that the failure
came from allocation and that the runtime recorded the request that failed.

Direct trapping would be simpler, but it would lose information. A compiler
runtime benefits from failures that are easy to classify, especially while the
object layout and helper set are still growing.

[^memory-grow]: MDN, "`memory.grow`": https://developer.mozilla.org/en-US/docs/WebAssembly/Reference/Memory/grow
