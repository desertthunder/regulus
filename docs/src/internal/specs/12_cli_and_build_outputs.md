# CLI and build outputs

The CLI should compile projects and produce artifacts that are useful for users,
tests, and contributors inspecting the compiler.

## Responsibilities

- Compile a Gleam project from `gleam.toml`.
- Compile single files for small tests and examples.
- Write `.wasm` output.
- Optionally write WAT, AST, resolved AST, typed output, and IR dumps.
- Choose target settings such as Wasmtime, browser, or WASI where supported.
- Render diagnostics with source snippets.
- Return useful exit codes for automation.

## Artifacts

Suggested outputs:

- `module.wasm`
- `module.wat`
- debug dumps under a configurable directory
- test snapshots for compiler-owned representations

## Usability

Commands should be boring and predictable. Debug output should be opt-in, and
normal compilation should focus on the final artifact and diagnostics.
