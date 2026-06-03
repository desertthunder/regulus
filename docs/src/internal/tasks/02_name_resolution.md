# Name resolution tasks

## Goal

Resolve AST names to stable symbols before type checking.

## Tasks

- [ ] Define symbol IDs, scopes, and a symbol table.
- [ ] Collect top-level function declarations.
- [ ] Resolve function parameters and local `let` bindings.
- [ ] Resolve variable references to symbols.
- [ ] Implement duplicate-name diagnostics.
- [ ] Implement unknown-name diagnostics.
- [ ] Add initial support for qualified module imports.
- [ ] Add resolver tests for shadowing, duplicates, imports, and unknown names.

## Done when

Every variable reference in the supported AST subset either points to a symbol ID
or produces a targeted diagnostic.
