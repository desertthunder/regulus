# WebAssembly modules and the stack machine

A WebAssembly program is packaged as a module. The core specification treats a
module as the unit of deployment, loading, and compilation. A module can contain
types, imports, globals, memories, tables, functions, data segments, element
segments, exports, and an optional start function.[^modules]

For a compiler backend, the module is the object being built. Regulus turns one
lowered IR module into one WebAssembly module:

```wat
(module
  (func $id (export "id") (param $0 i64) (result i64)
    local.get $0
  )
)
```

This module has one function and one export. Larger modules add memory, runtime
helpers, static data, imports, and more functions.

## Functions and locals

WebAssembly functions declare their parameter and result types. Parameters are
locals too: they live at the front of the function's local index space, followed
by explicitly declared local variables.[^modules]

```wat
(func $main (export "main") (param $0 i64) (result i64)
  (local $1 i64)
  local.get $0
  local.set $1
  local.get $1
)
```

In this example, `$0` is the parameter and `$1` is a mutable local. The backend
uses this model directly because core IR already assigns stable local IDs.

## Operand stack

WebAssembly execution is based on an operand stack. Instructions execute in
order. Simple instructions pop their operands from the stack and push their
results back.[^overview]

```wat
(func $answer (result i64)
  i64.const 42
)
```

`i64.const 42` pushes one `i64`. The function has one `i64` result, so the value
left on the stack becomes the return value.

Calls follow the same rule. Arguments are pushed first, and `call` consumes
them:

```wat
(func $id (param $0 i64) (result i64)
  local.get $0
)

(func $main (result i64)
  i64.const 1
  call $id
)
```

The specification notes that an implementation does not need to literally keep
an operand stack. It can compile stack positions into registers or machine
temporaries. The important contract for a compiler emitting Wasm is that
validation can prove the stack height and types at every instruction.[^overview]

## Structured control flow

WebAssembly has structured control flow: `block`, `loop`, `if`, `else`, `br`,
and `br_if` target well-nested constructs rather than arbitrary byte offsets.
This is different from many native assembly formats, but it is a good fit for a
compiler IR that already represents blocks and branch expressions.

```wat
(func $choose (param $x i64) (result i64)
  local.get $x
  i64.const 0
  i64.eq
  if (result i64)
    i64.const 1
  else
    i64.const 2
  end
)
```

Both arms leave one `i64` on the stack, matching the `if` result type and the
function result type.

## Validation as a backend guardrail

Before a module can execute, the host validates it. Validation checks that the
module is meaningful and safe, including type checking instruction sequences and
ensuring the operand stack is used consistently.[^overview]

That makes invalid stack code visible quickly. If a Regulus backend change emits
a function that declares `(result i64)` but leaves no value on the stack, WAT
assembly or WebAssembly validation fails before Wasmtime can call the export.

[^modules]: WebAssembly Core Specification, "Modules": https://webassembly.github.io/spec/core/syntax/modules.html

[^overview]: WebAssembly Core Specification, "Overview": https://webassembly.github.io/spec/core/intro/overview.html
