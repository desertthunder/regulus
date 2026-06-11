# The arena allocator

Regulus currently allocates managed runtime values in a resettable bump arena.
An arena is the region of memory used by an allocator, and a bump allocator is
the simple version of allocation: keep a pointer to the next free byte, return
that pointer for each request, and move it forward.

The runtime prelude keeps that next-free offset in a mutable WebAssembly global:

```wat
(memory (export "memory") 1)
(global $__heap (mut i32) ...)
```

Static managed values, such as string literals, are emitted as data segments
near the beginning of memory. The dynamic heap starts after those bytes. When a
helper needs a string, tuple, list cons cell, record, custom value, closure, or
panic object, it calls `__alloc`.

## Allocation shape

An allocation request has three steps:

1. Round the requested object size up to the runtime alignment.
2. Check that the aligned end offset fits in current linear memory.
3. Return the old heap pointer and store the aligned end as the new heap
   pointer.

The runtime uses 8-byte alignment. That matches the object layout from chapter
6, where payload fields that can hold arbitrary Gleam values use 8-byte slots.
Alignment affects correctness as well as speed. A memory manager has to
allocate blocks with the alignment required by the objects stored
there.[^alignment]

The allocator does not keep a free list. It does not split blocks, merge
adjacent free space, or reclaim individual objects. Those techniques matter in
general-purpose allocators, but they add machinery that Regulus does not yet
need. The current runtime allocates many small immutable objects and resets the
whole instance between tests or executions.

## Non-moving objects

Objects allocated by the arena are non-moving. A string pointer returned from a
function keeps pointing to the same byte offset after later allocations. A tuple
field that points at a list keeps the same offset too.

This property matters because managed values cross the raw ABI as `i32`
pointers. Generated code stores those pointers in locals and object fields.
Wasmtime tests read them from exported functions. A moving collector would need
root tracking and pointer updates before it could preserve those guarantees.

[^alignment]: Memory Management Reference, "alignment": https://www.memorymanagement.org/glossary/a.html#alignment
