# Usage

Regulus has two compiler paths:

- `build` compiles a Gleam project from `gleam.toml` into one linked Wasm
  artifact.
- `compile` compiles one `.gleam` file for tests, fixtures, and small examples.

Most users should start with `build`. Use `compile` when you want the smallest
possible compiler input or when you are debugging one source file.

## Common commands

```sh
reggie build
reggie build path/to/project
reggie compile path/to/module.gleam
reggie run path/to/module.gleam
```

`run` is a single-file helper that compiles a module in memory and executes one
exported function with Wasmtime.

## Learn more

- [CLI commands and flags](./cli.md)
- [Examples](../examples.md)
- [Supported subset](../../reference/supported-subset.md)
