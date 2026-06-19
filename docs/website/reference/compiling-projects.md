# Project compilation and dependencies

Use `reggie build` for Gleam projects with a `gleam.toml` file. Project builds
compile supported project modules and selected dependency source modules into
one linked WebAssembly artifact.

```sh
reggie build
reggie build examples/scalar_project
reggie build examples/scalar_project/gleam.toml
```

With no project argument, Regulus builds the current directory. A directory
argument builds that project root. A `gleam.toml` argument builds the project
that owns that manifest.

## What build does

Project builds compile every selected module in dependency order:

```text
load project -> compile modules -> link modules -> emit Wasm
```

The final output is one linked Wasm module for the selected target. Per-module
views are available as debug dumps. Same-project calls are linked inside the
Wasm module rather than emitted as host imports.

## Project inputs

Regulus reads project metadata from `gleam.toml`, including:

- package name and version
- source roots
- target selection
- dependency entries
- module names and source paths

The loader discovers modules under the configured source roots and reports
project-shape diagnostics before later compiler phases run.

Project-shape diagnostics include:

- missing project files
- unreadable source files
- duplicate module names
- unknown imported modules
- unsupported project configuration

## Dependencies

Regulus supports selected Hex and path dependency source loading. Dependency
modules are compiled through the same compiler pipeline as project modules when
they fit the supported compiler subset.

Hex package metadata and tarballs are fetched from the public Hex repository
when needed. Downloaded tarballs are cached under the user Regulus store, and
packages are extracted into the project-local `build/packages/` directory.

Path dependencies are loaded from their declared paths without network access.

Project builds print stable dependency progress to stderr:

```text
Resolving dependencies
Using cached gleam_stdlib 0.50.0
Extracting gleam_stdlib 0.50.0
```

Use `--verbose` to include local paths, cache paths, source paths, URLs, and
checksums where available.

## Targets

Project builds accept the same target names as single-file compilation:

```sh
reggie build --target wasmtime
reggie build --target browser
reggie build --target bundler
reggie build --target nodejs
reggie build --target wasi
```

When `--target` is omitted, project builds use the target from `gleam.toml`.

Target selection picks the declarations for the requested runtime before name
resolution and checks that host imports are valid for that target.

## Artifacts

By default, project builds write:

```text
build/<package>.wasm
```

Use `--output` to choose the exact final Wasm path:

```sh
reggie build examples/scalar_project --output out/app.wasm
```

Use `--out-dir` to write compiler-named artifacts into a directory:

```sh
reggie build examples/scalar_project --out-dir build/examples
```

`--emit` selects artifact kinds:

```sh
reggie build examples/scalar_project --emit wasm,wat
reggie build examples/scalar_project --emit wat,ast,resolved,typed,ir
reggie build examples/scalar_project --emit runtime,abi
```

Supported project artifact kinds are:

| Value      | Output                                  |
| ---------- | --------------------------------------- |
| `wasm`     | Final WebAssembly binary.               |
| `wat`      | WebAssembly text for the linked module. |
| `ast`      | Per-module AST debug dumps.             |
| `resolved` | Per-module resolved AST debug dumps.    |
| `typed`    | Per-module typed-module debug dumps.    |
| `ir`       | Linked IR debug dump.                   |
| `runtime`  | Runtime layout and object tag summary.  |
| `abi`      | Import/export ABI boundary summary.     |

Use `--dump-dir` to write debug dumps into a separate directory. If
compilation fails, Regulus does not write the final Wasm artifact. Debug
artifacts are only written when the requested compiler phase completes.

For the `bundler` target, Regulus writes a deterministic `.mjs` adapter next to
the `.wasm` artifact when Wasm output is requested.

## Inspect modules

Use `list` to inspect discovered modules without compiling artifacts:

```sh
reggie list examples/multi_module_project
```

## Current limits

Project compilation is supported for the current compiler subset. These areas
are still incomplete:

- compiling every valid Gleam project shape
- broad dependency source compilation without subset limits
- bodyless `@external` declarations from project and dependency source
- full standard-library source compilation
- complete browser, Node.js, and WASI host adapters
- automatic conversion for every managed host import and export shape
