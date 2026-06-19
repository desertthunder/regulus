# Changelog

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### 2026-06-19

#### Added

- CLI project and single-file compilation now share output path, target, WAT,
  emit, dump, verbose, and JSON reservation flags.
- `run` and `exec` now compile one source file, execute a Wasmtime export, and
  render scalar and supported managed return values.
- Build output now includes deterministic Wasm, optional WAT, AST, resolved,
  typed, IR, runtime layout, ABI, and import/export metadata artifacts.
- Browser, bundler, and Node.js targets now emit deterministic `.mjs` host
  adapter files when Wasm output is requested.
- CLI diagnostics now include stable multi-module ordering, source snippets,
  labels, notes, clearer project errors, clearer target and ABI mismatch
  messages, and improved type-inference context.

#### Changed

- The CLI now supports a global `--no-color` flag and honors the
  [`NO_COLOR`](https://no-color.org/) environment variable for plain
  human-readable output.
- WAT is now treated as a rendered debug artifact; structured Wasm byte
  emission is the source of truth for final `.wasm` output.

### 2026-06-05

#### Added

- Type and generic inference now includes reusable inference variables, type
  schemes, substitutions, constraint generation, unification, generalization,
  constructor schemes, and inference interfaces.
- The type checker now infers unannotated function parameters, eligible local
  bindings, polymorphic calls, generic lists, generic custom-type constructors
  and patterns, and imported generic functions.
- Type diagnostics now include ambiguous return types, generic arity mismatches,
  and recursive inferred types.
- The main type checker now uses the shared inference unifier for type
  compatibility checks.
- Bit-string patterns match integer segments, bind integer and variable
  bit-array tails, validate segment shapes, and execute in Wasm tests.

- The WASM backend now documents and emits scalar and managed value
  representations for strings, bit arrays, lists, tuples, records, custom
  values, closures, opaque values, results, options, errors, and panics.
- The WASM runtime prelude now includes allocation, string, bit-array, list,
  tuple, record, custom-type, closure, equality, ordering, panic, assertion,
  and debug helpers.
- WASM code generation now emits current IR expression and instruction forms,
  branches, guards, lowered patterns, failure paths, operators,
  short-circuiting booleans, direct/imported/exported/indirect calls, and
  module constants/static data in deterministic order.
- The WASM backend now validates target-aware ABI rules for Wasmtime, browser,
  and WASI host modules, emits raw-Wasm string export adapters, diagnoses
  unsupported target and ABI combinations before assembly, and keeps WAT/Wasm
  output deterministic.
- Backend validation now covers WAT snapshots, Wasmtime execution, static and
  dynamic memory inspection, runtime helpers, import/export ABI checks, and
  unsupported target/ABI diagnostics.

### 2026-06-04

#### Added

- Project model and module loading now reads Gleam project metadata, discovers
  modules, assigns stable source IDs, and reports project graph diagnostics.
- Full Gleam syntax is represented in the compiler AST or rejected with targeted
  source-spanned diagnostics for known limitations.
- Name resolution now uses Gleam-like namespaces across values, types,
  constructors, fields, modules, imports, and project visibility checks.
- Type checking now records module interfaces, constructors, fields, generics,
  typed expressions, and real-language pattern metadata for lowering.
- Runtime representation now documents and tests object headers, tags,
  alignment, strings, lists, tuples, records, custom values, closures, managed
  pointers, and allocation helpers.
- Pattern matching now parses, resolves, type-checks, lowers, diagnoses, and
  emits the supported scalar and structured pattern forms with explicit branch
  behavior.
- Structured language support now covers declarations, constants, externals,
  target groups, operators, pipelines, `use`, anonymous functions, captures,
  records, updates, tuples, lists, bit arrays, imported members, opaque values,
  and module interfaces.
- Core IR now represents module declarations, constants, managed value forms,
  function values, call ABI metadata, structured control flow, failure paths,
  and stable debug output.
