# Open standard library and host interop

The backend has a raw scalar and managed-pointer ABI for current tests. This
spec tracks the work needed for useful standard library modules and concrete
host interfaces for Wasmtime, WASI, and the browser.

## Strategies

Possible standard library strategies include:

- compile Gleam standard library modules to WASM
- provide selected modules as host imports
- implement selected modules as runtime intrinsics
- combine compiled modules with host shims

The strategy can vary by module, but it should be explicit and tested.

Regulus should support the stdlib in two groups.

### Group 1: initial useful stdlib

Group 1 is the small stdlib surface needed for examples and tests. These
modules should be visible to resolver and type checker through the same module
interface path as project modules. Members that are not implemented yet should
still fail with source-spanned unsupported diagnostics, not during WASM
assembly.

The group contains:

- `gleam/io`
- `gleam/int`
- `gleam/string`
- `gleam/list`
- `gleam/result`
- `gleam/option`
- `gleam/order`

Initial implementations should cover:

| Module         | Initial support                             |
| -------------- | ------------------------------------------- |
| `gleam/io`     | `println`, `print`, maybe `debug`           |
| `gleam/int`    | `to_string`, later `parse`                  |
| `gleam/string` | `append`, `concat`, `length`, `is_empty`    |
| `gleam/list`   | `length`, `reverse`, later `map` and `fold` |
| `gleam/result` | `Result`, `Ok`, `Error`, maybe `map`        |
| `gleam/option` | `Option`, `Some`, `None`, maybe `map`       |
| `gleam/order`  | `Order`, `Lt`, `Eq`, `Gt`                   |

Each implemented member should declare one lowering strategy: intrinsic, host
import, compiled Gleam source, or adapter around a host import.

### Group 2: remaining stdlib

Group 2 is the rest of the Gleam stdlib. It should be finished after Group 1,
the host ABI, and the target adapters are stable.

The remaining modules are:

- `gleam/bit_array`
- `gleam/bool`
- `gleam/bytes_tree`
- `gleam/dict`
- `gleam/dynamic`
- `gleam/dynamic/decode`
- `gleam/float`
- `gleam/function`
- `gleam/pair`
- `gleam/set`
- `gleam/string_tree`
- `gleam/uri`

Group 2 should prefer compiling stdlib Gleam source where possible. Runtime
intrinsics and host adapters should be reserved for functions that require
special ABI support, target capabilities, or efficient runtime primitives.

## Host ABI

The host ABI should define how values cross the WASM boundary:

- scalar values
- strings
- lists and tuples
- custom types
- functions and closures
- errors and panics
- memory ownership
- arbitrary managed-value import/export wrappers

Concrete host ABI for Group 1 uses raw WASM values plus runtime adapters:

| Gleam shape    | WASM shape | Host rule                          |
| -------------- | ---------- | ---------------------------------- |
| `Int`          | `i64`      | signed 64-bit integer              |
| `Float`        | `f64`      | IEEE-754 double                    |
| `Bool`         | `i32`      | `0` false, non-zero true           |
| `Nil`          | no result  | unit value                         |
| managed values | `i32`      | borrowed pointer into guest memory |

Managed values include strings, bit arrays, lists, tuples, records, custom
values, functions, errors, and panics. The guest runtime owns these values.
Hosts may read them during and after a call while the instance is alive, but
must not mutate object memory or retain pointers across instance reset or any
future arena reset.

The runtime exports adapter helpers for low-level hosts:

| Export                                  | Purpose                              |
| --------------------------------------- | ------------------------------------ |
| `memory`                                | guest linear memory                  |
| `__regulus_string_len(ptr)`             | byte length for a string pointer     |
| `__regulus_string_data(ptr)`            | data address for a string pointer    |
| `__regulus_value_tag(ptr)`              | runtime object tag, `0` for nil/null |
| `__regulus_value_size(ptr)`             | runtime object size/arity/bit length |
| `__regulus_value_field_i64(ptr, index)` | raw managed field slot               |
| `__regulus_value_field_i32(ptr, index)` | raw field slot narrowed to `i32`     |

Compiler code receives host-provided managed values as borrowed `i32` pointers.
The host must only pass pointers to values allocated in the same guest memory
or adapter functions that explicitly document another ownership rule.

Current stdlib host imports are:

| Gleam member       | Wasmtime import    | Browser import         | WASI        |
| ------------------ | ------------------ | ---------------------- | ----------- |
| `gleam/io.print`   | `env.print(ptr)`   | `browser.print(ptr)`   | unsupported |
| `gleam/io.println` | `env.println(ptr)` | `browser.println(ptr)` | unsupported |

WASI `gleam/io` is deliberately unsupported until a concrete `fd_write`
adapter is added. Unsupported host calls, target combinations, and ABI shapes
produce source-spanned diagnostics before WAT assembly.

## External functions

General Gleam external functions should lower to Wasm imports, not only current
stdlib shims. The compiler should preserve the declared external module and
function name, validate that the selected target accepts that module, and reject
unsupported ABI shapes before byte emission.

Browser examples need imports for fetch, local storage, and browser state.
Worker examples need imports or exported wrappers for request routing and
response construction. These should be ordinary target-specific external
functions with documented adapters, not special cases in user code.

## Browser and Worker adapters

The low-level managed-pointer ABI is useful for tests, but browser and Worker
hosts need stable helpers for common shapes:

- strings into and out of guest memory
- lists of strings or records
- result and option values
- small response records with status, headers, and body
- opaque handles only when string or record adapters are not enough

Cloudflare Workers may use the browser target initially, but the compiler needs
an explicit decision: either add a `workers` target or document Workers as a
browser-host profile with a distinct adapter contract.

## Decoding and structured data

The weather example needs a small JSON decoding path for NWS responses. The
Wisp reference API needs structured JSON output. This can come from selected
`gleam/dynamic` and `gleam/dynamic/decode` support, a small compiler-provided
JSON bridge, or a deliberate choice to keep JSON parsing in the host for the
first example milestone.

Whichever strategy is chosen must be explicit. Unsupported decoders,
dependency modules, and structured response shapes should fail with
source-spanned diagnostics.

## Diagnostics

Unsupported stdlib modules, dependency modules, host calls, ABI shapes, or
target combinations should produce clear diagnostics rather than failing during
WASM assembly.

## Active tasks

See [Stdlib and host interop tasks](../tasks/14_stdlib_and_host_interop.md).
