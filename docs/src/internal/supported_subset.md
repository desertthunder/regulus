# Supported subset

The compiler currently parses Gleam source with tree-sitter, builds a small
compiler-owned AST, resolves names, and checks a scalar type subset. It does not
yet compile programs to executable WebAssembly.

## Current behavior

- The CLI can read a source file and run the compiler pipeline.
- Tree-sitter parses Gleam source for a single file.
- Parse errors are reported as diagnostics with source spans.
- The AST builder supports imports, functions, parameters, type annotations,
  blocks, literals, variables, calls, field access, `let`, and simple `case`.
- Unsupported parsed constructs produce AST diagnostics.
- Source file IDs, spans, and diagnostics are defined.
- Name resolution supports top-level functions, imports, parameters, local
  bindings, shadowing, and qualified module references.
- Type checking supports scalar types, function types, typed parameters,
  literals, variables, local bindings, direct calls, arity checks, argument
  checks, and simple `case` branch compatibility.
- WASM output is an empty placeholder.

## Not supported yet

- Constants, custom types, records with arguments, lists, tuples, bit arrays,
  external functions, attributes, and advanced patterns.
- Unqualified imported values and prelude resolution.
- Generic types, custom types, imported function types, and Gleam compiler type
  import.
- Core IR lowering.
- Executable `.wasm` generation.
