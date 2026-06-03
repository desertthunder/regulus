# Pipeline overview

This compiler starts from Gleam source and uses tree-sitter for parsing. It does
not own a production lexer or parser. The compiler-owned frontend begins at the
translation from tree-sitter's concrete syntax tree into our typed AST.

```text
source.gleam
  -> tree-sitter concrete syntax tree
  -> compiler AST
  -> name resolution
  -> type checking, or type import from the Gleam compiler later
  -> core IR
  -> WebAssembly IR / WAT
  -> .wasm
  -> Wasmtime or browser runtime
```

## Goals

- Compile a small, well-specified subset of Gleam to WebAssembly.
- Keep compiler phases explicit and independently testable.
- Reuse the official Gleam compiler where it is practical, especially for types.
- Emit readable diagnostics that point back to Gleam source spans.

## Non-goals for the first milestone

- Implementing a full Gleam lexer and parser.
- Supporting all Gleam language features.
- Matching the official JavaScript or Erlang backends.
- Optimizing generated WebAssembly.

## Phase contracts

Each phase should have a narrow input and output type:

| Phase     | Input            | Output                       |
| --------- | ---------------- | ---------------------------- |
| Parse     | Source text      | tree-sitter CST              |
| AST build | CST + source     | compiler AST                 |
| Resolve   | AST              | resolved AST + symbol tables |
| Type      | resolved AST     | typed AST                    |
| Lower     | typed AST        | core IR                      |
| Codegen   | core IR          | WASM IR / WAT                |
| Assemble  | WAT or binary IR | `.wasm`                      |

Phase boundaries are public within the crate. Tests should be able to construct
inputs for a phase without running the whole compiler pipeline.
