# Compiler notes for Gleam syntax

The language features in this chapter are also the features a WebAssembly
compiler has to represent.

Modules and packages give the compiler source files, module names, dependencies,
and package metadata. Imports affect name resolution because a dotted expression
such as `io.println("hello")` can only be understood after the name `io` is
resolved.

Functions, values, and expressions become the input to type checking and
lowering. Simple scalar values can map directly to WebAssembly value types, while
strings, lists, tuples, records, custom types, and closures need a runtime
representation.

Pattern matching crosses several parts of the compiler. It binds names, checks
value shapes, depends on type information, and eventually has to become explicit
branching logic.

The AST can preserve syntax that is not executable yet by storing raw syntax
nodes with the tree-sitter kind, source text, and span. That lets the parser and
AST builder accept real Gleam modules while execution support grows feature by
feature.
