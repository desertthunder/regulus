# Standard library and host interop

A useful compiler needs a plan for Gleam's standard library and for host
functions supplied by Wasmtime, WASI, or the browser.

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
