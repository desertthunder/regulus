# Pass pipelines and invariants

A compiler pass is a unit of work over a program representation. Some passes
compute information. Some transform the program. Some write artifacts or dumps.
The architecture question is not "how many passes should there be?" It is "what
must be true before and after each pass?"

LLVM's pass documentation divides passes into analysis, transform, and utility
categories. Analysis passes compute information other passes can use.
Transform passes mutate the program. Utility passes perform work that is neither
pure analysis nor a normal program transformation.[^llvm-passes]

Regulus is small, but the same categories already fit:

| Regulus work    | Category                            |
| --------------- | ----------------------------------- |
| Name resolution | analysis over AST                   |
| Type checking   | analysis plus checked annotations   |
| Lowering        | transformation from typed AST to IR |
| WAT emission    | artifact-producing backend pass     |
| Debug dumps     | utility output                      |

## Invariants

An invariant is a fact a phase promises to maintain. Examples:

- AST nodes have source spans.
- Resolved references point to known symbols.
- Typed expressions have a `Type`.
- IR locals have stable IDs and representation types.
- WAT output assembles into a valid WebAssembly module.

Invariants make tests precise. A resolver test should not need to run Wasmtime.
A WAT snapshot should not need to prove the parser accepts every syntax form.
Each test should verify the invariant owned by the phase it targets.

## Analysis preservation

As a compiler grows, analyses become expensive enough to cache or reuse. MLIR's
pass manager treats analyses as non-mutating computations that can be cached and
invalidated when transformations change the IR.[^mlir-pass]

Regulus does not need a pass manager yet, but it should still avoid mixing
analysis and mutation casually. For example:

- name resolution can build symbol data without lowering expressions
- type checking can build expression-type maps without choosing Wasm locals
- lowering can allocate locals without modifying the AST

That discipline keeps future caching and incremental compilation possible.

## Failure stops the pipeline

When a phase fails, later phases should not run. MLIR's pass infrastructure lets
a pass signal failure so no later passes execute on invalid IR.[^mlir-pass]

Regulus follows the same rule with `Result<_, Diagnostics>`:

```rust
let cst = parse::parse(source)?;
let ast = ast::build(&cst)?;
let resolved = resolve::resolve(ast)?;
let typed = types::check(resolved)?;
let ir = ir::lower(typed)?;
let wasm = wasm::emit(&ir)?;
```

The `?` operator is architectural here. A parse error stops AST building. A type
error stops lowering. A Wasm emission error stops artifact writing.

## Dumpable boundaries

Every major representation should be dumpable:

- AST shows what syntax the compiler owns.
- Resolved output shows names, scopes, and references.
- Typed output shows expression and interface types.
- IR shows evaluation order, locals, calls, and failure paths.
- WAT shows backend code before binary encoding.

Dumpable boundaries are a maintainability tool. If a change breaks execution,
the dumps show where the program first became wrong.

[^llvm-passes]: LLVM, "Analysis and Transform Passes": https://llvm.org/docs/Passes.html

[^mlir-pass]: MLIR, "Pass Management": https://mlir.llvm.org/docs/PassManagement/
