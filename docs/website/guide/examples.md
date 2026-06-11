# Examples

The `examples/` directory contains checked projects and fixtures that show the
compiler's current surface area.

## Working examples

```sh
gleam-wasm build examples/scalar_project
gleam-wasm build examples/multi_module_project
gleam-wasm build examples/browser_scalar --target browser
```

## Example projects

| Example | Purpose |
| --- | --- |
| `examples/scalar_project` | Smallest normal project build. |
| `examples/multi_module_project` | Same-project imports and linked output. |
| `examples/browser_scalar` | Browser-target Wasm with host glue. |
| `examples/diagnostics/duplicate_modules` | Intentional project diagnostic. |

Diagnostic examples are expected to fail before Wasm emission.

## Fixture workflow

Use `fixtures/` for focused compiler behavior. End-to-end fixtures should stay
small and should exercise one feature or diagnostic at a time.

```sh
cargo test -p compiler_core
```
