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

## Project compilation milestone

Project compilation should run the same explicit phases as single-file
compilation, but across all modules in dependency order:

```text
load project -> parse -> target select -> resolve -> type check -> lower ->
link IR -> emit Wasm
```

The project path should preserve source IDs and file paths for diagnostics.
Same-project module calls should link without becoming host imports. Dependency
calls should either link to loaded dependency code or fail as unsupported
dependency interface calls before lowering.

The first milestone can load dependency interfaces without compiling dependency
source. That is enough for examples that keep dependency execution behind host
adapters or compiler-supported intrinsics. Later milestones should compile
selected dependency source when the supported language surface is broad enough.
That is the preferred path for stdlib and package behavior, including decoder,
routing, and response helper code that does not require a host primitive.

## Targets and host profiles

The CLI already accepts Wasmtime, browser, and WASI-oriented targets. Example
projects add a Cloudflare Workers use case. The build docs should either add a
Worker target or define a Worker host profile under the browser target, with
stable import module names and artifact paths for the JS Worker adapter.

Target-specific project compilation should validate target groups, external
imports, export ABI, and generated host adapter expectations before writing the
final artifact.

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
fragments while the builder API matures, but their shape should make that
remaining debt explicit.

Runtime WAT fragments are not the structured builder. The current helper source
should therefore be named for what it is: checked fragments. Rename
`crates/core/src/wasm/helpers.rs` to `fragments.rs`, then split it into domain
modules under `crates/core/src/wasm/fragments/`. Use `*.wat.rs` filenames for
these modules so readers can tell that the contents are WAT-backed Rust
constants, not native builder code. Expected domains include allocation, panic,
strings, lists, bit arrays, dictionaries, managed values, equality/ordering,
debug, and host adapters.

The split should preserve explicit dependency tracking. Codegen should request
helpers by stable fragment names, the fragment registry should resolve
transitive dependencies, and emission should include only reachable fragments.
Tests should cover both sides: a small program must not emit unrelated runtime
fragments, and a program using a helper family must still include all
transitive helper dependencies.

After the split, the next optional migration is to port one domain at a time
from checked WAT fragments to builder-native helper modules or checked
precompiled binary fragments. Each port should delete the corresponding WAT
fragment module once equivalent validation and snapshots exist.

## Usability

Commands should be boring and predictable. Debug output should be opt-in, and
normal compilation should focus on the final artifact and diagnostics.

## Active tasks

See [CLI and build outputs tasks](../tasks/15_cli_and_build_outputs.md).
