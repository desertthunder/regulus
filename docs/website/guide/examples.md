# Examples

The `examples/` directory contains checked projects and fixtures that show the
compiler's current surface area.

## Working examples

```sh
reggie build examples/scalar_project
reggie build examples/multi_module_project
reggie build examples/browser_scalar --target browser
```

## Example projects

| Example | Purpose |
| --- | --- |
| `examples/scalar_project` | Smallest normal project build. |
| `examples/multi_module_project` | Same-project imports and linked output. |
| `examples/browser_scalar` | Browser-target Wasm with host glue. |
| `examples/diagnostics/duplicate_modules` | Intentional project diagnostic. |

Diagnostic examples are expected to fail before Wasm emission.

## Roadmap examples

No roadmap examples are checked in yet. When added, they should stay separate
from working examples so users can tell supported behavior from planned
behavior.

## Fixture workflow

Use `fixtures/` for focused compiler behavior. End-to-end fixtures should stay
small and should exercise one feature or diagnostic at a time.

```sh
cargo test -p compiler_core
```
