# Artifacts and execution

The compiler should produce artifacts that are useful both to users and to
people debugging the compiler. The normal output is executable WebAssembly. The
debug outputs expose intermediate representations so a failure can be traced to
the phase that introduced it.

This mirrors the way mature compiler stacks expose intermediate artifacts.
LLVM includes utility passes for writing modules, printing graphs, stripping
debug data, verifying IR, and other non-transform work.[^llvm-passes] Regulus
does not need that breadth, but it does need artifacts that make the pipeline
auditable.

## CLI compile path

The CLI currently reads one source file and runs the same phase order as
`compile_source`:

```text
parse -> AST -> resolve -> type check -> IR -> Wasm
```

On success, it writes a `.wasm` file. With `--wat`, it also writes the generated
WebAssembly text. With `--dump-dir`, it writes:

```text
ast.txt
resolved.txt
typed.txt
ir.txt
wat.wat
```

Those dumps are intentionally phase-shaped. They are not a second compiler API,
but they make it possible to inspect what each phase believed about the source.

## Binary output

The backend returns `WasmModule`:

```rust
pub struct WasmModule {
    pub wat: String,
    pub bytes: Vec<u8>,
}
```

Keeping both forms is useful. WAT is human-readable and stable enough for
snapshots. Binary `.wasm` is what runtimes validate and execute.

## Execution boundary

Compilation ends at a WebAssembly module. Running that module requires a host.
In tests, Wasmtime provides the host: it compiles module bytes, instantiates the
module, supplies imports where needed, and exposes exports that Rust tests can
call.[^wasmtime]

This distinction matters for architecture. The compiler should emit a valid
module and a documented ABI. It should not hide host behavior inside earlier
phases. Host-specific adapters belong at target boundaries.

## Diagnostics and exit codes

Each phase can fail with diagnostics. A parse error should not continue into
AST building. A type error should not lower. A Wasm assembly error should not
write a binary artifact.

The CLI follows that shape: it prints diagnostics and returns failure when a
phase fails, and writes artifacts only after compilation succeeds.

## Test strategy

The narrowest useful test should own each invariant:

- parser and AST tests for source shape
- resolver tests for names and visibility
- type-checker tests for type rules
- IR tests for explicit evaluation order and locals
- WAT snapshots for deterministic backend output
- Wasmtime tests for executable behavior

End-to-end tests are valuable, but they should not be the only tests. A broken
phase is easier to fix when its own representation has focused tests.

[^wasmtime]: Wasmtime Rust API documentation: https://docs.wasmtime.dev/api/wasmtime/
[^llvm-passes]: LLVM, "Analysis and Transform Passes": https://llvm.org/docs/Passes.html
