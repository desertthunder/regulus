# WebAssembly code generation

WebAssembly, or WASM, is a portable binary instruction format for executable
programs.[^1] It is designed to run in browsers and in standalone runtimes such
as Wasmtime.[^2] A WebAssembly program is stored in a module. A module can define
functions, locals, imports, exports, memory, tables, and other runtime items.

This compiler emits WebAssembly from core IR. For the scalar subset, the job is
small enough to see directly:

```gleam
pub fn id(x: Int) -> Int {
  x
}
```

can become WAT like this:

```wat
(module
  (func $id (export "id") (param $0 i64) (result i64)
    local.get $0
  )
)
```

WAT is WebAssembly's text format. It is easier for people to read than binary
`.wasm`, and it can be assembled into the binary format that runtimes execute.
The OpenCS WAT material gives small examples of this text format and how it maps
to WebAssembly modules.[^3]

## A stack machine

WebAssembly is built around a stack-machine execution model. A stack machine
uses a stack for intermediate values: instructions push values onto the stack,
and later instructions pop values off.[^4]

This WAT function returns the integer `42`:

```wat
(func $answer (result i64)
  i64.const 42
)
```

The instruction `i64.const 42` pushes an `i64` value onto the stack. Since the
function result type is `i64`, that value becomes the function result.

A function call follows the same idea. Arguments are pushed first, then the call
instruction consumes them:

```wat
(func $id (param $0 i64) (result i64)
  local.get $0
)

(func $main (result i64)
  i64.const 1
  call $id
)
```

`i64.const 1` pushes the argument. `call $id` consumes it and pushes the return
value. Koopman's stack computer notes describe this style of evaluation in terms
of values moving through a data stack rather than through named temporary
registers.[^5]

## Types

WebAssembly has a small set of numeric value types. The compiler maps Gleam's
scalar types onto those value types:

| Gleam type | WebAssembly type |
| ---------- | ---------------- |
| `Int`      | `i64`            |
| `Float`    | `f64`            |
| `Bool`     | `i32`            |
| `Nil`      | no result value  |

`Bool` uses `i32` because WebAssembly does not have a separate boolean value
type. `False` is emitted as `0`, and `True` is emitted as `1`.

Strings are different. A string is not just one WebAssembly number. It needs a
memory representation: bytes in linear memory, plus a convention for length,
ownership, allocation, and passing values between functions. This compiler
rejects string-valued WASM output until that runtime representation exists.

## Functions, parameters, and locals

A WebAssembly function declares its parameters and result:

```wat
(func $id (param $0 i64) (result i64)
  local.get $0
)
```

The CodeRunDebug WAT guide shows this same shape: parameters are declared with
`param`, and return values are declared with `result`.[^6]

Core IR locals map to WebAssembly locals. A Gleam function like this:

```gleam
pub fn main(x: Int) -> Int {
  let y = x
  y
}
```

can be emitted as:

```wat
(module
  (func $main (export "main") (param $0 i64) (result i64)
    (local $1 i64)
    local.get $0
    local.set $1
    local.get $1
  )
)
```

The parameter `x` is local `$0`. The `let` binding `y` is local `$1`. The value
of `x` is pushed onto the stack with `local.get $0`, then stored into `y` with
`local.set $1`. The final `local.get $1` leaves the return value on the stack.

## Exports

A WebAssembly module can hide functions or export them. Exported functions are
visible to the host runtime. In this compiler, public Gleam functions with
supported scalar signatures are exported:

```gleam
pub fn id(x: Int) -> Int {
  x
}
```

emits an export:

```wat
(func $id (export "id") (param $0 i64) (result i64)
  local.get $0
)
```

A host can then load the module and call the exported function.

## Running with Wasmtime

Wasmtime is a standalone WebAssembly runtime from the Bytecode Alliance.[^2] It
can load a `.wasm` module, instantiate it, find an export, and call it from Rust.

A test for the `id` function follows this pattern:

```rust
let engine = wasmtime::Engine::default();
let module = wasmtime::Module::new(&engine, wasm_bytes)?;
let mut store = wasmtime::Store::new(&engine, ());
let instance = wasmtime::Instance::new(&mut store, &module, &[])?;
let id = instance.get_typed_func::<i64, i64>(&mut store, "id")?;

assert_eq!(id.call(&mut store, 42)?, 42);
```

That gives the compiler an end-to-end check: source code becomes IR, IR becomes
WAT, WAT becomes `.wasm`, and Wasmtime executes the exported function.

## What this compiler emits today

The WebAssembly backend currently handles:

- modules
- functions
- scalar function signatures
- local declarations
- `Int`, `Float`, and `Bool` constants
- `Nil` functions with no result value
- local reads and writes
- direct function calls
- exports for public scalar functions
- WAT generation
- binary `.wasm` assembly from WAT
- Wasmtime execution tests for exported scalar functions

It rejects runtime-managed values such as strings.

[^1]: WebAssembly Core Specification: https://webassembly.github.io/spec/core/

[^2]: Wasmtime documentation: https://docs.wasmtime.dev/

[^3]: Aalto University OpenCS, "Example: WAT and WASM": https://opencs.aalto.fi/en/courses/modern-and-emerging-programming-languages/part-7/7-example-wat-and-wasm

[^4]: Wikipedia, "Stack machine": https://en.wikipedia.org/wiki/Stack_machine

[^5]: Philip J. Koopman, "Stack Computers: Chapter 6": https://users.ece.cmu.edu/~koopman/stack_computers/chap6.html

[^6]: CodeRunDebug, "WAT Functions: Parameters and Results": https://coderundebug.com/learn/wat/functions/#parameters-and-results
