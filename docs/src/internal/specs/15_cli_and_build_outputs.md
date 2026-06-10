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

## Structured Wasm construction milestone

The backend currently emits WAT by appending text, then assembles it with
`wat::parse_str`. This keeps output readable, but it makes stack discipline,
function signatures, helper dependencies, import ordering, and local naming easy
to break with string edits. Wasmtime tests catch many mistakes after the fact,
but the backend should eventually build a typed Wasm representation first.

The milestone is complete when source programs lower to a compiler-owned Wasm
module model that can emit bytes directly and optionally print WAT only as a
rendered artifact. The model should make imports, functions, locals, memories,
data segments, exports, and helper dependencies explicit. It should validate
operand-stack effects before byte emission, so backend bugs fail as structured
compiler diagnostics rather than WAT parse or Wasmtime translation errors.

The migration should be incremental. Keep textual WAT snapshots available while
introducing typed instructions, then move one codegen area at a time from string
printing to the structured builder. Runtime helpers can remain as checked WAT
blocks at first, but should become structured helper modules or precompiled
binary fragments before this milestone is considered complete.

## Usability

Commands should be boring and predictable. Debug output should be opt-in, and
normal compilation should focus on the final artifact and diagnostics.

## Active tasks

See [CLI and build outputs tasks](../tasks/15_cli_and_build_outputs.md).
