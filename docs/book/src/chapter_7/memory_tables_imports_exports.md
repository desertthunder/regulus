# WebAssembly memory, tables, imports, and exports

Functions and locals are enough for scalar examples. Real language runtimes need
more module machinery: memory for data, tables for indirect references, imports
for host or dependency functions, and exports for values the host may call or
inspect.

## Linear memory

Linear memory is a contiguous, mutable byte array. It is created with an initial
size and can grow dynamically. Loads and stores read or write at byte offsets,
and an out-of-bounds access traps.[^overview]

```wat
(memory (export "memory") 1)
```

Memory sizes are measured in WebAssembly pages. One page is 64 KiB.[^memory]
The example above defines one page and exports it under the name `memory`.

This matters because WebAssembly does not have built-in Gleam strings, lists,
tuples, records, or custom values. Regulus stores managed runtime objects in
linear memory and passes `i32` pointers to those objects. A string pointer is a
byte offset. So is a tuple pointer, a list pointer, and a custom-value pointer.

The current runtime prelude defines:

```wat
(memory (export "memory") 1)
(global $__heap (mut i32) (i32.const ...))
```

The heap global is a bump pointer. Runtime allocation returns the current heap
offset, advances it by the requested size, aligns the result, and leaves freeing
or garbage collection for a later runtime design.

## Data segments

Data segments initialize memory. The module can place static bytes at an offset
during instantiation:

```wat
(data (i32.const 1024) "\05\00\00\00hello")
```

Regulus uses data segments for static runtime objects such as string literals.
The exact bytes follow the runtime layout from chapter 6. The backend's job is
to place those bytes deterministically and emit pointers to their offsets.

## Tables

A table is an array of opaque references. The common compiler use is indirect
function calls: store function references in a table, then call through a
dynamic index.[^overview]

```wat
(table 2 funcref)
(elem (i32.const 0) $f $g)

(call_indirect (type $binary_i64)
  local.get $arg
  local.get $callee_index
)
```

For Gleam, tables become relevant for function values and closures. A closure is
not only a table index. It also needs an environment pointer for captured
values. A simple runtime representation can store both pieces in a heap object:
the function identity and the captured environment.

## Imports

Imports are definitions supplied by the host or by another module at
instantiation time. Each import has a module name, an item name, and an external
type. Importable definitions include functions, globals, memories, tables, and
tags.[^modules]

```wat
(import "gleam" "print_i64" (func $print_i64 (param i64)))
```

Imports are part of the module's index spaces. Function imports come before
module-defined functions in the function index space.[^modules] That is why a
backend must emit imports in a stable order before emitting functions that call
them.

Regulus supports host and module import boundaries in IR. The backend validates
that imported function parameter and result types have a raw WebAssembly ABI
before it emits an import declaration.

## Exports

Exports are definitions made visible to the host after instantiation. A module
can export functions, globals, memories, tables, and tags. Export names are
unique within a module.[^modules]

```wat
(func $id (export "id") (param $0 i64) (result i64)
  local.get $0
)
```

For Regulus, exported functions are the main executable boundary in Wasmtime
tests. Exported memory is the main inspection boundary for managed values. A
test can call a function that returns an `i32` pointer, then read the exported
memory at that pointer and check object tags, sizes, and payload fields.

## ABI rule of thumb

The raw core-Wasm ABI should stay boring:

- scalars cross as `i64`, `f64`, or `i32`
- `Nil` returns no WebAssembly value
- managed values cross as `i32` memory pointers
- host-specific adapters can wrap those raw values later

Keeping this boundary small makes Wasmtime tests direct and keeps browser
interop honest. Rich JavaScript objects, strings, or future component-model
types should be adapters over this lower-level contract, not hidden assumptions
inside ordinary core-Wasm functions.

[^overview]: WebAssembly Core Specification, "Overview": https://webassembly.github.io/spec/core/intro/overview.html

[^memory]: WebAssembly Core Specification, "Modules", memories: https://webassembly.github.io/spec/core/syntax/modules.html#memories

[^modules]: WebAssembly Core Specification, "Modules": https://webassembly.github.io/spec/core/syntax/modules.html
