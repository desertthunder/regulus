# Project model and modules tasks

## Goal

Compile projects instead of only isolated source strings.

## Tasks

- [ ] Read `gleam.toml` into a project configuration type.
- [ ] Discover source files under project source directories.
- [ ] Map source files to Gleam module names.
- [ ] Assign stable source file IDs across a project.
- [ ] Build a package/module graph for project modules.
- [ ] Add dependency package metadata hooks.
- [ ] Report duplicate modules and missing module files.
- [ ] Keep single-file compilation available for tests.

## Done when

The compiler can load a small Gleam project and produce a module graph with
stable source IDs and useful project diagnostics.
