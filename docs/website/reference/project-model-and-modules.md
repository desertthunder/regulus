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
dependency hooks. Selected Hex and path dependency sources can be loaded from
this metadata when they fit the supported compiler subset.

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
- selected loaded dependency source modules

Source file IDs are stable within a loaded project. Diagnostics use paths when
no source span exists and normal spans once a source file is available.

## Fixtures and Examples

`fixtures/projects/scalar_app` is the small project fixture for current compiler
behavior. It should grow as language support grows.

The checked examples cover usable project shapes:

- `examples/scalar_project` is the smallest working project build.
- `examples/multi_module_project` exercises same-project imports.
- `examples/diagnostics/duplicate_modules` shows a project-shape diagnostic.

JS target adapter emission is covered by building `examples/scalar_project`
with a JS target such as `--target browser`, avoiding a duplicate scalar
project.

A larger long-term fixture should exercise real Gleam project structure,
dependencies, UI code, records, custom types, and browser-facing Wasm.

## Reference

- Gleam, `gleam.toml` documentation: https://gleam.run/writing-gleam/gleam-toml/
