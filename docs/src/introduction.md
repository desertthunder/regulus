# Introduction

This book follows a small compiler from Gleam source code to WebAssembly.

Gleam is a friendly functional language with static types. WebAssembly is a
portable instruction format that can run in browsers and standalone runtimes
such as Wasmtime. Putting them together is a practical way to learn how a
compiler works: source text becomes syntax trees, names become resolved symbols,
expressions get types, high-level code is lowered into an intermediate
representation, and that representation becomes WebAssembly.

The project is written in Rust. It uses tree-sitter to parse Gleam source, then
builds its own compiler data structures for the rest of the work.

```text
Gleam source
  -> tree-sitter syntax tree
  -> AST
  -> name resolution
  -> type checking
  -> core IR
  -> WAT / WebAssembly
  -> Wasmtime or a browser
```

The compiler is intentionally developed in small, visible pieces. Each chapter
introduces a compiler concept and connects it to this project. You should be able
to read the book as a guide to the codebase, but also as a general introduction
to compilers, Gleam, and WebAssembly.

## What this book covers

The first chapters introduce the main compiler pipeline:

- syntax trees and parsing
- name resolution and scopes
- type systems and type checking
- intermediate representations and lowering
- WebAssembly code generation

The Gleam chapter introduces the source language itself: modules, imports,
functions, expressions, custom types, records, tuples, lists, pattern matching,
and type inference.

The development pages describe what the compiler supports today and how tests
and documentation are organized.

## What to expect from the compiler

The compiler can already compile a small scalar subset of Gleam to executable
WebAssembly. For example, a public function with scalar parameters can be emitted
as WAT, assembled to `.wasm`, loaded in Wasmtime, and called from a test.

Many parts of real Gleam still need more work before they can execute as
WebAssembly: strings, lists, records, custom types, imported functions, advanced
patterns, and full package dependency compilation all require more runtime and
compiler support.

That gap is useful for learning. The existing code shows the complete shape of a
compiler, while the unsupported language features show where real compiler work
gets interesting.

## How to read this book

If you are new to compilers, read the chapters in order. Each one builds on the
previous one.

If you know compilers but are new to Gleam, start with the Gleam chapter, then
come back to the pipeline chapters.

If you are working on the project, keep the development pages nearby. They show
what is supported, where fixtures live, and how tests should be structured.
