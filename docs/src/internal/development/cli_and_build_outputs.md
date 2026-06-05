# CLI and build outputs

The CLI gives contributors a small way to run the compiler pipeline and inspect
intermediate representations without adding test-only entry points.

## Commands

`gleam-wasm compile <input>` compiles one Gleam source file. It runs the same
pipeline used by tests:

```text
source -> parse -> AST -> resolved AST -> typed module -> IR -> WAT -> Wasm
```

The command writes a `.wasm` file next to the input unless `--output` is set.
`--wat` also writes the generated WebAssembly text format. Passing `--wat`
without a path uses the `.wat` path matching the output file.

`gleam-wasm project <input>` loads a project directory containing `gleam.toml`
and prints the discovered modules. It is an inspection command today; linked
project compilation is still future work.

## Targets

`compile` accepts `--target wasmtime`, `--target browser`, and `--target wasi`.
Wasmtime is the default. Browser and WASI selection currently records intent
and uses the generic backend until target-specific adapters are implemented.

## Debug dumps

`--dump-dir <dir>` writes deterministic debug files:

- `ast.txt`
- `resolved.txt`
- `typed.txt`
- `ir.txt`
- `wat.wat`

These dumps are for contributor inspection. Normal CLI output stays focused on
the final artifact path, optional WAT path, and diagnostics.

## Exit behavior

The CLI returns success after writing requested artifacts. It returns failure
for unreadable input, compiler diagnostics, or artifact write errors.
