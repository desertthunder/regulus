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
reggie build examples/scalar_project --target browser
reggie build examples/scalar_project --target bundler
reggie compile path/to/module.gleam --target wasmtime
```

Supported target values are `wasmtime`, `browser`, `bundler`, `nodejs`, and
`wasi`. Project builds use the target from `gleam.toml` when `--target` is not
provided.

`bundler` emits a deterministic `.mjs` adapter next to the `.wasm` artifact
when Wasm output is requested. That adapter loads the Wasm module, checks
imports, converts scalar and string calls, and reads supported structured
export results. `browser` and `nodejs` are accepted targets, but their complete
host glue and profile-specific APIs are still in progress.

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

Use `--dump-dir` to write all compiler debug dumps into a separate directory:

```sh
reggie build examples/multi_module_project --dump-dir build/dumps
```

Single-file dumps include AST, resolved AST, typed output, IR, WAT, runtime
layout, and ABI output. Project dumps include per-module AST, resolved AST,
typed output, linked IR, WAT, runtime layout, and ABI output.

If compilation fails, Regulus does not write the final Wasm artifact. Debug
artifacts are only written when the requested compiler phase completes.

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
reggie list examples/multi_module_project
```

## Current limitations

Project compilation is still growing. Broad Hex dependency language coverage,
bodyless externals from dependency source, and richer host ABI adapters are
tracked in `docs/internal`.

[project-compilation]: ../../reference/compiling-projects.md
