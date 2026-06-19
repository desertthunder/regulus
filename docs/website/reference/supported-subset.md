# Supported subset

Regulus currently compiles single-file Gleam programs and supported Gleam
projects to WebAssembly through the full compiler pipeline:

```text
Gleam source -> parse -> AST -> resolve -> type check -> IR -> Wasm
```

## Summary

| Area                        | Status                          |
| --------------------------- | ------------------------------- |
| Single-file compilation     | Supported                       |
| Whole-project linked output | Supported for project modules   |
| Project discovery           | Partial                         |
| Type checking               | Broad subset                    |
| Managed runtime values      | Partial                         |
| Standard library            | Selected modules and intrinsics |
| JavaScript host ABI         | Browser, bundler, and Node.js   |
| WASI ABI                    | Incomplete                      |
| Memory management           | Bump allocation only            |

## Supported today

### Inputs

- Compile one Gleam source file to `.wasm`.
- Compile supported Gleam projects to one linked `.wasm` artifact.
- Load `gleam.toml` and discover project modules.
- Filter target-group declarations for Wasmtime, browser, bundler, Node.js, and
  WASI targets.
- Emit optional debug dumps: AST, resolved AST, typed AST, IR, and WAT.
- Emit generated `.mjs` JS host adapters for browser, bundler, and Node.js
  targets when Wasm output is requested.

### Language surface

Supported syntax includes:

- modules, imports, constants, functions, and externals
- type aliases, custom types, and opaque types
- blocks, `let`, and `let assert`
- literals, variables, calls, and captures
- records, record updates, and field access
- tuples and tuple access
- lists
- bit arrays
- unary and binary operators
- pipelines
- `use`
- anonymous functions
- `panic`, `todo`, `assert`, and `echo`
- `case`, guards, and nested patterns

### Name resolution

Supported:

- local variables and shadowing
- module references
- qualified and unqualified imports
- values, types, constructors, and fields
- prelude type names
- project-module visibility checks
- private constructor checks
- duplicate, unknown, and ambiguous name diagnostics

### Type checking

Supported:

- scalar types
- tuples, lists, records, and custom types
- generic declarations and constructors
- function types
- typed parameters and return annotations
- local bindings
- direct, imported, and external calls
- arity and argument checks
- field and tuple access
- record construction and updates
- unary and binary operations
- pipelines
- anonymous functions
- captures
- `use`
- `case`
- guards and nested patterns
- basic exhaustiveness checks
- module interfaces
- imported constructor patterns across project modules

Inference supports:

- unannotated function parameters
- eligible local values
- anonymous-function parameters
- generic functions
- empty and generic lists
- generic custom-type constructors
- constructor patterns
- polymorphic calls
- imported generic functions

The checker reports ambiguous return types, recursive inferred types, and
invalid generic arity.

### WebAssembly output

The backend emits deterministic Wasm and optional WAT for:

- scalar values
- managed heap values
- locals
- direct, external, and indirect calls
- imports and exports
- arithmetic and float operations
- boolean operators
- string concatenation
- equality and comparisons
- branches and guards
- pattern tests and bindings
- failure paths
- memory operations
- static data segments
- selected runtime helpers
- target-aware host imports
- selected stdlib intrinsics
- JS host ABI helpers for strings, structured readers, and opaque handles

ABI mapping:

| Gleam type     | Wasm ABI      |
| -------------- | ------------- |
| `Int`          | `i64`         |
| `Float`        | `f64`         |
| `Bool`         | `i32`         |
| `Nil`          | no result     |
| managed values | `i32` pointer |

String exports with no parameters also receive:

- `<name>__data`
- `<name>__len`

JS host builds export stable runtime helpers for writing strings, reading
strings and structured managed values, and wrapping opaque host handles.

## Partially supported

### Project model

Regulus can:

- read `gleam.toml`
- model package metadata
- model dependency entries
- discover modules under source roots
- assign stable source file IDs
- detect duplicate modules
- report missing modules
- load selected Hex and path dependency source modules
- link supported dependency source modules into the final artifact
- keep single-file loading available for tests and examples

Project builds compile discovered project modules into one linked Wasm output.

### Standard library

The stdlib registry models selected interfaces and lowering strategies for:

- `gleam/io`
- `gleam/int`
- `gleam/string`
- `gleam/list`
- `gleam/result`
- `gleam/option`
- `gleam/order`
- `gleam/bool`
- `gleam/dict`
- `gleam/dynamic`
- `gleam/dynamic/decode`
- `gleam/float`
- `gleam/function`
- `gleam/bit_array`

Some remaining modules are interface-only or unsupported beyond dependency
metadata. This is not full stdlib source compilation.

### Externals and targets

General non-stdlib externals lower to Wasm imports when their ABI is supported.
Bodyless `@external` declarations from project and dependency source lower
through the selected target ABI when metadata is available.

The compiler:

- preserves import module and function names
- filters target-specific declarations
- validates selected target modules
- rejects unsupported ABI shapes before byte emission
- emits generated JavaScript adapter glue for browser, bundler, and Node.js
  targets when Wasm output is requested
- embeds import and export metadata for checked JS host calls
- exposes standard browser and Node.js import helpers

The JavaScript adapter supports scalar and string imports, scalar and string
export parameters, supported structured export returns, and opaque host-handle
conversion. Structured JavaScript values passed into Gleam imports or exported
function parameters are still deferred.

## Not yet supported

### Projects and dependencies

- Compiling every valid Gleam project shape.
- Broad dependency source compilation without subset limits.
- Full stdlib source compilation.

### Host interop

- Complete WASI adapters.
- Writing arbitrary structured JavaScript values into Gleam.
- Structured JS import parameters and returns.
- Arbitrary managed-value wrappers for every import and export shape.

### Runtime and memory management

- Freeing.
- Garbage collection.
- Reference counting.
- Heap growth checks.

## Known approximations

Some implemented forms intentionally have smaller semantics than Gleam proper:

- Residual raw `Use` IR is rejected before WAT assembly.
- `BitStringDeconstruct` supports current segment matching, but not complete
  Gleam bit-string semantics.

## Validation

Tests cover:

- parsing
- AST construction
- resolution
- project loading
- type checking
- IR lowering
- runtime layouts
- WAT snapshots
- Wasmtime execution
- memory inspection
- runtime helpers
- host imports
- export adapters
- deterministic output
- unsupported ABI diagnostics
- unsupported target diagnostics
