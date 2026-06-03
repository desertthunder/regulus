# Supported subset

The compiler currently parses Gleam source with tree-sitter and builds a small
compiler-owned AST. Later phases are still placeholders, so it does not yet
compile programs to executable WebAssembly.

## Current behavior

- The CLI can read a source file and run the compiler pipeline.
- Tree-sitter parses Gleam source for a single file.
- Parse errors are reported as diagnostics with source spans.
- The AST builder supports imports, functions, parameters, type annotations,
  blocks, literals, variables, calls, field access, `let`, and simple `case`.
- Unsupported parsed constructs produce AST diagnostics.
- Source file IDs, spans, and diagnostics are defined.
- WASM output is an empty placeholder.

## Not supported yet

- Constants, custom types, records with arguments, lists, tuples, bit arrays,
  external functions, attributes, and advanced patterns.
- Name resolution.
- Type checking or Gleam compiler type import.
- Core IR lowering.
- Executable `.wasm` generation.
