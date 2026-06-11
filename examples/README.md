# Examples

Examples are small project directories used by users and contributors.

## Working examples

- `scalar_project`: smallest project build with scalar exported functions.
- `multi_module_project`: same-project import and linked project build.
- `browser_scalar`: browser-target build with minimal JS instantiation glue.

## Diagnostic examples

- `diagnostics/duplicate_modules`: duplicate module names across source roots.

Working examples should build with `gleam-wasm build`. Diagnostic examples are
expected to fail with clear diagnostics and should not reach Wasm emission.
