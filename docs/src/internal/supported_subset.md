# Supported subset

The compiler currently parses Gleam source with tree-sitter, builds a small
compiler-owned AST, resolves names, checks a scalar type subset, lowers that
subset to core IR, and emits executable WebAssembly for public scalar functions.

## Current behavior

- The CLI can read a source file and run the compiler pipeline.
- Tree-sitter parses Gleam source for a single file.
- Parse errors are reported as diagnostics with source spans.
- The AST builder supports executable syntax for imports, functions, parameters,
  type annotations, blocks, literals, variables, calls, field access, `let`, and
  simple `case`.
- The AST builder preserves broader Gleam syntax as raw syntax nodes with source
  spans.
- Source file IDs, spans, and diagnostics are defined.
- Name resolution supports namespaced symbols, top-level functions, imports,
  unqualified imports, prelude type names, parameters, local bindings,
  shadowing, custom-type names, constructors, fields, qualified module
  references, and project-module visibility checks.
- Type checking supports scalar types, tuples, lists, custom type names,
  generics, opaque type declarations, function types, typed parameters,
  literals, variables, local bindings, direct calls, arity checks, argument
  checks, simple constructors, module interfaces, constructor metadata, field
  metadata, and simple `case` branch compatibility.
- Core IR lowering supports functions, params, locals, literals, local reads and
  writes, direct calls, blocks, and simple branches.
- WebAssembly output supports scalar signatures, string pointers, locals,
  constants, local reads and writes, direct calls, exports, WAT generation, and
  `.wasm` assembly.
- Runtime representation includes object headers, tags, alignment, static string
  objects, managed-value pointers, and a bump allocator helper.
- Wasmtime tests execute exported scalar functions and inspect string memory
  layout.

## Not supported yet

- Executable support for constants, custom types, records with arguments, lists,
  tuples, bit arrays, external functions, attributes, and advanced patterns.
- Full dependency package resolution and imported function type information.
- Full inference for generic types, full custom-type checking, imported function
  types from dependencies, and Gleam compiler type import.
- Lowering for imported calls, records, lists, tuples, and function values.
- WASM code generation for branches, records, lists, tuples, imports, dynamic
  allocation of managed values, and full runtime-managed value operations.
