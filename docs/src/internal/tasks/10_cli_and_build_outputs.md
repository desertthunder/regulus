# CLI and build outputs tasks

## Goal

Make the CLI compile projects and produce useful artifacts.

## Tasks

### Commands and inputs

- [ ] Add project compile command using `gleam.toml`.
- [x] Keep single-file compilation available for tests and examples.
- [x] Add output path configuration.
- [x] Add target selection for supported runtimes: Wasmtime, browser, and WASI
      where implemented.
- [ ] Add package/dependency discovery flags or configuration once dependency
      metadata is supported.
- [x] Return useful exit codes for success, diagnostics, and command misuse.

### Artifacts

- [x] Write `.wasm` artifacts.
- [x] Add optional WAT output.
- [x] Add optional AST, resolved AST, typed output, and IR debug dumps.
- [ ] Add optional runtime layout and ABI debug output where helpful.
- [ ] Keep generated artifact names deterministic for multi-module projects.
- [ ] Avoid writing partial final artifacts after a failed compile unless the
      user explicitly requested debug dumps.

### Diagnostics and user output

- [ ] Render diagnostics with source snippets, labels, notes, and file paths.
- [ ] Group diagnostics across project modules in a stable order.
- [x] Show unsupported-feature diagnostics from AST, resolver, type, lowering,
      backend, stdlib, and ABI stages without losing source spans.
- [x] Keep normal compile output concise; make debug output opt-in.
- [ ] Add human-readable messages for missing project files, duplicate modules,
      unsupported exports, and backend target mismatches.

### Integration tests

- [ ] Add CLI integration tests for successful project compilation.
- [ ] Add CLI integration tests for diagnostics across multiple files.
- [ ] Add tests for output path handling and optional WAT/debug artifacts.
- [ ] Add tests for target selection and unsupported target combinations.
- [ ] Add tests that compile fixtures using records, custom types, pattern
      matching, managed values, stdlib calls, and host imports as those stages
      become available.

## Done when

A user can run one command against a Gleam project and receive a `.wasm` file or
clear source-rendered diagnostics, with optional deterministic debug artifacts for
compiler contributors.
