# JavaScript host ABI contract

Regulus targets browser-capable WebAssembly. Raw Wasm only passes scalar values,
so JavaScript hosts need a stable contract for strings, managed values, imports,
exports, and glue.

This page defines the first JS host ABI. Browser, bundler, and Node.js profiles
share these value rules. Profiles only change loading behavior, available host
APIs, and accepted import module names.

## Ownership model

Scalar values cross the boundary as raw Wasm values. Managed Gleam values cross
as borrowed `i32` pointers into guest memory.

JavaScript hosts may read managed values through exported Regulus runtime
helpers. Hosts must not mutate guest object memory. Hosts must not keep managed
pointers after a Wasm instance is discarded, reset, or after any future arena
reset point.

## Value shapes

| Gleam shape      | Wasm ABI                       | JavaScript contract                    |
| ---------------- | ------------------------------ | -------------------------------------- |
| `Int`            | `i64`                          | JS `BigInt`,                           |
| `Float`          | `f64`                          | JS `number`.                           |
| `Bool`           | `i32`                          | `0` is `false`; `1` is `true`.         |
| `Nil` return     | no result                      | JS `undefined`.                        |
| `String`         | managed `i32` pointer          | Read and write through string helpers. |
| tuples           | managed `i32` pointer          | Planned                                |
| records          | managed `i32` pointer          | Planned                                |
| custom types     | managed `i32` pointer          | Planned                                |
| lists            | managed `i32` pointer          | Planned                                |
| opaque externals | managed or host handle pointer | Deferred to the opaque-handle ABI.     |
| functions        | none                           | Unsupported across the JS host ABI.    |

The first stable ergonomic conversion layer is scalar and string focused.
Managed structured values are represented as borrowed pointers until the reader
helper contract is complete.

## Runtime helpers

JS host builds export stable string helpers:

- `__regulus_alloc(size: i32) -> i32`
- `__regulus_string_new(data: i32, len: i32) -> i32`
- `__regulus_string_len(ptr: i32) -> i32`
- `__regulus_string_data(ptr: i32) -> i32`

Glue writes a JS string by UTF-8 encoding it, allocating guest bytes, copying
those bytes into memory, and calling `__regulus_string_new`. Glue reads a Gleam
string by calling the length and data helpers, then UTF-8 decoding the byte
range.

Future helpers will expose managed value tags, arity, and fields for structured
values.

## Import modules

The shared JS import namespace is `regulus/js`. Profile-specific modules are
reserved for host APIs.

| JS host profile | Accepted import modules |
| --------------- | ----------------------- |
| `bundler`       | `regulus/js`            |
| `browser`       | `regulus/js`, `browser` |
| `nodejs`        | `regulus/js`, `nodejs`  |

The `browser` module is reserved for browser APIs such as fetch, local storage,
time, and online state. The `nodejs` module is reserved for Node-specific APIs.
The compiler does not implement those APIs; host glue provides them.

Non-JS targets use different modules and are outside this contract. Wasmtime
uses `env`, and WASI uses `wasi_snapshot_preview1`.

## Imported functions

A JavaScript host import is a Gleam `external fn` whose module is accepted by
the selected JS profile.

Supported imported parameter shapes are:

- `Int`
- `Float`
- `Bool`
- `String`

Supported imported return shapes are:

- `Int`
- `Float`
- `Bool`
- `String`
- `Nil`

Structured managed values may lower as borrowed pointers internally, but stable
JS conversion is not part of this milestone. Opaque types, generic values, and
function values are unsupported across JS host imports until their ABI contracts
are defined.

## Exported functions

A public Gleam function exported to a JS host should use the same first-stable
shape set as imports.

Supported exported parameter shapes are:

- `Int`
- `Float`
- `Bool`
- `String`

Supported exported return shapes are:

- `Int`
- `Float`
- `Bool`
- `String`
- `Nil`

Glue should expose checked wrappers for these shapes so application code does
not perform pointer arithmetic.

Public functions that return records, lists, tuples, custom types, `Result`, or
`Option` are deferred until managed reader helpers are stable. Public functions
that accept or return opaque handles are deferred until the opaque-handle ABI is
stable.

## Diagnostics

Unsupported JS host ABI shapes should fail before Wasm emission with
source-spanned diagnostics.

Diagnostics should cover:

- unsupported profile names
- unsupported import modules for the selected profile
- unsupported imported parameter or return shapes
- unsupported exported parameter or return shapes
- opaque-handle use before the handle ABI is defined
- function or closure values crossing the JS boundary

Diagnostics should point at the external module string, parameter type, return
type, or public function annotation that caused the unsupported shape.

## Deferred contracts

This contract intentionally does not define:

- structured managed value readers
- `Result` and `Option` conversion
- opaque JS handle representation and lifetime
- browser API semantics
- Node.js loading semantics
- generated binding metadata

Those pieces build on the scalar, string, module-name, and validation contract
above.
