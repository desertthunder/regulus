# CLI and build outputs

The CLI gives contributors a small way to run the compiler pipeline and inspect
intermediate representations without adding test-only entry points.

## Commands

`gleam-wasm build [project]` compiles a Gleam project from `gleam.toml`.
With no argument it builds the current directory. A directory argument builds
that project root, and a `gleam.toml` argument builds the owning project.

The command writes `build/<package>.wasm` by default. `--output` writes the
final Wasm artifact to an exact path. `--out-dir` writes compiler-named
artifacts such as `<package>.wasm` and `<package>.wat` into the given
directory. `--emit` accepts comma-separated artifact kinds: `wasm`, `wat`,
`ast`, `resolved`, `typed`, and `ir`.

`gleam-wasm compile <input>` compiles one Gleam source file. It runs the same
single-file pipeline used by tests:

```text
source -> parse -> AST -> resolved AST -> typed module -> IR -> WAT -> Wasm
```

The command writes a `.wasm` file next to the input unless `--output` or
`--out-dir` is set. `--wat` remains a compatibility alias for emitting WAT.
Passing `--wat` without a path uses the `.wat` path matching the output file.

`gleam-wasm list [project]` loads a project and prints discovered modules. It is
an inspection command and does not write artifacts.

## Examples

The `examples/` directory keeps user-facing projects aligned with this CLI
contract:

- `examples/scalar_project` builds the smallest project Wasm artifact.
- `examples/multi_module_project` builds linked same-project modules.
- `examples/browser_scalar` builds a browser-target artifact with JS glue.
- `examples/diagnostics/duplicate_modules` documents a failing project shape.

See `docs/website/guide/usage/cli.md` for user-facing commands.

## Targets

`compile` accepts `--target wasmtime`, `--target browser`, and `--target wasi`.
Wasmtime is the default. `build` accepts the same `--target` values; when
omitted it uses the project target from `gleam.toml`.

Target selection filters target-group declarations before later compiler phases
and configures backend host import validation.

## Debug dumps

`--dump-dir <dir>` writes deterministic debug files. Single-file compilation
writes AST, resolved AST, typed output, IR, and WAT dumps. Project builds write
per-module AST, resolved AST, typed output, linked IR, and WAT dumps.

These dumps are for contributor inspection. Normal CLI output stays focused on
the final artifact path, optional WAT path, and diagnostics.

## Exit behavior

The CLI returns success after writing requested artifacts. It returns failure
for unreadable input, compiler diagnostics, or artifact write errors.
