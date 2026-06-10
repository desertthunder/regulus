# Artifacts and execution

The compiler should produce artifacts that are useful both to users and
to people debugging the compiler. The normal output is executable
WebAssembly. The debug outputs expose intermediate representations so a
failure can be traced to the phase that introduced it.

This mirrors the way mature compiler stacks expose intermediate artifacts.
LLVM includes utility passes for writing modules, printing graphs,
stripping debug data, verifying IR, and other non-transform work.[^llvm-passes]
Regulus does not need that breadth, but it does need artifacts that make
the pipeline auditable.

## CLI compile path

The CLI currently reads one source file and runs the same phase order as
`compile_source`:

```text
parse -> AST -> resolve -> type check -> IR -> Wasm
```

On success, it writes a `.wasm` file. With `--wat`, it also writes the
generated WebAssembly text. With `--dump-dir`, it writes:

```text
ast.txt
resolved.txt
typed.txt
ir.txt
wat.wat
```

Those dumps are intentionally phase-shaped. They are not a second compiler
API, but they make it possible to inspect what each phase believed about
the source.

### Reading a dump

A concrete example shows the value. Given this Gleam source:

```gleam
pub fn add(a: Int, b: Int) -> Int {
  a + b
}
```

the `ir.txt` dump would show something close to:

```text
function add [export]
  params: a:Int, b:Int
  locals: []
  body: BinOp(Add, Local(a), Local(b))
```

and the `wat.wat` dump would show:

```wat
(func $add (export "add")
      (param $0 i64) (param $1 i64) (result i64)
  local.get $0
  local.get $1
  i64.add
)
```

If the emitted Wasm fails a test, comparing `ir.txt` and `wat.wat` shows
whether the bug is in lowering (IR is wrong) or in emission (IR is right
but WAT is wrong).

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

## Snapshot testing and determinism

Snapshot tests compare a compiler output against a stored expected output.
When the output changes, the test fails: the developer inspects the diff,
decides whether the change is correct, and updates the snapshot.

Snapshot tests are only useful if the output is **deterministic**: the
same source must always produce the same WAT, regardless of hash-map
iteration order, threading, or system clock. Regulus achieves determinism
by:

- emitting functions, constants, and imports in stable declaration order
- using sorted or insertion-ordered maps for symbol tables
- generating local names from stable IDs (`$0`, `$1`) rather than from
  hash-based identifiers

This is the same approach used by LLVM's `FileCheck`-based tests: outputs
are compared against annotated expected patterns, and any change to the
emitted instructions shows up immediately as a test failure.[^llvm-filecheck]

## Execution boundary

Compilation ends at a WebAssembly module. Running that module requires a
host. In tests, Wasmtime provides the host: it compiles module bytes,
instantiates the module, supplies imports where needed, and exposes
exports that Rust tests can call.[^wasmtime]

This distinction matters for architecture. The compiler should emit a
valid module and a documented ABI. It should not hide host behavior inside
earlier phases. Host-specific adapters belong at target boundaries.

### Wasmtime as a test harness

Wasmtime provides a Rust API where a `Module` holds compiled code and an
`Instance` is the runtime object whose exports can be called. A typical
Wasm test in Regulus:

1. Compiles Gleam source to bytes.
2. Calls `Module::new(&engine, &bytes)` to validate and compile the module.
3. Calls `Instance::new(&mut store, &module, &[])` to instantiate it.
4. Acquires a typed export with `instance.get_typed_func`.
5. Calls the function and asserts the result.

Wasmtime validates the binary format and type-checks the Wasm before step
3. If the emitted bytes fail validation, the test fails at step 2 with a
specific validation error—a stronger check than only inspecting WAT.

## Diagnostics and exit codes

Each phase can fail with diagnostics. A parse error should not continue
into AST building. A type error should not lower. A Wasm assembly error
should not write a binary artifact.

The CLI follows that shape: it prints diagnostics and returns failure when
a phase fails, and writes artifacts only after compilation succeeds.

## Test strategy

The narrowest useful test should own each invariant:

- parser and AST tests for source shape
- resolver tests for names and visibility
- type-checker tests for type rules
- IR tests for explicit evaluation order and locals
- WAT snapshots for deterministic backend output
- Wasmtime tests for executable behavior

End-to-end tests are valuable, but they should not be the only tests. A
broken phase is easier to fix when its own representation has focused
tests. This is the same principle Rust's test suite follows: the compiler
has unit tests for individual analyses, integration tests for the CLI,
and UI tests that snapshot error messages.[^rustc-tests]

[^wasmtime]: Wasmtime Rust API documentation: https://docs.wasmtime.dev/api/wasmtime/
[^llvm-passes]: LLVM, "Analysis and Transform Passes": https://llvm.org/docs/Passes.html
[^llvm-filecheck]: LLVM, "FileCheck - Flexible pattern matching file verifier": https://llvm.org/docs/CommandGuide/FileCheck.html
[^rustc-tests]: Rust Compiler Development Guide, "Running tests": https://rustc-dev-guide.rust-lang.org/tests/intro.html
