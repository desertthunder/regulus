# Project model and modules tasks

## Goal

Compile projects instead of only isolated source strings.

## Tasks

- [x] Read `gleam.toml` into a project configuration type.
- [x] Discover source files under project source directories.
- [x] Map source files to Gleam module names.
- [x] Assign stable source file IDs across a project.
- [x] Build a package/module graph for project modules.
- [x] Add dependency package metadata hooks.
- [x] Report duplicate modules and missing module files.
- [x] Keep single-file compilation available for tests.

## Done when

The compiler can load a small Gleam project and produce a module graph with
stable source IDs and useful project diagnostics.
