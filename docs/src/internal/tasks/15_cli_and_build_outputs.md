# CLI and build outputs tasks

## Goal

Make the CLI compile projects and produce useful artifacts.

## Tasks

### Commands and inputs

- [ ] Add project compile command using `gleam.toml` that emits linked Wasm.
- [ ] Run parse, target selection, resolution, type checking, lowering, and
      Wasm emission across all project modules.
- [x] Keep single-file compilation available for tests and examples.
- [x] Add output path configuration.
- [x] Add target selection for supported runtimes: Wasmtime, browser, and WASI
      where implemented.
- [ ] Decide whether Cloudflare Workers use a dedicated target or browser host
      profile.
- [ ] Add package/dependency discovery flags or configuration once dependency
      metadata is supported.
- [ ] Load enough dependency metadata for project compile inputs.
- [ ] Load dependency module interfaces for values, types, constructors, and
      labels used by project compilation.
- [ ] Add a dependency source loader for selected Hex and path modules.
- [ ] Add a dependency-source compile fixture that links one package module
      without modeling the package function as an intrinsic or host import.
- [ ] Report unsupported dependency calls before lowering or byte emission.
- [x] Return useful exit codes for success, diagnostics, and command misuse.

### Artifacts

- [x] Write `.wasm` artifacts.
- [x] Add optional WAT output.
- [x] Add optional AST, resolved AST, typed output, and IR debug dumps.
- [ ] Add optional runtime layout and ABI debug output where helpful.
- [ ] Keep generated artifact names deterministic for multi-module projects.
- [ ] Link multi-module project output in dependency order.
- [ ] Link same-project module calls without treating them as host imports.
- [ ] Link compiled dependency module calls without treating them as host
      imports.
- [ ] Keep dependency interface calls distinct from host imports in debug dumps.
- [ ] Emit stable artifact paths for example host adapters.
- [ ] Avoid writing partial final artifacts after a failed compile unless the
      user explicitly requested debug dumps.

### Structured Wasm construction

- [x] Define compiler-owned Wasm module, import, function, local, memory,
      table, export, data segment, and custom-section data structures.
- [x] Define typed Wasm instruction enums with explicit operand-stack effects.
- [x] Add a validation pass for instruction stack effects, branch result types,
      local indices, function signatures, and call signatures.
- [x] Emit Wasm bytes from the structured module without going through WAT.
- [x] Keep optional WAT output by rendering the structured module, not by using
      handwritten backend strings as the source of truth.
- [x] Move scalar operations, direct calls, branches, locals, and exports to the
      structured builder.
- [x] Move managed-value allocation, pattern matching, closures, and indirect
      calls to the structured builder.
- [x] Move stdlib intrinsics, host imports, and target adapters to structured
      imports and calls.
- [x] Replace runtime helper WAT string blocks with structured helper modules or
      checked precompiled helper fragments.
- [x] Track helper dependencies explicitly so unused helpers are not emitted.
- [x] Rename `crates/core/src/wasm/helpers.rs` to `fragments.rs`
- [x] Split runtime fragments by domain under `crates/core/src/wasm/fragments/`
      using `*.wat.rs` modules for allocation, strings, lists, bit arrays,
      dictionaries, managed values, equality/ordering, panic, debug, and host
      adapters.
- [x] Keep fragment dependency metadata explicit after the split, with tests
      proving unused domain fragments are not emitted.
- [x] Add deterministic WAT snapshots generated from structured Wasm.
- [ ] Delete the fallback WAT emitter after porting the following to
      structured codegen:
  - [x] string export adapters
  - [x] bit-string pattern matching
  - [x] record updates
  - [x] dynamic decode helpers
  - [x] stdlib/runtime helper intrinsics
    - [x] Port simple structured cases: bit-array size predicates,
          bool text/compare, float compare, dynamic properties, and dynamic
          classify.
    - [x] Port remaining helper-backed cases: int/float string conversion,
          list helpers, bit-array append/concat/starts_with, and dictionaries.
    - [x] Add no-fallback structured tests for the ported stdlib/runtime
          intrinsics.
  - [x] Disable silent fallback from structured codegen to the WAT emitter.
  - [x] Keep unsupported IR as source-spanned `WasmError` diagnostics.
  - [x] Audit remaining `StructuredError::Unsupported` sites
    - [x] Port dynamic tuple/list/record literals with non-static fields.
    - [x] Port `BitArrayConcat`, `BitStringDeconstruct`, `ListDeconstruct`,
          `Failure`, and `MemoryOperation::Allocate` IR.
    - [x] Replace literal/static-value parse failures with source-spanned
          diagnostics.
    - [x] Replace missing scratch/allocation/dynamic local and signature lookups
          with checked invariants.
    - [x] Add tests that each unsupported residual IR path reports a precise
          diagnostic with a source span.
  - [x] Delete the old `Emitter` implementation and fallback-only tests.
  - [ ] Helper-backed modules need direct structured byte emission too.
- [x] Add tests that backend validation reports source-spanned diagnostics
      before byte emission for stack, signature, local, and target-adapter
      mistakes.

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

- [ ] Add CLI integration tests for successful project compilation.
- [ ] Add CLI integration tests for diagnostics across multiple files.
- [ ] Add tests for output path handling and optional WAT/debug artifacts.
- [ ] Add tests for target selection and unsupported target combinations.
- [ ] Add tests for Worker target or browser Worker-profile output.
- [ ] Add tests for dependency interface loading and unsupported dependencies.
- [ ] Add tests that compile fixtures using records, custom types, pattern
      matching, managed values, stdlib calls, and host imports as those stages
      become available.

## Done when

A user can run one command against a Gleam project and receive a `.wasm` file or
clear source-rendered diagnostics, with optional deterministic debug artifacts
for compiler contributors.
