# Supported subset

The compiler is currently a scaffold. It wires the major phases together, but it
does not yet parse or compile real Gleam programs to executable WebAssembly.

## Current behavior

- The CLI can read a source file and run the compiler pipeline.
- Core exposes separate phase modules for parsing, AST building, resolution,
  typing, lowering, and WASM emission.
- Source file IDs, spans, and diagnostics are defined.
- WASM output is an empty placeholder until backend work begins.

## Not supported yet

- Tree-sitter Gleam parsing.
- Real AST construction.
- Name resolution.
- Type checking or Gleam compiler type import.
- Core IR lowering.
- Executable `.wasm` generation.
