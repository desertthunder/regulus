# CLI and build outputs tasks

## Goal

Make compiler commands predictable and generated artifacts useful.

## Tasks

### Commands

- [x] Keep single-file compilation available for tests and examples.
- [x] Add output path configuration.
- [x] Add target selection for supported runtimes and host profiles: Wasmtime,
      browser, bundler, Node.js, and WASI where implemented.
- [x] Return useful exit codes for success, diagnostics, and command misuse.
- [x] Keep project compilation flags consistent with single-file compilation.
- [x] Add CLI `run`/`exec` (aliases for one another) integration tests for Wasmtime execution.
- [x] Add ABI-aware rendering for managed `run` return values such as strings,
      tuples, lists, records, `Result`, and `Option`.

### Artifacts

- [x] Write `.wasm` artifacts.
- [x] Add optional WAT output.
- [x] Add optional AST, resolved AST, typed output, and IR debug dumps.
- [x] Emit Wasm bytes from a structured module without going through WAT.
- [x] Add deterministic WAT snapshots generated from structured Wasm.
- [x] Emit deterministic `.mjs` adapter files when Wasm output is requested for
      browser, bundler, and Node.js targets.
- [x] Expose JS host import and export metadata needed by checked host calls.
- [x] Add optional runtime layout and ABI debug output where helpful.
- [x] Include import and export metadata in explicit debug output where useful.
- [x] Emit or package deterministic browser and Node.js host adapter files when
      requested.

### Backend cleanup

- [x] Disable silent fallback from structured codegen to the old WAT emitter.
- [x] Keep unsupported IR as source-spanned `WasmError` diagnostics.
- [x] Delete the old `Emitter` implementation and fallback-only tests.
- [ ] Give helper-backed modules direct structured byte emission or checked
      precompiled fragments.

### Diagnostics and user output

- [ ] Render diagnostics with source snippets, labels, notes, and file paths.
- [ ] Group diagnostics across project modules in a stable order.
- [ ] Improve type-inference diagnostics for local generalization, ambiguous
      types, recursive types, constructor fields, and branch mismatches.
- [x] Show unsupported-feature diagnostics from AST, resolver, type, lowering,
      backend, stdlib, and ABI stages without losing source spans.
- [x] Keep normal compile output concise; make debug output opt-in.
- [ ] Add human-readable messages for missing project files, duplicate modules,
      unsupported exports, and backend target mismatches.

### Integration tests

- [x] Add CLI integration tests for project compilation commands.
- [x] Add CLI integration tests for diagnostics across multiple files.
- [x] Add tests for output path handling and optional WAT/debug artifacts.
- [ ] Add tests for target selection and unsupported target combinations.
- [x] Add tests for browser and Node.js JS host profile output.
- [x] Add tests for bundler host adapter artifact paths.

## Done when

Users get concise command output, deterministic artifacts, and clear
source-rendered diagnostics for both single-file and project compilation.
