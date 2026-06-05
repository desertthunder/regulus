# WebAssembly text format and binary format

WebAssembly has two standard representations that matter to a compiler writer:
the text format and the binary format.

The text format is usually called WAT. It is readable, S-expression based, and
useful in tests, examples, browser developer tools, and compiler
snapshots.[^mdn]

The binary format is the compact `.wasm` format that engines validate, compile,
instantiate, and execute.[^binary]

## Why emit WAT first

Regulus emits WAT first because it is easy to inspect:

```wat
(module
  (func $add_one (export "add_one") (param $0 i64) (result i64)
    local.get $0
    i64.const 1
    i64.add
  )
)
```

That is much easier to snapshot than raw bytes. A focused WAT test can show
exactly which function signature, local declaration, memory operation, or export
changed.

The backend then assembles WAT into `.wasm` bytes. MDN's WAT guide describes the
same workflow with `wat2wasm`: write a `.wat` file, assemble it, and run the
resulting `.wasm` file in a host.[^mdn]

## Text format shape

The text format uses nested forms:

```wat
(module
  (memory (export "memory") 1)
  (global $__heap (mut i32) (i32.const 1024))
  (func $__alloc (param $size i32) (result i32)
    ;; body omitted
  )
)
```

This mirrors the module structure without requiring the reader to know binary
section codes. Names such as `$__heap` and `$__alloc` are text-format helpers.
The binary format ultimately indexes definitions numerically.

## Binary format shape

The binary format is organized as bytes and sections. The core specification has
separate binary encodings for values, types, instructions, and modules.[^binary]
Module sections encode the same semantic pieces seen in WAT: types, imports,
functions, tables, memories, globals, exports, code, data, and related metadata.

The binary format is not a second language with different behavior. It is the
serialized form of the same validated WebAssembly module. The specification
describes three semantic phases:

- decoding turns binary bytes into an internal module representation
- validation checks types, indices, stack use, and safety rules
- execution instantiates the valid module and invokes exported functions

Those phases are useful when debugging compiler output. A bad byte stream fails
to decode. A well-formed but ill-typed module fails validation. A valid module
can still trap at runtime, for example on `unreachable` or an out-of-bounds
memory access.

## Determinism

Compiler tests depend on deterministic output. The same IR should produce the
same WAT and the same binary bytes unless the compiler intentionally changes
code generation.

Regulus keeps deterministic WAT by emitting module items in a stable order:
runtime prelude, imports, functions, then data segments. This also keeps
WebAssembly index assignment predictable, because imported functions occupy
function indices before module-defined functions.

[^mdn]: MDN, "Converting WebAssembly text format to binary": https://developer.mozilla.org/en-US/docs/WebAssembly/Guides/Text_format_to_Wasm

[^binary]: WebAssembly Core Specification, "Binary Format": https://webassembly.github.io/spec/core/binary/index.html
