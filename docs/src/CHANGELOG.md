# Changelog

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### [0.1.0] - 2026-06-04

#### Added

- Project model and module loading now reads Gleam project metadata, discovers modules, assigns
  stable source IDs, and reports project graph diagnostics.
- Full Gleam syntax is represented in the compiler AST or rejected with targeted source-spanned
  diagnostics for known limitations.
- Name resolution now uses Gleam-like namespaces across values, types, constructors, fields,
  modules, imports, and project visibility checks.
- Type checking now records module interfaces, constructors, fields, generics, typed expressions,
  and real-language pattern metadata for lowering.
- Runtime representation now documents and tests object headers, tags, alignment, strings, lists,
  tuples, records, custom values, closures, managed pointers, and allocation helpers.
- Pattern matching now parses, resolves, type-checks, lowers, diagnoses, and emits the supported
  scalar and structured pattern forms with explicit branch behavior.
- Structured language support now covers declarations, constants, externals,
  target groups, operators, pipelines, `use`, anonymous functions, captures,
  records, updates, tuples, lists, bit arrays, imported members, opaque values,
  and module interfaces.
- Core IR now represents module declarations, constants, managed value forms, function values, call
  ABI metadata, structured control flow, failure paths, and stable debug output.
