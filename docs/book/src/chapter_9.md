# Compiler architecture from source to executable

The compiler is a pipeline. Each phase accepts a representation with known
invariants, adds information or rejects invalid input, and hands a smaller
or more explicit representation to the next phase.

Most compiler texts and production compilers use some version of a front
end, middle end, and back end split. The front end understands the source
language. The middle end works on source-independent representations and
analyses. The back end emits target code or target-specific artifacts.
GCC's internals documentation[^gcc-passes] describes its compiler as a
sequence of parsing, gimplification, pass-manager, interprocedural, SSA,
RTL, and code-generation passes, while LLVM describes optimization as
passes that either compute information, transform the program, or provide
utilities.[^llvm-passes]

For a single source file, Regulus runs:

```text
SourceFile
  -> tree-sitter concrete syntax tree
  -> compiler-owned AST
  -> resolved module
  -> typed module
  -> core IR module
  -> WAT
  -> .wasm bytes
```

The public `compile_source` function in `crates/core/src/lib.rs` keeps
that ordering explicit: parse, build AST, resolve names, check types,
lower to IR, and emit WebAssembly.

## Why the pipeline is split

The phases are split because each one answers a different question:

| Phase      | Question                                            |
| ---------- | --------------------------------------------------- |
| Parse      | Is the text syntactically shaped like Gleam?        |
| AST build  | Which source constructs does this compiler own?     |
| Resolve    | Which declaration does each name refer to?          |
| Type check | Are values used with compatible types?              |
| Lower      | What exact runtime work must happen, in what order? |
| Emit Wasm  | How does that work map to WebAssembly?              |

A later phase should not redo an earlier phase's job. The WebAssembly
backend should not search lexical scopes. Lowering should not infer a
function's return type. Type checking should not decide heap object layout.
Keeping those boundaries sharp is what makes the compiler inspectable as
it grows.

This is also why Regulus has more than one internal representation.
Crafting Interpreters contrasts an AST-walking implementation with a
compiler that emits bytecode for a virtual machine; the bytecode path
exists because walking source syntax directly is the wrong execution model
once performance and runtime control matter.[^ci-bytecode] Regulus makes
a similar architectural move: source syntax is good for diagnostics and
language rules, while core IR and WAT are better for execution.

## Front end, middle end, back end

For Regulus, the split is:

| Layer      | Regulus phases                                   |
| ---------- | ------------------------------------------------ |
| Front end  | parse, AST build, name resolution, type checking |
| Middle end | core IR lowering and future IR analyses          |
| Back end   | WebAssembly text and binary emission             |

The boundary is not about about information. The front end is allowed to
know Gleam syntax, imports, types, and pattern rules. The middle end should
know evaluation order, locals, calls, and failure paths. The back end should
know WebAssembly value types, memory, tables, imports, exports, and ABI rules.

Regulus does not yet have an optimization-heavy middle end, but it already
has a middle-end boundary: `ir::Module`. That boundary is where future
passes can add canonicalization, dead-code checks, closure conversion,
pattern-match decision trees, or representation-aware rewrites without
touching parser code.

## Multiple levels of IR

Production compilers rarely carry a single IR from front to back. Instead
they lower progressively through several levels, each suited to a
different class of analysis or transformation.

**Clang / LLVM**: Clang parses C or C++ into an AST, then lowers to LLVM
IR. LLVM IR is the stable middle-end representation—typed, in static
single assignment form, and target-independent. Optimization passes run on
LLVM IR, and machine backends lower from LLVM IR to target code.[^llvm]

**Rust (`rustc`)**: Rust lowers through four IRs before machine code.
The AST is desugared into HIR (high-level IR) for name resolution and type
checking. HIR is lowered to THIR (typed HIR) for pattern exhaustiveness.
THIR is lowered to MIR (mid-level IR) for borrow checking, data-flow
analyses, and optimizations. MIR is then lowered to LLVM IR or
Cranelift.[^rustc-dev]

**MLIR**: The Multi-Level IR framework lets compiler projects define their
own dialects—each with typed operations, regions, and verifiers—and lower
progressively. A front end can stay high-level while the middle progressively
closes the gap to the target without a single monolithic IR design.[^mlir]

Regulus uses two owned levels: the typed AST and core IR. WAT is a
human-readable target form that the `wat` crate assembles into binary
WebAssembly. Core IR is intentionally small today, but the design leaves
room for future IR levels: for example, a lower-level IR with explicit
control-flow graphs would support dead-code elimination and constant
propagation without touching the front end.

## Control flow graphs and SSA

Most production middle ends organize code into a **control-flow graph
(CFG)**: a graph of basic blocks, where each block is a straight-line
sequence of instructions ending in a branch or return. Edges represent
possible flow from one block to the next.

**Static Single Assignment (SSA)** form adds the constraint that each
variable is defined exactly once in the CFG. Where control flow merges, a
φ-function selects a value depending on which predecessor was taken. SSA
makes data-flow analyses—constant propagation, dead-code elimination,
value numbering—efficient because def-use chains are explicit and
acyclic.[^ssa]

LLVM, GCC, and Rust's MIR all use SSA for their optimization phases.
Cranelift—the code generator inside Wasmtime—also represents functions in
SSA form and translates them to machine code via a register
allocator.[^cranelift]

Core IR in Regulus is not SSA, but the design is compatible with it.
Locals have stable IDs, blocks carry ordered instruction lists with
explicit results, and branches are structured. Converting to a CFG with
φ-functions would let the middle end verify liveness and eliminate
redundant local writes without touching parser or type-checker code.

## Incremental compilation

An incremental compiler avoids re-running phases on parts of the program
that have not changed. Rust's query system models each compiler phase as a
function that can be cached by its inputs; when a file changes, only the
queries whose inputs changed are re-evaluated.[^rustc-dev] The Salsa
library provides a reusable framework for this kind of demand-driven
incremental computation.[^salsa]

Regulus does not currently cache intermediate outputs between builds.
However, the design stays compatible with incremental compilation. Phase
outputs are typed values, not mutable global state. Module loading assigns
stable source IDs. Phase boundaries are clean enough that recomputing one
module does not implicitly touch another. Adding file-change tracking and
a phase cache would be an extension of the existing structure, not a
rewrite of it.

## Two entry points

Regulus supports a small single-file path and a project path.

The single-file path is used by `compile`, tests, and the CLI's current
compile command. It assigns one source ID, runs the full pipeline, and
returns a `WasmModule` containing WAT and binary bytes.

The project path reads `gleam.toml`, discovers modules under `src` and
`test`, assigns stable source IDs, and records dependency declarations.
That project model is the input needed for multi-module resolution and
type checking.

## Executable output

The backend emits WebAssembly text first because WAT is readable and
useful for snapshots. It then assembles WAT into `.wasm` bytes. The CLI
writes the binary artifact, optionally writes WAT, and can write debug
dumps for AST, resolved module, typed module, IR, and WAT.

Execution is a host concern. In tests, Wasmtime loads the emitted bytes,
instantiates the module, and calls exported functions. Browser and WASI
targets will use different host interfaces, but they should consume the
same checked and lowered core module where possible.

[^gcc-passes]: GCC Internals, "Passes and Files of the Compiler": https://gcc.gnu.org/onlinedocs/gccint/Passes.html

[^llvm-passes]: LLVM, "Analysis and Transform Passes": https://llvm.org/docs/Passes.html

[^ci-bytecode]: Robert Nystrom, _Crafting Interpreters_, "Chunks of Bytecode": https://craftinginterpreters.com/chunks-of-bytecode.html

[^llvm]: LLVM Language Reference Manual: https://llvm.org/docs/LangRef.html

[^rustc-dev]: Rust Compiler Development Guide, "Overview of the compiler": https://rustc-dev-guide.rust-lang.org/overview.html

[^mlir]: MLIR Documentation, "MLIR: A Compiler Infrastructure for the End of Moore's Law": https://mlir.llvm.org/

[^ssa]: Braun et al., "Simple and Efficient Construction of Static Single Assignment Form", CC 2013: https://c9x.me/compile/bib/braun13cc.pdf

[^cranelift]: Cranelift Code Generator, "Cranelift IR reference": https://github.com/bytecodealliance/wasmtime/blob/main/cranelift/docs/ir.md

[^salsa]: Salsa incremental computation framework: https://salsa-rs.github.io/salsa/
