# Usage

Regulus provides two compiler entry points: project builds and single-file
compilation.

## Build a project

Use `build` for normal Gleam projects with a `gleam.toml` file.

```sh
gleam-wasm build
gleam-wasm build examples/scalar_project
gleam-wasm build examples/scalar_project/gleam.toml
```

With no project argument, the current directory is used. A directory argument
builds that project root. A `gleam.toml` argument builds the project that owns
that manifest.

By default, project builds write:

```text
build/<package>.wasm
```

Use `--output` to choose the exact final Wasm path:

```sh
gleam-wasm build examples/scalar_project --output out/app.wasm
```

Use `--out-dir` to write compiler-named artifacts into a directory:

```sh
gleam-wasm build examples/scalar_project --out-dir build/examples
```

## Compile one file

Use `compile` for a single `.gleam` file. This path is useful for small tests,
fixtures, and compiler debugging.

```sh
gleam-wasm compile path/to/module.gleam
```

By default, single-file compilation writes a `.wasm` file next to the input.
`--output` and `--out-dir` work the same way as project builds.

## Targets

Both `build` and `compile` accept `--target`:

```sh
gleam-wasm build examples/browser_scalar --target browser
gleam-wasm compile path/to/module.gleam --target wasmtime
```

Supported target values are `wasmtime`, `browser`, and `wasi`. Project builds
use the target from `gleam.toml` when `--target` is not provided.

## Artifacts and debug dumps

`--emit` selects emitted artifact kinds:

```sh
gleam-wasm build examples/scalar_project --emit wasm,wat
```

`wasm` is the normal binary output. `wat` writes WebAssembly text next to the
Wasm output, or into `--out-dir` when that option is used.

`--dump-dir` writes compiler debug dumps for contributor inspection:

```sh
gleam-wasm build examples/multi_module_project --dump-dir build/dumps
```

Single-file dumps include AST, resolved AST, typed output, IR, and WAT. Project
build dumps currently include linked IR and WAT.

## List project modules

Use `list` to inspect discovered modules without building artifacts.

```sh
gleam-wasm list examples/multi_module_project
```

## Examples

The `examples/` directory contains checked examples:

- `examples/scalar_project`: smallest working project build.
- `examples/multi_module_project`: same-project imports and linking.
- `examples/browser_scalar`: browser-target Wasm plus minimal JS host glue.
- `examples/diagnostics/duplicate_modules`: intentional project diagnostic.

Working examples should compile with `gleam-wasm build`. Diagnostic examples
are expected to fail before Wasm emission.

## Current limitations

Project compilation is still growing. Dependency interfaces, dependency source
loading, general external functions, and richer host ABI adapters are tracked in
the inflight work section of this book.
