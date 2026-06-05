# Module interfaces for later phases

A module interface is the typed public surface of a module. It lets the compiler
check downstream modules without reinterpreting private source code, and it
gives later phases the type metadata needed for lowering and code generation.

## Interface contents

Regulus stores three maps in `ModuleInterface`:

- `functions`: public or known function names to function types
- `types`: type names to type declarations
- `constructors`: constructor names to constructor metadata

The local `TypeDeclaration` records:

- the type name
- generic parameter names
- whether the type is opaque
- constructor metadata
- the source span of the declaration

Constructor metadata records the constructor name, field names, field types, the
return type, and the constructor span.

## Why this matches Gleam's package data

The official package-interface format is produced by the Gleam compiler when
building documentation or exporting a package interface. It contains public type
definitions, type aliases, constants, and functions, with type data attached to
the public items.[^package]

Its type model includes tuples, function types, variables, and named types.
Named types record their package, module, name, and type
parameters.[^package-types]
That is the same kind of information Regulus needs, even if the local Rust
representation is simpler.

## Building an interface

Regulus builds interface data before checking function bodies:

1. Collect type definitions, type aliases, and external types.
2. Collect fully annotated function signatures.
3. Collect external function signatures.
4. Collect constants with explicit or inferable types.

This order matters. A function body can call a function declared later in the
same module only if the callee's type has already been collected.

```gleam
fn main() {
  id(1)
}

fn id(x: Int) -> Int {
  x
}
```

If `id` has a complete annotated signature, `main` can be checked before `id`'s
body has been walked.

## Project checking

Project checking extends the same idea across modules. Regulus first resolves
the project, then collects constructor and value type data from all resolved
module ASTs. Each module is then checked with those external maps available.

This is still a simplified model. A complete implementation should use module
names, import aliases, visibility, target availability, and package metadata
precisely. The important invariant is already visible: imported calls and
constructor patterns should be checked against the same interface data that
module-local calls and constructor patterns use.

## Output to lowering

Lowering receives a `TypedModule`, not raw syntax. That gives it:

- function signatures for ABI and call lowering
- expression types for choosing runtime representations
- constructor fields for record and pattern lowering
- type declarations for exports and future dependency metadata

Lowering should not rediscover whether `name` is a `String`, whether `Ok(value)`
binds an `Int`, or whether a public function returns a managed value. Those are
type-checking facts. The backend can still reject target-specific ABI shapes,
but it should not repair or reinterpret type information.

[^package]: `gleam_package_interface`, "What's a package interface?": https://gleam-package-interface.hexdocs.pm/index.html
[^package-types]: `gleam_package_interface`, `Type`: https://gleam-package-interface.hexdocs.pm/gleam/package_interface.html#Type
