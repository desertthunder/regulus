# Development

These pages describe how Regulus is built and how to work on it.

## Compiler phases

Regulus keeps compiler phases explicit:

```text
Gleam source
  -> tree-sitter syntax tree
  -> AST
  -> name resolution
  -> type checking
  -> core IR
  -> WebAssembly
```

## Useful references

- [Testing](./testing.md)
- [Core IR](./core-ir.md)
- [Runtime representation](./runtime-representation.md)
- [Runtime memory](./runtime-memory.md)
- [Wasm backend and runtime](./wasm-backend-and-runtime.md)
- [Project model](../reference/project-model-and-modules.md)
