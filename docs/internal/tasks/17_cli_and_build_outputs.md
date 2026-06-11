# CLI and build outputs tasks

## Goal

Make compiler commands predictable and generated artifacts useful.

## Tasks

### Commands

- [x] Keep single-file compilation available for tests and examples.
- [x] Add output path configuration.
- [x] Add target selection for supported runtimes: Wasmtime, browser, and WASI
      where implemented.
- [x] Return useful exit codes for success, diagnostics, and command misuse.
- [ ] Add CLI target or profile selection for JS host profiles: browser,
      bundler, and Node.js.
- [ ] Keep project compilation flags consistent with single-file compilation.
- [ ] Add CLI `run` or `exec` integration tests for Wasmtime execution.
- [ ] Add ABI-aware rendering for managed `run` return values such as strings,
      tuples, lists, records, `Result`, and `Option`.

### Artifacts

- [x] Write `.wasm` artifacts.
- [x] Add optional WAT output.
- [x] Add optional AST, resolved AST, typed output, and IR debug dumps.
- [x] Emit Wasm bytes from a structured module without going through WAT.
- [x] Add deterministic WAT snapshots generated from structured Wasm.
- [ ] Add optional runtime layout and ABI debug output where helpful.
- [ ] Include import and export metadata needed by host adapters.
- [ ] Emit stable artifact paths for example host adapters.
- [ ] Decide whether JS glue is emitted, copied from checked templates, or
      documented as a stable handwritten adapter.
- [ ] Emit or package deterministic JS host adapter files when requested.

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

- [ ] Add CLI integration tests for project compilation commands.
- [ ] Add CLI integration tests for diagnostics across multiple files.
- [ ] Add tests for output path handling and optional WAT/debug artifacts.
- [ ] Add tests for target selection and unsupported target combinations.
- [ ] Add tests for browser, bundler, and Node.js JS host profile output.
- [ ] Add tests for stable host adapter artifact paths.

## Done when

Users get concise command output, deterministic artifacts, and clear
source-rendered diagnostics for both single-file and project compilation.
