# Regulus

Regulus ("Reggie") is an experimental Gleam to WebAssembly compiler written in
Rust.

<img src="./docs/book/src/favicon.png" alt="Reggie" width="196"/>

The compiler uses tree-sitter to parse Gleam source, builds compiler-owned data
structures, lowers a supported subset of Gleam to core IR, and emits `.wasm`
with optional WAT output.

```text
Gleam source
  -> tree-sitter syntax tree
  -> AST
  -> name resolution
  -> type checking
  -> core IR
  -> WebAssembly module
  -> .wasm / optional WAT
```

## Status

Regulus is not a full Gleam compiler yet. It can compile and run small programs
using scalar values, managed values, pattern matching, closures, selected stdlib
modules, and host imports. The compiler pipeline is intentionally visible so
that each layer can be tested and explained.

### At a glance

| Area                          | Status                          |
| ----------------------------- | ------------------------------- |
| Single-file Gleam to Wasm     | Supported                       |
| Type checking                 | Broad subset                    |
| Managed runtime values        | Partial                         |
| Standard library              | Selected modules and intrinsics |
| Whole-project linked output   | Supported subset                |
| Dependency source loading     | Selected Hex and path packages  |
| Browser, bundler, and Node.js | Supported adapter subset        |
| WASI ABI                      | Incomplete                      |

### Working today

- [x] Parse Gleam source with tree-sitter
- [x] Build an AST with source spans
- [x] Report diagnostics with source spans
- [x] Resolve imports, modules, locals, types, constructors, and fields
- [x] Type check scalar values, structured values, calls, branches, and patterns
- [x] Infer generic functions, generic constructors, closures, and list types
- [x] Lower the supported subset to core IR
- [x] Emit deterministic `.wasm`
- [x] Render optional WAT
- [x] Run scalar and managed-value exports with Wasmtime
- [x] Load `gleam.toml` and discover project modules
- [x] Compile supported projects into linked Wasm output
- [x] Load selected Hex and path dependency sources
- [x] Lower supported externals to Wasm imports
- [x] Validate target-aware host imports
- [x] Emit deterministic browser, bundler, and Node.js `.mjs` adapters
- [x] Emit optional AST, resolved, typed, IR, runtime, and ABI debug artifacts
- [x] Render source diagnostics with snippets, labels, notes, and stable
      project ordering
- [x] Keep normal CLI output concise, with `--no-color` and `NO_COLOR` support

### Supported Gleam surface

- [x] Public and private functions
- [x] Function annotations and inferred parameters
- [x] `Int`, `Float`, `Bool`, `String`, and `Nil`
- [x] Local bindings and structured patterns
- [x] Direct, imported, and external calls
- [x] `case` expressions, guards, and nested patterns
- [x] Type aliases, custom types, opaque types, generics, and constructors
- [x] Records, custom values, tuples, lists, and bit arrays
- [x] Anonymous functions, captures, closures, pipelines, and `use`
- [x] Selected stdlib modules and intrinsics

### WebAssembly output

- [x] Function definitions, imports, and exports
- [x] Scalar ABI values: `i64`, `f64`, and `i32`
- [x] Managed values as guest-memory pointers
- [x] Linear memory and static data segments
- [x] Runtime objects for strings, records, lists, tuples, closures, and custom
      values
- [x] Branches, comparisons, equality, pattern checks, and failure paths
- [x] Selected runtime helpers and stdlib intrinsics
- [x] Checked runtime helper fragments before final module output

### Not yet implemented

- [ ] Compile every valid Gleam project shape
- [ ] Compile broad dependency source modules without subset limits
- [ ] Compile the full published `gleam_stdlib` package without registry shims
- [ ] Provide complete browser, bundler, and Node.js host APIs beyond the
      current adapter subset
- [ ] Provide complete WASI adapters
- [ ] Add garbage collection or reference counting for long-lived managed
      values

For more detail, see the case-study book in
[docs/book](./docs/book/src/introduction.md) and the user/development docs in
[docs/website](./docs/website/index.md).

## Usage

Build a Gleam project into one linked Wasm artifact:

```sh
cargo run -q -p compiler_cli -- build examples/scalar_project
```

Compile a single Gleam source file:

```sh
cargo run -q -p compiler_cli -- compile fixtures/e2e/public_id.gleam \
  -o .sandbox/public_id.wasm \
  --emit wasm,wat \
  --dump-dir .sandbox/dumps
```

`--emit` selects artifacts such as `wasm`, `wat`, `ast`, `resolved`, `typed`,
and `ir`. The `--dump-dir` flag writes debug output for compiler stages. See
[CLI usage](./docs/website/guide/usage/cli.md) for project builds, targets,
artifacts, and `run`.

## Development

Use the Rust workspace from the repository root:

```sh
cargo fmt
cargo test
cargo clippy --workspace --all-targets
```

To run examples & tests:

```sh
cargo test -p compiler_core
cargo test -p compiler_cli
```

```sh
cargo run -q -p compiler_cli -- compile fixtures/e2e/public_id.gleam \
  --out-dir .sandbox --emit wasm,wat
cargo run -q -p compiler_cli -- run fixtures/e2e/public_id.gleam
cargo run -q -p compiler_cli -- build examples/scalar_project \
  --target bundler --out-dir .sandbox
```

### Documentation

```sh
cd docs/website
pnpm install
pnpm docs:dev
```
