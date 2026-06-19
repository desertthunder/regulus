# Examples

Examples are small project directories used by users and contributors.

## Working examples

- `scalar_project`: smallest project build with scalar exported functions.
- `multi_module_project`: same-project import and linked project build.

## Diagnostic examples

- `diagnostics/duplicate_modules`: duplicate module names across source roots.

## Roadmap examples

No roadmap examples are checked in yet. Add future examples here only when they
document planned behavior rather than supported behavior.

Working examples should build with `reggie build`. Use `--target browser`,
`--target bundler`, or `--target nodejs` with `scalar_project` when checking JS
adapter emission. Diagnostic examples are expected to fail with clear
diagnostics and should not reach Wasm emission.
