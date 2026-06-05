# Supported subset

Regulus can compile a meaningful single-file Gleam subset end to end:

```text
Gleam source -> parse -> AST -> resolve -> type check -> IR -> WAT -> Wasm
```

It can execute scalar and managed-value examples in Wasmtime tests. It is not a
full Gleam compiler yet: project compilation, dependency packages, stdlib, and
polished host/browser/WASI interop are still incomplete.

## Current behavior

### CLI

- Compile a single Gleam source file to `.wasm`.
- Optionally write generated WAT.
- Optionally write AST, resolved, typed, IR, and WAT debug dumps.
- Load a Gleam project and print discovered modules.
- Accept target selection for Wasmtime, browser, and WASI.

Non-Wasmtime CLI targets currently use the generic Wasm backend with target
metadata and validation.

### Project model

- Read `gleam.toml`.
- Model package metadata and dependency entries.
- Discover modules under source roots.
- Assign stable source file IDs.
- Detect duplicate modules across roots.
- Report missing modules.
- Keep single-file loading available for tests and examples.

### Parsing and AST

The AST layer represents imports, functions, constants, externals, type aliases,
custom types, attributes, target groups, comments, blocks, `let`, `let assert`,
literals, variables, calls, captures, field access, records, record updates,
tuples, tuple access, lists, bit arrays, operators, pipelines, `use`, anonymous
functions, `panic`, `todo`, `assert`, `echo`, `case`, and many pattern forms.

Tree-sitter parse errors are reported as diagnostics with source spans.

### Name resolution

Resolution supports namespaced values, types, constructors, fields, modules,
imports, unqualified imports, qualified module references, prelude type names,
parameters, locals, shadowing, project-module visibility checks, private
constructor restrictions, and duplicate/unknown/ambiguous-name diagnostics.

### Type checking

The type layer supports scalar types, tuples, lists, records, custom types,
generic declarations, opaque types, function types, typed parameters, return
annotations, literals, variables, local bindings, direct calls, arity checks,
argument checks, field access, tuple access, records, constructors, record
updates, pipelines, unary and binary operations, anonymous functions, captures,
`use`, `case`, guards, nested patterns, simple exhaustiveness checks, module
interfaces, and imported constructor patterns across project modules.

The type checker infers unannotated function parameters, local values,
anonymous-function parameters, generic functions, empty and generic lists,
generic custom-type constructors, constructor patterns, polymorphic calls, and
imported generic functions. The inference layer has reusable inference
variables, type schemes, substitutions, constraint generation, unification,
occurs checks, generalization, constructor schemes, and inference interfaces.

### Core IR

Core IR represents modules, imports, declarations, constants, module init
metadata, references, exports, functions, locals, blocks, ordered instructions,
local sets, assert-match instructions, literals, direct calls, indirect calls,
function values, anonymous functions, pipelines, use-lowering, branches, tuples,
lists, bit arrays, bit-array concat, bit-string deconstruction, records,
constructors, field access, record update, list cons/deconstruction, tuple
access, comparisons, runtime equality, memory operations, and failure paths.

### Runtime and Wasm backend

The runtime representation defines object headers, tags, sizes, alignment,
static objects, bump allocation, and layouts for strings, bit arrays, lists,
tuples, records, custom values, closures, opaque values, runtime errors, and
panic values.

The backend emits deterministic WAT and Wasm for scalar and managed values,
locals, calls, imports, exports, arithmetic, float operations, boolean
operators, string concat, equality, comparisons, branches, guards, pattern
tests, pattern bindings, failure paths, memory operations, static data segments,
runtime prelude helpers, and target-aware host imports.

Raw ABI mapping:

| Gleam type     | Wasm ABI      |
| -------------- | ------------- |
| `Int`          | `i64`         |
| `Float`        | `f64`         |
| `Bool`         | `i32`         |
| `Nil`          | no result     |
| managed values | `i32` pointer |

String exports with no parameters also get `<name>__data` and `<name>__len`
adapter exports.

## Not supported yet

### Projects and dependencies

- Whole-project compilation into linked Wasm output.
- Fetching or loading Hex packages and real dependency modules.
- Real Gleam stdlib interfaces, shims, or source compilation.

### Language semantics

- Complete target-group semantics.
- Full bit-string segment matching semantics.
- Complete closure invocation with captured environments.

### Host interop and targets

- Complete browser and WASI host adapters.
- Rich managed-value import/export wrappers for arbitrary function shapes.

### Runtime and memory management

- Freeing, garbage collection, reference counting, or heap growth checks.
- Full recursive structural equality and ordering for all managed values.
- Full string inspection/debug rendering of arbitrary values.
- Complete panic/error payload reporting.

## Backend approximations to know about

Some IR forms are emitted but still have intentionally small semantics:

- `Use` emits its call expression.
- `RecordUpdate` currently emits the original record expression.
- `BitStringDeconstruct` checks for a bit-array object, not full segment
  matches.
- Managed equality is not yet full recursive Gleam equality.

## Validation coverage

Tests cover parsing, AST construction, resolution, project loading, type
checking, IR lowering, runtime layouts, WAT snapshots, Wasmtime execution,
memory inspection, runtime helpers, host imports, export adapters,
deterministic output, and unsupported ABI/target diagnostics.
