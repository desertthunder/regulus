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

| Module | Initial support |
| --- | --- |
| `gleam/io` | `println`, `print`, maybe `debug` |
| `gleam/int` | `to_string`, later `parse` |
| `gleam/string` | `append`, `concat`, `length`, `is_empty` |
| `gleam/list` | `length`, `reverse`, later `map` and `fold` |
| `gleam/result` | `Result`, `Ok`, `Error`, maybe `map` |
| `gleam/option` | `Option`, `Some`, `None`, maybe `map` |
| `gleam/order` | `Order`, `Lt`, `Eq`, `Gt` |

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

Browser and WASI adapters should be concrete, not just target names. Each target
should document required imports, exports, memory access rules, and unsupported
combinations.

## Diagnostics

Unsupported stdlib modules, dependency modules, host calls, ABI shapes, or
target combinations should produce clear diagnostics rather than failing during
WASM assembly.

## Active tasks

See [Stdlib and host interop tasks](../tasks/14_stdlib_and_host_interop.md).
