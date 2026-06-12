# CLI

The CLI binaries are named `reggie` and `regulus`. When running from a
checkout, prefix commands with Cargo:

```sh
cargo run -q -p compiler_cli -- build
```

Installed binaries can be run directly. The docs use `reggie`, but `regulus`
is also available as an alias:

```sh
reggie build
regulus build
```

## Build a project

Use `build` for normal Gleam projects with a `gleam.toml` file.

```sh
reggie build
reggie build examples/scalar_project
reggie build examples/scalar_project/gleam.toml
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
reggie build examples/scalar_project --output out/app.wasm
```

Use `--out-dir` to write compiler-named artifacts into a directory:

```sh
reggie build examples/scalar_project --out-dir build/examples
```

Project builds print dependency progress to stderr. Quiet output keeps package
messages stable, while `--verbose` adds paths and other local details.

```text
Resolving dependencies
Using cached gleam_stdlib 0.50.0
Extracting gleam_stdlib 0.50.0
```

Path dependencies and selected package sources are loaded from the project graph
and can be compiled into the linked Wasm module when they use the supported
language subset.

## Compile one file

Use `compile` for a single `.gleam` file. This path is useful for small tests,
fixtures, and compiler debugging.

```sh
reggie compile path/to/module.gleam
```

By default, single-file compilation writes a `.wasm` file next to the input.
`--output` and `--out-dir` work the same way as project builds.

## Run one file

Use `run` to compile a single `.gleam` file and execute one exported function
with Wasmtime.

```sh
reggie run path/to/module.gleam
reggie run path/to/module.gleam --function answer
reggie run path/to/module.gleam --function add 1 2
```

`run` defaults to the `main` export. The `exec` command is an alias:

```sh
reggie exec path/to/module.gleam
```

Scalar arguments and return values use the low-level Wasm ABI. `Int` values are
passed as `i64`, `Float` values as `f64`, and `Bool` values as `i32`. Managed
values such as strings, lists, tuples, records, and custom types are pointers
into guest memory at the Wasm boundary. Programs can still print strings through
`gleam/io.print` and `gleam/io.println` when targeting Wasmtime.

## Targets

Both `build` and `compile` accept `--target`:

```sh
reggie build examples/browser_scalar --target browser
reggie compile path/to/module.gleam --target wasmtime
```

Supported target values are `wasmtime`, `browser`, and `wasi`. Project builds
use the target from `gleam.toml` when `--target` is not provided.

## Artifacts

`--emit` selects emitted artifact kinds:

```sh
reggie build examples/scalar_project --emit wasm,wat
reggie build examples/scalar_project --emit wat,ast,resolved,typed,ir
```

Supported emit values are:

| Value      | Output                                  |
| ---------- | --------------------------------------- |
| `wasm`     | Final WebAssembly binary.               |
| `wat`      | WebAssembly text for the linked module. |
| `ast`      | Per-module AST debug dumps.             |
| `resolved` | Per-module resolved AST debug dumps.    |
| `typed`    | Per-module typed-module debug dumps.    |
| `ir`       | Linked IR debug dump.                   |

`wasm` is the default. `wat` writes next to the Wasm output, or into
`--out-dir` when that option is used. Debug emit values write deterministic
files beside the selected output path unless `--dump-dir` is set.

Use `--dump-dir` to write all compiler debug dumps into a separate directory:

```sh
reggie build examples/multi_module_project --dump-dir build/dumps
```

Single-file dumps include AST, resolved AST, typed output, IR, and WAT. Project
dumps include per-module AST, resolved AST, typed output, linked IR, and WAT.

If compilation fails, Regulus does not write the final Wasm artifact. Debug
artifacts are only written when the requested compiler phase completes.

## List project modules

Use `list` to inspect discovered modules without building artifacts.

```sh
reggie list examples/multi_module_project
```

## Current limitations

Project compilation is still growing. Broad Hex dependency language coverage,
general external functions, and richer host ABI adapters are tracked in
`docs/internal`.
