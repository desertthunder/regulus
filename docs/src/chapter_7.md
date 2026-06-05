# WebAssembly code generation

WebAssembly is the compiler's executable target. The core specification defines
modules, value types, instructions, validation, execution, and the text and
binary formats without tying those rules to one host environment.[^spec]

This chapter explains the parts of WebAssembly that matter when lowering Gleam
core IR into a module:

- modules, functions, locals, and the operand stack
- the readable text format and compact binary format
- linear memory, tables, imports, and exports
- running generated modules in Wasmtime and the browser

Those constraints shape the backend. Generated code must validate before it can
run, so stack balance and value types are part of correctness, not just cleanup.
Host interaction must go through imports, exports, memories, tables, and the
embedding API, so the compiler needs an explicit ABI rather than assuming it can
pass Gleam values directly.[^pldi]

## The compiler boundary

Regulus emits WebAssembly from core IR, not directly from Gleam syntax. Earlier
phases have already resolved names, checked types, and made locals and calls
explicit. That leaves the backend with a narrower job:

```text
typed Gleam AST -> core IR -> WAT -> .wasm
```

For a small scalar function:

```gleam
pub fn id(x: Int) -> Int {
  x
}
```

the backend can emit:

```wat
(module
  (func $id (export "id") (param $0 i64) (result i64)
    local.get $0
  )
)
```

The generated WAT is assembled into binary WebAssembly with the `wat` crate.
Tests then load the bytes with Wasmtime. In Wasmtime's Rust API, a `Module` is
compiled code ready to instantiate, while an `Instance` is the runtime object
whose exports can be acquired and called.[^wasmtime]

## Current backend shape

The backend currently emits deterministic WAT and binary `.wasm` for the
supported IR subset. It handles functions, locals, scalar values, managed-value
pointers, simple runtime helpers, direct and imported calls, branch control
flow, pattern tests, exports, static data segments, and a linear memory prelude
when runtime-managed values are used.

The target ABI is intentionally small. Scalars map directly to WebAssembly
values:

| Gleam type | WebAssembly ABI value |
| ---------- | --------------------- |
| `Int`      | `i64`                 |
| `Float`    | `f64`                 |
| `Bool`     | `i32`                 |
| `Nil`      | no result value       |

Managed values such as strings, bit arrays, lists, tuples, records, custom
values, and functions cross the raw core-Wasm boundary as `i32` pointers into
linear memory. Chapter 6 describes the object layout used behind those pointers.

[^spec]: WebAssembly Core Specification: https://webassembly.github.io/spec/core/

[^wasmtime]: Wasmtime documentation: https://docs.wasmtime.dev/

[^pldi]: Andreas Haas et al., "Bringing the Web up to Speed with WebAssembly", PLDI 2017: https://research.google/pubs/bringing-the-web-up-to-speed-with-webassembly/
