# WebAssembly backend

The backend turns core IR into WebAssembly. Early milestones should prioritize
readable WAT and correctness in Wasmtime over compact binary output or browser
integration.

## Responsibilities

- Map core IR types to WebAssembly value types or runtime-managed references.
- Emit functions, locals, blocks, calls, and exports.
- Generate WAT for inspection and tests.
- Produce `.wasm` through an assembler or direct binary writer.
- Run smoke tests in Wasmtime.

## Initial type mapping

| Core type | WASM representation |
| --- | --- |
| Int | `i64` |
| Float | `f64` |
| Bool | `i32` with `0` false and `1` true |
| Nil | no value or sentinel, depending on context |
| String | runtime pointer/handle, initially unsupported or host-provided |

String, list, custom type, and closure representation should be designed before
large language features depend on them.

## Exports and entry points

For early tests, top-level public functions can be exported by name when their
signatures only use supported scalar types. A conventional `main` entry point can
be added later for executable modules.

## Invariants

- The backend receives only valid core IR.
- Generated WAT is deterministic for snapshot tests.
- Backend errors mention the core construct and original source span when
  available.
