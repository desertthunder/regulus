# CLI and build outputs

Current single-file CLI behavior is documented in [CLI and build outputs][cli].
This spec tracks the remaining user-facing command and artifact work that is not
specific to project linking.

Project compilation itself is defined in [Project compilation and
dependencies](./14_project_compilation_and_dependencies.md).

[cli]: ../../website/reference/cli-and-build-outputs.md

## Responsibilities

The CLI should provide predictable commands and stable artifacts for both users
and compiler contributors.

Remaining responsibilities:

- render diagnostics with source snippets
- choose target and host profiles explicitly
- keep normal output concise
- make debug output opt-in
- write deterministic artifacts
- avoid partial final artifacts after failed compilation
- expose enough metadata for host adapters and tests

## Commands

Single-file compilation remains useful for fixtures and small examples. Project
compilation should reuse the same flags where possible.

Important command concerns:

- output path selection
- optional WAT output
- optional debug dump directory
- target selection for Wasmtime, browser, and WASI
- JS host profile selection for browser, bundler, and Node.js
- clear exit codes for success, diagnostics, and command misuse

## Artifacts

Suggested outputs:

- final `.wasm`
- optional `.wat`
- optional AST, resolved, typed, IR, and WAT dumps
- optional runtime layout and ABI metadata
- optional import/export metadata for host adapters
- deterministic JS host adapter files when requested

Artifact paths should be stable enough for examples, snapshots, and host smoke
tests.

## Diagnostics

Diagnostics should be rendered for humans by default and remain structured
enough for tests.

The CLI should support:

- file paths
- source snippets
- labels
- notes
- stable multi-module ordering
- stage-specific unsupported-feature messages
- clear messages for target and ABI mismatches

## Backend output model

The backend now builds a compiler-owned Wasm module and can emit bytes directly.
CLI output should treat WAT as a rendered debug artifact, not as the source of
truth for byte emission.

Any remaining helper-backed modules should eventually use direct structured
byte emission or checked precompiled fragments.

## Active tasks

See [CLI and build outputs tasks](../tasks/17_cli_and_build_outputs.md).
