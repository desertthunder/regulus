# Compiler notes for Gleam syntax

The language features in this chapter are also the features a WebAssembly
compiler has to represent.

## Modules

Modules and packages give the compiler source files, module names, dependencies,
and package metadata. Imports affect name resolution because a dotted expression
such as `io.println("hello")` can only be understood after the name `io` is
resolved.

The compiler also needs module interfaces. A module interface records public
functions, public types, constructors, fields, and private details that should
not be used from other modules.

## Expressions

Functions, values, and expressions become the input to type checking and
lowering. Simple scalar values can map directly to WebAssembly value types, while
strings, lists, tuples, records, custom types, and closures need a runtime
representation.

Expression-oriented syntax is useful for lowering because a block, a function
body, a `case`, and a pipeline all produce values. The compiler can translate
those expressions into a smaller intermediate representation before emitting
WebAssembly.

## Data

Custom types, records, tuples, and lists define the value shapes the runtime must
represent. Records need field layout. Custom types need variant tags. Lists need
a representation for empty and non-empty lists. Tuples need ordered fields.

Opaque types are a compile-time boundary. They do not necessarily require a
different runtime representation, but they do affect which constructors and
patterns other modules are allowed to use.

## Patterns

Pattern matching crosses several parts of the compiler. It binds names, checks
value shapes, depends on type information, and eventually has to become explicit
branching logic.

Exhaustiveness checking is also a type-driven analysis. The compiler needs to
know the possible variants of a custom type and the possible values covered by
literal, tuple, list, record, and discard patterns.

## Syntax preservation

The AST can preserve syntax that is not executable yet by storing raw syntax
nodes with the tree-sitter kind, source text, and span. That lets the parser and
AST builder accept real Gleam modules while later compiler passes report precise
diagnostics for unsupported runtime behavior.

The important invariant is that syntax remains source-linked. Even when a
feature is not fully compiled yet, the compiler should know what source text it
came from and where to point when explaining the limitation.
