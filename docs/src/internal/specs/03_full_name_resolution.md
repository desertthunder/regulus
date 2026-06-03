# Full name resolution

Name resolution must match Gleam's module and import behavior closely enough for
real packages. It should resolve values, types, constructors, fields, modules,
and imported names across a whole project.

## Responsibilities

- Build scopes for modules, declarations, functions, patterns, and blocks.
- Resolve qualified and unqualified imports.
- Resolve prelude names according to Gleam's rules.
- Resolve type names, constructors, record fields, and value names.
- Detect duplicate, unknown, private, and ambiguous names.
- Preserve source spans for all resolved references and diagnostics.

## Imports

The resolver should support:

- module imports with and without aliases
- unqualified value imports
- unqualified type and constructor imports
- import aliasing
- dependency package modules

## Output

Resolved references should point to stable symbol IDs. Symbols record their
namespace so value, type, constructor, field, and module names are not conflated.
Project resolution should also preserve module interfaces so qualified access can
check public and private members.
