# Changelog

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Added

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

### Changed

- The type inference plan graduated from internal specs and tasks into the
  development docs as conceptual implementation documentation.

## [1.0.0] - 2026-06-05

### Added

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

## [0.1.0] - 2026-06-04

### Added

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
