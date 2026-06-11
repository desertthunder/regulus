# Runtime memory management

The compiler has enough information to type-check a tuple, but WebAssembly has
no tuple value. It has numbers, functions, tables, globals, and linear memory.
Regulus has to decide where managed values live, how generated code allocates
them, when those allocations can fail, and how long host code may read the
resulting pointers.

This chapter covers the current memory design:

- the resettable bump arena used by the runtime prelude
- checked growth with `memory.grow`
- panic payloads for allocation failure
- borrowed host pointers and reset boundaries
- the information needed before reference counting or tracing GC can replace
  the arena

The design is intentionally small. It supports deterministic tests,
short-running compiled programs, and proof of the runtime object layout. Longer
executions will need a collector or region reset plan.

## Why memory management is explicit

WebAssembly linear memory is a mutable byte array owned by a module instance.
The core specification describes a memory as raw bytes with an initial page
count and an optional maximum page count.[^wasm-memory] WebAssembly does not
know that a range of bytes is a Gleam string or a custom value. The compiler and
runtime prelude assign that meaning.

Managed Regulus values use the layout from chapter 6. The value crossing an
internal or host ABI boundary is usually an `i32` byte offset into linear
memory. Generated code can pass the offset around cheaply. Runtime helpers can
inspect the object header at that offset.

That choice turns memory management into part of the ABI. If allocation can
move objects, every saved pointer rule changes. If the host owns a pointer, the
guest cannot reset the arena without a transfer protocol. If allocation failure
traps without state, tests and adapters cannot report which allocation failed.

## Contents

- [The arena allocator](./chapter_10/arena_allocator.md)
- [Growth and allocation failure](./chapter_10/growth_and_failure.md)
- [Host pointers and reset boundaries](./chapter_10/host_pointers.md)
- [Future collectors](./chapter_10/collector_families.md)

[^wasm-memory]: WebAssembly Core Specification, "Modules", memories: https://webassembly.github.io/spec/core/syntax/modules.html#memories
