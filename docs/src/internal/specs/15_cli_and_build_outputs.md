# Open CLI and build outputs

Current single-file CLI behavior is documented in
[CLI and build outputs](../development/cli_and_build_outputs.md). This spec
tracks the remaining work for project compilation and richer user-facing
artifacts.

## Remaining responsibilities

- Discover package/dependency metadata needed for project compilation.
- Render diagnostics with source snippets.
- Compile a Gleam project from `gleam.toml` into linked Wasm output.
- Choose concrete target adapters for Wasmtime, browser, and WASI.
- Keep generated artifact names deterministic for multi-module projects.
- Avoid partial final artifacts after failed project compilation.

## Artifacts

Suggested outputs:

- deterministic project and module `.wasm` files
- deterministic project and module `.wat` files
- debug dumps under a configurable directory
- test snapshots for compiler-owned representations

## Usability

Commands should be boring and predictable. Debug output should be opt-in, and
normal compilation should focus on the final artifact and diagnostics.

## Active tasks

See [CLI and build outputs tasks](../tasks/15_cli_and_build_outputs.md).
