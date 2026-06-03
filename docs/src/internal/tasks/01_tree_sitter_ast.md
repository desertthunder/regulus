# Tree-sitter and AST tasks

## Goal

Parse Gleam source with tree-sitter and build a compiler-owned AST.

## Tasks

- [x] Add the tree-sitter Gleam grammar dependency.
- [x] Implement parser setup for a single source file.
- [x] Detect and report tree-sitter error nodes.
- [x] Define AST nodes for modules, imports, functions, parameters, blocks,
      literals, variables, calls, `let`, and simple `case`.
- [x] Implement CST-to-AST conversion for the initial subset.
- [x] Preserve spans on all AST nodes.
- [x] Add AST snapshot tests for simple modules and unsupported syntax.

## Done when

A valid small Gleam module can be parsed into AST, and malformed or unsupported
syntax produces a useful diagnostic.
