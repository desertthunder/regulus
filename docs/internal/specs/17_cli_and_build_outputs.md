# CLI and build outputs

Current single-file CLI behavior is documented in [CLI and build outputs][cli].
This spec tracks the remaining user-facing command and artifact work that is not
specific to project linking.

Project compilation itself is defined in the website development design record:
[Project compilation and dependencies][projects].

[cli]: ../../website/reference/cli-and-build-outputs.md
[projects]: ../../website/development/projects.md

## Responsibilities

The CLI should provide predictable commands and stable artifacts for both users
and compiler contributors.

Remaining responsibilities:

- keep normal output concise
- make debug output opt-in
- write deterministic artifacts
- avoid partial final artifacts after failed compilation
- expose enough metadata for host adapters and tests

## Commands

Single-file compilation remains useful for fixtures and small examples. Project
compilation reuses the same output, target, WAT, emit, dump, verbose, and JSON
reservation flags where possible.

Current command surface:

- `build [project]` compiles a Gleam project.
- `compile <input>` compiles one source file.
- `run <input>` compiles one source file and executes an export with Wasmtime.
- `exec` is an alias for `run`.
- `debug`/`dbg` inspect compiler-internal views.
- `list [project]` prints discovered project modules.

`build` and `compile` both support output path selection, optional WAT output,
optional debug dump directories, target selection, and explicit artifact
selection. `run` and `exec` support Wasmtime execution for scalar arguments and
ABI-aware rendering of scalar and managed return values.

Commands should continue to return clear exit codes for success, diagnostics,
and command misuse.

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

Diagnostics are rendered for humans by default and remain structured enough for
tests.

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

Helper-backed modules should continue to assemble through checked helper
fragments before they are included in final output.

## Active tasks

See [CLI and build outputs tasks](../tasks/17_cli_and_build_outputs.md).
