# Project model and modules

The project model gives the compiler a complete view of source files before
resolution, type checking, lowering, or code generation begins. It supports full
Gleam project inputs while keeping single-file compilation available for small
tests.

## Project configuration

Project configuration is read from `gleam.toml`. The compiler models the Gleam
package fields used by current projects:

- `name`
- `version`
- `description`
- `licences`
- `repository`
- `links`
- `gleam`
- `target`
- `dependencies`
- `dev-dependencies`

Dependency entries may be plain version requirements or option tables with
metadata such as `version`, `path`, or `git`. The project model records these as
dependency hooks. Loading dependency source can build on this data later.

## Module discovery

Project loading discovers modules under the configured source roots, assigns
stable source file IDs, and maps module names to paths. Later phases use that
map to resolve qualified module references and enforce visibility across project
modules.

The model reports project-shape diagnostics before later phases run, including:

- missing project files
- unreadable source files
- duplicate module names
- unknown modules requested by callers

## Data passed forward

The project model produces package and module metadata:

- package name and version
- project root
- module name
- source path
- source file ID
- dependency requirements

Source file IDs are stable within a loaded project. Diagnostics use paths when
no source span exists and normal spans once a source file is available.

## Fixtures

`fixtures/projects/scalar_app` is the small project fixture for current compiler
behavior. It should grow as language support grows. A larger long-term fixture
should exercise real Gleam project structure, dependencies, UI code, records,
custom types, and browser-facing Wasm.

## Reference

- Gleam, `gleam.toml` documentation: https://gleam.run/writing-gleam/gleam-toml/
