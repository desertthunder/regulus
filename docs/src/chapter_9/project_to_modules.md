# Project input and module graph

A compiler that only accepts one source string is useful for tests, but real
Gleam programs are projects. Project compilation starts by finding package
configuration and source modules before language phases run.

## Project root

Regulus treats a directory as a project root. If the user gives a file path, the
file's parent directory is used as the root for project loading.

The project loader reads `gleam.toml`. Gleam's project configuration documents
package metadata such as `name`, `version`, `description`, `licences`,
repository links, target, dependencies, and dev dependencies.[^gleam-toml]
Regulus models those fields in `GleamToml`.

## Module discovery

The loader walks:

```text
src/
test/
```

and collects `.gleam` files. Module names come from paths relative to the source
root:

```text
src/app.gleam      -> app
src/app/view.gleam -> app/view
```

Each module receives a stable `SourceFileId`. Diagnostics and later compiler
data can use that ID to refer back to the original source.

## Duplicate modules

`src/app.gleam` and `test/app.gleam` both map to module name `app`. Regulus
reports that as a project error because later phases need one unique source file
per module name.

This is an architecture issue, not only a user-experience issue. Name
resolution and imported interfaces are keyed by module names. Duplicate names
would make imports ambiguous.

## Dependencies

Regulus records dependency declarations from `gleam.toml` as package-graph
metadata. A dependency entry may be a version string or an options table with
fields such as `version`, `path`, or `git`.

The current loader records these requirements but does not yet load dependency
source or package-interface data. That is the right next boundary: project
loading should discover what packages exist, then resolution and type checking
should consume module interfaces for the packages that are available.

## Single-file path

The single-file compiler path remains important. Unit tests and small examples
should not need a full project. `SourceFile::new(SourceFileId(0), source)` is
enough to parse, resolve, type check, lower, and emit a small module.

The risk is letting the single-file path become the only architecture. Project
loading exists so module IDs, imports, visibility, package dependencies, and
interfaces have a place to live.

[^gleam-toml]: Gleam, "`gleam.toml`": https://gleam.run/writing-gleam/gleam-toml/
