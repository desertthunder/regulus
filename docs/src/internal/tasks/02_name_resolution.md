# Name resolution tasks

## Goal

Resolve AST names to stable symbols before type checking.

## Tasks

- [x] Define symbol IDs, scopes, and a symbol table.
- [x] Collect top-level function declarations.
- [x] Resolve function parameters and local `let` bindings.
- [x] Resolve variable references to symbols.
- [x] Implement duplicate-name diagnostics.
- [x] Implement unknown-name diagnostics.
- [x] Add initial support for qualified module imports.
- [x] Add resolver tests for shadowing, duplicates, imports, and unknown names.

## Done when

Every variable reference in the supported AST subset either points to a symbol ID
or produces a targeted diagnostic.
