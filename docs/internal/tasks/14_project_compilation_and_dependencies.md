# Project compilation and dependencies tasks

## Goal

Compile a normal Gleam project from `gleam.toml` into one linked Wasm artifact.

## Tasks

### Project command

- [x] Add `build [PROJECT]` as the high-level project compilation command.
- [x] Make `build` with no `PROJECT` load the current project.
- [x] Accept project directories and `gleam.toml` paths as `build` inputs.
- [x] Keep `compile <INPUT.gleam>` as the single-file path for tests and
      examples.
- [x] Share familiar flags where meanings match: `-o, --output`, `--out-dir`,
      `--target`, `--emit`, `--dump-dir`, `-v, --verbose`, and future `--json`.
- [x] Replace the existing `project` command with `list` command
- [x] Load package metadata, source roots, target settings, and dependencies.
- [x] Assign stable source file IDs for every project module.
- [ ] Report missing project files, unreadable files, duplicate modules, and
      invalid module names with clear diagnostics.

### Generated-name scheme

The goal here is to make linked project names deterministic and collision-free.

- [x] Define backend name data types for package, module, member, helper kind,
      compiler-generated index, export names, and host import ABI names.
- [x] Add a central backend-name renderer that escapes components without
      punctuation-sanitization collisions.
- [x] Assign backend names from structured compiler identity instead of raw
      lowered declaration strings.
- [x] Preserve intentional public export names separately from internal backend
      names during emission.
- [x] Rename project functions, constants, constructors, lifted closures, and
      helper functions during linking.
- [x] Rewrite same-project direct calls, function values, constructor names,
      record update constructors, and debug references to generated names.
- [x] Keep host import and module import ABI names stable while namespacing
      compiler-owned wrapper functions.
- [x] Detect generated-name collisions deterministically and report the source
      declarations that caused them.
- [x] Show source names and generated names in linked IR debug dumps.
- [ ] Add fixtures for duplicate function names in different modules, duplicate
      module basenames, dependency module name overlap, and lifted closures.

### Examples

- [x] Add small working project examples for scalar functions, same-project
      imports, and browser-target scalar Wasm.
- [x] Add a diagnostic example for duplicate module names across source roots.
- [x] Document example commands in the user-facing usage page.
- [x] Reference examples from internal project-model and CLI development docs.
- [ ] Add compile or CLI integration coverage for working examples.
- [ ] Add snapshot coverage for intentional diagnostic examples.
- [ ] Keep examples categorized as working, diagnostic, or roadmap examples.

### Module pipeline

- [x] Discover all selected project modules in deterministic order.
- [x] Parse every module before cross-module resolution.
- [x] Apply target-group filtering before name resolution.
- [ ] Resolve project imports, stdlib imports, dependency interfaces, and
      prelude names through one module-interface path.
- [x] Type-check modules in dependency order.
- [x] Lower every typed project module to IR.
- [x] Preserve source paths and spans through project diagnostics.

### Linking

- [x] Link same-project module calls without treating them as host imports.
- [x] Link lowered module IR into one backend module.
- [ ] Keep dependency interface calls distinct from host imports in debug dumps.
- [ ] Add project fixtures for cross-module calls, constructors, records,
      patterns, and module-private members.

### Dependency interfaces

- [ ] Load enough dependency metadata for project compile inputs.
- [ ] Load dependency module interfaces for values, types, constructors, and
      labels used by project compilation.
- [ ] Report unsupported dependency members before lowering.
- [ ] Add tests for dependency interface lookup, visibility, generic schemes,
      constructors, and unsupported members.

### Dependency source loading

- [ ] Define selected Hex and path dependency source loading rules.
- [ ] Add a dependency source loader for selected packages and paths.
- [ ] Compile one package module from source through the normal pipeline.
- [ ] Link compiled dependency module calls without treating them as host
      imports.
- [ ] Add a fixture that links one dependency function without modeling it as an
      intrinsic or host import.

### Project artifacts

- [x] Make `-o, --output` write the exact final `.wasm` path.
- [x] Make `--out-dir` write deterministic compiler-named artifacts.
- [x] Define the default output path for `build` without `-o` or `--out-dir`.
- [x] Keep generated artifact names deterministic for multi-module projects.
- [ ] Support `--emit` values for `wasm`, `wat`, and useful debug artifacts.
- [ ] Emit optional per-module debug dumps for AST, resolved AST, typed output,
      IR, and WAT where useful.
- [ ] Avoid writing partial final artifacts after failed compilation unless the
      user explicitly requested debug dumps.

## Done when

A user can compile a `gleam.toml` project with multiple modules and receive one
linked Wasm artifact or stable, source-rendered diagnostics.
