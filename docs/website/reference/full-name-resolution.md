# Name resolution

Name resolution maps textual names in the AST to stable compiler symbols. It
uses Gleam-like namespaces so values, types, constructors, fields, and modules
stay distinct.

## Scopes and symbols

The resolver builds scopes for modules, declarations, functions, patterns, and
blocks. Symbols record their namespace and source span. Resolved references
point to symbol IDs rather than repeating textual lookup work in later phases.

The resolver supports:

- top-level functions and constants
- parameters and local bindings
- lexical shadowing
- custom-type names, constructors, and fields
- qualified module references
- project-module visibility checks
- public and private members across project modules

## Imports

Supported import behavior includes:

- module imports with and without aliases
- unqualified value imports
- unqualified constructor imports
- imported type and constructor metadata
- ambiguous unqualified import diagnostics

Dependency package module loading is not complete yet, but the resolver uses the
same interface structures that dependency metadata should populate.

## Diagnostics

Resolution reports duplicate, unknown, private, and ambiguous names with source
spans. Pattern resolution also checks for duplicate bindings in a single pattern
and records names introduced by nested patterns.

## Output

Resolved modules preserve module interfaces so later phases can check qualified
access, public/private visibility, constructors, and fields without repeating
name lookup.
