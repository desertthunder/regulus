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

## Global options

Pass `--no-color` before or after a subcommand to disable ANSI color in
human-readable output:

```sh
reggie --no-color build examples/scalar_project
reggie build examples/scalar_project --no-color
```

Regulus also disables ANSI color when the `NO_COLOR` environment variable is
set[^no-color]

## Build a project

Use `build` for Gleam projects with a `gleam.toml` file.

```sh
reggie build
reggie build examples/scalar_project
reggie build examples/scalar_project/gleam.toml
```

With no project argument, the current directory is used. A directory argument
builds that project root. A `gleam.toml` argument builds the project that owns
that manifest.

Project builds write `build/<package>.wasm` by default. Use `--output` to
choose an exact final path or `--out-dir` to write compiler-named artifacts into
a directory.

Example:

```sh
reggie --no-color build examples/scalar_project --out-dir build/docs
```

```text
Resolving dependencies
wasm build/docs/scalar_project.wasm (63 bytes)
```

See [Project compilation and dependencies][project-compilation] for dependency
loading, linked output, and current project limits.

## Compile one file

Use `compile` for a single `.gleam` file. This path is useful for small tests,
fixtures, and compiler debugging.

```sh
reggie compile path/to/module.gleam
```

By default, single-file compilation writes a `.wasm` file next to the input.
`--output` and `--out-dir` work the same way as project builds.

Single-file compilation exists for fixtures, small examples, and compiler
debugging. Project compilation should use `build` so module discovery,
dependencies, and linked output all follow the project model.

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

Example:

```sh
reggie run examples/scalar_project/src/main.gleam --function add_one 41
```

```text
42
```

Scalar arguments use the low-level Wasm ABI. `Int` values are passed as `i64`,
`Float` values as `f64`, and `Bool` values as `i32`. Return values are rendered
for scalars and supported managed values such as strings, tuples, lists,
records, `Result`, and `Option`. Programs can also print strings through
`gleam/io.print` and `gleam/io.println` when targeting Wasmtime.

## Targets

Both `build` and `compile` accept `--target`:

```sh
reggie build examples/scalar_project --target browser
reggie build examples/scalar_project --target bundler
reggie compile path/to/module.gleam --target wasmtime
```

Supported target values are `wasmtime`, `browser`, `bundler`, `nodejs`, and
`wasi`. Project builds use the target from `gleam.toml` when `--target` is not
provided.

`browser`, `bundler`, and `nodejs` emit deterministic `.mjs` adapters next to
the `.wasm` artifact when Wasm output is requested. The adapters load the Wasm
module, check imports, convert scalar and string calls, and read supported
structured export results.

Example:

```sh
reggie --no-color build examples/scalar_project --target nodejs --out-dir build/node
```

```text
Resolving dependencies
wasm build/node/scalar_project.wasm (2651 bytes)
js build/node/scalar_project.mjs
```

Target-specific externals are checked before Wasm assembly. If a source file
imports a browser or Node.js host module while compiling for Wasmtime, the CLI
reports the target mismatch with a source label and recovery note.

## Artifacts

`--emit` selects emitted artifact kinds:

```sh
reggie build examples/scalar_project --emit wasm,wat
reggie build examples/scalar_project --emit wat,ast,resolved,typed,ir
reggie build examples/scalar_project --emit runtime,abi
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
| `runtime`  | Runtime layout and object tag summary.  |
| `abi`      | Import/export ABI boundary summary.     |

`wasm` is the default. `wat` writes next to the Wasm output, or into
`--out-dir` when that option is used. Debug emit values write deterministic
files beside the selected output path unless `--dump-dir` is set.

Example:

```sh
reggie --no-color build examples/scalar_project --out-dir build/debug --emit wasm,wat
```

```text
Resolving dependencies
wasm build/debug/scalar_project.wasm (63 bytes)
wat build/debug/scalar_project.wat
```

Use `--dump-dir` to write all compiler debug dumps into a separate directory:

```sh
reggie build examples/multi_module_project --dump-dir build/dumps
```

Single-file dumps include AST, resolved AST, typed output, IR, WAT, runtime
layout, and ABI output. Project dumps include per-module AST, resolved AST,
typed output, linked IR, WAT, runtime layout, and ABI output.

If compilation fails, Regulus does not write the final Wasm artifact. Debug
artifacts are only written when the requested compiler phase completes.

The backend emits final Wasm bytes from its structured module. WAT is rendered
from that module for debugging and snapshots; it is not the source of truth for
the final `.wasm` artifact.

## Diagnostics and exit codes

Successful commands exit with status code `0`. Compilation diagnostics and
project loading errors exit with a non-zero status code. Command misuse, such
as an unknown flag or invalid subcommand, is reported by the CLI argument
parser and also exits non-zero.

Human diagnostics include file paths, source snippets when a span is available,
labels, and notes. Project diagnostics are grouped in a stable order across
modules.

Missing project manifests are reported with the path Regulus tried to load:

```sh
reggie --no-color build /tmp/not-a-regulus-project
```

```text
error could not load project /tmp/not-a-regulus-project
diagnostic ProjectError: project manifest not found at /tmp/not-a-regulus-project/gleam.toml
  note: pass a project directory or a path to gleam.toml
```

Duplicate modules include both conflicting source paths:

```sh
reggie --no-color build examples/diagnostics/duplicate_modules
```

```text
error could not load project examples/diagnostics/duplicate_modules
diagnostic ProjectError: duplicate module `app` in examples/diagnostics/duplicate_modules/src/app.gleam and examples/diagnostics/duplicate_modules/test/app.gleam
  note: each module name must be unique across src and test
```

## Inspect one source file

Use `debug` when changing parser or AST support for one `.gleam` file. The
`dbg` command is an alias.

```sh
reggie debug ts path/to/module.gleam
reggie debug spans path/to/module.gleam
reggie debug ast path/to/module.gleam
reggie debug json path/to/module.gleam --ast --spans
```

The debug views are:

| View    | Output                                               |
| ------- | ---------------------------------------------------- |
| `ts`    | Tree-sitter concrete syntax tree S-expression.       |
| `spans` | Tree-sitter nodes with spans, positions, and fields. |
| `ast`   | Regulus AST built from the tree-sitter tree.         |
| `json`  | Selected debug views as JSON.                        |

`spans` is for tree-sitter nodes. `json --spans` includes tree-sitter span
details. If no `json` view flags are passed, `json` defaults to tree-sitter
output.

By default, debug/dbg require a source file and at least one view flag.

## List project modules

Use `list` to inspect discovered modules without building artifacts.

```sh
reggie --no-color list examples/multi_module_project
```

```text
project multi_module_project 1.0.0 (2 modules)
module main -> examples/multi_module_project/src/main.gleam
module math -> examples/multi_module_project/src/math.gleam
```

## Current limitations

Project compilation is still growing. Broad Hex dependency language coverage
and additional host APIs are documented in the development and reference docs
as they stabilize.

[project-compilation]: ../../reference/compiling-projects.md

[^no-color]: https://no-color.org/
