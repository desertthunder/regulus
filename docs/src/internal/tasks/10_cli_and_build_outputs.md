# CLI and build outputs tasks

## Goal

Make the CLI compile projects and produce useful artifacts.

## Tasks

- [ ] Add project compile command using `gleam.toml`.
- [ ] Add output path configuration.
- [ ] Write `.wasm` artifacts.
- [ ] Add optional WAT output.
- [ ] Add optional AST, resolved AST, typed output, and IR debug dumps.
- [ ] Add target selection for supported runtimes.
- [ ] Render diagnostics with source snippets.
- [ ] Add CLI integration tests.

## Done when

A user can run one command against a Gleam project and receive a `.wasm` file or
clear diagnostics.
