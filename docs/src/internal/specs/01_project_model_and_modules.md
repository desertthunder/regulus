# Project model and modules

A real compiler must compile Gleam projects, not only isolated source strings.
The project model is responsible for discovering modules, reading package
configuration, and giving later work a complete view of source files and package
dependencies.

## Responsibilities

- Read `gleam.toml` and project source directories.
- Assign stable source file IDs to every module.
- Map module names to source files.
- Discover dependency packages and their module interfaces.
- Report duplicate modules and missing modules.
- Support single-file tests without requiring a full project.

## Data model

The project model should produce a package graph with module metadata:

- package name and version where available
- source root
- module name
- source path
- source file ID
- dependency edges between packages

## Diagnostics

Project diagnostics should use file paths when no source span exists. Once a
source file is known, diagnostics should prefer normal source spans.
