# Regulus

Regulus ("Reggie") is an experimental Gleam to WebAssembly compiler written in
Rust.

<img src="./docs/src/assets/images/lucywasm.png" alt="Lucy WASM" width="250"/>

The compiler uses tree-sitter to parse Gleam source, builds compiler-owned data
structures, and lowers a supported subset of Gleam to WAT and `.wasm`.

```text
Gleam source
  -> tree-sitter syntax tree
  -> AST
  -> name resolution
  -> type checking
  -> core IR
  -> WAT / WebAssembly
```

## Status

Regulus is not a full Gleam compiler yet. It can compile and run small scalar
programs, and it keeps the pipeline visible so each compiler layer can be tested
and explained.

### Compiler pipeline

- [x] Parse a single Gleam source file with tree-sitter
- [x] Build a compiler-owned AST with source spans
- [x] Report parse and compiler diagnostics with source spans
- [x] Resolve top-level functions, imports, local bindings, and constructors
- [x] Type check scalar expressions, direct calls, and simple `case` branches
- [x] Lower the supported subset to a small core IR
- [x] Emit WAT and assemble `.wasm`
- [x] Execute exported scalar functions in Wasmtime tests
- [ ] Compile full packages and dependencies
      ([spec](./docs/src/internal/specs/01_project_model_and_modules.md))
- [ ] Import type information from Gleam packages
      ([spec](./docs/src/internal/specs/04_gleam_types_and_interfaces.md))

### Gleam language subset

- [x] Public and private functions
- [x] Typed function parameters and return annotations
- [x] `Int`, `Float`, `Bool`, `String`, and `Nil` literals
- [x] Local bindings with simple name and discard patterns
- [x] Direct calls to functions in the same module
- [x] Simple `case` expressions over scalar values
- [x] Literal, binding, discard, and alias patterns in supported contexts
- [x] Type declarations, aliases, constructors, fields, generics, and opaque
      type names in name resolution and type checking
- [ ] Executable records, custom values, tuples, lists, and bit arrays
      ([spec](./docs/src/internal/specs/07_core_ir_for_real_programs.md))
- [ ] External functions and host imports
      ([spec](./docs/src/internal/specs/09_stdlib_and_host_interop.md))
- [ ] Advanced pattern matching over structured values
      ([spec](./docs/src/internal/specs/06_pattern_matching.md))
- [ ] Full generic type inference
      ([spec](./docs/src/internal/specs/04_gleam_types_and_interfaces.md))

### WebAssembly output

- [x] Function definitions and exports
- [x] Scalar WebAssembly signatures (`i64`, `f64`, `i32`)
- [x] Locals, constants, local reads, and local writes
- [x] Direct function calls
- [x] Branches for supported `case` expressions
- [x] Linear memory export and static string objects
- [x] Bump allocation helper in the runtime prelude
- [ ] Imported functions
      ([spec](./docs/src/internal/specs/09_stdlib_and_host_interop.md))
- [ ] Runtime-managed records, lists, tuples, and custom values
      ([spec](./docs/src/internal/specs/05_runtime_representation.md))
- [ ] Standard library and browser/WASI interop
      ([spec](./docs/src/internal/specs/09_stdlib_and_host_interop.md))

For more detail, see the book in [`docs/src`](./docs/src/introduction.md) and
the current supported subset in
[`docs/src/internal/supported_subset.md`](./docs/src/internal/supported_subset.md).

## Usage

Compile a Gleam source file to WebAssembly:

```sh
cargo run -q -p compiler_cli -- compile fixtures/e2e/public_id.gleam \
  -o .sandbox/public_id.wasm \
  --wat \
  --dump-dir .sandbox/dumps
```

The `--wat` flag also writes WebAssembly text format. The `--dump-dir` flag
writes debug output for compiler stages such as AST, resolved names, typed
expressions, IR, and WAT.

## Development

```sh
cargo fmt
cargo test
mdbook build docs
```
