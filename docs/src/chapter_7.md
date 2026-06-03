# Runtime value representation

WebAssembly gives a compiler numeric values, functions, tables, and linear
memory. A source language such as Gleam has richer values: strings, lists,
tuples, records, custom types, and closures. Runtime representation is the set of
choices that explain how those values live in WebAssembly memory and how code
passes them around.

Scalar values can often map directly to WebAssembly value types. Managed values
need layout rules, allocation, and an ABI between generated WebAssembly and the
host runtime.

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
