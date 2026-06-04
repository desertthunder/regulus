# Runtime value representation

WebAssembly gives a compiler numeric values, functions, tables, and linear
memory. A source language such as Gleam has richer values: strings, lists,
tuples, records, custom types, and closures. Runtime representation is the set of
choices that explain how those values live in WebAssembly memory and how code
passes them around.

Scalar values can often map directly to WebAssembly value types. Managed values
need layout rules, allocation, and an ABI between generated WebAssembly and the
host runtime.

## Runtime ABI

An ABI, or application binary interface, is the rulebook for how values cross a
compiled-code boundary. For this compiler, the boundary is usually between a
Gleam function compiled to WebAssembly and host code such as a Wasmtime test or a
browser JavaScript caller.

Scalar values cross that boundary as ordinary WebAssembly values:

| Gleam type | ABI value |
| ---------- | --------- |
| `Int`      | `i64`     |
| `Float`    | `f64`     |
| `Bool`     | `i32`     |
| `Nil`      | no value  |

Managed values cross as `i32` pointers into WebAssembly linear memory. A pointer
is a byte offset where the runtime object starts. The object begins with an
8-byte header:

```text
0..4  tag:  object kind
4..8  size: length, arity, or field count
8..   payload bytes or fields
```

For example, a function that returns a tuple gives the host an `i32`, not the
tuple fields directly. The host reads memory at that pointer, checks the tag to
see that it is a tuple, reads the arity from the second word, and then reads the
payload fields. Strings, bit arrays, lists, records, custom values, and closures
use the same pointer-based idea with different tags and payload layouts.

This convention keeps WebAssembly function signatures small, but it means both
sides must agree on layout, ownership, and when a wrapper is needed. A raw
pointer is enough for generated WebAssembly to pass managed values around; a
human-facing or JavaScript-facing API may still wrap that pointer in a safer
shape.

<!--
TODO (research):
  - Why runtime representation exists
      - WebAssembly has numbers and memory, not Gleam strings/lists/records directly
      - A compiler must choose how language values live in WASM
  - WebAssembly linear memory
      - memory as a byte array
      - pointers as `i32` offsets
      - alignment
      - reading/writing fields with loads and stores
  - Immediate vs managed values
      - `Int`, `Float`, `Bool`, `Nil`
      - heap-managed values like `String`, `List`, tuple, record, custom type
  - Object headers
      - tags
      - sizes
      - arity or constructor IDs
      - why headers make runtime inspection possible
  - Allocation
      - bump allocator as a first allocator
      - heap pointer global
      - no-free model
      - why GC or reference counting comes later
  - String representation
      - UTF-8 bytes
      - length
      - padding/alignment
      - passing strings across host boundaries
  - Lists
      - empty list representation
      - cons cell layout
      - head/tail fields
      - tradeoff between linked lists and arrays
  - Tuples and records
      - fixed-size field layout
      - arity
      - record field order
      - named fields at compile time vs positional fields at runtime
  - Custom types and constructors
      - constructor tags
      - payload fields
      - how pattern matching inspects tags
  - Closures and function values
      - function pointer/table index
      - captured environment pointer
      - why closures need two pieces of data
  - Host ABI
      - how values cross between WASM and Wasmtime/browser
      - scalar values as WASM numbers
      - managed values as pointers
      - ownership questions
  - Testing layouts
      - Wasmtime memory inspection
      - golden layout tests
      - checking tags, lengths, fields, and pointers
  - Tradeoffs
      - simple vs efficient layouts
      - boxed vs unboxed values
      - portability
      - future GC/runtime choices
-->
