# Phase boundaries

The compiler's main architecture rule is that every phase has one owner
and one output contract. That keeps correctness local. When a bug appears
in generated Wasm, the compiler can ask which invariant was broken instead
of treating the whole pipeline as one large transformation.

This phase discipline is the practical version of a pass pipeline. LLVM
divides passes into analyses, transformations, and utilities: analyses
compute information, transformations mutate the program, and utilities do
work that does not fit either category.[^llvm-passes] MLIR makes the same
distinction sharp by treating analyses as cached, non-mutating computations
and by letting passes signal failure so the remaining pipeline does not run
on invalid IR.[^mlir-pass]

## Parse

Parsing turns source text into a concrete syntax tree. Tree-sitter is
designed for concrete syntax trees that support editor use, incremental
parsing, and error recovery.[^tree-sitter]

Regulus uses that tree as input, but it does not let every later phase
depend on tree-sitter node names. The concrete tree is too close to parser
details.

### Error recovery and resilient parsing

Most production parsers do not stop at the first syntax error. GCC, Clang,
and tree-sitter all implement some form of error recovery: the parser
inserts a synthetic token, skips to a known synchronization point such as
the next statement or closing brace, and continues.[^clang-diag]

Tree-sitter's recovery is designed for editors, where a half-written
function should still return a usable tree for the rest of the file. The
`ERROR` node marks the unrecoverable region; everything outside it remains
well-structured.[^tree-sitter]

Regulus surfaces tree-sitter `ERROR` nodes as diagnostics with source
spans. The AST builder rejects fragments that span an `ERROR` node rather
than silently producing a broken AST. That choice keeps later phases from
operating on structurally invalid input.

## AST build

The AST builder converts concrete syntax into compiler-owned Rust data:

```text
tree-sitter node -> ast::Expression
tree-sitter node -> ast::Declaration
tree-sitter node -> ast::Pattern
```

The AST preserves source spans, names, literal source text, annotations,
and raw syntax for constructs that are parsed but not fully supported yet.
This is the first representation later phases should depend on.

AST building is a transformation pass: it changes representation and
discards parser-only details. It must preserve the source details that
later diagnostics need, especially spans and source text for literals and
annotations.

### Diagnostics as first-class output

Every phase produces diagnostics alongside its primary output. A parse
error carries a source span. A resolution error names the missing symbol
and the span where it was referenced. A type error describes the expected
type and the actual type at the conflicting expression.

Diagnostics are not afterthoughts. The `Diagnostic` type is defined in
`crates/core/src/diagnostic.rs` and carries a severity, a message, and an
optional source span. Phases that can report multiple independent errors
do so: a single compile call on a file with three type errors should report
all three, not just the first.

This design mirrors Clang's philosophy of recovering from errors to produce
more diagnostics, rather than aborting at the first error.[^clang-diag]
Compilers that stop at the first error force users into a slow
edit-compile-fix loop. The boundary is that once a phase fails, it does
not pass its output to the next phase.

## Resolve

Name resolution turns textual references into known declarations. It
handles lexical scopes, imports, module-qualified references, namespaces,
visibility, constructors, fields, and pattern bindings.

Resolution output is a `ResolvedModule`: the AST plus a symbol table and
reference data. It does not decide whether calling a value is type-correct.
It only answers what the name refers to.

Resolution is mostly an analysis over the AST. It can report errors, and
it can produce symbol/reference tables, but it should not change expression
meaning. That separation keeps type checking from depending on textual
lookup rules.

## Type check

Type checking consumes resolved syntax and produces `TypedModule`. It
validates calls, operators, records, constructors, patterns, branches, and
annotations. It also builds `ModuleInterface`, which carries function
types, type declarations, and constructor metadata for imports and later
phases.

The typed output is the compiler's semantic boundary. Lowering should not
need to parse type annotation strings or infer local variable types.

Type checking is also where module-interface data becomes trustworthy. A
later module should import a checked function type, not an unverified
annotation string.

### Hindley–Milner and unification

Gleam uses a Hindley–Milner type system with parametric polymorphism.
Type inference works by generating equality constraints between types and
solving them by unification: two types are unified by finding a
substitution that makes them equal.[^hm] Where a type variable appears,
the substitution determines its final type.

Regulus's type checker implements inference variables, substitutions,
generalization, and an occurs check (to prevent infinite types). Unification
runs during expression checking, and the final types are read back from the
substitution. Generalization happens at function definitions to allow
polymorphic calls.

## Lower

Lowering turns typed syntax into core IR. This is where source-level
constructs become explicit runtime work:

- parameters and bindings become locals
- blocks become ordered instructions with a result
- calls become direct or indirect call nodes
- pattern matches become branch clauses and successful bindings
- assert patterns become explicit failure paths

A small example shows the difference. The Gleam block:

```gleam
let y = x + 1
y
```

becomes in core IR (in pseudocode):

```text
local y: Int
set y, add(get x, 1)
result get y
```

The `let` keyword, the block scoping rules, and the implicit final
expression are all gone. The IR records the local, the explicit write, and
the explicit read.

Lowering still does not choose final machine instructions. It creates a
smaller compiler IR that the WebAssembly backend can emit deterministically.

### Explicit failure paths

Gleam's `let assert` and `panic` have observable runtime behavior: they
halt the program with a message. Core IR makes that explicit:

```text
assert_match subject, pattern, on_failure: Panic("bad match")
```

The backend emits a conditional branch to a failure block that calls the
runtime's panic helper. The failure path is visible in the IR rather than
being a special case invented during code generation.

## Emit

WebAssembly emission turns core IR into WAT and then binary bytes. The
core specification defines validation and execution over modules, types,
instructions, and the text and binary formats.[^wasm-spec]

This phase owns target representation: scalar ABI values, managed-value
pointers, linear memory operations, imports, exports, runtime helpers, and
target diagnostics for shapes the backend cannot emit.

Emission should verify its own target invariants. LLVM encourages front
ends to verify generated IR before optimization because malformed IR can
crash later tools.[^llvm-passes] Regulus gets a similar guardrail from WAT
assembly and WebAssembly validation: bad stack types, invalid imports, or
unsupported ABI shapes should fail before an artifact is treated as
executable.

### The runtime prelude

When a compiled module uses managed values—strings, lists, tuples, records,
custom types, closures—the emitter inserts a runtime prelude before the
user functions. The prelude defines linear memory, a bump-allocation heap
pointer, the allocator, and helper functions for value construction,
comparison, and inspection.

The prelude is not source-level Gleam. It is compiler-owned WAT code that
belongs to the backend phase, not the front end. Keeping it here means the
front end never makes assumptions about memory layout or allocation
strategy.

## Future optimization phases

Regulus does not yet have a dedicated optimization phase. As the middle end
matures, a sensible next layer would include:

- **Dead-code elimination**: remove unreachable locals and functions.
- **Constant folding**: evaluate constant expressions at compile time.
- **Inlining**: replace a call with the callee's body for small functions.
- **Monomorphization**: specialize generic functions for their concrete
  type arguments to avoid indirect dispatch.

These passes would operate on core IR, not on the AST. That separation is
why a middle-end boundary exists even before the passes do.

[^tree-sitter]: Tree-sitter documentation, "Introduction": https://tree-sitter.github.io/tree-sitter/
[^wasm-spec]: WebAssembly Core Specification: https://webassembly.github.io/spec/core/
[^llvm-passes]: LLVM, "Analysis and Transform Passes": https://llvm.org/docs/Passes.html
[^mlir-pass]: MLIR, "Pass Management": https://mlir.llvm.org/docs/PassManagement/
[^clang-diag]: Clang documentation, "Diagnostics": https://clang.llvm.org/docs/DiagnosticsReference.html
[^hm]: Damas and Milner, "Principal type-schemes for functional programs", POPL 1982: https://dl.acm.org/doi/10.1145/582153.582176
