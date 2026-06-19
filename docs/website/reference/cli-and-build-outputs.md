# CLI and build outputs

The CLI gives contributors a small way to run the compiler pipeline and inspect
intermediate representations without adding test-only entry points.

## Commands

`reggie build [project]` compiles a Gleam project from `gleam.toml`.
With no argument it builds the current directory. A directory argument builds
that project root, and a `gleam.toml` argument builds the owning project.

The command writes `build/<package>.wasm` by default. `--output` writes the
final Wasm artifact to an exact path. `--out-dir` writes compiler-named
artifacts such as `<package>.wasm` and `<package>.wat` into the given
directory. `--emit` accepts comma-separated artifact kinds: `wasm`, `wat`,
`ast`, `resolved`, `typed`, `ir`, `runtime`, and `abi`.
`--wat` is a compatibility alias for emitting WAT, matching `compile`.
Passing `--wat` without a path uses the `.wat` path matching the output file.

See [Project compilation and dependencies][project-compilation] for dependency
loading, linked output, and current project limits.

`reggie compile <input>` compiles one Gleam source file. It runs the same
single-file pipeline used by tests:

```text
source -> parse -> AST -> resolved AST -> typed module -> IR -> WAT -> Wasm
```

The command writes a `.wasm` file next to the input unless `--output` or
`--out-dir` is set. `--wat` remains a compatibility alias for emitting WAT.
Passing `--wat` without a path uses the `.wat` path matching the output file.

`reggie list [project]` loads a project and prints discovered modules. It is
an inspection command and does not write artifacts.

## Examples

The `examples/` directory follows this command behavior:

- `examples/scalar_project` builds the smallest project Wasm artifact.
- `examples/multi_module_project` builds linked same-project modules.
- `examples/diagnostics/duplicate_modules` documents a failing project shape.

Use `examples/scalar_project --target browser`, `--target bundler`, or
`--target nodejs` to check JS adapter emission without duplicating the scalar
project example.

See `docs/website/guide/usage/cli.md` for a usage guide/manual.

## Targets

`compile` and `build` accept `--target wasmtime`, `--target browser`,
`--target bundler`, `--target nodejs`, and `--target wasi`. Wasmtime is the
default for `compile`. When `build` omits `--target`, it uses the project
target from `gleam.toml`.

The `browser`, `bundler`, and `nodejs` targets write a `.mjs` adapter next to
the `.wasm` artifact when Wasm output is requested. The adapter is the current
checked JavaScript host path. It loads the Wasm module, exposes ABI metadata,
checks imports, converts scalar and string calls, reads supported structured
export results, and keeps opaque JavaScript handles in an adapter-owned table.

Target selection filters target-group declarations before later compiler phases
and checks that host imports are valid for the selected target.

## Debug dumps

`--dump-dir <dir>` writes deterministic debug files. Single-file compilation
writes AST, resolved AST, typed output, IR, WAT, runtime layout, and ABI dumps.
Project builds write per-module AST, resolved AST, typed output, linked IR,
WAT, runtime layout, and ABI dumps. The `runtime` and `abi` emit kinds write
runtime layout and ABI boundary summaries without requiring a full dump
directory.

These dumps are for contributor inspection. Normal CLI output stays focused on
the final artifact path, optional WAT path, and diagnostics.

## Exit behavior

The CLI returns success after writing requested artifacts. It returns failure
for unreadable input, compiler diagnostics, or artifact write errors.

[project-compilation]: ./compiling-projects.md
