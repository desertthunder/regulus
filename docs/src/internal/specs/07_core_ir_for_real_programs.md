# Core IR for real programs

The current core IR handles scalar functions. Real Gleam programs require an IR
that can represent runtime-managed values, control flow, function values, module
initialization, and lowered pattern matching.

## Responsibilities

- Represent modules, functions, constants, and initialization.
- Represent runtime-managed values and representation types.
- Represent closures and indirect calls.
- Represent lowered pattern matching and guards.
- Represent records, tuples, lists, and custom types.
- Make evaluation order explicit.
- Preserve source spans for diagnostics and debug output.

## Possible structure

The compiler may need more than one IR:

- a typed high-level IR close to Gleam semantics
- a lowered control-flow IR
- a WASM-oriented IR with explicit locals, memory operations, and calls

The design should keep each representation small and testable.
