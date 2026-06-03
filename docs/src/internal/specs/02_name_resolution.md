# Name resolution

Name resolution maps textual names in the AST to declarations. It should not
infer types or lower expressions. Its output is a resolved AST plus symbol data
that later phases can query.

## Responsibilities

- Build module, import, function, parameter, and local scopes.
- Resolve variable references to local bindings, parameters, imported names, or
  top-level definitions.
- Detect duplicate definitions in the same scope.
- Detect unknown names and ambiguous imported names.
- Assign stable internal IDs to symbols.

## Initial scope model

- A module scope contains top-level functions and imported modules or values.
- A function scope contains parameters and nested local bindings.
- A `let` binding is visible after its binding site in the current block.
- Inner bindings may shadow outer bindings if Gleam permits that form.

## Imports

The first implementation may support a small import subset:

- `import gleam/int`
- `import module/name as alias`
- Qualified references through imported module aliases

Unqualified imported values and prelude behavior should be documented before
being implemented.

## Output requirements

Resolved nodes should carry symbol IDs rather than raw strings for references.
The original textual name and source span should remain available for
user-facing diagnostics.
