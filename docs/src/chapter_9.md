# Compiler architecture from source to executable

The compiler is a pipeline. Each phase accepts a representation with known
invariants, adds information or rejects invalid input, and hands a smaller or
more explicit representation to the next phase.

Most compiler texts and production compilers use some version of a front end,
middle end, and back end split. The front end understands the source language.
The middle end works on source-independent representations and analyses. The
back end emits target code or target-specific artifacts. GCC's internals
documentation[^gcc-passes] describes its compiler as a sequence of parsing,
gimplification, pass-manager, interprocedural, SSA, RTL, and code-generation
passes, while LLVM describes optimization as passes that either compute
information, transform the program, or provide utilities[^llvm-passes].

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

The public `compile_source` function in `crates/core/src/lib.rs` keeps that
ordering explicit: parse, build AST, resolve names, check types, lower to IR,
and emit WebAssembly.

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

A later phase should not redo an earlier phase's job. The WebAssembly backend
should not search lexical scopes. Lowering should not infer a function's return
type. Type checking should not decide heap object layout. Keeping those
boundaries sharp is what makes the compiler inspectable as it grows.

This is also why Regulus has more than one internal representation. Crafting
Interpreters contrasts an AST-walking implementation with a compiler that emits
bytecode for a virtual machine; the bytecode path exists because walking source
syntax directly is the wrong execution model once performance and runtime
control matter.[^ci-bytecode] Regulus makes a similar architectural move:
source syntax is good for diagnostics and language rules, while core IR and WAT
are better for execution.

## Front end, middle end, back end

For Regulus, the split is:

| Layer      | Regulus phases                                   |
| ---------- | ------------------------------------------------ |
| Front end  | parse, AST build, name resolution, type checking |
| Middle end | core IR lowering and future IR analyses          |
| Back end   | WebAssembly text and binary emission             |

The boundary is not about file names. It is about information. The front end is
allowed to know Gleam syntax, imports, types, and pattern rules. The middle end
should know evaluation order, locals, calls, and failure paths. The back end
should know WebAssembly value types, memory, tables, imports, exports, and ABI
rules.

Regulus does not yet have an optimization-heavy middle end, but it already has a
middle-end boundary: `ir::Module`. That boundary is where future passes can add
canonicalization, dead-code checks, closure conversion, pattern-match decision
trees, or representation-aware rewrites without touching parser code.

## Two entry points

Regulus supports a small single-file path and a project path.

The single-file path is used by `compile`, tests, and the CLI's current compile
command. It assigns one source ID, runs the full pipeline, and returns a
`WasmModule` containing WAT and binary bytes.

The project path reads `gleam.toml`, discovers modules under `src` and `test`,
assigns stable source IDs, and records dependency declarations. That project
model is the input needed for multi-module resolution and type checking.

## Executable output

The backend emits WebAssembly text first because WAT is readable and useful for
snapshots. It then assembles WAT into `.wasm` bytes. The CLI writes the binary
artifact, optionally writes WAT, and can write debug dumps for AST, resolved
module, typed module, IR, and WAT.

Execution is a host concern. In tests, Wasmtime loads the emitted bytes,
instantiates the module, and calls exported functions. Browser and WASI targets
will use different host interfaces, but they should consume the same checked and
lowered core module where possible.

[^gcc-passes]: GCC Internals, "Passes and Files of the Compiler": https://gcc.gnu.org/onlinedocs/gccint/Passes.html

[^llvm-passes]: LLVM, "Analysis and Transform Passes": https://llvm.org/docs/Passes.html

[^ci-bytecode]: Robert Nystrom, _Crafting Interpreters_, "Chunks of Bytecode": https://craftinginterpreters.com/chunks-of-bytecode.html
