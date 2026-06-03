# Project model and modules

A real compiler must compile Gleam projects, not only isolated source strings.
The project model is responsible for discovering modules, reading package
configuration, and giving later work a complete view of source files and package
dependencies.

## Gleam project configuration

Project configuration is read from `gleam.toml`. The compiler currently models
the package fields documented by Gleam, including package `name`, `version`,
`description`, `licences`, `repository`, `links`, `gleam`, `target`,
`dependencies`, and `dev-dependencies`.

Dependency entries may be plain version requirements or option tables containing
metadata such as `version`, `path`, or `git`. The project model records these as
dependency hooks; dependency source loading can be added after the root project
model is stable.

## Responsibilities

- Read `gleam.toml` and project source directories.
- Assign stable source file IDs to every module.
- Map module names to source files.
- Discover dependency package metadata.
- Report duplicate modules and missing modules.
- Support single-file tests without requiring a full project.

## Data model

The project model produces a package graph with module metadata:

- package name and version
- project root
- module name
- source path
- source file ID
- dependency requirements

## Fixture direction

`fixtures/projects/scalar_app` is the small project fixture that matches what the
compiler can do today. It should expand as language support grows. The long-term
sample project should be a Lustre app, because that exercises real Gleam project
structure, dependencies, UI code, records, custom types, and browser-facing WASM.

## Diagnostics

Project diagnostics should use file paths when no source span exists. Once a
source file is known, diagnostics should prefer normal source spans.

## Reference

- Gleam, `gleam.toml` documentation: https://gleam.run/writing-gleam/gleam-toml/
