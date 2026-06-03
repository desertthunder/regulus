# Tree-sitter and AST tasks

## Goal

Parse Gleam source with tree-sitter and build a compiler-owned AST.

## Tasks

- [ ] Add the tree-sitter Gleam grammar dependency.
- [ ] Implement parser setup for a single source file.
- [ ] Detect and report tree-sitter error nodes.
- [ ] Define AST nodes for modules, imports, functions, parameters, blocks,
      literals, variables, calls, `let`, and simple `case`.
- [ ] Implement CST-to-AST conversion for the initial subset.
- [ ] Preserve spans on all AST nodes.
- [ ] Add AST snapshot tests for simple modules and unsupported syntax.

## Done when

A valid small Gleam module can be parsed into AST, and malformed or unsupported
syntax produces a useful diagnostic.
