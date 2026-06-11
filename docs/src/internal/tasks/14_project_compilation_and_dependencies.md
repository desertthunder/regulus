# Project compilation and dependencies tasks

## Goal

Compile a normal Gleam project from `gleam.toml` into one linked Wasm artifact.

## Tasks

### Project command

- [ ] Add `build [PROJECT]` as the high-level project compilation command.
- [ ] Make `build` with no `PROJECT` load the current project.
- [ ] Accept project directories and `gleam.toml` paths as `build` inputs.
- [ ] Keep `compile <INPUT.gleam>` as the single-file path for tests and
      examples.
- [ ] Share familiar flags where meanings match: `-o, --output`, `--out-dir`,
      `--target`, `--emit`, `--dump-dir`, `-v, --verbose`, and future `--json`.
- [ ] Replace the existing `project` command with `list` command
- [ ] Load package metadata, source roots, target settings, and dependencies.
- [ ] Assign stable source file IDs for every project module.
- [ ] Report missing project files, unreadable files, duplicate modules, and
      invalid module names with clear diagnostics.

### Module pipeline

- [ ] Discover all selected project modules in deterministic order.
- [ ] Parse every module before cross-module resolution.
- [ ] Apply target-group filtering before name resolution.
- [ ] Resolve project imports, stdlib imports, dependency interfaces, and
      prelude names through one module-interface path.
- [ ] Type-check modules in dependency order.
- [ ] Lower every typed project module to IR.
- [ ] Preserve source paths and spans through project diagnostics.

### Linking

- [ ] Link same-project module calls without treating them as host imports.
- [ ] Link lowered module IR into one backend module.
- [ ] Preserve deterministic generated names for functions, constructors,
      helpers, imports, and exports.
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

- [ ] Make `-o, --output` write the exact final `.wasm` path.
- [ ] Make `--out-dir` write deterministic compiler-named artifacts.
- [ ] Define the default output path for `build` without `-o` or `--out-dir`.
- [ ] Keep generated artifact names deterministic for multi-module projects.
- [ ] Support `--emit` values for `wasm`, `wat`, and useful debug artifacts.
- [ ] Emit optional per-module debug dumps for AST, resolved AST, typed output,
      IR, and WAT where useful.
- [ ] Avoid writing partial final artifacts after failed compilation unless the
      user explicitly requested debug dumps.

## Done when

A user can compile a `gleam.toml` project with multiple modules and receive one
linked Wasm artifact or stable, source-rendered diagnostics.
