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
