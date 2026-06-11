# Pass pipelines and invariants

A compiler pass is a unit of work over a program representation. Some
passes compute information. Some transform the program. Some write
artifacts or dumps. The architecture question is not "how many passes
should there be?" It is "what must be true before and after each pass?"

LLVM's pass documentation divides passes into analysis, transform, and
utility categories. Analysis passes compute information other passes can
use. Transform passes mutate the program. Utility passes perform work that
is neither pure analysis nor a normal program transformation.[^llvm-passes]

Regulus is small, but the same categories already fit:

| Regulus work    | Category                             |
| --------------- | ------------------------------------ |
| Name resolution | analysis over AST                    |
| Type checking   | analysis plus checked annotations    |
| Lowering        | transformation from typed AST to IR  |
| WAT emission    | artifact-producing backend pass      |
| Debug dumps     | utility output                       |

## Invariants

An invariant is a fact a phase promises to maintain. Examples:

- AST nodes have source spans.
- Resolved references point to known symbols.
- Typed expressions have a `Type`.
- IR locals have stable IDs and representation types.
- WAT output assembles into a valid WebAssembly module.

Invariants make tests precise. A resolver test should not need to run
Wasmtime. A WAT snapshot should not need to prove the parser accepts
every syntax form. Each test should verify the invariant owned by the
phase it targets.

### Representation invariants vs. semantic invariants

A representation invariant describes what the data structure looks like:
all `LocalId` values referenced in an IR body are declared in the same
function's locals list. A semantic invariant describes program meaning:
a typed function call has the same number of arguments as the callee's
parameter list.

Both kinds matter. A violation of a representation invariant usually
panics the compiler. A violation of a semantic invariant produces wrong
code. Separating them makes it easier to decide what kind of check
belongs where: representation checks can be assertions or debug-mode
verifiers; semantic checks belong in the relevant phase.

## SSA and data-flow invariants

Many production compilers add an **SSA (Static Single Assignment)**
invariant to their IR: every variable is defined exactly once, and every
use refers unambiguously to one definition. SSA makes classic data-flow
analyses—reaching definitions, liveness, constant propagation, and
dead-code elimination—simple to implement and fast to run.[^ssa]

When a CFG has a join point where two control paths meet, SSA introduces a
φ-function (`phi`) to pick the right definition:

```text
; simplified SSA pseudocode for: if cond { x = 1 } else { x = 2 }
block_then:
  x_1 = 1
block_else:
  x_2 = 2
block_merge:
  x_3 = phi(x_1 from block_then, x_2 from block_else)
```

LLVM IR and Rust's MIR are both in SSA form. Cranelift—the code generator
used by Wasmtime—also uses SSA for its block-based IR.[^cranelift]

Core IR in Regulus does not yet use SSA. Locals can be written multiple
times, and control flow is represented as structured branch expressions
rather than a CFG with explicit edges. The current form is simpler to
emit and read but limits what analyses can do without additional work.
Converting to SSA would be a middle-end transformation: it would accept
the current IR and produce an SSA form, without any changes to front-end
phases.

## Analysis preservation

As a compiler grows, analyses become expensive enough to cache or reuse.
MLIR's pass manager treats analyses as non-mutating computations that can
be cached and invalidated when transformations change the IR.[^mlir-pass]

Regulus does not need a pass manager yet, but it should still avoid mixing
analysis and mutation casually. For example:

- name resolution can build symbol data without lowering expressions
- type checking can build expression-type maps without choosing Wasm locals
- lowering can allocate locals without modifying the AST

That discipline keeps future caching and incremental compilation possible.
The Rust compiler's query system makes this explicit: every analysis is a
function from input to output, and the framework tracks which queries
depend on which inputs.[^rustc-dev] Regulus achieves the same discipline
structurally, by passing typed values between phases rather than mutating
shared state.

## Failure stops the pipeline

When a phase fails, later phases should not run. MLIR's pass infrastructure
lets a pass signal failure so no later passes execute on invalid
IR.[^mlir-pass]

Regulus follows the same rule with `Result<_, Diagnostics>`:

```rust
let cst = parse::parse(source)?;
let ast = ast::build(&cst)?;
let resolved = resolve::resolve(ast)?;
let typed = types::check(resolved)?;
let ir = ir::lower(typed)?;
let wasm = wasm::emit(&ir)?;
```

The `?` operator is architectural here. A parse error stops AST building.
A type error stops lowering. A Wasm emission error stops artifact writing.

### Collecting multiple errors

The early-exit rule applies at phase granularity, not per-diagnostic.
Within a phase, Regulus collects multiple errors where it can. A module
with three type errors should report all three. Only once the phase is
done does the `Result::Err` propagate and halt further phases.

This is the same strategy GCC and Clang use: gather as many diagnostics
as possible within a phase, then decide whether to continue into the next
phase based on severity.[^clang-diag]

## Dumpable boundaries

Every major representation should be dumpable:

- AST shows what syntax the compiler owns.
- Resolved output shows names, scopes, and references.
- Typed output shows expression and interface types.
- IR shows evaluation order, locals, calls, and failure paths.
- WAT shows backend code before binary encoding.

Dumpable boundaries are a maintainability tool. If a change breaks
execution, the dumps show where the program first became wrong. The same
mechanism is used by LLVM's `--print-before-all` and `--print-after-all`
flags, which dump IR before and after every pass to locate regressions.

## Tests as invariant checkers

A well-structured test suite is a form of invariant documentation. When
a resolver test asserts that a specific symbol resolves to a known
declaration, it is checking the resolver's invariant that unresolved names
are rejected. When a WAT snapshot test asserts that a specific function
compiles to a known WAT fragment, it is checking the backend's invariant
that a given IR maps to a specific instruction sequence.

This means invariants that matter enough to document also matter enough
to test, and tests that do not correspond to a phase invariant are probably
testing the wrong thing. A resolver test that runs Wasmtime is usually
testing too much; a backend test that checks whether a variable name
resolves is testing the wrong phase.

[^llvm-passes]: LLVM, "Analysis and Transform Passes": https://llvm.org/docs/Passes.html

[^mlir-pass]: MLIR, "Pass Management": https://mlir.llvm.org/docs/PassManagement/

[^ssa]: Braun et al., "Simple and Efficient Construction of Static Single Assignment Form", CC 2013: https://c9x.me/compile/bib/braun13cc.pdf

[^cranelift]: Cranelift Code Generator IR reference: https://github.com/bytecodealliance/wasmtime/blob/main/cranelift/docs/ir.md

[^rustc-dev]: Rust Compiler Development Guide, "Queries: demand-driven compilation": https://rustc-dev-guide.rust-lang.org/query.html

[^clang-diag]: Clang documentation, "Diagnostics Reference": https://clang.llvm.org/docs/DiagnosticsReference.html
