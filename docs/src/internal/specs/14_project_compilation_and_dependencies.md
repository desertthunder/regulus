# Project compilation and dependencies

Regulus currently compiles one source file at a time. Project loading can read
`gleam.toml` and discover modules, but it does not yet compile a linked project
artifact.

This spec tracks the next compiler milestone: compile a normal Gleam project
from `gleam.toml` into deterministic Wasm.

## Goal

Project compilation should run the same explicit phases as single-file
compilation, but across all modules in dependency order:

```text
load project -> parse -> target select -> resolve -> type check -> lower ->
link IR -> emit Wasm
```

The output should be one linked Wasm module for the selected target. Debug dumps
may include per-module views, but the final artifact should behave like one
compiled project.

## Command-line interface

Project compilation should use a high-level build command, while preserving the
existing single-file command as a lower-level compiler entry point:

```sh
gleam-wasm build [PROJECT]
gleam-wasm compile [OPTIONS] <INPUT.gleam>
```

`build` should feel like Go or Cargo. With no `PROJECT`, it builds the current
directory. A directory argument builds the project rooted there. A `gleam.toml`
argument builds the project that owns that manifest. The command should discover
`gleam.toml`; users should not need to pass a special project flag.

`compile` should remain the rustc-like single-file path for tests, fixtures, and
small examples. It should not load `gleam.toml` unless a later explicit option
requires that behavior.

Initial build flags should be familiar and shared with single-file compilation
where they mean the same thing:

- `-o, --output <FILE>` writes the final `.wasm` artifact to an exact path.
- `--out-dir <DIR>` writes compiler-named artifacts into a directory.
- `--target <TARGET>` selects `wasmtime`, `browser`, or `wasi`.
- `--emit <KIND[,KIND]>` selects artifacts such as `wasm`, `wat`, `ast`,
  `resolved`, `typed`, and `ir`.
- `--dump-dir <DIR>` may remain as a compatibility alias for debug dumps.
- `-v, --verbose` prints modules as they are compiled.
- `--json` may later emit machine-readable status and diagnostics.

Inspection should stay separate from building. The existing `project` should
can continue to print discovered modules as a `list` command.

## Project inputs

The build command should read enough project metadata to drive compilation:

- package name and version
- source roots
- target selection
- dependency entries
- dev-dependency entries when explicitly requested
- stable source file IDs
- module names and source paths

Diagnostics should report missing project files, unreadable source files,
duplicate modules, invalid module names, and unsupported configuration with file
paths and source spans where possible.

## Module graph

Project compilation needs a deterministic module graph:

1. Discover project modules under source roots.
2. Parse every selected module.
3. Apply target-group filtering before name resolution.
4. Resolve imports against project modules, dependency interfaces, stdlib
   interfaces, and the prelude.
5. Type-check modules in dependency order.
6. Lower typed modules to IR.
7. Link IR declarations into one backend module.

Same-project calls should never become host imports. Unknown dependency calls
should fail before lowering or byte emission.

## Dependencies

Dependency support should grow in layers.

### Interface-only dependencies

The first layer can load dependency metadata and module interfaces without
compiling dependency source. This supports type checking and clear diagnostics
for examples that keep dependency behavior behind intrinsics or host adapters.

### Source dependencies

The second layer should load selected Hex and path dependency source modules and
compile them through the same pipeline as project modules. This is preferred
when the package code uses the supported language subset.

### Unsupported dependencies

Unsupported dependency members, modules, syntax, runtime primitives, or ABI
shapes should produce source-spanned diagnostics before backend emission.

## Linking

The linker should combine lowered project and compiled dependency modules into a
single backend module. It should:

- preserve public export decisions
- rewrite same-project calls to direct or indirect internal calls
- keep dependency interface calls distinct from host imports in debug dumps
- reject duplicate generated names deterministically
- preserve source spans for linked declarations and diagnostics

## Generated names

The next project-compilation milestone is a deterministic generated-name scheme
for linked modules. The current linked project path can only compile modules
whose lowered names do not collide. A real project linker must assign backend
names from stable package, module, member, and helper identity before Wasm
emission.

The scheme should cover:

- project functions, constants, constructors, and type helpers
- anonymous and lifted functions, including closure helpers
- runtime and stdlib helper functions
- dependency package members, once dependency source loading is enabled
- host imports and module imports without changing their ABI names
- public exports, which should keep user-facing export names intentional

Generated names should be deterministic across platforms and filesystem order.
They should avoid collisions between modules such as `app/main.gleam` and
`test/main.gleam`, between dependencies with the same module names, and between
compiler-generated helpers and user declarations.

The linker should rewrite every same-project reference to the generated backend
name and report any remaining collision as a source-spanned project diagnostic.
Debug dumps should show both source names and generated names so users can trace
linked calls without treating same-project members as host imports.

## Artifacts

Project compilation should produce deterministic artifact names. With `-o`, the
final `.wasm` path is exact. With `--out-dir`, names should be derived from the
package and selected artifact kind. Without either flag, the default output path
should be deterministic and documented.

Supported artifacts should grow behind `--emit`:

- final project `.wasm`
- optional project `.wat`
- optional per-module debug dumps
- optional linked IR or import/export metadata

Failed compilation should not leave partial final artifacts unless the user
explicitly requested debug dumps.

## Active tasks

See the project compilation task list.
